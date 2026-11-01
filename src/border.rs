//! Exact 1.21 world-border settings, bounds and time-dependent size.
//!
//! Time is an injected monotonic millisecond value, as in Util.getMillis.
//! Listener callbacks, client packets and border damage belong to the host.
use crate::nbt::Tag;
use anyhow::Result;

pub const ABSOLUTE_MAX_SIZE: i32 = 29_999_984;
pub const MAX_SIZE: f64 = 59_999_968.;
pub const MAX_CENTER_COORDINATE: f64 = 29_999_984.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderSettings {
    pub center_x: f64,
    pub center_z: f64,
    pub damage_per_block: f64,
    pub safe_zone: f64,
    pub warning_blocks: i32,
    pub warning_time: i32,
    pub size: f64,
    pub size_lerp_time: i64,
    pub size_lerp_target: f64,
}
impl Default for BorderSettings {
    fn default() -> Self {
        Self {
            center_x: 0.,
            center_z: 0.,
            damage_per_block: 0.2,
            safe_zone: 5.,
            warning_blocks: 5,
            warning_time: 15,
            size: MAX_SIZE,
            size_lerp_time: 0,
            size_lerp_target: 0.,
        }
    }
}
impl BorderSettings {
    /// Settings.read uses DynamicLike/NbtOps: every NumericTag is accepted;
    /// missing fields and nonnumeric tags use the supplied field defaults.
    /// Numeric int/long conversion uses Number.intValue/longValue, not the
    /// floor conversion performed by CompoundTag.getInt/getLong.
    pub fn read(data: &Tag, defaults: Self) -> Self {
        Self {
            center_x: java_clamp(
                number_double(data.get("BorderCenterX")).unwrap_or(defaults.center_x),
                -MAX_CENTER_COORDINATE,
                MAX_CENTER_COORDINATE,
            ),
            center_z: java_clamp(
                number_double(data.get("BorderCenterZ")).unwrap_or(defaults.center_z),
                -MAX_CENTER_COORDINATE,
                MAX_CENTER_COORDINATE,
            ),
            damage_per_block: number_double(data.get("BorderDamagePerBlock"))
                .unwrap_or(defaults.damage_per_block),
            safe_zone: number_double(data.get("BorderSafeZone")).unwrap_or(defaults.safe_zone),
            warning_blocks: number_int(data.get("BorderWarningBlocks"))
                .unwrap_or(defaults.warning_blocks),
            warning_time: number_int(data.get("BorderWarningTime"))
                .unwrap_or(defaults.warning_time),
            size: number_double(data.get("BorderSize")).unwrap_or(defaults.size),
            size_lerp_time: number_long(data.get("BorderSizeLerpTime"))
                .unwrap_or(defaults.size_lerp_time),
            size_lerp_target: number_double(data.get("BorderSizeLerpTarget"))
                .unwrap_or(defaults.size_lerp_target),
        }
    }

