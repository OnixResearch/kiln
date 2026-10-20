//! Exact checked arithmetic envelope primitives.
use vstd::prelude::*;

verus! {
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NumericEnvelope { pub min: i32, pub max: i32 }
pub const NUMERIC_OP_ADD:u8=1; pub const NUMERIC_OP_SUB:u8=2; pub const NUMERIC_OP_MUL:u8=3; pub const NUMERIC_OP_DIV:u8=4;
// r[depends domain-primitives.numeric-checked-arithmetic-envelope]
pub open spec fn numeric_envelope_valid_spec(e: NumericEnvelope) -> bool { e.min <= e.max }
// r[depends domain-primitives.numeric-checked-arithmetic-envelope]
pub open spec fn numeric_op_result_spec(op:u8, a:i16, b:i16) -> Option<int> { if op==NUMERIC_OP_ADD { Some(a as int + b as int) } else if op==NUMERIC_OP_SUB { Some(a as int - b as int) } else if op==NUMERIC_OP_MUL { Some((a as int) * (b as int)) } else if op==NUMERIC_OP_DIV && b != 0 && a == 0 { Some(0) } else { None } }
// r[depends domain-primitives.numeric-checked-arithmetic-envelope]
pub open spec fn numeric_checked_arithmetic_spec(e: NumericEnvelope, op:u8, a:i16, b:i16) -> bool { numeric_envelope_valid_spec(e) && numeric_op_result_spec(op,a,b).is_some() && e.min as int <= numeric_op_result_spec(op,a,b).unwrap() && numeric_op_result_spec(op,a,b).unwrap() <= e.max as int }
// r[impl domain-primitives.numeric-checked-arithmetic-envelope]
pub fn numeric_checked_arithmetic(e: NumericEnvelope, op:u8, a:i16, b:i16) -> (result: bool) ensures result == numeric_checked_arithmetic_spec(e,op,a,b) { if !(e.min<=e.max) { false } else if op==NUMERIC_OP_ADD { let r=(a as i32)+(b as i32); e.min<=r && r<=e.max } else if op==NUMERIC_OP_SUB { let r=(a as i32)-(b as i32); e.min<=r && r<=e.max } else if op==NUMERIC_OP_MUL { assert((a as int)*(b as int) <= 32768int*32768int) by(nonlinear_arith); assert(-(32768int*32768int) <= (a as int)*(b as int)) by(nonlinear_arith); let r=(a as i32)*(b as i32); e.min<=r && r<=e.max } else if op==NUMERIC_OP_DIV && b!=0 && a==0 { e.min<=0 && 0<=e.max } else { false } }
// r[verify domain-primitives.numeric-checked-arithmetic-envelope]
pub proof fn numeric_checked_arithmetic_has_envelope(e: NumericEnvelope, op:u8, a:i16, b:i16) requires numeric_checked_arithmetic_spec(e,op,a,b) ensures numeric_envelope_valid_spec(e) {}
} // verus!
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arithmetic_rejects_out_of_bounds() {
        let e = NumericEnvelope { min: -10, max: 10 };
        assert!(numeric_checked_arithmetic(e, NUMERIC_OP_ADD, 3, 4));
        assert!(!numeric_checked_arithmetic(e, NUMERIC_OP_ADD, 8, 4));
        assert!(!numeric_checked_arithmetic(e, NUMERIC_OP_DIV, 1, 0));
    }
}
