# Grindstone: Java 1.21 source facts and native boundary

The reference is the unchanged Minecraft Java **1.21**, protocol **767** inner
server JAR, SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official server mappings SHA-1 is
`31c77994d96f05ba25a870ada70f47f315330437`.
This stage used **JDK 21 `javap -p -c`**, not REA: `GrindstoneMenu` (`cqt`),
its input slots (`cqt$2`, `cqt$3`) and result slot (`cqt$4`),
`EnchantmentHelper` (`dae`), `ItemEnchantments.Mutable` (`dai$a`),
`ItemStack` (`cuq`) and `Container` (`bqk`). Raw disassembly and proprietary
Java artifacts are outside the source checkout and are not redistributed.

## Computation and slot facts

- Menu type is 15, with input indices 0/1, result 2, player inventory 3–29 and
  hotbar 30–38. Both inputs accept a damageable item or an item with any normal
  or stored enchantments. The ordinary container maximum is 99, further bounded
  by the item's maximum. Counts above one in either input suppress the result.
- A single plain damaged item produces no result. A single enchanted item copies
  its component patch and removes non-curses from the crafting enchantment
  component: stored enchantments for an enchanted book, normal enchantments for
  other items. A different extra enchantment component is not silently removed.
- Two inputs must have the same item. Their maximum damage is the maximum of
  both values. Remaining durability is added with a five-percent integer bonus
  of that maximum; damage is clamped as `setDamageValue` does. Non-damageable
  pairs require maximum stack size at least two and identical components,
  and produce count two. The first item's unrelated component patch survives.
- Second-input curses only merge when the first item lacks that curse. An
  existing level-one curse is not upgraded by a second level-three curse.
  Non-curses are removed after merging. Tooltip visibility is inherited from
  the first crafting enchantment component. Repair cost resets to zero and
  applies `2 * cost + 1` once per retained curse, using Java integer wrapping.
- An enchanted book without remaining curses becomes a normal book through
  `ItemStack.transmuteCopy`; this retains its component patch while changing
  the item prototype. Reconstructing a component-free book loses unrelated data.

## Take and lifecycle contracts

`GrindstoneState` owns exactly two inputs and one derived preview. `set_input`
recomputes atomically. `prepare_take(amount, registry)` does not consume an input
or RNG draw. Its token cannot be cloned through the public API, and commit
checks the exact captured inputs and preview. Removing a result consumes both
inputs even for a partial take of a count-two result; the remaining displayed
result is not another owned stack.

`commit_take` requires the real Level RNG callback. XP sums each non-curse's
actual registry minimum cost over both original inputs. Positive totals produce
`ceil(sum / 2.0) + Level.random.nextInt(ceil(sum / 2.0))`; bound one still draws.
Zero or overflow-negative totals do not draw and return zero. Creative mode has
no bypass. The source then awards actual experience orbs at the block centre
on the server, emits Level event **1042**, data **0**, clears input zero and
clears input one. Each clear runs `slotsChanged/createResult/broadcastChanges`;
the returned take record carries the intermediate and final previews for that
ordering. No item-crafted callback or recipe advancement is added here.

The owning adapter must stage the menu, actual Level RNG and actual orb/world
transaction together before publishing any of them. The native kernel does not
award player XP directly, create entities, invent a registry or bind curse tags.
`GrindstoneEnchantments` queries the actual holder/tag/minimum-cost owner and must
fail when unresolved. Closing drains only the two owned inputs, after the host
processes the actual carried stack. The preview is discarded. Validity requires
the real captured block/Level access and the ordinary distance check.

## Verification and integration limits

A temporary local reflection harness invoked the **unchanged** `cqt.computeResult`
on an allocated menu object; it supplied vanilla registries and explicitly bound
the official curse tag. It did not simulate a Level, clicks or PLAY traffic.
Observed results were:

| Inputs | Java result |
| --- | --- |
| Single unenchanted iron sword, damage 200 | Empty |
| Iron swords damage 200/100; first Sharpness II + Vanishing I, second Unbreaking I + Vanishing III | Damage 38; Vanishing I; first tooltip visibility retained; repair cost 1 |
| Sharpness II enchanted book | Ordinary book |
| Two identical Vanishing I sticks | Two sticks, curse retained, repair cost 1 |

Focused native checks cover those source facts, an invalidated take token, the
bound-one RNG draw, a partial count-two take and close ownership. These checks
use a labeled fixture enchantment registry; that test fixture is not a production
world owner. Full compiler/runtime results belong in the team PR handoff.

Main has no grindstone callsite in this stage. Click modes, destination transfer,
cursor/state-ID synchronization, world validity, actual XP entities, closing and
disconnect dispatch require the central adapter and real owners described above.
This bounded kernel does not complete `inventory.4` or `crafting.5`.
