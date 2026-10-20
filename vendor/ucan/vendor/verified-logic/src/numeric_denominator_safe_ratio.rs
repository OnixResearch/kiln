//! Exact denominator-safe ratio primitives.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)] pub struct NumericRatio { pub numerator:i32, pub denominator:i32 }
// r[depends domain-primitives.numeric-denominator-safe-ratio]
pub open spec fn numeric_ratio_valid_spec(r: NumericRatio)->bool { r.denominator != 0 }
// r[impl domain-primitives.numeric-denominator-safe-ratio]
pub fn numeric_ratio_valid(r: NumericRatio)->(result:bool) ensures result == numeric_ratio_valid_spec(r) { r.denominator != 0 }
// r[verify domain-primitives.numeric-denominator-safe-ratio]
pub proof fn numeric_ratio_valid_nonzero(r: NumericRatio) requires numeric_ratio_valid_spec(r) ensures r.denominator != 0 {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ratio_rejects_zero_denominator() {
        assert!(numeric_ratio_valid(NumericRatio {
            numerator: 1,
            denominator: 2
        }));
        assert!(!numeric_ratio_valid(NumericRatio {
            numerator: 1,
            denominator: 0
        }));
    }
}
