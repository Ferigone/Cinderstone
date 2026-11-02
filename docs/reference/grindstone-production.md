# Native Grindstone production boundary

**Runtime status: one controlled actual Main flow passed.** GrindstoneMenu is
connected at source `18314fecde381929a1984d3ab941abebdaa2d113`, tree
`9eea8d01ae97f61aedf9e7e92838eca6cbc70ea6`, targeting Minecraft Java **1.21**,
protocol **767**. The full Main executable build passed. Its private binary
SHA-256 was
`0b76c6d0b35b2395b9fcff4600cecfa26aef6ed6504017fe35c64e0f604c24f1`.
The successful bounded flow completed at
`2026-11-01T23:59:21.627444+00:00`. The separate earlier attempt failed at an
existing equipped-enchantment profile boundary; its report remains `ok: false`.
Neither result completes an aggregate checklist delivery or establishes full
vanilla workstation compatibility.

## Actual Main call paths and owners

| Production entry | Actual operation |
| --- | --- |
| `main.rs`, `LivePlayers.grindstones` | Retain one captured `OpenGrindstone` and genuine `GrindstoneMenu` token/context per current user |
| PLAY block use `0x38` | `use_native_grindstone` checks the actual provider/block/actor/reach, retires a preceding external menu and uses the existing window allocator |
| Container click `0x0e` | `click_native_grindstone` selects this actual current owner; unequal wire windows are handled by ignoring them before inactive InventoryMenu fallback |
| Client close `0x0f` | Main atomically selects the sole current owner; received window ID does not choose a stale retained menu |
| Actual player menu phase | Broadcast first, then fresh captured-block/reach validation and context-aware close |
| Native body and existing connection pickup | Select this retained menu's pure synchronization callback with the same inventory/gameplay owner |
| Player persistence capture | Observe canonical inventory with the external menu's actual separate cursor |
| Player removal/reconfiguration | Remove retained InventoryMenu first, then Grindstone with actual disconnected context, save and remove the actor |
| Native shutdown checkpoint | Retire connected external menus only in shutdown; ordinary autosave retains them |
| Component refusal recovery | Select the genuine retained owner and invoke its actual full-state operation |

The existing shared `PlayerContainer` owns canonical player inventory. The menu
owns exactly two temporary inputs and its external cursor; result slot 2 is a
derived preview. MenuType is 15 with 39 slots: inputs 0/1, result 2, player main 3..29
and hotbar 30..38, backed by canonical 9..44. Offhand remains canonical 45. Window 0
keeps its own cursor and 2x2 inputs. No grindstone block entity, second live
inventory, private connection writer, independent item index or entity-ID
allocator is introduced.

## Shared current-menu pickup correction

The current level-player phase captures all eight actual external menu owners before validity can close one. Native Generic and Crafting pickup now uses the retained current menu at the literal Player.take phase, after Inventory.add and Collect. Each genuine local slot observation reaches the existing player gameplay listener before that slot's remote baseline and packet. Generic top-container slots and Crafting result/grid slots retain their actual foreign-container identity; only player-backed slots map to canonical InventoryMenu coordinates.

Pure validation of actual retained baselines, the incoming item and gameplay owner runs before Inventory.add. Only the existing typed UnsupportedFoodIdentity boundary refuses without inventory/stack/revision mutation or Collect; unexpected owner/state errors propagate. The [actual current-menu pickup flow](../validation/current-menu-pickup-flow.json) separately passed on source `c136c78` and binary `af50dfb4`: real Collect preceded the correct active Generic/Crafting slot updates and item/preview ownership was conserved. See [its bounded evidence](current-menu-pickup.md). This does not complete the older Generic/Crafting click/listener implementations or arbitrary reentrant advancement behavior. The final focused ordinary lib+Main compiler check passed; the separate runtime report remains tied to its recorded source.

## Supported source profile and registry

Mutable actions require the actual certified native flat-world source and
existing alive, unmounted creative Overworld body/lifecycle profile. The
read-only spectator/full-state branch is wired in source but unexercised here.
Applicable custom equipped interaction-range modifiers use a typed unsupported
boundary that can retire the real menu; generic errors are not silently
reclassified. Unknown imported sources, unsupported callbacks/effects and
arbitrary custom actor profiles remain outside this evidence.

The actual gameplay owner certifies the native empty-criteria/nonreentrant
floor. Unknown registered advancement rewards/predicates, statistic objectives
or mutating item/container hooks are not ignored. Native equipment currently
also requires no normal enchantments in the actually equipped slots because
movement/location enchantment effects need their real owner. Enchanted items
in inactive inventory are supported by this flow; equipping them is not.

`NativeGrindstoneEnchantments` binds the same actual configured enchantment
registry, curse tag and feature packets sent by Main, matching the pinned
default snapshot and holder domain. It reads minimum costs from the real
configured definition. Alternate or omitted known-pack definitions, unresolved
holders/tags and unknown feature snapshots fail instead of receiving guessed
costs or a fabricated empty curse set. Grindstone uses no recipe manager or
crafted/book/XP callback borrowed from another workstation.

