//! Real connection CONFIGURATION phase. The encrypted/framed reader remains
//! owned by the same outer connection pump throughout PLAY/configuration.
use super::*;
use mc121_rust::{
    connection_cookies::{CookieResponse, UNEXPECTED_COOKIE_RESPONSE_REASON},
    reconfiguration::{
        ConfigurationInput, ConfigurationPlan, ConfigurationTask, ConnectionPhase, ConnectionTick,
        Reconfiguration, TaskStart, SERVERBOUND_PLAY_CONFIGURATION_ACKNOWLEDGED,
    },
    resource_packs::{
        ResourcePackAction, ResourcePackPhase, ResourcePackResponse, ResourcePackSession,
        REQUIRED_PACK_DISCONNECT,
    },
};

async fn disconnect(
    codec: &FrameCodec,
    writer: &mut NetworkWriter,
    translation: &str,
) -> Result<()> {
    let reason = Tag::Compound(vec![("translate".into(), Tag::String(translation.into()))])
        .encode_network_anonymous()?;
    // Even while waiting for the PLAY acknowledgement the server has already
    // changed its OUTBOUND protocol to CONFIGURATION.
    send(codec, writer, 0x02, reason).await
}

fn validate_custom_payload(payload: &[u8]) -> Result<()> {
    let _ = mc121_rust::connection_payload::decode_custom_payload(payload)?;
    Ok(())
}

fn validate_ignored_play_common(packet: &Packet) -> Result<()> {
    ensure!(
        (0x00..=0x39).contains(&packet.id),
        "unknown late PLAY packet {}",
        packet.id
    );
    match packet.id {
        0x0a => {
            let _ = read_client_information(&packet.payload)?;
        }
        0x11 => {
            let _ = CookieResponse::decode(&packet.payload)?;
        }
        0x12 => validate_custom_payload(&packet.payload)?,
        0x18 | 0x21 => {
            let mut fields = Reader::new(&packet.payload);
            fields.i64()?;
            fields.finish()?;
        }
        0x27 => { let _ = mc121_rust::connection_payload::decode_pong(&packet.payload)?; }
        0x2b => {
            let _ = ResourcePackResponse::decode(&packet.payload)?;
        }
        // The source listener rejects gameplay dispatch at this boundary.
        // Main's raw frame pump does not yet contain every PLAY body decoder;
        // this does not claim malformed late-gameplay packet equivalence.
        _ => {}
    }
    Ok(())
}

async fn start_task(
    task: TaskStart,
    codec: &FrameCodec,
    writer: &mut NetworkWriter,
    server: &Server,
    packs: &mut ResourcePackSession,
) -> Result<()> {
    let packet = match task {
        TaskStart::SynchronizeRegistries(packet) | TaskStart::JoinWorld(packet) => packet,
        TaskStart::ResourcePack => {
            let offer = server
                .options
                .resource_pack
                .as_ref()
                .context("resource-pack task has no actual configured offer")?;
            packs.begin_configuration(offer)?
        }
    };
    send(codec, writer, packet.id, packet.payload).await
}

