//! Java 1.21 EnchantingTableBlockEntity server state and Nameable contract.
//!
//! The actual factory must retain one class RNG owner and supply the canonical
//! base BE, registry/component codec and component-map owners. This type owns
//! no inventory, menu, world substitute or server book-animation ticker.
use crate::{
    block_entity::{BlockEntityComponentCodec, BlockEntityComponentMap, BlockEntityCompoundOwner, BlockEntityTypeKeys, NativeBlockEntityBase},
    native_block_entities::BlockPosition,
    nbt::{JavaString, Tag},
};
use anyhow::{bail, ensure, Result};
use std::{marker::PhantomData, sync::{Mutex, OnceLock}};

pub const ENCHANTING_TABLE_ID: &str = "minecraft:enchanting_table";
pub const DEFAULT_NAME_KEY: &str = "container.enchant";

const INNER_SHA256: &str = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01";
const MAPPINGS_SHA1: &str = "31c77994d96f05ba25a870ada70f47f315330437";

/// The actual inherited BlockEntity object. Construction starts with an
/// immutable position copy, the supplied block state, no attached Level,
/// remove=false and the real empty DataComponentMap. The runtime attaches its
/// actual Level afterwards and retains this same owner through name/NBT calls.
/// No method has a default that fabricates a base, world or component map.
pub trait EnchantingTableBase {
    type Level: ?Sized;
    fn block_position(&self) -> BlockPosition;
    fn block_state(&self) -> u32;
    fn level(&self) -> Option<&Self::Level>;
    fn is_removed(&self) -> bool;
    fn components_are_empty(&self) -> bool;
}

pub trait EnchantingTableBaseFactory {
    type Base: EnchantingTableBase;
    /// Invoke the real BlockEntity ctor with the canonical ENCHANTING_TABLE
    /// type, the immutable position and this actual block state. Allocation,
    /// identity and the empty component-map singleton remain with that owner.
    fn create_enchanting_base(
        &mut self, position: BlockPosition, state: u32,
    ) -> Result<Self::Base>;
}

impl<T, L, M: BlockEntityComponentMap> EnchantingTableBase for NativeBlockEntityBase<T, L, M> {
    type Level = L;
    fn block_position(&self) -> BlockPosition { self.block_position() }
    fn block_state(&self) -> u32 { self.block_state() }
    fn level(&self) -> Option<&L> { self.level() }
    fn is_removed(&self) -> bool { self.is_removed() }
    fn components_are_empty(&self) -> bool { self.components().is_empty() }
}

/// Concrete registry queries required to bind the source ENCHANTING_TABLE type
/// to the compiled supported state catalogue. The actual identity must change
/// on registry replacement; a caller-supplied "supported" boolean is not used.
pub trait ActualEnchantingBlockEntityRegistry {
    type Identity: Clone + Eq;
    type Type: Clone + Eq;
    fn identity(&self) -> Result<Self::Identity>;
    fn minecraft_version(&self) -> Result<&str>;
    fn protocol_version(&self) -> Result<i32>;
    fn original_inner_sha256(&self) -> Result<&str>;
    fn official_mappings_sha1(&self) -> Result<&str>;
    fn enchanting_table_type(&self) -> Result<Self::Type>;
    fn type_key(&self, actual_type: &Self::Type) -> Result<Option<JavaString>>;
    fn valid_states(&self, actual_type: &Self::Type) -> Result<Vec<u32>>;
    fn block_key(&self, state: u32) -> Result<JavaString>;
}

/// The actual canonical type/state binding, borrowed from the real registry.
/// This object has no class RNG and may be retained outside world transactions.
pub struct CapturedEnchantingType<'a, R: ActualEnchantingBlockEntityRegistry> {
    registry: &'a R,
    identity: R::Identity,
    actual_type: R::Type,
    state: u32,
}

