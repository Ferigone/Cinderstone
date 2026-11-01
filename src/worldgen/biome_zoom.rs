//! BiomeManager's seeded block-to-quart zoom over an actual biome owner.
//!
//! This selects one source coordinate, not a biome. The provider must supply
//! the real quart biome, including neighbor availability and its own Y clamp.
use crate::world::obfuscate_biome_seed;

/// A real quart-biome source. Missing chunks/BIOMES state must return an error;
/// the zoom helper never replaces them with a fixed or inferred biome.
pub trait QuartBiomeProvider {
    type Biome;
    type Error;

    fn get_noise_biome(
        &mut self,
        quart_x: i32,
        quart_y: i32,
        quart_z: i32,
    ) -> Result<Self::Biome, Self::Error>;
}

/// Immutable BiomeManager zoom seed; it consumes no sequential world RNG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BiomeZoom {
    zoom_seed: i64,
}

impl BiomeZoom {
    /// Construct from the real world seed, obfuscating it exactly once.
    pub fn from_world_seed(world_seed: i64) -> Self {
        Self::from_zoom_seed(obfuscate_biome_seed(world_seed))
    }

    /// Construct from an already-obfuscated BiomeManager/client zoom seed.
    pub const fn from_zoom_seed(zoom_seed: i64) -> Self {
        Self { zoom_seed }
    }

    pub const fn zoom_seed(self) -> i64 {
        self.zoom_seed
    }

    /// BiomeManager.getBiome: evaluate all eight corner distances, then make
    /// exactly one query to the mandatory real quart-biome provider.
    pub fn get_biome<P: QuartBiomeProvider + ?Sized>(
        &self,
        provider: &mut P,
        block_x: i32,
        block_y: i32,
        block_z: i32,
    ) -> Result<P::Biome, P::Error> {
        let (x, y, z) = self.selected_quart(block_x, block_y, block_z);
        provider.get_noise_biome(x, y, z)
    }

    /// The coordinate selected by the original seeded eight-corner zoom.
    /// This does not query, clamp or fabricate a biome owner.
    pub fn selected_quart(&self, block_x: i32, block_y: i32, block_z: i32) -> (i32, i32, i32) {
        // Java subtracts in int before the arithmetic right shift. In
        // particular, MIN_VALUE/MIN_VALUE+1 wrap rather than saturating.
        let shifted = [
            block_x.wrapping_sub(2),
            block_y.wrapping_sub(2),
            block_z.wrapping_sub(2),
        ];
        let base = shifted.map(|coordinate| coordinate >> 2);
        let fraction = shifted.map(|coordinate| f64::from(coordinate & 3) / 4.0);
        let best = nearest_corner(|corner| {
            let (position, offset) = corner_position(base, fraction, corner);
            fiddled_distance(self.zoom_seed, position, offset)
        });
        let (position, _) = corner_position(base, fraction, best);
        (position[0], position[1], position[2])
    }
}

fn corner_position(base: [i32; 3], fraction: [f64; 3], corner: usize) -> ([i32; 3], [f64; 3]) {
    let upper = [corner & 4 != 0, corner & 2 != 0, corner & 1 != 0];
    let mut position = base;
    let mut offset = fraction;
    for axis in 0..3 {
        if upper[axis] {
            position[axis] = position[axis].wrapping_add(1);
            offset[axis] -= 1.0;
        }
    }
    (position, offset)
}

fn nearest_corner(mut distance: impl FnMut(usize) -> f64) -> usize {
    let mut nearest = 0;
    let mut minimum = f64::INFINITY;
    for corner in 0..8 {
        let value = distance(corner);
        // The original strict comparison retains the first corner on ties.
        if minimum > value {
            nearest = corner;
            minimum = value;
        }
    }
    nearest
}

fn next(seed: i64, salt: i64) -> i64 {
    seed.wrapping_mul(
        seed.wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407),
    )
    .wrapping_add(salt)
}

fn fiddle(seed: i64) -> f64 {
    (f64::from((seed >> 24).rem_euclid(1024) as i32) / 1024.0 - 0.5) * 0.9
}

