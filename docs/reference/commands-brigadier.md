# Brigadier reader, exceptions, primitive arguments, dispatcher core, usage and completion (WP-E3 stages 1-4)

Scope: `commands_data.1` ("Komendy i uprawnienia operatorów") is **partial**. Stages 1-4
port the lowest layers of the command system. Nothing is called by Main, the
existing bounded administration commands keep their own parser
(`admission_commands.rs`, `command_protocol.rs`), and the fixed checklist indicators
(`scope_version: 1`, 56/100 code, 51/100 Main) are unchanged.

## Source identity

- Server: `fixtures/source-lock.json` (SHA-1 `450698d1863ab5180c25d7c804ef0fe6369dd1ba`,
  inner server SHA-256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`);
  official mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
- Brigadier 1.2.9 (`com.mojang.brigadier`) is bundled unobfuscated in the server's
  `META-INF/libraries`. The classes ported are `StringReader`, `CommandSyntaxException`,
  `BoolArgumentType`, `IntegerArgumentType`, `LongArgumentType`, `FloatArgumentType`,
  `DoubleArgumentType` and `StringArgumentType`; stage 4 adds `Suggestion`,
  `IntegerSuggestion`, `Suggestions`, `SuggestionsBuilder` and `SuggestionContext`.
- The vanilla server replaces Brigadier's English literals with
  `net.minecraft.commands.BrigadierExceptions` (obfuscated `en`, found in the official
  mappings), which builds translatable components. Message keys and argument order were
  read with direct `javap` inspection of that class and confirmed by execution; this is not
  an REA MCP run. A message's identity is therefore its key plus typed arguments
  (`CommandArg`); `CommandMessage::english()` renders the pinned `en_us.json` text and exists
  for logs and tests. Turning a message into a network `Component` belongs to a later stage.

## What is implemented

Stage 1 (reader and arguments):
- `StringReader` over UTF-16 code units, because the original cursor indexes a Java
  `String`: `read*`, `expect`, `skipWhitespace`, `peek`, `canRead`, quoted-string escapes.
  `read()` advances the cursor before its bounds failure, as `charAt(cursor++)` does.
- `CommandSyntaxException`: key, typed arguments, input, cursor, `getContext()` (ten code
  units, `...` prefix, `<--[HERE]`) and `getMessage()`.
- Primitive argument types with the original range checks and cursor reset.
- `java_text`: `Character.isWhitespace` for all BMP units and `Double.toString` /
  `Float.toString` as in JDK 21 (shortest digits, closest two-digit form for one-digit
  results, even digit on exact ties).

Stage 2 (`src/commands/dispatcher/`, dispatcher core):
- An arena-based command tree (`CommandTree`, `NodeId`): `RootCommandNode`,
  `LiteralCommandNode`, `ArgumentCommandNode` with requirements, executable commands,
  redirects, redirect modifiers and forks; `LiteralArgumentBuilder` /
  `RequiredArgumentBuilder` as one `ArgumentBuilder`; `addChild` merging onto a same-named
  child. As in 1.2.9, children are **not** re-sorted: `literals` and `arguments` keep
  insertion order, which decides the order in which argument nodes are tried.
- `CommandContextBuilder` / `CommandContext` / `StringRange` / `ParsedArgument`, including
  `LinkedHashMap.put` semantics for repeated argument names and the original quirk that
  `copy()` does not carry the redirect modifier.
- `CommandDispatcher`: `register`, `parse` (relevant-node selection, per-node failures,
  `expected.separator`, redirect recursion, stable ordering of competing parses) and
  `execute`.
- `execute` uses Brigadier 1.2.x's `ContextChain` (`tryFlatten`, `runModifier`,
  `runExecutable`, `executeAll`, forked mode where every successful executable counts `1`).
  The older recursive context loop that earlier Brigadier releases (and many descriptions of
  Brigadier) use is **not** what the pinned server runs; the executed oracle caught this
  during the port and the algorithm was then read from the bytecode of the bundled classes
  (direct `javap` inspection, not an REA run).

Stage 3 (usage strings): `CommandDispatcher::all_usage(node, source, restricted)` and
`smart_usage(node, source)` plus the node usage text (`<name>` for arguments, `...` or
`-> target` for redirects, `[optional]` / `(a|b)` groups), following `getAllUsage` /
`getSmartUsage`. Children are visited in the original's insertion order.

Stage 4 (completion suggestions, `src/commands/suggestion.rs`, `java_text.rs`):
- `Suggestion` / `IntegerSuggestion` / `Suggestions` / `SuggestionsBuilder` over UTF-16 offsets:
  `suggest(String)` ignores text equal to the typed remainder, `Suggestions.create` expands every
  suggestion to the union range, de-duplicates and sorts with `compareToIgnoreCase`, and
  `Suggestions.merge` handles zero or one input specially. Because both steps pass through
  `java.util.HashSet`, suggestions that compare equal while being different (`Foo` and `foo`)
  appear in hash-bucket order; `JavaHashSet` reproduces `String.hashCode`, `Objects.hash`,
  `HashMap.hash` spreading, the load-factor resize and the resize that replaces treeification
  of a nine-entry chain in a table below 64 slots. `IntegerSuggestion` keeps the original's
  oddities (numeric order against another integer suggestion, `expand` returns a plain
  `Suggestion`, asymmetric `equals`).
- `CommandContextBuilder::find_suggestion_context`, node `listSuggestions` (a literal offers
  itself when its lower-cased text starts with the lower-cased typed remainder, whether or not the
  source may use it, as in the original; `BoolArgumentType` offers `true` and `false`; an
  argument node asks its custom `SuggestionProvider`, else its type) and
  `CommandDispatcher::completion_suggestions(parse, cursor)` / `completion_suggestions_at_end`.
  A provider that fails with a `CommandSyntaxException` contributes nothing. The two unchecked
  exceptions are `SuggestionError::NoNodeBeforeCursor` and `StringIndexOutOfBounds` (a cursor past
  the end of the input).
- `ErasedArgumentType<S>` is now generic over the source so that an argument type can read the
  command context while listing suggestions.
- `java_text`: `Character.toUpperCase/toLowerCase(int)` for every Unicode scalar, `String.hashCode`,
  `String.compareTo`, `String.compareToIgnoreCase` (including the JDK 21 rule that two UTF-16
  strings compare by code point while a Latin-1 operand makes it compare by UTF-16 unit) and
  `toLowerCase(Locale.ROOT)`.

## Not implemented (later WP-E3 stages)

`ContextChain`'s
stage helpers (`getStage`, `getTopContext`, `nextStage`, used by Minecraft's execution
queue), custom `LiteralMessage` / `Dynamic*CommandExceptionType` exceptions, `findAmbiguities`,
Minecraft argument types and their serialization (`ArgumentTypeInfos`), selectors,
`CommandSourceStack`, the command catalog and the migration of the administration commands.
The wrapping of unchecked exceptions thrown by an argument parser into `command.exception`
cannot occur in this port (a panic is a bug here, not an input error).

## Evidence

Stage 1.

`fixtures/commands-brigadier-vectors.json` is produced by **executing** the unchanged
Brigadier classes with the server's exception provider (`tools/brigadier_oracle.java`,
`tools/brigadier_generate.py`; seed 20261102). It holds 17,721 cases (6,361 failures) for
every reader operation and primitive argument type, plus the exact sets of BMP code units for
`isWhitespace`, `isAllowedNumber`, `isAllowedInUnquotedString` and `isQuotedStringStart`.
`cargo test --locked --lib commands::` reproduces every result value, the cursor afterwards,
and for failures the key, typed arguments, English text, `getMessage()`, `getContext()`,
exception cursor and input. The two Java number printers are checked on seeded random bit
patterns, subnormals, powers of ten and exact ties (2,717 doubles, 2,712 floats).

Stage 2. `fixtures/commands-brigadier-dispatch-vectors.json` is produced by executing the
unchanged Brigadier 1.2.9 builders and `CommandDispatcher` (`tools/brigadier_dispatch_oracle.java`,
`tools/brigadier_dispatch_generate.py`, seed 20261103) on 90 seeded random command trees
(literals, typed arguments, requirements, executable nodes, `plain` / `single` / `fork`
redirects with identity, duplicate, empty and throwing modifiers, duplicate registrations that
merge) and 4,048 inputs (valid paths plus truncations, junk, extra spaces). For every input
`cargo test --locked --lib commands::dispatcher_tests` compares the parse cursor, the per-node
failures (node, key, typed arguments, messages, context, cursor), the full context chain (source,
input, ranges, parsed nodes, argument values, fork/modifier/command flags), the order and
content of every executed command and modifier, every result-consumer notification, and the
final result or exception. Stage 3 adds, for every node of every tree (root included) and the
sources 0, 2 and 4, the restricted and unrestricted `getAllUsage` lines and the `getSmartUsage`
map.

Stage 4. `fixtures/commands-brigadier-suggest-vectors.json` (1.2 MB) is produced by the same
oracle class (`tools/brigadier_suggest_generate.py`, seed 20261104) with two kinds of evidence:

- 70 seeded random trees (240 argument nodes carry custom providers: fixed words, the usual
  prefix-filtered `suggest(Iterable)` shape, a context-reading provider and a throwing one) and 2,100
  inputs (valid paths, truncations, trailing spaces, upper- and lower-cased text) completed at
  4,252 cursors, 2,224 of them with suggestions and 88 past the end of the input.
  `cargo test --locked --lib commands::suggestion_tests` compares the result range and every
  suggestion's range, text and integer value, or the exception.
- 1,811 direct cases: `Suggestions.create` (488) and `merge` (264) with random ranges (1,074 adjacent case-variant
  ties in the results), chains of up to twelve colliding hash buckets and sets of up to 55 entries that cross the 12/24/48
  resize thresholds; `SuggestionsBuilder` (350: `suggest`, `suggest(int)`, `createOffset`, `restart`,
  start past the end); 708 string pairs for `compareToIgnoreCase`, `compareTo`, `hashCode` and
  `toLowerCase(Locale.ROOT)`; and the full table of 2,879 code points with a case mapping, which the
  test compares with the Rust derivation for all 1,112,064 Unicode scalars. Every suggestion's
  `hashCode()` and `apply(command)` are compared too. Deliberately breaking the hash spreading, the
  load-factor resize or the chain resize makes the suite fail.

Reproduce (the oracle directory must stay outside git):

```text
python3 tools/bootstrap_oracle.py --oracle-dir /path/outside/git
python3 tools/brigadier_generate.py --oracle-dir /path/outside/git
python3 tools/brigadier_dispatch_generate.py --oracle-dir /path/outside/git
python3 tools/brigadier_suggest_generate.py --oracle-dir /path/outside/git
cargo test --locked --lib commands::
```

The harnesses live in package `cinderstone.oracle` because the server jar is signed and
occupies the default package. No jar, mapping or decompiler output is committed.

## Known limits

- A Java `String` can hold a lone surrogate (for example when `getContext()` cuts a surrogate
  pair); a Rust `String` cannot. 19 generated cases with such output are skipped and counted
  in the fixture (`skipped_cases_with_lone_surrogate_output`); the port converts a cut unit to
  U+FFFD. The dispatcher fixture skips 2 inputs for the same reason.
- `set_cursor` clamps to the input length; Java accepts any int and fails on later access.
  `peek`/`read` return `None` where Java throws `StringIndexOutOfBoundsException`.
- `Float.parseFloat`/`Double.parseDouble` are checked only on the strings the reader can
  produce (`[0-9.-]*`), using Rust's correctly rounded parser. Agreement on the 1,920 float and
  double reads in the fixture is not a proof for other inputs.
- Suggestions are computed synchronously: the original's `CompletableFuture` plumbing (a provider
  returning a failed future would leave the result incomplete) is not modelled, and neither are
  tooltips (`Message`s hash by identity in the original, so tie order with tooltips is not
  reproducible) or `HashSet` buckets treeified at nine collisions in a table of 64 slots or more.
  `IntegerSuggestion`s mixed with text suggestions can make the original's comparator inconsistent;
  the fixture never mixes them in one list.
- `toLowerCase(Locale.ROOT)` uses Rust's Unicode data and its Final_Sigma rule. Java decides a final
  sigma with its legacy word `BreakIterator`, which treats `-` between letters as part of a word
  (`AΣ-É` keeps a medial sigma in Java and gets a final one in Rust). The random alphabet of the
  fixture excludes capital sigma for that reason. Case mappings of characters added after Unicode 15.0
  are handled by the `JAVA_UNMAPPED` table of `java_text.rs`; a newer toolchain that adds more makes
  `case_mapping_matches_every_unicode_scalar` fail with the code points to add.
- Brigadier is MIT-licensed (Copyright (c) Microsoft Corporation); the upstream text is kept in
  `licenses/brigadier-MIT.txt` and referenced from each source file. `THIRD_PARTY_NOTICES.md` is
  coordinator-owned and still needs an entry for it.
