//! Genuine Item/XP world callbacks with method-local gameplay/light/cache views.
//! The current actor and query/sound capability arrive from its actual owner;
//! this structure neither creates an entity nor borrows a second item runtime.
use super::*;
use mc121_rust::{
    entity_move::ProfileEvent,
    entity_state::EntityPhysicalState,
    fluid_physics::{FluidState, RuntimeFluidWorld},
    geometry::VoxelShape,
    inventory::{PlayerInventory, ItemStack},
    item_entity::{ItemState, Pickup},
    item_host::{RuntimeGameplayHooks, RuntimeInventoryWorldCallbacks},
    native_rain::NativeFlatPlainsRainFrame,
    placement::BlockPosition,
    rain_physics::RainExposure,
    runtime_entities::{PickupPlayer, RuntimeEntityQuerySound},
    runtime_entity_index::{EntityHandle, EntityKind},
};

pub(super) struct MainPressurePlateEntities<'a> {
    pub(super) gameplay: &'a mut MainItemGameplay,
    pub(super) registry: &'a EntityRegistry,
    pub(super) player_motion: &'a main_player_motion_hooks::MainPlayerMotionRuntime,
    pub(super) chunks: &'a mut MainChunkRuntime,
    pub(super) interactions: &'a mut main_world_interactions::WorldInteractionRuntime,
    pub(super) source: &'a mut Option<NativeEntitySource>,
    pub(super) block_runtime: &'a mut main_block_ticks::MainBlockRuntime,
    pub(super) fluid_runtime: &'a mut main_fluid_simulation::MainFluidRuntime,
    pub(super) redstone_runtime: &'a mut main_redstone::MainRedstoneRuntime,
    pub(super) random_ticks: &'a mut main_random_block_ticks::MainRandomBlockTicks,
    pub(super) world_revision: &'a mut u64,
    pub(super) context: &'a WorldContext,
    pub(super) rain: Option<&'a mut mc121_rust::rain_physics::FreshFlatPlainsRain>,
    pub(super) changes: Option<&'a broadcast::Sender<WorldUpdate>>,
    pub(super) events: &'a broadcast::Sender<PlayerEvent>,
    pub(super) external: Vec<main_redstone::NativeTriggerBody>,
    pub(super) query_region: Option<main_chunk_runtime::CurrentChunkQueryRegion>,
    pub(super) now: i64,
    pub(super) border: CollisionBorder,
}

