//! Bounded verification-cache store.
//!
//! The store executes the retention decisions of
//! [`ucan_core::verification_cache::CachePolicy`]. It owns the lock, the
//! bounded map, and the deferred release of evicted and replaced results.
//!
//! # Claim boundary
//!
//! The store bounds retention and defers releases. It does not prove
//! verification correctness or revocation freshness. Verifier authority stays
//! with the token verifier.

// Entry counts are bounded by the configured `u32` capacity, so the widening
// casts used for saturation checks cannot truncate meaningful values.
#![allow(clippy::cast_possible_truncation)]

use std::collections::HashMap;
use std::sync::Mutex;

use ucan_core::verification_cache::CachePolicy;
use ucan_core::verification_cache::PolicyError;
use ucan_core::verification_cache::PromotionDecision;
use ucan_core::verification_cache::VerificationKey;

/// A small inline buffer that defers one result drop until after unlock.
///
/// No store operation evicts or replaces more than one result, so capacity
/// one is sufficient and the buffer never allocates.
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

/// One cached verification result with its promotion stamp.
struct Entry<V> {
    value: V,
    last_promoted: u64,
}

/// The state under one lock.
struct Inner<V> {
    entries: HashMap<VerificationKey, Entry<V>>,
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

/// A bounded verification-result cache.
pub struct VerificationCache<V> {
    inner: Mutex<Inner<V>>,
}

impl<V> VerificationCache<V> {
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
        let policy = CachePolicy::new(capacity, promotion_threshold, high_watermark, low_watermark)?;
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
    pub fn insert(&self, key: VerificationKey, value: V) -> bool {
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
            guard.generation = guard.generation.wrapping_add(1);
            let stamp = guard.generation;
            guard.entries.insert(key, Entry {
                value,
                last_promoted: stamp,
            });
            drop(guard);
            drop(deferred);
            return true;
        }

        if guard.policy.eviction_required(active) {
            let candidate = CachePolicy::eviction_candidate(
                guard.entries.iter().map(|(candidate_key, entry)| (*candidate_key, entry.last_promoted)),
            );
            let Some(candidate) = candidate else {
                drop(guard);
                drop(deferred);
                drop(value);
                return false;
            };
            let evicted = guard.entries.remove(&candidate).expect("candidate is present in the map");
            deferred.push(evicted.value);
            guard.generation = guard.generation.wrapping_add(1);
            let stamp = guard.generation;
            guard.entries.insert(key, Entry {
                value,
                last_promoted: stamp,
            });
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
    pub fn lookup(&self, key: &VerificationKey) -> Option<V>
    where V: Clone {
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
    pub fn remove(&self, key: &VerificationKey) -> bool {
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
        self.inner.lock().expect("cache mutex poisoned").entries.len()
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
                guard.entries.iter().map(|(candidate_key, entry)| (*candidate_key, entry.last_promoted)),
            );
            let Some(candidate) = candidate else {
                break;
            };
            let mut deferred = DeferredDrop::new();
            let evicted = guard.entries.remove(&candidate).expect("candidate is present in the map");
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
    /// Applies the policy promotion decision to one entry by key.
    fn maybe_promote(&mut self, key: &VerificationKey) -> bool {
        let promote = self.entries.get(key).is_some_and(|entry| {
            self.policy.promotion(self.generation, entry.last_promoted) == PromotionDecision::Promote
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
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use super::*;

    fn key(label: u8) -> VerificationKey {
        VerificationKey::from_bytes([label; 32])
    }

    #[test]
    fn insert_lookup_remove_round_trip() {
        let cache = VerificationCache::new(8, 0, 90, 85).expect("policy");
        let k = key(7);
        assert!(cache.insert(k, "seven".to_string()));
        assert_eq!(cache.lookup(&k), Some("seven".to_string()));
        assert_eq!(cache.len(), 1);
        assert!(cache.remove(&k));
        assert!(cache.is_empty());
    }

    #[test]
    fn capacity_never_exceeded() {
        let cache = VerificationCache::new(4, 0, 90, 85).expect("policy");
        for label in 0..20u8 {
            assert!(cache.insert(key(label), label.to_string()));
        }
        assert_eq!(cache.len(), 4);
    }

    #[test]
    fn strict_lru_evicts_least_recently_used() {
        let cache = VerificationCache::new(3, 0, 90, 85).expect("policy");
        for label in 0..3u8 {
            assert!(cache.insert(key(label), label.to_string()));
        }
        // Touching key 0 moves it to the most recently used end.
        cache.lookup(&key(0));
        assert!(cache.insert(key(3), "3".to_string()));
        assert_eq!(cache.lookup(&key(1)), None, "key 1 is now the LRU end");
        assert_eq!(cache.lookup(&key(0)), Some("0".to_string()));
        assert_eq!(cache.lookup(&key(3)), Some("3".to_string()));
    }

    #[test]
    fn fifo_never_promotes_on_read() {
        let cache = VerificationCache::new(3, 100, 90, 85).expect("policy");
        for label in 0..3u8 {
            assert!(cache.insert(key(label), label.to_string()));
        }
        cache.lookup(&key(0));
        assert!(cache.insert(key(3), "3".to_string()));
        assert_eq!(cache.lookup(&key(0)), None, "reads must not protect entries");
        assert_eq!(cache.lookup(&key(1)), Some("1".to_string()));
    }

    #[test]
    fn zero_capacity_holds_nothing() {
        let cache = VerificationCache::new(0, 0, 90, 85).expect("policy");
        assert!(!cache.insert(key(1), "one".to_string()));
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
        let cache = VerificationCache::new(2, 0, 90, 85).expect("policy");
        assert!(cache.insert(key(1), make(&drops)));
        assert!(cache.insert(key(2), make(&drops)));
        assert!(cache.insert(key(3), make(&drops)));
        assert_eq!(drops.load(Ordering::SeqCst), 1, "one eviction");
        assert!(cache.insert(key(3), make(&drops)));
        assert_eq!(drops.load(Ordering::SeqCst), 2, "one replacement");
        assert!(cache.remove(&key(2)));
        assert_eq!(drops.load(Ordering::SeqCst), 3);
        drop(cache);
        assert_eq!(drops.load(Ordering::SeqCst), 4, "all four created results dropped");
    }

    #[test]
    fn watermark_trim_stops_at_low() {
        let cache = VerificationCache::new(100, 0, 90, 85).expect("policy");
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
        let cache = VerificationCache::new(10, 0, 90, 85).expect("policy");
        for label in 0..10u8 {
            assert!(cache.insert(key(label), label.to_string()));
        }
        let trimmed = cache.trim(true);
        assert_eq!(trimmed, 10);
        assert!(cache.is_empty());
    }

    #[test]
    fn reversed_watermarks_negative() {
        let error = VerificationCache::<u8>::new(100, 0, 40, 60).err().expect("must fail");
        assert_eq!(error, PolicyError::ReversedWatermarks { high: 40, low: 60 });
    }

    #[test]
    fn threshold_out_of_range_negative() {
        let error = VerificationCache::<u8>::new(100, 101, 90, 85).err().expect("must fail");
        assert_eq!(error, PolicyError::ThresholdOutOfRange(101));
    }
}
