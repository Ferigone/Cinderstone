// SPDX-License-Identifier: GPL-2.0-only
//! Literal TeamMsgCommand over the actual admitted player, team and chat owners.
use super::*;
use mc121_rust::{chat_chain::PlayerChatMessage, chat_listener::AuthorizedCommand,
    chat_wire::BoundChatType, components::text::{ClickAction, ClickEvent, Component, HoverEvent, TextColor},
    scoreboard::PlayerTeam};

pub(super) fn is_command(line: &str) -> bool {
    matches!(line.split(' ').next(), Some("teammsg" | "tm"))
}

/// Retain the exact MessageArgument substring after Brigadier's single ASCII
/// separator. Further leading/trailing spaces are signed message text.
pub(super) fn literal_argument(line: &str) -> std::result::Result<Option<&str>, &'static str> {
    let Some((_, remainder)) = line.split_once(' ') else { return Ok(None) };
    let text = remainder;
    if text.is_empty() { return Ok(None) }
    if text.encode_utf16().count() > mc121_rust::chat_wire::CHAT_MESSAGE_MAX_UTF16 {
        return Err("team messages above 256 UTF-16 units are not supported");
    }
    if text.as_bytes().windows(2).any(|p| p[0] == b'@' && b"paresn".contains(&p[1])) {
        return Err("selector expansion in team messages is not implemented (WP-E3)");
    }
    Ok(Some(text))
}

/// PlayerTeam.getFormattedDisplayName followed by TeamMsgCommand.SUGGEST_STYLE.
fn target_name(team: &PlayerTeam) -> Result<serde_json::Value> {
    let mut display = team.display_name.clone();
    display.style.insertion = Some(team.name.clone());
    display.style.hover_event = Some(HoverEvent::Text(Box::new(Component::literal(&team.name))));
    let mut wrapped = Component::from_json(&json!({"translate":"chat.square_brackets","with":[display.to_json()]}))?;
    let color = team.color.context("team chat has no actual team color")?;
    if color.id() >= 0 { wrapped.style.color = Some(TextColor::parse(color.name())?); }
    wrapped.style.hover_event = Some(HoverEvent::Text(Box::new(
        Component::from_json(&json!({"translate":"chat.type.team.hover"}))?)));
    wrapped.style.click_event = Some(ClickEvent { action: ClickAction::SuggestCommand, value: "/teammsg ".into() });
    Ok(wrapped.to_json())
}

impl Server {
    fn team_chat_failure(&self, sender: Option<i32>, component: serde_json::Value) -> Result<()> {
        if let Some(sender) = sender {
            let mut outcome = main_chat::MainChatOutcome::default();
            outcome.append_system(sender, &json!({"text":"","extra":[component],"color":"red"}));
            let mut players = self.players.write().map_err(|_| anyhow!("players poisoned"))?;
            self.publish_chat_outcome(&mut players, sender, outcome)?;
        } else {
            println!("{}", json!({"event":"administration_feedback","component":component}));
        }
        Ok(())
    }

    /// Caller retains the actual chat/admission sequencing locks. Decode/signing
    /// checks have already run; never re-create a signed message from plaintext.
    pub(super) fn execute_team_message(&self, command: &AuthorizedCommand, sender: Option<i32>) -> Result<()> {
        let text = match literal_argument(&command.command) {
            Ok(Some(text)) => text,
            Ok(None) => return self.team_chat_failure(sender, json!({"translate":"command.unknown.command"})),
            Err(detail) => return self.team_chat_failure(sender, json!({"text":detail})),
        };
        let Some(sender) = sender else {
            return self.team_chat_failure(None, json!({"translate":"permissions.requires.entity"}));
        };
        let mut players = self.players.write().map_err(|_| anyhow!("players poisoned"))?;
        let profile = players.registry.get(sender).context("team chat sender absent")?.profile.clone();
        let board = &players.gameplay.scoreboard;
        let Some(team) = board.players_team(&profile.name) else {
            drop(players);
            return self.team_chat_failure(Some(sender), json!({"translate":"commands.teammsg.failed.noteam"}));
        };
        let target = target_name(team)?;
        let members = players.registry.snapshot().iter().filter(|p|
            board.players_team(&p.profile.name).is_some_and(|t| t.name == team.name))
            .map(|p| p.entity_id).collect::<BTreeSet<_>>();
        let name = main_scoreboard::decorated_name(board, &profile);
        let incoming = BoundChatType {
            registry_id: main_chat_keys::captured_chat_type_named(&self.fixtures.configuration, "minecraft:team_msg_command_incoming")?,
            name: name.clone(), target_name: Some(target.clone()),
        };
        let outgoing = BoundChatType {
            registry_id: main_chat_keys::captured_chat_type_named(&self.fixtures.configuration, "minecraft:team_msg_command_outgoing")?,
            name, target_name: Some(target),
        };
        let message = match command.signed_arguments.get("message") {
            Some(message) => {
                ensure!(message.signed_content() == text, "authorized team message range differs");
                message.clone()
            }
            None => PlayerChatMessage::system(text, main_admin_dispatch::wall_time_ms()?),
        };
        let outcome = players.chat.send_team_message(sender, &members, message, incoming, outgoing)?;
        self.publish_chat_outcome(&mut players, sender, outcome)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_range_retains_internal_spacing_quotes_unicode_and_trailing_spaces() {
        assert_eq!(literal_argument("tm   cześć  \"team\"  ").unwrap(), Some("  cześć  \"team\"  "));
        assert!(is_command("teammsg hi")); assert!(is_command("tm hi"));
        assert!(!is_command("teammsgx hi")); assert!(!is_command("/tm hi"));
        assert_eq!(literal_argument("tm ").unwrap(), None);
        assert!(literal_argument("teammsg hello @a").is_err());
        assert!(literal_argument(&format!("tm {}", "😀".repeat(129))).is_err());
        assert!(literal_argument(&format!("tm {}", "😀".repeat(128))).unwrap().is_some());
    }
    #[test]
    fn parsed_message_ranges_match_original_team_msg_registration() {
        let cases: serde_json::Value = serde_json::from_str(include_str!("../fixtures/team-message-ranges.json")).unwrap();
        for row in cases["cases"].as_array().unwrap() {
            let line = row["command"].as_str().unwrap();
            let message = literal_argument(line).unwrap();
            assert_eq!(message, row["message"].as_str(), "{line:?}");
            if let Some(message) = message {
                assert_eq!((line.encode_utf16().count() - message.encode_utf16().count()) as u64,
                    row["start_utf16"].as_u64().unwrap());
                assert_eq!(line.encode_utf16().count() as u64, row["end_utf16"].as_u64().unwrap());
            }
        }
    }
}
