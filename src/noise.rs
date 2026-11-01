//! Native Minecraft 1.21 world-generation noise primitives.
//!
//! Arithmetic order follows the pinned eof/eoi/eoh/eok bytecode. The probe
//! compares raw f64 bits against the actual classes, including derivatives and
//! constructor random consumption. This module is not the biome/density router,
//! terrain surface builder, carvers, structures, or chunk generator.
use crate::random::RandomSource;
use anyhow::{bail, Result};
use std::collections::BTreeSet;

const GRADIENT: [[i32; 3]; 16] = [
    [1, 1, 0],
    [-1, 1, 0],
    [1, -1, 0],
    [-1, -1, 0],
    [1, 0, 1],
    [-1, 0, 1],
    [1, 0, -1],
    [-1, 0, -1],
    [0, 1, 1],
    [0, -1, 1],
    [0, 1, -1],
    [0, -1, -1],
    [1, 1, 0],
    [0, -1, 1],
    [-1, 1, 0],
    [0, -1, -1],
];
fn floor_i32(value: f64) -> i32 {
    let integer = value as i32;
    if value < f64::from(integer) {
        integer.wrapping_sub(1)
    } else {
        integer
    }
}
fn floor_i64(value: f64) -> i64 {
    let integer = value as i64;
    if value < integer as f64 {
        integer.wrapping_sub(1)
    } else {
        integer
    }
}
fn smoothstep(value: f64) -> f64 {
    value * value * value * (value * (value * 6.0 - 15.0) + 10.0)
}
fn smoothstep_derivative(value: f64) -> f64 {
    30.0 * value * value * (value - 1.0) * (value - 1.0)
}
fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}
fn lerp2(x: f64, y: f64, c: [f64; 4]) -> f64 {
    lerp(y, lerp(x, c[0], c[1]), lerp(x, c[2], c[3]))
}
fn lerp3(x: f64, y: f64, z: f64, c: [f64; 8]) -> f64 {
    lerp(
        z,
        lerp2(x, y, [c[0], c[1], c[2], c[3]]),
        lerp2(x, y, [c[4], c[5], c[6], c[7]]),
    )
}
fn dot(gradient: [i32; 3], x: f64, y: f64, z: f64) -> f64 {
    f64::from(gradient[0]) * x + f64::from(gradient[1]) * y + f64::from(gradient[2]) * z
}
fn permutation(random: &mut impl RandomSource) -> [u8; 256] {
    let mut p = std::array::from_fn(|i| i as u8);
    for i in 0..256 {
        let offset = random
            .next_bounded_i32((256 - i) as i32)
            .expect("positive shuffle bound") as usize;
        p.swap(i, i + offset);
    }
    p
}
#[derive(Debug, Clone)]
pub struct ImprovedNoise {
    pub xo: f64,
    pub yo: f64,
    pub zo: f64,
    permutation: [u8; 256],
}
impl ImprovedNoise {
    pub fn new(random: &mut impl RandomSource) -> Self {
        let xo = random.next_f64() * 256.0;
        let yo = random.next_f64() * 256.0;
        let zo = random.next_f64() * 256.0;
        Self {
            xo,
            yo,
            zo,
            permutation: permutation(random),
        }
    }
    pub fn permutation(&self) -> &[u8; 256] {
        &self.permutation
    }
    fn p(&self, index: i32) -> i32 {
        i32::from(self.permutation[(index & 255) as usize])
    }
    fn gradients(&self, x: i32, y: i32, z: i32) -> [[i32; 3]; 8] {
        let px = self.p(x);
        let px1 = self.p(x.wrapping_add(1));
        let a = self.p(px.wrapping_add(y));
        let b = self.p(px1.wrapping_add(y));
        let c = self.p(px.wrapping_add(y).wrapping_add(1));
        let d = self.p(px1.wrapping_add(y).wrapping_add(1));
        let indices = [a, b, c, d, a, b, c, d];
        std::array::from_fn(|i| {
            GRADIENT
                [(self.p(indices[i].wrapping_add(z).wrapping_add(i32::from(i >= 4))) & 15) as usize]
        })
    }
    pub fn noise(&self, x: f64, y: f64, z: f64) -> f64 {
        self.noise_scaled(x, y, z, 0.0, 0.0)
    }
    pub fn noise_scaled(&self, x: f64, y: f64, z: f64, y_scale: f64, y_max: f64) -> f64 {
        let x = x + self.xo;
        let y = y + self.yo;
        let z = z + self.zo;
        let ix = floor_i32(x);
        let iy = floor_i32(y);
        let iz = floor_i32(z);
        let fx = x - f64::from(ix);
        let fy = y - f64::from(iy);
        let fz = z - f64::from(iz);
        let shift = if y_scale != 0.0 {
            let capped = if y_max >= 0.0 && y_max < fy {
                y_max
            } else {
                fy
            };
            f64::from(floor_i32(capped / y_scale + f64::from(1.0e-7_f32))) * y_scale
        } else {
            0.0
        };
        self.sample(ix, iy, iz, fx, fy - shift, fz, fy)
    }
    #[allow(clippy::too_many_arguments)]
    fn sample(&self, x: i32, y: i32, z: i32, fx: f64, fy: f64, fz: f64, original_y: f64) -> f64 {
        let gradients = self.gradients(x, y, z);
        let corners = std::array::from_fn(|i| {
            dot(
                gradients[i],
                fx - f64::from(i32::from((i & 1) != 0)),
                fy - f64::from(i32::from((i & 2) != 0)),
                fz - f64::from(i32::from((i & 4) != 0)),
            )
        });
        lerp3(
            smoothstep(fx),
            smoothstep(original_y),
            smoothstep(fz),
            corners,
        )
    }
    /// Adds the derivative contribution to the supplied array, as vanilla does.
    pub fn noise_with_derivative(&self, x: f64, y: f64, z: f64, derivative: &mut [f64; 3]) -> f64 {
        let x = x + self.xo;
        let y = y + self.yo;
        let z = z + self.zo;
        let ix = floor_i32(x);
        let iy = floor_i32(y);
        let iz = floor_i32(z);
        let fx = x - f64::from(ix);
        let fy = y - f64::from(iy);
        let fz = z - f64::from(iz);
        let gradients = self.gradients(ix, iy, iz);
        let c: [f64; 8] = std::array::from_fn(|i| {
            dot(
                gradients[i],
                fx - f64::from(i32::from((i & 1) != 0)),
                fy - f64::from(i32::from((i & 2) != 0)),
                fz - f64::from(i32::from((i & 4) != 0)),
            )
        });
        let u = smoothstep(fx);
        let v = smoothstep(fy);
        let w = smoothstep(fz);
        let gx = lerp3(u, v, w, gradients.map(|g| f64::from(g[0])));
        let gy = lerp3(u, v, w, gradients.map(|g| f64::from(g[1])));
        let gz = lerp3(u, v, w, gradients.map(|g| f64::from(g[2])));
        let dx = lerp2(v, w, [c[1] - c[0], c[3] - c[2], c[5] - c[4], c[7] - c[6]]);
        let dy = lerp2(w, u, [c[2] - c[0], c[6] - c[4], c[3] - c[1], c[7] - c[5]]);
        let dz = lerp2(u, v, [c[4] - c[0], c[5] - c[1], c[6] - c[2], c[7] - c[3]]);
        derivative[0] += gx + smoothstep_derivative(fx) * dx;
        derivative[1] += gy + smoothstep_derivative(fy) * dy;
        derivative[2] += gz + smoothstep_derivative(fz) * dz;
        lerp3(u, v, w, c)
    }
}
#[derive(Debug, Clone)]
pub struct SimplexNoise {
    pub xo: f64,
    pub yo: f64,
    pub zo: f64,
    permutation: [u8; 256],
}
impl SimplexNoise {
    pub fn new(random: &mut impl RandomSource) -> Self {
        let xo = random.next_f64() * 256.0;
        let yo = random.next_f64() * 256.0;
        let zo = random.next_f64() * 256.0;
        Self {
            xo,
            yo,
            zo,
            permutation: permutation(random),
        }
    }
    pub fn permutation(&self) -> &[u8; 256] {
        &self.permutation
    }
    fn p(&self, index: i32) -> i32 {
        i32::from(self.permutation[(index & 255) as usize])
    }
    fn corner(gradient: usize, x: f64, y: f64, z: f64, radius: f64) -> f64 {
        let mut attenuation = radius - x * x - y * y - z * z;
        if attenuation < 0.0 {
            0.0
        } else {
            attenuation *= attenuation;
            attenuation * attenuation * dot(GRADIENT[gradient], x, y, z)
        }
    }
    pub fn noise_2d(&self, x: f64, y: f64) -> f64 {
        // Stored as exact IEEE results of Java's class-initializer sqrt(3).
        const F2: f64 = 0.3660254037844386;
        const G2: f64 = 0.21132486540518713;
        let skew = (x + y) * F2;
        let i = floor_i32(x + skew);
        let j = floor_i32(y + skew);
        let unskew = f64::from(i.wrapping_add(j)) * G2;
        let fx = x - (f64::from(i) - unskew);
        let fy = y - (f64::from(j) - unskew);
        let (i1, j1) = if fx > fy { (1, 0) } else { (0, 1) };
        let x1 = fx - f64::from(i1) + G2;
        let y1 = fy - f64::from(j1) + G2;
        let x2 = fx - 1.0 + 2.0 * G2;
        let y2 = fy - 1.0 + 2.0 * G2;
        let i = i & 255;
        let j = j & 255;
        let g0 = (self.p(i + self.p(j)) % 12) as usize;
        let g1 = (self.p(i + i1 + self.p(j + j1)) % 12) as usize;
        let g2 = (self.p(i + 1 + self.p(j + 1)) % 12) as usize;
        70.0 * (Self::corner(g0, fx, fy, 0.0, 0.5)
            + Self::corner(g1, x1, y1, 0.0, 0.5)
            + Self::corner(g2, x2, y2, 0.0, 0.5))
    }
    pub fn noise_3d(&self, x: f64, y: f64, z: f64) -> f64 {
        const F3: f64 = 1.0 / 3.0;
        const G3: f64 = 1.0 / 6.0;
        let skew = (x + y + z) * F3;
        let i = floor_i32(x + skew);
        let j = floor_i32(y + skew);
        let k = floor_i32(z + skew);
        let unskew = f64::from(i.wrapping_add(j).wrapping_add(k)) * G3;
        let fx = x - (f64::from(i) - unskew);
        let fy = y - (f64::from(j) - unskew);
        let fz = z - (f64::from(k) - unskew);
        let (a, b) = if fx >= fy {
            if fy >= fz {
                ([1, 0, 0], [1, 1, 0])
            } else if fx >= fz {
                ([1, 0, 0], [1, 0, 1])
            } else {
                ([0, 0, 1], [1, 0, 1])
            }
        } else if fy < fz {
            ([0, 0, 1], [0, 1, 1])
        } else if fx < fz {
            ([0, 1, 0], [0, 1, 1])
        } else {
            ([0, 1, 0], [1, 1, 0])
        };
        let x1 = fx - f64::from(a[0]) + G3;
        let y1 = fy - f64::from(a[1]) + G3;
        let z1 = fz - f64::from(a[2]) + G3;
        let x2 = fx - f64::from(b[0]) + F3;
        let y2 = fy - f64::from(b[1]) + F3;
        let z2 = fz - f64::from(b[2]) + F3;
        let x3 = fx - 1.0 + 0.5;
        let y3 = fy - 1.0 + 0.5;
        let z3 = fz - 1.0 + 0.5;
        let i = i & 255;
        let j = j & 255;
        let k = k & 255;
        let g0 = (self.p(i + self.p(j + self.p(k))) % 12) as usize;
        let g1 = (self.p(i + a[0] + self.p(j + a[1] + self.p(k + a[2]))) % 12) as usize;
        let g2 = (self.p(i + b[0] + self.p(j + b[1] + self.p(k + b[2]))) % 12) as usize;
        let g3 = (self.p(i + 1 + self.p(j + 1 + self.p(k + 1))) % 12) as usize;
        32.0 * (Self::corner(g0, fx, fy, fz, 0.6)
            + Self::corner(g1, x1, y1, z1, 0.6)
            + Self::corner(g2, x2, y2, z2, 0.6)
            + Self::corner(g3, x3, y3, z3, 0.6))
    }
}

