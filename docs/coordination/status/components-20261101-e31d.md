# components-20261101-e31d — text `Component` model (WP-E8 stage 2)

Instance: components-20261101-e31d (same session as loot-20261101-e31d); team: lane E data-driven (no team
issue). Claim: [issue #29 comment](https://github.com/Ferigone/Cinderstone/issues/29#issuecomment-6567421929).
State: ready_for_integration (library stage; no Main callsite)
Branch: `ccr-e31d38f2-ovzstg`, restarted from master after PR #89 merged.
Capacity: one Claude Code session, no subordinate agents. Nothing continues in the background after this turn.

## Owned files

New and exclusive: `src/components/**`, `fixtures/component-oracle-vectors.json`,
`fixtures/component-parse-parity.json`, `tools/component_generate.py`,
`docs/reference/components-text.md`, this file. Shared edits: one `pub mod components;` line in
`src/lib.rs` (standing mechanical-export exception) and a new `component` mode in
`tools/loot_oracle.java` (the oracle harness this instance already owns). `src/item_nbt.rs`,
`src/inventory.rs` and `src/nbt.rs` were **not** touched (the model only uses the public
`nbt::Tag` API). No Main, runtime, manifest, checklist, README, roadmap, journal or deployment
file was touched.

## Delivery IDs and bounded behavior

`registries.4` — **partial** (stage 2 of the work package only). Preserves `scope_version: 1`,
100 items, **56/100 code, 51/100 Main** (unchanged; this stage adds no point).

Implemented: `Component` with the six contents types, `Style` (colour, five flags, click, hover
`show_text`, insertion, font), siblings, the string/list shorthand, JSON decode/encode and NBT
encode (network form) of `ComponentSerialization.CODEC`, `TextColor` equality, `HashMap` key
order and `NbtOps` list collectors. Not implemented: `show_item`/`show_entity` hovers
(`Unsupported`), decoding from NBT (stage 3), resolution/`getString`/legacy formatting, lone
UTF-16 surrogates. Details: [components-text.md](../../reference/components-text.md).

## Source evidence

Pinned server and official mappings (identities in the reference document); the **executed**
unchanged original `ComponentSerialization.CODEC` through `tools/loot_oracle.java component`.
Local decompilation of `NbtOps`, `HoverEvent`, `ClickEvent` and `Style` for reading only (output
not committed). No REA tool execution is claimed.

## Validation

`cargo test --locked --lib components::`: 14 tests passed, including **1579 differential vectors**
and a parity net whose full set (**142,489 one-step mutants**, 39,818 accepted / 102,671 rejected
by the original) was run with zero disagreements and byte-identical network NBT for every accepted
mutant (362 mutants containing `show_item`/`show_entity` are not compared); 713 of them are committed.
Not run: a protocol client, Main flow, deployment. No Railway change, no deployment claim.

## Blockers / requested contracts

None for the library. A consumer that needs to *read* components from NBT (item `custom_name`,
`lore`, block-entity names, packets) needs stage 3 (`Component::from_nbt`); the writer side can
already produce the wire form.

## Next action / handoff

Owner: this instance for stage 3 (`from_nbt`, verified through the same oracle with `NbtOps`
input) unless another instance claims it on #29 first. After it, the loot functions that need
styled components (`set_lore`, styled `set_name`, book functions) become portable.
