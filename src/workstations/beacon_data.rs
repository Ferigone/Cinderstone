//! Java 1.21 BeaconBlockEntity ContainerData and power-field persistence.
//!
//! Direct pinned JDK21 javap: dqc$1 get92–96/set102–112 and dqc
//! filterEffect122/loadEffect300–309/loadAdditional314–324/saveAdditional328–339.
//! The actual BE owns these fields; this module creates no registry, payment,
//! living effect, text, LockCode, advancement or persistent block-entity owner.
use super::{
    beacon_block_entity::{BeaconAction, BeaconTransactionHost},
    beacon_effects::{BeaconSound, BeaconSoundHost},
};
use crate::{
    connection_cookies::CookieKey,
    nbt::{JavaString, Tag},
};
use anyhow::{ensure, Result};

pub const PRIMARY_EFFECT_KEY: &str = "primary_effect";
pub const SECONDARY_EFFECT_KEY: &str = "secondary_effect";
pub const LEVELS_KEY: &str = "Levels";

/// The same actual built-in mob-effect holders used by this BE's fields.
/// No numeric ID, name table or reconstructed holder substitutes for them.
pub trait BeaconDataHost: BeaconTransactionHost + BeaconSoundHost {
    /// BuiltInRegistries.MOB_EFFECT.asHolderIdMap().getId(holder), including
    /// its source -1 result for an unmapped actual holder.
    fn builtin_effect_id(&self, effect: &Self::Effect) -> Result<i32>;
    /// The actual holder IdMap.byId; out-of-range returns None. Missing registry
    /// context is an error, never a successful unresolved lookup.
    fn builtin_effect_by_id(&mut self, id: i32) -> Result<Option<Self::Effect>>;
    fn builtin_effect_by_key(&mut self, key: &CookieKey) -> Result<Option<Self::Effect>>;
    /// Actual holder.unwrapKey().map(key.location()); a direct/unkeyed holder
    /// returns None and source storeEffect does not remove an existing tag key.
    fn builtin_effect_key(&self, effect: &Self::Effect) -> Result<Option<CookieKey>>;
    /// Actual canonical VALID_EFFECTS set membership, not an effect tier test
    /// or comparison of unproven registry IDs. Clone preserves holder identity.
    fn is_valid_beacon_effect(&mut self, effect: &Self::Effect) -> Result<bool>;
}

/// Source ContainerData getter: raw levels, ID+1 powers, otherwise zero.
/// This is distinct from SetBeaconPacket's optional direct registry-ID codec.
pub fn data_get<H: BeaconDataHost>(host: &H, index: i32) -> Result<i32> {
    match index {
        0 => Ok(host.levels()),
        1 => encode_effect(host, host.primary()),
        2 => encode_effect(host, host.secondary()),
        _ => Ok(0),
    }
}

pub fn data_values<H: BeaconDataHost>(host: &H) -> Result<[i32; 3]> {
    Ok([data_get(host, 0)?, data_get(host, 1)?, data_get(host, 2)?])
}

fn encode_effect<H: BeaconDataHost>(host: &H, effect: Option<&H::Effect>) -> Result<i32> {
    effect.map_or(Ok(0), |effect| {
        Ok(host.builtin_effect_id(effect)?.wrapping_add(1))
    })
}

fn decode_effect<H: BeaconDataHost>(host: &mut H, encoded: i32) -> Result<Option<H::Effect>> {
    if encoded == 0 {
        Ok(None)
    } else {
        host.builtin_effect_by_id(encoded.wrapping_sub(1))
    }
}

fn filter_effect<H: BeaconDataHost>(
    host: &mut H,
    effect: Option<H::Effect>,
) -> Result<Option<H::Effect>> {
    match effect {
        Some(effect) if host.is_valid_beacon_effect(&effect)? => Ok(Some(effect)),
        _ => Ok(None),
    }
}

/// Atomic standalone ContainerData setter. Unknown signed indices are a source
/// no-op and do not begin/publish a transaction or consume an owner revision.
pub fn set_data<H: BeaconDataHost>(host: &mut H, index: i32, value: i32) -> Result<()> {
    if !(0..=2).contains(&index) {
        return Ok(());
    }
    let action = BeaconAction::SetData { index, value };
    let mut staged = host.begin_stage(action)?;
    set_data_in_stage(&mut staged, index, value)?;
    host.publish_stage(staged, action)
}

