//! Verified natural-loop utilities over a block CFG.
//!
//! Exact public helpers operate over a CFG plus previously computed reachability
//! and dominance views. Back edges are CFG edges whose target dominates their
//! source. Loop bodies are modeled as a seeded backward predecessor closure
//! restricted to reachable, header-dominated blocks. Tests compare those bodies
//! against a brute-force path oracle for reachable non-self loops.

#![allow(clippy::too_many_lines)]

use vstd::prelude::*;

verus! {

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NaturalLoop {
    pub header: usize,
    pub body: Vec<bool>,
}

// ============================================================
// Specs
// ============================================================

pub open spec fn has_edge_spec(edges: Seq<(usize, usize)>, src: int, dst: int) -> bool {
    exists|i: int| #![trigger edges[i]] 0 <= i < edges.len()
        && edges[i].0 as int == src
        && edges[i].1 as int == dst
}

// r[depends loop.back-edge]
pub open spec fn is_back_edge_spec(dom: Seq<Vec<bool>>, src: int, dst: int) -> bool {
    0 <= src < dom.len() && 0 <= dst < dom.len() && dom[src]@[dst]
}

pub open spec fn back_edges_prefix_spec(
    edges: Seq<(usize, usize)>, dom: Seq<Vec<bool>>, upto: int,
) -> Seq<(usize, usize)>
    decreases if upto > 0 { upto } else { 0 },
{
    if upto <= 0 {
        seq![]
    } else {
        let prev = back_edges_prefix_spec(edges, dom, upto - 1);
        let e = edges[upto - 1];
        if is_back_edge_spec(dom, e.0 as int, e.1 as int) {
            prev.push(e)
        } else {
            prev
        }
    }
}

// r[impl loop.back-edge]
pub open spec fn back_edges_spec(edges: Seq<(usize, usize)>, dom: Seq<Vec<bool>>) -> Seq<(usize, usize)> {
    back_edges_prefix_spec(edges, dom, edges.len() as int)
}

pub open spec fn loop_body_base_spec(n: int, header: int, source: int) -> Seq<bool>
{
    Seq::new(n as nat, |i: int| i == header || i == source)
}

pub open spec fn seeded_body_spec(prev: Seq<bool>, header: int, source: int) -> Seq<bool>
{
    Seq::new(prev.len(), |i: int| prev[i] || i == header || i == source)
}

pub open spec fn loop_body_round_prefix_spec(
    edges: Seq<(usize, usize)>,
    reachable: Seq<bool>,
    dom: Seq<Vec<bool>>,
    prev: Seq<bool>,
    header: int,
    source: int,
    upto: int,
) -> Seq<bool>
    decreases if upto > 0 { upto } else { 0 },
{
    if upto <= 0 {
        seeded_body_spec(prev, header, source)
    } else {
        let prior = loop_body_round_prefix_spec(edges, reachable, dom, prev, header, source, upto - 1);
        let (src, dst) = edges[upto - 1];
        if 0 <= src < reachable.len()
            && 0 <= src < dom.len()
            && 0 <= header < dom[src as int]@.len()
            && 0 <= dst < prev.len()
            && reachable[src as int]
            && dom[src as int]@[header]
            && prev[dst as int]
            && (source == header || dst as int != header)
        {
            prior.update(src as int, true)
        } else {
            prior
        }
    }
}

pub open spec fn loop_body_round_spec(
    edges: Seq<(usize, usize)>,
    reachable: Seq<bool>,
    dom: Seq<Vec<bool>>,
    prev: Seq<bool>,
    header: int,
    source: int,
) -> Seq<bool>
{
    loop_body_round_prefix_spec(edges, reachable, dom, prev, header, source, edges.len() as int)
}

pub open spec fn loop_body_iter_spec(
    edges: Seq<(usize, usize)>,
    reachable: Seq<bool>,
    dom: Seq<Vec<bool>>,
    header: int,
    source: int,
    rounds: nat,
) -> Seq<bool>
    decreases rounds,
{
    if rounds == 0 {
        loop_body_base_spec(reachable.len() as int, header, source)
    } else {
        let prev = loop_body_iter_spec(edges, reachable, dom, header, source, (rounds - 1) as nat);
        loop_body_round_spec(edges, reachable, dom, prev, header, source)
    }
}

// r[impl loop.body]
pub open spec fn loop_body_spec(
    edges: Seq<(usize, usize)>,
    reachable: Seq<bool>,
    dom: Seq<Vec<bool>>,
    header: int,
    source: int,
) -> Seq<bool>
{
    loop_body_iter_spec(edges, reachable, dom, header, source, reachable.len() as nat)
}

// r[impl loop.header-dominance]
pub open spec fn header_dominates_body_spec(dom: Seq<Vec<bool>>, header: int, body: Seq<bool>) -> bool {
    body.len() == dom.len()
        && 0 <= header < body.len()
        && forall|node: int| #![trigger body[node]] 0 <= node < body.len() && body[node] ==> dom[node]@[header]
}

// r[impl loop.single-entry]
pub open spec fn single_entry_spec(
    edges: Seq<(usize, usize)>,
    reachable: Seq<bool>,
    header: int,
    body: Seq<bool>,
) -> bool {
    reachable.len() == body.len()
        && 0 <= header < body.len()
        && forall|i: int| #![trigger edges[i]] 0 <= i < edges.len() ==> {
            let (src, dst) = edges[i];
            0 <= src < body.len() && 0 <= dst < body.len() ==>
                (reachable[src as int] && body[dst as int] && !body[src as int] ==> dst as int == header)
        }
}