#[derive(Debug, Clone)]
pub struct PerlinNoise {
    levels: Vec<Option<ImprovedNoise>>,
    first_octave: i32,
    amplitudes: Vec<f64>,
    lowest_frequency_value_factor: f64,
    lowest_frequency_input_factor: f64,
    max_value: f64,
}
impl PerlinNoise {
    pub fn new(random: &mut impl RandomSource, first_octave: i32, amplitudes: Vec<f64>) -> Self {
        let factory = random.fork_positional();
        let levels = amplitudes
            .iter()
            .enumerate()
            .map(|(i, amplitude)| {
                if *amplitude != 0.0 {
                    Some(ImprovedNoise::new(&mut factory.from_hash_of(&format!(
                        "octave_{}",
                        first_octave.wrapping_add(i as i32)
                    ))))
                } else {
                    None
                }
            })
            .collect();
        Self::finish(levels, first_octave, amplitudes)
    }
    pub fn legacy(
        random: &mut impl RandomSource,
        first_octave: i32,
        amplitudes: Vec<f64>,
    ) -> Result<Self> {
        let len = amplitudes.len();
        let zero = first_octave.wrapping_neg();
        let mut levels = vec![None; len];
        let noise = ImprovedNoise::new(random);
        if zero >= 0 && (zero as usize) < len && amplitudes[zero as usize] != 0.0 {
            levels[zero as usize] = Some(noise);
        }
        for i in (0..zero).rev() {
            if (i as usize) < len && amplitudes[i as usize] != 0.0 {
                levels[i as usize] = Some(ImprovedNoise::new(random));
            } else {
                random.consume_count(262);
            }
        }
        if levels.iter().filter(|value| value.is_some()).count()
            != amplitudes.iter().filter(|value| **value != 0.0).count()
        {
            bail!("Failed to create correct number of noise levels for given non-zero amplitudes");
        }
        if i64::from(zero) < len as i64 - 1 {
            bail!("Positive octaves are temporarily disabled");
        }
        Ok(Self::finish(levels, first_octave, amplitudes))
    }
    pub fn from_octaves(
        random: &mut impl RandomSource,
        octaves: impl IntoIterator<Item = i32>,
    ) -> Result<Self> {
        let (first, amplitudes) = Self::make_amplitudes(octaves)?;
        Ok(Self::new(random, first, amplitudes))
    }
    pub fn legacy_from_octaves(
        random: &mut impl RandomSource,
        octaves: impl IntoIterator<Item = i32>,
    ) -> Result<Self> {
        let (first, amplitudes) = Self::make_amplitudes(octaves)?;
        Self::legacy(random, first, amplitudes)
    }
    pub fn make_amplitudes(octaves: impl IntoIterator<Item = i32>) -> Result<(i32, Vec<f64>)> {
        let octaves: BTreeSet<_> = octaves.into_iter().collect();
        let Some(first) = octaves.first().copied() else {
            bail!("Need some octaves!");
        };
        let last = *octaves.last().expect("nonempty octaves");
        let size = last.wrapping_sub(first).wrapping_add(1);
        if size < 1 {
            bail!("Total number of octaves needs to be >= 1");
        }
        let mut amplitudes = vec![0.0; size as usize];
        for octave in octaves {
            amplitudes[octave.wrapping_sub(first) as usize] = 1.0;
        }
        Ok((first, amplitudes))
    }
    fn finish(levels: Vec<Option<ImprovedNoise>>, first_octave: i32, amplitudes: Vec<f64>) -> Self {
        let len = amplitudes.len() as i32;
        let mut result = Self {
            levels,
            first_octave,
            amplitudes,
            lowest_frequency_input_factor: 2.0_f64.powi(first_octave),
            lowest_frequency_value_factor: 2.0_f64.powi(len.wrapping_sub(1))
                / (2.0_f64.powi(len) - 1.0),
            max_value: 0.0,
        };
        result.max_value = result.edge_value(2.0);
        result
    }
    pub fn first_octave(&self) -> i32 {
        self.first_octave
    }
    pub fn amplitudes(&self) -> &[f64] {
        &self.amplitudes
    }
    pub fn max_value(&self) -> f64 {
        self.max_value
    }
    pub fn get_octave_noise(&self, octave: usize) -> Option<&ImprovedNoise> {
        self.levels
            .get(self.levels.len().checked_sub(1 + octave)?)
            .and_then(Option::as_ref)
    }
    pub fn noise(&self, x: f64, y: f64, z: f64) -> f64 {
        self.noise_scaled(x, y, z, 0.0, 0.0, false)
    }
    pub fn noise_scaled(
        &self,
        x: f64,
        y: f64,
        z: f64,
        y_scale: f64,
        y_max: f64,
        use_origin: bool,
    ) -> f64 {
        let mut value = 0.0;
        let mut input = self.lowest_frequency_input_factor;
        let mut output = self.lowest_frequency_value_factor;
        for (i, level) in self.levels.iter().enumerate() {
            if let Some(level) = level {
                let sample = level.noise_scaled(
                    Self::wrap(x * input),
                    if use_origin {
                        -level.yo
                    } else {
                        Self::wrap(y * input)
                    },
                    Self::wrap(z * input),
                    y_scale * input,
                    y_max * input,
                );
                value += self.amplitudes[i] * sample * output;
            }
            input *= 2.0;
            output /= 2.0;
        }
        value
    }
    pub fn wrap(value: f64) -> f64 {
        value - (floor_i64(value / 33_554_432.0 + 0.5) as f64) * 33_554_432.0
    }
    fn edge_value(&self, value: f64) -> f64 {
        let mut result = 0.0;
        let mut factor = self.lowest_frequency_value_factor;
        for (i, level) in self.levels.iter().enumerate() {
            if level.is_some() {
                result += self.amplitudes[i] * value * factor;
            }
            factor /= 2.0;
        }
        result
    }
    pub fn max_broken_value(&self, value: f64) -> f64 {
        self.edge_value(value + 2.0)
    }
}
/// Legacy octave simplex sum used by biome/climate paths, including the
/// positive-octave reseed from the origin simplex sample.
#[derive(Debug, Clone)]
pub struct PerlinSimplexNoise {
    levels: Vec<Option<SimplexNoise>>,
    highest_frequency_value_factor: f64,
    highest_frequency_input_factor: f64,
}
impl PerlinSimplexNoise {
    pub fn new(
        random: &mut impl RandomSource,
        octaves: impl IntoIterator<Item = i32>,
    ) -> Result<Self> {
        let octaves: BTreeSet<_> = octaves.into_iter().collect();
        let Some(first) = octaves.first().copied() else {
            bail!("Need some octaves!");
        };
        let last = *octaves.last().expect("nonempty octaves");
        let count = last.wrapping_sub(first).wrapping_add(1);
        if count < 1 {
            bail!("Total number of octaves needs to be >= 1");
        }
        let mut levels = vec![None; count as usize];
        let base = SimplexNoise::new(random);
        for i in last.wrapping_add(1)..count {
            if i >= 0 && octaves.contains(&last.wrapping_sub(i)) {
                levels[i as usize] = Some(SimplexNoise::new(random));
            } else {
                random.consume_count(262);
            }
        }
        if last > 0 {
            let seed = (base.noise_3d(base.xo, base.yo, base.zo) * 9.223_372_036_854_776e18) as i64;
            // WorldgenRandom<LegacyRandomSource> delegates the identical bit
            // samples; its discarded count has no effect on these levels.
            let mut positive =
                crate::random::WorldgenRandom::new(crate::random::LegacyRandomSource::new(seed));
            for i in (0..last).rev() {
                if i < count && octaves.contains(&last.wrapping_sub(i)) {
                    levels[i as usize] = Some(SimplexNoise::new(&mut positive));
                } else {
                    positive.consume_count(262);
                }
            }
        }
        if last >= 0 && last < count && octaves.contains(&0) {
            levels[last as usize] = Some(base);
        }
        Ok(Self {
            levels,
            highest_frequency_input_factor: 2.0_f64.powi(last),
            highest_frequency_value_factor: 1.0 / (2.0_f64.powi(count) - 1.0),
        })
    }
    pub fn levels(&self) -> &[Option<SimplexNoise>] {
        &self.levels
    }
    pub fn noise(&self, x: f64, y: f64, use_origin: bool) -> f64 {
        let mut result = 0.0;
        let mut input = self.highest_frequency_input_factor;
        let mut output = self.highest_frequency_value_factor;
        for level in &self.levels {
            if let Some(level) = level {
                result += level.noise_2d(
                    x * input + if use_origin { level.xo } else { 0.0 },
                    y * input + if use_origin { level.yo } else { 0.0 },
                ) * output;
            }
            input /= 2.0;
            output *= 2.0;
        }
        result
    }
}

