# Dedicated administration commands: bounded native engine

## Pinned source and inspection

Target: original Minecraft Java 1.21 dedicated server, protocol767. The source
identities remain those in `fixtures/source-lock.json`; inner server SHA-256:
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official mappings identify the following original classes and methods:

| Original owner | Mapped class | Native behavior |
| --- | --- | --- |
| BanPlayerCommands | `amc` | `banPlayers`: add/save, feedback, then actual online UUID disconnect |
| BanIpCommands | `ama` | `banIpOrName`, `banIp`: literal IP or actual online name, affected-address snapshot, add/save, success/info, disconnects |
| BanListCommands | `amb` | `showList`: empty/header/entry messages and entry count |
| PardonCommand / PardonIpCommand | `anm` / `ann` | Existing-entry-only removal/save and success |
| OpCommand / DeOpCommands | `anl` / `amj` | Permission mutation/save, permission/tree callbacks, original feedback capture |
| WhitelistCommand | `apd` | Membership, persisted enable flag, reload and enforce callback ordering |
| KickCommand | `anf` | Published-server guard, owner exclusion, disconnect before success |
| ListPlayersCommand | `anh` | Ordered actual player names or names with UUIDs; return current count |

These methods were inspected directly with JDK21 `javap -c -p`; selected lambda
bootstrap targets were checked with `javap -v`. This is direct JVM inspection,
not an REA MCP execution or a Java runtime observation. Raw disassembly remains
private and is not committed.

## Callable contract

`src/admission_commands.rs` exports:

- `parse_command`, `AdminCommand`, and `required_permission`.
- `execute<H: AdministrationHost>`, returning the original command result count.
- Typed `CommandSource`, `OnlinePlayer`, `CommandMessage`, `MessageArg` and errors.
- `CommandMessage::json_component`, `network_component`, and the restricted
  `encode_component_json` helper for native administration feedback.
- `default_player_display_component` for the current no-scoreboard-team player
  profile, using the supplied real name and UUID.

The host supplies actual known profiles, admitted player order, peer addresses,
persistent list mutation, operator permission packets/tree refresh, feedback and
disconnect actions. The engine never invents an offline UUID or player owner.
Mutation callbacks include list save; Java logs save failures while retaining
the memory mutation, so a host must warn without rolling it back or suppressing
the remaining command effects.

Supported literals are `list [uuids]`, `banlist [players|ips]`, `ban`, `pardon`,
`ban-ip`, `pardon-ip`, `op`, `deop`, `kick`, and whitelist
`on|off|list|reload|add|remove`. Target collections are resolved by the host. The
parser accepts one host-owned target token and bounded plain MessageArgument
reasons. Ban-IP string arguments use the original unquoted-word character set;
the underlying IP function accepting IPv6 does not make a colon-containing
literal valid in that command argument grammar.

## Source ordering and details

- Every listed administration root requires permission3, including `op` and
  `deop`; `list` has no permission predicate. Unknown/inaccessible roots retain
  the command-unknown error identity rather than an invented permission message.
- Player ban: add/save, increment result, per-profile success, then disconnect
  that actual online UUID with `multiplayer.disconnect.banned`.
- IP ban: take affected players before insertion, add/save, success, optional
  affected-player info, then disconnect each with `multiplayer.disconnect.ip_banned`.
  A successful address ban with nobody connected returns0.
- Kick: actual target resolution, published-server check, skip singleplayer owner,
  disconnect, then success. No changed targets yields the original owner error.
- Op/deop preserve the original lambda capture: every successful mutation's
  feedback uses the first member of the resolved target collection. Deop invokes
  `MinecraftServer.kickUnlistedPlayers` after its mutation/feedback loop.
- Whitelist on, reload and remove invoke that same enforcement callback after
  feedback; off and add do not. Enforcement is a distinct source predicate:
  when `enforce-whitelist` is true it checks raw whitelist membership, without
  the admission operator exemption or a `usingWhiteList` check. See the separate
  login-admission ledger and policy for the admission/enforcement distinction.
- Banlist, whitelist-list and player-list feedback does not broadcast to ops.
  Mutating command feedback does. `CommandSourceStack.broadcastToAdmins` uses
  `PlayerList.isOp`, excludes the source object and applies the actual
  `sendCommandFeedback` and `logAdminCommands` rules; the runtime owns delivery.

## Restricted feedback codec

Translation arguments retain integers, strings and nested components. Anonymous
network NBT follows the inspected `TranslatableContents.ARG_CODEC` and NbtOps
list collectors: homogeneous byte/int/long streams use primitive arrays;
heterogeneous lists wrap non-compounds with the empty-key compound form. Empty
translation argument lists are omitted. List separators are gray comma-space.
The no-team Player display preserves the original `/tell NAME` suggest click,
show-entity hover with real UUID/type/name, and insertion name. UUID display uses
the actual canonical UUID. The codec supports these generated shapes and their
known style/admin wrappers; unsupported fields fail explicitly.

## Boundaries and integration evidence

This is a native engine and host contract, not proof of full administration,
secure chat or all Brigadier grammar. Arbitrary selectors, selector expansion in
reasons, custom/team display components and full parser cursor/diagnostic
equivalence remain outside this bounded parser. The MessageArgument remains
signable even when plain: the PLAY listener must apply its actual
`enforce-secure-profile` rule before unsigned execution. Signed-command
verification is not implemented by this engine.

Main integration, exact source commit and the focused native runtime observation
are recorded by the integration coordinator. This ledger by itself establishes
no Main call, runtime success, checklist completion or performance advantage.
