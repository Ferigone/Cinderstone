//! Production transport adapter for one actual connection's packet counters.
//!
//! The server registry calls `tick_connection` directly once in each actual
//! connection phase. A watch receiver must not replay coalesced ticks: receive
//! counts cannot then be assigned to their original twenty-tick sample windows.

use anyhow::{anyhow, Result};
use mc121_rust::login_limits::{ConnectionPacketRate, PacketRateExceeded};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tokio::sync::{watch, Notify};

#[derive(Clone)]
pub(super) struct ConnectionRate {
    inner: Arc<ConnectionRateOwner>,
}

struct ConnectionRateOwner {
    counters: Mutex<ConnectionPacketRate>,
    terminal: watch::Sender<Option<PacketRateExceeded>>,
    read_only: AtomicBool,
    read_only_changed: Notify,
}

/// An actual counter snapshot, not an estimate from elapsed wall-clock time.
#[derive(Clone, Copy, Debug)]
pub(super) struct PacketRateSnapshot {
    pub elapsed_connection_ticks: i32,
    pub pending_received_packets: i32,
    pub pending_sent_packets: i32,
    pub average_received_packets: f32,
    pub average_sent_packets: f32,
    pub rate_limit: Option<i32>,
    pub read_only: bool,
}

impl ConnectionRate {
    /// Install at TCP acceptance and retain across every protocol transition.
    pub(super) fn new(rate_limit_property: i32) -> Self {
        Self {
            inner: Arc::new(ConnectionRateOwner {
                counters: Mutex::new(ConnectionPacketRate::new(rate_limit_property)),
                terminal: watch::channel(None).0,
                read_only: AtomicBool::new(false),
                read_only_changed: Notify::new(),
            }),
        }
    }

    /// Call at the accepted listener dispatch/scheduling boundary, including
    /// the source's handled rescheduling/rejection/class-cast escape paths.
    /// Do not call for raw frames, decoder errors or shouldHandle=false packets.
    /// Auto-read state is deliberately not an acceptance predicate: a packet
    /// already in dispatch can still reach the source's counter increment.
    pub(super) fn accepted_packet_received(&self) -> Result<()> {
        self.inner
            .counters
            .lock()
            .map_err(|_| anyhow!("connection packet counters poisoned"))?
            .accepted_packet_received();
        Ok(())
    }

    /// Count actual send dispatch before the write, including the rate reason
    /// itself and a dispatch whose encoder/write later fails.
    pub(super) fn packet_send_dispatched(&self) -> Result<()> {
        self.inner
            .counters
            .lock()
            .map_err(|_| anyhow!("connection packet counters poisoned"))?
            .packet_send_dispatched();
        Ok(())
    }

    /// Invoke directly after the active listener tick and channel flush, once
    /// per actual connection tick. The existing core preserves first/21st/41st
    /// sample cadence and JVM float/int operations. Only the first terminal
    /// action is published: the host retains it until send completion closes
    /// that actual connection. This notification is native transport plumbing,
    /// not another source timer, packet policy or read-only transition.
    pub(super) fn tick_connection(&self) -> Result<Option<PacketRateExceeded>> {
        let exceeded = self
            .inner
            .counters
            .lock()
            .map_err(|_| anyhow!("connection packet counters poisoned"))?
            .tick();
        let Some(exceeded) = exceeded else {
            return Ok(None);
        };
        let first = self.inner.terminal.send_if_modified(|terminal| {
            if terminal.is_some() {
                false
            } else {
                *terminal = Some(exceeded);
                true
            }
        });
        Ok(first.then_some(exceeded))
    }

    /// A persisted terminal action is visible even to a late subscriber.
    pub(super) fn subscribe_terminal(&self) -> watch::Receiver<Option<PacketRateExceeded>> {
        self.inner.terminal.subscribe()
    }

    /// The host calls this after dispatching the common disconnect packet, and
    /// closes on either successful or failed send completion. Stop future raw
    /// reads without rejecting or reclassifying already dispatched packets.
    pub(super) fn mark_read_only(&self) {
        if !self.inner.read_only.swap(true, Ordering::AcqRel) {
            self.inner.read_only_changed.notify_waiters();
        }
    }

    pub(super) fn is_read_only(&self) -> bool {
        self.inner.read_only.load(Ordering::Acquire)
    }

    /// Wake an actual socket-read owner when auto-read is disabled. Enabling
    /// notification before checking the flag avoids a lost-wakeup race.
    pub(super) async fn wait_read_only(&self) {
        loop {
            let changed = self.inner.read_only_changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.is_read_only() {
                return;
            }
            changed.await;
        }
    }

    pub(super) fn snapshot(&self) -> Result<PacketRateSnapshot> {
        let counters = self
            .inner
            .counters
            .lock()
            .map_err(|_| anyhow!("connection packet counters poisoned"))?;
        Ok(PacketRateSnapshot {
            elapsed_connection_ticks: counters.elapsed_connection_ticks(),
            pending_received_packets: counters.pending_received_packets(),
            pending_sent_packets: counters.pending_sent_packets(),
            average_received_packets: counters.average_received_packets(),
            average_sent_packets: counters.average_sent_packets(),
            rate_limit: counters.rate_limit(),
            read_only: self.is_read_only(),
        })
    }
}
