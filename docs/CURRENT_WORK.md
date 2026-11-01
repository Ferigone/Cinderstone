# Current native port status

Cinderstone targets Minecraft Java 1.21, protocol 767 and DataVersion 3953. It is
an experimental native Rust implementation. Full vanilla compatibility is not
complete. The current supported player profile is an alive, unmounted creative
player in the native flat world, without active status effects.

## Native chat, sessions and signatures — bounded local flow passed

Native chat/session code is connected to the actual production owners in source
`b3a0d1dc0ed01de666c7bf56723ebfe30140e0be`, following core `66ebc57`, Main
`1f56b98`, sequencing/key-wait fixes `cea8569` and cleanup `396b589`.
The full Main build passed. The [local chat flow](validation/chat-flow.json)
passed on **1 November 2026 at 17:53:46 Europe/Warsaw**
(`2026-11-01T16:53:46.366Z`) on binary SHA-256
`509a4dfb7d95ce7d05403b169b5f4922f0d3482713ab6f961a003be9e3c662f0`.

- Actual offline players exchanged Unicode player-chat packets with their real
  sender identities. Only FULL chat recipients received them; SYSTEM and HIDDEN
  clients did not. HIDDEN senders received the source refusal without executing
  their commands or broadcasting their messages.
- Bounded unsigned commands remain connected. An empty-signature signed-command
  envelope in offline/enforce-false mode executed a real ban with its original
  reason, persisted the actual target profile and delivered its disconnect.
  This is not proof of a trusted signed command or Mojang certificate.
- Invalid acknowledgments and illegal characters produced their source
  disconnect reasons. Nonoperator spam disconnected, actual operators were
  exempt and the spam state recovered with observed native tick progress.
- Trusted-service key refresh, profile certificate and message signature
  verification, signed chains, acknowledgment windows, per-recipient signature
  caches and actual session/PlayerInfo ownership now have native Main paths.
  A separate [generated-key core smoke](validation/chat-crypto-smoke.json) passed
  certificate binding, actual listener/session acceptance, signed-chain/cache
  progression, tampering rejection and decoder-owned expiry. It uses generated
  trust keys, not a Mojang-issued certificate or production Main authenticity.
- Chat/session/administration output now shares the ordered entity-event stream.
  Whole command validation and execution share the actual chat/admission owners.
  Terminal connections retain the reader during write-half close and bounded
  input draining, instead of dropping unread input and risking a TCP reset.

The optimized code release reached **SUCCESS** and native offline listening at
**17:56:46 Europe/Warsaw**, with one replica and the existing `/data` volume.
The fresh external status probe failed DNS; public gameplay remains unverified.

The [existing admission flow was rechecked](validation/admission-after-chat-flow.json)
on `cea8569`; it passed after the shared-output/command changes.
[Production contracts and remaining limits](reference/chat-production.md)
include the default no-team/no-external-filter profile, bounded signed argument
ranges, complete Components/selectors, real-account/public verification and exact
asynchronous scheduling. **authentication.4 remains open**. Fixed checklist
indicators stay **56/100 code**, **51/100 Main**, `scope_version: 1`.

## Dedicated admission and administration — bounded local flow passed

Native access-list ownership and production admission checks are now connected.
The source stage is `6c5401882c2cf2d454968455fa98b1e91b653aa5`, following core
`76141a7` and initial Main integration `870349d`. The full Main build passed.
The [focused local flow](validation/admission-flow.json) passed at
**17:10:14 Europe/Warsaw on 1 November 2026** (`2026-11-01T16:10:14.211Z`),
on binary SHA-256 `f5e29a9ecb012d4685c9358612e453189c28475cd2d9fb50624e85c6a0a55411`.

- Actual UUID/profile and accepted TCP peer feed player-ban, whitelist, IP-ban
  and capacity checks before LOGIN success and again after CONFIGURATION.
  The final gate runs before player/entity/RNG construction. Admission and
  administration policy mutations share the same serialization owner.
