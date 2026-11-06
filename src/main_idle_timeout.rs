// SPDX-License-Identifier: GPL-2.0-only
//! Dedicated action-idle policy over the actual admitted player's monotonic clock.
use super::*;
use main_administration::{AdminEffect, CommandOutcome};
use mc121_rust::{commands::idle_command, admission_commands::{CommandError, CommandMessage, MessageArg}};

impl Server {
    pub(super) fn record_player_action(&self, id: i32) -> Result<()> {
        let mut players = self.players.write().map_err(|_|anyhow!("players poisoned"))?;
        *players.player_last_actions.get_mut(&id).context("last-action player missing")? = self.epoch.elapsed().as_millis() as i64;
        Ok(())
    }
    pub(super) fn player_is_idle(&self, id: i32) -> Result<bool> {
        let minutes = self.administration.read().map_err(|_|anyhow!("administration poisoned"))?.idle_timeout();
        let players = self.players.read().map_err(|_|anyhow!("players poisoned"))?;
        let last = *players.player_last_actions.get(&id).context("last-action player missing")?;
        Ok(idle_command::expired(self.epoch.elapsed().as_millis() as i64, last, minutes))
    }
    pub(super) fn execute_idle_timeout(&self, line: &str, permission: u8) -> Result<CommandOutcome> {
        match idle_command::execute(line, permission) {
            Ok(minutes) => {
                let mut owner = self.administration.write().map_err(|_|anyhow!("administration poisoned"))?;
                let mut effects = owner.set_idle_timeout(minutes);
                effects.push(AdminEffect::Feedback {
                    message: CommandMessage::new("commands.setidletimeout.success", vec![MessageArg::Integer(minutes)]),
                    broadcast_to_ops: true,
                });
                Ok(CommandOutcome { result: Ok(minutes), effects })
            }
            Err(error) => Ok(CommandOutcome { result: Err(CommandError::Host(error.message())), effects: vec![] }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_property_survives_reload_without_replacing_unrelated_properties() {
        let directory = std::env::temp_dir().join(format!("idle-property-{}-{}",std::process::id(),wall_time_ms().unwrap()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("server.properties");
        let original = "# keep this\nunknown.setting=owned elsewhere\nplayer-idle-timeout=bad\n";
        std::fs::write(&path,original).unwrap();
        let load = || main_administration::Administration::load(directory.clone(),Default::default(),1).unwrap();
        let mut owner=load(); assert_eq!(owner.idle_timeout(),0);
        assert!(owner.set_idle_timeout(2).is_empty()); assert_eq!(owner.idle_timeout(),2);
        assert!(std::fs::read_to_string(&path).unwrap().starts_with(original));
        assert_eq!(load().idle_timeout(),2);
        owner.set_idle_timeout(0); assert_eq!(load().idle_timeout(),0);
        std::fs::remove_dir_all(directory).unwrap();
    }
    fn wall_time_ms() -> Result<i64> {main_admin_dispatch::wall_time_ms()}
}
