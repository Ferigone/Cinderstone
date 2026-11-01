//! Minecraft 1.21 sequential RNGs, seed support and positional factories.
//!
//! Derived from the pinned server's `dyz` and `dyn` bytecode (see
//! `evidence/legacy-random-bytecode.txt`), rather than Java's Random facade.
//! Xoroshiro/seed/factory coverage is recorded in tools/noise_conformance.json.
//! Gaussian sampling and Java's concurrent-misuse CAS exception behavior are
//! outside this module's verified scope.

const MULTIPLIER: u64 = 25_214_903_917;
const ADDEND: u64 = 11;
const MASK: u64 = (1_u64 << 48) - 1;

#[derive(Debug, Clone)]
pub struct LegacyRandomSource {
    state: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidBound;

impl std::fmt::Display for InvalidBound {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Bound must be positive")
    }
}

impl std::error::Error for InvalidBound {}

impl LegacyRandomSource {
    pub fn new(seed: i64) -> Self {
        Self {
            state: ((seed as u64) ^ MULTIPLIER) & MASK,
        }
    }

    pub fn state48(&self) -> u64 {
        self.state
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.state = ((seed as u64) ^ MULTIPLIER) & MASK;
    }

    /// Equivalent to dyz.c(int), including Java's masked long shift distance.
    /// Minecraft normally requests 1..=32 bits. Even next(0) advances the state.
    pub fn next(&mut self, bits: i32) -> i32 {
        self.state = self.state.wrapping_mul(MULTIPLIER).wrapping_add(ADDEND) & MASK;
        let distance = (48_i32.wrapping_sub(bits) as u32) & 63;
        (self.state >> distance) as i32
    }

    pub fn next_bits(&mut self, bits: i32) -> i32 {
        self.next(bits)
    }

    pub fn next_i32(&mut self) -> i32 {
        self.next(32)
    }

    pub fn next_bounded_i32(&mut self, bound: i32) -> Result<i32, InvalidBound> {
        if bound <= 0 {
            return Err(InvalidBound);
        }
        if bound & (bound - 1) == 0 {
            // Bound 1 still consumes a 31-bit sample, as in the Java bytecode.
            return Ok(((i64::from(bound) * i64::from(self.next(31))) >> 31) as i32);
        }
        loop {
            let bits = self.next(31);
            let value = bits % bound;
            // Java's signed int overflow is essential for rejection sampling.
            if bits.wrapping_sub(value).wrapping_add(bound - 1) >= 0 {
                return Ok(value);
            }
        }
    }

    pub fn next_i64(&mut self) -> i64 {
        let upper = i64::from(self.next(32)).wrapping_shl(32);
        // The low sample is SIGN EXTENDED, not an unsigned concatenation.
        upper.wrapping_add(i64::from(self.next(32)))
    }

    pub fn next_bool(&mut self) -> bool {
        self.next(1) != 0
    }

    pub fn next_f32(&mut self) -> f32 {
        (self.next(24) as f32) * (1.0_f32 / 16_777_216.0_f32)
    }

    pub fn next_f64(&mut self) -> f64 {
        let upper = i64::from(self.next(26)) << 27;
        let value = upper + i64::from(self.next(27));
        (value as f64) * (1.0_f64 / 9_007_199_254_740_992.0_f64)
    }
}

