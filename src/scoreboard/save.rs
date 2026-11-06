//! `ScoreboardSaveData`: the `scoreboard.dat` payload of a [`Scoreboard`].

use super::criteria::{ObjectiveCriteria, RenderType};
use super::format::{ChatFormatting, NumberFormat};
use super::model::{CollisionRule, DisplaySlot, Score, Scoreboard, Visibility};
use crate::components::java::numeric_as_int;
use crate::components::text::{hash_ordered_compound, nbt_string, Component};
use crate::nbt::{JavaString, Tag};
use crate::saved_data::{DataFixType, Factory, SavedData};
use std::any::Any;

/// `FILE_ID`.
pub const FILE_ID: &str = "scoreboard";

fn json_text(component: &Component) -> String {
    component.to_json_string()
}

/// `Component.Serializer.fromJson(String)`: Gson's lenient reader, then the component codec.
fn component_from_json(text: &str, what: &str) -> Result<Component, String> {
    let value =
        crate::components::lenient::parse(text).map_err(|error| format!("{what}: {error}"))?;
    Component::from_json(&value).map_err(|error| format!("{what}: {error}"))
}

// ---- CompoundTag accessors with the original's defaults

pub(crate) fn get_string(tag: &Tag, key: &str) -> String {
    match tag.get(key) {
        Some(Tag::String(text)) => text.to_string_lossy().into_owned(),
        _ => String::new(),
    }
}

fn contains(tag: &Tag, key: &str, kind: u8) -> bool {
    match tag.get(key) {
        Some(found) => kind == 99 && (1..=6).contains(&found.tag_id()) || found.tag_id() == kind,
        None => false,
    }
}

/// `NumericTag.getAsByte`: floor-narrowed, so a float like -0.5 is -1.
pub(crate) fn get_boolean(tag: &Tag, key: &str) -> bool {
    let Some(found) = tag.get(key) else {
        return false;
    };
    match found {
        Tag::Byte(n) => *n != 0,
        Tag::Short(n) => (*n & 255) != 0,
        Tag::Int(n) => (*n & 255) != 0,
        Tag::Long(n) => (*n & 255) != 0,
        Tag::Float(_) | Tag::Double(_) => numeric_as_int(found).is_some_and(|n| (n & 255) != 0),
        _ => false,
    }
}

pub(crate) fn get_int(tag: &Tag, key: &str) -> i32 {
    tag.get(key).and_then(numeric_as_int).unwrap_or(0)
}

/// `getList(key, 10)` (or 8): the list when its element type matches or it is empty.
fn get_list(tag: &Tag, key: &str, element: u8) -> Vec<Tag> {
    match tag.get(key) {
        Some(Tag::List {
            element_type,
            values,
        }) if values.is_empty() || *element_type == element => values.clone(),
        _ => Vec::new(),
    }
}

fn compound_or_empty(tag: &Tag) -> Tag {
    match tag {
        Tag::Compound(_) => tag.clone(),
        _ => Tag::Compound(Vec::new()),
    }
}

// ---- Score

pub fn score_to_nbt(score: &Score, holder: &str, objective: &str) -> Tag {
    let mut entries: Vec<(&str, Tag)> = vec![
        ("Score", Tag::Int(score.value)),
        ("Locked", Tag::Byte(i8::from(score.locked))),
    ];
    if let Some(display) = &score.display {
        entries.push(("display", nbt_string(&json_text(display))));
    }
    if let Some(format) = &score.number_format {
        entries.push(("format", format.to_nbt()));
    }
    entries.push(("Name", nbt_string(holder)));
    entries.push(("Objective", nbt_string(objective)));
    hash_ordered_compound(entries)
}

/// `Score.read`. `Err` is a thrown exception (an unparsable `display`).
pub fn score_from_nbt(tag: &Tag) -> Result<Score, String> {
    let mut score = Score {
        value: get_int(tag, "Score"),
        locked: get_boolean(tag, "Locked"),
        display: None,
        number_format: None,
    };
    if contains(tag, "display", 8) {
        score.display = Some(component_from_json(
            &get_string(tag, "display"),
            "score display",
        )?);
    }
    if contains(tag, "format", 10) {
        if let Some(format) = tag.get("format") {
            if let Ok(parsed) = NumberFormat::from_nbt(format) {
                score.number_format = Some(parsed);
            }
        }
    }
    Ok(score)
}

// ---- the whole scoreboard

fn list(element_type: u8, values: Vec<Tag>) -> Tag {
    Tag::List {
        element_type: if values.is_empty() { 0 } else { element_type },
        values,
    }
}

