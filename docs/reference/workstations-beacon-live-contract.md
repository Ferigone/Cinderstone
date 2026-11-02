# Beacon ordered/live owner contract

This additive contract reconciles the actual [Team4 effect-owner warning](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6566707913)
with [Team3's live-boundary assignment](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6566833766).
The source base is `884a9ceb52bf3ed2344809ed67831893ef4bd556`.
It adds callable owner interfaces and an ordered tick path; it does not supply a
native beacon record, Main arena, concrete living-effect host binding or beacon
gameplay witness.

## Why the boundary changed

Team4's actual `effects::actor_ops::add_effect` and retained-instance callbacks
operate on canonical actors/maps/instances. Mutation, attribute updates, packets,
criteria and reentrant operations retain their completed prefix when a callback
fails. These calls cannot be placed inside a detached beacon image and made
atomic by cloning actors or discarding the image afterward. Missing ownership
also cannot be treated as an empty player list or a callback returning success.

The previous `tick`/`*_staged` APIs remain only for an explicitly proven
restricted staged domain. Its proof must cover actual identities, reentry,
failure prefixes, RNG and outbox behavior. The old controlled fixtures prove no
general living-effect domain. See the [earlier runtime ledger](workstations-beacon-runtime.md)
for unchanged source algorithms, data fields and historical staged validation.

## Exact live API

| API | Required ownership |
| --- | --- |
| `BeaconLiveOwner::require_actual_live(BeaconAction::Tick)` | Admit the actual retained BE and original source-call Level/BlockPos leases before any observation or mutation. No caller Boolean or current-index approximation. |
| `BeaconLiveTickHost` | Extends live-owner and effect-owner contracts, with the six actual world-query methods. It does not require `BeaconTransactionHost`. |
| `tick_live(&mut host)` | Executes the shared private source tick algorithm directly on those live owners; no begin/publish stage. |
| `apply_effects_live(&mut host, levels, primary, secondary)` | Checks the live capability first, then uses the same private effect core as the restricted staged wrapper. |
| `activate_live(&mut host)` | Checks the live capability first, then uses the same private activation core as the restricted staged wrapper. |

`BeaconFields` remains shared. A live invocation does not call its staged
certificate method or obtain permission to run staged data/menu operations.
The capability is a requirement on the embedding owner, not a concrete arena or
lease implementation supplied by this library.
The world-query methods are `world_surface_height`, `block_state`, `beam_color`,
`light_block`, `is_bedrock` and `is_beacon_base`; they use the actual captured
Level, canonical block/tag identities and contextual opacity.

The host retains the **same canonical BE object** throughout the invocation,
including after source removal, replacement or reentry. Queries/sounds continue
using the original Level and BlockPos argument leases; they must not follow a
reattached `BE.level` or a fresh position lookup. Captured player-query results
retain the original actor objects across callbacks, including removal/replacement
and reused wire IDs. Holder references retain canonical definition identity.
No clone of mutable BE/actor/effect state implements this contract. Completed
own/reentrant mutations are not a stale-version failure that redirects or rejects
the still-retained original object.

World reads remain an explicit restricted query domain: actual readonly,
nonreentrant world/tag/opacity/height queries, valid world coordinates and the
source short-circuit order. The current beam-section value representation does
not certify arbitrary reentrant world-query handlers or observable Java list/
section identity. Actual effect, sound and criterion callbacks may reenter;
no BE field borrow is held across those callback boundaries.

## Source order and committed errors

The pinned Java1.21 `dqc` order is unchanged:

1. Incremental scan, then capture old raw levels.
2. The80-tick phase uses the old committed beam, assigns computed levels, applies
   captured power arguments to all primary recipients then eligible secondaries,
   and calls ambient sound.
3. A completed checking beam is published; the old/new levels transition decides
   activation/deactivation. Activation sounds first, queries its separate actual
   ServerPlayer list, then rereads retained-BE levels for each criterion.

The effect box, activation box, ARGB arithmetic, stale-level behavior and repeated
holder comparisons are shared with the previous source core. The captured player
list is reused; actors are not resolved again by ID. Each application must create
a fresh actual effect-instance lease and call Team4's genuine add-effect owner,
null cause, ignoring only the source Boolean result. Canonical map merging,
attributes, callbacks, per-recipient payloads and criteria still execute normally.

An `Err` stops at that boundary and keeps everything already executed:

- Completed scan/reset/height/base-field mutations remain on the retained BE.
- Earlier player applications and any partial mutation inside the failing actual
  actor callback remain. Later recipients/phases are not run as compensation.
- Completed sound draws/output remain. Each beacon sound wrapper uses the actual
  Level.threadSafeRandom once, including an empty audience; it is not rewound or
  replaced by a player/local stream. Main already has that sound-stream owner.
- A criterion failure retains the published beam, preceding activation sound,
  earlier awards and any reentrant field changes already performed.

There is no detached copy, `begin_stage`, `publish_stage`, rollback, automatic
retry or callback replay in this live path. The caller reports/handles failure
without restarting the same invocation. Future ticker policy must respect its
completed prefix. Retention and callback order are not a transaction that can be
reconstructed from receipts or replayed onto a replacement actor/BE.

## Separate data/menu proof and missing Main binding

This change does not alter beacon payment, ContainerData setters/load/save or the
menu controller. Their staged admission remains separate. A live tick capability
is not a proof that menu/data callbacks, sound RNG or outbox effects can be
rolled back, nor does an old stage certificate admit live actor APIs.

The actual native BE/arena must still provide retained lifetime/access, original
Level/position context, ordered spatial queries and real effect/definition/
criterion/sound owners. Main must bind these to its ticker, profile admission,
persistence, removal and output phases. No cloned receiver, guessed registry,
empty criterion or new sound RNG is provided. Existing effect-operation source
and sound APIs do not by themselves create this Main adapter.

Primary evidence remains direct pinned-JAR `javap` and official mappings,
including `dqc` tick130–213, applyEffects250–272 and playSound275–276, plus the
Team4 retained actor contracts. JAR SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
mapping SHA1 `31c77994d96f05ba25a870ada70f47f315330437`.
No REA or Java gameplay execution is claimed.

## Frozen source and focused validation

Root's isolated all-target compiler check passed in1m25s. The
`workstations::beacon_` library filter passed seven focused checks, with format
and diff checks also passing. Logs are `/tmp/ws-beacon-live-check.log` and
`/tmp/ws-beacon-live-native.log`; this ledger's author ran no build or server.

The added live-only case is a **controlled contract fixture**: a non-Clone host
retains its boxed fixture BE and original Level/position leases while a callback
removes/replaces the current index, reattaches the old object and then injects an
error. It checks the retained object's committed prefix and untouched replacement.
Its absent primary and deliberately unavailable actor callbacks provide no
successful real actor/effect, sound-RNG, criterion or Main witness. The earlier
staged/geometry fixtures likewise remain controlled tests, not a production
staging proof. No native-world or aggregate workstation completion is implied.

| Source | Final SHA256 |
| --- | --- |
| `src/workstations/beacon_block_entity.rs` | `8999330558b41180a66b1fa702cfbe0381da2da61ec78b40b00f1103dbadcc71` |
| `src/workstations/beacon_effects.rs` | `fdc88ac94591ae1c09b19cdaaef795196cd427b062c4979b12aa672eaa67b5e2` |
