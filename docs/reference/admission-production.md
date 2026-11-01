# Bounded production admission and administration

Target: native Minecraft Java **1.21**, protocol **767**. This note records actual
production calls in `src/main.rs`, `src/main_admin_dispatch.rs` and
`src/main_administration.rs`. The source stage compiles at commit `6c5401882c2cf2d454968455fa98b1e91b653aa5`.
The [focused local flow](../validation/admission-flow.json) passed at
`2026-11-01T16:10:14.211Z` on binary SHA-256
`f5e29a9ecb012d4685c9358612e453189c28475cd2d9fb50624e85c6a0a55411`.
It uses local protocol clients and an actual native console, not a graphical
client or public-network session.

## Actual owners and admission phases

Main constructs one `Administration` owner, held by the server's synchronous
read/write lock. It owns the four access-list maps, administration settings,
selected profile-name index and storage directory. The actual admitted-player
registry remains in `LivePlayers`. `connection_addresses` maps each admitted
entity ID to its accepted TCP socket address. No proxy header, handshake host,
client-claimed UUID or generated command profile replaces those owners.

The initial LOGIN profile is either the verified session-service profile from
the existing online-authentication path, or the actual selected offline profile.
`negotiate_login` calls `check_initial_admission` with that profile and the real
socket peer **before** compression, duplicate eviction and LoginSuccess.
The enclosing LOGIN continuation holds `player_admission` while inspecting the actual registry and
applying the ordered policy:

| Gate | Production data and behavior |
| --- | --- |
| Player ban | UUID-keyed player-ban contains, then expiration-purging get; stored reason and optional expiry form the rejection |
| Whitelist | Disabled whitelist, actual operator membership or direct UUID whitelist membership permits entry |
| IP ban | Legacy source IP key from the accepted socket; contains and expiration-purging get are reached only after earlier gates permit |
| Capacity | Actual admitted registry count compared with signed `max_players`; only the stored operator bypass flag bypasses this check |

An expired target disappearing between contains and get remains an explicit
exceptional outcome. It is not silently treated as an ordinary absent ban.
The earlier accepted player still counts at the initial capacity gate. A
rejected replacement login cannot evict that player. If all gates permit, an
negotiated compression is enabled before an existing same-UUID actor is removed
through its actual cleanup owner, followed by LoginSuccess. Administration
commands acquire the same admission mutex across their actual player snapshot,
policy mutation and ordered effects, preventing a policy change from slipping
between the final gate and publication of the incoming actor.

After the client finishes CONFIGURATION, `play` holds `player_admission` and
calls `check_final_admission` before allocating or constructing a native player.
The final check first rejects an already-admitted UUID, then reruns the list and
capacity gates against current ownership. Its disconnect uses the PLAY packet
and anonymous NBT because the outbound protocol has already changed. This same
check also covers actual return-to-world reconfiguration. Pending connections
are not invented players or reserved slots in the admitted count.

The status response now reports the actual admitted count and configured
maximum. A separate, hardcoded **64-permit native transport bound** still limits
concurrent connection tasks, including status, negotiating and PLAY sockets.
Each permit lasts until its real connection task exits. Exhaustion closes the
newly accepted socket; it is not the vanilla player-capacity rejection and does
not replace `max_players` or its operator bypass. That transport limit is an
explicit native boundary, not a ported vanilla login rule.

## Commands, permissions and actual connections

The dedicated stdin reader only forwards lines into Main's console queue.
Main executes them using the actual `Administration` owner and a console source
named `Server` with permission4. EOF disables the console receive branch. A
blocked stdin reader is not a second owner of runtime state.

For a PLAY player, packet04 decodes an unsigned command and dispatches it using
that admitted entity ID, actual profile and current operator permission. Packet0b
routes command suggestions using real online names and current access lists.
The initial PLAY setup and later op/deop effects send the permission entity event
and refreshed selected command tree. The wire graph hides unavailable roots:
`list` is permission0; selected administration roots require permission3.

Implemented roots are `list [uuids]`, `banlist [players|ips]`, `ban`, `pardon`,
`ban-ip`, `pardon-ip`, `op`, `deop`, `kick` and whitelist
`on|off|list|reload|add|remove`. Literal player targets resolve only from actual
online, loaded-list or previously selected LOGIN profiles. The host additionally
supports `@a` and an actual-player `@s`; unsupported selector forms and unresolved
remote profiles fail explicitly. It never invents a UUID to make a command work.

