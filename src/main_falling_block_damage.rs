//! Actual configured damage holders and indexed native falling-block targets.
use super::*;
use mc121_rust::{
    falling_block::damage::{FallingBlockDamageActor, FallingBlockDamageHost},
    geometry::Aabb,
    runtime_entity_index::{EntityHandle, EntityKind},
};

#[derive(Debug)]
struct DamageTypes {
    values: Vec<(String, Tag)>,
    falling_anvil: usize,
    falling_block: usize,
}

/// Retained ONCE from the actual admitted configuration, not rebuilt per tick.
/// Arc identity is the holder owner. The finite native profile additionally
/// certifies the exact compiled tag catalogue before consulting its Anvil tag.
#[derive(Clone, Debug)]
pub(super) struct NativeFallingDamageTypes(Arc<DamageTypes>);
#[derive(Clone, Debug)]
pub(super) struct NativeFallingDamageSource {
    registry: Arc<DamageTypes>,
    type_index: usize,
    pub(super) direct_entity: EntityHandle,
    pub(super) causing_entity: EntityHandle,
}

impl NativeFallingDamageTypes {
    pub(super) fn capture(fixtures: &Fixtures) -> Result<Self> {
        let pinned: Capture = serde_json::from_str(include_str!("../fixtures/configuration.json"))?;
        ensure!(pinned.version == "1.21" && pinned.protocol == 767 && pinned.complete,
            "foreign compiled falling configuration");
        ensure!(pinned.packets.len() == fixtures.configuration.len(),
            "falling configured block-tag owner is not certified");
        for (expected, actual) in pinned.packets.iter().zip(&fixtures.configuration) {
            ensure!(expected.id == actual.id && STANDARD.decode(&expected.payload)? == actual.payload,
                "falling native profile requires the actual pinned configuration/tag snapshot");
        }
        let mut values = None;
        for packet in &fixtures.configuration {
            if packet.id != 0x07 { continue; }
            let mut reader = Reader::new(&packet.payload);
            if reader.string(32767)? != "minecraft:damage_type" { continue; }
            ensure!(values.is_none(), "duplicate actual damage_type registry");
            let count = reader.varint()?;
            ensure!((0..=65536).contains(&count), "invalid damage_type holder count");
            let mut registry = Vec::with_capacity(count as usize);
            let mut names = BTreeSet::new();
            for _ in 0..count {
                let key = reader.string(32767)?;
                ensure!(names.insert(key.clone()), "duplicate damage_type holder key");
                ensure!(reader.bool()?, "actual damage_type definition is absent");
                let (definition, bytes) = Tag::decode_anonymous_prefix(reader.remaining())?;
                reader.bytes(bytes)?;
                ensure!(matches!(&definition, Tag::Compound(_)), "damage_type definition is not CompoundTag");
                registry.push((key, definition));
            }
            ensure!(reader.remaining().is_empty(), "trailing actual damage_type registry bytes");
            values = Some(registry);
        }
        let values = values.context("actual configured damage_type registry is absent")?;
        let falling_anvil = values.iter().position(|(key, _)| key == "minecraft:falling_anvil")
            .context("actual falling_anvil damage holder is absent")?;
        let falling_block = values.iter().position(|(key, _)| key == "minecraft:falling_block")
            .context("actual falling_block damage holder is absent")?;
        Ok(Self(Arc::new(DamageTypes { values, falling_anvil, falling_block })))
    }

    fn anvil_source(&self, actor: EntityHandle) -> NativeFallingDamageSource {
        NativeFallingDamageSource { registry: self.0.clone(), type_index: self.0.falling_anvil,
            direct_entity: actor, causing_entity: actor }
    }
    fn block_source(&self, actor: EntityHandle) -> NativeFallingDamageSource {
        NativeFallingDamageSource { registry: self.0.clone(), type_index: self.0.falling_block,
            direct_entity: actor, causing_entity: actor }
    }
}

/// Genuine AnvilBlock.damage: resolve the next singleton's default and copy
/// only the source FACING property, rather than incrementing a state integer.
pub(super) fn damaged_anvil_state(state: u32) -> Result<Option<u32>> {
    let target = match mc121_rust::placement::placement_block_name(state)? {
        "minecraft:anvil" => "minecraft:chipped_anvil",
        "minecraft:chipped_anvil" => "minecraft:damaged_anvil",
        "minecraft:damaged_anvil" => return Ok(None),
        _ => bail!("falling Anvil wear received a foreign Block singleton"),
    };
    #[derive(Deserialize)] struct Catalogue { blocks: Vec<Block> }
    #[derive(Deserialize)] struct Block { name: String, default: u32 }
    static CATALOGUE: std::sync::OnceLock<Catalogue> = std::sync::OnceLock::new();
    let catalogue = CATALOGUE.get_or_init(|| serde_json::from_str(include_str!(
        "../fixtures/placement-1.21.json")).expect("pinned placement catalogue"));
    let default = catalogue.blocks.iter().find(|block| block.name == target)
        .context("actual next Anvil Block singleton is absent")?.default;
    let facing = mc121_rust::inventory::block_state_property(state, "facing")
        .context("actual Anvil FACING property is absent")?;
    Ok(Some(mc121_rust::inventory::block_state_with_properties(default,
        &BTreeMap::from([("facing".into(), facing)]))?))
}

