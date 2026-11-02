# Native integer experience awards for grindstone callbacks

This stage adds genuine experience-orb preparation in the isolated grindstone
worktree. It does not represent integer XP as a furnace recipe reward, grant
points directly to a player or claim a connected production grindstone menu.

## Pinned source observations

Direct JDK 21 `javap -p -c` inspection used the original Java 1.21 server and
official mappings pinned by `fixtures/source-lock.json`: inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`. This is direct
disassembly evidence, not an REA MCP execution or a Java gameplay observation.

| Source method | Mapped owner | RNG and ordering fact |
| --- | --- | --- |
| Grindstone result `getExperienceAmount` | `cqt$4.a(Level)` | Positive non-curse minimum-cost total produces `ceil(total / 2.0)` plus one `Level.random.nextInt(half)`; bound one still draws. Nonpositive totals return zero without a draw |
| Grindstone result `onTake` callback | `cqt$4.a(Level,BlockPos)` | Award integer XP at block center, then level event 1042/data0; input clearing follows the callback |
| `ExperienceOrb.award` | `btc.a(ServerLevel,Vec3,int)` | Split positive remaining XP through `getExperienceValue`, then try to merge or create each piece in sequence |
| `ExperienceOrb.tryMergeToExisting` | `btc.b(ServerLevel,Vec3,int)` | Draw `Level.getRandom().nextInt(40)` before the entity query; use the first matching actual orb, increment its count and reset age |
| `ExperienceOrb` constructor | `btc(Level,double,double,double,int)` | After inherited construction, four samples from the orb's own `Entity.random` set yaw and velocity |
| `Entity` constructor | `bsr(EntityType,Level)` | Create its actual entity RNG and derive the UUID through `Mth.createInsecureUUID`; no global `Math.random` call occurs in this constructor |

`Level.random` is mapped to `dcw.z`; it is distinct from
`Level.threadSafeRandom` (`dcw.f`) and the orb's `Entity.random` (`bsr.ah`).
Integer orb awarding and orb construction do not require a global-Math rounding
draw. The existing native orb constructor retains UUID and yaw/velocity draws
on its actual entity RNG. `random_seed::create_random` supplies that existing
native factory; no hardcoded orb seed or replacement RNG stream is added.

## Retained synchronous preparation

`NativeMenuExperienceStage::new(&LivePlayers)` captures the actual XP owner's
proposal and revision. The cloneable stage supports repeated synchronous calls:

```rust
stage.award(
    players, amount, position,
    &mut next_entity_id, &mut index, &mut level_random,
)?;
```

The supplied index, counter and Level RNG are the caller's one transaction-local
continuation of actual source owners. Each call immediately consumes its merge
group draws, queries the real index snapshot, retains earlier staged merges and
registers newly proposed orbs in source order. Later `getExperienceAmount` calls
therefore observe the preceding award's RNG consumption. Actual locked players
provide chunk visibility, observers and sent-chunk gates; these facts are not
invented from a sorted entity-ID list.

The stage exposes `award_count`, `awards`, `registration_ids` and consuming
`into_awards`. Each `PreparedNativeExperience` checkpoint contains its actual
XP-state proposal and ordered source deliveries, and exposes:

- `registration_states()` for validated `ExperienceOrbRegistration::from_state`.
- `expected_registration_handle(id)` for shared-index generation/order checks.
- `attach_registered(id, handle)` for the genuine mixed batch's XP Registered
  event. An unexpected or repeated handle is rejected.
- `commit_registered_parts(runtime, events)` to install this award checkpoint
  and publish its deliveries only after all of its new XP handles are bound.
  Earlier award handles are rebound from the actual existing XP runtime.

The mixed native batch owns actual item/XP index, tick/tracking insertion and the
single final Level RNG. This mixed checkpoint path does not register the same
orbs again, alter the Level RNG or borrow a fictitious Math owner. Merge-only and
zero awards retain an ordered checkpoint even when they create no entity entry.

Every checkpoint must commit at its actual callback position before the
corresponding level event/input clears. An XP award in result THROW precedes the
later Player.drop; a final batch of all drops followed by all XP changes shared
index generations and encounter order and is not a valid substitute.

## Single-award API and furnace preservation

`prepare_grindstone_experience_after_drops(world, players, block_position,
amount, next_entity_id, index_after_drops, level_after_drops)` prepares one real
integer award at block center. The amount has already been sampled by the
grindstone owner, and its Level RNG input includes that draw and only actual
source predecessors. The token's `commit_parts(runtime, items, events)` validates
the XP revision, pre-award Level RNG and planned shared-index handles, then uses
the actual registration pipeline and installs the resulting Level RNG. No
source-later drop may be moved ahead merely to use this helper.

The existing `PreparedFurnaceExperience` type and furnace entry points remain
unchanged. Furnace recipe rounding still uses its existing optional global-Math
sample before entering the shared integer orb loop. The extracted orb loop
retains the original split/merge/construction and tracking order.

## Validation and integration boundary

Source inspection and `git diff --check` completed. Focused
`cargo check --locked --offline --bin mc121-rust` passed against isolated base
`5553015344f939f6bb255f414439e1c3b022af5a`, with this XP module and the mixed
entity runtime compiled through the existing Stone Main. The check took 13.36s
and used a separate target directory. It does not compile the still-unexported
grindstone Host or establish a production grindstone callsite. No broad tests
were run. The mixed runtime capability and real grindstone Host are independent
assigned implementation areas; their complete ordered commit and Main callsites
must be composed and checked before claiming runtime integration. Unknown world sources,
nonempty unsupported criteria and unsupported Mending pickup contexts retain
their existing explicit guards. No checklist delivery or parity percentage is
advanced by this source stage.
