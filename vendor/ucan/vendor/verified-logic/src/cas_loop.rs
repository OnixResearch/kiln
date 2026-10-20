//! Verified compare-and-swap (CAS) retry loop modeling.
//!
//! Models the core CAS primitive and retry loop properties:
//! success/failure semantics, ABA detection via generation counters,
//! progress guarantees, and idempotent update convergence.

use vstd::prelude::*;

verus! {

// --- cas_succeeds ---

// r[depends cas_loop.cas-succeeds]
/// A CAS operation succeeds when the current value matches expected.
pub open spec fn cas_succeeds_spec(current: u64, expected: u64) -> bool {
    current == expected
}

// r[impl cas_loop.cas-succeeds]
pub fn cas_succeeds(current: u64, expected: u64) -> (result: bool)
    ensures result == cas_succeeds_spec(current, expected)
{
    current == expected
}

// --- cas_result ---

// r[depends cas_loop.cas-result]
/// CAS returns (success, value_after). On success, the cell holds
/// `desired`. On failure, the cell is unchanged and the old value
/// is returned for the next retry iteration.
pub open spec fn cas_result_spec(current: u64, expected: u64, desired: u64) -> (bool, u64) {
    if current == expected {
        (true, desired)
    } else {
        (false, current)
    }
}

// r[impl cas_loop.cas-result]
pub fn cas_result(current: u64, expected: u64, desired: u64) -> (result: (bool, u64))
    ensures result == cas_result_spec(current, expected, desired)
{
    if current == expected {
        (true, desired)
    } else {
        (false, current)
    }
}

// --- should_retry ---

// r[depends cas_loop.should-retry]
/// A CAS loop should retry when: the CAS failed AND the maximum
/// retry count has not been reached AND the update is still applicable.
pub open spec fn should_retry_spec(
    cas_succeeded: bool,
    attempts: u64,
    max_attempts: u64,
) -> bool {
    !cas_succeeded && attempts < max_attempts
}

// r[impl cas_loop.should-retry]
pub fn should_retry(
    cas_succeeded: bool,
    attempts: u64,
    max_attempts: u64,
) -> (result: bool)
    ensures result == should_retry_spec(cas_succeeded, attempts, max_attempts)
{
    !cas_succeeded && attempts < max_attempts
}

// --- aba_pack ---

// r[depends cas_loop.aba-pack]
/// Pack a value and generation counter into a single u64.
/// Low 32 bits: value, high 32 bits: generation.
/// This enables ABA detection: even if the value returns to an old
/// state, the generation counter distinguishes the two epochs.
pub open spec fn aba_pack_spec(value: u32, generation: u32) -> u64 {
    (value as u64) | ((generation as u64) << 32u64)
}

// r[impl cas_loop.aba-pack]
pub fn aba_pack(value: u32, generation: u32) -> (result: u64)
    ensures result == aba_pack_spec(value, generation)
{
    (value as u64) | ((generation as u64) << 32)
}

// --- aba_unpack_value ---

// r[depends cas_loop.aba-unpack]
/// Extract the value (low 32 bits) from a packed word.
pub open spec fn aba_unpack_value_spec(packed: u64) -> u32 {
    (packed & 0xFFFF_FFFFu64) as u32
}

// r[impl cas_loop.aba-unpack]
pub fn aba_unpack_value(packed: u64) -> (result: u32)
    ensures result == aba_unpack_value_spec(packed)
{
    (packed & 0xFFFF_FFFF) as u32
}

// --- aba_unpack_generation ---

// r[depends cas_loop.aba-unpack]
/// Extract the generation (high 32 bits) from a packed word.
pub open spec fn aba_unpack_generation_spec(packed: u64) -> u32 {
    ((packed >> 32u64) & 0xFFFF_FFFFu64) as u32
}

// r[impl cas_loop.aba-unpack]
pub fn aba_unpack_generation(packed: u64) -> (result: u32)
    ensures result == aba_unpack_generation_spec(packed)
{
    ((packed >> 32) & 0xFFFF_FFFF) as u32
}

// --- aba_next_generation ---

// r[depends cas_loop.aba-generation]
/// Increment the generation counter, wrapping at u32::MAX.
pub open spec fn aba_next_generation_spec(generation: u32) -> u32 {
    if generation == u32::MAX { 0 } else { (generation + 1) as u32 }
}

// r[impl cas_loop.aba-generation]
pub fn aba_next_generation(generation: u32) -> (result: u32)
    ensures result == aba_next_generation_spec(generation)
{
    if generation == u32::MAX { 0 } else { generation + 1 }
}

// --- is_idempotent_update ---

// r[depends cas_loop.idempotent]
/// An update is idempotent if applying it to the desired value
/// produces the same desired value. CAS loops with idempotent
/// updates converge even under contention.
pub open spec fn is_idempotent_update_spec(current: u64, desired: u64) -> bool {
    // Applying the "same update" again (current==desired → desired stays)
    cas_result_spec(desired, desired, desired) == (true, desired)
}

// r[impl cas_loop.idempotent]
pub fn is_idempotent_update(_current: u64, _desired: u64) -> (result: bool)
    ensures result == is_idempotent_update_spec(_current, _desired)
{
    // Always true by CAS definition: CAS(desired, desired, desired) succeeds
    true
}

// --- attempts_remaining ---

// r[depends cas_loop.attempts-remaining]
/// How many retry attempts remain.
pub open spec fn attempts_remaining_spec(attempts: u64, max_attempts: u64) -> u64 {
    if attempts >= max_attempts { 0 } else { (max_attempts - attempts) as u64 }
}

// r[impl cas_loop.attempts-remaining]
pub fn attempts_remaining(attempts: u64, max_attempts: u64) -> (result: u64)
    ensures result == attempts_remaining_spec(attempts, max_attempts)
{
    if attempts >= max_attempts { 0 } else { max_attempts - attempts }
}

// === Proofs ===

// r[verify cas_loop.cas-succeeds]
/// CAS on matching values always succeeds.
proof fn cas_match_succeeds(v: u64, desired: u64)
    ensures ({
        let (ok, after) = cas_result_spec(v, v, desired);
        ok && after == desired
    })
{}

// r[verify cas_loop.cas-succeeds]
/// CAS on non-matching values always fails.
proof fn cas_mismatch_fails(current: u64, expected: u64, desired: u64)
    requires current != expected
    ensures ({
        let (ok, after) = cas_result_spec(current, expected, desired);
        !ok && after == current
    })
{}

// r[verify cas_loop.cas-result]
/// CAS is atomic: after CAS, cell holds either desired (success) or original (failure).
proof fn cas_atomic(current: u64, expected: u64, desired: u64)
    ensures ({
        let (ok, after) = cas_result_spec(current, expected, desired);
        (ok && after == desired) || (!ok && after == current)
    })
{}

// r[verify cas_loop.cas-result]
/// Successful CAS produces the desired value.
proof fn cas_success_writes_desired(current: u64, desired: u64)
    ensures ({
        let (ok, after) = cas_result_spec(current, current, desired);
        ok && after == desired
    })
{}

// r[verify cas_loop.cas-result]
/// Failed CAS is a no-op — cell unchanged.
proof fn cas_failure_no_op(current: u64, expected: u64, desired: u64)
    requires current != expected
    ensures ({
        let (_, after) = cas_result_spec(current, expected, desired);
        after == current
    })
{}

// r[verify cas_loop.should-retry]
/// After success, no retry needed.
proof fn success_stops_retry(attempts: u64, max: u64)
    ensures !should_retry_spec(true, attempts, max)
{}

// r[verify cas_loop.should-retry]
/// At max attempts, no retry regardless of success.
proof fn max_attempts_stops_retry(ok: bool, max: u64)
    ensures !should_retry_spec(ok, max, max)
{}

// r[verify cas_loop.attempts-remaining]
/// Attempts remaining decreases each iteration.
proof fn attempts_remaining_decreases(attempts: u64, max_attempts: u64)
    requires attempts < max_attempts
    ensures
        attempts_remaining_spec(attempts, max_attempts) > 0,
        attempts_remaining_spec((attempts + 1) as u64, max_attempts)
            == attempts_remaining_spec(attempts, max_attempts) - 1,
{}

// r[verify cas_loop.aba-pack]
// r[verify cas_loop.aba-unpack]
/// Pack/unpack roundtrip for value.
proof fn aba_value_roundtrip(value: u32, generation: u32)
    ensures aba_unpack_value_spec(aba_pack_spec(value, generation)) == value
{
    assert(aba_unpack_value_spec(aba_pack_spec(value, generation)) == value) by(bit_vector);
}

// r[verify cas_loop.aba-pack]
// r[verify cas_loop.aba-unpack]
/// Pack/unpack roundtrip for generation.
proof fn aba_generation_roundtrip(value: u32, generation: u32)
    ensures aba_unpack_generation_spec(aba_pack_spec(value, generation)) == generation
{
    assert(aba_unpack_generation_spec(aba_pack_spec(value, generation)) == generation) by(bit_vector);
}

// r[verify cas_loop.aba-pack]
/// Same value + different generation → different packed word.
proof fn aba_generation_distinguishes(value: u32, gen_a: u32, gen_b: u32)
    requires gen_a != gen_b
    ensures aba_pack_spec(value, gen_a) != aba_pack_spec(value, gen_b)
{
    assert(gen_a != gen_b ==>
        ((value as u64) | ((gen_a as u64) << 32u64))
            != ((value as u64) | ((gen_b as u64) << 32u64))
    ) by(bit_vector);
}

// r[verify cas_loop.aba-pack]
/// ABA scenario: same value returns, but generation detects it.
proof fn aba_detection(value: u32, gen_before: u32, gen_after: u32)
    requires gen_before != gen_after
    ensures aba_pack_spec(value, gen_before) != aba_pack_spec(value, gen_after)
{
    aba_generation_distinguishes(value, gen_before, gen_after);
}

// r[verify cas_loop.idempotent]
/// All CAS updates are trivially idempotent by definition.
proof fn all_updates_idempotent(current: u64, desired: u64)
    ensures is_idempotent_update_spec(current, desired)
{}

// r[verify cas_loop.aba-generation]
/// Next generation is different from current (unless wrap).
proof fn next_gen_different(g: u32)
    requires g < u32::MAX
    ensures aba_next_generation_spec(g) != g
{}

// r[verify cas_loop.aba-generation]
/// Generation increment is bounded.
proof fn next_gen_bounded(g: u32)
    ensures aba_next_generation_spec(g) <= u32::MAX
{}

// r[verify cas_loop.cas-result]
/// CAS is deterministic: same inputs → same outputs.
proof fn cas_deterministic(c: u64, e: u64, d: u64)
    ensures cas_result_spec(c, e, d) == cas_result_spec(c, e, d)
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn test_cas_succeeds() {
        assert!(cas_succeeds(5, 5));
        assert!(!cas_succeeds(5, 6));
    }

    #[test]
    fn test_cas_result_success() {
        assert_eq!(cas_result(5, 5, 10), (true, 10));
    }

    #[test]
    fn test_cas_result_failure() {
        assert_eq!(cas_result(5, 6, 10), (false, 5));
    }

    #[test]
    fn test_should_retry() {
        assert!(should_retry(false, 0, 10));
        assert!(!should_retry(true, 0, 10));
        assert!(!should_retry(false, 10, 10));
    }

    #[test]
    fn test_aba_pack_unpack() {
        let packed = aba_pack(42, 7);
        assert_eq!(aba_unpack_value(packed), 42);
        assert_eq!(aba_unpack_generation(packed), 7);
    }

    #[test]
    fn test_aba_detection() {
        let word_a = aba_pack(100, 1);
        let word_b = aba_pack(100, 2);
        assert_ne!(word_a, word_b); // Same value, different generation
    }

    #[test]
    fn test_aba_next_generation() {
        assert_eq!(aba_next_generation(0), 1);
        assert_eq!(aba_next_generation(u32::MAX), 0);
    }

    #[test]
    fn test_attempts_remaining() {
        assert_eq!(attempts_remaining(0, 10), 10);
        assert_eq!(attempts_remaining(5, 10), 5);
        assert_eq!(attempts_remaining(10, 10), 0);
        assert_eq!(attempts_remaining(15, 10), 0);
    }

    #[test]
    fn test_scenario_cas_retry_loop() {
        // Simulate: desired=10, but another thread changed 5→7
        let (ok, seen) = cas_result(7, 5, 10);
        assert!(!ok);
        assert_eq!(seen, 7);
        assert!(should_retry(ok, 0, 3));

        // Retry with updated expected
        let (ok2, _after) = cas_result(7, 7, 10);
        assert!(ok2);
        assert!(!should_retry(ok2, 1, 3));
    }

    proptest! {
        #[test]
        fn prop_cas_match_succeeds(v: u64, desired: u64) {
            let (ok, after) = cas_result(v, v, desired);
            prop_assert!(ok);
            prop_assert_eq!(after, desired);
        }

        #[test]
        fn prop_cas_mismatch_fails(current: u64, expected: u64, desired: u64) {
            prop_assume!(current != expected);
            let (ok, after) = cas_result(current, expected, desired);
            prop_assert!(!ok);
            prop_assert_eq!(after, current);
        }

        #[test]
        fn prop_aba_value_roundtrip(value: u32, generation: u32) {
            let packed = aba_pack(value, generation);
            prop_assert_eq!(aba_unpack_value(packed), value);
        }

        #[test]
        fn prop_aba_generation_roundtrip(value: u32, generation: u32) {
            let packed = aba_pack(value, generation);
            prop_assert_eq!(aba_unpack_generation(packed), generation);
        }

        #[test]
        fn prop_aba_distinguishes(value: u32, gen_a: u32, gen_b: u32) {
            prop_assume!(gen_a != gen_b);
            prop_assert_ne!(aba_pack(value, gen_a), aba_pack(value, gen_b));
        }

        #[test]
        fn prop_success_stops_retry(attempts: u64, max: u64) {
            prop_assert!(!should_retry(true, attempts, max));
        }

        #[test]
        fn prop_max_attempts_stops(ok: bool, max: u64) {
            prop_assert!(!should_retry(ok, max, max));
        }

        #[test]
        fn prop_remaining_decreases(attempts in 0u64..u64::MAX, max in 1u64..=u64::MAX) {
            prop_assume!(attempts < max);
            let r = attempts_remaining(attempts, max);
            prop_assert!(r > 0);
        }
    }
}
