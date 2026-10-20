//! Exact finite default-value metadata admission.
use vstd::prelude::*;

use crate::config_key::ConfigKeySet;
use crate::config_key::config_key_known;
use crate::config_typed_value::ConfigTypedValue;
use crate::config_typed_value::config_typed_value_admitted;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ConfigDefault { pub namespace: u8, pub key: u16, pub value: ConfigTypedValue }

// r[depends domain-primitives.configuration-feature-flag.default]
pub open spec fn config_default_admitted_spec(keys: ConfigKeySet, d: ConfigDefault) -> bool { crate::config_key::config_key_known_spec(keys, d.namespace, d.key) && crate::config_typed_value::config_typed_value_admitted_spec(d.value) }
// r[impl domain-primitives.configuration-feature-flag.default]
pub fn config_default_admitted(keys: ConfigKeySet, d: ConfigDefault) -> (result: bool) ensures result == config_default_admitted_spec(keys, d) { config_key_known(keys, d.namespace, d.key) && config_typed_value_admitted(d.value) }
// r[verify domain-primitives.configuration-feature-flag.default]
pub proof fn unknown_default_key_rejected(keys: ConfigKeySet, d: ConfigDefault) requires !crate::config_key::config_key_known_spec(keys, d.namespace, d.key) ensures !config_default_admitted_spec(keys, d) {}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_key::ConfigKeySet;
    use crate::config_typed_value::ConfigTypedValue;
    #[test]
    fn default_requires_key_and_value() {
        let keys = ConfigKeySet {
            namespace: 1,
            key0: 2,
            key1: 0,
            len: 1,
        };
        let v = ConfigTypedValue {
            tag: 0,
            bool_value: true,
            int_value: 0,
            min_int: 0,
            max_int: 0,
            text_len: 0,
            max_text_len: 0,
        };
        assert!(config_default_admitted(keys, ConfigDefault {
            namespace: 1,
            key: 2,
            value: v
        }));
        assert!(!config_default_admitted(keys, ConfigDefault {
            namespace: 1,
            key: 3,
            value: v
        }));
    }
}
