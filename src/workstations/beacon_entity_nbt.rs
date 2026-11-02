//! Java 1.21 BeaconBlockEntity additional NBT and inherited save/load wrappers.
//!
//! Direct pinned JDK21 javap: dqc.loadAdditional314–324,
//! saveAdditional328–339, getUpdateTag289 and bqu.fromTag32–35.
//! The actual decoder/compound/name/effect/component owners remain required.
//! These operations preserve completed source prefixes; they neither stage nor
//! roll back BE fields, codecs, diagnostics or an existing compound target.
//! Required holder/name/component-codec queries are read-only and nonreentrant
//! with respect to this retained BE in the admitted domain. They must reject
//! unavailable or unsupported contexts, not reenter another BE index or claim
//! arbitrary callback reentry through these ordinary borrows.

use super::beacon_entity::{BeaconBlockEntity, BeaconLockCode, BeaconPowerRegistry};
use crate::{
    block_entity::{
        BlockEntityComponentCodec, BlockEntityComponentMap, BlockEntityCompoundOwner,
        BlockEntityTypeKeys,
    },
    connection_cookies::CookieKey,
    enchanting_block_entity::EnchantingNameCodec,
    nbt::{JavaString, Tag},
    source_nbt_decode::SourceCompoundView,
};
use anyhow::Result;

const PRIMARY: &str = "primary_effect";
const SECONDARY: &str = "secondary_effect";

/// ResourceLocation.tryParse accepts ASCII identifier characters only. The
/// existing native ResourceLocation implementation preserves its default
/// namespace, first-colon and empty-path behavior. A non-ASCII/invalid string
/// is the original null result, not a failed registry-owner capability.
fn effect_key(value: &JavaString) -> Option<CookieKey> {
    let mut ascii = String::with_capacity(value.len_utf16());
    for unit in value.iter_units() {
        if unit > 0x7f {
            return None;
        }
        ascii.push(char::from(unit as u8));
    }
    CookieKey::parse(&ascii).ok()
}

fn load_effect<E: Clone, R: BeaconPowerRegistry<E>>(
    source: SourceCompoundView<'_>,
    key: &str,
    powers: &R,
    identity: &R::Identity,
) -> Result<Option<E>> {
    let Some(Tag::String(value)) = source.tag().get(key) else {
        return Ok(None);
    };
    let Some(key) = effect_key(value) else {
        return Ok(None);
    };
    powers.validate_identity(identity)?;
    let decoded = powers.builtin_effect_by_key(&key)?;
    powers.validate_identity(identity)?;
    let filtered = match decoded {
        Some(effect) if powers.is_valid_beacon_effect(&effect)? => Some(effect),
        _ => None,
    };
    powers.validate_identity(identity)?;
    Ok(filtered)
}

fn store_effect<E: Clone, R: BeaconPowerRegistry<E>, O: BlockEntityCompoundOwner>(
    target: &mut O::Compound,
    key: &str,
    effect: Option<&E>,
    powers: &R,
    identity: &R::Identity,
    compounds: &mut O,
) -> Result<()> {
    if let Some(effect) = effect {
        powers.validate_identity(identity)?;
        let location = powers.builtin_effect_key(effect)?;
        powers.validate_identity(identity)?;
        if let Some(location) = location {
            compounds.put(target, key.into(), Tag::String(location.as_str().into()))?;
        }
    }
    // A null or direct/unkeyed holder does not remove a pre-existing key.
    Ok(())
}

