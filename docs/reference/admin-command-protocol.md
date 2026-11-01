# Bounded administration command protocol

Target: the pinned original Minecraft Java 1.21 dedicated server, protocol767.
This ledger documents static source evidence and the new callable native wire
interface. It does not establish live Main integration or secure-chat parity.

## Source and inspection

The source identity is pinned in `fixtures/source-lock.json`. Inspection used
the official mappings and the pinned inner server JAR through JDK21 `javap -p -c`.
No REA MCP tool was available or used for this stage. Raw disassembly remains
outside the public repository. The locally installed Minecraft data protocol
schema for1.21.1 was used as a secondary wire-layout cross-check; the class
implementations and official1.21 mappings remain the source authority.

| Official class/method | Mapped class | Observed contract |
| --- | --- | --- |
| `ClientboundCommandsPacket.write`, `Entry.write`, `ArgumentNodeStub.write` | `acn`, `acn$b`, `acn$a` | VarInt node count; flags, child indices, optional redirect, node data; VarInt root index |
| `ArgumentTypeInfos.bootstrap` | `ip` | Registry order gives string5, entity6, game_profile7, message19 |
| `StringArgumentSerializer` | `ix` | SINGLE_WORD discriminator0 is a VarInt |
| `ServerboundChatCommandPacket` constructor | `ago` | Default FriendlyByteBuf UTF limit32767; command only |
| `ServerboundChatCommandSignedPacket` constructor | `agp` | Command, Instant as millis, salt, argument signatures, last-seen update |
| `ArgumentSignatures` and `Entry` constructors | `fa`, `fa$a` | At most8 signatures; names at most16 UTF-16 units; signature256bytes |
| `LastSeenMessages.Update` constructor | `xg$b` | VarInt offset and fixed20-bit BitSet occupying3bytes |
| `ServerboundCommandSuggestionPacket` constructor | `agu` | VarInt transaction ID; command at most32500 UTF-16 units |
| `ClientboundCommandSuggestionsPacket` and entry codec | `acm`, `acm$a` | ID, UTF-16 start/length, strings and optional NBT Component tooltip |
| `ServerGamePacketListenerImpl.handleCustomCommandSuggestions` | `aru`, mapped lines538–548 | Skip one leading `/`; parse with actual command source; retain original input offsets |
| Suggestion completion callback | `aru`, mapped lines545–547 | At most1000 sorted results are sent |
| `SharedSuggestionProvider.matchesSubStr` | `ey`, mapped lines264–280 | Match at start or after `.`/`_`; names matched ignoring case |
| `StringUtil.isAllowedChatCharacter` | `azl`, mapped line77 | Reject section sign, UTF-16 units below32 and127 |

PLAY packet IDs are unsigned command04, signed command05, suggestion request0b,
suggestion response10 and command tree11, all hexadecimal.

## Native API and actual ownership

`src/command_protocol.rs` implements:

- `decode_unsigned_command`: command payload decoding with Java UTF-16 length
  limits and full consumption. The decoder does not strip `/`, trim or execute.
- `decode_signed_command`: full wire decoding into an explicitly **unverified**
  `SignedCommand` envelope. Signature bytes, timestamp, salt and acknowledgment
  fields remain available to the real signing and last-seen owners. Decoding
  does not authorize execution, validate a signature, or establish chat history.
- `command_contains_illegal_characters`: the source UTF-16 character predicate,
  kept separate from packet structure and command parsing.
- `administration_command_tree`: a valid rooted, permission-filtered selected
  administration graph returned as a native `protocol::Packet`.
- `decode_suggestion_request`, `administration_suggestions` and
  `encode_suggestions`: bounded literal/profile/IP completion and the native
  response packet, with the source32500-unit request and1000-result limits.

The Main adapter must supply actual permission levels and actual profile/list
snapshots. In particular, operator and whitelist-add eligibility are computed
by the owning profile UUID predicates; this module does not infer eligibility
from a stored display name. It never owns or mutates access lists or players.

## Selected tree

The graph contains `list [uuids]`, `banlist [ips|players]`, `ban`, `ban-ip`,
`pardon`, `pardon-ip`, `kick`, `op`, `deop` and whitelist subcommands. Original
registration inspection used `BanPlayerCommands` (`amc`), `BanIpCommands`
(`ama`), `BanListCommands` (`amb`), `PardonCommand` (`anm`), `PardonIpCommand`
(`ann`), `KickCommand` (`anf`), `OpCommand` (`anl`), `DeOpCommands` (`amj`),
`WhitelistCommand` (`apd`) and `ListPlayersCommand` (`anh`).

All selected administration roots require permission level3, including `op`
and `deop`; `list` has no permission predicate. The serialized graph preserves
these source argument contracts:

- Profile arguments use parser7 and no property bytes.
- Kick targets use parser6 with flags2: multiple targets, players only.
- IP target arguments use parser5 with SINGLE_WORD property0.
- Optional reasons use parser19, the actual signable `MessageArgument`.
- Source custom suggestion callbacks use `minecraft:ask_server`. There are
  no redirects or fabricated selector/function/datapack nodes.

The graph is an explicitly selected subset, not the complete vanilla tree.
Providing the source argument parser does not claim every selector form or
target context is executable by the bounded command engine.

## Signed versus unsigned execution boundary

`ServerGamePacketListenerImpl.performUnsignedChatCommand`, mapped lines1313–1320,
first parses the command with the actual source. When
`MinecraftServer.enforceSecureProfile()` is true and the parsed command has
signable arguments, it sends the red system Component
`chat.disabled.invalid_command_signature` and returns without execution. That
path does **not** disconnect the player. With enforcement false, this particular
unsigned-command gate does not apply.

The signed-command listener nevertheless has separate acknowledgment, signing
context and argument-validation owners. A normal client may choose signed
packet05 for a reason because the source parser is `MessageArgument`. A host
without those real owners must report the unsupported signed path explicitly
and must not execute it after merely discarding its signature envelope. The
wire decoder makes no cryptographic, replay, timestamp or chat-history claim.

## Bounded suggestions and remaining work

Literal completions and ordinary name/IP contexts are implemented. Custom list
suggestions use the host's actual operator, whitelist and ban snapshots. `ban-ip`
has no invented online-name suggestions: its source string argument has no
custom completion provider. Unimplemented selector-option contexts, message
reason completions and general Brigadier parsing produce empty suggestions.

For implemented contexts, result ranges count Java UTF-16 units, including a
leading slash in the original input. Matches equal to the remaining input are
omitted, duplicates removed, names matched after `.`/`_` as well as at the start,
and results sorted before the listener's1000-result cap. Empty results retain
Brigadier's empty range at0. Optional tooltips are absent for these selected
tooltip-free completions; arbitrary tooltip Component handling is not added.

Main admission, runtime command execution, feedback, permission refresh and
live validation are integration-coordinator responsibilities. No checklist
delivery or production connection is declared complete by this isolated module.