- `banned-players.json`, `banned-ips.json`, `ops.json` and `whitelist.json` have
  native owners. Whitelist enable changes persist in `server.properties`.
- Console and bounded unsigned player commands drive real bans/pardons,
  whitelist changes, op/deop, kick, list and banlist. Actual operator changes
  publish permission events and a refreshed permission-filtered command tree.
- The local flow observed persisted live bans, actual disconnect/reconnect
  rejection, whitelist ordering, denied nonoperator mutation, current-list
  suggestions, and operator capacity bypass only from a true stored flag.
  A real pending resource-pack task held CONFIGURATION while its profile was
  banned; finishing then rejected it without creating a PLAY actor.
- The LOGIN timer now consumes cumulative actual connection ticks across packet
  receipt and authentication, ending when Login Acknowledged replaces its owner.
  Its timeout mechanism compiled; the recorded flow does not time out a client.

The optimized code release reached **SUCCESS** and native offline listening at
**17:12:58 Europe/Warsaw**, with one instance and the existing `/data` volume.
The fresh external status probe still failed DNS; public gameplay is unverified.

See [production owners and configuration](reference/admission-production.md).
Complete signed-command contexts, full Brigadier/selectors and remote
profile-cache lookup, all Gson/date/locale compatibility, exact asynchronous
LOGIN scheduling and packet-rate transport enforcement remain unfinished.
A separate native 64-connection bound is explicit; it is not vanilla capacity.
This is bounded implementation and integration, **not completion of
`authentication.5`, `commands_data.1` or another aggregate checklist delivery**.
The indicators remain **56/100 code**, **51/100 Main**, `scope_version: 1`.

## Configuration lifecycle — published and bounded local flow passed

The existing `network.5` delivery, **reconfiguration, cookies and resource packs**,
now has native code and production Main callsites. This closes that named item
under its unchanged definition. At `scope_version: 1`, with the same 100-item
denominator, the planning-checklist indicators are **56/100 code deliveries**
and **51/100 Main integrations**. These are not functional compatibility,
engineering-effort or remaining-time percentages.

The implementation was published in source commit
`643106e8eb60d970a8df67eb9acf0ab5f107c788`. The full Main build passed, and the
[focused local configuration flow](validation/configuration-flow.json) **passed**
on 1 November 2026 at **15:18:52 Europe/Warsaw**
(`2026-11-01T14:18:52.638Z`), using debug binary SHA-256
`d68f5c8de6c95cec4df4d28777c3e35f045890ac9a725181a29b38d0eb3690dd`.
The recorded flow establishes:

- An actual admitted PLAY connection switching through CONFIGURATION on native
  SIGHUP, then readmitting a new real player on the same TCP connection without
  a second login/authentication handshake. Eleven emeralds and moved position
  were retained, and a subsequent authoritative world interaction succeeded.
- Initial known-pack query/acknowledgement and brand-before-feature-flags order.
  `RETURN_TO_WORLD` did not repeat initial brand, registry or known-pack tasks.
- Cookie responses in LOGIN, CONFIGURATION and PLAY producing the exact vanilla
  `multiplayer.disconnect.unexpected_query_response` disconnect reason.
- Optional resource-pack ACCEPTED and DOWNLOADED responses keeping the active
  configuration task pending, with a terminal optional failure permitting PLAY
  even when the response UUID was unknown. Required DECLINED disconnected before
  PLAY with `multiplayer.requiredTexturePrompt.disconnect`.
- A required flag without a configured URL producing no offer or effective
  requirement, and an unsolicited PLAY decline with no pack being ignored.
  Malformed prompt JSON syntax produced a warning and omitted the prompt while
  allowing the implemented optional-pack flow to finish.