pub open spec fn exit_edges_prefix_spec(
    edges: Seq<(usize, usize)>, body: Seq<bool>, upto: int,
) -> Seq<(usize, usize)>
    decreases if upto > 0 { upto } else { 0 },
{
    if upto <= 0 {
        seq![]
    } else {
        let prev = exit_edges_prefix_spec(edges, body, upto - 1);
        let e = edges[upto - 1];
        if 0 <= e.0 < body.len() && 0 <= e.1 < body.len() && body[e.0 as int] && !body[e.1 as int] {
            prev.push(e)
        } else {
            prev
        }
    }
}

// r[impl loop.exits]
pub open spec fn loop_exits_spec(edges: Seq<(usize, usize)>, body: Seq<bool>) -> Seq<(usize, usize)> {
    exit_edges_prefix_spec(edges, body, edges.len() as int)
}

// r[depends loop.nesting]
pub open spec fn body_count_from_spec(body: Seq<bool>, from: int) -> int
    decreases if from < body.len() as int { (body.len() - from) as int } else { 0 },
{
    if from < 0 || from >= body.len() {
        0
    } else {
        let rest = body_count_from_spec(body, from + 1);
        if body[from] { rest + 1 } else { rest }
    }
}

pub open spec fn body_count_spec(body: Seq<bool>) -> int {
    body_count_from_spec(body, 0)
}

pub open spec fn body_subset_spec(a: Seq<bool>, b: Seq<bool>) -> bool {
    a.len() == b.len()
        && forall|i: int| #![trigger a[i], b[i]] 0 <= i < a.len() ==> (a[i] ==> b[i])
}

// r[impl loop.nesting]
pub open spec fn strict_body_subset_spec(a: Seq<bool>, b: Seq<bool>) -> bool {
    body_subset_spec(a, b)
        && exists|i: int| 0 <= i < b.len() && b[i] && !a[i]
}

// r[depends loop.nesting-forest]
pub open spec fn nesting_parent_valid_spec(loops: Seq<NaturalLoop>, parents: Seq<Option<usize>>) -> bool {
    parents.len() == loops.len()
        && forall|i: int| #![trigger parents[i]] 0 <= i < parents.len() ==> match parents[i] {
            Some(p) => (p as int) < loops.len() && strict_body_subset_spec(loops[i].body@, loops[p as int].body@),
            None => true,
        }
}

// r[impl loop.nesting-forest]
pub open spec fn is_loop_header_spec(back_edges: Seq<(usize, usize)>, header: int) -> bool {
    exists|i: int| #![trigger back_edges[i]] 0 <= i < back_edges.len() && back_edges[i].1 as int == header
}

// ============================================================
// Helpers
// ============================================================

fn zero_body(n: usize) -> (result: Vec<bool>)
    ensures
        result@.len() == n as int,
        forall|i: int| 0 <= i < n as int ==> !result@[i],
{
    let mut out: Vec<bool> = Vec::new();
    let mut i: usize = 0;
    while i < n
        invariant
            0 <= i <= n,
            out@.len() == i as int,
            forall|j: int| 0 <= j < i as int ==> !out@[j],
        decreases n - i,
    {
        out.push(false);
        i += 1;
    }
    out
}

fn clone_body(body: &[bool]) -> (result: Vec<bool>)
    ensures
        result@.len() == body@.len(),
        forall|i: int| 0 <= i < body@.len() ==> result@[i] == body@[i],
{
    let mut out: Vec<bool> = Vec::new();
    let mut i: usize = 0;
    while i < body.len()
        invariant
            0 <= i <= body@.len(),
            out@.len() == i as int,
            forall|j: int| 0 <= j < i as int ==> out@[j] == body@[j],
        decreases body.len() - i,
    {
        out.push(body[i]);
        i += 1;
    }
    out
}

fn base_body(n: usize, header: usize, source: usize) -> (result: Vec<bool>)
    requires
        header < n,
        source < n,
    ensures
        result@ == loop_body_base_spec(n as int, header as int, source as int),
{
    let mut out = zero_body(n);
    out.set(header, true);
    out.set(source, true);
    out
}

