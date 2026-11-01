//! Minecraft 1.21's ordinary 20 TPS runServer deadline/overload clock.
//!
//! The native server retains Burst catch-up. Only the original >2-second lag
//! and >=15-second warning-deadline gate can advance the deadline past missed
//! ticks. This is the normal-rate path, not tick sprinting/frozen tick control.
use anyhow::{Context, Result};
use tokio::time::{self, Duration, Instant, Interval, MissedTickBehavior};

const PERIOD_NS: i64 = 50_000_000;
const OVERLOAD_LAG_NS: i64 = 1_000_000_000 + 20 * PERIOD_NS;
const WARNING_INTERVAL_NS: i64 = 10_000_000_000 + 100 * PERIOD_NS;

pub(super) struct Overload {
    pub(super) behind_ms: i64,
    pub(super) skipped_ticks: i64,
}

pub(super) struct NativeServerTiming {
    interval: Interval,
    next_tick_time_ns: i64,
    last_overload_warning_ns: i64,
    #[cfg(not(unix))]
    portable_epoch: std::time::Instant,
}

impl NativeServerTiming {
    pub(super) fn new() -> Result<Self> {
        #[cfg(not(unix))]
        let portable_epoch = std::time::Instant::now();
        #[cfg(unix)]
        let now_ns = monotonic_ns()?;
        #[cfg(not(unix))]
        let now_ns = portable_epoch
            .elapsed()
            .as_nanos()
            .try_into()
            .context("portable monotonic epoch exceeds the signed server clock")?;
        // runServer starts its first normal tick immediately after resetting
        // nextTickTimeNanos following server initialization.
        let mut interval =
            time::interval_at(Instant::now(), Duration::from_nanos(PERIOD_NS as u64));
        interval.set_missed_tick_behavior(MissedTickBehavior::Burst);
        Ok(Self {
            interval,
            next_tick_time_ns: now_ns,
            // This is the Java field's literal initial value, not now, and not
            // an invented fifteen-second warmup from world/server creation.
            last_overload_warning_ns: 0,
            #[cfg(not(unix))]
            portable_epoch,
        })
    }

    /// Cancellation-safe until the interval becomes ready; after it is ready
    /// no further await separates the source overload check from this tick.
    pub(super) async fn next_tick(&mut self) -> Result<Option<Overload>> {
        let scheduled = self.interval.tick().await;
        #[cfg(unix)]
        let now_ns = monotonic_ns()?;
        #[cfg(not(unix))]
        let now_ns: i64 = self
            .portable_epoch
            .elapsed()
            .as_nanos()
            .try_into()
            .context("portable monotonic epoch exceeds the signed server clock")?;
        let lag = now_ns.wrapping_sub(self.next_tick_time_ns);
        let overload = if lag > OVERLOAD_LAG_NS
            && self
                .next_tick_time_ns
                .wrapping_sub(self.last_overload_warning_ns)
                >= WARNING_INTERVAL_NS
        {
            let skipped_ticks = lag / PERIOD_NS;
            self.next_tick_time_ns = self
                .next_tick_time_ns
                .wrapping_add(skipped_ticks.wrapping_mul(PERIOD_NS));
            self.last_overload_warning_ns = self.next_tick_time_ns;
            // scheduled is this tick's original deadline in Tokio's clock.
            // The next wake-up is the adjusted deadline plus exactly one tick;
            // reset_after(now) would lose the source deadline's fractional lag.
            let delay_ns = (skipped_ticks as u64) * PERIOD_NS as u64 + PERIOD_NS as u64;
            let next = scheduled
                .checked_add(Duration::from_nanos(delay_ns))
                .context("server tick deadline exceeds the native timer range")?;
            self.interval.reset_at(next);
            Some(Overload {
                behind_ms: lag / 1_000_000,
                skipped_ticks,
            })
        } else {
            None
        };
        self.next_tick_time_ns = self.next_tick_time_ns.wrapping_add(PERIOD_NS);
        Ok(overload)
    }
}

#[cfg(unix)]
fn monotonic_ns() -> Result<i64> {
    let mut value = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // Linux/HotSpot System.nanoTime uses this actual monotonic OS clock. The
    // absolute origin matters for the initially-zero overload-warning field.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut value) } != 0 {
        return Err(std::io::Error::last_os_error()).context("read native server monotonic clock");
    }
    Ok((value.tv_sec as i64)
        .wrapping_mul(1_000_000_000)
        .wrapping_add(value.tv_nsec as i64))
}

// Non-Unix builds deliberately use an elapsed-process monotonic epoch above.
// It bounds timer behavior without claiming the OS-epoch/initial-warning parity
// established by CLOCK_MONOTONIC on the production Linux target.
