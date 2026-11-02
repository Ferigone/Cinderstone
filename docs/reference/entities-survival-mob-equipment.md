# Mob equipment/death/base-XP source ledger — 2026-11-02

Agent: `/root/mob_spawning`, instance `es-20261101-1425-a7c3`. Root owns Git, exports, docs and shared/Main integration. Exact implementation claim: issue #4 comment `6566070764`, only `src/mobs/equipment_drops.rs` in `/workspace/cinderstone-mob-owners-es-20261101-1425-a7c3`. Root supplied fresh master base `26c4d2e`; this agent made no Git mutations. No constructor/MobState/inventory/RNG/entity/AI/loot-engine owner is created.

## Evidence

Direct original `javap` inspection, not an REA/MCP run. Original Java 1.21 server SHA1 `450698d1863ab5180c25d7c804ef0fe6369dd1ba`; official mappings SHA1 `31c77994d96f05ba25a870ada70f47f315330437`; inner jar SHA256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.

Scratch original disassemblies:

- `oracle/mob-spawn-owner.javap` (Mob/btp full code and original line tables), SHA256 `a5bbd458027dc2e4eec8b0e49e4854ca119b54bed5c1a4ceb7928c8084fcbb22`.
- `oracle/living-lifecycle.javap` (LivingEntity/btn), SHA256 `a5b9ef74dfc3c8b2402af87c06a14f9f75a65edb6aafc9ff88741bf199669411`.

The APIs reuse actual public `crate::equipment::EquipmentSlot`, whose seven source-order variants are MainHand, OffHand, Feet, Legs, Chest, Head, Body. They do not change the shared equipment codec.

## Bounded callable implementation

- `drop_custom_death_loot`, Mob989–1014. Complete inspected LivingEntity parent1451 is literally empty and is implemented as a private explicit empty body. For each ALL7 slot: retain actual canonical stack, raw chance, zero skip; capture raw `>1` guarantee; causing `DamageSource.getEntity` LivingEntity then current actor Level/ServerLevel check then real enchantment processing, even before empty/predicate checks. Passed ServerLevel and current actor Level remain distinct values. Accepted draw uses canonical retained Entity.random; unguaranteed damageable mutation retains first maxDamage, OUTER random receiver then INNER receiver, second maxDamage, inner nextInt(max(remaining-3,1)), outer nextInt(1+inner), set damage firstMax-outer with Java wrapping. Spawn actual retained stack before clearing actual virtual slot.
- `get_base_experience_reward`, Mob324–343. Original xpReward rereads and nonpositive fast path; live armor list before live hand list before body. Nonempty raw chance<=1 gives actual retained Entity.random nextInt(3)+1; wrapped int arithmetic. No orbs or post-base enchantment/XP owner is supplied.
- `drop_from_loot_table`, Mob527–529. Mandatory real inherited loot engine callback precedes clearing only Mob.lootTable; lootTableSeed is preserved, including inherited-call failure.
- `set_item_slot`, Mob975–985. Actual verify before primitive hand/armor/body replacement, then actual onEquip callback with canonical old/new handles. New stack is live during callback and remains replaced if the callback fails.
- `get_equipment_drop_chance`, Mob1017–1020, `set_drop_chance`, Mob1224–1229, `set_guaranteed_drop`, Mob646–651. Primitive actual raw hand/armor/body fields; no IEEE/clamp normalization; guaranteed writes2.0.
- `set_item_slot_and_drop_when_killed`, Mob640–643. Complete virtual setItemSlot, then virtual guaranteed drop, then persistenceRequired=true, stopping on error without rollback.
- `drop_preserved_equipment_if`, Mob1035–1052. Actual local Java HashSet handle is provided by mandatory owner. ALL7 retains canonical item; nonempty predicate false adds slot to returned rejection set, rather than to an invented all-undropped set. Accepted chance promoted float->double and >1 clears actual virtual slot BEFORE spawning retained item. No enchantment or RNG shortcut is introduced.
- `drop_preserved_equipment`, Mob1025–1026. Exact always-true source lambda and ignored returned set, not a default gameplay predicate.

All fallible host callbacks retain completed real mutations/draws and canonical object lifetimes. No rollback occurs on a later callback error.

## Verification and limits

`rustfmt` ran on only the owned file. Focused external rustc test harness uses actual public `EquipmentSlot` and existing actual `LegacyRandomSource` through the compiled repository library; the canonical equipment/world/enchantment/HashSet recorder is explicitly controlled test code, not a production actor or original-Java runtime probe. Tests passed for raw guarantee before enchant/empty checks, ALL7 order, nested original RNG receiver/durability draw order, actual canonical item retention and opposite spawn/clear orders, partial failure prefix preservation, armor->hands->body XP/IEEE/overflow cases, setter/onEquip/persistence order and custom table clearing/seed preservation. Separate non-test standalone library compilation succeeded. No full Cargo run was made by this agent; root owns serial full-crate compilation.

Harness: `equipment-drops-harness-nov2.rs`, SHA256 `56c9098a2187ca465336c672794369c745ddfe11cc951935b75486c8303b5b90`; test executable `equipment-drops-tests-nov2`; non-test library artifact `libequipment-drops-nov2.rlib`. Toolchain: scratch rustup stable rustc1.99.1; compiled native library dependency is `libmc121_rust-0f71457713b7cad7.rlib` from prior actual crate build. Source implementation SHA256 at handoff: `0e08b4cabd0f1652e1ee63f446f745298c01e758057b9413a3733161d53531bd`.

Independent `/root/living_lifecycle` read-only audit against the unchanged original btp reported no material source mismatch. Reviewer confirmed raw guarantee/current-vs-passed Level, original handles/RNG receiver order, XP arithmetic/order, opposite death/preserved spawn-clear order and all real external owners. This is bounded source-component approval, not Main runtime proof.

Remaining integration: actual Mob actor constructor/stores/metadata, ItemStack and EMPTY identities, body/hand/armor arrays and verified onEquip side effects, retained Entity.random, source/level/enchantment engine, actual HashSet and predicate context, real superclass loot table engine, actual item entity RNG/UUID/ID/world add/tracking, removal/death/save ownership, full XP dispatch and actual Main production callsites. The new source code implements no actor scheduler, ordinary-mob AI, loot table evaluator, item entity constructor or persistence codec, and does not claim a complete entity/survival delivery. No original-Java equipment runtime probe was performed; previous finalizeSpawn/Bat probes do not establish these bodies' runtime integration.
