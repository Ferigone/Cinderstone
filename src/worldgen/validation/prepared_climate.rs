//! Controlled prepared-climate -> NOISE witness, not actual BIOMES generation.
use anyhow::{anyhow, Context, Result};
use mc121_rust::worldgen::{FreshGenerationContext, GenerationInfluence, NoiseGenerator};
use serde_json::{json, Value};
use std::{
    io::{self, BufRead},
    sync::Arc,
};

fn main() -> Result<()> {
    let output_directory = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .context("usage: prepared_climate <output-directory>")?,
    );
    std::fs::create_dir_all(&output_directory)?;
    for line in io::stdin().lock().lines() {
        let q: Value = serde_json::from_str(&line?)?;
        let name = q["settings"].as_str().context("settings")?;
        let seed = q["seed"].as_i64().context("seed")?;
        let x = q["x"].as_i64().context("x")? as i32;
        let z = q["z"].as_i64().context("z")? as i32;
        let generator = Arc::new(NoiseGenerator::builtin(seed, name)?);
        let grid = generator.graph().grid().context("noise grid")?;
        let observed = GenerationInfluence::ObservedEmpty;
        let mut prepared = generator.prepare_chunk(
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
        let mut climate = Vec::new();
        for point in q["points"].as_array().context("quart points")? {
            let point = point.as_array().context("quart point")?;
            let sample = prepared.sample_climate(
                point[0].as_i64().context("quart X")? as i32,
                point[1].as_i64().context("quart Y")? as i32,
                point[2].as_i64().context("quart Z")? as i32,
            )?;
            climate.push(sample.parameter_array().map(|value| value.to_string()));
        }
        // Exercise an actual owned job boundary. No reference into this local
        // generator binding can remain after it is dropped or cross the worker.
        drop(generator);
        let mut terrain = std::thread::spawn(move || prepared.fill_noise())
            .join()
            .map_err(|_| anyhow!("prepared NOISE worker panicked"))??;
        let mut climate_after = Vec::new();
        for point in q["points"].as_array().context("quart points")? {
            let point = point.as_array().context("quart point")?;
            let sample = terrain.sample_climate(
                point[0].as_i64().context("quart X")? as i32,
                point[1].as_i64().context("quart Y")? as i32,
                point[2].as_i64().context("quart Z")? as i32,
            )?;
            climate_after.push(sample.parameter_array().map(|value| value.to_string()));
        }
        let mut bytes = Vec::new();
        for y in grid.min_y..grid.min_y + grid.height {
            for local_x in 0..16 {
                for local_z in 0..16 {
                    bytes.extend(terrain.block(local_x, y, local_z)?.to_le_bytes());
                }
            }
        }
        let file = output_directory.join(format!(
            "rust-prepared-{}-{seed}-{x}-{z}.bin",
            name.replace(':', "-")
        ));
        std::fs::write(&file, bytes)?;
        println!(
            "{}",
            json!({
                "settings":name,"seed":seed,"x":x,"z":z,"min_y":grid.min_y,"height":grid.height,
                "points":q["points"],"climate":climate,"climate_after":climate_after,"blocks_file":file,
                "postprocessing_count":terrain.postprocessing().len(),
                "stage":format!("{:?}",terrain.stage()),
                "dropped_outer_generator_binding":true,"moved_prepared_to_worker":true,
            })
        );
    }
    Ok(())
}
