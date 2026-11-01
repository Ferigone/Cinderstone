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
No external issue/branch check-in has been observed. The user has reported an
external effects worker; its unpublished branch and actual capacity are unknown.

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

The coordinator previously received HTTP403 for issue comments/updates. The
user-approved Issues and Pull requests read/write permissions are now saved for
its existing token, restricted to Ferigone/Cinderstone. A real token-authenticated
[issue4 handoff comment](https://github.com/Ferigone/Cinderstone/issues/4#issuecomment-6559845309)
was created and read back at2026-11-01T15:16:38Z. PR creation has not been exercised.
The GitHub integration still rejects writes with403; another instance's GitHub
App or token permissions are unchanged. Git pushes worked in the earlier
environment. After recovery, the coordinator's private credential is unavailable
and its newer connection stage is committed locally only; current issue/blob
writes also returned 403. Retain the per-instance branch/status fallback when
authenticated Git access is available. Do not infer another session's access.

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
responses in these versioned status records. Only the central coordinator
reviews and integrates the code into `master`.

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
- Do not force-push, merge into `master`, deploy, change the checklist/journal,
  introduce source archives, commit secrets/private saves/proprietary Java
  artifacts, or change licensing. Preserve GPL-2.0-only and existing notices.
- A team may propose evidence that a complete named delivery is callable or
  invoked by Main. Only the integration coordinator records checklist changes,
  after reviewing the entire delivery against its existing definition. A
  partial workstation or mob does not complete an all-workstations/entity row.
- Before handoff, publish the current branch and a truthful issue/PR update.
  Report active agents from observed state. If a turn ends or work pauses,
  record what stopped and what remains; do not imply ongoing background coding.

Only the integration coordinator merges accepted work, resolves central-file
conflicts, updates public status/checklist evidence and the persistent journal,
and verifies the resulting Railway deployment. Teams communicate by actually
reading GitHub updates; this document creates no automatic cross-chat channel.
