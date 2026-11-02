# Main anvil item and actual-owner context

This note covers `main_anvil_context.rs`, the narrow Main assignment on
issue3 comment6564589182. It does not establish production invocation by itself.
The runtime/dispatch/lifecycle/pickup modules own their separately assigned
callsites. The required controller dependency is PR76's staged
`AnvilMenuHost::{begin_stage,publish_stage}` and actual enchantment-session
interfaces, rather than the older master no-merge controller.

## Primary evidence

The original Java1.21 server inner JAR has SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
its official server mappings SHA1 is
`31c77994d96f05ba25a870ada70f47f315330437`.
Evidence here uses JDK21 `javap -c -p` and a small original-bootstrap Java
reflection probe, not REA or a decompiler. Java files, class files, JARs, the
language asset and raw disassembly remain outside the repository.

* `Item.isValidRepairItem` (`cul.a(ItemStack,ItemStack)`, original line417)
  returns false. Original runtime reflection confirmed this is the inherited
  method for bow, crossbow, fishing rod, flint and steel, shears, brush and
  trident, as well as the table's simple materials, stone and paper.
* `TieredItem.isValidRepairItem` (`cwj.a`, line22) consults the actual tier's
  repair ingredient, then its false superclass. `Tiers` (`cwk`) suppliers use
  iron ingot, diamond, gold ingot and netherite ingot for the corresponding
  tiers. Wood and stone use PLANKS and STONE_TOOL_MATERIALS tags. Those two
  tag-dependent branches are explicit unsupported contexts in this milestone.
* `ArmorItem.isValidRepairItem` (`csf.a`, line151) consults its actual armor
  material holder's ingredient. Original `ArmorMaterials` (`csh`) suppliers
  bind leather; iron ingot for chain and iron; gold ingot; diamond; turtle
  scute; netherite ingot; armadillo scute. Runtime reflection confirmed wolf
  armor (`cse`) inherits this method. These fixed source bindings are ported;
  unrelated item-name prefixes cannot enter them because the item must first
  belong to the finite captured original set.
* `ElytraItem.isValidRepairItem` (`ctm.a`, line26) tests phantom membrane;
  `MaceItem.isValidRepairItem` (`cvb.a`, line107) tests breeze rod.
  `ShieldItem.isValidRepairItem` (`cvt.a`, line61) needs actual PLANKS and
  therefore returns an explicit unsupported-context error here.
* `ItemStack.getHoverName` (`cuq.w`, lines725–733) chooses CUSTOM_NAME5,
  ITEM_NAME6, then `Item.getName(stack)`. `Component.getString` visits content
  and siblings without style. The port preserves literal UTF-16 units, including
  isolated surrogates. Nonliteral contents need their actual text owner and
  are rejected. A colored shield's component50 changes its description key
  (`ShieldItem.getDescriptionId`, lines30–34); the plain Shield name is never
  silently used for that case.
* `Language.loadDefault` (`tw.c`, lines39–48) loads the JAR's real
  `/assets/minecraft/lang/en_us.json`; it is not generally correct to return a
  translation key. After the unchanged original `SharedConstants.tryDetectVersion`
  and `Bootstrap.bootStrap`, reflection observed each of the77 rows' runtime
  class, declaring repair method and actual
  `ItemStack.getHoverName().getString()`. Only those functional name facts
  are retained as source constants. No whole language file is copied.
  A second unchanged original-Java invocation directly confirmed the fixed
  repair predicates across all admitted material families, elytra and mace,
  plus inherited-false and wrong-material negative cases.
* `AnvilMenu.onTake` and its `ContainerLevelAccess` lambda (`cpw`) call the
  current block owner. Actual creative instabuild short-circuits wear and the
  player RNG draw and emits level event1030/data0. The host reads the captured
  current actual block, stages that event, and explicitly rejects noncreative
  wear, destruction or XP mutation instead of inventing a substitute owner.

The77 name observations include the seven inherited-false damageable families,
all six tool tiers and six armor families, elytra/mace/shield/wolf/turtle,
needed repair materials, stone and paper. Unknown default names fail; arbitrary
items with supported literal CUSTOM_NAME/ITEM_NAME can still resolve those
owned component values. This finite service represents the certified native
bootstrap/default-language profile, not an injected Language or reloadable
Item registry.

## Actual transaction contract

`NativeAnvilBinding` captures actual player id/UUID, Overworld dimension,
block position and external window id. The controller additionally retains its
opaque nonreused UI token/revision. The actual outer transaction holds the
current native world/player/container owners; a copied inventory or a recycled
protocol window is not sufficient identity.

`NativeAnvilHost::new(binding, access, inventory, player_random, instabuild,
experience_level, block, gameplay_certificate, publish)` receives actual
captured owner values. `instabuild` comes from the real lifecycle abilities and
`experience_level` from `PlayerLifecycle.experience.level`, even in creative.
`ActualAnvilAccess` is captured from the real Level/block/player reach helpers;
it is an immutable observation, not another world or an invented block entity.

