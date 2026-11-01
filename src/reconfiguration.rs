//! Connection-owned Java 1.21 configuration/listener transitions.
//!
//! This module owns protocol direction changes, task ordering and the common
//! listener keepalive. The host owns the authenticated profile, actual player
//! removal/admission, registry encoding, resource packs, cookies and transport.
//! No world snapshot or synthetic player is manufactured by a protocol change.

use crate::{
    protocol::{Packet, Reader, Writer},
    session::{KeepAlive, KeepAliveAction},
};
use anyhow::{bail, ensure, Result};
use std::collections::{BTreeSet, VecDeque};

pub const CLIENTBOUND_PLAY_START_CONFIGURATION: i32 = 0x69;
pub const SERVERBOUND_PLAY_CONFIGURATION_ACKNOWLEDGED: i32 = 0x0c;
pub const CLIENTBOUND_CONFIGURATION_FINISH: i32 = 0x03;
pub const SERVERBOUND_CONFIGURATION_FINISH: i32 = 0x03;
pub const CLIENTBOUND_CONFIGURATION_SELECT_KNOWN_PACKS: i32 = 0x0e;
pub const SERVERBOUND_CONFIGURATION_SELECT_KNOWN_PACKS: i32 = 0x07;
pub const MAX_SELECTED_KNOWN_PACKS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolState {
    Play,
    Configuration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionPhase {
    Play,
    WaitingForConfigurationAck,
    Configuration,
    WaitingForFinishAck,
    ReadyForPlayAdmission,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigurationTask {
    SynchronizeRegistries,
    ResourcePack,
    JoinWorld,
}

/// KnownPack is three UTF strings, not a ResourceLocation. Java compares the
/// complete ordered response list with the complete offered list.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KnownPack {
    pub namespace: String,
    pub id: String,
    pub version: String,
}

impl KnownPack {
    fn validate(&self) -> Result<()> {
        for value in [&self.namespace, &self.id, &self.version] {
            ensure!(
                value.encode_utf16().count() <= 32_767 && value.len() <= 32_767 * 3,
                "known-pack string exceeds Java UTF limit"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnownPackSelection {
    pub client_packs: Vec<KnownPack>,
    pub matches_offer: bool,
    /// An ordered-list mismatch selects the empty known-pack set; it does not
    /// accept an arbitrary subset supplied by the client.
    pub accepted_packs: BTreeSet<KnownPack>,
}

pub fn select_known_packs_packet(packs: &[KnownPack]) -> Result<Packet> {
    let length = i32::try_from(packs.len())?;
    for pack in packs {
        pack.validate()?;
    }
    let mut writer = Writer::new();
    writer.varint(length);
    for pack in packs {
        writer
            .string(&pack.namespace)
            .string(&pack.id)
            .string(&pack.version);
    }
    Ok(Packet {
        id: CLIENTBOUND_CONFIGURATION_SELECT_KNOWN_PACKS,
        payload: writer.into_inner(),
    })
}

pub fn read_selected_known_packs(payload: &[u8]) -> Result<Vec<KnownPack>> {
    let mut reader = Reader::new(payload);
    let length = reader.varint()?;
    ensure!(
        (0..=MAX_SELECTED_KNOWN_PACKS as i32).contains(&length),
        "selected known-pack count exceeds Java limit"
    );
    let mut packs = Vec::with_capacity(length as usize);
    for _ in 0..length {
        packs.push(KnownPack {
            namespace: reader.string(32_767)?,
            id: reader.string(32_767)?,
            version: reader.string(32_767)?,
        });
    }
    reader.finish()?;
    Ok(packs)
}

/// The two Java entry points are distinct: startConfiguration synchronizes
/// registries and optional resources; returnToWorld queues only JoinWorld.
#[derive(Clone, Debug)]
pub struct ConfigurationPlan {
    tasks: VecDeque<ConfigurationTask>,
    offered_packs: Vec<KnownPack>,
}

impl ConfigurationPlan {
    pub fn synchronize_registries(
        offered_packs: Vec<KnownPack>,
        resource_pack: bool,
    ) -> Result<Self> {
        // Preflight packet fields before changing the listener task owner.
        let _ = select_known_packs_packet(&offered_packs)?;
        let mut tasks = VecDeque::from([ConfigurationTask::SynchronizeRegistries]);
        if resource_pack {
            tasks.push_back(ConfigurationTask::ResourcePack);
        }
        tasks.push_back(ConfigurationTask::JoinWorld);
        Ok(Self {
            tasks,
            offered_packs,
        })
    }

    pub fn return_to_world() -> Self {
        Self {
            tasks: VecDeque::from([ConfigurationTask::JoinWorld]),
            offered_packs: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum TaskStart {
    SynchronizeRegistries(Packet),
    /// The actual resource-pack owner supplies its request packet and handles
    /// acceptance, download, final status and mandatory-pack disconnection.
    ResourcePack,
    JoinWorld(Packet),
}

#[derive(Clone, Debug)]
pub enum ConfigurationInput<'a> {
    ClientInformation(&'a [u8]),
    CookieResponse(&'a [u8]),
    CustomPayload(&'a [u8]),
    KeepAlive { latency: Option<i32> },
    Pong(i32),
    ResourcePackResponse(&'a [u8]),
    KnownPacks(KnownPackSelection),
    ReadyForPlayAdmission,
}

#[derive(Clone, Debug)]
pub enum ConnectionTick {
    Send(Packet),
    DisconnectTimeout,
}

#[derive(Clone, Debug)]
pub struct Reconfiguration {
    phase: ConnectionPhase,
    keep_alive: KeepAlive,
    queued_tasks: VecDeque<ConfigurationTask>,
    current_task: Option<ConfigurationTask>,
    offered_packs: Vec<KnownPack>,
    selected_packs: Option<KnownPackSelection>,
    plan_started: bool,
}

impl Reconfiguration {
    /// Move the actual existing PLAY keepalive here, preserving an outstanding
    /// challenge and latency until the old listener is replaced.
    pub fn from_play(keep_alive: KeepAlive) -> Self {
        Self::new(ConnectionPhase::Play, keep_alive)
    }

    pub fn initial_configuration(now_ms: i64, latency: i32) -> Self {
        Self::new(
            ConnectionPhase::Configuration,
            KeepAlive::with_latency(now_ms, latency),
        )
    }

    fn new(phase: ConnectionPhase, keep_alive: KeepAlive) -> Self {
        Self {
            phase,
            keep_alive,
            queued_tasks: VecDeque::new(),
            current_task: None,
            offered_packs: Vec::new(),
            selected_packs: None,
            plan_started: false,
        }
    }

    pub fn phase(&self) -> ConnectionPhase {
        self.phase
    }

    pub fn inbound_protocol(&self) -> Option<ProtocolState> {
        match self.phase {
            ConnectionPhase::Play | ConnectionPhase::WaitingForConfigurationAck => {
                Some(ProtocolState::Play)
            }
            ConnectionPhase::Configuration | ConnectionPhase::WaitingForFinishAck => {
                Some(ProtocolState::Configuration)
            }
            ConnectionPhase::ReadyForPlayAdmission | ConnectionPhase::Closed => None,
        }
    }

    pub fn outbound_protocol(&self) -> Option<ProtocolState> {
        match self.phase {
            ConnectionPhase::Play | ConnectionPhase::ReadyForPlayAdmission => {
                Some(ProtocolState::Play)
            }
            ConnectionPhase::WaitingForConfigurationAck
            | ConnectionPhase::Configuration
            | ConnectionPhase::WaitingForFinishAck => Some(ProtocolState::Configuration),
            ConnectionPhase::Closed => None,
        }
    }

    pub fn current_task(&self) -> Option<ConfigurationTask> {
        self.current_task
    }

    pub fn keep_alive(&self) -> &KeepAlive {
        &self.keep_alive
    }

    pub fn keep_alive_mut(&mut self) -> &mut KeepAlive {
        &mut self.keep_alive
    }

    /// The host must remove/save the actual world player before calling this.
    /// Send the returned terminal packet with the OLD PLAY outbound protocol;
    /// all subsequent outbound packets use CONFIGURATION.
    pub fn begin_reconfiguration(&mut self, now_ms: i64) -> Result<Packet> {
        ensure!(
            self.phase == ConnectionPhase::Play,
            "configuration switch already pending"
        );
        self.keep_alive.close(now_ms);
        self.phase = ConnectionPhase::WaitingForConfigurationAck;
        Ok(Packet {
            id: CLIENTBOUND_PLAY_START_CONFIGURATION,
            payload: Vec::new(),
        })
    }

    pub fn acknowledge_configuration(&mut self, packet: &Packet, now_ms: i64) -> Result<()> {
        ensure!(
            self.phase == ConnectionPhase::WaitingForConfigurationAck,
            "client acknowledged configuration, but none was requested"
        );
        ensure!(
            packet.id == SERVERBOUND_PLAY_CONFIGURATION_ACKNOWLEDGED && packet.payload.is_empty(),
            "expected empty PLAY configuration acknowledgement"
        );
        self.keep_alive = KeepAlive::with_latency(now_ms, self.keep_alive.latency());
        self.phase = ConnectionPhase::Configuration;
        self.plan_started = false;
        self.queued_tasks.clear();
        self.current_task = None;
        self.offered_packs.clear();
        self.selected_packs = None;
        Ok(())
    }

    /// Send brand/features/other actual prelude packets before starting the
    /// registry task, as ServerConfigurationPacketListenerImpl does.
    pub fn begin_tasks(&mut self, plan: ConfigurationPlan, now_ms: i64) -> Result<TaskStart> {
        ensure!(
            self.phase == ConnectionPhase::Configuration && !self.plan_started,
            "configuration tasks already started or listener is not configured"
        );
        ensure!(
            self.current_task.is_none(),
            "configuration task already active"
        );
        self.plan_started = true;
        self.queued_tasks = plan.tasks;
        self.offered_packs = plan.offered_packs;
        self.selected_packs = None;
        self.start_next_task(now_ms)?
            .ok_or_else(|| anyhow::anyhow!("configuration plan has no task"))
    }

    pub fn selected_packs(&self) -> Option<&KnownPackSelection> {
        self.selected_packs.as_ref()
    }

    /// Complete registry synchronization only after its actual reply and after
    /// the host has emitted registry/tag data using that accepted-pack set.
    /// A resource pack completes only for its owner's final response. JoinWorld
    /// completes through the client's empty finish acknowledgement instead.
    pub fn complete_task(
        &mut self,
        task: ConfigurationTask,
        now_ms: i64,
    ) -> Result<Option<TaskStart>> {
        ensure!(
            self.phase == ConnectionPhase::Configuration,
            "configuration task not active"
        );
        ensure!(
            self.current_task == Some(task),
            "unexpected configuration task completion"
        );
        ensure!(
            task != ConfigurationTask::JoinWorld,
            "JoinWorld requires finish acknowledgement"
        );
        if task == ConfigurationTask::SynchronizeRegistries {
            ensure!(
                self.selected_packs.is_some(),
                "known-pack negotiation has not completed"
            );
        }
        self.current_task = None;
        self.start_next_task(now_ms)
    }

    fn start_next_task(&mut self, now_ms: i64) -> Result<Option<TaskStart>> {
        ensure!(
            self.current_task.is_none(),
            "previous configuration task still active"
        );
        let Some(task) = self.queued_tasks.front().copied() else {
            return Ok(None);
        };
        let start = match task {
            ConfigurationTask::SynchronizeRegistries => {
                TaskStart::SynchronizeRegistries(select_known_packs_packet(&self.offered_packs)?)
            }
            ConfigurationTask::ResourcePack => TaskStart::ResourcePack,
            ConfigurationTask::JoinWorld => TaskStart::JoinWorld(Packet {
                id: CLIENTBOUND_CONFIGURATION_FINISH,
                payload: Vec::new(),
            }),
        };
        self.queued_tasks.pop_front();
        self.current_task = Some(task);
        if task == ConfigurationTask::JoinWorld {
            self.keep_alive.close(now_ms);
            self.phase = ConnectionPhase::WaitingForFinishAck;
        }
        Ok(Some(start))
    }

    /// Only IDs registered in the pinned 1.21 CONFIGURATION serverbound codec
    /// are accepted. Cookie/resource-pack/custom/settings bodies belong to the
    /// existing typed host handlers; successful routing does not validate them.
    pub fn receive_configuration<'a>(
        &mut self,
        packet: &'a Packet,
        now_ms: i64,
    ) -> Result<ConfigurationInput<'a>> {
        ensure!(
            matches!(
                self.phase,
                ConnectionPhase::Configuration | ConnectionPhase::WaitingForFinishAck
            ),
            "serverbound packet reached inactive configuration listener"
        );
        match packet.id {
            0x00 => Ok(ConfigurationInput::ClientInformation(&packet.payload)),
            0x01 => Ok(ConfigurationInput::CookieResponse(&packet.payload)),
            0x02 => Ok(ConfigurationInput::CustomPayload(&packet.payload)),
            SERVERBOUND_CONFIGURATION_FINISH => {
                ensure!(
                    self.phase == ConnectionPhase::WaitingForFinishAck
                        && self.current_task == Some(ConfigurationTask::JoinWorld)
                        && self.queued_tasks.is_empty(),
                    "client finished an unexpected configuration task"
                );
                ensure!(
                    packet.payload.is_empty(),
                    "configuration finish has trailing bytes"
                );
                self.current_task = None;
                self.phase = ConnectionPhase::ReadyForPlayAdmission;
                Ok(ConfigurationInput::ReadyForPlayAdmission)
            }
            0x04 => {
                let mut fields = Reader::new(&packet.payload);
                let id = fields.i64()?;
                fields.finish()?;
                let latency = self.keep_alive.receive(now_ms, id)?;
                Ok(ConfigurationInput::KeepAlive { latency })
            }
            0x05 => {
                let mut fields = Reader::new(&packet.payload);
                let id = fields.i32()?;
                fields.finish()?;
                Ok(ConfigurationInput::Pong(id))
            }
            0x06 => Ok(ConfigurationInput::ResourcePackResponse(&packet.payload)),
            SERVERBOUND_CONFIGURATION_SELECT_KNOWN_PACKS => {
                ensure!(
                    self.current_task == Some(ConfigurationTask::SynchronizeRegistries)
                        && self.selected_packs.is_none(),
                    "received pack selection, but no negotiation is ongoing"
                );
                let client_packs = read_selected_known_packs(&packet.payload)?;
                let matches_offer = client_packs == self.offered_packs;
                let accepted_packs = if matches_offer {
                    self.offered_packs.iter().cloned().collect()
                } else {
                    BTreeSet::new()
                };
                let selection = KnownPackSelection {
                    client_packs,
                    matches_offer,
                    accepted_packs,
                };
                self.selected_packs = Some(selection.clone());
                Ok(ConfigurationInput::KnownPacks(selection))
            }
            _ => bail!("unknown configuration packet {}", packet.id),
        }
    }

    /// Call only after the actual PlayerList-style admission succeeds. Profile,
    /// inventory, world position, tickets and entity IDs belong to that owner.
    pub fn complete_play_admission(&mut self, now_ms: i64) -> Result<()> {
        ensure!(
            self.phase == ConnectionPhase::ReadyForPlayAdmission,
            "PLAY admission is not ready"
        );
        self.keep_alive = KeepAlive::with_latency(now_ms, self.keep_alive.latency());
        self.phase = ConnectionPhase::Play;
        self.plan_started = false;
        self.offered_packs.clear();
        self.selected_packs = None;
        Ok(())
    }

    pub fn tick(&mut self, now_ms: i64) -> Option<ConnectionTick> {
        if self.phase == ConnectionPhase::Closed {
            return None;
        }
        match self.keep_alive.tick(now_ms)? {
            KeepAliveAction::Timeout => Some(ConnectionTick::DisconnectTimeout),
            KeepAliveAction::Challenge(id) => {
                let packet_id = match self.outbound_protocol()? {
                    ProtocolState::Play => 0x26,
                    ProtocolState::Configuration => 0x04,
                };
                let mut writer = Writer::new();
                writer.i64(id);
                Some(ConnectionTick::Send(Packet {
                    id: packet_id,
                    payload: writer.into_inner(),
                }))
            }
        }
    }

    pub fn close(&mut self) {
        self.phase = ConnectionPhase::Closed;
        self.queued_tasks.clear();
        self.current_task = None;
    }
}