impl<'a, R: ActualEnchantingBlockEntityRegistry> CapturedEnchantingType<'a, R> {
    pub fn capture(registry: &'a R) -> Result<Self> {
        let identity = registry.identity()?;
        ensure!(registry.minecraft_version()? == "1.21" && registry.protocol_version()? == 767,
            "enchanting block entity requires the actual supported 1.21 registry");
        ensure!(registry.original_inner_sha256()? == INNER_SHA256
            && registry.official_mappings_sha1()? == MAPPINGS_SHA1,
            "enchanting block-entity registry has foreign source provenance");
        let state = pinned_enchanting_state()?;
        let actual_type = registry.enchanting_table_type()?;
        ensure!(registry.type_key(&actual_type)?.as_ref().is_some_and(|key| key == ENCHANTING_TABLE_ID),
            "actual ENCHANTING_TABLE type has a different canonical registry key");
        ensure!(registry.block_key(state)? == ENCHANTING_TABLE_ID,
            "actual enchanting state has a different block registry binding");
        ensure!(registry.valid_states(&actual_type)? == vec![state],
            "actual enchanting type's valid states differ from the pinned catalogue");
        ensure!(registry.identity()? == identity, "enchanting registry changed during capture");
        Ok(Self { registry, identity, actual_type, state })
    }

    fn require_current(&self) -> Result<()> {
        ensure!(self.registry.identity()? == self.identity, "captured enchanting registry was replaced");
        Ok(())
    }

    pub fn actual_type(&self) -> &R::Type { &self.actual_type }
    pub fn state(&self) -> u32 { self.state }

    /// Real source subclass construction: validate the retained registry
    /// binding, initialize the one native class RNG, then construct the actual
    /// inherited fields with the real EMPTY map supplied by its owner. The
    /// resulting base has no Level until the actual runtime attaches it.
    pub fn construct<L, M: BlockEntityComponentMap, C: Clone>(
        &self, position: BlockPosition, state: u32,
        actual_empty_map: &mut impl FnMut() -> Result<M>,
    ) -> Result<EnchantingTableBlockEntity<NativeBlockEntityBase<R::Type, L, M>, C>> {
        self.require_current()?;
        ensure!(state == self.state, "enchanting factory received an unsupported actual block state");
        let mut factory = NativeEnchantingBaseFactory::<R::Type, L, M, _> {
            actual_type: self.actual_type.clone(), actual_empty_map,
            level: PhantomData,
        };
        let entity = construct_with_native_class(position, state, &mut factory)?;
        self.require_current()?;
        Ok(entity)
    }
}

impl<R: ActualEnchantingBlockEntityRegistry> BlockEntityTypeKeys<R::Type> for CapturedEnchantingType<'_, R> {
    fn block_entity_type_key(&self, actual_type: &R::Type) -> Result<Option<JavaString>> {
        self.require_current()?;
        ensure!(actual_type == &self.actual_type, "enchanting save received another canonical type object");
        let key = self.registry.type_key(actual_type)?;
        self.require_current()?;
        Ok(key)
    }
}

struct NativeEnchantingBaseFactory<'a, T, L, M, F: FnMut() -> Result<M>> {
    actual_type: T,
    actual_empty_map: &'a mut F,
    level: PhantomData<fn() -> (L, M)>,
}

impl<T: Clone, L, M: BlockEntityComponentMap, F: FnMut() -> Result<M>> EnchantingTableBaseFactory
    for NativeEnchantingBaseFactory<'_, T, L, M, F>
{
    type Base = NativeBlockEntityBase<T, L, M>;
    fn create_enchanting_base(&mut self, position: BlockPosition, state: u32) -> Result<Self::Base> {
        NativeBlockEntityBase::construct(self.actual_type.clone(), position, state, (self.actual_empty_map)()?)
    }
}

/// One native process/class lifetime, outside every Clone world image. This
/// uses the existing genuine RandomSource.create process uniquifier/clock
/// owner. Native server table behavior never advances its animation stream.
fn construct_with_native_class<F: EnchantingTableBaseFactory, C: Clone>(
    position: BlockPosition, state: u32, factory: &mut F,
) -> Result<EnchantingTableBlockEntity<F::Base, C>> {
    static CLASS: OnceLock<Mutex<EnchantingTableClass<crate::random::LegacyRandomSource>>> = OnceLock::new();
    let class = CLASS.get_or_init(|| Mutex::new(EnchantingTableClass::uninitialized()));
    let mut class = class.lock().map_err(|_| anyhow::anyhow!("native enchanting class owner is poisoned"))?;
    class.construct(position, state, &mut || Ok(crate::random_seed::create_random()?), factory)
}

