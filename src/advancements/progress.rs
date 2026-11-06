//! `CriterionProgress`, `AdvancementProgress` and the `advancements/<uuid>.json` document.

use super::requirements::AdvancementRequirements;
use super::time::{format_obtained, parse_obtained};
use crate::components::java::resource_location;
use crate::components::lenient::parse_strict_prefix;
use serde_json::{Map, Number, Value};
use std::collections::BTreeMap;

/// `SharedConstants.getCurrentVersion().getDataVersion().getVersion()` for 1.21.
pub const DATA_VERSION: i32 = 3953;

/// The version a progress file is read as when it has no usable `DataVersion`
/// (`DataFixTypes.ADVANCEMENTS.wrapCodec(.., 1343)`).
pub const DEFAULT_FILE_VERSION: i32 = 1343;

/// The newest data fixer that can change a progress file is `CriteriaRenameFix` at schema 3097
/// (British shorthair cat criterion); the earlier ones rename advancements (1501, 2100, 2503,
/// 2846), criteria (3086) and biomes/entity names inside criteria. A file at this version or later
/// goes through the `ADVANCEMENTS` fixers unchanged; an older one needs them (not ported).
pub const LAST_FIXER_VERSION: i32 = 3097;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressError {
    /// The document or one of its entries is rejected by the codec.
    Codec(String),
    /// The file is older than [`LAST_FIXER_VERSION`] and the `ADVANCEMENTS` data fixers are not
    /// ported (renamed advancements and criteria, biome and entity renames inside criteria).
    DataFixerRequired(i32),
    /// The document cannot be written: an instant is beyond what `LocalDate` can print (the
    /// original throws from `save()`).
    Encode(String),
}

/// `CriterionProgress`: when the criterion was obtained (epoch seconds), if it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CriterionProgress {
    pub obtained: Option<i64>,
}

impl CriterionProgress {
    pub fn is_done(&self) -> bool {
        self.obtained.is_some()
    }
}

/// `AdvancementProgress`.
///
/// The criteria are kept in a sorted map; the original keeps a `HashMap`, whose order only shows
/// in the network form of the progress (see `ClientboundUpdateAdvancementsPacket`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AdvancementProgress {
    criteria: BTreeMap<String, CriterionProgress>,
    requirements: AdvancementRequirements,
}

impl AdvancementProgress {
    pub fn new() -> Self {
        Self::default()
    }

    /// `update(requirements)`: keep exactly the criteria the requirements name.
    pub fn update(&mut self, requirements: AdvancementRequirements) {
        let names: Vec<String> = requirements
            .names()
            .into_iter()
            .map(str::to_owned)
            .collect();
        self.criteria.retain(|name, _| names.contains(name));
        for name in names {
            self.criteria.entry(name).or_default();
        }
        self.requirements = requirements;
    }

    pub fn requirements(&self) -> &AdvancementRequirements {
        &self.requirements
    }

    pub fn is_done(&self) -> bool {
        self.requirements.test(|name| self.is_criterion_done(name))
    }

    pub fn has_progress(&self) -> bool {
        self.criteria.values().any(CriterionProgress::is_done)
    }

    fn is_criterion_done(&self, name: &str) -> bool {
        self.criteria
            .get(name)
            .is_some_and(CriterionProgress::is_done)
    }

    /// `grantProgress(name)` at `now` (epoch seconds): true when the criterion exists and was not
    /// done yet.
    pub fn grant_progress(&mut self, name: &str, now: i64) -> bool {
        match self.criteria.get_mut(name) {
            Some(criterion) if !criterion.is_done() => {
                criterion.obtained = Some(now);
                true
            }
            _ => false,
        }
    }

    /// `revokeProgress(name)`: true when the criterion existed and was done.
    pub fn revoke_progress(&mut self, name: &str) -> bool {
        match self.criteria.get_mut(name) {
            Some(criterion) if criterion.is_done() => {
                criterion.obtained = None;
                true
            }
            _ => false,
        }
    }

    pub fn criterion(&self, name: &str) -> Option<&CriterionProgress> {
        self.criteria.get(name)
    }

    pub fn criteria(&self) -> impl Iterator<Item = (&str, &CriterionProgress)> {
        self.criteria
            .iter()
            .map(|(name, progress)| (name.as_str(), progress))
    }

    /// `getPercent()`: completed requirements over all requirements, as a `float`.
    pub fn percent(&self) -> f32 {
        if self.criteria.is_empty() {
            0.0
        } else {
            self.requirements.count(|name| self.is_criterion_done(name)) as f32
                / self.requirements.len() as f32
        }
    }

    /// `getFirstProgressDate()`: the earliest obtained instant.
    pub fn first_progress_date(&self) -> Option<i64> {
        self.criteria
            .values()
            .filter_map(|criterion| criterion.obtained)
            .min()
    }

