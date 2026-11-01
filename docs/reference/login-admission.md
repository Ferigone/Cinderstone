# Dedicated login admission — Java 1.21

## Reference and inspection

The reference is the pinned original Java 1.21 dedicated server and official
mappings in `fixtures/source-lock.json`: outer server SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
mapping SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
These findings come from direct JDK21 `javap -c -p` inspection, not an REA MCP
execution or a runtime observation. Raw disassembly stays outside this repository.

## Ordered gates and actual owners

`PlayerList.canPlayerLogin(SocketAddress, GameProfile)` (`aur.a`, mapped lines
410–440) evaluates these gates in order and returns the first rejection:

| Gate | Actual condition | Rejection |
| --- | --- | --- |
| Player ban | `UserBanList.isBanned(profile)`, then `get(profile)` | `multiplayer.disconnect.banned.reason`, with the stored reason as a literal string argument |
| Whitelist | `DedicatedPlayerList.isWhiteListed(profile)` is false | `multiplayer.disconnect.not_whitelisted` |
| IP ban | `IpBanList.isBanned(address)`, then `get(address)` | `multiplayer.disconnect.banned_ip.reason`, with the stored reason as a literal string argument |
| Capacity | Actual `players.size() >= maxPlayers` and no actual op-entry bypass | `multiplayer.disconnect.server_full` |

`DedicatedPlayerList.isWhiteListed` (`apm.c`, line125) permits the profile when
whitelist use is disabled, `PlayerList.isOp(profile)` is true, or its UUID is a
member of `UserWhiteList`. This gate does not inspect `enforce-whitelist`.
The dedicated profile's operator check comes from the actual operator owner;
it is not a comparison against permission level3. `PlayerList.isOp` additionally
has integrated-owner/all-commands branches, which are not implied by this
dedicated server's `is_operator` input.

`DedicatedPlayerList.canBypassPlayerLimit` (`apm.d`, line135) calls
`ServerOpList.canBypassPlayerLimit` (`aus.a`, lines25–31), which reads
`ServerOpListEntry.bypassesPlayerLimit`. An op entry with a high permission level
but a false bypass flag still fails a full server's capacity gate. Pending
connections are not admitted players. The existing same-UUID owner still counts
at the initial gate, before duplicate removal. A negative maximum is a signed
Java value, not a large unsigned capacity.

`StoredUserList.contains` (`auw.d`, line82) does not remove expired entries;
`get` (`auw.b`, lines52–53) first removes expired entries from that entire list.
An expired target can therefore be present at contains and missing at get. The
pinned Java caller dereferences that missing entry. Native `BanLookup` keeps
`MissingAfterContains` as an explicit exceptional outcome; it is not silently
converted to permission to join or an invented ban message. Host integration
must preserve the source lookup order, including avoiding an IP-list purge when
an earlier player-ban/whitelist gate has already rejected the connection.

IP lookup retains the pinned `IpBanList.getIpFromAddress` behavior; the storage
module owns conversion from the actual socket address. It must not silently use
a normalized/proxied/header IP or repair the original first-colon truncation for
IPv6. That surprising source behavior is distinct from a desired future fix.

## Login and configuration boundaries

`ServerLoginPacketListenerImpl.verifyLoginAndFinishConnectionSetup` (`arw.c`,
lines152–170) calls `canPlayerLogin` **before** enabling compression, disconnecting
duplicate profiles and sending LoginSuccess. A banned or capacity-rejected login
must not evict the existing player. The successful path disconnects duplicate
profiles and, when necessary, waits for that real owner's removal before
`finishLoginAndWaitForClient` sends LoginSuccess.

`ServerConfigurationPacketListenerImpl.handleConfigurationFinished` (`ars.a`,
lines132–158) finishes the join-world task and changes the **outbound** codec to
`GameProtocols.CLIENTBOUND_TEMPLATE` before its final checks. It then rejects an
already-admitted UUID with `multiplayer.disconnect.duplicate_login`, reruns
`canPlayerLogin` against current access lists/player count, and only then creates
and places the player. Consequently the final rejection uses the PLAY disconnect
packet and anonymous network NBT; it is not a CONFIGURATION disconnect or a new
LOGIN failure. The gate also applies to actual return-to-world configuration.

The pure module supplies `can_finish_configuration` for this duplicate-first
ordering. The Main owner must hold the actual admission/registry ownership seam
around the final check and insertion; a check against an earlier snapshot is
insufficient. This module does not manufacture an admitted actor or pending-slot
reservation.

## Ban components and date formatting

For a finite ban, Java appends a translatable sibling to the reason component:
`multiplayer.disconnect.banned.expiration` or
`multiplayer.disconnect.banned_ip.expiration`. There is **no additional newline**
inserted by `canPlayerLogin`. Its sole argument is formatted with
`yyyy-MM-dd 'at' HH:mm:ss z` using the JVM's default locale/timezone.

Native rejections keep the reason and expiration milliseconds structured. The
bounded renderer produces LOGIN JSON or CONFIGURATION/PLAY anonymous NBT with
only these known translations, literal string arguments and an optional sibling.
It does not extend or claim a complete general Component codec. Direct inspection
of `TranslatableContents` (`yk`, mapped lines32–70) confirms the primitive-string
argument codec and optional `with` list; NBT string arguments use a homogeneous
TAG_String list, while the expiration sibling list contains TAG_Compound.

`format_ban_expiration_local` uses the native Unix host's local civil time and
short zone name, with Gregorian dates/English ASCII digits for the supported
dedicated runtime. `*_with_date_formatter` accepts an explicit date owner for a
different Java locale/timezone/calendar. Full locale/calendar equivalence is not
inferred from the native formatter. Milliseconds before an epoch second are
floored rather than truncated toward zero.

## Enforce-whitelist is a distinct operation

`MinecraftServer.kickUnlistedPlayers(CommandSourceStack)` (`a(et)`, mapped lines
1582–1595) first checks `isEnforceWhitelist`; when true it copies the admitted
player list and checks **direct `UserWhiteList.isWhiteListed(profile)` membership**
for each player. It does not call the admission predicate, does not inspect
whether whitelist use is enabled, and does not exempt operators. The policy's
`should_kick_unlisted(enforce_whitelist, whitelist_member)` deliberately preserves
this distinction. Commands must call this operation only at their actual source
callback phase; this pure helper does not itself schedule any kicks.

## Native API and limits

`src/admission_policy.rs` supplies `AdmissionView`, `BanLookup`, `BanView`,
`AdmissionRejection`, sequential ban/capacity helpers, the combined login gate,
the configuration duplicate-first gate, whitelist predicates and bounded wire
component rendering. Main integration, live access-list ownership, persistence,
duplicate cleanup, tick deadlines, commands and runtime evidence belong to their
respective implementation owners. This ledger is source evidence; it is not Main
connection proof, full authentication-delivery closure or a compatibility metric.
