//! Java 1.21 common-listener keepalive and PlayerList latency publication state.
//!
//! The caller supplies monotonic milliseconds, polls once per server tick and
//! sends the appropriate protocol-state packet or translated timeout message.
//! Arithmetic follows the original `arr` and `aur` bytecode, including Java
//! narrowing conversions and integer overflow.

use anyhow::{bail, Result};

pub const KEEPALIVE_INTERVAL_MS: i64 = 15_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeepAliveAction {
    Challenge(i64),
    Timeout,
}

#[derive(Debug, Clone)]
pub struct KeepAlive {
    last_check_ms: i64,
    pending: Option<i64>,
    latency: i32,
    closed_at: Option<i64>,
    singleplayer_owner: bool,
}

impl KeepAlive {
    pub fn new(now_ms: i64) -> Self {
        Self::with_latency(now_ms, 0)
    }

    /// Configuration/play listener transitions inherit latency from the
    /// listener cookie, but reset their keepalive timer and pending challenge.
    pub fn with_latency(now_ms: i64, latency: i32) -> Self {
        Self {
            last_check_ms: now_ms,
            pending: None,
            latency,
            closed_at: None,
            singleplayer_owner: false,
        }
    }

    pub fn for_singleplayer_owner(now_ms: i64, latency: i32) -> Self {
        Self {
            singleplayer_owner: true,
            ..Self::with_latency(now_ms, latency)
        }
    }

    pub fn latency(&self) -> i32 {
        self.latency
    }

    /// Mirrors ServerCommonPacketListenerImpl.close, used when a terminal
    /// packet closes a listener while its transport remains temporarily open.
    pub fn close(&mut self, now_ms: i64) {
        if self.closed_at.is_none() {
            self.closed_at = Some(now_ms);
        }
    }

    pub fn tick(&mut self, now_ms: i64) -> Option<KeepAliveAction> {
        if self.singleplayer_owner
            || now_ms.wrapping_sub(self.last_check_ms) < KEEPALIVE_INTERVAL_MS
        {
            return None;
        }
        if self.pending.is_some() {
            return Some(KeepAliveAction::Timeout);
        }
        if let Some(closed_at) = self.closed_at {
            return (now_ms.wrapping_sub(closed_at) >= KEEPALIVE_INTERVAL_MS)
                .then_some(KeepAliveAction::Timeout);
        }
        self.pending = Some(now_ms);
        self.last_check_ms = now_ms;
        Some(KeepAliveAction::Challenge(now_ms))
    }

    /// Some(latency) means a matching challenge was accepted. Dedicated
    /// servers disconnect on both mismatched and unsolicited responses.
    pub fn receive(&mut self, now_ms: i64, id: i64) -> Result<Option<i32>> {
        if self.pending == Some(id) {
            let elapsed = now_ms.wrapping_sub(self.last_check_ms) as i32;
            self.latency = self.latency.wrapping_mul(3).wrapping_add(elapsed) / 4;
            self.pending = None;
            return Ok(Some(self.latency));
        }
        if self.singleplayer_owner {
            Ok(None)
        } else {
            bail!("disconnect.timeout: unexpected keepalive response")
        }
    }
}

/// Vanilla PlayerList broadcasts all current latency values after incrementing
/// its counter past 600, so the period is exactly 601 simulation ticks.
#[derive(Debug, Clone, Default)]
pub struct LatencyUpdateClock {
    ticks: i32,
}

