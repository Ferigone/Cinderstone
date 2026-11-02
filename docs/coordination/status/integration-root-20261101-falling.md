# Central FallingBlock source handoff

Instance: `integration-root-20261101`; state: published for review; bounded runtime passed.
Queue: [#1](https://github.com/Ferigone/Cinderstone/issues/1); related teams: [#3](https://github.com/Ferigone/Cinderstone/issues/3), [#4](https://github.com/Ferigone/Cinderstone/issues/4).
Target: Minecraft Java 1.21, protocol 767, DataVersion 3953.

## Exact source boundary

| Worktree | Branch | Evidence boundary |
| --- | --- | --- |
| `Cinderstone-falling-stage` | `teams/integration/integration-root-20261101/falling-block` | Initial source `a204254`; current-master composition `a8cc5a766a0ad7ba6217fa56d73ef0760e3f5b5c`, tree `e2e398445e35b958a713c24588a5691a96d9e770` |
| `Cinderstone-falling-next` | `teams/integration/integration-root-20261101/falling-next` | Separate future source, based on the same checkpoint; its later working changes are outside the primary validation |

The initial source passed a focused compiler check. The composed primary passed all-target compilation23.94s, executable build37.04s and the [actual Anvil/falling/menu/restart flow](../../validation/falling-anvil-flow.json) at2026-11-02T01:30:57Z. Published PR97 head `c6d61702` has exactly the composed Git tree. Mid-flight restore remains unverified after a separate execution-review interruption. No Falling deployment or checklist delta is asserted.

## Implemented Main calls and real owners

| Entry | Callable source and ownership |
| --- | --- |
| Startup | `NativeFallingDamageTypes::capture` binds the actual configuration; `MainBlockRuntime::install_falling_owner` installs that retained holder owner |
| BLOCK scheduler | `owns_anvil_tick` admits the actual two-tick Anvil obligation; `tick_block_world` rereads Block identity and calls `tick_native_anvil_block_locked` |
| Construction | `NativeAnvilScheduledHost` constructs with the shared ID counter and actual Entity RNG, performs original-fluid flags3 replacement, then attempts real UUID admission and configures the same object |
| Shared level tick | `simulate_native_entities_with_player_menus` invokes `tick_native_falling_block_locked` at the actual captured Item/Player/XP/Falling list position; the canonical Box stays retained through physics/world callbacks |
| Tracking | `publish_falling_tracking` uses the actual manager range predicate and `MainFallingBlockRuntime::publish_one`; shared registration/move/discard APIs update the genuine index, tick and sender owners |
| Persistence | `EntityCheckpoint::capture_with_native_entities` captures Item/XP/Falling bodies and `PersistentEntityOrder` atomically; `restore_native_entity_cohort` restores genuine bodies/fresh handles before visibility |

Core exports are `falling_block::{state,persistence,tick,damage}`. Snapshot,
constructor, movement, landing, damage and NBT bodies are callable source;
their host capability boundaries remain explicit. The original Anvil block
kernel from PR83 is reused. The ordered plan schedules onPlace/updateShape
attempts once rather than adding a duplicate placement-tail attempt.

## Strict supported profile

The concrete primary path admits actual Anvil actors in the certified native
flat Overworld, with the pinned configuration, quiescent callback owners and
current alive, unmounted creative player profile.
It preserves real collision/state queries, retained per-entity RNG, Anvil wear,
tracked block updates, level events and source-ordered Item drops. Active
fluid/fire/redstone callbacks, generic non-Anvil falling behavior, block-data
map ownership, passenger graphs and survival hurt callbacks remain restricted
or unsupported. Missing owners produce explicit errors; no dummy actor,
fabricated settled world or rewritten checkpoint history substitutes for them.

Native resume captures real registration/section/tick order but starts sender
baselines, live players and chunk readiness fresh. A present atomic cohort,
including EMPTY, wins over legacy split-file XP data. The old migration path
does not claim recovered historical mixed order.

Source evidence is direct pinned `javap` and explicitly identified original
fact extraction, not REA MCP execution. See
[runtime owners](../../reference/falling-block-runtime.md),
[state/persistence](../../reference/falling-block-state.md) and
[production contracts](../../reference/falling-block-production-contract.md).

## Publication and next ownership

Corrected seed PR87 merged as `904ae365`. PR84 merged as `f19197a` and its native offline Railway startup was verified. PR97 has its own bounded local Anvil runtime proof and still awaits review/merge/deployment; startup is not public PLAY proof.

Root reserves the primary shared Main/runtime/index/persistence/export files.
The three independent coordinators must retain their existing team namespaces;
this handoff grants no overlapping edits. Future ordinary falling classification,
spawn/water callbacks, codec, placement obstruction and enchanting prerequisites
are separate, narrowly assigned work in `Cinderstone-falling-next`. They passed a separate compiler check and are not runtime-tested or merged primary evidence. Further primary changes require an explicit
Root handoff. No background execution after a stopped session is implied.
