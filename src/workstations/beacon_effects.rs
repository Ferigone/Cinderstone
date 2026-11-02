//! Java 1.21 BeaconBlockEntity effect, activation and sound owner contracts.
//!
//! Direct JDK21 `javap -p -c` of pinned `dqc` applyEffects250–272 and
//! playSound275–276, its tick activation branch, `dcn` EntityGetter, `brz`
//! MobEffectInstance constructors and `dcw` Level sound overloads is primary
//! evidence. Inner server SHA256:
//! 8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01;
//! official mappings SHA1: 31c77994d96f05ba25a870ada70f47f315330437.
//! This is bytecode inspection, not REA or a Java gameplay observation.
//!
//! Helpers ending in `_staged` run ONLY inside the actual caller's detached
//! BeaconTransactionHost stage. They never begin/publish another transaction,
//! own a second live effect map, resolve manufactured effect holders, award a
//! fabricated criterion, or substitute a world/player/sound RNG. A callback
//! error propagates to that outer stage; merely returning Result cannot undo
//! a callback that incorrectly escaped into live owners.
//!
//! `_live` helpers instead borrow the actual retained BE and actor owners.
//! They preserve every completed mutation, RNG draw and callback on an error or
//! reentry. They never clone, stage, restore, retry or republish those owners.
//! Restricted staging is not a general substitute for live living-effect maps.

use super::beacon_block_entity::{BeaconAction, BeaconFields, BeaconLiveOwner};
use crate::{
    geometry::{Aabb, Vec3},
    placement::BlockPosition,
};
use anyhow::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeaconSound {
    Ambient,
    Activate,
    Deactivate,
    PowerSelect,
}

/// Bind the captured BE's actual Level and BlockPos to its real sound callback.
/// No implementation or guessed sound/RNG owner is supplied here.
pub trait BeaconSoundHost: BeaconFields {
    /// Actual Level.playSound(null, position, corresponding BEACON_* SoundEvent,
    /// BLOCKS, 1f, 1f). Resolve the configured actual SoundEvent. Level converts
    /// coordinates to block centers, then consumes exactly ONE nextLong from
    /// its actual threadSafeRandom BEFORE seeded sound broadcast, even when no
    /// recipient is present. Neither Level.random nor a player/local RNG can
    /// replace it; there is no additional pitch draw at this source site.
    ///
    /// Existing native Main has the actual sound stream through
    /// RuntimeEntities::next_sound_seed/sound_random_state48 and its checkpoint;
    /// a future binding may borrow that real owner directly. A fake Item batch
    /// or second sound stream is unnecessary. Actual BE binding, source phase,
    /// sound range/dimension audience and ordered publication still belong to
    /// the actual live owner or a certified restricted stage. Missing capability
    /// is an error. In a live call, a completed seed draw/output prefix persists
    /// if a later callback fails; do not rewind, redraw or replay that sound.
    fn play_sound(&mut self, sound: BeaconSound) -> Result<()>;
}

/// A source constructor request, not an active effect or owned registry entry.
/// The host constructs a FRESH actual MobEffectInstance for each invocation.
pub struct BeaconEffectApplication<'a, E> {
    pub effect: &'a E,
    pub duration: i32,
    pub amplifier: i32,
    pub ambient: bool,
    pub visible: bool,
    pub show_icon: bool,
}

/// Actual ordered spatial queries and living-effect/advancement callbacks.
///
/// Implement on the same actual retained BE/Level/player/registry/criterion/
/// sound owners as the tick driver, or its proven restricted stage. Opaque
/// handles retain the actual queried objects through removal, replacement and
/// callback reentry; they must never silently redirect to a replacement player
/// with a reused ID. Also retain the original source-call Level and BlockPos
/// argument leases: reattaching/removing the BE must not redirect later world,
/// position, sound or query operations to its newly attached/current index.
/// Live callbacks operate on those canonical actor objects
/// and their actual effect maps, source cause=None, packets and criteria. A
/// later error preserves the completed source prefix; no clone/restore/retry
/// protocol is implied. No permissive defaults certify unavailable context as
/// an empty player list, ignored effect or absent advancement listener.
pub trait BeaconEffectsHost: BeaconFields + BeaconSoundHost {
    type Player;
    type ServerPlayer;

