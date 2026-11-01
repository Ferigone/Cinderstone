//! Packet-facing Player walking and landing effects for the native world.
//!
//! Calls preserve Player.playSound's excluded source player and the level's
//! required threadSafeRandom seed boundary. Particle audiences use the player's
//! block centre, matching ServerLevel.sendParticles, rather than sound range.
use anyhow::{ensure, Result};
use mc121_rust::{
    entity_move::BlockPos,
    entity_state::{java_floor, EntityPhysicalState},
    geometry::Vec3,
    main_player_motion::FallParticles,
    placement,
    protocol::{Packet, Writer},
    random::LegacyRandomSource,
    runtime_world::RuntimeWorld,
};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PlayerStepState {
    pub crystal_sound_intensity: f32,
    pub last_crystal_sound_play_tick: i32,
}

#[derive(Clone, Debug)]
pub(super) struct PlayerMotionBroadcast {
    pub packet: Packet,
    pub origin: Vec3,
    pub radius: f64,
    pub excluded: Option<i32>,
    pub use_block_center: bool,
}
impl PlayerMotionBroadcast {
    pub fn in_range(&self, player_id: i32, position: Vec3, same_dimension: bool) -> bool {
        if !same_dimension || self.excluded == Some(player_id) {
            return false;
        }
        let position = if self.use_block_center {
            Vec3::new(
                f64::from(java_floor(position.x)) + 0.5,
                f64::from(java_floor(position.y)) + 0.5,
                f64::from(java_floor(position.z)) + 0.5,
            )
        } else {
            position
        };
        let dx = position.x - self.origin.x;
        let dy = position.y - self.origin.y;
        let dz = position.z - self.origin.z;
        (dx * dx + dy * dy) + dz * dz < self.radius * self.radius
    }
}

#[derive(Clone, Copy)]
struct StepSound {
    id: i32,
    volume: f32,
    pitch: f32,
}
fn step_sound(block: u32) -> Result<StepSound> {
    ensure!(block < 26_684, "unknown player step block state {block}");
    let index = STEP_SOUND_RANGES.partition_point(|entry| entry.0 <= block) - 1;
    let (_, id, volume, pitch) = STEP_SOUND_RANGES[index];
    Ok(StepSound {
        id: i32::from(id),
        volume: f32::from_bits(volume),
        pitch: f32::from_bits(pitch),
    })
}
pub(super) fn sound(
    state: &EntityPhysicalState,
    id: i32,
    volume: f32,
    pitch: f32,
    next_sound_seed: &mut impl FnMut() -> Result<i64>,
    emit: &mut impl FnMut(PlayerMotionBroadcast) -> Result<()>,
) -> Result<()> {
    // Player's override directly calls Level.playSound(this,...), including for
    // a silent Player; Entity's separate isSilent gate is not invoked here.
    let seed = next_sound_seed()?;
    let position = state.position_vec();
    let mut out = Writer::new();
    out.varint(id + 1)
        .varint(7) // PLAYERS
        .i32((position.x * 8.) as i32)
        .i32((position.y * 8.) as i32)
        .i32((position.z * 8.) as i32)
        .f32(volume)
        .f32(pitch)
        .i64(seed);
    emit(PlayerMotionBroadcast {
        packet: Packet {
            id: 0x68,
            payload: out.into_inner(),
        },
        origin: position,
        // Every extracted step SoundEvent and the amethyst chime uses the
        // original variable-range SoundEvent form.
        radius: f64::from(if volume > 1. { 16_f32 * volume } else { 16. }),
        excluded: Some(state.entity_id),
        use_block_center: false,
    })
}

/// Borrow the actual player RNG and required level-seed/effect boundaries.
pub(super) struct PlayerStepEffects<'a, Seed, Emit> {
    pub entity_random: &'a mut LegacyRandomSource,
    pub next_sound_seed: Seed,
    pub emit: Emit,
}

