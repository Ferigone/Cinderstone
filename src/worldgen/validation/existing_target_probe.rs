//! Controlled original INITIAL -> native existing-target phase replay.
//! Only INITIAL storage is imported; later original phase outputs are rejected.
use anyhow::{bail, ensure, Context, Result};
use mc121_rust::{
    chunk_readiness::ChunkStatus,
    density::DensityPoint,
    worldgen::{
        beardifier::{Beardifier, BlockBox, Junction, Rigid, TerrainAdjustment},
        biome_source::{select_end_biome, BuiltinMultiNoiseBiomeSource, MultiNoisePreset},
        biome_temperature::BiomeTemperature,
        biome_zoom::{BiomeZoom, QuartBiomeProvider},
        biomes::ResolvedBiome,
        blending::Blender,
        generator::{NoiseGenerator, TargetGeneration, TargetSurfaceOutcome},
        noise_chunk::NoiseFillPlan,
        retrogen::{parse_status, BelowZeroRetrogen, RetrogenSlot},
        target::{
            BuiltinStateFacts, GenerationTargetHost, HeightRange, NativeHeightmap,
            NativeProtoTarget, NativeSection, NativeTargetParts, NoiseHeightmap, NoiseSectionLease,
            SectionCounts, SourceStateFacts, TargetDefaultStates, TargetStateFacts,
        },
    },
};
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    io::{self, BufRead},
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

fn i(v: &Value, key: &str) -> Result<i32> {
    Ok(v[key]
        .as_i64()
        .with_context(|| format!("missing integer {key}"))?
        .try_into()?)
}
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .with_context(|| format!("missing string {key}"))
}
fn boolean(v: &Value, key: &str) -> Result<bool> {
    v[key]
        .as_bool()
        .with_context(|| format!("missing observed boolean {key}"))
}
fn seed(v: &Value) -> Result<i64> {
    if let Some(value) = v["seed"].as_i64() {
        return Ok(value);
    }
    Ok(text(v, "seed")?.parse()?)
}

fn point(v: &Value) -> Result<DensityPoint> {
    let values = v.as_array().context("beardifier point must be an array")?;
    ensure!(
        values.len() == 3,
        "beardifier point needs three coordinates"
    );
    let coordinate = |n: usize| -> Result<i32> {
        Ok(values[n]
            .as_i64()
            .context("noninteger beardifier coordinate")?
            .try_into()?)
    };
    Ok(DensityPoint::new(
        coordinate(0)?,
        coordinate(1)?,
        coordinate(2)?,
    ))
}

/// Ordered data ingress only. These declared parts are not a StructureManager
/// selection/dependency witness and contain no future original phase output.
fn beardifier(q: &Value) -> Result<Arc<Beardifier>> {
    let Some(parts) = q.get("beardifier") else {
        return Ok(Beardifier::empty());
    };
    let mut rigids = Vec::new();
    for row in parts["rigids"]
        .as_array()
        .context("missing ordered rigid list")?
    {
        let bounds = row["box"].as_array().context("missing rigid box")?;
        ensure!(bounds.len() == 6, "rigid box needs six coordinates");
        let coordinate = |n: usize| -> Result<i32> {
            Ok(bounds[n]
                .as_i64()
                .context("noninteger rigid box coordinate")?
                .try_into()?)
        };
        let terrain_adjustment = match text(row, "terrain_adjustment")? {
            "none" => TerrainAdjustment::None,
            "bury" => TerrainAdjustment::Bury,
            "beard_thin" => TerrainAdjustment::BeardThin,
            "beard_box" => TerrainAdjustment::BeardBox,
            "encapsulate" => TerrainAdjustment::Encapsulate,
            value => bail!("unsupported terrain adjustment {value}"),
        };
        rigids.push(Rigid {
            bounds: BlockBox {
                min_x: coordinate(0)?,
                min_y: coordinate(1)?,
                min_z: coordinate(2)?,
                max_x: coordinate(3)?,
                max_y: coordinate(4)?,
                max_z: coordinate(5)?,
            },
            terrain_adjustment,
            ground_level_delta: i(row, "ground_level_delta")?,
        });
    }
    let mut junctions = Vec::new();
    for row in parts["junctions"]
        .as_array()
        .context("missing ordered junction list")?
    {
        junctions.push(Junction {
            source_x: i(row, "source_x")?,
            source_ground_y: i(row, "source_ground_y")?,
            source_z: i(row, "source_z")?,
        });
    }
    Ok(Beardifier::from_ordered_parts(rigids, junctions))
}

