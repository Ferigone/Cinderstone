//! Controlled actual BIOMES -> NOISE -> SURFACE -> AIR CARVERS owners.
use anyhow::{anyhow, ensure, Context, Result};
use mc121_rust::worldgen::{
    biome_temperature::BiomeTemperature,
    biome_zoom::{BiomeZoom, QuartBiomeProvider},
    biomes::{BiomeResolver, BuiltinBiomeResolver, PreparedBiomes, RetrogenContext},
    carver::{
        AirCarvingContext, AirMaskOwner, BuiltinCarverRegistry, CarverBiomeCache,
        CarverBiomeSource, CarvingMaskFilter, CarvingNeighbors, CarvingRegionFacts,
    },
    BuiltinMultiNoiseBiomeSource, FreshGenerationContext, GenerationInfluence, MultiNoisePreset,
    NoiseGenerator, OwnedNoiseTerrain, PreparedGeneration,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::{self, BufRead},
    sync::Arc,
};

struct ObserveResolver<'a> {
    source: BuiltinBiomeResolver<'a>,
    queries: Vec<[i32; 3]>,
}
impl BiomeResolver for ObserveResolver<'_> {
    fn get_noise_biome(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        sampler: &mut PreparedGeneration,
    ) -> Result<String> {
        self.queries.push([x, y, z]);
        BiomeResolver::get_noise_biome(&mut self.source, x, y, z, sampler)
    }
}
struct Neighbors {
    images: BTreeMap<(i32, i32), PreparedBiomes>,
    queries: Vec<[i32; 3]>,
}
impl QuartBiomeProvider for Neighbors {
    type Biome = String;
    type Error = anyhow::Error;
    fn get_noise_biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
        self.queries.push([x, y, z]);
        let image = self
            .images
            .get(&(x.div_euclid(4), z.div_euclid(4)))
            .context("unavailable actual BIOMES neighbor image")?;
        image.biomes().get_noise_biome(x, y, z).map(str::to_owned)
    }
}
fn image(terrain: &OwnedNoiseTerrain) -> Result<(Vec<u8>, Value)> {
    let (cx, cz) = terrain.chunk();
    let min = terrain.min_y();
    let height = terrain.height();
    let mut blocks = Vec::new();
    for y in min..min + height {
        for x in 0..16 {
            for z in 0..16 {
                blocks.extend(terrain.block(x, y, z)?.to_le_bytes());
            }
        }
    }
    let first_section = min.div_euclid(16);
    let sections = (min + height - 1).div_euclid(16) - first_section + 1;
    let mut packed = vec![Vec::<i16>::new(); sections as usize];
    let mut positions = vec![Vec::<[i32; 3]>::new(); sections as usize];
    for p in terrain.postprocessing() {
        ensure!(
            p.x.div_euclid(16) == cx
                && p.z.div_euclid(16) == cz
                && (min..min + height).contains(&p.y),
            "postprocessing left owned chunk"
        );
        let section = (p.y.div_euclid(16) - first_section) as usize;
        packed[section].push(((p.x & 15) | ((p.y & 15) << 4) | ((p.z & 15) << 8)) as i16);
        positions[section].push([p.x, p.y, p.z]);
    }
    Ok((
        blocks,
        json!({"world_surface_wg":terrain.world_surface_wg().to_vec(),"ocean_floor_wg":terrain.ocean_floor_wg().to_vec(),"postprocessing_count":terrain.postprocessing().len(),"postprocessing_sections":packed,"postprocessing_positions_by_section":positions}),
    ))
}
struct CarverNeighbors {
    caches: BTreeMap<(i32, i32), CarverBiomeCache>,
}
impl CarvingNeighbors for CarverNeighbors {
    fn carver_biome_cache(&mut self, chunk: (i32, i32)) -> Result<&mut CarverBiomeCache> {
        self.caches
            .get_mut(&chunk)
            .context("actual controlled CARVERS neighbor unavailable")
    }
}
struct FreshFacts {
    target: (i32, i32),
    min: i32,
    height: i32,
}
impl CarvingRegionFacts for FreshFacts {
    fn generation_height(&mut self, target: (i32, i32)) -> Result<(i32, i32)> {
        ensure!(target == self.target, "different target accessor");
        Ok((self.min, self.height))
    }
    fn is_upgrading(&mut self, target: (i32, i32)) -> Result<bool> {
        ensure!(target == self.target, "different target upgrade owner");
        Ok(false)
    }
}
fn main() -> Result<()> {
    let directory = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .context("usage: carver_probe <output-directory>")?,
    );
    std::fs::create_dir_all(&directory)?;
    // Same worker/preset cursor and original built-in biome cache history across
    // all cases. Neither mutable owner is moved into a shared generator.
    let mut overworld = BuiltinMultiNoiseBiomeSource::new(MultiNoisePreset::Overworld)?;
    let mut nether = BuiltinMultiNoiseBiomeSource::new(MultiNoisePreset::Nether)?;
    let mut thermal = BiomeTemperature::builtin()?;
    for line in io::stdin().lock().lines() {
        let q: Value = serde_json::from_str(&line?)?;
        let settings = q["settings"].as_str().context("settings")?;
        let source_name = q["biome_source"]
            .as_str()
            .context("explicit biome source")?;
        let seed = q["seed"].as_i64().context("seed")?;
        let cx = q["x"].as_i64().context("chunk x")? as i32;
        let cz = q["z"].as_i64().context("chunk z")? as i32;
        let min = q["level_min_y"].as_i64().context("level minimum")? as i32;
        let height = q["level_height"].as_i64().context("level height")? as i32;
        let generator = Arc::new(NoiseGenerator::builtin(seed, settings)?);
        let legacy = generator.graph().uses_legacy_random_source();
        let weak = Arc::downgrade(&generator);
        let mut chunks = Vec::new();
        let mut palettes = Vec::new();
        let mut resolver_queries = Vec::new();
        let mut neighbors = Neighbors {
            images: BTreeMap::new(),
            queries: Vec::new(),
        };
        let mut center = None;
        for dx in -1..=1 {
            for dz in -1..=1 {
                let x = cx + dx;
                let z = cz + dz;
                let empty = GenerationInfluence::ObservedEmpty;
                let prepared = generator.prepare_chunk(
                    x,
                    z,
                    min,
                    height,
                    FreshGenerationContext {
                        initial_blocks: empty,
                        blender: empty,
                        structure_beardifier: empty,
                    },
                )?;
                let source = match source_name {
                    "minecraft:overworld" => BuiltinBiomeResolver::MultiNoise(&mut overworld),
                    "minecraft:nether" => BuiltinBiomeResolver::MultiNoise(&mut nether),
                    "minecraft:end" => BuiltinBiomeResolver::End,
                    _ => return Err(anyhow!("unsupported explicit builtin biome source")),
                };
                let mut resolver = ObserveResolver {
                    source,
                    queries: Vec::new(),
                };
                let biomes =
                    PreparedBiomes::fill(prepared, RetrogenContext::ObservedAbsent, &mut resolver)?;
                resolver_queries.push(std::mem::take(&mut resolver.queries));
                drop(resolver);
                chunks.push([x, z]);
                palettes.push(biomes.biomes().values().to_vec());
                if x == cx && z == cz {
                    center = Some(biomes);
                } else {
                    neighbors.images.insert((x, z), biomes);
                }
            }
        }
        drop(generator);
        ensure!(weak.strong_count() == 9, "prepared owner count changed");
        let mut center = center.context("no center BIOMES owner")?.fill_noise()?;
        let before_palette = center.biomes().values().to_vec();
        center.build_surface(
            BiomeZoom::from_world_seed(seed),
            &mut neighbors,
            &mut thermal,
        )?;
        let (before_blocks, before) = image(center.terrain())?;
        let mut caches = CarverNeighbors {
            caches: BTreeMap::new(),
        };
        for dx in -8..=8 {
            for dz in -8..=8 {
                caches
                    .caches
                    .insert((cx + dx, cz + dz), CarverBiomeCache::observed_absent());
            }
        }
        let mut registry = BuiltinCarverRegistry;
        let mut facts = FreshFacts {
            target: (cx, cz),
            min,
            height,
        };
        let source = match source_name {
            "minecraft:overworld" => CarverBiomeSource::MultiNoise(&mut overworld),
            "minecraft:nether" => CarverBiomeSource::MultiNoise(&mut nether),
            "minecraft:end" => CarverBiomeSource::End,
            _ => return Err(anyhow!("unsupported explicit source")),
        };
        let carved = center.carve_air(AirCarvingContext {
            world_seed: seed,
            source,
            zoom: BiomeZoom::from_world_seed(seed),
            temperature: &mut thermal,
            neighbors: &mut caches,
            registry: &mut registry,
            facts: &mut facts,
            mask: AirMaskOwner::observed_absent((cx, cz), CarvingMaskFilter::ObservedNoUpdate),
        })?;
        ensure!(
            carved.terrain().biomes().values() == before_palette.as_slice(),
            "carvers changed stored biomes"
        );
        ensure!(
            caches.caches.values().all(CarverBiomeCache::is_cached),
            "neighbor cache not retained"
        );
        let (after_blocks, after) = image(carved.terrain().terrain())?;
        let mask = carved.mask().to_array();
        let cardinality: u32 = mask.iter().map(|x| x.count_ones()).sum();
        let prefix = format!(
            "{}-{}-{seed}-{cx}-{cz}",
            settings.replace(':', "-"),
            source_name.replace(':', "-")
        );
        let before_file = directory.join(format!("rust-before-carvers-{prefix}.bin"));
        let after_file = directory.join(format!("rust-after-carvers-{prefix}.bin"));
        let palettes_file = directory.join(format!("rust-carver-palettes-{prefix}.json"));
        let resolver_file = directory.join(format!("rust-carver-biome-queries-{prefix}.json"));
        std::fs::write(&before_file, before_blocks)?;
        std::fs::write(&after_file, after_blocks)?;
        std::fs::write(&palettes_file, serde_json::to_vec(&palettes)?)?;
        std::fs::write(&resolver_file, serde_json::to_vec(&resolver_queries)?)?;
        println!(
            "{}",
            json!({"settings":settings,"biome_source":source_name,"seed":seed,"x":cx,"z":cz,"level_min_y":min,"level_height":height,"chunks":chunks,"use_legacy_random_source":legacy,"palettes_file":palettes_file,"resolver_queries_file":resolver_file,"before_file":before_file,"after_file":after_file,"before":before,"after":after,"report":carved.report(),"mask":mask,"mask_cardinality":cardinality,"biomes_unchanged":true,"neighbor_cache_owners_retained":true,"stage":format!("{:?}",carved.terrain().terrain().stage()),"retained_prepared_owner_count":weak.strong_count()})
        );
    }
    Ok(())
}
