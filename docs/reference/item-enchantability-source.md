# Original 1.21 item enchantability certificate

[The native item certificate](../../fixtures/native-item-enchantability.json)
records the actual original registry's **1333 items**, their enchantment values,
real declaring predicate owners and default/count-one observations. Every
numeric ID and resource key matched `fixtures/items-1.21.json`; the original
registry itself reported size 1333. No row is padded with a default value or an
unknown-item fallback. These are source facts, not completion metrics or proof
of a working native enchanting table, anvil or full enchantment pipeline.

The key distinction is dynamic: `Item.isEnchantable(ItemStack)` is not a static
per-item default boolean, and `ItemStack.isEnchantable()` adds a separate normal
ENCHANTMENTS-component gate. The fixture's observation booleans describe only
the captured stacks. Consumers must evaluate the source rules against the real
current stack and effective components.

## Original acquisition and bounded execution

The original inner-server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official mappings SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`.
Both match [source-lock.json](../../fixtures/source-lock.json). Inspection used
**direct JDK 21 `javap -p -c`** and execution of the pinned original bootstrap,
not an REA MCP execution. The bootstrap capture exited 0 with no stderr output.
No proprietary Java binary, raw disassembly, private path or player/world data
is included in the public fixture or this ledger.

For each exact original item, the capture independently queried:

- Registry numeric ID and resource key, checked against the native catalogue.
- Actual runtime item class, `getEnchantmentValue()` return value and its real
  declaring class, rather than a guessed material-name or tool suffix.
- The declaring class of `isEnchantable(ItemStack)` and the result on an actual
  original `new ItemStack(item,1)` using its genuine prototype.
- The effective count, emptiness, max-stack-size, MAX_DAMAGE presence, normal
  ENCHANTMENTS presence/emptiness and `ItemStack.isEnchantable()` on that stack.
- The actual virtual `item.getDefaultInstance()` and the same gate observations,
  including its genuine declaring owner.

The listed gate observations matched between the count-one constructor and the
actual default instance for every registered item. This is **not** whole-stack
component equality: item-specific default-instance methods may add unrelated
components. AIR requested with count 1 is observed as empty with effective count 0
and empty components. Nonempty prototype component facts were also checked
against the existing original catalogue; missing/mismatched entries fail.

## Exact dynamic predicates and source owners

| Mapped original method | Obfuscated method | Official lines | Source rule |
| --- | --- | --- | --- |
| `Item.isEnchantable(ItemStack)` | `cul.a(cuq)` |401| Effective max-stack-size equals 1 and effective MAX_DAMAGE is present |
| `BookItem.isEnchantable(ItemStack)` | `csr.a(cuq)` |10| Effective stack count equals 1 |
| `EnchantedBookItem.isEnchantable(ItemStack)` | `cto.a(cuq)` |13| Always false |
| `AnimalArmorItem.isEnchantable(ItemStack)` | `cse.a(cuq)` |51| Always false |
| `ItemStack.isEnchantable()` | `cuq.z()` |899–903| Dispatch the effective item's predicate, then require normal ENCHANTMENTS present and empty |
| `ItemStack.isEnchanted()` | `cuq.A()` |911| Normal ENCHANTMENTS nonempty; missing normal component defaults to empty for this different query |
| `ItemStack.isEmpty()` | `cuq.e()` |303| EMPTY singleton, raw AIR item or raw count≤0 |
| `ItemStack.getMaxStackSize()` | `cuq.j()` |414| Effective MAX_STACK_SIZE, with source default 1 when absent |
| `ItemStack.getCount()` | `cuq.H()` |992| Zero when empty, otherwise actual stored count |
| `ItemStack.isDamageableItem()` | `cuq.l()` |422| MAX_DAMAGE and DAMAGE present, UNBREAKABLE absent; not the enchantability predicate |

Official mappings and every registered item expose only the four captured
predicate owners above: 1326 items use Item, five use AnimalArmorItem, one uses
BookItem and one uses EnchantedBookItem. The fixture encodes them as
`default_max_stack_one_and_max_damage_present`, `count_one` and `never`, with
fully qualified declaring names retained per row. Unknown owners must be
explicitly unsupported, rather than assigned one of those rules heuristically.

Empty-stack handling precedes raw-item dispatch. The original effective
`getItem()` returns AIR and effective component/prototype maps are empty when
the stack is empty. Applying a raw tool/book override to count≤0 would be wrong.
For a nonempty base Item, however, its predicate itself imposes no count==1
condition: it reads max-stack-size and MAX_DAMAGE presence. BookItem alone adds
its actual count==1 override. Do not silently clamp an overfull count or import
that book condition into the default rule.

| Component | Protocol ID | Original field | Role |
| --- | --- | --- | --- |
| MAX_STACK_SIZE |1| `kq.c` | Effective value used by the default predicate; absent falls back to 1 |
| MAX_DAMAGE |2| `kq.d` | Effective presence required by the default predicate; do not replace with a damageability query |
| DAMAGE |3| `kq.e` | Not tested by this enchantability gate |
| UNBREAKABLE |4| `kq.f` | Not tested by this enchantability gate |
| Normal ENCHANTMENTS |9| `kq.k` | Must exist and have an empty levels map after the item predicate passes |
| STORED_ENCHANTMENTS |23| `kq.y` | Does not participate in this gate |

Patched component values override prototypes; explicit removal resolves to
absence. `DataComponentMap.has` means its effective get is nonnull. The
MAX_STACK_SIZE default 1 is an actual source component lookup rule; it does not
authorize a fallback for an unknown item or absent certificate. MAX_DAMAGE's
numeric value, current damage and UNBREAKABLE do not change the default Item
predicate. Codec, component validity and native ownership admission remain
separate obligations; this does not certify arbitrary malformed patches.

`ItemEnchantments.isEmpty()` (`dai.d`, line 125) checks the actual levels map's
emptiness, independently of tooltip visibility. A scan for positive levels is
not the same operation. Missing normal ENCHANTMENTS makes `isEnchantable` false,
even though the separate `isEnchanted` query also returns false. Testing only
`!isEnchanted()` would therefore admit unsupported stacks incorrectly.

## Enchantment values are a separate captured property

| Actual declaring owner | Method / official line | Value source |
| --- | --- | --- |
| Item | `cul.g`,413 |0|
| BookItem | `csr.g`,15 |1|
| FishingRodItem | `ctx.g`,50 |1|
| ProjectileWeaponItem | `cvn.g`,48 |1|
| TridentItem | `cwn.g`,171 |1|
| MaceItem | `cvb.g`,63 |15|
| ArmorItem | `csf.g`,142 | Actual captured armor material's enchantment value |
| TieredItem | `cwj.g`,17 | Actual captured tier's enchantment value |

All values in the fixture were obtained by invoking the actual registered
item's virtual no-argument method, including its material/tier owner. Zero is a
real observed value, not an unknown/missing sentinel. Value and dynamic
predicate are independent source facts. A consumer must not equate either
`getEnchantmentValue()>0` or a default observation with full stack enchantability.

| Actual captured item | Value | Declaring predicate | Prototype count-one gate |
| --- | --- | --- | --- |
| Book 925 |1| BookItem count-one | true |
| Diamond sword 838 |10| Item effective components | true |
| Wolf armor 797 |10| AnimalArmorItem never | false |
| Leather horse armor 1127 |15| AnimalArmorItem never | false |
| Enchanted book 1114 |0| EnchantedBookItem never | false |
| Shears 983 |0| Item effective components | true |

The last example shows why an original value 0 must remain distinct from an
absent certificate. The different table/selection/offer algorithms own their
subsequent use of that value; this fixture does not implement those algorithms.

## Consumer contract and limits

`schema_version:1` identifies the public data layout. `provenance` pins the
original hashes, native catalogue digest, actual registry size and capture.
`items` contains exactly one row per original numeric ID/resource key, including
all required typed values and declaring owners. Default/count-one observations
are supplementary evidence; they cannot substitute for dynamic evaluation.
Changed registries, item classes/owners, unknown component semantics or missing
rows require explicit unsupported handling. There is no permissive or zero
fallback for unknown source facts.

A native enchanting or anvil caller still needs its actual effective-component
owner, normal enchantment-map semantics, current stack count/emptiness and
correct source call phase. This certificate supplies no fake ItemStack owner,
enchantment HolderSet, selection/RNG algorithm, registry encounter order,
menu/XP/book callback or Main integration. In particular, actual enchantment-map
identity/iteration obligations are not satisfied by sorting the captured keys.

No arbitrary patched-stack runtime matrix, native Main enchanting/anvil flow,
public/graphical-client result, performance advantage or aggregate checklist
completion is established. Source rules prove which observations can change;
the bounded original bootstrap establishes the registered default facts.