fn beardifier_observation(q: &Value, owner: &Beardifier) -> Result<Value> {
    let mut samples = Vec::new();
    if let Some(points) = q.get("beardifier_points") {
        for coordinates in points
            .as_array()
            .context("beardifier_points must be an array")?
        {
            let p = point(coordinates)?;
            samples.push(json!([
                p.x,
                p.y,
                p.z,
                owner.compute(p).to_bits().to_string()
            ]));
        }
    }
    Ok(json!({
        "beardifier_samples":samples,
        "beardifier_min_bits":f64::NEG_INFINITY.to_bits().to_string(),
        "beardifier_max_bits":f64::INFINITY.to_bits().to_string()
    }))
}
fn digest(bytes: &[u8]) -> String {
    openssl::sha::sha256(bytes)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect()
}
fn words(v: &Value) -> Result<Vec<u64>> {
    v.as_array()
        .context("missing bitset words")?
        .iter()
        .map(|x| {
            if let Some(n) = x.as_i64() {
                return Ok(n as u64);
            }
            let s = x.as_str().context("bitset word is not an integer/string")?;
            Ok(if s.starts_with('-') {
                s.parse::<i64>()? as u64
            } else {
                s.parse()?
            })
        })
        .collect()
}
fn path(base: &Path, name: &str) -> PathBuf {
    let value = Path::new(name);
    if value.is_absolute() {
        value.to_owned()
    } else {
        base.join(value)
    }
}
fn biome_file(base: &Path, name: &str) -> Result<Vec<String>> {
    Ok(serde_json::from_slice(&std::fs::read(path(base, name))?)?)
}
fn heightmap(initial: &Value, key: &str, build: HeightRange) -> Result<NativeHeightmap> {
    let heights = initial[key]
        .as_array()
        .with_context(|| format!("INITIAL lacks {key}"))?;
    ensure!(heights.len() == 256, "incomplete INITIAL heightmap");
    let bits = 32 - (build.height as u32).leading_zeros();
    let per_long = 64 / bits as usize;
    let mask = (1_u64 << bits) - 1;
    let mut raw = vec![0_u64; 256_usize.div_ceil(per_long)];
    for (index, value) in heights.iter().enumerate() {
        let first: i32 = value
            .as_i64()
            .context("noninteger INITIAL height")?
            .try_into()?;
        let offset = first
            .checked_sub(build.min_y)
            .context("height offset overflow")?;
        ensure!(
            offset >= 0 && offset as u64 <= mask,
            "INITIAL height cannot fit source storage"
        );
        raw[index / per_long] |= (offset as u64) << ((index % per_long) * bits as usize);
    }
    NativeHeightmap::from_raw(build, raw.into_boxed_slice())
}

type Trace = Rc<RefCell<Option<Vec<Value>>>>;

