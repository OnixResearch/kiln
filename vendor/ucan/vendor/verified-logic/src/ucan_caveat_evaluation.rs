//! Verified UCAN caveat evaluation transcript admission.
//!
//! This module proves only finite fail-closed aggregation over caller- or
//! shell-supplied caveat evaluation facts. Policy lookup truth, arbitrary policy
//! correctness, typed payload parsing, clocks, counters, freshness, token
//! provenance, and cryptography remain runtime/caller boundaries.

use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaveatEvaluationFact {
    pub policy_present: bool,
    pub satisfied: bool,
}

pub open spec fn caveat_fact_accepts_spec(fact: CaveatEvaluationFact) -> bool {
    fact.policy_present && fact.satisfied
}

pub open spec fn caveat_evaluation_transcript_accepts_spec(facts: Seq<CaveatEvaluationFact>) -> bool {
    forall|i: int| #![trigger facts[i]] 0 <= i < facts.len() ==> caveat_fact_accepts_spec(facts[i])
}

pub open spec fn first_failed_caveat_index_spec(facts: Seq<CaveatEvaluationFact>) -> int {
    if caveat_evaluation_transcript_accepts_spec(facts) {
        facts.len() as int
    } else {
        choose|i: int| 0 <= i < facts.len()
            && !caveat_fact_accepts_spec(facts[i])
            && (forall|j: int| #![trigger facts[j]] 0 <= j < i ==> caveat_fact_accepts_spec(facts[j]))
    }
}

// r[impl ucan-caveat-evaluation.fact-accepts]
pub fn caveat_fact_accepts(fact: CaveatEvaluationFact) -> (result: bool)
    ensures result == caveat_fact_accepts_spec(fact),
{
    fact.policy_present && fact.satisfied
}

// r[impl ucan-caveat-evaluation.transcript-accepts]
pub fn caveat_evaluation_transcript_accepts(facts: &Vec<CaveatEvaluationFact>) -> (result: bool)
    ensures result == caveat_evaluation_transcript_accepts_spec(facts@),
{
    let mut i: usize = 0;
    while i < facts.len()
        invariant
            0 <= i <= facts.len(),
            forall|j: int| #![trigger facts@[j]] 0 <= j < i ==> caveat_fact_accepts_spec(facts@[j]),
        decreases facts.len() - i,
    {
        if !caveat_fact_accepts(facts[i]) {
            assert(!caveat_fact_accepts_spec(facts@[i as int]));
            assert(!caveat_evaluation_transcript_accepts_spec(facts@));
            return false;
        }
        assert(caveat_fact_accepts_spec(facts@[i as int]));
        assert forall|j: int| #![trigger facts@[j]] 0 <= j < i + 1 implies caveat_fact_accepts_spec(facts@[j]) by {
            if j < i {
                assert(facts@[j] == facts@[j]);
            } else {
                assert(j == i);
            }
        }
        i = i + 1;
        assert forall|j: int| #![trigger facts@[j]] 0 <= j < i implies caveat_fact_accepts_spec(facts@[j]) by {
            assert(facts@[j] == facts@[j]);
        };
    }
    true
}

// r[impl ucan-caveat-evaluation.first-failed-index]
pub fn first_failed_caveat_index(facts: &Vec<CaveatEvaluationFact>) -> (result: usize)
    ensures
        0 <= result <= facts.len(),
        result as int == first_failed_caveat_index_spec(facts@),
{
    let mut i: usize = 0;
    while i < facts.len()
        invariant
            0 <= i <= facts.len(),
            forall|j: int| #![trigger facts@[j]] 0 <= j < i ==> caveat_fact_accepts_spec(facts@[j]),
        decreases facts.len() - i,
    {
        if !caveat_fact_accepts(facts[i]) {
            assert(!caveat_fact_accepts_spec(facts@[i as int]));
            assert forall|j: int| #![trigger facts@[j]] 0 <= j < i implies caveat_fact_accepts_spec(facts@[j]) by {
                assert(facts@[j] == facts@[j]);
            };
            assert(first_failed_caveat_index_spec(facts@) == i as int);
            return i;
        }
        assert(caveat_fact_accepts_spec(facts@[i as int]));
        i = i + 1;
    }
    assert(caveat_evaluation_transcript_accepts_spec(facts@));
    assert(first_failed_caveat_index_spec(facts@) == facts@.len() as int);
    facts.len()
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify ucan-caveat-evaluation.fact-accepts]
    // r[verify ucan-caveat-evaluation.transcript-accepts]
    #[test]
    fn accepts_empty_and_all_satisfied_transcripts() {
        assert!(caveat_evaluation_transcript_accepts(&Vec::new()));
        let facts = vec![
            CaveatEvaluationFact {
                policy_present: true,
                satisfied: true,
            },
            CaveatEvaluationFact {
                policy_present: true,
                satisfied: true,
            },
        ];
        assert!(caveat_evaluation_transcript_accepts(&facts));
        assert_eq!(first_failed_caveat_index(&facts), facts.len());
    }

    // r[verify ucan-caveat-evaluation.transcript-accepts]
    // r[verify ucan-caveat-evaluation.first-failed-index]
    #[test]
    fn rejects_missing_policy_at_first_failure() {
        let facts = vec![
            CaveatEvaluationFact {
                policy_present: true,
                satisfied: true,
            },
            CaveatEvaluationFact {
                policy_present: false,
                satisfied: false,
            },
            CaveatEvaluationFact {
                policy_present: true,
                satisfied: false,
            },
        ];
        assert!(!caveat_evaluation_transcript_accepts(&facts));
        assert_eq!(first_failed_caveat_index(&facts), 1);
    }

    // r[verify ucan-caveat-evaluation.transcript-accepts]
    // r[verify ucan-caveat-evaluation.first-failed-index]
    #[test]
    fn rejects_unsatisfied_policy_at_first_failure() {
        let facts = vec![
            CaveatEvaluationFact {
                policy_present: true,
                satisfied: true,
            },
            CaveatEvaluationFact {
                policy_present: true,
                satisfied: false,
            },
        ];
        assert!(!caveat_evaluation_transcript_accepts(&facts));
        assert_eq!(first_failed_caveat_index(&facts), 1);
    }
}