CLI `--resource-pack-url`, `--resource-pack-sha1`, `--resource-pack-id`,
`--resource-pack-prompt` and `--require-resource-pack` are connected to the native
host. The container entrypoint maps the corresponding `MC_RESOURCE_PACK_*`
variables. SIGHUP selects the native operator `RETURN_TO_WORLD` transition;
it is not datapack or dynamic-registry reload. The
[reconfiguration](reference/reconfiguration.md),
[cookie](reference/connection-cookies.md) and
[resource-pack](reference/resource-packs.md) ledgers keep source facts separate
from the recorded native flow.

The optimized hosted release reached **SUCCESS**, with one instance, the existing
volume and native offline listening at **15:25:08 Europe/Warsaw** on 1 November
2026. Public gameplay remains unverified because the external status probe fails
DNS resolution. No
resource-pack URL was fetched or applied in this flow. It does not establish a
graphical-client session, public-network gameplay, arbitrary dynamic registries,
the general component codec or every malformed late-PLAY decoder. Narrow native
water-movement statistics and FoodData healing also reuse existing core logic;
they do not close `player_motion.3` or `survival_combat.2`, whose remaining Main
ownership and behavior must still be completed.

## Earlier closed single-container removal — bounded local flow passed

The newer bounded creative-removal stage is published for a
closed **single dry chest** or closed barrel. `main_container_removal` connects
an opaque native `ContainerOwnerKey`, the actual 27-slot snapshot and a privately
drained staging clone to the owned native source-history/world transition.
Container contents become real Item entities using the actual level RNG,
independent entity RNG factories and shared entity IDs. An emptied staging image
is not published as a live container.

The path retains the source creative item guard, including rejected sword
destruction, and stages contents before the comparator-notification boundary
and block-entity removal. The supported comparator path certifies **no actual
receiver**; connected comparators are not silently ignored. This callback creates
neither recipe XP nor a loot item for the block in creative mode.

The bounded scope rejects actual open menus, including spectator menus that may
not appear in an opener counter, double or waterlogged chests, unresolved loot
and comparator receivers. Other viewer lifetimes and survival block loot remain
unfinished. The [container-removal source ledger](reference/container-removal.md)
records the Java callback order and boundaries.

The full Main build passed. The
[focused local removal flow](validation/container-removal-flow.json) **passed**
on 1 November 2026 at **14:27:29 Europe/Warsaw**
(`2026-11-01T13:27:29.235Z`), for source commit
`be2582a9ea032205c3885fd04e10ff1419d63fb9` and debug binary SHA-256
`9d6eea10d5ac164d26017156ff6b59f76e379e518017681649a8c8937190b542`.
Two protocol clients observed the creative sword guard preserving the chest
and its contents, then successful removal of the closed chest and barrel with
real item entities and air at their original block positions. Their native
block-entity owners were absent from the checkpoint.

The exact owned-item totals—37 dirt, two coal, 33 quartz and seven emeralds—were
unchanged after cold restart, with the saved remaining Item identities retained.
This is a local protocol-client result for the recorded closed-container scope,
not an open-viewer, survival, graphical-client or public-network result.
The optimized hosted release reached **SUCCESS**, with one instance, the
existing volume and native offline listening at **14:31:18 Europe/Warsaw**.
The shared-menu flow also passed on this same debug binary at 14:31:27, after
the new container-owner changes; its earlier source result remains recorded.
Public gameplay remains unverified because the external status probe fails DNS.
That bounded removal addition did not complete a full fixed checklist row.
The current checklist indicators are recorded above and below.
The earlier recorded results below retain their source versions and scopes.

## Earlier shared menu close lifecycle — bounded local flow passed

The implementation connects a shared `MenuCloseContext` to window0, crafting-table
and chest/barrel close paths, extending the earlier bounded furnace disconnect
handling. Actual disconnect cleanup selects the disconnected context; normal
live closes and live-player save cleanup retain the inventory-return context.
The implementation includes:

- Dropping each removed menu's owned cursor through the native Item/RNG
  transaction on disconnect, without inserting it into the player's inventory.
- Removing and dropping the nine temporary crafting-table inputs or four
  window0 inputs in their original order, without treating the derived result
  preview as another owned stack. Chest/barrel contents remain in their owner.