/// Every operation delegates to the real native section-backed target. During
/// SURFACE only, the biome callback exposes raw stored palettes, matching the
/// original oracle's explicit base-ChunkAccess storage view at actual EMPTY.
/// No persisted/highest status is manufactured to bypass the ProtoChunk guard.
struct ObservedTarget {
    native: Box<NativeProtoTarget>,
    surface_trace: Trace,
}
impl GenerationTargetHost for ObservedTarget {
    type SectionLease = NoiseSectionLease;
    fn chunk_pos(&self) -> (i32, i32) {
        self.native.chunk_pos()
    }
    fn build_height(&self) -> HeightRange {
        self.native.build_height()
    }
    fn generation_height(&self) -> Result<HeightRange> {
        self.native.generation_height()
    }
    fn persisted_status(&self) -> ChunkStatus {
        self.native.persisted_status()
    }
    fn highest_generated_status(&self) -> Result<ChunkStatus> {
        self.native.highest_generated_status()
    }
    fn retrogen(&self) -> Result<Option<Arc<BelowZeroRetrogen>>> {
        self.native.retrogen()
    }
    fn state_facts(&self, state: u32) -> Result<TargetStateFacts> {
        self.native.state_facts(state)
    }
    fn default_states(&self) -> TargetDefaultStates {
        self.native.default_states()
    }
    fn debug_void_terrain(&self) -> Result<bool> {
        self.native.debug_void_terrain()
    }
    fn acquire_noise_sections(&mut self, top: i32, bottom: i32) -> Result<Self::SectionLease> {
        self.native.acquire_noise_sections(top, bottom)
    }
    fn get_or_create_wg_heightmap(&mut self, kind: NoiseHeightmap) -> Result<()> {
        self.native.get_or_create_wg_heightmap(kind)
    }
    fn direct_section_set(
        &mut self,
        lease: &Self::SectionLease,
        p: DensityPoint,
        state: u32,
    ) -> Result<u32> {
        self.native.direct_section_set(lease, p, state)
    }
    fn update_wg_heightmap(
        &mut self,
        kind: NoiseHeightmap,
        p: DensityPoint,
        state: u32,
    ) -> Result<bool> {
        self.native.update_wg_heightmap(kind, p, state)
    }
    fn mark_pos_for_postprocessing(&mut self, p: DensityPoint) -> Result<()> {
        self.native.mark_pos_for_postprocessing(p)
    }
    fn block(&self, p: DensityPoint) -> Result<u32> {
        self.native.block(p)
    }
    fn proto_set_block(&mut self, p: DensityPoint, state: u32) -> Result<u32> {
        self.native.proto_set_block(p, state)
    }
    fn wg_height(&mut self, kind: NoiseHeightmap, x: i32, z: i32) -> Result<i32> {
        self.native.wg_height(kind, x, z)
    }
    fn get_noise_biome(&self, x: i32, y: i32, z: i32) -> Result<String> {
        if self.surface_trace.borrow().is_none() {
            return self.native.get_noise_biome(x, y, z);
        }
        let build = self.native.build_height();
        let value = stored_biome(build, self.native.sections(), x, y, z)?;
        self.surface_trace
            .borrow_mut()
            .as_mut()
            .unwrap()
            .push(json!([x, y, z, value]));
        Ok(value)
    }
    fn biome_registry_key(&self, identity: &str) -> Result<Option<String>> {
        self.native.biome_registry_key(identity)
    }
    fn commit_biome_section(&mut self, y: i32, biomes: Box<[String; 64]>) -> Result<()> {
        self.native.commit_biome_section(y, biomes)
    }
}
fn stored_biome(
    build: HeightRange,
    sections: &[Box<NativeSection>],
    x: i32,
    y: i32,
    z: i32,
) -> Result<String> {
    let min = build.min_y >> 2;
    let max = min.wrapping_add(build.height >> 2).wrapping_sub(1);
    let y = y.max(min).min(max);
    let section = y.div_euclid(4) - build.min_section_y();
    let section = sections
        .get(usize::try_from(section)?)
        .context("stored biome outside actual sections")?;
    Ok(section.biomes()[(((y & 3) << 4) | ((z & 3) << 2) | (x & 3)) as usize].clone())
}
struct Neighbor {
    build: HeightRange,
    values: Vec<String>,
}
struct Neighbors {
    chunks: BTreeMap<(i32, i32), Neighbor>,
    trace: Trace,
}
impl QuartBiomeProvider for Neighbors {
    type Biome = String;
    type Error = anyhow::Error;
    fn get_noise_biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
        let owner = self
            .chunks
            .get(&(x.div_euclid(4), z.div_euclid(4)))
            .context("missing actual supplied SURFACE neighbor")?;
        let min = owner.build.min_y >> 2;
        let max = min.wrapping_add(owner.build.height >> 2).wrapping_sub(1);
        let clamped = y.max(min).min(max);
        let index = (clamped - owner.build.min_section_y() * 4) as usize * 16
            + (z & 3) as usize * 4
            + (x & 3) as usize;
        let value = owner
            .values
            .get(index)
            .context("neighbor palette outside physical sections")?
            .clone();
        if let Some(trace) = self.trace.borrow_mut().as_mut() {
            trace.push(json!([x, y, z, value]));
        }
        Ok(value)
    }
}

