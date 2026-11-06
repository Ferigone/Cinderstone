//! Criterion hooks (WP-E4 stage 4): the places `ServerPlayer` writes to scores itself.
//!
//! * `doTick`: the read-only vitals criteria (`health`, `food`, `air`, `armor`, `xp`, `level`) are
//!   set when the value differs from the last recorded one ([`PlayerCriteriaTracker`]);
//! * `die`: `deathCount` is incremented; the kill credit's `awardKillScore` follows;
//! * `awardKillScore` (a player killing another entity): `totalKillCount`, `playerKillCount` for a
//!   player victim, `teamkill.<victim team colour>` for the killer and `killedByTeam.<killer team
//!   colour>` for the victim;
//! * `awardStat` / `resetStat`: a statistic criterion (`minecraft.custom:minecraft.jump`) is added to
//!   or reset.
//!
//! Each hook is `Scoreboard.forAllObjectives(criteria, holder, action)` over the objectives of that
//! criterion, so the scores, their packets and their persistence go through the oracle-checked
//! scoreboard model. The hook bodies are transcribed from the original's `ServerPlayer`
//! (decompiled reading, not executed: the oracle cannot construct a `ServerPlayer`).

use super::format::ChatFormatting;
use super::model::{ScoreHolder, Scoreboard, ScoreboardError};

/// The values `ServerPlayer.doTick` compares each tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerVitals {
    pub health: f32,
    pub absorption: f32,
    pub food_level: i32,
    pub air_supply: i32,
    pub armor_value: i32,
    pub total_experience: i32,
    pub experience_level: i32,
}

/// The `lastRecorded*` fields of one `ServerPlayer`. They start at `Float.MIN_VALUE` /
/// `Integer.MIN_VALUE`, so the first tick sets every vitals score; a respawned player is a new
/// `ServerPlayer` and starts over.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerCriteriaTracker {
    health_and_absorption: f32,
    food_level: i32,
    air_level: i32,
    armor: i32,
    level: i32,
    experience: i32,
}

impl Default for PlayerCriteriaTracker {
    fn default() -> Self {
        Self {
            health_and_absorption: f32::from_bits(1),
            food_level: i32::MIN,
            air_level: i32::MIN,
            armor: i32::MIN,
            level: i32::MIN,
            experience: i32::MIN,
        }
    }
}

/// `Mth.ceil(float)`.
fn ceil(value: f32) -> i32 {
    let truncated = value as i32;
    if value > truncated as f32 {
        truncated.wrapping_add(1)
    } else {
        truncated
    }
}

impl PlayerCriteriaTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// The scoreboard part of `ServerPlayer.doTick`, in the original's order.
    pub fn tick(&mut self, scoreboard: &mut Scoreboard, holder: &ScoreHolder, vitals: &PlayerVitals) -> Result<(), ScoreboardError> {
        let total = vitals.health + vitals.absorption;
        // `!=` on floats: NaN always differs, as in Java.
        if total != self.health_and_absorption {
            self.health_and_absorption = total;
            update_score_for_criteria(scoreboard, holder, "health", ceil(total))?;
        }
        for (last, value, criteria) in [
            (&mut self.food_level, vitals.food_level, "food"),
            (&mut self.air_level, vitals.air_supply, "air"),
            (&mut self.armor, vitals.armor_value, "armor"),
            (&mut self.experience, vitals.total_experience, "xp"),
            (&mut self.level, vitals.experience_level, "level"),
        ] {
            if value != *last {
                *last = value;
                update_score_for_criteria(scoreboard, holder, criteria, ceil(value as f32))?;
            }
        }
        Ok(())
    }
}

/// `ServerPlayer.updateScoreForCriteria`: every objective of the criterion gets `value`.
pub fn update_score_for_criteria(scoreboard: &mut Scoreboard, holder: &ScoreHolder, criteria: &str, value: i32) -> Result<(), ScoreboardError> {
    for mut access in scoreboard.for_all_objectives(criteria, holder) {
        access.set(scoreboard, value)?;
    }
    Ok(())
}

fn increment_all(scoreboard: &mut Scoreboard, holder: &ScoreHolder, criteria: &str) -> Result<(), ScoreboardError> {
    for mut access in scoreboard.for_all_objectives(criteria, holder) {
        access.increment(scoreboard)?;
    }
    Ok(())
}

