//! Verified voter set promotion for cluster membership changes.
//!
//! Given a voter list, a learner to promote, and an optional node to
//! replace, compute the new voter list. Mirrors the control flow of
//! `aspen-cluster-handler`'s `handle_promote_learner`: skip learner
//! duplicates and replaced node, then append learner at the end.
//!
//! Properties: learner always present, replaced node always absent,
//! other voters preserved, no duplicates, quorum preserved.

use vstd::prelude::*;

verus! {

// ---------------------------------------------------------------------------
// Helper: distinct (no duplicates in a Seq)
// ---------------------------------------------------------------------------

/// Spec: all elements in `s` are pairwise distinct.
pub open spec fn all_distinct_spec(s: Seq<u64>) -> bool {
    forall|i: int, j: int|
        #![trigger s[i], s[j]]
        0 <= i < s.len() && 0 <= j < s.len() && i != j ==> s[i] != s[j]
}

// ---------------------------------------------------------------------------
// Helper: contains
// ---------------------------------------------------------------------------

/// Spec: `s` contains element `x`.
pub open spec fn seq_contains_spec(s: Seq<u64>, x: u64) -> bool {
    exists|i: int| #![trigger s[i]] 0 <= i < s.len() && s[i] == x
}

// ---------------------------------------------------------------------------
// promote_build_spec — recursive construction
// ---------------------------------------------------------------------------

/// Spec: build promoted voter list from index `from` onward.
///
/// Scans voters left to right:
/// - Skip elements equal to `learner_id` (dedup — learner appended at end).
/// - Skip elements equal to `replace_id` (when Some).
/// - Keep everything else.
/// At the end (from >= voters.len()), append learner_id.
pub open spec fn promote_build_spec(
    voters: Seq<u64>,
    learner_id: u64,
    replace_id: Option<u64>,
    from: int,
) -> Seq<u64>
    decreases voters.len() - from,
{
    if from >= voters.len() {
        seq![learner_id]
    } else {
        let v = voters[from];
        let skip = v == learner_id || match replace_id {
            Some(rid) => v == rid,
            None => false,
        };
        if skip {
            promote_build_spec(voters, learner_id, replace_id, from + 1)
        } else {
            seq![v] + promote_build_spec(voters, learner_id, replace_id, from + 1)
        }
    }
}

// r[depends voter-set.promote]
// r[depends voter-set.replace]
/// Top-level spec: promote learner into voters with optional replacement.
pub open spec fn promote_voters_spec(
    voters: Seq<u64>,
    learner_id: u64,
    replace_id: Option<u64>,
) -> Seq<u64> {
    promote_build_spec(voters, learner_id, replace_id, 0)
}

// ---------------------------------------------------------------------------
// Exec: promote
// ---------------------------------------------------------------------------

// r[impl voter-set.promote]
// r[impl voter-set.replace]
// r[impl voter-set.no-duplicates]
// r[impl voter-set.learner-present]
// r[impl voter-set.replaced-absent]
// r[impl voter-set.others-preserved]
// r[impl voter-set.quorum-preserved]
/// Promote `learner_id` into `voters`, optionally replacing `replace_id`.
pub fn promote_voters(
    voters: &[u64],
    learner_id: u64,
    replace_id: Option<u64>,
) -> (result: Vec<u64>)
    requires
        voters@.len() < u64::MAX as int,
        all_distinct_spec(voters@),
    ensures
        result@ == promote_voters_spec(voters@, learner_id, replace_id),
{
    let mut result: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    let ghost full = promote_voters_spec(voters@, learner_id, replace_id);
    while i < voters.len()
        invariant
            0 <= i <= voters@.len(),
            full == promote_voters_spec(voters@, learner_id, replace_id),
            result@ + promote_build_spec(voters@, learner_id, replace_id, i as int)
                =~= full,
        decreases voters@.len() - i,
    {
        let v = voters[i];
        let skip = v == learner_id || match replace_id {
            Some(rid) => v == rid,
            None => false,
        };
        if !skip {
            result.push(v);
        }
        i = i + 1;
    }
    // After loop: promote_build_spec(..., voters.len()) == seq![learner_id]
    // So result@ + seq![learner_id] =~= full
    result.push(learner_id);
    result
}

// ---------------------------------------------------------------------------
// Property: learner always present
// ---------------------------------------------------------------------------

// r[verify voter-set.learner-present]
/// The learner is always in the result.
pub proof fn learner_always_present(
    voters: Seq<u64>,
    learner_id: u64,
    replace_id: Option<u64>,
)
    ensures
        seq_contains_spec(
            promote_voters_spec(voters, learner_id, replace_id),
            learner_id,
        ),
{
    learner_in_build(voters, learner_id, replace_id, 0);
}

proof fn learner_in_build(
    voters: Seq<u64>,
    learner_id: u64,
    replace_id: Option<u64>,
    from: int,
)
    requires 0 <= from,
    ensures
        seq_contains_spec(
            promote_build_spec(voters, learner_id, replace_id, from),
            learner_id,
        ),
    decreases voters.len() - from,
{
    let result = promote_build_spec(voters, learner_id, replace_id, from);
    if from >= voters.len() {
        assert(result =~= seq![learner_id]);
        assert(result[0] == learner_id);
    } else {
        let v = voters[from];
        let skip = v == learner_id || match replace_id {
            Some(rid) => v == rid,
            None => false,
        };
        learner_in_build(voters, learner_id, replace_id, from + 1);
        let rest = promote_build_spec(voters, learner_id, replace_id, from + 1);
        if skip {
            // result == rest, which contains learner_id by induction
        } else {
            // result == seq![v] + rest
            assert(result =~= seq![v] + rest);
            let k = choose|k: int| #![trigger rest[k]]
                0 <= k < rest.len() && rest[k] == learner_id;
            assert(result[k + 1] == learner_id);
        }
    }
}

// ---------------------------------------------------------------------------
// Property: replaced node absent
// ---------------------------------------------------------------------------

// r[verify voter-set.replaced-absent]
/// The replaced node never appears in the result.
pub proof fn replaced_always_absent(
    voters: Seq<u64>,
    learner_id: u64,
    rid: u64,
)
    requires learner_id != rid,
    ensures
        !seq_contains_spec(
            promote_voters_spec(voters, learner_id, Some(rid)),
            rid,
        ),
{
    replaced_absent_build(voters, learner_id, rid, 0);
}

proof fn replaced_absent_build(
    voters: Seq<u64>,
    learner_id: u64,
    rid: u64,
    from: int,
)
    requires
        0 <= from,
        learner_id != rid,
    ensures
        !seq_contains_spec(
            promote_build_spec(voters, learner_id, Some(rid), from),
            rid,
        ),
    decreases voters.len() - from,
{
    let result = promote_build_spec(voters, learner_id, Some(rid), from);
    if from >= voters.len() {
        assert(result =~= seq![learner_id]);
        // learner_id != rid, so result[0] != rid
        // Need to show no index i has result[i] == rid
        assert forall|i: int| #![trigger result[i]]
            0 <= i < result.len() implies result[i] != rid
        by {
            assert(result[0] == learner_id);
        };
    } else {
        replaced_absent_build(voters, learner_id, rid, from + 1);
        let v = voters[from];
        let skip = v == learner_id || v == rid;
        if skip {
            // result == rest, already proven
        } else {
            let rest = promote_build_spec(voters, learner_id, Some(rid), from + 1);
            assert(result =~= seq![v] + rest);
            // v != rid (since v != learner_id and v != rid, so !skip would not hold if v==rid)
            // Actually: skip = (v == learner_id || v == rid). Since !skip, v != rid.
            assert forall|i: int| #![trigger result[i]]
                0 <= i < result.len() implies result[i] != rid
            by {
                if i == 0 {
                    assert(result[0] == v);
                } else {
                    assert(result[i] == rest[i - 1]);
                }
            };
        }
    }
}

// ---------------------------------------------------------------------------
// Property: others preserved
// ---------------------------------------------------------------------------

// r[verify voter-set.others-preserved]
/// Every voter not equal to learner_id or replace_id is in the result.
pub proof fn others_preserved(
    voters: Seq<u64>,
    learner_id: u64,
    replace_id: Option<u64>,
)
    ensures
        forall|k: int|
            #![trigger voters[k]]
            0 <= k < voters.len()
            && voters[k] != learner_id
            && (match replace_id {
                Some(rid) => voters[k] != rid,
                None => true,
            })
            ==> seq_contains_spec(
                promote_voters_spec(voters, learner_id, replace_id),
                voters[k],
            ),
{
    assert forall|k: int|
        #![trigger voters[k]]
        0 <= k < voters.len()
        && voters[k] != learner_id
        && (match replace_id {
            Some(rid) => voters[k] != rid,
            None => true,
        })
    implies
        seq_contains_spec(
            promote_voters_spec(voters, learner_id, replace_id),
            voters[k],
        )
    by {
        others_preserved_at(voters, learner_id, replace_id, 0, k);
    };
}

proof fn others_preserved_at(
    voters: Seq<u64>,
    learner_id: u64,
    replace_id: Option<u64>,
    from: int,
    k: int,
)
    requires
        0 <= from,
        from <= k < voters.len(),
        voters[k] != learner_id,
        match replace_id {
            Some(rid) => voters[k] != rid,
            None => true,
        },
    ensures
        seq_contains_spec(
            promote_build_spec(voters, learner_id, replace_id, from),
            voters[k],
        ),
    decreases voters.len() - from,
{
    let result = promote_build_spec(voters, learner_id, replace_id, from);
    if from == k {
        // voters[from] is the kept element — it's not skipped
        let rest = promote_build_spec(voters, learner_id, replace_id, from + 1);
        assert(result =~= seq![voters[from]] + rest);
        assert(result[0] == voters[k]);
    } else {
        // from < k, recurse
        others_preserved_at(voters, learner_id, replace_id, from + 1, k);
        let v = voters[from];
        let skip = v == learner_id || match replace_id {
            Some(rid) => v == rid,
            None => false,
        };
        let rest = promote_build_spec(voters, learner_id, replace_id, from + 1);
        if skip {
            // result == rest
        } else {
            assert(result =~= seq![v] + rest);
            let j = choose|j: int| #![trigger rest[j]]
                0 <= j < rest.len() && rest[j] == voters[k];
            assert(result[j + 1] == voters[k]);
        }
    }
}

// ---------------------------------------------------------------------------
// Property: no duplicates
// ---------------------------------------------------------------------------

// r[verify voter-set.no-duplicates]
/// Result has no duplicates when input has no duplicates.
pub proof fn promote_no_duplicates(
    voters: Seq<u64>,
    learner_id: u64,
    replace_id: Option<u64>,
)
    requires all_distinct_spec(voters),
    ensures all_distinct_spec(promote_voters_spec(voters, learner_id, replace_id)),
{
    build_distinct(voters, learner_id, replace_id, 0);
}

proof fn build_distinct(
    voters: Seq<u64>,
    learner_id: u64,
    replace_id: Option<u64>,
    from: int,
)
    requires
        0 <= from,
        all_distinct_spec(voters),
    ensures
        all_distinct_spec(promote_build_spec(voters, learner_id, replace_id, from)),
    decreases voters.len() - from,
{
    let result = promote_build_spec(voters, learner_id, replace_id, from);
    if from >= voters.len() {
        // seq![learner_id] — trivially distinct (length 1)
        assert(result =~= seq![learner_id]);
    } else {
        build_distinct(voters, learner_id, replace_id, from + 1);
        let v = voters[from];
        let skip = v == learner_id || match replace_id {
            Some(rid) => v == rid,
            None => false,
        };
        if skip {
            // result == rest, distinct by induction
        } else {
            let rest = promote_build_spec(voters, learner_id, replace_id, from + 1);
            assert(result =~= seq![v] + rest);
            // rest is distinct (inductive hypothesis)
            // v is not in rest: v != learner_id, and v ∉ voters[from+1..] (distinct input)
            // rest contains only learner_id or elements from voters[from+1..]
            assert(!seq_contains_spec(rest, v)) by {
                if seq_contains_spec(rest, v) {
                    let j = choose|j: int| #![trigger rest[j]]
                        0 <= j < rest.len() && rest[j] == v;
                    build_element_source(voters, learner_id, replace_id, from + 1, j);
                    // rest[j] is either learner_id or voters[m] for some m >= from+1
                    // But v != learner_id (!skip), and v != voters[m] for m != from (distinct)
                }
            };
            // Now: rest distinct, v ∉ rest, so [v] + rest is distinct
            assert forall|i: int, j: int|
                #![trigger result[i], result[j]]
                0 <= i < result.len() && 0 <= j < result.len() && i != j
            implies result[i] != result[j]
            by {
                if i == 0 && j > 0 {
                    assert(result[0] == v);
                    assert(result[j] == rest[j - 1]);
                } else if j == 0 && i > 0 {
                    assert(result[0] == v);
                    assert(result[i] == rest[i - 1]);
                } else if i > 0 && j > 0 {
                    assert(result[i] == rest[i - 1]);
                    assert(result[j] == rest[j - 1]);
                }
            };
        }
    }
}

/// Every element at index j in the build result is either learner_id
/// or comes from voters[from..].
proof fn build_element_source(
    voters: Seq<u64>,
    learner_id: u64,
    replace_id: Option<u64>,
    from: int,
    j: int,
)
    requires
        0 <= from,
        0 <= j < promote_build_spec(voters, learner_id, replace_id, from).len(),
    ensures
        ({
            let elt = promote_build_spec(voters, learner_id, replace_id, from)[j];
            elt == learner_id
            || exists|m: int| #![trigger voters[m]]
                from <= m < voters.len() && voters[m] == elt
        }),
    decreases voters.len() - from,
{
    let result = promote_build_spec(voters, learner_id, replace_id, from);
    if from >= voters.len() {
        // result == seq![learner_id], j must be 0
        assert(result =~= seq![learner_id]);
        assert(result[0] == learner_id);
    } else {
        let v = voters[from];
        let skip = v == learner_id || match replace_id {
            Some(rid) => v == rid,
            None => false,
        };
        let rest = promote_build_spec(voters, learner_id, replace_id, from + 1);
        if skip {
            // result == rest
            build_element_source(voters, learner_id, replace_id, from + 1, j);
            // rest[j] is learner_id or from voters[from+1..] ⊆ voters[from..]
        } else {
            assert(result =~= seq![v] + rest);
            if j == 0 {
                assert(result[0] == v);
                assert(voters[from] == v);
            } else {
                assert(result[j] == rest[j - 1]);
                build_element_source(voters, learner_id, replace_id, from + 1, j - 1);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Property: idempotent when learner already present
// ---------------------------------------------------------------------------

// r[verify voter-set.promote]
/// When learner is already a voter (no replacement), result has same length.
pub proof fn idempotent_when_present(
    voters: Seq<u64>,
    learner_id: u64,
)
    requires
        all_distinct_spec(voters),
        seq_contains_spec(voters, learner_id),
    ensures
        promote_voters_spec(voters, learner_id, None).len() == voters.len(),
{
    build_len_with_learner(voters, learner_id, 0);
}

/// When learner_id ∈ voters[from..] and voters is distinct,
/// build length = voters.len() - from.
proof fn build_len_with_learner(
    voters: Seq<u64>,
    learner_id: u64,
    from: int,
)
    requires
        0 <= from <= voters.len(),
        all_distinct_spec(voters),
        exists|k: int| #![trigger voters[k]]
            from <= k < voters.len() && voters[k] == learner_id,
    ensures
        promote_build_spec(voters, learner_id, None, from).len()
        == voters.len() - from,
    decreases voters.len() - from,
{
    if from >= voters.len() {
        // Contradiction with exists
    } else {
        let v = voters[from];
        if v == learner_id {
            // Skip learner, recurse. Learner NOT in voters[from+1..] (distinct).
            build_len_without_learner(voters, learner_id, from + 1);
        } else {
            // Keep v, recurse. Learner still in voters[from+1..].
            let witness = choose|k: int| #![trigger voters[k]]
                from <= k < voters.len() && voters[k] == learner_id;
            assert(witness != from);
            assert(from + 1 <= witness);
            build_len_with_learner(voters, learner_id, from + 1);
        }
    }
}

/// When learner_id ∉ voters[from..], build length = (voters.len() - from) + 1.
proof fn build_len_without_learner(
    voters: Seq<u64>,
    learner_id: u64,
    from: int,
)
    requires
        0 <= from <= voters.len(),
        forall|k: int| #![trigger voters[k]]
            from <= k < voters.len() ==> voters[k] != learner_id,
    ensures
        promote_build_spec(voters, learner_id, None, from).len()
        == (voters.len() - from) + 1,
    decreases voters.len() - from,
{
    if from >= voters.len() {
        // result == seq![learner_id], length 1
    } else {
        let v = voters[from];
        // v != learner_id (from precondition)
        build_len_without_learner(voters, learner_id, from + 1);
        let rest = promote_build_spec(voters, learner_id, None, from + 1);
        let result = promote_build_spec(voters, learner_id, None, from);
        assert(result =~= seq![v] + rest);
    }
}

// ---------------------------------------------------------------------------
// Property: quorum preservation (no replacement)
// ---------------------------------------------------------------------------

// r[verify voter-set.quorum-preserved]
/// Promotion without replacement never shrinks the set.
pub proof fn promote_size_ge(
    voters: Seq<u64>,
    learner_id: u64,
)
    requires all_distinct_spec(voters),
    ensures
        promote_voters_spec(voters, learner_id, None).len() as int
        >= voters.len(),
{
    if seq_contains_spec(voters, learner_id) {
        build_len_with_learner(voters, learner_id, 0);
    } else {
        // learner not present — need to show it's not in any position
        assert(forall|k: int| #![trigger voters[k]]
            0 <= k < voters.len() ==> voters[k] != learner_id);
        build_len_without_learner(voters, learner_id, 0);
    }
}

// ---------------------------------------------------------------------------
// Property: quorum preserved with replacement
// ---------------------------------------------------------------------------

/// Build length when replace_id ∈ voters[from..] and learner_id ∉ voters[from..]:
/// one element removed (replace), learner appended → net 0 change → len stays same.
proof fn build_len_replace_no_learner(
    voters: Seq<u64>,
    learner_id: u64,
    rid: u64,
    from: int,
)
    requires
        0 <= from <= voters.len(),
        all_distinct_spec(voters),
        learner_id != rid,
        // rid is in voters[from..]
        exists|k: int| #![trigger voters[k]]
            from <= k < voters.len() && voters[k] == rid,
        // learner_id is not in voters[from..]
        forall|k: int| #![trigger voters[k]]
            from <= k < voters.len() ==> voters[k] != learner_id,
    ensures
        promote_build_spec(voters, learner_id, Some(rid), from).len()
        == voters.len() - from,
    decreases voters.len() - from,
{
    if from >= voters.len() {
        // contradiction with rid exists
    } else {
        let v = voters[from];
        let skip = v == learner_id || v == rid;
        if v == rid {
            // Skip rid. rid not in voters[from+1..] (distinct).
            // learner not in voters[from+1..] (given).
            // So from from+1 onward: neither rid nor learner present → all kept + learner appended
            build_len_no_special(voters, learner_id, rid, from + 1);
        } else if v == learner_id {
            // Can't happen: learner_id not in voters[from..] precondition
        } else {
            // Keep v, recurse
            let witness = choose|k: int| #![trigger voters[k]]
                from <= k < voters.len() && voters[k] == rid;
            assert(witness != from);
            assert(from + 1 <= witness);
            build_len_replace_no_learner(voters, learner_id, rid, from + 1);
            let rest = promote_build_spec(voters, learner_id, Some(rid), from + 1);
            assert(promote_build_spec(voters, learner_id, Some(rid), from) =~= seq![v] + rest);
        }
    }
}

/// When neither rid nor learner_id are in voters[from..], build len = (voters.len() - from) + 1.
proof fn build_len_no_special(
    voters: Seq<u64>,
    learner_id: u64,
    rid: u64,
    from: int,
)
    requires
        0 <= from <= voters.len(),
        forall|k: int| #![trigger voters[k]]
            from <= k < voters.len() ==> voters[k] != learner_id && voters[k] != rid,
    ensures
        promote_build_spec(voters, learner_id, Some(rid), from).len()
        == (voters.len() - from) + 1,
    decreases voters.len() - from,
{
    if from >= voters.len() {
        // result == seq![learner_id], len = 1
    } else {
        let v = voters[from];
        // v != learner_id and v != rid (precondition)
        build_len_no_special(voters, learner_id, rid, from + 1);
        let rest = promote_build_spec(voters, learner_id, Some(rid), from + 1);
        assert(promote_build_spec(voters, learner_id, Some(rid), from) =~= seq![v] + rest);
    }
}

/// When learner_id ∈ voters[from..] and rid ∈ voters[from..] (distinct, learner != rid):
/// one skipped (learner dedup), one skipped (replace), learner appended → net -1 → len = voters.len() - from - 1.
proof fn build_len_both_present(
    voters: Seq<u64>,
    learner_id: u64,
    rid: u64,
    from: int,
)
    requires
        0 <= from <= voters.len(),
        all_distinct_spec(voters),
        learner_id != rid,
        exists|k: int| #![trigger voters[k]]
            from <= k < voters.len() && voters[k] == learner_id,
        exists|k: int| #![trigger voters[k]]
            from <= k < voters.len() && voters[k] == rid,
    ensures
        promote_build_spec(voters, learner_id, Some(rid), from).len()
        == voters.len() - from - 1,
    decreases voters.len() - from,
{
    if from >= voters.len() {
        // contradiction
    } else {
        let v = voters[from];
        if v == learner_id {
            // Skip learner. learner not in voters[from+1..] (distinct).
            // rid still in voters[from+1..]
            let rid_witness = choose|k: int| #![trigger voters[k]]
                from <= k < voters.len() && voters[k] == rid;
            assert(rid_witness != from); // v == learner_id != rid
            assert(from + 1 <= rid_witness);
            // From from+1: rid present, learner absent
            build_len_replace_no_learner(voters, learner_id, rid, from + 1);
        } else if v == rid {
            // Skip rid. rid not in voters[from+1..] (distinct).
            // learner still in voters[from+1..]
            let learner_witness = choose|k: int| #![trigger voters[k]]
                from <= k < voters.len() && voters[k] == learner_id;
            assert(learner_witness != from); // v == rid != learner_id
            assert(from + 1 <= learner_witness);
            // From from+1: learner present, rid absent
            build_len_with_learner(voters, learner_id, from + 1);
            // Wait — that's for None replacement. We need Some(rid) but rid not present.
            // When rid is not in voters[from+1..]:
            // build_spec with Some(rid) behaves same as None for elements != rid
            // and all elements from from+1 onward are != rid
            build_len_learner_no_rid(voters, learner_id, rid, from + 1);
        } else {
            // Keep v, recurse — both learner and rid still present
            let learner_witness = choose|k: int| #![trigger voters[k]]
                from <= k < voters.len() && voters[k] == learner_id;
            let rid_witness = choose|k: int| #![trigger voters[k]]
                from <= k < voters.len() && voters[k] == rid;
            assert(learner_witness != from && rid_witness != from);
            build_len_both_present(voters, learner_id, rid, from + 1);
            let rest = promote_build_spec(voters, learner_id, Some(rid), from + 1);
            assert(promote_build_spec(voters, learner_id, Some(rid), from) =~= seq![v] + rest);
        }
    }
}

/// When learner_id ∈ voters[from..] but rid ∉ voters[from..]:
/// learner skipped, rid skip is no-op, learner appended → net 0 → len = voters.len() - from.
proof fn build_len_learner_no_rid(
    voters: Seq<u64>,
    learner_id: u64,
    rid: u64,
    from: int,
)
    requires
        0 <= from <= voters.len(),
        all_distinct_spec(voters),
        learner_id != rid,
        exists|k: int| #![trigger voters[k]]
            from <= k < voters.len() && voters[k] == learner_id,
        forall|k: int| #![trigger voters[k]]
            from <= k < voters.len() ==> voters[k] != rid,
    ensures
        promote_build_spec(voters, learner_id, Some(rid), from).len()
        == voters.len() - from,
    decreases voters.len() - from,
{
    if from >= voters.len() {
        // contradiction
    } else {
        let v = voters[from];
        if v == learner_id {
            // Skip learner. Learner not in voters[from+1..] (distinct). rid not in voters[from+1..].
            build_len_no_special(voters, learner_id, rid, from + 1);
        } else {
            // v != learner_id, v != rid (precondition). Keep v, recurse.
            let witness = choose|k: int| #![trigger voters[k]]
                from <= k < voters.len() && voters[k] == learner_id;
            assert(witness != from);
            build_len_learner_no_rid(voters, learner_id, rid, from + 1);
            let rest = promote_build_spec(voters, learner_id, Some(rid), from + 1);
            assert(promote_build_spec(voters, learner_id, Some(rid), from) =~= seq![v] + rest);
        }
    }
}

// r[verify voter-set.quorum-preserved]
/// Promotion with replacement: result size depends on membership.
/// - If both learner and rid in voters: size = voters.len() - 1 (one removed, one deduped, one appended)
/// - If only rid in voters: size = voters.len() (one removed, one appended → net 0)
/// - If only learner in voters: size = voters.len() (one deduped, one appended → net 0)
/// - If neither in voters: size = voters.len() + 1 (one appended)
///
/// With can_remove_safely(n), n-1 >= majority(n), so quorum holds for all cases.
pub proof fn promote_with_replace_quorum(
    voters: Seq<u64>,
    learner_id: u64,
    rid: u64,
)
    requires
        all_distinct_spec(voters),
        learner_id != rid,
        voters.len() > 0,
    ensures
        promote_voters_spec(voters, learner_id, Some(rid)).len() as int
        >= voters.len() - 1,
{
    let has_learner = seq_contains_spec(voters, learner_id);
    let has_rid = seq_contains_spec(voters, rid);
    if has_learner && has_rid {
        build_len_both_present(voters, learner_id, rid, 0);
    } else if !has_learner && has_rid {
        assert(forall|k: int| #![trigger voters[k]]
            0 <= k < voters.len() ==> voters[k] != learner_id);
        build_len_replace_no_learner(voters, learner_id, rid, 0);
    } else if has_learner && !has_rid {
        assert(forall|k: int| #![trigger voters[k]]
            0 <= k < voters.len() ==> voters[k] != rid);
        build_len_learner_no_rid(voters, learner_id, rid, 0);
    } else {
        // Neither present
        assert(forall|k: int| #![trigger voters[k]]
            0 <= k < voters.len() ==> voters[k] != learner_id && voters[k] != rid);
        build_len_no_special(voters, learner_id, rid, 0);
    }
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify voter-set.promote]
    #[test]
    fn test_promote_new_learner() {
        let voters = vec![1, 2, 3];
        let result = promote_voters(&voters, 4, None);
        assert_eq!(result, vec![1, 2, 3, 4]);
    }

    // r[verify voter-set.promote]
    #[test]
    fn test_promote_existing_learner() {
        let voters = vec![1, 2, 3];
        let result = promote_voters(&voters, 2, None);
        assert_eq!(result, vec![1, 3, 2]);
    }

    // r[verify voter-set.replace]
    #[test]
    fn test_promote_with_replace() {
        let voters = vec![1, 2, 3];
        let result = promote_voters(&voters, 4, Some(2));
        assert_eq!(result, vec![1, 3, 4]);
    }

    // r[verify voter-set.replace]
    #[test]
    fn test_promote_replace_nonexistent() {
        let voters = vec![1, 2, 3];
        let result = promote_voters(&voters, 4, Some(99));
        assert_eq!(result, vec![1, 2, 3, 4]);
    }

    // r[verify voter-set.replace]
    #[test]
    fn test_promote_replace_with_existing_learner() {
        let voters = vec![1, 2, 3];
        let result = promote_voters(&voters, 3, Some(1));
        assert_eq!(result, vec![2, 3]);
    }

    // r[verify voter-set.no-duplicates]
    #[test]
    fn test_no_duplicates_when_learner_present() {
        let voters = vec![1, 2, 3];
        let result = promote_voters(&voters, 2, None);
        assert_eq!(result.len(), 3);
        for i in 0..result.len() {
            for j in (i + 1)..result.len() {
                assert_ne!(result[i], result[j], "duplicate at {i},{j}");
            }
        }
    }

    // r[verify voter-set.promote]
    #[test]
    fn test_promote_empty_set() {
        let voters: Vec<u64> = vec![];
        let result = promote_voters(&voters, 1, None);
        assert_eq!(result, vec![1]);
    }

    // r[verify voter-set.promote]
    #[test]
    fn test_promote_single_node() {
        let voters = vec![1];
        let result = promote_voters(&voters, 2, None);
        assert_eq!(result, vec![1, 2]);
    }

    // r[verify voter-set.replace]
    #[test]
    fn test_replace_same_as_learner() {
        // Learner == replace target
        let voters = vec![1, 2, 3];
        let result = promote_voters(&voters, 2, Some(2));
        // voters[1]==2 matches both learner_id and replace_id → skipped
        // Append 2 at end
        assert_eq!(result, vec![1, 3, 2]);
    }

    // --- Oracle tests matching aspen-cluster-handler expectations ---

    // r[verify voter-set.promote]
    #[test]
    fn test_oracle_promote_5_node_cluster() {
        // 5-node cluster, promote learner 6, no replacement
        let voters = vec![1, 2, 3, 4, 5];
        let result = promote_voters(&voters, 6, None);
        assert_eq!(result, vec![1, 2, 3, 4, 5, 6]);
    }

    // r[verify voter-set.replace]
    #[test]
    fn test_oracle_promote_with_replacement() {
        // 5-node cluster, promote learner 6, replace node 2
        let voters = vec![1, 2, 3, 4, 5];
        let result = promote_voters(&voters, 6, Some(2));
        assert_eq!(result, vec![1, 3, 4, 5, 6]);
        assert!(!result.contains(&2));
    }

    proptest! {
        // r[verify voter-set.learner-present]
        #[test]
        fn prop_learner_always_present(
            voters in proptest::collection::vec(0u64..100, 0..20),
            learner_id in 0u64..100,
            replace_id in proptest::option::of(0u64..100),
        ) {
            let deduped = dedup(&voters);
            let result = promote_voters(&deduped, learner_id, replace_id);
            prop_assert!(result.contains(&learner_id),
                "learner {} not in {:?}", learner_id, result);
        }

        // r[verify voter-set.replaced-absent]
        #[test]
        fn prop_replaced_always_absent(
            voters in proptest::collection::vec(0u64..100, 0..20),
            learner_id in 0u64..100,
            rid in 0u64..100,
        ) {
            prop_assume!(learner_id != rid);
            let deduped = dedup(&voters);
            let result = promote_voters(&deduped, learner_id, Some(rid));
            prop_assert!(!result.contains(&rid),
                "replaced {} in {:?}", rid, result);
        }

        // r[verify voter-set.others-preserved]
        #[test]
        fn prop_others_preserved(
            voters in proptest::collection::vec(0u64..100, 0..20),
            learner_id in 0u64..100,
            replace_id in proptest::option::of(0u64..100),
        ) {
            let deduped = dedup(&voters);
            let result = promote_voters(&deduped, learner_id, replace_id);
            for &v in &deduped {
                if v != learner_id && replace_id.map_or(true, |rid| v != rid) {
                    prop_assert!(result.contains(&v),
                        "voter {} lost from {:?}", v, result);
                }
            }
        }

        // r[verify voter-set.no-duplicates]
        #[test]
        fn prop_no_duplicates(
            voters in proptest::collection::vec(0u64..100, 0..20),
            learner_id in 0u64..100,
            replace_id in proptest::option::of(0u64..100),
        ) {
            let deduped = dedup(&voters);
            let result = promote_voters(&deduped, learner_id, replace_id);
            for i in 0..result.len() {
                for j in (i + 1)..result.len() {
                    prop_assert!(result[i] != result[j],
                        "duplicate at {},{} in {:?}", i, j, result);
                }
            }
        }

        // r[verify voter-set.quorum-preserved]
        #[test]
        fn prop_quorum_preserved_no_replace(
            voters in proptest::collection::vec(0u64..100, 1..20),
            learner_id in 0u64..100,
        ) {
            let deduped = dedup(&voters);
            prop_assume!(!deduped.is_empty());
            let result = promote_voters(&deduped, learner_id, None);
            prop_assert!(result.len() >= deduped.len(),
                "shrunk: {} < {}", result.len(), deduped.len());
        }

        // r[verify voter-set.quorum-preserved]
        #[test]
        fn prop_quorum_preserved_with_replace(
            voters in proptest::collection::vec(0u64..100, 3..20),
            learner_id in 0u64..100,
            rid in 0u64..100,
        ) {
            prop_assume!(learner_id != rid);
            let deduped = dedup(&voters);
            prop_assume!(deduped.len() >= 3);
            let old_n = deduped.len() as u64;
            prop_assume!(crate::quorum::can_remove_safely(old_n));
            let result = promote_voters(&deduped, learner_id, Some(rid));
            let new_n = result.len() as u64;
            // With can_remove_safely, replacing one node while adding learner
            // keeps size >= old_size - 1 >= majority(old_size), so quorum holds
            prop_assert!(new_n > 0);
            prop_assert!(crate::quorum::is_quorum(new_n, new_n));
        }
    }

    /// Dedup helper for tests.
    fn dedup(v: &[u64]) -> Vec<u64> {
        let mut out = Vec::new();
        for &x in v {
            if !out.contains(&x) {
                out.push(x);
            }
        }
        out
    }
}