impl MainPressurePlateEntities<'_> {
    pub(super) fn frame(&mut self) -> main_item_gameplay::MainGameplayFrame<'_> {
        // This owner is constructed only for the existing certified native
        // Overworld phase; retain the same dimension fact as its prior frame.
        match self.query_region {
            Some(region) => self.gameplay.frame_current(self.registry, &self.chunks.manager, true, region),
            None => self.gameplay.frame(self.registry, &self.chunks.manager, true),
        }
    }
    pub(super) fn pressure_plate_inside(&mut self, world: &mut WorldBackend,
        actor: &mut EntityPhysicalState, handle: EntityHandle, kind: EntityKind,
        position: BlockPosition, block_state: u32, access: &mut dyn RuntimeEntityQuerySound,
    ) -> Result<()> {
        ensure!(matches!(kind, EntityKind::Item | EntityKind::ExperienceOrb)
            && actor.entity_id == handle.id && !actor.removed
            && access.entity_handle(handle.id) == Some(handle)
            && access.entity_kind(handle) == Some(kind),
            "pressure callback has a stale or foreign actual Item/XP owner");
        ensure!(!actor.is_spectator, "nonplayer pressure spectator override lacks its actual class owner");
        ensure!(world.block(position.0, position.1, position.2)? == block_state
            && main_falling_block_redstone::pressure_plate_profile(block_state)?.is_some(),
            "pressure callback source state is not the actual declaring plate");
        let changes = self.changes.context("pressure callback requires the actual world-change publisher")?;
        let region = self.query_region.context("pressure callback requires its actual current query region")?;
        let mut services = main_redstone::NativePressurePlateServices {
            world, runtime: access, gameplay: self.gameplay, registry: self.registry,
            player_motion: self.player_motion, chunks: self.chunks,
            interactions: self.interactions, source: self.source,
            block_runtime: self.block_runtime, fluid_runtime: self.fluid_runtime,
            redstone_runtime: self.redstone_runtime, random_ticks: self.random_ticks,
            world_revision: self.world_revision, context: self.context,
            changes, events: self.events, external_trigger_bodies: &self.external,
            query_region: region, now: self.now, border: self.border,
        };
        main_redstone::native_pressure_plate_inside_parts(&mut services, handle, kind, actor, position)
    }
    fn item_pressure_inside(&mut self, world: &mut WorldBackend,
        actor: &mut EntityPhysicalState, handle: EntityHandle, position: BlockPosition,
        block_state: u32, access: &mut dyn RuntimeEntityQuerySound,
    ) -> Result<()> {
        self.pressure_plate_inside(world, actor, handle, EntityKind::Item, position, block_state, access)
    }
    pub(super) fn fluid_shape(&mut self, world: &mut WorldBackend,
        position: BlockPosition, fluid: FluidState,
    ) -> Result<VoxelShape> {
        self.gameplay.require_supported_floor()?;
        self.interactions.fluid_shapes.shape(&mut RuntimeFluidWorld { world }, position, fluid)
    }
    pub(super) fn rain_exposure(&mut self, world: &mut WorldBackend,
        actor: &EntityPhysicalState,
    ) -> Result<bool> {
        self.gameplay.require_supported_floor()?;
        let owner = self.rain.as_deref_mut().context("actual Item/XP rain exposure owner is absent")?;
        let (lighting, manager) = self.chunks.native_lighting_parts()?;
        NativeFlatPlainsRainFrame::new(owner, lighting, manager).is_in_rain(world, actor)
    }
}

pub(super) fn world_callbacks<'a>() -> RuntimeInventoryWorldCallbacks<MainPressurePlateEntities<'a>> {
    RuntimeInventoryWorldCallbacks {
        pressure_plate_inside: MainPressurePlateEntities::item_pressure_inside,
        fluid_shape: MainPressurePlateEntities::fluid_shape,
        rain_exposure: MainPressurePlateEntities::rain_exposure,
    }
}

impl RuntimeGameplayHooks for MainPressurePlateEntities<'_> {
    fn level_player_tick(&mut self, player: EntityHandle) -> Result<()> { self.frame().level_player_tick(player) }
    fn profile(&mut self, event: ProfileEvent) { self.frame().profile(event) }
    fn player_in_overworld(&self, player: &PlayerState) -> Result<bool> {
        self.gameplay.validate_active_player(player.entity_id, player.profile.uuid)?;
        // Same certified native Overworld source used by the prior frame.
        Ok(true)
    }
    fn hit_ground(&mut self, actor: &EntityPhysicalState, block: u32) -> Result<()> {
        self.frame().hit_ground(actor, block)
    }
    fn splash(&mut self, actor: &EntityPhysicalState) -> Result<()> { self.frame().splash(actor) }
    fn entity_damage(&mut self, actor: &EntityPhysicalState, stack: &ItemStack,
        health: i32, causing: Option<i32>,
    ) -> Result<()> { self.frame().entity_damage(actor, stack, health, causing) }
    fn discarded(&mut self, actor: &EntityPhysicalState, stack: &ItemStack, health: i32) -> Result<()> {
        self.frame().discarded(actor, stack, health)
    }
    fn award_pickup_stat(&mut self, player: PickupPlayer, item: i32, count: i32) -> Result<()> {
        self.frame().award_pickup_stat(player, item, count)
    }
    fn inventory_slot_changed(&mut self, player: PickupPlayer, slot: usize,
        inventory: &PlayerInventory, stack: &ItemStack,
    ) -> Result<()> { self.frame().inventory_slot_changed(player, slot, inventory, stack) }
    fn on_item_pickup(&mut self, player: PickupPlayer, state: &ItemState, pickup: Pickup) -> Result<()> {
        self.frame().on_item_pickup(player, state, pickup)
    }
}
