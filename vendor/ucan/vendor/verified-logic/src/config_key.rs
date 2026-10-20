//! Exact finite configuration key and namespace admission.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ConfigKeySet { pub namespace: u8, pub key0: u16, pub key1: u16, pub len: u8 }

// r[depends domain-primitives.configuration-feature-flag.key]
pub open spec fn config_key_set_valid_spec(s: ConfigKeySet) -> bool { s.len <= 2 && (s.len < 2 || s.key0 != s.key1) }
// r[depends domain-primitives.configuration-feature-flag.key]
pub open spec fn config_key_known_spec(s: ConfigKeySet, namespace: u8, key: u16) -> bool { config_key_set_valid_spec(s) && namespace == s.namespace && ((s.len >= 1 && key == s.key0) || (s.len >= 2 && key == s.key1)) }
// r[impl domain-primitives.configuration-feature-flag.key]
pub fn config_key_set_valid(s: ConfigKeySet) -> (result: bool) ensures result == config_key_set_valid_spec(s) { s.len <= 2 && (s.len < 2 || s.key0 != s.key1) }
// r[impl domain-primitives.configuration-feature-flag.key]
pub fn config_key_known(s: ConfigKeySet, namespace: u8, key: u16) -> (result: bool) ensures result == config_key_known_spec(s, namespace, key) { config_key_set_valid(s) && namespace == s.namespace && ((s.len >= 1 && key == s.key0) || (s.len >= 2 && key == s.key1)) }
// r[verify domain-primitives.configuration-feature-flag.key]
pub proof fn unknown_key_rejected(s: ConfigKeySet, namespace: u8, key: u16) requires config_key_set_valid_spec(s), !((namespace == s.namespace) && ((s.len >= 1 && key == s.key0) || (s.len >= 2 && key == s.key1))) ensures !config_key_known_spec(s, namespace, key) {}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn key_membership_is_finite() {
        let s = ConfigKeySet {
            namespace: 7,
            key0: 10,
            key1: 11,
            len: 2,
        };
        assert!(config_key_known(s, 7, 10));
        assert!(!config_key_known(s, 7, 12));
        assert!(!config_key_known(s, 8, 10));
    }
}
