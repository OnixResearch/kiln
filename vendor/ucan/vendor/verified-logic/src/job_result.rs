//! Verified job result validation.
//!
//! Classifies job exit codes into success/failure/error categories,
//! validates output against expected hash and size bounds, and combines
//! all criteria into a single validation status. The validation is
//! deterministic and returns the first failing criterion.
//!
//! Exit code ranges: 0=success, 1-127=failure, 128+=error (signal/crash).
//! Validation status: 0=valid, 1=bad exit, 2=bad hash, 3=bad size.

use vstd::prelude::*;

verus! {

/// Validation status constants.
pub const VALID: u8 = 0u8;
pub const BAD_EXIT: u8 = 1u8;
pub const BAD_HASH: u8 = 2u8;
pub const BAD_SIZE: u8 = 3u8;

// ============================================================
// Exit code classification
// ============================================================

// r[depends job-result.exit-code-classification]
/// Spec: exit code 0 is success.
pub open spec fn is_success_code_spec(exit_code: u8) -> bool {
    exit_code == 0
}

// r[impl job-result.exit-code-classification]
/// Check if an exit code indicates success.
pub fn is_success_code(exit_code: u8) -> (result: bool)
    ensures result == is_success_code_spec(exit_code)
{
    exit_code == 0
}

// r[depends job-result.exit-code-classification]
/// Spec: exit codes 1-127 are failures.
pub open spec fn is_failure_code_spec(exit_code: u8) -> bool {
    exit_code >= 1 && exit_code <= 127
}

// r[impl job-result.exit-code-classification]
/// Check if an exit code indicates a failure.
pub fn is_failure_code(exit_code: u8) -> (result: bool)
    ensures result == is_failure_code_spec(exit_code)
{
    exit_code >= 1 && exit_code <= 127
}

// r[depends job-result.exit-code-classification]
/// Spec: exit codes >= 128 are errors (signal/crash).
pub open spec fn is_error_code_spec(exit_code: u8) -> bool {
    exit_code >= 128
}

// r[impl job-result.exit-code-classification]
/// Check if an exit code indicates an error (signal/crash).
pub fn is_error_code(exit_code: u8) -> (result: bool)
    ensures result == is_error_code_spec(exit_code)
{
    exit_code >= 128
}

// ============================================================
// Output hash validation
// ============================================================

// r[depends job-result.output-hash-matches]
/// Spec: computed hash matches expected hash.
pub open spec fn output_hash_matches_spec(computed: u64, expected: u64) -> bool {
    computed == expected
}

// r[impl job-result.output-hash-matches]
/// Validate that the output hash matches the expected hash.
pub fn output_hash_matches(computed: u64, expected: u64) -> (result: bool)
    ensures result == output_hash_matches_spec(computed, expected)
{
    computed == expected
}

// ============================================================
// Output size validation
// ============================================================

// r[depends job-result.output-size-valid]
/// Spec: output size is within [min_size, max_size].
pub open spec fn output_size_valid_spec(
    actual_size: u64,
    min_size: u64,
    max_size: u64,
) -> bool {
    actual_size >= min_size && actual_size <= max_size
}

// r[impl job-result.output-size-valid]
/// Validate that the output size is within the allowed range.
pub fn output_size_valid(
    actual_size: u64,
    min_size: u64,
    max_size: u64,
) -> (result: bool)
    ensures result == output_size_valid_spec(actual_size, min_size, max_size)
{
    actual_size >= min_size && actual_size <= max_size
}

// ============================================================
// validate_result: combined first-failure check
// ============================================================

// r[depends job-result.validate-result]
/// Spec: combined validation returning first-failure criterion.
/// 0=all pass, 1=bad exit, 2=bad hash, 3=bad size.
pub open spec fn validate_result_spec(
    exit_code: u8,
    computed_hash: u64,
    expected_hash: u64,
    actual_size: u64,
    min_size: u64,
    max_size: u64,
) -> u8 {
    if !is_success_code_spec(exit_code) {
        BAD_EXIT
    } else if !output_hash_matches_spec(computed_hash, expected_hash) {
        BAD_HASH
    } else if !output_size_valid_spec(actual_size, min_size, max_size) {
        BAD_SIZE
    } else {
        VALID
    }
}

// r[impl job-result.validate-result]
/// Validate a job result against all criteria.
/// Returns: 0=valid, 1=bad exit, 2=bad hash, 3=bad size.
pub fn validate_result(
    exit_code: u8,
    computed_hash: u64,
    expected_hash: u64,
    actual_size: u64,
    min_size: u64,
    max_size: u64,
) -> (result: u8)
    ensures result == validate_result_spec(exit_code, computed_hash, expected_hash, actual_size, min_size, max_size)
{
    if !is_success_code(exit_code) {
        BAD_EXIT
    } else if !output_hash_matches(computed_hash, expected_hash) {
        BAD_HASH
    } else if !output_size_valid(actual_size, min_size, max_size) {
        BAD_SIZE
    } else {
        VALID
    }
}

// ============================================================
// Safety proofs
// ============================================================

// r[verify job-result.exit-code-classification]
/// Exit code classification is exhaustive: every u8 falls in exactly one category.
proof fn classification_exhaustive(exit_code: u8)
    ensures
        is_success_code_spec(exit_code)
        || is_failure_code_spec(exit_code)
        || is_error_code_spec(exit_code)
{}

// r[verify job-result.exit-code-classification]
/// Exit code classification is mutually exclusive.
proof fn classification_exclusive(exit_code: u8)
    ensures
        !(is_success_code_spec(exit_code) && is_failure_code_spec(exit_code)),
        !(is_success_code_spec(exit_code) && is_error_code_spec(exit_code)),
        !(is_failure_code_spec(exit_code) && is_error_code_spec(exit_code)),
{}

// r[verify job-result.exit-code-classification]
/// Zero is the only success code.
proof fn only_zero_succeeds(exit_code: u8)
    ensures is_success_code_spec(exit_code) <==> exit_code == 0
{}

// r[verify job-result.validate-result]
/// Valid (0) iff all criteria pass.
proof fn valid_iff_all_pass(
    exit_code: u8,
    computed_hash: u64,
    expected_hash: u64,
    actual_size: u64,
    min_size: u64,
    max_size: u64,
)
    ensures
        (validate_result_spec(exit_code, computed_hash, expected_hash, actual_size, min_size, max_size) == VALID)
        <==> (
            is_success_code_spec(exit_code)
            && output_hash_matches_spec(computed_hash, expected_hash)
            && output_size_valid_spec(actual_size, min_size, max_size)
        )
{}

// r[verify job-result.validate-result]
/// Validation is deterministic: same inputs → same output.
proof fn validation_deterministic(
    exit_code: u8,
    computed_hash: u64,
    expected_hash: u64,
    actual_size: u64,
    min_size: u64,
    max_size: u64,
)
    ensures
        validate_result_spec(exit_code, computed_hash, expected_hash, actual_size, min_size, max_size)
        == validate_result_spec(exit_code, computed_hash, expected_hash, actual_size, min_size, max_size)
{}

// r[verify job-result.validate-result]
/// Bad exit code is checked first (before hash and size).
proof fn bad_exit_first(
    exit_code: u8,
    computed_hash: u64,
    expected_hash: u64,
    actual_size: u64,
    min_size: u64,
    max_size: u64,
)
    requires !is_success_code_spec(exit_code)
    ensures
        validate_result_spec(exit_code, computed_hash, expected_hash, actual_size, min_size, max_size) == BAD_EXIT
{}

// r[verify job-result.validate-result]
/// Hash checked second (when exit code passes).
proof fn bad_hash_second(
    exit_code: u8,
    computed_hash: u64,
    expected_hash: u64,
    actual_size: u64,
    min_size: u64,
    max_size: u64,
)
    requires
        is_success_code_spec(exit_code),
        !output_hash_matches_spec(computed_hash, expected_hash),
    ensures
        validate_result_spec(exit_code, computed_hash, expected_hash, actual_size, min_size, max_size) == BAD_HASH
{}

// r[verify job-result.validate-result]
/// Size checked third (when exit code and hash pass).
proof fn bad_size_third(
    exit_code: u8,
    computed_hash: u64,
    expected_hash: u64,
    actual_size: u64,
    min_size: u64,
    max_size: u64,
)
    requires
        is_success_code_spec(exit_code),
        output_hash_matches_spec(computed_hash, expected_hash),
        !output_size_valid_spec(actual_size, min_size, max_size),
    ensures
        validate_result_spec(exit_code, computed_hash, expected_hash, actual_size, min_size, max_size) == BAD_SIZE
{}

// r[verify job-result.validate-result]
/// Result status is always one of the four valid values.
proof fn status_in_range(
    exit_code: u8,
    computed_hash: u64,
    expected_hash: u64,
    actual_size: u64,
    min_size: u64,
    max_size: u64,
)
    ensures ({
        let s = validate_result_spec(exit_code, computed_hash, expected_hash, actual_size, min_size, max_size);
        s == VALID || s == BAD_EXIT || s == BAD_HASH || s == BAD_SIZE
    })
{}

// r[verify job-result.output-hash-matches]
/// Hash matching is reflexive.
proof fn hash_reflexive(hash: u64)
    ensures output_hash_matches_spec(hash, hash)
{}

// r[verify job-result.output-hash-matches]
/// Hash matching is symmetric.
proof fn hash_symmetric(a: u64, b: u64)
    ensures output_hash_matches_spec(a, b) <==> output_hash_matches_spec(b, a)
{}

// r[verify job-result.output-size-valid]
/// Size with min=0 and max=u64::MAX always passes.
proof fn any_size_in_full_range(actual: u64)
    ensures output_size_valid_spec(actual, 0, u64::MAX)
{}

// r[verify job-result.output-size-valid]
/// Exact size match passes when min == max == actual.
proof fn exact_size_match(size: u64)
    ensures output_size_valid_spec(size, size, size)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify job-result.exit-code-classification]
    #[test]
    fn test_exit_code_classification() {
        assert!(is_success_code(0));
        assert!(!is_success_code(1));

        assert!(!is_failure_code(0));
        assert!(is_failure_code(1));
        assert!(is_failure_code(127));
        assert!(!is_failure_code(128));

        assert!(!is_error_code(0));
        assert!(!is_error_code(127));
        assert!(is_error_code(128));
        assert!(is_error_code(137)); // SIGKILL
        assert!(is_error_code(255));
    }

    // r[verify job-result.output-hash-matches]
    #[test]
    fn test_output_hash_matches() {
        assert!(output_hash_matches(0xABCD, 0xABCD));
        assert!(!output_hash_matches(0xABCD, 0x1234));
    }

    // r[verify job-result.output-size-valid]
    #[test]
    fn test_output_size_valid() {
        assert!(output_size_valid(500, 100, 1000));
        assert!(!output_size_valid(50, 100, 1000));
        assert!(!output_size_valid(1500, 100, 1000));
        assert!(output_size_valid(100, 100, 1000)); // boundary
        assert!(output_size_valid(1000, 100, 1000)); // boundary
    }

    // r[verify job-result.validate-result]
    #[test]
    fn test_validate_result() {
        // All pass
        assert_eq!(validate_result(0, 0xAA, 0xAA, 500, 100, 1000), VALID);
        // Bad exit code
        assert_eq!(validate_result(1, 0xAA, 0xAA, 500, 100, 1000), BAD_EXIT);
        // Good exit, bad hash
        assert_eq!(validate_result(0, 0xAA, 0xBB, 500, 100, 1000), BAD_HASH);
        // Good exit + hash, bad size
        assert_eq!(validate_result(0, 0xAA, 0xAA, 50, 100, 1000), BAD_SIZE);
        // Error code (signal)
        assert_eq!(validate_result(137, 0xAA, 0xAA, 500, 100, 1000), BAD_EXIT);
    }

    // r[verify job-result.validate-result]
    #[test]
    fn test_first_failure_order() {
        // When multiple criteria fail, first one wins
        // Bad exit + bad hash + bad size → BAD_EXIT (first)
        assert_eq!(validate_result(1, 0xAA, 0xBB, 50, 100, 1000), BAD_EXIT);
        // Good exit + bad hash + bad size → BAD_HASH (second)
        assert_eq!(validate_result(0, 0xAA, 0xBB, 50, 100, 1000), BAD_HASH);
    }

    proptest! {
        // r[verify job-result.exit-code-classification]
        #[test]
        fn prop_classification_exhaustive(exit_code in 0u8..=255) {
            let s = is_success_code(exit_code);
            let f = is_failure_code(exit_code);
            let e = is_error_code(exit_code);
            // Exactly one is true
            prop_assert_eq!((s as u8) + (f as u8) + (e as u8), 1);
        }

        // r[verify job-result.validate-result]
        #[test]
        fn prop_valid_iff_all_pass(
            exit_code in 0u8..=255,
            computed in 0u64..1000,
            expected in 0u64..1000,
            actual_size in 0u64..2000,
            min_size in 0u64..1000,
            max_size in 0u64..2000,
        ) {
            let status = validate_result(exit_code, computed, expected, actual_size, min_size, max_size);
            let all_pass = is_success_code(exit_code)
                && output_hash_matches(computed, expected)
                && output_size_valid(actual_size, min_size, max_size);
            prop_assert_eq!(status == VALID, all_pass);
        }

        // r[verify job-result.validate-result]
        #[test]
        fn prop_status_in_range(
            exit_code in 0u8..=255,
            computed in any::<u64>(),
            expected in any::<u64>(),
            actual_size in any::<u64>(),
            min_size in any::<u64>(),
            max_size in any::<u64>(),
        ) {
            let status = validate_result(exit_code, computed, expected, actual_size, min_size, max_size);
            prop_assert!(status == VALID || status == BAD_EXIT || status == BAD_HASH || status == BAD_SIZE);
        }

        // r[verify job-result.validate-result]
        #[test]
        fn prop_bad_exit_first(
            exit_code in 1u8..=255,
            computed in any::<u64>(),
            expected in any::<u64>(),
            actual_size in any::<u64>(),
            min_size in any::<u64>(),
            max_size in any::<u64>(),
        ) {
            let status = validate_result(exit_code, computed, expected, actual_size, min_size, max_size);
            prop_assert_eq!(status, BAD_EXIT);
        }

        // r[verify job-result.output-hash-matches]
        #[test]
        fn prop_hash_reflexive(hash in any::<u64>()) {
            prop_assert!(output_hash_matches(hash, hash));
        }

        // r[verify job-result.output-size-valid]
        #[test]
        fn prop_full_range_always_valid(actual in any::<u64>()) {
            prop_assert!(output_size_valid(actual, 0, u64::MAX));
        }

        // r[verify job-result.output-size-valid]
        #[test]
        fn prop_exact_match_valid(size in any::<u64>()) {
            prop_assert!(output_size_valid(size, size, size));
        }
    }
}
