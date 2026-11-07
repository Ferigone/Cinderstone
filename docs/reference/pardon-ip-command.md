# IP unban command (#194)

Console and permission-level3 players execute `/pardon-ip <target>` through
native Brigadier. The original word parser, permission-filtered tree and
retained-IP suggestions are used before the actual administration host validates
and removes a key. Failed or unauthorized input does not write the ban file.
Successful removal returns1 and persists through the existing IP-ban writer.
Retained expired entries remain removable; the command does not perform a broad
expiry cleanup. Failures are private, success uses operator feedback with the
current source display name, and Hidden recipients receive no system messages.

`tools/pardon_ip_command_oracle.java` executes original PardonIpCommand with real
IpBanList/entry instances and the original tree, suggestion and component codecs.
The focused fixture covers word grammar, reachable validation, missing/expired
entries and exact removal/feedback. The recording source captures original
source feedback; the live flow independently compares player packet bytes.
Regenerate it with
`tools/pardon_ip_command_vectors.py --named-jar ... --original-jar ... --libraries
... --out fixtures/pardon-ip-command-vectors.json`, using the pinned bootstrap
and existing remapper, with proprietary artifacts outside the checkout.

The library and actual Main administration-owner tests consume these witnesses.
`node tools/pardon_ip_command_flow.mjs BINARY PINNED_JAR EXISTING_EULA REPORT`
runs real Java/native servers in isolated temporary directories after `npm ci`.
It requires existing EULA acceptance and ports25641/25642. It compares source/
operator packet bytes, live suggestions and file membership across removal,
checks failure privacy and Hidden feedback, then bans the loopback address,
observes a rejected login, unbans it from console, rejoins and cold-restarts.
[Runtime evidence](../validation/pardon-ip-runtime.json) records source/binary
identity. No Railway volume is modified by this flow.

Other commands, selectors/profile lookup, general date/locale/HashMap behavior
and full #24/#64 remain out of scope. Scope1/100 and58/53 stay unchanged; both
checklist rows remain partial. Hosted verification is tracked under #19.
