//! SURFACE callbacks over an actual quart-biome provider and thermal owner.
//!
//! The adapter performs no biome selection or chunk generation. Its provider
//! must already own the required BIOMES data and report unavailable neighbors
//! as errors. The borrowed thermal owner retains this worker's cache history.

use super::{
    biome_temperature::BiomeTemperature,
    biome_zoom::{BiomeZoom, QuartBiomeProvider},
    generator::SurfaceContext,
};
use crate::density::DensityPoint;
use anyhow::Result;

/// Built-in biome-registry SURFACE context for one synchronous generation worker.
///
/// Supply the real dimension's zoom seed/provider and reuse its worker-local
/// `BiomeTemperature` across chunk tasks. The mutable borrow preserves that
/// owner's `!Send`/`!Sync` boundary; this context does not belong in the shared
/// generator. The provider must certify the pinned built-in registry contents:
/// names alone cannot detect a datapack override under an existing key. Custom
/// biome climates require a different thermal implementation.
pub struct BuiltinBiomeSurfaceContext<'a, P: QuartBiomeProvider<Biome = String> + ?Sized> {
    zoom: BiomeZoom,
    provider: &'a mut P,
    temperature: &'a mut BiomeTemperature,
}

impl<'a, P: QuartBiomeProvider<Biome = String> + ?Sized> BuiltinBiomeSurfaceContext<'a, P> {
    /// Use an explicit BiomeManager zoom owner without hashing its seed again.
    pub fn new(
        zoom: BiomeZoom,
        provider: &'a mut P,
        temperature: &'a mut BiomeTemperature,
    ) -> Self {
        Self {
            zoom,
            provider,
            temperature,
        }
    }

    /// Use the known real world seed, applying its zoom obfuscation exactly once.
    pub fn from_world_seed(
        world_seed: i64,
        provider: &'a mut P,
        temperature: &'a mut BiomeTemperature,
    ) -> Self {
        Self::new(
            BiomeZoom::from_world_seed(world_seed),
            provider,
            temperature,
        )
    }
}

impl<P: QuartBiomeProvider<Biome = String> + ?Sized> SurfaceContext
    for BuiltinBiomeSurfaceContext<'_, P>