fn pinned_enchanting_state() -> Result<u32> {
    #[derive(serde::Deserialize)]
    struct Catalogue { minecraft: String, jar_sha256: String, blocks: Vec<Block> }
    #[derive(serde::Deserialize)]
    struct Block {
        name: String, class: String, has_block_entity: bool,
        block_entity_type: Option<String>, min: u32, max: u32, default: u32,
    }
    let catalogue: Catalogue = serde_json::from_str(include_str!("../fixtures/placement-1.21.json"))?;
    ensure!(catalogue.minecraft == "1.21" && catalogue.jar_sha256 == INNER_SHA256,
        "enchanting catalogue source identity changed");
    let mut matches = catalogue.blocks.iter().filter(|block| block.name == ENCHANTING_TABLE_ID);
    let block = matches.next().ok_or_else(|| anyhow::anyhow!("pinned enchanting block is absent"))?;
    ensure!(matches.next().is_none() && block.class == "dik" && block.has_block_entity
        && block.block_entity_type.as_deref() == Some(ENCHANTING_TABLE_ID)
        && block.min == block.max && block.default == block.min,
        "pinned enchanting block/type/state catalogue is inconsistent");
    Ok(block.default)
}

/// C is a canonical Component reference/handle: cloning it retains the same
/// source component object, rather than parsing another component or replacing
/// it with text. The codec uses the actual HolderLookup.Provider argument.
pub trait EnchantingNameCodec<C: Clone> {
    type RegistryLookup: ?Sized;
    fn translatable(&mut self, key: &'static str) -> Result<C>;
    fn serialize_custom_name(
        &mut self, component: &C, registries: &Self::RegistryLookup,
    ) -> Result<JavaString>;
    /// Match BlockEntity.parseCustomNameSafe: a real component parse Exception
    /// logs the original warning and returns Ok(None). An absent codec/registry
    /// owner is an explicit Err, not a fabricated malformed-name result.
    fn parse_custom_name_safe(
        &mut self, json: &JavaString, registries: &Self::RegistryLookup,
    ) -> Result<Option<C>>;
}

/// Read the actual DataComponents.CUSTOM_NAME from the real component input.
/// Absence is meaningful: applyImplicitComponents clears the previous name.
pub trait EnchantingComponentInput<C> {
    fn custom_name(&mut self) -> Result<Option<C>>;
}

/// Write the real CUSTOM_NAME key in the actual DataComponentMap.Builder.
/// None has the source Builder.set(type,null) behavior: remove that key.
pub trait EnchantingComponentBuilder<C> {
    fn set_custom_name(&mut self, component: Option<C>) -> Result<()>;
}

enum ClassRandom<R> {
    Uninitialized,
    Ready(R),
    InitializationFailed,
}

/// One retained owner per original class initialization lifetime. It is
/// intentionally not Clone. Every table uses this same class-owned stream;
/// none creates a per-table RNG or aliases Level.random for the book.
pub struct EnchantingTableClass<R> {
    random: ClassRandom<R>,
}

impl<R> EnchantingTableClass<R> {
    pub fn uninitialized() -> Self { Self { random: ClassRandom::Uninitialized } }

    pub fn is_initialized(&self) -> bool { matches!(&self.random, ClassRandom::Ready(_)) }

    /// An observation for actual owner diagnostics only. Subclass NBT never
    /// saves this static field; server behavior consumes no animation sample.
    pub fn class_random(&self) -> Option<&R> {
        match &self.random { ClassRandom::Ready(random) => Some(random), _ => None }
    }

    fn initialize(&mut self, create_random: &mut impl FnMut() -> Result<R>) -> Result<()> {
        match &self.random {
            ClassRandom::Ready(_) => return Ok(()),
            ClassRandom::InitializationFailed => bail!("enchanting-table class initialization previously failed"),
            ClassRandom::Uninitialized => {}
        }
        // A failed real class initializer is not retried with another stream.
        // Host errors remain explicit; no seed or empty RNG is substituted.
        self.random = ClassRandom::InitializationFailed;
        self.random = ClassRandom::Ready(create_random()?);
        Ok(())
    }

