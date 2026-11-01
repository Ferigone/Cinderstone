# Native ServerPlayer state restoration

Instance: es-20261101-1425-a7c3; issue [#4](https://github.com/Ferigone/Cinderstone/issues/4).
Source base: `c1baf5c70f07e53b6b76edaf84be5da57f6bd40b`.
Pinned original Minecraft Java1.21/protocol767 server SHA1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`; official mappings SHA1
`31c77994d96f05ba25a870ada70f47f315330437`; unchanged inner server SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Evidence is direct JDK21 javap and specifically labeled controlled runtime
observations, not an unperformed REA run. Raw disassembly, Java binaries and
probe artifacts stay outside Git. No actual Main invocation is inferred.

Instance: es-20261101-1425-a7c3. Sole new implementation: src/living_entities/restore.rs.
Worktree: /workspace/cinderstone-living-next-es-20261101-1425-a7c3; basec1baf5c70f07e53b6b76edaf84be5da57f6bd40b.
Root claim: #4 comment6564074170. No other file, Git, exports or public docs edited by author.

API: ServerPlayerRestoreHost (all required fallible source operations; actual Actor/owner handles), restore_from(new,old,alive,host)->Result<(),Error>.

Source evidence: direct original JDK21 javap with official 1.21 mappings, unchanged inner server SHA2568d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01; aqv.a(aqv,boolean),1415-1467. No REA run or original-Java restore runtime observation claimed.
1415: same old WardenSpawnTracker reference.
1416: same old nullable RemoteChatSession reference.
1419-1421: retain NEW gameMode receiver, OLD gameMode/current query, separate OLD gameMode/previous query, target.setGameModeForPlayer(current,previous).
1423: actual virtual ServerPlayer.onUpdateAbilities.
1425: retain NEW attributes, read OLD attributes, actual AttributeMap.assignBaseValues (NOT all values/modifiers).
1426: current NEW maxHealth -> actual setHealth.
1428-1443 alive: NEW inventory target before OLD inventory, actual replaceWith (retains source stack references/selected); OLD current health -> NEW setHealth; SAME old FoodData object -> NEW field; live OLD active-effects iterator. For each canonical retained effect, actual native EffectInstance.copy_instance constructor is called in short read, then actual new.addEffect takes that exact new object and its boolean is ignored. Copy retains canonical holder/details, omits hidden chain and blend state (brz87-90 +117-122). No borrowed old instance across addEffect callback. Only then per-field old level->new level, old total->new total, old progress->new progress, old virtual score->new virtual score; then SAME nullable old PortalProcessor.
1444-1450 dead: keepInventory on NEW current level, OR (short-circuit) OLD spectator. If admitted, only inventory replace + XP/score; no old health/FoodData/effect/portal copy.
1453: enchantmentSeed field, after branch callbacks (no RNG reseed/undo constructor draws).
1454: SAME old EnderChest object.
1455: retain NEW SynchedEntityData receiver BEFORE OLD entity data/read model Byte, then actual metadata set.
1456-1458: individual sentinels lastSentExp=-1, lastSentHealth=-1.0f, lastSentFood=-1 in this exact order.
1459: retain NEW recipeBook then OLD recipeBook, actual copyOverData into constructor-owned target, not sharing whole old book.
1460-1462: old seenCredits; SAME nullable enteredNetherPosition; SAME ChunkTrackingView.
1464-1466: old left CompoundTag getter -> actual new left setter; then right getter->setter; then old Optional<GlobalPos> lastDeathLocation getter->new setter.

Validation: focused rustc --test harness /workspace/scratch/es-20261101-1425-a7c3/restore-focused.rs compiles actual existing private effects::instance plus PUBLIC effects::EffectInstance reexport and only new restore module. Filter restore::tests:: passes. Meaningful mocked-owner checks: exact captured receiver order; alive canonical shared FoodData/object handles; fresh copied effect identity/holder/details with no hidden/blend; XP reread after reentrant addEffect mutation and ignored false boolean; dead keep/spectator subset/gates; consumed mutation prefix after addEffect failure; live iterator failure stops XP/tail; sentinel failure Exp->Health stops Food/book. Existing effect copy_blend_state unused warning only in this small harness.

Limits: no constructor/Actor implementation or connection/ID/list replacement; those remain caller respawn obligations. No actual AttributeMap/definition/gameMode/Inventory/food/effect-map/recipe-book/metadata/persistence adapters here. Missing owners must error, not return dummy maps or copied lifecycles. Actor/holder/object handles retain real identity/generation and must not resolve by reusable wire ID. This stage stores no health/absorption/food/RNG/session/registry or actor state; actual metadata and retained canonical owner remain singular. Not Main integration or whole survival/entity completion.

## Integration and validation boundary

These callable source bodies require actual canonical actor, world, item,
attribute/effect, inventory/menu, entity ID/generation, packet, RNG, persistence
and connection-lifetime owners at their source callsites. Missing owners cannot
return successful no-op behavior, manufacture an empty world or reset a listener.
Synchronous errors retain completed mutation/output prefixes; they are not
rollback transactions. A later async transport failure belongs to the actual
connection completion owner and cannot retroactively stop a completed body.

Independent source reviews and focused controlled native flows cover the
bounded implementations; final exact compiler/crate results and source SHAs
are recorded on the issue-linked PR and canonical per-instance status. The
selected actual milestone is a lethal ServerPlayer lifecycle. Shared Main/body/
index/session/menu-generation/save seams still require central narrow assignment.
No whole entities or survival delivery, public gameplay or checklist change is
claimed. [Concrete production contract](entities-survival-production-milestone.md)
and [original lifecycle facts](entities-survival-death-respawn.md) describe the
remaining work.

Binding detail: Inventory.replaceWith557–561 assigns actual old ItemStack references
through setItem and does not copy each stack. FoodData in the alive branch and
EnderChest always retain original object identity. XP level/total/progress are
direct source field writes, not giveExperience/award APIs; no recalculation,
statistic or extra packet is added. Enchantment seed assignment does not reseed
or replay constructor RNG. Native value snapshots need a real identity owner
before they can satisfy these contracts.
