# Native death loot, inventory and experience dispatch

Instance: es-20261101-1425-a7c3; issue [#4](https://github.com/Ferigone/Cinderstone/issues/4).
Source base: `c1baf5c70f07e53b6b76edaf84be5da57f6bd40b`.
Pinned original Minecraft Java1.21/protocol767 server SHA1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`; official mappings SHA1
`31c77994d96f05ba25a870ada70f47f315330437`; unchanged inner server SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Evidence is direct JDK21 javap and specifically labeled controlled runtime
observations, not an unperformed REA run. Raw disassembly, Java binaries and
probe artifacts stay outside Git. No actual Main invocation is inferred.

# Death loot API/source ledger (scratch; not production integration proof)

Base implementation checkout: c1baf5c70f07e53b6b76edaf84be5da57f6bd40b.
Owned new source only: src/living_entities/death_loot.rs.
Original evidence: pinned 1.21 server-inner SHA256 8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01 + server-1.21.txt official mappings. Direct javap (no REA).

## Public functions and source
- living_drop_all_death_loot<H:LivingDeathLootHost>(host, &ServerLevel, &Source): btn.a(aqu,brk),1431-1439; recentlyHit local snapshot -> shouldDropLoot/doMobLoot -> actual table/custom -> virtual equipment -> source.getEntity -> virtual XP.
- living_drop_experience<H:LivingDeathExperienceHost>(host, Option<&Entity>): btn.C(bsr),1445-1448; retained ServerLevel, !consumed, always || recent/should/doMobLoot; position snapshot BEFORE getExperienceReward; actual integer-orb award even zero/negative result.
- living_drop_equipment(): btn.ez1442 literal return body, not default missing callback.
- player_drop_equipment<H:PlayerDeathEquipmentHost>: cmx.ez719-724; literal parent then actual keepInventory -> vanishing -> Inventory.dropAll; no creative exemption.
- destroy_vanishing_cursed_items<H:PlayerVanishingHost>: cmx.fT727-733; getContainerSize every loop; actual retained ItemStack; EnchantmentHelper.has(PREVENT_EQUIPMENT_DROP); removeItemNoUpdate CURRENT slot. dae.has375-381 checks registered effect presence over actual enchantment entries, not curse ID or positive level.
- inventory_drop_all<H:PlayerInventoryDeathHost>: cmw.k497-506; retained compartments list (constructor36-48 immutable order36items,4armorFEET->HEAD,1offhand); each nonempty actual Player.drop(stack,true,false) then current slot EMPTY. No cursor/crafting/ender chest operation or eager slot packets.
- player_base_experience_reward<H:PlayerBaseExperienceHost>: cmx.eg1861-1869; actual keepInventory || spectator ->0 else wrapping level*7 capped above100 only; no XP reset.
- player_is_always_experience_dropper(): cmx.eh1875 literal true.
- JAVA_INVENTORY_TO_MENU_SLOT maps Java0..40 into existing native46slots: hotbar36..44, inventory9..35,armor8,7,6,5,offhand45. Adapter only, no new inventory.

## Actual production seams required
- Central canonical PlayerInventory/SharedContainer owner must expose source-prefix slot operations with no mutable borrow held across real producer callbacks. Current SharedContainer.transact rolls back on Err and forbids reentry: it cannot wrap whole death with earlier real drops then rollback. Inventory slot clear happens AFTER drop returns; never publish empty staging inventory first. All components/counts retained; around=true trace=false means pickupDelay40, no thrower/drop stats.
- Existing item_entity::spawn_player_drop_with_factory, RuntimeEntities::prepare_items/PreparedItemBatch::commit, shared ID counter, LivePlayers.player_random, RuntimeEntities.world_random and actual random_seed factory are real foundations. Preserve each item's2Level doubles before factory/entity RNG UUID/bob/yaw then2Player floats. Register/tracking/tick order before next source slot; no fabricated perdeath RNG.
- Existing private main_experience::prepare_furnace_experience_at has integer-orb split/group/query/merge/add code, but must be generalized for integer death awards rather than fake FurnaceRecipeReward or Math.random. Each denomination draws actual Level.nextInt40, takes first actual query candidate (count++/age0), else independent orb RNG+shared ID+new actual orb+addFreshEntity. Source ExperienceOrb.award160-179.
- Living.getExperienceReward608 delegates actual EnchantmentHelper.processMobExperience108-113 using CAUSING entity, victim, actual retained ServerLevel and virtual baseXP. Living causer iterates actual equipment/MOB_EXPERIENCE effects; null/nonliving causer returns base. No lower clamp or death-time XP reset invented.
- Pinned player loot table is {type:entity,random_sequence:minecraft:entities/player}, no pools; still requires actual source/direct/context/loot registry/namedstream owner through Living.dropFromLootTable1470-1486. No table-empty no-op was implemented.
- Real loot/enchantment/neutral/packet/attribute/Main/persistence owners and complete lethal runtime remain pending central integration. Proposed first actual lethal flow should use nonempty hotbar+ordinary inventory+armor+offhand, real vanilla keepInventory=false, retained XPlevel>0, real Items/XP and cold restart; source closed/no-cursor profile can be stated explicitly, not inferred from absence.

Validation: standalone rustc --test death_loot.rs; callback-recording nonempty41-slot inventory verifies vanishing then compartment order, drop before current-slot clear, mutation/error prefix; loot recentlyHit snapshot survives callback mutation; XP position snapshot precedes reward mutation, keepInventory short circuit and wrapping base reward. Not actual actor/world execution.

## Controlled original Inventory.dropAll probe (after implementation)
Original signed cmw.Inventory.dropAll method is unchanged and invoked on an original Inventory constructed with36items+4armor+1offhand. Original ItemStack constructors used. A generated Player subclass, allocated without constructing a world/actor, overrides ONLY Player.drop with a controlled recorder/mutator. No method-body modification, real Item entity, RNG, packet or production actor validation.
Normal original trace: dirt1 (main0), callback inserts apple3 into future main1 and replaces current slot with coal9; then apple3, diamond2(main35), boots1(armor0), helmet1(armor3), shield1(offhand0). Every callback observed its actual stack still in the source slot. Even nullable drop return clears current slot afterward; replacement coal is overwritten; final41slots empty.
Failure trace: callback on boots replaces current armor0 with coal12 and throws; previous main drops/clears remain, armor0coal12 + later helmet1/offhandshield1 survive. This directly observes current-slot callback mutation and partial failure prefix in unchanged original loop.
Compile via java --module jdk.compiler/com.sun.tools.javac.Main with compiler AND VM --add-exports java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED; JVM run oracle.InventoryDeathProbe, exit0. Artifacts only scratch/oracle.
Probe source SHA25615684a255067d51aae9209a5ce535bc3bb19341e4000e0e0d2c74774eb9360e5.
Probe output SHA2562cdd2c3ff0d4cbd374670b2831d126926e6c270eb0ed551df531c11dd34a2c3c.

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