/// RandomSupport's deterministic 128-bit seed pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seed128 {
    pub lo: u64,
    pub hi: u64,
}
impl Seed128 {
    pub const fn new(lo: u64, hi: u64) -> Self {
        Self { lo, hi }
    }
    pub fn xor(self, other: Self) -> Self {
        Self::new(self.lo ^ other.lo, self.hi ^ other.hi)
    }
    pub fn mixed(self) -> Self {
        Self::new(mix_stafford13(self.lo), mix_stafford13(self.hi))
    }
}
pub const GOLDEN_RATIO_64: u64 = 0x9e37_79b9_7f4a_7c15;
pub const SILVER_RATIO_64: u64 = 0x6a09_e667_f3bc_c909;
pub fn mix_stafford13(mut seed: u64) -> u64 {
    seed = (seed ^ (seed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    seed = (seed ^ (seed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    seed ^ (seed >> 31)
}
pub fn upgrade_seed_unmixed(seed: i64) -> Seed128 {
    let lo = (seed as u64) ^ SILVER_RATIO_64;
    Seed128::new(lo, lo.wrapping_add(GOLDEN_RATIO_64))
}
pub fn upgrade_seed(seed: i64) -> Seed128 {
    upgrade_seed_unmixed(seed).mixed()
}
pub fn seed_from_hash_of(value: &str) -> Seed128 {
    seed_from_hash_bytes(value.as_bytes())
}
/// Java UTF-8 encoding replaces an isolated UTF-16 surrogate with ASCII `?`.
pub fn seed_from_hash_of_utf16(value: &[u16]) -> Seed128 {
    let mut bytes = Vec::with_capacity(value.len());
    for character in char::decode_utf16(value.iter().copied()) {
        match character {
            Ok(character) => {
                let mut encoded = [0; 4];
                bytes.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
            }
            Err(_) => bytes.push(b'?'),
        }
    }
    seed_from_hash_bytes(&bytes)
}
fn seed_from_hash_bytes(value: &[u8]) -> Seed128 {
    let digest = md5::compute(value).0;
    Seed128::new(
        u64::from_be_bytes(digest[..8].try_into().expect("fixed MD5 half")),
        u64::from_be_bytes(digest[8..].try_into().expect("fixed MD5 half")),
    )
}
pub fn java_string_hash(value: &[u16]) -> i32 {
    value.iter().fold(0_i32, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(i32::from(*unit))
    })
}
/// Mth.getSeed: x multiplication overflows as an INT before long promotion.
pub fn coordinate_seed(x: i32, y: i32, z: i32) -> i64 {
    let value = i64::from(x.wrapping_mul(3_129_871))
        ^ i64::from(z).wrapping_mul(116_129_781)
        ^ i64::from(y);
    value
        .wrapping_mul(value)
        .wrapping_mul(42_317_861)
        .wrapping_add(value.wrapping_mul(11))
        >> 16
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Xoroshiro128PlusPlus {
    state: Seed128,
}
impl Xoroshiro128PlusPlus {
    pub fn new(lo: u64, hi: u64) -> Self {
        Self {
            state: if lo | hi == 0 {
                Seed128::new(GOLDEN_RATIO_64, SILVER_RATIO_64)
            } else {
                Seed128::new(lo, hi)
            },
        }
    }
    pub const fn state(&self) -> Seed128 {
        self.state
    }
    pub fn next_u64(&mut self) -> u64 {
        let lo = self.state.lo;
        let mut hi = self.state.hi;
        let result = lo.wrapping_add(hi).rotate_left(17).wrapping_add(lo);
        hi ^= lo;
        self.state = Seed128::new(lo.rotate_left(49) ^ hi ^ (hi << 21), hi.rotate_left(28));
        result
    }
}
#[derive(Debug, Clone)]
pub struct XoroshiroRandomSource {
    random: Xoroshiro128PlusPlus,
}
impl XoroshiroRandomSource {
    pub fn new(seed: i64) -> Self {
        Self::from_seed128(upgrade_seed(seed))
    }
    pub fn from_seed128(seed: Seed128) -> Self {
        Self::from_state(seed.lo, seed.hi)
    }
    pub fn from_state(lo: u64, hi: u64) -> Self {
        Self {
            random: Xoroshiro128PlusPlus::new(lo, hi),
        }
    }
    pub fn state(&self) -> Seed128 {
        self.random.state()
    }
    pub fn set_seed(&mut self, seed: i64) {
        self.random = Xoroshiro128PlusPlus::new(upgrade_seed(seed).lo, upgrade_seed(seed).hi);
    }
    pub fn next_bits(&mut self, bits: i32) -> i32 {
        (self.random.next_u64() >> ((64_i32.wrapping_sub(bits) as u32) & 63)) as i32
    }
    pub fn next_i32(&mut self) -> i32 {
        self.random.next_u64() as i32
    }
    pub fn next_i64(&mut self) -> i64 {
        self.random.next_u64() as i64
    }
    pub fn next_bool(&mut self) -> bool {
        self.random.next_u64() & 1 != 0
    }
    pub fn next_f32(&mut self) -> f32 {
        ((self.random.next_u64() >> 40) as f32) * (1.0_f32 / 16_777_216.0_f32)
    }
    pub fn next_f64(&mut self) -> f64 {
        ((self.random.next_u64() >> 11) as f64) * (1.0_f64 / 9_007_199_254_740_992.0_f64)
    }
    pub fn next_bounded_i32(&mut self, bound: i32) -> Result<i32, InvalidBound> {
        if bound <= 0 {
            return Err(InvalidBound);
        }
        let bound = bound as u32;
        let mut product = u64::from(self.next_i32() as u32) * u64::from(bound);
        let mut low = product as u32;
        if low < bound {
            let threshold = bound.wrapping_neg() % bound;
            while low < threshold {
                product = u64::from(self.next_i32() as u32) * u64::from(bound);
                low = product as u32;
            }
        }
        Ok((product >> 32) as i32)
    }
    pub fn consume_count(&mut self, count: i32) {
        for _ in 0..count {
            self.next_i64();
        }
    }
    pub fn fork(&mut self) -> Self {
        let lo = self.next_i64() as u64;
        Self::from_state(lo, self.next_i64() as u64)
    }
    pub fn fork_positional(&mut self) -> XoroshiroPositionalFactory {
        XoroshiroPositionalFactory::new(self.next_i64() as u64, self.next_i64() as u64)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyPositionalFactory {
    pub seed: i64,
}
impl LegacyPositionalFactory {
    pub const fn new(seed: i64) -> Self {
        Self { seed }
    }
    pub fn at(&self, x: i32, y: i32, z: i32) -> LegacyRandomSource {
        LegacyRandomSource::new(coordinate_seed(x, y, z) ^ self.seed)
    }
    pub fn from_hash_of(&self, value: &str) -> LegacyRandomSource {
        self.from_hash_of_utf16(&value.encode_utf16().collect::<Vec<_>>())
    }
    pub fn from_hash_of_utf16(&self, value: &[u16]) -> LegacyRandomSource {
        LegacyRandomSource::new(i64::from(java_string_hash(value)) ^ self.seed)
    }
    /// Legacy's fromSeed ignores the factory seed, unlike its at/hash methods.
    pub fn from_seed(&self, seed: i64) -> LegacyRandomSource {
        LegacyRandomSource::new(seed)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XoroshiroPositionalFactory {
    pub seed: Seed128,
}
impl XoroshiroPositionalFactory {
    pub const fn new(lo: u64, hi: u64) -> Self {
        Self {
            seed: Seed128::new(lo, hi),
        }
    }
    pub fn at(&self, x: i32, y: i32, z: i32) -> XoroshiroRandomSource {
        XoroshiroRandomSource::from_state(
            (coordinate_seed(x, y, z) as u64) ^ self.seed.lo,
            self.seed.hi,
        )
    }
    pub fn from_hash_of(&self, value: &str) -> XoroshiroRandomSource {
        XoroshiroRandomSource::from_seed128(seed_from_hash_of(value).xor(self.seed))
    }
    pub fn from_hash_of_utf16(&self, value: &[u16]) -> XoroshiroRandomSource {
        XoroshiroRandomSource::from_seed128(seed_from_hash_of_utf16(value).xor(self.seed))
    }
    pub fn from_seed(&self, seed: i64) -> XoroshiroRandomSource {
        XoroshiroRandomSource::from_state(
            (seed as u64) ^ self.seed.lo,
            (seed as u64) ^ self.seed.hi,
        )
    }
}
impl LegacyRandomSource {
    pub fn consume_count(&mut self, count: i32) {
        for _ in 0..count {
            self.next_i32();
        }
    }
    pub fn fork(&mut self) -> Self {
        Self::new(self.next_i64())
    }
    pub fn fork_positional(&mut self) -> LegacyPositionalFactory {
        LegacyPositionalFactory::new(self.next_i64())
    }
}

/// The two vanilla sequential sources used by noise constructors. The enum
/// preserves each source's consumption rules without heap allocation or boxing.
#[derive(Debug, Clone)]
pub enum RandomSourceKind {
    Legacy(LegacyRandomSource),
    Xoroshiro(XoroshiroRandomSource),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionalRandomFactory {
    Legacy(LegacyPositionalFactory),
    Xoroshiro(XoroshiroPositionalFactory),
}
impl PositionalRandomFactory {
    pub fn at(&self, x: i32, y: i32, z: i32) -> RandomSourceKind {
        match self {
            Self::Legacy(factory) => RandomSourceKind::Legacy(factory.at(x, y, z)),
            Self::Xoroshiro(factory) => RandomSourceKind::Xoroshiro(factory.at(x, y, z)),
        }
    }
    pub fn from_hash_of(&self, value: &str) -> RandomSourceKind {
        match self {
            Self::Legacy(factory) => RandomSourceKind::Legacy(factory.from_hash_of(value)),
            Self::Xoroshiro(factory) => RandomSourceKind::Xoroshiro(factory.from_hash_of(value)),
        }
    }
    pub fn from_hash_of_utf16(&self, value: &[u16]) -> RandomSourceKind {
        match self {
            Self::Legacy(factory) => RandomSourceKind::Legacy(factory.from_hash_of_utf16(value)),
            Self::Xoroshiro(factory) => {
                RandomSourceKind::Xoroshiro(factory.from_hash_of_utf16(value))
            }
        }
    }
    pub fn from_seed(&self, seed: i64) -> RandomSourceKind {
        match self {
            Self::Legacy(factory) => RandomSourceKind::Legacy(factory.from_seed(seed)),
            Self::Xoroshiro(factory) => RandomSourceKind::Xoroshiro(factory.from_seed(seed)),
        }
    }
}
pub trait RandomSource {
    fn set_seed(&mut self, seed: i64);
    fn next_bits(&mut self, bits: i32) -> i32 {
        ((self.next_i64() as u64) >> ((64_i32.wrapping_sub(bits) as u32) & 63)) as i32
    }
    fn fork(&mut self) -> RandomSourceKind;
    fn next_i32(&mut self) -> i32;
    fn next_i64(&mut self) -> i64;
    fn next_bounded_i32(&mut self, bound: i32) -> Result<i32, InvalidBound>;
    fn next_bool(&mut self) -> bool;
    fn next_f32(&mut self) -> f32;
    fn next_f64(&mut self) -> f64;
    fn consume_count(&mut self, count: i32);
    fn fork_positional(&mut self) -> PositionalRandomFactory;
}
macro_rules! impl_random_source {
    ($source:ty,$factory:ident) => {
        impl RandomSource for $source {
            fn set_seed(&mut self, seed: i64) {
                <$source>::set_seed(self, seed)
            }
            fn next_bits(&mut self, bits: i32) -> i32 {
                <$source>::next_bits(self, bits)
            }
            fn fork(&mut self) -> RandomSourceKind {
                RandomSourceKind::$factory(<$source>::fork(self))
            }
            fn next_i32(&mut self) -> i32 {
                <$source>::next_i32(self)
            }
            fn next_i64(&mut self) -> i64 {
                <$source>::next_i64(self)
            }
            fn next_bounded_i32(&mut self, bound: i32) -> Result<i32, InvalidBound> {
                <$source>::next_bounded_i32(self, bound)
            }
            fn next_bool(&mut self) -> bool {
                <$source>::next_bool(self)
            }
            fn next_f32(&mut self) -> f32 {
                <$source>::next_f32(self)
            }
            fn next_f64(&mut self) -> f64 {
                <$source>::next_f64(self)
            }
            fn consume_count(&mut self, count: i32) {
                <$source>::consume_count(self, count)
            }
            fn fork_positional(&mut self) -> PositionalRandomFactory {
                PositionalRandomFactory::$factory(<$source>::fork_positional(self))
            }
        }
    };
}
impl_random_source!(LegacyRandomSource, Legacy);
impl_random_source!(XoroshiroRandomSource, Xoroshiro);
impl RandomSource for RandomSourceKind {
    fn set_seed(&mut self, seed: i64) {
        match self {
            Self::Legacy(random) => random.set_seed(seed),
            Self::Xoroshiro(random) => random.set_seed(seed),
        }
    }
    fn next_bits(&mut self, bits: i32) -> i32 {
        match self {
            Self::Legacy(random) => random.next_bits(bits),
            Self::Xoroshiro(random) => random.next_bits(bits),
        }
    }
    fn fork(&mut self) -> RandomSourceKind {
        match self {
            Self::Legacy(random) => Self::Legacy(random.fork()),
            Self::Xoroshiro(random) => Self::Xoroshiro(random.fork()),
        }
    }
    fn next_i32(&mut self) -> i32 {
        match self {
            Self::Legacy(random) => random.next_i32(),
            Self::Xoroshiro(random) => random.next_i32(),
        }
    }
    fn next_i64(&mut self) -> i64 {
        match self {
            Self::Legacy(random) => random.next_i64(),
            Self::Xoroshiro(random) => random.next_i64(),
        }
    }
    fn next_bounded_i32(&mut self, bound: i32) -> Result<i32, InvalidBound> {
        match self {
            Self::Legacy(random) => random.next_bounded_i32(bound),
            Self::Xoroshiro(random) => random.next_bounded_i32(bound),
        }
    }
    fn next_bool(&mut self) -> bool {
        match self {
            Self::Legacy(random) => random.next_bool(),
            Self::Xoroshiro(random) => random.next_bool(),
        }
    }
    fn next_f32(&mut self) -> f32 {
        match self {
            Self::Legacy(random) => random.next_f32(),
            Self::Xoroshiro(random) => random.next_f32(),
        }
    }
    fn next_f64(&mut self) -> f64 {
        match self {
            Self::Legacy(random) => random.next_f64(),
            Self::Xoroshiro(random) => random.next_f64(),
        }
    }
    fn consume_count(&mut self, count: i32) {
        match self {
            Self::Legacy(random) => random.consume_count(count),
            Self::Xoroshiro(random) => random.consume_count(count),
        }
    }
    fn fork_positional(&mut self) -> PositionalRandomFactory {
        match self {
            Self::Legacy(random) => PositionalRandomFactory::Legacy(random.fork_positional()),
            Self::Xoroshiro(random) => PositionalRandomFactory::Xoroshiro(random.fork_positional()),
        }
    }
}

/// WorldgenRandom wraps either source but exposes BitRandomSource's sampling
/// rules (two source samples for nextLong/nextDouble). Its count tracks calls to
/// next(bits), not calls to the underlying source's fork/factory operations.
#[derive(Debug, Clone)]
pub struct WorldgenRandom<R> {
    source: R,
    count: i32,
}
impl<R: RandomSource> WorldgenRandom<R> {
    pub fn new(source: R) -> Self {
        Self { source, count: 0 }
    }
    pub fn count(&self) -> i32 {
        self.count
    }
    pub fn source(&self) -> &R {
        &self.source
    }
    pub fn into_source(self) -> R {
        self.source
    }
    pub fn set_decoration_seed(&mut self, seed: i64, x: i32, z: i32) -> i64 {
        self.set_seed(seed);
        let a = self.next_i64() | 1;
        let b = self.next_i64() | 1;
        let mixed = i64::from(x)
            .wrapping_mul(a)
            .wrapping_add(i64::from(z).wrapping_mul(b))
            ^ seed;
        self.set_seed(mixed);
        mixed
    }
    pub fn set_feature_seed(&mut self, seed: i64, index: i32, step: i32) {
        self.set_seed(
            seed.wrapping_add(i64::from(index))
                .wrapping_add(i64::from(10_000_i32.wrapping_mul(step))),
        );
    }
    pub fn set_large_feature_seed(&mut self, seed: i64, x: i32, z: i32) {
        self.set_seed(seed);
        let a = self.next_i64();
        let b = self.next_i64();
        self.set_seed(i64::from(x).wrapping_mul(a) ^ i64::from(z).wrapping_mul(b) ^ seed);
    }
    pub fn set_large_feature_with_salt(&mut self, seed: i64, x: i32, z: i32, salt: i32) {
        self.set_seed(
            i64::from(x)
                .wrapping_mul(341_873_128_712)
                .wrapping_add(i64::from(z).wrapping_mul(132_897_987_541))
                .wrapping_add(seed)
                .wrapping_add(i64::from(salt)),
        );
    }
}
impl<R: RandomSource> RandomSource for WorldgenRandom<R> {
    fn set_seed(&mut self, seed: i64) {
        self.source.set_seed(seed);
    }
    fn next_bits(&mut self, bits: i32) -> i32 {
        self.count = self.count.wrapping_add(1);
        self.source.next_bits(bits)
    }
    fn fork(&mut self) -> RandomSourceKind {
        self.source.fork()
    }
    fn next_i32(&mut self) -> i32 {
        self.next_bits(32)
    }
    fn next_i64(&mut self) -> i64 {
        i64::from(self.next_bits(32))
            .wrapping_shl(32)
            .wrapping_add(i64::from(self.next_bits(32)))
    }
    fn next_bounded_i32(&mut self, bound: i32) -> Result<i32, InvalidBound> {
        if bound <= 0 {
            return Err(InvalidBound);
        }
        if bound & (bound - 1) == 0 {
            return Ok(((i64::from(bound) * i64::from(self.next_bits(31))) >> 31) as i32);
        }
        loop {
            let bits = self.next_bits(31);
            let value = bits % bound;
            if bits.wrapping_sub(value).wrapping_add(bound - 1) >= 0 {
                return Ok(value);
            }
        }
    }
    fn next_bool(&mut self) -> bool {
        self.next_bits(1) != 0
    }
    fn next_f32(&mut self) -> f32 {
        (self.next_bits(24) as f32) * (1.0_f32 / 16_777_216.0_f32)
    }
    fn next_f64(&mut self) -> f64 {
        (((i64::from(self.next_bits(26)) << 27) + i64::from(self.next_bits(27))) as f64)
            * (1.0_f64 / 9_007_199_254_740_992.0_f64)
    }
    fn consume_count(&mut self, count: i32) {
        for _ in 0..count {
            self.next_i32();
        }
    }
    fn fork_positional(&mut self) -> PositionalRandomFactory {
        self.source.fork_positional()
    }
}
pub fn seed_slime_chunk(x: i32, z: i32, seed: i64, salt: i64) -> LegacyRandomSource {
    let mixed = seed
        .wrapping_add(i64::from(x.wrapping_mul(x).wrapping_mul(4_987_142)))
        .wrapping_add(i64::from(x.wrapping_mul(5_947_611)))
        .wrapping_add(i64::from(z.wrapping_mul(z)).wrapping_mul(4_392_871))
        .wrapping_add(i64::from(z.wrapping_mul(389_711)))
        ^ salt;
    LegacyRandomSource::new(mixed)
}

#[cfg(test)]
mod tests {
    use super::*;

    // These fixed vectors are checked against the shipped dyz/dyn class by
    // tools/check_random.py. The differential runner is the independent oracle.
    #[test]
    fn java_seed_zero_i32_vector() {
        let mut random = LegacyRandomSource::new(0);
        assert_eq!(
            (0..5).map(|_| random.next_i32()).collect::<Vec<_>>(),
            [
                -1_155_484_576,
                -723_955_400,
                1_033_096_058,
                -1_690_734_402,
                -1_557_280_266
            ]
        );
    }

    #[test]
    fn java_next_long_signed_low_word_vector() {
        assert_eq!(
            LegacyRandomSource::new(0).next_i64(),
            -4_962_768_465_676_381_896
        );
        assert_eq!(
            LegacyRandomSource::new(1).next_i64(),
            -4_964_420_948_893_066_024
        );
    }

    #[test]
    fn rejected_bound_preserves_state_and_bound_one_consumes() {
        let mut random = LegacyRandomSource::new(0);
        assert!(random.next_bounded_i32(0).is_err());
        assert!(random.next_bounded_i32(-1).is_err());
        assert_eq!(random.next_i32(), -1_155_484_576);
        random.set_seed(0);
        assert_eq!(random.next_bounded_i32(1).unwrap(), 0);
        assert_eq!(random.next_i32(), -723_955_400);
    }

    #[test]
    fn seed_reset_and_zero_bits_preserve_java_consumption() {
        let mut random = LegacyRandomSource::new(-1);
        let first = random.next_i64();
        random.set_seed(-1);
        assert_eq!(random.next_i64(), first);
        random.set_seed(0);
        assert_eq!(random.next(0), 0);
        assert_eq!(random.next_i32(), -723_955_400);
    }
}
