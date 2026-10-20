//! Canonical verification-cache projection and bounded retention decisions.
//!
//! This module follows the `bounded-cache` retention contract without
//! depending on it. UCAN owns the projection and retention decisions here so
//! the standard shell can later adopt a published `bounded-cache` revision
//! without changing the core contract.
//!
//! The module is `no_std`. It reads no clocks, performs no I/O, and holds no
//! locks.
//!
//! # Claim boundary
//!
//! This module proves deterministic projection and retention over supplied
//! values. It does not prove verification correctness, freshness, or
//! key-distribution quality.

use alloc::vec::Vec;
use core::fmt;

/// Byte length of a BLAKE3 digest.
pub const HASH_OUTPUT_LEN: usize = 32;

/// Frame version prefix for the canonical projection.
const FRAME_VERSION: u8 = 1;

/// Strict-LRU promotion age.
const STRICT_LRU_AGE: u64 = 0;

/// FIFO promotion age.
const FIFO_AGE: u64 = u64::MAX;

/// Percentage denominator for thresholds and watermarks.
const PERCENT_BASE: u64 = 100;

/// A canonical verification-cache key: the BLAKE3 digest of the projection.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct VerificationKey([u8; HASH_OUTPUT_LEN]);

impl VerificationKey {
    /// Returns the raw digest bytes.
    #[must_use]
    pub const fn digest(&self) -> &[u8; HASH_OUTPUT_LEN] {
        &self.0
    }

    /// Builds a key from a raw digest.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; HASH_OUTPUT_LEN]) -> Self {
        Self(bytes)
    }
}

impl fmt::Debug for VerificationKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl fmt::Display for VerificationKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// A projection failed because required fields are absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionError {
    /// The issuer projection is empty.
    EmptyIssuer,
    /// The audience projection is empty.
    EmptyAudience,
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIssuer => formatter.write_str("issuer projection must not be empty"),
            Self::EmptyAudience => formatter.write_str("audience projection must not be empty"),
        }
    }
}

impl core::error::Error for ProjectionError {}

/// The verification-relevant projection of one capability.
#[derive(Debug, Clone, Copy)]
pub struct CapabilityProjection<'a> {
    /// The canonical resource bytes.
    pub resource: &'a [u8],
    /// The canonical ability bytes.
    pub ability: &'a [u8],
    /// The canonical serialized caveats.
    pub caveats: &'a [u8],
}

/// Exactly the fields a verifier reads to decide an outcome.
///
/// The projection excludes payload bytes that do not affect the verdict.
#[derive(Debug, Clone, Copy)]
pub struct VerificationInputs<'a> {
    /// The issuer identity bytes.
    pub issuer: &'a [u8],
    /// The audience identity bytes.
    pub audience: &'a [u8],
    /// The optional nonce.
    pub nonce: Option<&'a [u8]>,
    /// The capability projections in verification order.
    pub capabilities: &'a [CapabilityProjection<'a>],
    /// The proof reference bytes in verification order.
    pub proofs: &'a [&'a [u8]],
    /// The optional not-before time bound.
    pub not_before: Option<u64>,
    /// The optional expiration time bound.
    pub expires_at: Option<u64>,
}

/// Builds the canonical verification-cache key for supplied inputs.
///
/// The projection is an ordered, length-prefixed frame hashed with BLAKE3.
/// Equal inputs always produce equal keys. Distinguishable inputs never
/// collide through the framing: each field is prefixed by its byte length.
///
/// # Errors
///
/// Returns `ProjectionError::EmptyIssuer` or `ProjectionError::EmptyAudience`
/// when a required identity projection is empty.
pub fn projection_key(inputs: &VerificationInputs<'_>) -> Result<VerificationKey, ProjectionError> {
    if inputs.issuer.is_empty() {
        return Err(ProjectionError::EmptyIssuer);
    }
    if inputs.audience.is_empty() {
        return Err(ProjectionError::EmptyAudience);
    }
    let mut frame = Vec::new();
    push_frame_prefix(&mut frame);
    push_bytes(&mut frame, inputs.issuer);
    push_bytes(&mut frame, inputs.audience);
    push_optional_bytes(&mut frame, inputs.nonce);
    push_count(&mut frame, inputs.capabilities.len());
    for capability in inputs.capabilities {
        push_bytes(&mut frame, capability.resource);
        push_bytes(&mut frame, capability.ability);
        push_bytes(&mut frame, capability.caveats);
    }
    push_count(&mut frame, inputs.proofs.len());
    for proof in inputs.proofs {
        push_bytes(&mut frame, proof);
    }
    push_optional_u64(&mut frame, inputs.not_before);
    push_optional_u64(&mut frame, inputs.expires_at);
    let digest = blake3::hash(&frame);
    Ok(VerificationKey(*digest.as_bytes()))
}

