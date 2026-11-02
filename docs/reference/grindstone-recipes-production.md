# Native grindstone registry and callback contract

`src/main_grindstone_recipes.rs` supplies the actual configured default
enchantment/curse queries and phase admission for the controller reviewed at
`b4bccba` (PR73), now composed with PR76's genuine copy/full-state APIs in the
stations integration branch. This helper alone neither proves a gameplay flow
nor completes an aggregate checklist delivery.

## Actual registry data

`NativeGrindstoneEnchantments::new(&Fixtures, &MainItemGameplay)` first checks
the existing native gameplay source profile. It locates the actual configured
`minecraft:enchantment` RegistryData packet, tags packet and enabled-feature
packet. Their bytes must match the checked-in immutable 1.21 default capture
used by the native item holder decoder. Alternate data packs or omitted
known-pack definitions fail explicitly; no empty registry, invented costs or
hardcoded curse fallback is provided.

The capture contains 42 complete enchantment NBT definitions. Each actual
`min_cost.base` and `min_cost.per_level_above_first` was independently compared
with its JSON resource inside the pinned original server and matched. The
actual configured `minecraft:curse` tag resolves to holder 2
`minecraft:binding_curse` and holder 40 `minecraft:vanishing_curse`; the helper
derives that set from the actual packet rather than those observed constants.
For example, Sharpness holder32 has base1/per-level11; Mending holder22 has
base25/per-level25. Those examples are evidence, not substitute lookup tables.

The cloneable/equatable `NativeGrindstoneRegistrySnapshot` retains the exact
relevant packet bytes. The generic `GrindstoneMenuHost` must bind that snapshot
to its actual actor/Level/menu transaction and keep it stable through
`begin_stage`/`publish_stage`. `GrindstoneEnchantments::minimum_cost` implements
the original `Enchantment.Cost.calculate`: `base + perLevel * (level - 1)` with
Java signed-int wrapping, without clamping to the declared enchantment level.
Unknown holder IDs are errors.

## Source observations and ordering

The original reference is Minecraft Java1.21, protocol767, inner server SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
with official mappings SHA1`31c77994d96f05ba25a870ada70f47f315330437`.
Direct JDK21 `javap -p -c` inspected `GrindstoneMenu$4` (`cqt$4`),
`Enchantment.Cost` (`dac$b`) and their mapped source boundaries. This is direct
JVM analysis; no REA MCP execution is claimed. Original binaries and raw
disassembly remain outside the repository.

The result slot sums current input enchantment minimum costs, excluding the
actual curse tag, with Java int wrapping. For a positive sum the controller
draws exactly once from actual `Level.random` using bound`ceil(sum / 2)` and
returns `bound + nextInt(bound)`; bound1 still consumes a draw. Zero or
overflow-negative sums draw nothing. There is no creative bypass. The generic
controller owns this sample; the helper does not draw it again or substitute a
new RNG stream.

The source then calls actual `ExperienceOrb.award` at
`Vec3.atCenterOf(capturedBlockPos)`, even for amount0, emits Level event1042/data0,
then clears input0 and input1 in order. Each source input notification retains
the old-preview broadcast before recomputation and the subsequent new-preview
broadcast. There is no item-crafted statistic, recipe-crafted criterion or
recipe-book award in this result `onTake` path.

`grindstone_preflight_phase` checks both input observations, result preview,
external cursor, all46 canonical inventory slots and the retained window0
cursor, including nested item admission. It also validates all41 backing-slot
stack-size queries used by the actual inventory criterion callback.
`grindstone_preflight_sync(events, phase)` validates the controller's real
`SyncEvent` vector without inventing a second phased-event API. Player menu
slots3–38 map to canonical InventoryMenu slots9–44; the two inputs/result have
no player inventory listener owner. Packet offsets preserve the original event
order. Custom/reentrant criterion and callback contexts remain outside the
certified native gameplay profile.

## Real XP publication seam

`grindstone_experience_position` derives the source block-centred orb location.
`grindstone_level_event_packet` validates only1042/data0 and delegates to the
existing protocol767 level-event encoder. Its real audience belongs to the
captured Level owner: raw integer block XYZ, same dimension, radius64, null
excluded player and non-global event. This event consumes no server sound RNG.

The existing `PreparedFurnaceExperience` adapter operates on floating furnace
recipe rewards and conditional global Math.random rounding. It must not be
fed a fabricated furnace reward as a grindstone substitute. The central XP
owner must stage an actual integer `ExperienceOrb.award` transaction after the
controller's Level sample, using the same Level stream for each orb merge
group draw, the real shared entity index/ID allocator and actual new-orb RNG.
Its real world-add/merge publications precede the Level event and input clears.
No direct player XP balance increment, dummy orb or second entity store is
provided here.

The generic transactional Main host, its real integer-XP publisher, opening,
click/lifetime dispatch, compiler/runtime checks and publication belong to
their assigned integration owners. They remain separate from this immutable
registry and observation helper; no Main/deployment proof is claimed here.
