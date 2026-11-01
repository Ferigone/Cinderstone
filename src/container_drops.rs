//! Minecraft Java 1.21 `Containers.dropContents` / `dropItemStack` construction.
//!
//! The caller stages the actual container slots, level random stream and entity
//! counter, then commits the returned items through its native world-add owner.
//! This function is the empty-listener construction path, not a replacement for
//! callbacks that may mutate the world or random stream during addFreshEntity.
//! Source: pinned `bqn`, `cjh` and `ayw` bytecode; no Gaussian sampler is used.

use crate::{
    entity::Position,
    geometry::Vec3,
    inventory::ItemStack,
    item_entity::ItemState,
    random::{LegacyRandomSource, RandomSource},
};
use anyhow::{ensure, Context, Result};

/// Actual `RandomSource.triangle` deviation used by Containers in 1.21.
const DROP_DEVIATION: f64 = 0.11485000171139836;
const ITEM_WIDTH: f64 = 0.25;

/// Stage all slots in container encounter order, including the random draws
/// made for empty slots. The same position is reused for every split from one
/// slot. Every returned entity retains its independent RNG after construction.
///
/// `next_entity_id` is the next unused ID, not the last allocated ID. The
/// factory must supply the actual Entity RandomSource.create stream; it runs
/// after the two level draws in ItemEntity's constructor argument evaluation.
/// Inputs are intentionally mutable staging owners: a factory/error partway
/// through can leave them advanced. Invoke on clones before canonical commit.
/// Native world-add must have no RNG-consuming or item-mutating listeners;
/// otherwise use the callback variant to execute them between constructions.
pub fn drop_contents_with_factory(
    position: Vec3,
    slots: &mut [ItemStack],
    level_random: &mut impl RandomSource,
    next_entity_id: &mut i32,
    factory: impl FnMut() -> Result<LegacyRandomSource>,
) -> Result<Vec<(ItemState, LegacyRandomSource)>> {
    let mut entries = Vec::new();
    drop_contents_with_callbacks(
        position,
        slots,
        level_random,
        next_entity_id,
        factory,
        |_, item, random| {
            entries.push((item, random));
            Ok(())
        },
    )?;
    Ok(entries)
}

/// The same source loop with synchronous addFreshEntity placement. A host with
/// real callbacks may advance the supplied level RNG in `add` before the next
/// split or slot is visited. Errors do not roll back these staging owners.
pub fn drop_contents_with_callbacks<R: RandomSource>(
    position: Vec3,
    slots: &mut [ItemStack],
    level_random: &mut R,
    next_entity_id: &mut i32,
    mut factory: impl FnMut() -> Result<LegacyRandomSource>,
    mut add: impl FnMut(&mut R, ItemState, LegacyRandomSource) -> Result<()>,
) -> Result<()> {
    ensure!(
        [position.x, position.y, position.z]
            .iter()
            .all(|v| v.is_finite()),
        "nonfinite container drop position"
    );
    ensure!(
        *next_entity_id > 0,
        "container drop entity counter must be positive"
    );
    // Decode every component before requesting real factory seeds or draws.
    for stack in slots.iter() {
        stack.encode_canonical()?;
    }
    for stack in slots {
        // This precedes isEmpty in the shipped method, including for EMPTY.
        let span = 1.0 - ITEM_WIDTH;
        let half_width = ITEM_WIDTH / 2.0;
        let x = position.x.floor() + level_random.next_f64() * span + half_width;
        let y = position.y.floor() + level_random.next_f64() * span;
        let z = position.z.floor() + level_random.next_f64() * span + half_width;
        while !stack.is_empty() {
            let split_count = (level_random.next_bounded_i32(21)? + 10).min(stack.count);
            let mut split = stack.clone();
            split.count = split_count;
            stack.count -= split_count;

            // ItemEntity(Level, x, y, z, stack) evaluates its default velocity
            // arguments before the delegated constructor creates Entity's RNG.
            // Containers replaces the velocity, but these draws still occur.
            let _ = level_random.next_f64();
            let _ = level_random.next_f64();
            let entity_id = *next_entity_id;
            let next_id = entity_id
                .checked_add(1)
                .context("entity ID space exhausted")?;
            let mut entity_random = factory()?;
            let uuid = random_uuid(&mut entity_random);
            let bob_offset = entity_random.next_f32() * std::f32::consts::PI * 2.0_f32;
            let yaw = entity_random.next_f32() * 360.0_f32;
            let motion = Vec3::new(
                triangle(level_random, 0.0),
                triangle(level_random, 0.2),
                triangle(level_random, 0.0),
            );
            let mut item = ItemState::new(
                entity_id,
                uuid,
                Position {
                    x,
                    y,
                    z,
                    yaw,
                    pitch: 0.0,
                    on_ground: false,
                },
                motion,
                split,
            )?;
            item.bob_offset = bob_offset;
            // Containers does not call setDefaultPickUpDelay; keep the actual
            // constructor's zero delay and absent target/thrower/stat owners.
            *next_entity_id = next_id;
            add(level_random, item, entity_random)?;
        }
    }
    Ok(())
}

fn triangle(random: &mut impl RandomSource, mean: f64) -> f64 {
    mean + DROP_DEVIATION * (random.next_f64() - random.next_f64())
}

fn random_uuid(random: &mut impl RandomSource) -> [u8; 16] {
    let hi = (random.next_i64() as u64 & 0xffff_ffff_ffff_0fff) | 0x4000;
    let lo = (random.next_i64() as u64 & 0x3fff_ffff_ffff_ffff) | 0x8000_0000_0000_0000;
    let mut uuid = [0; 16];
    uuid[..8].copy_from_slice(&hi.to_be_bytes());
    uuid[8..].copy_from_slice(&lo.to_be_bytes());
    uuid
}
