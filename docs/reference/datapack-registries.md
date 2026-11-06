# Data-pack registries — `RegistryDataLoader` and `registry_data`

Target: Minecraft Java **1.21**, protocol **767**. Primary evidence is the unchanged original
`RegistryDataLoader` (the loading codecs over `RegistryOps<JsonOps>`) and `RegistrySynchronization`
(the network codecs over `NbtOps`), run through the local oracle (`tools/loot_oracle.java`
`registry-codec`, `export-data` and `export-ids`), and the CONFIGURATION packets the unchanged
server sent (`fixtures/configuration.json`). Pinned artifacts: see
[`fixtures/source-lock.json`](../../fixtures/source-lock.json). Java artifacts and decompiled text
stay outside Git.

Scope: `src/datapack/registries.rs`, work package WP-E1 **stage 2** (delivery `registries.5`,
**partial**). It claims **no** checklist point and **no** Main integration. Nothing outside the
module calls it. `main_configuration.rs` still sends the captured payloads.

## What is implemented

- `SYNCHRONIZED`: the eleven synchronized registries in the order they are sent.
- `load_registry(packs)`: `RegistryDataLoader.loadContentsFromManager` for one registry directory.
  - Packs are given lowest priority first. A later pack's file replaces an earlier pack's file with
    the same id.
  - Elements are ordered by `ResourceLocation` (path first, then namespace).
  - Each file is read with the lenient Gson reader (`components::lenient::parse`).
  - Errors are collected per element, and any error fails the load.
- `encode_element(registry, json, known)`: the loading codec's validation followed by the network
  form of one element as NBT. It covers biome (`Biome.NETWORK_CODEC` fields), chat type, trim
  pattern, trim material, wolf variant, painting variant, dimension type, damage type, banner pattern
  and jukebox song.
  - `KnownIds` supplies the fixed registries an element may name: items, sound events, biomes,
    armor materials and particle types.
- `registry_data_payload(registry, elements, known, omit)`: the `ClientboundRegistryDataPacket`
  payload.
  - An element the `omit` predicate selects is sent without data. This is how
    `RegistrySynchronization` treats an element from a pack the client already knows.

Behaviours the oracle forced (each was a real mismatch before it was fixed):

- **Compound member order is the codec's insertion order, then `HashMap` order.**
  - The bytes of a compound follow its `HashMap` iteration. Keys that share a bucket keep the order
    in which the codec inserted them.
  - A `RecordCodecBuilder` does not insert fields in declaration order. Every record's order was
    read off the original's `JsonOps` output, where insertion order is visible.
  - Examples: `DimensionType` starts with `infiniburn, effects, ambient_light`;
    `BiomeSpecialEffects` with `mood_sound, additions_sound, music`.
- **Defaults are omitted.**
  - Optional fields equal to their default are not written: `effects: hurt`,
    `death_message_type: default`, `temperature_modifier: none`, `grass_color_modifier: none`, an
    empty style and an empty armour-material override map.
  - A `constant` int provider is written as a bare int.
  - A biome list with one entry is written as a single id.
- **Gson number narrowing.**
  - An int field reads `Number.intValue()`, so `1.5` is `1` and `1e40` is `0`.
  - A long field (`fixed_time`) keeps the low 64 bits of the exact decimal, as
    `BigDecimal.longValue()` does.
- **Range checks are the original's.**
  - `POSITIVE_FLOAT` rejects an infinite length.
  - Dimension type: `coordinate_scale` must lie in `[1e-5, 3e7]`, and the `guardY` rules apply
    (multiples of 16, `min_y + height <= 2032`, `logical_height <= height`).
  - Painting width and height must lie in `1..=16`, and comparator output in `0..=15`.
- **`orElse` fields accept anything.** A missing or unreadable `decal` is `false`, and a missing or
  unreadable dimension `effects` is `minecraft:overworld`.
- **A chat decoration `style` must be an object.**

## Evidence

| Check | Source | Result |
| --- | --- | --- |
| Vanilla packets | the vanilla data pack JSON (`fixtures/datapack-vanilla-registries-1.21.json`, from `export-data`) loaded and encoded for a client that knows no packs | all ten non-enchantment `registry_data` payloads equal the captured packets byte for byte. For enchantment, the ids and order match. |
| Element mutants | `fixtures/datapack-registry-vectors.json` (`tools/datapack_registries_generate.py`): 6,000 one-step mutants of every shipped element (field deleted, replaced by a value from a fixed list, extra field, lenient-text variants) | the port accepts and rejects exactly what the original does, and accepted elements are written to the same NBT. A local run of all 107,366 mutants (22,594 accepted by the original) agrees too. |

Test: `cargo test --locked --lib datapack::registry_tests`. Point `DATAPACK_REGISTRY_VECTORS` at a
full generator run (`--per-registry 0`) to check every mutant.

## Known gaps

- **Enchantment element codec.** The ids are loaded and ordered, but `encode_element` answers
  `Unsupported`. A client that knows the vanilla pack, which is every vanilla client, receives no
  enchantment data, so this only matters for enchantments from a custom pack.
- **Biome generation and spawn settings.** The loading codec also reads `features`, `carvers`,
  `spawners` and `spawn_costs`. The port does not validate them, so a biome that the original
  rejects for those fields alone loads here. Mutants touch only the fields a client receives.
- **Unported variants answer `Unsupported`.** They are 49 of 107,366 mutants (6 of the 6,000
  committed):
  - particle options other than the simple particle types (for example `dust`);
  - int providers other than `constant`, `uniform` and `biased_to_bottom`.
- **No consumer.**
  - The CONFIGURATION phase still replays the captured payloads.
  - Choosing `omit` from the client's known packs (`ServerboundSelectKnownPacks`), reload
    (stage 4) and registry consumers (stage 5) are open.
