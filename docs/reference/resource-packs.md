# Minecraft 1.21 server resource packs

This ledger records the pinned Java server's packet and listener behavior. The
native module and production callsites must be verified separately; these
static facts do not establish a successful pack download or graphical-client
reload.

## Pinned reference

The official inner server JAR has SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The matching server mappings have SHA-1
`31c77994d96f05ba25a870ada70f47f315330437`; the outer server JAR SHA-1 is
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`. Source URLs belong to
[`source-lock.json`](../../fixtures/source-lock.json).

JDK 21 `javap -p -c` was used for the following shipped classes:

| Mapped class | Shipped name | Relevant behavior |
| --- | --- | --- |
| `ClientboundResourcePackPushPacket` | `zt` | push fields, hash limit, stream codec |
| `ClientboundResourcePackPopPacket` | `zs` | optional UUID removal |
| `ServerboundResourcePackPacket` | `aae` | UUID and enum response |
| `ServerboundResourcePackPacket.Action` | `aae$a` | ordinal values and terminal status |
| `ServerCommonPacketListenerImpl` | `arr` | required-pack decline rule |
| `ServerConfigurationPacketListenerImpl` | `ars` | configuration task ordering/completion |
| `ServerResourcePackConfigurationTask` | `asc` | offer from actual server configuration |
| `DedicatedServerProperties` | `apo` | URL, hash, prompt and UUID configuration |
| `ConfigurationProtocols` | `abj` | configuration registration order |
| `GameProtocols` | `agh` | play registration order |
| `ProtocolInfoBuilder` | `zl` | packet codec registration and bundle delimiter |

Packet IDs below were checked against the actual protocol registration order,
including the play bundle delimiter entry. The existing local protocol-767
schema supplies an additional cross-check. Raw Java artifacts/disassembly are
not redistributed.

## Pack stack wire messages

| Message | Configuration packet ID | Play packet ID | Payload |
| --- | --- | --- | --- |
| Push | `0x09` | `0x46` | UUID, URL string, hash string, required boolean, optional prompt Component |
| Pop | `0x08` | `0x45` | boolean UUID-present, then UUID when present |
| Response | `0x06` | `0x2b` | UUID, action VarInt |

The prompt is a presence boolean followed by the Component's structured
anonymous network NBT. Sending a JSON string in its place is a different wire
format. URL strings use the normal 32,767 UTF-16-character limit. The push
constructor and hash codec restrict the hash to at most 40 UTF-16 characters;
the packet constructor does not itself require a valid hexadecimal SHA-1.

A pop with no UUID removes all server packs from the client stack. A pop with a
UUID identifies that pack. Sending a pop is not a terminal client response and
does not itself finish a pending configuration task.

## Actual response values and required-pack rule

| VarInt | Action | Terminal |
| --- | --- | --- |
| `0` | `SUCCESSFULLY_LOADED` | yes |
| `1` | `DECLINED` | yes |
| `2` | `FAILED_DOWNLOAD` | yes |
| `3` | `ACCEPTED` | no |
| `4` | `DOWNLOADED` | no |
| `5` | `INVALID_URL` | yes |
| `6` | `FAILED_RELOAD` | yes |
| `7` | `DISCARDED` | yes |

The common server listener checks one condition: action `DECLINED` and the
server's global `isResourcePackRequired()` setting. That condition disconnects
using translation key `multiplayer.requiredTexturePrompt.disconnect`.
`FAILED_DOWNLOAD`, `INVALID_URL`, `FAILED_RELOAD` and `DISCARDED` do not trigger
this particular disconnect rule, even when the configured pack is required.

The common listener does not correlate the response UUID with a pending offer
and does not require an `ACCEPTED` or `DOWNLOADED` response before a terminal
response. A native implementation must not add those checks as if they were
vanilla behavior. The rule applies in both Configuration and Play. Invalid enum
ordinals and trailing packet data remain wire-decoding errors.

## Configuration task lifecycle

`startConfiguration` first queues registry synchronization, then its optional
server resource-pack task, then the join-world task. Resource-pack task
`start` sends the configured server pack's actual UUID, URL, hash, required
flag and nullable prompt.

The configuration response handler invokes the common handler first. If the
action is terminal, it then finishes the current task by the
`server_resource_pack` task **type**. It does not compare response UUIDs.
`ACCEPTED` and `DOWNLOADED` leave the task pending. Finishing with no matching
current task is an unexpected-task error, not permission to advance arbitrary
registry or join tasks. The host must process a required-decline disconnect
before continuing the connection's configuration writes.

This is the normal resource-pack task in initial configuration. Returning to
Configuration, cookies, transfer, all other configuration tasks and a complete
task-queue framework are separate porting scopes.

## Dedicated-server configuration

An empty resource-pack URL produces no configured offer. When the resource-pack
UUID property is absent, Java computes
`UUID.nameUUIDFromBytes(url.getBytes(UTF_8))`: MD5 of the raw URL bytes, with UUID
version 3 and the RFC variant bits. There is no additional name prefix.

The SHA-1 property takes precedence over the deprecated hash property. A missing
hash is permitted; a malformed nonempty hash produces a warning while its text
is retained. URL scheme validity is left to the client rather than replaced by
a fabricated server-side download result. Java parses a nonempty prompt as a
JSON Component and logs/omits an invalid prompt. An invalid explicit UUID
prevents creation of that server's offer.

`resource_packs.rs` exposes native push/pop packets, all response actions, the
normal configuration task, URL-derived UUIDs and a packet-offer/status ledger.
That ledger is bookkeeping only; it adds no UUID or response-order acceptance
restriction. Its required-pack setting belongs to the connection's actual
server configuration, rather than an untrusted response or the individual
offered pack's bookkeeping entry.

The native constructor reuses the existing bounded JSON-Component/network-NBT
codec. Malformed prompt JSON syntax is logged and omitted, so the actual pack
offer remains available without that prompt. This handling is separate from
the Component codec: valid JSON containing unsupported Component fields still
raises an explicit native configuration error rather than being silently
discarded. The module does not fabricate a prompt or claim full
arbitrary-Component parity. Deprecated property loading
and all command/admin surfaces require their actual host integrations. Static
packet construction and response handling do not prove the client downloaded,
verified or reloaded resource files.
