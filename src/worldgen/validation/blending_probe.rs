//! Controlled calculated old snapshots and native retained BIOMES/NOISE jobs.
//! No production WorldGenRegion, status certificate or fallback is supplied.
use anyhow::{Context, Result};
use mc121_rust::{
    density::{
        ChunkDensity, ChunkDensityError, DensityBounds, DensityContext, DensityGraph, DensityPoint,
        MarkerKind, NodeId,
    },
    geometry,
    worldgen::{
        biome_source::{BuiltinMultiNoiseBiomeSource, ClimateSampler, MultiNoisePreset},
        biome_temperature::BiomeTemperature,
        biome_zoom::{BiomeZoom, QuartBiomeProvider},
        biomes::{BiomeResolver, PreparedBiomes, RetrogenContext},
        blending::{
            chunk_key, height_to_offset, Blender, BlendingData, Direction8, Direction8Set,
            OldChunkDataHost,
        },
        generator::{
            BlendedGenerationContext, GenerationInfluence, NoiseGenerator, OwnedNoiseTerrain,
        },
        PreparedGeneration,
    },
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, BufRead},
    path::Path,
    sync::Arc,
};
fn bits(v: f64) -> String {
    v.to_bits().to_string()
}
fn doubles(values: &[f64]) -> Vec<String> {
    values.iter().map(|v| bits(*v)).collect()
}
fn state(name: &str) -> Result<u32> {
    mc121_rust::worldgen::generator::resolve_block_state(
        &json!({"Name":format!("minecraft:{name}")}),
    )
}
fn triple(value: &Value) -> Result<DensityPoint> {
    let a = value.as_array().context("point missing")?;
    Ok(DensityPoint::new(
        a[0].as_i64().context("x")? as i32,
        a[1].as_i64().context("y")? as i32,
        a[2].as_i64().context("z")? as i32,
    ))
}
struct RawSampler<'a>(&'a DensityGraph);
impl ClimateSampler for RawSampler<'_> {
    type Error = anyhow::Error;
    fn sample_climate(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
    ) -> Result<mc121_rust::density::ClimateTargetPoint> {
        self.0.sample_climate(x, y, z)
    }
}
struct Snapshot {
    chunk: (i32, i32),
    states: Vec<u32>,
    heights: [i32; 256],
    biomes: Vec<String>,
    block_reads: usize,
    biome_reads: usize,
    shape_reads: usize,
}
impl Snapshot {
    fn new(
        spec: &Value,
        graph: &DensityGraph,
        source: &mut BuiltinMultiNoiseBiomeSource,
    ) -> Result<Self> {
        let c = spec["chunk"].as_array().context("old chunk missing")?;
        let chunk = (
            c[0].as_i64().context("old x")? as i32,
            c[1].as_i64().context("old z")? as i32,
        );
        let profile = spec["profile"].as_i64().context("old profile missing")? as i32;
        let air = state("air")?;
        let water = state("water")?;
        let grass = state("grass_block")?;
        let dirt = state("dirt")?;
        let stone = state("stone")?;
        let mut states = vec![air; 384 * 256];
        let mut heights = [-64; 256];
        for y in 0..256 {
            for z in 0..16 {
                for x in 0..16 {
                    let top = 60 + profile * 17 + (x * 3 + z * 5) % 7;
                    let value = if y > top {
                        if y < 63 {
                            water
                        } else {
                            air
                        }
                    } else if y == top {
                        grass
                    } else if y > top - 4 {
                        dirt
                    } else if y >= 20 + profile && y <= 23 + profile && ((x + z) & 3) == 0 {
                        air
                    } else {
                        stone
                    };
                    states[(y + 64) as usize * 256 + (z * 16 + x) as usize] = value;
                    if !mc121_rust::lighting::state_properties(value)?.is_air() {
                        heights[(z * 16 + x) as usize] = y + 1;
                    }
                }
            }
        }
        let fixed = spec["biome"]
            .as_str()
            .map(|name| {
                BiomeTemperature::builtin()?
                    .canonical_biome(name)
                    .map(str::to_owned)
            })
            .transpose()?;
        let mut biomes = vec![String::new(); 96 * 16];
        let mut sampler = RawSampler(graph);
        for section in -4..20 {
            for x in 0..4 {
                for y in 0..4 {
                    for z in 0..4 {
                        let qy = section * 4 + y;
                        let value = match &fixed {
                            Some(name) => name.clone(),
                            None => source.get_noise_biome(
                                chunk.0.wrapping_shl(2).wrapping_add(x),
                                qy,
                                chunk.1.wrapping_shl(2).wrapping_add(z),
                                &mut sampler,
                            )?,
                        };
                        biomes[(qy + 16) as usize * 16 + (z * 4 + x) as usize] = value;
                    }
                }
            }
        }
        Ok(Self {
            chunk,
            states,
            heights,
            biomes,
            block_reads: 0,
            biome_reads: 0,
            shape_reads: 0,
        })
    }
}
impl OldChunkDataHost for Snapshot {
    type Biome = String;
    fn block_state(&mut self, p: DensityPoint) -> Result<u32> {
        self.block_reads += 1;
        if p.y < -64 || p.y >= 320 {
            return state("void_air");
        }
        Ok(self.states[(p.y + 64) as usize * 256 + ((p.z & 15) * 16 + (p.x & 15)) as usize])
    }
    fn world_surface_wg_first_available(&mut self, x: i32, z: i32) -> Result<Option<i32>> {
        Ok(Some(self.heights[((z & 15) * 16 + (x & 15)) as usize]))
    }
    fn noise_biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
        self.biome_reads += 1;
        Ok(
            self.biomes[(y.clamp(-16, 79) + 16) as usize * 16 + ((z & 3) * 4 + (x & 3)) as usize]
                .clone(),
        )
    }
    fn collision_shape_is_empty(&mut self, _: DensityPoint, state: u32) -> Result<bool> {
        self.shape_reads += 1;
        Ok(geometry::static_block_collision_boxes(state)
            .context("actual snapshot static collision shape unavailable")?
            .is_empty())
    }
}
fn direction_name(d: Direction8) -> &'static str {
    match d {
        Direction8::North => "NORTH",
        Direction8::NorthEast => "NORTH_EAST",
        Direction8::East => "EAST",
        Direction8::SouthEast => "SOUTH_EAST",
        Direction8::South => "SOUTH",
        Direction8::SouthWest => "SOUTH_WEST",
        Direction8::West => "WEST",
        Direction8::NorthWest => "NORTH_WEST",
    }
}
fn data_image(host: &Snapshot, data: &BlendingData, sides: Direction8Set) -> Value {
    let mut heights = Vec::new();
    data.iterate_heights(host.chunk.0 << 2, host.chunk.1 << 2, |x, z, h| {
        heights.push(json!([x, z, bits(h)]))
    });
    let mut densities = Vec::new();
    data.iterate_densities(
        host.chunk.0 << 2,
        host.chunk.1 << 2,
        -16,
        40,
        |x, y, z, d| densities.push(json!([x, y, z, bits(d)])),
    );
    let mut biomes = Vec::new();
    for y in [-1, 0, 31, 32, 63, 64] {
        let mut row = Vec::new();
        data.iterate_biomes(host.chunk.0 << 2, y, host.chunk.1 << 2, |x, z, b| {
            row.push(json!([x, z, b]))
        });
        biomes.push(json!({"quart_y":y,"values":row}));
    }
    let mut reads = Vec::new();
    for x in [0, 1, 3, 4] {
        for y in [-1, 0, 1, 7, 31, 32, 33] {
            for z in [0, 1, 3, 4] {
                reads.push(json!([
                    x,
                    y,
                    z,
                    bits(data.get_height(x, y, z)),
                    bits(data.get_density(x, y, z))
                ]));
            }
        }
    }
    json!({"chunk":[host.chunk.0,host.chunk.1],"sides":sides.iter().map(direction_name).collect::<Vec<_>>(),"heights":doubles(data.heights()),
        "densities":data.density_columns().iter().map(|c|c.as_ref().map(|v|doubles(v))).collect::<Vec<_>>(),"biomes":data.biome_columns(),
        "iterate_heights":heights,"iterate_densities":densities,"iterate_biomes":biomes,"calculated":data.is_calculated(),"source_status":"minecraft:empty","reads":reads})
}
struct FillSpy<'a> {
    owner: &'a mut ChunkDensity,
    calls: Vec<usize>,
}
impl DensityContext for FillSpy<'_> {
    type Error = ChunkDensityError;
    fn bounds(&self, g: &DensityGraph, id: NodeId) -> DensityBounds {
        self.owner
            .wrapped_bounds(id)
            .unwrap_or_else(|_| g.bounds(id))
    }
    fn rewritten_constant_argument(&self, id: NodeId) -> Option<(NodeId, f64)> {
        DensityContext::rewritten_constant_argument(self.owner, id)
    }
    fn marker(
        &mut self,
        g: &DensityGraph,
        id: NodeId,
        k: MarkerKind,
        input: NodeId,
        p: DensityPoint,
    ) -> std::result::Result<f64, Self::Error> {
        DensityContext::marker(self.owner, g, id, k, input, p)
    }
    fn for_index(&mut self, index: usize) -> DensityPoint {
        self.calls.push(index);
        DensityContext::for_index(self.owner, index)
    }
    fn blend_density(&mut self, p: DensityPoint, value: f64) -> f64 {
        DensityContext::blend_density(self.owner, p, value)
    }
    fn blend_alpha(&mut self, id: NodeId, p: DensityPoint) -> f64 {
        DensityContext::blend_alpha(self.owner, id, p)
    }
    fn blend_offset(&mut self, id: NodeId, p: DensityPoint) -> f64 {
        DensityContext::blend_offset(self.owner, id, p)
    }
    fn blend_alpha_fill(
        &mut self,
        g: &DensityGraph,
        id: NodeId,
        values: &mut [f64],
    ) -> std::result::Result<(), Self::Error> {
        if self.owner.blender().is_empty() {
            values.fill(1.0);
            Ok(())
        } else {
            self.fill_direct(g, id, values)
        }
    }
    fn blend_offset_fill(
        &mut self,
        g: &DensityGraph,
        id: NodeId,
        values: &mut [f64],
    ) -> std::result::Result<(), Self::Error> {
        if self.owner.blender().is_empty() {
            values.fill(0.0);
            Ok(())
        } else {
            self.fill_direct(g, id, values)
        }
    }
}
struct ObserveResolver<'a> {
    source: &'a mut BuiltinMultiNoiseBiomeSource,
    expected: Arc<Blender>,
    calls: Vec<Value>,
}
impl BiomeResolver for ObserveResolver<'_> {
    fn get_noise_biome(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        sampler: &mut PreparedGeneration,
    ) -> Result<String> {
        self.calls.push(json!([
            x,
            y,
            z,
            Arc::ptr_eq(&self.expected, &sampler.blender()?)
        ]));
        self.source.get_noise_biome(x, y, z, sampler)
    }
}

