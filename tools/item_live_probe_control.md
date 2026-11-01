# Live item probe

The probe uses unmodified protocol-767 clients. Select the intended scope:

```sh
node tools/item_live_probe.mjs --server-binary target/debug/mc121-rust --scenario stock
node tools/item_live_probe.mjs --host 127.0.0.1 --port 25571 --label java --scenario stock --wait-common-position
node tools/item_live_probe.mjs --scenario all --control-program /path/to/test-adapter
```

`stock` checks the real selected-hotbar DROP_ITEM and DROP_ALL_ITEMS actions,
both clients' AddEntity/Slot8/velocity pairing order, gravity and ground motion,
observer departure/return, collect-before-remove and private inventory updates.
The players move apart before the forty-tick pickup delay. Their drops are four
blocks apart with explicit yaw/pitch zero, so this scenario does not depend on
two random trajectories merging. The observer later collects both separately.
The raw protocol probe applies the vanilla client's local creative/Q inventory
prediction, because the original server deliberately suppresses some selected
slot echoes. That display is never accepted as server evidence. After creative
assignment and each Q action, the probe sends a harmless stale `window_click`
for slot `-1` and requires a fresh authoritative `window_items` response before
asserting the server's selected count. Pickup keeps its private synchronization
check and also verifies a fresh full readback.
Each connection waits for its actual initial forty-five-chunk requested view
before dropping, so unreceived/pending chunks cannot hide the gravity phase.

`merge` requires a controller before starting a server. `all` runs stock checks
first, then fails if merge control is unavailable. Missing packets, unsupported
actions, controller errors and deadlines are failures, with a `WAIT` phase and
the failed phase saved in `evidence/item-live-<label>.json`. No scenario silently
omits a required check. The default is `all`.

Rust may be launched by `--server-binary`. Java may be external, as above, or
launched with `--java-jar <official-jar> --java-server-dir <prepared-run-dir>`.
The Java launcher verifies the pinned official outer JAR SHA1; an external
Java-server harness must independently verify that server's provenance.
The Java directory must already have its EULA accepted and offline creative
flat-world settings. The tool does not create or change that acceptance.
Use a fresh Java test-world copy as in `tools/creative_compare.py`. With
`--wait-common-position`, the external Java harness teleports ItemObserver and
ItemDropper to `8.5 65 8.5` after joining, before the probe proceeds. Rust and
Java must use the same dry flat geometry. Random trajectory bytes are not
compared; live network success is separate from the original-method fixtures.

## Controller contract

The controller is a **separate test process**, spawned with `shell: false`.
`--control-args` supplies a JSON array of arguments. There is no new Minecraft
network cheat packet or production administrative endpoint. The adapter is not
implemented by this probe; each target's test harness must provide it.

One JSON request and one matching JSON reply occupy separate lines. Diagnostics
go to stderr. Unknown operations must return `ok: false`; an acknowledgement
means the real target completed the mutation synchronously. Adapter responses
and requests are retained in the report. Reply within ten seconds.

```json
{"id":1,"protocol":1,"op":"hello","platform":"rust","server":{"host":"127.0.0.1","port":25573},"required":["prepare_merge","allow_pickup"]}
{"id":1,"ok":true,"result":{"capabilities":["prepare_merge","allow_pickup"]}}
```

`prepare_merge` identifies the two already spawned entities by both entity ID
and UUID. Verify their current counts are one and seven; fail on an identity or
count mismatch. Apply the supplied equal position, zero velocity, age zero and
pickup delay 200. Do not alter either stack count, metadata dirty state, sender
cache or tracked baselines. Position changes must invoke the actual target's
normal set-position/index callback. The target must continue its normal level
and tracking phases so original merge eligibility, stack copying, source
removal and next-tracking metadata publication remain under test.

```json
{"id":2,"protocol":1,"op":"prepare_merge","platform":"java","server":{"host":"127.0.0.1","port":25571},"entities":[{"entityId":17,"uuid":"...","count":1},{"entityId":18,"uuid":"...","count":7}],"position":{"x":12.5,"y":66,"z":11.5},"velocity":{"x":0,"y":0,"z":0},"pickupDelay":200,"age":0}
{"id":2,"ok":true,"result":{"applied":true}}
```

For Java, an in-process test adapter must write the actual age/pickup-delay and
velocity fields and invoke the original normal position setter. Generic
console/RCON `data merge entity` is unsuitable: EntityDataAccessor.setData
reloads ItemEntity's NBT, calls setItem with a newly decoded stack and dirties
Slot8, violating this controller contract. An attach/instrumentation harness
must preserve the original Minecraft methods and report its mutations.
For Rust, the harness must use the real authoritative item store and normal
position/index callback. A fixed world seed alone is insufficient because it
does not pin all entity/player RNG state.

The one-count entity must disappear, the seven-count entity must survive with
Slot8 count eight, and both clients must receive source removal before the
survivor's later metadata update. This checks observable packet order. Packet
order alone does not prove the separate tracking/simulation phase boundary:
an immediate metadata flush after removal would satisfy the same assertion.
That boundary requires the core oracle or explicit server phase instrumentation.
The probe does not synthesize a metadata update.

`allow_pickup` sets only the surviving entity's pickup delay to zero after
identity validation. The observer then approaches the actual last sent item
position and must receive a collect count of eight, removal and inventory gain.

```json
{"id":3,"protocol":1,"op":"allow_pickup","platform":"rust","server":{"host":"127.0.0.1","port":25573},"entities":[{"entityId":18,"uuid":"..."}],"pickupDelay":0}
{"id":3,"ok":true,"result":{"applied":true}}
```

## Original action values

The pinned original `ahm$a` (`ServerboundPlayerActionPacket.Action`) static
initializer creates `DROP_ALL_ITEMS` with ordinal 3 and `DROP_ITEM` with ordinal
4. These are the actual enum constructor operands, not a guessed client name.
The source is the unchanged inner 1.21 JAR with SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.

An offline negative check needs no running server and must exit with status 1:

```sh
node tools/item_live_probe.mjs --scenario merge --label selftest-no-controller
```

Successful syntax/help/negative checks do not establish successful live item
behavior. The main runtime and target-specific controllers must be integrated
before a live conformance result can be claimed.
