# Furnace reference ledger

This ledger records the shipped Minecraft Java **1.21** behavior used for the
native furnace, smoker and blast-furnace port. It does not by itself establish
implementation completeness or runtime compatibility; see the main roadmap for
the current integration status.

## Source identity and evidence boundary

- Inner server JAR SHA-256:
  `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
- REA CLI **6.8.0** inspected the JAR as a ZIP container. Parent Evidence:
  `ev_8062f8049307794f7a4fdb13f2a8e90b1c3c3a9f596760ee21b9aa77832b9e09`.
  Its complete central-directory inventory recorded 12,952 file entries and no
  declared integrity contradictions. This is archive evidence, not JVM
  decompilation, execution or gameplay validation.
- Method behavior below comes separately from **JDK 21 `javap -p -c`**, using
  the matching official Mojang server mappings. Static bytecode inspection does
  not establish that a particular runtime path was executed.
- The reference classes are `AbstractFurnaceBlockEntity` (`dpv`),
  `AbstractFurnaceBlock` (`det`), `AbstractFurnaceMenu` (`cpv`),
  `FurnaceResultSlot` (`cqs`), `ExperienceOrb` (`btc`) and `MenuType` (`crc`).
  Raw Java binaries and disassembly are not redistributed in this repository.

## Menus and processing

| Menu | Protocol registry ID |
| --- | ---: |
| Blast furnace | 10 |
| Furnace | 14 |
| Smoker | 22 |

The menu IDs follow the original `MenuType` registration order. In protocol 767,
container data uses clientbound packet `0x14`: container ID, signed 16-bit data
index and signed 16-bit value. Data indices 0–3 are remaining burn time, burn
duration, cooking progress and total cooking time respectively.

The shipped data contains 70 smelting recipes with 200-tick cooking time, 24
blasting recipes with 100-tick cooking time and 9 smoking recipes with 100-tick
cooking time. The engine must select the matched recipe's actual cooking time,
not infer it only from the menu type. The burn countdown is decreased before
processing. In the unlit cooldown branch, partial progress decreases by two
ticks, clamped to the range from zero to the current total. Replacing input with a non-equivalent item
and component set recomputes the total and clears progress.

Fuel ownership, output capacity, stack components, recipe remainders and the
wet-sponge/water-bucket case belong to the actual inventory transaction. A menu
must not retain a separate copy of the player's inventory or furnace contents.

## Result rewards and observable ordering

Taking a furnace result invokes the item's crafted callback with the accumulated
removed count. For a server player, the furnace then:

1. Resolves the recorded used recipes and emits their experience at the player's
   actual position.
2. Awards the recipes to the player.
3. Triggers each recipe-crafted criterion using the furnace's actual three slots.
4. Clears the used-recipe counters.

There is **no creative-mode bypass** in this result-slot reward path. Experience
uses a Java `float` product of recipe count and experience, its floor and
fractional part. A nonzero fraction consumes the shared `Math.random()` owner;
the result rounds up when that draw is below the fraction. The integer total
then enters the real experience-orb award/merge/spawn path.

Used recipes are held in Fastutil 8.5.12 `Object2IntOpenHashMap`, not a sorted
map. Its encounter order can affect random draws and entity spawning. A cold
NBT load first observes `CompoundTag`'s Java `HashMap` key order. Preserving counts
alone does not preserve these ordering effects.

## World callbacks and persistence obligations

All three furnace blocks inherit the base `onPlace` and the no-op base
`setPlacedBy`. `AbstractFurnaceBlock.onRemove` returns immediately when the new
state has the same block identity. A lit or facing change therefore retains its
owned block entity.

Replacing the block with a different block reads the actual block entity. For
an owned furnace on the server, it drops inventory contents, pops recorded
recipe experience at the block center, calls the base removal and updates
neighboring comparator output. An empty inventory can still hold recipe
experience; checking only whether slots are empty is insufficient.

Analog output reads all three actual slots. It sums each nonempty stack's
`count / container.getMaxStackSize(stack)` in Java `float`, divides by three,
then returns `floor(fullness * 14) + (fullness > 0 ? 1 : 0)`.
Block-entity dirty notification also requests comparator updates for a non-air
block state.

Integration must retain the actual world, inventory, recipe, random, entity,
advancement and persistence owners. This ledger does not establish support for
datapack reload, arbitrary imported worlds, all enchantment/mending contexts or
every block-removal context. Those boundaries must remain explicit until their
runtime owners are connected and verified.
