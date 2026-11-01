# Configuration and reconfiguration reference

This ledger records static Minecraft Java **1.21**, protocol **767**, behavior
used by the native connection state machine. It is not a runtime result or a
claim of complete packet-decoder, administration or vanilla compatibility.

## Pinned source

The official server artifacts are the same pinned inputs as
[`furnace-menu-lifetime.md`](furnace-menu-lifetime.md): outer JAR SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inner JAR SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
and official mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
Their exact URLs remain in [`source-lock.json`](../../fixtures/source-lock.json).

JDK 21 `javap -p -c` was used on the pinned classes, independently from the
historical REA archive inventory. Raw Java binaries and disassembly remain
private. Relevant mapped classes are `ServerGamePacketListenerImpl` (`aru`),
`ServerConfigurationPacketListenerImpl` (`ars`),
`ServerCommonPacketListenerImpl` (`arr`), `PacketListener` (`wf`),
`ConfigurationProtocols` (`abj`), `GameProtocols` (`agh`),
`SynchronizeRegistriesTask` (`asd`), `JoinWorldTask` (`asb`),
`KnownPack` (`atl`), and the terminal configuration packets.

## Protocol directions change at different boundaries

`ServerGamePacketListenerImpl.switchToConfig` (`aru.n`, mapped lines 1578–1582):

1. Sets `waitingForSwitchToConfig`.
2. Removes the actual player from the world through `removePlayerFromWorld`.
3. Sends `ClientboundStartConfigurationPacket` in the old PLAY protocol.
4. Changes the outbound codec to CONFIGURATION.

The inbound codec remains PLAY until the client sends its terminal empty
`ServerboundConfigurationAcknowledgedPacket`. An acknowledgement without a
pending switch throws. Its handler installs a new configuration listener using
the actual profile, client information and latency from `CommonListenerCookie`.
It does not reauthenticate the transport or create a world snapshot.

| Packet | Direction/state | ID | Body |
| --- | --- | --- | --- |
| Start configuration | Clientbound PLAY | `0x69` | Empty |
| Configuration acknowledged | Serverbound PLAY | `0x0c` | Empty |
| Finish configuration | Clientbound CONFIGURATION | `0x03` | Empty |
| Finish configuration | Serverbound CONFIGURATION | `0x03` | Empty |
| Select known packs | Clientbound CONFIGURATION | `0x0e` | List of namespace/id/version strings |
| Select known packs | Serverbound CONFIGURATION | `0x07` | At most 64 entries |
| Keepalive | Clientbound CONFIGURATION | `0x04` | Signed 64-bit challenge |
| Keepalive | Serverbound CONFIGURATION | `0x04` | Signed 64-bit challenge |
| Keepalive | Clientbound / serverbound PLAY | `0x26` / `0x18` | Signed 64-bit challenge |

The packet IDs were counted in the actual protocol registration order; the
installed protocol-767 schema corroborates them. The pinned CONFIGURATION
serverbound registration has only IDs `0x00` through `0x07`. Additional
serverbound report/server-link entries present in the auxiliary schema are not
accepted as authoritative additions to the official registry.

## The old PLAY listener stops dispatching gameplay

`aru.isAcceptingMessages` is `connected && !waitingForSwitchToConfig`.
`aru.shouldHandleMessage` additionally permits only the configuration
acknowledgement while waiting. The inherited default gate uses the virtual
`isAcceptingMessages` result. Consequently already decoded late PLAY packets,
including common cookie/resource-pack/keepalive replies, do not run their
handlers while that switch is pending.

The native adapter preserves this dispatch gate. Its raw frame pump does not
implement every ignored PLAY packet body's decoder, so malformed late-gameplay
body equivalence remains incomplete. Common bodies handled at that boundary
are decoded before being discarded; no world edit or player update is replayed.

## Configuration tasks are ordered actual owners

`ars.startConfiguration` sends the brand, optional server links and feature
flags, then queues `synchronize_registries`, an optional `server_resource_pack`
task, and `join_world`, in that order. `ars.returnToWorld` instead queues only
`join_world`. Merely installing the listener is distinct from choosing either
of those entry points.

Only one task is current. Starting another while one remains active, or
finishing a different task type, throws. The registry task sends its offered
known-pack list and waits for the actual reply. It compares the complete
ordered reply list with the complete offered list. An exact match uses the
offered pack set; any mismatch uses the empty known-pack set. It then emits
registry data and tags before finishing that task.

The native captured-registry host currently offers an empty known-pack list
and sends its actual complete serialized registry/tag entries after the reply.
It does not claim dynamic registry reload or client-known entry omission.

The optional resource-pack owner validates the actual response and finishes
its task only on a terminal action. The common required-pack DECLINED rule
runs before configuration-task completion. Response UUIDs do not select a
different task. The native resource-pack module owns those details separately.

`JoinWorldTask.start` sends the terminal empty finish-configuration packet. A
matching empty client acknowledgement finishes that task and switches outbound
protocol to PLAY. Java checks admission and constructs/places a new actual
`ServerPlayer` from the profile and client information; it does not continue a
retained in-world player while pretending to rejoin. Native world admission
owns persistence, inventory, position, entity IDs and chunk readiness separately
from the connection state machine.

## Listener keepalive and transport ownership

Sending a terminal packet closes its common listener. The close timestamp and
pending challenge remain owned by that old listener until it is replaced.
A new common listener inherits latency but initializes a fresh monotonic timer
and clears the pending challenge. Configuration downloads have no invented
global 15-second deadline: the ordinary actual keepalive challenge/timeout
policy still runs as the server polls the listener.

The host retains one encrypted/framed input pump and one connection channel
across configuration and PLAY. Aborting an in-flight frame read and attaching
a second reader is not a protocol transition. During CONFIGURATION the adapter
does not send the removed player's gameplay/chunk/event stream.

The native engine and adapter require focused compilation and runtime
validation at their integration stage. This ledger alone establishes no
successful reconfiguration, graphical-client session, full disconnect
callback/persistence ordering, arbitrary registry reload or throughput result.
