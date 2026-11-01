# Parallel Codex teams

This workflow coordinates independent Codex sessions through GitHub. Sessions
do not automatically share chat messages, local files, agent state or tool
sessions. Each coordinator must read and update its assigned issue and PRs.
The integration coordinator remains the owner of `master`, final integration,
checklist accounting, Railway deployment and the persistent progress journal.

Integration queue: [issue #1](https://github.com/Ferigone/Cinderstone/issues/1).

Assigned team issues: [world generation #2](https://github.com/Ferigone/Cinderstone/issues/2),
[workstations #3](https://github.com/Ferigone/Cinderstone/issues/3),
[entities/survival #4](https://github.com/Ferigone/Cinderstone/issues/4).
Published world-generation, workstation and damage/effect branches and actual
issue2–4 check-ins are now observed. Draft PRs5/workstations and6/worldgen have progressed through their owners' reviews/merges; source publication does not establish Main calls. Later issue updates record exact stages.

## Start one independent instance per team

| Team | Copyable prompt | Primary checklist targets |
| --- | --- | --- |
| World generation | [world-generation.md](prompts/world-generation.md) | Main integration of `world_generation.1` and `.2`; complete delivery `.3` |
| Workstations | [workstations.md](prompts/workstations.md) | `inventory.4`, `crafting.2`, `crafting.5` |
| Entities and survival | [entities-survival.md](prompts/entities-survival.md) | Remaining `entities` and `survival_combat` deliveries |

Each prompt needs a **different, preassigned GitHub team issue**. The integration
coordinator creates those issues and replaces the issue placeholder before the
prompt is copied into a new session. An unfilled placeholder is not an issue or
an ownership claim. Do not start overlapping edits without an assigned issue.

Use one coordinator plus up to six subordinate agents **when that instance's
actual tools and limits allow it**. Inspect the available capacity first and
use the real maximum. Seven agents per instance is a requested ceiling, not a
guarantee of capacity or proof that any agents are running. Do not claim that
another session has started work merely because its prompt has been prepared.

## File ownership and shared seams

Use a separate clone or worktree per independent session. Choose a unique,
branch-safe instance ID once, for example `worldgen-20261101-a7f3`. Record it on
the assigned issue, then branch from the current `origin/master`:
`teams/<team>/<instance-id>/<stage>`. Never reuse another instance's checkout,
branch, Git index or credentials.

An assigned issue already authorizes independent work inside that team's unused default areas. Post the factual claim and proceed when there is no
conflict; do not wait for another user approval. Reserved seams and existing
conflicting claims still require a narrow assignment or handoff.

The team issue records the exact file/module ownership for each stage. These
defaults keep the three teams apart; an explicit confirmed assignment controls
any exception:

| Team | Default implementation area | Team-owned reference notes |
| --- | --- | --- |
| World generation | `src/density.rs`, `src/density/**`, `src/noise.rs`, new `src/worldgen/**`; generator-only fixtures with a team-specific prefix | `docs/reference/world-generation-*.md` |
| Workstations | New `src/workstations/**`, new `src/container_workstation_*.rs`; explicitly claimed special-recipe sections of `src/crafting.rs` | `docs/reference/workstations-*.md` |
| Entities and survival | New `src/mobs/**`, `src/living_entities/**`, `src/combat/**`, `src/effects/**`; existing entity/survival modules only after an exact file claim is confirmed | `docs/reference/entities-survival-*.md` |

A narrow standing exception permits only adding `pub mod` declarations in
`src/lib.rs` for a team's newly implemented namespace; list those mechanical
exports in the PR. Do not edit other exports or shared implementation there.

The integration coordinator reserves `src/main.rs`, `src/main_*.rs`, shared
runtime/world ownership seams, `src/lib.rs`, Cargo manifests/lockfile,
`AGENTS.md`, the top-level README/roadmap/current-status documents, deployment
configuration and the progress journal. Existing player/container/furnace
foundations are not automatically assigned to the workstation team. Existing
damage and entity foundations are not automatically assigned to the entities
team. Read active claims, including work by the integration coordinator.

When a reserved seam must change, post an **integration contract request** on
the team issue: required exported types/signatures, actual state/RNG/scheduler
owners, production call phase, affected files and expected observable ordering.
The integration coordinator implements it or explicitly assigns a narrow patch.
Keep independent implementation moving while that request is resolved. Do not
silently edit a reserved file to make a local build pass.

## Claims and communication

At startup and before expanding a stage, inspect all three team issues and
their open PRs. Post a claim containing the instance ID, base commit, named
delivery IDs, exact files/modules and intended bounded behavior. Confirm that
the assigned issue and current file owners agree before editing shared areas.
For a team sharing one checkout, assign disjoint files to its agents and let
only its coordinator perform Git mutations.

Renew the claim with a factual issue comment after a coherent stage and before
resuming paused work. State `claimed`, `implementing`, `ready_for_integration`,
`blocked`, `paused`, `handoff` or `completed`. A comment or timestamp is **not**
an atomic lock or automatically expiring lease. A stale claim does not grant
ownership: obtain an explicit handoff or reassignment. If claims collide, pause
only the conflicting edits, report the collision and continue independent work.

Use this compact issue/PR update shape:

```text
Instance: <unique ID>; team: <team>; state: <state>
Base: <master SHA>; branch/PR: <link>
Owned files: <exact paths or agreed module groups>
Delivery IDs and bounded behavior: <scope>
Implemented since last update: <actual changes, or none>
Main integration: <actual callsites, or pending contract request>
Source evidence: <pinned methods and team reference note>
Validation: <compiler/runtime result and exact commit, or not run>
Blockers / requested contracts: <concrete details>
Next action / handoff: <owner and next bounded step>
```

## Communication when issue/PR writes are unavailable

Earlier403 errors describe the previous connection state. The user restored
GitHub integration access; actual issue1/team comments and Git-object writes now
succeed. Root published the native connection source with identical Git trees,
and master advanced without force to `1e72694`. Ordinary CLI Git credentials
remain unavailable; the working API path does not require exposing a token.
Each independent instance must discover and verify its own current tools and
connection. An `Unknown tool` transport error is distinct from a GitHub403.
Use the fallback below only when the instance actually cannot write. Later issue
comments supersede historical publication-blocked text in branch status files.

Use issue comments and PRs when the instance can write them. Otherwise publish
`docs/coordination/status/<instance-id>.md` on the team's own feature branch.
That status file is part of its assigned documentation area. Include the same
claim/status/contract fields above, exact source commits and intended recipient.
A published branch and status commit are the handoff even if PR creation fails.
Do not wait for a new permission merely to continue independent authorized work.

During active work, fetch `origin` and inspect the current integration status on
`master` and status files on `origin/teams/*` branches. Read other branches with
`git show <remote-branch>:docs/coordination/status/<instance-id>.md` without
switching or modifying their checkouts. Route integration-contract requests and
responses in these versioned status records. The user now authorizes each assigned team to review and merge its own owned source PRs. Central Main integration, fixed-checklist accounting and deployment configuration remain reserved.

Each owner writes its own branch/status file, avoiding a shared multi-writer
status file. The coordinator's current record is
[status/integration-root-20261101.md](status/integration-root-20261101.md).
No polling, work or session wakeup is automatic after a turn stops. If both
GitHub API writes and authenticated Git pushes are unavailable, report the
access blocker and retain local commits; do not claim a remote handoff occurred.

## Implementation and delivery rules

- Read current `AGENTS.md`, the English roadmap, current status,
  `docs/coordination/checklist-v1.json` and the fixed
  checklist before choosing work. Existing code may already cover part of a
  named delivery. Preserve `scope_version: 1` and the 100-item denominator.
- Prioritize porting and real integration. Use the pinned original Java 1.21
  server, official mappings and actual source inspection as the primary
  reference. Preserve callback order, canonical items, RNG, world identity and
  actual scheduler/entity ownership. Unsupported contexts stay explicit.
- Use REA tools only when actually available. Report direct `javap` inspection
  as such; never invent an MCP execution or an oracle/runtime observation.
- Commit coherent small stages to the team's feature branch and push ordinary
  source files. Open or update a PR against `master`; use a draft for unfinished
  work. Include the assigned issue, implementation scope, integration contract,
  source evidence, focused validation and remaining limits.
- Run the appropriate compiler check and one focused flow when the integration
  allows it. Defer comprehensive conformance suites. Test counts are not a
  progress metric. Do not claim functional compatibility or performance gains
  from protocol support, compilation, isolated numerical code or test volume.
- Do not force-push, deploy manually, change the checklist/journal,
  introduce source archives, commit secrets/private saves/proprietary Java
  artifacts, or change licensing. Preserve GPL-2.0-only and existing notices.
- A team may propose evidence that a complete named delivery is callable or
  invoked by Main. Only the integration coordinator records checklist changes,
  after reviewing the entire delivery against its existing definition. A
  partial workstation or mob does not complete an all-workstations/entity row.
- Before handoff, publish the current branch and a truthful issue/PR update.
  Report active agents from observed state. If a turn ends or work pauses,
  record what stopped and what remains; do not imply ongoing background coding.

Each team may self-review and merge its owned source PRs under the user's explicit authorization. The integration coordinator integrates Main, resolves central-file
conflicts, updates public status/checklist evidence and the persistent journal,
and verifies the resulting Railway deployment. Teams communicate by actually
reading GitHub updates; this document creates no automatic cross-chat channel.
