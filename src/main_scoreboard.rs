// SPDX-License-Identifier: GPL-2.0-only
//! Live scoreboard wiring. The player lock serializes mutations and packet publication.
use super::*;
use mc121_rust::{
    nbt::JavaString,
    saved_data,
    scoreboard::hooks::PlayerVitals,
    scoreboard::{self, DisplaySlot, Outbound, Scoreboard, ScoreboardPacket},
};

pub(super) fn path(options: &Options) -> PathBuf {
    options
        .anvil_world
        .clone()
        .unwrap_or_else(|| options.world.parent().unwrap_or(Path::new(".")).to_owned())
        .join("data/scoreboard.dat")
}
pub(super) fn load(path: &Path) -> Result<Scoreboard> {
    if !path.exists() {
        return Ok(Scoreboard::new_server());
    }
    let root = saved_data::file::read_root(&std::fs::read(path)?).map_err(|e| anyhow!("{e:?}"))?;
    let version = saved_data::file::data_version(&root, 1343);
    ensure!(
        version == saved_data::DATA_VERSION,
        "scoreboard.dat DataVersion {version} requires a data fixer (expected 3953)"
    );
    scoreboard::save::load_scoreboard(&saved_data::file::compound_member(&root, "data"))
        .map_err(|e| anyhow!(e))
}
/// Replace only scoreboard-owned top-level payload fields, retaining extension data.
fn merge(old: Option<Tag>, data: Tag) -> Tag {
    let mut entries = match old {
        Some(Tag::Compound(e)) => e,
        _ => Vec::new(),
    };
    if let Tag::Compound(new) = data {
        for (key, value) in new {
            if let Some((_, v)) = entries.iter_mut().find(|(k, _)| *k == key) {
                *v = value
            } else {
                entries.push((key, value))
            }
        }
    }
    Tag::Compound(entries)
}
pub(super) fn save(path: &Path, board: &mut Scoreboard) -> Result<()> {
    if !board.is_dirty() {
        return Ok(());
    }
    let old = if path.exists() {
        Some(saved_data::file::read_root(&std::fs::read(path)?).map_err(|e| anyhow!("{e:?}"))?)
    } else {
        None
    };
    let previous_data = old
        .as_ref()
        .and_then(|r| r.get("data"))
        .cloned()
        .map(|tag| {
            let Tag::Compound(mut entries) = tag else {
                return tag;
            };
            entries.retain(|(key, _)| {
                !matches!(
                    key.to_string_lossy().as_ref(),
                    "Objectives" | "PlayerScores" | "Teams" | "DisplaySlots"
                )
            });
            Tag::Compound(entries)
        });
    let data = merge(previous_data, scoreboard::save::save_scoreboard(board));
    let root = merge(
        old,
        Tag::Compound(vec![
            (JavaString::from("data"), data),
            (
                JavaString::from("DataVersion"),
                Tag::Int(saved_data::DATA_VERSION),
            ),
        ]),
    );
    let bytes = saved_data::file::write_root(&root).map_err(|e| anyhow!("{e:?}"))?;
    let parent = path.parent().context("scoreboard path has no parent")?;
    std::fs::create_dir_all(parent)?;
    let tmp = path.with_extension("dat.tmp");
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    std::fs::File::open(parent)?.sync_all()?;
    board.set_dirty(false);
    Ok(())
}
fn packet(p: ScoreboardPacket) -> Result<Packet> {
    Ok(Packet {
        id: p.id(),
        payload: p.encode().map_err(|e| anyhow!("{e}"))?,
    })
}
/// PlayerList.updateEntireScoreboard: teams first, then each displayed objective once.
pub(super) fn join_packets(board: &Scoreboard) -> Result<Vec<Packet>> {
    let mut packets = Vec::new();
    for team in board.teams() {
        packets.push(packet(ScoreboardPacket::SetPlayerTeam {
            name: team.name.clone(),
            method: 0,
            parameters: Some(team.parameters()),
            players: team.players().into_iter().map(str::to_owned).collect(),
        })?);
    }
    let mut seen = BTreeSet::new();
    for slot in DisplaySlot::all() {
        if let Some(name) = board.display_objective(slot) {
            if seen.insert(name.to_owned()) {
                for p in board.start_tracking_packets(name)? {
                    packets.push(packet(p)?);
                }
            }
        }
    }
    Ok(packets)
}
pub(super) fn publish(
    players: &mut LivePlayers,
    events: &broadcast::Sender<PlayerEvent>,
) -> Result<()> {
    let recipients = Arc::new(
        players
            .registry
            .snapshot()
            .iter()
            .map(|p| p.entity_id)
            .collect::<Vec<_>>(),
    );
    for outbound in players.gameplay.scoreboard.take_outbox() {
        let packets = match outbound {
            Outbound::Broadcast(p) => vec![packet(p)?],
            Outbound::EachPlayer(ps) => ps.into_iter().map(packet).collect::<Result<Vec<_>>>()?,
        };
        players.scoreboard_revision = players
            .scoreboard_revision
            .checked_add(1)
            .context("scoreboard revision exhausted")?;
        if !recipients.is_empty() {
            let _ = events.send(PlayerEvent::Scoreboard {
                revision: players.scoreboard_revision,
                recipients: Arc::clone(&recipients),
                packets: Arc::new(packets),
            });
        }
    }
    Ok(())
}
pub(super) fn tick(
    players: &mut LivePlayers,
    events: &broadcast::Sender<PlayerEvent>,
) -> Result<()> {
    for player in players.registry.snapshot() {
        let Some(lifecycle) = players.player_lifecycle.get(&player.profile.uuid) else {
            continue;
        };
        let health = match player.entity_data.get(9) {
            Some(MetadataValue::Float(v)) => *v,
            _ => bail!("scoreboard health owner missing"),
        };
        let absorption = match player.entity_data.get(15) {
            Some(MetadataValue::Float(v)) => *v,
            _ => bail!("scoreboard absorption owner missing"),
        };
        let air_supply = match player.entity_data.get(1) {
            Some(MetadataValue::Int(v)) => *v,
            _ => bail!("scoreboard air owner missing"),
        };
        let armor_value = players
            .containers
            .get(&player.entity_id)
            .context("scoreboard inventory owner missing")?
            .read(|m| armor_value(&m.inventory))??;
        let holder = scoreboard::names::player_holder(
            &players.gameplay.scoreboard,
            &player.profile.name,
            player.profile.uuid,
        );
        players
            .scoreboard_trackers
            .entry(player.entity_id)
            .or_default()
            .tick(
                &mut players.gameplay.scoreboard,
                &holder,
                &PlayerVitals {
                    health,
                    absorption,
                    air_supply,
                    armor_value,
                    food_level: lifecycle.food.level,
                    total_experience: lifecycle.experience.total,
                    experience_level: lifecycle.experience.level,
                },
            )?;
    }
    publish(players, events)
}
// Project the admitted, effect-free equipment profile through the existing Java attribute
// arithmetic. Full persistent/effect-modified actor attributes remain WP-S4.
fn armor_value(inventory: &mc121_rust::inventory::PlayerInventory) -> Result<i32> {
    use mc121_rust::effects::{
        attribute_instance::{
            self as ai, AttributeDefinition, AttributeInstance, DirtyListener, Modifier, Operation,
            ResourceLocationKey,
        },
        definitions::BUILTIN_ATTRIBUTE_SPECS,
    };
    use mc121_rust::equipment::{EquipmentSlot, PlayerEquipment};
    #[derive(Clone, PartialEq, Eq)]
    struct Key(String);
    impl ResourceLocationKey for Key {
        fn java_hash_code(&self) -> i32 {
            let (n, p) = self.0.split_once(':').unwrap_or(("minecraft", &self.0));
            ai::resource_location_java_hash(n, p)
        }
    }
    struct Definition;
    impl AttributeDefinition for Definition {
        type Key = str;
        fn key(&self) -> &str {
            BUILTIN_ATTRIBUTE_SPECS[0].name
        }
        fn default_value(&self) -> f64 {
            BUILTIN_ATTRIBUTE_SPECS[0].default_value
        }
        fn sanitize_value(&self, v: f64) -> f64 {
            BUILTIN_ATTRIBUTE_SPECS[0]
                .create_value()
                .expect("pinned armor range")
                .sanitize_value(v)
        }
    }
    struct Listener;
    impl DirtyListener<Definition, Key> for Listener {
        type Error = anyhow::Error;
        fn on_dirty(&mut self, _: &mut AttributeInstance<Definition, Key>) -> Result<()> {
            Ok(())
        }
    }
    let mut armor = AttributeInstance::<Definition, Key>::new(Arc::new(Definition));
    let equipment = PlayerEquipment::from_inventory(inventory)?;
    for slot in EquipmentSlot::ALL {
        let group = match slot {
            EquipmentSlot::MainHand => "main_hand",
            EquipmentSlot::OffHand => "off_hand",
            EquipmentSlot::Feet => "feet",
            EquipmentSlot::Legs => "legs",
            EquipmentSlot::Chest => "chest",
            EquipmentSlot::Head => "head",
            EquipmentSlot::Body => "body",
        };
        if let Some(component) = equipment.get(slot).component_value(12)? {
            for modifier in component["attributes"]
                .as_array()
                .context("invalid attribute component")?
            {
                if modifier["typeId"].as_u64() != Some(0) {
                    continue;
                }
                let apply = modifier["slot"]
                    .as_str()
                    .context("attribute slot missing")?;
                if apply != "any"
                    && apply != group
                    && !(apply == "hand"
                        && matches!(slot, EquipmentSlot::MainHand | EquipmentSlot::OffHand))
                    && !(apply == "armor"
                        && matches!(
                            slot,
                            EquipmentSlot::Feet
                                | EquipmentSlot::Legs
                                | EquipmentSlot::Chest
                                | EquipmentSlot::Head
                        ))
                {
                    continue;
                }
                let operation = match modifier["operation"].as_str() {
                    Some("add") => Operation::AddValue,
                    Some("multiply_base") => Operation::AddMultipliedBase,
                    Some("multiply_total") => Operation::AddMultipliedTotal,
                    _ => bail!("unknown armor operation"),
                };
                armor.add_or_update_transient_modifier(
                    Arc::new(Modifier::new(
                        Key(modifier["name"]
                            .as_str()
                            .context("modifier name missing")?
                            .to_owned()),
                        modifier["value"]
                            .as_f64()
                            .context("modifier amount missing")?,
                        operation,
                    )),
                    &mut Listener,
                )?;
            }
        }
    }
    Ok(armor.value() as i32)
}
impl Server {
    pub(super) fn scoreboard_join_packets(&self) -> Result<(u64, Vec<Packet>)> {
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("players poisoned"))?;
        // Assign revisions to every pending change before selecting the snapshot cutoff.
        publish(&mut players, &self.entity_events)?;
        Ok((
            players.scoreboard_revision,
            join_packets(&players.gameplay.scoreboard)?,
        ))
    }
    pub(super) fn execute_scoreboard(
        &self,
        line: &str,
        source_id: Option<i32>,
        source_name: &str,
        permission: u8,
    ) -> Result<()> {
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("players poisoned"))?;
        let online = players
            .registry
            .snapshot()
            .iter()
            .map(|p| {
                scoreboard::names::player_holder(
                    &players.gameplay.scoreboard,
                    &p.profile.name,
                    p.profile.uuid,
                )
            })
            .collect::<Vec<_>>();
        let source = if let Some(id) = source_id {
            let profile = &players
                .registry
                .get(id)
                .context("scoreboard command source absent")?
                .profile;
            Some(scoreboard::names::player_holder(
                &players.gameplay.scoreboard,
                source_name,
                profile.uuid,
            ))
        } else {
            None
        };
        let result = scoreboard::commands::execute_as(
            &mut players.gameplay.scoreboard,
            line,
            source.as_ref(),
            permission,
            &online,
        );
        publish(&mut players, &self.entity_events)?;
        let (message, error) = match result {
            Ok(v) => (v, false),
            Err(e) => (e.to_string(), true),
        };
        if let Some(id) = source_id {
            if !error
                && !players
                    .world_context
                    .rules
                    .boolean("sendCommandFeedback")
                    .context("sendCommandFeedback rule missing")?
            {
                return Ok(());
            }
            let feedback = mc121_rust::chat_wire::encode_system_chat(
                &json!({"text":message,"color":if error{"red"}else{"white"}}),
                false,
            )?;
            let _ = self.entity_events.send(PlayerEvent::WorldAudience {
                recipients: Arc::new(vec![id]),
                packets: Arc::new(vec![feedback]),
            });
        } else {
            println!(
                "{}",
                json!({"event":"scoreboard_command","command":line,"ok":!error,"message":message})
            );
        }
        Ok(())
    }
}
pub(super) fn decorated_name(board: &Scoreboard, profile: &PlayerProfile) -> serde_json::Value {
    scoreboard::names::player_holder(board, &profile.name, profile.uuid)
        .display_name
        .expect("native player display")
        .to_json()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc121_rust::scoreboard::ScoreHolder;
    #[test]
    fn persisted_board_replays_team_before_objective_and_keeps_extensions() {
        let folder =
            std::env::temp_dir().join(format!("scoreboard-runtime-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("scoreboard.dat");
        let mut board = Scoreboard::new_server();
        for line in [
            "scoreboard objectives add hp health",
            "scoreboard objectives setdisplay sidebar hp",
            "team add blue",
            "team join blue Alice",
        ] {
            scoreboard::commands::execute(&mut board, line, None, 4, &[]).unwrap();
        }
        scoreboard::hooks::update_score_for_criteria(
            &mut board,
            &ScoreHolder::named("Alice"),
            "health",
            19,
        )
        .unwrap();
        let root = Tag::Compound(vec![
            ("extension".into(), Tag::Int(17)),
            (
                "data".into(),
                Tag::Compound(vec![("custom".into(), Tag::Int(7))]),
            ),
        ]);
        std::fs::write(&path, saved_data::file::write_root(&root).unwrap()).unwrap();
        save(&path, &mut board).unwrap();
        let loaded = load(&path).unwrap();
        let packets = join_packets(&loaded).unwrap();
        assert_eq!(
            packets.iter().map(|p| p.id).collect::<Vec<_>>(),
            vec![0x60, 0x5e, 0x57, 0x61]
        );
        assert_eq!(loaded.player_score_info("Alice", "hp").unwrap().value, 19);
        let root = saved_data::file::read_root(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(root.get("extension"), Some(&Tag::Int(17)));
        assert_eq!(root.get("data").unwrap().get("custom"), Some(&Tag::Int(7)));
        board
            .set_display_objective(DisplaySlot::Sidebar, None)
            .unwrap();
        save(&path, &mut board).unwrap();
        assert_eq!(
            load(&path).unwrap().display_objective(DisplaySlot::Sidebar),
            None
        );
        std::fs::remove_dir_all(folder).unwrap();
    }
}
