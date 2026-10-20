//! Verified ChaosControl assertion-oracle verdict and accounting primitives.
//!
//! Models the four assertion kinds (always, sometimes, reachable, unreachable)
//! and their verdict rules over valid aggregate records.

use vstd::prelude::*;

verus! {

// ── Assertion kind ──────────────────────────────────────────────────

/// Assertion kind tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssertionKind {
    Always,
    Sometimes,
    Reachable,
    Unreachable,
}

/// Verdict outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Passed,
    Failed,
}

/// Aggregate assertion record.
pub struct OracleRecord {
    pub kind: AssertionKind,
    pub hit_count: u64,
    pub true_count: u64,
    pub false_count: u64,
    pub runs_hit: u32,
    pub runs_satisfied: u32,
}

// ── Spec helpers ────────────────────────────────────────────────────

/// Shared validity for all records.
pub open spec fn valid_shared_spec(r: OracleRecord) -> bool {
    &&& r.runs_satisfied <= r.runs_hit
    &&& (r.runs_hit == 0 <==> r.hit_count == 0)
}

/// Valid boolean record (always or sometimes).
pub open spec fn valid_boolean_spec(r: OracleRecord) -> bool {
    &&& valid_shared_spec(r)
    &&& (r.kind == AssertionKind::Always || r.kind == AssertionKind::Sometimes)
    &&& r.hit_count == r.true_count + r.false_count
    &&& r.runs_hit as u64 <= r.hit_count
}

/// Valid sometimes record: strengthened with runs_satisfied <= true_count.
pub open spec fn valid_sometimes_spec(r: OracleRecord) -> bool {
    &&& valid_boolean_spec(r)
    &&& r.kind == AssertionKind::Sometimes
    &&& r.runs_satisfied as u64 <= r.true_count
}

/// Valid reachable record: exact shape.
pub open spec fn valid_reachable_spec(r: OracleRecord) -> bool {
    &&& valid_shared_spec(r)
    &&& r.kind == AssertionKind::Reachable
    &&& r.true_count == r.hit_count
    &&& r.false_count == 0
    &&& r.runs_satisfied == r.runs_hit
}

/// Valid unreachable record: exact shape.
pub open spec fn valid_unreachable_spec(r: OracleRecord) -> bool {
    &&& valid_shared_spec(r)
    &&& r.kind == AssertionKind::Unreachable
    &&& r.true_count == 0
    &&& r.false_count == r.hit_count
    &&& r.runs_satisfied == 0
}

/// Overall validity: shared + kind-specific.
pub open spec fn valid_oracle_record_spec(r: OracleRecord) -> bool {
    match r.kind {
        AssertionKind::Always => valid_boolean_spec(r),
        AssertionKind::Sometimes => valid_sometimes_spec(r),
        AssertionKind::Reachable => valid_reachable_spec(r),
        AssertionKind::Unreachable => valid_unreachable_spec(r),
    }
}

/// Verdict spec.
pub open spec fn verdict_spec(r: OracleRecord) -> Verdict
    recommends valid_oracle_record_spec(r)
{
    match r.kind {
        AssertionKind::Always => {
            if r.hit_count > 0 && r.false_count == 0 { Verdict::Passed } else { Verdict::Failed }
        }
        AssertionKind::Sometimes => {
            if r.hit_count > 0 && r.true_count > 0 { Verdict::Passed } else { Verdict::Failed }
        }
        AssertionKind::Reachable => {
            if r.hit_count > 0 { Verdict::Passed } else { Verdict::Failed }
        }
        AssertionKind::Unreachable => {
            if r.hit_count == 0 { Verdict::Passed } else { Verdict::Failed }
        }
    }
}

// ── Proof artifacts ─────────────────────────────────────────────────

// r[verify chaoscontrol-oracle.verdict]
/// Verdict is total: every valid record gets exactly one outcome.
pub proof fn verdict_total(r: OracleRecord)
    requires valid_oracle_record_spec(r)
    ensures
        verdict_spec(r) == Verdict::Passed || verdict_spec(r) == Verdict::Failed,
{
}

// r[verify chaoscontrol-oracle.verdict]
/// Pass and fail are complements for each kind.
pub proof fn verdict_complement(r: OracleRecord)
    requires valid_oracle_record_spec(r)
    ensures
        (verdict_spec(r) == Verdict::Passed) <==> !(verdict_spec(r) == Verdict::Failed),
{
}

// r[verify chaoscontrol-oracle.accounting]
/// Shared invariant holds for all valid records.
pub proof fn accounting_shared_invariant(r: OracleRecord)
    requires valid_oracle_record_spec(r)
    ensures
        r.runs_satisfied <= r.runs_hit,
        r.runs_hit == 0 <==> r.hit_count == 0,
{
}

// r[verify chaoscontrol-oracle.accounting]
/// Boolean records split hit_count into true and false.
pub proof fn accounting_boolean_split(r: OracleRecord)
    requires valid_boolean_spec(r)
    ensures
        r.hit_count == r.true_count + r.false_count,
        r.runs_hit as u64 <= r.hit_count,
{
}