- Running window0's removed callback before the current external menu during
  disconnect cleanup, matching the bounded source callback order.
- Retaining window0's own cursor and 2×2 grid when opening a block menu from
  window0, instead of unconditionally closing the inventory menu first.
- Explicit alive inventory returns with per-insertion raw `-2`, state-ID `0`
  slot packets, followed by transfer of the old menu's shared local and remote
  slot baselines when an external menu closes.
- Publishing the server-initiated external close packet before its prepared
  cursor/grid return or drop effects.

The full Main build passed. The
[focused local menu flow](validation/disconnected-menus-flow.json) **passed** on
1 November 2026 at **14:20:00 Europe/Warsaw**
(`2026-11-01T13:20:00.117Z`), for source commit
`93438c6055ec755e85f528739a41073ef5563b5a` and debug binary SHA-256
`b4e368a716239879e0e5ada20b5db258aa45ad93a76ed853bc6bf3e5cb2de8c3`.
It exercised actual disconnects from chest, barrel, 3×3 crafting and window0.
An ordinary connected chest close returned its cursor through a raw-slot packet.
Opening the chest retained window0's cursor and grid; disconnect published the
inventory cursor as Item ID 3, its grid input as ID 4, and the chest cursor as
ID 5, establishing that recorded source order.

The crafting flows dropped the owned inputs and cursor without creating the
displayed hay-block or crafting-table recipe results. All recorded owned-item
totals were unchanged after cold restart, including three emeralds and two coal
retained in block containers, with empty saved player cursors and crafting grids.
This is a local protocol-client result, not a graphical-client or public-network
gameplay result. Its optimized hosted release reached **SUCCESS** in deployment
`737cb031-94ce-45d0-9f15-cd3633b80b1a`, with native offline listening at
`2026-11-01T13:23:02.384209111Z` (**14:23:02 Europe/Warsaw**), one running
instance and the existing persistent volume. Current public gameplay remains
unverified because the external status probe fails environment DNS resolution.
These observations do not verify hosted execution of the later removal stage above.

The [menu-close source ledger](reference/menu-close-lifecycle.md) records the
pinned Java 1.21 methods, source condition, input ownership and opening/removal
order separately from these bounded native observations.

Full `PlayerList.remove` player-save/persistence ordering, every asynchronous
disconnect race, integrated death/survival, mixed alive inventory-return and
overflow-drop packet interleaving, other workstation families and complete
player/server tick phases remain unfinished. This bounded menu stage does not
complete a full inventory or simulation checklist row. Its historical validation
does not change the current checklist indicators recorded above.

## Earlier disconnected furnace cursor — bounded local flow passed

Furnace menu closure now distinguishes an alive, connected owner from a dead or
disconnected owner. Main's actual disconnect and duplicate-login cleanup select
the disconnected context: the carried stack enters the real player-drop path
without first returning to inventory. Normal live closure and save cleanup keep
their inventory-return context. This does not implement the player-death or full
survival lifecycle.

The [focused local disconnected-cursor flow](validation/disconnected-furnace-flow.json)
**passed** on 1 November 2026 at 13:55:31 Europe/Warsaw
(`2026-11-01T12:55:31.284Z`), for source commit
`07e290cc66411d0d1ededfae69a592899a2e5d10` and debug binary SHA-256
`cea142d01922dca01f548bec76f430ac07e4faac6872876ff825a622a01a437d`.
Two protocol clients exercised an actual disconnect with five quartz on the
furnace cursor. The remaining client observed the real item entity. The saved
player cursor and inventory contained zero quartz; exactly one item entity,
ID 3 carrying five quartz, retained the same ID and count through cold restart.

This result covers that local protocol flow, not a graphical vanilla-client
session, public-network gameplay or every disconnection race. The optimized hosted
release reached SUCCESS and native offline listening at 12:59:45 UTC, as recorded
in the [deployment observation](validation/railway-preview.json). Public gameplay
remains unverified. Full player-death handling, full-inventory overflow
conformance and the complete Java `PlayerList.remove` persistence order remain
incomplete. This bounded furnace addition did not close an aggregate checklist
delivery; current indicators are recorded in the configuration section above.

