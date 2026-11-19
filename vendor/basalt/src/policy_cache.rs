//! Bounded policy-evaluation cache.
//!
//! The cache follows the `bounded-cache` retention contract without depending
//! on it. It keys decisions by the BLAKE3 digest of a canonical projection
//! over the policy-source identity, the normalized decision inputs, and the
//! capability-context projection. Eviction, promotion, and trimming decisions
//! come from a pure policy; the store defers every result drop until after
//! the guard releases.
//!
//! # Claim boundary
//!
//! A cache hit does not prove policy correctness or revocation freshness.
//! Freshness stays consumer-owned. The store only bounds retention.

// Entry counts are bounded by the configured `u32` capacity, so the widening
// casts used for saturation checks cannot truncate meaningful values.
#![allow(clippy::cast_possible_truncation)]

use std::collections::HashMap;
use std::sync::Mutex;

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

/// A canonical policy-cache key: the BLAKE3 digest of the projection.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PolicyCacheKey([u8; HASH_OUTPUT_LEN]);

impl PolicyCacheKey {
    /// Returns the raw digest bytes.
    #[must_use]
    pub const fn digest(&self) -> &[u8; HASH_OUTPUT_LEN] {
        &self.0
    }
}

impl std::fmt::Debug for PolicyCacheKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl std::fmt::Display for PolicyCacheKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Builds the canonical key for a policy evaluation.
///
/// The projection is an ordered, length-prefixed frame hashed with BLAKE3.
/// Equal inputs always produce equal keys; distinguishable inputs never
/// collide through the framing. Every field in the frame is a required,
/// caller-supplied projection. The frame embeds the field counts so length
/// changes cannot be confused.
#[must_use]
pub fn policy_evaluation_key(
    policy_source_identity: &[u8],
    normalized_inputs: &[u8],
    capability_context: &[u8],
) -> PolicyCacheKey {
    let mut frame: Vec<u8> = Vec::new();
    frame.push(FRAME_VERSION);
    push_bytes(&mut frame, policy_source_identity);
    push_bytes(&mut frame, normalized_inputs);
    push_bytes(&mut frame, capability_context);
    let digest = blake3::hash(&frame);
    PolicyCacheKey(*digest.as_bytes())
}

fn push_bytes(frame: &mut Vec<u8>, bytes: &[u8]) {
    push_u64(frame, bytes.len() as u64);
    frame.extend_from_slice(bytes);
}

fn push_u64(frame: &mut Vec<u8>, value: u64) {
    frame.extend_from_slice(&value.to_le_bytes());
}

/// A promotion decision from the policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromotionDecision {
    /// The entry qualifies as most recently used and must promote.
    Promote,
    /// The entry is too recent to promote under the threshold.
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

impl std::fmt::Display for PolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ThresholdOutOfRange(value) => {
                write!(formatter, "promotion threshold {value} exceeds 100")
            }
            Self::WatermarkOutOfRange(value) => {
                write!(formatter, "watermark percentage {value} exceeds 100")
            }
            Self::ReversedWatermarks { high, low } => {
                write!(
                    formatter,
                    "high watermark {high} is below low watermark {low}"
                )
            }
        }
    }
}

