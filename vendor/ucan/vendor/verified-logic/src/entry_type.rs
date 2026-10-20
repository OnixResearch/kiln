//! Verified log entry type routing for Raft.
//!
//! Raft log entries come in three types: normal commands (applied to the
//! state machine), no-ops (identity operations inserted by new leaders),
//! and configuration changes (take effect on append, not commit). This
//! module provides the classification and routing predicates.

use vstd::prelude::*;

verus! {

/// Normal command entry — applied to state machine on commit.
pub const ENTRY_NORMAL: u8 = 0u8;
/// No-op entry — identity operation inserted by new leaders.
pub const ENTRY_NOOP: u8 = 1u8;
/// Configuration change entry — takes effect on append, not commit.
pub const ENTRY_CONFIG: u8 = 2u8;

/// No-op value (matches noop_commit::NOOP_VALUE).
pub const NOOP_VALUE: u64 = 0u64;

/// Sentinel value for configuration change entries.
/// Must differ from NOOP_VALUE (0) and any normal command value.
pub const CONFIG_CHANGE_SENTINEL: u64 = u64::MAX;

// --- classify_entry ---

// r[depends entry-type.classify]
/// Spec: classify a log entry by its value field.
pub open spec fn classify_entry_spec(value: u64) -> u8 {
    if value == NOOP_VALUE {
        ENTRY_NOOP
    } else if value == CONFIG_CHANGE_SENTINEL {
        ENTRY_CONFIG
    } else {
        ENTRY_NORMAL
    }
}

// r[impl entry-type.classify]
/// Classify a log entry's type based on its value.
pub fn classify_entry(value: u64) -> (result: u8)
    ensures result == classify_entry_spec(value)
{
    if value == NOOP_VALUE {
        ENTRY_NOOP
    } else if value == CONFIG_CHANGE_SENTINEL {
        ENTRY_CONFIG
    } else {
        ENTRY_NORMAL
    }
}

// --- should_apply_to_state_machine ---

// r[depends entry-type.should-apply]
/// Spec: should this entry type be applied to the state machine?
///
/// Only normal command entries affect state machine output. No-ops and
/// config changes are handled separately.
pub open spec fn should_apply_to_state_machine_spec(entry_type: u8) -> bool {
    entry_type == ENTRY_NORMAL
}

// r[impl entry-type.should-apply]
/// Check whether this entry type should be applied to the state machine.
pub fn should_apply_to_state_machine(entry_type: u8) -> (result: bool)
    ensures result == should_apply_to_state_machine_spec(entry_type)
{
    entry_type == ENTRY_NORMAL
}

// --- is_config_change ---

// r[depends entry-type.is-config-change]
/// Spec: is this entry a configuration change?
pub open spec fn is_config_change_spec(entry_type: u8) -> bool {
    entry_type == ENTRY_CONFIG
}

// r[impl entry-type.is-config-change]
/// Check whether this entry is a configuration change.
pub fn is_config_change(entry_type: u8) -> (result: bool)
    ensures result == is_config_change_spec(entry_type)
{
    entry_type == ENTRY_CONFIG
}

// --- takes_effect_on_append ---

// r[depends entry-type.takes-effect-on-append]
/// Spec: does this entry type take effect when appended to the log?
///
/// Only config changes take effect on append. Normal entries and no-ops
/// take effect on commit (application to state machine).
pub open spec fn takes_effect_on_append_spec(entry_type: u8) -> bool {
    entry_type == ENTRY_CONFIG
}

// r[impl entry-type.takes-effect-on-append]
/// Check whether this entry takes effect on append rather than commit.
pub fn takes_effect_on_append(entry_type: u8) -> (result: bool)
    ensures result == takes_effect_on_append_spec(entry_type)
{
    entry_type == ENTRY_CONFIG
}

// --- is_noop_entry ---

// r[depends entry-type.is-noop]
/// Spec: is this entry a no-op?
pub open spec fn is_noop_entry_spec(entry_type: u8) -> bool {
    entry_type == ENTRY_NOOP
}

// r[impl entry-type.is-noop]
/// Check whether this entry is a no-op.
pub fn is_noop_entry(entry_type: u8) -> (result: bool)
    ensures result == is_noop_entry_spec(entry_type)
{
    entry_type == ENTRY_NOOP
}

// --- Safety proofs ---

// r[verify entry-type.classify]
/// Classification is exhaustive: every value maps to one of three types.
proof fn classify_exhaustive(value: u64)
    ensures ({
        let t = classify_entry_spec(value);
        t == ENTRY_NORMAL || t == ENTRY_NOOP || t == ENTRY_CONFIG
    })
{}

// r[verify entry-type.classify]
/// NOOP_VALUE classifies as ENTRY_NOOP.
proof fn noop_value_classifies_as_noop()
    ensures classify_entry_spec(NOOP_VALUE) == ENTRY_NOOP
{}

// r[verify entry-type.classify]
/// CONFIG_CHANGE_SENTINEL classifies as ENTRY_CONFIG.
proof fn config_sentinel_classifies_as_config()
    ensures classify_entry_spec(CONFIG_CHANGE_SENTINEL) == ENTRY_CONFIG
{}

// r[verify entry-type.classify]
/// Non-sentinel values classify as ENTRY_NORMAL.
proof fn normal_value_classifies_as_normal(value: u64)
    requires value != NOOP_VALUE, value != CONFIG_CHANGE_SENTINEL
    ensures classify_entry_spec(value) == ENTRY_NORMAL
{}

// r[verify entry-type.should-apply]
/// Normal entries are applied.
proof fn normal_applied()
    ensures should_apply_to_state_machine_spec(ENTRY_NORMAL)
{}

// r[verify entry-type.should-apply]
/// No-ops are not applied.
proof fn noop_not_applied()
    ensures !should_apply_to_state_machine_spec(ENTRY_NOOP)
{}

// r[verify entry-type.should-apply]
/// Config changes are not applied to the state machine.
proof fn config_not_applied()
    ensures !should_apply_to_state_machine_spec(ENTRY_CONFIG)
{}

// r[verify entry-type.is-config-change]
/// Only ENTRY_CONFIG returns true for is_config_change.
proof fn only_config_is_config()
    ensures
        !is_config_change_spec(ENTRY_NORMAL),
        !is_config_change_spec(ENTRY_NOOP),
        is_config_change_spec(ENTRY_CONFIG),
{}

// r[verify entry-type.takes-effect-on-append]
/// Only config entries take effect on append.
proof fn only_config_on_append()
    ensures
        !takes_effect_on_append_spec(ENTRY_NORMAL),
        !takes_effect_on_append_spec(ENTRY_NOOP),
        takes_effect_on_append_spec(ENTRY_CONFIG),
{}

// r[verify entry-type.is-noop]
/// Only ENTRY_NOOP returns true for is_noop_entry.
proof fn only_noop_is_noop()
    ensures
        !is_noop_entry_spec(ENTRY_NORMAL),
        is_noop_entry_spec(ENTRY_NOOP),
        !is_noop_entry_spec(ENTRY_CONFIG),
{}

// r[verify entry-type.classify]
/// The three entry types have mutually exclusive apply/config/noop predicates.
proof fn types_mutually_exclusive(entry_type: u8)
    requires entry_type == ENTRY_NORMAL || entry_type == ENTRY_NOOP || entry_type == ENTRY_CONFIG
    ensures ({
        let apply = should_apply_to_state_machine_spec(entry_type);
        let config = is_config_change_spec(entry_type);
        let noop = is_noop_entry_spec(entry_type);
        // Exactly one is true
        (apply && !config && !noop)
        || (!apply && config && !noop)
        || (!apply && !config && noop)
    })
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify entry-type.classify]
    #[test]
    fn test_classify_entry() {
        assert_eq!(classify_entry(0), ENTRY_NOOP); // NOOP_VALUE = 0
        assert_eq!(classify_entry(u64::MAX), ENTRY_CONFIG); // CONFIG_CHANGE_SENTINEL
        assert_eq!(classify_entry(1), ENTRY_NORMAL);
        assert_eq!(classify_entry(42), ENTRY_NORMAL);
        assert_eq!(classify_entry(u64::MAX - 1), ENTRY_NORMAL);
    }

    // r[verify entry-type.should-apply]
    #[test]
    fn test_should_apply() {
        assert!(should_apply_to_state_machine(ENTRY_NORMAL));
        assert!(!should_apply_to_state_machine(ENTRY_NOOP));
        assert!(!should_apply_to_state_machine(ENTRY_CONFIG));
    }

    // r[verify entry-type.is-config-change]
    #[test]
    fn test_is_config_change() {
        assert!(!is_config_change(ENTRY_NORMAL));
        assert!(!is_config_change(ENTRY_NOOP));
        assert!(is_config_change(ENTRY_CONFIG));
    }

    // r[verify entry-type.takes-effect-on-append]
    #[test]
    fn test_takes_effect_on_append() {
        assert!(!takes_effect_on_append(ENTRY_NORMAL));
        assert!(!takes_effect_on_append(ENTRY_NOOP));
        assert!(takes_effect_on_append(ENTRY_CONFIG));
    }

    // r[verify entry-type.is-noop]
    #[test]
    fn test_is_noop_entry() {
        assert!(!is_noop_entry(ENTRY_NORMAL));
        assert!(is_noop_entry(ENTRY_NOOP));
        assert!(!is_noop_entry(ENTRY_CONFIG));
    }

    proptest! {
        // r[verify entry-type.classify]
        #[test]
        fn prop_classify_exhaustive(value: u64) {
            let t = classify_entry(value);
            prop_assert!(t == ENTRY_NORMAL || t == ENTRY_NOOP || t == ENTRY_CONFIG);
        }

        // r[verify entry-type.classify]
        #[test]
        fn prop_classify_model_oracle(value: u64) {
            let t = classify_entry(value);
            let expected = if value == 0 {
                ENTRY_NOOP
            } else if value == u64::MAX {
                ENTRY_CONFIG
            } else {
                ENTRY_NORMAL
            };
            prop_assert_eq!(t, expected);
        }

        // r[verify entry-type.classify]
        #[test]
        fn prop_types_mutually_exclusive(value: u64) {
            let t = classify_entry(value);
            let apply = should_apply_to_state_machine(t);
            let config = is_config_change(t);
            let noop = is_noop_entry(t);
            // At most one of config/noop, normal applies to SM
            let count = apply as u8 + config as u8 + noop as u8;
            prop_assert_eq!(count, 1, "exactly one predicate should be true");
        }

        #[test]
        fn prop_noop_sentinel_roundtrip(term: u64) {
            let entry = crate::log::LogEntry { term, value: NOOP_VALUE };
            prop_assert!(is_noop_entry(classify_entry(entry.value)));
        }

        #[test]
        fn prop_config_sentinel_roundtrip(term: u64) {
            let entry = crate::log::LogEntry { term, value: CONFIG_CHANGE_SENTINEL };
            prop_assert!(is_config_change(classify_entry(entry.value)));
        }

        #[test]
        fn prop_normal_applies_to_sm(value in 1u64..u64::MAX) {
            if value != CONFIG_CHANGE_SENTINEL {
                let t = classify_entry(value);
                prop_assert!(should_apply_to_state_machine(t));
            }
        }
    }
}