fn seeded_round_body(prev: &[bool], header: usize, source: usize) -> (result: Vec<bool>)
    requires
        header < prev@.len(),
        source < prev@.len(),
    ensures
        result@.len() == prev@.len(),
        forall|i: int| 0 <= i < prev@.len() ==> result@[i] == (prev@[i] || i == header as int || i == source as int),
{
    let mut out = clone_body(prev);
    out.set(header, true);
    out.set(source, true);
    out
}

// ============================================================
// Back edges
// ============================================================

// r[impl loop.back-edge]
pub fn is_back_edge(dom: &[Vec<bool>], src: usize, dst: usize) -> (result: bool)
    requires
        crate::dominator_tree::dom_rows_shape_spec(dom@, dom@.len() as int),
        src < dom@.len(),
        dst < dom@.len(),
    ensures result == is_back_edge_spec(dom@, src as int, dst as int)
{
    dom[src][dst]
}

// r[impl loop.back-edge]
pub fn find_back_edges(edges: &[(usize, usize)], dom: &[Vec<bool>]) -> (result: Vec<(usize, usize)>)
    requires
        crate::dominator_tree::dom_rows_shape_spec(dom@, dom@.len() as int),
        crate::dominator_tree::valid_graph_spec(edges@, dom@.len() as int),
    ensures result@ == back_edges_spec(edges@, dom@)
{
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut i: usize = 0;
    while i < edges.len()
        invariant
            0 <= i <= edges@.len(),
            crate::dominator_tree::dom_rows_shape_spec(dom@, dom@.len() as int),
            crate::dominator_tree::valid_graph_spec(edges@, dom@.len() as int),
            out@ == back_edges_prefix_spec(edges@, dom@, i as int),
        decreases edges.len() - i,
    {
        let (src, dst) = edges[i];
        if dom[src][dst] {
            out.push((src, dst));
            proof {
                assert(back_edges_prefix_spec(edges@, dom@, i as int + 1)
                    == back_edges_prefix_spec(edges@, dom@, i as int).push(edges@[i as int]));
            }
        } else {
            proof {
                assert(back_edges_prefix_spec(edges@, dom@, i as int + 1)
                    == back_edges_prefix_spec(edges@, dom@, i as int));
            }
        }
        i += 1;
    }
    out
}

// ============================================================
// Loop body and derived properties
// ============================================================

// r[impl loop.body]
pub fn compute_loop_body(
    edges: &[(usize, usize)],
    reachable: &[bool],
    dom: &[Vec<bool>],
    header: usize,
    source: usize,
) -> (result: Vec<bool>)
    requires
        reachable@.len() == dom@.len(),
        crate::dominator_tree::dom_rows_shape_spec(dom@, reachable@.len() as int),
        crate::dominator_tree::valid_graph_spec(edges@, reachable@.len() as int),
        header < reachable@.len(),
        source < reachable@.len(),
    ensures result@ == loop_body_spec(edges@, reachable@, dom@, header as int, source as int)
{
    let n = reachable.len();
    let mut body = base_body(n, header, source);
    let mut iter: usize = 0;
    while iter < n
        invariant
            0 <= iter <= n,
            reachable@.len() == dom@.len(),
            crate::dominator_tree::dom_rows_shape_spec(dom@, reachable@.len() as int),
            crate::dominator_tree::valid_graph_spec(edges@, reachable@.len() as int),
            header < reachable@.len(),
            source < reachable@.len(),
            body@.len() == reachable@.len(),
            body@ == loop_body_iter_spec(edges@, reachable@, dom@, header as int, source as int, iter as nat),
        decreases n - iter,
    {
        let prev = clone_body(&body);
        assert(header < prev.len());
        assert(source < prev.len());
        let mut next = seeded_round_body(&prev, header, source);
        let mut ei: usize = 0;
        while ei < edges.len()
            invariant
                0 <= ei <= edges@.len(),
                reachable@.len() == dom@.len(),
                crate::dominator_tree::dom_rows_shape_spec(dom@, reachable@.len() as int),
                crate::dominator_tree::valid_graph_spec(edges@, reachable@.len() as int),
                header < reachable@.len(),
                source < reachable@.len(),
                prev@.len() == reachable@.len(),
                next@.len() == reachable@.len(),
                prev@ == loop_body_iter_spec(edges@, reachable@, dom@, header as int, source as int, iter as nat),
                next@ == loop_body_round_prefix_spec(edges@, reachable@, dom@, prev@, header as int, source as int, ei as int),
            decreases edges.len() - ei,
        {
            let (src, dst) = edges[ei];
            proof {
                let e = edges@[ei as int];
                assert(e == (src, dst));
                assert((src as int) < reachable@.len());
                assert((dst as int) < reachable@.len());
            }
            assert(src < reachable.len());
            assert(src < dom.len());
            assert(dst < reachable.len());
            assert(dst < prev.len());
            if reachable[src] && dom[src][header] && prev[dst] && (source == header || dst != header) {
                assert(src < next.len());
                next.set(src, true);
                proof {
                    assert(loop_body_round_prefix_spec(edges@, reachable@, dom@, prev@, header as int, source as int, ei as int + 1)
                        == loop_body_round_prefix_spec(edges@, reachable@, dom@, prev@, header as int, source as int, ei as int).update(src as int, true));
                }
            } else {
                proof {
                    assert(loop_body_round_prefix_spec(edges@, reachable@, dom@, prev@, header as int, source as int, ei as int + 1)
                        == loop_body_round_prefix_spec(edges@, reachable@, dom@, prev@, header as int, source as int, ei as int));
                }
            }
            ei += 1;
        }
        proof {
            assert(loop_body_iter_spec(edges@, reachable@, dom@, header as int, source as int, (iter + 1) as nat)
                == loop_body_round_spec(edges@, reachable@, dom@, prev@, header as int, source as int));
        }
        body = next;
        iter += 1;
    }
    body
}