/// `ScoreboardSaveData.save`: the payload stored under `data`.
pub fn save_scoreboard(scoreboard: &Scoreboard) -> Tag {
    let objectives: Vec<Tag> = scoreboard
        .objectives()
        .into_iter()
        .map(|objective| {
            let mut entries: Vec<(&str, Tag)> = vec![
                ("Name", nbt_string(&objective.name)),
                ("CriteriaName", nbt_string(objective.criteria.name())),
                (
                    "DisplayName",
                    nbt_string(&json_text(&objective.display_name)),
                ),
                ("RenderType", nbt_string(objective.render_type.id())),
                (
                    "display_auto_update",
                    Tag::Byte(i8::from(objective.display_auto_update)),
                ),
            ];
            if let Some(format) = &objective.number_format {
                entries.push(("format", format.to_nbt()));
            }
            hash_ordered_compound(entries)
        })
        .collect();
    let mut scores = Vec::new();
    for (holder, player_scores) in scoreboard.all_scores() {
        for (objective, score) in player_scores.iter() {
            scores.push(score_to_nbt(score, holder, objective));
        }
    }
    let teams: Vec<Tag> = scoreboard
        .teams()
        .into_iter()
        .map(|team| {
            let mut entries: Vec<(&str, Tag)> = vec![
                ("Name", nbt_string(&team.name)),
                ("DisplayName", nbt_string(&json_text(&team.display_name))),
            ];
            if let Some(color) = team.color {
                if color.id() >= 0 {
                    entries.push(("TeamColor", nbt_string(color.name())));
                }
            }
            entries.push((
                "AllowFriendlyFire",
                Tag::Byte(i8::from(team.allow_friendly_fire)),
            ));
            entries.push((
                "SeeFriendlyInvisibles",
                Tag::Byte(i8::from(team.see_friendly_invisibles)),
            ));
            entries.push(("MemberNamePrefix", nbt_string(&json_text(&team.prefix))));
            entries.push(("MemberNameSuffix", nbt_string(&json_text(&team.suffix))));
            entries.push((
                "NameTagVisibility",
                nbt_string(team.nametag_visibility.name()),
            ));
            entries.push((
                "DeathMessageVisibility",
                nbt_string(team.death_message_visibility.name()),
            ));
            entries.push(("CollisionRule", nbt_string(team.collision_rule.name())));
            let players = team
                .players()
                .into_iter()
                .map(|player| Tag::String(JavaString::from(player)))
                .collect();
            entries.push(("Players", list(8, players)));
            hash_ordered_compound(entries)
        })
        .collect();
    let mut slots: Vec<(&str, Tag)> = Vec::new();
    let names: Vec<(String, String)> = DisplaySlot::all()
        .into_iter()
        .filter_map(|slot| {
            scoreboard
                .display_objective(slot)
                .map(|name| (slot.name(), name.to_owned()))
        })
        .collect();
    for (slot, objective) in &names {
        slots.push((slot.as_str(), nbt_string(objective)));
    }
    let mut payload: Vec<(&str, Tag)> = vec![
        ("Objectives", list(10, objectives)),
        ("PlayerScores", list(10, scores)),
        ("Teams", list(10, teams)),
    ];
    if !slots.is_empty() {
        payload.push(("DisplaySlots", hash_ordered_compound(slots)));
    }
    hash_ordered_compound(payload)
}

