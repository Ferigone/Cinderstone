# Horse inventory menu: Java1.21 source and native owner contract

The primary reference is the unchanged Java **1.21**, protocol **767** inner
server JAR SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
with official mappings SHA-1
`31c77994d96f05ba25a870ada70f47f315330437`.
Evidence is actual JDK21 `javap -p -c`, **not REA or Java gameplay execution**:
`HorseInventoryMenu` (`cqv`) and its two slots, `ArmorSlot` (`cpx`),
`AbstractHorse` (`chi`, `chi$1`), `AbstractChestedHorse` (`chh`), `Horse` (`chk`),
`Llama` (`chl`), `Camel` (`cgv`), `SingleItemContainer` (`eyn`),
`Slot` (`crq`), `SimpleContainer` (`bra`), `Mob` (`btp`),
`LivingEntity` (`btn`), `ServerPlayer.openHorseInventory` (`aqv`),
`ClientboundHorseScreenOpenPacket` (`adc`) and `GameProtocols` (`agh`).
Raw disassembly and proprietary Java artifacts are not redistributed.

## Separate canonical owners and source layout

Java1.21 constructs this menu with a **null MenuType**. It has no ordinary
OpenScreen MenuType ID. `ServerPlayer.openHorseInventory` first handles the old
menu/window counter, sends the dedicated Horse Screen Open packet, then
constructs the menu and initializes its actual listeners/synchronizer.
`HorseInventoryMenu` calls `horseContainer.startOpen(player)` during construction.

The dedicated clientbound packet is ID **0x20**, derived from the pinned
GameProtocols registration order (the neighboring content/slot ordinals also
match the native protocol767 catalog). Its payload is unsigned-byte window ID,
VarInt **inventory columns**, then big-endian int entity ID. The middle field is
not the total inventory-slot count. The native controller stages this packet,
then the actual startOpen callback, then MenuSync initialization in that order.

| Menu slot | Actual backing owner |
| --- | --- |
| 0 | Captured horse SimpleContainer index0: saddle |
| 1 | Separate `horse.getBodyArmorAccess()` index0: actual BODY equipment |
| 2 onward | Captured horse SimpleContainer indices1 onward, three rows |
| `2 + 3 * columns` onward | Canonical player slots9–44, 36 slots |

The horse SimpleContainer has **1 + 3 * columns** slots. The menu has
**2 + 3 * columns** top slots, plus36 player slots, and no DataSlots. BODY is
not a second slot in the horse container. A menu does not own a result preview,
recipe, advancement or XP balance. Neither mount inventory nor BODY equipment
is copied into a second live menu-owned inventory.

| Actual family | Columns | Saddle admission | BODY admission |
| --- | --- | --- | --- |
| Horse | 0 | Actual alive, adult, tamed predicate | Equestrian `AnimalArmorItem` |
| Donkey/mule | 0, or5 with chest | Inherited ordinary predicate | Inactive; no admitted body item |
| Llama | 0, or actual strength1–5 with chest | Always false | Actual `WOOL_CARPETS` tag |
| Camel | 0 | Ordinary predicate; camel is always tamed | Inactive; no admitted body item |

These are facts about the real entity methods, not production rules provided by
a test family enum. `HorseMenuHost` must call the actual predicates and item/tag
owners. The saddle slot requires saddle item, empty slot and `isSaddleable()`;
it inherits ordinary container/item capacity rather than an extra hardcoded
one-item override. BODY uses ArmorSlot maximum1 and the actual
`isBodyArmorItem`. Its `isActive` flag follows `canUseSlot(BODY)`; active flags
are not substituted for the source click admission methods.

ArmorSlot removal checks actual `Player.isCreative()` and
`EnchantmentHelper.has(stack, PREVENT_ARMOR_CHANGE)`. A curse-tag guess or a
hardcoded Binding Curse ID does not implement this effect owner. CLONE and
creative drag independently check actual abilities.instabuild.

## Source mutation and callback phases

The controller implements the seven ordinary click modes. BODY
`ArmorSlot.setByPlayer(new,old)` invokes actual
`LivingEntity.onEquipItem(BODY,old,new)` **before** the body-access setter.
That setter calls actual `Mob.setBodyArmorItem`, including its equipment,
guaranteed-drop and persistence owners. These are distinct source boundaries:
the host must not collapse them or replay them against final state.

Raw count changes remain distinct from container setters. In matching
`Slot.safeInsert`, the cursor shrinks and the actual target grows before
`setByPlayer` reads its old-stack argument. This includes zero acceptance and
signed negative capacity for a loaded overfull target. A BODY callback therefore
observes the already-grown actual BODY value. Ordinary BODY `tryRemove` splits
the actual equipment stack first; when emptied, `setByPlayer(empty,taken)` gets
the removed old stack while its actual BODY access is already empty.

The real SimpleContainer setter replaces/clamps its slot and synchronously
runs its own listeners, then Slot.setChanged causes the additional source
notification. In-place split/merge has its separate notification boundary.
Actual AbstractHorse.containerChanged updates saddle metadata and can play a
saddle sound; these effects remain with the live entity/Level owners. BODY
access setChanged and vanilla SimpleContainer startOpen/stopOpen are source
no-op methods, but their calls are retained rather than replaced by invented
viewer counters or world effects.

