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

Bounded native player chat/session integration is published in source
`b3a0d1dc0ed01de666c7bf56723ebfe30140e0be`, following core `66ebc57`, Main
`1f56b98`, sequencing/key-wait fixes `cea8569` and registration cleanup `396b589`.
The final Main build and [local chat flow](../../validation/chat-flow.json)
passed at `2026-11-01T16:53:46.366Z` on binary
`509a4dfb7d95ce7d05403b169b5f4922f0d3482713ab6f961a003be9e3c662f0`.
Actual visibility, Unicode routing, ACK/character refusal, spam and tick recovery,
and an offline empty-signature command envelope were exercised. The separate
[crypto smoke](../../validation/chat-crypto-smoke.json) passed against the actual
native library with generated trust keys; no Mojang certificate or production
Main authenticity claim is made.

Production owners and limits are in
[chat-production.md](../../reference/chat-production.md). Chat/session/admin
output uses one ordered stream; whole chat-command execution and admission
mutations share their sequence owner. Terminal output is half-closed while the
actual reader drains late input with a declared native bound. Native service-key
refresh and session verification are connected, without client-supplied roots.
The earlier admission flow also passed on `cea8569` after its shared-output
changes. The fixed scope1 checklist stays56/100 code and51/100 Main;
authentication.4, authentication.5 and commands_data.1 remain incomplete.
The optimized code release reachedSUCCESS/native offline listening at
2026-11-01T16:56:46.030735652Z, with one replica and retained volume. Public
PLAY remains unverified because fresh DNS resolution failed. Hosted evidence is
in docs/validation/railway-preview.json and the persistent journal.

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
