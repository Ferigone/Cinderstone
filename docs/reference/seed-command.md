# Seed command (#179)

Dedicated-server console and permission-level2 players can execute `seed`
through the native Brigadier dispatcher. Its tree is one executable literal;
arguments are rejected. Suggestions preserve the existing command roots.

Main retains an immutable optional raw seed at startup. An imported source must
contain the canonical `Data.WorldGenSettings.seed` long; missing seed provenance
is not replaced with `LevelData::seed()`'s legacy default. The native flat
profile declares seed0, as used by the existing parity scenarios. The captured
biome hash checks that declaration only; another captured hash yields unavailable
provenance. No hash is inverted or returned as a seed, and no seed-change or new
generator feature is introduced.

Success matches `SeedCommand` and `ComponentUtils.copyOnClickText`: a green seed
inside translated square brackets, copy-to-clipboard, insertion, and the
`chat.copy.click` tooltip. It returns the low32 bits of the raw long, as Java's
`l2i` does. The full Component codec handles wire encoding. Success goes only to
the actual source, respects the player's `sendCommandFeedback` rule, and never
broadcasts to other operators. Console feedback remains available when that
rule is false. Unknown provenance is reported explicitly.

## Evidence

`tools/seed_command_oracle.java` executes the original registration, result and
system-chat codec, with recorded command feedback and an original `WorldOptions`
provider. The small fixture covers permission trees0–4, six seed boundaries,
denial and extra/trailing input. Regenerate it with
`tools/seed_command_vectors.py --named-jar ... --original-jar ... --libraries ...
--out fixtures/seed-command-vectors.json`, using the existing pinned bootstrap
and remapper; keep proprietary artifacts outside the checkout.

Run `node tools/seed_command_flow.mjs BINARY PINNED_JAR EXISTING_EULA REPORT`
after `npm ci`. It uses isolated temporary world/admin directories and ports
25641/25642 and requires existing EULA acceptance. Ordinary Java commands prepare
the imported scene; normal shutdown is required. It does not touch Railway/data.

[The real Java/native report](../validation/seed-command-runtime.json) compares
full feedback bytes, the usable command slice and completions for an imported
seed `-1234567890123456789`. It checks non-op denial, privacy from another op,
console execution, feedback suppression, unchanged saved seed and cold restart.
The fresh-native seed0 response also agrees with the original class witness and
survives restart. Tests compare original result/component bytes, reject unknown
provenance and verify that actual Main console reads leave the retained clock
and level RNG unchanged.

Single-player/integrated permissions, seed changes, new generators/dimensions
and the rest of WP-E3 remain out of scope. Scope1/100 and checklist counts58/53
stay unchanged. Hosted deployment and public TCP checks remain tracked in #19.
