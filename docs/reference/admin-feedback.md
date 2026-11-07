# Current command-source identity (#195)

The shared Main feedback path for `time`, `weather`, `difficulty` and
`worldborder` now reads the current scoreboard when constructing operator
feedback. Team prefix, suffix and color survive alongside the actual profile's
hover, click and insertion identity. Console uses `Server`. Operator packets
use the canonical Component codec, as the existing idle/save paths do.

`tools/admin_feedback_oracle.java` executes original 1.21
`Player.getDisplayName` and `CommandSourceStack.sendSuccess/broadcastToAdmins`
with recording connections, an actual team and actual operator membership.
It also executes the original `WorldBorderCommand` for its success component;
the other three success components exercise the shared original feedback
method directly. The pinned original JAR is checked by size and SHA-1.
External world, connection and profile lookups are supplied by the harness.
This is an original-class witness, not a full Java server protocol run.

After the existing `tools/loot_oracle_setup.py --oracle-dir OUTSIDE_CHECKOUT`
bootstrap/remap, compile this harness using the named JAR and bundled library
classpath in `cp.txt`, then run `cinderstone.oracle.admin_feedback_oracle
ORIGINAL_JAR fixtures/admin-feedback-vectors.json`. Keep all JARs, classes and
temporary saves outside the checkout.

After `npm ci` and building the native binary, run:

```sh
node tools/admin_feedback_flow.mjs target/debug/mc121-rust REPORT.json
```

The flow uses an isolated temporary native flat world and loopback port25702.
Two actual operators execute the four commands with a red team decoration,
change to blue with a different prefix/suffix while still online, leave the
team and exercise console execution. All16 selected operator packet payloads
and12 private player payloads equal the original witnesses byte for byte;
console output contains the same success component. Border-center packets and
difficulty updates prove real world mutations. The report is
[admin-feedback-runtime.json](../validation/admin-feedback-runtime.json).
Compiler and relevant command/server tests pass. Full #24, additional commands,
selectors/signing expansion and general error equivalence remain outside scope.
Fixed scope1/100 and58code/53Main counts are unchanged. Hosted checks are under #19.
