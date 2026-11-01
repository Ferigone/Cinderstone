# Persistent item NBT port, Java 1.21 / data version3953

The implementation in `src/item_nbt.rs` translates persistent item compounds
to native `ItemStack` component stream payloads. It does not treat persistent
NBT as the Slot protocol representation.

Source: official1.21 `server-inner-1.21.jar`, SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Original mappings and read-only JVM bytecode inspection establish:

- `cuq` (`ItemStack`) has separate nonempty, optional, single-item and stream
  codecs. Its ordinary count codec is `intRange(1,99).fieldOf("count").orElse(1)`.
- `kp` (`DataComponentType`) exposes nullable persistent codecs. The actual
  bootstrap extractor `item_nbt_component_facts.java` writes57 registered types
  and their persistence flags to `fixtures/item-nbt-component-facts.json`.
  Only types17 and28 are transient. Persistent decode rejects these; encoding
  omits them as the original component-patch codec does.
- `cxz` (`Unbreakable`) persists `show_in_tooltip` defaulttrue, streams one bool.
- `cxk` (`DyedItemColor`) accepts the full `rgb`/tooltip form or shorthand int,
  streams signed int RGB followed by the tooltip bool.
- `cxp` (`ItemLore`) stores a size-limited list of JSON component strings and
  streams a size-limited list of structured component NBT values.
- `dai` (`ItemEnchantments`) uses levels0..255 and tooltip defaulttrue. Registry
  IDs come from the exact captured configuration's enchantment registry.
- `cxo$a` (`ItemContainerContents.Slot`) constrains slot0..255 and uses the
  nonempty `ItemStack.CODEC` for `item`. Native wire arrays include intervening
  empty slots; persistent output includes only nonempty numbered records.
- `up$b` (`NbtOps.HeterogenousListCollector`) wraps noncompound entries under
  an empty key when an NBT list mixes component strings and compounds.

Implemented component subset: custom-data NBT; max stack/max damage/damage;
unbreakable; bounded custom/item names and lore; rarity; enchantments/stored
enchantments; custom model/repair cost; tooltip units; glint override;
intangible-projectile/fire-resistant units; dyes/map colors/map ID; charged
projectiles and bundle stacks; ominous-bottle amplifier; recipe keys;
note-block sound; base dye color; nested container slots and string block-state
properties. Common text literals, styles, extra siblings and content fields
are translated to structured NBT rather than encoded as literal JSON text.

Unsupported complex component codecs and text fields return errors. This
includes attributes, food/potion/stew components, books, trim, instruments,
jukebox songs, lodestones, fireworks, profiles, banners, pot decorations,
bees, locks and container-loot components. Text `with`, score, click/hover
events, contextual resolution and arbitrary malformed-codec leniency remain
unsupported. No full component or old-version/DataFixer interoperability claim.

Unrecognized item root fields and the original component representation are
retained when the logical item/components are unchanged, including when only
count changes. Unsupported data is not silently replaced by an empty stack.

The first whole-library compiler invocation stopped inside the vendored OpenSSL
build before compiling project modules. It is not recorded as a passing Rust
validation. Root owns the environment repair and subsequent joint check.
