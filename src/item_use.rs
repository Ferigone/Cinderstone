//! LivingEntity item-use clock and the original default food/milk/honey paths.
//! The host owns inventory, synchronized flags, RNG, sounds and criteria. This
//! module never substitutes an unsupported item subclass callback with a no-op.
use crate::{
    inventory::{self, ItemStack},
    random::LegacyRandomSource,
};
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::{
    io,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
};

/// java.lang.Math.RandomNumberGeneratorHolder owns one process-wide Random.
/// Its constructor seed uniquifier is distinct from Minecraft RandomSupport.
#[derive(Clone)]
pub struct JdkMathRandom(Arc<Mutex<LegacyRandomSource>>);
static MATH_RANDOM: OnceLock<Arc<Mutex<LegacyRandomSource>>> = OnceLock::new();
static MATH_INITIALIZE: Mutex<()> = Mutex::new(());
static JDK_SEED_UNIQUIFIER: AtomicU64 = AtomicU64::new(8_682_522_807_148_012);
impl JdkMathRandom {
    pub fn fresh() -> io::Result<Self> {
        let _initialization = MATH_INITIALIZE
            .lock()
            .map_err(|_| io::Error::other("Math.random initialization poisoned"))?;
        if let Some(random) = MATH_RANDOM.get() {
            return Ok(Self(Arc::clone(random)));
        }
        let mut old = JDK_SEED_UNIQUIFIER.load(Ordering::SeqCst);
        let seed = loop {
            let next = old.wrapping_mul(1_181_783_497_276_652_981);
            match JDK_SEED_UNIQUIFIER.compare_exchange(
                old,
                next,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => {
                    break (next ^ crate::random_seed::system_monotonic_nanos()? as u64) as i64
                }
                Err(actual) => old = actual,
            }
        };
        let random = Arc::new(Mutex::new(LegacyRandomSource::new(seed)));
        MATH_RANDOM
            .set(Arc::clone(&random))
            .map_err(|_| io::Error::other("Math.random owner already initialized"))?;
        Ok(Self(random))
    }
    pub fn next_f64(&self) -> Result<f64> {
        Ok(self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("Math.random owner poisoned"))?
            .next_f64())
    }
    /// Holds the real shared stream throughout a synchronous callback. A real
    /// mutation prefix is retained if a later callback fails.
    pub fn with_random<T>(
        &self,
        call: impl FnOnce(&mut LegacyRandomSource) -> Result<T>,
    ) -> Result<T> {
        let mut random = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("Math.random owner poisoned"))?;
        call(&mut random)
    }
    pub fn state48(&self) -> Result<u64> {
        Ok(self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("Math.random owner poisoned"))?
            .state48())
    }
}

