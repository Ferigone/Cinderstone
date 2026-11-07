# Public literal emotes (#207)

Non-op players and console execute `me <action>` with native Brigadier. The
existing literal message grammar selects only the root, permission, argument
name and chat-type metadata for say/emote. Emotes are public; their original
signable argument name is `action`. Spacing, quotes and Unicode up to256 UTF-16
units remain unchanged. Unsupported selectors fail explicitly. Result is1;
there is no private/admin success echo.

The actual AuthorizedCommand message enters the existing ordered chat audience
with current decorated player name or Server console name and the configured
`minecraft:emote_command` holder. Existing Full-only visibility, recipient
filtering, signature/pending caches, secure unsigned rejection and spam owners
remain in use. No additional signing or trust owner is introduced.

Run the [existing witness](say-command.md) with the optional `emote` mode:
`cinderstone.oracle.say_oracle ORIGINAL_JAR fixtures/emote-vectors.json emote`.
It executes original EmoteCommands/MessageArgument and result, display names
and disguised-chat codec. The recording PlayerList broadcast sink and separate
ChatType.bootstrap holder ordering remain disclosed. This is an original-class
witness, not a full Java server flow. The focused differential retains `/say`
and adds three public permission trees/eight emote cases. Proprietary artifacts
remain outside the checkout.

Run `node tools/say_flow.mjs target/debug/mc121-rust REPORT.json emote` after
building and `npm ci`. The temporary loopback25706 flow uses a non-op source,
checks actual tree/root completion, player/console packets, live rich team
identity and CommandsOnly/Hidden suppression/Full restoration, rejected input,
no success echo and empty ops.json. Ordered packets at both recipients bound
absence checks. Default mode still tests permission2 `/say`; the committed
[report](../validation/emote-runtime.json) includes that regression against the
same source tree/binary. Selected complete packets use original body/name bytes
with each actual configured registry ID. Focused signing tests verify `action`,
secure unsigned rejection and existing downstream cache/visibility behavior;
synthetic signature delivery fixtures do not claim real-account trust.

Selectors, external moderation, real-account trust and full #24/#63 remain out
of scope. Scope_version1, denominator100 and counts58/53 remain unchanged.
Hosted checks stay under #19.
