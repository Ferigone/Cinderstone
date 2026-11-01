# Integration coordinator status

Instance: integration-root-20261101
State: completed_stage; ready for published external-team handoffs
Integration queue: https://github.com/Ferigone/Cinderstone/issues/1
Team assignments: world-generation #2; workstations #3; entities-survival #4.

## Ownership

The current coordinator owns master, central Main adapters, connection/session
protocol and admission/administration modules, shared exports, checklist
accounting, persistent journal and Railway. External teams own unused default
areas in the coordination guide. No separate external instance is assumed active
without an actual issue or Git branch/status check-in.

## Current delivery

Bounded native dedicated admission/administration is implemented in source
`6c5401882c2cf2d454968455fa98b1e91b653aa5`, after core `76141a7` and Main
integration `870349d`. The final Main build and
[local admission flow](../../validation/admission-flow.json) passed at
`2026-11-01T16:10:14.211Z` on binary
`f5e29a9ecb012d4685c9358612e453189c28475cd2d9fb50624e85c6a0a55411`.
Actual list/peer/profile owners feed ordered pre-success and final admission;
commands mutate persistent access lists, update permissions/tree and disconnect
actual connections. Policy changes serialize with admission. The source-review
correction preserves compression-before-duplicate-eviction ordering. Scope and
remaining native boundaries are in [admission-production.md](../../reference/admission-production.md).

This bounded stage does not complete authentication.5 or commands_data.1; the
unchanged scope1 checklist remains56/100 code and51/100 Main. Earlier network.5
remains complete with its own source and runtime evidence. The optimized code release reachedSUCCESS/offline listening at
2026-11-01T16:12:58.911137306Z, with one replica and the retained volume. Public
gameplay is unverified because the fresh external DNS probe failed. Hosted
verification is recorded in docs/validation/railway-preview.json and the progress
journal; a source push or build-in-progress is not itself deployment success.

## Communication

The user-approved Issues and Pull requests read/write permissions are saved for
the existing coordinator token, restricted to Ferigone/Cinderstone. A real
[Team4 handoff comment](https://github.com/Ferigone/Cinderstone/issues/4#issuecomment-6559845309)
was created through that token and read back at2026-11-01T15:16:38Z. PR creation
has not been exercised. The GitHub integration still returns403 for writes;
this permission change does not alter another instance's GitHub App or token.
Ordinary Git pushes work. Retain the per-instance branch/status fallback and
never share credentials.

## Integration contracts

Team 4 has an explicit current grant in
[assignments/entities-survival.md](../assignments/entities-survival.md).
The earlier integration-team damage audit and healing edits are complete; no
competing survival_combat.2 implementation is active. The user supplied a
screenshot of the external effects team asking for this clarification. Its
actual capacity and unpublished source/branch are not inferred from that image.

Teams should publish exact exported API, canonical state/RNG/scheduler owners,
required Main call phase and source ordering. Mechanical new-namespace lib exports
are permitted as documented. Main/runtime/manifests/deployment changes otherwise
remain centrally integrated. The damage integration contract documents existing
owners and actual blockers for the entities/survival team.

## Next handoff

Publish team feature branches and per-instance status files or writable PRs.
No external team check-in was observed in issues2–4 or teams/* branches during
this stage; an external effects worker was reported by the user. The coordinator reads those records while its session is active;
no automatic wakeup, polling or continuing background coding is claimed.
