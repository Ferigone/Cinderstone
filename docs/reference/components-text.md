# Text components — source ledger and evidence

Target: Minecraft Java **1.21**, protocol **767**. Primary evidence is the unchanged
original `net.minecraft.network.chat.ComponentSerialization.CODEC` run through the local
differential oracle (`tools/loot_oracle.java component`); the pinned artifacts (bundler
SHA-1 `450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inner server jar SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`, official mappings
SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`) come from
[`fixtures/source-lock.json`](../../fixtures/source-lock.json). Java artifacts, remapped
jars and decompiled text stay outside Git.

Scope of this document: `src/components/**`, work package WP-E8 stage 2 (delivery
`registries.4`, **partial**). It claims **no** checklist point and **no** Main
integration. Nothing in `src/` calls it yet.

## What is implemented

| Java | Rust | Notes |
| --- | --- | --- |
| `Component` / `MutableComponent` | `components::text::Component` | contents, siblings, style |
| `ComponentContents` (6 types) | `text::Contents` | `text`, `translatable`, `keybind`, `score`, `selector`, `nbt` |
| `Style`, `TextColor` | `text::{Style, TextColor}` | `TextColor` equality is by RGB value, serialization keeps the legacy name |
| `ClickEvent` (server-sendable actions) | `text::ClickEvent` | `open_file` is rejected as in `filterForSerialization` |
| `HoverEvent` | `text::HoverEvent` | `show_text`, including the legacy `value` form |
| `ComponentSerialization.CODEC` decode (JSON) | `Component::from_json` | string shorthand, list form, fuzzy and typed contents, `extra` |
| `CODEC` encode (JSON) | `Component::to_json` | a plain component is a bare string |
| `CODEC` encode (NBT, network form) | `Component::to_nbt`, `to_network_nbt` | `HashMap` key order, `NbtOps` list collectors |
| `java.util.HashMap` iteration order | `components::java` | `String.hashCode`, spread, bucket and resize order |

Rules that reading the source alone did not give, each one a real mismatch against the
original before it was fixed:

- **A JSON `null` member is absent** (`JsonOps`), for every optional member.
- **`Codec.BOOL` accepts only a JSON boolean** (`"Not a boolean: 1"`); `lenientOptionalFieldOf`
  members (`fallback`, `nbt.interpret`, `nbt.separator`) swallow a bad value instead.
- **Fuzzy contents order** is `text`, `translatable`, `keybind`, `score`, `selector`, `nbt`;
  with a `type` member the named type is required. `nbt` data sources are tried
  `entity`, `block`, `storage` unless `source` names one.
- **`translate` arguments** go through `ExtraCodecs.JAVA`: `JsonOps.convertTo(JavaOps)` narrows
  a number with `BigDecimal.longValueExact` to byte/short/int/long, else float when
  `(float) d == d`, else double. An object that holds a JSON `null` member anywhere throws
  (`Collectors.toMap`), so it is rejected. A component that collapses to a plain string
  becomes a string argument.
- **Legacy hover form.** `Codec.withAlternative(contents form, value form)` uses `value` only
  when the `contents` form fails (a bad `contents` next to a good `value` is accepted).
- **`ResourceLocation`** splits on the first `:`; an empty namespace means `minecraft`; an
  empty path is legal (`"x:"`, `""` and `":x"` are accepted).
- **NBT lists are built by `NbtOps.ListCollector`**: a leading byte, int or long makes a
  `ByteArray`/`IntArray`/`LongArray`; a leading compound or a type change makes a list of
  compounds in which every non-compound element is wrapped as `{"": element}`; other
  homogeneous lists stay typed lists.
- **Compound key order is `java.util.HashMap` order.** Colliding keys keep insertion order, and
  the original inserts contents, then `extra`, then the style members in *reverse*
  declaration order (`font`, `insertion`, `hoverEvent`, `clickEvent`, `obfuscated`,
  `strikethrough`, `underlined`, `italic`, `bold`, `color`). That was measured for every key
  pair that shares a bucket at capacity 16 and 32 (the only ones where order is observable).

## Evidence

`tools/component_generate.py` feeds the original codec (via `tools/loot_oracle.java component`)
and records, per input, accept or reject, the network NBT bytes (`NbtIo.writeAnyTag`) and the
`JsonOps` re-encoding.

- `fixtures/component-oracle-vectors.json`: **1579 hand-written and combinatorial inputs**
  (1425 accepted, 154 rejected): every contents type, every style member and bad value,
  colour forms, click and hover events, 51 number forms and 25 argument-list shapes for the
  list collectors, and each contents type crossed with all single and pair style members plus
  seeded larger subsets, with and without `extra` (the combinations are what exercise
  hash-bucket order). Six accepted vectors use `show_item`/`show_entity` and are expected to
  be reported as `Unsupported`.
- `fixtures/component-parse-parity.json`: a sample of **713** of **142,489** one-step
  mutants (each member removed, each value replaced by ten wrong-typed values, a foreign or
  unknown `type` added; one nested level). 39,818 were accepted and 102,671 rejected by the
  original. **The full set agrees with the Rust parser with zero disagreements, and the NBT
  bytes of every accepted mutant are identical** (142,127 compared; the other 362 mutants
  contain `show_item`/`show_entity` and are not compared). Regenerate the full set with
  `--sample 1` (about 77 MB, not committed) and run
  `COMPONENT_PARITY_FILE=<json> cargo test --lib components::tests::explore_parity -- --ignored`.

`cargo test --locked --lib components::` runs both fixtures plus 11 focused tests in about a
tenth of a second.

## Not implemented (explicit)

- `show_item` and `show_entity` hover events return `ComponentError::Unsupported` (they need
  the item-stack and entity-tooltip codecs; the legacy string forms likewise).
- **Decoding from NBT.** The model encodes to NBT but does not yet read the NBT form that item
  components, block entities and packets carry; that is the next stage.
- Resolution (`ComponentUtils.updateForEntity`, selector/score/nbt contents), `getString`,
  legacy `§` formatting, `Style.applyTo`/`Style.applyFormat`, and the `ChatFormatting` enum.
  `selector` strings are kept verbatim: the original only logs a warning for an invalid
  pattern and still accepts it.
- Text is Rust `String`: a lone UTF-16 surrogate (legal in Java and in NBT) cannot be
  represented, and `serde_json` rejects it on input.
- `serde_json::Number` keeps no decimal text, so an integer literal below `i64::MIN` is read
  as `-9223372036854775808.0` (a long) where the original makes it a float. Float values in
  `to_json` print the shortest `f32` digits; Java may add a trailing `.0` or use `E` notation,
  so the comparison in the tests is numeric.

## Reproducing the oracle

```bash
python3 tools/loot_oracle_setup.py --oracle-dir /path/outside/git      # JDK 21 required
python3 tools/component_generate.py --oracle-dir /path/outside/git     # --sample 1 keeps every mutant
cargo test --locked --lib components::
```
