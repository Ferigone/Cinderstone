# Main Loom source-phased callbacks — Java 1.21

`src/main_loom_callbacks.rs` supplies the source-position callback preflight and
packet helpers for the actual native `LoomMenu`. It owns no inventory, world,
random source, statistic, recipe book, XP or advancement registry. The primary
transaction retains the actual canonical owners and publishes each event in
order. Source phase snapshots are observations, not additional owned items.

The reference is the unchanged original Java1.21/protocol767 inner server,
SHA-256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`, with
official mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
Evidence is direct JDK21 disassembly and bounded original-bootstrap observation,
not an REA MCP invocation. Detailed source methods and observed sound/listener
facts are in [loom-sound-source.md](loom-sound-source.md),
[workstations-loom-menu.md](workstations-loom-menu.md) and
[loom-patterns-production.md](loom-patterns-production.md). No raw disassembly
or original Java binary is added by this stage.

## Actual callback capability

`PreparedLoomCallbacks::prepare(gameplay, patterns, events, context)` requires
the actual `MainItemGameplay::require_supported_floor` owner and the real
`NativeLoomContext::require_default_callbacks` capability. That context has
private construction and carries the native factory's verified original-server
default profile; the helper does not accept an arbitrary listener boolean.
Its pattern binding must equal the actual configured resolver passed to
preparation. Another fixture/tag/feature snapshot does not inherit this proof.

The original constructor Runnable targets `cra.r()`, whose body is a sole
return. Java **does** expose `registerUpdateListener(Runnable)` at source
lines225–226. Only the retained constructor-default/native sealed context is
supported, not a custom replacement. Likewise, registered/reentrant container
listeners cannot be silently represented by an empty callback.

| Source event | Required actual handling |
| --- | --- |
| `InputContainerChanged` | Visit the genuine constructor-profile superclass listener boundary. The kernel has already calculated the input `slotsChanged` state transitions and emitted their phased events. Do not run a second recomputation. |
| `OutputContainerChanged` | Visit its own superclass listener boundary; do not treat output as an input recomputation or a player inventory. |
| `UpdateListener` | Complete only the proved constructor-default `cra.r()` return at this source index. |
| `BroadcastChanges` | Visit the kernel-completed broadcast boundary. Its resulting ordered `Sync` events are already present; do not emit another broadcast. |
| `PlayerInventoryChanged` | The primary uses actual canonical `PlayerInventory::set_changed` through its existing menu commit capability at this index. The helper creates no side counter. |
| `Sync::SlotListener` with player backing | Execute the real gameplay inventory criterion owner with this event's `phase.player_inventory`, not a later/final inventory. |
| `TakeSound` | Use the real captured-level time, actual independent sound RNG and actual same-dimension audience at this index. |
| `Drop` | The actual indexed item owner consumes that one owned drop with its original RNG/trace flags; a phase observation is not another item. |

All known `LoomSourceEvent` variants are matched explicitly. An indexed source
completion must have been prepared; a guessed or missing index rejects.
The helper tracks actual counter and sound event positions without mutating
their real owners during preparation. Original result taking has no crafted-item
statistic, recipe award or XP callback, and none is added here. Actual Player.drop
statistics remain with the existing drop owner.

## Phase admission and player-slot mapping

`loom_preflight_phase(phase, patterns)` checks the actual phase's three inputs,
derived preview, external cursor, every canonical player slot and the retained
window0 cursor. It uses recursive native component admission and the actual
configured original-default item-feature certificate. It also resolves maximum
stack-size queries used by the real InventoryChangeTrigger scan and checks that
selectable pattern holder IDs belong to the captured registry.

Preflight deliberately does not recompute a result or demand that an
intermediate pattern list already matches newly mutated inputs. Input callbacks
have meaningful historical phases before result/list/selection changes. A
final-inventory substitution would lose those source observations.

Loom inputs0–2 and preview3 belong to the station's container owners. Actual
player-backed menu slots4–39 map to canonical InventoryMenu slots9–44:
`canonical_slot = loom_slot + 5`. InventoryMenu slots9–35 correspond to the
same raw main-inventory index; canonical36–44 correspond to raw hotbar0–8.
The external cursor is distinct from retained window0 carried state.

`loom_local_changes` verifies this exact backing/mapping and produces
`LocalSlotChange` values only for the actual player inventory. Input/result
slot listeners do not trigger a fabricated player-inventory criterion. The
single data listener must be index0 and retain that event's actual selected
pattern. The source ServerPlayer data listener itself returns; MenuSync owns
its real local baseline and data packets.

## Packet and sound contract

`loom_sync_packets` extracts packets only. A normal transaction uses indexed
events so that actual listeners, counter changes, sound and item drops stay
interleaved with packets. A pickup's genuine synchronization-only broadcast
can extract its whole packet vector after source-phase preflight. Neither
operation installs a packet writer or executes a listener implicitly.

`loom_sound_packet(position, seed)` encodes protocol767 packet0x68 with the
actual registered sound holder `LOOM_TAKE_SOUND_ID + 1`, BLOCKS ordinal4,
captured block center, volume/pitch1 and the supplied real seed. The sole sound
constant remains in `main_loom_patterns`: raw ID1429, backed by
`fixtures/loom-production-sound.json`. Position conversion follows the source
double-coordinate multiplication and Java narrowing to signed int, including
saturation outside the ordinary supported coordinate slice.

Packet construction draws no random value. The actual transaction obtains one
`nextLong` from **Level.threadSafeRandom** at each emitted `TakeSound` event,
even with an empty audience, and no draw for a suppressed event. Player/drop
RNG remains separate. The kernel owns the source equality guard against the
menu's prior time, initially0; the helper additionally requires an emitted
event's signed time to equal its actual context's captured level time. It does
not substitute wall-clock time or invent an increasing-time condition.

## Callable APIs and handoff

- `PreparedLoomCallbacks::prepare`, `complete_source_at`,
  `is_player_inventory_change_at`, `is_sound_at`.
- `loom_event_phase` and `loom_preflight_phase`.
- `loom_local_changes`, `loom_sync_packets` and `loom_sound_packet`.

The primary `main_loom` owner consumes these helpers using its sealed context,
canonical transaction, actual callback owners and existing ordered publication.
Lifecycle/pickup owners reuse the same phase preflight. Multi-target Java Slot
identity-HashSet order, arbitrary listener replacement and unknown component
identity remain explicit unsupported contexts; these helpers do not fabricate
them. The original source review and concrete API handoff are complete. No Rust
build or Main runtime was run by this helper's owner; assembly and focused
verification remain with the integration coordinator.
