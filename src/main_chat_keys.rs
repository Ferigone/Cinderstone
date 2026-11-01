//! Actual dedicated server service-key owner and configured chat registry holder.
use super::*;
use mc121_rust::chat_crypto::{ServiceKeyRefresh, ServiceKeySet};

pub(super) fn start_key_refresh(server: Arc<Server>) {
    tokio::spawn(async move {
        let mut refresh = ServiceKeyRefresh::new();
        loop {
            if server.shutting_down.load(Ordering::Acquire) { return; }
            let fetched = match ServiceKeySet::fetch_official().await {
                Ok(keys) => Some(keys),
                Err(error) => {
                    eprintln!("{}", json!({"event":"chat_service_keys_unavailable","reason":error.to_string()}));
                    None
                }
            };
            let delay = refresh.record_attempt(fetched);
            if let Some(keys) = refresh.current() {
                server.chat_keys.send_replace(Arc::new(keys.clone()));
            }
            server.chat_keys_attempted.send_replace(true);
            println!("{}", json!({"event":"chat_service_keys_attempt_complete",
                "profile_keys":server.chat_keys.borrow().profile_certificate_key_count(),
                "next_refresh_seconds":delay.as_secs()}));
            time::sleep(delay).await;
        }
    });
}

impl Server {
    pub(super) fn current_secure_chat_enforcement(&self) -> bool {
        self.options.enforce_secure_profile && self.options.online_mode
            && self.chat_keys.borrow().can_validate_profile_keys()
    }
    pub(super) async fn wait_first_chat_key_attempt(&self) -> Result<()> {
        let mut ready = self.chat_keys_attempted.subscribe();
        loop {
            if *ready.borrow_and_update() { return Ok(()); }
            ready.changed().await.context("service-key owner closed before first attempt")?;
        }
    }
}

/// Resolve the real minecraft:chat holder from the registry bytes sent to this
/// client. An absent/omitted registry is not replaced with a guessed ID.
pub(super) fn captured_chat_type(configuration: &[Packet]) -> Result<i32> {
    for packet in configuration.iter().filter(|packet| packet.id == 0x07) {
        let mut reader = Reader::new(&packet.payload);
        if reader.string(32767)? != "minecraft:chat_type" { continue; }
        let count = reader.varint()?;
        ensure!((0..=65536).contains(&count), "invalid chat registry entry count");
        let mut chat = None;
        for index in 0..count {
            let name = reader.string(32767)?;
            let present = reader.bool()?;
            if present {
                let (_, consumed) = Tag::decode_anonymous_prefix(reader.remaining())?;
                reader.bytes(consumed)?;
            }
            if name == "minecraft:chat" {
                ensure!(present, "chat holder has no configured serialized data");
                ensure!(chat.replace(index).is_none(), "duplicate chat registry key");
            }
        }
        reader.finish()?;
        return chat.context("configured minecraft:chat holder missing");
    }
    bail!("configured chat-type registry missing")
}