    /// Actual Level.getEntitiesOfClass(Player.class, bounds): its default
    /// EntitySelector.NO_SPECTATORS filter, current body-box intersection and
    /// actual spatial encounter order. Do not sort by ID/UUID, add alive or
    /// creative/effect-immunity filters, or replace this with radial distance.
    fn players_in_effect_box(&mut self, bounds: Aabb) -> Result<Vec<Self::Player>>;
    /// A separate actual ServerPlayer.class query at the activation phase,
    /// with the same default NO_SPECTATORS and source encounter ordering.
    fn server_players_in_activation_box(&mut self, bounds: Aabb)
        -> Result<Vec<Self::ServerPlayer>>;
    /// Construct a fresh actual MobEffectInstance from this canonical holder
    /// and flags, with null hidden effect and constructor-reset blend state.
    /// Then invoke this exact player's addEffect(instance), whose source entity
    /// is null; IGNORE its Boolean. Preserve actual canBeAffected, active/hidden
    /// merging, attributes, callbacks, packets and effect ticking through the
    /// Team4/central living-effect owner. Pure EffectInstance construction alone
    /// is not effect application, and rejection is not a refund or retry.
    fn add_beacon_effect(
        &mut self,
        player: &Self::Player,
        application: BeaconEffectApplication<'_, Self::Effect>,
    ) -> Result<()>;
    /// Actual CONSTRUCT_BEACON.trigger(player, levels): use its configured
    /// listener set, level predicate, optional player context and real awards/
    /// rewards. A genuinely empty configured listener set may return normally;
    /// missing ownership must not masquerade as that empty-set source branch.
    fn trigger_construct_beacon(&mut self, player: &Self::ServerPlayer, levels: i32) -> Result<()>;
}

fn effect_range(levels: i32) -> f64 {
    levels.wrapping_mul(10).wrapping_add(10) as f64
}
fn effect_duration(levels: i32) -> i32 {
    9_i32.wrapping_add(levels.wrapping_mul(2)).wrapping_mul(20)
}

/// Exact source AABB composition. Height is actual Level.getHeight(), not an
/// absolute maximum Y. Unit BlockPos coordinates include Java wrapping +1.
pub fn effect_query_box(position: BlockPosition, levels: i32, dimension_height: i32) -> Aabb {
    effect_box_with_range(position, effect_range(levels), dimension_height)
}

fn effect_box_with_range(position: BlockPosition, radius: f64, dimension_height: i32) -> Aabb {
    Aabb::unit_block(position.0, position.1, position.2)
        .inflate(Vec3::new(radius, radius, radius))
        .expand_towards(Vec3::new(0., dimension_height as f64, 0.))
}

/// This is NOT the effect AABB: source constructs the normalized box from
/// (x,y,z) to (x,y-4,z), then inflates by (10,5,10). No unit-block +1 appears.
pub fn activation_query_box(position: BlockPosition) -> Aabb {
    Aabb::new(
        position.0 as f64,
        position.1 as f64,
        position.2 as f64,
        position.0 as f64,
        position.1.wrapping_sub(4) as f64,
        position.2 as f64,
    )
    .inflate(Vec3::new(10., 5., 10.))
}

/// Source applyEffects with CAPTURED argument snapshots. Callers clone only
/// their actual canonical holder references before borrowing the staged host;
/// callbacks changing BE selections/levels cannot replace these arguments.
///
/// Primary effects run for ALL queried players before any secondary effect.
/// The same actual captured list is reused even if callbacks change positions
/// or live effect state. Holder comparisons remain the actual Objects.equals
/// owner, including the second source comparison AFTER all primary callbacks.
/// The mandatory stage certificate is checked before any source observation;
/// a caller flag or shallow clone is not an admitted actual-owner capability.
/// This API remains restricted to domains whose real owner proves detached
/// staging; arbitrary live actor/effect graphs are not that domain.
pub fn apply_effects_staged<H: BeaconEffectsHost>(
    host: &mut H,
    levels: i32,
    primary: Option<&H::Effect>,
    secondary: Option<&H::Effect>,
) -> Result<()> {
    host.require_actual_stage(BeaconAction::Tick)?;
    apply_effects_source_core(host, levels, primary, secondary)
}

/// Source applyEffects on actual canonical retained BE/actor/registry/world
/// owners. The live capability is checked FIRST. It must admit retained source
/// objects even if a legitimate callback removes/replaces their current index
/// entries; neither reused-ID lookup nor a mutable-version comparison may
/// redirect or abort the remaining source calls after such a callback.
///
/// Captured level/power arguments and the queried object list keep the same
/// source semantics as the staged API. Completed live effects/callbacks and
/// source RNG/output prefixes persist if this function returns Err. There is
/// no begin/publish stage, clone, rollback, retry or automatic callback replay.
pub fn apply_effects_live<H: BeaconEffectsHost + BeaconLiveOwner>(
    host: &mut H,
    levels: i32,
    primary: Option<&H::Effect>,
    secondary: Option<&H::Effect>,
) -> Result<()> {
    host.require_actual_live(BeaconAction::Tick)?;
    apply_effects_source_core(host, levels, primary, secondary)
}