`NativeAnvilGameplayCertificate::new(&actual_gameplay)` verifies Main's existing
native empty-criteria/default-listener floor and records that exact ephemeral
owner. Before actual publication, the runtime callback must call
`certificate.validate(&same_actual_gameplay)` under its existing lock, validate
actor/binding/UI/canonical inventory owners and prepare every fallible
ItemEntity/stat/listener/world/packet effect. The certificate does not create
an advancement registry and must not survive moving or replacing that owner.

`begin_stage` copies every mutable proposal: canonical inventory and its real
transient counter, player RNG, actual XP level/ability flag, current block and
ordered event log. It shares only immutable access/item-method observations and
an ephemeral synchronous one-shot publication capability. That `Rc` is never
stored in `LivePlayers` or retained across `await`; the pure item service is
`Arc<dyn NativeAnvilItemMethods + Send + Sync>`.

`publish_stage` checks the binding/token/capability and component admission
before calling the one-shot real publisher. An error must leave actual
menu/item/stat owners unchanged. In Main's approved same-owner composition,
the publisher prepares all fallible work and seals the one actual commit
capability while the actual SharedContainer retains its rollback copy. The
controller then installs its already-validated UI infallibly; the outer locked
transaction reinserts that UI and immediately applies the sealed actual-owner
commit infallibly. No owner mutation or packet preparation that could fail may
be deferred past this point. Packet records stay private until UI adoption;
runtime queues them in the existing single `PlayerEvent` FIFO under its commit lock. The host
adds no writer or network drain, and a later terminal transport close never
rolls back a completed source mutation.

`NativeAnvilEvent` records `Sync`, `InputChanged`, `InventoryChanged`, `Drop`
and `LevelEvent`, each with exact `AnvilPhase` and an inventory snapshot.
`InventoryChanged` captures the inventory **before** its one
`Inventory.setChanged` increment, then increments the same detached inventory
counter. Runtime must replay the actual source increment once and install the
validated final counter without adding a second increment. Sync/listener
observations see source intermediate input/result/cursor states. Input changes
are certified original default-container callbacks, not invented listeners.
Drops remain owned `DroppedStack` values with actual `around`/`trace_item`
arguments; only the real runtime allocates their entities, RNG, IDs and stats.
`add_to_player_inventory` delegates to the existing native Inventory.add using
a detached `PlayerContainer` wrapper, retaining its result/remainder semantics
without changing the retained window0 cursor/grid/sync owner.

The inherited enchantment snapshot is explicitly `None`. The controller kernel
can preserve copied base enchantments, but this Main milestone deliberately
rejects surviving nonempty normal9/stored23 maps in canonical inventory,
inputs/preview/cursor and client predictions, including nested item components
29/30/52. Preserving encoded bytes does not establish canonical holder identity
for menu equality. `require_anvil_stack_context`,
`require_anvil_inventory_context` and `require_anvil_prediction_context` expose
this boundary with typed `UnsupportedAnvilEnchantmentContext`; empty maps with
their tooltip flag contain no holder keys and remain admitted. An incoming
enchanted-book merge still requires the canonical configured registry, actual
holder/map encounter order and source session, so it errors while that
capability is absent. No holder IDs, costs, compatibility or empty registry
are fabricated. Multi-slot QuickCraft likewise
errors without the actual Slot-identity HashSet encounter-order owner;
single-slot source recursion remains available. Source creative profile guards
mutable initialization/rename/click; broadcast and removal can use their
captured real ability flag without pretending an unknown actor is creative.
Food20/nested identity contexts use the existing actual component admission
rule; they are never normalized by changing a live stack.

## Focused validation

The original-Java probes above observe class/method/name and repair facts directly. The
native tests in this file are source-derived flows, not original-Java menu
execution: finite fixed item repair/name predicates, unsupported tag/name,
colored-shield and nested enchantment-identity boundaries, lossless literal siblings, actual-owner gameplay
certificate rejection, and a real native anvil controller over a FlatWorld /
PlayerState fixture. The controller flow checks repair output, phase/cursor
observations, creative actual XP/RNG preservation and late-publication failure
without menu/inventory/revision mutation.

The external combined integration probe (fresh Root005e source, PR76's staged
core, the five proposed anvil modules and agreed temporary Main seams) passed
`cargo test --bin mc121-rust main_anvil_context::tests -- --nocapture`:
all three tests actually ran and passed. The log remains outside the repository
at `/tmp/ws-anvil-context-focused.log`. This is external probe evidence, not
unchanged-master compilation or production Main invocation. Full Main
open/click/close/network invocation remains the integrator's separate
production verification.
