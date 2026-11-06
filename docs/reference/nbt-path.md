# NBT paths — `NbtPathArgument.NbtPath`

Target: Minecraft Java **1.21**. `src/nbt_path.rs` ports the path syntax and the tag operations
of `net.minecraft.commands.arguments.NbtPathArgument.NbtPath`.

Its first user is the loot function `copy_custom_data`. The `/data` and `/execute store`
commands will need the same engine. No Main callsite uses it, and it claims no checklist point.

## What it covers

### Parsing

`NbtPath::of` and `parse_prefix` parse a path. A path is built from these nodes, separated by `.`:

- **Named child:** `name` (unquoted, up to `"'[].{}` or a space), or `"quoted"` / `'quoted'`.
  A child may carry an object pattern: `name{k:1}`.
- **List and array elements:** `[index]` (negative indices count from the end), `[]` for all
  elements, or `[{pattern}]` for list elements that match the pattern.
- **Root pattern:** `{pattern}` is allowed only as the first node.

Parsing stops at the first space. Patterns are read with `crate::tag_parser`.

### Operations

These mirror the original's methods:

- `get` and `count_matching`.
- `get_or_create`, which creates missing parents using the next node's preferred container.
- `set`: the count of changed tags, with the original's equality short-cuts and its 512-level
  depth check.
- `insert(index, values)`.
- `remove`.

Each operation follows the original's rules:

- **Matching:** patterns match with `NbtUtils.compareNbt(…, true)`, so compounds and lists may
  hold more than the pattern.
- **Element types:** a list keeps a single element type. An array element is a fresh numeric tag,
  so writes through it are lost, as in the original.
- **Numeric narrowing:** the array setters narrow numbers the way `NumericTag` does
  (`Mth.floor` for floats, `& 0xff` for bytes).
- **Check order:** `insert` checks a value's type before its index. A rejected type never
  raises `invalid_index`.
- **Errors:**
  - `CommandSyntaxException` cases are separate variants: syntax, nothing found, too deep,
    expected list, and invalid index.
  - The Java runtime exceptions the original throws are reported as `Runtime`. These are
    `StringIndexOutOfBoundsException` for a path ending in `[` and
    `UnsupportedOperationException` when adding a pattern to a list of another type.

The original holds references to tags. The port addresses every match by its steps from the
root, which is equivalent because each node descends exactly one level (a root pattern none).
The matches of one step therefore never contain one another.

## Evidence

`tools/loot_oracle.java nbt-path` runs the unchanged original. `tools/nbt_path_generate.py` builds
the inputs from 77 hand-written paths plus their one-character mutants, applied to five root
compounds. For each path and root it runs:

- `get`, `count`, `get_or_create` and `remove`;
- `set` with ten values;
- `insert` with seven index and value combinations.

The test `nbt_path::tests::nbt_paths_match_the_original` checks four things:

- whether the path parses and, if so, how much of it;
- the error stage and kind;
- the count or matched tags;
- the root after the operation, compared as a map.

Results:

- **Committed fixture:** `fixtures/nbt-path-vectors.json` holds every 8th run (6,731).
- **Full local run:** `--per-path 0 --sample 1` produces 642,360 runs over 6,060 paths (138,409
  succeed). Point `NBT_PATH_VECTORS` at that output to run them. All agree with the original.

## Not covered

- **Error text:** message text and cursors are not compared.
- **Suggestions:** path suggestions and `NbtPathArgument` as a command argument type (its network
  form) belong to the command system (#24).
