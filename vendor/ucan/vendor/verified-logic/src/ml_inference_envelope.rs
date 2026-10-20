//! Deterministic ML inference-result envelope admission primitives.
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MlInferenceEnvelope { pub class_count: u16, pub min_regression: i32, pub max_regression: i32 }

// r[depends domain-primitives.ml-inference-envelope]
pub open spec fn ml_inference_envelope_valid_spec(env: MlInferenceEnvelope) -> bool {
    0 < env.class_count && env.min_regression <= env.max_regression
}

// r[depends domain-primitives.ml-inference-envelope]
pub open spec fn ml_classification_result_valid_spec(env: MlInferenceEnvelope, class_index: u16, score_basis_points: u16) -> bool {
    ml_inference_envelope_valid_spec(env) && class_index < env.class_count && score_basis_points <= 10000
}

// r[depends domain-primitives.ml-inference-envelope]
pub open spec fn ml_regression_result_valid_spec(env: MlInferenceEnvelope, value: i32) -> bool {
    ml_inference_envelope_valid_spec(env) && env.min_regression <= value && value <= env.max_regression
}

// r[impl domain-primitives.ml-inference-envelope]
pub fn ml_accept_classification(env: MlInferenceEnvelope, class_index: u16, score_basis_points: u16) -> (result: bool)
    ensures result == ml_classification_result_valid_spec(env, class_index, score_basis_points)
{
    0 < env.class_count && env.min_regression <= env.max_regression && class_index < env.class_count && score_basis_points <= 10000
}

// r[impl domain-primitives.ml-inference-envelope]
pub fn ml_accept_regression(env: MlInferenceEnvelope, value: i32) -> (result: bool)
    ensures result == ml_regression_result_valid_spec(env, value)
{
    0 < env.class_count && env.min_regression <= env.max_regression && env.min_regression <= value && value <= env.max_regression
}

// r[verify domain-primitives.ml-inference-envelope]
pub proof fn ml_classification_score_bounded(env: MlInferenceEnvelope, class_index: u16, score: u16)
    requires ml_classification_result_valid_spec(env, class_index, score)
    ensures score <= 10000
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inference_envelope_admits_and_rejects() {
        let e = MlInferenceEnvelope {
            class_count: 3,
            min_regression: -10,
            max_regression: 10,
        };
        assert!(ml_accept_classification(e, 2, 9000));
        assert!(!ml_accept_classification(e, 3, 9000));
        assert!(ml_accept_regression(e, 0));
    }
}
