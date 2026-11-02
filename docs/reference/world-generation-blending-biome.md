# Blender biome selection

Target: unchanged Minecraft Java **1.21**, protocol **767**, pinned by
`fixtures/source-lock.json`. Inner server SHA-256:
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Inspection used direct Java 21 `javap`; no REA execution is claimed.

`Blender` maps to `eaa`. Relevant official mappings are `getBiomeResolver`
line 261, its resolver callback lines 262–266, `blendBiome` lines 272–304,
and its collection callbacks lines 276–291. `BlendingData.iterateBiomes`
(`eab`) maps to lines 321–334, with `getBiomeColumn` at lines 252–258.

The selector visits the actual `heightAndBiomeBlendingData` map in its original
Fastutil `forEach` encounter order. Each packed chunk key supplies the base
quart X/Z by shifting its signed chunk coordinates left two bits. The query's
quart Y is passed unchanged to `BlendingData.iterateBiomes`. That owner checks
the actual old-generation height span, then visits its column slots in ascending
index order, skipping absent columns or biome holders. Its columns contain
actual `ChunkAccess.getNoiseBiome` observations, not a fresh biome-source or
zoom query. The parent data owner supplies these values and encounter order.

The callback subtracts quart X/Z as wrapping Java ints before widening, then
uses `sqrt(dx*dx + dz*dz)`. Candidates farther than 27 quart cells are ignored.
The nearest candidate replaces its predecessor only on strict `<`; equal
distances keep the first map/column observation. Collection finishes before
the shift noise is sampled. With no candidate, the selector returns absent
without sampling shift noise.

The original static initializer constructs `NormalNoise.create` with a direct
`XoroshiroRandomSource(42)` and `NoiseData.DEFAULT_SHIFT` (`qu.a`), whose
first octave is -3 and amplitudes are `[1,1,1,0]`. These parameters also appear
in the pinned noise fixture's `minecraft:offset` row. This is a separate fixed
source, without a positional name hash, world seed, Level RNG or process seed
uniquifier. The native shared `shift_noise()` owner preserves that constructor
and makes no global unique-seed call.

For a nearest candidate, the selector computes
`clamp((distance + SHIFT_NOISE(quartX, 0, quartZ) * 12) / 28, 0, 1)` and
returns the old biome precisely when that value is at most 0.5. It does not
change Y or quantize noise. `blend_biome(&Blender, x, y, z)` exposes this optional
selection.

`BlendedBiomeResolver::new(&Blender, &mut actual_resolver)` implements the
existing `biomes::BiomeResolver` boundary. It first attempts old-data selection.
Only an absent result calls the supplied resolver, once, with unchanged quart
X/Y/Z and the same mutable `PreparedGeneration`. Old-biome success performs no
new-source or climate callback. The original EMPTY Blender returns its supplied
resolver directly; the parent may preserve that shortcut. A wrapper over empty
data still delegates every query without extra source or climate calls.

This module supplies the selection and callback boundary. The parent owns real
region/data construction, retained generator integration and validation evidence;
this source file alone does not establish a Main call or completed chunk status.