    /// Retain unrelated fields. Vanilla writes both integer warning settings as
    /// DoubleTag; the interpolation duration alone is LongTag.
    pub fn write(&self, data: &mut Tag) -> Result<()> {
        for (name, value) in [
            ("BorderCenterX", self.center_x),
            ("BorderCenterZ", self.center_z),
            ("BorderSize", self.size),
            ("BorderSafeZone", self.safe_zone),
            ("BorderDamagePerBlock", self.damage_per_block),
            ("BorderSizeLerpTarget", self.size_lerp_target),
            ("BorderWarningBlocks", f64::from(self.warning_blocks)),
            ("BorderWarningTime", f64::from(self.warning_time)),
        ] {
            data.insert(name, Tag::Double(if value == 0. { 0. } else { value }))?;
        }
        data.insert("BorderSizeLerpTime", Tag::Long(self.size_lerp_time))?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderBounds {
    pub min_x: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_z: f64,
}
impl BorderBounds {
    /// Lower bounds are inclusive and upper bounds exclusive, including the
    /// optional margin used for collision activation near the border.
    pub fn contains(self, x: f64, z: f64, margin: f64) -> bool {
        x >= self.min_x - margin
            && x < self.max_x + margin
            && z >= self.min_z - margin
            && z < self.max_z + margin
    }
    pub fn distance(self, x: f64, z: f64) -> f64 {
        let x_min = x - self.min_x;
        let x_max = self.max_x - x;
        let z_min = z - self.min_z;
        let z_max = self.max_z - z;
        java_min(java_min(java_min(x_min, x_max), z_min), z_max)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BorderStatus {
    Growing,
    Shrinking,
    Stationary,
}

#[derive(Clone, Copy, Debug)]
enum BorderExtent {
    Static {
        size: f64,
    },
    Moving {
        from: f64,
        to: f64,
        begin: i64,
        end: i64,
        duration: f64,
    },
}

#[derive(Clone, Debug)]
pub struct WorldBorder {
    configuration: BorderSettings,
    absolute_max_size: i32,
    extent: BorderExtent,
}
impl Default for WorldBorder {
    fn default() -> Self {
        Self::from_settings(BorderSettings::default(), 0)
    }
}
impl WorldBorder {
    pub fn from_settings(settings: BorderSettings, now_ms: i64) -> Self {
        let mut border = Self {
            configuration: settings,
            absolute_max_size: ABSOLUTE_MAX_SIZE,
            extent: BorderExtent::Static {
                size: settings.size,
            },
        };
        if settings.size_lerp_time > 0 {
            border.lerp_size_between(
                settings.size,
                settings.size_lerp_target,
                settings.size_lerp_time,
                now_ms,
            );
        }
        border
    }
    pub fn center(&self) -> (f64, f64) {
        (self.configuration.center_x, self.configuration.center_z)
    }
    pub fn set_center(&mut self, x: f64, z: f64) {
        // The command/settings reader applies its own coordinate validation;
        // WorldBorder.setCenter itself does not clamp the values.
        self.configuration.center_x = x;
        self.configuration.center_z = z;
    }
    pub fn absolute_max_size(&self) -> i32 {
        self.absolute_max_size
    }
    pub fn set_absolute_max_size(&mut self, maximum: i32) {
        self.absolute_max_size = maximum;
    }
    pub fn set_size(&mut self, size: f64) {
        self.extent = BorderExtent::Static { size };
    }
    pub fn lerp_size_between(&mut self, from: f64, to: f64, duration_ms: i64, now_ms: i64) {
        self.extent = if from == to {
            BorderExtent::Static { size: to }
        } else {
            BorderExtent::Moving {
                from,
                to,
                begin: now_ms,
                end: now_ms.wrapping_add(duration_ms),
                duration: duration_ms as f64,
            }
        };
    }
    pub fn size(&self, now_ms: i64) -> f64 {
        match self.extent {
            BorderExtent::Static { size } => size,
            BorderExtent::Moving {
                from,
                to,
                begin,
                duration,
                ..
            } => {
                let progress = now_ms.wrapping_sub(begin) as f64 / duration;
                if progress < 1. {
                    from + progress * (to - from)
                } else {
                    to
                }
            }
        }
    }
    pub fn lerp_remaining_time(&self, now_ms: i64) -> i64 {
        match self.extent {
            BorderExtent::Static { .. } => 0,
            BorderExtent::Moving { end, .. } => end.wrapping_sub(now_ms),
        }
    }
    pub fn lerp_target(&self) -> f64 {
        match self.extent {
            BorderExtent::Static { size } => size,
            BorderExtent::Moving { to, .. } => to,
        }
    }
    pub fn lerp_speed(&self) -> f64 {
        match self.extent {
            BorderExtent::Static { .. } => 0.,
            BorderExtent::Moving {
                from,
                to,
                begin,
                end,
                ..
            } => (from - to).abs() / end.wrapping_sub(begin) as f64,
        }
    }
    pub fn status(&self) -> BorderStatus {
        match self.extent {
            BorderExtent::Static { .. } => BorderStatus::Stationary,
            BorderExtent::Moving { from, to, .. } => {
                if to < from {
                    BorderStatus::Shrinking
                } else {
                    BorderStatus::Growing
                }
            }
        }
    }
    /// Expired interpolation retains its status/negative remaining time until
    /// this explicit tick replaces the moving extent, matching the Java class.
    pub fn tick(&mut self, now_ms: i64) {
        if matches!(self.extent, BorderExtent::Moving { .. })
            && self.lerp_remaining_time(now_ms) <= 0
        {
            self.set_size(self.lerp_target());
        }
    }
    pub fn bounds(&self, now_ms: i64) -> BorderBounds {
        let half_size = self.size(now_ms) / 2.;
        let cap = f64::from(self.absolute_max_size);
        let min = f64::from(self.absolute_max_size.wrapping_neg());
        let (x, z) = self.center();
        BorderBounds {
            min_x: java_clamp(x - half_size, min, cap),
            min_z: java_clamp(z - half_size, min, cap),
            max_x: java_clamp(x + half_size, min, cap),
            max_z: java_clamp(z + half_size, min, cap),
        }
    }
    pub fn settings(&self, now_ms: i64) -> BorderSettings {
        BorderSettings {
            size: self.size(now_ms),
            size_lerp_time: self.lerp_remaining_time(now_ms),
            size_lerp_target: self.lerp_target(),
            ..self.configuration
        }
    }
    pub fn set_damage_per_block(&mut self, damage: f64) {
        self.configuration.damage_per_block = damage;
    }
    pub fn set_safe_zone(&mut self, safe_zone: f64) {
        self.configuration.safe_zone = safe_zone;
    }
    pub fn set_warning_time(&mut self, seconds: i32) {
        self.configuration.warning_time = seconds;
    }
    pub fn set_warning_blocks(&mut self, blocks: i32) {
        self.configuration.warning_blocks = blocks;
    }
}

fn number_double(tag: Option<&Tag>) -> Option<f64> {
    match tag? {
        Tag::Byte(value) => Some(f64::from(*value)),
        Tag::Short(value) => Some(f64::from(*value)),
        Tag::Int(value) => Some(f64::from(*value)),
        Tag::Long(value) => Some(*value as f64),
        Tag::Float(value) => Some(f64::from(*value)),
        Tag::Double(value) => Some(*value),
        _ => None,
    }
}
fn number_int(tag: Option<&Tag>) -> Option<i32> {
    match tag? {
        Tag::Byte(value) => Some(i32::from(*value)),
        Tag::Short(value) => Some(i32::from(*value)),
        Tag::Int(value) => Some(*value),
        Tag::Long(value) => Some(*value as i32),
        Tag::Float(value) => Some(*value as i32),
        Tag::Double(value) => Some(*value as i32),
        _ => None,
    }
}
fn number_long(tag: Option<&Tag>) -> Option<i64> {
    match tag? {
        Tag::Byte(value) => Some(i64::from(*value)),
        Tag::Short(value) => Some(i64::from(*value)),
        Tag::Int(value) => Some(i64::from(*value)),
        Tag::Long(value) => Some(*value),
        Tag::Float(value) => Some(*value as i64),
        Tag::Double(value) => Some(*value as i64),
        _ => None,
    }
}
fn java_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == 0. && b == 0. {
        if a.is_sign_negative() || b.is_sign_negative() {
            -0.
        } else {
            0.
        }
    } else if a <= b {
        a
    } else {
        b
    }
}
fn java_clamp(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else {
        java_min(value, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nbt::{JavaString, NbtRoot};
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use serde_json::{json, Value};
    use std::collections::BTreeMap;

    fn fixture() -> Value {
        serde_json::from_str(include_str!("../fixtures/border_vectors.json")).unwrap()
    }
    fn double(value: &Value) -> f64 {
        f64::from_bits(u64::from_str_radix(value.as_str().unwrap(), 16).unwrap())
    }
    fn bits(value: f64) -> String {
        format!(
            "{:016x}",
            if value.is_nan() {
                f64::NAN.to_bits()
            } else {
                value.to_bits()
            }
        )
    }
    fn read_settings(values: &Value) -> BorderSettings {
        BorderSettings {
            center_x: double(&values[0]),
            center_z: double(&values[1]),
            damage_per_block: double(&values[2]),
            safe_zone: double(&values[3]),
            warning_blocks: values[4].as_i64().unwrap() as i32,
            warning_time: values[5].as_i64().unwrap() as i32,
            size: double(&values[6]),
            size_lerp_time: values[7].as_i64().unwrap(),
            size_lerp_target: double(&values[8]),
        }
    }
    fn settings_values(settings: BorderSettings) -> Value {
        json!([
            bits(settings.center_x),
            bits(settings.center_z),
            bits(settings.damage_per_block),
            bits(settings.safe_zone),
            settings.warning_blocks,
            settings.warning_time,
            bits(settings.size),
            settings.size_lerp_time,
            bits(settings.size_lerp_target)
        ])
    }
    fn nbt(value: &Value) -> NbtRoot {
        NbtRoot::decode(&STANDARD.decode(value.as_str().unwrap()).unwrap()).unwrap()
    }
    fn fields(tag: &Tag) -> BTreeMap<JavaString, Vec<u8>> {
        let Tag::Compound(entries) = tag else {
            panic!("expected compound")
        };
        // Java CompoundTag uses map order. All output fields in this corpus are
        // scalar; compare their exact tag IDs and canonical wire payloads.
        entries
            .iter()
            .map(|(key, value)| (key.clone(), value.encode_anonymous().unwrap()))
            .collect()
    }

    #[test]
    fn actual_java_settings_numeric_conversion_defaults_and_canonical_nbt() {
        let data = fixture();
        for (index, case) in data["settings_cases"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let mut input = nbt(&case["query"]["input"]);
            let defaults = if case["query"]["official_defaults"].as_bool() == Some(true) {
                BorderSettings::default()
            } else {
                read_settings(&case["query"]["defaults"])
            };
            let settings = BorderSettings::read(&input.tag, defaults);
            assert_eq!(
                settings_values(settings),
                case["expected"]["settings"],
                "settings case {index}"
            );
            settings.write(&mut input.tag).unwrap();
            assert_eq!(
                fields(&input.tag),
                fields(&nbt(&case["expected"]["written"]).tag),
                "written NBT case {index}"
            );
        }
    }

    #[test]
    fn actual_java_moving_border_bounds_tick_and_point_queries() {
        let data = fixture();
        for (index, case) in data["world_cases"].as_array().unwrap().iter().enumerate() {
            let query = &case["query"];
            let mut border = WorldBorder::from_settings(
                read_settings(&query["settings"]),
                query["start_ms"].as_i64().unwrap(),
            );
            border.set_absolute_max_size(query["absolute_max"].as_i64().unwrap() as i32);
            for (step, (action, expected)) in query["actions"]
                .as_array()
                .unwrap()
                .iter()
                .zip(case["expected"].as_array().unwrap())
                .enumerate()
            {
                let now = action["now_ms"].as_i64().unwrap();
                match action["op"].as_str().unwrap() {
                    "snapshot" => {}
                    "tick" => border.tick(now),
                    "center" => border.set_center(double(&action["x"]), double(&action["z"])),
                    "size" => border.set_size(double(&action["size"])),
                    "lerp" => border.lerp_size_between(
                        double(&action["from"]),
                        double(&action["to"]),
                        action["duration"].as_i64().unwrap(),
                        now,
                    ),
                    "absolute" => {
                        border.set_absolute_max_size(action["value"].as_i64().unwrap() as i32)
                    }
                    other => panic!("unknown oracle border action {other}"),
                }
                let bounds = border.bounds(now);
                let status = match border.status() {
                    BorderStatus::Growing => 0,
                    BorderStatus::Shrinking => 1,
                    BorderStatus::Stationary => 2,
                };
                let queries = expected["queries"].as_array().unwrap().iter().map(|entry| {
                    let point = &entry["point"];
                    let (x, z, margin) = (double(&point[0]), double(&point[1]), double(&point[2]));
                    json!({"point":point, "within":bounds.contains(x,z,margin), "distance":bits(bounds.distance(x,z))})
                }).collect::<Vec<_>>();
                let actual = json!({
                    "settings": settings_values(border.settings(now)),
                    "bounds": [bits(bounds.min_x),bits(bounds.min_z),bits(bounds.max_x),bits(bounds.max_z)],
                    "size": bits(border.size(now)), "speed": bits(border.lerp_speed()),
                    "remaining": border.lerp_remaining_time(now), "target": bits(border.lerp_target()),
                    "status": status, "queries": queries
                });
                assert_eq!(
                    actual, *expected,
                    "world case {index}, step {step}, action {action}"
                );
            }
        }
    }
}