// r[impl loop.header-dominance]
pub fn header_dominates_body(dom: &[Vec<bool>], header: usize, body: &[bool]) -> (result: bool)
    requires
        crate::dominator_tree::dom_rows_shape_spec(dom@, dom@.len() as int),
        body@.len() == dom@.len(),
        header < body@.len(),
    ensures result == header_dominates_body_spec(dom@, header as int, body@)
{
    let mut i: usize = 0;
    while i < body.len()
        invariant
            0 <= i <= body@.len(),
            crate::dominator_tree::dom_rows_shape_spec(dom@, dom@.len() as int),
            body@.len() == dom@.len(),
            header < body@.len(),
            forall|j: int| #![trigger body@[j]] 0 <= j < i as int && body@[j] ==> dom@[j]@[header as int],
        decreases body.len() - i,
    {
        if body[i] && !dom[i][header] {
            return false;
        }
        i += 1;
    }
    true
}

// r[impl loop.single-entry]
pub fn has_single_entry(
    edges: &[(usize, usize)],
    reachable: &[bool],
    header: usize,
    body: &[bool],
) -> (result: bool)
    requires
        reachable@.len() == body@.len(),
        crate::dominator_tree::valid_graph_spec(edges@, body@.len() as int),
        header < body@.len(),
    ensures result == single_entry_spec(edges@, reachable@, header as int, body@)
{
    let mut i: usize = 0;
    while i < edges.len()
        invariant
            0 <= i <= edges@.len(),
            reachable@.len() == body@.len(),
            crate::dominator_tree::valid_graph_spec(edges@, body@.len() as int),
            header < body@.len(),
            forall|j: int| #![trigger edges@[j]] 0 <= j < i as int ==> {
                let (src, dst) = edges@[j];
                0 <= src < body@.len() && 0 <= dst < body@.len() ==>
                    (reachable@[src as int] && body@[dst as int] && !body@[src as int] ==> dst as int == header as int)
            },
        decreases edges.len() - i,
    {
        let (src, dst) = edges[i];
        proof {
            let e = edges@[i as int];
            assert(e == (src, dst));
            assert((src as int) < body@.len());
            assert((dst as int) < body@.len());
        }
        assert(src < body.len());
        assert(dst < body.len());
        assert(src < reachable.len());
        if reachable[src] && body[dst] && !body[src] && dst != header {
            return false;
        }
        i += 1;
    }
    true
}

// r[impl loop.exits]
pub fn loop_exits(edges: &[(usize, usize)], body: &[bool]) -> (result: Vec<(usize, usize)>)
    requires
        crate::dominator_tree::valid_graph_spec(edges@, body@.len() as int),
    ensures result@ == loop_exits_spec(edges@, body@)
{
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut i: usize = 0;
    while i < edges.len()
        invariant
            0 <= i <= edges@.len(),
            crate::dominator_tree::valid_graph_spec(edges@, body@.len() as int),
            out@ == exit_edges_prefix_spec(edges@, body@, i as int),
        decreases edges.len() - i,
    {
        let (src, dst) = edges[i];
        if body[src] && !body[dst] {
            out.push((src, dst));
            proof {
                assert(exit_edges_prefix_spec(edges@, body@, i as int + 1)
                    == exit_edges_prefix_spec(edges@, body@, i as int).push(edges@[i as int]));
            }
        } else {
            proof {
                assert(exit_edges_prefix_spec(edges@, body@, i as int + 1)
                    == exit_edges_prefix_spec(edges@, body@, i as int));
            }
        }
        i += 1;
    }
    out
}

