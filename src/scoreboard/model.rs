//! `Scoreboard`, `ServerScoreboard` and the objects they hold.
//!
//! One struct plays both classes: operations behave as `ServerScoreboard` when `server` is set
//! (broadcast packets, tracked objectives, dirty marking) and as the bare `Scoreboard` otherwise.
//! Packets that Java hands to `PlayerList.broadcastAll` or to each online player's connection
//! are queued in an outbox for the caller to deliver, in the order the original sends them.

use super::criteria::{ObjectiveCriteria, RenderType};
use super::fastutil::OpenHashMap;
use super::format::{ChatFormatting, NumberFormat};
use super::packets::{ScoreboardPacket, TeamParameters};
use crate::components::text::Component;
use std::collections::{HashMap, HashSet};

/// `DisplaySlot`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DisplaySlot {
    List,
    Sidebar,
    BelowName,
    Team(ChatFormatting),
}

impl DisplaySlot {
    pub const COUNT: usize = 19;

    pub fn all() -> [DisplaySlot; 19] {
        let mut slots = [DisplaySlot::List; 19];
        slots[1] = DisplaySlot::Sidebar;
        slots[2] = DisplaySlot::BelowName;
        for (index, color) in ChatFormatting::ALL[..16].iter().enumerate() {
            slots[3 + index] = DisplaySlot::Team(*color);
        }
        slots
    }

    pub fn id(self) -> i32 {
        match self {
            Self::List => 0,
            Self::Sidebar => 1,
            Self::BelowName => 2,
            Self::Team(color) => 3 + color.id(),
        }
    }

    pub fn from_id(id: i32) -> Option<Self> {
        Self::all().get(usize::try_from(id).ok()?).copied()
    }

    /// `getSerializedName()`.
    pub fn name(self) -> String {
        match self {
            Self::List => "list".to_owned(),
            Self::Sidebar => "sidebar".to_owned(),
            Self::BelowName => "below_name".to_owned(),
            Self::Team(color) => format!("sidebar.team.{}", color.name()),
        }
    }

    pub fn by_name(name: &str) -> Option<Self> {
        Self::all().into_iter().find(|slot| slot.name() == name)
    }

    /// `teamColorToSlot`.
    pub fn team_color_to_slot(color: ChatFormatting) -> Option<Self> {
        (color.id() >= 0).then_some(Self::Team(color))
    }
}

/// `Team.Visibility`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Always,
    Never,
    HideForOtherTeams,
    HideForOwnTeam,
}

impl Visibility {
    pub fn name(self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::Never => "never",
            Self::HideForOtherTeams => "hideForOtherTeams",
            Self::HideForOwnTeam => "hideForOwnTeam",
        }
    }

    pub fn by_name(name: &str) -> Option<Self> {
        [
            Self::Always,
            Self::Never,
            Self::HideForOtherTeams,
            Self::HideForOwnTeam,
        ]
        .into_iter()
        .find(|v| v.name() == name)
    }
}

/// `Team.CollisionRule`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionRule {
    Always,
    Never,
    PushOtherTeams,
    PushOwnTeam,
}

impl CollisionRule {
    pub fn name(self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::Never => "never",
            Self::PushOtherTeams => "pushOtherTeams",
            Self::PushOwnTeam => "pushOwnTeam",
        }
    }

    pub fn by_name(name: &str) -> Option<Self> {
        [
            Self::Always,
            Self::Never,
            Self::PushOtherTeams,
            Self::PushOwnTeam,
        ]
        .into_iter()
        .find(|v| v.name() == name)
    }
}

/// `Objective`.
#[derive(Debug, Clone, PartialEq)]
pub struct Objective {
    pub name: String,
    pub criteria: ObjectiveCriteria,
    pub display_name: Component,
    pub render_type: RenderType,
    pub display_auto_update: bool,
    pub number_format: Option<NumberFormat>,
}

/// `Score`: new scores are locked.
#[derive(Debug, Clone, PartialEq)]
pub struct Score {
    pub value: i32,
    pub locked: bool,
    pub display: Option<Component>,
    pub number_format: Option<NumberFormat>,
}

impl Default for Score {
    fn default() -> Self {
        Self {
            value: 0,
            locked: true,
            display: None,
            number_format: None,
        }
    }
}

