//! Java 1.21 AnvilBlock's inherited FallingBlock callbacks, protocol 767.
//!
//! Primary evidence: direct JDK21 `javap -p -c` of the pinned original inner
//! server (SHA256 8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01),
//! official mappings SHA1 31c77994d96f05ba25a870ada70f47f315330437. FallingBlock
//! (`diu`) onPlace32–33/updateShape37–39/tick44–50/isFree61 and AnvilBlock
//! (`dey`) falling97–98 supply these phases. This is bytecode inspection, not
//! REA or an original-Java gameplay observation.
//!
//! This module owns no Level, registry, scheduler, RNG or falling entity. The
//! caller supplies a staged borrow of those actual owners. Central admission,
//! dispatch and persistence must use the same global BLOCK scheduler; accepting
//! these functions is not a callback/save certificate or production invocation.

use crate::{geometry::Direction, placement::BlockPosition};
use anyhow::Result;

pub const FALL_DELAY_TICKS: i32 = 2;
pub const FALL_DAMAGE_PER_DISTANCE: f32 = 2.;
pub const FALL_DAMAGE_MAX: i32 = 40;

/// Native capability checks, not extra Java world callbacks. They must not
/// query below/minimum height early, consume RNG, enqueue or publish effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnvilBlockAction {
    OnPlace,
    UpdateShape,
    ScheduledTick,
    /// Requested only after the actual free-below and minimum-height gates.
    /// Certify both actual fall and subsequent same-handle hurt configuration
    /// before acquiring constructor entropy or mutating canonical owners.
    FallingEntity,
}

/// The actual arguments to FallingBlock.updateShape's superclass delegation.
/// States remain borrowed from their registry/world owner and need not Clone.
pub struct ShapeUpdate<'a, S> {
    pub state: &'a S,
    pub direction: Direction,
    pub neighbor_state: &'a S,
    pub position: BlockPosition,
    pub neighbor_position: BlockPosition,
}

/// Implement over one staged borrow of the actual Level/registry, global BLOCK
/// queue, shared subtick counter, world callbacks and falling-entity owner.
/// Missing owners return errors; there are no permissive/no-op defaults.
///
/// Preflight must bind the real Level, position, invoked AnvilBlock singleton
/// and actual state's Block identity (including all three wear variants).
/// Registry/tag/configuration provenance belongs to that owner. Canonical
/// world, queue/shared subtick, entity index, item/player/Level RNG streams and
/// output publication must be staged together; a late error publishes none of
/// those staged changes. A cloned local world/entity is not proof of that
/// contract. Genuine process-global constructor seed acquisition can advance
/// irreversibly: finish fallible capability checks and document that boundary
/// BEFORE acquisition, and never substitute/reseed entropy or promise its
/// rollback. FallingEntity preflight certifies subsequent same-handle hurt
/// configuration before fall_block can acquire it. UpdateShape preflight must
/// establish its actual superclass delegate before the first enqueue;
/// ScheduledTick preflight must allow the stable branch without requiring an
/// unavailable falling-entity capability.
pub trait AnvilBlockHost {
    type Block;
    type BlockState;
    /// An opaque capability resolving to the SAME actual object returned by
    /// FallingBlockEntity.fall, not a copied entity or substitute ItemEntity.
    /// Dropping this capability must not remove or respawn the actual entity.
    type FallingEntity;

    fn preflight(
        &mut self,
        action: AnvilBlockAction,
        block: &Self::Block,
        state: &Self::BlockState,
        position: BlockPosition,
    ) -> Result<()>;

    /// Actual LevelAccessor.scheduleTick(position, captured Block, delay):
    /// NORMAL priority at actual gameTime+delay, incrementing the ONE shared
    /// nextSubTickCount on EVERY attempt, before queue deduplication. The key
    /// is Block singleton identity, not facing/full-state equality or a tag.
    /// Do not also enqueue an independently copied placement-plan obligation.
    fn schedule_block_tick(
        &mut self,
        position: BlockPosition,
        block: &Self::Block,
        delay: i32,
    ) -> Result<()>;

    /// The actual inherited superclass updateShape, AFTER enqueue, with all
    /// original arguments. Pinned BlockBehaviour returns the unchanged state;
    /// an unavailable delegate must error rather than fabricate its result.
    fn superclass_update_shape(
        &mut self,
        update: ShapeUpdate<'_, Self::BlockState>,
    ) -> Result<Self::BlockState>;

    fn block_state(&mut self, position: BlockPosition) -> Result<Self::BlockState>;
    fn state_is_air(&mut self, state: &Self::BlockState) -> Result<bool>;
    fn state_is_fire_tag(&mut self, state: &Self::BlockState) -> Result<bool>;
    fn state_is_liquid(&mut self, state: &Self::BlockState) -> Result<bool>;
    /// Actual BlockState.canBeReplaced() without a placement context.
    fn state_can_be_replaced(&mut self, state: &Self::BlockState) -> Result<bool>;
    fn minimum_build_height(&mut self) -> Result<i32>;