/// Original dry Player.playStepSound followed by Entity's amethyst overlay.
/// Entity RNG and persistent crystal fields are supplied by the player owner;
/// the seed callback consumes the actual shared level sound stream per sound.
pub(super) fn walking_step_sound(
    world: &mut RuntimeWorld,
    state: &EntityPhysicalState,
    position: BlockPos,
    block: u32,
    step_state: &mut PlayerStepState,
    effects: &mut PlayerStepEffects<
        '_,
        impl FnMut() -> Result<i64>,
        impl FnMut(PlayerMotionBroadcast) -> Result<()>,
    >,
) -> Result<()> {
    ensure!(
        !state.was_touching_water,
        "player water step sound needs the actual swim sound context"
    );
    // Validate the supplied state before tag lookup can treat unknown IDs as
    // missing membership. Query the upper block even if its sound is unused.
    let original = step_sound(block)?;
    let above = world.block(position.0, position.1.wrapping_add(1), position.2)?;
    let upper = step_sound(above)?;
    let inside = placement::in_block_tag(above, "minecraft:inside_step_sound_blocks");
    let combination = placement::in_block_tag(above, "minecraft:combination_step_sound_blocks");
    let primary = if inside || combination {
        upper
    } else {
        original
    };
    sound(
        state,
        primary.id,
        primary.volume * 0.15_f32,
        primary.pitch,
        &mut effects.next_sound_seed,
        &mut effects.emit,
    )?;
    if combination {
        sound(
            state,
            original.id,
            original.volume * 0.05_f32,
            original.pitch * 0.8_f32,
            &mut effects.next_sound_seed,
            &mut effects.emit,
        )?;
    }
    if placement::in_block_tag(block, "minecraft:crystal_sound_blocks")
        && state.tick_count >= step_state.last_crystal_sound_play_tick.wrapping_add(20)
    {
        let elapsed = state
            .tick_count
            .wrapping_sub(step_state.last_crystal_sound_play_tick);
        step_state.crystal_sound_intensity *= 0.997_f64.powf(f64::from(elapsed)) as f32;
        let increased = step_state.crystal_sound_intensity + 0.07_f32;
        step_state.crystal_sound_intensity = if increased.is_nan() || increased < 1. {
            increased
        } else {
            1.
        };
        let pitch = 0.5_f32
            + (step_state.crystal_sound_intensity * effects.entity_random.next_f32()) * 1.2_f32;
        let volume = 0.1_f32 + step_state.crystal_sound_intensity * 1.2_f32;
        sound(
            state,
            30,
            volume,
            pitch,
            &mut effects.next_sound_seed,
            &mut effects.emit,
        )?;
        step_state.last_crystal_sound_play_tick = state.tick_count;
    }
    Ok(())
}

/// ServerLevel.sendParticles(new BlockParticleOption(BLOCK,state),false,...).
/// No server RNG is consumed: clients independently sample the packet's count.
pub(super) fn fall_particles(
    particles: &FallParticles,
    emit: &mut impl FnMut(PlayerMotionBroadcast) -> Result<()>,
) -> Result<()> {
    ensure!(
        particles.block_state < 26_684,
        "unknown fall-particle block state"
    );
    let mut out = Writer::new();
    out.bool(false)
        .f64(particles.position.x)
        .f64(particles.position.y)
        .f64(particles.position.z)
        .f32(particles.offset.x as f32)
        .f32(particles.offset.y as f32)
        .f32(particles.offset.z as f32)
        .f32(particles.speed as f32)
        .i32(particles.count)
        .varint(1) // original ParticleTypes.BLOCK registry ID
        .varint(particles.block_state as i32);
    emit(PlayerMotionBroadcast {
        packet: Packet {
            id: 0x29,
            payload: out.into_inner(),
        },
        origin: particles.position,
        radius: 32.,
        excluded: None,
        use_block_center: true,
    })
}

