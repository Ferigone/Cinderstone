use anyhow::{Context, Result};
use mc121_rust::worldgen::{FreshGenerationContext, GenerationInfluence, NoiseGenerator};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::{self, BufRead},
};
fn main() -> Result<()> {
    let output_directory = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .context("usage: noise_probe <output-directory>")?,
    );
    std::fs::create_dir_all(&output_directory)?;
    for line in io::stdin().lock().lines() {
        let line = line?;
        let q: Value = serde_json::from_str(&line)?;
        let name = q["settings"].as_str().context("settings")?;
        let seed = q["seed"].as_i64().context("seed")?;
        let x = q["x"].as_i64().context("x")? as i32;
        let z = q["z"].as_i64().context("z")? as i32;
        let generator = NoiseGenerator::builtin(seed, name)?;
        let grid = generator.graph().grid().context("grid")?;
        let observed = GenerationInfluence::ObservedEmpty;
        let terrain = generator.fill_noise(
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
        let mut bytes = Vec::new();
        let mut counts = BTreeMap::<u32, usize>::new();
        for y in grid.min_y..grid.min_y + grid.height {
            for lx in 0..16 {
                for lz in 0..16 {
                    let state = terrain.block(lx, y, lz)?;
                    bytes.extend(state.to_le_bytes());
                    *counts.entry(state).or_default() += 1;
                }
            }
        }
        let filename = output_directory.join(format!(
            "rust-{}-{seed}-{x}-{z}.bin",
            name.replace(':', "-")
        ));
        std::fs::write(&filename, &bytes)?;
        println!(
            "{}",
            json!({"seed":seed,"settings":name,"x":x,"z":z,"blocks_file":filename,"counts":counts,"stage":format!("{:?}",terrain.stage()),"postprocessing_count":terrain.postprocessing().len()})
        );
    }
    Ok(())
}