fn apply_effects_source_core<H: BeaconEffectsHost>(
    host: &mut H,
    levels: i32,
    primary: Option<&H::Effect>,
    secondary: Option<&H::Effect>,
) -> Result<()> {
    if host.is_client_side() {
        return Ok(());
    }
    let Some(primary) = primary else {
        return Ok(());
    };
    let radius = effect_range(levels);
    let upgraded = if levels >= 4 {
        match secondary {
            Some(secondary) => host.same_effect_holder(primary, secondary)?,
            None => false,
        }
    } else {
        false
    };
    let duration = effect_duration(levels);
    let position = host.position();
    let bounds = effect_box_with_range(position, radius, host.dimension_height());
    let players = host.players_in_effect_box(bounds)?;
    for player in &players {
        host.add_beacon_effect(
            player,
            BeaconEffectApplication {
                effect: primary,
                duration,
                amplifier: i32::from(upgraded),
                ambient: true,
                visible: true,
                show_icon: true,
            },
        )?;
    }
    if levels >= 4 {
        // Source compares again here, rather than caching its earlier result.
        let same = match secondary {
            Some(secondary) => host.same_effect_holder(primary, secondary)?,
            None => false,
        };
        if !same {
            if let Some(secondary) = secondary {
                for player in &players {
                    host.add_beacon_effect(
                        player,
                        BeaconEffectApplication {
                            effect: secondary,
                            duration,
                            amplifier: 0,
                            ambient: true,
                            visible: true,
                            show_icon: true,
                        },
                    )?;
                }
            }
        }
    }
    Ok(())
}

/// Call ONLY at the tick driver's actual server-side inactive→active branch,
/// AFTER publishing the newly checked beam list into its same staged BE.
/// Source activation sound precedes its DISTINCT ServerPlayer query. Each
/// criterion observes BE.levels anew; an earlier callback may change that field.
pub fn activate_staged<H: BeaconEffectsHost>(host: &mut H) -> Result<()> {
    host.require_actual_stage(BeaconAction::Tick)?;
    activate_source_core(host)
}

/// Source activation on the same actual live retained BE/actor/world domains.
/// Check the live capability before sound, queries or field observations. Its
/// retained BE/actor leases survive legitimate removal/replacement/reentry;
/// subsequent criterion calls still read that source BE's current levels.
/// On Err, preserve activation sound/seed and prior criterion award/output
/// prefixes. Do not restore, repeat the sound, re-query, or retry the trigger.
pub fn activate_live<H: BeaconEffectsHost + BeaconLiveOwner>(host: &mut H) -> Result<()> {
    host.require_actual_live(BeaconAction::Tick)?;
    activate_source_core(host)
}

fn activate_source_core<H: BeaconEffectsHost>(host: &mut H) -> Result<()> {
    let position = host.position();
    host.play_sound(BeaconSound::Activate)?;
    let players = host.server_players_in_activation_box(activation_query_box(position))?;
    for player in &players {
        let levels = host.levels();
        host.trigger_construct_beacon(player, levels)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pure source geometry/arithmetic checks, not an invented successful
    // living-effect, criterion, sound, player-query or Main implementation.
    #[test]
    fn actual_aabb_composition_keeps_unit_extent_and_dimension_height() {
        assert_eq!(
            effect_query_box((3, 64, -2), 4, 384),
            Aabb::new(-47., 14., -52., 54., 499., 49.)
        );
        assert_eq!(
            effect_query_box((3, 64, -2), 1, 384),
            Aabb::new(-17., 44., -22., 24., 469., 19.)
        );
        assert_eq!(effect_duration(1), 220);
        assert_eq!(effect_duration(4), 340);
    }

    #[test]
    fn activation_uses_normalized_degenerate_box_without_unit_extent() {
        assert_eq!(
            activation_query_box((3, 64, -2)),
            Aabb::new(-7., 55., -12., 13., 69., 8.)
        );
        // Java subtracts four as an int BEFORE converting to double.
        let extreme = activation_query_box((0, i32::MIN, 0));
        assert_eq!(extreme.min_y, i32::MIN as f64 - 5.);
        assert_eq!(extreme.max_y, i32::MAX.wrapping_sub(3) as f64 + 5.);
    }

    #[test]
    fn source_range_and_duration_wrap_before_double_conversion() {
        assert_eq!(effect_range(i32::MAX), 0.);
        assert_eq!(effect_duration(i32::MAX), 140);
        assert_eq!(effect_range(i32::MIN), 10.);
        assert_eq!(effect_duration(i32::MIN), 180);
    }
}
