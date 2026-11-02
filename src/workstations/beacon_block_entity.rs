//! Java 1.21 BeaconBlockEntity's incremental beam, pyramid and tick phases.
//!
//! This kernel borrows the actual block entity and Level through a mandatory
//! host. It neither registers a BE nor supplies a second payment, power, living
//! effect, registry, criterion or sound/RNG owner.

use super::beacon_effects::{
    activate_staged, apply_effects_staged, BeaconEffectsHost, BeaconSound,
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

/// Mandatory transaction over the actual BE and every owner a callback touches.
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
pub trait BeaconTickHost: BeaconTransactionHost + BeaconEffectsHost {
    type BlockState;

    fn world_surface_height(&mut self, x: i32, z: i32) -> Result<i32>;
    fn block_state(&mut self, position: (i32, i32, i32)) -> Result<Self::BlockState>;
    fn beam_color(&self, state: &Self::BlockState) -> Result<Option<u32>>;
    fn light_block(&mut self, state: &Self::BlockState, position: (i32, i32, i32)) -> Result<i32>;
    fn is_bedrock(&self, state: &Self::BlockState) -> Result<bool>;
    fn is_beacon_base(&self, state: &Self::BlockState) -> Result<bool>;
}

/// Execute one admitted SERVER tick against detached actual owners. No scan,
/// effect, RNG, criterion or packet is published if staging/preparation fails.
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
    tick_staged(&mut staged)?;
    host.publish_stage(staged, BeaconAction::Tick)
}

fn tick_staged<H: BeaconTickHost>(host: &mut H) -> Result<()> {
    let (x, y, z) = host.position();
    let mut scan_y = if host.scan().last_check_y < y {
        let scan = host.scan_mut();
        scan.checking.clear();
        scan.last_check_y = y.wrapping_sub(1);
        y
    } else {
        host.scan().last_check_y.wrapping_add(1)
    };
    let surface_y = host.world_surface_height(x, z)?;

    for _ in 0..10 {
        if scan_y > surface_y {
            break;
        }
        let position = (x, scan_y, z);
        let block = host.block_state(position)?;
        if let Some(color) = host.beam_color(&block)? {
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
                && (host.light_block(&block, position)? < 15 || host.is_bedrock(&block)?);
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
            let levels = update_base_staged(host, (x, y, z))?;
            host.set_levels_raw(levels);
        }
        if host.levels() > 0 && !host.scan().committed.is_empty() {
            let levels = host.levels();
            let primary = host.primary().cloned();
            let secondary = host.secondary().cloned();
            apply_effects_staged(host, levels, primary.as_ref(), secondary.as_ref())?;
            host.play_sound(BeaconSound::Ambient)?;
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
            activate_staged(host)?;
        } else if was_active && !is_active {
            host.play_sound(BeaconSound::Deactivate)?;
        }
    }
    Ok(())
}

fn update_base_staged<H: BeaconTickHost>(host: &mut H, (x, y, z): (i32, i32, i32)) -> Result<i32> {
    let mut levels = 0;
    for layer in 1_i32..=4 {
        let below_y = y.wrapping_sub(layer);
        if below_y < host.min_build_height() {
            break;
        }
        for offset_x in -layer..=layer {
            for offset_z in -layer..=layer {
                let block = host.block_state((
                    x.wrapping_add(offset_x),
                    below_y,
                    z.wrapping_add(offset_z),
                ))?;
                if !host.is_beacon_base(&block)? {
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
}
