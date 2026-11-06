# Difficulty command runtime

Minecraft Java 1.21 / protocol 767, standalone #180 under #24.

`commands::difficulty_command` uses the native Brigadier dispatcher for the root
query and peaceful/easy/normal/hard literals at permission 2. It rejects invalid
or trailing arguments before mutation. The query returns the current id; a set
returns 0. Setting the current value fails with commands.difficulty.failure.

Main updates the actual difficulty used by player body/food and scheduled fire.
It publishes ClientboundChangeDifficulty to all players before feedback and uses
that current state on later admission. The saved lock is preserved: the command
forces the change despite it, matching MinecraftServer.setDifficulty(..., true).
Saved hardcore metadata forces effective Hard; feedback still names the requested
value as the original does. This does not establish hardcore gameplay support.

Normal checkpointing writes Difficulty to imported level.dat without changing
other metadata owners. Native fire checkpoints retain the current fire difficulty.
Cold startup deliberately reapplies the configured difficulty (native CLI/default
owner), matching DedicatedServer.forceDifficulty; /difficulty never changes
server.properties. Missing native property support remains outside this command.

## Evidence

The unchanged pinned server (fixtures/source-lock.json) is the primary reference.
Direct `java -m jdk.jdeps/com.sun.tools.javap.Main` inspection covered
DifficultyCommand.register/setDifficulty, MinecraftServer.setDifficulty,
DedicatedServer.forceDifficulty and CommandSourceStack.sendFailure. No Java
binaries or disassembly are committed.

The small executed-original harness records usable trees at permissions 0–4,
parse/completion cases and 31 query/set/error executions, including the hardcore
force rule. Success component bytes use the original context-free network codec.
The Rust fixture tests compare results, effective state, components and trees.
The public execute cases were also run against the built library locally.

Reproduce with a named/remapped original and libraries outside the checkout:

```sh
python tools/difficulty_command_vectors.py --named-jar /outside/server-named.jar --original-jar /outside/server-1.21.jar --libraries /outside/libraries --out fixtures/difficulty-command-vectors.json
cargo test --locked --lib commands::difficulty_command
node tools/difficulty_command_flow.mjs target/debug/mc121-rust /outside/server-1.21.jar /outside/eula.txt docs/validation/difficulty-command-runtime.json
```

The protocol flow uses an original-generated imported scene with a controlled
saved lock, a real observer, console/operator/non-op commands, exact selected
feedback bytes and completion (root-prefix comparisons retain the implemented deop/difficulty roots),
reconnect and cold restart. A separate fresh native
flow checks changed fire state in the checkpoint and configured difficulty after
restart. Timing, grammar-error rendering, mob spawning/damage, new dimensions and
full command-catalog parity are excluded. Local all-target test metadata exceeded
the worker memory limit; exact-head hosted CI supplies the complete build/test gate.
