//! Verified compare-and-swap condition logic.
//!
//! Typed condition evaluation for atomic read-modify-write operations.
//! Matches aspen's `check_cas_condition`, `check_condition_met`, and
//! `evaluate_conditions`. Every condition type has a machine-checked
//! spec and proven properties.

use vstd::prelude::*;

verus! {

/// Condition type: value equals expected.
pub const CONDITION_VALUE_EQUALS: u8 = 0u8;

/// Condition type: key exists (has any value).
pub const CONDITION_KEY_EXISTS: u8 = 1u8;

/// Condition type: key does not exist.
pub const CONDITION_KEY_NOT_EXISTS: u8 = 2u8;

// --- check_condition ---

// r[depends cas.check-condition]
/// Spec: evaluate a typed condition against a current value.
pub open spec fn check_condition_spec(
    condition_type: u8,
    current_is_some: bool,
    values_equal: bool,
) -> bool {
    if condition_type == CONDITION_VALUE_EQUALS {
        current_is_some && values_equal
    } else if condition_type == CONDITION_KEY_EXISTS {
        current_is_some
    } else if condition_type == CONDITION_KEY_NOT_EXISTS {
        !current_is_some
    } else {
        false // unknown condition type
    }
}

// r[impl cas.check-condition]
/// Evaluates a typed condition against a current value.
///
/// Matches aspen's `check_condition_met`. Uses abstract booleans for
/// current-value presence and equality to keep the module generic
/// (no string/byte dependencies).
pub fn check_condition(
    condition_type: u8,
    current_is_some: bool,
    values_equal: bool,
) -> (result: bool)
    ensures result == check_condition_spec(condition_type, current_is_some, values_equal)
{
    if condition_type == CONDITION_VALUE_EQUALS {
        current_is_some && values_equal
    } else if condition_type == CONDITION_KEY_EXISTS {
        current_is_some
    } else if condition_type == CONDITION_KEY_NOT_EXISTS {
        !current_is_some
    } else {
        false
    }
}

// r[verify cas.check-condition]
/// VALUE_EQUALS on a missing key always fails.
proof fn value_equals_requires_key()
    ensures
        !check_condition_spec(CONDITION_VALUE_EQUALS, false, true),
        !check_condition_spec(CONDITION_VALUE_EQUALS, false, false),
{}

// r[verify cas.check-condition]
/// KEY_EXISTS and KEY_NOT_EXISTS are complements.
proof fn exists_not_exists_complement(current_is_some: bool)
    ensures
        check_condition_spec(CONDITION_KEY_EXISTS, current_is_some, false)
            != check_condition_spec(CONDITION_KEY_NOT_EXISTS, current_is_some, false)
{}

// r[verify cas.check-condition]
/// Unknown condition types always fail.
proof fn unknown_condition_fails(ct: u8, cs: bool, eq: bool)
    requires
        ct != CONDITION_VALUE_EQUALS,
        ct != CONDITION_KEY_EXISTS,
        ct != CONDITION_KEY_NOT_EXISTS,
    ensures !check_condition_spec(ct, cs, eq)
{}

// --- check_cas ---

// r[depends cas.check-cas]
/// Spec: CAS condition — expected and current must match.
pub open spec fn check_cas_spec(expected_is_some: bool, current_is_some: bool, values_equal: bool) -> bool {
    if !expected_is_some && !current_is_some {
        true // both None
    } else if expected_is_some && current_is_some {
        values_equal // both Some, check equality
    } else {
        false // one present, one absent
    }
}

// r[impl cas.check-cas]
/// Returns true if the CAS condition is met.
///
/// Matches aspen's `check_cas_condition`. Abstracted over value types —
/// the caller provides presence and equality booleans.
pub fn check_cas(
    expected_is_some: bool,
    current_is_some: bool,
    values_equal: bool,
) -> (result: bool)
    ensures result == check_cas_spec(expected_is_some, current_is_some, values_equal)
{
    if !expected_is_some && !current_is_some {
        true
    } else if expected_is_some && current_is_some {
        values_equal
    } else {
        false
    }
}

// r[verify cas.check-cas]
/// Both absent: condition met.
proof fn cas_both_none()
    ensures check_cas_spec(false, false, false)
{}

// r[verify cas.check-cas]
/// CAS is symmetric in the "both present" case: if the values are equal,
/// the direction doesn't matter.
proof fn cas_symmetric_equality()
    ensures
        check_cas_spec(true, true, true),
        !check_cas_spec(true, true, false),
{}

// r[verify cas.check-cas]
/// Presence mismatch always fails.
proof fn cas_presence_mismatch()
    ensures
        !check_cas_spec(true, false, true),
        !check_cas_spec(true, false, false),
        !check_cas_spec(false, true, true),
        !check_cas_spec(false, true, false),
{}

// --- evaluate_conditions ---

// r[depends cas.evaluate-conditions]
/// Spec: index of first failing condition, or -1 if all pass.
pub open spec fn evaluate_conditions_spec(
    conditions: Seq<(u8, bool, bool)>, from: int,
) -> int
    decreases conditions.len() - from
{
    if from >= conditions.len() {
        -1int // all passed
    } else {
        let (ct, cs, eq) = conditions[from];
        if !check_condition_spec(ct, cs, eq) {
            from
        } else {
            evaluate_conditions_spec(conditions, from + 1)
        }
    }
}

// r[impl cas.evaluate-conditions]
/// Evaluates conditions in order, returning the index of the first failure
/// or None if all pass.
///
/// Matches aspen's `evaluate_conditions`. Conditions are pre-evaluated
/// into `(condition_type, current_is_some, values_equal)` triples.
pub fn evaluate_conditions(conditions: &[(u8, bool, bool)]) -> (result: Option<usize>)
    ensures
        result.is_some() ==> {
            let idx = result.unwrap() as int;
            &&& 0 <= idx < conditions@.len()
            &&& !check_condition_spec(
                    conditions@[idx].0,
                    conditions@[idx].1,
                    conditions@[idx].2,
                )
            &&& forall|j: int| 0 <= j < idx ==> check_condition_spec(
                    conditions@[j].0,
                    conditions@[j].1,
                    conditions@[j].2,
                )
        },
        result.is_none() ==> forall|j: int| 0 <= j < conditions@.len()
            ==> check_condition_spec(
                    conditions@[j].0,
                    conditions@[j].1,
                    conditions@[j].2,
                ),
{
    let mut i: usize = 0;
    while i < conditions.len()
        invariant
            0 <= i <= conditions@.len(),
            forall|j: int| 0 <= j < i as int ==> check_condition_spec(
                conditions@[j].0,
                conditions@[j].1,
                conditions@[j].2,
            ),
        decreases conditions@.len() - i
    {
        let (ct, cs, eq) = conditions[i];
        if !check_condition(ct, cs, eq) {
            return Some(i);
        }
        i = i + 1;
    }
    None
}

// r[verify cas.evaluate-conditions]
/// Empty condition list always succeeds.
proof fn evaluate_empty()
    ensures evaluate_conditions_spec(Seq::<(u8, bool, bool)>::empty(), 0) == -1int
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify cas.check-condition]
    #[test]
    fn test_check_condition() {
        // VALUE_EQUALS
        assert!(check_condition(CONDITION_VALUE_EQUALS, true, true));
        assert!(!check_condition(CONDITION_VALUE_EQUALS, true, false));
        assert!(!check_condition(CONDITION_VALUE_EQUALS, false, true));
        assert!(!check_condition(CONDITION_VALUE_EQUALS, false, false));

        // KEY_EXISTS
        assert!(check_condition(CONDITION_KEY_EXISTS, true, false));
        assert!(check_condition(CONDITION_KEY_EXISTS, true, true));
        assert!(!check_condition(CONDITION_KEY_EXISTS, false, false));

        // KEY_NOT_EXISTS
        assert!(check_condition(CONDITION_KEY_NOT_EXISTS, false, false));
        assert!(!check_condition(CONDITION_KEY_NOT_EXISTS, true, false));

        // Unknown
        assert!(!check_condition(99, true, true));
    }

    // r[verify cas.check-cas]
    #[test]
    fn test_check_cas() {
        assert!(check_cas(false, false, false)); // both None
        assert!(check_cas(true, true, true)); // both Some, equal
        assert!(!check_cas(true, true, false)); // both Some, not equal
        assert!(!check_cas(true, false, false)); // expected Some, got None
        assert!(!check_cas(false, true, false)); // expected None, got Some
    }

    // r[verify cas.evaluate-conditions]
    #[test]
    fn test_evaluate_conditions() {
        // Empty — all met
        assert_eq!(evaluate_conditions(&[]), None);

        // All pass
        let conds = [
            (CONDITION_KEY_EXISTS, true, false),
            (CONDITION_VALUE_EQUALS, true, true),
        ];
        assert_eq!(evaluate_conditions(&conds), None);

        // First fails
        let conds = [
            (CONDITION_KEY_EXISTS, false, false),
            (CONDITION_VALUE_EQUALS, true, true),
        ];
        assert_eq!(evaluate_conditions(&conds), Some(0));

        // Second fails
        let conds = [
            (CONDITION_KEY_EXISTS, true, false),
            (CONDITION_VALUE_EQUALS, true, false),
        ];
        assert_eq!(evaluate_conditions(&conds), Some(1));
    }

    proptest! {
        // r[verify cas.check-condition]
        #[test]
        fn prop_exists_complement(current_is_some: bool) {
            prop_assert_ne!(
                check_condition(CONDITION_KEY_EXISTS, current_is_some, false),
                check_condition(CONDITION_KEY_NOT_EXISTS, current_is_some, false),
            );
        }

        // r[verify cas.check-cas]
        #[test]
        fn prop_cas_reflexive(is_some: bool) {
            // Same presence + equal values → condition met
            if is_some {
                prop_assert!(check_cas(true, true, true));
            } else {
                prop_assert!(check_cas(false, false, false));
            }
        }

        // r[verify cas.check-cas]
        #[test]
        fn prop_cas_presence_mismatch(eq: bool) {
            // One present, one absent → always fails
            prop_assert!(!check_cas(true, false, eq));
            prop_assert!(!check_cas(false, true, eq));
        }

        // r[verify cas.evaluate-conditions]
        #[test]
        fn prop_evaluate_all_key_exists(n in 0usize..=20) {
            let conds: Vec<(u8, bool, bool)> = vec![(CONDITION_KEY_EXISTS, true, false); n];
            prop_assert_eq!(evaluate_conditions(&conds), None);
        }

        // r[verify cas.check-condition]
        // r[verify cas.check-cas]
        // r[verify cas.evaluate-conditions]
        /// Model oracle: generate random conditions, evaluate manually and
        /// via evaluate_conditions, check they agree.
        #[test]
        fn prop_evaluate_model_oracle(
            conds in proptest::collection::vec(
                (0u8..4, any::<bool>(), any::<bool>()),
                0..=20,
            ),
        ) {
            let result = evaluate_conditions(&conds);

            // Find first failure manually
            let mut expected_fail: Option<usize> = None;
            for (i, &(ct, cs, eq)) in conds.iter().enumerate() {
                if !check_condition(ct, cs, eq) {
                    expected_fail = Some(i);
                    break;
                }
            }

            prop_assert_eq!(result, expected_fail);
        }
    }
}
