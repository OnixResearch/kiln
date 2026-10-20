//! Deterministic finite ML top-k primitives for a two-score vector.
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MlTop2 { pub first: u8, pub second: u8 }

// r[depends domain-primitives.ml-top-k]
pub open spec fn ml_top_k_valid_spec(len: u8, k: u8) -> bool { 0 < k && k <= len && len <= 2 }

// r[depends domain-primitives.ml-top-k]
pub open spec fn ml_top2_spec(score0: i32, score1: i32) -> MlTop2 {
    if score0 >= score1 { MlTop2 { first: 0, second: 1 } } else { MlTop2 { first: 1, second: 0 } }
}

// r[impl domain-primitives.ml-top-k]
pub fn ml_top_k_admit(len: u8, k: u8) -> (result: bool)
    ensures result == ml_top_k_valid_spec(len, k)
{
    0 < k && k <= len && len <= 2
}

// r[impl domain-primitives.ml-top-k]
pub fn ml_top2(score0: i32, score1: i32) -> (result: MlTop2)
    ensures result == ml_top2_spec(score0, score1), result.first <= 1, result.second <= 1, result.first != result.second
{
    if score0 >= score1 { MlTop2 { first: 0, second: 1 } } else { MlTop2 { first: 1, second: 0 } }
}

// r[verify domain-primitives.ml-top-k]
pub proof fn ml_top2_indices_in_range(score0: i32, score1: i32)
    ensures ml_top2_spec(score0, score1).first <= 1,
            ml_top2_spec(score0, score1).second <= 1,
            ml_top2_spec(score0, score1).first != ml_top2_spec(score0, score1).second
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn top2_orders_with_stable_tie_break() {
        assert_eq!(ml_top2(5, 7), MlTop2 { first: 1, second: 0 });
        assert_eq!(ml_top2(7, 7), MlTop2 { first: 0, second: 1 });
        assert!(!ml_top_k_admit(2, 0));
    }
}
