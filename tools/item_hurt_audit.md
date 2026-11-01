# ItemEntity.hurt prerequisites — Minecraft Java 1.21

This audit combines original bytecode with **728 executions of the unchanged `cjh.hurt` method**. The executable probes instantiate the actual `cjh` and `cuq` classes. An oracle-only `DamageSource` subclass supplies tag, creative-source and causing-entity answers; the generated world records game-event calls. It does **not** prove native Rust damage handling, real damage-registry loading, special-item destruction effects or packet delivery.

The inner JAR SHA-256 is `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`. Exact query/result vectors are in `fixtures/item-hurt-vectors.json`; focused `javap` excerpts are in `tools/item_hurt_bytecode.txt`.

Reproduce the original executable evidence with `python3 tools/item_hurt_check.py`. The driver verifies the JAR and input hashes, recompiles the helper, checks its original class origin and compares every archived result. It does not regenerate expected vectors or execute native Rust damage handling.

## Generic invulnerability, evaluated first

The inherited `Entity.isInvulnerableTo` returns true for the following short-circuited expression, in this order:

```text
removed
OR (invulnerable AND NOT source.BYPASSES_INVULNERABILITY AND NOT source.isCreativePlayer())
OR (source.IS_FIRE AND entity.fireImmune())
OR (source.IS_FALL AND entityType.FALL_DAMAGE_IMMUNE)
```

A bypass tag or creative player source bypasses the generic **invulnerable flag** only. It does not bypass removal, fire immunity or fall immunity. The original ITEM entity type is absent from the official `fall_damage_immune` entity-type tag. A normal item's fire immunity is its effective `FIRE_RESISTANT` component OR its entity type's fire-immunity fact; ITEM's type fact is false. The vectors retain predicate-query traces to verify early returns and short circuiting.

## Exact ItemEntity.hurt order

1. Reject with false when the generic invulnerability check above returns true.
2. Reject a **nonempty nether star** for an `IS_EXPLOSION` damage source.
3. Reject when `ItemStack.canBeHurtBy(source)` is false. That method rejects precisely `stack.has(FIRE_RESISTANT) && source.IS_FIRE`.
4. On the client, return true immediately. This accepted path changes neither health nor `hurtMarked`, and emits no game event.
5. On the server, call `markHurt`, setting **hurtMarked (`U`) to true**, independently of `hasImpulse` (`av`).
6. Set health using Java's exact operation: **`(int)((float)oldHealth - damageAmount)`**. The subtraction is f32; the result narrows with Java `f2i` rules.
7. Emit **`minecraft:entity_damage`**, using `DamageSource.getEntity()` as the causing entity. The executable world callback observes the new health, `hurtMarked=true` and removal still false here.
8. If health <= 0, invoke `ItemStack.onDestroyed(itemEntity)`, then `discard()`.
9. Return true.

Rejected paths leave any preexisting `hurtMarked` flag untouched. This method does not validate that the amount is positive or finite, and does not itself set `hasImpulse`. It does not itself send a damage packet or emit `ENTITY_DIE`: `discard` differs from the separate `kill` method, which explicitly emits `ENTITY_DIE`.

`ItemStack.onDestroyed` dispatches to the actual item's destruction hook before removal. Container/bundle items can release contents; treating that hook as empty for every item would silently change behavior. Those item-specific effects are outside these probes.

## Numeric consequences observed in the original class

Starting with ordinary stone health 5 on the server, with an accepted generic source:

| Amount | New health | Removed |
|---|---:|---|
| 0 | 5 | false |
| 0.01f | 4 | false |
| 1f | 4 | false |
| 4f | 1 | false |
| 5f | 0 | true |
| NaN | 0 | true |
| +Infinity | -2147483648 | true |
| -Infinity | 2147483647 | false |

Even zero damage marks the entity hurt and emits the damage game event. Very small positive values can round away before narrowing; use the exact f32 operation rather than an integer subtraction or a general rounding rule. The vectors also cover negative and large health, signed zero and subnormal amounts.

## Existing BaseDamage variants

Official signed-JAR damage data and tag resources establish:

| Native variant | Vanilla damage type | Base-tick amount | IS_FIRE | BYPASSES_INVULNERABILITY / IS_EXPLOSION / IS_FALL |
|---|---|---:|---|---|
| OnFire | `minecraft:on_fire` | 1f | true | all false |
| Lava | `minecraft:lava` | 4f | true | all false |

These `DamageSources` have no causing entity and are not creative-player sources. Fire-resistant stacks reject both. Ordinary stone with health 5 accepts OnFire and ends at health 4, or accepts Lava and ends at health 1.

The existing runtime provider must continue returning an explicit unsupported error for damage until the mutation, ordered game event, destruction hook and tracker `hurtMarked` behavior are implemented and tested together. These vectors are prerequisites for that work, not permission to substitute a successful no-op.