// r[verify chaoscontrol-oracle.accounting]
/// Non-boolean reachable record shapes.
pub proof fn accounting_reachable_shape(r: OracleRecord)
    requires valid_reachable_spec(r)
    ensures
        r.true_count == r.hit_count,
        r.false_count == 0,
        r.runs_satisfied == r.runs_hit,
{
}

// r[verify chaoscontrol-oracle.accounting]
/// Non-boolean unreachable record shapes.
pub proof fn accounting_unreachable_shape(r: OracleRecord)
    requires valid_unreachable_spec(r)
    ensures
        r.true_count == 0,
        r.false_count == r.hit_count,
        r.runs_satisfied == 0,
{
}

// r[verify chaoscontrol-oracle.verdict]
/// Always: passes iff hit and never false.
pub proof fn always_pass_iff(r: OracleRecord)
    requires valid_oracle_record_spec(r), r.kind == AssertionKind::Always
    ensures
        (verdict_spec(r) == Verdict::Passed) <==> (r.hit_count > 0 && r.false_count == 0),
{
}

// r[verify chaoscontrol-oracle.verdict]
/// Sometimes: passes iff hit and at least one true.
pub proof fn sometimes_pass_iff(r: OracleRecord)
    requires valid_oracle_record_spec(r), r.kind == AssertionKind::Sometimes
    ensures
        (verdict_spec(r) == Verdict::Passed) <==> (r.hit_count > 0 && r.true_count > 0),
{
}

// r[verify chaoscontrol-oracle.verdict]
/// Reachable: passes iff hit.
pub proof fn reachable_pass_iff(r: OracleRecord)
    requires valid_oracle_record_spec(r), r.kind == AssertionKind::Reachable
    ensures
        (verdict_spec(r) == Verdict::Passed) <==> (r.hit_count > 0),
{
}

// r[verify chaoscontrol-oracle.verdict]
/// Unreachable: passes iff never hit.
pub proof fn unreachable_pass_iff(r: OracleRecord)
    requires valid_oracle_record_spec(r), r.kind == AssertionKind::Unreachable
    ensures
        (verdict_spec(r) == Verdict::Passed) <==> (r.hit_count == 0),
{
}

// ── Exec constructors ───────────────────────────────────────────────

// r[impl chaoscontrol-oracle.accounting]
/// Construct a valid boolean always record.
pub fn new_boolean_always_record(
    hit_count: u64, true_count: u64, false_count: u64,
    runs_hit: u32, runs_satisfied: u32,
) -> (result: OracleRecord)
    requires
        hit_count == true_count + false_count,
        runs_hit as u64 <= hit_count,
        runs_satisfied <= runs_hit,
        (runs_hit == 0) <==> (hit_count == 0),
    ensures valid_oracle_record_spec(result)
{
    OracleRecord {
        kind: AssertionKind::Always,
        hit_count, true_count, false_count, runs_hit, runs_satisfied,
    }
}

// r[impl chaoscontrol-oracle.accounting]
/// Construct a valid boolean sometimes record.
pub fn new_boolean_sometimes_record(
    hit_count: u64, true_count: u64, false_count: u64,
    runs_hit: u32, runs_satisfied: u32,
) -> (result: OracleRecord)
    requires
        hit_count == true_count + false_count,
        runs_hit as u64 <= hit_count,
        runs_satisfied <= runs_hit,
        (runs_hit == 0) <==> (hit_count == 0),
        runs_satisfied as u64 <= true_count,
    ensures valid_oracle_record_spec(result)
{
    OracleRecord {
        kind: AssertionKind::Sometimes,
        hit_count, true_count, false_count, runs_hit, runs_satisfied,
    }
}

// r[impl chaoscontrol-oracle.accounting]
/// Construct a valid reachable record.
pub fn new_reachable_record(hit_count: u64, runs_hit: u32) -> (result: OracleRecord)
    requires
        (runs_hit == 0) <==> (hit_count == 0),
        runs_hit as u64 <= hit_count,
    ensures valid_oracle_record_spec(result)
{
    OracleRecord {
        kind: AssertionKind::Reachable,
        hit_count,
        true_count: hit_count,
        false_count: 0,
        runs_hit,
        runs_satisfied: runs_hit,
    }
}

// r[impl chaoscontrol-oracle.accounting]
/// Construct a valid unreachable record.
pub fn new_unreachable_record(hit_count: u64, runs_hit: u32) -> (result: OracleRecord)
    requires
        (runs_hit == 0) <==> (hit_count == 0),
        runs_hit as u64 <= hit_count,
    ensures valid_oracle_record_spec(result)
{
    OracleRecord {
        kind: AssertionKind::Unreachable,
        hit_count,
        true_count: 0,
        false_count: hit_count,
        runs_hit,
        runs_satisfied: 0,
    }
}

