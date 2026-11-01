//! Java 1.21 shulker-box animation and opener callbacks over real world owners.
//! The actual 27-slot block inventory/menu is borrowed separately; this module
//! never constructs a second inventory or replaces entity collision movement.
use crate::{
    geometry::{Aabb, Direction, Vec3},
    inventory::{self, ItemStack},
};
use anyhow::{ensure, Result};

pub const MENU_TYPE: i32 = 20;
pub const CONTAINER_SLOTS: usize = 27;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationStatus {
    Closed,
    Opening,
    Opened,
    Closing,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShulkerSound {
    Open,
    Close,
}
#[derive(Clone, Debug)]
pub struct ShulkerLifecycle {
    open_count: i32,
    status: AnimationStatus,
    progress: f32,
    progress_old: f32,
}
impl Default for ShulkerLifecycle {
    fn default() -> Self {
        Self {
            open_count: 0,
            status: AnimationStatus::Closed,
            progress: 0.,
            progress_old: 0.,
        }
    }
}

/// Bind to one actual ShulkerBoxBlockEntity identity, its captured state/level,
/// actual player and entity index. All callbacks belong to the enclosing real
/// world transaction; on failure it must roll back its own changes/RNG as well.
/// No default empty entity query or no-op movement/sound implementation exists.
pub trait ShulkerHost {
    fn removed(&self) -> bool;
    fn player_is_spectator(&self, player_entity_id: i32) -> Result<bool>;
    fn position(&self) -> (i32, i32, i32);
    fn facing(&self) -> Result<Direction>;
    fn block_is_shulker_box(&self) -> Result<bool>;
    /// Actual Level.blockEvent(pos, capturedBlock,1,count). Queue/process it
    /// through the real block-event phase; do not directly trigger animation.
    fn block_event(&mut self, id: i32, count: i32) -> Result<()>;
    fn game_event(&mut self, player_entity_id: i32, opening: bool) -> Result<()>;
    fn next_level_random_float(&mut self) -> Result<f32>;
    fn play_sound(&mut self, sound: ShulkerSound, volume: f32, pitch: f32) -> Result<()>;
    fn update_neighbor_shapes(&mut self, flags: i32) -> Result<()>;
    fn update_neighbors_at(&mut self) -> Result<()>;
    fn no_collision(&mut self, bounds: Aabb) -> Result<bool>;
    /// Actual Level.getEntities(null,bounds) encounter order and live identities.
    fn entities_in(&mut self, bounds: Aabb) -> Result<Vec<i32>>;
    fn entity_push_reaction_ignores(&mut self, entity_id: i32) -> Result<bool>;
    /// Actual Entity.move(MoverType.SHULKER_BOX,delta), including collision and
    /// native entity/world callbacks. Direct coordinate assignment is incorrect.
    fn move_entity_shulker_box(&mut self, entity_id: i32, delta: Vec3) -> Result<()>;
}

impl ShulkerLifecycle {
    pub fn open_count(&self) -> i32 {
        self.open_count
    }
    pub fn status(&self) -> AnimationStatus {
        self.status
    }
    pub fn progress(&self, partial_tick: f32) -> f32 {
        self.progress_old + partial_tick * (self.progress - self.progress_old)
    }
    pub fn is_closed(&self) -> bool {
        self.status == AnimationStatus::Closed
    }

    pub fn start_open(&mut self, player: i32, host: &mut impl ShulkerHost) -> Result<()> {
        if host.removed() || host.player_is_spectator(player)? {
            return Ok(());
        }
        let mut next = self.clone();
        if next.open_count < 0 {
            next.open_count = 0;
        }
        next.open_count = next.open_count.wrapping_add(1);
        host.block_event(1, next.open_count)?;
        if next.open_count == 1 {
            host.game_event(player, true)?;
            play_sound(host, ShulkerSound::Open)?;
        }
        *self = next;
        Ok(())
    }
    pub fn stop_open(&mut self, player: i32, host: &mut impl ShulkerHost) -> Result<()> {
        if host.removed() || host.player_is_spectator(player)? {
            return Ok(());
        }
        let mut next = self.clone();
        next.open_count = next.open_count.wrapping_sub(1);
        host.block_event(1, next.open_count)?;
        if next.open_count <= 0 {
            host.game_event(player, false)?;
            play_sound(host, ShulkerSound::Close)?;
        }
        *self = next;
        Ok(())
    }
    /// Invoke only when the actual queued block event reaches this same owner.
    /// False means the caller must delegate to its real base triggerEvent path.
    pub fn trigger_event(&mut self, id: i32, count: i32) -> bool {
        if id != 1 {
            return false;
        }
        self.open_count = count;
        if count == 0 {
            self.status = AnimationStatus::Closing;
        }
        if count == 1 {
            self.status = AnimationStatus::Opening;
        }
        true
    }
    pub fn bounding_box(&self, facing: Direction) -> Aabb {
        progress_aabb(facing, 0.5_f32 * self.progress(1.))
    }
    /// Source ShulkerBoxBlock.canOpen: the actual collision query is required
    /// only when fully closed; another opener does not restart that check.
    pub fn can_open(&self, host: &mut impl ShulkerHost) -> Result<bool> {
        if !self.is_closed() {
            return Ok(true);
        }
        let bounds = progress_delta_aabb(host.facing()?, 0., 0.5)
            .moved(position(host))
            .inflate(Vec3::new(-1e-6, -1e-6, -1e-6));
        host.no_collision(bounds)
    }
    /// Actual block-entity ticker phase. Preserve Java float additions and
    /// endpoint comparisons; count of elapsed wall-clock ticks is irrelevant.
    pub fn tick(&mut self, host: &mut impl ShulkerHost) -> Result<()> {
        let mut next = self.clone();
        next.progress_old = next.progress;
        match next.status {
            AnimationStatus::Closed => next.progress = 0.,
            AnimationStatus::Opening => {
                next.progress += 0.1_f32;
                if next.progress_old == 0. {
                    neighbor_updates(host)?;
                }
                if next.progress >= 1. {
                    next.status = AnimationStatus::Opened;
                    next.progress = 1.;
                    neighbor_updates(host)?;
                }
                next.move_collided_entities(host)?;
            }
            AnimationStatus::Closing => {
                next.progress -= 0.1_f32;
                if next.progress_old == 1. {
                    neighbor_updates(host)?;
                }
                if next.progress <= 0. {
                    next.status = AnimationStatus::Closed;
                    next.progress = 0.;
                    neighbor_updates(host)?;
                }
            }
            AnimationStatus::Opened => next.progress = 1.,
        }
        *self = next;
        Ok(())
    }
    fn move_collided_entities(&self, host: &mut impl ShulkerHost) -> Result<()> {
        if !host.block_is_shulker_box()? {
            return Ok(());
        }
        let facing = host.facing()?;
        let bounds =
            progress_delta_aabb(facing, self.progress_old, self.progress).moved(position(host));
        let size = bounds.size();
        let (x, y, z) = facing.step();
        let delta = Vec3::new(
            (size.x + 0.01) * x as f64,
            (size.y + 0.01) * y as f64,
            (size.z + 0.01) * z as f64,
        );
        for entity in host.entities_in(bounds)? {
            if !host.entity_push_reaction_ignores(entity)? {
                host.move_entity_shulker_box(entity, delta)?;
            }
        }
        Ok(())
    }
}
fn position(host: &impl ShulkerHost) -> Vec3 {
    let (x, y, z) = host.position();
    Vec3::new(x as f64, y as f64, z as f64)
}
fn neighbor_updates(host: &mut impl ShulkerHost) -> Result<()> {
    host.update_neighbor_shapes(3)?;
    host.update_neighbors_at()
}
fn play_sound(host: &mut impl ShulkerHost, sound: ShulkerSound) -> Result<()> {
    let random = host.next_level_random_float()?;
    ensure!(
        (0.0..1.0).contains(&random),
        "invalid actual Level RNG float"
    );
    host.play_sound(sound, 0.5, random * 0.1_f32 + 0.9_f32)
}

/// Shulker.getProgressDeltaAabb at scale1. Both expansion and contraction are
/// source operations; the swept slab must not be replaced by the full box.
pub fn progress_delta_aabb(facing: Direction, first: f32, second: f32) -> Aabb {
    let (x, y, z) = facing.step();
    let high = first.max(second) as f64;
    let low = first.min(second) as f64;
    Aabb::new(0., 0., 0., 1., 1., 1.)
        .expand_towards(Vec3::new(x as f64 * high, y as f64 * high, z as f64 * high))
        .contract(Vec3::new(
            -x as f64 * (1. + low),
            -y as f64 * (1. + low),
            -z as f64 * (1. + low),
        ))
}
pub fn progress_aabb(facing: Direction, progress: f32) -> Aabb {
    progress_delta_aabb(facing, -1., progress)
}

/// ShulkerBoxSlot and sided insertion reject all actual vanilla shulker-box
/// block items, including uncolored and every colored variant.
pub fn may_place(stack: &ItemStack) -> bool {
    !inventory::item_default_block_state(stack.item_id)
        .and_then(inventory::block_state_name)
        .is_some_and(|name| name == "shulker_box" || name.ends_with("_shulker_box"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crafting::RecipeBook;
    struct Host {
        calls: Vec<String>,
        moves: Vec<(i32, Vec3)>,
        spectator: bool,
        removed: bool,
        collision_free: bool,
    }
    impl Host {
        fn new() -> Self {
            Self {
                calls: vec![],
                moves: vec![],
                spectator: false,
                removed: false,
                collision_free: false,
            }
        }
    }
    impl ShulkerHost for Host {
        fn removed(&self) -> bool {
            self.removed
        }
        fn player_is_spectator(&self, _: i32) -> Result<bool> {
            Ok(self.spectator)
        }
        fn position(&self) -> (i32, i32, i32) {
            (4, 5, 6)
        }
        fn facing(&self) -> Result<Direction> {
            Ok(Direction::Up)
        }
        fn block_is_shulker_box(&self) -> Result<bool> {
            Ok(true)
        }
        fn block_event(&mut self, id: i32, count: i32) -> Result<()> {
            self.calls.push(format!("block:{id}:{count}"));
            Ok(())
        }
        fn game_event(&mut self, player: i32, opening: bool) -> Result<()> {
            self.calls.push(format!("game:{player}:{opening}"));
            Ok(())
        }
        fn next_level_random_float(&mut self) -> Result<f32> {
            self.calls.push("rng".into());
            Ok(0.25)
        }
        fn play_sound(&mut self, sound: ShulkerSound, volume: f32, pitch: f32) -> Result<()> {
            assert_eq!(volume, 0.5);
            assert_eq!(pitch, 0.25_f32 * 0.1 + 0.9);
            self.calls.push(format!("sound:{sound:?}"));
            Ok(())
        }
        fn update_neighbor_shapes(&mut self, flags: i32) -> Result<()> {
            self.calls.push(format!("shape:{flags}"));
            Ok(())
        }
        fn update_neighbors_at(&mut self) -> Result<()> {
            self.calls.push("neighbors".into());
            Ok(())
        }
        fn no_collision(&mut self, _: Aabb) -> Result<bool> {
            self.calls.push("collision".into());
            Ok(self.collision_free)
        }
        fn entities_in(&mut self, _: Aabb) -> Result<Vec<i32>> {
            self.calls.push("entities".into());
            Ok(vec![1, 2])
        }
        fn entity_push_reaction_ignores(&mut self, id: i32) -> Result<bool> {
            Ok(id == 1)
        }
        fn move_entity_shulker_box(&mut self, id: i32, delta: Vec3) -> Result<()> {
            self.moves.push((id, delta));
            Ok(())
        }
    }
    #[test]
    fn opener_events_defer_animation_and_opening_moves_real_entities() -> Result<()> {
        let mut state = ShulkerLifecycle::default();
        let mut host = Host::new();
        assert!(!state.can_open(&mut host)?);
        host.calls.clear();
        state.start_open(7, &mut host)?;
        assert_eq!(state.status(), AnimationStatus::Closed);
        assert_eq!(
            host.calls,
            ["block:1:1", "game:7:true", "rng", "sound:Open"]
        );
        state.trigger_event(1, 1);
        host.calls.clear();
        assert!(state.can_open(&mut host)?);
        assert!(host.calls.is_empty());
        state.tick(&mut host)?;
        assert_eq!(host.calls, ["shape:3", "neighbors", "entities"]);
        assert_eq!(host.moves.len(), 1);
        assert_eq!(host.moves[0].0, 2);
        assert_eq!(host.moves[0].1.x, 0.);
        assert!((host.moves[0].1.y - (0.1_f32 as f64 + 0.01)).abs() < 1e-12);
        for _ in 1..10 {
            state.tick(&mut host)?;
        }
        assert_eq!(state.status(), AnimationStatus::Opened);
        assert_eq!(state.progress(1.), 1.);
        let moves = host.moves.len();
        state.stop_open(7, &mut host)?;
        assert_eq!(state.status(), AnimationStatus::Opened);
        state.trigger_event(1, 0);
        for _ in 0..10 {
            state.tick(&mut host)?;
        }
        assert!(state.is_closed());
        assert_eq!(host.moves.len(), moves);
        Ok(())
    }
    #[test]
    fn spectator_has_no_opener_callbacks_and_nested_boxes_are_rejected() -> Result<()> {
        let mut state = ShulkerLifecycle::default();
        let mut host = Host::new();
        host.spectator = true;
        state.start_open(7, &mut host)?;
        state.stop_open(7, &mut host)?;
        assert_eq!(state.open_count(), 0);
        assert!(host.calls.is_empty());
        let book = RecipeBook::vanilla();
        for name in ["shulker_box", "white_shulker_box", "red_shulker_box"] {
            assert!(!may_place(&ItemStack::new(book.item_id(name).unwrap(), 1)?));
        }
        assert!(may_place(&ItemStack::new(
            book.item_id("dirt").unwrap(),
            1
        )?));
        let bounds = state.bounding_box(Direction::Up);
        assert_eq!(bounds, Aabb::new(0., 0., 0., 1., 1., 1.));
        Ok(())
    }
}
