//! Verified Unix file permission bit manipulation.
//!
//! Models the 9-bit Unix permission system: read/write/execute for
//! owner/group/other. Bit positions follow the standard layout where
//! owner bits occupy positions 6-8, group 3-5, other 0-2.

use vstd::prelude::*;

use crate::bitwise::bit_at;
#[cfg(verus_keep_ghost)]
use crate::bitwise::bit_at_spec;
use crate::bitwise::clear_bit;
#[cfg(verus_keep_ghost)]
use crate::bitwise::clear_bit_spec;
use crate::bitwise::set_bit;
#[cfg(verus_keep_ghost)]
use crate::bitwise::set_bit_spec;

verus! {

// --- bit_position ---

// r[depends permission.bit-position]
/// Spec: maps (who, perm) to a bit index in [0, 8].
/// who: 0=owner, 1=group, 2=other
/// perm: 0=execute, 1=write, 2=read
pub open spec fn bit_position_spec(who: u64, perm: u64) -> u64
    recommends who <= 2, perm <= 2
{
    ((2 - who as int) * 3 + perm as int) as u64
}

// r[impl permission.bit-position]
/// Returns the bit index for the given role and access type.
pub fn bit_position(who: u64, perm: u64) -> (result: u64)
    requires who <= 2, perm <= 2
    ensures result == bit_position_spec(who, perm), result <= 8
{
    (2 - who) * 3 + perm
}

// r[verify permission.bit-position]
/// Owner read is bit 8, other execute is bit 0.
proof fn bit_position_examples()
    ensures
        bit_position_spec(0, 2) == 8,  // owner read
        bit_position_spec(0, 1) == 7,  // owner write
        bit_position_spec(0, 0) == 6,  // owner execute
        bit_position_spec(1, 2) == 5,  // group read
        bit_position_spec(1, 1) == 4,  // group write
        bit_position_spec(1, 0) == 3,  // group execute
        bit_position_spec(2, 2) == 2,  // other read
        bit_position_spec(2, 1) == 1,  // other write
        bit_position_spec(2, 0) == 0,  // other execute
{}

// r[verify permission.bit-position]
/// Bit position is bounded by 8.
proof fn bit_position_bounded(who: u64, perm: u64)
    requires who <= 2, perm <= 2
    ensures bit_position_spec(who, perm) <= 8
{}

// r[verify permission.bit-position]
/// Different (who, perm) pairs produce different bit positions.
proof fn bit_position_injective(w1: u64, p1: u64, w2: u64, p2: u64)
    requires w1 <= 2, p1 <= 2, w2 <= 2, p2 <= 2
    ensures bit_position_spec(w1, p1) == bit_position_spec(w2, p2) ==>
        (w1 == w2 && p1 == p2)
{}

// --- has_permission ---

// r[depends permission.has]
/// Spec: true when the permission bit is set in mode.
pub open spec fn has_permission_spec(mode: u64, who: u64, perm: u64) -> bool
    recommends who <= 2, perm <= 2
{
    bit_at_spec(mode, bit_position_spec(who, perm))
}

// r[impl permission.has]
/// Returns true if the permission bit for (who, perm) is set in mode.
pub fn has_permission(mode: u64, who: u64, perm: u64) -> (result: bool)
    requires who <= 2, perm <= 2
    ensures result == has_permission_spec(mode, who, perm)
{
    let pos = bit_position(who, perm);
    bit_at(mode, pos)
}

// --- set_permission ---

// r[depends permission.set]
/// Spec: mode with the permission bit set.
pub open spec fn set_permission_spec(mode: u64, who: u64, perm: u64) -> u64
    recommends who <= 2, perm <= 2
{
    set_bit_spec(mode, bit_position_spec(who, perm))
}

// r[impl permission.set]
/// Returns mode with the permission bit for (who, perm) set to 1.
pub fn set_permission(mode: u64, who: u64, perm: u64) -> (result: u64)
    requires who <= 2, perm <= 2
    ensures result == set_permission_spec(mode, who, perm)
{
    let pos = bit_position(who, perm);
    set_bit(mode, pos)
}

// r[verify permission.set]
/// After setting, has_permission returns true.
proof fn set_then_has(mode: u64, who: u64, perm: u64)
    requires who <= 2, perm <= 2
    ensures has_permission_spec(set_permission_spec(mode, who, perm), who, perm)
{
    let pos = bit_position_spec(who, perm);
    assert(pos < 64 ==> ((mode | (1u64 << pos)) >> pos) & 1u64 == 1u64) by(bit_vector);
}

// r[verify permission.set]
/// Setting is idempotent.
proof fn set_idempotent(mode: u64, who: u64, perm: u64)
    requires who <= 2, perm <= 2
    ensures set_permission_spec(set_permission_spec(mode, who, perm), who, perm)
        == set_permission_spec(mode, who, perm)
{
    let pos = bit_position_spec(who, perm);
    assert(pos < 64 ==> (mode | (1u64 << pos)) | (1u64 << pos)
        == mode | (1u64 << pos)) by(bit_vector);
}

// --- clear_permission ---

// r[depends permission.clear]
/// Spec: mode with the permission bit cleared.
pub open spec fn clear_permission_spec(mode: u64, who: u64, perm: u64) -> u64
    recommends who <= 2, perm <= 2
{
    clear_bit_spec(mode, bit_position_spec(who, perm))
}

// r[impl permission.clear]
/// Returns mode with the permission bit for (who, perm) set to 0.
pub fn clear_permission(mode: u64, who: u64, perm: u64) -> (result: u64)
    requires who <= 2, perm <= 2
    ensures result == clear_permission_spec(mode, who, perm)
{
    let pos = bit_position(who, perm);
    clear_bit(mode, pos)
}

// r[verify permission.clear]
/// After clearing, has_permission returns false.
proof fn clear_then_not_has(mode: u64, who: u64, perm: u64)
    requires who <= 2, perm <= 2
    ensures !has_permission_spec(clear_permission_spec(mode, who, perm), who, perm)
{
    let pos = bit_position_spec(who, perm);
    assert(pos < 64 ==> ((mode & !(1u64 << pos)) >> pos) & 1u64 == 0u64) by(bit_vector);
}

// r[verify permission.clear]
/// Clearing is idempotent.
proof fn clear_idempotent(mode: u64, who: u64, perm: u64)
    requires who <= 2, perm <= 2
    ensures clear_permission_spec(clear_permission_spec(mode, who, perm), who, perm)
        == clear_permission_spec(mode, who, perm)
{
    let pos = bit_position_spec(who, perm);
    assert(pos < 64 ==> (mode & !(1u64 << pos)) & !(1u64 << pos)
        == mode & !(1u64 << pos)) by(bit_vector);
}

// r[verify permission.set]
// r[verify permission.clear]
/// Set then clear leaves the bit cleared.
proof fn set_clear_roundtrip(mode: u64, who: u64, perm: u64)
    requires who <= 2, perm <= 2
    ensures !has_permission_spec(
        clear_permission_spec(set_permission_spec(mode, who, perm), who, perm),
        who, perm,
    )
{
    let pos = bit_position_spec(who, perm);
    assert(pos < 64 ==>
        (((mode | (1u64 << pos)) & !(1u64 << pos)) >> pos) & 1u64 == 0u64
    ) by(bit_vector);
}

// r[verify permission.clear]
// r[verify permission.set]
/// Clear then set leaves the bit set.
proof fn clear_set_roundtrip(mode: u64, who: u64, perm: u64)
    requires who <= 2, perm <= 2
    ensures has_permission_spec(
        set_permission_spec(clear_permission_spec(mode, who, perm), who, perm),
        who, perm,
    )
{
    let pos = bit_position_spec(who, perm);
    assert(pos < 64 ==>
        (((mode & !(1u64 << pos)) | (1u64 << pos)) >> pos) & 1u64 == 1u64
    ) by(bit_vector);
}

// --- apply_umask ---

// r[depends permission.apply-umask]
/// Spec: apply umask by clearing masked bits.
pub open spec fn apply_umask_spec(mode: u64, umask: u64) -> u64 {
    mode & !umask
}

// r[impl permission.apply-umask]
/// Returns mode with all bits in umask cleared.
pub fn apply_umask(mode: u64, umask: u64) -> (result: u64)
    ensures result == apply_umask_spec(mode, umask)
{
    mode & !umask
}

// r[verify permission.apply-umask]
/// Result is a bitwise subset of mode.
proof fn umask_subset_of_mode(mode: u64, umask: u64)
    ensures apply_umask_spec(mode, umask) & mode == apply_umask_spec(mode, umask)
{
    assert((mode & !umask) & mode == mode & !umask) by(bit_vector);
}

// r[verify permission.apply-umask]
/// Result has no bits from umask.
proof fn umask_clears_masked_bits(mode: u64, umask: u64)
    ensures apply_umask_spec(mode, umask) & umask == 0
{
    assert((mode & !umask) & umask == 0u64) by(bit_vector);
}

// r[verify permission.apply-umask]
/// Zero umask is identity.
proof fn umask_zero_identity(mode: u64)
    ensures apply_umask_spec(mode, 0) == mode
{
    assert(mode & !0u64 == mode) by(bit_vector);
}

// r[verify permission.apply-umask]
/// Full umask clears everything.
proof fn umask_full_clears_all(mode: u64)
    ensures apply_umask_spec(mode, u64::MAX) == 0
{
    assert(mode & !u64::MAX == 0u64) by(bit_vector);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify permission.bit-position]
    #[test]
    fn test_bit_position_layout() {
        // Standard Unix permission bit layout (octal)
        assert_eq!(bit_position(0, 2), 8); // owner read   (0o400)
        assert_eq!(bit_position(0, 1), 7); // owner write  (0o200)
        assert_eq!(bit_position(0, 0), 6); // owner execute (0o100)
        assert_eq!(bit_position(1, 2), 5); // group read   (0o040)
        assert_eq!(bit_position(1, 1), 4); // group write  (0o020)
        assert_eq!(bit_position(1, 0), 3); // group execute (0o010)
        assert_eq!(bit_position(2, 2), 2); // other read   (0o004)
        assert_eq!(bit_position(2, 1), 1); // other write  (0o002)
        assert_eq!(bit_position(2, 0), 0); // other execute (0o001)
    }

    // r[verify permission.has]
    #[test]
    fn test_has_permission() {
        let mode = 0o755; // rwxr-xr-x
        // Owner: rwx
        assert!(has_permission(mode, 0, 2)); // read
        assert!(has_permission(mode, 0, 1)); // write
        assert!(has_permission(mode, 0, 0)); // execute
        // Group: r-x
        assert!(has_permission(mode, 1, 2)); // read
        assert!(!has_permission(mode, 1, 1)); // no write
        assert!(has_permission(mode, 1, 0)); // execute
        // Other: r-x
        assert!(has_permission(mode, 2, 2)); // read
        assert!(!has_permission(mode, 2, 1)); // no write
        assert!(has_permission(mode, 2, 0)); // execute
    }

    // r[verify permission.set]
    #[test]
    fn test_set_permission() {
        let mode = 0u64;
        let mode = set_permission(mode, 0, 2); // +owner read
        assert_eq!(mode, 0o400);
        let mode = set_permission(mode, 0, 1); // +owner write
        assert_eq!(mode, 0o600);
        let mode = set_permission(mode, 0, 0); // +owner execute
        assert_eq!(mode, 0o700);
    }

    // r[verify permission.clear]
    #[test]
    fn test_clear_permission() {
        let mode = 0o777u64; // rwxrwxrwx
        let mode = clear_permission(mode, 1, 1); // -group write
        assert_eq!(mode, 0o757);
        let mode = clear_permission(mode, 2, 1); // -other write
        assert_eq!(mode, 0o755);
    }

    // r[verify permission.apply-umask]
    #[test]
    fn test_apply_umask() {
        // Default umask 022 applied to 0o777
        assert_eq!(apply_umask(0o777, 0o022), 0o755);
        // Umask 077 applied to 0o777
        assert_eq!(apply_umask(0o777, 0o077), 0o700);
        // Zero umask does nothing
        assert_eq!(apply_umask(0o644, 0), 0o644);
    }

    proptest! {
        // r[verify permission.has]
        // r[verify permission.set]
        #[test]
        fn prop_set_then_has(mode: u64, who in 0u64..=2, perm in 0u64..=2) {
            prop_assert!(has_permission(set_permission(mode, who, perm), who, perm));
        }

        // r[verify permission.has]
        // r[verify permission.clear]
        #[test]
        fn prop_clear_then_not_has(mode: u64, who in 0u64..=2, perm in 0u64..=2) {
            prop_assert!(!has_permission(clear_permission(mode, who, perm), who, perm));
        }

        // r[verify permission.set]
        #[test]
        fn prop_set_idempotent(mode: u64, who in 0u64..=2, perm in 0u64..=2) {
            let once = set_permission(mode, who, perm);
            let twice = set_permission(once, who, perm);
            prop_assert_eq!(once, twice);
        }

        // r[verify permission.clear]
        #[test]
        fn prop_clear_idempotent(mode: u64, who in 0u64..=2, perm in 0u64..=2) {
            let once = clear_permission(mode, who, perm);
            let twice = clear_permission(once, who, perm);
            prop_assert_eq!(once, twice);
        }

        // r[verify permission.apply-umask]
        #[test]
        fn prop_umask_subset(mode: u64, umask: u64) {
            let result = apply_umask(mode, umask);
            prop_assert_eq!(result & mode, result);
        }

        // r[verify permission.apply-umask]
        #[test]
        fn prop_umask_clears_bits(mode: u64, umask: u64) {
            let result = apply_umask(mode, umask);
            prop_assert_eq!(result & umask, 0);
        }

        // r[verify permission.bit-position]
        #[test]
        fn prop_bit_position_bounded(who in 0u64..=2, perm in 0u64..=2) {
            prop_assert!(bit_position(who, perm) <= 8);
        }

        // r[verify permission.bit-position]
        // r[verify permission.has]
        // r[verify permission.set]
        // r[verify permission.clear]
        // r[verify permission.apply-umask]
        /// Model oracle: build mode from individual permission flags.
        #[test]
        fn prop_model_oracle(flags in proptest::collection::vec(proptest::bool::ANY, 9)) {
            let mut mode = 0u64;
            // Set permissions according to flags
            for (i, &flag) in flags.iter().enumerate() {
                let who = (2 - i / 3) as u64;
                let perm = (i % 3) as u64;
                if flag {
                    mode = set_permission(mode, who, perm);
                }
            }
            // Verify each permission matches the flag
            for (i, &flag) in flags.iter().enumerate() {
                let who = (2 - i / 3) as u64;
                let perm = (i % 3) as u64;
                prop_assert_eq!(has_permission(mode, who, perm), flag);
            }
        }
    }
}
