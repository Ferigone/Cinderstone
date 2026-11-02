//! Java 1.21 BeaconBlockEntity's incremental beam, pyramid and tick phases.
//!
//! This kernel borrows the actual block entity and Level through a mandatory
//! host. It neither registers a BE nor supplies a second payment, power, living
//! effect, registry, criterion or sound/RNG owner.

use super::beacon_effects::{
    activate_live, activate_staged, apply_effects_live, apply_effects_staged, BeaconEffectsHost,
    BeaconSound,
};
use anyhow::{ensure, Result};

/// Source packed DyeColor texture-diffuse ARGB and Java-int section height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BeaconBeamSection {
    pub color: u32,
    pub height: i32,
}

/// The actual BE's transient beam fields. They are not part of its saved NBT.
///
/// Default matches the Java constructor: two empty lists and lastCheckY=0.
/// The real setLevel operation subsequently calls `after_set_level`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BeaconScanState {
    committed: Vec<BeaconBeamSection>,
    checking: Vec<BeaconBeamSection>,
    last_check_y: i32,
}

impl BeaconScanState {
    /// Raw committed list, including while levels=0. ContainerData's primary
    /// setter observes this list rather than the levels-filtered beam getter.
    pub fn committed_sections(&self) -> &[BeaconBeamSection] {
        &self.committed
    }

    pub fn checking_sections(&self) -> &[BeaconBeamSection] {
        &self.checking
    }

    /// Source getBeamSections checks equality with zero, not positivity.
    pub fn visible_sections(&self, actual_levels: i32) -> &[BeaconBeamSection] {
        if actual_levels == 0 {
            &[]
        } else {
            &self.committed
        }
    }

    pub fn last_check_y(&self) -> i32 {
        self.last_check_y
    }

    /// Call only AFTER the outer actual BlockEntity.setLevel attached its real
    /// Level. Source changes no other beam or power field here.
    pub fn after_set_level(&mut self, actual_min_build_height: i32) {
        self.last_check_y = actual_min_build_height.wrapping_sub(1);
    }
}

/// Borrow the SAME actual BE fields used by its existing menu ContainerData.
///
/// Every scalar is from a captured, admitted actual BE/Level binding. Admission
/// must reject unavailable context; these infallible getters permit no guessed
/// world, side, time, height, position, level or holder defaults. Effect clones
/// retain the same actual canonical holder reference, never a manufactured ID.
/// Raw setters are field assignments only, with no filtering/sound/dirty call.
pub trait BeaconFields {
    type Effect: Clone;

