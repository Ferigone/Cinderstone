//! Connection-owned login timeout and packet-rate counters from Java 1.21.
//!
//! The host supplies actual connection ticks and accepted packet events. These
//! counters do not impose a wall-clock login deadline, per-IP throttle, extra
//! concurrent-login cap or packet limit when the `rate-limit` property is zero.

/// `ServerLoginPacketListenerImpl.MAX_TICKS_BEFORE_LOGIN`.
pub const MAX_TICKS_BEFORE_LOGIN: i32 = 600;
pub const SLOW_LOGIN_REASON: &str = "multiplayer.disconnect.slow_login";
pub const EXCEEDED_PACKET_RATE_REASON: &str = "disconnect.exceeded_packet_rate";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginTimeoutAction {
    /// Send the LOGIN disconnect component, then close the actual connection.
    DisconnectSlowLogin,
}

impl LoginTimeoutAction {
    pub const fn reason(self) -> &'static str {
        match self {
            Self::DisconnectSlowLogin => SLOW_LOGIN_REASON,
        }
    }
}

/// The timer belongs to one LOGIN listener. Receiving packets or changing its
/// HELLO/key/authentication/duplicate-wait state does not reset it. Drop it only
/// when Login Acknowledged replaces that listener, or the connection closes.
#[derive(Clone, Debug, Default)]
pub struct LoginTimeout {
    ticks: i32,
}

impl LoginTimeout {
    pub const fn new() -> Self {
        Self { ticks: 0 }
    }

    /// Call after that tick's verification/duplicate-disconnect work, matching
    /// Java's `if (tick++ == 600)`. The first action is on invocation 601.
    pub fn tick(&mut self) -> Option<LoginTimeoutAction> {
        let previous = self.ticks;
        self.ticks = self.ticks.wrapping_add(1);
        (previous == MAX_TICKS_BEFORE_LOGIN).then_some(LoginTimeoutAction::DisconnectSlowLogin)
    }

    /// Account for a losslessly observed number of actual connection ticks.
    ///
    /// This is useful when a native watch receiver coalesces notifications but
    /// each notification carries a monotonically increasing server tick number.
    /// It must not be fed elapsed milliseconds or a guessed 20-TPS duration.
    /// A returned action means the span includes the source timeout boundary;
    /// the host must stop LOGIN rather than finish another listener transition.
    pub fn advance_ticks(&mut self, ticks: u64) -> Option<LoginTimeoutAction> {
        let ticks_before_boundary = MAX_TICKS_BEFORE_LOGIN.wrapping_sub(self.ticks) as u32 as u64;
        self.ticks = self.ticks.wrapping_add(ticks as i32);
        (ticks > ticks_before_boundary).then_some(LoginTimeoutAction::DisconnectSlowLogin)
    }

    /// Java's signed int counter, including its post-increment behavior.
    pub const fn elapsed_ticks(&self) -> i32 {
        self.ticks
    }
}

/// Native observation adapter for a lossless cumulative connection-tick clock.
/// The clock is owned by the server, not by this listener or a packet reader.
#[derive(Clone, Debug)]
pub struct ObservedLoginTimeout {
    timeout: LoginTimeout,
    last_server_tick: u64,
}

impl ObservedLoginTimeout {
    pub const fn new(server_tick_at_listener_installation: u64) -> Self {
        Self {
            timeout: LoginTimeout::new(),
            last_server_tick: server_tick_at_listener_installation,
        }
    }

    /// Observe the same wrapping u64 server counter that was used at creation.
    /// Repeated observations at one tick do not advance or reset the deadline.
    pub fn observe(&mut self, server_tick: u64) -> Option<LoginTimeoutAction> {
        let elapsed = server_tick.wrapping_sub(self.last_server_tick);
        self.last_server_tick = server_tick;
        self.timeout.advance_ticks(elapsed)
    }