## Furnace menu lifetime after removal — bounded local flow passed

The removed furnace container now remains owned by its existing menus after
creative destruction. Its dropped slots are empty, while the old container's
burn counters and used-recipe data remain available until the final viewer
releases it. Menus capture the actual owner identity, so a newly placed furnace
at the same coordinates cannot become the old menu's container. The Level player
phase broadcasts the captured container, checks validity and then closes it.
The close-window packet precedes cursor-return effects.

The [focused local open-menu flow](validation/open-furnace-menu-flow.json)
**passed** on 1 November 2026 at 13:29:55 Europe/Warsaw, for source commit
`482f11b1e269e8e8f78b2b3d7e4ccdf1763b1b1b` and debug binary SHA-256
`c85bd34e0ca87b41acfb355edd6c7e2e5349a42517523fae04904bf0f2b3dba1`.
The recorded scope includes:

- Two viewers holding the same ordinary furnace menu while another player
  destroys the furnace; both observe its emptied old slots before menu closure.
- Both viewers observing a replacement block entity at the same position before
  their old menus close, without rebinding those menus to the new container.
- The old burn value remaining 1,391 while the replacement furnace has its own
  empty contents and independent state.
- Close-window publication before cursor return, with cursor contents returned
  once to each alive player's inventory.
- Conserved totals of 34 dirt, four coal, one diamond, five quartz and seven
  emeralds through clean shutdown and cold restart, with one XP point before
  and after. Remaining item IDs are a subset of the original saved IDs because
  legitimate merges can remove entities; no replacement IDs were introduced.

The compiler check also passed. This is an alive-creative, local protocol-client
result for the recorded path. The optimized hosted release and native offline
startup are verified separately below. It does not establish graphical-client or public-network
validation, full vanilla compatibility, or a performance advantage.

That flow does not cover disconnected-player cursor drops; the separately recorded
result above records that path separately. Player-death handling and
full-inventory overflow conformance remain incomplete. Survival breaking,
comparator receivers, Mending repairs and other menu families remain incomplete.
The earlier processing and closed-menu results below retain their exact recorded
versions and scopes.

## Earlier closed-menu furnace block removal — bounded runtime flow passed

The current implementation connects creative destruction of owned native
furnaces, smokers and blast furnaces to their real inventory contents and stored
recipe experience. It stages container stack splits and item-entity creation
before recipe XP, using the actual random and shared entity owners, then removes
the block entity and its ticker with the world change. A privately drained
staging image is never published as a live furnace state. Creative destruction
does not award a loot item for the furnace block itself.

The [focused local removal flow](validation/furnace-removal-flow.json) **passed**
on 1 November 2026 at 11:47:29 Europe/Warsaw, for source commit
`6eaf7487ffcca7856f233280a1e3e1b7d1ec192d` and debug binary SHA-256
`46a3514fd9906ed975e44c1e6577dc2fa021fcead1ca659a456e0d0053c50b8f`.
The recorded scope includes:

- Closed-menu destruction of all three furnace types, actual item entities and
  recipe-experience orbs emitted at the block center.
- Normal menu closure returning the cursor stack, plus the creative sword guard
  acknowledging the request without removing the furnace or spawning items/XP.
- Lit-state changes retaining the furnace contents and owner.
- Item conservation across player inventory and native entities before and after
  cold restart: 51 dirt, six coal, one diamond, one cooked beef, one netherite
  scrap and five quartz. The remaining item-entity IDs were preserved.
- Experience conservation across the same restart: one player XP point plus two
  points in saved orbs, totaling three both before and after restart.
- Removed blocks and block entities absent from the checkpoint, with clean
  SIGTERM shutdown after draining actual pending world/scheduler work before
  menu closure and checkpointing.