/// Source setter inside an already certified actual menu/BE transaction. This
/// never publishes separately or repeats the menu's later blockEntityChanged.
pub fn set_data_in_stage<H: BeaconDataHost>(host: &mut H, index: i32, value: i32) -> Result<()> {
    if !(0..=2).contains(&index) {
        return Ok(());
    }
    host.require_actual_stage(BeaconAction::SetData { index, value })?;
    match index {
        0 => host.set_levels_raw(value),
        1 => {
            // Raw committed beam list, including when levels == 0. Source
            // sound precedes lookup/filter/assignment even for unchanged,
            // absent or invalid powers, so callbacks observe the old primary.
            if !host.is_client_side() && !host.scan().committed_sections().is_empty() {
                host.play_sound(BeaconSound::PowerSelect)?;
            }
            let decoded = decode_effect(host, value)?;
            let filtered = filter_effect(host, decoded)?;
            host.set_primary_raw(filtered);
        }
        2 => {
            let decoded = decode_effect(host, value)?;
            let filtered = filter_effect(host, decoded)?;
            host.set_secondary_raw(filtered);
        }
        _ => unreachable!(),
    }
    Ok(())
}

/// The ResourceLocation character rules are all ASCII. Inspect UTF-16 units
/// without lossy replacement, then reuse the existing native parse semantics
/// (default namespace, first colon, allowed empty path). The 65535-unit bound
/// is native NBT-string admission, not an additional Java ResourceLocation rule.
fn effect_key(value: &JavaString) -> Result<Option<CookieKey>> {
    ensure!(
        value.len_utf16() <= 65_535,
        "beacon effect key exceeds native NBT-string admission"
    );
    let mut ascii = String::with_capacity(value.len_utf16());
    for unit in value.iter_units() {
        if unit > 0x7f {
            return Ok(None);
        }
        ascii.push(char::from(unit as u8));
    }
    Ok(CookieKey::parse(&ascii).ok())
}

fn load_effect<H: BeaconDataHost>(host: &mut H, tag: &Tag, key: &str) -> Result<Option<H::Effect>> {
    let Some(Tag::String(value)) = tag.get(key) else {
        return Ok(None);
    };
    let Some(key) = effect_key(value)? else {
        return Ok(None);
    };
    let decoded = host.builtin_effect_by_key(&key)?;
    filter_effect(host, decoded)
}

/// Atomic PARTIAL load of the two power fields only. This does not execute a
/// full BeaconBlockEntity load, reset levels/scan state, parse Name/Lock or
/// process generic components. Full loaders use the in-stage form below.
pub fn load_effect_fields<H: BeaconDataHost>(host: &mut H, tag: &Tag) -> Result<()> {
    ensure!(
        matches!(tag, Tag::Compound(_)),
        "beacon effect load requires a compound"
    );
    let action = BeaconAction::LoadEffectFields;
    let mut staged = host.begin_stage(action)?;
    load_effect_fields_in_stage(&mut staged, tag)?;
    host.publish_stage(staged, action)
}

/// Outer actual full-BE load order is superclass -> THIS power helper ->
/// conditional actual CustomName JSON -> actual LockCode, then one publication.
/// No ContainerData setter/selection sound/dirty callback occurs on this path.
/// Source saves Levels but DOES NOT load it, including on an existing BE.
pub fn load_effect_fields_in_stage<H: BeaconDataHost>(host: &mut H, tag: &Tag) -> Result<()> {
    ensure!(
        matches!(tag, Tag::Compound(_)),
        "beacon effect load requires a compound"
    );
    host.require_actual_stage(BeaconAction::LoadEffectFields)?;
    let primary = load_effect(host, tag, PRIMARY_EFFECT_KEY)?;
    host.set_primary_raw(primary);
    let secondary = load_effect(host, tag, SECONDARY_EFFECT_KEY)?;
    host.set_secondary_raw(secondary);
    Ok(())
}