fn import(q: &Value, initial: &Value, base: &Path, trace: Trace) -> Result<Box<ObservedTarget>> {
    let build = HeightRange::new(i(q, "level_min_y")?, i(q, "level_height")?)?;
    let physical = build.physical_sections()?;
    ensure!(
        i(initial, "physical_min_y")? == physical.min_y
            && i(initial, "physical_height")? == physical.height,
        "INITIAL physical section geometry differs"
    );
    let bytes = std::fs::read(path(base, text(initial, "blocks_file")?))?;
    ensure!(
        digest(&bytes) == text(initial, "blocks_sha256")?,
        "stale INITIAL block artifact"
    );
    ensure!(
        bytes.len() == physical.height as usize * 256 * 4,
        "INITIAL block image incomplete"
    );
    let biome_bytes = std::fs::read(path(base, text(initial, "biomes_file")?))?;
    ensure!(
        digest(&biome_bytes) == text(initial, "biomes_sha256")?,
        "stale INITIAL biome artifact"
    );
    let biomes: Vec<String> = serde_json::from_slice(&biome_bytes)?;
    let count = build.section_count()?;
    ensure!(biomes.len() == count * 64, "INITIAL biome image incomplete");
    let counts = initial["section_counts"]
        .as_array()
        .context("INITIAL section counts missing")?;
    ensure!(counts.len() == count, "INITIAL count coverage differs");
    let facts: Arc<dyn SourceStateFacts> = Arc::new(BuiltinStateFacts::new()?);
    let mut sections = Vec::new();
    for section in 0..count {
        let mut states = Box::new([0_u32; 4096]);
        for ly in 0..16 {
            for x in 0..16 {
                for z in 0..16 {
                    let source = ((section * 16 + ly) * 256 + x * 16 + z) * 4;
                    states[ly * 256 + z * 16 + x] =
                        u32::from_le_bytes(bytes[source..source + 4].try_into()?);
                }
            }
        }
        let palette: Box<[String; 64]> = biomes[section * 64..(section + 1) * 64]
            .to_vec()
            .into_boxed_slice()
            .try_into()
            .map_err(|_| anyhow::anyhow!("biome palette length"))?;
        let c = &counts[section];
        sections.push(NativeSection::from_existing_parts(
            states,
            palette,
            SectionCounts {
                non_empty_blocks: i(c, "non_empty_block_count")?.try_into()?,
                ticking_blocks: i(c, "ticking_block_count")?.try_into()?,
                ticking_fluids: i(c, "ticking_fluid_count")?.try_into()?,
            },
            &*facts,
        )?);
    }
    let posts = initial["posts"]
        .as_array()
        .context("INITIAL queues missing")?;
    let allocated = initial["post_allocated"]
        .as_array()
        .context("INITIAL queue allocation missing")?;
    ensure!(
        posts.len() == count && allocated.len() == count,
        "INITIAL queue coverage differs"
    );
    let mut queues = Vec::new();
    for (row, exists) in posts.iter().zip(allocated) {
        let values = row.as_array().context("invalid INITIAL queue")?;
        let exists = exists.as_bool().context("invalid INITIAL allocation")?;
        if !exists {
            ensure!(values.is_empty(), "unallocated INITIAL queue has values");
            queues.push(None);
        } else {
            queues.push(Some(
                values
                    .iter()
                    .map(|v| Ok(i16::try_from(v.as_i64().context("invalid signed post")?)?))
                    .collect::<Result<Vec<_>>>()?,
            ));
        }
    }
    let retrogen = if boolean(initial, "retrogen_present")? {
        let input = q
            .get("retrogen")
            .context("INITIAL retrogen lacks actual input payload")?;
        RetrogenSlot::Present(Arc::new(BelowZeroRetrogen::new(
            parse_status(text(input, "target_status")?)?,
            input
                .get("missing_bedrock")
                .map(words)
                .transpose()?
                .unwrap_or_default(),
        )?))
    } else {
        ensure!(
            q.get("retrogen").is_none(),
            "input retrogen differs from INITIAL"
        );
        RetrogenSlot::ObservedAbsent
    };
    let native = NativeProtoTarget::from_parts(
        NativeTargetParts {
            chunk: (i(q, "x")?, i(q, "z")?),
            build,
            sections: sections.into_boxed_slice(),
            persisted_status: parse_status(text(initial, "persisted_status")?)?,
            unsaved: boolean(initial, "unsaved")?,
            retrogen,
            ocean_floor_wg: Some(heightmap(initial, "ocean_floor_wg", build)?),
            world_surface_wg: Some(heightmap(initial, "world_surface_wg", build)?),
            postprocessing: queues.into_boxed_slice(),
            debug_void_terrain: boolean(initial, "debug_void_terrain")?,
            unrelated: (),
        },
        facts,
    )?;
    Ok(Box::new(ObservedTarget {
        native,
        surface_trace: trace,
    }))
}

