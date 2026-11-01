# Java 1.21 UTF-8 string decoding

This stage ports the replacement-decoding behavior of network string fields.
It does not complete the connection, secure-chat or administration deliveries.

## Source identity and observations

The original server is pinned in `fixtures/source-lock.json`: server SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, official mappings SHA-1
`31c77994d96f05ba25a870ada70f47f315330437`, inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.

Direct JDK `javap` inspection identified
`net.minecraft.network.Utf8String.read(ByteBuf,int)` as `wp.a`. The original
method checks the encoded byte bound and available bytes, converts through
`ByteBuf.toString(index,length,StandardCharsets.UTF_8)`, advances the reader
index, and then checks `String.length()`. Its limit therefore counts UTF-16
units, including two units for a supplementary character. Bundled Netty is
4.1.97.Final. This evidence uses direct `javap`, not an REA MCP execution.

The Java conversion replaces malformed UTF-8 rather than rejecting the entire
field. Replacement grouping also differs from Rust's `String::from_utf8_lossy`:
an encoded surrogate or a truncated valid prefix produces one U+FFFD. A byte
which breaks a valid prefix is processed again as the start of the next unit.
Overlong encodings and code points above U+10FFFF have an invalid leading
prefix and leave their continuation bytes to be decoded separately.

## Native behavior

`protocol::Reader::string` now invokes a private Java-compatible replacement
decoder. Valid UTF-8 retains a direct native decoding path. Malformed sequences
preserve Java's consumption boundaries and U+FFFD grouping; no Rust string can
contain an unpaired surrogate. The existing API, encoded-byte bound and UTF-16
unit bound remain unchanged. The complete field is consumed before a decoded
character-limit error, while an encoded-byte-limit error consumes only the
length prefix. Later packet-specific validation still applies to the result.

Representative observed results below show UTF-16 units in hexadecimal. Input
bytes exclude the field's VarInt length prefix.

| Input bytes | Maximum Java units | Original and native result |
| --- | ---: | --- |
| `ED A0 80` | 1 | `FFFD`; field consumed |
| `ED A0` | 100 | `FFFD`; truncated prefix consumed together |
| `ED A0 41` | 100 | `FFFD 0041` |
| `E0 80 80` | 100 | `FFFD FFFD FFFD` |
| `F0 90 80` | 100 | `FFFD` |
| `F0 90 80 41` | 100 | `FFFD 0041` |
| `F4 90 80 80` | 100 | `FFFD FFFD FFFD FFFD` |
| `F0 90 80 80` | 2 | `D800 DC00` |
| `F0 90 80 80` | 1 | encoded-byte-bound error; prefix consumed |
| `41 42` | 1 | UTF-16-unit-bound error; field consumed |

## Focused validation

On `2026-11-01T19:49:43Z`, a private fixture invoked the unchanged original
`wp.a` method using its bundled Netty dependencies on Microsoft OpenJDK
`21.0.13.0.2+1-LTS`. A named-package launcher and isolated fixture class loader
preserved the original signed JAR instead of modifying it.

A private `rustc` harness compiled the actual updated `src/protocol.rs` against
the project's existing dependencies on Rust 1.99.1. Original and native output
matched for valid multibyte strings, supplementary characters, malformed
leading/continuation bytes, surrogate encodings, interrupted/truncated prefixes,
overlong/out-of-range sequences, length-limit outcomes and reader consumption.
Trailing sentinel bytes confirmed that decoding did not consume the next field.
The source file SHA-256 at this smoke was
`4281689a0f4b24832863045b250f6af3d51d9bf52bbef876b7d7a001f4fb9c7c`.

The fixture and raw disassembly remain private. No exhaustive byte-space,
graphical client, public network or whole-game compatibility result is claimed.
The integration coordinator owns the subsequent full Main compiler/runtime
checks and checklist accounting.