impl std::error::Error for PolicyError {}

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
    pub const fn new(
        capacity: u32,
        promotion_threshold: u32,
        high_watermark: u32,
        low_watermark: u32,
    ) -> Result<Self, PolicyError> {
        if promotion_threshold > 100 {
            return Err(PolicyError::ThresholdOutOfRange(promotion_threshold));
        }
        if high_watermark > 100 {
            return Err(PolicyError::WatermarkOutOfRange(high_watermark));
        }
        if low_watermark > 100 {
            return Err(PolicyError::WatermarkOutOfRange(low_watermark));
        }
        if high_watermark < low_watermark {
            return Err(PolicyError::ReversedWatermarks {
                high: high_watermark,
                low: low_watermark,
            });
        }
        Ok(Self {
            capacity,
            threshold_age: threshold_to_age(promotion_threshold, capacity),
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

    /// Evaluates whether an entry qualifies for promotion.
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
    where
        I: IntoIterator<Item = (K, u64)>,
    {
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

/// A small inline buffer that defers one result drop until after unlock.
struct DeferredDrop<T> {
    items: [Option<T>; 1],
}

impl<T> DeferredDrop<T> {
    fn new() -> Self {
        Self {
            items: core::array::from_fn(|_| None),
        }
    }

    fn push(&mut self, item: T) {
        match &mut self.items[0] {
            slot @ None => *slot = Some(item),
            Some(_) => panic!("deferred drop capacity exceeded"),
        }
    }
}

/// One cached policy result with its promotion stamp.
struct Entry<V> {
    value: V,
    last_promoted: u64,
}

/// The state under one lock.
struct Inner<V> {
    entries: HashMap<PolicyCacheKey, Entry<V>>,
    policy: CachePolicy,
    generation: u64,
}

/// A read-only policy snapshot for telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicySnapshot {
    /// The configured capacity.
    pub capacity: u32,
    /// The active entry count.
    pub active: u32,
    /// The precomputed promotion age threshold.
    pub threshold_age: u64,
    /// The high watermark in entries.
    pub high_watermark_entries: u32,
    /// The low watermark in entries.
    pub low_watermark_entries: u32,
}

/// A bounded policy-evaluation cache.
pub struct PolicyEvaluationCache<V> {
    inner: Mutex<Inner<V>>,
}

impl<V> PolicyEvaluationCache<V> {
    /// Builds a cache from explicit bounds.
    ///
    /// # Errors
    ///
    /// Returns `PolicyError` when the threshold or a watermark is out of
    /// range, or when the watermarks are reversed.
    pub fn new(
        capacity: u32,
        promotion_threshold: u32,
        high_watermark: u32,
        low_watermark: u32,
    ) -> Result<Self, PolicyError> {
        let policy =
            CachePolicy::new(capacity, promotion_threshold, high_watermark, low_watermark)?;
        Ok(Self {
            inner: Mutex::new(Inner {
                entries: HashMap::new(),
                policy,
                generation: 0,
            }),
        })
    }

    /// Inserts a result, evicting the policy-selected candidate when full.
    ///
    /// An existing key is replaced. Every evicted or replaced result is
    /// dropped strictly after the guard releases.
    ///
    /// # Panics
    ///
    /// Panics on lock poison or an exhausted deferred-drop buffer.
    #[must_use]
    pub fn insert(&self, key: PolicyCacheKey, value: V) -> bool {
        let mut guard = self.inner.lock().expect("cache mutex poisoned");
        let mut deferred = DeferredDrop::new();

        if guard.entries.contains_key(&key) {
            let entry = guard.entries.get_mut(&key).expect("key is present");
            let old = core::mem::replace(&mut entry.value, value);
            deferred.push(old);
            guard.maybe_promote(&key);
            drop(guard);
            drop(deferred);
            return true;
        }

        let active = guard.entries.len() as u32;
        if guard.policy.can_place_without_eviction(active) {
            guard.place(key, value);
            drop(guard);
            drop(deferred);
            return true;
        }

        if guard.policy.eviction_required(active) {
            let candidate = CachePolicy::eviction_candidate(
                guard
                    .entries
                    .iter()
                    .map(|(candidate_key, entry)| (*candidate_key, entry.last_promoted)),
            );
            let Some(candidate) = candidate else {
                drop(guard);
                drop(deferred);
                drop(value);
                return false;
            };
            let evicted = guard
                .entries
                .remove(&candidate)
                .expect("candidate is present in the map");
            deferred.push(evicted.value);
            guard.place(key, value);
            drop(guard);
            drop(deferred);
            return true;
        }

        drop(guard);
        drop(deferred);
        drop(value);
        false
    }

    /// Returns the cloned result for a key, promoting per policy.
    ///
    /// Returns `None` when the key is absent.
    ///
    /// # Panics
    ///
    /// Panics on lock poison.
    pub fn lookup(&self, key: &PolicyCacheKey) -> Option<V>
    where
        V: Clone,
    {
        let mut guard = self.inner.lock().expect("cache mutex poisoned");
        guard.maybe_promote(key);
        let value = guard.entries.get(key).map(|entry| entry.value.clone());
        drop(guard);
        value
    }

    /// Removes a key. Returns true when the key was present.
    ///
    /// The removed result is dropped strictly after the guard releases.
    ///
    /// # Panics
    ///
    /// Panics on lock poison or an exhausted deferred-drop buffer.
    #[must_use]
    pub fn remove(&self, key: &PolicyCacheKey) -> bool {
        let mut guard = self.inner.lock().expect("cache mutex poisoned");
        let mut deferred = DeferredDrop::new();
        let removed = match guard.entries.remove(key) {
            Some(entry) => {
                deferred.push(entry.value);
                true
            }
            None => false,
        };
        drop(guard);
        drop(deferred);
        removed
    }

    /// Returns the number of active entries.
    ///
    /// # Panics
    ///
    /// Panics on lock poison.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .expect("cache mutex poisoned")
            .entries
            .len()
    }

    /// Returns true when the cache holds no results.
    ///
    /// # Panics
    ///
    /// Panics on lock poison.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Applies proactive trimming per the watermark policy.
    ///
    /// When `force` is false, trimming runs only above the high watermark and
    /// stops at the low watermark. Each eviction drops its result after the
    /// guard releases.
    ///
    /// # Panics
    ///
    /// Panics on lock poison or an exhausted deferred-drop buffer.
    #[must_use]
    pub fn trim(&self, force: bool) -> usize {
        let mut trimmed = 0usize;
        loop {
            let mut guard = self.inner.lock().expect("cache mutex poisoned");
            let active = guard.entries.len() as u32;
            let should_trim = force || guard.policy.should_trim(active);
            if !should_trim {
                break;
            }
            // Watermark stopping applies only to proactive trimming.
            // Forced trimming runs the store to empty.
            if !force && guard.policy.reached_low_watermark(active) {
                break;
            }
            let candidate = CachePolicy::eviction_candidate(
                guard
                    .entries
                    .iter()
                    .map(|(candidate_key, entry)| (*candidate_key, entry.last_promoted)),
            );
            let Some(candidate) = candidate else {
                break;
            };
            let mut deferred = DeferredDrop::new();
            let evicted = guard
                .entries
                .remove(&candidate)
                .expect("candidate is present in the map");
            deferred.push(evicted.value);
            trimmed += 1;
            drop(guard);
            drop(deferred);
        }
        trimmed
    }

    /// Returns a policy snapshot for telemetry.
    ///
    /// # Panics
    ///
    /// Panics on lock poison.
    #[must_use]
    pub fn policy(&self) -> PolicySnapshot {
        let guard = self.inner.lock().expect("cache mutex poisoned");
        PolicySnapshot {
            capacity: guard.policy.capacity(),
            active: guard.entries.len() as u32,
            threshold_age: guard.policy.threshold_age(),
            high_watermark_entries: guard.policy.high_watermark_entries(),
            low_watermark_entries: guard.policy.low_watermark_entries(),
        }
    }
}

impl<V> Inner<V> {
    fn place(&mut self, key: PolicyCacheKey, value: V) {
        self.generation = self.generation.wrapping_add(1);
        let stamp = self.generation;
        self.entries.insert(
            key,
            Entry {
                value,
                last_promoted: stamp,
            },
        );
    }

    /// Applies the policy promotion decision to one entry by key.
    fn maybe_promote(&mut self, key: &PolicyCacheKey) -> bool {
        let promote = self.entries.get(key).is_some_and(|entry| {
            self.policy.promotion(self.generation, entry.last_promoted)
                == PromotionDecision::Promote
        });
        if !promote {
            return false;
        }
        self.generation = self.generation.wrapping_add(1);
        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_promoted = self.generation;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn key(label: u8) -> PolicyCacheKey {
        PolicyCacheKey([label; 32])
    }

    #[test]
    fn projection_is_deterministic_and_sensitive() {
        let a = policy_evaluation_key(b"policy-a", b"inputs", b"ctx");
        let b = policy_evaluation_key(b"policy-a", b"inputs", b"ctx");
        let c = policy_evaluation_key(b"policy-b", b"inputs", b"ctx");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn insert_lookup_remove_round_trip() {
        let cache = PolicyEvaluationCache::new(8, 0, 90, 85).expect("policy");
        let k = key(7);
        assert!(cache.insert(k, "granted".to_string()));
        assert_eq!(cache.lookup(&k), Some("granted".to_string()));
        assert!(cache.remove(&k));
        assert!(cache.is_empty());
    }

    #[test]
    fn capacity_never_exceeded() {
        let cache = PolicyEvaluationCache::new(4, 0, 90, 85).expect("policy");
        for label in 0..20u8 {
            assert!(cache.insert(key(label), label.to_string()));
        }
        assert_eq!(cache.len(), 4);
    }

    #[test]
    fn strict_lru_evicts_least_recently_used() {
        let cache = PolicyEvaluationCache::new(3, 0, 90, 85).expect("policy");
        for label in 0..3u8 {
            assert!(cache.insert(key(label), label.to_string()));
        }
        cache.lookup(&key(0));
        assert!(cache.insert(key(3), "3".to_string()));
        assert_eq!(cache.lookup(&key(1)), None, "key 1 is now the LRU end");
    }

    #[test]
    fn fifo_never_promotes_on_read() {
        let cache = PolicyEvaluationCache::new(3, 100, 90, 85).expect("policy");
        for label in 0..3u8 {
            assert!(cache.insert(key(label), label.to_string()));
        }
        cache.lookup(&key(0));
        assert!(cache.insert(key(3), "3".to_string()));
        assert_eq!(
            cache.lookup(&key(0)),
            None,
            "reads must not protect entries"
        );
    }

    #[test]
    fn zero_capacity_holds_nothing() {
        let cache = PolicyEvaluationCache::new(0, 0, 90, 85).expect("policy");
        assert!(!cache.insert(key(1), "denied".to_string()));
        assert!(cache.is_empty());
    }

    #[derive(Clone)]
    struct Counted {
        counter: Arc<AtomicUsize>,
    }

    impl Drop for Counted {
        fn drop(&mut self) {
            self.counter.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn evicted_and_replaced_results_drop_exactly_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let make = |counter: &Arc<AtomicUsize>| Counted {
            counter: Arc::clone(counter),
        };
        let cache = PolicyEvaluationCache::new(2, 0, 90, 85).expect("policy");
        assert!(cache.insert(key(1), make(&drops)));
        assert!(cache.insert(key(2), make(&drops)));
        assert!(cache.insert(key(3), make(&drops)));
        assert_eq!(drops.load(Ordering::SeqCst), 1, "one eviction");
        assert!(cache.insert(key(3), make(&drops)));
        assert_eq!(drops.load(Ordering::SeqCst), 2, "one replacement");
        assert!(cache.remove(&key(2)));
        assert_eq!(drops.load(Ordering::SeqCst), 3);
        drop(cache);
        assert_eq!(
            drops.load(Ordering::SeqCst),
            4,
            "all four created results dropped"
        );
    }

    #[test]
    fn watermark_trim_stops_at_low() {
        let cache = PolicyEvaluationCache::new(100, 0, 90, 85).expect("policy");
        for label in 0..100u8 {
            assert!(cache.insert(key(label), label.to_string()));
        }
        let trimmed = cache.trim(false);
        assert!(trimmed > 0);
        let remaining = cache.len();
        assert!(remaining <= 90, "trim starts at the high watermark");
        assert!(remaining >= 85, "trim stops at the low watermark");
    }

    #[test]
    fn forced_trim_empties_store() {
        let cache = PolicyEvaluationCache::new(10, 0, 90, 85).expect("policy");
        for label in 0..10u8 {
            assert!(cache.insert(key(label), label.to_string()));
        }
        let trimmed = cache.trim(true);
        assert_eq!(trimmed, 10);
        assert!(cache.is_empty());
    }

    #[test]
    fn reversed_watermarks_negative() {
        let error = PolicyEvaluationCache::<u8>::new(100, 0, 40, 60)
            .err()
            .expect("must fail");
        assert_eq!(error, PolicyError::ReversedWatermarks { high: 40, low: 60 });
    }

    #[test]
    fn threshold_out_of_range_negative() {
        let error = PolicyEvaluationCache::<u8>::new(100, 101, 90, 85)
            .err()
            .expect("must fail");
        assert_eq!(error, PolicyError::ThresholdOutOfRange(101));
    }
}