fn snapshot(target: &ObservedTarget, base: &Path, id: &str, phase: &str) -> Result<Value> {
    let t = &*target.native;
    let physical = t.physical_height()?;
    let mut bytes = Vec::with_capacity(physical.height as usize * 256 * 4);
    for section in t.sections() {
        for y in 0..16 {
            for x in 0..16 {
                for z in 0..16 {
                    bytes.extend_from_slice(&section.states()[y * 256 + z * 16 + x].to_le_bytes());
                }
            }
        }
    }
    let biomes: Vec<_> = t
        .sections()
        .iter()
        .flat_map(|s| s.biomes().iter().cloned())
        .collect();
    let block_file = base.join(format!("native-target-{id}-{phase}.bin"));
    std::fs::write(&block_file, &bytes)?;
    let biome_file = base.join(format!("native-target-{id}-{phase}-biomes.json"));
    let biome_bytes = serde_json::to_vec(&biomes)?;
    std::fs::write(&biome_file, &biome_bytes)?;
    let heights = |kind| -> Result<Vec<i32>> {
        let map = t
            .heightmap(kind)
            .context("snapshot lacks actual INITIAL/phase WG map")?;
        Ok((0..16)
            .flat_map(|z| (0..16).map(move |x| map.first_available(x, z)))
            .collect())
    };
    let posts: Vec<_> = t
        .postprocessing()
        .iter()
        .map(|r| r.clone().unwrap_or_default())
        .collect();
    let count: usize = posts.iter().map(Vec::len).sum();
    let generation = t.generation_height()?;
    Ok(
        json!({"blocks_file":block_file,"blocks_sha256":digest(&bytes),"biomes_file":biome_file,
        "biomes_sha256":digest(&biome_bytes),"physical_min_y":physical.min_y,"physical_height":physical.height,
        "world_surface_wg":heights(NoiseHeightmap::WorldSurfaceWg)?,"ocean_floor_wg":heights(NoiseHeightmap::OceanFloorWg)?,
        "posts":posts,"post_count":count,"post_allocated":t.postprocessing().iter().map(Option::is_some).collect::<Vec<_>>(),
        "section_counts":t.sections().iter().map(|s|{let c=s.counts();json!({"non_empty_block_count":c.non_empty_blocks,
            "ticking_block_count":c.ticking_blocks,"ticking_fluid_count":c.ticking_fluids})}).collect::<Vec<_>>(),
        "persisted_status":t.persisted_status().name(),"highest_generated_status":t.highest_generated_status()?.name(),
        "retrogen_present":t.retrogen()?.is_some(),"generation_min_y":generation.min_y,"generation_height":generation.height,
        "unsaved":t.is_unsaved(),"debug_void_terrain":t.debug_void_terrain()?,
        "all_section_leases_released":t.sections().iter().all(|s|!s.is_acquired())}),
    )
}
fn pointers(t: &ObservedTarget) -> Vec<usize> {
    let n = &*t.native;
    let mut result = vec![
        t as *const _ as usize,
        n as *const _ as usize,
        n.sections().as_ptr() as usize,
        n.postprocessing().as_ptr() as usize,
    ];
    for s in n.sections() {
        result.push((&**s) as *const _ as usize);
        result.push(s.states().as_ptr() as usize);
    }
    for kind in [NoiseHeightmap::OceanFloorWg, NoiseHeightmap::WorldSurfaceWg] {
        result.push(n.heightmap(kind).map_or(0, |m| m.raw().as_ptr() as usize));
    }
    result
}