enum SurfacePalette<'a> {
    Old(&'a Snapshot),
    New(PreparedBiomes),
}
impl SurfacePalette<'_> {
    fn values(&self) -> &[String] {
        match self {
            Self::Old(snapshot) => &snapshot.biomes,
            Self::New(prepared) => prepared.biomes().values(),
        }
    }

    fn get(&self, x: i32, y: i32, z: i32) -> Result<String> {
        match self {
            Self::Old(snapshot) => {
                anyhow::ensure!(
                    (x.div_euclid(4), z.div_euclid(4)) == snapshot.chunk,
                    "old SURFACE palette query belongs to another chunk"
                );
                // This is the existing full-level Snapshot palette, not the
                // narrower old-generation BlendingData height span.
                Ok(snapshot.biomes
                    [(y.clamp(-16, 79) + 16) as usize * 16 + ((z & 3) * 4 + (x & 3)) as usize]
                    .clone())
            }
            Self::New(prepared) => prepared
                .biomes()
                .get_noise_biome(x, y, z)
                .map(str::to_owned),
        }
    }
}

struct SurfaceNeighbors<'a> {
    palettes: BTreeMap<(i32, i32), SurfacePalette<'a>>,
    queries: Vec<Value>,
}
impl QuartBiomeProvider for SurfaceNeighbors<'_> {
    type Biome = String;
    type Error = anyhow::Error;

    fn get_noise_biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
        let chunk = (x.div_euclid(4), z.div_euclid(4));
        let palette = self
            .palettes
            .get(&chunk)
            .with_context(|| format!("SURFACE requested unavailable neighbor palette {chunk:?}"))?;
        let biome = palette.get(x, y, z)?;
        // The public BiomeTerrain adapter serves its own center image before
        // this provider. These are actual foreign-center callbacks only.
        self.queries.push(json!([x, y, z, biome]));
        Ok(biome)
    }
}