impl LatencyUpdateClock {
    pub fn tick(&mut self) -> bool {
        self.ticks = self.ticks.wrapping_add(1);
        if self.ticks > 600 {
            self.ticks = 0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_and_timeout_boundaries_match_common_listener() {
        let mut state = KeepAlive::new(123);
        assert_eq!(state.tick(15_122), None);
        assert_eq!(state.tick(15_123), Some(KeepAliveAction::Challenge(15_123)));
        assert_eq!(state.tick(30_122), None);
        assert_eq!(state.tick(30_123), Some(KeepAliveAction::Timeout));
    }

    #[test]
    fn latency_smoothing_and_cadence_are_independent_of_response_time() {
        let mut state = KeepAlive::with_latency(0, 100);
        assert_eq!(state.tick(15_000), Some(KeepAliveAction::Challenge(15_000)));
        assert_eq!(state.receive(15_011, 15_000).unwrap(), Some(77));
        assert_eq!(state.tick(29_999), None);
        assert_eq!(state.tick(30_000), Some(KeepAliveAction::Challenge(30_000)));
        assert!(state.receive(30_001, 15_000).is_err());
        assert_eq!(state.receive(30_007, 30_000).unwrap(), Some(59));
        assert!(state.receive(30_008, 30_000).is_err());
    }

    #[test]
    fn signed_java_arithmetic_and_owner_exception_are_preserved() {
        let mut state = KeepAlive::with_latency(i64::MAX - 10_000, i32::MAX);
        let challenge = (i64::MAX - 10_000).wrapping_add(15_000);
        assert_eq!(
            state.tick(challenge),
            Some(KeepAliveAction::Challenge(challenge))
        );
        let now = challenge.wrapping_add(i64::from(i32::MAX) + 100);
        let rtt = now.wrapping_sub(challenge) as i32;
        assert_eq!(
            state.receive(now, challenge).unwrap(),
            Some(i32::MAX.wrapping_mul(3).wrapping_add(rtt) / 4)
        );
        let mut owner = KeepAlive::for_singleplayer_owner(0, 5);
        assert_eq!(owner.tick(100_000), None);
        assert_eq!(owner.receive(100_001, 123).unwrap(), None);
        assert_eq!(owner.latency(), 5);
    }

    #[test]
    fn closed_listener_timeout_does_not_restart_on_repeated_close() {
        let mut state = KeepAlive::new(0);
        state.close(10_000);
        state.close(20_000);
        assert_eq!(state.tick(15_000), None);
        assert_eq!(state.tick(24_999), None);
        assert_eq!(state.tick(25_000), Some(KeepAliveAction::Timeout));
    }

    #[test]
    fn player_list_publishes_latency_every_601_ticks() {
        let mut clock = LatencyUpdateClock::default();
        for tick in 1..=1803 {
            assert_eq!(clock.tick(), tick % 601 == 0);
        }
    }
    #[test]
    fn keepalive_decisions_match_actual_java_common_listener() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tools/session-keepalive.json")).unwrap();
        assert_eq!(
            fixture["jar_sha256"],
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
        );
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 903);
        let mut state = KeepAlive::new(0);
        for (index, case) in cases.iter().enumerate() {
            let now = case["now"].as_i64().unwrap();
            let mut timeout = false;
            let mut sent = Vec::new();
            match case["op"].as_str().unwrap() {
                "new" => {
                    let latency = case["latency"].as_i64().unwrap() as i32;
                    state = if case["owner"].as_bool().unwrap() {
                        KeepAlive::for_singleplayer_owner(now, latency)
                    } else {
                        KeepAlive::with_latency(now, latency)
                    };
                }
                "tick" => match state.tick(now) {
                    Some(KeepAliveAction::Challenge(id)) => sent.push(id),
                    Some(KeepAliveAction::Timeout) => timeout = true,
                    None => {}
                },
                "receive" => timeout = state.receive(now, case["id"].as_i64().unwrap()).is_err(),
                "close" => state.close(now),
                op => panic!("unknown Java oracle op {op}"),
            }
            let expected = &case["expected"];
            assert_eq!(
                timeout,
                expected["timeout"].as_bool().unwrap(),
                "Java keepalive case{index}"
            );
            assert_eq!(
                state.latency(),
                expected["latency"].as_i64().unwrap() as i32,
                "Java keepalive case{index}"
            );
            assert_eq!(
                sent,
                expected["sent"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|id| id.as_i64().unwrap())
                    .collect::<Vec<_>>(),
                "Java keepalive case{index}"
            );
        }
    }
}
