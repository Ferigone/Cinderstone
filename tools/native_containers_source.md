# Native chest and barrel owner (1.21)

Artifact: official inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Sources inspected through the bundled mappings and actual JVM bytecode:

- `cqc` ChestMenu: 27/54 container slots, followed by main inventory 9..35
  and hotbar0..8. QuickMove from container visits player slots in reverse;
  QuickMove from player visits the container forwards.
- `cpu` AbstractContainerMenu: the ordinary unrestricted-slot click modes
  follow the existing source-derived InventoryMenu engine. Slot validation,
  client predictions, stale state synchronization, cursor, drag distribution,
  swap including offhand, throw and two-pass PickupAll are implemented.
- `dqu` ContainerOpenersCounter: onOpen/onClose, game events, count changes and
  five-tick scheduled recheck. An already scheduled recheck remains after the
  last viewer closes. Rechecks use the actual active menu registry.
- `dqo`, `dqo$1`: chest sound emits from the right half for double chests,
  midpoint origin, block event1 carries the current opener count.
- `dqa`, `dqa$1`: barrel sound at face offset, OPEN state updates, no chest lid
  block event. Spectator start/stop callbacks do not increment/decrement.
- `dqh.getUpdateTag`: actual implementation returns an empty CompoundTag.
  Chest/barrel do not override it. Chunk data therefore carries actual type
  and position with the empty update tag, not inventory data.

`native_container_facts.java` captures sound IDs and block IDs from the actual
bootstrapped registries. Existing `storage_blockentity_factories.json` confirms
chest BE type1, barrel type26. This is static artifact evidence, not a claim of
full runtime conformance.

The FlatWorld owner creates a BE only at a native block transition. Loading
palette changes deliberately creates no inventory. The separate typed native
checkpoint restores and validates the actual slots/type/state/name/lock/NBT.
Java NBT import/export delegates item component handling to `item_nbt` and
retains original per-slot item fields. Unknown unresolved loot tables fail.

The Main bridge borrows canonical SharedContainer inventory per transaction.
BE slots, source history and open menu state commit before item registration
and before menu publication; only temporary staged values are copied. Overflow
drops use the actual DropCommitOwner/RNG/ID/index/tracking owners. Closing
InventoryMenu and generic menus returns cursor/input items or real drops.

Scheduled chest/barrel block callbacks are dispatched by the shared global
BLOCK scheduler. The Main container loop only checks menu validity and sends
menu changes; it does not run a duplicate timer queue.

Explicit remaining boundaries: nonempty container destruction requires loot
and content-drop integration; complex custom titles/lock text require their
text component owner; unresolved LootTable inventory is unsupported. Trapped
chests, shulker boxes, furnaces and other BE types are not represented by this
owner. Exact full-server callback/publication timing still requires final
validation. No full vanilla parity or performance advantage is claimed.