fn neighbor_blender(
    chunk: (i32, i32),
    data: &BTreeMap<(i32, i32), Arc<BlendingData>>,
    singleton_empty: bool,
) -> Result<Arc<Blender>> {
    let mut heights = Vec::new();
    let mut densities = Vec::new();
    for dx in -7_i32..=7 {
        for dz in -7_i32..=7 {
            if dx * dx + dz * dz > 64 {
                continue;
            }
            let position = (chunk.0.wrapping_add(dx), chunk.1.wrapping_add(dz));
            if let Some(value) = data.get(&position) {
                let key = chunk_key(position.0, position.1);
                heights.push((key, Arc::clone(value)));
                if (-1..=1).contains(&dx) && (-1..=1).contains(&dz) {
                    densities.push((key, Arc::clone(value)));
                }
            }
        }
    }
    if singleton_empty || (heights.is_empty() && densities.is_empty()) {
        Ok(Blender::empty())
    } else {
        Blender::from_calculated_data(heights, densities)
    }
}

fn terrain_image(terrain: &OwnedNoiseTerrain, file: &Path) -> Result<Value> {
    let mut bytes = Vec::new();
    for y in terrain.min_y()..terrain.min_y() + terrain.height() {
        for x in 0..16 {
            for z in 0..16 {
                bytes.extend_from_slice(&terrain.block(x, y, z)?.to_le_bytes());
            }
        }
    }
    std::fs::write(file, bytes)?;
    let mut posts = vec![Vec::<i16>::new(); 24];
    for point in terrain.postprocessing() {
        posts[((point.y + 64) >> 4) as usize]
            .push(((point.x & 15) | ((point.y & 15) << 4) | ((point.z & 15) << 8)) as i16);
    }
    Ok(
        json!({"file":file,"world_surface_wg":terrain.world_surface_wg().to_vec(),
        "ocean_floor_wg":terrain.ocean_floor_wg().to_vec(),"posts":posts,
        "post_count":terrain.postprocessing().len(),"stage":format!("{:?}",terrain.stage())}),
    )
}

