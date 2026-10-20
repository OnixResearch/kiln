//! Verified UCAN parsed temporal-order admission.
//!
//! This module proves only a finite predicate over caller- or shell-supplied
//! parsed timestamp values. It does not verify JSON/serde parsing,
//! duplicate-key behavior, numeric deserialization provenance, wall-clock
//! freshness, clock-source correctness, expiration-at-now policy, signing input
//! construction, cryptography, proof truth, backend truth, or token provenance.

use vstd::prelude::*;

verus! {

/// Shell-supplied parsed UCAN timestamp facts.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ParsedTemporalOrder {
    pub not_before_seconds: u64,
    pub expires_at_seconds: u64,
}

/// Parsed temporal order is valid exactly when `nbf` is strictly before `exp`.
// r[impl ucan-temporal-order.not-before-precedes-expiration]
pub open spec fn parsed_temporal_order_spec(facts: ParsedTemporalOrder) -> bool {
    facts.not_before_seconds < facts.expires_at_seconds
}

/// Check parsed temporal order before a UCAN time window is admitted.
// r[impl ucan-temporal-order.not-before-precedes-expiration]
pub fn parsed_temporal_order_is_valid(facts: ParsedTemporalOrder) -> (result: bool)
    ensures result == parsed_temporal_order_spec(facts),
{
    facts.not_before_seconds < facts.expires_at_seconds
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // r[verify ucan-temporal-order.not-before-precedes-expiration]
    fn accepts_strictly_ordered_bounds() {
        assert!(parsed_temporal_order_is_valid(ParsedTemporalOrder {
            not_before_seconds: 1,
            expires_at_seconds: 2,
        }));
    }

    #[test]
    // r[verify ucan-temporal-order.not-before-precedes-expiration]
    fn rejects_equal_bounds() {
        assert!(!parsed_temporal_order_is_valid(ParsedTemporalOrder {
            not_before_seconds: 2,
            expires_at_seconds: 2,
        }));
    }

    #[test]
    // r[verify ucan-temporal-order.not-before-precedes-expiration]
    fn rejects_reversed_bounds() {
        assert!(!parsed_temporal_order_is_valid(ParsedTemporalOrder {
            not_before_seconds: 3,
            expires_at_seconds: 2,
        }));
    }
}
