# Placement fluid-reference correction

The original placement extractor confused `Fluids.FLOWING_LAVA` with
`Fluids.FLOWING_WATER`. This separate source-data fix changes only that field
reference and regenerates the existing placement fixture's `fluids` value from
fresh reads of every actual original state. It does not modify PR100's
`FluidState`-based WATER admission, which used the correct fluid owner already.

## Original source and minimal extraction

Pinned Minecraft Java 1.21 inner-server SHA-256:
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official mappings SHA-1:
`31c77994d96f05ba25a870ada70f47f315330437`.

The official `Fluids -> epf` mappings, lines 92061 onward, and retained direct
`javap` evidence identify:

| Original field | Actual singleton |
| --- | --- |
| `epf.a` | EMPTY |
| `epf.b` | FLOWING_WATER |
| `epf.c` | WATER |
| `epf.d` | FLOWING_LAVA |
| `epf.e` | LAVA |

`tools/placement_catalog.java` previously selected `epf.c` as source water but
`epf.d` as its flowing-water companion. The correction changes exactly the
mistaken `flowing` field lookup from `d` to `b`; the rest of that extractor's
bytes remain unchanged.

The new minimal `tools/placement_fluid_facts.java` runs through the JDK21 source
launcher with the unchanged pinned original inner server and bundled libraries.
It obtains the actual Block state IdMapper, requires its 26,684-state domain,
and queries every nonnull state using `getFluidState`, `getType` and `getAmount`.
It compares actual singleton references, not class-name suffixes or guessed
fluid registry IDs. The raw output retains observed type codes, amounts and
generated flags for every state, plus explicit field-name/code interpretation.
Those codes are local observation indexes, not registry IDs.

This direct original-class extraction exited 0 with empty stderr. It invokes no
block-entity factory or world/server gameplay; the older whole placement
extractor and its block-entity constructor loop were not re-executed. This was
direct JDK work, not REA MCP execution. No original binary or raw disassembly is
distributed with the source.

## Corrected flags and exact scope

The existing bit definitions are unchanged:

| Bit | Predicate |
| --- | --- |
| 1 | Actual type is source WATER (`epf.c`) |
| 2 | Actual type is WATER or FLOWING_WATER (`epf.c` or `epf.b`) |
| 4 | The water predicate above holds and the actual fluid amount is 8 |
| 8 | Actual type differs from EMPTY (`epf.a`) |

The original reads found FLOWING_WATER exactly at states 81..95 and FLOWING_LAVA
at 97..111. The observed corrected differences are:

| State range | Previous flags | Correct flags |
| --- | --- | --- |
| 81..87 | 8 | 10 |
| 88..95 | 8 | 14 |
| 97..103 | 10 | 8 |
| 104..111 | 14 | 8 |

Source-water state 80 remains 15 and source-lava state 96 remains 8. All other
states, including source water carried by waterlogged states, remain unchanged.
The old array was independently reconstructed exactly from the old mistaken
formula using the newly observed original types/amounts at every state. Thus
the field-selection error explains the entire difference; no expected flags
were hand-patched to make a case pass.

The fixture update replaces only the `fluids` array value. All other JSON fields
compare equal and every byte outside that one array value remains identical,
including formatting. The array's original formatting is retained while its
values come directly from the original extraction. The actual canonical
placement OnceLock and its new borrowed-view API are not replaced by another
catalogue or cache.

## Artifact identities

| Artifact | SHA-256 |
| --- | --- |
| Original placement fixture | `e044d4bdf90cfcae62043b630b1c673bb3582025b684a7fbe5878fe6747e8a50` |
| Corrected placement fixture | `97d799588bbb62449d74b12addcc4722358be7e5fcbce10082e81b058e1df412` |
| Original whole extractor | `74e801ef7ea0a7ad4a85303d027083a85197bc09f5525abdf7bcf9dde61e8232` |
| Corrected whole extractor | `d08c6e565c2b8fd66590fce825a0687e39c7a203e5e6fd0d6876464c62a54475` |
| New minimal extractor | `8214b791f1aabe9fe0de1b00db67ff5f8197fafad221d0dd9c0bf315b29dd05f` |
| Raw original type/amount/flag observations | `20a9dcbd8d75bf9d0f95740e7e60054adfa3d7105aaf1df386434401d6c73e82` |
| Scope/difference verification record | `3c65752933b0d7a07b3ff3293cbb362b6a66d01e16e7265b9e25ec1414f513e9` |

Raw original observations and the exact original fixture/extractor copies are
retained privately for provenance. Full-file digest consumers must be rebound
explicitly after this verified change; historical runtime records retain their
original source/fixture identity. This correction does not rerun gameplay,
certify Java saves, repair every fluid path or alter checklist scope/progress.
