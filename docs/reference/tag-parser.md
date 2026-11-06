# SNBT reader — `TagParser`

Target: Minecraft Java **1.21**. `src/tag_parser.rs` ports `net.minecraft.nbt.TagParser` over
Brigadier's `StringReader`:
- `parse_compound` is `TagParser.parseTag`: one compound with nothing after it.
- `parse_value_prefix` is `readValue`, which reads one value and leaves the rest unread.

The first user is the loot function `set_custom_data` (`TagParser.LENIENT_CODEC`). Commands that
take NBT arguments (`/data`, `/give`, `/summon`) will need the same reader. No Main callsite uses
it yet, and it claims no checklist point.

## Behaviour (each rule checked against the original)

- **Number suffixes.** Suffixes and `e` exponents are case-insensitive (`1B`, `2L`, `3S`, `1.5F`,
  `1D`, `1E3`).
  - A plain integer that fits is an int.
  - A decimal without a suffix is a double.
  - `true` and `false`, in any case, are bytes.
- **Out-of-range numbers become strings.** A token that matches a pattern but does not fit its
  type is a string. For example, `300b` and `2147483648` are strings, and no later pattern is
  tried for them.
- **Leading zeros.** `007` and `01b` are strings, but `00.5` is a double.
- **Signed zero.** `-0f` and `-0.0d` read as positive zero, because `FloatTag.valueOf` and
  `DoubleTag.valueOf` return their shared zero.
- **Strings and keys.** Both may be quoted with `"` or `'`, where `\\` and the quote character
  are the only escapes. Unquoted text uses `0-9A-Za-z_-.+`. Whitespace is Java's
  `Character.isWhitespace`.
- **Lists and arrays.**
  - A list holds a single tag type.
  - `[B;`, `[I;` and `[L;` arrays accept only bytes, ints or longs respectively.
  - A repeated key replaces the earlier value.
  - A trailing comma before `}` or `]` is accepted.

## Evidence

`tools/loot_oracle.java snbt` runs the unchanged `TagParser.parseTag`.
`tools/tag_parser_generate.py` builds its inputs from 65 hand-written seeds plus each number
token alone and inside a list. It then mutates each of those strings one character at a time
(delete, duplicate, or replace from a fixed set).

The test `tag_parser::tests::snbt_matches_the_original_tag_parser` checks accept/reject and the
parsed tag. Compounds are compared as maps, because the original iterates a `HashMap`.

- **Committed fixture:** `fixtures/tag-parser-vectors.json` has 10,801 vectors (2,194 accepted).
- **Full local run:** `--per-seed 0` produces 41,004 vectors (9,553 accepted). Point
  `TAG_PARSER_VECTORS` at that output to run them. All agree.

## Not covered

- **No depth limit.** The original overflows its stack on very deep input, and this port recurses
  in the same way. Bound the length of untrusted input.
- **Error messages.** Messages and cursor positions are approximations; only accept/reject is
  checked.