pub(super) struct MainFallingDamageHost<'a> {
    pub(super) items: &'a mut RuntimeEntities,
    pub(super) registry: &'a EntityRegistry,
    pub(super) player_motion: &'a main_player_motion_hooks::MainPlayerMotionRuntime,
    pub(super) actor: EntityHandle,
    pub(super) damage_types: &'a NativeFallingDamageTypes,
}

impl<A: FallingBlockDamageActor> FallingBlockDamageHost<A> for MainFallingDamageHost<'_> {
    type DamageSource = NativeFallingDamageSource;
    type Target = EntityHandle;
    fn preflight_fall_damage(&mut self, actor: &A) -> Result<()> {
        ensure!(actor.physical().entity_id == self.actor.id
            && self.items.entity_kind(self.actor) == Some(EntityKind::FallingBlock),
            "falling damage actor/generation is not the actual indexed owner");
        ensure!(mc121_rust::falling_block::block::source_class(actor.block_state())?.is_some(),
            "falling damage subclass has no actual native source owner");
        Ok(())
    }
    fn fall_damage_source(&mut self, actor: &A) -> Result<Self::DamageSource> {
        use mc121_rust::falling_block::block::{source_class, NativeFallingClass};
        // Actual AnvilBlock override selects falling_anvil. Ordinary and
        // ConcretePowder inherit Fallable's falling_block holder. Resolve it
        // before Level.getEntities, even if the eventual victim list is empty.
        match source_class(actor.block_state())? {
            Some(NativeFallingClass::Anvil) => Ok(self.damage_types.anvil_source(self.actor)),
            Some(NativeFallingClass::Ordinary | NativeFallingClass::ConcretePowder) =>
                Ok(self.damage_types.block_source(self.actor)),
            None => bail!("falling damage-source override is unavailable"),
        }
    }
    fn damage_targets(&mut self, actor: &A, bounds: Aabb) -> Result<Vec<Self::Target>> {
        // Player is the only current indexed LivingEntity class. Item, XP and
        // FallingBlock are actual non-living classes, not absent actor stubs.
        let registry = self.registry;
        let motion = self.player_motion;
        let candidates = self.items.native_body_query(bounds, actor.physical().entity_id, |handle| {
            let player = registry.get(handle.id).context("indexed falling damage player is absent")?;
            let body = motion.physical(handle.id).context("indexed falling damage body is absent")?;
            ensure!(body.entity_id == handle.id && body.uuid == player.profile.uuid,
                "falling damage player/body identity mismatch");
            Ok((!body.removed).then_some(body.bounds))
        })?;
        let mut targets = Vec::new();
        for handle in candidates {
            ensure!(self.items.entity_kind(handle) == Some(EntityKind::Player), "stale falling victim generation");
            let player = registry.get(handle.id).context("actual falling victim disappeared")?;
            if player.gamemode == 1 || player.gamemode == 3 { continue; }
            let body = motion.physical(handle.id).context("actual falling victim body disappeared")?;
            let health = match player.entity_data.get(9) {
                Some(MetadataValue::Float(health)) => *health,
                _ => bail!("actual falling victim health owner is absent"),
            };
            if !body.removed && health > 0. { targets.push(handle); }
        }
        Ok(targets)
    }
    fn hurt_target(&mut self, _actor: &mut A, target: &Self::Target,
        source: &Self::DamageSource, _amount: f32) -> Result<()> {
        ensure!(Arc::ptr_eq(&source.registry, &self.damage_types.0)
            && source.registry.values.get(source.type_index).is_some()
            && source.direct_entity == self.actor && source.causing_entity == self.actor,
            "foreign actual falling damage source holder");
        ensure!(self.items.entity_kind(*target) == Some(EntityKind::Player), "stale falling victim");
        bail!("actual survival LivingEntity.hurt owner is not connected to falling damage")
    }
    fn state_in_anvil_tag(&mut self, state: u32) -> Result<bool> {
        mc121_rust::placement::placement_block_name(state)?;
        Ok(mc121_rust::placement::in_block_tag(state, "minecraft:anvil"))
    }
    fn damaged_anvil_state(&mut self, state: u32) -> Result<Option<u32>> { damaged_anvil_state(state) }
}
