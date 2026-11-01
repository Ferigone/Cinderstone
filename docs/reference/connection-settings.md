# Dedicated connection property values

Target: Minecraft Java 1.21, protocol 767. The server and official mappings are
pinned by `fixtures/source-lock.json`: server SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, mapping SHA-1
`31c77994d96f05ba25a870ada70f47f315330437`. The extracted inner server SHA-256
is `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.

This stage used direct JDK 21 `javap -p -c` and bootstrap-method inspection
against the original classes. It did not use an REA MCP tool. A small runtime
probe invoked the original `Settings` getters through an injected subclass;
the original server classes were not modified. Raw disassembly, the private
probe and proprietary binaries remain outside the repository.

## Source contracts

| Original owner | Observed behavior |
| --- | --- |
| `DedicatedServerProperties` constructor (`apo`, source lines 52–138; bytecode 428–435) | Reads `rate-limit` through the plain integer getter with default `0`. No clamp is applied by this getter. |
| Same constructor, bytecode 479–488 | Reads `network-compression-threshold` through the plain integer getter with default `256`. Negative values remain negative; zero is valid. |
| Same constructor, bytecode 653–660 | Reads configured `enforce-secure-profile` through the boolean getter with default `true`. Effective enforcement belongs to separate online-mode and service-key owners. |
| `Settings.get(String,int)` (`apr.a`, source line 167) and `wrapNumberDeserializer` (lines 94–98) | Uses `Integer.parseInt`; `NumberFormatException` becomes null and selects the default. Input is not trimmed. ASCII signs, leading zeros and BMP decimal digits are accepted. Overflow, whitespace and supplementary digits fail. |
| `Settings.get(String,boolean)` (`apr.a`, line 183) | Uses `Boolean.valueOf(String)`. A supplied value other than case-insensitive `true` becomes false, including malformed text; only a missing value selects the default. |
| `Settings.get` generic overload (lines 129–132) | Reads the raw string, parses or selects the default, and stores the canonical serialized result back into its actual `Properties` map. This is map normalization, not an extra disk-write operation. |

## Native API and ownership

`src/connection_settings.rs` exports `DedicatedConnectionSettings` with the
three typed fields above. `from_values` accepts already-decoded optional values;
`from_lookup` reads each key in the original constructor order.
`try_from_lookup` propagates an existing reader's I/O or invalid-escape failure
instead of hiding it as an integer fallback. `canonical_property_values` returns
the normalized strings for the actual property-map owner.

The module does not open or rewrite `server.properties`. Main already has a
Java-properties lexical reader in `main_administration`; the integration
coordinator owns exposing/reusing that reader, selecting startup precedence,
preserving explicit CLI configuration and constructing one packet-rate owner
per real connection. No separate timeout setting, file writer or environment
owner is introduced here. This standalone module does not prove Main wiring or
completion of `authentication.5`.

## Focused validation

The new module compiled directly with Rust 1.99.1. Its results and canonical
strings matched the actual original `Settings` integer and boolean getters for
the bounded parsing probe: signed limits and overflow, empty/sign-only values,
leading zeros, whitespace, Arabic-Indic/fullwidth digits, supplementary digits,
mixed-case true and malformed booleans. Examples include `+0010` becoming `10`,
`2147483648` selecting the integer default, and `true ` becoming false.
The BMP decimal-digit ranges used by the native parser were also obtained from
the actual JDK 21 `Character.digit(char, 10)` behavior.

This is evidence for these typed getters, not for the entire properties-file
loader, full dedicated-server settings, production transport or vanilla parity.
Main compiler/runtime evidence is recorded separately by the coordinator.
