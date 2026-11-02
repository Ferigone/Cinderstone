# Brigadier reader, exceptions, primitive arguments and dispatcher core (WP-E3 stages 1-2)

Scope: `commands_data.1` ("Komendy i uprawnienia operatorów") is **partial**. Stages 1-2
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
  `DoubleArgumentType` and `StringArgumentType`.
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

## Not implemented (later WP-E3 stages)

Usage strings (`getAllUsage`, `getSmartUsage`), completion suggestions, `ContextChain`'s
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
final result or exception.

Reproduce (the oracle directory must stay outside git):

```text
python3 tools/bootstrap_oracle.py --oracle-dir /path/outside/git
python3 tools/brigadier_generate.py --oracle-dir /path/outside/git
python3 tools/brigadier_dispatch_generate.py --oracle-dir /path/outside/git
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
- Brigadier is MIT-licensed (Copyright (c) Microsoft Corporation); the upstream text is kept in
  `licenses/brigadier-MIT.txt` and referenced from each source file. `THIRD_PARTY_NOTICES.md` is
  coordinator-owned and still needs an entry for it.
