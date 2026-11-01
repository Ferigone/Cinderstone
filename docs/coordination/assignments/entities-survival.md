# Entities/survival assignment for issue 4

Assigned team: https://github.com/Ferigone/Cinderstone/issues/4
Coordinator: integration-root-20261101
Decision: authorized; no further confirmation is needed for the files below.

## Active claims versus reserved integration

The integration team's earlier movement-statistics, healing reuse and damage
audit are complete and committed through `cde7373665404658807445b5045ca49756f1afd6`.
There is **no active competing implementation claim on survival_combat.2** or
on the core files now assigned to this team. Shared integration ownership is
distinct from an active edit claim.

## Immediate file ownership

Team 4 may implement and publish changes to:

- `src/survival.rs`
- `src/player_food.rs`
- `src/equipment.rs`
- new `src/living_damage.rs`, `src/player_damage.rs`, `src/player_effects.rs`
- its already assigned new `src/effects/**`, `src/combat/**`,
  `src/living_entities/**` and `src/mobs/**` namespaces
- its team reference notes and per-instance branch/status records

Preserve existing callable APIs or include the required compatibility layer.
Port actual source-ordered damage/effect behavior with real-owner callbacks;
do not synthesize missing world, health, loot, RNG or scheduler owners.

## Shared seams still coordinated centrally

`src/main.rs`, `src/main_*.rs`, shared runtime/index/persistence/tracking files
and existing shared entity foundations such as `src/entity.rs`,
`src/entity_base.rs` and `src/entity_state.rs` remain integration seams. They
serve Item/XP as well as players. Publish the exact additional field/callback
patch and call-phase contract needed there; this reservation does not block
independent work in the assigned core/new modules. Mechanical module exports
follow the standing exception in the coordination guide.

`src/item_damage.rs` is ItemEntity-specific; it is not automatically assigned
as part of Player damage. Root retains final checklist, Main and deployment
decisions. Numerical or isolated effect code alone does not complete Main.

## HTTP403 handoff

Do not wait for issue comments or PR creation to become writable. Use the
existing authorized Git connection and your own branch:
`teams/entities-survival/<instance-id>/<stage>`.

Commit `docs/coordination/status/<instance-id>.md` on that branch with exact
owned files, source commits, progress, blockers and requests addressed to the
integration coordinator. Push normal source files. If PR creation fails, the
published branch plus status file is the integration handoff. No credentials
may be put in a prompt, status file or repository. If authenticated Git pushes
also fail, report that separate access blocker and retain local commits.

See [the damage integration contract](../damage-integration-contract.md) and
[the coordination guide](../README.md). No automatic cross-chat wakeup or
background polling is implied.
