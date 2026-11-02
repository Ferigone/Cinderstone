# Ordinary FallingBlock source and native integration

This future stage adds ordinary sand, red sand and gravel to the retained native
falling-entity pipeline. It is separate from the earlier built Anvil stage;
source publication, a compiler result or an Anvil flow does not prove these new
ordinary paths ran.

The pinned Java 1.21 inner server has SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official mappings have SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
Evidence is direct JDK21 javap inspection of `diu` (FallingBlock), `dhg`
(ColoredFallingBlock) and `dit` (Fallable), not REA execution or Java gameplay.
Raw disassembly remains private.

| Actual source | Native path | Preserved contract |
| --- | --- | --- |
| FallingBlock.onPlace32–33 | falling_block::block::on_place | Reuses the existing real world/scheduler capability and unconditionally schedules delay2. Stable support does not suppress the attempt. |
| FallingBlock.updateShape37–39 | block::update_shape and ordered world-interaction planner | Every direction schedules before the inherited unchanged-state return. Each actual attempt consumes the same shared sub-tick count, including deduplication hits. |
| FallingBlock.tick44–50 | block::scheduled_tick | Query actual below state and short-circuit isFree before minimum build height; call the same actual FallingBlockEntity.fall owner, then the actual inherited falling hook. |
| FallingBlock.falling53 | NativeOrdinaryFallingHost::inherited_falling | The actual dhg instances inherit the literal return-only body. The same constructed/registered object is resolved; no Anvil hurt2/40 setter, entity RNG draw or second entity is introduced. |
| ColoredFallingBlock | block::source_class | The pinned catalogue contains exactly sand112, red_sand117 and gravel118 for dhg; it overrides dust color, not scheduling/falling/landing behavior. |
| Fallable.onLand14 / onBrokenAfterFall16 | retained Main tick host | The ordinary family inherits these literal return-only bodies. It does not emit Anvil events1031/1029. |
| Fallable.getFallDamageSource19 | actual configured NativeFallingDamageTypes | Resolve the same actual registry's falling_block holder; Anvil keeps its falling_anvil override. Ordinary constructors retain HurtEntities=false and the original default fields. |

## Actual owners and phases

`main_falling_block_generic` dispatches the actual scheduled BLOCK callback to
ordinary or existing Anvil source. `MainBlockRuntime::owns_falling_tick` requires
the actual installed callback/configuration owner; `owns_anvil_tick` retains its
narrow Anvil meaning. The outer shared scheduler still rereads the current Block
singleton before invocation, skipping stale ticks without inventing a revision
or exact-facing lock.

World-interaction Write/updateShape obligations use the finite source classifier
for the implemented ordinary and Anvil classes. Each changed write emits its
one inherited onPlace schedule before subsequent propagation; every shape query
emits its own schedule. The old selected-placement tail copy is removed only for
these represented classes. Unrelated or special FallingBlock subclasses keep
explicit unsupported admission rather than receiving a generic success.

The generic adapter reuses one genuine common constructor/preflight capability
from the native spawn owner: carried-state normalization, actual ID and process
constructor RNG/UUID, original-state fluid replacement with flags3 and real
callbacks, then actual UUID world-add admission. A false add result retains the
same actual constructed object for the falling hook; the source does not refund
the replaced origin. Anvil configures hurt only after this shared helper returns;
ordinary falling does not.

The existing retained Box, real motion/collision/index/sender owners, source
level tick body, actual Item constructor/drop path, atomic native cohort and
restored-order policy remain shared. A failed ordinary landing drops its original
Block item, including gravel itself; it does not run gravel's mining loot table
or invent a flint roll.

Concrete-powder water conversion/raycast, pointed-dripstone damage, other special
falling subclasses, active portals, nonempty block data, unsupported world
callbacks and actual survival hurt remain separate unfinished delivery. No
checklist point or whole-game compatibility percentage follows from this stage.