    /// Validate the actual owner's detached-stage capability for this action,
    /// binding/lifetime/version and captured context. No unconditional success,
    /// local boolean substitute or permissive default is a production proof.
    /// Pure field getters may read live owners; public mutating staged helpers
    /// require this proof before their first mutation or callback.
    fn require_actual_stage(&self, action: BeaconAction) -> Result<()>;
    fn position(&self) -> (i32, i32, i32);
    fn is_client_side(&self) -> bool;
    fn game_time(&self) -> i64;
    fn min_build_height(&self) -> i32;
    fn dimension_height(&self) -> i32;
    fn scan(&self) -> &BeaconScanState;
    fn scan_mut(&mut self) -> &mut BeaconScanState;
    fn levels(&self) -> i32;
    fn set_levels_raw(&mut self, levels: i32);
    fn primary(&self) -> Option<&Self::Effect>;
    fn secondary(&self) -> Option<&Self::Effect>;
    fn set_primary_raw(&mut self, effect: Option<Self::Effect>);
    fn set_secondary_raw(&mut self, effect: Option<Self::Effect>);
    fn same_effect_holder(&self, left: &Self::Effect, right: &Self::Effect) -> Result<bool>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeaconAction {
    Tick,
    /// Source ContainerData uses signed Java-int indices; unknown indices have
    /// its source zero/no-op behavior rather than becoming unsigned indices.
    SetData {
        index: i32,
        value: i32,
    },
    LoadEffectFields,
}

/// Transaction for an explicitly proven RESTRICTED staged domain only.
///
/// Arbitrary live actor/effect graphs, source callbacks, constructor acquisition
/// and rewards do not become detachable merely by implementing this trait. A
/// live-only owner may honestly reject `require_actual_stage` and need not
/// implement this transaction trait; it uses `BeaconLiveOwner` instead.
///
/// `begin_stage` must detach all mutable state: BE, Level/tag/registry context,
/// living effects, criterion rewards, threadSafeRandom and ordered outbox. A
/// shallow clone with live aliases, global-factory/RNG rewind, guessed empty
/// effects/criteria or callbacks escaping the stage is not an implementation.
/// `publish_stage` validates the same actual BE lifetime/version and captured
/// world/tag/registry owners, then atomically adopts the stage exactly once.
/// Any fallible publication preparation precedes mutation of live owners. It
/// must not drain packets, reenter callbacks or save before the outer owner lock
/// releases. Returned callback observations never authorize a second replay.
/// Independently acquired constructor/factory entropy cannot be rolled back by
/// this stage. Its actual owner must admit and persist the genuine acquisition
/// at its independent boundary, without rewinding a global factory or inventing
/// a substitute object. This trait does not promise global rollback.
pub trait BeaconTransactionHost: BeaconFields + Sized {
    fn begin_stage(&self, action: BeaconAction) -> Result<Self>;
    fn publish_stage(&mut self, staged: Self, action: BeaconAction) -> Result<()>;
}

/// Owner-certified access to the actual retained BE and original call context.
///
/// The actual arena/owner retains the SAME canonical BE through this call,
/// including removal, replacement at its position and reentrant mutations.
/// Source field accesses keep using that retained object; they must not resolve
/// a current BE index, reused ID or replacement at the same block position.
/// The original tick Level and BlockPos argument leases are also retained:
/// reattaching the BE to another Level must not redirect queries or callbacks.
///
/// This is an actual owner capability, not a caller Boolean or kernel-created
/// counter. Rechecking it validates that retained capability, not a mutable
/// version: the call's own mutations, reentry or removal do not make it stale.
/// Admission validates required real world/tag/registry/effect/criterion/sound
/// owners before mutation. There is no staged clone, rollback, deferred replay,
/// global factory rewind or automatic retry after a source prefix executes.
pub trait BeaconLiveOwner: BeaconFields {
    fn require_actual_live(&self, action: BeaconAction) -> Result<()>;
}

/// Real contextual world queries, in the source's short-circuit order.
///
/// `BlockState` is the actual state observed at the queried position. The beam
/// predicate uses its real Block's BeaconBeamBlock interface and DyeColor's
/// texture-diffuse ARGB. Opacity uses BlockState.getLightBlock(Level,pos), not a
/// guessed static opacity; base membership uses the captured BEACON_BASE_BLOCKS
/// tag, and bedrock uses the actual canonical Block identity. Missing chunks,
/// tags, dynamic light owners or other unsupported context must return errors.
/// Admission binds an actual valid Level block position whose four-block base
/// extent does not overflow Java int coordinates. Arbitrary i32-endpoint tuples
/// outside that native world domain are unsupported; offset iteration below is
/// equivalent to source endpoint loops within the admitted coordinate domain.
/// Queries are certified read-only/nonreentrant. Arbitrary custom query reentry
/// that mutates beam lists requires a retained current-section owner and is not
/// claimed by this Vec representation.
pub trait BeaconTickHost: BeaconTransactionHost + BeaconEffectsHost {
    type BlockState;

    fn world_surface_height(&mut self, x: i32, z: i32) -> Result<i32>;
    fn block_state(&mut self, position: (i32, i32, i32)) -> Result<Self::BlockState>;
    fn beam_color(&self, state: &Self::BlockState) -> Result<Option<u32>>;
    fn light_block(&mut self, state: &Self::BlockState, position: (i32, i32, i32)) -> Result<i32>;
    fn is_bedrock(&self, state: &Self::BlockState) -> Result<bool>;
    fn is_beacon_base(&self, state: &Self::BlockState) -> Result<bool>;
}

/// Live queries of the retained BE's ORIGINAL tick Level/BlockPos context.
///
/// The state/interface/light/tag query contracts and admitted coordinates are
/// the same as `BeaconTickHost`, but no transaction implementation is required.
/// Queries must be actual read-only, nonreentrant source world observations.
/// Java retains a current beam-section object local while scanning; this Vec
/// kernel does not claim arbitrary custom query callbacks that mutate/reenter
/// its beam lists. Effect, sound and criterion callbacks may reenter and mutate
/// the actual retained owners at their source phases.
pub trait BeaconLiveTickHost: BeaconLiveOwner + BeaconEffectsHost {
    type BlockState;

