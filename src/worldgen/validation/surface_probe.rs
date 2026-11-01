use anyhow::{bail, ensure, Context, Result};
use mc121_rust::worldgen::{
    FreshGenerationContext, GenerationInfluence, NoiseGenerator, SurfaceContext, TerrainStage,
};
use serde_json::{json, Value};
use std::io::{self, BufRead};

struct ObservedBiome {
    biome: String,
    thermal: Value,
    callbacks: usize,
}
impl SurfaceContext for ObservedBiome {
    fn biome(&mut self, _: i32, _: i32, _: i32) -> Result<String> {
        self.callbacks += 1;
        Ok(self.biome.clone())
    }
    fn cold_enough_to_snow(&mut self, biome: &str, x: i32, y: i32, z: i32) -> Result<bool> {
        self.callbacks += 1;
        ensure!(biome == self.biome, "unobserved biome");
        let cx = self.thermal["x"].as_i64().context("thermal chunk x")? as i32;
        let cz = self.thermal["z"].as_i64().context("thermal chunk z")? as i32;
        let min = self.thermal["min_y"].as_i64().context("thermal min y")? as i32;
        let max = self.thermal["max_y_inclusive"]
            .as_i64()
            .context("thermal max y")? as i32;
        let lx = x - cx * 16;
        let lz = z - cz * 16;
        ensure!(
            (0..16).contains(&lx) && (0..16).contains(&lz) && (min..=max).contains(&y),
            "unobserved thermal position"
        );
        let column = self.thermal["cold_enough_to_snow"][(lx * 16 + lz) as usize]
            .as_str()
            .context("missing actual cold column")?;
        match column.as_bytes().get((y - min) as usize) {
            Some(b'0') => Ok(false),
            Some(b'1') => Ok(true),
            _ => bail!("unobserved original cold-enough-to-snow callback"),
        }
    }
    fn should_melt_frozen_ocean_iceberg_slightly(
        &mut self,
        biome: &str,
        x: i32,
        y: i32,
        z: i32,
    ) -> Result<bool> {
        self.callbacks += 1;
        ensure!(biome == self.biome, "unobserved biome");
        let cx = self.thermal["x"].as_i64().context("thermal chunk x")? as i32;
        let cz = self.thermal["z"].as_i64().context("thermal chunk z")? as i32;
        let lx = x - cx * 16;
        let lz = z - cz * 16;
        ensure!(
            (0..16).contains(&lx) && (0..16).contains(&lz) && y == 63,
            "unobserved iceberg thermal position"
        );
        self.thermal["should_melt_frozen_ocean_iceberg_slightly_at_63"][(lx * 16 + lz) as usize]
            .as_bool()
            .context("unobserved original iceberg-temperature callback")
    }
}
struct MissingBiome;
impl SurfaceContext for MissingBiome {
    fn biome(&mut self, _: i32, _: i32, _: i32) -> Result<String> {
        bail!("deliberately unavailable original biome owner")
    }
    fn cold_enough_to_snow(&mut self, _: &str, _: i32, _: i32, _: i32) -> Result<bool> {
        bail!("unavailable original temperature owner")
    }
    fn should_melt_frozen_ocean_iceberg_slightly(
        &mut self,
        _: &str,
        _: i32,
        _: i32,
        _: i32,
    ) -> Result<bool> {
        bail!("unavailable original temperature owner")
    }
}
fn main() -> Result<()> {
    let output_directory = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .context("usage: surface_probe <output-directory>")?,
    );
    std::fs::create_dir_all(&output_directory)?;
    for line in io::stdin().lock().lines() {
        let q: Value = serde_json::from_str(&line?)?;
        let name = q["settings"].as_str().context("settings")?;
        let biome = q["biome"].as_str().context("biome")?;
        let seed = q["seed"].as_i64().context("seed")?;
        let x = q["x"].as_i64().context("x")? as i32;
        let z = q["z"].as_i64().context("z")? as i32;
        let generator = NoiseGenerator::builtin(seed, name)?;
        let grid = generator.graph().grid().context("grid")?;
        let observed = GenerationInfluence::ObservedEmpty;
        let mut terrain = generator.fill_noise(
            x,
            z,
            grid.min_y,
            grid.height,
            FreshGenerationContext {
                initial_blocks: observed,
                blender: observed,
                structure_beardifier: observed,
            },
        )?;
        let before = terrain.states().to_vec();
        let before_posts = terrain.postprocessing().to_vec();
        let before_height = *terrain.world_surface_wg();
        let before_ocean_floor = *terrain.ocean_floor_wg();
        ensure!(
            generator
                .build_surface(&mut terrain, &mut MissingBiome)
                .is_err(),
            "missing owner was silently accepted"
        );
        ensure!(
            terrain.stage() == TerrainStage::Noise
                && terrain.states() == before
                && terrain.postprocessing() == before_posts
                && terrain.world_surface_wg() == &before_height
                && terrain.ocean_floor_wg() == &before_ocean_floor,
            "failed surface call changed the NOISE terrain image"
        );
        let thermal: Value = if let Some(file) = q["thermal"].as_str() {
            let path = std::path::Path::new(file);
            let path = if path.is_absolute() {
                path.to_path_buf()
            } else {
                output_directory.join(path)
            };
            serde_json::from_slice(&std::fs::read(path)?)?
        } else {
            Value::Null
        };
        let mut observed_biome = ObservedBiome {
            biome: biome.to_owned(),
            thermal,
            callbacks: 0,
        };
        let report = generator.build_surface(&mut terrain, &mut observed_biome)?;
        ensure!(
            terrain.stage() == TerrainStage::Surface,
            "successful SURFACE call did not advance its stage"
        );
        let mut bytes = Vec::new();
        let mut changes = 0;
        for y in grid.min_y..grid.min_y + grid.height {
            for lx in 0..16 {
                for lz in 0..16 {
                    let state = terrain.block(lx, y, lz)?;
                    bytes.extend(state.to_le_bytes());
                }
            }
        }
        changes += terrain
            .states()
            .iter()
            .zip(before)
            .filter(|(a, b)| **a != *b)
            .count();
        let output = output_directory.join(format!(
            "rust-surface-{}-{seed}-{x}-{z}-{}.bin",
            name.replace(':', "-"),
            biome.replace(':', "-")
        ));
        std::fs::write(&output, &bytes)?;
        let completed_states = terrain.states().to_vec();
        let completed_posts = terrain.postprocessing().to_vec();
        let completed_height = *terrain.world_surface_wg();
        let completed_ocean_floor = *terrain.ocean_floor_wg();
        let callbacks_before_duplicate = observed_biome.callbacks;
        // Reuse the successful, available context. An unavailable biome would
        // make a missing duplicate-stage guard appear to pass this check.
        let duplicate_error = generator
            .build_surface(&mut terrain, &mut observed_biome)
            .err()
            .context("duplicate SURFACE stage was accepted")?;
        ensure!(
            duplicate_error.to_string() == "surface stage already completed",
            "duplicate SURFACE call failed for the wrong reason: {duplicate_error}"
        );
        ensure!(
            observed_biome.callbacks == callbacks_before_duplicate,
            "duplicate SURFACE call reached a biome or thermal callback"
        );
        ensure!(
            terrain.stage() == TerrainStage::Surface
                && terrain.states() == completed_states
                && terrain.postprocessing() == completed_posts
                && terrain.world_surface_wg() == &completed_height
                && terrain.ocean_floor_wg() == &completed_ocean_floor,
            "duplicate SURFACE call changed the completed terrain"
        );
        println!(
            "{}",
            json!({"settings":name,"seed":seed,"x":x,"z":z,"biome":biome,"output":output,"changes":changes,"rule_replacements":report.rule_replacements,"columns":report.columns,"postprocess":terrain.postprocessing().len(),"noise_postprocess":before_posts.len(),"surface_postprocess":terrain.postprocessing().len()-before_posts.len(),"stage":format!("{:?}",terrain.stage()),"failed_context_preserves_noise":true,"duplicate_stage_rejected":true,"duplicate_stage_error":duplicate_error.to_string(),"duplicate_stage_context_callbacks":observed_biome.callbacks-callbacks_before_duplicate,"duplicate_stage_preserves_surface":true,"heights":terrain.world_surface_wg().as_slice(),"ocean_floor":terrain.ocean_floor_wg().as_slice()})
        );
    }
    Ok(())
}
