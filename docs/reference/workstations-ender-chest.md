# Personal ender chest ownership and callbacks

Target: Java 1.21, protocol767. Primary inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
official mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
Direct JDK21 `javap -p -c` inspection of PlayerEnderChestContainer (`crh`),
EnderChestBlockEntity (`drd`, `drd$1`), EnderChestBlock (`dip`),
ChestLidController (`dqp`), ContainerOpenersCounter (`dqu`) and numeric NBT
FloatTag/DoubleTag methods supplies the source facts below. No REA, Java
server gameplay, proprietary artifact or raw disassembly is claimed/published.

## Native personal container

`EnderChestInventory<P,K>` owns one actual player's27 slots. `P` is the real
player binding; `K` is the captured actual EnderChestBlockEntity object key.
The runtime must register exactly one personal owner per player. A replacement
block entity at the same coordinates receives a different key; coordinates
cannot substitute for object identity. There is no invented world inventory in
an ender chest block entity.

Slot replacement uses SimpleContainer's count limit min(container99,itemmax).
Closing invokes the captured actual chest's stopOpen callback, then clears the
active binding; contents are retained in the personal owner. A different player
cannot use these lifecycle methods. Failed unsupported callbacks do not clear
the active binding; the outer actual world/player transaction must roll back
its own side effects on error. Source startOpen/stopOpen superclass callbacks
are inherited no-ops, not additional fabricated opener counters.

Validity delegates the captured chest to actual
Container.stillValidBlockEntity/player reach with additional range4.0. With
no active chest the SimpleContainer superclass is valid. Actual opener counter,
spectator/removal gates, Level RNG sounds, block events and five-tick scheduler
rechecks remain the real block/world host's callbacks. The key is not persisted.

`load_items`/`save_items` reuse the existing ItemStack persistent codecs. Load
starts with27 empty slots, interprets Slot as unsigned byte, ignores out-of-range
slots and lets later duplicate entries win. Numeric Float/Double Slot values
use the original Mth.floor then byte narrowing; truncating -0.5 would wrongly
replace slot0 instead of skipping unsigned255. Unknown native item/component
codecs fail atomically and retain the previous owner, an explicit unsupported
import boundary rather than silent data loss. The active binding remains
transient; persistence integrates with the real player owner, not block NBT.

## Block use, menu and lid

`use_on_server` retains actual source order: resolve the actual block entity,
check the block above with isRedstoneConductor, bind the same player's active
personal inventory, call real Player.openMenu, award the ender-chest-open stat,
then PiglinAi.angerNearbyPiglins(player,true). Missing/blocked contexts still
consume the server interaction. Unsupported piglin/world ownership must return
an error; no default empty-world or no-op award is provided.

The openMenu host must preserve old-menu closing and new constructor callbacks
against that SAME personal container; the active binding is already set at
that phase. Reuse the existing three-row GenericContainerMenu, protocol MenuType2
and27 personal slots. Its ordinary menu cursor close is separate from personal
contents. This does not rewrite or newly claim the pre-existing chest/barrel
click foundation.

EnderChestLid ports ChestLidController: block event1 alone requests open when
count>0; each animation tick copies old openness then moves0.1f toward0/1;
getOpenNess uses raw linear Mth.lerp. Renderer easing is not part of this owner.
No Level or player RNG is consumed by that animation helper.

## Verification and pending integration

Focused native checks passed with
`cargo test --locked --lib workstations::ender_chest::tests`: player-specific
contents, actual replacement-key validity, failure/close retaining contents,
item NBT roundtrip with no active binding, negative fractional Slot handling
and source lid interpolation. These are isolated native owner checks, not a
protocol-client or gameplay flow.

No Main callsite, registered player EnderItems owner, actual world block entity,
scheduler/opener/piglin callbacks or cold-restart player integration is claimed.
Main/player persistence remains centrally owned. The aggregate fixed checklist
deliveries remain incomplete.