    /// Delegate to the actual FallingBlockEntity.fall(Level,position,state).
    /// Its owner constructs at (x+.5,y,z+.5), preserving the carried state
    /// except WATERLOGGED=false when present, then replaces the origin with
    /// ORIGINAL state's fluid legacy block using flags3, then attempts actual
    /// addFreshEntity. Both source booleans are ignored. Return the same actual
    /// object even if addFreshEntity returned false; do not refund or substitute
    /// an item drop. Real entity counter, constructor RNG/UUID, block callbacks,
    /// tracking, ticking and persistence remain with that entity/world owner.
    fn fall_block(
        &mut self,
        position: BlockPosition,
        state: &Self::BlockState,
    ) -> Result<Self::FallingEntity>;
    /// Invoked only AFTER fall_block's actual addFreshEntity attempt returns.
    /// Configure the returned actual object; do not construct another entity.
    fn set_hurts_entities(
        &mut self,
        entity: &mut Self::FallingEntity,
        damage_per_distance: f32,
        damage_max: i32,
    ) -> Result<()>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnvilTickOutcome {
    /// Exact !FallingBlock.isFree(below), not a support-face/sturdiness test.
    NotFreeBelow,
    BelowMinimumBuildHeight,
    Falling,
}

/// FallingBlock.onPlace schedules unconditionally, including stable support.
/// The inherited unused old-state/moving arguments have no additional effects.
pub fn on_place<H: AnvilBlockHost>(
    host: &mut H,
    block: &H::Block,
    state: &H::BlockState,
    position: BlockPosition,
) -> Result<()> {
    host.preflight(AnvilBlockAction::OnPlace, block, state, position)?;
    host.schedule_block_tick(position, block, FALL_DELAY_TICKS)
}

/// Every direction schedules BEFORE the actual superclass updateShape call.
/// Stable support and an unchanged returned state do not suppress scheduling.
pub fn update_shape<H: AnvilBlockHost>(
    host: &mut H,
    block: &H::Block,
    update: ShapeUpdate<'_, H::BlockState>,
) -> Result<H::BlockState> {
    host.preflight(
        AnvilBlockAction::UpdateShape,
        block,
        update.state,
        update.position,
    )?;
    host.schedule_block_tick(update.position, block, FALL_DELAY_TICKS)?;
    host.superclass_update_shape(update)
}

/// Preserve Java's short-circuit query order. Unknown registry/predicate
/// ownership is an error, never a guessed solid or empty state.
pub fn is_free<H: AnvilBlockHost>(host: &mut H, state: &H::BlockState) -> Result<bool> {
    if host.state_is_air(state)? {
        return Ok(true);
    }
    if host.state_is_fire_tag(state)? {
        return Ok(true);
    }
    if host.state_is_liquid(state)? {
        return Ok(true);
    }
    host.state_can_be_replaced(state)
}

/// The real outer scheduler MUST first reread the current state and skip a
/// stale tick whose Block singleton differs from the scheduled Block. Property
/// changes of the same Block (e.g. facing) remain valid. Do not replace that
/// source check with an invented exact-state/revision token.
///
/// Source reads below and short-circuits isFree BEFORE minimum build height.
/// Neither branch draws from Level/player RNG. The falling branch delegates
/// actual constructor/RNG/world work; its unavailable owner is not a stable
/// success. Later neighbor updates must still call update_shape and reschedule.
pub fn scheduled_tick<H: AnvilBlockHost>(
    host: &mut H,
    block: &H::Block,
    state: &H::BlockState,
    position: BlockPosition,
) -> Result<AnvilTickOutcome> {
    host.preflight(AnvilBlockAction::ScheduledTick, block, state, position)?;
    let below = host.block_state((position.0, position.1.wrapping_sub(1), position.2))?;
    if !is_free(host, &below)? {
        return Ok(AnvilTickOutcome::NotFreeBelow);
    }
    if position.1 < host.minimum_build_height()? {
        return Ok(AnvilTickOutcome::BelowMinimumBuildHeight);
    }
    host.preflight(AnvilBlockAction::FallingEntity, block, state, position)?;
    let mut entity = host.fall_block(position, state)?;
    host.set_hurts_entities(&mut entity, FALL_DAMAGE_PER_DISTANCE, FALL_DAMAGE_MAX)?;
    Ok(AnvilTickOutcome::Falling)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        block_ticks::{self, BlockTickScheduler},
        placement,
        random::LegacyRandomSource,
        tick::TickPriority,
        world::{self, FlatWorld},
    };
    use anyhow::{bail, ensure};
    use std::convert::Infallible;