fn fiddled_distance(zoom_seed: i64, position: [i32; 3], offset: [f64; 3]) -> f64 {
    let mut state = zoom_seed;
    for coordinate in position.into_iter().chain(position) {
        state = next(state, i64::from(coordinate));
    }
    let x = fiddle(state);
    state = next(state, zoom_seed);
    let y = fiddle(state);
    state = next(state, zoom_seed);
    let z = fiddle(state);
    let z = offset[2] + z;
    let y = offset[1] + y;
    let x = offset[0] + x;
    // Preserve the original square(Z) + square(Y) + square(X) association.
    z * z + y * y + x * x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_coordinate_queries_and_seed_obfuscation() {
        struct CoordinateOwner(Vec<(i32, i32, i32)>);
        impl QuartBiomeProvider for CoordinateOwner {
            type Biome = (i32, i32, i32);
            type Error = std::convert::Infallible;
            fn get_noise_biome(
                &mut self,
                x: i32,
                y: i32,
                z: i32,
            ) -> Result<Self::Biome, Self::Error> {
                self.0.push((x, y, z));
                Ok((x, y, z))
            }
        }
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../fixtures/world-generation-biome-zoom.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let zoom_seed = case["zoom_seed"].as_str().unwrap().parse().unwrap();
            let zoom = BiomeZoom::from_zoom_seed(zoom_seed);
            assert_eq!(zoom.zoom_seed(), zoom_seed);
            if let Some(world_seed) = case["world_seed"].as_str() {
                assert_eq!(
                    BiomeZoom::from_world_seed(world_seed.parse().unwrap()),
                    zoom
                );
            }
            for query in case["queries"].as_array().unwrap() {
                let block = query["block"].as_array().unwrap();
                let expected = query["selected_quart"].as_array().unwrap();
                assert_eq!(
                    zoom.selected_quart(
                        block[0].as_i64().unwrap() as i32,
                        block[1].as_i64().unwrap() as i32,
                        block[2].as_i64().unwrap() as i32,
                    ),
                    (
                        expected[0].as_i64().unwrap() as i32,
                        expected[1].as_i64().unwrap() as i32,
                        expected[2].as_i64().unwrap() as i32,
                    ),
                    "{query} seed {zoom_seed}"
                );
                let mut provider = CoordinateOwner(Vec::new());
                let returned = zoom
                    .get_biome(
                        &mut provider,
                        block[0].as_i64().unwrap() as i32,
                        block[1].as_i64().unwrap() as i32,
                        block[2].as_i64().unwrap() as i32,
                    )
                    .unwrap();
                assert_eq!(
                    returned,
                    (
                        expected[0].as_i64().unwrap() as i32,
                        expected[1].as_i64().unwrap() as i32,
                        expected[2].as_i64().unwrap() as i32,
                    )
                );
                assert_eq!(provider.0, vec![returned]);
                assert_eq!(query["provider_calls"].as_u64(), Some(1));
            }
        }
    }

    #[test]
    fn original_fiddle_distances_and_wrapping_lcg_bits() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../fixtures/world-generation-biome-zoom.json"
        ))
        .unwrap();
        for probe in fixture["distance_probes"].as_array().unwrap() {
            let position =
                std::array::from_fn(|axis| probe["quart"][axis].as_i64().unwrap() as i32);
            let offset = std::array::from_fn(|axis| probe["offset"][axis].as_f64().unwrap());
            let seed = probe["zoom_seed"].as_str().unwrap().parse().unwrap();
            let expected: u64 = probe["bits"].as_str().unwrap().parse().unwrap();
            assert_eq!(
                fiddled_distance(seed, position, offset).to_bits(),
                expected,
                "{probe}"
            );
        }
        for probe in fixture["fiddle_probes"].as_array().unwrap() {
            let seed = probe["seed"].as_str().unwrap().parse().unwrap();
            let expected: u64 = probe["bits"].as_str().unwrap().parse().unwrap();
            assert_eq!(fiddle(seed).to_bits(), expected, "{probe}");
        }
        for probe in fixture["lcg_probes"].as_array().unwrap() {
            let seed = probe["seed"].as_str().unwrap().parse().unwrap();
            let salt = probe["salt"].as_str().unwrap().parse().unwrap();
            let expected: i64 = probe["next"].as_str().unwrap().parse().unwrap();
            assert_eq!(next(seed, salt), expected, "{probe}");
        }
        assert_eq!(fixture["provider_failure"]["propagated"], true);
        assert_eq!(fixture["provider_failure"]["provider_calls"], 1);
    }

    #[test]
    fn provider_failure_is_returned_after_one_actual_query() {
        struct MissingOwner(Vec<(i32, i32, i32)>);
        impl QuartBiomeProvider for MissingOwner {
            type Biome = String;
            type Error = &'static str;
            fn get_noise_biome(&mut self, x: i32, y: i32, z: i32) -> Result<String, &'static str> {
                self.0.push((x, y, z));
                Err("actual BIOMES neighbor unavailable")
            }
        }
        let zoom = BiomeZoom::from_zoom_seed(-1);
        let mut provider = MissingOwner(Vec::new());
        assert_eq!(
            zoom.get_biome(&mut provider, -17, 319, 16),
            Err("actual BIOMES neighbor unavailable")
        );
        assert_eq!(provider.0, vec![zoom.selected_quart(-17, 319, 16)]);
    }

    #[test]
    fn all_corners_are_considered_and_ties_keep_the_first() {
        let mut visited = Vec::new();
        let result = nearest_corner(|corner| {
            visited.push(corner);
            if corner == 3 || corner == 6 {
                0.0
            } else {
                1.0
            }
        });
        assert_eq!(visited, (0..8).collect::<Vec<_>>());
        assert_eq!(result, 3);
        assert_eq!(nearest_corner(|_| 1.0), 0);
    }
}