Command outcomes retain their ordered effects even if a later callback fails.
List mutation precedes that list's persistence attempt; a failed save emits a
warning without rolling back the live mutation or suppressing remaining source
callbacks. Feedback, permission refresh and disconnect effects target the
captured admitted entity ID and UUID. Only the matching connection's event
receiver writes its packets; a replacement actor does not inherit the old
actor's queued command effect.

System feedback uses the bounded administration Component codec. Direct/admin
broadcast routing reads the actual `sendCommandFeedback` and `logAdminCommands`
game rules. Whitelist enforcement is deliberately distinct from admission:
when `enforce_whitelist` is true, its command callback checks direct whitelist
membership and can disconnect an unlisted operator. It does not reuse the
admission operator exemption or require whitelist use to be enabled.

Packet05 decodes the signed-command envelope and checks illegal characters,
but reports signed execution as unsupported rather than dropping signatures
and executing it as unsigned. Signature verification, acknowledgment/chat-chain
ownership and complete `enforce-secure-profile` command behavior remain open.

## LOGIN timeout and connection ticks

Main publishes a cumulative counter after its actual connection phase on each
native server tick. A LOGIN connection installs one `ObservedLoginTimeout` from
that counter and retains it across Login Start, key exchange, authentication,
duplicate cleanup, LoginSuccess and the wait for Login Acknowledged. Packet
arrivals do not reset the timer. Successful acknowledgment ends this listener
and its timeout ownership.

Coalesced watch notifications carry the cumulative tick count, so the adapter
accounts for every observed tick rather than treating one notification as one
tick or converting elapsed milliseconds into a guessed deadline. The source
postincrement threshold acts on invocation601 and sends
`multiplayer.disconnect.slow_login` through the actual LOGIN codec/writer.
This is separate from existing handshake, write and later raw-reader deadlines.

The callable `ConnectionPacketRate` counters are **not** connected to production
accepted-packet/send-dispatch accounting or rate kicks. No per-IP login throttle
or positive `rate-limit` enforcement is claimed by this stage.

## Persistent files and configuration

`--server-directory PATH` selects the real administration directory. Without it,
Main uses the parent directory of its configured `--world` path. The directory
owns `banned-players.json`, `banned-ips.json`, `ops.json`, `whitelist.json` and the
bounded `white-list` setting in `server.properties`.

Startup follows the implemented dedicated-list load/save order and warning
boundaries; it does not route these lists through a private world checkpoint.
`white-list` is read with the implemented properties escapes, continuations,
separators and last-key-wins behavior. Whitelist on/off appends its canonical
setting while preserving unrelated bytes. Other properties are not silently
claimed as configured by that reader. In particular `max-players`, operator
level and enforce-whitelist currently come from Main's CLI settings:

| CLI | Container entrypoint | Default |
| --- | --- | --- |
| `--server-directory PATH` | `MC_SERVER_DIRECTORY`, otherwise `MC_DATA_DIR` | CLI: parent of `--world`; container: `/data` |
| `--max-players INTEGER` | `MC_MAX_PLAYERS` | 20 |
| `--op-permission-level INTEGER` | `MC_OP_PERMISSION_LEVEL` | 4 |
| `--enforce-whitelist` | `MC_ENFORCE_WHITELIST=true` | false |

The entrypoint retains the existing `MC_ONLINE_MODE` selection and persistent
data directory. This note does not establish a Railway deployment or public
connection result merely from those entrypoint arguments.

## Source evidence and remaining boundaries

The supporting ledgers separate pinned source inspection from production calls:
[stored access lists](access-lists.md), [ordered login policy](login-admission.md),
[command callbacks](admission-commands.md), [wire commands and suggestions](admin-command-protocol.md)
and [LOGIN/packet-rate ownership](login-limits.md). They identify direct JDK21
`javap` work accurately; this stage does not invent an REA MCP invocation.

Remaining boundaries include full Gson lenient JSON/coercion and Java
locale/calendar equivalence, exact map enumeration order, remote user-cache
discovery/persistence, arbitrary selectors and reason expansion, full Brigadier
diagnostics and the complete vanilla command tree. Signed-command security and
positive packet-rate enforcement are also unfinished. These limits remain
visible even though the selected admission and administration paths now have
real Main calls.