// r[impl loop.nesting]
pub fn nested_in(inner: &NaturalLoop, outer: &NaturalLoop) -> (result: bool)
    requires inner.body@.len() == outer.body@.len()
    ensures result == strict_body_subset_spec(inner.body@, outer.body@)
{
    let subset = body_subset(&inner.body, &outer.body);
    if !subset {
        return false;
    }
    let mut i: usize = 0;
    while i < outer.body.len()
        invariant
            0 <= i <= outer.body@.len(),
            inner.body@.len() == outer.body@.len(),
            body_subset_spec(inner.body@, outer.body@),
            forall|j: int| 0 <= j < i as int ==> !(outer.body@[j] && !inner.body@[j]),
        decreases outer.body.len() - i,
    {
        if outer.body[i] && !inner.body[i] {
            return true;
        }
        i += 1;
    }
    false
}

fn body_subset(a: &[bool], b: &[bool]) -> (result: bool)
    requires a@.len() == b@.len()
    ensures result == body_subset_spec(a@, b@)
{
    let mut i: usize = 0;
    while i < a.len()
        invariant
            0 <= i <= a@.len(),
            a@.len() == b@.len(),
            forall|j: int| #![trigger a@[j], b@[j]] 0 <= j < i as int ==> (a@[j] ==> b@[j]),
        decreases a.len() - i,
    {
        if a[i] && !b[i] {
            return false;
        }
        i += 1;
    }
    true
}

fn body_count(body: &[bool]) -> (result: usize)
    ensures result as int == body_count_spec(body@)
{
    let mut count: usize = 0;
    let mut i: usize = 0;
    while i < body.len()
        invariant
            0 <= i <= body@.len(),
            count <= i,
            count as int == body_count_from_spec(body@, 0) - body_count_from_spec(body@, i as int),
        decreases body.len() - i,
    {
        if body[i] {
            assert(count < body.len());
            count += 1;
        }
        i += 1;
    }
    proof {
        assert(body_count_from_spec(body@, body@.len() as int) == 0);
    }
    count
}

// r[impl loop.nesting-forest]
pub fn is_loop_header(back_edges: &[(usize, usize)], header: usize) -> (result: bool)
    requires header < usize::MAX,
    ensures result == is_loop_header_spec(back_edges@, header as int)
{
    let mut i: usize = 0;
    while i < back_edges.len()
        invariant
            0 <= i <= back_edges@.len(),
            forall|j: int| #![trigger back_edges@[j]] 0 <= j < i as int ==> back_edges@[j].1 as int != header as int,
        decreases back_edges.len() - i,
    {
        if back_edges[i].1 == header {
            return true;
        }
        i += 1;
    }
    false
}

