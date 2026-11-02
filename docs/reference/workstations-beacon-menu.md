# Beacon menu controller — Java 1.21

This adds a callable controller over the existing `BeaconState`. It owns only
the temporary payment, cursor and menu synchronization/lifetime. The actual
captured block entity retains its live data, registry filtering, lock, beam/base,
powers, effects and persistence. Main remains unintegrated; this does not finish
an all-menu or all-workstation checklist delivery.

## Pinned source evidence

Primary evidence is JDK21 `javap -p -c` of the original inner server JAR,
SHA-256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
and official mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
This is static inspection, not REA or original-server gameplay execution.

Inspected `BeaconMenu` (`cpy`)30–72, `removed`76–85, `stillValid`89,
`setData`94–96, `quickMoveStack`100–139 and `updateEffects`166–172;
`cpy$1`33–38 and `PaymentSlot`185–190; `BeaconBlockEntity$1` (`dqc$1`)
92–116; `BeaconBlockEntity.createMenu`354–357 and `filterEffect`122;
`BaseContainerBlockEntity.canUnlock` (`dqb`)83–89; `BeaconBlock` (`dfq`)
49–58; `ServerGamePacketListenerImpl.handleSetBeaconPacket` (`aru`)654–662;
`ServerboundSetBeaconPacket` (`ahu`)14–16 and `MobEffect.STREAM_CODEC` (`brx`),
plus the common `cpu`/`crq`/`bra` click, insertion, removal and dirty callbacks.
`javap -p -v` resolves the update lambda to `Level.blockEntityChanged`
(`dcw.q`752–755), which marks a loaded chunk unsaved. No comparator callback or
additional block-entity setter is substituted. The kernel's earlier
[beacon ledger](workstations-beacon.md) remains applicable.

## Layout, payment and ordinary actions

MenuType9 has payment0, player1–36 (canonical9–44), and three data values:
levels0, primary1, secondary2. Each data read comes from the captured actual
ContainerData. MenuSync's baselines and phase snapshots are observations, not
another authoritative beacon record. There is no result preview or recipe.

Both the temporary container and payment Slot have maximum1. The actual
`beacon_payment_items` tag supplies admission. Ordinary pickup/drag/hotbar swap
splits to that limit. Shift inserts into empty payment only when the entire
player source has count1; larger valid-payment stacks move between main1–27
and hotbar28–36. Payment shift targets player slots in reverse. All seven
ordinary click modes use source callbacks, including payment pick-all,
creative clone, offhand swap, source alias decrements before target callbacks,
signed/zero player insertion and nonempty self-hotbar swap multiplicity.

The payment container is a plain SimpleContainer with tag/max overrides.
Changing payment invokes its temporary container callbacks; it does not mark the
persistent beacon dirty or add a broadcast absent from source. Player slot
setters/onTake retain their actual Inventory.timesChanged operations. Every
callback, sync and drop phase separates the external cursor from retained
window0 carried state. Bundle overrides require their actual item/feature owner
and reject where used. Explicit FoodProperties component20 overrides reject
before generic inventory/sync equality, including predictions; stable prototype
food remains admitted. No nested ItemStack reference identity is fabricated.

Completed multi-target drag requires the host to certify independence of ALL
resulting native broadcasts/listeners/counters/cursor/sync and actual callbacks
from Java's Slot HashSet encounter order. It receives the exact target set and
starting phase and is checked again at commit. A custom no-op listener alone is
insufficient. Unknown/order-dependent contexts reject rather than invent order.

## Opening, selection and live data

`prepare_open` stages the actual `canUnlock` check, including real denial text
and chest-locked sound. `BeaconOpening.commit` publishes those denial effects
even when its result is None. Success creates a unique temporary UI. The host
must use the real player's spectator/main-hand state, LockCode and display name.
The separate block-use interaction stat belongs to Main and occurs after the
openMenu attempt when the source block entity exists, including lock denial.