Quick move uses the actual captured horse-container size plus the BODY slot:

- Mount slots transfer to the player in reverse order.
- Player items first try an empty valid BODY slot, then saddle, then storage.
- If storage cannot move an item, main inventory/hotbar fallback can still
  mutate player contents but **returns EMPTY early**. Source-slot cleanup is
  skipped, and AbstractContainerMenu's repeated quick-move loop stops.
- Successful primary branches clean/change the source slot and return the
  original stack. This menu calls neither Slot.onTake nor onQuickCraft.

`Slot.tryRemove` first rejects an insufficient destination capacity when
`allowModification(player)` is false (`mayPickup && mayPlace(current)`). An
occupied saddle slot rejects mayPlace even for matching saddle items. Thus a
source saddle count2 with explicit max-stack-size99 and matching cursor98 is
unchanged by PICKUP/PICKUP_ALL: room1 cannot authorize this removal. Refused
PICKUP still runs its unconditional final Slot.setChanged; no split or setter
callback is manufactured.

PICKUP/THROW/SWAP retain inherited Slot behavior. Creative CLONE creates its
source-allowed copy without a fabricated equipment/crafting callback. Drag uses
the actual selected Slot-identity HashSet encounter order supplied by the host;
one selected creative type2 slot recursively dispatches unsupported PICKUP
button2 and does nothing. Outside-left PICKUP drops before clearing cursor;
outside-right splits the cursor before Player.drop.

Closing runs superclass cursor handling, then only
`horseContainer.stopOpen(player)`. Saddle, chest storage and BODY remain owned
by the mount. The actual close-window packet precedes removed(), and transfer
of old inventory sync baselines to window0 follows it. No mount contents are
returned, dropped or drained by this menu's close callback.

## Required production bindings and supported context

`HorseInventoryMenu<Binding, ContextToken>` has a private nonreused instance and
checked revision token. Click and close require the captured token; public menu
clones are not exposed. Window reuse and equal stacks do not revive old actions.
Binding identifies the real captured player, Level, entity lifetime, horse
container and BODY-access objects. A network entity ID alone is insufficient.
The ContextToken certifies actual registry/equipment-effect interpretation,
nonreentrant callback scope and actual Slot identity/drag order. Changing it
rejects an action.

Source validity short-circuits in this order: the current horse inventory must
be the captured container; horse-container validity; BODY-access validity;
living mount; actual `canInteractWithEntity(mount,+4.0)`. This final reach check
uses the real player attribute/AABB owner, not a guessed position radius.
Inventory resizing/replacement must close an old menu rather than silently
rebinding it. Actual opening admission stays with the entity and packet owner:
ordinary AbstractHorse checks server/tamed and unoccupied-or-player-passenger;
Camel's override opens on the server without that ordinary admission gate.
No extra owner-UUID restriction is introduced.

Every action uses mandatory host `begin_stage`/`publish_stage`. The host privately
stages all canonical inventory, mount equipment/metadata/drop chance/persistence,
entity and Level RNG, actual sounds/game events, drops, listeners and packets.
Publication validates actual captured revisions and commits everything atomically;
an error publishes nothing. `HorsePhase` is a read-only callback observation,
not another live owner. Actual container/equipment/Inventory.add/placeItemBack
methods own their internal callback and packet phases. Shallow copies of shared
live owners do not meet this contract.

Shared component guards reject unresolved custom food-component20 identity,
including predictions; prototype elisions certified by the existing native
comparator remain supported. Bundle overrides are explicitly rejected. Actual
mutable/reentrant menu listeners, missing equipment/enchantment owners and
unknown Slot encounter order must be rejected by preflight. There is no fallback
fake mount, global block key, empty Level, synthetic curse registry or no-op
world implementation.

## Verification and remaining integration

Focused native scenarios use an explicitly restricted recording host with
fixture family/item/effect predicates. They execute the actual new controller:
opening packet/layout/start order; separate BODY and saddle/storage ownership;
actual borrowed player shrink before BODY callback; mandatory equip-before-set;
failed equipment callback rolling back staged inventory/menu/native RNG;
armor-removal effect admission; loaded overfull BODY callback phases;
occupied saddle count2/cursor98/max99 capacity refusal with final PICKUP dirty;
llama3-column and donkey5-column layouts/storage; all seven click modes;
quick-move's early EMPTY/no-source-dirty fallback; source outside cursor phases;
close retaining mount contents; and stale/reused UI, context/container binding
and prediction-component refusal. Fixture recording counters/effects are not
claims of a production horse, Java gameplay, entity tracking or PLAY execution.

Root adds the dynamic `MenuSync::new_horse_inventory` layout and mechanical
export. **No Main horse callsite or live horse owner is provided in this stage.**
The current shared entity index has Player/Item/ExperienceOrb kinds; this module
adds no fake kind or mount state. The entity owner/integrator must supply the
actual mount container, BODY equipment, registry/effect, player, Level and
callback transaction owners before production dispatch. Riding, AI, breeding,
chest attachment/removal, strength changes, death/loot, tracking and entity
persistence remain with those real owners. This bounded menu controller does
not complete fixed `inventory.4`, `crafting.2` or `crafting.5`.