fn push_frame_prefix(frame: &mut Vec<u8>) {
    frame.push(FRAME_VERSION);
}

fn push_bytes(frame: &mut Vec<u8>, bytes: &[u8]) {
    push_u64(frame, bytes.len() as u64);
    frame.extend_from_slice(bytes);
}

fn push_optional_bytes(frame: &mut Vec<u8>, bytes: Option<&[u8]>) {
    if let Some(bytes) = bytes {
        push_u64(frame, 0);
        push_bytes(frame, bytes);
    } else {
        push_u64(frame, 1);
        push_bytes(frame, &[]);
    }
}

fn push_optional_u64(frame: &mut Vec<u8>, value: Option<u64>) {
    if let Some(value) = value {
        push_u64(frame, 0);
        push_u64(frame, value);
    } else {
        push_u64(frame, 1);
        push_u64(frame, 0);
    }
}

fn push_count(frame: &mut Vec<u8>, count: usize) {
    push_u64(frame, count as u64);
}

fn push_u64(frame: &mut Vec<u8>, value: u64) {
    frame.extend_from_slice(&value.to_le_bytes());
}

/// A retention policy following the `bounded-cache` contract.
///
/// Every bound is explicit. The policy reads no clock, memory, or scheduler
/// state and allocates nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CachePolicy {
    capacity: u32,
    threshold_age: u64,
    high_entries: u32,
    low_entries: u32,
}

impl CachePolicy {
    /// Builds a policy from explicit bounds.
    ///
    /// A threshold of 0 selects strict LRU promotion. A threshold of 100
    /// selects FIFO retention.
    ///
    /// # Errors
    ///
    /// Returns `PolicyError` when the threshold or a watermark is out of
    /// range, or when the high watermark is below the low watermark.
    pub fn new(
        capacity: u32,
        promotion_threshold: u32,
        high_watermark: u32,
        low_watermark: u32,
    ) -> Result<Self, PolicyError> {
        if promotion_threshold > 100 {
            return Err(PolicyError::ThresholdOutOfRange(promotion_threshold));
        }
        validate_watermark(high_watermark)?;
        validate_watermark(low_watermark)?;
        if high_watermark < low_watermark {
            return Err(PolicyError::ReversedWatermarks {
                high: high_watermark,
                low: low_watermark,
            });
        }
        let threshold_age = threshold_to_age(promotion_threshold, capacity);
        Ok(Self {
            capacity,
            threshold_age,
            high_entries: proportion_entries(capacity, high_watermark),
            low_entries: proportion_entries(capacity, low_watermark),
        })
    }

    /// Returns the configured capacity.
    #[must_use]
    pub const fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Returns the precomputed promotion age threshold.
    #[must_use]
    pub const fn threshold_age(&self) -> u64 {
        self.threshold_age
    }

    /// Returns the high watermark in entries.
    #[must_use]
    pub const fn high_watermark_entries(&self) -> u32 {
        self.high_entries
    }

    /// Returns the low watermark in entries.
    #[must_use]
    pub const fn low_watermark_entries(&self) -> u32 {
        self.low_entries
    }

    /// Returns true when a new entry can be placed without an eviction.
    #[must_use]
    pub const fn can_place_without_eviction(&self, active: u32) -> bool {
        active < self.capacity
    }

    /// Returns true when an eviction is required before placement.
    #[must_use]
    pub const fn eviction_required(&self, active: u32) -> bool {
        active >= self.capacity && self.capacity > 0
    }