`BeaconUpdatePacket` decodes two optional-presence booleans and raw registered
mob-effect IDs. There is no window ID, state ID, tier index or generic menu
button. Main must bind SET_BEACON to the actual current BeaconMenu. Registry
holder ID0 is present when encoded as true/0. ContainerData instead represents
None as0 and a holder ID as ID+1. Unknown packet holders reject at decoding.

The actual handler checks active menu class and stillValid. With payment,
update executes primary setter, secondary setter, payment removeItem callback,
then actual Level.blockEntityChanged. Both absent choices still consume payment;
there is no creative bypass, tier/combination check or synthetic effect owner.
The actual primary setter owns its server/beam-dependent selection sound before
decode/filter, even when the requested value is unchanged or filtered out.
The actual BE's VALID_EFFECTS determines accepted powers. This controller
neither substitutes registered-for-valid nor computes beam/base/effect ticks.

The observed core update hook records pre/post data-set and payment/dirty
boundaries around those real operations. SET_BEACON neither immediately
broadcasts nor closes. The next actual player-phase broadcast reads live data.
In contrast, `prepare_set_data` implements BeaconMenu.setData's setter followed
by immediate broadcast. Initialization, ordinary click completion, full-state
resend and later broadcasts also read the actual host data.

stillValid uses the captured level/position's beacon block type and source reach,
not an invented block-entity identity test. The data handle nevertheless retains
the originally captured BE. A replacement must not silently rebind it to a new
same-position owner; ownership checks and source block validity are distinct.

## Transactions, close and production boundaries

`BeaconMenuHost` is mandatory and stages actual captured world/player/block,
registry/tag/feature, callback, packet, sound, dirty and drop/RNG owners. Its key
contains real identities/revisions; its mutable revision guards actual owners.
`stage` must isolate all mutable state. `publish_stage` must atomically publish
it or leave live owners unchanged on error. Cloning shared mutable handles is
not a private stage. Unsupported/reentrant listeners or unavailable owners must
reject admission. No production default/no-op host is supplied.

`stage_event` executes the actual temporary-container/player dirty, listener,
packet and drop operations against their readonly source phase during
preparation. Data-set and block-entity-dirty source records are observations of
operations already executed once by BeaconHost; they must not execute again.
Returned events/drops are publication evidence, not a second effects queue.
The real host publishes its staged operations once. Plans are non-Clone and
validate the full UI, unique lifetime, checked revision, canonical inventory,
actual owner key/revision/live data, validity when used and drag certification.
Closing marks the UI closed; old plans and same-window reopen replays reject.

Close first handles the real cursor: alive connected return/overflow packets
and drops, or dead/disconnected drop. It then removes payment with its container
callback and always Player.drop(false,false), even alive with inventory space.
Payment is never routed through the ordinary input-return helper. Ordinary
throw/outside drops use trace=true, while menu-removal drops use trace=false.
After close, old shared-slot baselines remain transferable by Main.

Production still requires actual block-use/menu allocation/active dispatch,
lock and interaction-stat phases, persistent BE lifetime/data, source primary
sound/filter and loaded-chunk dirty owners, world effects/ticking/persistence,
actual player counters/listeners/cursor/Item/RNG/IDs and packet publication.
The player phase must broadcast before validity/close. Neither the native
controller nor fixtures supply a beacon world/effect registry owner.

## Focused native verification

Focused flows exercise live setter/sound/filter/payment/dirty order and deferred
update sync, immediate setData/full sync, max1 and shift count1, all seven clicks,
cursor-first alive payment drop, lock denial publication, callback/publish abort,
live-data staleness, closed/same-window UI rejection and FoodProperties limits.
Fixtures implement only controlled owners needed by those observations; they
are not production beacon implementations. With the assigned shared
CARGO_TARGET_DIR, `cargo test --lib beacon -- --nocapture` passed four controller
flows, three existing kernel flows and the shared beacon/lectern layout check on
1 November 2026. Both owned Rust files passed rustfmt and the owned diff passed
whitespace checks. Existing unrelated compiler warnings remain. No
protocol-client or original-Java execution is claimed.