/// `ScoreboardSaveData.load` into a new server scoreboard. `Err` is a thrown exception, after
/// which the original treats the whole file as unreadable.
pub fn load_scoreboard(data: &Tag) -> Result<Scoreboard, String> {
    let mut scoreboard = Scoreboard::new_server();
    // loadObjectives
    for entry in get_list(data, "Objectives", 10) {
        let entry = compound_or_empty(&entry);
        let criteria_name = get_string(&entry, "CriteriaName");
        let criteria = match ObjectiveCriteria::by_name(&criteria_name) {
            Ok(Some(found)) => found,
            Ok(None) => ObjectiveCriteria::dummy(),
            Err(error) => return Err(format!("invalid criteria name {}", error.0)),
        };
        let name = get_string(&entry, "Name");
        let display =
            component_from_json(&get_string(&entry, "DisplayName"), "objective DisplayName")?;
        let render_type = RenderType::by_id(&get_string(&entry, "RenderType"));
        let auto = get_boolean(&entry, "display_auto_update");
        let format = entry
            .get("format")
            .and_then(|tag| NumberFormat::from_nbt(tag).ok());
        scoreboard
            .add_objective(&name, criteria, display, render_type, auto, format)
            .map_err(|error| error.to_string())?;
    }
    // loadPlayerScores
    for entry in get_list(data, "PlayerScores", 10) {
        let entry = compound_or_empty(&entry);
        let score = score_from_nbt(&entry)?;
        let holder = get_string(&entry, "Name");
        let objective = get_string(&entry, "Objective");
        if scoreboard.objective(&objective).is_some() {
            scoreboard.insert_loaded_score(&holder, &objective, score);
        }
    }
    // loadDisplaySlots
    if contains(data, "DisplaySlots", 10) {
        if let Some(Tag::Compound(slots)) = data.get("DisplaySlots") {
            let names: Vec<&str> = slots.iter().filter_map(|(key, _)| key.as_str()).collect();
            for index in crate::components::java::hashmap_iteration_order(&names) {
                let key = names[index];
                if let Some(slot) = DisplaySlot::by_name(key) {
                    let objective = get_string(&Tag::Compound(slots.clone()), key);
                    let known = scoreboard
                        .objective(&objective)
                        .is_some()
                        .then_some(objective.as_str());
                    scoreboard
                        .set_display_objective(slot, known)
                        .map_err(|error| error.to_string())?;
                }
            }
        }
    }
    // loadTeams
    if contains(data, "Teams", 9) {
        for entry in get_list(data, "Teams", 10) {
            let entry = compound_or_empty(&entry);
            let name = get_string(&entry, "Name");
            scoreboard.add_player_team(&name);
            let display =
                component_from_json(&get_string(&entry, "DisplayName"), "team DisplayName")?;
            let change = |scoreboard: &mut Scoreboard,
                          f: &dyn Fn(&mut super::model::PlayerTeam)| {
                scoreboard
                    .modify_team(&name, |team| f(team))
                    .map_err(|error| error.to_string())
            };
            change(&mut scoreboard, &|team| team.display_name = display.clone())?;
            if contains(&entry, "TeamColor", 8) {
                let color = ChatFormatting::by_name(&get_string(&entry, "TeamColor"));
                change(&mut scoreboard, &|team| team.color = color)?;
            }
            if contains(&entry, "AllowFriendlyFire", 99) {
                let flag = get_boolean(&entry, "AllowFriendlyFire");
                change(&mut scoreboard, &|team| team.allow_friendly_fire = flag)?;
            }
            if contains(&entry, "SeeFriendlyInvisibles", 99) {
                let flag = get_boolean(&entry, "SeeFriendlyInvisibles");
                change(&mut scoreboard, &|team| team.see_friendly_invisibles = flag)?;
            }
            if contains(&entry, "MemberNamePrefix", 8) {
                let prefix =
                    component_from_json(&get_string(&entry, "MemberNamePrefix"), "prefix")?;
                change(&mut scoreboard, &|team| team.prefix = prefix.clone())?;
            }
            if contains(&entry, "MemberNameSuffix", 8) {
                let suffix =
                    component_from_json(&get_string(&entry, "MemberNameSuffix"), "suffix")?;
                change(&mut scoreboard, &|team| team.suffix = suffix.clone())?;
            }
            if contains(&entry, "NameTagVisibility", 8) {
                if let Some(visibility) =
                    Visibility::by_name(&get_string(&entry, "NameTagVisibility"))
                {
                    change(&mut scoreboard, &|team| {
                        team.nametag_visibility = visibility
                    })?;
                }
            }
            if contains(&entry, "DeathMessageVisibility", 8) {
                if let Some(visibility) =
                    Visibility::by_name(&get_string(&entry, "DeathMessageVisibility"))
                {
                    change(&mut scoreboard, &|team| {
                        team.death_message_visibility = visibility
                    })?;
                }
            }
            if contains(&entry, "CollisionRule", 8) {
                if let Some(rule) = CollisionRule::by_name(&get_string(&entry, "CollisionRule")) {
                    change(&mut scoreboard, &|team| team.collision_rule = rule)?;
                }
            }
            for player in get_list(&entry, "Players", 8) {
                let player = match player {
                    Tag::String(text) => text.to_string_lossy().into_owned(),
                    _ => String::new(),
                };
                scoreboard
                    .add_player_to_team(&player, &name)
                    .map_err(|error| error.to_string())?;
            }
        }
    }
    // The original broadcasts to nobody while loading; nothing is queued for delivery.
    scoreboard.take_outbox();
    Ok(scoreboard)
}

/// The `scoreboard` saved data: owns the [`Scoreboard`] whose changes mark it dirty.
#[derive(Debug, Clone)]
pub struct ScoreboardData {
    pub scoreboard: Scoreboard,
}

impl ScoreboardData {
    pub fn new() -> Self {
        Self {
            scoreboard: Scoreboard::new_server(),
        }
    }

    /// `ServerScoreboard.dataFactory()`.
    pub fn factory() -> Factory<ScoreboardData> {
        Factory {
            constructor: Box::new(ScoreboardData::new),
            deserializer: Box::new(|data| {
                load_scoreboard(data).map(|scoreboard| ScoreboardData { scoreboard })
            }),
            fix: DataFixType::SavedDataScoreboard,
        }
    }
}

impl Default for ScoreboardData {
    fn default() -> Self {
        Self::new()
    }
}

impl SavedData for ScoreboardData {
    fn save(&self) -> Tag {
        save_scoreboard(&self.scoreboard)
    }
    fn is_dirty(&self) -> bool {
        self.scoreboard.is_dirty()
    }
    fn set_dirty(&mut self, dirty: bool) {
        self.scoreboard.set_dirty(dirty);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
