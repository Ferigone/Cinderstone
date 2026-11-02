# Actual configured native enchantment owner

`src/main_native_enchantments.rs` is a shared, bounded registry/item-query
helper for future Enchanting and Anvil integrations. It exposes no production
menu path and creates no table, actor, world, XP owner or effect evaluator.
Existing Anvil computation still explicitly rejects enchantment combining;
this helper does not relabel that branch as implemented.

## Configuration and source identities

The constructor requires the actual `Fixtures` plus an explicit
`NativeItemEnchantmentCertificate`. `pinned()` binds the complete original-class
certificate SHA-256
`828c99f328a9dc684e66582ed07c26c2417a99f3df5dc364b6a0710d0f5e8da5`,
original 1.21/protocol 767 artifact identities, the exact compiled item-catalogue
digest and every dense item ID/name. Each original enchantment value and
predicate comes from that inspected certificate. Missing facts are errors;
observed zero is a genuine value. Declaring predicate owners are checked against
the four source classes. The ordinary JSON fact file is
`fixtures/native-item-enchantability.json`; no Java artifact is distributed.

The helper locates the same configured RegistryData, tags and enabled-feature
packets Main sends to clients. All42 enchantment IDs/keys must match the native
Slot holder domain in `crafting_metadata.json`. Actual definition NBT supplies
weight, maximum level, minimum/maximum cost, anvil cost, supported and optional
primary item HolderSets, and the exclusive enchantment HolderSet. Actual named
sets resolve through the configured item/enchantment tags; direct holder lists
retain their order. Missing referenced tags or omitted known-pack definition
payloads are errors, never a static or permissive substitute.

The actual `in_enchanting_table` tag retains its configured encounter vector;
an absent optional tag returns `None`. Snapshots include the stable actual
decoded registry owner, exact relevant packet bytes and the required item
certificate digest. The registry retains this owner through an `Arc`; cloning
the registry or a snapshot preserves it, while a second construction has a
different identity even when its packet bytes match. No borrowed Fixtures
address is retained or treated as a permanent owner ID. A holder is bound to
this same live registry and its resolved protocol ID. Different owners reject
each other's equal-looking holders.

An eventual Main runtime must construct and retain this canonical registry once
and lend its clones to the relevant menus. `require_configuration(actual_fixtures)`
checks that the current relevant registry/tag/feature packet bytes still match
the captured owner. This check does not reconstruct a registry or make another
owner's holders interchangeable. `tag_holders` preserves actual configured
encounter order and distinguishes an absent tag from a present empty one;
`holder_is_in_tag` validates holder ownership before membership lookup.

## Verified Java behavior

Direct JDK21 `javap -p -c` inspected the pinned inner server SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`
and official mappings SHA1`31c77994d96f05ba25a870ada70f47f315330437`.
This is direct JVM analysis, not REA MCP execution. Raw disassembly and Java
binaries are not part of the repository.

`Enchantment` (`dac`) and `Enchantment.Cost` (`dac$b`) establish:

- Cost is `base + perLevel * (level - 1)` with signed Java-int wrapping.
- Primary applicability requires supported-item membership **and**, when a
  primary set is present, membership in that set too.
- Compatibility requires different actual holders and excludes either
  direction's exclusive-set membership.
- An absent optional `exclusive_set` has the original empty direct set.

Item HolderSet membership follows the effective item holder even for an empty
stack (AIR), rather than inventing an empty=false rule for a configured set
which could contain AIR. This is separate from the empty component view and
false Item enchantability predicate.

The default Item predicate (`cul.a(cuq)`) requires effective max-stack-size1
and MAX_DAMAGE component2 presence. It does not test count, DAMAGE presence,
UNBREAKABLE or generic `isDamageableItem`. Book (`csr`) requires count1;
AnimalArmorItem (`cse`) and EnchantedBookItem (`cto`) return false. The required
certificate selects the actual declaring predicate/value for each item.
Empty stacks dispatch to AIR with an empty component view. The registry trait's
`item_is_enchantable` implements that declaring Item predicate;
`stack_is_enchantable` additionally reads and validates the actual normal
ENCHANTMENTS9 map and requires it to exist and be empty, matching
`ItemStack.isEnchantable`. The Enchanting core performs the same separate stack
gate. STORED_ENCHANTMENTS23, normal-enchantment tooltip visibility, DAMAGE3 and
UNBREAKABLE4 do not replace it. A removed normal component remains absent.

`ItemStack.enchant` (`cuq.a(Holder,int)`) calls
`EnchantmentHelper.updateEnchantments` (`dae.a`). The target is component23
only for the actual enchanted-book item, otherwise9. An absent target component
returns EMPTY without invoking the mutable-map consumer or changing the stack.
The mutable constructor preserves tooltip state. Upgrade ignores nonpositive
levels, clamps a positive request to255 and keeps `max(old,new)`; it does not
clamp to the enchantment's declared maximum. Existing level0 entries remain
legal. Unrelated components remain intact, and only the existing original-
stream normalization certificate can elide a redundant prototype patch.

## Reference-map order boundary

`ItemEnchantments.Mutable` (`dai$a`) creates a default
`Object2IntOpenHashMap`, copies the original map with `putAll`, then upgrades.
The actual `Holder.Reference` class (`jm$c`) does not override Java object
equality/hash. Its encounter order cannot be reconstructed from sorted protocol
IDs or assumed equal to input wire order.

Empty/single-holder resulting maps have unambiguous order and are supported
directly. A multi-holder mutation requires a supplied
`NativeEnchantmentMapOrder` capability bound to this exact registry snapshot
AND the actual canonical or genuinely staged component-map instance/history.
Its mandatory `require_component_owner(item, component)` cannot be satisfied by
comparing wire bytes or level values. Source-order iteration receives that real
item/component plus values for lookup. Mutable upgrade additionally receives the
actual bound holder and original requested level, including nonpositive levels,
so it can preserve the source default-map/putAll/upgrade construction sequence.
The helper verifies the returned IDs are exactly the resulting map permutation.
No implementation of this required capability, identity-hash source or sorted
fallback is supplied here. Multi-holder operations therefore remain unsupported
until their actual owner is installed. An unchanged value map or a nonpositive
upgrade does not authorize inventing its mutable-map encounter order. Failure
occurs before changing the actual stack. Source-order iteration has the same
requirement; an anvil cannot choose conflicts by sorted IDs.
`enchantment_values` exposes value lookup only; its map ordering authorizes no
source iteration. `enchantments_in_source_order` rejects an unresolved actual
reference-map owner. Applicability and compatibility queries do not invent one.

## Integration boundary

The helper's queries consume no RNG and invoke no player/world callback.
An actual Enchanting host must still own bookshelf/chunk queries, player level
and enchantment-seed mutation, criteria/statistics, real menu cursor/inventory,
source callbacks and sound RNG/publication. An actual Anvil host still owns
repair/text rules, its genuine enchantment-combining implementation, player
payment and block-damage callbacks. Unsupported item effects, custom/reentrant
listeners and component identity remain the owning Main profile's admission
responsibility. Canonical player seed/NBT admission is supplied separately by
its corrected PR87 source. Same-TCP reconfiguration is a new saved-data load;
it does not invoke respawn `restoreFrom`.

Recovered source was inspected and `git diff --check` passed. Mechanical module
exports and the focused ordinary compiler composition belong to Root; no
compiler or gameplay observation is inferred from this file's presence. This
source stage claims no working EnchantingMenu, Main path or checklist delivery.
