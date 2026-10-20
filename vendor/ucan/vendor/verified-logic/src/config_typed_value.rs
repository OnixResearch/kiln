//! Exact finite typed configuration value admission.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ConfigTypedValue { pub tag: u8, pub bool_value: bool, pub int_value: i16, pub min_int: i16, pub max_int: i16, pub text_len: u8, pub max_text_len: u8 }

// r[depends domain-primitives.configuration-feature-flag.typed-value]
pub open spec fn config_typed_value_admitted_spec(v: ConfigTypedValue) -> bool { if v.tag == 0 { true } else if v.tag == 1 { v.min_int <= v.int_value && v.int_value <= v.max_int } else if v.tag == 2 { v.text_len <= v.max_text_len && v.max_text_len <= 32 } else { false } }
// r[impl domain-primitives.configuration-feature-flag.typed-value]
pub fn config_typed_value_admitted(v: ConfigTypedValue) -> (result: bool) ensures result == config_typed_value_admitted_spec(v) { if v.tag == 0 { true } else if v.tag == 1 { v.min_int <= v.int_value && v.int_value <= v.max_int } else if v.tag == 2 { v.text_len <= v.max_text_len && v.max_text_len <= 32 } else { false } }
// r[verify domain-primitives.configuration-feature-flag.typed-value]
pub proof fn invalid_tag_rejected(v: ConfigTypedValue) requires v.tag != 0, v.tag != 1, v.tag != 2 ensures !config_typed_value_admitted_spec(v) {}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_value_rejects_mismatch() {
        let ok = ConfigTypedValue {
            tag: 1,
            bool_value: false,
            int_value: 5,
            min_int: 0,
            max_int: 10,
            text_len: 0,
            max_text_len: 0,
        };
        let bad = ConfigTypedValue {
            tag: 1,
            bool_value: false,
            int_value: 50,
            min_int: 0,
            max_int: 10,
            text_len: 0,
            max_text_len: 0,
        };
        assert!(config_typed_value_admitted(ok));
        assert!(!config_typed_value_admitted(bad));
    }
}