## Source phases, inventory counter and genuine mixed entities

The retained kernel's nonreused instance/revision and binding/registry snapshot
participate in each staged action. The Host carries the actual actor/UUID,
transient level owner, dimension, captured position, canonical container,
player/Level random states, complete entity-index proposal, shared ID sequence,
XP proposal and ordered events. It uses the genuine same-token staged-menu
capability; equal stacks or reused wire windows do not authorize another menu.

Each source event retains its `GrindstonePhase`: current inputs, preview,
canonical player inventory observation and external cursor. Input change
handling retains its old-preview broadcast, recomputation and new-preview
broadcast. The Host consumes those emitted constructor-default boundaries
without invoking them a second time. Player-backed listeners use their own
phase inventory rather than the action's final image.

`player_inventory_set_changed` records the before-image and increments its
actual staged inventory counter immediately. Subsequent phases therefore see
the source after-image. Commit installs each event's actual inventory image
before replaying listeners/packets and performs the real canonical
`set_changed` at its source position. The final counter is checked against the
prepared final inventory before installing that image. Protocol state IDs are
a separate wire synchronization owner; this runtime does not independently
instrument transient `Inventory.timesChanged` or every internal callback.

All fallible encoding, source/registry/phase, world/rain/chunk, callback and
entity-index preparation precedes live commit. Owned drops move once into real
ItemEntity construction with actual player/Level RNGs and the existing random
factory. The actual XP stage immediately advances its source Level stream,
merge queries/index and registration proposal at each award. It does not defer
all XP to an end-of-action batch or grant points directly to a player.

The mixed Item/XP registration token publishes Before/Registered/Complete
boundaries through the existing ordered output stream. Each genuine XP handle
is attached before its award checkpoint publishes; an Item drop advances drop
statistics only after world-add. Menu packets, source callbacks and level events
remain between these registrations. There is one item outbox, actual XP owner,
spatial index and shared entity-ID allocator. Result THROW is a useful bounded
witness because its XP award and level event must precede the later Item drop.

## XP and world-event source contract

The actual non-curse minimum-cost sum is computed over both current source
inputs. A positive total produces `ceil(sum/2)` plus
`Level.random.nextInt(ceil(sum/2))`; bound 1 still consumes a draw. Nonpositive
or overflow-negative totals produce 0 without that draw. Creative mode has no
bypass. The source awards actual orbs at the block center, emits level 1042/data 0,
then clears input 0 and input 1 with their source callbacks. The taken preview
itself is not an extra item. No crafted statistic, recipe-book award or direct
player-XP increment is invented.

The real orb award retains its own source split/merge/constructor behavior and
random ownership, distinct from Level.threadSafeRandom sound seeds. The server
level-event packet 0x28 uses captured integer block coordinates and actual
same-dimension strict radius 64 audience. The actor's event was observed here;
multi-player/dimension audience edges and complete RNG streams were not
runtime-exercised.

## Lifetime and synchronization boundary

Connected close uses actual health/removal and connection context. It returns
owned cursor first, then owned inputs via the existing Inventory.add/place-back
paths, with overflow as genuine drops. Disconnected cleanup supplies
`DeadOrDisconnected` after retained InventoryMenu removal and drops only actual
owned cursor/inputs. Preview is never an additional return, drop or persisted
stack. Requested CloseScreen precedes removal publication; old player-slot
synchronization baselines transfer to retained window 0.

Ordinary autosave retains current menus and observes their external cursor;
shutdown retirement is confined to its actual shutdown branch. Current-menu
full-state recovery is a true retained-menu operation, not initialization,
incremental broadcast, dummy click or inactive window 0 correction. Pickup uses
the same current owner immediately after Collect and retains source listener
encounter order. Its private packet batch is published after callbacks under
the empty-criteria profile; arbitrary packet-publishing/reentrant callbacks are
not certified. Reach/refusal/full-resync/pickup paths remain unexercised here.
Multi-target drag still requires its actual Slot-identity HashSet encounter
owner and remains unsupported.

## Separate actual runtime records

Both runs used the identical source/tree/private binary above, actual protocol
framing/compression/Slot/NBT readers and fresh synthetic offline worlds. The
configured fixture supplied diamond sword 838, grindstone 1210 and actual
Sharpness holder 32. Level 1's captured minimum cost is 1, so this positive source
path must award exactly XP 1. Three source swords occupied separate legal
max-stack-size 1 slots; none was an oversized tool stack.