/// `ServerPlayer.awardStat(stat, amount)`: the statistic's criterion (`Stat.getName()`, for example
/// `minecraft.custom:minecraft.jump`) is added to.
pub fn award_stat(scoreboard: &mut Scoreboard, holder: &ScoreHolder, stat_criteria: &str, amount: i32) -> Result<(), ScoreboardError> {
    for mut access in scoreboard.for_all_objectives(stat_criteria, holder) {
        access.add(scoreboard, amount)?;
    }
    Ok(())
}

/// `ServerPlayer.resetStat(stat)`.
pub fn reset_stat(scoreboard: &mut Scoreboard, holder: &ScoreHolder, stat_criteria: &str) -> Result<(), ScoreboardError> {
    for mut access in scoreboard.for_all_objectives(stat_criteria, holder) {
        access.reset(scoreboard)?;
    }
    Ok(())
}

/// The `deathCount` step of `ServerPlayer.die` (before the kill credit's `awardKillScore`).
pub fn player_died(scoreboard: &mut Scoreboard, victim: &ScoreHolder) -> Result<(), ScoreboardError> {
    increment_all(scoreboard, victim, "deathCount")
}

/// `ServerPlayer.handleTeamKill(holder, other, criteria)`: when `other` is on a team whose colour
/// is one of the sixteen colours, `holder`'s `<prefix>.<colour>` objectives are incremented. A team
/// whose colour was set to `null` makes the original throw a `NullPointerException`.
fn handle_team_kill(scoreboard: &mut Scoreboard, holder: &ScoreHolder, other: &str, prefix: &str) -> Result<(), ScoreboardError> {
    let Some(team) = scoreboard.players_team(other) else { return Ok(()) };
    let Some(color) = team.color else {
        return Err(ScoreboardError::NullTeamColor(team.name.clone()));
    };
    let id = color.id();
    if (0..16).contains(&id) {
        let criteria = format!("{prefix}.{}", ChatFormatting::ALL[id as usize].name());
        increment_all(scoreboard, holder, &criteria)?;
    }
    Ok(())
}