    /// Source new drc initializes the class first, then calls the actual base
    /// ctor. A later base-construction failure leaves its class RNG initialized.
    /// The supplied RandomFactory must be the real RandomSource.create owner.
    pub fn construct<F: EnchantingTableBaseFactory, C: Clone>(
        &mut self,
        position: BlockPosition,
        state: u32,
        create_random: &mut impl FnMut() -> Result<R>,
        factory: &mut F,
    ) -> Result<EnchantingTableBlockEntity<F::Base, C>> {
        self.initialize(create_random)?;
        let base = factory.create_enchanting_base(position, state)?;
        ensure!(base.block_position() == position && base.block_state() == state,
            "enchanting-table factory changed its actual position/state");
        ensure!(base.level().is_none() && !base.is_removed() && base.components_are_empty(),
            "enchanting-table factory did not supply the actual initial base state");
        Ok(EnchantingTableBlockEntity {
            base, custom_name: None, animation: BookAnimationState::default(),
        })
    }
}

/// These are the real source fields, initially JVM +0/zero. They are neither
/// loaded nor saved by this class. Its animation mutator is client-only and is
/// deliberately not represented as a runnable native server tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BookAnimationState {
    pub time: i32,
    pub flip: f32,
    pub old_flip: f32,
    pub flip_target: f32,
    pub flip_acceleration: f32,
    pub open: f32,
    pub old_open: f32,
    pub rotation: f32,
    pub old_rotation: f32,
    pub target_rotation: f32,
}

/// Source drc extends the actual base BE and implements Nameable, not Container
/// or MenuProvider. B clones, when supported, must retain its actual source
/// identity; C clones retain their canonical component reference.
#[derive(Clone, Debug)]
pub struct EnchantingTableBlockEntity<B: EnchantingTableBase, C: Clone> {
    base: B,
    custom_name: Option<C>,
    animation: BookAnimationState,
}

impl<B: EnchantingTableBase, C: Clone> EnchantingTableBlockEntity<B, C> {
    pub fn base(&self) -> &B { &self.base }
    pub fn base_mut(&mut self) -> &mut B { &mut self.base }
    pub fn book_animation(&self) -> &BookAnimationState { &self.animation }
    pub fn has_server_ticker(&self) -> bool { false }
    pub fn has_custom_name(&self) -> bool { self.custom_name.is_some() }
    pub fn get_custom_name(&self) -> Option<&C> { self.custom_name.as_ref() }

    /// Original setter only assigns the field: no invented setChanged,
    /// inventory mutation, block event or update packet is emitted.
    pub fn set_custom_name(&mut self, component: Option<C>) { self.custom_name = component; }

    pub fn get_name(&self, names: &mut impl EnchantingNameCodec<C>) -> Result<C> {
        match &self.custom_name {
            Some(name) => Ok(name.clone()),
            None => names.translatable(DEFAULT_NAME_KEY),
        }
    }

    /// Nameable.getDisplayName delegates to getName, with no fallback title.
    pub fn get_display_name(&self, names: &mut impl EnchantingNameCodec<C>) -> Result<C> {
        self.get_name(names)
    }

    /// Source fixed BlockEntity.saveAdditional body returns before this child
    /// serializes its name. Full base metadata/component-map saving belongs to
    /// the actual base owner; this is not saveWithFullMetadata/saveToItem.
    pub fn save_additional<N: EnchantingNameCodec<C>, O: BlockEntityCompoundOwner>(
        &self, tag: &mut O::Compound, registries: &N::RegistryLookup, names: &mut N,
        compounds: &mut O,
    ) -> Result<()> {
        if let Some(name) = &self.custom_name {
            let json = names.serialize_custom_name(name, registries)?;
            compounds.put(tag, "CustomName".into(), Tag::String(json))?;
        }
        // With no custom name, source leaves any existing target key alone.
        Ok(())
    }

    /// Missing or wrong-typed CustomName preserves the current field; a String
    /// invokes the actual safe parser and assigns its nullable result.
    pub fn load_additional<N: EnchantingNameCodec<C>>(
        &mut self, tag: &Tag, registries: &N::RegistryLookup, names: &mut N,
    ) -> Result<()> {
        ensure!(matches!(tag, Tag::Compound(_)), "enchanting-table NBT must be compound");
        if let Some(Tag::String(json)) = tag.get("CustomName") {
            self.custom_name = names.parse_custom_name_safe(json, registries)?;
        }
        Ok(())
    }

