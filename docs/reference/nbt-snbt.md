# Source SNBT diagnostics

`src/nbt_snbt.rs` ports compact Java1.21 StringTagVisitor and
StringTag.quoteAndEscape into a pure lossless UTF-16 formatter. It does not
create a source-decoder capability, reconstruct a CompoundTag history, own a
logger or establish an integrated importer. Module/Main wiring and the actual
warning sink remain integration work.

## Pinned evidence

Evidence is direct JDK21 `javap -p -c` of the original `ux` (StringTagVisitor)
and `uw` (StringTag). REA tools were not exposed. No Java runtime execution or
additional conformance campaign was performed for this formatter stage.
Original outer server SHA-1 is `450698d1863ab5180c25d7c804ef0fe6369dd1ba`,
inner SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
and official mappings SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`.
Raw disassembly and Java binaries remain outside the repository.

| Original branch | Native behavior |
| --- | --- |
| StringTag.quoteAndEscape136–158, `uw.b`, offsets0–128 | Reserve the opening character; the first encountered quote selects the opposite delimiter; escape backslash and that selected delimiter; default to double quotes when no quote occurred; append the closing delimiter. |
| StringTagVisitor.handleEscape123–127, `ux.a(String)`, offsets0–19 | Leave a key unquoted only when the entire nonempty UTF-16 string matches ASCII `[A-Za-z0-9._+-]+`; otherwise use quoteAndEscape. Values of StringTag are always quoted. |
| visitString22–23 | Append quoteAndEscape of the actual string, without converting isolated surrogates or escaping control characters as JSON. |
| visitByte27–28 / Short32–33 / Int37–38 / Long42–43 | Signed decimal integers with suffixes `b`, `s`, none and `L`, respectively. |
| visitFloat47–48 / Double52–53, offsets0–17 | Actual Java StringBuilder.append(float/double) text followed by lowercase `f`/`d`. A required numeric provider supplies that exact Java21 decimal conversion. |
| visitByteArray57–66 / IntArray70–79 / LongArray83–92 | Prefix `[B;`, `[I;` or `[L;`, comma-separated signed values, then `]`. Each byte has uppercase `B`; each long has uppercase `L`; ints have no suffix. Empty arrays retain their typed prefix. |
| visitList96–104, offsets0–73 | Visit actual elements in list order, comma separated inside brackets. No element-type grouping, whitespace or additional decimal conversion is inserted. |
| visitCompound108–120, offsets10–19 and70–100 | Copy the distinct logical key set and **always sort it by Java String.compareTo**, then fetch each current value and recursively use a fresh child visitor. This is independent of HashMap encounter order. |
| visitEnd132–133 | Literal `END`. Current Tag has no End variant; `end_tag_snbt` exposes this text without minting an NBT value. |

## Ordering, duplicate names and text ownership

Native JavaString ordering compares raw unsigned UTF-16 units, including
isolated surrogates. The formatter uses that existing ordering for the source's
sorted distinct key set. Its temporary BTreeMap contains only borrowed keys and
values; it is a sorted logical projection, not an invented Java HashMap history.
For duplicate logical names in a raw Tag compound, the last value wins, matching
CompoundTag insertion and the existing native lookup policy. A shadowed earlier
value is not visited. Sorting applies independently at every compound depth.

Quotes, backslashes, NUL, tabs, newlines and non-ASCII code units follow the
literal source loop. Only backslashes and occurrences of the selected quote
gain a backslash. There is no JSON escaping, surrogate replacement, whitespace
normalization or UTF-8 conversion. The result is a JavaString, not a lossy Rust
String. A diagnostic sink must preserve that distinction when delivering its
actual log arguments; the formatter does not authorize a replacement logger.

The pure API accepts an immutable Tag. A genuine source warning can supply
`SourceCompoundView::tag()` from its already retained binary-decoder owner.
Formatting itself does not certify that identity or manufacture the view.
The actual BlockState input/warning context remains responsible for matching
the original decoder-issued node and preserving source callback order.

## Public API and explicit numeric boundary

`to_snbt(tag, numbers)` returns lossless JavaString text and requires a
`SnbtNumberFormatter`. Its two mandatory methods are
`java_float_to_string(f32)` and `java_double_to_string(f64)`, returning exact
Java21 number text without SNBT suffixes. They must preserve Java's decimal
selection, exponent spelling, signed zero, NaN and infinity behavior. They
must not mutate input, draw randomness, emit a warning or change world state.

`to_snbt_without_floating(tag)` handles every current integer, string, list,
compound and integer-array branch. A reached Float/Double returns typed
MissingSnbtNumberFormatter with the actual bits. It emits neither substitute
text nor partial output. `quote_and_escape`, `escape_key` and `end_tag_snbt`
expose the corresponding source string operations independently.

The future worktree has no shared exact Java floating formatter module. The
separate composed commands tree has a Rust-based Java-number port; this stage
does not copy it, assume an unavailable module or certify all-bit numerical
equivalence from selected examples. A genuine shared numeric owner must be
connected explicitly before Float/Double diagnostics are admitted. Rust float
Display/LowerExp is never used by this formatter. Its private Display helper
is called only with signed integer primitives, whose decimal form is exact.

Source review was limited to the formatter and its existing immutable Tag/
JavaString interfaces. No tests, builds, module declarations, Main changes,
Git operations or new progress metrics were performed in this stage.
