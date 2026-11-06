# Explicit save command (#189)

Console and permission-level4 players can execute `/save-all` and `/save-all flush`
through native Brigadier. Parsing/authorization happen before the save. The
source receives `commands.save.saving`, then success with result1 after the real
save completes; only success uses operator feedback. Hidden chat recipients receive no system feedback. An I/O failure logs its
actual detail and sends `commands.save.failed`, never success.

Both console and authorized player paths retain the actual chat/admission
serialization. `Server::save` keeps its normal autosave/shutdown entry point; a
shared inner transaction accepts the already-held chat guard for explicit saves.
It saves actual world/player/entity/SavedData/level owners without closing live
menus. Both variants wait for the current synchronous checkpoint writers.
This does not claim Java's asynchronous IO scheduling or power-loss durability.
Storage formats, save-off/on, autosave cadence and shutdown order are unchanged.

`tools/save_all_command_oracle.java` executes original SaveAllCommand registration,
ordered feedback, result and failure, replacing only the saveEverything host.
The focused fixture also records the original `(true, flush, true)` arguments.
Regenerate with `tools/save_all_command_vectors.py` using the existing pinned
bootstrap, named original and external libraries; keep proprietary files outside
the checkout.

Run `node tools/save_all_command_flow.mjs BINARY PINNED_JAR EXISTING_EULA REPORT`
after `npm ci`. It requires existing EULA acceptance and ports25641/25642, uses
isolated temporary worlds and does not touch Railway/data. It compares original
and native feedback/operator bytes, command tree and delivered-root completions,
checks both variants and console, and verifies saved clock/inventory files as
soon as success arrives. It then uses SIGKILL, restarts and checks actual world
time and player inventory over protocol. Native I/O failure is also exercised.
[Runtime evidence](../validation/save-all-runtime.json) records binary/source
identity. The native scene uses the supported flat creative profile; general
imported gameplay and full #24/#45 remain outside this delivery. Both checklist
rows stay partial, scope1/100 and58/53 unchanged. Hosted verification is under #19.
