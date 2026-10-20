//! Exact finite rollout-bucket metadata admission.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FeatureFlagRolloutBucket { pub subject: u16, pub bucket: u8, pub start: u8, pub end: u8, pub total: u8 }

// r[depends domain-primitives.configuration-feature-flag.rollout-bucket]
pub open spec fn feature_flag_rollout_valid_spec(r: FeatureFlagRolloutBucket) -> bool { r.total <= 100 && r.bucket < r.total && r.start <= r.end && r.end <= r.total }
// r[depends domain-primitives.configuration-feature-flag.rollout-bucket]
pub open spec fn feature_flag_rollout_member_spec(r: FeatureFlagRolloutBucket) -> bool { feature_flag_rollout_valid_spec(r) && r.start <= r.bucket && r.bucket < r.end }
// r[impl domain-primitives.configuration-feature-flag.rollout-bucket]
pub fn feature_flag_rollout_valid(r: FeatureFlagRolloutBucket) -> (result: bool) ensures result == feature_flag_rollout_valid_spec(r) { r.total <= 100 && r.bucket < r.total && r.start <= r.end && r.end <= r.total }
// r[impl domain-primitives.configuration-feature-flag.rollout-bucket]
pub fn feature_flag_rollout_member(r: FeatureFlagRolloutBucket) -> (result: bool) ensures result == feature_flag_rollout_member_spec(r) { feature_flag_rollout_valid(r) && r.start <= r.bucket && r.bucket < r.end }
// r[verify domain-primitives.configuration-feature-flag.rollout-bucket]
pub proof fn out_of_range_bucket_rejected(r: FeatureFlagRolloutBucket) requires !(r.bucket < r.total) ensures !feature_flag_rollout_valid_spec(r) {}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bucket_bounds_are_explicit() {
        let r = FeatureFlagRolloutBucket {
            subject: 1,
            bucket: 5,
            start: 0,
            end: 10,
            total: 100,
        };
        assert!(feature_flag_rollout_member(r));
        let bad = FeatureFlagRolloutBucket {
            subject: 1,
            bucket: 101,
            start: 0,
            end: 10,
            total: 100,
        };
        assert!(!feature_flag_rollout_valid(bad));
    }
}