/// Actual built-in ATTRIBUTE registry order from the same original bootstrap.
pub(super) const ATTRIBUTE_KEYS: [&str; 31] = [
    "minecraft:generic.armor",
    "minecraft:generic.armor_toughness",
    "minecraft:generic.attack_damage",
    "minecraft:generic.attack_knockback",
    "minecraft:generic.attack_speed",
    "minecraft:player.block_break_speed",
    "minecraft:player.block_interaction_range",
    "minecraft:generic.burning_time",
    "minecraft:generic.explosion_knockback_resistance",
    "minecraft:player.entity_interaction_range",
    "minecraft:generic.fall_damage_multiplier",
    "minecraft:generic.flying_speed",
    "minecraft:generic.follow_range",
    "minecraft:generic.gravity",
    "minecraft:generic.jump_strength",
    "minecraft:generic.knockback_resistance",
    "minecraft:generic.luck",
    "minecraft:generic.max_absorption",
    "minecraft:generic.max_health",
    "minecraft:player.mining_efficiency",
    "minecraft:generic.movement_efficiency",
    "minecraft:generic.movement_speed",
    "minecraft:generic.oxygen_bonus",
    "minecraft:generic.safe_fall_distance",
    "minecraft:generic.scale",
    "minecraft:player.sneaking_speed",
    "minecraft:zombie.spawn_reinforcements",
    "minecraft:generic.step_height",
    "minecraft:player.submerged_mining_speed",
    "minecraft:player.sweeping_damage_ratio",
    "minecraft:generic.water_movement_efficiency",
];

