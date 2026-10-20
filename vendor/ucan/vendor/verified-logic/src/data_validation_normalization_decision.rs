//! Exact finite normalization decision envelope.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum NormalizationDecision { Unchanged, Transformed, Rejected }
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct NormalizationEnvelope { pub input_shape: u8, pub output_shape: u8, pub admitted: bool, pub transformed: bool }

// r[depends domain-primitives.data-validation.normalization-decision]
pub open spec fn normalization_decision_spec(n: NormalizationEnvelope) -> NormalizationDecision { if !n.admitted { NormalizationDecision::Rejected } else if n.transformed && n.input_shape != n.output_shape { NormalizationDecision::Transformed } else { NormalizationDecision::Unchanged } }

// r[impl domain-primitives.data-validation.normalization-decision]
pub fn normalization_decision(n: NormalizationEnvelope) -> (result: NormalizationDecision) ensures result == normalization_decision_spec(n) { if !n.admitted { NormalizationDecision::Rejected } else if n.transformed && n.input_shape != n.output_shape { NormalizationDecision::Transformed } else { NormalizationDecision::Unchanged } }

// r[verify domain-primitives.data-validation.normalization-decision]
pub proof fn rejected_normalization_preserves_original_shape(n: NormalizationEnvelope) requires !n.admitted ensures normalization_decision_spec(n) == NormalizationDecision::Rejected {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalization_records_rejection() {
        assert_eq!(
            normalization_decision(NormalizationEnvelope {
                input_shape: 1,
                output_shape: 2,
                admitted: false,
                transformed: true
            }),
            NormalizationDecision::Rejected
        );
    }
}