/// `PlayerScores`: one holder's scores by objective name. The original keys them by object
/// identity, so their order is not reproducible; they are kept in creation order.
#[derive(Debug, Clone, Default)]
pub struct PlayerScores {
    scores: Vec<(String, Score)>,
}

impl PlayerScores {
    fn get(&self, objective: &str) -> Option<&Score> {
        self.scores
            .iter()
            .find(|(name, _)| name == objective)
            .map(|(_, score)| score)
    }
    fn get_mut(&mut self, objective: &str) -> Option<&mut Score> {
        self.scores
            .iter_mut()
            .find(|(name, _)| name == objective)
            .map(|(_, score)| score)
    }
    fn remove(&mut self, objective: &str) -> bool {
        let before = self.scores.len();
        self.scores.retain(|(name, _)| name != objective);
        self.scores.len() != before
    }
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Score)> {
        self.scores
            .iter()
            .map(|(name, score)| (name.as_str(), score))
    }
}

/// `PlayerTeam`. `players` iterates in `java.util.HashSet` order (`player_order`).
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerTeam {
    pub name: String,
    pub display_name: Component,
    pub prefix: Component,
    pub suffix: Component,
    pub allow_friendly_fire: bool,
    pub see_friendly_invisibles: bool,
    pub nametag_visibility: Visibility,
    pub death_message_visibility: Visibility,
    /// `None` is the `null` that `ChatFormatting.getByName` can hand to `setColor`.
    pub color: Option<ChatFormatting>,
    pub collision_rule: CollisionRule,
    players: Vec<String>,
    /// Bucket count of the `HashSet` behind `getPlayers()`; it grows with the member count and never shrinks.
    table_size: usize,
}

impl PlayerTeam {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            display_name: Component::literal(name),
            prefix: Component::literal(""),
            suffix: Component::literal(""),
            allow_friendly_fire: true,
            see_friendly_invisibles: true,
            nametag_visibility: Visibility::Always,
            death_message_visibility: Visibility::Always,
            color: Some(ChatFormatting::Reset),
            collision_rule: CollisionRule::Always,
            players: Vec::new(),
            table_size: 16,
        }
    }

    fn add_member(&mut self, player: &str) -> bool {
        if self.players.iter().any(|member| member == player) {
            return false;
        }
        self.players.push(player.to_owned());
        if self.players.len() > self.table_size * 3 / 4 {
            self.table_size *= 2;
        }
        true
    }

    /// `getPlayers()` in `HashSet<String>` iteration order.
    pub fn players(&self) -> Vec<&str> {
        let names: Vec<&str> = self.players.iter().map(String::as_str).collect();
        crate::components::java::hashmap_iteration_order_in(&names, self.table_size)
            .into_iter()
            .map(|index| names[index])
            .collect()
    }

    /// `packOptions()`.
    pub fn pack_options(&self) -> i32 {
        i32::from(self.allow_friendly_fire) | (i32::from(self.see_friendly_invisibles) << 1)
    }

    pub fn parameters(&self) -> TeamParameters {
        TeamParameters {
            display_name: self.display_name.clone(),
            options: self.pack_options(),
            nametag_visibility: self.nametag_visibility.name().to_owned(),
            collision_rule: self.collision_rule.name().to_owned(),
            color: self.color,
            prefix: self.prefix.clone(),
            suffix: self.suffix.clone(),
        }
    }
}

/// `ScoreHolder`: a scoreboard name and an optional display name.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreHolder {
    pub name: String,
    pub display_name: Option<Component>,
}

impl ScoreHolder {
    pub fn named(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            display_name: None,
        }
    }
}

/// An error where Java throws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScoreboardError {
    /// `IllegalArgumentException`: an objective with that name already exists.
    ObjectiveExists(String),
    /// `IllegalStateException`: modifying a read-only score.
    ReadOnlyScore,
    /// `IllegalStateException`: the player is on another team or none.
    NotOnTeam(String),
    /// The named objective or team does not exist (Java would hold a `null`).
    Unknown(String),
}

