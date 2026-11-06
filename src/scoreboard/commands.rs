// SPDX-License-Identifier: GPL-2.0-only
//! Scoreboard command operations over the same live ServerScoreboard owner.
use super::{
    ChatFormatting, CollisionRule, DisplaySlot, NumberFormat, ObjectiveCriteria, RenderType,
    ScoreHolder, Scoreboard, Visibility,
};
use crate::components::text::Component;
use anyhow::{bail, ensure, Context, Result};

struct Input<'a>(&'a str);
impl<'a> Input<'a> {
    fn word(&mut self) -> Result<&'a str> {
        self.0 = self.0.trim_start();
        ensure!(!self.0.is_empty(), "Incomplete command");
        let end = self.0.find(char::is_whitespace).unwrap_or(self.0.len());
        let word = &self.0[..end];
        self.0 = &self.0[end..];
        Ok(word)
    }
    fn rest(&mut self) -> &'a str {
        let rest = self.0.trim_start();
        self.0 = "";
        rest
    }
    fn done(&self) -> Result<()> {
        ensure!(
            self.0.trim().is_empty(),
            "Unexpected trailing command input"
        );
        Ok(())
    }
    fn int(&mut self) -> Result<i32> {
        self.word()?.parse().context("Expected integer")
    }
    fn boolean(&mut self) -> Result<bool> {
        match self.word()? {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => bail!("Expected true or false"),
        }
    }
    fn component(&mut self) -> Result<Component> {
        let value =
            crate::components::lenient::parse(self.rest()).map_err(|e| anyhow::anyhow!("{e}"))?;
        Component::from_json(&value).map_err(|e| anyhow::anyhow!("{e}"))
    }
}
fn literal(text: &str) -> Component {
    Component::from_json(&serde_json::json!(text)).expect("literal component")
}
fn targets(
    sb: &Scoreboard,
    input: &str,
    source: Option<&str>,
    online: &[String],
) -> Result<Vec<String>> {
    let names = match input {
        "@s" => vec![source.context("This command requires a player")?.to_owned()],
        "@a" => online.to_vec(),
        "*" => sb
            .tracked_players()
            .into_iter()
            .map(str::to_owned)
            .collect(),
        name if name.starts_with('@') => {
            bail!("This selector requires the command-context implementation in WP-E3")
        }
        name => vec![name.to_owned()],
    };
    ensure!(!names.is_empty(), "No entities found");
    Ok(names)
}
fn format(input: &mut Input<'_>) -> Result<Option<NumberFormat>> {
    if input.0.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(match input.word()? {
        "blank" => NumberFormat::Blank,
        "fixed" => NumberFormat::Fixed(input.component()?),
        "styled" => {
            let value = crate::components::lenient::parse(input.rest())
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let mut value = value;
            value
                .as_object_mut()
                .context("Expected style object")?
                .insert("type".into(), serde_json::json!("minecraft:styled"));
            NumberFormat::from_json(&value).map_err(|e| anyhow::anyhow!("{e}"))?
        }
        _ => bail!("Unknown number format"),
    }))
}
/// Names and @s/@a are resolved from the actual caller and admitted player list.
/// Wider entity selectors remain owned by WP-E3, never silently interpreted as names.
pub fn execute(
    sb: &mut Scoreboard,
    line: &str,
    source: Option<&str>,
    permission: u8,
    online: &[String],
) -> Result<String> {
    let holder = source.map(ScoreHolder::named);
    let players = online
        .iter()
        .map(|name| ScoreHolder::named(name))
        .collect::<Vec<_>>();
    execute_as(sb, line, holder.as_ref(), permission, &players)
}
/// Execute with the actual player identity, including its current display component.
pub fn execute_as(
    sb: &mut Scoreboard,
    line: &str,
    source: Option<&ScoreHolder>,
    permission: u8,
    online: &[ScoreHolder],
) -> Result<String> {
    let mut staged = sb.clone();
    let feedback = execute_inner(&mut staged, line, source, permission, online)?;
    *sb = staged;
    Ok(feedback)
}
fn execute_inner(
    sb: &mut Scoreboard,
    line: &str,
    source_holder: Option<&ScoreHolder>,
    permission: u8,
    online_holders: &[ScoreHolder],
) -> Result<String> {
    let source = source_holder.map(|holder| holder.name.as_str());
    let online = online_holders
        .iter()
        .map(|holder| holder.name.clone())
        .collect::<Vec<_>>();
    let holder = |name: &str| {
        source_holder
            .filter(|h| h.name == name)
            .or_else(|| online_holders.iter().find(|h| h.name == name))
            .cloned()
            .unwrap_or_else(|| ScoreHolder::named(name))
    };
    let mut input = Input(line);
    let root = input.word()?;
    ensure!(
        root == "trigger" || permission >= 2,
        "You do not have permission to use this command"
    );
    let result = match root {
        "scoreboard" => match input.word()? {
            "objectives" => match input.word()? {
                "list" => sb.objective_names().join(", "),
                "add" => {
                    let name = input.word()?;
                    let criteria = input.word()?;
                    let criteria = ObjectiveCriteria::by_name(criteria)
                        .map_err(|e| anyhow::anyhow!("{e:?}"))?
                        .context("Unknown criterion")?;
                    let render = criteria.default_render_type();
                    let display = if input.0.trim().is_empty() {
                        literal(name)
                    } else {
                        input.component()?
                    };
                    sb.add_objective(name, criteria, display, render, false, None)?;
                    format!("Created objective {name}")
                }
                "remove" => {
                    let name = input.word()?;
                    sb.remove_objective(name)?;
                    format!("Removed objective {name}")
                }
                "setdisplay" => {
                    let slot =
                        DisplaySlot::by_name(input.word()?).context("Unknown display slot")?;
                    let name = if input.0.trim().is_empty() {
                        None
                    } else {
                        Some(input.word()?)
                    };
                    sb.set_display_objective(slot, name)?;
                    "Updated display slot".into()
                }
                "modify" => {
                    let name = input.word()?;
                    match input.word()? {
                        "displayname" => sb.set_objective_display_name(name, input.component()?)?,
                        "rendertype" => {
                            let r = input.word()?;
                            ensure!(matches!(r, "hearts" | "integer"), "Unknown render type");
                            sb.set_objective_render_type(name, RenderType::by_id(r))?
                        }
                        "displayautoupdate" => {
                            sb.set_objective_auto_update(name, input.boolean()?)?
                        }
                        "numberformat" => {
                            sb.set_objective_number_format(name, format(&mut input)?)?
                        }
                        _ => bail!("Unknown objective option"),
                    };
                    format!("Updated objective {name}")
                }
                _ => bail!("Unknown objectives command"),
            },
            "players" => {
                let action = input.word()?;
                let display_action = if action == "display" {
                    Some(input.word()?)
                } else {
                    None
                };
                if action == "list" {
                    if input.0.trim().is_empty() {
                        sb.tracked_players().join(", ")
                    } else {
                        let holders = targets(sb, input.word()?, source, &online)?;
                        ensure!(holders.len() == 1, "Expected one score holder");
                        sb.player_scores(&holders[0])
                            .map(|s| {
                                s.iter()
                                    .map(|(o, s)| format!("{o}: {}", s.value))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .unwrap_or_default()
                    }
                } else {
                    let names = targets(sb, input.word()?, source, &online)?;
                    if action == "reset" && input.0.trim().is_empty() {
                        for name in names {
                            sb.reset_all_player_scores(&holder(&name));
                        }
                        "Reset scores".into()
                    } else {
                        let objective = input.word()?.to_owned();
                        ensure!(
                            sb.objective(&objective).is_some(),
                            "Unknown objective {objective}"
                        );
                        match action {
                            "get" => {
                                ensure!(names.len() == 1, "Expected one score holder");
                                let value = sb
                                    .player_score_info(&names[0], &objective)
                                    .context("No score is set")?
                                    .value;
                                format!("{} has {value} for {objective}", names[0])
                            }
                            "reset" => {
                                for name in names {
                                    sb.reset_single_player_score(&holder(&name), &objective)?;
                                }
                                "Reset scores".into()
                            }
                            "enable" => {
                                ensure!(
                                    sb.objective(&objective).unwrap().criteria.name() == "trigger",
                                    "Not a trigger objective"
                                );
                                for name in names {
                                    sb.get_or_create_player_score(
                                        &holder(&name),
                                        &objective,
                                        false,
                                    )?
                                    .unlock(sb);
                                }
                                "Enabled trigger".into()
                            }
                            "set" | "add" | "remove" => {
                                let value = input.int()?;
                                ensure!(
                                    action == "set" || value >= 0,
                                    "Expected non-negative value"
                                );
                                for name in names {
                                    let mut access = sb.get_or_create_player_score(
                                        &holder(&name),
                                        &objective,
                                        false,
                                    )?;
                                    if action == "set" {
                                        access.set(sb, value)?;
                                    } else {
                                        access.add(
                                            sb,
                                            if action == "remove" { -value } else { value },
                                        )?;
                                    }
                                }
                                "Updated scores".into()
                            }
                            "operation" => {
                                let operation = input.word()?;
                                let sources = targets(sb, input.word()?, source, &online)?;
                                let source_objective = input.word()?;
                                ensure!(
                                    sb.objective(source_objective).is_some(),
                                    "Unknown source objective"
                                );
                                ensure!(
                                    matches!(
                                        operation,
                                        "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "<" | ">" | "><"
                                    ),
                                    "Unknown operation"
                                );
                                for name in &names {
                                    for source in &sources {
                                        let mut target = sb.get_or_create_player_score(
                                            &holder(name),
                                            &objective,
                                            false,
                                        )?;
                                        let mut from = sb.get_or_create_player_score(
                                            &holder(source),
                                            source_objective,
                                            operation != "><",
                                        )?;
                                        let a = target.get(sb);
                                        let b = from.get(sb);
                                        let operation_kind =
                                            crate::commands::ScoreOperation::from_symbol(operation)
                                                .context("Unknown operation")?;
                                        let (v, _) = operation_kind
                                            .apply(a, b)
                                            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
                                        target.set(sb, v)?;
                                        if operation == "><" {
                                            from.set(sb, a)?;
                                        }
                                    }
                                }
                                "Applied score operation".into()
                            }
                            "display" => {
                                match display_action.expect("parsed display subcommand") {
                                    "name" => {
                                        let component = if input.0.trim().is_empty() {
                                            None
                                        } else {
                                            Some(input.component()?)
                                        };
                                        for name in names {
                                            sb.get_or_create_player_score(
                                                &holder(&name),
                                                &objective,
                                                true,
                                            )?
                                            .set_display(sb, component.clone());
                                        }
                                    }
                                    "numberformat" => {
                                        let f = format(&mut input)?;
                                        for name in names {
                                            sb.get_or_create_player_score(
                                                &holder(&name),
                                                &objective,
                                                true,
                                            )?
                                            .set_number_format_override(sb, f.clone());
                                        }
                                    }
                                    _ => bail!("Unknown display option"),
                                };
                                "Updated score display".into()
                            }
                            _ => bail!("Unknown players command"),
                        }
                    }
                }
            }
            _ => bail!("Unknown scoreboard command"),
        },
        "team" => match input.word()? {
            "list" => {
                if input.0.trim().is_empty() {
                    sb.teams()
                        .iter()
                        .map(|t| t.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                } else {
                    sb.team(input.word()?)
                        .context("Unknown team")?
                        .players()
                        .join(", ")
                }
            }
            "add" => {
                let name = input.word()?;
                ensure!(sb.team(name).is_none(), "Team already exists");
                let display = if input.0.trim().is_empty() {
                    None
                } else {
                    Some(input.component()?)
                };
                sb.add_player_team(name);
                if let Some(display) = display {
                    sb.modify_team(name, |t| t.display_name = display)?;
                }
                format!("Created team {name}")
            }
            "remove" => {
                let name = input.word()?;
                sb.remove_player_team(name)?;
                format!("Removed team {name}")
            }
            "empty" => {
                let name = input.word()?;
                let members = sb
                    .team(name)
                    .context("Unknown team")?
                    .players()
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                for member in members {
                    sb.remove_player_from_team_of(&member, name)?;
                }
                "Emptied team".into()
            }
            "join" => {
                let team = input.word()?;
                ensure!(sb.team(team).is_some(), "Unknown team");
                let members = if input.0.trim().is_empty() {
                    vec![source.context("This command requires a player")?.to_owned()]
                } else {
                    targets(sb, input.word()?, source, &online)?
                };
                for member in members {
                    sb.add_player_to_team(&member, team)?;
                }
                "Joined team".into()
            }
            "leave" => {
                for member in targets(sb, input.word()?, source, &online)? {
                    sb.remove_player_from_team(&member)?;
                }
                "Left team".into()
            }
            "modify" => {
                let name = input.word()?;
                ensure!(sb.team(name).is_some(), "Unknown team");
                match input.word()? {
                    "displayName" => {
                        let c = input.component()?;
                        sb.modify_team(name, |t| t.display_name = c)?
                    }
                    "prefix" => {
                        let c = input.component()?;
                        sb.modify_team(name, |t| t.prefix = c)?
                    }
                    "suffix" => {
                        let c = input.component()?;
                        sb.modify_team(name, |t| t.suffix = c)?
                    }
                    "color" => {
                        let color =
                            ChatFormatting::by_name(input.word()?).context("Unknown color")?;
                        sb.modify_team(name, |t| t.color = Some(color))?
                    }
                    "friendlyFire" => {
                        let v = input.boolean()?;
                        sb.modify_team(name, |t| t.allow_friendly_fire = v)?
                    }
                    "seeFriendlyInvisibles" => {
                        let v = input.boolean()?;
                        sb.modify_team(name, |t| t.see_friendly_invisibles = v)?
                    }
                    "nametagVisibility" => {
                        let v = Visibility::by_name(input.word()?).context("Unknown visibility")?;
                        sb.modify_team(name, |t| t.nametag_visibility = v)?
                    }
                    "deathMessageVisibility" => {
                        let v = Visibility::by_name(input.word()?).context("Unknown visibility")?;
                        sb.modify_team(name, |t| t.death_message_visibility = v)?
                    }
                    "collisionRule" => {
                        let v = CollisionRule::by_name(input.word()?)
                            .context("Unknown collision rule")?;
                        sb.modify_team(name, |t| t.collision_rule = v)?
                    }
                    _ => bail!("Unknown team option"),
                };
                "Updated team".into()
            }
            _ => bail!("Unknown team command"),
        },
        "trigger" => {
            let objective = input.word()?;
            ensure!(
                sb.objective(objective)
                    .is_some_and(|o| o.criteria.name() == "trigger"),
                "Not a trigger objective"
            );
            let name = source.context("This command requires a player")?;
            ensure!(
                sb.player_score_info(name, objective)
                    .is_some_and(|s| !s.locked),
                "Trigger is not enabled"
            );
            let (set, value) = if input.0.trim().is_empty() {
                (false, 1)
            } else {
                let action = input.word()?;
                ensure!(matches!(action, "set" | "add"), "Unknown trigger operation");
                (action == "set", input.int()?)
            };
            let mut access = sb.get_or_create_player_score(
                source_holder.context("This command requires a player")?,
                objective,
                false,
            )?;
            access.lock(sb);
            if set {
                access.set(sb, value)?;
            } else {
                access.add(sb, value)?;
            }
            "Triggered objective".into()
        }
        _ => bail!("Unknown command"),
    };
    input.done()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(sb: &mut Scoreboard, s: &str) -> Result<String> {
        execute(sb, s, Some("Alice"), 2, &["Alice".into(), "Bob".into()])
    }
    #[test]
    fn commands_enforce_permissions_readonly_and_trigger_locks() {
        let mut sb = Scoreboard::new_server();
        assert!(execute(
            &mut sb,
            "scoreboard objectives add x dummy",
            Some("Alice"),
            0,
            &[]
        )
        .is_err());
        run(&mut sb, "scoreboard objectives add hp health").unwrap();
        assert!(run(&mut sb, "scoreboard players set Alice hp 4").is_err());
        run(&mut sb, "scoreboard objectives add button trigger").unwrap();
        assert!(execute(&mut sb, "trigger button", Some("Alice"), 0, &[]).is_err());
        run(&mut sb, "scoreboard players enable Alice button").unwrap();
        execute(&mut sb, "trigger button add 3", Some("Alice"), 0, &[]).unwrap();
        assert_eq!(sb.player_score_info("Alice", "button").unwrap().value, 3);
        assert!(execute(&mut sb, "trigger button", Some("Alice"), 0, &[]).is_err());
        assert!(run(&mut sb, "team add broken trailing garbage").is_err());
        assert!(sb.team("broken").is_none());
    }
    #[test]
    fn teams_and_score_operations_use_live_model() {
        let mut sb = Scoreboard::new_server();
        for cmd in [
            "team add blue",
            r#"team modify blue prefix {"text":"[B] ","color":"blue"}"#,
            "team join blue @a",
            "scoreboard objectives add n dummy",
            "scoreboard players set Alice n -7",
            "scoreboard players set Bob n 3",
            "scoreboard players operation Alice n /= Bob n",
        ] {
            run(&mut sb, cmd).unwrap();
        }
        assert_eq!(sb.players_team("Bob").unwrap().name, "blue");
        assert_eq!(sb.player_score_info("Alice", "n").unwrap().value, -3);
        assert!(run(&mut sb, "scoreboard players list @s")
            .unwrap()
            .contains("n: -3"));
        assert!(run(&mut sb, "scoreboard players list @a").is_err());
        run(
            &mut sb,
            r#"scoreboard objectives modify n numberformat styled {"color":"red"}"#,
        )
        .unwrap();
        assert!(matches!(
            sb.objective("n").unwrap().number_format,
            Some(NumberFormat::Styled(_))
        ));
    }
}