#[derive(Debug, Clone)]
pub struct NoiseParameters {
    pub first_octave: i32,
    pub amplitudes: Vec<f64>,
}
#[derive(Debug, Clone)]
pub struct NormalNoise {
    first: PerlinNoise,
    second: PerlinNoise,
    value_factor: f64,
    max_value: f64,
    parameters: NoiseParameters,
}
impl NormalNoise {
    pub fn new(random: &mut impl RandomSource, parameters: NoiseParameters) -> Self {
        let first = PerlinNoise::new(
            random,
            parameters.first_octave,
            parameters.amplitudes.clone(),
        );
        let second = PerlinNoise::new(
            random,
            parameters.first_octave,
            parameters.amplitudes.clone(),
        );
        Self::finish(first, second, parameters)
    }
    pub fn legacy(random: &mut impl RandomSource, parameters: NoiseParameters) -> Result<Self> {
        let first = PerlinNoise::legacy(
            random,
            parameters.first_octave,
            parameters.amplitudes.clone(),
        )?;
        let second = PerlinNoise::legacy(
            random,
            parameters.first_octave,
            parameters.amplitudes.clone(),
        )?;
        Ok(Self::finish(first, second, parameters))
    }
    fn finish(first: PerlinNoise, second: PerlinNoise, parameters: NoiseParameters) -> Self {
        let mut min = i32::MAX;
        let mut max = i32::MIN;
        for (i, amplitude) in parameters.amplitudes.iter().enumerate() {
            if *amplitude != 0.0 {
                min = min.min(i as i32);
                max = max.max(i as i32);
            }
        }
        let expected = 0.1 * (1.0 + 1.0 / f64::from(max.wrapping_sub(min).wrapping_add(1)));
        let value_factor = (1.0 / 6.0) / expected;
        let max_value = (first.max_value() + second.max_value()) * value_factor;
        Self {
            first,
            second,
            value_factor,
            max_value,
            parameters,
        }
    }
    pub fn noise(&self, x: f64, y: f64, z: f64) -> f64 {
        const INPUT_FACTOR: f64 = 1.0181268882175227;
        (self.first.noise(x, y, z)
            + self
                .second
                .noise(x * INPUT_FACTOR, y * INPUT_FACTOR, z * INPUT_FACTOR))
            * self.value_factor
    }
    pub fn max_value(&self) -> f64 {
        self.max_value
    }
    pub fn parameters(&self) -> &NoiseParameters {
        &self.parameters
    }
}

