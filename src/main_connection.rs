//! Actual connection owners shared by the protocol's safe dispatch boundaries.
use super::*;
use main_connection_rate::ConnectionRate;
use mc121_rust::login_limits::PacketRateExceeded;
use std::sync::atomic::AtomicU8;

#[derive(Clone, Copy)]
pub(super) enum OutboundPhase { Handshake = 0, Status = 1, Login = 2, Configuration = 3, Play = 4 }

pub(super) struct ConnectionContext {
    pub rate: ConnectionRate,
    phase: AtomicU8,
    raw_reader: tokio::task::AbortHandle,
    writing: AtomicBool,
}

impl ConnectionContext {
    pub(super) fn new(rate: ConnectionRate, raw_reader: tokio::task::AbortHandle) -> Self {
        Self { rate, phase: AtomicU8::new(OutboundPhase::Handshake as u8), raw_reader, writing: AtomicBool::new(false) }
    }
}

tokio::task_local! { pub(super) static CONNECTION: ConnectionContext; }

pub(super) struct WriteGuard;
pub(super) fn begin_write() -> WriteGuard {
    CONNECTION.with(|connection| connection.writing.store(true, Ordering::Release));
    WriteGuard
}
pub(super) fn writing() -> bool {
    CONNECTION.with(|connection| connection.writing.load(Ordering::Acquire))
}
impl Drop for WriteGuard {
    fn drop(&mut self) {
        let _ = CONNECTION.try_with(|connection| connection.writing.store(false, Ordering::Release));
    }
}

pub(super) fn phase(phase: OutboundPhase) {
    CONNECTION.with(|connection| connection.phase.store(phase as u8, Ordering::Release));
}

pub(super) fn accepted() -> Result<()> {
    CONNECTION.with(|connection| connection.rate.accepted_packet_received())
}

pub(super) fn rate() -> ConnectionRate {
    CONNECTION.with(|connection| connection.rate.clone())
}

pub(super) fn terminal() -> Option<PacketRateExceeded> {
    CONNECTION.with(|connection| *connection.rate.subscribe_terminal().borrow())
}

pub(super) async fn wait_terminal() -> PacketRateExceeded {
    let mut updates = rate().subscribe_terminal();
    loop {
        if let Some(action) = *updates.borrow_and_update() { return action; }
        if updates.changed().await.is_err() { std::future::pending::<()>().await; }
    }
}

/// Execute only at a writer boundary: never cancel a partially written frame
/// and append a new disconnect frame to its unfinished ciphertext.
pub(super) async fn disconnect_rate(codec: &FrameCodec, writer: &mut NetworkWriter) -> Result<()> {
    let (phase, rate) = CONNECTION.with(|connection| {
        (connection.phase.load(Ordering::Acquire), connection.rate.clone())
    });
    rate.packet_send_dispatched()?;
    rate.mark_read_only();
    CONNECTION.with(|connection| connection.raw_reader.abort());
    let packet_id = match phase {
        value if value == OutboundPhase::Configuration as u8 => Some(0x02),
        value if value == OutboundPhase::Play as u8 => Some(0x1d),
        // The original always dispatches a common packet; early protocol
        // encoders cannot encode it. Its failure callback still closes.
        _ => None,
    };
    let result = if let Some(id) = packet_id {
        let reason = Tag::Compound(vec![("translate".into(), Tag::String(
            mc121_rust::login_limits::EXCEEDED_PACKET_RATE_REASON.into(),
        ))]).encode_network_anonymous()?;
        time::timeout(WRITE_DEADLINE, async {
            codec.write(writer, id, &reason).await?;
            writer.flush().await?;
            Ok::<_, anyhow::Error>(())
        }).await.context("rate disconnect write deadline")?
    } else { Ok(()) };
    // PacketSendListener.thenRun closes after success AND failed completion.
    let _ = time::timeout(WRITE_DEADLINE, writer.shutdown()).await;
    result
}

pub(super) struct ConnectionRegistration { pub server: Arc<Server>, pub admission: u64 }
impl Drop for ConnectionRegistration {
    fn drop(&mut self) {
        if let Ok(mut connections) = self.server.connection_rates.write() {
            connections.remove(&self.admission);
        }
    }
}