    fn world_surface_height(&mut self, x: i32, z: i32) -> Result<i32>;
    fn block_state(&mut self, position: (i32, i32, i32)) -> Result<Self::BlockState>;
    fn beam_color(&self, state: &Self::BlockState) -> Result<Option<u32>>;
    fn light_block(&mut self, state: &Self::BlockState, position: (i32, i32, i32)) -> Result<i32>;
    fn is_bedrock(&self, state: &Self::BlockState) -> Result<bool>;
    fn is_beacon_base(&self, state: &Self::BlockState) -> Result<bool>;
}

/// Execute a SERVER tick ONLY in an explicitly proven restricted staged domain.
/// That owner must actually support detachment and atomic publication of every
/// reached mutation/callback. Ordinary live effects, RNG, rewards or reentry are
/// not made rollbackable by Result; use `tick_live` for live source semantics.
/// This entry does not register a ticker, attach a Level, mark dirty or save.
pub fn tick<H: BeaconTickHost>(host: &mut H) -> Result<()> {
    ensure!(
        !host.is_client_side(),
        "beacon server tick requires an actual server Level"
    );
    let mut staged = host.begin_stage(BeaconAction::Tick)?;
    staged.require_actual_stage(BeaconAction::Tick)?;
    ensure!(
        !staged.is_client_side(),
        "beacon staged tick changed actual Level side"
    );
    tick_source::<H, StagedTickAccess>(&mut staged)?;
    host.publish_stage(staged, BeaconAction::Tick)
}

/// Execute one live SERVER tick on the SAME canonical retained BE. Admission
/// precedes mutation. A later error keeps ALL completed source mutations and
/// callbacks; callers must not roll back, replay the call or automatically retry.
/// No begin/publish stage, host clone, packet batch or replacement BE is made.
pub fn tick_live<H: BeaconLiveTickHost>(host: &mut H) -> Result<()> {
    host.require_actual_live(BeaconAction::Tick)?;
    ensure!(
        !host.is_client_side(),
        "beacon live server tick requires an actual server Level"
    );
    tick_source::<H, LiveTickAccess>(host)
}

// Static dispatch adapters share one source algorithm without forwarding the
// complete field/owner host or forcing a live owner to implement transactions.
trait TickAccess<H: BeaconFields> {
    type BlockState;
    fn world_surface_height(host: &mut H, x: i32, z: i32) -> Result<i32>;
    fn block_state(host: &mut H, position: (i32, i32, i32)) -> Result<Self::BlockState>;
    fn beam_color(host: &H, state: &Self::BlockState) -> Result<Option<u32>>;
    fn light_block(
        host: &mut H,
        state: &Self::BlockState,
        position: (i32, i32, i32),
    ) -> Result<i32>;
    fn is_bedrock(host: &H, state: &Self::BlockState) -> Result<bool>;
    fn is_beacon_base(host: &H, state: &Self::BlockState) -> Result<bool>;
    fn apply_effects(
        host: &mut H,
        levels: i32,
        primary: Option<&H::Effect>,
        secondary: Option<&H::Effect>,
    ) -> Result<()>;
    fn play_sound(host: &mut H, sound: BeaconSound) -> Result<()>;
    fn activate(host: &mut H) -> Result<()>;
}

struct StagedTickAccess;
impl<H: BeaconTickHost> TickAccess<H> for StagedTickAccess {
    type BlockState = H::BlockState;
    fn world_surface_height(host: &mut H, x: i32, z: i32) -> Result<i32> {
        host.world_surface_height(x, z)
    }
    fn block_state(host: &mut H, position: (i32, i32, i32)) -> Result<Self::BlockState> {
        host.block_state(position)
    }
    fn beam_color(host: &H, state: &Self::BlockState) -> Result<Option<u32>> {
        host.beam_color(state)
    }
    fn light_block(
        host: &mut H,
        state: &Self::BlockState,
        position: (i32, i32, i32),
    ) -> Result<i32> {
        host.light_block(state, position)
    }
    fn is_bedrock(host: &H, state: &Self::BlockState) -> Result<bool> {
        host.is_bedrock(state)
    }
    fn is_beacon_base(host: &H, state: &Self::BlockState) -> Result<bool> {
        host.is_beacon_base(state)
    }
    fn apply_effects(
        host: &mut H,
        levels: i32,
        primary: Option<&H::Effect>,
        secondary: Option<&H::Effect>,
    ) -> Result<()> {
        apply_effects_staged(host, levels, primary, secondary)
    }
    fn play_sound(host: &mut H, sound: BeaconSound) -> Result<()> {
        host.play_sound(sound)
    }
    fn activate(host: &mut H) -> Result<()> {
        activate_staged(host)
    }
}

struct LiveTickAccess;
impl<H: BeaconLiveTickHost> TickAccess<H> for LiveTickAccess {
    type BlockState = H::BlockState;
    fn world_surface_height(host: &mut H, x: i32, z: i32) -> Result<i32> {
        host.world_surface_height(x, z)
    }
    fn block_state(host: &mut H, position: (i32, i32, i32)) -> Result<Self::BlockState> {
        host.block_state(position)
    }
    fn beam_color(host: &H, state: &Self::BlockState) -> Result<Option<u32>> {
        host.beam_color(state)
    }
    fn light_block(
        host: &mut H,
        state: &Self::BlockState,
        position: (i32, i32, i32),
    ) -> Result<i32> {
        host.light_block(state, position)
    }
    fn is_bedrock(host: &H, state: &Self::BlockState) -> Result<bool> {
        host.is_bedrock(state)
    }
    fn is_beacon_base(host: &H, state: &Self::BlockState) -> Result<bool> {
        host.is_beacon_base(state)
    }
    fn apply_effects(
        host: &mut H,
        levels: i32,
        primary: Option<&H::Effect>,
        secondary: Option<&H::Effect>,
    ) -> Result<()> {
        apply_effects_live(host, levels, primary, secondary)
    }
    fn play_sound(host: &mut H, sound: BeaconSound) -> Result<()> {
        host.play_sound(sound)
    }
    fn activate(host: &mut H) -> Result<()> {
        activate_live(host)
    }
}

fn tick_source<H: BeaconFields, A: TickAccess<H>>(host: &mut H) -> Result<()> {
    let (x, y, z) = host.position();
    let mut scan_y = if host.scan().last_check_y < y {
        let scan = host.scan_mut();
        scan.checking.clear();
        scan.last_check_y = y.wrapping_sub(1);
        y
    } else {
        host.scan().last_check_y.wrapping_add(1)
    };
    let surface_y = A::world_surface_height(host, x, z)?;

    for _ in 0..10 {
        if scan_y > surface_y {
            break;
        }
        let position = (x, scan_y, z);
        let block = A::block_state(host, position)?;
        if let Some(color) = A::beam_color(host, &block)? {
            let scan = host.scan_mut();
            if scan.checking.len() <= 1 {
                scan.checking.push(BeaconBeamSection { color, height: 1 });
            } else if let Some(section) = scan.checking.last_mut() {
                if section.color == color {
                    section.height = section.height.wrapping_add(1);
                } else {
                    let mixed = average_argb(section.color, color);
                    scan.checking.push(BeaconBeamSection {
                        color: mixed,
                        height: 1,
                    });
                }
            }
        } else {
            // Source does not query contextual opacity with no current section,
            // nor test BEDROCK when the light-block result is already below15.
            let passes = !host.scan().checking.is_empty()
                && (A::light_block(host, &block, position)? < 15 || A::is_bedrock(host, &block)?);
            let scan = host.scan_mut();
            if !passes {
                scan.checking.clear();
                scan.last_check_y = surface_y;
                break;
            }
            if let Some(section) = scan.checking.last_mut() {
                section.height = section.height.wrapping_add(1);
            }
        }
        scan_y = scan_y.wrapping_add(1);
        let scan = host.scan_mut();
        scan.last_check_y = scan.last_check_y.wrapping_add(1);
    }

    let previous_levels = host.levels();
    if host.game_time() % 80 == 0 {
        if !host.scan().committed.is_empty() {
            let levels = update_base_source::<H, A>(host, (x, y, z))?;
            host.set_levels_raw(levels);
        }
        if host.levels() > 0 && !host.scan().committed.is_empty() {
            let levels = host.levels();
            let primary = host.primary().cloned();
            let secondary = host.secondary().cloned();
            A::apply_effects(host, levels, primary.as_ref(), secondary.as_ref())?;
            A::play_sound(host, BeaconSound::Ambient)?;
        }
    }

    if host.scan().last_check_y >= surface_y {
        let min_y = host.min_build_height();
        let scan = host.scan_mut();
        scan.last_check_y = min_y.wrapping_sub(1);
        // Java aliases its completed checking list until the next scan replaces
        // that list. Neither list has a public mutable accessor here; keeping
        // the same completed values preserves the source's observable phases.
        scan.committed = scan.checking.clone();
        let was_active = previous_levels > 0;
        let is_active = host.levels() > 0;
        if !was_active && is_active {
            A::activate(host)?;
        } else if was_active && !is_active {
            A::play_sound(host, BeaconSound::Deactivate)?;
        }
    }
    Ok(())
}

fn update_base_source<H: BeaconFields, A: TickAccess<H>>(
    host: &mut H,
    (x, y, z): (i32, i32, i32),
) -> Result<i32> {
    let mut levels = 0;
    for layer in 1_i32..=4 {
        let below_y = y.wrapping_sub(layer);
        if below_y < host.min_build_height() {
            break;
        }
        for offset_x in -layer..=layer {
            for offset_z in -layer..=layer {
                let block = A::block_state(
                    host,
                    (x.wrapping_add(offset_x), below_y, z.wrapping_add(offset_z)),
                )?;
                if !A::is_beacon_base(host, &block)? {
                    return Ok(levels);
                }
            }
        }
        levels = layer;
    }
    Ok(levels)
}

/// FastColor.ARGB32.average: unsigned per-channel integer floor, including alpha.
fn average_argb(left: u32, right: u32) -> u32 {
    let mut result = 0;
    for shift in [0, 8, 16, 24] {
        let value = (((left >> shift) & 255) + ((right >> shift) & 255)) / 2;
        result |= value << shift;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        geometry::Aabb,
        workstations::beacon_effects::{
            BeaconEffectApplication, BeaconEffectsHost, BeaconSoundHost,
        },
    };
    use std::{collections::BTreeMap, sync::Arc};

    #[derive(Clone, Copy)]
    enum Block {
        Beam(u32),
        Air,
        Opaque,
        Base,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Event {
        EffectPlayers,
        Effect(u8, i32, i32, usize, usize),
        Sound(BeaconSound, i32, usize),
        ConstructionPlayers,
        Construct(i32, i32, usize),
    }

    /// Detached, explicitly bounded fixture owners, not a production Level,
    /// effect registry, active-effect map, RNG or advancement implementation.
    #[derive(Clone)]
    struct Host {
        scan: BeaconScanState,
        levels: i32,
        primary: Option<Arc<u8>>,
        secondary: Option<Arc<u8>>,
        time: i64,
        blocks: BTreeMap<(i32, i32, i32), Block>,
        reads: Vec<(i32, i32, i32)>,
        events: Vec<Event>,
        fail_publish: bool,
        stage: Option<BeaconAction>,
    }

    impl Host {
        fn fixture() -> Self {
            let mut blocks = BTreeMap::new();
            blocks.insert((0, 10, 0), Block::Beam(0xffff_ffff));
            blocks.insert((0, 11, 0), Block::Beam(0xffff_0000));
            blocks.insert((0, 12, 0), Block::Beam(0xff00_00ff));
            for x in -1..=1 {
                for z in -1..=1 {
                    blocks.insert((x, 9, z), Block::Base);
                }
            }
            let mut scan = BeaconScanState::default();
            scan.after_set_level(0);
            Self {
                scan,
                levels: 0,
                primary: Some(Arc::new(7)),
                secondary: None,
                time: 1,
                blocks,
                reads: Vec::new(),
                events: Vec::new(),
                fail_publish: false,
                stage: None,
            }
        }
    }

    impl BeaconFields for Host {
        type Effect = Arc<u8>;

        fn require_actual_stage(&self, action: BeaconAction) -> Result<()> {
            ensure!(self.stage == Some(action), "fixture stage action mismatch");
            Ok(())
        }
        fn position(&self) -> (i32, i32, i32) {
            (0, 10, 0)
        }
        fn is_client_side(&self) -> bool {
            false
        }
        fn game_time(&self) -> i64 {
            self.time
        }
        fn min_build_height(&self) -> i32 {
            0
        }
        fn dimension_height(&self) -> i32 {
            64
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
        fn primary(&self) -> Option<&Self::Effect> {
            self.primary.as_ref()
        }
        fn secondary(&self) -> Option<&Self::Effect> {
            self.secondary.as_ref()
        }
        fn set_primary_raw(&mut self, effect: Option<Self::Effect>) {
            self.primary = effect;
        }
        fn set_secondary_raw(&mut self, effect: Option<Self::Effect>) {
            self.secondary = effect;
        }
        fn same_effect_holder(&self, left: &Self::Effect, right: &Self::Effect) -> Result<bool> {
            Ok(Arc::ptr_eq(left, right))
        }
    }

    impl BeaconTransactionHost for Host {
        fn begin_stage(&self, action: BeaconAction) -> Result<Self> {
            ensure!(action == BeaconAction::Tick, "fixture only admits tick");
            let mut staged = self.clone();
            staged.stage = Some(action);
            Ok(staged)
        }

        fn publish_stage(&mut self, mut staged: Self, action: BeaconAction) -> Result<()> {
            ensure!(action == BeaconAction::Tick, "fixture only admits tick");
            ensure!(
                !self.fail_publish,
                "fixture publication preparation refused"
            );
            staged.require_actual_stage(action)?;
            staged.stage = None;
            *self = staged;
            Ok(())
        }
    }

    impl BeaconSoundHost for Host {
        fn play_sound(&mut self, sound: BeaconSound) -> Result<()> {
            self.events
                .push(Event::Sound(sound, self.levels, self.scan.committed.len()));
            Ok(())
        }
    }

    impl BeaconEffectsHost for Host {
        type Player = i32;
        type ServerPlayer = i32;

        fn players_in_effect_box(&mut self, _bounds: Aabb) -> Result<Vec<Self::Player>> {
            self.events.push(Event::EffectPlayers);
            Ok(vec![11, 12])
        }

        fn server_players_in_activation_box(
            &mut self,
            _bounds: Aabb,
        ) -> Result<Vec<Self::ServerPlayer>> {
            self.events.push(Event::ConstructionPlayers);
            Ok(vec![11, 12])
        }

        fn add_beacon_effect(
            &mut self,
            _player: &Self::Player,
            application: BeaconEffectApplication<'_, Self::Effect>,
        ) -> Result<()> {
            assert!(application.ambient && application.visible && application.show_icon);
            self.events.push(Event::Effect(
                **application.effect,
                application.duration,
                application.amplifier,
                self.scan.committed.len(),
                self.scan.checking.len(),
            ));
            Ok(())
        }

        fn trigger_construct_beacon(
            &mut self,
            player: &Self::ServerPlayer,
            levels: i32,
        ) -> Result<()> {
            self.events
                .push(Event::Construct(*player, levels, self.scan.committed.len()));
            Ok(())
        }
    }

    impl BeaconTickHost for Host {
        type BlockState = Block;

        fn world_surface_height(&mut self, x: i32, z: i32) -> Result<i32> {
            ensure!((x, z) == (0, 0), "fixture surface column unavailable");
            Ok(22)
        }

        fn block_state(&mut self, position: (i32, i32, i32)) -> Result<Block> {
            self.reads.push(position);
            Ok(self.blocks.get(&position).copied().unwrap_or(Block::Air))
        }

        fn beam_color(&self, state: &Block) -> Result<Option<u32>> {
            Ok(match state {
                Block::Beam(color) => Some(*color),
                _ => None,
            })
        }

        fn light_block(&mut self, state: &Block, _position: (i32, i32, i32)) -> Result<i32> {
            Ok(if matches!(state, Block::Opaque | Block::Base) {
                15
            } else {
                0
            })
        }

        fn is_bedrock(&self, _state: &Block) -> Result<bool> {
            Ok(false)
        }
        fn is_beacon_base(&self, state: &Block) -> Result<bool> {
            Ok(matches!(state, Block::Base))
        }
    }

    #[test]
    fn incremental_beam_periodic_old_beam_and_atomic_phase_flow() -> Result<()> {
        let mut host = Host::fixture();
        tick(&mut host)?;
        assert_eq!(host.reads, (10..20).map(|y| (0, y, 0)).collect::<Vec<_>>());
        assert_eq!(host.scan.last_check_y(), 19);
        assert!(host.scan.committed_sections().is_empty());
        host.time = 2;
        tick(&mut host)?;
        assert_eq!(
            host.scan.committed_sections(),
            &[
                BeaconBeamSection {
                    color: 0xffff_ffff,
                    height: 1
                },
                BeaconBeamSection {
                    color: 0xffff_0000,
                    height: 1
                },
                BeaconBeamSection {
                    color: 0xff7f_007f,
                    height: 11
                },
            ]
        );
        assert!(host.scan.visible_sections(0).is_empty());
        assert_eq!(host.scan.visible_sections(-1).len(), 3);
        assert_eq!(host.scan.last_check_y(), -1);
        assert!(host.events.is_empty());

        // The next completed scan has two sections, while effects must observe
        // the old three-section publication and source base query order.
        host.blocks.insert((0, 12, 0), Block::Beam(0xffff_0000));
        host.time = 79;
        tick(&mut host)?;
        host.reads.clear();
        host.time = 80;
        tick(&mut host)?;
        let mut expected_reads = (20..=22).map(|y| (0, y, 0)).collect::<Vec<_>>();
        for x in -1..=1 {
            for z in -1..=1 {
                expected_reads.push((x, 9, z));
            }
        }
        expected_reads.push((-2, 8, -2));
        assert_eq!(host.reads, expected_reads);
        assert_eq!(
            host.events,
            vec![
                Event::EffectPlayers,
                Event::Effect(7, 220, 0, 3, 2),
                Event::Effect(7, 220, 0, 3, 2),
                Event::Sound(BeaconSound::Ambient, 1, 3),
                Event::Sound(BeaconSound::Activate, 1, 2),
                Event::ConstructionPlayers,
                Event::Construct(11, 1, 2),
                Event::Construct(12, 1, 2),
            ]
        );

        // Blocked scan clears published beams without inventing a level reset
        // or deactivation. On a later 80tick empty OLD publication skips base.
        host.events.clear();
        host.blocks.insert((0, 11, 0), Block::Opaque);
        host.time = 81;
        tick(&mut host)?;
        assert_eq!(host.levels, 1);
        assert!(host.scan.committed_sections().is_empty());
        host.time = 160;
        host.reads.clear();
        tick(&mut host)?;
        assert_eq!(host.reads, vec![(0, 10, 0), (0, 11, 0)]);
        assert_eq!(host.levels, 1);
        assert!(host.events.is_empty());

        // Failed publication keeps the live scan, fields, world observations
        // and callback outbox. Detached stage callbacks are not replayed.
        host.fail_publish = true;
        let before = (
            host.scan.clone(),
            host.levels,
            host.reads.clone(),
            host.events.clone(),
        );
        assert!(tick(&mut host).is_err());
        assert_eq!(
            before,
            (
                host.scan.clone(),
                host.levels,
                host.reads.clone(),
                host.events.clone()
            )
        );
        assert_eq!(average_argb(0xff00_0001, 0x8000_01ff), 0xbf00_0080);
        Ok(())
    }

    struct LiveBe {
        scan: BeaconScanState,
        levels: i32,
        primary: Option<Arc<u8>>,
        secondary: Option<Arc<u8>>,
        removed: bool,
        attached_level: usize,
    }

    impl LiveBe {
        fn new(attached_level: usize) -> Self {
            Self {
                scan: BeaconScanState::default(),
                levels: 0,
                primary: None,
                secondary: None,
                removed: false,
                attached_level,
            }
        }
    }

    struct LiveLevel {
        time: i64,
        height: i32,
    }

    struct BeLease {
        slot: usize,
        address: *const LiveBe,
    }

    #[derive(Debug, PartialEq, Eq)]
    struct LiveSoundObservation {
        sound: BeaconSound,
        levels: i32,
        published_sections: usize,
        removed: bool,
        current_slot: usize,
        dimension_height: i32,
    }

    // Controlled canonical fixture arena: actual boxes retain their identity
    // while its current-position index changes. This is deliberately non-Clone
    // and has NO BeaconTransactionHost/BeaconTickHost implementation. It tests
    // bridge ownership/error semantics, not a production BE/Level/sound owner.
    // No living actor, sound RNG, packet or criterion success is fabricated.
    struct LiveHost {
        arena: Vec<Box<LiveBe>>,
        retained: BeLease,
        current_slot: usize,
        level_arena: Vec<Box<LiveLevel>>,
        captured_level: *const LiveLevel,
        observations: Vec<LiveSoundObservation>,
    }

    impl LiveHost {
        fn fixture() -> Self {
            let mut be = Box::new(LiveBe::new(0));
            be.scan.after_set_level(0);
            be.scan.committed = vec![
                BeaconBeamSection {
                    color: 0xffff_ffff,
                    height: 1,
                },
                BeaconBeamSection {
                    color: 0xffff_0000,
                    height: 1,
                },
                BeaconBeamSection {
                    color: 0xff7f_007f,
                    height: 1,
                },
            ];
            let retained = BeLease {
                slot: 0,
                address: be.as_ref(),
            };
            let level = Box::new(LiveLevel {
                time: 80,
                height: 64,
            });
            let captured_level = level.as_ref() as *const LiveLevel;
            Self {
                arena: vec![be],
                retained,
                current_slot: 0,
                level_arena: vec![
                    level,
                    Box::new(LiveLevel {
                        time: 1,
                        height: 16,
                    }),
                ],
                captured_level,
                observations: Vec::new(),
            }
        }

        fn retained_be(&self) -> &LiveBe {
            &self.arena[self.retained.slot]
        }
        fn retained_be_mut(&mut self) -> &mut LiveBe {
            &mut self.arena[self.retained.slot]
        }
        fn original_level(&self) -> &LiveLevel {
            &self.level_arena[0]
        }
    }

    impl BeaconFields for LiveHost {
        type Effect = Arc<u8>;

        fn require_actual_stage(&self, _action: BeaconAction) -> Result<()> {
            anyhow::bail!("controlled live owner has no detached-stage capability")
        }
        fn position(&self) -> (i32, i32, i32) {
            (0, 10, 0)
        }
        fn is_client_side(&self) -> bool {
            false
        }
        fn game_time(&self) -> i64 {
            self.original_level().time
        }
        fn min_build_height(&self) -> i32 {
            0
        }
        fn dimension_height(&self) -> i32 {
            self.original_level().height
        }
        fn scan(&self) -> &BeaconScanState {
            &self.retained_be().scan
        }
        fn scan_mut(&mut self) -> &mut BeaconScanState {
            &mut self.retained_be_mut().scan
        }
        fn levels(&self) -> i32 {
            self.retained_be().levels
        }
        fn set_levels_raw(&mut self, levels: i32) {
            self.retained_be_mut().levels = levels;
        }
        fn primary(&self) -> Option<&Self::Effect> {
            self.retained_be().primary.as_ref()
        }
        fn secondary(&self) -> Option<&Self::Effect> {
            self.retained_be().secondary.as_ref()
        }
        fn set_primary_raw(&mut self, effect: Option<Self::Effect>) {
            self.retained_be_mut().primary = effect;
        }
        fn set_secondary_raw(&mut self, effect: Option<Self::Effect>) {
            self.retained_be_mut().secondary = effect;
        }
        fn same_effect_holder(&self, left: &Self::Effect, right: &Self::Effect) -> Result<bool> {
            Ok(Arc::ptr_eq(left, right))
        }
    }

    impl BeaconLiveOwner for LiveHost {
        fn require_actual_live(&self, action: BeaconAction) -> Result<()> {
            ensure!(action == BeaconAction::Tick, "fixture only admits Tick");
            ensure!(
                std::ptr::eq(self.retained_be(), self.retained.address),
                "retained fixture BE lease lost"
            );
            ensure!(
                std::ptr::eq(self.original_level(), self.captured_level),
                "original fixture Level lease lost"
            );
            // Deliberately no current_slot, removed, attached_level or mutable
            // revision check: none can redirect/invalidate this retained call.
            Ok(())
        }
    }

    impl BeaconSoundHost for LiveHost {
        fn play_sound(&mut self, sound: BeaconSound) -> Result<()> {
            self.observations.push(LiveSoundObservation {
                sound,
                levels: self.levels(),
                published_sections: self.scan().committed_sections().len(),
                removed: self.retained_be().removed,
                current_slot: self.current_slot,
                dimension_height: self.dimension_height(),
            });
            match sound {
                BeaconSound::Ambient => {
                    // Controlled reentrant removal/replacement of the current
                    // arena index, and reattachment of the retained BE. Its Box
                    // stays canonical and alive for the rest of this call.
                    let retained = self.retained_be_mut();
                    retained.removed = true;
                    retained.attached_level = 1;
                    retained.levels = 2;
                    self.arena.push(Box::new(LiveBe::new(1)));
                    self.current_slot = self.arena.len() - 1;
                    Ok(())
                }
                BeaconSound::Activate => {
                    anyhow::bail!("injected live failure at activation callback")
                }
                _ => anyhow::bail!("unexpected controlled sound callback"),
            }
        }
    }

    enum NoFixtureActor {}
    impl BeaconEffectsHost for LiveHost {
        type Player = NoFixtureActor;
        type ServerPlayer = NoFixtureActor;
        fn players_in_effect_box(&mut self, _bounds: Aabb) -> Result<Vec<Self::Player>> {
            anyhow::bail!("controlled fixture must not query actors")
        }
        fn server_players_in_activation_box(
            &mut self,
            _bounds: Aabb,
        ) -> Result<Vec<Self::ServerPlayer>> {
            anyhow::bail!("controlled failure precedes actor query")
        }
        fn add_beacon_effect(
            &mut self,
            _player: &Self::Player,
            _application: BeaconEffectApplication<'_, Self::Effect>,
        ) -> Result<()> {
            anyhow::bail!("controlled fixture must not apply effects")
        }
        fn trigger_construct_beacon(
            &mut self,
            _player: &Self::ServerPlayer,
            _levels: i32,
        ) -> Result<()> {
            anyhow::bail!("controlled fixture must not trigger criteria")
        }
    }

    impl BeaconLiveTickHost for LiveHost {
        type BlockState = Block;
        fn world_surface_height(&mut self, x: i32, z: i32) -> Result<i32> {
            ensure!((x, z) == (0, 0), "fixture column unavailable");
            Ok(10)
        }
        fn block_state(&mut self, position: (i32, i32, i32)) -> Result<Block> {
            Ok(if position == (0, 10, 0) {
                Block::Beam(0xffff_ffff)
            } else if position.1 == 9 && position.0.abs() <= 1 && position.2.abs() <= 1 {
                Block::Base
            } else {
                Block::Air
            })
        }
        fn beam_color(&self, state: &Block) -> Result<Option<u32>> {
            Ok(match state {
                Block::Beam(color) => Some(*color),
                _ => None,
            })
        }
        fn light_block(&mut self, state: &Block, _position: (i32, i32, i32)) -> Result<i32> {
            Ok(if matches!(state, Block::Opaque | Block::Base) {
                15
            } else {
                0
            })
        }
        fn is_bedrock(&self, _state: &Block) -> Result<bool> {
            Ok(false)
        }
        fn is_beacon_base(&self, state: &Block) -> Result<bool> {
            Ok(matches!(state, Block::Base))
        }
    }

    #[test]
    fn live_only_retained_owner_continues_after_replacement_and_keeps_failure_prefix() -> Result<()>
    {
        let mut host = LiveHost::fixture();
        assert!(host.require_actual_stage(BeaconAction::Tick).is_err());
        let error = tick_live(&mut host).unwrap_err();
        assert!(error
            .to_string()
            .contains("injected live failure at activation"));
        assert_eq!(
            host.observations,
            vec![
                LiveSoundObservation {
                    sound: BeaconSound::Ambient,
                    levels: 1,
                    published_sections: 3,
                    removed: false,
                    current_slot: 0,
                    dimension_height: 64,
                },
                LiveSoundObservation {
                    sound: BeaconSound::Activate,
                    levels: 2,
                    published_sections: 1,
                    removed: true,
                    current_slot: 1,
                    dimension_height: 64,
                },
            ]
        );
        // Prefix remains on the original retained object and original context;
        // the replacement at the same position stays untouched. No retry occurs.
        host.require_actual_live(BeaconAction::Tick)?;
        assert!(host.retained_be().removed);
        assert_eq!(host.retained_be().attached_level, 1);
        assert_eq!(host.retained_be().levels, 2);
        assert_eq!(host.retained_be().scan.last_check_y(), -1);
        assert_eq!(
            host.retained_be().scan.committed_sections(),
            &[BeaconBeamSection {
                color: 0xffff_ffff,
                height: 1
            },]
        );
        assert_eq!(host.current_slot, 1);
        assert_eq!(host.arena[1].levels, 0);
        assert!(host.arena[1].scan.committed_sections().is_empty());
        assert_eq!(host.dimension_height(), 64);
        assert_eq!(host.level_arena[1].height, 16);
        Ok(())
    }
}