impl<T, L, M, E, C> BeaconBlockEntity<T, L, M, E, C>
where
    M: BlockEntityComponentMap,
    E: Clone,
    C: Clone,
{
    /// The input must be the actual source-issued compound node. An arbitrary
    /// equal Tag cannot manufacture its decoder/history capability. Powers
    /// and the name codec must be bound to their actual retained registry
    /// contexts; neither unavailable owner is treated as an absent value.
    ///
    /// The fixed superclass additional hook is empty. Source then ASSIGNS
    /// primary, secondary, a typed String name if present, and finally Lock.
    /// Levels, scan state and inherited components are deliberately not reset.
    pub fn load_additional<N, R>(
        &mut self,
        source: SourceCompoundView<'_>,
        powers: &R,
        registries: &N::RegistryLookup,
        names: &mut N,
    ) -> Result<()>
    where
        N: EnchantingNameCodec<C>,
        R: BeaconPowerRegistry<E>,
    {
        let identity = powers.identity()?;
        powers.validate_identity(&identity)?;
        self.primary = load_effect(source, PRIMARY, powers, &identity)?;
        self.secondary = load_effect(source, SECONDARY, powers, &identity)?;
        if let Some(Tag::String(json)) = source.tag().get("CustomName") {
            // A genuine parser exception is logged by the actual shared safe
            // parser and returns None, which clears the field. Missing or
            // wrong-typed NBT never enters this branch and preserves its name.
            self.name = names.parse_custom_name_safe(json, registries)?;
        }
        self.lock = BeaconLockCode::from_tag(source);
        powers.validate_identity(&identity)
    }

    pub fn load_custom_only<N, R>(
        &mut self,
        source: SourceCompoundView<'_>,
        powers: &R,
        registries: &N::RegistryLookup,
        names: &mut N,
    ) -> Result<()>
    where
        N: EnchantingNameCodec<C>,
        R: BeaconPowerRegistry<E>,
    {
        self.load_additional(source, powers, registries, names)
    }

    /// Source executes subclass assignment before ComponentHelper parsing.
    /// Codec errors preserve those assignments and any actual diagnostic
    /// prefix; unknown/nonempty maps never become a guessed EMPTY map.
    pub fn load_with_components<N, R, K>(
        &mut self,
        source: SourceCompoundView<'_>,
        powers: &R,
        registries: &N::RegistryLookup,
        names: &mut N,
        components: &mut K,
    ) -> Result<()>
    where
        N: EnchantingNameCodec<C>,
        R: BeaconPowerRegistry<E>,
        K: BlockEntityComponentCodec<M, RegistryLookup = N::RegistryLookup>,
    {
        self.load_additional(source, powers, registries, names)?;
        self.base
            .load_components(source.tag(), registries, components)
    }

    /// Mutate THIS actual target, preserving its original HashMap history.
    /// No target clone/publication is performed. Null/unkeyed powers, absent
    /// name and an empty Lock leave existing target keys untouched.
    pub fn save_additional<N, R, O>(
        &self,
        target: &mut O::Compound,
        powers: &R,
        registries: &N::RegistryLookup,
        names: &mut N,
        compounds: &mut O,
    ) -> Result<()>
    where
        N: EnchantingNameCodec<C>,
        R: BeaconPowerRegistry<E>,
        O: BlockEntityCompoundOwner,
    {
        let identity = powers.identity()?;
        powers.validate_identity(&identity)?;
        store_effect(
            target,
            PRIMARY,
            self.primary.as_ref(),
            powers,
            &identity,
            compounds,
        )?;
        store_effect(
            target,
            SECONDARY,
            self.secondary.as_ref(),
            powers,
            &identity,
            compounds,
        )?;
        compounds.put(target, "Levels".into(), Tag::Int(self.levels))?;
        if let Some(name) = &self.name {
            let json = names.serialize_custom_name(name, registries)?;
            compounds.put(target, "CustomName".into(), Tag::String(json))?;
        }
        if self.lock.key().len_utf16() != 0 {
            compounds.put(target, "Lock".into(), Tag::String(self.lock.key().clone()))?;
        }
        powers.validate_identity(&identity)
    }

    fn fresh_beacon_custom<N, R, O>(
        &self,
        powers: &R,
        registries: &N::RegistryLookup,
        names: &mut N,
        compounds: &mut O,
    ) -> Result<O::Compound>
    where
        N: EnchantingNameCodec<C>,
        R: BeaconPowerRegistry<E>,
        O: BlockEntityCompoundOwner,
    {
        let mut target = compounds.fresh_compound()?;
        self.save_additional(&mut target, powers, registries, names, compounds)?;
        Ok(target)
    }

    fn fresh_beacon_without_metadata<N, R, K, O>(
        &self,
        powers: &R,
        registries: &N::RegistryLookup,
        names: &mut N,
        components: &mut K,
        compounds: &mut O,
    ) -> Result<O::Compound>
    where
        N: EnchantingNameCodec<C>,
        R: BeaconPowerRegistry<E>,
        K: BlockEntityComponentCodec<M, RegistryLookup = N::RegistryLookup>,
        O: BlockEntityCompoundOwner,
    {
        let mut target = self.fresh_beacon_custom(powers, registries, names, compounds)?;
        self.base
            .save_components_into(&mut target, registries, components, compounds)?;
        Ok(target)
    }

    pub fn save_custom_only<N, R, O>(
        &self,
        powers: &R,
        registries: &N::RegistryLookup,
        names: &mut N,
        compounds: &mut O,
    ) -> Result<Tag>
    where
        N: EnchantingNameCodec<C>,
        R: BeaconPowerRegistry<E>,
        O: BlockEntityCompoundOwner,
    {
        let target = self.fresh_beacon_custom(powers, registries, names, compounds)?;
        compounds.into_ordered_tag(target)
    }

    /// Beacon.getUpdateTag delegates to saveCustomOnly: no inherited stored
    /// component map, registered type id or position metadata is serialized.
    pub fn get_update_tag<N, R, O>(
        &self,
        powers: &R,
        registries: &N::RegistryLookup,
        names: &mut N,
        compounds: &mut O,
    ) -> Result<Tag>
    where
        N: EnchantingNameCodec<C>,
        R: BeaconPowerRegistry<E>,
        O: BlockEntityCompoundOwner,
    {
        self.save_custom_only(powers, registries, names, compounds)
    }

    pub fn save_custom_and_metadata<N, R, O>(
        &self,
        powers: &R,
        registries: &N::RegistryLookup,
        names: &mut N,
        keys: &impl BlockEntityTypeKeys<T>,
        compounds: &mut O,
    ) -> Result<Tag>
    where
        N: EnchantingNameCodec<C>,
        R: BeaconPowerRegistry<E>,
        O: BlockEntityCompoundOwner,
    {
        let mut target = self.fresh_beacon_custom(powers, registries, names, compounds)?;
        self.base.save_metadata(&mut target, keys, compounds)?;
        compounds.into_ordered_tag(target)
    }

    pub fn save_without_metadata<N, R, K, O>(
        &self,
        powers: &R,
        registries: &N::RegistryLookup,
        names: &mut N,
        components: &mut K,
        compounds: &mut O,
    ) -> Result<Tag>
    where
        N: EnchantingNameCodec<C>,
        R: BeaconPowerRegistry<E>,
        K: BlockEntityComponentCodec<M, RegistryLookup = N::RegistryLookup>,
        O: BlockEntityCompoundOwner,
    {
        let target =
            self.fresh_beacon_without_metadata(powers, registries, names, components, compounds)?;
        compounds.into_ordered_tag(target)
    }

    pub fn save_with_id<N, R, K, O>(
        &self,
        powers: &R,
        registries: &N::RegistryLookup,
        names: &mut N,
        components: &mut K,
        keys: &impl BlockEntityTypeKeys<T>,
        compounds: &mut O,
    ) -> Result<Tag>
    where
        N: EnchantingNameCodec<C>,
        R: BeaconPowerRegistry<E>,
        K: BlockEntityComponentCodec<M, RegistryLookup = N::RegistryLookup>,
        O: BlockEntityCompoundOwner,
    {
        let mut target =
            self.fresh_beacon_without_metadata(powers, registries, names, components, compounds)?;
        self.base.save_id(&mut target, keys, compounds)?;
        compounds.into_ordered_tag(target)
    }

    pub fn save_with_full_metadata<N, R, K, O>(
        &self,
        powers: &R,
        registries: &N::RegistryLookup,
        names: &mut N,
        components: &mut K,
        keys: &impl BlockEntityTypeKeys<T>,
        compounds: &mut O,
    ) -> Result<Tag>
    where
        N: EnchantingNameCodec<C>,
        R: BeaconPowerRegistry<E>,
        K: BlockEntityComponentCodec<M, RegistryLookup = N::RegistryLookup>,
        O: BlockEntityCompoundOwner,
    {
        let mut target =
            self.fresh_beacon_without_metadata(powers, registries, names, components, compounds)?;
        self.base.save_metadata(&mut target, keys, compounds)?;
        compounds.into_ordered_tag(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        block_entity::NativeBlockEntityBase,
        block_entity_compound::NativeFreshCompoundOwner,
        block_entity_empty_components::NativeEmptyComponentMap,
        source_nbt_decode::SourceDecodedNbt,
        workstations::beacon_entity::{
            BeaconBaseFactory, BeaconClass, BeaconClassEffects, BeaconClassInitializer,
        },
    };
    use anyhow::{bail, ensure, Context};
    use std::{cell::Cell, sync::Arc};

    type Effect = Arc<CookieKey>;
    type Name = Arc<JavaString>;

    #[derive(Clone, Debug)]
    struct Identity(Arc<()>);
    impl PartialEq for Identity {
        fn eq(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.0, &other.0)
        }
    }
    impl Eq for Identity {}

    // These are controlled holder/name/class/type capabilities, not an actual
    // configured registry or Main owner. The base EMPTY, decoder-issued
    // compound views and mutable native CompoundTag algorithm are genuine
    // existing native implementations, rather than copied Tag stand-ins.
    struct Powers {
        identity: Identity,
        values: [Effect; 6],
        fail_secondary: Cell<bool>,
    }
    impl BeaconPowerRegistry<Effect> for Powers {
        type Identity = Identity;
        fn identity(&self) -> Result<Identity> {
            Ok(self.identity.clone())
        }
        fn validate_identity(&self, identity: &Identity) -> Result<()> {
            ensure!(identity == &self.identity, "fixture registry replaced");
            Ok(())
        }
        fn builtin_effect_id(&self, effect: &Effect) -> Result<i32> {
            Ok(self
                .values
                .iter()
                .position(|value| Arc::ptr_eq(value, effect))
                .map_or(-1, |index| index as i32))
        }
        fn builtin_effect_by_id(&self, id: i32) -> Result<Option<Effect>> {
            Ok(usize::try_from(id)
                .ok()
                .and_then(|id| self.values.get(id))
                .cloned())
        }
        fn builtin_effect_by_key(&self, key: &CookieKey) -> Result<Option<Effect>> {
            if key.as_str() == "minecraft:regeneration" && self.fail_secondary.get() {
                bail!("fixture secondary lookup unavailable");
            }
            Ok(self
                .values
                .iter()
                .find(|value| value.as_ref() == key)
                .cloned())
        }
        fn builtin_effect_key(&self, effect: &Effect) -> Result<Option<CookieKey>> {
            ensure!(
                self.values.iter().any(|value| Arc::ptr_eq(value, effect)),
                "fixture received foreign holder"
            );
            Ok(Some(effect.as_ref().clone()))
        }
        fn is_valid_beacon_effect(&self, effect: &Effect) -> Result<bool> {
            Ok(self.values.iter().any(|value| Arc::ptr_eq(value, effect)))
        }
    }

    struct Constants([Vec<Effect>; 4]);
    impl BeaconClassEffects<Effect> for Constants {
        fn tier(&self, index: usize) -> Result<&[Effect]> {
            Ok(self.0.get(index).context("fixture tier out of range")?)
        }
        fn contains(&self, effect: &Effect) -> Result<bool> {
            Ok(self
                .0
                .iter()
                .flatten()
                .any(|value| Arc::ptr_eq(value, effect)))
        }
    }
    struct Initializer {
        constants: Arc<Constants>,
        name: Name,
    }
    impl BeaconClassInitializer<Effect, Name> for Initializer {
        fn initialize_effect_constants(&mut self) -> Result<Arc<dyn BeaconClassEffects<Effect>>> {
            Ok(self.constants.clone())
        }
        fn translatable_default_name(&mut self, key: &'static str) -> Result<Name> {
            ensure!(
                key == "container.beacon",
                "fixture received wrong default name"
            );
            Ok(self.name.clone())
        }
    }
    struct BaseFactory(Arc<JavaString>);
    impl BeaconBaseFactory<Arc<JavaString>, (), NativeEmptyComponentMap> for BaseFactory {
        fn create_beacon_base(
            &mut self,
            position: (i32, i32, i32),
            state: u32,
        ) -> Result<NativeBlockEntityBase<Arc<JavaString>, (), NativeEmptyComponentMap>> {
            NativeBlockEntityBase::construct(
                self.0.clone(),
                position,
                state,
                NativeEmptyComponentMap::singleton(),
            )
        }
    }
    struct Names {
        parses: usize,
    }
    impl EnchantingNameCodec<Name> for Names {
        type RegistryLookup = ();
        fn translatable(&mut self, _: &'static str) -> Result<Name> {
            bail!("fixture name translation is not used by serialization");
        }
        fn serialize_custom_name(&mut self, component: &Name, _: &()) -> Result<JavaString> {
            Ok(component.as_ref().clone())
        }
        fn parse_custom_name_safe(&mut self, json: &JavaString, _: &()) -> Result<Option<Name>> {
            self.parses += 1;
            // Controlled callback outcome only: this is not a JSON parser or
            // a substitute production Component codec.
            Ok((json != "fixture-invalid").then(|| Arc::new(json.clone())))
        }
    }
    struct RejectComponents;
    impl BlockEntityComponentCodec<NativeEmptyComponentMap> for RejectComponents {
        type RegistryLookup = ();
        fn parse_with_source_diagnostics(
            &mut self,
            _: &Tag,
            _: &(),
        ) -> Result<Option<NativeEmptyComponentMap>> {
            bail!("fixture component codec unavailable after subclass fields");
        }
        fn encode_with_source_diagnostics(
            &mut self,
            _: &NativeEmptyComponentMap,
            _: &(),
        ) -> Result<Option<Tag>> {
            bail!("fixture component encoder is not used");
        }
    }
    fn decoded(entries: Vec<(&str, Tag)>) -> Result<SourceDecodedNbt> {
        let tag = Tag::Compound(
            entries
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .collect(),
        );
        SourceDecodedNbt::decode_anonymous(&tag.encode_anonymous()?)
    }

    #[test]
    fn source_additional_prefix_omissions_and_update_tag_retain_actual_compound_owner() -> Result<()>
    {
        let powers = Powers {
            identity: Identity(Arc::new(())),
            values: [
                "speed",
                "haste",
                "resistance",
                "jump_boost",
                "strength",
                "regeneration",
            ]
            .map(|key| CookieKey::parse(key).map(Arc::new))
            .into_iter()
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| anyhow::anyhow!("fixture effect constant count"))?,
            fail_secondary: Cell::new(false),
        };
        let mut class = BeaconClass::uninitialized();
        class.initialize(&mut Initializer {
            constants: Arc::new(Constants([
                powers.values[0..2].to_vec(),
                powers.values[2..4].to_vec(),
                powers.values[4..5].to_vec(),
                powers.values[5..6].to_vec(),
            ])),
            name: Arc::new("fixture class default".into()),
        })?;
        let mut entity = BeaconBlockEntity::construct(
            (4, 64, 5),
            0,
            Arc::new(class),
            &mut BaseFactory(Arc::new("minecraft:beacon".into())),
        )?;
        entity.levels = 7;
        entity.secondary = Some(powers.values[0].clone());
        let old_name = Arc::new(JavaString::from("Beacon witness"));
        entity.name = Some(old_name.clone());
        let old_lock = BeaconLockCode::from_key("named key".into());
        entity.lock = old_lock.clone();
        let mut names = Names { parses: 0 };
        let source = decoded(vec![
            (PRIMARY, Tag::String("speed".into())),
            (SECONDARY, Tag::String("regeneration".into())),
            ("Levels", Tag::Int(99)),
            ("CustomName", Tag::Int(5)),
            ("Lock", Tag::String("".into())),
        ])?;

        powers.fail_secondary.set(true);
        assert!(entity
            .load_additional(source.root_compound().unwrap(), &powers, &(), &mut names)
            .is_err());
        assert!(Arc::ptr_eq(
            entity.primary.as_ref().unwrap(),
            &powers.values[0]
        ));
        assert!(Arc::ptr_eq(
            entity.secondary.as_ref().unwrap(),
            &powers.values[0]
        ));
        assert!(Arc::ptr_eq(entity.name.as_ref().unwrap(), &old_name));
        assert!(Arc::ptr_eq(&entity.lock, &old_lock));
        assert_eq!(entity.levels, 7);

        powers.fail_secondary.set(false);
        entity.load_additional(source.root_compound().unwrap(), &powers, &(), &mut names)?;
        assert!(Arc::ptr_eq(
            entity.secondary.as_ref().unwrap(),
            &powers.values[5]
        ));
        assert!(Arc::ptr_eq(entity.name.as_ref().unwrap(), &old_name));
        assert_eq!(names.parses, 0);
        assert_eq!(entity.levels, 7);
        assert_eq!(&*entity.lock, &*BeaconLockCode::no_lock());
        assert!(!Arc::ptr_eq(&entity.lock, &BeaconLockCode::no_lock()));

        let later = decoded(vec![
            (PRIMARY, Tag::String("haste".into())),
            ("CustomName", Tag::String("fixture new name".into())),
            ("Lock", Tag::String("fixture new lock".into())),
        ])?;
        assert!(entity
            .load_with_components(
                later.root_compound().unwrap(),
                &powers,
                &(),
                &mut names,
                &mut RejectComponents,
            )
            .is_err());
        assert!(Arc::ptr_eq(
            entity.primary.as_ref().unwrap(),
            &powers.values[1]
        ));
        assert!(entity.secondary.is_none());
        assert_eq!(entity.name.as_ref().unwrap().as_ref(), "fixture new name");
        assert_eq!(entity.lock.key(), "fixture new lock");
        assert_eq!(entity.levels, 7);

        entity.name = None;
        entity.lock = BeaconLockCode::no_lock();
        let mut compounds = NativeFreshCompoundOwner::new();
        let mut target = compounds.fresh_compound()?;
        for key in [SECONDARY, "CustomName", "Lock"] {
            compounds.put(&mut target, key.into(), Tag::String("preexisting".into()))?;
        }
        entity.save_additional(&mut target, &powers, &(), &mut names, &mut compounds)?;
        let tag = compounds.into_ordered_tag(target)?;
        assert_eq!(
            tag.get(PRIMARY),
            Some(&Tag::String("minecraft:haste".into()))
        );
        assert_eq!(tag.get("Levels"), Some(&Tag::Int(7)));
        for key in [SECONDARY, "CustomName", "Lock"] {
            assert_eq!(tag.get(key), Some(&Tag::String("preexisting".into())));
        }
        let update = entity.get_update_tag(&powers, &(), &mut names, &mut compounds)?;
        let Tag::Compound(entries) = update else {
            bail!("actual fresh save is not compound");
        };
        assert_eq!(entries.len(), 2);
        assert!(entries
            .iter()
            .all(|(key, _)| key == PRIMARY || key == "Levels"));
        Ok(())
    }
}
