# Native stored access lists

Target: the pinned Minecraft Java **1.21** dedicated server, protocol767.
This is source evidence for `src/access_lists.rs`, not a claim that all login
administration, all command behavior or the full fixed checklist item is done.

## Source inspected

The original inner server JAR has SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official mappings have SHA-1
`31c77994d96f05ba25a870ada70f47f315330437`.
The inspection used **JDK21 `javap -c -p` directly**, not an REA MCP invocation.
Raw disassembly and the Java artifacts remain outside the repository.

| Official owner | Obfuscated class | Inspected contracts |
| --- | --- | --- |
| `StoredUserList` | `auw` | `add`, `remove`, `get`, `contains`, `removeExpired`, `save`, `load` |
| `BanListEntry` | `aum` | constructors, dates/source/reason defaults, expiration, serialization |
| `UserBanList` / entry | `auy` / `auz` | UUID keys, profile construction and ban fields |
| `IpBanList` / entry | `auo` / `aup` | literal IP keys, socket-address substring, ban fields |
| `ServerOpList` / entry | `aus` / `aut` | UUID keys, permission level, player-limit bypass |
| `UserWhiteList` / entry | `ava` / `avb` | UUID keys, profile construction and serialization |

## Native callable owners

`AccessLists` owns four separate maps. Profile keys are actual `[u8;16]` UUIDs;
IP keys are the stored strings. `AccessProfile` carries both that UUID and the
stored name. Permission entries retain the loaded integer without clamping it
to0–4, and retain `bypassesPlayerLimit` separately.

The host selects the server directory and supplies wall-clock milliseconds:

- `AccessLists::load(directory, now_ms)` and `reload` are convenience methods.
- `load_players`, `load_ips`, `load_ops`, `load_whitelist` allow startup or
  commands to preserve their own per-file call ordering and warning policy.
- Matching `save_*` methods write the actual corresponding JSON file.
- `contains_player_ban` / `contains_ip_ban` do not remove expired entries.
- `get_player_ban` / `get_ip_ban` remove expired entries from that whole map
  before lookup. Expiration is strictly **expires < now**, not <=.

The host must preserve the source's separate contains/get calls. An expired
target can disappear between those calls; storage does not silently reinterpret
that source condition as an ordinary absent ban. See the admission-policy owner.

The list maps are mutable so command owners can preserve source order:
mutate the live map, then call that list's save. Java catches/logs save I/O
failure after mutation; the native save returns an error to the host without
undoing the in-memory change. A host must not claim a successful disk write
when it only changed the live map.

## File behavior and JSON identities

The file names are `banned-players.json`, `banned-ips.json`, `ops.json` and
`whitelist.json`. Existing lists load from UTF-8 JSON arrays. A missing file
leaves the map unchanged, including on reload. Once an existing file is read,
its map is cleared before JSON conversion and entry insertion. JSON `null` and
an empty document produce an empty map. Duplicate keys overwrite the earlier
entry, as Java `Map.put` does. Missing or unparsable UUID identity skips that
profile entry; names are retained rather than lowercased.

`parse_profile_uuid` accepts Java's short hexadecimal UUID groups as well as
canonical UUID text. Serialization emits the canonical lowercase hyphenated
UUID. Profile entries require both UUID and name properties. Present null or
unusable typed fields return an error rather than being turned into a fabricated
profile. Unknown JSON properties are ignored, as in the subclass constructors.

`BanMetadata::new` uses the actual host clock, source `(Unknown)` when absent,
reason `Banned by an operator.` when absent, and no expiry by default. Missing
or malformed creation dates use the supplied now; missing or malformed expiry
dates mean no expiry. That state serializes as `forever`. Source and reason
are independent strings and are never reconstructed from display text.

The supported date codec reads numeric-offset `yyyy-MM-dd HH:mm:ss Z`, variable
numeric widths, lenient calendar/time rollover and trailing unused input.
Formatting uses the process local time zone, matching the source's default-zone
date formatter rather than hardcoding UTC or preserving a loaded textual offset.
The native date representation is milliseconds; the file format has seconds.

`legacy_ip_key(SocketAddr)` preserves the shipped `IpBanList` substring rule:
strip through the slash, then truncate at the first colon. On IPv4 this removes
the port. It does **not** implement a corrected IPv6 address parser. Java prints
uncompressed IPv6 groups, so loopback `::1` produces the legacy key `[0`; using
Rust's compressed socket display would incorrectly produce `[` instead.

## Explicit remaining boundaries

- The native JSON reader supports strict JSON and common Gson primitive/single
  element coercions. Gson's entire lenient JSON grammar and malformed UTF-8
  replacement behavior are not implemented.
- Noncanonical named-time-zone date inputs, dates before1583 or after9999 and
  extreme floating operator-level inputs outside the exact native numeric
  profile return an explicit error. The full Java locale/calendar/decimal parser
  is not replaced by a silent fallback for these recognized unsupported cases.
- Map serialization and list enumeration do not establish Java HashMap bucket
  iteration order. Profile identity, duplicate overwrite and actual map ownership
  are separate from that remaining observable ordering boundary.
- Legacy `.txt` conversion, authenticated profile discovery/cache, root admission
  callsites, commands, kick scheduling and disconnect packets have other owners.
  This module does not fabricate any of those states.

`rustfmt` completed for this module. Compiler and focused production-flow
validation are performed centrally against the integrated source; this ledger
does not claim a compiler/runtime result that this worker did not run.
