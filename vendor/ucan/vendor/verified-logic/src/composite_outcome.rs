//! Common outcome tags for composite primitives.
//!
//! Composite facade modules use these helpers to map domain-specific results
//! into the shared OpenSpec `Accept` / `Reject` / `Wait` model.

use vstd::prelude::*;

verus! {

/// Shared proof-facing outcome tag for composite facades.
pub enum CompositeOutcomeTag {
    /// The facade admitted the requested transition or decision.
    Accept,
    /// The facade rejected the request and preserves mutable state unless documented read-only.
    Reject,
    /// The facade entered a valid wait/backoff transition.
    Wait,
}

/// Spec: whether an outcome is accepted.
// r[depends composite-outcome.tags]
pub open spec fn composite_accept_spec(outcome: CompositeOutcomeTag) -> bool {
    outcome is Accept
}

/// Spec: whether an outcome is rejected.
// r[depends composite-outcome.tags]
pub open spec fn composite_reject_spec(outcome: CompositeOutcomeTag) -> bool {
    outcome is Reject
}

/// Spec: whether an outcome is a wait/backoff transition.
// r[depends composite-outcome.tags]
pub open spec fn composite_wait_spec(outcome: CompositeOutcomeTag) -> bool {
    outcome is Wait
}

/// Spec: outcomes are pairwise disjoint and cover exactly the three common cases.
// r[depends composite-outcome.tags]
pub open spec fn composite_outcome_valid_spec(outcome: CompositeOutcomeTag) -> bool {
    composite_accept_spec(outcome) || composite_reject_spec(outcome) || composite_wait_spec(outcome)
}

/// Runtime helper for domain facades that need to map an accepted branch.
// r[impl composite-outcome.tags]
pub fn composite_accept() -> (outcome: CompositeOutcomeTag)
    ensures
        composite_accept_spec(outcome),
        composite_outcome_valid_spec(outcome),
        !composite_reject_spec(outcome),
        !composite_wait_spec(outcome),
{
    CompositeOutcomeTag::Accept
}

/// Runtime helper for domain facades that need to map a rejected branch.
// r[impl composite-outcome.tags]
pub fn composite_reject() -> (outcome: CompositeOutcomeTag)
    ensures
        composite_reject_spec(outcome),
        composite_outcome_valid_spec(outcome),
        !composite_accept_spec(outcome),
        !composite_wait_spec(outcome),
{
    CompositeOutcomeTag::Reject
}

/// Runtime helper for the blocking-channel wait/backoff branch.
// r[impl composite-outcome.tags]
pub fn composite_wait() -> (outcome: CompositeOutcomeTag)
    ensures
        composite_wait_spec(outcome),
        composite_outcome_valid_spec(outcome),
        !composite_accept_spec(outcome),
        !composite_reject_spec(outcome),
{
    CompositeOutcomeTag::Wait
}

/// Bridge helper for per-module aliases.
// r[impl composite-outcome.tags]
pub fn composite_outcome_code(outcome: CompositeOutcomeTag) -> (code: u8)
    ensures
        code == 1 <==> composite_accept_spec(outcome),
        code == 2 <==> composite_reject_spec(outcome),
        code == 3 <==> composite_wait_spec(outcome),
        code >= 1 && code <= 3,
{
    match outcome {
        CompositeOutcomeTag::Accept => 1,
        CompositeOutcomeTag::Reject => 2,
        CompositeOutcomeTag::Wait => 3,
    }
}

/// Accept/Reject/Wait are mutually exclusive.
// r[verify composite-outcome.tags]
pub proof fn composite_outcomes_disjoint(outcome: CompositeOutcomeTag)
    ensures
        !(composite_accept_spec(outcome) && composite_reject_spec(outcome)),
        !(composite_accept_spec(outcome) && composite_wait_spec(outcome)),
        !(composite_reject_spec(outcome) && composite_wait_spec(outcome)),
{}

/// The common model is exhaustive for its tag type.
// r[verify composite-outcome.tags]
pub proof fn composite_outcomes_exhaustive(outcome: CompositeOutcomeTag)
    ensures composite_outcome_valid_spec(outcome)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify composite-outcome.tags]
    #[test]
    fn outcome_codes_are_stable() {
        assert_eq!(composite_outcome_code(CompositeOutcomeTag::Accept), 1);
        assert_eq!(composite_outcome_code(CompositeOutcomeTag::Reject), 2);
        assert_eq!(composite_outcome_code(CompositeOutcomeTag::Wait), 3);
    }
}