This is a local protocol-client result. The optimized hosted release and native
offline startup are verified separately below. The result does not establish a graphical vanilla-client
session, public-network gameplay, complete scheduler equivalence or a throughput
advantage. The earlier processing observations below retain their original
binary and scope.

That recorded path used the supported native creative-player profile and
furnaces with no active menu viewers. The newer bounded active-viewer lifetime
path is described above. Survival breaking and other removal contexts remain
unsupported. Comparator receivers, Mending repairs, custom cooking datapacks
and menu output-XP actions that also drop items remain open.

## Earlier furnace processing integration — bounded runtime flow passed

The default cooking catalog was published in `872d638`; native furnace engines,
menus and persistent block-entity foundations were published in `2f5f716`.
The current Main integration connects:

- Furnace, smoker and blast-furnace processing with captured vanilla recipe and
  fuel data, canonical three-slot contents, counters and lit-state changes.
- Authoritative menus using the shared player inventory and cursor, with window
  contents and cooking/burn data synchronization.
- Owned block entities and native save/restart records. New native records retain
  registration order; legacy records restore their recorded order.
- Furnace experience rewards through real orbs, shared entity ticking, pickup
  and tracking, using the supported native player host.

The compiled batch passed a bounded local protocol-client flow on 1 November
2026 at 04:27:37 Europe/Warsaw. The
[machine-readable flow report](validation/furnace-preview-flow.json) identifies
binary SHA-256
`46340001f58f69500955c0f8e278b4670c707c18279a0cab1768e5c8044489b4`.
The observed scope was:

- Native PLAY join using Minecraft Java 1.21 protocol 767.
- Furnace processing at 200 native cooking ticks and smoker/blast-furnace
  processing at 100 ticks, with actual fuel, counters and menu synchronization.
- Normal and shift result transfers, lit-state changes and live lighting.
- Real XP-orb spawning/pickup and the player's experience packet.
- Shared chest-inventory behavior alongside furnace interactions.
- SIGTERM save and cold restart preserving partial burn/cooking state, output,
  used recipes, player XP and the correct chunk block-entity types.

The saved partial furnace had cooking progress 13/200 and 1,110 burn ticks
remaining. The report records two XP points before restart and four after the
continued flow. These observations apply to the recorded binary and path. The
probe was a protocol client, not a graphical vanilla-client session. Hosted
startup is recorded separately below. Source behavior and
evidence boundaries are described in the
[furnace reference ledger](reference/furnaces.md).

Remaining furnace limits are explicit:

- The bounded closed-menu, alive-creative active-viewer removal and disconnected
  cursor paths passed their separate recorded local flows above. Player death,
  survival and other removal contexts remain incomplete; full-inventory
  overflow conformance and complete `PlayerList.remove` persistence ordering
  remain open.
- Comparator receivers and Mending experience repairs are unsupported.
- Menu actions combining furnace output experience and item drops are rejected
  before commit. The new block-removal transaction is a separate path.
- The captured default catalog does not establish support for custom cooking
  datapacks or dynamic recipe reload.

These are bounded implementation deliveries, not a claim that every furnace or
experience interaction is complete.

## Normal-rate server clock — exercised in the bounded flow

The earlier processing batch also replaced the unbounded timer-backlog behavior
with the original ordinary 20 TPS deadline and overload policy. The first tick is ready
immediately after initialization; the period is 50,000,000 nanoseconds. Modest
backlog is caught up. A lag greater than two seconds advances the deadline past
whole missed periods only when `nextTickTime - lastOverloadWarning` is at least
15 seconds. This is not an unconditional missed-tick skip policy.

The Linux implementation uses the absolute `CLOCK_MONOTONIC` origin, matching the
installed JDK21 reference runtime and the initially-zero warning field. The
non-Unix fallback uses a process elapsed-time epoch and does not claim the same
initial-warning behavior. Tick sprinting, freeze control and complete Java task
queue/draining semantics are outside this bounded clock change.