fn run(
    input: Value,
    base: &Path,
    sources: &mut BTreeMap<String, BuiltinMultiNoiseBiomeSource>,
    temperature: &mut BiomeTemperature,
) -> Result<Value> {
    for future in ["biomes", "noise_before_retrogen", "noise", "surface"] {
        ensure!(
            input.get(future).is_none(),
            "native replay must not receive original future phase {future}"
        );
    }
    let q = input.get("query").unwrap_or(&input);
    if q.get("mode").and_then(Value::as_str) == Some("beardifier_kernel") {
        ensure!(
            input
                .as_object()
                .is_some_and(|row| row.len() == 1 && row.contains_key("query")),
            "kernel replay accepts exactly query and no original state"
        );
        let owner = beardifier(q)?;
        let mut result = beardifier_observation(q, &owner)?;
        result["query"] = q.clone();
        return Ok(result);
    }
    let initial = input
        .get("initial")
        .context("only original INITIAL must be supplied")?;
    let id = text(q, "id")?;
    ensure!(
        !id.is_empty()
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
        "unsafe case ID"
    );
    let trace = Rc::new(RefCell::new(None));
    let target = import(q, initial, base, Rc::clone(&trace))?;
    let identity = pointers(&target);
    let generator = Arc::new(NoiseGenerator::builtin(seed(q)?, text(q, "settings")?)?);
    let actual_seed = generator.seed();
    let settings = generator
        .graph()
        .grid()
        .context("actual native noise settings missing")?;
    let generation = target.generation_height()?;
    let plan = NoiseFillPlan::new(settings, generation.min_y, generation.height)?;
    let uses_legacy = generator.graph().uses_legacy_random_source();
    let skip_biomes = q
        .get("skip_biomes")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let structure_owner = if skip_biomes {
        ensure!(
            q.get("beardifier").is_none(),
            "unavailable no-BIOMES context cannot supply structure parts"
        );
        None
    } else {
        Some(beardifier(q)?)
    };
    let structure_observation = structure_owner
        .as_ref()
        .map(|owner| beardifier_observation(q, owner))
        .transpose()?;
    ensure!(
        structure_owner.is_some() || q.get("beardifier_points").is_none(),
        "beardifier samples need an actual owner"
    );
    let mut job = TargetGeneration::new_with_influences(
        generator,
        target,
        if skip_biomes {
            None
        } else {
            Some(Blender::empty())
        },
        structure_owner.clone(),
    );
    let mut result = json!({"query":q,"initial":snapshot(job.target(),base,id,"initial")?,"native_cache_initially_absent":!job.has_cached_noise_chunk()});
    if let Some(observation) = structure_observation {
        for (key, value) in observation.as_object().expect("observation object") {
            result[key] = value.clone();
        }
    }
    let source_name = if skip_biomes {
        String::new()
    } else {
        text(q, "biome_source")?.to_owned()
    };
    let mut biome_queries = Vec::new();
    let mut sampler_pointer = None;
    let mut same_sampler = true;
    if !skip_biomes {
        if !sources.contains_key(&source_name)
            && source_name != "minecraft:end"
            && source_name != "end"
        {
            let preset = match source_name.as_str() {
                "minecraft:overworld" | "overworld" => MultiNoisePreset::Overworld,
                "minecraft:nether" | "nether" => MultiNoisePreset::Nether,
                _ => bail!("unsupported explicit biome source"),
            };
            sources.insert(
                source_name.clone(),
                BuiltinMultiNoiseBiomeSource::new(preset)?,
            );
        }
        job.fill_biomes(|sampler, x, y, z| {
            let pointer = sampler as *const _ as usize;
            if let Some(prior) = sampler_pointer {
                same_sampler &= prior == pointer;
            } else {
                sampler_pointer = Some(pointer);
            }
            let biome = if source_name == "minecraft:end" || source_name == "end" {
                select_end_biome(x, y, z, sampler)?.to_owned()
            } else {
                sources
                    .get_mut(&source_name)
                    .expect("existing worker")
                    .get_noise_biome(x, y, z, sampler)?
            };
            biome_queries.push(json!([x, y, z, biome]));
            Ok(ResolvedBiome::from_holder(biome.clone(), Some(biome)))
        })?;
        result["biomes"] = snapshot(job.target(), base, id, "biomes")?;
        result["native_beardifier_identity_after_biomes"] = json!(job
            .captured_beardifier()?
            .as_ref()
            .zip(structure_owner.as_ref())
            .is_some_and(|(captured, initial)| Arc::ptr_eq(captured, initial)));
    }
    result["biomes_ran"] = json!(!skip_biomes);
    if !skip_biomes {
        result["biomes_same_sampler"] = json!(same_sampler);
    }
    result["biome_queries"] = json!(biome_queries);
    let cached_before_noise = job.has_cached_noise_chunk();
    let outcome = job.fill_from_noise()?;
    result["material_outcome"] = json!(format!("{outcome:?}"));
    result["noise_before_retrogen"] = snapshot(job.target(), base, id, "noise-before-retrogen")?;
    job.finish_noise_task()?;
    result["noise"] = snapshot(job.target(), base, id, "noise")?;
    if structure_owner.is_some() {
        result["native_beardifier_identity_after_noise"] = json!(job
            .captured_beardifier()?
            .as_ref()
            .zip(structure_owner.as_ref())
            .is_some_and(|(captured, initial)| Arc::ptr_eq(captured, initial)));
    }
    if let Some(descriptions) = q.get("surface_neighbors") {
        let mut neighbors = Neighbors {
            chunks: BTreeMap::new(),
            trace: Rc::clone(&trace),
        };
        for row in descriptions
            .as_array()
            .context("invalid supplied neighbors")?
        {
            let chunk = (i(row, "x")?, i(row, "z")?);
            ensure!(
                chunk != job.target().chunk_pos(),
                "center supplied as foreign neighbor"
            );
            let build = HeightRange::new(i(row, "level_min_y")?, i(row, "level_height")?)?;
            let values = biome_file(base, text(row, "biomes_file")?)?;
            ensure!(
                values.len() == build.section_count()? * 64,
                "incomplete supplied neighbor"
            );
            ensure!(
                neighbors
                    .chunks
                    .insert(chunk, Neighbor { build, values })
                    .is_none(),
                "duplicate supplied neighbor"
            );
        }
        *trace.borrow_mut() = Some(Vec::new());
        let build = job.target().build_height();
        let surface = job.build_surface_with_influences(
            Blender::empty(),
            None,
            build,
            BiomeZoom::from_world_seed(actual_seed),
            &mut neighbors,
            temperature,
        );
        result["surface_biome_queries"] = json!(trace.borrow_mut().take().expect("active trace"));
        match surface {
            Ok(TargetSurfaceOutcome::Built(report)) => {
                result["surface"] = snapshot(job.target(), base, id, "surface")?;
                result["surface_report"] =
                    json!({"columns":report.columns,"rule_replacements":report.rule_replacements});
            }
            Ok(TargetSurfaceOutcome::DebugVoidSkipped) => {
                result["surface_skipped"] = json!(true);
                result["surface"] = snapshot(job.target(), base, id, "surface")?;
            }
            Err(error) => {
                result["error"] = json!(format!("{error:#}"));
                result["failed_surface"] = snapshot(job.target(), base, id, "failed-surface")?;
            }
        }
    } else {
        result["surface_biome_queries"] = json!([]);
    }
    if structure_owner.is_some() && q.get("surface_neighbors").is_some() {
        result["native_beardifier_identity_after_surface"] = json!(job
            .captured_beardifier()?
            .as_ref()
            .zip(structure_owner.as_ref())
            .is_some_and(|(captured, initial)| Arc::ptr_eq(captured, initial)));
    }
    result["surface_ran"] = json!(q.get("surface_neighbors").is_some());
    result["native_phase"] = json!(format!("{:?}", job.phase()));
    result["native_poisoned"] = json!(format!("{:?}", job.phase()) == "Poisoned");
    result["same_target_storage_owners"] = json!(identity == pointers(job.target()));
    result["native_cached_noise_chunk_present"] = json!(job.has_cached_noise_chunk());
    result["native_cache_present_before_noise"] = json!(cached_before_noise);
    result["native_private_aquifer_pointer_observed"] = json!(false);
    result["noise_chunk_absent"] = json!(!job.has_cached_noise_chunk());
    result["fill_cell_count_y"] = json!(plan.cell_count_y());
    result["native_noise_task_callback_executed"] = json!(true);
    result["native_use_legacy_random_source"] = json!(uses_legacy);
    result["persisted_status_unchanged"] =
        json!(text(initial, "persisted_status")? == job.target().persisted_status().name());
    result["surface_stored_biomes_unchanged"] = json!(result
        .get("surface")
        .map(|s| s["biomes_sha256"] == result["noise"]["biomes_sha256"])
        .unwrap_or(true));
    Ok(result)
}
fn main() -> Result<()> {
    let base = PathBuf::from(
        std::env::args()
            .nth(1)
            .context("usage: existing_target_probe <output-directory>")?,
    );
    std::fs::create_dir_all(&base)?;
    let mut sources = BTreeMap::new();
    let mut temperature = BiomeTemperature::builtin()?;
    for line in io::stdin().lock().lines() {
        let input: Value = serde_json::from_str(&line?)?;
        println!(
            "{}",
            serde_json::to_string(&run(input, &base, &mut sources, &mut temperature)?)?
        );
    }
    Ok(())
}
