# Player action idle timeout (#185)

Console and permission-level3 players can use `/setidletimeout <minutes>` through
native Brigadier. The integer is0..2147483647, zero disables the policy, and the
result is the supplied value. Success uses normal source/operator feedback and
the canonical component codec. Rejected input changes neither memory nor disk.

Administration loads `player-idle-timeout` from the actual dedicated properties.
A successful command updates the retained setting and appends the canonical key,
preserving unrelated bytes. Last-key-wins parsing survives restart. As in Java,
a write failure warns after the memory update; it does not undo the command.
The pre-existing unreadable-properties startup fallback is preserved.

Each PLAY connection checks its actual retained last-action time against the
server's monotonic clock after keep-alive processing. It disconnects normally
with `multiplayer.disconnect.idling` only when last action and minutes are
positive and elapsed time strictly exceeds minutes*60000 in64-bit arithmetic.
Operators have no exemption. Chat uses its existing action owner; supported
client commands, swings, player actions, hotbar selection, valid use-item/use-on
and menu click/button packets reset activity. Movement and keep-alive do not.
Entity interaction, sign update and recipe placement remain unsupported gameplay
contexts, not newly implemented actions. Raw transport timeout remains separate.

`tools/idle_command_oracle.java` executes original command registration/result/
feedback and the original listener tick. The deadline witness replaces unrelated
player-body/transport callbacks and injects a synthetic original Util clock for
the exact boundary; the live protocol flow uses real elapsed time. Regenerate
`fixtures/idle-command-vectors.json` with `tools/idle_command_vectors.py` using
the existing named original, pinned bootstrap and external libraries.

Run `node tools/idle_command_flow.mjs BINARY PINNED_JAR EXISTING_EULA REPORT`
after `npm ci`. Existing EULA acceptance and ports25641/25642 are required. It
uses isolated temporary worlds, waits over a real minute per server with zero
disabled, then enables1 and checks stale operators disconnect while swing/hotbar
activity survives. It also checks denial, exact source/operator packets, tree,
completions, unchanged invalid-input disk state and saved/restarted properties.
Set TMPDIR to external scratch storage if the system temporary filesystem is full.
[Runtime evidence](../validation/idle-timeout-runtime.json) records the result.

Full #24/#64 remain open; both checklist rows retain partial status and fixed
scope1/100 and58/53. Hosted verification remains under #19.
