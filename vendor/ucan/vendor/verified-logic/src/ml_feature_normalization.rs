//! Integer ML feature normalization admission primitives.
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MlFeatureRange { pub min: i32, pub max: i32, pub target_lo: i32, pub target_hi: i32 }

// r[depends domain-primitives.ml-feature-normalization]
pub open spec fn ml_feature_range_valid_spec(range: MlFeatureRange) -> bool {
    range.min < range.max && range.target_lo <= range.target_hi
}

// r[depends domain-primitives.ml-feature-normalization]
pub open spec fn ml_feature_value_valid_spec(range: MlFeatureRange, value: i32) -> bool {
    ml_feature_range_valid_spec(range) && range.min <= value && value <= range.max
}

// r[depends domain-primitives.ml-feature-normalization]
pub open spec fn ml_feature_normalized_in_target_spec(range: MlFeatureRange, result: i32) -> bool {
    range.target_lo <= result && result <= range.target_hi
}

// r[impl domain-primitives.ml-feature-normalization]
pub fn ml_feature_admit(range: MlFeatureRange, value: Option<i32>) -> (result: bool)
    ensures result == (value.is_some() && ml_feature_value_valid_spec(range, value.unwrap()))
{
    match value {
        Some(v) => range.min < range.max && range.target_lo <= range.target_hi && range.min <= v && v <= range.max,
        None => false,
    }
}

// r[impl domain-primitives.ml-feature-normalization]
pub fn ml_feature_normalize_endpoint(range: MlFeatureRange, value: i32) -> (result: Option<i32>)
    ensures result.is_some() ==> ml_feature_normalized_in_target_spec(range, result.unwrap()),
            result.is_none() ==> !ml_feature_value_valid_spec(range, value)
{
    if !(range.min < range.max && range.target_lo <= range.target_hi && range.min <= value && value <= range.max) {
        None
    } else if value == range.max {
        Some(range.target_hi)
    } else {
        Some(range.target_lo)
    }
}

// r[verify domain-primitives.ml-feature-normalization]
pub proof fn ml_feature_endpoint_is_bounded(range: MlFeatureRange, result: i32)
    requires ml_feature_range_valid_spec(range), result == range.target_lo || result == range.target_hi
    ensures ml_feature_normalized_in_target_spec(range, result)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn feature_admission_and_endpoint_normalization() {
        let r = MlFeatureRange {
            min: 10,
            max: 20,
            target_lo: 0,
            target_hi: 100,
        };
        assert!(ml_feature_admit(r, Some(15_i32)));
        assert_eq!(ml_feature_normalize_endpoint(r, 20_i32), Some(100_i32));
        assert!(!ml_feature_admit(r, None));
    }
}
