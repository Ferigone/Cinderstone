# Shared world spawn command (#201)

Console and permission-level2 players can execute `setworldspawn [<pos> [<angle>]]`
through native Brigadier. Existing BlockPos and Angle arguments resolve the
actual source position/rotation, including relative/local coordinates and angle
wrapping. Explicit positions use original spawnable world bounds. Invalid or
unauthorized input changes no spawn state and emits no spawn notification.

One retained Main spawn supplies the actual spawn-position packet (the ordinary
Overworld compass target), later admissions and dynamic console coordinates.
Native checkpoints gain an optional `world_spawn` field; missing fields preserve
the legacy `(8,64,8,0)` default. Imported worlds load and save the existing
SpawnX/Y/Z/Angle fields via the original save transaction. Other level.dat fields
are retained. Saved-player positions take precedence on rejoin. Fresh native
actors use the existing one-block-above admission profile anchored at shared
spawn; full vanilla safe/randomized placement, respawn, other dimensions and
spawn-ticket/generation migration remain out of scope.

The shared save-command feedback adapter now also accepts a typed Component.
This retains the original Integer coordinate and Float angle arguments without
a JSON-number round trip. Save/stop callers keep their existing JSON adapter.
Visibility, permission, gamerule and operator/source feedback policies remain.

`tools/setworldspawn_oracle.java` executes the original registration, arguments,
result and success component against a recording level mutation sink. The
notification is encoded by the original ClientboundSetDefaultSpawnPositionPacket
codec using the recorded position/angle. World ticket/player-list internals are
not executed by this witness. The pinned original JAR is checked by size/SHA-1.
Compile it together with `tools/admin_feedback_oracle.java` using the
[existing external bootstrap/classpath](admin-feedback.md); run
`cinderstone.oracle.setworldspawn_oracle ORIGINAL_JAR fixtures/setworldspawn-vectors.json`.
The focused Rust test compares original tree, suggestions, resolved values,
result and typed feedback. Keep proprietary artifacts outside the checkout.

After `npm ci` and a native build, run `node tools/setworldspawn_flow.mjs
target/debug/mc121-rust REPORT.json`. The flow uses temporary data/loopback
port25704, compares complete selected spawn/private/operator packet bytes,
checks denial, live source coordinates/rotation, fresh/rejoining actors,
explicit-save feedback, native restart and legacy checkpoint defaults. Its
synthetic current-version imported metadata case uses console/status and
checks level.dat spawn/unknown-field preservation through restart; it does not
claim imported gameplay. [Report](../validation/setworldspawn-runtime.json).

Full #24/#41 and aggregate checklist rows remain open. Scope_version1,
denominator100 and counts58/53 are unchanged. Hosted verification is under #19.