| Record | Actual result |
| --- | --- |
| [Initial strict flow](../validation/grindstone-flow-initial.json), completed `2026-11-01T23:50:19.338583+00:00` | `ok: false`. Actual preview, XP 1→level1042→Item order and connected input return passed. Later native player body rejected equipped normal enchantments before the final disconnect/cold stages. |
| [Controlled supported-equipment flow](../validation/grindstone-flow-supported.json), completed `2026-11-01T23:59:21.627444+00:00` | `ok: true`. One authorized retry retained inert unenchanted stone 1 in selected hotbar 0. Original sword 3/XP 1/order/conservation expectations were unchanged; connected return, active-menu disconnect and native cold checkpoint completed. |

The initial harness SHA-256 is
`6a3c2af62f69c992838cbb86a501190a4fa258808a5239f8b5162fbd357e08b7`;
the controlled separate variant is
`bf65fa198116c17cebd09c13d8197708120618e76904e66ccd5b8124e54a6bc1`.
The initial report remains byte-for-byte separate, with SHA-256
`fce5ab93648965fdb4142bbc44b072c378fc3fb6532d6768dd82cf26de36418d`;
the controlled success has SHA-256
`cec482e3e6aed00e1d47fdedd3e411e93b2c1220e78fd21a42604b6a6c84322f`.

The initial failure's actual server diagnostic was
`native_player_body_failed: enchanted movement/location effects require their actual owner`.
Saved selected hotbar 0/canonical slot 36 held a Sharpness I sword. Connected
Inventory.add had returned that input into the previously empty selected slot.
The saved prefix still conserved two enchanted swords in the player, one plain
sword in the world, orb XP 1 and empty cursor. The later assertions were
unreached, not converted into a complete PASS.

The controlled precondition reserves the selected slot with one actual inert
stone, so a returned enchanted sword occupies an inactive slot. It changes no
sword/XP expectation and retains that same source body boundary; it does not
implement equipped enchantment effects. The stone remains player-owned once
through the native save/restart.

| Controlled actual observation | Result |
| --- | --- |
| Real open and single Sharpness I input | MenuType 15, 39 slots; one actual plain-sword result preview |
| Result THROW | Genuine XP entity 2,value 1 at 10.5,64.5,8.5; integer-position level 1042/data 0/global false; then real plain Item entity 3 |
| Source order and shared allocator | Observed XP spawn ordinal 97 before level event 99 before Item spawn 100; adjacent entity IDs 2→3 |
| Connected return without taking another preview | Two owned enchanted swords returned to inventory, cursor empty, no extra take/XP |
| Disconnect with another active preview | Player retained enchanted sword 1; actual world item owners retained enchanted sword 1 and plain sword 1; preview added nothing |
| XP ownership | Player XP 0 plus genuine saved world-orb XP 1 equals exactly 1 |
| Native cold restart without PLAY actor | Same Item/orb counts, unique saved IDs and actual next_entity_id 5 retained; inert stone 1 and empty cursor retained |

Thus three initial enchanted swords became two remaining enchanted swords plus
one owned plain sword, with exactly one actual take and one source XP award.
Crafted statistics remained empty. Every isolated server completed its real
SIGINT shutdown with exit 0; the initial harness itself exited unsuccessfully
and its status is preserved.

The no-PLAY cold restart proves native checkpoint acceptance, exact item/XP
counts and shared-counter retention, not restored tracking/ticking readiness.
XP stayed in the actual world orb: player pickup and Mending were not exercised.
The source draw at bound 1 is required by source code; this protocol observation
does not independently reconstruct the full RNG sequence. Curses, repair pairs,
partial quick-move, multi-holder/custom components, drag/click-mode matrices,
ordinary autosave timing, Java saves, public/graphical clients, all stations and
performance remain unproven. No checklist percentage or denominator changes.

## Separate Anvil placement blocker

At this same source/binary, the independent Anvil flow stopped before OpenScreen
while awaiting real block placement. Its saved world had no changes; no rename,
take or Anvil lifecycle PASS is claimed. Source inspection identifies a concrete
admission boundary consistent with that observation: Anvil's captured
FallingBlock.onPlace schedules a tick at delay 2, while the current native block
scheduler supports its existing fire/redstone/container callbacks and refuses
unported falling callbacks. The runtime did not log that exact rejection
branch, so the branch attribution is source analysis, not a captured diagnostic.

A menu-only Anvil flow would need a genuine already-settled input with an
accepted complete native world/entity source history. Editing a failed world
and rehashing its certified checkpoint would not create that history. No Anvil
input, block entity, pending tick or virtual tile was fabricated. This is a
separate integration blocker and does not weaken the Grindstone expectations.

Source evidence uses direct JDK 21 `javap` and bounded pinned-original
observations, not REA MCP. The original 1.21 server and official mappings are
pinned in [source-lock.json](../../fixtures/source-lock.json). See
[kernel source phases](workstations-grindstone.md),
[actual transactional Host](main-grindstone-host.md),
[integer XP owner](grindstone-experience.md),
[opening](grindstone-opening.md),
[configured callbacks](grindstone-recipes-production.md) and
[transactions](grindstone-transactions.md) for narrower source contracts;
earlier unexported-stage descriptions are historical.
