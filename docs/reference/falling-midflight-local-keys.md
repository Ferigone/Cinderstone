# Native Falling mid-flight continuation with local service keys

The genuine airborne save and hidden restart were observed, and the separately
authorized remaining PLAY continuation passed. The first local-file harness
report remains **`ok: false`**: it incorrectly required internal handle
generations to remain equal across processes. It was not edited into a PASS.
The [published continuation record](../validation/falling-midflight-local-keys.json)
cites that failure, its successful prefix and the exact genuine input pairs.

This is a single-actor native checkpoint witness, separate from the earlier
one-block falling plus Anvil-menu proof on source `a8cc5a7`. It does not complete
a checklist delivery or establish Java-save or full FallingBlock compatibility.

## Exact source and records

| Identity | Value |
| --- | --- |
| Minecraft / protocol | Java 1.21 / 767 |
| Isolated local-mode source | `a2b020f934cfbe00abca260f1a33593c31873518` |
| Source tree | `64f645097fb0e287e33e65cb6e3cabe4c7ae227e` |
| Actual executable SHA-256 | `39adbead1c345ee5b9870a7adbef5c1cc58c47f60bdda9b1e4421edb66796395` |
| Local-file prefix harness SHA-256 | `cf74fb22ac5ebb4c4beb9839ec1dee408d20109b1f6f611d44b1bd961e80846a` |
| Preserved failed prefix report SHA-256 | `c3e2babf162e3234390cf919ba9593e3b2bf9ebb9905bd519c1a6fdefe5b5214` |
| Continuation harness SHA-256 | `902a52e5747ff5245823e657161cd845c5df2acad894018c1326ea23b1d26384` |
| Explicit local-mode adapter SHA-256 | `15478b75055d041a6091c969ac77181f91a3a25db04735fb0cc3f30420548ae7` |
| Published continuation report SHA-256 | `721dd2b755adc286db7b7a4bff52741230707c53e779a7cdf8275c1a6ae6d882` |

The all-target compiler check passed in 21.87 seconds and the full executable
build in 43.95 seconds. Actual capture/hidden phases ran from
`2026-11-02T02:28:18.839263+00:00` to `02:28:50.955520+00:00`.
The authorized continuation ran from `02:34:09.601879+00:00` to
`02:34:31.757457+00:00`. All three isolated servers completed normal SIGINT
shutdown with exit 0. No scene was recaptured for the continuation, and no saved
state, integrity value, UUID, RNG, block history or ordering record was edited.

## Local source removes the external request

Every server explicitly used `--allow-offline --bind 127.0.0.1` and
`--loopback-service-keys` with the same actual local JSON file. Its bytes were:

```json
{"playerCertificateKeys":[],"profilePropertyKeys":[]}
```

The file includes a final newline and has SHA-256
`ba261e749d8c501b61f029f0a44e9f8d0a04ab3a7860949a9c8ba78225767962`.
The real `ServiceKeySet::from_keyset_json` parsed both empty collections. This
provides zero trust roots, does not make any signature valid and is not evidence
of signed-certificate or secure-chat validation.

`main_chat_keys.rs::prepare_key_source` resolves the file before listener and
authentication-owner startup. Its actual-read limit is 1 MiB. Local mode is
accepted only offline with a loopback IP, and the bound listener address is
checked again. `ServiceKeySource::LoopbackFile` remains fixed for the server
lifetime: `read` returns the parsed initial value or reads that file again.
Only the separate `Official` match arm calls `fetch_official`; neither initial
nor later local errors switch to it. Offline Main constructs no Authenticator
or SessionVerifier.

Each actual process emitted, before listening:

```json
{"event":"chat_service_key_source","source":"loopback-file","network_refresh":false}
```

The real refresh owner also emitted a completed local attempt with
`profile_keys: 0` and `next_refresh_seconds: 86400`. The harness checks the actual
source fields rather than treating generic refresh-event names as HTTP. No
official-source event appeared. The unreachable HTTP branch is a source finding;
the runtime evidence is actual local-source selection, not a packet-capture
audit of every network interface. Default Official mode is a separate source
choice and was not used here.