    /// The fixed base apply/collect hooks return; this subclass transfers only
    /// actual CUSTOM_NAME. Applying absence differs from absent NBT loading.
    pub fn apply_implicit_components(
        &mut self, input: &mut impl EnchantingComponentInput<C>,
    ) -> Result<()> {
        self.custom_name = input.custom_name()?;
        Ok(())
    }

    pub fn collect_implicit_components(
        &self, builder: &mut impl EnchantingComponentBuilder<C>,
    ) -> Result<()> {
        builder.set_custom_name(self.custom_name.clone())
    }

    pub fn remove_components_from_tag(&self, tag: &mut Tag) -> Result<()> {
        let Tag::Compound(entries) = tag else { bail!("enchanting-table NBT must be compound"); };
        // The native Tag retains decoded duplicate entries; CompoundTag.remove
        // removes the logical key entirely, not only its latest occurrence.
        entries.retain(|(key, _)| key != "CustomName");
        Ok(())
    }
}

/// The real block's SimpleMenuProvider owns the display-name component and
/// captured ContainerLevelAccess location. It does not own another inventory
/// or retain the BE identity as an extra stillValid condition.
pub struct NativeEnchantingMenuProvider<L, C: Clone> {
    actual_level: L,
    position: BlockPosition,
    display_name: C,
}

/// Actual EnchantmentMenu construction owner. Implementations must bind the
/// supplied real player Inventory and ContainerLevelAccess.create(level,pos),
/// including the menu's own genuine RandomSource.create owner. No default menu
/// implementation, detached inventory or invented random source is supplied.
pub trait NativeEnchantingMenuFactory<L> {
    type Inventory: ?Sized;
    type Menu;
    fn create_enchantment_menu(
        &mut self, menu_id: i32, inventory: &mut Self::Inventory,
        actual_level: &L, position: BlockPosition,
    ) -> Result<Self::Menu>;
}

impl<L, C: Clone> NativeEnchantingMenuProvider<L, C> {
    pub fn display_name(&self) -> &C { &self.display_name }
    pub fn actual_level(&self) -> &L { &self.actual_level }
    pub fn block_position(&self) -> BlockPosition { self.position }
    pub fn create_menu<F: NativeEnchantingMenuFactory<L>>(
        &self, menu_id: i32, inventory: &mut F::Inventory, factory: &mut F,
    ) -> Result<F::Menu> {
        factory.create_enchantment_menu(menu_id, inventory, &self.actual_level, self.position)
    }
}

