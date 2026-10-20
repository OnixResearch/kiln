//! Exact fixed-point scale admission primitives.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NumericFixedScale { pub scale: u16, pub min_raw: i32, pub max_raw: i32 }
// r[depends domain-primitives.numeric-fixed-point-scale]
pub open spec fn numeric_fixed_scale_valid_spec(s: NumericFixedScale) -> bool { s.scale > 0 && s.min_raw <= s.max_raw }
// r[depends domain-primitives.numeric-fixed-point-scale]
pub open spec fn numeric_fixed_value_admit_spec(s: NumericFixedScale, raw: i32) -> bool { numeric_fixed_scale_valid_spec(s) && s.min_raw <= raw && raw <= s.max_raw }
// r[impl domain-primitives.numeric-fixed-point-scale]
pub fn numeric_fixed_scale_valid(s: NumericFixedScale) -> (result: bool) ensures result == numeric_fixed_scale_valid_spec(s) { s.scale > 0 && s.min_raw <= s.max_raw }
// r[impl domain-primitives.numeric-fixed-point-scale]
pub fn numeric_fixed_value_admit(s: NumericFixedScale, raw: i32) -> (result: bool) ensures result == numeric_fixed_value_admit_spec(s, raw) { numeric_fixed_scale_valid(s) && s.min_raw <= raw && raw <= s.max_raw }
// r[verify domain-primitives.numeric-fixed-point-scale]
pub proof fn numeric_fixed_admit_has_valid_scale(s: NumericFixedScale, raw: i32) requires numeric_fixed_value_admit_spec(s, raw) ensures numeric_fixed_scale_valid_spec(s) {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_scale_admits_range() {
        let s = NumericFixedScale {
            scale: 100,
            min_raw: -5,
            max_raw: 5,
        };
        assert!(numeric_fixed_value_admit(s, 0));
        assert!(!numeric_fixed_value_admit(NumericFixedScale { scale: 0, ..s }, 0));
        assert!(!numeric_fixed_value_admit(s, 6));
    }
}