// r[impl chaoscontrol-oracle.accounting]
/// Validates an existing record.
pub fn valid_oracle_record(r: &OracleRecord) -> (result: bool)
    ensures result == valid_oracle_record_spec(*r)
{
    let shared = r.runs_satisfied <= r.runs_hit
        && ((r.runs_hit == 0) == (r.hit_count == 0));
    if !shared { return false; }
    match r.kind {
        AssertionKind::Always => {
            r.true_count <= r.hit_count
            && r.false_count == r.hit_count - r.true_count
            && r.runs_hit as u64 <= r.hit_count
        }
        AssertionKind::Sometimes => {
            r.true_count <= r.hit_count
            && r.false_count == r.hit_count - r.true_count
            && r.runs_hit as u64 <= r.hit_count
            && r.runs_satisfied as u64 <= r.true_count
        }
        AssertionKind::Reachable => {
            r.true_count == r.hit_count
            && r.false_count == 0
            && r.runs_satisfied == r.runs_hit
        }
        AssertionKind::Unreachable => {
            r.true_count == 0
            && r.false_count == r.hit_count
            && r.runs_satisfied == 0
        }
    }
}

// r[impl chaoscontrol-oracle.verdict]
/// Compute the verdict for a valid record.
pub fn compute_verdict(r: &OracleRecord) -> (result: Verdict)
    requires valid_oracle_record_spec(*r)
    ensures result == verdict_spec(*r)
{
    match r.kind {
        AssertionKind::Always => {
            if r.hit_count > 0 && r.false_count == 0 { Verdict::Passed } else { Verdict::Failed }
        }
        AssertionKind::Sometimes => {
            if r.hit_count > 0 && r.true_count > 0 { Verdict::Passed } else { Verdict::Failed }
        }
        AssertionKind::Reachable => {
            if r.hit_count > 0 { Verdict::Passed } else { Verdict::Failed }
        }
        AssertionKind::Unreachable => {
            if r.hit_count == 0 { Verdict::Passed } else { Verdict::Failed }
        }
    }
}

} // verus!

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_always_pass() {
        let r = new_boolean_always_record(5, 5, 0, 2, 2);
        assert!(valid_oracle_record(&r));
        assert_eq!(compute_verdict(&r), Verdict::Passed);
    }

    #[test]
    fn test_always_fail_on_false() {
        let r = new_boolean_always_record(5, 4, 1, 2, 1);
        assert!(valid_oracle_record(&r));
        assert_eq!(compute_verdict(&r), Verdict::Failed);
    }

    #[test]
    fn test_always_fail_unhit() {
        let r = new_boolean_always_record(0, 0, 0, 0, 0);
        assert!(valid_oracle_record(&r));
        assert_eq!(compute_verdict(&r), Verdict::Failed);
    }

    #[test]
    fn test_sometimes_pass() {
        let r = new_boolean_sometimes_record(5, 1, 4, 3, 1);
        assert!(valid_oracle_record(&r));
        assert_eq!(compute_verdict(&r), Verdict::Passed);
    }

    #[test]
    fn test_sometimes_fail_no_true() {
        let r = new_boolean_sometimes_record(5, 0, 5, 3, 0);
        assert!(valid_oracle_record(&r));
        assert_eq!(compute_verdict(&r), Verdict::Failed);
    }

    #[test]
    fn test_reachable_pass() {
        let r = new_reachable_record(3, 2);
        assert!(valid_oracle_record(&r));
        assert_eq!(compute_verdict(&r), Verdict::Passed);
    }

    #[test]
    fn test_reachable_fail() {
        let r = new_reachable_record(0, 0);
        assert!(valid_oracle_record(&r));
        assert_eq!(compute_verdict(&r), Verdict::Failed);
    }

    #[test]
    fn test_unreachable_pass() {
        let r = new_unreachable_record(0, 0);
        assert!(valid_oracle_record(&r));
        assert_eq!(compute_verdict(&r), Verdict::Passed);
    }

    #[test]
    fn test_unreachable_fail() {
        let r = new_unreachable_record(3, 2);
        assert!(valid_oracle_record(&r));
        assert_eq!(compute_verdict(&r), Verdict::Failed);
    }

    #[test]
    fn test_invalid_record_rejected() {
        // Broken boolean: hit_count != true + false
        let r = OracleRecord {
            kind: AssertionKind::Always,
            hit_count: 5,
            true_count: 2,
            false_count: 2,
            runs_hit: 1,
            runs_satisfied: 1,
        };
        assert!(!valid_oracle_record(&r));
    }

    #[test]
    fn test_invalid_reachable_shape_rejected() {
        // Reachable with false_count > 0
        let r = OracleRecord {
            kind: AssertionKind::Reachable,
            hit_count: 3,
            true_count: 2,
            false_count: 1,
            runs_hit: 1,
            runs_satisfied: 1,
        };
        assert!(!valid_oracle_record(&r));
    }

    #[test]
    fn test_invalid_unreachable_shape_rejected() {
        // Unreachable hit but runs_satisfied > 0
        let r = OracleRecord {
            kind: AssertionKind::Unreachable,
            hit_count: 1,
            true_count: 0,
            false_count: 1,
            runs_hit: 1,
            runs_satisfied: 1,
        };
        assert!(!valid_oracle_record(&r));
    }
}
