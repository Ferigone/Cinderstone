# Native configured Loom pattern owner — Java 1.21

`src/main_loom_patterns.rs` implements `LoomPatterns` from the **same immutable
configuration** that Main sends to its Java client. It does not install a Main
module, open a menu, implement a player/world context or create an inventory.
The future `LoomMenuContext` owner must compose its actual player, level, block,
callback and lifetime identities with this resolver's configuration binding.

## Pinned source and observations

The reference is the unchanged Java1.21 inner server JAR, SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`, with official
mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
Work used direct **JDK21 `javap -p -c`**, `-v` constructor-bootstrap inspection
and bounded reflection queries against the unchanged original bootstrap. REA
MCP tools were not used. Raw Java artifacts, queries and disassembly remain
outside the repository.

Source owners inspected:

- `BannerPatternItem` (`csm`): constructor stores the actual `TagKey`;
  `getBannerPattern`, `b()`, returns that field.
- `BannerPatternTags` (`awc`): the actual `NO_ITEM_REQUIRED` field names
  `minecraft:no_item_required`.
- `LoomMenu` (`cra`): `getSelectablePatterns`, source lines151–157, uses the
  actual default tag for an empty template, the actual `BannerPatternItem` tag
  for that class, and an empty list for another item. A missing tag returns an
  empty list. Its bootstrap maps the returned `HolderSet.Named` through
  `ImmutableList.copyOf(Iterable)`, preserving encounter order.
- `TagNetworkSerialization` (`awx`): serialization iterates the actual holder
  set and appends each registry ID to an `IntList`; deserialization traverses
  that list in order. Tag-key map order is not pattern-list order.
- `ClientboundRegistryDataPacket` (`abe`) supplies ordered registry entries;
  `ClientboundUpdateEnabledFeaturesPacket` (`abh`) carries a feature **set**.
- Original `BannerItem` (`csl`), `DyeItem` (`ctj`), the actual built-in item
  registry (`lt.g`), `Item.requiredFeatures` and `FeatureFlags` (`cpn`).

The original bootstrap query inspected real registered objects, not names
selected by a suffix. The exact template results were:

| Original item | Item ID | Actual `getBannerPattern().location()` |
| --- | --- | --- |
| `flower_banner_pattern` |1195| `minecraft:pattern_item/flower` |
| `creeper_banner_pattern` |1196| `minecraft:pattern_item/creeper` |
| `skull_banner_pattern` |1197| `minecraft:pattern_item/skull` |
| `mojang_banner_pattern` |1198| `minecraft:pattern_item/mojang` |
| `globe_banner_pattern` |1199| `minecraft:pattern_item/globe` |
| `piglin_banner_pattern` |1200| `minecraft:pattern_item/piglin` |
| `flow_banner_pattern` |1201| `minecraft:pattern_item/flow` |
| `guster_banner_pattern` |1202| `minecraft:pattern_item/guster` |

The same query identified actual BannerItem IDs1133–1148 and DyeItem
IDs944–959. The module exposes explicit class membership for these IDs.
Every registered original item, dense IDs0–1332, had required features contained
in the actual `VANILLA_SET`; the observed `DEFAULT_FLAGS` names were exactly
`minecraft:vanilla`. Every native item ID/name matched this original registry
query. This is an enabled-feature certificate, not an item-callback, attribute,
bundle-click or component-equality certificate.

## Configuration decoding and binding

`NativeLoomPatterns::new(&Fixtures)` locates the unique CONFIG0x07 registry whose
key is `minecraft:banner_pattern`, the actual CONFIG0x0d tags and actual
CONFIG0x0c feature packet. It decodes their actual payloads; no default tag list
or registry order is supplied when data is absent. Registry entries retain
their actual numeric position, resource key and optional NBT. An omitted
KnownPack payload stays `None`, never an invented asset or translation record.

The existing Slot component49 decoder already uses the pinned registry in
`tools/crafting_metadata.json`. The new resolver validates the actual configured
ID→key sequence against that same existing owner before accepting it. A changed,
reordered, shortened or extended registry is explicitly unsupported; the module
does not mix the client registry with another native holder domain. The expected
keys are admission evidence only. Selectable patterns are always taken from the
actual tag payload, never generated from metadata or sorted keys.

The resolver stores each tag's IDs in its exact vector order. The current
capture's default sequence is:

```text
26,27,28,29,31,38,35,37,32,36,34,33,25,5,30,39,40,
41,42,7,10,9,8,3,23,19,17,20,18,1,6,14,15,2
```

Template tags in that capture map to actual holder IDs: flower12, creeper4,
skull24, mojang21, globe13, piglin22, flow11 and guster16. These observations
verify the configured input; they are **not** fallback constants in the code.
Missing tags return the source's empty list. Non-template nonempty items return
an empty selectable list rather than acquiring a guessed tag.

`LoomPatternBinding` contains the address of the actual stable `Fixtures` owner
and exact registry/tag/feature payload bytes held by shared immutable slices.
It contains no menu-local registry counter. It is not persisted. A future Main
context must retain the actual server/configuration owner for the menu's
lifetime and carry resolver clones, rather than transferring an old menu to a
new owner. Equal feature-set contents do not hide a changed packet snapshot.

The parser uses the existing configuration2MiB admission cap and bounded
collection sizes, validates resource locations, rejects duplicate relevant
registry/tag keys and out-of-range banner holders, and requires complete packet
consumption. These are explicit native admission checks, not a claim that every
malformed or custom configuration has the same acceptance behavior as Java.
The enabled profile is exactly the original default set; alternate feature
snapshots are rejected rather than treated as all-features-enabled.

## Callable contract

| API | Result / responsibility |
| --- | --- |
| `NativeLoomPatterns::new(&Fixtures)` | Validated immutable actual configured registry, tags and feature profile. |
| `binding()` | Cloneable/equatable `LoomPatternBinding` for the actual context binding. |
| `enabled_features()` | Borrow of the actual decoded feature set. |
| `registry()` | Actual `CapturedBannerPattern` entries: `registry_id`, `key`, optional original `data`. |
| `patterns_for_tag(&str)` | Borrowed vector in source encounter order; missing tag gives empty slice. |
| `template_tag(&ItemStack)` | Original class-derived tag; empty/non-template gives `None`. |
| `is_banner_item` / `is_dye_item` | Original class-derived membership, with empty short circuit. |
| `require_item_enabled` | Source `isItemEnabled` empty behavior and captured default item-feature admission. |
| `LoomPatterns::is_pattern_item` / `selectable_patterns` | Actual template classification and ordered tag resolution for the kernel. |
| `LOOM_TAKE_SOUND_ID` |1429: original `avp.At` is the same actual registered `minecraft:ui.loom.take_result` object. |

Sound distribution/RNG and the constructor-default update-listener source proof
have a separate owner in
[loom-sound-source.md](loom-sound-source.md), with the source facts in
`fixtures/loom-production-sound.json`. Independent bounded observation here
also established the exact sound object identity, ID1429 and absent fixed
range. The `cra` constructor's Runnable bootstrap0 targets `cra.r()`, whose
body consists of `return`. This proves only that genuine default listener;
it does not certify a custom callback environment as empty. The actual Main
host must consume the kernel's source events without repeating its already
recorded recomputation/broadcasts.

## Integration limits and verification

The constructor and resolver implement configuration admission and pattern
lookups only. Production opening, binding to the live level/player, ordered
slot/counter/source callbacks, item ownership, actual sound RNG/audience,
drop creation, pickup synchronization, removal and persistence remain with
their assigned Main owners. No XP, crafted-item statistic, advancement or
recipe callback is invented here. In particular, the pattern provider does not
provide Java Slot identity-HashSet order for a completed multi-target drag.
That context must remain unsupported until its real owner exists.

Validation performed by this stage: unchanged original bootstrap item/tag and
sound queries, direct source inspection, actual captured-tags decoding with
complete consumption, and original/native item-ID comparison. No Rust build,
Main launch or production client flow was performed by this module's owner.
The module's mere presence does not establish a Main callsite or a checklist
delivery. Comprehensive conformance testing remains deferred.