pub(super) async fn configure(
    rx: &mut mpsc::Receiver<Result<Packet>>,
    writer: &mut NetworkWriter,
    codec: &FrameCodec,
    server: &Server,
    protocol: &mut Reconfiguration,
    packs: &mut ResourcePackSession,
    mut information: Option<ClientInformation>,
    initial: bool,
) -> Result<Option<ClientInformation>> {
    main_connection::phase(main_connection::OutboundPhase::Configuration);
    let now = || server.epoch.elapsed().as_millis() as i64;
    let mut ticks = server.tick_events.subscribe();
    if initial {
        ensure!(
            protocol.phase() == ConnectionPhase::Configuration,
            "initial configuration listener absent"
        );
        send(codec, writer, 0x01, encoded(|fields| {
            fields.string("minecraft:brand").string("Cinderstone");
        })).await?;
        // Feature flags precede known-pack selection in startConfiguration.
        // Registry/tag bytes are emitted only after the actual selection reply.
        for packet in &server.fixtures.configuration {
            if packet.id == 0x0c {
                send(codec, writer, packet.id, &packet.payload).await?;
            }
        }
        // The captured host owns complete serialized registry entries, not a
        // dynamic known-pack omission encoder. Offer no known packs rather
        // than claim client-side data ownership that this host cannot prove.
        let plan = ConfigurationPlan::synchronize_registries(
            Vec::new(),
            server.options.resource_pack.is_some(),
        )?;
        let task = protocol.begin_tasks(plan, now())?;
        start_task(task, codec, writer, server, packs).await?;
    } else {
        ensure!(
            protocol.phase() == ConnectionPhase::WaitingForConfigurationAck,
            "reconfiguration was not requested by the actual PLAY owner"
        );
    }

    loop {
        if server.shutting_down.load(Ordering::Acquire) {
            disconnect(codec, writer, "multiplayer.disconnect.server_shutdown").await?;
            bail!("server stopped during configuration");
        }
        tokio::select! {
            _ = main_connection::wait_terminal() => {
                main_connection::disconnect_rate(codec, writer).await?;
                bail!("{}",mc121_rust::login_limits::EXCEEDED_PACKET_RATE_REASON);
            }
            tick = ticks.changed() => {
                tick.context("server tick stream closed during configuration")?;
                let time = *ticks.borrow_and_update();
                if let Some(action) = protocol.tick(time) {
                    match action {
                        ConnectionTick::Send(packet) => send(codec,writer,packet.id,packet.payload).await?,
                        ConnectionTick::DisconnectTimeout => {
                            disconnect(codec,writer,"disconnect.timeout").await?;
                            bail!("configuration keepalive timed out");
                        }
                    }
                }
            }
            packet = rx.recv() => {
                let packet = packet.context("connection closed during configuration")??;
                if protocol.phase() == ConnectionPhase::WaitingForConfigurationAck {
                    main_connection_decode::validate_play(&packet)?;
                    if packet.id == SERVERBOUND_PLAY_CONFIGURATION_ACKNOWLEDGED {
                        protocol.acknowledge_configuration(&packet,now())?;
                        main_connection::accepted()?;
                        // returnToWorld is intentionally distinct from a fresh
                        // startConfiguration: no registry/world packet replay.
                        let task = protocol.begin_tasks(ConfigurationPlan::return_to_world(),now())?;
                        start_task(task,codec,writer,server,packs).await?;
                    } else {
                        // aru.shouldHandleMessage allows only the terminal
                        // acknowledgement while waiting. Other decoded PLAY
                        // packets, including common replies, are discarded.
                        validate_ignored_play_common(&packet)?;
                    }
                    continue;
                }
                main_connection_decode::validate_configuration(&packet)?;
                main_connection::accepted()?;
                // Keep decoder failure separate from the common listener's
                // timeout for a well-formed unexpected keepalive response.
                if packet.id == 0x04 && packet.payload.len() == 8 {
                    if protocol.receive_configuration(&packet,now()).is_err() {
                        disconnect(codec,writer,"disconnect.timeout").await?;
                        bail!("configuration keepalive response was not pending");
                    }
                    continue;
                }
                match protocol.receive_configuration(&packet,now())? {
                    ConfigurationInput::ClientInformation(payload) => {
                        information=Some(read_client_information(payload)?);
                    }
                    ConfigurationInput::CookieResponse(payload) => {
                        let _=CookieResponse::decode(payload)?;
                        disconnect(codec,writer,UNEXPECTED_COOKIE_RESPONSE_REASON).await?;
                        bail!("unexpected configuration cookie response");
                    }
                    ConfigurationInput::CustomPayload(payload) => validate_custom_payload(payload)?,
                    ConfigurationInput::KeepAlive { .. } | ConfigurationInput::Pong(_) => {}
                    ConfigurationInput::ResourcePackResponse(payload) => {
                        // The base common-listener required-pack rule runs
                        // before task completion, even for an unexpected UUID
                        // or a terminal response with no current pack task.
                        let response=ResourcePackResponse::decode(payload)?;
                        if server.options.resource_pack_required && response.action==ResourcePackAction::Declined {
                            disconnect(codec,writer,REQUIRED_PACK_DISCONNECT).await?;
                            bail!("required resource pack was declined");
                        }
                        let outcome=packs.handle_response(ResourcePackPhase::Configuration,payload)?;
                        if let Some(reason)=outcome.disconnect_reason {
                            disconnect(codec,writer,reason).await?;
                            bail!("resource-pack listener disconnected during configuration");
                        }
                        if outcome.configuration_finished {
                            if let Some(task)=protocol.complete_task(ConfigurationTask::ResourcePack,now())? {
                                start_task(task,codec,writer,server,packs).await?;
                            }
                        }
                    }
                    ConfigurationInput::KnownPacks(selection) => {
                        ensure!(selection.accepted_packs.is_empty(),"captured registry host cannot omit unencoded known-pack entries");
                        for packet in &server.fixtures.configuration {
                            if matches!(packet.id,0x07|0x0d) {
                                send(codec,writer,packet.id,&packet.payload).await?;
                            }
                        }
                        if let Some(task)=protocol.complete_task(ConfigurationTask::SynchronizeRegistries,now())? {
                            start_task(task,codec,writer,server,packs).await?;
                        }
                    }
                    ConfigurationInput::ReadyForPlayAdmission => {
                        main_connection::phase(main_connection::OutboundPhase::Play);
                        return Ok(information);
                    }
                }
            }
        }
    }
}