where
    P::Error: Into<anyhow::Error>,
{
    fn biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
        let supplied = self
            .zoom
            .get_biome(&mut *self.provider, x, y, z)
            .map_err(Into::<anyhow::Error>::into)?;
        // BiomeManager lookup must not evaluate a temperature or warm its
        // packed-position cache. SurfaceRules requests thermal predicates later.
        Ok(self.temperature.canonical_biome(&supplied)?.to_owned())
    }

    fn cold_enough_to_snow(&mut self, biome: &str, x: i32, y: i32, z: i32) -> Result<bool> {
        self.temperature
            .cold_enough_to_snow(biome, DensityPoint::new(x, y, z))
    }

    fn should_melt_frozen_ocean_iceberg_slightly(
        &mut self,
        biome: &str,
        x: i32,
        y: i32,
        z: i32,
    ) -> Result<bool> {
        self.temperature
            .should_melt_frozen_ocean_iceberg_slightly(biome, DensityPoint::new(x, y, z))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct ProviderFailure;
    impl std::fmt::Display for ProviderFailure {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("actual quart owner unavailable")
        }
    }
    impl std::error::Error for ProviderFailure {}

    /// Controlled provider for the adapter contract, not natural biome selection.
    struct QuartWitness {
        biome: String,
        unavailable: bool,
        calls: Vec<(i32, i32, i32)>,
    }
    impl QuartWitness {
        fn new(biome: &str) -> Self {
            Self {
                biome: biome.to_owned(),
                unavailable: false,
                calls: Vec::new(),
            }
        }
    }
    impl QuartBiomeProvider for QuartWitness {
        type Biome = String;
        type Error = ProviderFailure;

        fn get_noise_biome(
            &mut self,
            quart_x: i32,
            quart_y: i32,
            quart_z: i32,
        ) -> std::result::Result<String, ProviderFailure> {
            self.calls.push((quart_x, quart_y, quart_z));
            if self.unavailable {
                Err(ProviderFailure)
            } else {
                Ok(self.biome.clone())
            }
        }
    }

    #[test]
    fn original_selected_quart_and_thermal_callbacks_use_their_actual_owners() -> Result<()> {
        let mut provider = QuartWitness::new(":frozen_ocean");
        let mut temperature = BiomeTemperature::builtin()?;
        let mut context =
            BuiltinBiomeSurfaceContext::from_world_seed(0, &mut provider, &mut temperature);
        let biome = context.biome(0, 0, 0)?;
        assert_eq!(biome, "minecraft:frozen_ocean");

        // Actual original thermal observations for this biome, independently
        // recorded in world-generation-biome-temperature-vectors.json.
        assert!(!context.cold_enough_to_snow(&biome, 0, 63, 0)?);
        assert!(context.should_melt_frozen_ocean_iceberg_slightly(&biome, 0, 63, 0)?);
        assert!(context.cold_enough_to_snow(&biome, 37, 256, -103)?);
        assert!(!context.should_melt_frozen_ocean_iceberg_slightly(&biome, 37, 256, -103)?);
        // Original BiomeManager query in world-generation-biome-zoom.json:
        // world seed 0, block (0,0,0) selects quart (-1,0,-1) exactly once.
        // Thermal callbacks above must not query a replacement biome provider.
        assert_eq!(provider.calls, [(-1, 0, -1)]);
        Ok(())
    }

    #[test]
    fn biome_lookup_does_not_populate_the_thermal_cache() -> Result<()> {
        let mut provider = QuartWitness::new("plains");
        let mut temperature = BiomeTemperature::builtin()?;
        let mut context = BuiltinBiomeSurfaceContext::new(
            BiomeZoom::from_zoom_seed(8794265229978523055),
            &mut provider,
            &mut temperature,
        );
        let biome = context.biome(0, 4096, 0)?;
        assert_eq!(biome, "minecraft:plains");
        // The original packed key aliases Y=4096 and Y=0. Eagerly sampling
        // temperature during biome() would cache the high cold temperature and
        // make this low-altitude predicate wrong, as the original alias fixture
        // in world-generation-biome-temperature-vectors.json demonstrates.
        assert!(!context.cold_enough_to_snow(&biome, 0, 0, 0)?);
        assert!(context.should_melt_frozen_ocean_iceberg_slightly(&biome, 0, 0, 0)?);
        assert_eq!(provider.calls.len(), 1);
        Ok(())
    }

    #[test]
    fn unavailable_provider_and_unknown_biome_are_errors() -> Result<()> {
        let mut temperature = BiomeTemperature::builtin()?;
        let mut provider = QuartWitness::new("plains");
        provider.unavailable = true;
        let mut context =
            BuiltinBiomeSurfaceContext::from_world_seed(0, &mut provider, &mut temperature);
        let error = context.biome(0, 0, 0).unwrap_err();
        assert!(error.downcast_ref::<ProviderFailure>().is_some());
        assert_eq!(provider.calls, [(-1, 0, -1)]);

        let mut provider = QuartWitness::new("custom:unregistered");
        let mut context =
            BuiltinBiomeSurfaceContext::from_world_seed(0, &mut provider, &mut temperature);
        assert!(context
            .biome(0, 0, 0)
            .unwrap_err()
            .to_string()
            .contains("unsupported biome climate custom:unregistered"));
        assert!(context
            .cold_enough_to_snow("custom:unregistered", 0, 63, 0)
            .is_err());
        assert!(context
            .should_melt_frozen_ocean_iceberg_slightly("custom:unregistered", 0, 63, 0)
            .is_err());
        assert_eq!(provider.calls, [(-1, 0, -1)]);
        Ok(())
    }
}