    #[derive(Debug, PartialEq, Eq)]
    enum Event {
        Preflight(AnvilBlockAction),
        Schedule(i64),
        Super(Direction),
        Below(BlockPosition),
        Air,
        Fire,
        Liquid,
        Replaceable,
        MinimumHeight,
    }

    /// Explicit native contract fixture: actual pinned state catalogue,
    /// FlatWorld, BLOCK queue/subtick primitive and retained native RNG. It has
    /// NO falling-entity owner and never invents an Entity or production host.
    struct NativeFixture {
        world: FlatWorld,
        queue: BlockTickScheduler,
        order: i64,
        random: LegacyRandomSource,
        events: Vec<Event>,
        super_available: bool,
    }
    fn state(name: &str) -> u32 {
        let catalogue: serde_json::Value =
            serde_json::from_str(include_str!("../../fixtures/placement-1.21.json")).unwrap();
        catalogue["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["name"] == name)
            .unwrap()["default"]
            .as_u64()
            .unwrap() as u32
    }
    impl NativeFixture {
        fn new() -> Result<Self> {
            let mut queue = BlockTickScheduler::new();
            block_ticks::load(&mut queue, (0, 0), Vec::new(), 100)?;
            Ok(Self {
                world: FlatWorld::new(0, 1, 10, 9, 39),
                queue,
                order: 17,
                random: LegacyRandomSource::new(9182),
                events: Vec::new(),
                super_available: true,
            })
        }
    }
    impl AnvilBlockHost for NativeFixture {
        type Block = u32;
        type BlockState = u32;
        type FallingEntity = Infallible;
        fn preflight(
            &mut self,
            action: AnvilBlockAction,
            block: &u32,
            state: &u32,
            _: BlockPosition,
        ) -> Result<()> {
            self.events.push(Event::Preflight(action));
            ensure!(placement::placement_block_min_state(*state)? == *block);
            ensure!(placement::in_block_tag(*state, "minecraft:anvil"));
            ensure!(
                action != AnvilBlockAction::FallingEntity,
                "actual falling entity owner unavailable"
            );
            ensure!(
                action != AnvilBlockAction::UpdateShape || self.super_available,
                "actual superclass delegate unavailable"
            );
            Ok(())
        }
        fn schedule_block_tick(&mut self, p: BlockPosition, block: &u32, delay: i32) -> Result<()> {
            self.events.push(Event::Schedule(self.order));
            block_ticks::schedule(
                &mut self.queue,
                p,
                *block,
                100,
                delay,
                TickPriority::Normal,
                &mut self.order,
            )?;
            Ok(())
        }
        fn superclass_update_shape(&mut self, update: ShapeUpdate<'_, u32>) -> Result<u32> {
            self.events.push(Event::Super(update.direction));
            // Pinned BlockBehaviour superclass is the identity function. This
            // fixture supplies only that verified pure delegate, not callbacks.
            Ok(*update.state)
        }
        fn block_state(&mut self, p: BlockPosition) -> Result<u32> {
            self.events.push(Event::Below(p));
            Ok(self.world.block(p.0, p.1, p.2))
        }
        fn state_is_air(&mut self, value: &u32) -> Result<bool> {
            self.events.push(Event::Air);
            Ok(placement::placement_block_name(*value)? == "minecraft:air")
        }
        fn state_is_fire_tag(&mut self, value: &u32) -> Result<bool> {
            self.events.push(Event::Fire);
            Ok(placement::in_block_tag(*value, "minecraft:fire"))
        }
        fn state_is_liquid(&mut self, value: &u32) -> Result<bool> {
            self.events.push(Event::Liquid);
            Ok(matches!(
                placement::placement_block_name(*value)?,
                "minecraft:water" | "minecraft:lava"
            ))
        }
        fn state_can_be_replaced(&mut self, value: &u32) -> Result<bool> {
            self.events.push(Event::Replaceable);
            let catalogue: serde_json::Value =
                serde_json::from_str(include_str!("../../fixtures/placement-1.21.json"))?;
            catalogue["blocks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|b| {
                    b["min"].as_u64().unwrap() <= u64::from(*value)
                        && u64::from(*value) <= b["max"].as_u64().unwrap()
                })
                .and_then(|b| b["replaceable"].as_bool())
                .ok_or_else(|| anyhow::anyhow!("fixture state predicate unavailable"))
        }
        fn minimum_build_height(&mut self) -> Result<i32> {
            self.events.push(Event::MinimumHeight);
            Ok(world::MIN_Y)
        }
        fn fall_block(&mut self, _: BlockPosition, _: &u32) -> Result<Infallible> {
            bail!("fixture has no actual falling entity owner")
        }
        fn set_hurts_entities(&mut self, entity: &mut Infallible, _: f32, _: i32) -> Result<()> {
            match *entity {}
        }
    }

