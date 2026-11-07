# Stop command and graceful PLAY shutdown

Standalone #193 under #24/#45, Minecraft Java1.21 / protocol767.

Console and permission4 players can run `/stop` through native Brigadier. Invalid
or inaccessible input never requests halt. Successful feedback is
`commands.stop.stopping`, result1, using current requester/operator feedback rules.
It is queued before an internal Notify wakes the real server loop. No signal is
sent to the process and no command-held lock is re-entered by the final save.

The loop closes the listener, sets stopped admission/gameplay under chat/admission
serialization, completes accepted world callbacks and queues shutdown disconnects
on the same ordered PLAY stream. A packet arriving while stopping cannot discard
that queued feedback/disconnect. Existing PresenceGuard cleanup saves/removes the
actual player/menu owners. The loop waits at most the15-second write deadline
plus1 second for the admitted registry to drain, then calls the existing save
transaction. If a client/cleanup misses that budget, a diagnostic is emitted and
the save still captures retained actors and menus. Save failure exits with an
error. SIGTERM/Ctrl-C use this same path; new commands cannot continue gameplay.

This covers the existing single-level creative profile and admitted PLAY clients.
It does not establish all LOGIN/CONFIGURATION shutdown behavior, complete vanilla
stopServer ordering, save-on/off, RCON or watchdog support. Persistence formats and
Railway volume ownership are unchanged.

## Evidence

The focused harness registers unchanged StopCommand and invokes its real
MinecraftServer.halt(false) on an isolated server object. It checks fifteen valid,
invalid and unauthorized cases, the running flag, result and component bytes,
plus permission0–4 usable trees and selected completions. Rust compares these
original witnesses; no Java binary or disassembly is committed.

```sh
python tools/stop_command_vectors.py --named-jar /outside/server-named.jar --original-jar /outside/server-1.21.jar --libraries /outside/libraries --out fixtures/stop-command-vectors.json
cargo check --locked --all-targets
cargo test --locked --lib commands::stop_command
node tools/stop_command_flow.mjs target/debug/mc121-rust /outside/server-1.21.jar /outside/eula.txt docs/validation/stop-command-runtime.json
```

The real Java/native flow uses two PLAY clients and compares usable stop subtree,
selected completion ranges, exact stop/operator feedback and shutdown disconnect
bytes/order for player and console commands. Denied/invalid commands leave the
server running. A creative-slot write precedes a same-connection time-set response;
after stop and cold restart, slot identity/count are exact and world time must be
within200 ticks of the acknowledged setting (startup/admission scheduling).
A separate native SIGTERM check confirms the shutdown reason and normal exit.
Root completions retain implemented roots, explicitly excluding stopsound;
empty selected completions use range0/0. General syntax-error rendering, wall-clock
timing, full command-catalog and login/configuration phase parity are excluded.