/// Append only source power strings then raw Levels to a caller's detached NBT
/// compound. Null/unkeyed powers leave existing keys untouched. Actual text,
/// LockCode, metadata/components and complete save/updateTag remain outer-owned:
/// superclass -> THIS helper -> optional CustomName -> Lock. The outer caller
/// holds the same captured BE/registry while serializing and publishes only the
/// completed document. A holder/codec error leaves the supplied tag unchanged.
pub fn save_effect_fields<H: BeaconDataHost>(host: &H, target: &mut Tag) -> Result<()> {
    ensure!(
        matches!(target, Tag::Compound(_)),
        "beacon effect save requires a compound"
    );
    let mut staged = target.clone();
    for (key, effect) in [
        (PRIMARY_EFFECT_KEY, host.primary()),
        (SECONDARY_EFFECT_KEY, host.secondary()),
    ] {
        if let Some(effect) = effect {
            if let Some(name) = host.builtin_effect_key(effect)? {
                staged.insert(key, Tag::String(name.as_str().into()))?;
            }
        }
    }
    staged.insert(LEVELS_KEY, Tag::Int(host.levels()))?;
    *target = staged;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        geometry::Aabb,
        random::LegacyRandomSource,
        workstations::{
            beacon_block_entity::{self, BeaconFields, BeaconScanState, BeaconTickHost},
            beacon_effects::{BeaconEffectApplication, BeaconEffectsHost},
        },
    };
    use anyhow::bail;
    use std::sync::Arc;

    type Effect = Arc<Option<CookieKey>>;

    /// Controlled native field/holder/query fixture, not a production Level,
    /// registry or living-effect owner. The ordinary native scan builds its
    /// raw beam list; unavailable player/effect/criterion queries always error.
    #[derive(Clone)]
    struct Fixture {
        scan: BeaconScanState,
        levels: i32,
        primary: Option<Effect>,
        secondary: Option<Effect>,
        holders: Arc<Vec<Effect>>,
        valid: Vec<Effect>,
        sound_random: LegacyRandomSource,
        events: Vec<String>,
        active_stage: Option<BeaconAction>,
        registry_available: bool,
        lookup_fault: bool,
        publications: usize,
    }

    impl Fixture {
        fn new() -> Result<Self> {
            let export: serde_json::Value = serde_json::from_str(include_str!(
                "../../fixtures/connection-mob-effects-1.21.json"
            ))?;
            let holders = Arc::new(
                export["entries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| {
                        assert_eq!(entry["id"].as_i64(), Some(index as i64));
                        Arc::new(Some(
                            CookieKey::parse(entry["name"].as_str().unwrap()).unwrap(),
                        ))
                    })
                    .collect::<Vec<_>>(),
            );
            let valid = [
                "speed",
                "haste",
                "resistance",
                "jump_boost",
                "strength",
                "regeneration",
            ]
            .into_iter()
            .map(|name| {
                holders
                    .iter()
                    .find(|holder| {
                        holder.as_ref().as_ref().unwrap().as_str() == format!("minecraft:{name}")
                    })
                    .unwrap()
                    .clone()
            })
            .collect();
            let mut fixture = Self {
                scan: BeaconScanState::default(),
                levels: 0,
                primary: Some(holders[0].clone()),
                secondary: None,
                holders,
                valid,
                sound_random: LegacyRandomSource::new(143),
                events: Vec::new(),
                active_stage: None,
                registry_available: true,
                lookup_fault: false,
                publications: 0,
            };
            // Controlled query profile: white beam block at y64, surface64,
            // time1. This exercises the native scan but does not prove actual
            // Level queries; periodic effect/criteria callbacks remain errors.
            beacon_block_entity::tick(&mut fixture)?;
            assert!(!fixture.scan.committed_sections().is_empty());
            fixture.events.clear();
            Ok(fixture)
        }
        fn id(&self, effect: &Effect) -> i32 {
            self.holders
                .iter()
                .position(|known| Arc::ptr_eq(known, effect))
                .map_or(-1, |index| index as i32)
        }
        fn current_primary(&self) -> Option<i32> {
            self.primary.as_ref().map(|e| self.id(e))
        }
    }

    impl BeaconFields for Fixture {
        type Effect = Effect;
        fn require_actual_stage(&self, action: BeaconAction) -> Result<()> {
            ensure!(
                self.active_stage == Some(action),
                "actual detached stage unavailable"
            );
            ensure!(self.registry_available, "actual registry owner unavailable");
            Ok(())
        }
        fn position(&self) -> (i32, i32, i32) {
            (0, 64, 0)
        }
        fn is_client_side(&self) -> bool {
            false
        }
        fn game_time(&self) -> i64 {
            1
        }
        fn min_build_height(&self) -> i32 {
            -64
        }
        fn dimension_height(&self) -> i32 {
            384
        }
        fn scan(&self) -> &BeaconScanState {
            &self.scan
        }
        fn scan_mut(&mut self) -> &mut BeaconScanState {
            &mut self.scan
        }
        fn levels(&self) -> i32 {
            self.levels
        }
        fn set_levels_raw(&mut self, levels: i32) {
            self.levels = levels;
        }
        fn primary(&self) -> Option<&Effect> {
            self.primary.as_ref()
        }
        fn secondary(&self) -> Option<&Effect> {
            self.secondary.as_ref()
        }
        fn set_primary_raw(&mut self, effect: Option<Effect>) {
            self.events
                .push(format!("primary:{:?}", effect.as_ref().map(|e| self.id(e))));
            self.primary = effect;
        }
        fn set_secondary_raw(&mut self, effect: Option<Effect>) {
            self.events.push(format!(
                "secondary:{:?}",
                effect.as_ref().map(|e| self.id(e))
            ));
            self.secondary = effect;
        }
        fn same_effect_holder(&self, left: &Effect, right: &Effect) -> Result<bool> {
            Ok(Arc::ptr_eq(left, right))
        }
    }
    impl BeaconTransactionHost for Fixture {
        fn begin_stage(&self, action: BeaconAction) -> Result<Self> {
            ensure!(self.active_stage.is_none(), "fixture already staged");
            let mut staged = self.clone();
            staged.active_stage = Some(action);
            staged.events.clear();
            Ok(staged)
        }
        fn publish_stage(&mut self, mut staged: Self, action: BeaconAction) -> Result<()> {
            staged.require_actual_stage(action)?;
            ensure!(
                Arc::ptr_eq(&self.holders, &staged.holders),
                "actual holder owner changed"
            );
            ensure!(
                self.active_stage.is_none(),
                "fixture publication is not live"
            );
            staged.active_stage = None;
            staged.publications += 1;
            *self = staged;
            Ok(())
        }
    }
    impl BeaconSoundHost for Fixture {
        fn play_sound(&mut self, sound: BeaconSound) -> Result<()> {
            ensure!(
                sound == BeaconSound::PowerSelect,
                "unexpected sound owner request"
            );
            self.events
                .push(format!("sound:{:?}", self.current_primary()));
            self.sound_random.next_i64();
            Ok(())
        }
    }
    impl BeaconDataHost for Fixture {
        fn builtin_effect_id(&self, effect: &Effect) -> Result<i32> {
            Ok(self.id(effect))
        }
        fn builtin_effect_by_id(&mut self, id: i32) -> Result<Option<Effect>> {
            self.events.push(format!("id:{id}"));
            ensure!(!self.lookup_fault, "injected detached lookup failure");
            Ok(usize::try_from(id)
                .ok()
                .and_then(|id| self.holders.get(id))
                .cloned())
        }
        fn builtin_effect_by_key(&mut self, key: &CookieKey) -> Result<Option<Effect>> {
            self.events.push(format!("key:{}", key.as_str()));
            Ok(self
                .holders
                .iter()
                .find(|holder| holder.as_ref().as_ref() == Some(key))
                .cloned())
        }
        fn builtin_effect_key(&self, effect: &Effect) -> Result<Option<CookieKey>> {
            Ok(effect.as_ref().clone())
        }
        fn is_valid_beacon_effect(&mut self, effect: &Effect) -> Result<bool> {
            self.events.push(format!("filter:{}", self.id(effect)));
            Ok(self.valid.iter().any(|known| Arc::ptr_eq(known, effect)))
        }
    }
    impl BeaconEffectsHost for Fixture {
        type Player = std::convert::Infallible;
        type ServerPlayer = std::convert::Infallible;
        fn players_in_effect_box(&mut self, _: Aabb) -> Result<Vec<Self::Player>> {
            bail!("no living query owner in this data fixture")
        }
        fn server_players_in_activation_box(&mut self, _: Aabb) -> Result<Vec<Self::ServerPlayer>> {
            bail!("no advancement query owner in this data fixture")
        }
        fn add_beacon_effect(
            &mut self,
            _: &Self::Player,
            _: BeaconEffectApplication<'_, Effect>,
        ) -> Result<()> {
            bail!("no living effect owner in this data fixture")
        }
        fn trigger_construct_beacon(&mut self, _: &Self::ServerPlayer, _: i32) -> Result<()> {
            bail!("no criterion owner in this data fixture")
        }
    }
    impl BeaconTickHost for Fixture {
        type BlockState = ();
        fn world_surface_height(&mut self, x: i32, z: i32) -> Result<i32> {
            ensure!((x, z) == (0, 0));
            Ok(64)
        }
        fn block_state(&mut self, position: (i32, i32, i32)) -> Result<()> {
            ensure!(
                position == (0, 64, 0),
                "outside captured data-fixture column"
            );
            Ok(())
        }
        fn beam_color(&self, _: &()) -> Result<Option<u32>> {
            Ok(Some(0xffff_ffff))
        }
        fn light_block(&mut self, _: &(), _: (i32, i32, i32)) -> Result<i32> {
            bail!("contextual opacity owner not needed by captured beam block")
        }
        fn is_bedrock(&self, _: &()) -> Result<bool> {
            bail!("unexpected bedrock query")
        }
        fn is_beacon_base(&self, _: &()) -> Result<bool> {
            bail!("unexpected base query")
        }
    }

    #[test]
    fn primary_sound_precedes_lookup_filter_at_zero_and_failed_stage_publishes_nothing(
    ) -> Result<()> {
        let mut host = Fixture::new()?;
        let seed_before = host.sound_random.state48();
        let mut expected_random = host.sound_random.clone();
        expected_random.next_i64();
        assert!(set_data_in_stage(&mut host, 1, 2).is_err());
        set_data(&mut host, 1, 2)?; // registered slowness is not VALID_EFFECTS
        assert_eq!(host.levels, 0);
        assert_eq!(
            host.events,
            ["sound:Some(0)", "id:1", "filter:1", "primary:None"]
        );
        assert_eq!(host.sound_random.state48(), expected_random.state48());
        assert_ne!(host.sound_random.state48(), seed_before);

        host.lookup_fault = true;
        let before = (
            host.current_primary(),
            host.sound_random.state48(),
            host.events.clone(),
            host.publications,
        );
        assert!(set_data(&mut host, 1, 1).is_err());
        assert_eq!(
            (
                host.current_primary(),
                host.sound_random.state48(),
                host.events.clone(),
                host.publications
            ),
            before
        );
        host.lookup_fault = false;
        host.registry_available = false;
        assert!(set_data(&mut host, 1, 1).is_err());
        assert_eq!(host.sound_random.state48(), before.1);
        let publications = host.publications;
        set_data(&mut host, -1, 42)?;
        assert_eq!(host.publications, publications);
        assert_eq!(data_get(&host, -1)?, 0);
        Ok(())
    }

    #[test]
    fn power_load_does_not_load_levels_and_partial_save_preserves_outer_fields() -> Result<()> {
        let mut host = Fixture::new()?;
        host.levels = -7;
        let random_before = host.sound_random.state48();
        let input = Tag::Compound(vec![
            (
                PRIMARY_EFFECT_KEY.into(),
                Tag::String(JavaString::Utf16(":haste".encode_utf16().collect())),
            ),
            (
                SECONDARY_EFFECT_KEY.into(),
                Tag::String("INVALID:key".into()),
            ),
            (LEVELS_KEY.into(), Tag::Int(900)),
            (
                "CustomName".into(),
                Tag::String("outer actual text remains outer-owned".into()),
            ),
        ]);
        load_effect_fields(&mut host, &input)?;
        assert_eq!(data_values(&host)?, [-7, 3, 0]);
        assert_eq!(
            host.events,
            [
                "key:minecraft:haste",
                "filter:2",
                "primary:Some(2)",
                "secondary:None"
            ]
        );
        assert_eq!(host.sound_random.state48(), random_before);

        let mut output = Tag::Compound(vec![
            (
                "CustomName".into(),
                Tag::String("retained outer name".into()),
            ),
            ("Lock".into(), Tag::String("fixture-lock".into())),
            (
                SECONDARY_EFFECT_KEY.into(),
                Tag::String("preserve-existing-when-null".into()),
            ),
        ]);
        save_effect_fields(&host, &mut output)?;
        assert_eq!(
            output.get(PRIMARY_EFFECT_KEY),
            Some(&Tag::String("minecraft:haste".into()))
        );
        assert_eq!(output.get(LEVELS_KEY), Some(&Tag::Int(-7)));
        assert_eq!(
            output.get("Lock"),
            Some(&Tag::String("fixture-lock".into()))
        );
        assert_eq!(
            output.get("CustomName"),
            Some(&Tag::String("retained outer name".into()))
        );
        assert_eq!(
            output.get(SECONDARY_EFFECT_KEY),
            Some(&Tag::String("preserve-existing-when-null".into()))
        );
        host.primary = Some(Arc::new(None)); // actual unmapped/unkeyed fixture holder
        assert_eq!(data_get(&host, 1)?, 0);
        save_effect_fields(&host, &mut output)?;
        assert_eq!(
            output.get(PRIMARY_EFFECT_KEY),
            Some(&Tag::String("minecraft:haste".into()))
        );

        set_data(&mut host, 1, i32::MIN)?;
        assert_eq!(
            host.events,
            ["sound:Some(-1)", "id:2147483647", "primary:None"]
        );
        let malformed = Tag::Compound(vec![(
            PRIMARY_EFFECT_KEY.into(),
            Tag::String(JavaString::Utf16(vec![0xd800])),
        )]);
        load_effect_fields(&mut host, &malformed)?;
        assert_eq!(host.events, ["primary:None", "secondary:None"]);
        assert_eq!(host.levels, -7);
        Ok(())
    }
}
