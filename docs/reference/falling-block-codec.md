# Pinned BlockState NBT codec

`falling_block/block_state_codec.rs` implements the concrete Java 1.21
`NbtUtils.readBlockState`, property parsing and `writeBlockState` algorithm for
the pinned block/state domain. It implements the existing
`FallingBlockStateCodec` interface. Module declaration, compiler validation and
an actual Main/import caller remain integration work; this file is not a claim
of working Java entity import or complete FallingBlock gameplay.

## Source identity and inspection

The original server SHA-1 is
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`; inner server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official mapping SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`.
Evidence is direct JDK21 `javap -p -c` inspection of the pinned original classes
and the retained JDK21 implementation. No REA MCP execution or new Java
gameplay/original-registry bootstrap observation is claimed. Raw disassembly
and proprietary binaries stay outside the repository.

| Original method | Implemented behavior |
| --- | --- |
| `NbtUtils.readBlockState`, `uq.a(HolderGetter,CompoundTag)`, source151–175 | Requires StringTag Name; parses the resource location; obtains the actual optional holder; starts at that Block's default state; reads Properties only when it is a CompoundTag |
| `ResourceLocation.parse/bySeparator`, `akr.a`, source56/77–89 | First colon splits namespace/path; absent or explicitly empty namespace selects minecraft; permitted empty paths remain valid; namespace/path character validation is case-sensitive |
| `NbtUtils.setValueHelper`, source180–186 | Known valid property value replaces that one value; invalid value calls the actual logger with property, source getString value and original input, then retains the previous state; unknown property names are ignored |
| `BooleanProperty.getValue`, source28–31 | Only exact true/false strings are accepted |
| `IntegerProperty.getValue`, source63–67 | Actual Java Integer.valueOf/parseInt behavior, then inclusive property range membership; NumberFormatException becomes an absent value |
| `EnumProperty.getValue`, source39 | Exact serialized-name lookup; no case conversion or ordinal guessing |
| `NbtUtils.writeBlockState`, source190–204 | Writes actual registry key Name; only a state with properties writes Properties; values use actual Property.getName and StateHolder entry encounter order |
| `CompoundTag.getString`, `ub.l` | Wrong/missing value type produces the source empty Java String, not numeric text |
| `StateDefinition` constructor, `dtd` | Its property definitions use an ImmutableSortedMap; raw state/value binding is separately checked against the actual registry |

Missing/wrong-type Name and a syntactically valid but absent registry key return
the actual AIR default, as the original does. Invalid resource syntax throws in
the original and therefore returns a typed `InvalidBlockResourceLocation`
error, preserving the raw Java String and failing component. It must not be
converted into AIR. Empty, additional-colon, uppercase, whitespace and malformed
UTF-16 cases follow this distinction. The existing Falling additional-load
body later converts an actual AIR result to the actual SAND default at its own
source phase; the codec does not invent that replacement earlier.

The integer parser retains raw UTF-16, ASCII signs, leading zeros, overflow and
MIN_VALUE behavior. BMP decimal digits use the same 37 JDK21 zero ranges
already observed in the repository's `world_rules`/`connection_settings`
source work. Supplementary digits are surrogate pairs and are rejected by the
source char-by-char parser. Parsing failure is distinct from the valid zero
property value.

## Actual registry and input ownership

`PinnedBlockStateCodec::capture` borrows a mandatory `ActualBlockRegistry` and
`BlockStateNbtContext`. It checks actual source/version provenance, all 1060
block definitions/defaults/property domains and all 26684 raw-state/property
bindings before using the native catalogue. The captured registry identity is
checked again during later operations; registry/tag replacement is not silently
accepted. There is no Default, supported flag, guessed AIR/SAND ID or dynamic
registry-to-compiled fallback.

The original placement catalogue SHA-256 is
`e044d4bdf90cfcae62043b630b1c673bb3582025b684a7fbe5878fe6747e8a50`.
The secondary item/state catalogue SHA-256 is
`d2bdc81b2d69126f18b1518ffe674f55e362910bb7be118e548bcdd7815c61ea`.
That catalogue records shared-protocol minecraft-data 1.21.1 metadata; matching
placement ranges alone would not certify all property bindings. This is why
capture also compares every binding with the actual registry owner. No new
runtime observation of those comparisons is claimed until a real caller runs.

AIR and ANVIL membership queries delegate to that same actual registry/tag
owner. ANVIL does not use a compiled-tag guess. The writer obtains the actual
state's `StateHolder.getValues().entrySet` order and validates its complete key
membership before constructing output. No BTreeMap or wire order is substituted.

For nonempty input Properties, `property_key_order` must supply the actual
retained CompoundTag HashMap key encounter order. The codec checks its complete,
unique logical key set and reads the last logical value of duplicate names,
matching CompoundTag insertion. An arbitrary decoded NBT Vec/wire order is not
that owner. `warn_invalid_property` is mandatory and receives the exact source
message template and original input object. There is no successful no-op logger.
Callbacks must not mutate borrowed input/registry state, and the caller retains
the actual access/lifetime interval. Missing capabilities are errors, not an
invented warning order or successful import.

## Fresh output compounds

The writer has a bounded concrete HashMap port for its freshly constructed
compounds. Original `CompoundTag()` calls `Maps.newHashMap`; the actual JDK21
HashMap hashes Java String with `hash ^ (hash >>> 16)` and iterates table buckets.
Pinned states have at most seven properties; the outer compound has at most two
keys. Their fresh default-capacity16 table neither resizes nor treeifies, so
bucket order with stable insertion order within a bucket is sufficient and is
implemented directly. This does not reconstruct an existing arbitrary input
compound's capacity/history and is not used as its fallback.

## Handoff and limits

Callable APIs are `PinnedBlockStateCodec::capture` and the five implemented
`FallingBlockStateCodec` operations. Required owner interfaces are
`ActualBlockRegistry` and `BlockStateNbtContext`; registry property definitions,
actual per-state write order and typed resource parse errors are public types.
No actual Main registry/map/logger provider, Java entity importer, module export,
new manifest dependency, source archive or progress metric is added here.
Focused compilation belongs to the integration coordinator after module wiring;
comprehensive conformance work remains later.