    /// Evaluates whether a node qualifies for promotion.
    ///
    /// The subtraction wraps deliberately so generation rollover stays
    /// deterministic.
    #[must_use]
    pub const fn promotion(&self, generation: u64, last_promoted: u64) -> PromotionDecision {
        if generation.wrapping_sub(last_promoted) >= self.threshold_age {
            PromotionDecision::Promote
        } else {
            PromotionDecision::Skip
        }
    }

    /// Returns true when proactive trimming should run for the observed
    /// active count.
    #[must_use]
    pub const fn should_trim(&self, active: u32) -> bool {
        active > self.high_entries
    }

    /// Returns true when the active count has reached the low watermark.
    #[must_use]
    pub const fn reached_low_watermark(&self, active: u32) -> bool {
        active <= self.low_entries
    }

    /// Selects the deterministic eviction candidate from supplied
    /// `(identifier, promotion stamp)` pairs.
    ///
    /// The candidate with the smallest stamp is selected. Ties keep the first
    /// encountered item.
    pub fn eviction_candidate<I, K>(items: I) -> Option<K>
    where I: IntoIterator<Item = (K, u64)> {
        let mut best: Option<(K, u64)> = None;
        for item in items {
            match best {
                None => best = Some(item),
                Some((_, best_stamp)) if item.1 < best_stamp => best = Some(item),
                Some(_) => {}
            }
        }
        best.map(|(identifier, _)| identifier)
    }
}

/// A promotion decision from the policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromotionDecision {
    /// The node qualifies as most recently used and must promote.
    Promote,
    /// The node is too recent to promote under the threshold.
    Skip,
}

/// A supplied policy bound is invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyError {
    /// The promotion threshold percentage exceeds 100.
    ThresholdOutOfRange(u32),
    /// A watermark percentage exceeds 100.
    WatermarkOutOfRange(u32),
    /// The high watermark is strictly below the low watermark.
    ReversedWatermarks { high: u32, low: u32 },
}

impl fmt::Display for PolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ThresholdOutOfRange(value) => {
                write!(formatter, "promotion threshold {value} exceeds 100")
            }
            Self::WatermarkOutOfRange(value) => {
                write!(formatter, "watermark percentage {value} exceeds 100")
            }
            Self::ReversedWatermarks { high, low } => {
                write!(formatter, "high watermark {high} is below low watermark {low}")
            }
        }
    }
}

impl core::error::Error for PolicyError {}

const fn validate_watermark(value: u32) -> Result<(), PolicyError> {
    if value > 100 {
        Err(PolicyError::WatermarkOutOfRange(value))
    } else {
        Ok(())
    }
}

/// Maps a promotion threshold and capacity to a generation threshold age.
#[must_use]
#[allow(clippy::cast_lossless, clippy::cast_possible_truncation)]
pub const fn threshold_to_age(threshold: u32, capacity: u32) -> u64 {
    match threshold {
        0 => STRICT_LRU_AGE,
        100 => FIFO_AGE,
        value => ((capacity as u64) * (value as u64)) / PERCENT_BASE,
    }
}

