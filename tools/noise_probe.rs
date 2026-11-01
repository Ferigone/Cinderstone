//! Compile through tools/noise_check.py; no game runtime dependency on Java.
use anyhow::{anyhow, bail, Result};
use mc121_rust::{noise::*, random::*};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{self, BufRead},
};
struct Probe {
    randoms: HashMap<String, RandomSourceKind>,
    factories: HashMap<String, PositionalRandomFactory>,
    improved: HashMap<String, ImprovedNoise>,
    simplex: HashMap<String, SimplexNoise>,
    perlin: HashMap<String, PerlinNoise>,
    normal: HashMap<String, NormalNoise>,
    blended: HashMap<String, BlendedNoise>,
    perlin_simplex: HashMap<String, PerlinSimplexNoise>,
}
fn text<'a>(query: &'a Value, key: &str) -> Result<&'a str> {
    query[key]
        .as_str()
        .ok_or_else(|| anyhow!("missing string {key}"))
}
fn signed(query: &Value, key: &str) -> Result<i64> {
    query[key]
        .as_i64()
        .ok_or_else(|| anyhow!("missing integer {key}"))
}
fn integer(query: &Value, key: &str) -> Result<i32> {
    Ok(signed(query, key)? as i32)
}
fn floating(query: &Value, key: &str) -> Result<f64> {
    Ok(f64::from_bits(text(query, key)?.parse()?))
}
fn flag(query: &Value, key: &str) -> bool {
    query[key].as_bool().unwrap_or(false)
}
fn doubles(query: &Value, key: &str) -> Result<Vec<f64>> {
    query[key]
        .as_array()
        .ok_or_else(|| anyhow!("missing array"))?
        .iter()
        .map(|v| {
            Ok(f64::from_bits(
                v.as_str().ok_or_else(|| anyhow!("missing bits"))?.parse()?,
            ))
        })
        .collect()
}
fn units(query: &Value) -> Result<Vec<u16>> {
    query["units"]
        .as_array()
        .ok_or_else(|| anyhow!("missing units"))?
        .iter()
        .map(|v| Ok(v.as_u64().ok_or_else(|| anyhow!("invalid unit"))? as u16))
        .collect()
}
fn bits(value: f64) -> String {
    value.to_bits().to_string()
}
fn seed(seed: Seed128) -> Value {
    json!([seed.lo.to_string(), seed.hi.to_string()])
}
fn improved(value: &ImprovedNoise) -> Value {
    json!({"offsets":[bits(value.xo),bits(value.yo),bits(value.zo)],"permutation":value.permutation().iter().map(|v|format!("{v:02x}")).collect::<String>()})
}
fn simplex(value: &SimplexNoise) -> Value {
    json!({"offsets":[bits(value.xo),bits(value.yo),bits(value.zo)],"permutation":value.permutation().iter().map(|v|format!("{v:02x}")).collect::<String>()})
}
impl Probe {
    fn new() -> Self {
        Self {
            randoms: HashMap::new(),
            factories: HashMap::new(),
            improved: HashMap::new(),
            simplex: HashMap::new(),
            perlin: HashMap::new(),
            normal: HashMap::new(),
            blended: HashMap::new(),
            perlin_simplex: HashMap::new(),
        }
    }
    fn random(&mut self, q: &Value) -> Result<&mut RandomSourceKind> {
        self.randoms
            .get_mut(text(q, "source")?)
            .ok_or_else(|| anyhow!("missing random"))
    }
    fn run(&mut self, q: &Value) -> Result<Value> {
        match text(q, "op")? {
            "rng_new" => {
                let source = match text(q, "kind")? {
                    "legacy" => {
                        RandomSourceKind::Legacy(LegacyRandomSource::new(signed(q, "seed")?))
                    }
                    "xoro" => RandomSourceKind::Xoroshiro(if q.get("lo").is_some() {
                        XoroshiroRandomSource::from_state(
                            signed(q, "lo")? as u64,
                            signed(q, "hi")? as u64,
                        )
                    } else {
                        XoroshiroRandomSource::new(signed(q, "seed")?)
                    }),
                    _ => bail!("unknown kind"),
                };
                self.randoms.insert(text(q, "id")?.into(), source);
                Ok(Value::Null)
            }
            "rng_seed" => {
                match self.random(q)? {
                    RandomSourceKind::Legacy(r) => r.set_seed(signed(q, "seed")?),
                    RandomSourceKind::Xoroshiro(r) => r.set_seed(signed(q, "seed")?),
                };
                Ok(Value::Null)
            }
            "rng_consume" => {
                self.random(q)?.consume_count(integer(q, "count")?);
                Ok(Value::Null)
            }
            "rng_fork" => {
                let source = match self.random(q)? {
                    RandomSourceKind::Legacy(r) => RandomSourceKind::Legacy(r.fork()),
                    RandomSourceKind::Xoroshiro(r) => RandomSourceKind::Xoroshiro(r.fork()),
                };
                self.randoms.insert(text(q, "id")?.into(), source);
                Ok(Value::Null)
            }
            "rng_factory" => {
                let factory = self.random(q)?.fork_positional();
                self.factories.insert(text(q, "id")?.into(), factory);
                Ok(Value::Null)
            }
            "rng_sample" => {
                let source = self.random(q)?;
                Ok(match text(q, "method")? {
                    "i32" => json!(source.next_i32().to_string()),
                    "i64" => json!(source.next_i64().to_string()),
                    "bool" => json!(source.next_bool()),
                    "f32" => json!(source.next_f32().to_bits().to_string()),
                    "f64" => json!(bits(source.next_f64())),
                    "bounded" => match source.next_bounded_i32(integer(q, "bound")?) {
                        Ok(value) => json!(value.to_string()),
                        Err(_) => json!({"error":"Bound must be positive"}),
                    },
                    _ => bail!("unknown sample"),
                })
            }
            "rng_state" => Ok(match self.random(q)? {
                RandomSourceKind::Legacy(r) => json!(r.state48().to_string()),
                RandomSourceKind::Xoroshiro(r) => seed(r.state()),
            }),
            "factory_new" => {
                let factory = match text(q, "kind")? {
                    "legacy" => PositionalRandomFactory::Legacy(LegacyPositionalFactory::new(
                        signed(q, "seed")?,
                    )),
                    "xoro" => PositionalRandomFactory::Xoroshiro(XoroshiroPositionalFactory::new(
                        signed(q, "lo")? as u64,
                        signed(q, "hi")? as u64,
                    )),
                    _ => bail!("unknown kind"),
                };
                self.factories.insert(text(q, "id")?.into(), factory);
                Ok(Value::Null)
            }
            "factory_at" | "factory_hash" | "factory_seed" => {
                let factory = self
                    .factories
                    .get(text(q, "factory")?)
                    .ok_or_else(|| anyhow!("missing factory"))?;
                let random = match text(q, "op")? {
                    "factory_at" => {
                        factory.at(integer(q, "x")?, integer(q, "y")?, integer(q, "z")?)
                    }
                    "factory_hash" => factory.from_hash_of_utf16(&units(q)?),
                    _ => factory.from_seed(signed(q, "seed")?),
                };
                self.randoms.insert(text(q, "id")?.into(), random);
                Ok(Value::Null)
            }
            "mix" => Ok(json!(mix_stafford13(signed(q, "seed")? as u64).to_string())),
            "upgrade" => Ok(seed(if flag(q, "mixed") {
                upgrade_seed(signed(q, "seed")?)
            } else {
                upgrade_seed_unmixed(signed(q, "seed")?)
            })),
            "hash" => Ok(seed(seed_from_hash_of_utf16(&units(q)?))),
            "coord" => Ok(json!(coordinate_seed(
                integer(q, "x")?,
                integer(q, "y")?,
                integer(q, "z")?
            )
            .to_string())),
            "improved_new" => {
                let value = ImprovedNoise::new(self.random(q)?);
                let result = improved(&value);
                self.improved.insert(text(q, "id")?.into(), value);
                Ok(result)
            }
            "simplex_new" => {
                let value = SimplexNoise::new(self.random(q)?);
                let result = simplex(&value);
                self.simplex.insert(text(q, "id")?.into(), value);
                Ok(result)
            }
            "perlin_new" => {
                let amps = doubles(q, "amplitudes")?;
                let first = integer(q, "first")?;
                let value = if flag(q, "legacy") {
                    PerlinNoise::legacy(self.random(q)?, first, amps)
                        .map_err(|error| anyhow!(error.to_string()))?
                } else {
                    PerlinNoise::new(self.random(q)?, first, amps)
                };
                let levels = (0..value.amplitudes().len())
                    .rev()
                    .map(|i| value.get_octave_noise(i).map(improved))
                    .collect::<Vec<_>>();
                let result = json!({"levels":levels,"max":bits(value.max_value())});
                self.perlin.insert(text(q, "id")?.into(), value);
                Ok(result)
            }
            "normal_new" => {
                let parameters = NoiseParameters {
                    first_octave: integer(q, "first")?,
                    amplitudes: doubles(q, "amplitudes")?,
                };
                let value = if flag(q, "legacy") {
                    NormalNoise::legacy(self.random(q)?, parameters)
                        .map_err(|error| anyhow!(error.to_string()))?
                } else {
                    NormalNoise::new(self.random(q)?, parameters)
                };
                let result = json!(bits(value.max_value()));
                self.normal.insert(text(q, "id")?.into(), value);
                Ok(result)
            }
            "improved" => Ok(json!(bits(
                self.improved
                    .get(text(q, "id")?)
                    .ok_or_else(|| anyhow!("missing improved"))?
                    .noise_scaled(
                        floating(q, "x")?,
                        floating(q, "y")?,
                        floating(q, "z")?,
                        floating(q, "scale")?,
                        floating(q, "ymax")?
                    )
            ))),
            "derivative" => {
                let mut d: [f64; 3] = doubles(q, "initial")?
                    .try_into()
                    .map_err(|_| anyhow!("derivative needs three values"))?;
                let value = self
                    .improved
                    .get(text(q, "id")?)
                    .ok_or_else(|| anyhow!("missing improved"))?
                    .noise_with_derivative(
                        floating(q, "x")?,
                        floating(q, "y")?,
                        floating(q, "z")?,
                        &mut d,
                    );
                Ok(json!([bits(value), bits(d[0]), bits(d[1]), bits(d[2])]))
            }
            "simplex2" => Ok(json!(bits(
                self.simplex
                    .get(text(q, "id")?)
                    .ok_or_else(|| anyhow!("missing simplex"))?
                    .noise_2d(floating(q, "x")?, floating(q, "y")?)
            ))),
            "simplex3" => Ok(json!(bits(
                self.simplex
                    .get(text(q, "id")?)
                    .ok_or_else(|| anyhow!("missing simplex"))?
                    .noise_3d(floating(q, "x")?, floating(q, "y")?, floating(q, "z")?)
            ))),
            "perlin" => Ok(json!(bits(
                self.perlin
                    .get(text(q, "id")?)
                    .ok_or_else(|| anyhow!("missing perlin"))?
                    .noise_scaled(
                        floating(q, "x")?,
                        floating(q, "y")?,
                        floating(q, "z")?,
                        floating(q, "scale")?,
                        floating(q, "ymax")?,
                        flag(q, "origin")
                    )
            ))),
            "normal" => Ok(json!(bits(
                self.normal
                    .get(text(q, "id")?)
                    .ok_or_else(|| anyhow!("missing normal"))?
                    .noise(floating(q, "x")?, floating(q, "y")?, floating(q, "z")?)
            ))),
            "blended_new" => {
                let value = BlendedNoise::new(
                    self.random(q)?,
                    floating(q, "xz_scale")?,
                    floating(q, "y_scale")?,
                    floating(q, "xz_factor")?,
                    floating(q, "y_factor")?,
                    floating(q, "smear")?,
                );
                let result = json!([bits(value.min_value()), bits(value.max_value())]);
                self.blended.insert(text(q, "id")?.into(), value);
                Ok(result)
            }
            "blended" => Ok(json!(bits(
                self.blended
                    .get(text(q, "id")?)
                    .ok_or_else(|| anyhow!("missing BlendedNoise"))?
                    .compute(integer(q, "x")?, integer(q, "y")?, integer(q, "z")?)
            ))),
            "perlinsimplex_new" => {
                let octaves = q["octaves"]
                    .as_array()
                    .ok_or_else(|| anyhow!("missing octaves"))?
                    .iter()
                    .map(|v| {
                        v.as_i64()
                            .map(|n| n as i32)
                            .ok_or_else(|| anyhow!("invalid octave"))
                    })
                    .collect::<Result<Vec<_>>>()?;
                let value = PerlinSimplexNoise::new(self.random(q)?, octaves)
                    .map_err(|error| anyhow!(error.to_string()))?;
                let levels = value
                    .levels()
                    .iter()
                    .map(|level| level.as_ref().map(simplex))
                    .collect::<Vec<_>>();
                self.perlin_simplex.insert(text(q, "id")?.into(), value);
                Ok(json!(levels))
            }
            "perlinsimplex" => Ok(json!(bits(
                self.perlin_simplex
                    .get(text(q, "id")?)
                    .ok_or_else(|| anyhow!("missing PerlinSimplex"))?
                    .noise(floating(q, "x")?, floating(q, "y")?, flag(q, "origin"))
            ))),
            "worldgen" => {
                let source = match text(q, "kind")? {
                    "legacy" => {
                        RandomSourceKind::Legacy(LegacyRandomSource::new(signed(q, "seed")?))
                    }
                    "xoro" => {
                        RandomSourceKind::Xoroshiro(XoroshiroRandomSource::new(signed(q, "seed")?))
                    }
                    _ => bail!("unknown source"),
                };
                let mut value = WorldgenRandom::new(source);
                let seed = signed(q, "seed")?;
                let x = integer(q, "x")?;
                let z = integer(q, "z")?;
                let mut mixed = None;
                match text(q, "method")? {
                    "decoration" => mixed = Some(value.set_decoration_seed(seed, x, z).to_string()),
                    "feature" => value.set_feature_seed(seed, x, z),
                    "large" => value.set_large_feature_seed(seed, x, z),
                    "salt" => value.set_large_feature_with_salt(seed, x, z, integer(q, "salt")?),
                    "plain" => {}
                    _ => bail!("unknown worldgen"),
                };
                let mut samples = Vec::new();
                for _ in 0..4 {
                    samples.push(json!(value.next_i32().to_string()));
                    samples.push(json!(value.next_i64().to_string()));
                    samples.push(json!(value.next_bool()));
                    samples.push(json!(value.next_f32().to_bits().to_string()));
                    samples.push(json!(bits(value.next_f64())));
                    samples.push(json!(value.next_bounded_i32(1073741825)?.to_string()));
                }
                Ok(json!({"seed":mixed,"samples":samples,"count":value.count()}))
            }
            "slime" => {
                let mut random = seed_slime_chunk(
                    integer(q, "x")?,
                    integer(q, "z")?,
                    signed(q, "seed")?,
                    signed(q, "salt")?,
                );
                Ok(json!([
                    random.next_i64().to_string(),
                    random.next_bounded_i32(10)?.to_string()
                ]))
            }
            "wrap" => Ok(json!(bits(PerlinNoise::wrap(floating(q, "value")?)))),
            _ => bail!("unknown operation"),
        }
    }
}
fn main() -> Result<()> {
    let mut probe = Probe::new();
    for line in io::stdin().lock().lines() {
        let query: Value = serde_json::from_str(&line?)?;
        let output = match probe.run(&query) {
            Ok(value) => value,
            Err(error) => json!({"error":error.to_string()}),
        };
        println!("{}", serde_json::to_string(&output)?);
    }
    Ok(())
}
