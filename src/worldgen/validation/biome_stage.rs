//! Actual built-in source selection -> owned biome image -> same-owner NOISE.
use anyhow::{anyhow, Context, Result};
use mc121_rust::worldgen::{
    biomes::{BiomeResolver, BuiltinBiomeResolver, PreparedBiomes, RetrogenContext},
    BuiltinMultiNoiseBiomeSource, FreshGenerationContext, GenerationInfluence, MultiNoisePreset,
    NoiseGenerator, PreparedGeneration,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::{self, BufRead},
    sync::Arc,
};

struct ObservedResolver<'a> {
    source: BuiltinBiomeResolver<'a>,
    queries: Vec<[i32; 3]>,
}
impl BiomeResolver for ObservedResolver<'_> {
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

fn main() -> Result<()> {
    let directory = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .context("usage: biome_stage <output-directory>")?,
    );
    std::fs::create_dir_all(&directory)?;
    // Original source ParameterList holds one last-leaf cursor per worker and
    // preset. Keep these real worker owners across all requested chunk tasks.
    let mut overworld = BuiltinMultiNoiseBiomeSource::new(MultiNoisePreset::Overworld)?;
    let mut nether = BuiltinMultiNoiseBiomeSource::new(MultiNoisePreset::Nether)?;
    for line in io::stdin().lock().lines() {
        let q: Value = serde_json::from_str(&line?)?;
        let settings = q["settings"].as_str().context("settings")?;
        let source_name = q["biome_source"]
            .as_str()
            .context("explicit biome source")?;
        let seed = q["seed"].as_i64().context("seed")?;
        let cx = q["x"].as_i64().context("chunk X")? as i32;
        let cz = q["z"].as_i64().context("chunk Z")? as i32;
        let min_y = q["level_min_y"].as_i64().context("level minimum")? as i32;
        let height = q["level_height"].as_i64().context("level height")? as i32;
        let generator = Arc::new(NoiseGenerator::builtin(seed, settings)?);
        let empty = GenerationInfluence::ObservedEmpty;
        let prepared = generator.prepare_chunk(
            cx,
            cz,
            min_y,
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
            _ => return Err(anyhow!("unsupported explicit built-in biome source")),
        };
        let mut observer = ObservedResolver {
            source,
            queries: Vec::new(),
        };
        let biomes =
            PreparedBiomes::fill(prepared, RetrogenContext::ObservedAbsent, &mut observer)?;
        let queries = std::mem::take(&mut observer.queries);
        drop(observer);
        let palette = biomes.biomes().values().to_vec();
        drop(generator);
        // Only the owned generated data/job moves. The persistent !Send search
        // cursor stays on the original worker, as required by biome selection.
        let result = std::thread::spawn(move || biomes.fill_noise())
            .join()
            .map_err(|_| anyhow!("BIOMES -> NOISE worker panicked"))??;
        if result.biomes().values() != palette.as_slice() {
            return Err(anyhow!("NOISE replaced stored biomes"));
        }
        let mut blocks = Vec::new();
        for y in min_y..min_y + height {
            for x in 0..16 {
                for z in 0..16 {
                    blocks.extend(result.terrain().block(x, y, z)?.to_le_bytes());
                }
            }
        }
        let first_section = min_y.div_euclid(16);
        let section_count = (min_y + height - 1).div_euclid(16) - first_section + 1;
        let mut post_sections = vec![Vec::<i16>::new(); section_count as usize];
        let mut post_positions = vec![Vec::<[i32; 3]>::new(); section_count as usize];
        // Actual terrain.postprocessing is global call order. Partition it
        // without sorting or deduplication, matching source section storage.
        for point in result.terrain().postprocessing() {
            anyhow::ensure!(
                point.x.div_euclid(16) == cx
                    && point.z.div_euclid(16) == cz
                    && (min_y..min_y + height).contains(&point.y),
                "postprocessing position outside actual chunk"
            );
            let section = (point.y.div_euclid(16) - first_section) as usize;
            // ProtoChunk.packOffsetCoordinates: X low4, Y next4, Z next4.
            let packed = ((point.x & 15) | ((point.y & 15) << 4) | ((point.z & 15) << 8)) as i16;
            post_sections[section].push(packed);
            post_positions[section].push([point.x, point.y, point.z]);
        }
        let prefix = format!(
            "{}-{}-{seed}-{cx}-{cz}",
            settings.replace(':', "-"),
            source_name.replace(':', "-")
        );
        let biome_file = directory.join(format!("rust-biomes-{prefix}.json"));
        let query_file = directory.join(format!("rust-biome-queries-{prefix}.json"));
        let block_file = directory.join(format!("rust-biome-noise-{prefix}.bin"));
        std::fs::write(&biome_file, serde_json::to_vec(&palette)?)?;
        std::fs::write(&query_file, serde_json::to_vec(&queries)?)?;
        std::fs::write(&block_file, blocks)?;
        let mut counts = BTreeMap::<String, usize>::new();
        for biome in &palette {
            *counts.entry(biome.clone()).or_default() += 1;
        }
        println!(
            "{}",
            json!({
                "settings":settings,"biome_source":source_name,"seed":seed,"x":cx,"z":cz,
                "level_min_y":min_y,"level_height":height,
                "biomes_file":biome_file,"queries_file":query_file,"blocks_file":block_file,
                "biome_counts":counts,"world_surface_wg":result.terrain().world_surface_wg().to_vec(),
                "ocean_floor_wg":result.terrain().ocean_floor_wg().to_vec(),
                "postprocessing_count":result.terrain().postprocessing().len(),
                "postprocessing_min_section_y":first_section,"postprocessing_sections":post_sections,
                "postprocessing_positions_by_section":post_positions,
                "biomes_retained":true,"stage":format!("{:?}",result.terrain().stage()),
                "dropped_outer_generator_binding":true,"owned_noise_worker":true,
            })
        );
    }
    Ok(())
}
