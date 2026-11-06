//! Immutable, explicit seed provenance and private /seed feedback.
use super::*;
use mc121_rust::{commands::seed_command, components::text::Component};

fn imported_seed(root: &Tag) -> Option<i64> {
    match root.get("Data")?.get("WorldGenSettings")?.get("seed")? {
        Tag::Long(seed) => Some(*seed),
        _ => None,
    }
}

/// The declared canonical native profile is seed0 (see the parity scenario).
/// The hash only checks that declaration; it is never inverted or returned.
pub(super) fn world_seed(level: Option<&LevelData>, fixture_hash: i64) -> Option<i64> {
    match level {
        Some(level) => imported_seed(&level.document().tag),
        None => (fixture_hash == world::obfuscate_biome_seed(0)).then_some(0),
    }
}

impl Server {
    pub(super) fn execute_seed_command(&self, line: &str, source: Option<i32>, permission: u8) -> Result<()> {
        let (component, result, error) = match seed_command::execute(line, permission, self.world_seed) {
            Ok(response) => (response.component, Some(response.result), false),
            Err(error) => (Component::from_json(&json!({"text":error.to_string(),"color":"red"}))
                .map_err(|error| anyhow!("{error}"))?, None, true),
        };
        if let Some(id) = source {
            let send_feedback = self.players.read().map_err(|_|anyhow!("players poisoned"))?
                .world_context.rules.boolean("sendCommandFeedback").context("sendCommandFeedback rule missing")?;
            if error || send_feedback {
                let bytes = component.to_network_nbt().map_err(|error| anyhow!("{error}"))?;
                let packet = Packet { id: 0x6c, payload: encoded(|writer| {
                    writer.raw(&bytes).bool(false);
                }) };
                let _ = self.entity_events.send(PlayerEvent::WorldAudience {
                    recipients: Arc::new(vec![id]), packets: Arc::new(vec![packet]),
                });
            }
        } else {
            println!("{}", json!({"event":"administration_feedback","component":component.to_json(),"result":result,"error":error}));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_seed_is_not_a_hash_or_a_missing_metadata_default() {
        assert_eq!(world_seed(None, world::obfuscate_biome_seed(0)), Some(0));
        assert_eq!(world_seed(None, world::obfuscate_biome_seed(123)), None);
        for seed in [i64::MIN, -1234567890123456789, 0, i64::MAX] {
            let root = Tag::Compound(vec![("Data".into(), Tag::Compound(vec![("WorldGenSettings".into(),
                Tag::Compound(vec![("seed".into(), Tag::Long(seed))]))]))]);
            assert_eq!(imported_seed(&root), Some(seed));
        }
        assert_eq!(imported_seed(&Tag::Compound(vec![])), None);
    }
}