impl std::fmt::Display for ScoreboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ObjectiveExists(name) => write!(f, "An objective with the name '{name}' already exists!"),
            Self::ReadOnlyScore => write!(f, "Cannot modify read-only score"),
            Self::NotOnTeam(team) => write!(f, "Player is either on another team or not on any team. Cannot remove from team '{team}'."),
            Self::Unknown(name) => write!(f, "unknown {name}"),
        }
    }
}

impl std::error::Error for ScoreboardError {}

/// What the original does with a packet: `PlayerList.broadcastAll`, or a send to each online player.
#[derive(Debug, Clone, PartialEq)]
pub enum Outbound {
    Broadcast(ScoreboardPacket),
    /// `startTrackingObjective` / `stopTrackingObjective`: these packets in order to every online player.
    EachPlayer(Vec<ScoreboardPacket>),
}

#[derive(Debug, Clone, Default)]
struct ServerLayer {
    tracked: HashSet<String>,
    outbox: Vec<Outbound>,
}

/// `Scoreboard` / `ServerScoreboard`.
#[derive(Debug, Clone)]
pub struct Scoreboard {
    objectives: OpenHashMap<Objective>,
    by_criteria: HashMap<String, Vec<String>>,
    player_scores: OpenHashMap<PlayerScores>,
    display_objectives: HashMap<DisplaySlot, String>,
    teams: OpenHashMap<PlayerTeam>,
    teams_by_player: OpenHashMap<String>,
    server: Option<ServerLayer>,
    dirty: bool,
}

impl Scoreboard {
    /// `new Scoreboard()`.
    pub fn new() -> Self {
        Self {
            objectives: OpenHashMap::with_capacity(16, 0.5),
            by_criteria: HashMap::new(),
            player_scores: OpenHashMap::with_capacity(16, 0.5),
            display_objectives: HashMap::new(),
            teams: OpenHashMap::new(),
            teams_by_player: OpenHashMap::new(),
            server: None,
            dirty: false,
        }
    }

    /// `new ServerScoreboard(server)`.
    pub fn new_server() -> Self {
        let mut scoreboard = Self::new();
        scoreboard.server = Some(ServerLayer::default());
        scoreboard
    }

