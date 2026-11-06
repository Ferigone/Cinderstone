# World border command runtime

Standalone #186 under #24, Java 1.21 / protocol 767.

Permission-2 players and console can use `worldborder get`, `center`, `set/add`
with optional seconds, `warning time/distance`, and `damage amount/buffer`.
The native Brigadier path parses before mutation. Main changes the retained
border already read by collision, placement and subsequent login, and queues
size/lerp/center/warning packets before requester feedback. Queries stay private;
changes use the existing operator feedback policy. Damage settings are retained;
this does not implement border damage gameplay.

Center uses the caller position (console: the immutable shared spawn), integer
centering and float narrowing from Vec2Argument. Add-with-time extends the current
remaining transition; add without time is immediate. State uses the existing
monotonic millisecond clock. Imported level.dat saves and native checkpoints retain
settings and remaining interpolation; an older native checkpoint without `border`
keeps the default. Existing imported metadata owners are preserved.

## Evidence

The pinned unchanged original is the reference. The focused harness registers
WorldBorderCommand, injects Util.timeSource, executes 27 sequential cases and
records original component and packet codecs. Three permission-filtered trees
and their completions are included. Rust compares feedback/errors, state,
results, trees, completions and packet payloads at controlled times. Only this
harness's glue is committed, never Java binaries or disassembly.

```sh
python tools/worldborder_command_vectors.py --named-jar /outside/server-named.jar --original-jar /outside/server-1.21.jar --libraries /outside/libraries --out fixtures/worldborder-command-vectors.json
cargo check --locked --all-targets
cargo test --locked --lib commands::worldborder_command
node tools/worldborder_command_flow.mjs target/debug/mc121-rust /outside/server-1.21.jar /outside/eula.txt docs/validation/worldborder-command-runtime.json
```

The real Java/native flow compares the selected command subtree, completions,
requester feedback bytes, border packet bytes, observer updates and static
rejoin/restart state. It checks non-op denial, invalid/no-change commands, private
queries, console-relative center, imported interpolation restart, native
interpolation checkpoint restart and missing-field compatibility. The live
2-second transition is queried after 2.5 seconds (500ms scheduling allowance).
Long-transition restart checks target and positive remaining duration bounded by
the saved duration; elapsed wall time is deliberately not compared byte-for-byte.
Controlled-clock vectors cover precise interpolation separately. General syntax
error rendering, border damage, dimensions and full command parity are excluded.

Local all-target checking and the focused fixture test pass; the full library
test harness exceeds this worker's memory limit. Hosted CI is the full test gate.
The fixed scope1/100 checklist remains58 code/53 Main. Hosted deployment evidence
is tracked separately in #19.