    /// `AdvancementProgress.CODEC` decoding of one entry. `criteria` is optional (a JSON `null` is
    /// absent) but must be well formed when present; `done` is read and ignored (`orElse(true)`
    /// swallows any error, and `done` is recomputed from the requirements anyway).
    pub fn from_json(value: &Value) -> Result<Self, ProgressError> {
        let Value::Object(map) = value else {
            return Err(ProgressError::Codec("progress is not an object".to_owned()));
        };
        let mut progress = Self::new();
        match map.get("criteria").filter(|v| !v.is_null()) {
            None => {}
            Some(Value::Object(criteria)) => {
                for (name, obtained) in criteria {
                    let text = obtained.as_str().ok_or_else(|| {
                        ProgressError::Codec(format!("criterion {name}: not a string"))
                    })?;
                    let seconds = parse_obtained(text).ok_or_else(|| {
                        ProgressError::Codec(format!("criterion {name}: bad time {text}"))
                    })?;
                    progress.criteria.insert(
                        name.clone(),
                        CriterionProgress {
                            obtained: Some(seconds),
                        },
                    );
                }
            }
            Some(_) => return Err(ProgressError::Codec("criteria is not an object".to_owned())),
        }
        Ok(progress)
    }

    /// `AdvancementProgress.CODEC` encoding: the criteria that are done (omitted when there are
    /// none) and `done`.
    pub fn to_json(&self) -> Result<Value, ProgressError> {
        let mut object = Map::new();
        let mut criteria = Map::new();
        for (name, criterion) in &self.criteria {
            if let Some(seconds) = criterion.obtained {
                let text = format_obtained(seconds).ok_or_else(|| {
                    ProgressError::Encode(format!("instant {seconds} is out of range"))
                })?;
                criteria.insert(name.clone(), Value::String(text));
            }
        }
        if !criteria.is_empty() {
            object.insert("criteria".to_owned(), Value::Object(criteria));
        }
        object.insert("done".to_owned(), Value::Bool(self.is_done()));
        Ok(Value::Object(object))
    }
}

/// `Number.intValue()` of a Gson `LazilyParsedNumber`: integers wrap to 32 bits, decimals go
/// through `BigDecimal.intValue()` (truncate, then the low 32 bits).
///
/// `serde_json` keeps a decimal as the nearest `f64`, so the low bits of a value above 2^53 can
/// differ from the original's exact decimal (`1E30` is 2^30 there and 0 here); a `DataVersion` that
/// large is not a real file. Above 2^127 both are 0 for every exponent form.
fn number_as_int(number: &Number) -> i32 {
    if let Some(value) = number.as_i64() {
        value as i32
    } else if let Some(value) = number.as_u64() {
        value as i32
    } else {
        number.as_f64().map_or(0, |value| {
            if value.abs() >= 2f64.powi(127) {
                0
            } else {
                value.trunc() as i128 as i32
            }
        })
    }
}

/// The version `DataFixTypes.wrapCodec` reads: a numeric `DataVersion`, otherwise the default
/// (`JsonOps.getNumberValue` takes numbers only: a string, boolean or `null` is "no version").
fn file_version(members: &Map<String, Value>) -> i32 {
    match members.get("DataVersion") {
        Some(Value::Number(number)) => number_as_int(number),
        _ => DEFAULT_FILE_VERSION,
    }
}

/// What `PlayerAdvancements.load` does with the text of `advancements/<uuid>.json`: strict JSON
/// (`JsonReader.setLenient(false)` and `Streams.parse`: one value is read, whatever follows it is
/// never looked at, and a byte-order mark is skipped), the `ADVANCEMENTS` data fix up to this
/// version, then `Data.CODEC`. Returns the progress entries by canonical advancement id; two
/// spellings of one id (`x` and `minecraft:x`) collapse onto the later member.
pub fn parse_progress_file(
    text: &str,
) -> Result<Vec<(String, AdvancementProgress)>, ProgressError> {
    let parsed = parse_strict_prefix(text).map_err(ProgressError::Codec)?;
    // A document that is not an object is not a map for the codec, whatever its version.
    let Value::Object(members) = parsed else {
        return Err(ProgressError::Codec(
            "progress file is not an object".to_owned(),
        ));
    };
    let version = file_version(&members);
    if version < LAST_FIXER_VERSION {
        return Err(ProgressError::DataFixerRequired(version));
    }
    let mut entries: Vec<(String, AdvancementProgress)> = Vec::new();
    for (key, value) in members
        .iter()
        .filter(|(name, _)| name.as_str() != "DataVersion")
    {
        let id = resource_location(key)
            .ok_or_else(|| ProgressError::Codec(format!("bad advancement id {key}")))?;
        let progress = AdvancementProgress::from_json(value)?;
        // `Codec.unboundedMap` refuses two keys that decode to the same id.
        if entries.iter().any(|(existing, _)| *existing == id) {
            return Err(ProgressError::Codec(format!(
                "Duplicate entry for key: {id}"
            )));
        }
        entries.push((id, progress));
    }
    Ok(entries)
}

/// The document `PlayerAdvancements.save` writes: the entries that have progress, then
/// `DataVersion`. (The original's entry order is that of an identity hash map, so it is not a
/// contract; readers do not depend on it.)
pub fn write_progress_file<'a>(
    entries: impl IntoIterator<Item = (&'a str, &'a AdvancementProgress)>,
) -> Result<Value, ProgressError> {
    let mut object = Map::new();
    for (id, progress) in entries {
        if progress.has_progress() {
            object.insert(id.to_owned(), progress.to_json()?);
        }
    }
    object.insert("DataVersion".to_owned(), Value::from(DATA_VERSION));
    Ok(Value::Object(object))
}