/// The legacy three-Perlin density sampler used by old_blended_noise nodes.
#[derive(Debug, Clone)]
pub struct BlendedNoise {
    min_limit: PerlinNoise,
    max_limit: PerlinNoise,
    main: PerlinNoise,
    pub xz_scale: f64,
    pub y_scale: f64,
    pub xz_factor: f64,
    pub y_factor: f64,
    pub smear_scale_multiplier: f64,
    xz_multiplier: f64,
    y_multiplier: f64,
    max_value: f64,
}
impl BlendedNoise {
    pub fn new(
        random: &mut impl RandomSource,
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
    ) -> Self {
        let min_limit =
            PerlinNoise::legacy_from_octaves(random, -15..=0).expect("valid vanilla blend octaves");
        let max_limit =
            PerlinNoise::legacy_from_octaves(random, -15..=0).expect("valid vanilla blend octaves");
        let main =
            PerlinNoise::legacy_from_octaves(random, -7..=0).expect("valid vanilla blend octaves");
        let xz_multiplier = 684.412 * xz_scale;
        let y_multiplier = 684.412 * y_scale;
        let max_value = min_limit.max_broken_value(y_multiplier);
        Self {
            min_limit,
            max_limit,
            main,
            xz_scale,
            y_scale,
            xz_factor,
            y_factor,
            smear_scale_multiplier,
            xz_multiplier,
            y_multiplier,
            max_value,
        }
    }
    pub fn with_new_random(&self, random: &mut impl RandomSource) -> Self {
        Self::new(
            random,
            self.xz_scale,
            self.y_scale,
            self.xz_factor,
            self.y_factor,
            self.smear_scale_multiplier,
        )
    }
    pub fn min_value(&self) -> f64 {
        -self.max_value
    }
    pub fn max_value(&self) -> f64 {
        self.max_value
    }
    pub fn compute(&self, x: i32, y: i32, z: i32) -> f64 {
        let x = f64::from(x) * self.xz_multiplier;
        let y = f64::from(y) * self.y_multiplier;
        let z = f64::from(z) * self.xz_multiplier;
        let main_x = x / self.xz_factor;
        let main_y = y / self.y_factor;
        let main_z = z / self.xz_factor;
        let smear = self.y_multiplier * self.smear_scale_multiplier;
        let main_smear = smear / self.y_factor;
        let mut blend = 0.0;
        let mut frequency = 1.0;
        for octave in 0..8 {
            if let Some(level) = self.main.get_octave_noise(octave) {
                blend += level.noise_scaled(
                    PerlinNoise::wrap(main_x * frequency),
                    PerlinNoise::wrap(main_y * frequency),
                    PerlinNoise::wrap(main_z * frequency),
                    main_smear * frequency,
                    main_y * frequency,
                ) / frequency;
            }
            frequency /= 2.0;
        }
        blend = (blend / 10.0 + 1.0) / 2.0;
        let use_max_only = blend >= 1.0;
        let use_min_only = blend <= 0.0;
        let mut min = 0.0;
        let mut max = 0.0;
        frequency = 1.0;
        for octave in 0..16 {
            let sx = PerlinNoise::wrap(x * frequency);
            let sy = PerlinNoise::wrap(y * frequency);
            let sz = PerlinNoise::wrap(z * frequency);
            let scale = smear * frequency;
            if !use_max_only {
                if let Some(level) = self.min_limit.get_octave_noise(octave) {
                    min += level.noise_scaled(sx, sy, sz, scale, y * frequency) / frequency;
                }
            }
            if !use_min_only {
                if let Some(level) = self.max_limit.get_octave_noise(octave) {
                    max += level.noise_scaled(sx, sy, sz, scale, y * frequency) / frequency;
                }
            }
            frequency /= 2.0;
        }
        let min = min / 512.0;
        let max = max / 512.0;
        (if blend < 0.0 {
            min
        } else if blend > 1.0 {
            max
        } else {
            lerp(blend, min, max)
        }) / 128.0
    }
}