/// The scoreboard part of `ServerPlayer.awardKillScore(victim, …)` for a player `killer`: nothing
/// when the player killed itself; else `totalKillCount`, `playerKillCount` (player victims only),
/// then the team-kill criteria in both directions.
pub fn player_awarded_kill(scoreboard: &mut Scoreboard, killer: &ScoreHolder, victim: &ScoreHolder, victim_is_player: bool) -> Result<(), ScoreboardError> {
    if killer.name == victim.name && victim_is_player {
        return Ok(());
    }
    increment_all(scoreboard, killer, "totalKillCount")?;
    if victim_is_player {
        increment_all(scoreboard, killer, "playerKillCount")?;
    }
    handle_team_kill(scoreboard, killer, &victim.name, "teamkill")?;
    handle_team_kill(scoreboard, victim, &killer.name, "killedByTeam")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::text::Component;
    use crate::scoreboard::criteria::ObjectiveCriteria;

    fn board(objectives: &[(&str, &str)]) -> Scoreboard {
        let mut sb = Scoreboard::new_server();
        for (name, criteria) in objectives {
            let criteria = ObjectiveCriteria::by_name(criteria).unwrap().unwrap();
            let render = criteria.default_render_type();
            sb.add_objective(name, criteria, Component::from_json(&serde_json::json!(name)).unwrap(), render, false, None).unwrap();
        }
        sb
    }

    fn score(sb: &Scoreboard, holder: &str, objective: &str) -> Option<i32> {
        sb.player_score_info(holder, objective).map(|s| s.value)
    }

    #[test]
    fn vitals_are_written_when_they_change() {
        let mut sb = board(&[("hp", "health"), ("food", "food"), ("air", "air"), ("armor", "armor"), ("xp", "xp"), ("lvl", "level")]);
        let steve = ScoreHolder::named("Steve");
        let mut tracker = PlayerCriteriaTracker::new();
        let mut vitals = PlayerVitals { health: 19.5, absorption: 0.25, food_level: 20, air_supply: 300, armor_value: 0, total_experience: 7, experience_level: 1 };
        tracker.tick(&mut sb, &steve, &vitals).unwrap();
        // The first tick records everything; health is Mth.ceil(health + absorption).
        assert_eq!(score(&sb, "Steve", "hp"), Some(20));
        assert_eq!(score(&sb, "Steve", "food"), Some(20));
        assert_eq!(score(&sb, "Steve", "air"), Some(300));
        assert_eq!(score(&sb, "Steve", "armor"), Some(0));
        assert_eq!(score(&sb, "Steve", "xp"), Some(7));
        assert_eq!(score(&sb, "Steve", "lvl"), Some(1));
        // An unchanged value is not written again: a score changed elsewhere stays.
        for mut access in sb.for_all_objectives("food", &steve) {
            access.set(&mut sb, 5).unwrap();
        }
        tracker.tick(&mut sb, &steve, &vitals).unwrap();
        assert_eq!(score(&sb, "Steve", "food"), Some(5));
        vitals.food_level = 19;
        vitals.health = 3.0;
        vitals.absorption = 0.0;
        tracker.tick(&mut sb, &steve, &vitals).unwrap();
        assert_eq!(score(&sb, "Steve", "food"), Some(19));
        assert_eq!(score(&sb, "Steve", "hp"), Some(3));
        // A new tracker (a respawned ServerPlayer) writes everything again.
        for mut access in sb.for_all_objectives("air", &steve) {
            access.set(&mut sb, 1).unwrap();
        }
        PlayerCriteriaTracker::new().tick(&mut sb, &steve, &vitals).unwrap();
        assert_eq!(score(&sb, "Steve", "air"), Some(300));
    }

    #[test]
    fn deaths_kills_and_team_kills() {
        let mut sb = board(&[
            ("deaths", "deathCount"),
            ("kills", "totalKillCount"),
            ("pk", "playerKillCount"),
            ("tk_red", "teamkill.red"),
            ("kbt_blue", "killedByTeam.blue"),
            ("jumps", "minecraft.custom:minecraft.jump"),
        ]);
        sb.add_player_team("reds");
        sb.modify_team("reds", |team| team.color = Some(ChatFormatting::by_name("red").unwrap())).unwrap();
        sb.add_player_team("blues");
        sb.modify_team("blues", |team| team.color = Some(ChatFormatting::by_name("blue").unwrap())).unwrap();
        sb.add_player_to_team("Alex", "reds").unwrap();
        sb.add_player_to_team("Steve", "blues").unwrap();
        let steve = ScoreHolder::named("Steve");
        let alex = ScoreHolder::named("Alex");
        let zombie = ScoreHolder::named("0f0e0d0c-0000-4000-8000-000000000001");

        // Steve (blue) kills Alex (red): Alex dies first, then Steve's awardKillScore.
        player_died(&mut sb, &alex).unwrap();
        player_awarded_kill(&mut sb, &steve, &alex, true).unwrap();
        assert_eq!(score(&sb, "Alex", "deaths"), Some(1));
        assert_eq!(score(&sb, "Steve", "kills"), Some(1));
        assert_eq!(score(&sb, "Steve", "pk"), Some(1));
        assert_eq!(score(&sb, "Steve", "tk_red"), Some(1));
        assert_eq!(score(&sb, "Alex", "kbt_blue"), Some(1));

        // A mob kill counts toward totalKillCount only; a team-less victim adds no team kill.
        player_awarded_kill(&mut sb, &steve, &zombie, false).unwrap();
        assert_eq!(score(&sb, "Steve", "kills"), Some(2));
        assert_eq!(score(&sb, "Steve", "pk"), Some(1));
        assert_eq!(score(&sb, "Steve", "tk_red"), Some(1));

        // Killing yourself awards nothing.
        player_awarded_kill(&mut sb, &steve, &steve, true).unwrap();
        assert_eq!(score(&sb, "Steve", "kills"), Some(2));

        // A team colour that is not one of the sixteen colours (here `bold`) is skipped.
        sb.modify_team("reds", |team| team.color = Some(ChatFormatting::by_name("bold").unwrap())).unwrap();
        player_awarded_kill(&mut sb, &steve, &alex, true).unwrap();
        assert_eq!(score(&sb, "Steve", "tk_red"), Some(1));
        // A null colour is the original's NullPointerException.
        sb.modify_team("reds", |team| team.color = None).unwrap();
        assert!(matches!(player_awarded_kill(&mut sb, &steve, &alex, true), Err(ScoreboardError::NullTeamColor(_))));

        // Statistics add and reset.
        award_stat(&mut sb, &steve, "minecraft.custom:minecraft.jump", 3).unwrap();
        award_stat(&mut sb, &steve, "minecraft.custom:minecraft.jump", 2).unwrap();
        assert_eq!(score(&sb, "Steve", "jumps"), Some(5));
        // `ScoreAccess.reset` is `set(0)`: the score stays, at zero.
        reset_stat(&mut sb, &steve, "minecraft.custom:minecraft.jump").unwrap();
        assert_eq!(score(&sb, "Steve", "jumps"), Some(0));
    }
}