fn run(
    q: Value,
    output: &Path,
    source: &mut BuiltinMultiNoiseBiomeSource,
    temperature: &mut BiomeTemperature,
) -> Result<Value> {
    let seed = q["seed"].as_str().context("seed missing")?.parse::<i64>()?;
    let settings = q["settings"].as_str().context("settings missing")?;
    let mut graph = DensityGraph::builtin(seed, settings)?;
    let c = q["chunk"].as_array().context("chunk missing")?;
    let chunk = (
        c[0].as_i64().context("x")? as i32,
        c[1].as_i64().context("z")? as i32,
    );
    let specs = q["old"].as_array().context("old inputs missing")?;
    let mut hosts = specs
        .iter()
        .map(|s| Snapshot::new(s, &graph, source))
        .collect::<Result<Vec<_>>>()?;
    let old: BTreeSet<_> = hosts.iter().map(|h| h.chunk).collect();
    let mut data = BTreeMap::new();
    let mut data_images = Vec::new();
    let mut ages = Vec::new();
    let mut second = true;
    for (spec, host) in specs.iter().zip(&mut hosts) {
        let mut sides = Direction8Set::NONE;
        for d in Direction8::ALL {
            let (dx, dz) = d.offset();
            let p = (host.chunk.0 + dx, host.chunk.1 + dz);
            ages.push(json!([p.0, p.1]));
            if !old.contains(&p) {
                sides.insert(d);
            }
        }
        let heights = spec
            .get("heights")
            .map(|h| {
                serde_json::from_value::<Vec<f64>>(h.clone()).and_then(|v| {
                    v.try_into().map_err(|_| {
                        serde_json::Error::io(io::Error::other("wrong height array length"))
                    })
                })
            })
            .transpose()?;
        let mut value = BlendingData::from_serialized(
            spec["min_section"].as_i64().context("min section")? as i32,
            spec["max_section"].as_i64().context("max section")? as i32,
            heights,
        )?;
        value.calculate_data(host, sides)?;
        let before = (host.block_reads, host.biome_reads, host.shape_reads);
        value.calculate_data(host, sides)?;
        second &= before == (host.block_reads, host.biome_reads, host.shape_reads);
        data_images.push(data_image(host, &value, sides));
        data.insert(host.chunk, Arc::new(value));
    }
    let mut hs = Vec::new();
    let mut ds = Vec::new();
    for dx in -7..=7 {
        for dz in -7..=7 {
            if dx * dx + dz * dz > 64 {
                continue;
            }
            let p = (chunk.0 + dx, chunk.1 + dz);
            if let Some(value) = data.get(&p) {
                let k = chunk_key(p.0, p.1);
                hs.push((k, Arc::clone(value)));
                if (-1..=1).contains(&dx) && (-1..=1).contains(&dz) {
                    ds.push((k, Arc::clone(value)));
                }
            }
        }
    }
    let blender = if q["singleton_empty"] == true {
        Blender::empty()
    } else {
        Blender::from_calculated_data(hs, ds)?
    };
    let map_order = |entries: Vec<(i64, &BlendingData)>| {
        entries
            .iter()
            .map(|(k, _)| json!([*k as i32, (*k >> 32) as i32]))
            .collect::<Vec<_>>()
    };
    let height_order = map_order(blender.height_and_biome_entries().collect());
    let density_order = map_order(blender.density_entries().collect());
    let heights = [
        -256.5, -128.5, -8.5, -8.0, -0.5, 0.0, 7.5, 8.0, 63.0, 64.0, 128.0, 255.0,
    ]
    .map(|h| json!([bits(h), bits(height_to_offset(h))]));
    let mut scalars = Vec::new();
    for value in q["points"].as_array().context("points missing")? {
        let p = triple(value)?;
        let b = blender.blend_offset_and_factor(p.x, p.z);
        scalars.push(json!([
            p.x,
            p.y,
            p.z,
            bits(b.alpha),
            bits(b.blending_offset),
            bits(blender.blend_density(p, 0.375))
        ]));
    }
    let mut resolved = Vec::new();
    let mut resolver_calls = Vec::new();
    let mut sampler = RawSampler(&graph);
    for value in q["biome_points"]
        .as_array()
        .context("biome points missing")?
    {
        let p = triple(value)?;
        let value = match blender.blend_biome(p.x, p.y, p.z) {
            Some(b) => b,
            None => {
                resolver_calls.push(json!([p.x, p.y, p.z, true]));
                source.get_noise_biome(p.x, p.y, p.z, &mut sampler)?
            }
        };
        resolved.push(json!([p.x, p.y, p.z, value]));
    }
    let roots = q["custom_roots"]
        .as_array()
        .context("roots missing")?
        .iter()
        .map(|r| {
            Ok((
                r["name"].as_str().context("name")?.to_owned(),
                graph.parse(&r["function"])?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let alpha = graph.parse(&json!({"type":"minecraft:blend_alpha"}))?;
    let offset = graph.parse(&json!({"type":"minecraft:blend_offset"}))?;
    let constant = graph.parse(&json!(0.0))?;
    let density = graph.parse(&json!({"type":"minecraft:blend_density","argument":0.375}))?;
    let grid = graph.grid().context("grid missing")?;
    let mut owner = ChunkDensity::new_with_blender(
        grid,
        chunk.0 << 4,
        chunk.1 << 4,
        (16 / grid.cell_width) as usize,
        Arc::clone(&blender),
    )?;
    owner.wrap_router(&graph)?;
    for id in [alpha, offset, density]
        .iter()
        .copied()
        .chain(roots.iter().map(|(_, id)| *id))
    {
        owner.wrap(&graph, id)?;
    }
    let mut prefill = Vec::new();
    for x in 0..=4 {
        for z in 0..=4 {
            let p = DensityPoint::new((chunk.0 << 4) + x * 4, 0, (chunk.1 << 4) + z * 4);
            prefill.push(json!([
                x,
                z,
                bits(owner.sample_external(&graph, alpha, p)?),
                bits(owner.sample_external(&graph, offset, p)?)
            ]));
        }
    }
    let points = q["cache_points"]
        .as_array()
        .context("cache points missing")?
        .iter()
        .map(triple)
        .collect::<Result<Vec<_>>>()?;
    let mut cache_samples = Vec::new();
    for p in &points {
        cache_samples.push(json!([
            p.x,
            p.y,
            p.z,
            bits(owner.sample_external(&graph, alpha, *p)?),
            bits(owner.sample_external(&graph, offset, *p)?)
        ]));
    }
    let own = owner.point();
    let contexts = json!([
        [
            "owner",
            own.x,
            own.y,
            own.z,
            bits(owner.sample(&graph, density)?)
        ],
        [
            "external",
            own.x,
            own.y,
            own.z,
            bits(owner.sample_external(&graph, density, own)?)
        ]
    ]);
    let mut custom = Vec::new();
    for (name, id) in roots {
        let raw = graph.bounds(id);
        let wrapped = owner.wrapped_bounds(id)?;
        let samples = points
            .iter()
            .map(|p| owner.sample_external(&graph, id, *p).map(bits))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        owner.fill_external(&graph, constant, &points)?;
        let mut values = vec![0.0; points.len()];
        let mut spy = FillSpy {
            owner: &mut owner,
            calls: Vec::new(),
        };
        graph.fill_sampled(id, &mut values, &mut spy)?;
        custom.push(json!({"name":name,"raw_bounds":[bits(raw.min),bits(raw.max)],"wrapped_bounds":[bits(wrapped.min),bits(wrapped.max)],"samples":samples,"fill":doubles(&values),"for_index_calls":spy.calls}));
    }
    // Independent production-path job. Component sentinel probes above cannot
    // warm or replace this job's real cache/aquifer owner.
    let generator = Arc::new(NoiseGenerator::builtin(seed, settings)?);
    let prepared = generator.prepare_chunk_with_blender(
        chunk.0,
        chunk.1,
        -64,
        384,
        BlendedGenerationContext {
            initial_blocks: GenerationInfluence::ObservedEmpty,
            structure_beardifier: GenerationInfluence::ObservedEmpty,
            blender: Arc::clone(&blender),
        },
    )?;
    let retained = Arc::ptr_eq(&prepared.blender()?, &blender);
    let mut observed = ObserveResolver {
        source: &mut *source,
        expected: Arc::clone(&blender),
        calls: Vec::new(),
    };
    let job = PreparedBiomes::fill(prepared, RetrogenContext::ObservedAbsent, &mut observed)?;
    let fallback = observed.calls;
    let id = q["id"].as_str().context("id missing")?;
    let biome_file = output.join(format!("native-blending-biomes-{id}.json"));
    std::fs::write(&biome_file, serde_json::to_vec(job.biomes().values())?)?;
    let mut terrain = job.fill_noise()?;
    let file = output.join(format!("native-blending-{id}.bin"));
    let noise_image = terrain_image(terrain.terrain(), &file)?;
    let noise_states = terrain.terrain().states().to_vec();
    let shift = mc121_rust::noise::NormalNoise::new(
        &mut mc121_rust::random::XoroshiroRandomSource::new(42),
        mc121_rust::noise::NoiseParameters {
            first_octave: -3,
            amplitudes: vec![1.0, 1.0, 1.0, 0.0],
        },
    );
    let shift_values = [
        [0, 0],
        [1, -1],
        [-1, 1],
        [27, -27],
        [1875066, 1875066],
        [i32::MAX, i32::MIN],
    ]
    .map(|p| {
        json!([
            p[0],
            p[1],
            bits(shift.noise(f64::from(p[0]), 0.0, f64::from(p[1])))
        ])
    });

    // Controlled continuation history: finish the existing center BIOMES and
    // NOISE first, then assemble the 3x3 palette owners with X outer/Z inner.
    // Existing old palettes are borrowed; new jobs retain their own prepared
    // sampler/aquifer and center-specific Blender over the same old data pool.
    let mut neighbors = SurfaceNeighbors {
        palettes: BTreeMap::new(),
        queries: Vec::new(),
    };
    let mut surface_neighbors = Vec::new();
    for dx in -1_i32..=1 {
        for dz in -1_i32..=1 {
            let position = (chunk.0.wrapping_add(dx), chunk.1.wrapping_add(dz));
            if position == chunk {
                surface_neighbors.push(json!({
                    "chunk":[position.0,position.1],"owner":"center","biome_file":biome_file
                }));
                continue;
            }
            let (owner_name, palette) = match hosts.iter().find(|host| host.chunk == position) {
                Some(snapshot) => ("old", SurfacePalette::Old(snapshot)),
                None => {
                    let neighbor_blender =
                        neighbor_blender(position, &data, q["singleton_empty"] == true)?;
                    let prepared = generator.prepare_chunk_with_blender(
                        position.0,
                        position.1,
                        -64,
                        384,
                        BlendedGenerationContext {
                            initial_blocks: GenerationInfluence::ObservedEmpty,
                            structure_beardifier: GenerationInfluence::ObservedEmpty,
                            blender: neighbor_blender,
                        },
                    )?;
                    let prepared = PreparedBiomes::fill(
                        prepared,
                        RetrogenContext::ObservedAbsent,
                        &mut *source,
                    )?;
                    ("new", SurfacePalette::New(prepared))
                }
            };
            let palette_file = output.join(format!(
                "native-blending-surface-biomes-{id}-{}-{}.json",
                position.0, position.1
            ));
            std::fs::write(&palette_file, serde_json::to_vec(palette.values())?)?;
            surface_neighbors.push(json!({
                "chunk":[position.0,position.1],"owner":owner_name,"biome_file":palette_file
            }));
            neighbors.palettes.insert(position, palette);
        }
    }

    let center_palette_owner = terrain.biomes() as *const _;
    let center_noise_owner = terrain.terrain() as *const _;
    let center_palette_values = terrain.biomes().values().to_vec();
    let neighbor_palette_values = neighbors
        .palettes
        .iter()
        .map(|(position, palette)| (*position, palette.values().to_vec()))
        .collect::<Vec<_>>();
    terrain.build_surface(
        BiomeZoom::from_world_seed(seed),
        &mut neighbors,
        temperature,
    )?;
    anyhow::ensure!(
        center_palette_owner == terrain.biomes() as *const _
            && center_noise_owner == terrain.terrain() as *const _,
        "SURFACE replaced the retained native center owner"
    );
    let mut surface_image = terrain_image(
        terrain.terrain(),
        &output.join(format!("native-blending-surface-{id}.bin")),
    )?;
    surface_image["stored_biomes_unchanged"] = json!(
        center_palette_values == terrain.biomes().values()
            && neighbor_palette_values.iter().all(|(position, values)| {
                neighbors
                    .palettes
                    .get(position)
                    .is_some_and(|palette| palette.values() == values)
            })
    );
    surface_image["changed_blocks_from_noise"] = json!(noise_states
        .iter()
        .zip(terrain.terrain().states())
        .filter(|(before, after)| before != after)
        .count());
    Ok(
        json!({"shift_noise":shift_values,"query":q,"data":data_images,"age_queries":ages,"second_calculation_skipped":second,"height_map_order":height_order,"density_map_order":density_order,"height_to_offset":heights,
        "scalars":scalars,"resolved_biomes":resolved,"resolver_calls":resolver_calls,"prefill":prefill,"cache_samples":cache_samples,"density_contexts":contexts,"custom":custom,
        "biome_file":biome_file,"biome_fallback":fallback,"prepared_blender_identity":retained,
        "noise":noise_image,"surface":surface_image,"surface_neighbors":surface_neighbors,
        "surface_biome_queries":neighbors.queries}),
    )
}
// This compiles the exact production source file, rather than a copied map
// algorithm. Primitive value owners do not represent fabricated terrain data.
#[allow(dead_code)]
#[path = "../blending/ordered_map.rs"]
mod source_ordered_map;
fn map_witness(q: &Value) -> Result<Value> {
    use source_ordered_map::SourceLongMap;
    let stencil = (-7_i32..=7)
        .flat_map(|x| (-7_i32..=7).filter_map(move |z| (x * x + z * z <= 64).then_some((x, z))))
        .collect::<Vec<_>>();
    let mut map = SourceLongMap::<Arc<Value>>::new();
    let mut rows = Vec::new();
    let mut zero = None;
    for i in 0..=stencil.len() {
        if [0, 1, 24, 25, 48, 49, 96, 97, 192, 193].contains(&i) {
            let order = map
                .iter()
                .map(|(k, v)| json!([k as i32, (k >> 32) as i32, v.as_ref()]))
                .collect::<Vec<_>>();
            rows.push(json!({"size":i,"order":order}));
        }
        if let Some(&(x, z)) = stencil.get(i) {
            let owner = Arc::new(json!(i + 1));
            if x == 0 && z == 0 {
                zero = Some(Arc::clone(&owner));
            }
            map.insert(chunk_key(x, z), owner);
        }
    }
    let before = map.iter().map(|(k, _)| k).collect::<Vec<_>>();
    let replacement = Arc::new(json!("replacement-owner"));
    let previous = map.insert(0, Arc::clone(&replacement));
    let after = map.iter().map(|(k, _)| k).collect::<Vec<_>>();
    let mut collision_map = SourceLongMap::new();
    for (i, p) in q["collision_keys"]
        .as_array()
        .context("collision input missing")?
        .iter()
        .enumerate()
    {
        let x = p[0].as_i64().context("collision X")? as i32;
        let z = p[1].as_i64().context("collision Z")? as i32;
        collision_map.insert(chunk_key(x, z), i + 1);
    }
    let collision_order = collision_map
        .iter()
        .map(|(k, v)| json!([k as i32, (k >> 32) as i32, v]))
        .collect::<Vec<_>>();
    Ok(
        json!({"stencil":rows,"replacement_preserves_order":before==after,"replacement_returns_old_owner":previous.as_ref().zip(zero.as_ref()).is_some_and(|(a,b)|Arc::ptr_eq(a,b)),"replacement_retains_new_owner":map.get(0).is_some_and(|v|Arc::ptr_eq(v,&replacement)),"collision_order":collision_order}),
    )
}
fn main() -> Result<()> {
    let output = std::env::args()
        .nth(1)
        .context("usage: blending_probe <output-directory>")?;
    let output = Path::new(&output);
    std::fs::create_dir_all(output)?;
    let mut source = BuiltinMultiNoiseBiomeSource::new(MultiNoisePreset::Overworld)?;
    let mut temperature = BiomeTemperature::builtin()?;
    for line in io::stdin().lock().lines() {
        let q: Value = serde_json::from_str(&line?)?;
        println!(
            "{}",
            if q["mode"] == "maps" {
                map_witness(&q)?
            } else {
                run(q, output, &mut source, &mut temperature)?
            }
        );
    }
    Ok(())
}
