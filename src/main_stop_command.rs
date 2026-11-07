// SPDX-License-Identifier: GPL-2.0-only
//! Ordered stop request and bounded PLAY drain before the existing final save.
use super::*;
impl Server {
    /// Caller retains chat/admission serialization; shutdown itself runs later
    /// in the real server loop, after those guards have been released.
    pub(super) fn try_stop_command(&self, line: &str, source: Option<i32>) -> Result<bool> {
        let input = if source.is_none() {
            line.strip_prefix('/').unwrap_or(line)
        } else {
            line
        };
        if input.split_whitespace().next() != Some("stop") {
            return Ok(false);
        }
        ensure!(
            !mc121_rust::command_protocol::command_contains_illegal_characters(input),
            "illegal command characters"
        );
        let permission = if let Some(id) = source {
            let uuid = self
                .players
                .read()
                .map_err(|_| anyhow!("players poisoned"))?
                .registry
                .get(id)
                .context("stop source absent")?
                .profile
                .uuid;
            self.administration
                .read()
                .map_err(|_| anyhow!("administration poisoned"))?
                .permission_level(uuid)
                .clamp(0, 4) as u8
        } else {
            4
        };
        match mc121_rust::commands::stop_command::parse(input, permission) {
            Ok(result) => {
                self.save_command_feedback(
                    source,
                    json!({"translate":"commands.stop.stopping"}),
                    true,
                    false,
                    Some(result),
                )?;
                self.stop_requested.notify_one();
            }
            Err(error) => self.save_command_feedback(
                source,
                json!({"text":error.message(),"color":"red"}),
                false,
                true,
                None,
            )?,
        }
        Ok(true)
    }
    pub(super) async fn graceful_shutdown(&self) -> Result<()> {
        {
            let _sequence = self.chat_dispatch.lock().await;
            let _admission = self.player_admission.lock().await;
            self.shutting_down.store(true, Ordering::Release);
            // Complete already accepted world callbacks without another game tick.
            self.drain_world_interaction_phase().await?;
            let ids = self
                .players
                .read()
                .map_err(|_| anyhow!("players poisoned"))?
                .registry
                .snapshot()
                .iter()
                .map(|p| p.entity_id)
                .collect::<Vec<_>>();
            for recipient in ids {
                let _ = self.entity_events.send(PlayerEvent::ChatDisconnect {
                    recipient,
                    reason: "multiplayer.disconnect.server_shutdown",
                });
            }
        }
        // PLAY returns only after the ordered disconnect write has flushed;
        // PresenceGuard then invokes the actual menu/player cleanup. Do not hold
        // either lock required by that cleanup while waiting for it.
        let deadline = time::Instant::now() + WRITE_DEADLINE + Duration::from_secs(1);
        loop {
            let remaining = self
                .players
                .read()
                .map_err(|_| anyhow!("players poisoned"))?
                .registry
                .len();
            if remaining == 0 {
                break;
            }
            if time::Instant::now() >= deadline {
                eprintln!(
                    "{}",
                    json!({"event":"shutdown_player_drain_timeout","remaining":remaining})
                );
                // The existing save captures retained actors/menus as well.
                break;
            }
            time::sleep(Duration::from_millis(10)).await;
        }
        self.save().await?;
        println!("{}", json!({"event":"shutdown_saved"}));
        Ok(())
    }
}
