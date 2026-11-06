// SPDX-License-Identifier: GPL-2.0-only
//! Player.getDisplayName for the admitted native profile, including team and identity style.
use super::{ScoreHolder, Scoreboard};
use crate::components::text::{ClickAction, ClickEvent, Component, HoverEvent, TextColor};

pub fn player_holder(board: &Scoreboard, name: &str, uuid: [u8; 16]) -> ScoreHolder {
    let plain = Component::literal(name);
    let mut display = if let Some(team) = board.players_team(name) {
        let mut component = Component::literal("");
        component.siblings = vec![team.prefix.clone(), plain.clone(), team.suffix.clone()];
        if let Some(color) = team.color {
            if color.id() >= 0 {
                component.style.color =
                    Some(TextColor::parse(color.name()).expect("registered text color"));
            }
        }
        component
    } else {
        plain.clone()
    };
    display.style.insertion = Some(name.to_owned());
    display.style.click_event = Some(ClickEvent {
        action: ClickAction::SuggestCommand,
        value: format!("/tell {name} "),
    });
    display.style.hover_event = Some(HoverEvent::PlayerEntity {
        uuid,
        name: Some(Box::new(plain)),
    });
    ScoreHolder {
        name: name.to_owned(),
        display_name: Some(display),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoreboard::{commands, hooks, save, NumberFormat};
    #[test]
    fn display_operations_and_native_auto_update_preserve_player_identity() {
        let mut board = Scoreboard::new_server();
        for line in [
            "scoreboard objectives add n health",
            "scoreboard objectives modify n displayautoupdate true",
            "team add test",
            r#"team modify test prefix {"text":"[T] "}"#,
            "team join test Alice",
            r#"scoreboard players display name Alice n {"text":"Old"}"#,
            "scoreboard players display numberformat Alice n blank",
        ] {
            commands::execute(&mut board, line, None, 4, &[]).unwrap();
        }
        assert_eq!(
            board.player_score_info("Alice", "n").unwrap().display,
            Some(Component::literal("Old"))
        );
        assert_eq!(
            board.player_score_info("Alice", "n").unwrap().number_format,
            Some(NumberFormat::Blank)
        );
        let holder = player_holder(&board, "Alice", [7; 16]);
        hooks::update_score_for_criteria(&mut board, &holder, "health", 19).unwrap();
        let display = board
            .player_score_info("Alice", "n")
            .unwrap()
            .display
            .as_ref()
            .unwrap();
        assert_eq!(Some(display), holder.display_name.as_ref());
        assert_eq!(display.style.insertion.as_deref(), Some("Alice"));
        assert!(
            matches!(display.style.hover_event,Some(HoverEvent::PlayerEntity{uuid,..}) if uuid == [7;16])
        );
        assert_eq!(Component::from_nbt(&display.to_nbt()).unwrap(), *display);
        for line in [
            "scoreboard objectives add button trigger",
            "scoreboard objectives modify button displayautoupdate true",
            r#"scoreboard players display name Alice button {"text":"Old"}"#,
            "scoreboard players enable Alice button",
        ] {
            commands::execute(&mut board, line, None, 4, &[]).unwrap();
        }
        commands::execute_as(&mut board, "trigger button add 3", Some(&holder), 0, &[]).unwrap();
        let trigger = board.player_score_info("Alice", "button").unwrap();
        assert_eq!(trigger.display.as_ref(), holder.display_name.as_ref());
        assert_eq!(trigger.value, 3);
        assert!(trigger.locked);
        commands::execute(
            &mut board,
            "scoreboard objectives add manual dummy",
            None,
            4,
            &[],
        )
        .unwrap();
        commands::execute(
            &mut board,
            "scoreboard objectives modify manual displayautoupdate true",
            None,
            4,
            &[],
        )
        .unwrap();
        commands::execute_as(
            &mut board,
            "scoreboard players set Alice manual 7",
            None,
            4,
            &[holder.clone()],
        )
        .unwrap();
        assert_eq!(
            board
                .player_score_info("Alice", "manual")
                .unwrap()
                .display
                .as_ref(),
            holder.display_name.as_ref()
        );
        let loaded = save::load_scoreboard(&save::save_scoreboard(&board)).unwrap();
        assert_eq!(
            loaded.player_score_info("Alice", "n"),
            board.player_score_info("Alice", "n")
        );
        assert!(commands::execute(
            &mut board,
            "scoreboard players display invalid Alice n",
            None,
            4,
            &[]
        )
        .is_err());
    }
}