#[derive(Deserialize)]
struct Facts {
    minecraft: String,
    jar_sha256: String,
    items: Vec<(String, String, String, String, i32, i32)>,
    sounds: std::collections::BTreeMap<String, i32>,
    game_event_radii: std::collections::BTreeMap<String, i32>,
}
fn facts() -> &'static Facts {
    static DATA: OnceLock<Facts> = OnceLock::new();
    DATA.get_or_init(|| {
        let data: Facts = serde_json::from_str(include_str!("../fixtures/item-use-facts.json"))
            .expect("original1.21 item use dispatch");
        assert_eq!(data.minecraft, "1.21");
        assert_eq!(
            data.jar_sha256,
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
        );
        assert_eq!(data.items.len(), 1333);
        data
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseKind {
    Food,
    Milk,
    Honey,
}
#[derive(Clone, Debug)]
pub struct UseSpec {
    pub kind: UseKind,
    pub has_food: bool,
    pub duration: i32,
    pub nutrition: i32,
    pub saturation: f32,
    pub always_eat: bool,
    pub sound: i32,
}
impl UseSpec {
    pub fn for_stack(stack: &ItemStack) -> Result<Option<Self>> {
        Self::resolve(stack, false)
    }
    pub fn for_active_stack(stack: &ItemStack) -> Result<Option<Self>> {
        Self::resolve(stack, true)
    }
    fn resolve(stack: &ItemStack, allow_removed_food: bool) -> Result<Option<Self>> {
        if stack.is_empty() {
            return Ok(None);
        }
        let (use_owner, finish_owner, tick_owner, release_owner, drink, eat) = facts()
            .items
            .get(stack.item_id as usize)
            .context("unknown item-use dispatch")?;
        let kind = match (use_owner.as_str(), finish_owner.as_str()) {
            ("cvd", "cvd") => UseKind::Milk,
            ("cuf", "cuf") => UseKind::Honey,
            ("cul", "cul") => UseKind::Food,
            _ => return Ok(None),
        };
        ensure!(
            tick_owner == "cul" && release_owner == "cul",
            "nonempty item use/release callback requires actual owner"
        );
        // Milk overrides finishUsingItem and never calls Item's food path.
        let food = if kind == UseKind::Milk {
            None
        } else {
            stack.component_value(20)?
        };
        if kind == UseKind::Food && food.is_none() && !allow_removed_food {
            return Ok(None);
        }
        let has_food = food.is_some();
        let (nutrition, saturation, always_eat, seconds) = if let Some(food) = food {
            ensure!(
                food["effects"].as_array().is_some_and(Vec::is_empty),
                "food effects require actual active-effect owner"
            );
            let nutrition = food["nutrition"]
                .as_i64()
                .context("food nutrition missing")? as i32;
            let saturation = food["saturationModifier"]
                .as_f64()
                .context("food saturation missing")? as f32;
            let seconds = food["secondsToEat"]
                .as_f64()
                .context("food duration missing")? as f32;
            ensure!(
                seconds.is_finite() && seconds > 0. && saturation.is_finite(),
                "invalid food properties"
            );
            (
                nutrition,
                saturation,
                food["canAlwaysEat"]
                    .as_bool()
                    .context("food permission missing")?,
                seconds,
            )
        } else {
            (0, 0., false, 0.)
        };
        Ok(Some(Self {
            kind,
            has_food,
            duration: match kind {
                UseKind::Milk => 32,
                UseKind::Honey => 40,
                UseKind::Food => (seconds * 20.) as i32,
            },
            nutrition,
            saturation,
            always_eat,
            sound: if kind == UseKind::Food { *eat } else { *drink },
        }))
    }
    pub fn burp_sound() -> i32 {
        facts().sounds["uj"]
    }
    pub fn game_event_radius(name: &str) -> Result<i32> {
        facts()
            .game_event_radii
            .get(name)
            .copied()
            .context("unknown original item-use game event")
    }
}

#[derive(Clone, Debug)]
pub struct ActiveUse {
    pub hand: i32,
    pub item_id: i32,
    pub remaining: i32,
}
#[derive(Clone, Debug, Default)]
pub struct ItemUseClock {
    pub active: Option<ActiveUse>,
}
pub trait ItemUseHost {
    fn use_flags(&mut self, hand: i32, using: bool) -> Result<()>;
    fn game_event(&mut self, name: &'static str) -> Result<()>;
    fn using_criterion(&mut self, stack: &ItemStack) -> Result<()>;
    fn trigger_effects(&mut self, spec: &UseSpec, count: i32) -> Result<()>;
    fn complete_event(&mut self) -> Result<()>;
    fn finish(&mut self, stack: &ItemStack, spec: &UseSpec, hand: i32) -> Result<()>;
}
impl ItemUseClock {
    pub fn is_using(&self) -> bool {
        self.active.is_some()
    }
    pub fn start(
        &mut self,
        hand: i32,
        stack: &ItemStack,
        spec: &UseSpec,
        host: &mut impl ItemUseHost,
    ) -> Result<()> {
        ensure!((0..=1).contains(&hand), "invalid use hand");
        if stack.is_empty() || self.is_using() {
            return Ok(());
        }
        self.active = Some(ActiveUse {
            hand,
            item_id: stack.item_id,
            remaining: spec.duration,
        });
        host.use_flags(hand, true)?;
        host.game_event("minecraft:item_interact_start")
    }
    pub fn stop(&mut self, host: &mut impl ItemUseHost) -> Result<()> {
        if let Some(active) = self.active.take() {
            host.use_flags(active.hand, false)?;
            host.game_event("minecraft:item_interact_finish")?;
        }
        Ok(())
    }
    pub fn tick(
        &mut self,
        current: &ItemStack,
        spec: Option<&UseSpec>,
        host: &mut impl ItemUseHost,
    ) -> Result<()> {
        let Some(active) = &mut self.active else {
            return Ok(());
        };
        // ItemStack.isSameItem compares item identity, not count/components.
        // LivingEntity then rebinds useItem to this actual current hand stack.
        if current.is_empty() || current.item_id != active.item_id {
            return self.stop(host);
        }
        let spec = spec.context("active item changed to unsupported use callback")?;
        host.using_criterion(current)?;
        let elapsed = spec.duration.wrapping_sub(active.remaining);
        let threshold = (spec.duration as f32 * 0.21875_f32) as i32;
        if elapsed > threshold && active.remaining % 4 == 0 {
            host.trigger_effects(spec, 5)?;
        }
        active.remaining = active.remaining.wrapping_sub(1);
        if active.remaining == 0 {
            let hand = active.hand;
            host.complete_event()?;
            host.trigger_effects(spec, 16)?;
            host.finish(current, spec, hand)?;
            self.stop(host)?;
        }
        Ok(())
    }
}

pub fn item_id(name: &str) -> Result<i32> {
    static IDS: OnceLock<std::collections::BTreeMap<String, i32>> = OnceLock::new();
    IDS.get_or_init(|| {
        (0..1333)
            .filter_map(|id| inventory::item_name(id).map(|n| (n.to_owned(), id)))
            .collect()
    })
    .get(name.strip_prefix("minecraft:").unwrap_or(name))
    .copied()
    .with_context(|| format!("unknown item {name}"))
}