    #[test]
    fn actual_block_queue_keeps_attempt_order_and_shape_super_phase() -> Result<()> {
        let mut host = NativeFixture::new()?;
        let state = state("minecraft:anvil");
        let block = block_ticks::key(state)?;
        let p = (0, 64, 0);
        let stone = 1;
        on_place(&mut host, &block, &state, p)?;
        for direction in [
            Direction::Down,
            Direction::Up,
            Direction::North,
            Direction::South,
            Direction::West,
            Direction::East,
        ] {
            let before = host.events.len();
            let order = host.order;
            let result = update_shape(
                &mut host,
                &block,
                ShapeUpdate {
                    state: &state,
                    direction,
                    neighbor_state: &stone,
                    position: p,
                    neighbor_position: (0, 63, 0),
                },
            )?;
            assert_eq!(result, state);
            assert_eq!(
                &host.events[before..],
                &[
                    Event::Preflight(AnvilBlockAction::UpdateShape),
                    Event::Schedule(order),
                    Event::Super(direction)
                ]
            );
        }
        assert_eq!(host.order, 24); // Every attempt, although one actual key survives.
        assert_eq!(host.queue.count(), 1);
        let ticks = host.queue.chunk((0, 0)).unwrap().saved_ticks(100);
        assert_eq!(ticks.len(), 1);
        assert_eq!(ticks[0].kind, block);
        assert_eq!(ticks[0].delay, 2);
        assert_eq!(ticks[0].priority, TickPriority::Normal);
        host.super_available = false;
        let order = host.order;
        assert!(update_shape(
            &mut host,
            &block,
            ShapeUpdate {
                state: &state,
                direction: Direction::Up,
                neighbor_state: &stone,
                position: p,
                neighbor_position: (0, 65, 0),
            }
        )
        .is_err());
        assert_eq!(host.order, order);
        Ok(())
    }

    #[test]
    fn stable_tick_queries_real_below_without_minimum_rng_or_falling_owner() -> Result<()> {
        let mut host = NativeFixture::new()?;
        let state = state("minecraft:anvil");
        let block = block_ticks::key(state)?;
        let p = (0, 64, 0);
        host.world.set_block(0, 63, 0, 1);
        let random = host.random.state48();
        let world = host.world.changes();
        assert_eq!(
            scheduled_tick(&mut host, &block, &state, p)?,
            AnvilTickOutcome::NotFreeBelow
        );
        assert_eq!(
            host.events,
            vec![
                Event::Preflight(AnvilBlockAction::ScheduledTick),
                Event::Below((0, 63, 0)),
                Event::Air,
                Event::Fire,
                Event::Liquid,
                Event::Replaceable
            ]
        );
        assert_eq!(host.random.state48(), random);
        assert_eq!(host.world.changes(), world);
        assert_eq!(host.queue.count(), 0);
        Ok(())
    }

    #[test]
    fn source_free_predicate_short_circuits_and_unavailable_fall_is_not_success() -> Result<()> {
        let mut host = NativeFixture::new()?;
        for (name, expected) in [
            ("minecraft:air", vec![Event::Air]),
            ("minecraft:fire", vec![Event::Air, Event::Fire]),
            (
                "minecraft:water",
                vec![Event::Air, Event::Fire, Event::Liquid],
            ),
            (
                "minecraft:short_grass",
                vec![Event::Air, Event::Fire, Event::Liquid, Event::Replaceable],
            ),
        ] {
            host.events.clear();
            assert!(is_free(&mut host, &state(name))?);
            assert_eq!(host.events, expected);
        }
        let state = state("minecraft:anvil");
        let block = block_ticks::key(state)?;
        host.events.clear();
        let random = host.random.state48();
        let world = host.world.changes();
        assert!(scheduled_tick(&mut host, &block, &state, (0, 65, 0)).is_err());
        assert_eq!(
            host.events,
            vec![
                Event::Preflight(AnvilBlockAction::ScheduledTick),
                Event::Below((0, 64, 0)),
                Event::Air,
                Event::MinimumHeight,
                Event::Preflight(AnvilBlockAction::FallingEntity)
            ]
        );
        assert_eq!(host.random.state48(), random);
        assert_eq!(host.world.changes(), world);
        host.events.clear();
        assert_eq!(
            scheduled_tick(&mut host, &block, &state, (0, world::MIN_Y - 1, 0))?,
            AnvilTickOutcome::BelowMinimumBuildHeight
        );
        assert_eq!(
            host.events,
            vec![
                Event::Preflight(AnvilBlockAction::ScheduledTick),
                Event::Below((0, world::MIN_Y - 2, 0)),
                Event::Air,
                Event::MinimumHeight
            ]
        );
        Ok(())
    }
}
