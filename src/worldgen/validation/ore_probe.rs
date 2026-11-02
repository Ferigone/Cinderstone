//! Controlled ore-leaf host used only for paired original observations.
//! This is not a production WorldGenRegion, scheduler or runtime fallback.
use anyhow::{bail, ensure, Context, Result};
use mc121_rust::{
    density::DensityPoint,
    random::{WorldgenRandom, XoroshiroRandomSource},
    worldgen::{
        feature::{self, config::OreRuleTest, ConfiguredOreFeature, OreHost},
        generator::resolve_block_state,
    },
};
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    io::{self, BufRead},
    path::Path,
};

fn state(name: &str) -> Result<u32> {
    resolve_block_state(&json!({"Name":format!("minecraft:{name}")}))
}
fn point(p: DensityPoint) -> [i32; 3] {
    [p.x, p.y, p.z]
}
struct ChunkOwner {
    states: Vec<u32>,
    surface: Vec<i32>,
    floor: Vec<i32>,
}
struct ControlledRegion {
    query: Value,
    min: i32,
    height: i32,
    top: i32,
    origin: DensityPoint,
    write_radius: i32,
    profile: String,
    owners: BTreeMap<(i32, i32), ChunkOwner>,
    events: RefCell<Vec<Value>>,
    bulk: Option<BTreeSet<(i32, i32, i32)>>,
    bulk_begins: usize,
    bulk_closes: usize,
    acquisitions: usize,
    releases: usize,
    air: u32,
    void_air: u32,
    stone: u32,
    deepslate: u32,
    netherrack: u32,
}
impl ControlledRegion {
    fn new(query: Value) -> Result<Self> {
        let number = |key: &str| -> Result<i32> {
            Ok(i32::try_from(
                query[key].as_i64().context("missing integer")?,
            )?)
        };
        let p = query["origin"].as_array().context("missing origin")?;
        let origin = DensityPoint::new(
            p[0].as_i64().context("origin X")? as i32,
            p[1].as_i64().context("origin Y")? as i32,
            p[2].as_i64().context("origin Z")? as i32,
        );
        let radius = number("available_radius")?;
        let mut host = Self {
            min: number("min_y")?,
            height: number("height")?,
            top: number("top_y")?,
            write_radius: number("write_radius")?,
            profile: query["profile"]
                .as_str()
                .context("missing controlled material profile")?
                .to_owned(),
            query,
            origin,
            owners: BTreeMap::new(),
            events: RefCell::new(Vec::new()),
            bulk: None,
            bulk_begins: 0,
            bulk_closes: 0,
            acquisitions: 0,
            releases: 0,
            air: state("air")?,
            void_air: state("void_air")?,
            stone: state("stone")?,
            deepslate: state("deepslate")?,
            netherrack: state("netherrack")?,
        };
        ensure!(
            host.height > 0 && host.min % 16 == 0 && host.height % 16 == 0,
            "controlled fixture requires full sections"
        );
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let chunk = (origin.x.div_euclid(16) + dx, origin.z.div_euclid(16) + dz);
                let mut states = Vec::with_capacity(host.height as usize * 256);
                let mut heights = vec![host.min; 256];
                for y in host.min..host.min + host.height {
                    for z in 0..16 {
                        for x in 0..16 {
                            let value = host.initial(DensityPoint::new(
                                chunk.0 * 16 + x,
                                y,
                                chunk.1 * 16 + z,
                            ))?;
                            states.push(value);
                            if value != host.air {
                                heights[(z * 16 + x) as usize] = y + 1;
                            }
                        }
                    }
                }
                host.owners.insert(
                    chunk,
                    ChunkOwner {
                        states,
                        surface: heights.clone(),
                        floor: heights,
                    },
                );
            }
        }
        Ok(host)
    }
    fn initial(&self, p: DensityPoint) -> Result<u32> {
        if p.y > self.top {
            return Ok(self.air);
        }
        if self.profile == "air_plane" && p.x == self.origin.x + 1 {
            return Ok(self.air);
        }
        Ok(match self.profile.as_str() {
            "netherrack" => self.netherrack,
            "layered" | "air_plane" => {
                if p.y < 0 {
                    self.deepslate
                } else {
                    self.stone
                }
            }
            "stone" => self.stone,
            _ => bail!("unsupported controlled material profile"),
        })
    }
    fn record(&self, event: Value) {
        self.events.borrow_mut().push(event);
    }
    fn chunk(p: DensityPoint) -> (i32, i32) {
        (p.x.div_euclid(16), p.z.div_euclid(16))
    }
    fn index(&self, p: DensityPoint) -> usize {
        (p.y - self.min) as usize * 256
            + p.z.rem_euclid(16) as usize * 16
            + p.x.rem_euclid(16) as usize
    }
    fn owner(&self, chunk: (i32, i32)) -> Result<&ChunkOwner> {
        self.owners.get(&chunk).context("missing_neighbor")
    }
    fn read(&self, p: DensityPoint) -> Result<u32> {
        let owner = self.owner(Self::chunk(p))?;
        if p.y < self.min || p.y >= self.min + self.height {
            return Ok(self.void_air);
        }
        Ok(owner.states[self.index(p)])
    }
    fn write(&mut self, p: DensityPoint, value: u32) -> Result<()> {
        let chunk = Self::chunk(p);
        let index = self.index(p);
        self.owners
            .get_mut(&chunk)
            .context("missing_neighbor")?
            .states[index] = value;
        Ok(())
    }
    fn image(&self, file: &Path) -> Result<Value> {
        let mut bytes = Vec::new();
        let mut changed = Vec::new();
        let mut heightmaps = Vec::new();
        for (&chunk, owner) in &self.owners {
            for y in self.min..self.min + self.height {
                for z in 0..16 {
                    for x in 0..16 {
                        let p = DensityPoint::new(chunk.0 * 16 + x, y, chunk.1 * 16 + z);
                        let value = owner.states[self.index(p)];
                        bytes.extend_from_slice(&value.to_le_bytes());
                        if value != self.initial(p)? {
                            changed.push(json!([p.x, p.y, p.z, value]));
                        }
                    }
                }
            }
            heightmaps.push(json!({"chunk":[chunk.0,chunk.1],"world_surface_wg":owner.surface,"ocean_floor_wg":owner.floor}));
        }
        std::fs::write(file, bytes)?;
        Ok(
            json!({"image_file":file,"changed":changed,"heightmaps":heightmaps,
            "bulk_sessions_balanced":self.bulk.is_none()&&self.bulk_begins==self.bulk_closes,
            "bulk_begins":self.bulk_begins,"bulk_closes":self.bulk_closes,
            "section_locks_restored":self.bulk.is_none()&&self.acquisitions==self.releases,
            "section_acquisitions":self.acquisitions,"section_releases":self.releases}),
        )
    }
}
impl OreHost for ControlledRegion {
    fn min_build_height(&self) -> i32 {
        self.min
    }
    fn max_build_height(&self) -> i32 {
        self.min + self.height
    }
    fn outside_build_height(&self, y: i32) -> bool {
        let outside = y < self.min || y >= self.min + self.height;
        self.record(json!(["outside_y", y, outside]));
        outside
    }
    fn ensure_can_write(&mut self, p: DensityPoint) -> Result<bool> {
        let chunk = Self::chunk(p);
        let center = Self::chunk(self.origin);
        let value = (chunk.0 - center.0).abs() <= self.write_radius
            && (chunk.1 - center.1).abs() <= self.write_radius;
        self.record(json!(["ensure", point(p), value]));
        Ok(value)
    }
    fn ocean_floor_wg(&mut self, x: i32, z: i32) -> Result<i32> {
        let value = self.owner((x.div_euclid(16), z.div_euclid(16)))?.floor
            [(z.rem_euclid(16) * 16 + x.rem_euclid(16)) as usize];
        self.record(json!(["height", [x, z], value]));
        Ok(value)
    }
    fn begin_bulk_section_access(&mut self) -> Result<()> {
        ensure!(self.bulk.is_none(), "bulk owner already open");
        self.bulk = Some(BTreeSet::new());
        self.bulk_begins += 1;
        Ok(())
    }
    fn close_bulk_section_access(&mut self) -> Result<()> {
        let owners = self.bulk.take().context("bulk owner was not open")?;
        self.releases += owners.len();
        self.bulk_closes += 1;
        Ok(())
    }
    fn bulk_section_block(&mut self, p: DensityPoint) -> Result<Option<u32>> {
        let section = p.y.div_euclid(16);
        let key = (p.x.div_euclid(16), section, p.z.div_euclid(16));
        let owners = self.bulk.as_ref().context("bulk owner not open")?;
        if section < self.min.div_euclid(16) || section >= (self.min + self.height).div_euclid(16) {
            self.record(json!(["bulk_null_y", p.y]));
            return Ok(None);
        }
        if !owners.contains(&key) {
            self.record(json!(["chunk", [key.0, key.2]]));
            self.owner((key.0, key.2))?;
            self.bulk.as_mut().unwrap().insert(key);
            self.acquisitions += 1;
            self.record(json!(["acquire", [key.0, key.1, key.2]]));
        }
        let value = self.read(p)?;
        self.record(json!(["bulk_get", point(p), value]));
        Ok(Some(value))
    }
    fn direct_section_set(&mut self, p: DensityPoint, value: u32) -> Result<()> {
        let key = (p.x.div_euclid(16), p.y.div_euclid(16), p.z.div_euclid(16));
        ensure!(
            self.bulk
                .as_ref()
                .context("bulk owner not open")?
                .contains(&key),
            "write section was not acquired"
        );
        self.write(p, value)?;
        self.record(json!(["section_set", point(p), value, false]));
        Ok(())
    }
    fn block(&mut self, p: DensityPoint) -> Result<u32> {
        let value = self.read(p)?;
        self.record(json!(["world_get", point(p), value]));
        Ok(value)
    }
    fn set_block(&mut self, p: DensityPoint, value: u32, flags: i32) -> Result<bool> {
        let accepted = self.ensure_can_write(p)?;
        if accepted {
            self.owner(Self::chunk(p))?;
            if p.y >= self.min && p.y < self.min + self.height {
                let old = mc121_rust::lighting::state_properties(self.read(p)?)?;
                let new = mc121_rust::lighting::state_properties(value)?;
                ensure!(
                    old.is_air() == new.is_air() && old.blocks_motion() == new.blocks_motion(),
                    "controlled ore fixture does not model changed WG predicates"
                );
                self.write(p, value)?;
            }
        }
        // Actual EMPTY ProtoChunk writes update both WG heightmaps. This
        // closed solid-material/ore fixture verifies unchanged source predicates
        // above; its already primed arrays therefore retain their actual values.
        // Other predicate transitions fail instead of silently omitting updates.
        self.record(json!(["region_set", point(p), value, flags, accepted]));
        Ok(accepted)
    }
}
fn run(query: Value, output: &Path) -> Result<Value> {
    let mut host = ControlledRegion::new(query)?;
    let configured = feature::builtin_configured_ore(
        host.query["feature"].as_str().context("missing feature")?,
    )?;
    let seed = host.query["rng_seed"]
        .as_str()
        .context("missing caller seed")?
        .parse::<i64>()?;
    let mut random = WorldgenRandom::new(XoroshiroRandomSource::new(seed));
    let origin = host.origin;
    let result = feature::place_configured_ore(configured, &mut host, &mut random, origin);
    let (placed, error) = match result {
        Ok(v) => (Some(v), None),
        Err(e) if e.to_string() == "missing_neighbor" => (None, Some("missing_neighbor")),
        Err(e) => return Err(e),
    };
    let config = configured.configuration();
    let state = random.source().state();
    let kind = match configured {
        ConfiguredOreFeature::Ore(_) => "ecz",
        ConfiguredOreFeature::ScatteredOre(_) => "edh",
    };
    let image = host.image(&output.join(format!(
        "native-ore-{}.bin",
        host.query["id"].as_str().context("missing id")?
    )))?;
    let mut answer = json!({"query":host.query,"placed":placed,"error":error,"events":host.events.take(),"random_count":random.count(),
        "random_state128":[state.lo.to_string(),state.hi.to_string()],"config_size":config.size,
        "config_discard_bits":config.discard_chance_on_air_exposure.to_bits(),"feature_type":kind});
    answer
        .as_object_mut()
        .unwrap()
        .extend(image.as_object().unwrap().clone());
    Ok(answer)
}
fn configs() -> Result<Value> {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/world-generation-ore-configs.json"
    ))?;
    let mut rows = Vec::new();
    for (key, _) in fixture["configured_ores"]
        .as_object()
        .context("missing catalog")?
    {
        let feature = feature::builtin_configured_ore(key)?;
        let config = feature.configuration();
        let mut targets = Vec::new();
        for target in &config.targets {
            let mut matches = serde_json::Map::new();
            for material in [
                "air",
                "stone",
                "deepslate",
                "netherrack",
                "dirt",
                "gravel",
                "blackstone",
                "end_stone",
            ] {
                matches.insert(
                    format!("minecraft:{material}"),
                    json!(target.target.matches(state(material)?)?),
                );
            }
            let class = match target.target {
                OreRuleTest::BlockMatch(_) => "emv",
                OreRuleTest::TagMatch(_) => "env",
            };
            targets.push(json!({"state":target.state,"rule_class":class,"matches":matches}));
        }
        let kind = match feature {
            ConfiguredOreFeature::Ore(_) => "ecz",
            ConfiguredOreFeature::ScatteredOre(_) => "edh",
        };
        let path = format!(
            "data/minecraft/worldgen/configured_feature/{}.json",
            key.strip_prefix("minecraft:").context("nonbuiltin name")?
        );
        rows.push(json!({"key":key,"feature_type":kind,"size":config.size,"discard_bits":config.discard_chance_on_air_exposure.to_bits(),"targets":targets,"source_sha256":fixture["source_sha256"][path]}));
    }
    Ok(json!({"configured_ores":rows}))
}
fn main() -> Result<()> {
    let output = std::env::args()
        .nth(1)
        .context("usage: ore_probe <output-directory>")?;
    let output = Path::new(&output);
    std::fs::create_dir_all(output)?;
    for line in io::stdin().lock().lines() {
        let q: Value = serde_json::from_str(&line?)?;
        println!(
            "{}",
            if q["mode"] == "configs" {
                configs()?
            } else {
                run(q, output)?
            }
        );
    }
    Ok(())
}