/// Computes the entry count for a percentage of a capacity.
#[must_use]
#[allow(clippy::cast_lossless, clippy::cast_possible_truncation)]
const fn proportion_entries(capacity: u32, proportion: u32) -> u32 {
    ((capacity as u64) * (proportion as u64) / PERCENT_BASE) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs<'a>(issuer: &'a str, audience: &'a str) -> VerificationInputs<'a> {
        VerificationInputs {
            issuer: issuer.as_bytes(),
            audience: audience.as_bytes(),
            nonce: None,
            capabilities: &[],
            proofs: &[],
            not_before: None,
            expires_at: None,
        }
    }

    #[test]
    fn projection_is_deterministic() {
        let first = projection_key(&inputs("issuer-a", "audience-b")).expect("projection");
        let second = projection_key(&inputs("issuer-a", "audience-b")).expect("projection");
        assert_eq!(first, second);
    }

    #[test]
    fn projection_distinguishes_issuer() {
        let a = projection_key(&inputs("issuer-a", "audience-b")).expect("projection");
        let b = projection_key(&inputs("issuer-b", "audience-b")).expect("projection");
        assert_ne!(a, b);
    }

    #[test]
    fn projection_distinguishes_optional_absence() {
        let none = inputs("a", "b");
        let empty = VerificationInputs {
            nonce: Some(b""),
            ..none
        };
        let key_none = projection_key(&none).expect("projection");
        let key_empty = projection_key(&empty).expect("projection");
        assert_ne!(key_none, key_empty);
    }

    #[test]
    fn projection_distinguishes_capability_order() {
        let cap_a = CapabilityProjection {
            resource: b"r1",
            ability: b"a1",
            caveats: b"",
        };
        let cap_b = CapabilityProjection {
            resource: b"r2",
            ability: b"a2",
            caveats: b"",
        };
        let forward = [cap_a, cap_b];
        let reverse = [cap_b, cap_a];
        let base = inputs("issuer", "audience");
        let key_forward = projection_key(&VerificationInputs {
            capabilities: &forward,
            ..base
        })
        .expect("projection");
        let key_reverse = projection_key(&VerificationInputs {
            capabilities: &reverse,
            ..base
        })
        .expect("projection");
        assert_ne!(key_forward, key_reverse);
    }
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    #[test]
    fn empty_issuer_negative() {
        let error = projection_key(&VerificationInputs {
            issuer: b"",
            audience: b"aud",
            nonce: None,
            capabilities: &[],
            proofs: &[],
            not_before: None,
            expires_at: None,
        })
        .expect_err("empty issuer must fail");
        assert_eq!(error, ProjectionError::EmptyIssuer);
    }

    #[test]
    fn strict_lru_promotes_always() {
        let policy = CachePolicy::new(100, 0, 90, 85).expect("policy");
        assert_eq!(policy.promotion(10, 1), PromotionDecision::Promote);
    }

    #[test]
    fn fifo_never_promotes() {
        let policy = CachePolicy::new(100, 100, 90, 85).expect("policy");
        assert_eq!(policy.promotion(10, 9), PromotionDecision::Skip);
    }

    #[test]
    fn lazy_threshold_promotes_only_aged() {
        let policy = CachePolicy::new(100, 50, 90, 85).expect("policy");
        assert_eq!(policy.threshold_age(), 50);
        assert_eq!(policy.promotion(10, 0), PromotionDecision::Skip);
        assert_eq!(policy.promotion(50, 0), PromotionDecision::Promote);
    }

    #[test]
    fn capacity_boundaries() {
        let policy = CachePolicy::new(3, 0, 90, 85).expect("policy");
        assert!(policy.can_place_without_eviction(2));
        assert!(!policy.can_place_without_eviction(3));
        assert!(policy.eviction_required(3));
        assert!(!policy.eviction_required(2));
    }

    #[test]
    fn zero_capacity_boundaries() {
        let policy = CachePolicy::new(0, 0, 90, 85).expect("policy");
        assert!(!policy.can_place_without_eviction(0));
        assert!(!policy.eviction_required(0));
    }

    #[test]
    fn watermark_boundaries() {
        let policy = CachePolicy::new(100, 0, 90, 85).expect("policy");
        assert!(!policy.should_trim(90));
        assert!(policy.should_trim(91));
        assert!(policy.reached_low_watermark(85));
        assert!(!policy.reached_low_watermark(86));
    }

    #[test]
    fn threshold_out_of_range_negative() {
        assert_eq!(CachePolicy::new(100, 101, 90, 85), Err(PolicyError::ThresholdOutOfRange(101)));
    }

    #[test]
    fn reversed_watermarks_negative() {
        assert_eq!(CachePolicy::new(100, 0, 40, 60), Err(PolicyError::ReversedWatermarks { high: 40, low: 60 }));
    }

    #[test]
    fn eviction_candidate_selects_smallest_stamp() {
        let items = [("a", 5u64), ("b", 3), ("c", 9)];
        let candidate = CachePolicy::eviction_candidate(items).expect("candidate");
        assert_eq!(candidate, "b");
    }

    #[test]
    fn eviction_candidate_empty_none() {
        let candidate = CachePolicy::eviction_candidate::<_, &str>(core::iter::empty());
        assert_eq!(candidate, None);
    }
}
