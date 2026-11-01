//! Original RandomSupport unique seeds and Level's sequential RNG construction.
//! Clock samples are injected for proof; Linux startup uses CLOCK_MONOTONIC.
use crate::random::LegacyRandomSource;
use std::{
    io,
    sync::atomic::{AtomicU64, Ordering},
};

pub const SEED_UNIQUIFIER_INITIAL: u64 = 8_682_522_807_148_012;
pub const SEED_UNIQUIFIER_MULTIPLIER: u64 = 1_181_783_497_276_652_981;

pub struct SeedUniquifier {
    value: AtomicU64,
}
impl Default for SeedUniquifier {
    fn default() -> Self {
        Self::new()
    }
}
impl SeedUniquifier {
    pub const fn new() -> Self {
        Self::from_state(SEED_UNIQUIFIER_INITIAL)
    }
    pub const fn from_state(state: u64) -> Self {
        Self {
            value: AtomicU64::new(state),
        }
    }
    pub fn state(&self) -> u64 {
        self.value.load(Ordering::SeqCst)
    }
    fn advance(&self) -> u64 {
        let old = self
            .value
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                Some(value.wrapping_mul(SEED_UNIQUIFIER_MULTIPLIER))
            })
            .expect("infallible unique-seed transition");
        old.wrapping_mul(SEED_UNIQUIFIER_MULTIPLIER)
    }
    /// Match updateAndGet before System.nanoTime, including observable callback
    /// order. A failed host clock is an explicit error after the atomic update.
    pub fn next_seed_with_clock(
        &self,
        clock: &mut impl FnMut() -> io::Result<i64>,
    ) -> io::Result<i64> {
        let updated = self.advance();
        let nanos = clock()?;
        Ok((updated ^ nanos as u64) as i64)
    }
    pub fn next_seed_at(&self, nanos: i64) -> i64 {
        (self.advance() ^ nanos as u64) as i64
    }
}
static PROCESS_SEED_UNIQUIFIER: SeedUniquifier = SeedUniquifier::new();

#[derive(Clone, Debug)]
pub struct LevelRandoms {
    pub rand_value: i32,
    pub random: LegacyRandomSource,
    /// Same sequential stream as ThreadSafeLegacyRandomSource. This value must
    /// have one host owner; its original concurrent CAS behavior is not ported.
    pub thread_safe_random: LegacyRandomSource,
    pub initial_seeds: [i64; 3],
}

pub fn create_random_with_clock(
    uniquifier: &SeedUniquifier,
    clock: &mut impl FnMut() -> io::Result<i64>,
) -> io::Result<LegacyRandomSource> {
    Ok(LegacyRandomSource::new(
        uniquifier.next_seed_with_clock(clock)?,
    ))
}
pub fn create_level_randoms_with_clock(
    uniquifier: &SeedUniquifier,
    clock: &mut impl FnMut() -> io::Result<i64>,
) -> io::Result<LevelRandoms> {
    let temporary_seed = uniquifier.next_seed_with_clock(clock)?;
    let mut temporary = LegacyRandomSource::new(temporary_seed);
    let rand_value = temporary.next_i32();
    let random_seed = uniquifier.next_seed_with_clock(clock)?;
    let random = LegacyRandomSource::new(random_seed);
    let thread_safe_seed = uniquifier.next_seed_with_clock(clock)?;
    let thread_safe_random = LegacyRandomSource::new(thread_safe_seed);
    Ok(LevelRandoms {
        rand_value,
        random,
        thread_safe_random,
        initial_seeds: [temporary_seed, random_seed, thread_safe_seed],
    })
}

/// HotSpot's Linux nanoTime clock family. No wall-clock or zero-elapsed fallback.
#[cfg(target_os = "linux")]
pub fn system_monotonic_nanos() -> io::Result<i64> {
    let mut sample = std::mem::MaybeUninit::<libc::timespec>::uninit();
    // SAFETY: clock_gettime initializes both timespec fields on success. The
    // uninitialized result is never read when libc reports a failure.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, sample.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: checked the successful clock_gettime result above.
    let sample = unsafe { sample.assume_init() };
    if !(0..1_000_000_000).contains(&sample.tv_nsec) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "CLOCK_MONOTONIC nanosecond field out of range",
        ));
    }
    Ok((i128::from(sample.tv_sec) * 1_000_000_000 + i128::from(sample.tv_nsec)) as i64)
}
#[cfg(not(target_os = "linux"))]
pub fn system_monotonic_nanos() -> io::Result<i64> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "exact host nanoTime clock requires Linux CLOCK_MONOTONIC",
    ))
}
pub fn create_random() -> io::Result<LegacyRandomSource> {
    create_random_with_clock(&PROCESS_SEED_UNIQUIFIER, &mut system_monotonic_nanos)
}
/// Entity's UUID consumes two nextLong calls. Actual stock ServerPlayer
/// constructor observations show the same inherited RNG object/state survives
/// LivingEntity, Player and ServerPlayer initialization. Their Math.random and
/// separate spawn-location sources do not belong to this retained stream.
pub fn initialize_player_random(random: &mut LegacyRandomSource) {
    let _ = random.next_i64();
    let _ = random.next_i64();
}
pub fn create_player_random() -> io::Result<LegacyRandomSource> {
    let mut random = create_random()?;
    initialize_player_random(&mut random);
    Ok(random)
}
pub fn create_level_randoms() -> io::Result<LevelRandoms> {
    create_level_randoms_with_clock(&PROCESS_SEED_UNIQUIFIER, &mut system_monotonic_nanos)
}

#[cfg(test)]
#[path = "../tools/random_seed_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tools/player_rng_constructor_tests.rs"]
mod player_constructor_tests;
