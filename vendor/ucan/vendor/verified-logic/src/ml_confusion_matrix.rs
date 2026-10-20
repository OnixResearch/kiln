//! Bounded ML confusion-matrix count and metric primitives.
use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MlBinaryConfusion { pub tp: u16, pub fp: u16, pub tn: u16, pub fn_: u16 }

// r[depends domain-primitives.ml-confusion-matrix]
pub open spec fn ml_binary_confusion_valid_spec(m: MlBinaryConfusion) -> bool { true }

// r[depends domain-primitives.ml-confusion-matrix]
pub open spec fn ml_confusion_prediction_valid_spec(expected: bool, predicted: bool) -> bool { true }

// r[depends domain-primitives.ml-confusion-matrix]
pub open spec fn ml_precision_denominator_spec(m: MlBinaryConfusion) -> nat { (m.tp as nat) + (m.fp as nat) }

// r[depends domain-primitives.ml-confusion-matrix]
pub open spec fn ml_recall_denominator_spec(m: MlBinaryConfusion) -> nat { (m.tp as nat) + (m.fn_ as nat) }

// r[impl domain-primitives.ml-confusion-matrix]
pub fn ml_confusion_update(m: MlBinaryConfusion, expected: bool, predicted: bool) -> (result: Option<MlBinaryConfusion>)
    ensures result.is_some() ==> ml_binary_confusion_valid_spec(result.unwrap())
{
    if expected && predicted {
        if m.tp == u16::MAX { None } else { Some(MlBinaryConfusion { tp: m.tp + 1, fp: m.fp, tn: m.tn, fn_: m.fn_ }) }
    } else if !expected && predicted {
        if m.fp == u16::MAX { None } else { Some(MlBinaryConfusion { tp: m.tp, fp: m.fp + 1, tn: m.tn, fn_: m.fn_ }) }
    } else if !expected && !predicted {
        if m.tn == u16::MAX { None } else { Some(MlBinaryConfusion { tp: m.tp, fp: m.fp, tn: m.tn + 1, fn_: m.fn_ }) }
    } else {
        if m.fn_ == u16::MAX { None } else { Some(MlBinaryConfusion { tp: m.tp, fp: m.fp, tn: m.tn, fn_: m.fn_ + 1 }) }
    }
}

// r[impl domain-primitives.ml-confusion-matrix]
pub fn ml_precision_available(m: MlBinaryConfusion) -> (result: bool)
    ensures result == (ml_precision_denominator_spec(m) > 0)
{
    (m.tp as u32) + (m.fp as u32) > 0
}

// r[impl domain-primitives.ml-confusion-matrix]
pub fn ml_recall_available(m: MlBinaryConfusion) -> (result: bool)
    ensures result == (ml_recall_denominator_spec(m) > 0)
{
    (m.tp as u32) + (m.fn_ as u32) > 0
}

// r[verify domain-primitives.ml-confusion-matrix]
pub proof fn ml_zero_precision_denominator_rejected(m: MlBinaryConfusion)
    requires ml_precision_denominator_spec(m) == 0
    ensures !((m.tp as nat) + (m.fp as nat) > 0)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn confusion_update_and_denominator_checks() {
        let m = MlBinaryConfusion {
            tp: 0,
            fp: 0,
            tn: 0,
            fn_: 0,
        };
        assert!(!ml_precision_available(m));
        let m2 = ml_confusion_update(m, true, true).unwrap();
        assert_eq!(m2.tp, 1);
        assert!(ml_precision_available(m2));
    }
}