The successful furnace flow ran against the binary containing this clock,
including steady normal-rate world-time observations. It does not establish
complete scheduler/task-queue equivalence or fixed wall-clock cooking duration
under every load condition.

## Existing integrated scope and historical verification

The main runtime also includes native player body phases for the supported
creative profile; water/lava travel; food, milk and honey use; Chest/Barrel
inventories and persistence; 2×2 and 3×3 crafting; selected redstone components;
a shared BLOCK scheduler; and selected current-version Java `player.dat`/item
NBT codecs.

The earlier [offline flow report](validation/offline-preview-flow.json) records
public-bind startup, status/pong, an unauthenticated PLAY login, name-derived UUID
and graceful shutdown for its recorded binary. A live offline PLAY join was also
observed in Railway deployment logs.

The earlier [bounded gameplay report](validation/native-preview-flow.json)
records join/movement, a water-bucket roundtrip, chest transfer, 3×3 furnace-item
crafting, milk use, SIGTERM save and cold restart for its exact recorded binary.
Crafting a furnace item in that report did **not** validate furnace cooking.
Those historical results do not validate the current furnace integration binary.
Comprehensive conformance and equal-workload performance validation remain deferred.

## Publication and deployment

Normal source files are published in the public
[Ferigone/Cinderstone repository](https://github.com/Ferigone/Cinderstone) on
`master`, with an English README and roadmap. Source archives are not committed.
Railway follows that unpinned branch automatically, using one replica and a
persistent `/data` volume.

Preview address: **`hopper.proxy.rlwy.net:24418`**, for Minecraft Java **1.21**.
The preview accepts offline accounts with `MC_ONLINE_MODE=false`; nicknames are
not authenticated, and name-derived UUIDs do not automatically reuse existing
online-account data. The image default remains `MC_ONLINE_MODE=true`.

The configuration lifecycle release reached successful automatic deployment
`cb51f40a-c758-4734-8dc1-1826dc400328`, with one running replica and native offline
listening on `0.0.0.0:25565` at 15:25:08 Europe/Warsaw on 1 November 2026. The
existing persistent volume is retained. The
[deployment observation](validation/railway-preview.json) records hosted startup
separately from local protocol-client behavior. No public PLAY join is recorded
for this release; the earlier join remains explicitly historical. A fresh
external status/pong probe failed DNS resolution from the development runtime.

## Remaining scope and checklist

Unsupported areas still include full survival/damage/death/respawn, active
effects, vehicles, elytra, mobs/AI, complete generation, remaining workstations and
block entities, complete redstone, TNT/explosions, commands/datapacks and full
Java-world interoperability. Unsupported owners and callbacks remain explicit
errors.

The fixed 100-item planning checklist records **56/100 code deliveries** and
**51/100 main-server integrations** at `scope_version: 1`. The existing
`network.5` delivery is now implemented and called by Main; the denominator and
definitions are unchanged. These are checklist
indicators, not measures of functional compatibility, engineering effort or
remaining time. The broader inventory and simulation deliveries still require
all remaining menus and complete block-entity ticking; these bounded furnace and
menu stages do not complete those checklist items. The denominator and definitions
are unchanged.

Porting and integration remain the priority. See the [README](../README.md) and
[roadmap](ROADMAP.md) for the English project scope and remaining work.

## Independent team coordination

The [coordination guide](coordination/README.md) and three copyable prompts are
available for independent sessions. Real GitHub assignments are the
[integration queue #1](https://github.com/Ferigone/Cinderstone/issues/1),
[world-generation team #2](https://github.com/Ferigone/Cinderstone/issues/2),
[workstations team #3](https://github.com/Ferigone/Cinderstone/issues/3) and
[entities/survival team #4](https://github.com/Ferigone/Cinderstone/issues/4).
They use feature branches, explicit file ownership and integration contracts;
the central coordinator retains Main, metrics, journal and deployment ownership.
HTTP 403 on issue comments has a documented versioned branch-status fallback.
These assignments and prompts are not evidence that another session or seven
agents are running; active work must be reported from observed state.