    pub const fn elapsed_ticks(&self) -> i32 {
        self.timeout.elapsed_ticks()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PacketRateExceeded {
    pub average_received_packets: f32,
    pub configured_limit: i32,
}

impl PacketRateExceeded {
    pub const fn reason(self) -> &'static str {
        EXCEEDED_PACKET_RATE_REASON
    }
}

/// `Connection` packet counters and optional `RateKickingConnection` decision.
///
/// Construct once for the actual TCP connection. Protocol/listener changes do
/// not replace these counters. `rate_limit_property <= 0` retains ordinary
/// Connection statistics and never produces a packet-rate kick.
#[derive(Clone, Debug)]
pub struct ConnectionPacketRate {
    rate_limit: Option<i32>,
    received_packets: i32,
    sent_packets: i32,
    average_received_packets: f32,
    average_sent_packets: f32,
    tick_count: i32,
}

impl Default for ConnectionPacketRate {
    fn default() -> Self {
        Self::new(0)
    }
}

impl ConnectionPacketRate {
    pub const fn new(rate_limit_property: i32) -> Self {
        Self {
            rate_limit: if rate_limit_property > 0 {
                Some(rate_limit_property)
            } else {
                None
            },
            received_packets: 0,
            sent_packets: 0,
            average_received_packets: 0.0,
            average_sent_packets: 0.0,
            tick_count: 0,
        }
    }

    /// Record a decoded packet only if the active listener accepted it. Java
    /// increments after dispatch/scheduling, including the handled rescheduling,
    /// rejected-execution and class-cast paths. An unhandled dispatch exception,
    /// rejected listener packet, partial frame or raw byte is not an event.
    pub fn accepted_packet_received(&mut self) {
        self.received_packets = self.received_packets.wrapping_add(1);
    }

    /// Record `Connection.sendPacket` dispatch, before the event-loop write.
    /// Do not count an unsent packet merely added to a disconnected queue.
    pub fn packet_send_dispatched(&mut self) {
        self.sent_packets = self.sent_packets.wrapping_add(1);
    }

    /// Call once after the actual active listener's tick and channel flush.
    ///
    /// Java samples on the first invocation, then invocations 21, 41, ... using
    /// `tickCount++ % 20 == 0`. Both averages use float operations and the
    /// `Mth.lerp(0.75f, currentCount, previousAverage)` argument order. A limit
    /// is exceeded only for a strict greater-than comparison after smoothing.
    ///
    /// On an exceeded result the host must send the common disconnect packet,
    /// make the actual channel read-only, and close it in the send completion
    /// callback. This method neither fabricates a phase-specific packet nor
    /// silently drops incoming packets.
    pub fn tick(&mut self) -> Option<PacketRateExceeded> {
        let previous = self.tick_count;
        self.tick_count = self.tick_count.wrapping_add(1);
        if previous % 20 != 0 {
            return None;
        }

        let sent = self.sent_packets as f32;
        let received = self.received_packets as f32;
        // Preserve JVM fsub/fmul/fadd order; do not replace with mul_add.
        self.average_sent_packets = sent + 0.75_f32 * (self.average_sent_packets - sent);
        self.average_received_packets =
            received + 0.75_f32 * (self.average_received_packets - received);
        self.sent_packets = 0;
        self.received_packets = 0;

        self.rate_limit.and_then(|limit| {
            (self.average_received_packets > limit as f32).then_some(PacketRateExceeded {
                average_received_packets: self.average_received_packets,
                configured_limit: limit,
            })
        })
    }

    pub const fn rate_limit(&self) -> Option<i32> {
        self.rate_limit
    }

    pub const fn average_received_packets(&self) -> f32 {
        self.average_received_packets
    }

    pub const fn average_sent_packets(&self) -> f32 {
        self.average_sent_packets
    }

    pub const fn pending_received_packets(&self) -> i32 {
        self.received_packets
    }

    pub const fn pending_sent_packets(&self) -> i32 {
        self.sent_packets
    }

    pub const fn elapsed_connection_ticks(&self) -> i32 {
        self.tick_count
    }
}
