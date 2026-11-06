# Datapack functions — `CommandFunction.fromLines` and macros

Target: Minecraft Java **1.21**. `src/functions/` ports WP-E7 (#28) stage 1:
- reading a `.mcfunction` file;
- instantiating macro functions.

It claims no checklist point, and no Main callsite uses it.

## What is implemented

`from_lines(lines, parse)` is `CommandFunction.fromLines`, with `parse` standing in for the
dispatcher's `parseCommand`. It reads a file as follows:

- **Trimming:** each line is trimmed as `String.trim` does, removing characters up to U+0020.
- **Continuation:** a trailing `\` joins the next trimmed line. A continuation on the last line is
  an error.
- **Length limit:** a joined line may hold at most 2,000,000 UTF-16 units.
- **Comments and blanks:** empty lines and `#` comments are skipped.
- **Slash errors:**
  - `//` gives the "use '#' not '//'" error;
  - a leading `/` gives the "Do not use a preceding forwards slash" error;
  - a line that is only `/` is the original's `StringIndexOutOfBoundsException`.
- **Macro lines:** `$` lines become `StringTemplate`s (`$(name)` references, where names are
  letters, digits and `_`), and they raise the original's errors:
  - an unterminated reference;
  - an invalid name;
  - a macro line without variables.
- **Becoming a macro function:** the first macro line turns the function into a macro function.
  Earlier commands become plain entries, and parameters are numbered in first-use order.

`Function::instantiate(arguments, parse)` is `MacroFunction.instantiate`:

- **Errors:** missing arguments and a missing argument are reported as the original does.
- **Argument strings (`stringify`):**
  - Floats and doubles use `DecimalFormat("#")` with up to 15 fraction digits, `HALF_EVEN`. There
    are no minimum integer digits, so `0.5` is `.5` while zero is `0`.
  - Integral tags use `String.valueOf`.
  - Strings are passed through raw.
  - Other tags use their SNBT text.
- **Substitution and parsing:** each template is substituted, then parsed with `parse`.

## Evidence

`tools/loot_oracle.java functions` runs the unchanged `CommandFunction.fromLines` and
`MacroFunction.instantiate` over the **real 1.21 command dispatcher**. The dispatcher is a
recording copy, which notes every command string it parsed and whether `parseCommand` accepts it.
The test then hands the port the dispatcher's verdicts.

`tools/functions_generate.py` builds 416 files: hand-written ones plus 400 seeded mixes of
commands, macro lines, comments, continuations and slashes. Each macro function is instantiated
with 15 argument compounds, covering floats, doubles, exponents, negative zero, longs, lists and
compounds.

The test `functions::tests::functions_match_the_original` checks that the following agree, for
all 416 files: which files are accepted, the error kind and line number, the parsed commands, the
macro parameters, segments, variables and indices, and each instantiation's commands or error
(`fixtures/functions-vectors.json`).

## Not covered

- **Command parsing.** The port does not parse commands itself; that belongs to the command
  system (#24). The caller supplies the parser.
- **The macro instantiation cache.** It is not reproduced, because it changes no result.
- **Decimal digits.** Digit generation uses the shortest round-trip form, whereas JDK 21's
  `DecimalFormat` takes `FloatingDecimal`'s digits, which can be longer for rare doubles. The
  seeded values agree.
- **Variable names.** They are checked with Rust's `is_alphanumeric`, which approximates
  `Character.isLetterOrDigit`.
- **Later stages.** Execution, `/function`, the load/tick tags, `/schedule` and `/reload`
  (stages 2–5) are not implemented.
