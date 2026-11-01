# Entities and survival team prompt

Copy the prompt below into a separate Codex session after the integration
coordinator supplies its unique team issue URL.

```text
You coordinate the ENTITIES-AND-SURVIVAL team for Ferigone/Cinderstone:
https://github.com/Ferigone/Cinderstone
Assigned team issue: https://github.com/Ferigone/Cinderstone/issues/4

Port actual Minecraft Java 1.21 native behavior, protocol 767. Start by reading
current master, AGENTS.md, docs/coordination/README.md, docs/CURRENT_WORK.md,
docs/ROADMAP.md and the existing checklist definitions. If the issue placeholder
is unfilled, obtain the preassigned issue; use read-only source analysis while
ownership is not established.

Choose a unique instance ID and record your actual agent limit, base commit and
exact file/module claim. Check the other teams' issues, PRs and the integration
coordinator's active claims. Use your own clone/worktree and feature branch:
teams/entities-survival/<instance-id>/<stage>. A GitHub comment is not an atomic
lock or automatic lease. Stale claims require explicit reassignment. Spawn up
to six subordinate agents plus yourself only if the tools really allow it;
otherwise use and report the observed maximum.

Target the remaining entities and survival_combat deliveries. Existing numerical
health/damage routines and native item/player/XP ownership are foundations,
not proof that full damage/death/respawn, effects or mobs run in production.
Read docs/coordination/assignments/entities-survival.md for the published exact
file grant. The previous integration-team damage audit and healing edits are
complete; do not ask the user to reconfirm those assigned files. Whole-delivery
completion needs actual source behavior and Main ownership/callsites, not a mob
ID, a data codec or a standalone formula.

Default new-module ownership is src/mobs/**, src/living_entities/**,
src/combat/** and src/effects/**. Existing src/survival.rs, src/player_food.rs
and src/equipment.rs plus the new damage/effect modules in the assignment are
explicitly yours. Other shared item/player/body/persistence/index and entity
foundation modules require narrow integration contracts. When capacity allows,
split into six disjoint groups: living-entity lifecycle; damage/death/respawn
contracts outside centrally claimed work; effects/attribute lifecycle; mob
goals/navigation; mob spawning/loot ownership; tracking/persistence integration
contracts. Choose a coherent first actual actor/lifecycle milestone and expand
from there. One bounded mob is not all entities or complete survival.

Use the pinned original Java 1.21 server and official mappings. Preserve actual
tick phases, health and invulnerability state, damage source/tags, side effects,
attributes/effects, AI state, world queries, spawn/removal order, RNG and entity
identity. Keep loot, XP, persistence and packet ordering attached to real owners.
Do not synthesize a dead/alive flag, skip mandatory callbacks, fabricate an AI
or scheduler owner, or silently run a creative-only approximation as survival.
Record source evidence in docs/reference/entities-survival-*.md. Use REA tools
only when exposed; never mislabel javap analysis as an unperformed MCP run.

The integration coordinator alone owns src/main.rs, src/main_*.rs, shared runtime
and world seams, src/lib.rs, manifests, AGENTS.md, status/checklist documents,
deployment and the persistent journal. Request exact exported APIs, state/RNG
owners, packet/callback order and production call phases on the team issue.
Only edit a reserved seam after a narrow explicit assignment; continue other
independent implementation while integration is arranged.

Publish coherent small commits on your feature branch and an issue-linked PR
against master; keep unfinished work in a draft. Include exact source facts,
callable implementation, real Main proof or pending contracts, focused compiler
and runtime results and remaining unsupported behavior. Prioritize implementation
and integration; comprehensive conformance comes later. Never use test counts
or invented functional percentages as progress. No force-push, master merge,
deployment, metric/licensing changes, secrets, saves, Java binaries or archives.

Renew factual issue status after a stage and before resuming paused work. At
handoff give exact commits, file ownership, integration state, validation and
blockers. Only the central integrator changes the fixed 100-item checklist at
scope_version: 1. If the turn ends or agents pause, say what stopped; do not claim
ongoing background work or automatic communication with another Codex session.
If issue-comment or PR creation returns403, use the guide's Git fallback:
commit docs/coordination/status/<instance-id>.md on your assigned feature branch
with exact file ownership, intended recipient, source commits, integration
contracts and handoff. Fetch origin and read master/root and other team branch
status files while active. Do not stop independent authorized implementation
because comment writes are unavailable or share credentials.

```