impl<T, L, M: BlockEntityComponentMap, C: Clone>
    EnchantingTableBlockEntity<NativeBlockEntityBase<T, L, M>, C>
{
    /// Called only AFTER the real Level.getBlockEntity(pos) lookup returned
    /// this actual retained EnchantingTableBlockEntity. The block's absent or
    /// other-type branch belongs to that lookup; it must never call this with
    /// a constructed placeholder. Source captures the Level/pos arguments and
    /// obtains its title from the object's actual getDisplayName immediately.
    pub fn get_menu_provider(
        &self, actual_lookup_level: L, actual_lookup_position: BlockPosition,
        names: &mut impl EnchantingNameCodec<C>,
    ) -> Result<NativeEnchantingMenuProvider<L, C>> {
        Ok(NativeEnchantingMenuProvider {
            actual_level: actual_lookup_level,
            position: actual_lookup_position,
            display_name: self.get_display_name(names)?,
        })
    }

    pub fn load_custom_only<N: EnchantingNameCodec<C>>(
        &mut self, tag: &Tag, registries: &N::RegistryLookup, names: &mut N,
    ) -> Result<()> {
        self.load_additional(tag, registries, names)
    }

    /// Source invokes the subclass BEFORE component decoding. A failed
    /// unsupported component context is propagated with that same prefix;
    /// neither the previous name nor a real partial map is silently rolled back.
    pub fn load_with_components<N, K>(
        &mut self, tag: &Tag, registries: &N::RegistryLookup, names: &mut N,
        components: &mut K,
    ) -> Result<()>
    where
        N: EnchantingNameCodec<C>,
        K: BlockEntityComponentCodec<M, RegistryLookup = N::RegistryLookup>,
    {
        self.load_additional(tag, registries, names)?;
        self.base.load_components(tag, registries, components)
    }

    fn fresh_custom<N: EnchantingNameCodec<C>, O: BlockEntityCompoundOwner>(
        &self, registries: &N::RegistryLookup, names: &mut N, compounds: &mut O,
    ) -> Result<O::Compound> {
        let mut tag = compounds.fresh_compound()?;
        self.save_additional(&mut tag, registries, names, compounds)?;
        Ok(tag)
    }

    fn fresh_without_metadata<N, K, O>(
        &self, registries: &N::RegistryLookup, names: &mut N, components: &mut K,
        compounds: &mut O,
    ) -> Result<O::Compound>
    where
        N: EnchantingNameCodec<C>,
        K: BlockEntityComponentCodec<M, RegistryLookup = N::RegistryLookup>,
        O: BlockEntityCompoundOwner,
    {
        let mut tag = self.fresh_custom(registries, names, compounds)?;
        self.base.save_components_into(&mut tag, registries, components, compounds)?;
        Ok(tag)
    }

    /// Source fresh CompoundTag + subclass fields only, then the real owner's
    /// one finalization. Excludes inherited component-map serialization.
    pub fn save_custom_only<N: EnchantingNameCodec<C>, O: BlockEntityCompoundOwner>(
        &self, registries: &N::RegistryLookup, names: &mut N, compounds: &mut O,
    ) -> Result<Tag> {
        let tag = self.fresh_custom(registries, names, compounds)?;
        compounds.into_ordered_tag(tag)
    }

    pub fn save_custom_and_metadata<N: EnchantingNameCodec<C>, O: BlockEntityCompoundOwner>(
        &self, registries: &N::RegistryLookup, names: &mut N,
        keys: &impl BlockEntityTypeKeys<T>, compounds: &mut O,
    ) -> Result<Tag> {
        let mut tag = self.fresh_custom(registries, names, compounds)?;
        self.base.save_metadata(&mut tag, keys, compounds)?;
        compounds.into_ordered_tag(tag)
    }

    /// Source subclass fields, actual stored component-map codec/merge, then
    /// one finalization retaining the real root's encounter history.
    pub fn save_without_metadata<N, K, O>(
        &self, registries: &N::RegistryLookup, names: &mut N, components: &mut K,
        compounds: &mut O,
    ) -> Result<Tag>
    where
        N: EnchantingNameCodec<C>,
        K: BlockEntityComponentCodec<M, RegistryLookup = N::RegistryLookup>,
        O: BlockEntityCompoundOwner,
    {
        let tag = self.fresh_without_metadata(registries, names, components, compounds)?;
        compounds.into_ordered_tag(tag)
    }

    pub fn save_with_id<N, K, O>(
        &self, registries: &N::RegistryLookup, names: &mut N, components: &mut K,
        keys: &impl BlockEntityTypeKeys<T>, compounds: &mut O,
    ) -> Result<Tag>
    where
        N: EnchantingNameCodec<C>,
        K: BlockEntityComponentCodec<M, RegistryLookup = N::RegistryLookup>,
        O: BlockEntityCompoundOwner,
    {
        let mut tag = self.fresh_without_metadata(registries, names, components, compounds)?;
        self.base.save_id(&mut tag, keys, compounds)?;
        compounds.into_ordered_tag(tag)
    }

    pub fn save_with_full_metadata<N, K, O>(
        &self, registries: &N::RegistryLookup, names: &mut N, components: &mut K,
        keys: &impl BlockEntityTypeKeys<T>, compounds: &mut O,
    ) -> Result<Tag>
    where
        N: EnchantingNameCodec<C>,
        K: BlockEntityComponentCodec<M, RegistryLookup = N::RegistryLookup>,
        O: BlockEntityCompoundOwner,
    {
        let mut tag = self.fresh_without_metadata(registries, names, components, compounds)?;
        self.base.save_metadata(&mut tag, keys, compounds)?;
        compounds.into_ordered_tag(tag)
    }
}
