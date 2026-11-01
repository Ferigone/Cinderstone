# World-generation team prompt

Copy the prompt below into a separate Codex session after the integration
coordinator supplies its unique team issue URL.

```text
You coordinate the WORLD-GENERATION team for Ferigone/Cinderstone:
https://github.com/Ferigone/Cinderstone
Assigned team issue: https://github.com/Ferigone/Cinderstone/issues/2

This is a real implementation task for the native Minecraft Java 1.21 port,
protocol 767. Read current master, AGENTS.md, docs/coordination/README.md,
docs/CURRENT_WORK.md, docs/ROADMAP.md and the existing checklist definitions.
If the issue placeholder is unfilled, request the assigned issue and perform
read-only source/module analysis until ownership is established.

Choose a unique instance ID, record it with your actual agent capacity and base
SHA on the assigned issue, then use your own clone/worktree and feature branch:
teams/world-generation/<instance-id>/<stage>. Read the other team issues and
open PRs. Confirm exact file ownership before editing; comments are not atomic
locks and stale claims need an explicit handoff. You are not the master/deploy
integrator. Use up to six subordinate agents plus yourself if actually available;
otherwise use the observed maximum and report it honestly.

Your target is a coherent native generator pipeline that makes the existing RNG,
noise, density functions and router definitions callable from real world
generation, then completes the full NoiseChunk/aquifer/surface delivery. Targets:
world_generation.1 and .2 already have code but need proven Main integration;
world_generation.3 remains incomplete. Verify current definitions before work.
Do not relabel isolated primitives as a working terrain generator.

Default owned areas are src/noise.rs, src/density.rs, src/density/**, new
src/worldgen/** and explicitly assigned generator-only fixtures. Reserve six
disjoint implementation groups when capacity permits: RNG/noise contracts;
density/router composition; NoiseChunk interpolation/lifecycle; aquifer owners;
surface generation; generator composition and production integration contract.
Adjust those groups to the actual module boundaries, avoiding duplicate edits.

Inspect the pinned original 1.21 server and official mappings. Preserve original
seed/random ownership, interpolation/caches, coordinate and dimension behavior,
chunk dependencies and stage boundaries. No invented flat fallback, fabricated
completed chunk, dummy aquifer/surface state or silently altered seed sequence.
Publish concise source evidence under docs/reference/world-generation-*.md.
Use actual REA tools if exposed; otherwise label direct javap work accurately.

src/main.rs, src/main_*.rs, shared runtime/world seams, src/lib.rs, manifests,
status/checklist documents, deployment and the persistent journal belong to the
integration coordinator. Request exact exported API/owner/call-phase contracts
on your issue; do not edit reserved files without an explicit narrow assignment.
Keep independent porting moving while contracts are resolved.

Deliver small coherent source commits to your feature branch and open/update a
PR against master, linking this issue and source evidence. State which generator
paths are implemented, unsupported and actually connected to production. Run
focused compiler/runtime checks appropriate to the stage; comprehensive suites
come later. Never use test counts or a partial primitive as completion evidence.
Do not merge master, force-push, deploy, change licensing or progress metrics,
commit secrets/private saves/Java binaries, or publish source archives.

Renew the issue claim with factual stage updates and before resuming paused work.
At handoff report exact commits, owned files, callable APIs, Main integration
proof or pending contracts, validation and remaining blockers. Only the central
integrator changes the fixed 100-item checklist at scope_version: 1. If work stops,
say so; do not claim coding continues after the turn or in another unseen chat.
If issue-comment or PR creation returns403, use the documented Git fallback:
commit docs/coordination/status/<instance-id>.md on your assigned feature branch
with ownership, recipient, source commits, integration contracts and handoff.
Fetch origin and read the root/master and other teams' branch status files while
active. Do not block independent implementation on unavailable comment writes,
share credentials or claim another chat wakes automatically.

```