// Original unchanged 1.21 dtc.w()/dnm.d(),a(),b() results, run-length encoded
// over all 26,684 BlockState IDs. All step events have no fixed sound range.
// server-inner-1.21.jar SHA256:
// 8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01
const STEP_SOUND_RANGES: &[(u32, u16, u32, u32)] = &[
    (0, 1369, 0x3f800000, 0x3f800000),
    (8, 632, 0x3f800000, 0x3f800000),
    (10, 637, 0x3f800000, 0x3f800000),
    (14, 1369, 0x3f800000, 0x3f800000),
    (15, 1573, 0x3f800000, 0x3f800000),
    (20, 274, 0x3f800000, 0x3f800000),
    (21, 1573, 0x3f800000, 0x3f800000),
    (23, 117, 0x3f800000, 0x3f800000),
    (25, 632, 0x3f800000, 0x3f800000),
    (35, 279, 0x3f800000, 0x3f800000),
    (37, 632, 0x3f800000, 0x3f800000),
    (79, 1369, 0x3f800000, 0x3f800000),
    (80, 940, 0x3f800000, 0x3f800000),
    (112, 1189, 0x3f800000, 0x3f800000),
    (113, 537, 0x3f800000, 0x3f800000),
    (117, 1189, 0x3f800000, 0x3f800000),
    (118, 637, 0x3f800000, 0x3f800000),
    (119, 542, 0x3f800000, 0x3f800000),
    (123, 1369, 0x3f800000, 0x3f800000),
    (124, 402, 0x3f800000, 0x3f800000),
    (125, 1369, 0x3f800000, 0x3f800000),
    (126, 402, 0x3f800000, 0x3f800000),
    (127, 1369, 0x3f800000, 0x3f800000),
    (128, 402, 0x3f800000, 0x3f800000),
    (129, 1165, 0x3f800000, 0x3f800000),
    (130, 1573, 0x3f800000, 0x3f800000),
    (145, 274, 0x3f800000, 0x3f800000),
    (148, 1573, 0x3f800000, 0x3f800000),
    (154, 813, 0x3f800000, 0x3f800000),
    (156, 860, 0x3f800000, 0x3f800000),
    (159, 117, 0x3f800000, 0x3f800000),
    (162, 1573, 0x3f800000, 0x3f800000),
    (174, 274, 0x3f800000, 0x3f800000),
    (177, 1573, 0x3f800000, 0x3f800000),
    (186, 117, 0x3f800000, 0x3f800000),
    (189, 1573, 0x3f800000, 0x3f800000),
    (204, 274, 0x3f800000, 0x3f800000),
    (207, 1573, 0x3f800000, 0x3f800000),
    (228, 274, 0x3f800000, 0x3f800000),
    (231, 1573, 0x3f800000, 0x3f800000),
    (237, 632, 0x3f800000, 0x3f800000),
    (377, 284, 0x3f800000, 0x3f800000),
    (405, 632, 0x3f800000, 0x3f800000),
    (461, 104, 0x3f800000, 0x3f800000),
    (517, 1353, 0x3f800000, 0x3f800000),
    (518, 1529, 0x3f800000, 0x3f800000),
    (519, 597, 0x3f800000, 0x3f800000),
    (520, 1369, 0x3f800000, 0x3f800000),
    (521, 402, 0x3f800000, 0x3f800000),
    (522, 1369, 0x3f800000, 0x3f800000),
    (538, 1573, 0x3f800000, 0x3f800000),
    (1944, 822, 0x3f800000, 0x3fc00000),
    (1992, 1369, 0x3f800000, 0x3f800000),
    (2004, 321, 0x3f800000, 0x3f800000),
    (2005, 632, 0x3f800000, 0x3f800000),
    (2008, 1523, 0x3f800000, 0x3f800000),
    (2011, 1369, 0x3f800000, 0x3f800000),
    (2047, 1578, 0x3f800000, 0x3f800000),
    (2063, 1369, 0x3f800000, 0x3f800000),
    (2075, 632, 0x3f800000, 0x3f800000),
    (2091, 822, 0x3f800000, 0x3fc00000),
    (2093, 1369, 0x3f800000, 0x3f800000),
    (2094, 632, 0x3f800000, 0x3f800000),
    (2096, 1573, 0x3f800000, 0x3f800000),
    (2097, 313, 0x3f800000, 0x3f800000),
    (2353, 1369, 0x3f800000, 0x3f800000),
    (2355, 1573, 0x3f800000, 0x3f800000),
    (2360, 1578, 0x3f800000, 0x3f800000),
    (2873, 822, 0x3f800000, 0x3fc00000),
    (2874, 1573, 0x3f800000, 0x3f800000),
    (2978, 1369, 0x3f800000, 0x3f800000),
    (4275, 402, 0x3f800000, 0x3f800000),
    (4276, 822, 0x3f800000, 0x3fc00000),
    (4277, 1573, 0x3f800000, 0x3f800000),
    (4278, 632, 0x3f800000, 0x3f800000),
    (4286, 637, 0x3f800000, 0x3f800000),
    (4294, 1369, 0x3f800000, 0x3f800000),
    (4302, 1573, 0x3f800000, 0x3f800000),
    (4430, 274, 0x3f800000, 0x3f800000),
    (4462, 1573, 0x3f800000, 0x3f800000),
    (4558, 117, 0x3f800000, 0x3f800000),
    (4590, 1573, 0x3f800000, 0x3f800000),
    (4654, 767, 0x3f800000, 0x3f800000),
    (4662, 822, 0x3f800000, 0x3fc00000),
    (4682, 1369, 0x3f800000, 0x3f800000),
    (4762, 1573, 0x3f800000, 0x3f800000),
    (4794, 274, 0x3f800000, 0x3f800000),
    (4802, 1573, 0x3f800000, 0x3f800000),
    (4826, 117, 0x3f800000, 0x3f800000),
    (4834, 653, 0x3f800000, 0x3f800000),
    (5090, 285, 0x3f800000, 0x3f800000),
    (5154, 653, 0x3f800000, 0x3f800000),
    (5282, 663, 0x3f800000, 0x3f800000),
    (5410, 653, 0x3f800000, 0x3f800000),
    (5474, 668, 0x3f800000, 0x3f800000),
    (5538, 653, 0x3f800000, 0x3f800000),
    (5570, 285, 0x3f800000, 0x3f800000),
    (5578, 653, 0x3f800000, 0x3f800000),
    (5602, 663, 0x3f800000, 0x3f800000),
    (5618, 668, 0x3f800000, 0x3f800000),
    (5626, 1369, 0x3f800000, 0x3f800000),
    (5652, 822, 0x3f800000, 0x3fc00000),
    (5716, 1573, 0x3f800000, 0x3f800000),
    (5726, 274, 0x3f800000, 0x3f800000),
    (5728, 1573, 0x3f800000, 0x3f800000),
    (5732, 117, 0x3f800000, 0x3f800000),
    (5734, 1369, 0x3f800000, 0x3f800000),
    (5736, 402, 0x3f800000, 0x3f800000),
    (5738, 1573, 0x3f800000, 0x3f800000),
    (5748, 1369, 0x3f800000, 0x3f800000),
    (5772, 1342, 0x3f800000, 0x3f800000),
    (5780, 597, 0x3f800000, 0x3f800000),
    (5781, 1342, 0x3f800000, 0x3f800000),
    (5782, 1578, 0x3f800000, 0x3f800000),
    (5798, 637, 0x3f800000, 0x3f800000),
    (5799, 632, 0x3f800000, 0x3f800000),
    (5815, 1573, 0x3f800000, 0x3f800000),
    (5849, 982, 0x3f800000, 0x3f800000),
    (5850, 1287, 0x3f800000, 0x3f800000),
    (5851, 1292, 0x3f800000, 0x3f800000),
    (5852, 131, 0x3f800000, 0x3f800000),
    (5858, 1573, 0x3f800000, 0x3f800000),
    (5863, 597, 0x3f800000, 0x3f800000),
    (5866, 1573, 0x3f800000, 0x3f800000),
    (5874, 1578, 0x3f800000, 0x3f800000),
    (5881, 1369, 0x3f800000, 0x3f800000),
    (5945, 597, 0x3f800000, 0x3f800000),
    (5961, 1573, 0x3f800000, 0x3f800000),
    (6281, 274, 0x3f800000, 0x3f800000),
    (6345, 1573, 0x3f800000, 0x3f800000),
    (6473, 117, 0x3f800000, 0x3f800000),
    (6537, 1369, 0x3f800000, 0x3f800000),
    (6541, 945, 0x3f800000, 0x3f800000),
    (6542, 855, 0x3f800000, 0x3f800000),
    (6543, 1369, 0x3f800000, 0x3f800000),
    (6549, 1573, 0x3f800000, 0x3f800000),
    (6741, 822, 0x3f800000, 0x3fc00000),
    (6773, 269, 0x3f800000, 0x3f800000),
    (6779, 597, 0x3f800000, 0x3f800000),
    (6811, 1573, 0x3f800000, 0x3f800000),
    (6837, 1482, 0x3f800000, 0x3f800000),
    (6997, 1573, 0x3f800000, 0x3f800000),
    (7029, 1369, 0x3f800000, 0x3f800000),
    (7189, 855, 0x3f800000, 0x3f800000),
    (7269, 632, 0x3f800000, 0x3f800000),
    (7271, 161, 0x3f800000, 0x3f800000),
    (7272, 919, 0x3f800000, 0x3f800000),
    (7385, 1369, 0x3f800000, 0x3f800000),
    (7407, 597, 0x3f800000, 0x3f800000),
    (7415, 1369, 0x3f800000, 0x3f800000),
    (7417, 597, 0x3f800000, 0x3f800000),
    (7419, 1573, 0x3f800000, 0x3f800000),
    (7431, 1369, 0x3f800000, 0x3f800000),
    (7512, 402, 0x3f800000, 0x3f800000),
    (7513, 1369, 0x3f800000, 0x3f800000),
    (7521, 1573, 0x3f800000, 0x3f800000),
    (7537, 1369, 0x3f800000, 0x3f800000),
    (7665, 822, 0x3f800000, 0x3fc00000),
    (7666, 1573, 0x3f800000, 0x3f800000),
    (7906, 1369, 0x3f800000, 0x3f800000),
    (8595, 632, 0x3f800000, 0x3f800000),
    (8611, 1573, 0x3f800000, 0x3f800000),
    (8731, 274, 0x3f800000, 0x3f800000),
    (8755, 1573, 0x3f800000, 0x3f800000),
    (8803, 117, 0x3f800000, 0x3f800000),
    (8827, 1369, 0x3f800000, 0x3f800000),
    (9107, 52, 0x3e99999a, 0x3f800000),
    (9119, 1573, 0x3f800000, 0x3f800000),
    (9143, 822, 0x3f800000, 0x3fc00000),
    (9175, 1369, 0x3f800000, 0x3f800000),
    (9191, 1573, 0x3f800000, 0x3f800000),
    (9223, 822, 0x3f800000, 0x3fc00000),
    (9224, 1170, 0x3f800000, 0x3f800000),
    (9225, 822, 0x3f800000, 0x3fc00000),
    (9235, 1369, 0x3f800000, 0x3f800000),
    (9320, 822, 0x3f800000, 0x3fc00000),
    (9344, 1369, 0x3f800000, 0x3f800000),
    (9372, 597, 0x3f800000, 0x3f800000),
    (9884, 1573, 0x3f800000, 0x3f800000),
    (9964, 274, 0x3f800000, 0x3f800000),
    (10044, 1573, 0x3f800000, 0x3f800000),
    (10204, 117, 0x3f800000, 0x3f800000),
    (10364, 1278, 0x3f800000, 0x3f800000),
    (10365, 1369, 0x3f800000, 0x3f800000),
    (10399, 822, 0x3f800000, 0x3fc00000),
    (10463, 1369, 0x3f800000, 0x3f800000),
    (10724, 597, 0x3f800000, 0x3f800000),
    (10725, 632, 0x3f800000, 0x3f800000),
    (10728, 1578, 0x3f800000, 0x3f800000),
    (10744, 1369, 0x3f800000, 0x3f800000),
    (10746, 597, 0x3f800000, 0x3f800000),
    (10747, 632, 0x3f800000, 0x3f800000),
    (10759, 1573, 0x3f800000, 0x3f800000),
    (11079, 1369, 0x3f800000, 0x3f800000),
    (11162, 1573, 0x3f800000, 0x3f800000),
    (11192, 274, 0x3f800000, 0x3f800000),
    (11198, 1573, 0x3f800000, 0x3f800000),
    (11210, 117, 0x3f800000, 0x3f800000),
    (11222, 1369, 0x3f800000, 0x3f800000),
    (11270, 855, 0x3f800000, 0x3f800000),
    (11276, 919, 0x3f800000, 0x3f800000),
    (11282, 1369, 0x3f800000, 0x3f800000),
    (11310, 1573, 0x3f800000, 0x3f800000),
    (11438, 274, 0x3f800000, 0x3f800000),
    (11470, 1573, 0x3f800000, 0x3f800000),
    (11534, 117, 0x3f800000, 0x3f800000),
    (11566, 1573, 0x3f800000, 0x3f800000),
    (11694, 274, 0x3f800000, 0x3f800000),
    (11726, 1573, 0x3f800000, 0x3f800000),
    (11790, 117, 0x3f800000, 0x3f800000),
    (11822, 1573, 0x3f800000, 0x3f800000),
    (12078, 274, 0x3f800000, 0x3f800000),
    (12142, 1573, 0x3f800000, 0x3f800000),
    (12270, 117, 0x3f800000, 0x3f800000),
    (12334, 1573, 0x3f800000, 0x3f800000),
    (12410, 1369, 0x3f800000, 0x3f800000),
    (12495, 632, 0x3f800000, 0x3f800000),
    (12514, 1369, 0x3f800000, 0x3f800000),
    (12539, 597, 0x3f800000, 0x3f800000),
    (12543, 1369, 0x3f800000, 0x3f800000),
    (12544, 972, 0x3f800000, 0x3f800000),
    (12545, 919, 0x3f800000, 0x3f800000),
    (12546, 178, 0x3f800000, 0x3f800000),
    (12549, 1369, 0x3f800000, 0x3f800000),
    (12744, 1189, 0x3f800000, 0x3f800000),
    (12760, 1523, 0x3f800000, 0x3f800000),
    (12787, 632, 0x3f800000, 0x3f800000),
    (12788, 822, 0x3f800000, 0x3fc00000),
    (12803, 1369, 0x3f800000, 0x3f800000),
    (12808, 364, 0x3f800000, 0x3f800000),
    (12813, 1369, 0x3f800000, 0x3f800000),
    (12823, 1523, 0x3f800000, 0x3f800000),
    (12833, 1369, 0x3f800000, 0x3f800000),
    (12843, 1523, 0x3f800000, 0x3f800000),
    (12853, 1369, 0x3f800000, 0x3f800000),
    (12893, 1523, 0x3f800000, 0x3f800000),
    (12933, 1278, 0x3f800000, 0x3f800000),
    (12941, 597, 0x3f800000, 0x3f800000),
    (12942, 1369, 0x3f800000, 0x3f800000),
    (12944, 109, 0x3f800000, 0x3f800000),
    (12957, 1369, 0x3f800000, 0x3f800000),
    (12960, 940, 0x3f800000, 0x3f800000),
    (12962, 1369, 0x3f800000, 0x3f800000),
    (13842, 919, 0x3f800000, 0x3f800000),
    (13922, 1369, 0x3f800000, 0x3f800000),
    (14142, 919, 0x3f800000, 0x3f800000),
    (14148, 1369, 0x3f800000, 0x3f800000),
    (16104, 855, 0x3f800000, 0x3f800000),
    (16428, 919, 0x3f800000, 0x3f800000),
    (16752, 1369, 0x3f800000, 0x3f800000),
    (17076, 919, 0x3f800000, 0x3f800000),
    (17400, 1369, 0x3f800000, 0x3f800000),
    (18372, 1194, 0x3f800000, 0x3f800000),
    (18404, 1573, 0x3f800000, 0x3f800000),
    (18420, 1369, 0x3f800000, 0x3f800000),
    (18436, 1573, 0x3f800000, 0x3f800000),
    (18438, 1369, 0x3f800000, 0x3f800000),
    (18450, 1573, 0x3f800000, 0x3f800000),
    (18467, 1369, 0x3f800000, 0x3f800000),
    (18471, 52, 0x3e99999a, 0x3f800000),
    (18503, 772, 0x3f800000, 0x3f800000),
    (18511, 1573, 0x3f800000, 0x3f800000),
    (18575, 632, 0x3f800000, 0x3f800000),
    (18579, 947, 0x3f800000, 0x3f800000),
    (18591, 952, 0x3f800000, 0x3f800000),
    (18592, 962, 0x3f800000, 0x3f800000),
    (18593, 972, 0x3f800000, 0x3f800000),
    (18594, 566, 0x3f800000, 0x3f800000),
    (18595, 957, 0x3f800000, 0x3f800000),
    (18596, 947, 0x3f800000, 0x3f800000),
    (18608, 952, 0x3f800000, 0x3f800000),
    (18609, 962, 0x3f800000, 0x3f800000),
    (18610, 1234, 0x3f800000, 0x3f800000),
    (18611, 967, 0x3f800000, 0x3f800000),
    (18665, 566, 0x3f800000, 0x3f800000),
    (18666, 929, 0x3f800000, 0x3f800000),
    (19356, 1369, 0x3f800000, 0x3f800000),
    (19372, 1573, 0x3f800000, 0x3f800000),
    (19381, 632, 0x3f800000, 0x3f800000),
    (19397, 1573, 0x3f800000, 0x3f800000),
    (19445, 703, 0x3f800000, 0x3f800000),
    (19446, 364, 0x3f800000, 0x3f800000),
    (19447, 977, 0x3f800000, 0x3f800000),
    (19448, 42, 0x3f800000, 0x3f800000),
    (19449, 1369, 0x3f800000, 0x3f800000),
    (19459, 795, 0x3f800000, 0x3f800000),
    (19460, 1369, 0x3f800000, 0x3f800000),
    (20285, 592, 0x3f800000, 0x3f800000),
    (20286, 1369, 0x3f800000, 0x3f800000),
    (20722, 919, 0x3f800000, 0x3f800000),
    (20724, 1369, 0x3f800000, 0x3f800000),
    (20725, 249, 0x3f800000, 0x3f800000),
    (20997, 1578, 0x3f800000, 0x3f800000),
    (21031, 35, 0x3f800000, 0x3f800000),
    (21033, 40, 0x3f800000, 0x3f800000),
    (21081, 1401, 0x3f800000, 0x3f800000),
    (21492, 1414, 0x3f800000, 0x3f800000),
    (21903, 1401, 0x3f800000, 0x3f800000),
    (21904, 1409, 0x3f800000, 0x3f800000),
    (22316, 227, 0x3f800000, 0x3f800000),
    (22317, 597, 0x3f800000, 0x3f800000),
    (22318, 1138, 0x3f800000, 0x3f800000),
    (22319, 1214, 0x3f800000, 0x3f800000),
    (22799, 1201, 0x3f800000, 0x3f800000),
    (22800, 1225, 0x3f800000, 0x3f800000),
    (22928, 1207, 0x3f800000, 0x3f800000),
    (22930, 1220, 0x3f800000, 0x3f800000),
    (22938, 347, 0x3f800000, 0x3f800000),
    (22942, 1369, 0x3f800000, 0x3f800000),
    (22943, 402, 0x3f800000, 0x3f800000),
    (22944, 347, 0x3f800000, 0x3f800000),
    (24676, 354, 0x3f800000, 0x3f800000),
    (24692, 340, 0x3f800000, 0x3f800000),
    (24724, 347, 0x3f800000, 0x3f800000),
    (24748, 434, 0x3f800000, 0x3f800000),
    (24768, 429, 0x3f800000, 0x3f800000),
    (24769, 263, 0x3f800000, 0x3f800000),
    (24823, 1301, 0x3f800000, 0x3f800000),
    (24824, 99, 0x3f800000, 0x3f800000),
    (24825, 524, 0x3f800000, 0x3f800000),
    (24826, 835, 0x3f800000, 0x3f800000),
    (24827, 840, 0x3f800000, 0x3f800000),
    (24843, 845, 0x3f800000, 0x3f800000),
    (24844, 161, 0x3f800000, 0x3f800000),
    (24884, 1285, 0x3f800000, 0x3f800000),
    (24900, 652, 0x3f800000, 0x3f800000),
    (24902, 1180, 0x3f800000, 0x3f800000),
    (24903, 850, 0x3f800000, 0x3f800000),
    (24904, 402, 0x3f800000, 0x3f800000),
    (25318, 1130, 0x3f800000, 0x3f800000),
    (25729, 407, 0x3f800000, 0x3f800000),
    (26140, 397, 0x3f800000, 0x3f800000),
    (26553, 407, 0x3f800000, 0x3f800000),
    (26554, 402, 0x3f800000, 0x3f800000),
    (26557, 131, 0x3f800000, 0x3f800000),
    (26558, 1369, 0x3f800000, 0x3f800000),
    (26563, 550, 0x3f800000, 0x3f800000),
    (26572, 551, 0x3f800000, 0x3f800000),
    (26573, 402, 0x3f800000, 0x3f800000),
    (26574, 390, 0x3f800000, 0x3f800000),
    (26590, 1369, 0x3f800000, 0x3f800000),
    (26638, 674, 0x3f800000, 0x3f800000),
    (26650, 1449, 0x3f800000, 0x3f800000),
    (26682, 662, 0x3f800000, 0x3f800000),
];