// r[impl loop.nesting-forest]
pub fn loop_nesting_forest(loops: &[NaturalLoop]) -> (result: Vec<Option<usize>>)
    requires forall|i: int| #![trigger loops@[i]] 0 <= i < loops@.len() ==> loops@[i].header < loops@[i].body@.len()
    ensures nesting_parent_valid_spec(loops@, result@)
{
    let mut out: Vec<Option<usize>> = Vec::new();
    let mut i: usize = 0;
    while i < loops.len()
        invariant
            0 <= i <= loops@.len(),
            forall|j: int| #![trigger loops@[j]] 0 <= j < loops@.len() ==> loops@[j].header < loops@[j].body@.len(),
            out@.len() == i as int,
            forall|j: int| #![trigger out@[j]] 0 <= j < i as int ==> match out@[j] {
                Some(p) => (p as int) < loops@.len() && strict_body_subset_spec(loops@[j].body@, loops@[p as int].body@),
                None => true,
            },
        decreases loops.len() - i,
    {
        let mut best: Option<usize> = None;
        let mut j: usize = 0;
        while j < loops.len()
            invariant
                0 <= j <= loops@.len(),
                i < loops@.len(),
                forall|k: int| #![trigger loops@[k]] 0 <= k < loops@.len() ==> loops@[k].header < loops@[k].body@.len(),
                best.is_some() ==> best.unwrap() < loops.len(),
                best.is_some() ==> strict_body_subset_spec(loops@[i as int].body@, loops@[best.unwrap() as int].body@),
            decreases loops.len() - j,
        {
            if j != i && loops[j].body.len() == loops[i].body.len() && nested_in(&loops[i], &loops[j]) {
                match best {
                    None => best = Some(j),
                    Some(current) => {
                        let candidate_count = body_count(&loops[j].body);
                        let current_count = body_count(&loops[current].body);
                        if candidate_count < current_count
                            || (candidate_count == current_count && j < current)
                        {
                            best = Some(j);
                        }
                    }
                }
            }
            j += 1;
        }
        out.push(best);
        i += 1;
    }
    out
}

} // verus!

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::collections::BTreeSet;

    use proptest::prelude::*;

    use super::*;

    fn manual_reachable_avoiding_header(
        edges: &[(usize, usize)],
        n: usize,
        start: usize,
        header: usize,
        source: usize,
    ) -> bool {
        fn dfs(edges: &[(usize, usize)], cur: usize, header: usize, source: usize, seen: &mut [bool]) -> bool {
            if cur == source {
                return true;
            }
            if cur == header {
                return false;
            }
            seen[cur] = true;
            for &(_, dst) in edges.iter().filter(|(src, _)| *src == cur) {
                if !seen[dst] && dfs(edges, dst, header, source, seen) {
                    seen[cur] = false;
                    return true;
                }
            }
            seen[cur] = false;
            false
        }

        if start == header {
            return true;
        }
        let mut seen = vec![false; n];
        dfs(edges, start, header, source, &mut seen)
    }

    fn manual_loop_body(edges: &[(usize, usize)], n: usize, header: usize, source: usize) -> Vec<bool> {
        let reachable = manual_reachable(edges, n, 0);
        let dom = manual_dominators(edges, n, 0);
        (0..n)
            .map(|node| {
                reachable[node] && dom[node][header] && manual_reachable_avoiding_header(edges, n, node, header, source)
            })
            .collect()
    }

    fn all_simple_paths(edges: &[(usize, usize)], entry: usize, target: usize, n: usize) -> Vec<Vec<usize>> {
        fn dfs(
            edges: &[(usize, usize)],
            cur: usize,
            target: usize,
            n: usize,
            seen: &mut Vec<bool>,
            path: &mut Vec<usize>,
            out: &mut Vec<Vec<usize>>,
        ) {
            if cur == target {
                out.push(path.clone());
                return;
            }
            for &(src, dst) in edges {
                if src == cur && dst < n && !seen[dst] {
                    seen[dst] = true;
                    path.push(dst);
                    dfs(edges, dst, target, n, seen, path, out);
                    path.pop();
                    seen[dst] = false;
                }
            }
        }

        let mut out = Vec::new();
        let mut seen = vec![false; n];
        seen[entry] = true;
        let mut path = vec![entry];
        dfs(edges, entry, target, n, &mut seen, &mut path, &mut out);
        out
    }

    fn manual_reachable(edges: &[(usize, usize)], n: usize, entry: usize) -> Vec<bool> {
        let mut seen = vec![false; n];
        let mut q = std::collections::VecDeque::new();
        seen[entry] = true;
        q.push_back(entry);
        while let Some(u) = q.pop_front() {
            for &(src, dst) in edges {
                if src == u && !seen[dst] {
                    seen[dst] = true;
                    q.push_back(dst);
                }
            }
        }
        seen
    }

    fn manual_dominators(edges: &[(usize, usize)], n: usize, entry: usize) -> Vec<Vec<bool>> {
        let reachable = manual_reachable(edges, n, entry);
        let mut dom = vec![vec![false; n]; n];
        for b in 0..n {
            if !reachable[b] {
                continue;
            }
            let paths = all_simple_paths(edges, entry, b, n);
            for a in 0..n {
                dom[b][a] = if a == b {
                    true
                } else if paths.is_empty() {
                    false
                } else {
                    paths.iter().all(|path| path.contains(&a))
                };
            }
        }
        dom
    }

    fn manual_back_edges(edges: &[(usize, usize)], dom: &[Vec<bool>]) -> Vec<(usize, usize)> {
        edges.iter().copied().filter(|(src, dst)| dom[*src][*dst]).collect()
    }

    fn compute_analyses(edges: &[(usize, usize)], n: usize) -> (Vec<bool>, Vec<Vec<bool>>) {
        let reachable = crate::reachability::reachable_from(edges, n, 0);
        let dom = crate::dominator_tree::compute_dominators(edges, n, 0);
        (reachable, dom)
    }

    fn body_count_runtime(body: &[bool]) -> usize {
        body.iter().filter(|&&b| b).count()
    }

    fn merged_loops_from_cfg(edges: &[(usize, usize)], n: usize) -> Vec<NaturalLoop> {
        let (reachable, dom) = compute_analyses(edges, n);
        let mut by_header: BTreeMap<usize, Vec<bool>> = BTreeMap::new();
        for (src, header) in find_back_edges(edges, &dom) {
            if src != header {
                let body = compute_loop_body(edges, &reachable, &dom, header, src);
                by_header
                    .entry(header)
                    .and_modify(|existing| {
                        for i in 0..existing.len() {
                            existing[i] = existing[i] || body[i];
                        }
                    })
                    .or_insert(body);
            }
        }
        by_header.into_iter().map(|(header, body)| NaturalLoop { header, body }).collect()
    }

    fn expected_parent(loops: &[NaturalLoop], child: usize) -> Option<usize> {
        let mut best: Option<usize> = None;
        for candidate in 0..loops.len() {
            if candidate == child {
                continue;
            }
            if nested_in(&loops[child], &loops[candidate]) {
                match best {
                    None => best = Some(candidate),
                    Some(current) => {
                        let candidate_count = body_count_runtime(&loops[candidate].body);
                        let current_count = body_count_runtime(&loops[current].body);
                        if candidate_count < current_count || (candidate_count == current_count && candidate < current)
                        {
                            best = Some(candidate);
                        }
                    }
                }
            }
        }
        best
    }

    fn random_graph_strategy(max_nodes: usize) -> impl Strategy<Value = (usize, Vec<(usize, usize)>)> {
        (1usize..=max_nodes).prop_flat_map(|n| {
            prop::collection::vec((0usize..n, 0usize..n), 0..=(n * n)).prop_map(move |edges| (n, edges))
        })
    }

    // r[verify loop.back-edge]
    #[test]
    fn test_find_back_edges_nested_loops() {
        let n = 6;
        let edges = vec![(0, 1), (1, 2), (1, 5), (2, 3), (3, 2), (3, 4), (4, 1), (4, 5)];
        let (_, dom) = compute_analyses(&edges, n);
        let backs = find_back_edges(&edges, &dom);
        assert_eq!(backs, vec![(3, 2), (4, 1)]);
        assert!(is_back_edge(&dom, 3, 2));
        assert!(is_back_edge(&dom, 4, 1));
        assert!(!is_back_edge(&dom, 1, 5));
    }

    // r[verify loop.body]
    #[test]
    fn test_compute_loop_body_nested_loops() {
        let n = 6;
        let edges = vec![(0, 1), (1, 2), (1, 5), (2, 3), (3, 2), (3, 4), (4, 1), (4, 5)];
        let (reachable, dom) = compute_analyses(&edges, n);
        let inner = compute_loop_body(&edges, &reachable, &dom, 2, 3);
        let outer = compute_loop_body(&edges, &reachable, &dom, 1, 4);
        assert_eq!(inner, vec![false, false, true, true, false, false]);
        assert_eq!(outer, vec![false, true, true, true, true, false]);
    }

    // r[verify loop.body]
    #[test]
    fn test_compute_loop_body_while_if_shape() {
        let n = 6;
        let edges = vec![(0, 1), (1, 2), (1, 3), (2, 4), (3, 4), (4, 1), (4, 5)];
        let (reachable, dom) = compute_analyses(&edges, n);
        let body = compute_loop_body(&edges, &reachable, &dom, 1, 4);
        assert_eq!(body, vec![false, true, true, true, true, false]);
        assert_eq!(loop_exits(&edges, &body), vec![(4, 5)]);
        assert!(has_single_entry(&edges, &reachable, 1, &body));
        assert!(header_dominates_body(&dom, 1, &body));
    }

    // r[verify loop.exits]
    #[test]
    fn test_loop_exits_nested_loops() {
        let n = 6;
        let edges = vec![(0, 1), (1, 2), (1, 5), (2, 3), (3, 2), (3, 4), (4, 1), (4, 5)];
        let (reachable, dom) = compute_analyses(&edges, n);
        let inner = compute_loop_body(&edges, &reachable, &dom, 2, 3);
        let outer = compute_loop_body(&edges, &reachable, &dom, 1, 4);
        assert_eq!(loop_exits(&edges, &inner), vec![(3, 4)]);
        assert_eq!(loop_exits(&edges, &outer), vec![(1, 5), (4, 5)]);
    }

    // r[verify loop.nesting]
    #[test]
    fn test_loop_nesting_nested_loops() {
        let inner = NaturalLoop {
            header: 2,
            body: vec![false, false, true, true, false, false],
        };
        let outer = NaturalLoop {
            header: 1,
            body: vec![false, true, true, true, true, false],
        };
        assert!(nested_in(&inner, &outer));
        assert!(!nested_in(&outer, &inner));
    }

    // r[verify loop.nesting-forest]
    #[test]
    fn test_loop_nesting_forest_nested_loops() {
        let loops = vec![
            NaturalLoop {
                header: 1,
                body: vec![false, true, true, true, true, false],
            },
            NaturalLoop {
                header: 2,
                body: vec![false, false, true, true, false, false],
            },
        ];
        let parents = loop_nesting_forest(&loops);
        assert_eq!(parents, vec![None, Some(0)]);
    }

    // r[verify loop.nesting-forest]
    #[test]
    fn test_loop_nesting_forest_picks_immediate_parent() {
        let loops = vec![
            NaturalLoop {
                header: 1,
                body: vec![false, true, true, true, true, true, true, false],
            },
            NaturalLoop {
                header: 3,
                body: vec![false, false, false, true, true, false, false, false],
            },
            NaturalLoop {
                header: 2,
                body: vec![false, false, true, true, true, true, false, false],
            },
        ];
        let parents = loop_nesting_forest(&loops);
        assert_eq!(parents, vec![None, Some(2), Some(0)]);
    }

    // r[verify loop.single-entry]
    #[test]
    fn test_single_entry_nested_loop() {
        let n = 6;
        let edges = vec![(0, 1), (1, 2), (1, 5), (2, 3), (3, 2), (3, 4), (4, 1), (4, 5)];
        let (reachable, dom) = compute_analyses(&edges, n);
        let inner = compute_loop_body(&edges, &reachable, &dom, 2, 3);
        let outer = compute_loop_body(&edges, &reachable, &dom, 1, 4);
        assert!(has_single_entry(&edges, &reachable, 2, &inner));
        assert!(has_single_entry(&edges, &reachable, 1, &outer));
    }

    // r[verify loop.header-dominance]
    #[test]
    fn test_header_dominates_nested_loop_bodies() {
        let n = 6;
        let edges = vec![(0, 1), (1, 2), (1, 5), (2, 3), (3, 2), (3, 4), (4, 1), (4, 5)];
        let (reachable, dom) = compute_analyses(&edges, n);
        let inner = compute_loop_body(&edges, &reachable, &dom, 2, 3);
        let outer = compute_loop_body(&edges, &reachable, &dom, 1, 4);
        assert!(header_dominates_body(&dom, 2, &inner));
        assert!(header_dominates_body(&dom, 1, &outer));
    }

    proptest! {
        // r[verify loop.back-edge]
        #[test]
        fn prop_back_edges_match_manual((n, edges) in random_graph_strategy(6)) {
            let (_, dom) = compute_analyses(&edges, n);
            let backs = find_back_edges(&edges, &dom);
            let manual = manual_back_edges(&edges, &manual_dominators(&edges, n, 0));
            prop_assert_eq!(backs, manual);
        }

        // r[verify loop.body]
        #[test]
        fn prop_loop_body_matches_bruteforce((n, edges) in random_graph_strategy(6)) {
            let (reachable, dom) = compute_analyses(&edges, n);
            for (src, header) in find_back_edges(&edges, &dom) {
                if src != header {
                    let body = compute_loop_body(&edges, &reachable, &dom, header, src);
                    let manual = manual_loop_body(&edges, n, header, src);
                    prop_assert_eq!(body, manual);
                }
            }
        }

        // r[verify loop.header-dominance]
        #[test]
        fn prop_headers_dominate_computed_bodies((n, edges) in random_graph_strategy(6)) {
            let (reachable, dom) = compute_analyses(&edges, n);
            for (src, header) in find_back_edges(&edges, &dom) {
                if src != header {
                    let body = compute_loop_body(&edges, &reachable, &dom, header, src);
                    prop_assert!(header_dominates_body(&dom, header, &body));
                }
            }
        }

        // r[verify loop.single-entry]
        #[test]
        fn prop_computed_loops_have_single_entry((n, edges) in random_graph_strategy(6)) {
            let (reachable, dom) = compute_analyses(&edges, n);
            for (src, header) in find_back_edges(&edges, &dom) {
                if src != header {
                    let body = compute_loop_body(&edges, &reachable, &dom, header, src);
                    prop_assert!(has_single_entry(&edges, &reachable, header, &body));
                }
            }
        }

        // r[verify loop.exits]
        #[test]
        fn prop_exit_edges_match_partition((n, edges) in random_graph_strategy(6)) {
            let (reachable, dom) = compute_analyses(&edges, n);
            for (src, header) in find_back_edges(&edges, &dom) {
                if src != header {
                    let body = compute_loop_body(&edges, &reachable, &dom, header, src);
                    let exits = loop_exits(&edges, &body);
                    let manual: Vec<_> = edges.iter().copied().filter(|(u, v)| body[*u] && !body[*v]).collect();
                    prop_assert_eq!(exits, manual);
                }
            }
        }

        // r[verify loop.nesting-forest]
        #[test]
        fn prop_nesting_forest_parent_selection((n, edges) in random_graph_strategy(6)) {
            let loops = merged_loops_from_cfg(&edges, n);
            let parents = loop_nesting_forest(&loops);
            prop_assert_eq!(parents.len(), loops.len());
            for child in 0..loops.len() {
                prop_assert_eq!(parents[child], expected_parent(&loops, child));
            }

            let mut seen_roots = BTreeSet::new();
            for child in 0..loops.len() {
                let mut cur = child;
                let mut chain = BTreeSet::new();
                while let Some(parent) = parents[cur] {
                    prop_assert!(chain.insert(parent));
                    cur = parent;
                }
                seen_roots.insert(cur);
            }
            let _ = seen_roots;
        }
    }
}