    /// `SavedData.isDirty()` of the scoreboard's save data (set by every change).
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }

    fn mark_dirty(&mut self) {
        if self.server.is_some() {
            self.dirty = true;
        }
    }

    fn broadcast(&mut self, packet: ScoreboardPacket) {
        if let Some(server) = &mut self.server {
            server.outbox.push(Outbound::Broadcast(packet));
        }
    }

    /// Takes the packets queued for delivery since the last call.
    pub fn take_outbox(&mut self) -> Vec<Outbound> {
        self.server
            .as_mut()
            .map(|server| std::mem::take(&mut server.outbox))
            .unwrap_or_default()
    }

    fn is_tracked(&self, objective: &str) -> bool {
        self.server
            .as_ref()
            .is_some_and(|server| server.tracked.contains(objective))
    }

    // ------------------------------------------------------------ objectives

    pub fn objective(&self, name: &str) -> Option<&Objective> {
        self.objectives.get(name)
    }

    /// `getObjectives()` in `Object2ObjectOpenHashMap` order.
    pub fn objectives(&self) -> Vec<&Objective> {
        self.objectives.values().collect()
    }

    pub fn objective_names(&self) -> Vec<&str> {
        self.objectives.keys().collect()
    }

    pub fn add_objective(
        &mut self,
        name: &str,
        criteria: ObjectiveCriteria,
        display_name: Component,
        render_type: RenderType,
        display_auto_update: bool,
        number_format: Option<NumberFormat>,
    ) -> Result<(), ScoreboardError> {
        if self.objectives.contains_key(name) {
            return Err(ScoreboardError::ObjectiveExists(name.to_owned()));
        }
        self.by_criteria
            .entry(criteria.name().to_owned())
            .or_default()
            .push(name.to_owned());
        self.objectives.insert(
            name,
            Objective {
                name: name.to_owned(),
                criteria,
                display_name,
                render_type,
                display_auto_update,
                number_format,
            },
        );
        // onObjectiveAdded
        self.mark_dirty();
        Ok(())
    }

    fn objective_changed(&mut self, name: &str) {
        if self.is_tracked(name) {
            let packet = self.set_objective_packet(name, 2);
            self.broadcast(packet);
        }
        self.mark_dirty();
    }

    fn set_objective_packet(&self, name: &str, method: i32) -> ScoreboardPacket {
        let objective = self.objectives.get(name).expect("objective exists");
        ScoreboardPacket::SetObjective {
            name: objective.name.clone(),
            method,
            display_name: objective.display_name.clone(),
            render_type: objective.render_type,
            number_format: objective.number_format.clone(),
        }
    }

    pub fn set_objective_display_name(
        &mut self,
        name: &str,
        display_name: Component,
    ) -> Result<(), ScoreboardError> {
        self.objectives
            .get_mut(name)
            .ok_or_else(|| ScoreboardError::Unknown(name.to_owned()))?
            .display_name = display_name;
        self.objective_changed(name);
        Ok(())
    }

    pub fn set_objective_render_type(
        &mut self,
        name: &str,
        render_type: RenderType,
    ) -> Result<(), ScoreboardError> {
        self.objectives
            .get_mut(name)
            .ok_or_else(|| ScoreboardError::Unknown(name.to_owned()))?
            .render_type = render_type;
        self.objective_changed(name);
        Ok(())
    }

    pub fn set_objective_auto_update(
        &mut self,
        name: &str,
        auto: bool,
    ) -> Result<(), ScoreboardError> {
        self.objectives
            .get_mut(name)
            .ok_or_else(|| ScoreboardError::Unknown(name.to_owned()))?
            .display_auto_update = auto;
        self.objective_changed(name);
        Ok(())
    }

    pub fn set_objective_number_format(
        &mut self,
        name: &str,
        format: Option<NumberFormat>,
    ) -> Result<(), ScoreboardError> {
        self.objectives
            .get_mut(name)
            .ok_or_else(|| ScoreboardError::Unknown(name.to_owned()))?
            .number_format = format;
        self.objective_changed(name);
        Ok(())
    }

    pub fn remove_objective(&mut self, name: &str) -> Result<(), ScoreboardError> {
        let objective = self
            .objectives
            .remove(name)
            .ok_or_else(|| ScoreboardError::Unknown(name.to_owned()))?;
        for slot in DisplaySlot::all() {
            if self.display_objectives.get(&slot).map(String::as_str) == Some(name) {
                // `setDisplayObjective` is virtual: the server override runs here, mid-removal.
                self.set_display_objective_inner(slot, Some(&objective), None);
            }
        }
        if let Some(list) = self.by_criteria.get_mut(objective.criteria.name()) {
            list.retain(|candidate| candidate != name);
        }
        let holders: Vec<String> = self.player_scores.keys().map(str::to_owned).collect();
        for holder in holders {
            if let Some(scores) = self.player_scores.get_mut(&holder) {
                scores.remove(name);
            }
        }
        // onObjectiveRemoved
        if self.is_tracked(name) {
            self.stop_tracking_inner(&objective);
        }
        self.mark_dirty();
        Ok(())
    }

    // ------------------------------------------------------------ display slots

    pub fn display_objective(&self, slot: DisplaySlot) -> Option<&str> {
        self.display_objectives.get(&slot).map(String::as_str)
    }

    fn display_slot_count(&self, name: &str) -> usize {
        DisplaySlot::all()
            .iter()
            .filter(|slot| self.display_objectives.get(slot).map(String::as_str) == Some(name))
            .count()
    }

    /// `ServerScoreboard.setDisplayObjective` (`Scoreboard.setDisplayObjective` without a server).
    pub fn set_display_objective(
        &mut self,
        slot: DisplaySlot,
        objective: Option<&str>,
    ) -> Result<(), ScoreboardError> {
        let new = match objective {
            Some(name) => Some(
                self.objectives
                    .get(name)
                    .ok_or_else(|| ScoreboardError::Unknown(name.to_owned()))?
                    .clone(),
            ),
            None => None,
        };
        let old = self
            .display_objective(slot)
            .and_then(|name| self.objectives.get(name))
            .cloned();
        self.set_display_objective_inner(slot, old.as_ref(), new.as_ref());
        Ok(())
    }

    fn set_display_objective_inner(
        &mut self,
        slot: DisplaySlot,
        old: Option<&Objective>,
        new: Option<&Objective>,
    ) {
        // super.setDisplayObjective
        match new {
            Some(objective) => {
                self.display_objectives.insert(slot, objective.name.clone());
            }
            None => {
                self.display_objectives.remove(&slot);
            }
        }
        if self.server.is_none() {
            return;
        }
        let same = old.map(|o| o.name.as_str()) == new.map(|o| o.name.as_str());
        if let (false, Some(old)) = (same, old) {
            if self.display_slot_count(&old.name) > 0 {
                self.broadcast(ScoreboardPacket::SetDisplayObjective {
                    slot,
                    objective: new.map(|o| o.name.clone()).unwrap_or_default(),
                });
            } else {
                self.stop_tracking_inner(old);
            }
        }
        if let Some(new) = new {
            if self.is_tracked(&new.name) {
                self.broadcast(ScoreboardPacket::SetDisplayObjective {
                    slot,
                    objective: new.name.clone(),
                });
            } else {
                self.start_tracking_inner(new);
            }
        }
        self.mark_dirty();
    }

    // ------------------------------------------------------------ tracking (server)

    /// `getStartTrackingPackets`.
    pub fn start_tracking_packets(
        &self,
        objective: &str,
    ) -> Result<Vec<ScoreboardPacket>, ScoreboardError> {
        self.objectives
            .get(objective)
            .ok_or_else(|| ScoreboardError::Unknown(objective.to_owned()))?;
        let mut packets = vec![self.set_objective_packet(objective, 0)];
        for slot in DisplaySlot::all() {
            if self.display_objective(slot) == Some(objective) {
                packets.push(ScoreboardPacket::SetDisplayObjective {
                    slot,
                    objective: objective.to_owned(),
                });
            }
        }
        for entry in self.list_player_scores(objective) {
            packets.push(ScoreboardPacket::SetScore {
                owner: entry.owner,
                objective: objective.to_owned(),
                score: entry.value,
                display: entry.display,
                number_format: entry.number_format_override,
            });
        }
        Ok(packets)
    }

    /// `getStopTrackingPackets`.
    pub fn stop_tracking_packets(&self, objective: &Objective) -> Vec<ScoreboardPacket> {
        let mut packets = vec![ScoreboardPacket::SetObjective {
            name: objective.name.clone(),
            method: 1,
            display_name: objective.display_name.clone(),
            render_type: objective.render_type,
            number_format: objective.number_format.clone(),
        }];
        for slot in DisplaySlot::all() {
            if self.display_objective(slot) == Some(objective.name.as_str()) {
                packets.push(ScoreboardPacket::SetDisplayObjective {
                    slot,
                    objective: objective.name.clone(),
                });
            }
        }
        packets
    }

    fn start_tracking_inner(&mut self, objective: &Objective) {
        if let Ok(packets) = self.start_tracking_packets(&objective.name) {
            if let Some(server) = &mut self.server {
                server.outbox.push(Outbound::EachPlayer(packets));
                server.tracked.insert(objective.name.clone());
            }
        }
    }

    fn stop_tracking_inner(&mut self, objective: &Objective) {
        let packets = self.stop_tracking_packets(objective);
        if let Some(server) = &mut self.server {
            server.outbox.push(Outbound::EachPlayer(packets));
            server.tracked.remove(&objective.name);
        }
    }

    pub fn start_tracking_objective(&mut self, name: &str) -> Result<(), ScoreboardError> {
        let objective = self
            .objectives
            .get(name)
            .ok_or_else(|| ScoreboardError::Unknown(name.to_owned()))?
            .clone();
        self.start_tracking_inner(&objective);
        Ok(())
    }

    pub fn stop_tracking_objective(&mut self, name: &str) -> Result<(), ScoreboardError> {
        let objective = self
            .objectives
            .get(name)
            .ok_or_else(|| ScoreboardError::Unknown(name.to_owned()))?
            .clone();
        self.stop_tracking_inner(&objective);
        Ok(())
    }

    // ------------------------------------------------------------ scores

    /// `getOrCreatePlayerScore(holder, objective, forceWritable)`.
    pub fn get_or_create_player_score(
        &mut self,
        holder: &ScoreHolder,
        objective: &str,
        force_writable: bool,
    ) -> Result<ScoreAccess, ScoreboardError> {
        let objective_ref = self
            .objectives
            .get(objective)
            .ok_or_else(|| ScoreboardError::Unknown(objective.to_owned()))?;
        let writable = force_writable || !objective_ref.criteria.is_read_only();
        if !self.player_scores.contains_key(&holder.name) {
            self.player_scores
                .insert(&holder.name, PlayerScores::default());
        }
        let scores = self
            .player_scores
            .get_mut(&holder.name)
            .expect("inserted above");
        let created = scores.get(objective).is_none();
        if created {
            scores.scores.push((objective.to_owned(), Score::default()));
        }
        Ok(ScoreAccess {
            holder: holder.clone(),
            objective: objective.to_owned(),
            writable,
            created,
        })
    }

    /// `forAllObjectives(criteria, holder, action)`: the holder's access to every objective of the criteria.
    pub fn for_all_objectives(&mut self, criteria: &str, holder: &ScoreHolder) -> Vec<ScoreAccess> {
        let names = self.by_criteria.get(criteria).cloned().unwrap_or_default();
        names
            .iter()
            .filter_map(|name| self.get_or_create_player_score(holder, name, true).ok())
            .collect()
    }

    pub fn player_score_info(&self, holder: &str, objective: &str) -> Option<&Score> {
        self.player_scores.get(holder)?.get(objective)
    }

    /// `listPlayerScores(objective)`.
    pub fn list_player_scores(&self, objective: &str) -> Vec<PlayerScoreEntry> {
        self.player_scores
            .iter()
            .filter_map(|(owner, scores)| {
                scores.get(objective).map(|score| PlayerScoreEntry {
                    owner: owner.to_owned(),
                    value: score.value,
                    display: score.display.clone(),
                    number_format_override: score.number_format.clone(),
                })
            })
            .collect()
    }

    /// `getTrackedPlayers()`.
    pub fn tracked_players(&self) -> Vec<&str> {
        self.player_scores.keys_ascending().collect()
    }

    pub fn player_scores(&self, holder: &str) -> Option<&PlayerScores> {
        self.player_scores.get(holder)
    }

    fn score_changed(&mut self, holder: &ScoreHolder, objective: &str) {
        if self.is_tracked(objective) {
            if let Some(score) = self
                .player_scores
                .get(&holder.name)
                .and_then(|scores| scores.get(objective))
                .cloned()
            {
                self.broadcast(ScoreboardPacket::SetScore {
                    owner: holder.name.clone(),
                    objective: objective.to_owned(),
                    score: score.value,
                    display: score.display,
                    number_format: score.number_format,
                });
            }
        }
        self.mark_dirty();
    }

    /// `resetAllPlayerScores(holder)`.
    pub fn reset_all_player_scores(&mut self, holder: &ScoreHolder) {
        if self.player_scores.remove(&holder.name).is_some() {
            // onPlayerRemoved
            self.broadcast(ScoreboardPacket::ResetScore {
                owner: holder.name.clone(),
                objective: None,
            });
            self.mark_dirty();
        }
    }

    /// `resetSinglePlayerScore(holder, objective)`.
    pub fn reset_single_player_score(
        &mut self,
        holder: &ScoreHolder,
        objective: &str,
    ) -> Result<(), ScoreboardError> {
        self.objectives
            .get(objective)
            .ok_or_else(|| ScoreboardError::Unknown(objective.to_owned()))?;
        let Some(scores) = self.player_scores.get_mut(&holder.name) else {
            return Ok(());
        };
        let removed = scores.remove(objective);
        if scores.scores.is_empty() {
            if self.player_scores.remove(&holder.name).is_some() {
                self.broadcast(ScoreboardPacket::ResetScore {
                    owner: holder.name.clone(),
                    objective: None,
                });
                self.mark_dirty();
            }
        } else if removed {
            // onPlayerScoreRemoved
            if self.is_tracked(objective) {
                self.broadcast(ScoreboardPacket::ResetScore {
                    owner: holder.name.clone(),
                    objective: Some(objective.to_owned()),
                });
            }
            self.mark_dirty();
        }
        Ok(())
    }

    // ------------------------------------------------------------ teams

    pub fn team(&self, name: &str) -> Option<&PlayerTeam> {
        self.teams.get(name)
    }

    /// `getPlayerTeams()` in `Object2ObjectOpenHashMap` order.
    pub fn teams(&self) -> Vec<&PlayerTeam> {
        self.teams.values().collect()
    }

    pub fn players_team(&self, player: &str) -> Option<&PlayerTeam> {
        self.teams_by_player
            .get(player)
            .and_then(|team| self.teams.get(team))
    }

    /// `addPlayerTeam(name)`: returns the existing team when there is one.
    pub fn add_player_team(&mut self, name: &str) {
        if self.teams.contains_key(name) {
            return;
        }
        self.teams.insert(name, PlayerTeam::new(name));
        // onTeamAdded
        let packet = self.team_packet(name, 0);
        self.broadcast(packet);
        self.mark_dirty();
    }

    fn team_packet(&self, name: &str, method: i32) -> ScoreboardPacket {
        let team = self.teams.get(name).expect("team exists");
        match method {
            0 => ScoreboardPacket::SetPlayerTeam {
                name: name.to_owned(),
                method: 0,
                parameters: Some(team.parameters()),
                players: team.players().into_iter().map(str::to_owned).collect(),
            },
            _ => ScoreboardPacket::SetPlayerTeam {
                name: name.to_owned(),
                method: 2,
                parameters: Some(team.parameters()),
                players: Vec::new(),
            },
        }
    }

    fn team_changed(&mut self, name: &str) {
        let packet = self.team_packet(name, 2);
        self.broadcast(packet);
        self.mark_dirty();
    }

    /// Applies `change` to a team, then runs `onTeamChanged`.
    pub fn modify_team(
        &mut self,
        name: &str,
        change: impl FnOnce(&mut PlayerTeam),
    ) -> Result<(), ScoreboardError> {
        change(
            self.teams
                .get_mut(name)
                .ok_or_else(|| ScoreboardError::Unknown(name.to_owned()))?,
        );
        self.team_changed(name);
        Ok(())
    }

    pub fn remove_player_team(&mut self, name: &str) -> Result<(), ScoreboardError> {
        let team = self
            .teams
            .remove(name)
            .ok_or_else(|| ScoreboardError::Unknown(name.to_owned()))?;
        for player in &team.players {
            self.teams_by_player.remove(player);
        }
        // onTeamRemoved
        self.broadcast(ScoreboardPacket::SetPlayerTeam {
            name: name.to_owned(),
            method: 1,
            parameters: None,
            players: Vec::new(),
        });
        self.mark_dirty();
        Ok(())
    }

    /// `addPlayerToTeam(player, team)`.
    pub fn add_player_to_team(
        &mut self,
        player: &str,
        team: &str,
    ) -> Result<bool, ScoreboardError> {
        if !self.teams.contains_key(team) {
            return Err(ScoreboardError::Unknown(team.to_owned()));
        }
        if self.players_team(player).is_some() {
            self.remove_player_from_team(player)?;
        }
        self.teams_by_player.insert(player, team.to_owned());
        let added = self
            .teams
            .get_mut(team)
            .expect("checked")
            .add_member(player);
        if added {
            self.broadcast(ScoreboardPacket::SetPlayerTeam {
                name: team.to_owned(),
                method: 3,
                parameters: None,
                players: vec![player.to_owned()],
            });
            self.mark_dirty();
        }
        Ok(added)
    }

    /// `removePlayerFromTeam(player)`.
    pub fn remove_player_from_team(&mut self, player: &str) -> Result<bool, ScoreboardError> {
        match self.players_team(player).map(|team| team.name.clone()) {
            Some(team) => {
                self.remove_player_from_team_of(player, &team)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// `removePlayerFromTeam(player, team)`.
    pub fn remove_player_from_team_of(
        &mut self,
        player: &str,
        team: &str,
    ) -> Result<(), ScoreboardError> {
        if self.players_team(player).map(|t| t.name.as_str()) != Some(team) {
            return Err(ScoreboardError::NotOnTeam(team.to_owned()));
        }
        self.teams_by_player.remove(player);
        if let Some(team_ref) = self.teams.get_mut(team) {
            team_ref.players.retain(|member| member != player);
        }
        self.broadcast(ScoreboardPacket::SetPlayerTeam {
            name: team.to_owned(),
            method: 4,
            parameters: None,
            players: vec![player.to_owned()],
        });
        self.mark_dirty();
        Ok(())
    }

    // ------------------------------------------------------------ persistence helpers

    pub(super) fn all_scores(&self) -> Vec<(&str, &PlayerScores)> {
        self.player_scores.iter().collect()
    }

    pub(super) fn insert_loaded_score(&mut self, holder: &str, objective: &str, score: Score) {
        if !self.player_scores.contains_key(holder) {
            self.player_scores.insert(holder, PlayerScores::default());
        }
        let scores = self.player_scores.get_mut(holder).expect("inserted");
        match scores.get_mut(objective) {
            Some(existing) => *existing = score,
            None => scores.scores.push((objective.to_owned(), score)),
        }
    }
}

impl Default for Scoreboard {
    fn default() -> Self {
        Self::new()
    }
}

/// `PlayerScoreEntry`.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerScoreEntry {
    pub owner: String,
    pub value: i32,
    pub display: Option<Component>,
    pub number_format_override: Option<NumberFormat>,
}

/// `ScoreAccess`, bound to one holder and objective.
#[derive(Debug, Clone)]
pub struct ScoreAccess {
    holder: ScoreHolder,
    objective: String,
    writable: bool,
    created: bool,
}

impl ScoreAccess {
    fn score<'a>(&self, sb: &'a mut Scoreboard) -> &'a mut Score {
        sb.player_scores
            .get_mut(&self.holder.name)
            .and_then(|scores| scores.get_mut(&self.objective))
            .expect("the access keeps its score alive")
    }

    fn send(&mut self, sb: &mut Scoreboard) {
        sb.score_changed(&self.holder, &self.objective);
        self.created = false;
    }

    pub fn get(&self, sb: &mut Scoreboard) -> i32 {
        self.score(sb).value
    }

    pub fn set(&mut self, sb: &mut Scoreboard, value: i32) -> Result<(), ScoreboardError> {
        if !self.writable {
            return Err(ScoreboardError::ReadOnlyScore);
        }
        let mut changed = self.created;
        let auto = sb
            .objectives
            .get(&self.objective)
            .is_some_and(|o| o.display_auto_update);
        let holder_display = self.holder.display_name.clone();
        let score = self.score(sb);
        if auto {
            if let Some(display) = holder_display {
                if score.display.as_ref() != Some(&display) {
                    score.display = Some(display);
                    changed = true;
                }
            }
        }
        if value != score.value {
            score.value = value;
            changed = true;
        }
        if changed {
            self.send(sb);
        }
        Ok(())
    }

    pub fn add(&mut self, sb: &mut Scoreboard, amount: i32) -> Result<i32, ScoreboardError> {
        let value = self.get(sb).wrapping_add(amount);
        self.set(sb, value)?;
        Ok(value)
    }

    pub fn increment(&mut self, sb: &mut Scoreboard) -> Result<i32, ScoreboardError> {
        self.add(sb, 1)
    }

    pub fn reset(&mut self, sb: &mut Scoreboard) -> Result<(), ScoreboardError> {
        self.set(sb, 0)
    }

    pub fn locked(&self, sb: &mut Scoreboard) -> bool {
        self.score(sb).locked
    }

    fn set_locked(&mut self, sb: &mut Scoreboard, locked: bool) {
        self.score(sb).locked = locked;
        if self.created {
            self.send(sb);
        }
        // onScoreLockChanged
        sb.mark_dirty();
    }

    pub fn lock(&mut self, sb: &mut Scoreboard) {
        self.set_locked(sb, true);
    }

    pub fn unlock(&mut self, sb: &mut Scoreboard) {
        self.set_locked(sb, false);
    }

    pub fn display(&self, sb: &mut Scoreboard) -> Option<Component> {
        self.score(sb).display.clone()
    }

    pub fn set_display(&mut self, sb: &mut Scoreboard, display: Option<Component>) {
        if self.created || self.score(sb).display != display {
            self.score(sb).display = display;
            self.send(sb);
        }
    }

    pub fn set_number_format_override(
        &mut self,
        sb: &mut Scoreboard,
        format: Option<NumberFormat>,
    ) {
        self.score(sb).number_format = format;
        self.send(sb);
    }
}
