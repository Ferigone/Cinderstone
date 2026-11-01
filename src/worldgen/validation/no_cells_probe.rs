//! Focused source-entry-point probe. Writes one JSON object to stdout; no files.
use anyhow::{bail, ensure, Result};
use mc121_rust::worldgen::{
    generator::NoiseFillOutcome, FreshGenerationContext, GenerationInfluence, NoiseGenerator,
};
use serde_json::json;

fn unavailable() -> FreshGenerationContext {
    FreshGenerationContext {
        initial_blocks: GenerationInfluence::Unavailable,
        blender: GenerationInfluence::Unavailable,
        structure_beardifier: GenerationInfluence::Unavailable,
    }
}

fn main() -> Result<()> {
    let generator = NoiseGenerator::builtin(123456789, ":overworld")?;
    ensure!(generator.settings_name() == "minecraft:overworld");
    let mut rows = Vec::new();
    for (level_min_y, level_height) in [(320, 16), (384, 16), (-160, 16), (317, 2)] {
        match generator.fill_from_noise(7, -9, level_min_y, level_height, unavailable())? {
            NoiseFillOutcome::NoCells {
                min_y,
                height,
                cell_count_y,
            } => rows.push(json!({
                "level_min_y": level_min_y, "level_height": level_height,
                "min_y": min_y, "height": height, "cell_count_y": cell_count_y,
                "no_cells": true,
            })),
            NoiseFillOutcome::Generated(_) => bail!("no-cell source path constructed terrain"),
        }
    }
    // The skipped path must not consume coordinate/dependency inputs. A real
    // positive overlap must retain dependency validation; no fallback is added.
    ensure!(matches!(
        generator.fill_from_noise(i32::MAX, i32::MIN, 320, 16, unavailable())?,
        NoiseFillOutcome::NoCells { .. }
    ));
    ensure!(generator
        .fill_from_noise(0, 0, -64, 384, unavailable())
        .is_err());
    ensure!(generator.fill_noise(0, 0, 320, 16, unavailable()).is_err());
    ensure!(generator
        .fill_from_noise(0, 0, 2032, 1, unavailable())
        .is_err());
    println!(
        "{}",
        json!({
            "rows": rows,
            "settings_name": generator.settings_name(),
            "unavailable_dependencies_skipped": true,
            "positive_overlap_requires_dependencies": true,
            "strict_fill_noise_unchanged": true,
            "dimension_bounds_enforced": true,
        })
    );
    Ok(())
}
