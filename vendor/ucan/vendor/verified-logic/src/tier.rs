//! Verified tier classification for tiered caching.
//!
//! Classifies entries into Hot/Warm/Cold tiers based on access count and
//! idle time thresholds. Provides promotion and demotion functions with
//! verified ordering properties.
//!
//! Generalized from seaglass's tiered-cache `TierThresholds`.

#![allow(clippy::bool_to_int_with_if)]

use vstd::prelude::*;

verus! {

// --- Tier constants ---

/// Cold tier (rarely accessed, lowest eviction resistance).
pub open spec fn COLD() -> u8 { 0 }
/// Warm tier (moderately accessed).
pub open spec fn WARM() -> u8 { 1 }
/// Hot tier (frequently accessed, highest eviction resistance).
pub open spec fn HOT() -> u8 { 2 }

/// Spec: whether a tier value is valid (0, 1, or 2).
pub open spec fn is_valid_tier_spec(tier: u8) -> bool {
    tier <= 2
}

// --- classify_tier ---

// r[depends tier.classify]
/// Spec: classify an entry into a tier based on access count and thresholds.
/// Hot if access_count >= hot_threshold, Warm if >= warm_threshold, Cold otherwise.
pub open spec fn classify_tier_spec(
    access_count: u64,
    warm_threshold: u64,
    hot_threshold: u64,
) -> u8 {
    if access_count >= hot_threshold {
        2 // Hot
    } else if access_count >= warm_threshold {
        1 // Warm
    } else {
        0 // Cold
    }
}

// r[impl tier.classify]
/// Classifies an entry into Hot (2), Warm (1), or Cold (0) based on access count.
///
/// Thresholds must be ordered: `warm_threshold <= hot_threshold`.
pub fn classify_tier(
    access_count: u64,
    warm_threshold: u64,
    hot_threshold: u64,
) -> (result: u8)
    requires warm_threshold <= hot_threshold
    ensures result == classify_tier_spec(access_count, warm_threshold, hot_threshold)
{
    if access_count >= hot_threshold {
        2
    } else if access_count >= warm_threshold {
        1
    } else {
        0
    }
}

// r[verify tier.classify]
/// Classification result is always a valid tier.
proof fn classify_tier_valid(access_count: u64, warm_threshold: u64, hot_threshold: u64)
    requires warm_threshold <= hot_threshold
    ensures is_valid_tier_spec(classify_tier_spec(access_count, warm_threshold, hot_threshold))
{}

// r[verify tier.classify]
/// Classification is monotonic: more accesses => same or higher tier.
proof fn classify_tier_monotonic(
    count1: u64, count2: u64,
    warm_threshold: u64, hot_threshold: u64,
)
    requires warm_threshold <= hot_threshold, count1 <= count2
    ensures
        classify_tier_spec(count1, warm_threshold, hot_threshold)
        <= classify_tier_spec(count2, warm_threshold, hot_threshold)
{}

// r[verify tier.classify]
/// Zero accesses is always Cold.
proof fn zero_accesses_is_cold(warm_threshold: u64, hot_threshold: u64)
    requires warm_threshold <= hot_threshold, warm_threshold > 0
    ensures classify_tier_spec(0, warm_threshold, hot_threshold) == COLD()
{}

// r[verify tier.classify]
/// Accesses at or above hot threshold is always Hot.
proof fn above_hot_is_hot(access_count: u64, warm_threshold: u64, hot_threshold: u64)
    requires warm_threshold <= hot_threshold, access_count >= hot_threshold
    ensures classify_tier_spec(access_count, warm_threshold, hot_threshold) == HOT()
{}

// r[verify tier.classify]
/// When thresholds are equal, only Cold and Hot exist (no Warm).
proof fn equal_thresholds_no_warm(access_count: u64, threshold: u64)
    ensures
        classify_tier_spec(access_count, threshold, threshold) == COLD()
        || classify_tier_spec(access_count, threshold, threshold) == HOT(),
{}

// --- promote ---

// r[depends tier.promote]
/// Spec: promote to the next higher tier.
/// Cold -> Warm, Warm -> Hot, Hot -> Hot (already max).
pub open spec fn promote_spec(tier: u8) -> u8 {
    if tier >= 2 { 2 } else { (tier + 1) as u8 }
}

// r[impl tier.promote]
/// Promotes an entry to the next higher tier. Hot stays Hot.
pub fn promote(tier: u8) -> (result: u8)
    requires is_valid_tier_spec(tier)
    ensures result == promote_spec(tier)
{
    if tier >= 2 { 2 } else { tier + 1 }
}

// r[verify tier.promote]
/// Promotion never decreases tier.
proof fn promote_ge(tier: u8)
    requires is_valid_tier_spec(tier)
    ensures promote_spec(tier) >= tier
{}

// r[verify tier.promote]
/// Promotion result is always valid.
proof fn promote_valid(tier: u8)
    requires is_valid_tier_spec(tier)
    ensures is_valid_tier_spec(promote_spec(tier))
{}

// r[verify tier.promote]
/// Promoting Hot is idempotent.
proof fn promote_hot_idempotent()
    ensures promote_spec(HOT()) == HOT()
{}

// r[verify tier.promote]
/// Cold promotes to Warm, Warm promotes to Hot.
proof fn promote_steps()
    ensures
        promote_spec(COLD()) == WARM(),
        promote_spec(WARM()) == HOT(),
{}

// --- demote ---

// r[depends tier.demote]
/// Spec: demote to the next lower tier.
/// Hot -> Warm, Warm -> Cold, Cold -> Cold (already min).
pub open spec fn demote_spec(tier: u8) -> u8 {
    if tier == 0 { 0 } else { (tier - 1) as u8 }
}

// r[impl tier.demote]
/// Demotes an entry to the next lower tier. Cold stays Cold.
pub fn demote(tier: u8) -> (result: u8)
    requires is_valid_tier_spec(tier)
    ensures result == demote_spec(tier)
{
    if tier == 0 { 0 } else { tier - 1 }
}

// r[verify tier.demote]
/// Demotion never increases tier.
proof fn demote_le(tier: u8)
    requires is_valid_tier_spec(tier)
    ensures demote_spec(tier) <= tier
{}

// r[verify tier.demote]
/// Demotion result is always valid.
proof fn demote_valid(tier: u8)
    requires is_valid_tier_spec(tier)
    ensures is_valid_tier_spec(demote_spec(tier))
{}

// r[verify tier.demote]
/// Demoting Cold is idempotent.
proof fn demote_cold_idempotent()
    ensures demote_spec(COLD()) == COLD()
{}

// r[verify tier.demote]
/// Hot demotes to Warm, Warm demotes to Cold.
proof fn demote_steps()
    ensures
        demote_spec(HOT()) == WARM(),
        demote_spec(WARM()) == COLD(),
{}

// --- promote/demote roundtrip ---

// r[verify tier.promote]
// r[verify tier.demote]
/// Promote then demote returns the original tier (for non-Hot).
proof fn promote_demote_roundtrip(tier: u8)
    requires is_valid_tier_spec(tier), tier < HOT()
    ensures demote_spec(promote_spec(tier)) == tier
{}

// r[verify tier.promote]
// r[verify tier.demote]
/// Demote then promote returns the original tier (for non-Cold).
proof fn demote_promote_roundtrip(tier: u8)
    requires is_valid_tier_spec(tier), tier > COLD()
    ensures promote_spec(demote_spec(tier)) == tier
{}

// --- should_demote ---

// r[depends tier.should-demote]
/// Spec: whether an entry should be demoted based on idle time.
/// Only entries above Cold can be demoted, and only when idle time
/// meets or exceeds the cold threshold.
pub open spec fn should_demote_spec(
    current_tier: u8,
    idle_time: u64,
    cold_idle_threshold: u64,
) -> bool {
    current_tier > COLD() && idle_time >= cold_idle_threshold
}

// r[impl tier.should-demote]
/// Returns true if the entry should be demoted based on idle time.
pub fn should_demote(
    current_tier: u8,
    idle_time: u64,
    cold_idle_threshold: u64,
) -> (result: bool)
    ensures result == should_demote_spec(current_tier, idle_time, cold_idle_threshold)
{
    current_tier > 0 && idle_time >= cold_idle_threshold
}

// r[verify tier.should-demote]
/// Cold entries are never demoted.
proof fn cold_never_demoted(idle_time: u64, threshold: u64)
    ensures !should_demote_spec(COLD(), idle_time, threshold)
{}

// r[verify tier.should-demote]
/// Active entries (below threshold) are never demoted.
proof fn active_not_demoted(tier: u8, idle_time: u64, threshold: u64)
    requires idle_time < threshold
    ensures !should_demote_spec(tier, idle_time, threshold)
{}

// r[verify tier.should-demote]
/// Idle Hot entries should be demoted.
proof fn idle_hot_demoted(idle_time: u64, threshold: u64)
    requires idle_time >= threshold
    ensures should_demote_spec(HOT(), idle_time, threshold)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify tier.classify]
    #[test]
    fn test_classify_tier() {
        // Cold: below warm threshold
        assert_eq!(classify_tier(0, 5, 20), 0);
        assert_eq!(classify_tier(4, 5, 20), 0);
        // Warm: between thresholds
        assert_eq!(classify_tier(5, 5, 20), 1);
        assert_eq!(classify_tier(19, 5, 20), 1);
        // Hot: at or above hot threshold
        assert_eq!(classify_tier(20, 5, 20), 2);
        assert_eq!(classify_tier(100, 5, 20), 2);
        // Boundary: warm == hot
        assert_eq!(classify_tier(9, 10, 10), 0);
        assert_eq!(classify_tier(10, 10, 10), 2);
    }

    // r[verify tier.promote]
    #[test]
    fn test_promote() {
        assert_eq!(promote(0), 1); // Cold -> Warm
        assert_eq!(promote(1), 2); // Warm -> Hot
        assert_eq!(promote(2), 2); // Hot -> Hot
    }

    // r[verify tier.demote]
    #[test]
    fn test_demote() {
        assert_eq!(demote(0), 0); // Cold -> Cold
        assert_eq!(demote(1), 0); // Warm -> Cold
        assert_eq!(demote(2), 1); // Hot -> Warm
    }

    // r[verify tier.should-demote]
    #[test]
    fn test_should_demote() {
        // Cold never demoted
        assert!(!should_demote(0, 1000, 300));
        // Hot + idle => demote
        assert!(should_demote(2, 300, 300));
        assert!(should_demote(2, 500, 300));
        // Hot + active => no demote
        assert!(!should_demote(2, 100, 300));
        // Warm + idle => demote
        assert!(should_demote(1, 300, 300));
    }

    // r[verify tier.promote]
    // r[verify tier.demote]
    #[test]
    fn test_roundtrip() {
        // Promote then demote
        assert_eq!(demote(promote(0)), 0);
        assert_eq!(demote(promote(1)), 1);
        // Demote then promote
        assert_eq!(promote(demote(1)), 1);
        assert_eq!(promote(demote(2)), 2);
    }

    proptest! {
        // r[verify tier.classify]
        #[test]
        fn prop_classify_valid(
            access_count: u64,
            warm in 0u64..=u64::MAX,
            hot in 0u64..=u64::MAX,
        ) {
            let w = warm.min(hot);
            let h = warm.max(hot);
            let tier = classify_tier(access_count, w, h);
            prop_assert!(tier <= 2);
        }

        // r[verify tier.classify]
        #[test]
        fn prop_classify_monotonic(
            count1: u64,
            count2 in 0u64..=u64::MAX,
            warm in 0u64..=u64::MAX,
            hot in 0u64..=u64::MAX,
        ) {
            let w = warm.min(hot);
            let h = warm.max(hot);
            let lo = count1.min(count2);
            let hi = count1.max(count2);
            let tier_lo = classify_tier(lo, w, h);
            let tier_hi = classify_tier(hi, w, h);
            prop_assert!(tier_lo <= tier_hi);
        }

        // r[verify tier.promote]
        #[test]
        fn prop_promote_ge(tier in 0u8..=2) {
            prop_assert!(promote(tier) >= tier);
        }

        // r[verify tier.demote]
        #[test]
        fn prop_demote_le(tier in 0u8..=2) {
            prop_assert!(demote(tier) <= tier);
        }

        // r[verify tier.promote]
        // r[verify tier.demote]
        #[test]
        fn prop_promote_demote_roundtrip(tier in 0u8..=1) {
            prop_assert_eq!(demote(promote(tier)), tier);
        }

        // r[verify tier.promote]
        // r[verify tier.demote]
        #[test]
        fn prop_demote_promote_roundtrip(tier in 1u8..=2) {
            prop_assert_eq!(promote(demote(tier)), tier);
        }

        // r[verify tier.should-demote]
        #[test]
        fn prop_cold_never_demoted(idle_time: u64, threshold: u64) {
            prop_assert!(!should_demote(0, idle_time, threshold));
        }

        // r[verify tier.classify]
        // r[verify tier.should-demote]
        #[test]
        fn prop_classify_hot_above_threshold_demotable(
            idle_time in 300u64..=u64::MAX,
        ) {
            // Hot entry with sufficient idle time should be demotable
            prop_assert!(should_demote(2, idle_time, 300));
        }
    }
}