The previous old-mode approval interruption and invalid-name setup attempt
remain separate incomplete records. This local-file mode removes the rejected
side effect; it does not proxy or reroute that request or reuse the old mode.

## Genuine capture and hidden phase

Normal protocol actions built a stone pillar and east support, then placed the
actual Anvil at `(11,68,8)`. State 9109 remained stable through actual game-time
publications 19 and 39. Genuine support removal created type 40/entity 2 at
`(11.5,68,8.5)`. Normal SIGINT was requested after its real AddEntity and before
any observed removal. The resulting actual native cohort held one airborne
FallingBlock, not an invented settled scene:

| Saved field | Actual observation |
| --- | --- |
| Actor / state / start position | ID 2 / state 9109 / `(11,68,8)` |
| Position | `(11.5,66.92369774182143,8.5)`, `on_ground: false` |
| Velocity | `(0,-0.25847395483642877,0)` |
| Entity tick / Falling time | 7 / 7 |
| Fall distance | 1.0763023 |
| Shared next entity ID | 3 |
| Full saved snapshot SHA-256 | `c092da4e9b6975cf65b64410d67761100dc0f473ec3ac461a2812381c13fcc7d` |

Snapshot hashes use sorted compact JSON of the actual saved values; whole-file
digests for the genuine world/checkpoint pairs are retained in the report.

The entire snapshot, including UUID, physical fields, retained 48-bit RNG, time
and start position, remained value-identical during the next genuine restart
with no PLAY player. Actual world game time advanced from 48 to 111. The hidden
tick list was empty and the shared next ID stayed 3. This observes the hidden
body remaining unticked while the server runs, rather than inferring hidden
state solely from an absence of network packets.

The same persistent actor ID, kind, section and relative registration/section
order survived. Original generation 2 became actual fresh generation 1 after
cold registration. `RuntimeEntityIndex::register` assigns fresh generations;
`restore_persistent_entity_order` validates saved references and rebinds by ID
to those actual handles. Every reference within each resulting index/section
was consistent. Equality of the old and fresh generation numbers would violate
that source contract. The preserved failed harness demanded precisely that
wrong equality and therefore stopped before its PLAY phase.

## Separate remaining PLAY continuation

The continuation first verified exact hashes of both earlier genuine pairs and
the preserved failed report. It compared complete bodies and the actual
logical order while validating each process's internal handles. Only then did
it start the still-unrun PLAY phase from the existing hidden pair. Private
copies preserved the original input bytes; normal server output was written by
the genuine runtime, not by a checkpoint editor.

| Actual resumed observation | Result |
| --- | --- |
| New chunk `(0,0)` publication | Wire ordinal 21 |
| Restored AddEntity | Ordinal 31, same ID 2/type 40, exact saved XYZ and carried state 9109 |
| UUID and velocity | Actual wire UUID equals the saved UUID; raw motion `(0,-2067,0)` matches quantization of the saved velocity |
| Start-position metadata | Ordinal 32, index 8/serializer 10, `(11,68,8)` |
| Landing | Ordinal 76, state 9109 at `(11,64,8)` |
| Removal and event | Same entity removed at ordinal 78; actual Anvil event 1031 |
| Final checkpoint | One landed Anvil, no Falling/Item/XP owners; next entity ID 4 |

The only new shared ID was the actual new PLAY player, advancing 3 to 4. No new
Falling constructor or duplicate block/item was introduced. The four-block fall
allowed source-valid states 9109 or 9113 with the same facing, rather than
guessing the wear RNG sample. Actual landing state was 9109. Stable time
publications followed landing. The retained RNG was compared exactly while
hidden; this does not independently reconstruct the later complete wear or
constructor RNG sequence.

## Limits

The checkpoint is the richer native Item/XP/Falling format, not Java entity NBT
or Java chunk-save import. New sender handles, network baselines and chunk
readiness are intentional native-resume owners. One actor cannot prove mixed
cohort ordering, cross-section encounter behavior or concurrent removals.
Damage victims, positive wear/breakage, failed landing/drop paths, longer
timeouts, fluids, block data, other falling families, multiple observers,
ordinary autosave timing, public/graphical clients and performance were not
exercised. No full checklist row, completion percentage or scope definition is
changed by this record.
