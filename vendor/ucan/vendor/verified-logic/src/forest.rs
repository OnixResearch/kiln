//! Verified parent-pointer forest primitives.
//!
//! Operations on topologically ordered forests represented as
//! `Seq<Option<int>>` where `parents[i] = Some(j)` means node `j`
//! is the parent of node `i` (with `j < i`), and `None` means root.
//! Applicable to git DAGs, file system trees, dependency graphs,
//! and conversation branching structures.

#![allow(
    clippy::single_match,
    clippy::collapsible_match,
    clippy::single_match_else,
    clippy::if_not_else
)]

use vstd::prelude::*;

verus! {

// ============================================================
// Well-formedness
// ============================================================

// r[depends forest.well-formedness]
/// Spec: a parent map is well-formed if every parent reference
/// points to a valid index strictly less than the child.
pub open spec fn well_formed_spec(parents: Seq<Option<int>>) -> bool {
    forall|i: int| #![trigger parents[i]] 0 <= i < parents.len() ==> match parents[i] {
        Some(j) => 0 <= j < i,
        None => true,
    }
}

// r[impl forest.well-formedness]
/// Check that a parent map is well-formed.
pub fn well_formed(parents: &[Option<usize>]) -> (result: bool)
    ensures result == well_formed_spec(
        parents@.map(|_i: int, p: Option<usize>| match p {
            Some(j) => Some(j as int),
            None => None::<int>,
        })
    ),
{
    let ghost pm = parents@.map(|_i: int, p: Option<usize>| match p {
        Some(j) => Some(j as int),
        None => None::<int>,
    });
    let mut i: usize = 0;
    while i < parents.len()
        invariant
            0 <= i <= parents@.len(),
            pm =~= parents@.map(|_i: int, p: Option<usize>| match p {
                Some(j) => Some(j as int),
                None => None::<int>,
            }),
            forall|k: int| #![trigger pm[k]] 0 <= k < i as int ==> match pm[k] {
                Some(j) => 0 <= j < k,
                None => true,
            },
        decreases parents.len() - i,
    {
        match parents[i] {
            Some(j) => {
                if j >= i {
                    proof {
                        // pm[i] == Some(j as int) where j >= i, violates well_formed
                        assert(pm[i as int] == Some(j as int));
                        assert(!(0 <= (j as int) < (i as int)));
                    }
                    return false;
                }
            }
            None => {}
        }
        i += 1;
    }
    true
}

// r[verify forest.well-formedness]
/// Empty forest is well-formed.
proof fn empty_well_formed()
    ensures well_formed_spec(Seq::<Option<int>>::empty()),
{}

// ============================================================
// Root and leaf classification
// ============================================================

// r[depends forest.root-leaf]
/// Spec: node `i` is a root if its parent is None.
pub open spec fn is_root_spec(parents: Seq<Option<int>>, i: int) -> bool {
    0 <= i < parents.len() && parents[i] == None::<int>
}

// r[depends forest.root-leaf]
/// Spec: node `i` is a leaf if no node has it as parent.
pub open spec fn is_leaf_spec(parents: Seq<Option<int>>, i: int) -> bool {
    0 <= i < parents.len()
    && forall|j: int| #![trigger parents[j]] 0 <= j < parents.len() ==> parents[j] != Some(i)
}

// r[impl forest.root-leaf]
/// Check if node is a leaf (no children).
pub fn is_leaf(parents: &[Option<usize>], node: usize) -> (result: bool)
    requires node < parents@.len()
    ensures result == is_leaf_spec(
        parents@.map(|_i: int, p: Option<usize>| match p {
            Some(j) => Some(j as int),
            None => None::<int>,
        }),
        node as int,
    ),
{
    let ghost pm = parents@.map(|_i: int, p: Option<usize>| match p {
        Some(j) => Some(j as int),
        None => None::<int>,
    });
    let mut j: usize = 0;
    while j < parents.len()
        invariant
            0 <= j <= parents@.len(),
            pm =~= parents@.map(|_i: int, p: Option<usize>| match p {
                Some(j) => Some(j as int),
                None => None::<int>,
            }),
            forall|k: int| #![trigger pm[k]] 0 <= k < j as int ==> pm[k] != Some(node as int),
        decreases parents.len() - j,
    {
        if parents[j] == Some(node) {
            assert(pm[j as int] == Some(node as int));
            return false;
        }
        j += 1;
    }
    true
}

// ============================================================
// children_of
// ============================================================

// r[depends forest.children-of]
/// Spec: collect children of parent `p`, scanning from index `from`.
pub open spec fn children_of_from_spec(
    parents: Seq<Option<int>>, p: Option<int>, from: int,
) -> Seq<int>
    decreases if from < parents.len() as int { (parents.len() - from) as int } else { 0 },
{
    if from >= parents.len() {
        seq![]
    } else if parents[from] == p {
        seq![from] + children_of_from_spec(parents, p, from + 1)
    } else {
        children_of_from_spec(parents, p, from + 1)
    }
}

// r[depends forest.children-of]
/// Spec: all children of parent `p`.
pub open spec fn children_of_spec(
    parents: Seq<Option<int>>, p: Option<int>,
) -> Seq<int> {
    children_of_from_spec(parents, p, 0)
}

// ============================================================
// ancestor_path — recursive spec, iterative exec
// ============================================================

// r[depends forest.ancestor-path]
/// Spec: path from root to `node`, following parent pointers upward.
/// Returns [root, ..., node] in root-to-leaf order.
/// Uses `fuel` to guarantee termination without requiring well_formed in
/// the `when` guard. Under well_formed, fuel = node + 1 always suffices.
pub open spec fn ancestor_path_fuel(
    parents: Seq<Option<int>>, node: int, fuel: nat,
) -> Seq<int>
    decreases fuel,
{
    if fuel == 0 || node < 0 || node >= parents.len() {
        seq![]
    } else {
        match parents[node] {
            Some(p) => ancestor_path_fuel(parents, p, (fuel - 1) as nat).push(node),
            None => seq![node],
        }
    }
}

/// Top-level ancestor path spec with sufficient fuel.
pub open spec fn ancestor_path_spec(
    parents: Seq<Option<int>>, node: int,
) -> Seq<int> {
    ancestor_path_fuel(parents, node, (if node >= 0 { node + 1 } else { 0 }) as nat)
}

/// Under well_formed, ancestor_path_spec unfolds one step.
proof fn ancestor_path_unfold(parents: Seq<Option<int>>, node: int)
    requires well_formed_spec(parents), 0 <= node < parents.len()
    ensures ancestor_path_spec(parents, node) == match parents[node] {
        Some(p) => ancestor_path_spec(parents, p).push(node),
        None => seq![node],
    }
    decreases node,
{
    let fuel: nat = (node + 1) as nat;
    // ancestor_path_spec(parents, node) = ancestor_path_fuel(parents, node, node+1)
    // Unfolds to: match parents[node] { Some(p) => fuel(parents, p, node).push(node) ... }
    // Need: fuel(parents, p, node) == fuel(parents, p, p+1) = ancestor_path_spec(parents, p)
    match parents[node] {
        Some(p) => {
            // well_formed: p < node, so p+1 <= node
            // fuel(parents, p, node as nat) with extra fuel
            // Need to show fuel(parents, p, node) == fuel(parents, p, p+1)
            ancestor_path_fuel_sufficient(parents, p, (fuel - 1) as nat);
        }
        None => {}
    }
}

/// Extra fuel beyond node+1 doesn't change the result under well_formed.
proof fn ancestor_path_fuel_sufficient(parents: Seq<Option<int>>, node: int, fuel: nat)
    requires well_formed_spec(parents), 0 <= node < parents.len(), fuel >= node + 1
    ensures ancestor_path_fuel(parents, node, fuel) ==
        ancestor_path_fuel(parents, node, (node + 1) as nat)
    decreases node,
{
    // Both unfold: fuel > 0 and (node+1) > 0, node in bounds
    // So both produce: match parents[node] { Some(p) => fuel(p, fuel-1/node).push(node), None => [node] }
    match parents[node] {
        Some(p) => {
            // well_formed: 0 <= p < node
            // Recursive: fuel(p, fuel-1) == fuel(p, p+1)
            ancestor_path_fuel_sufficient(parents, p, (fuel - 1) as nat);
            // And: fuel(p, node as nat) == fuel(p, p+1)
            ancestor_path_fuel_sufficient(parents, p, node as nat);
        }
        None => {
            // Both return seq![node]
        }
    }
}

// r[impl forest.ancestor-path]
/// Trace parent pointers from `node` to root, return path root→node.
/// Strategy: walk upward pushing to vec, then reverse by rebuilding.
pub fn ancestor_path(parents: &[Option<usize>], node: usize) -> (result: Vec<usize>)
    requires
        node < parents@.len(),
        well_formed_spec(parents@.map(|_i: int, p: Option<usize>| match p {
            Some(j) => Some(j as int),
            None => None::<int>,
        })),
    ensures
        result@.map(|_i: int, v: usize| v as int)
            =~= ancestor_path_spec(
                parents@.map(|_i: int, p: Option<usize>| match p {
                    Some(j) => Some(j as int),
                    None => None::<int>,
                }),
                node as int,
            ),
{
    let ghost pm = parents@.map(|_i: int, p: Option<usize>| match p {
        Some(j) => Some(j as int),
        None => None::<int>,
    });

    // Phase 1: walk upward, build path in leaf→root order
    let mut rev: Vec<usize> = Vec::new();
    let mut cur: usize = node;
    rev.push(cur);

    while parents[cur].is_some()
        invariant
            cur < parents@.len(),
            pm =~= parents@.map(|_i: int, p: Option<usize>| match p {
                Some(j) => Some(j as int),
                None => None::<int>,
            }),
            well_formed_spec(pm),
            rev@.len() > 0,
            rev@[0] == node,
            rev@[rev@.len() - 1] == cur,
            // All in bounds
            forall|k: int| #![trigger rev@[k]] 0 <= k < rev@.len()
                ==> 0 <= (rev@[k] as int) < parents@.len(),
            // Parent chain: rev[k]'s parent is rev[k+1]
            forall|k: int| #![trigger rev@[k]] 0 <= k < rev@.len() - 1
                ==> pm[rev@[k] as int] == Some(rev@[k + 1] as int),
            rev@.len() as int <= node as int - cur as int + 1,
        decreases cur,
    {
        let p = parents[cur].unwrap();
        proof { assert(pm[cur as int] == Some(p as int)); }
        rev.push(p);
        cur = p;
    }

    // cur is now the root
    proof {
        assert(pm[cur as int] == None::<int>);
    }

    // Phase 2: reverse into path (root→node order)
    let rlen = rev.len();
    let mut path: Vec<usize> = Vec::new();
    let mut i: usize = rlen;
    while i > 0
        invariant
            0 <= i <= rlen,
            rlen == rev@.len(),
            path@.len() == rlen - i,
            forall|k: int| 0 <= k < path@.len()
                ==> path@[k] == rev@[rlen - 1 - k],
        decreases i,
    {
        i -= 1;
        path.push(rev[i]);
    }

    proof {
        // Connect reversed vec to spec via induction on rev
        ancestor_path_from_rev(pm, node as int, rev@, path@);
    }
    path
}

/// Inductive proof: if rev = [node, p1, ..., root] with correct parent links,
/// then reversing rev gives ancestor_path_spec(pm, node).
proof fn ancestor_path_from_rev(
    pm: Seq<Option<int>>, node: int,
    rev: Seq<usize>, path: Seq<usize>,
)
    requires
        well_formed_spec(pm),
        0 <= node < pm.len(),
        rev.len() > 0,
        rev[0] == node,
        // Last element is root
        pm[rev[rev.len() - 1] as int] == None::<int>,
        0 <= (rev[rev.len() - 1] as int) < pm.len(),
        // All in bounds
        forall|k: int| #![trigger rev[k]] 0 <= k < rev.len()
            ==> 0 <= (rev[k] as int) < pm.len(),
        // Parent chain
        forall|k: int| #![trigger rev[k]] 0 <= k < rev.len() as int - 1
            ==> pm[rev[k] as int] == Some(rev[k + 1] as int),
        // path is reverse of rev
        path.len() == rev.len(),
        forall|k: int| 0 <= k < path.len()
            ==> path[k] == rev[rev.len() - 1 - k],
    ensures
        path.map(|_i: int, v: usize| v as int)
            =~= ancestor_path_spec(pm, node),
    decreases rev.len(),
{
    let path_int = path.map(|_i: int, v: usize| v as int);
    let spec = ancestor_path_spec(pm, node);
    ancestor_path_unfold(pm, node);

    if rev.len() == 1 {
        // rev = [node], node is root, path = [node]
        assert(pm[node] == None::<int>);
        assert(spec =~= seq![node]);
        assert(path[0] == node);
        assert(path_int =~= seq![node as int]);
    } else {
        // rev = [node, p, ...], path = [..., p, node]
        let p = rev[1] as int;
        assert(pm[node] == Some(p));
        // spec = ancestor_path_spec(pm, p).push(node)
        // path = [root, ..., p, node]
        // path[0..len-1] = [root, ..., p] which should be ancestor_path_spec(pm, p)

        // Build sub_rev = rev[1..] and sub_path = path[0..len-1]
        let sub_rev = rev.subrange(1, rev.len() as int);
        let sub_path = path.subrange(0, path.len() as int - 1);

        // sub_rev has the same structure for node p
        assert(sub_rev[0] == rev[1]);
        assert(sub_rev.len() == rev.len() - 1);
        assert(sub_rev[sub_rev.len() - 1] == rev[rev.len() - 1]);

        // sub_rev parent chain
        assert forall|k: int| #![trigger sub_rev[k]] 0 <= k < sub_rev.len() as int - 1
            implies pm[sub_rev[k] as int] == Some(sub_rev[k + 1] as int)
        by {
            assert(sub_rev[k] == rev[k + 1]);
            assert(sub_rev[k + 1] == rev[k + 2]);
        }

        // sub_rev bounds
        assert forall|k: int| #![trigger sub_rev[k]] 0 <= k < sub_rev.len()
            implies 0 <= (sub_rev[k] as int) < pm.len()
        by {
            assert(sub_rev[k] == rev[k + 1]);
        }

        // sub_path is reverse of sub_rev
        assert forall|k: int| 0 <= k < sub_path.len()
            implies sub_path[k] == sub_rev[sub_rev.len() - 1 - k]
        by {
            assert(sub_path[k] == path[k]);
            assert(path[k] == rev[rev.len() - 1 - k]);
            assert(rev[rev.len() - 1 - k] == sub_rev[rev.len() - 2 - k]);
            assert(sub_rev.len() - 1 - k == rev.len() - 2 - k);
        }

        // Recurse on sub problem
        ancestor_path_from_rev(pm, p, sub_rev, sub_path);
        // Now: sub_path.map(int) =~= ancestor_path_spec(pm, p)

        // path = sub_path ++ [node]
        // path_int = sub_path.map(int) ++ [node as int]
        //          = ancestor_path_spec(pm, p) ++ [node as int]
        //          = ancestor_path_spec(pm, p).push(node as int)
        //          = spec
        let sub_int = sub_path.map(|_i: int, v: usize| v as int);
        assert(path_int =~= sub_int.push(node as int));
    }
}

// ============================================================
// Ancestor path properties
// ============================================================

// r[verify forest.ancestor-path]
/// Path contains the target node as last element.
proof fn path_contains_node(parents: Seq<Option<int>>, node: int)
    requires well_formed_spec(parents), 0 <= node < parents.len()
    ensures ancestor_path_spec(parents, node).len() > 0,
        ancestor_path_spec(parents, node)[
            ancestor_path_spec(parents, node).len() as int - 1
        ] == node,
    decreases node,
{
    ancestor_path_unfold(parents, node);
    match parents[node] {
        Some(p) => {
            path_contains_node(parents, p);
        }
        None => {}
    }
}

// r[verify forest.ancestor-path]
/// Path first element is a root.
proof fn path_root_anchored(parents: Seq<Option<int>>, node: int)
    requires well_formed_spec(parents), 0 <= node < parents.len()
    ensures is_root_spec(parents, ancestor_path_spec(parents, node)[0]),
    decreases node,
{
    ancestor_path_unfold(parents, node);
    match parents[node] {
        Some(p) => {
            path_root_anchored(parents, p);
            let parent_path = ancestor_path_spec(parents, p);
            assert(ancestor_path_spec(parents, node) =~= parent_path.push(node));
            assert(ancestor_path_spec(parents, node)[0] == parent_path[0]);
        }
        None => {
            assert(ancestor_path_spec(parents, node) =~= seq![node]);
        }
    }
}

// r[verify forest.ancestor-path]
/// Path consecutive elements have parent→child relationship.
proof fn path_valid(parents: Seq<Option<int>>, node: int)
    requires well_formed_spec(parents), 0 <= node < parents.len()
    ensures ({
        let path = ancestor_path_spec(parents, node);
        forall|k: int| 0 < k < path.len() ==>
            #[trigger] parents[path[k]] == Some(path[k - 1])
    })
    decreases node,
{
    ancestor_path_unfold(parents, node);
    match parents[node] {
        Some(p) => {
            ancestor_path_unfold(parents, p);
            path_valid(parents, p);
            let parent_path = ancestor_path_spec(parents, p);
            let path = ancestor_path_spec(parents, node);
            assert(path =~= parent_path.push(node));
            assert forall|k: int| 0 < k < path.len() implies
                #[trigger] parents[path[k]] == Some(path[k - 1])
            by {
                if k < parent_path.len() as int {
                    assert(path[k] == parent_path[k]);
                    assert(path[k - 1] == parent_path[k - 1]);
                    // From IH on parent
                    assert(parents[parent_path[k]] == Some(parent_path[k - 1]));
                } else {
                    // k == parent_path.len(), so path[k] = node
                    assert(path[k] == node);
                    assert(path[k - 1] == parent_path[parent_path.len() as int - 1]);
                    path_contains_node(parents, p);
                    assert(parent_path[parent_path.len() as int - 1] == p);
                    assert(parents[node] == Some(p));
                }
            }
        }
        None => {}
    }
}

// r[verify forest.ancestor-path]
/// Path length is bounded by the number of nodes.
proof fn path_length_bound(parents: Seq<Option<int>>, node: int)
    requires well_formed_spec(parents), 0 <= node < parents.len()
    ensures ancestor_path_spec(parents, node).len() <= node + 1
    decreases node,
{
    ancestor_path_unfold(parents, node);
    match parents[node] {
        Some(p) => {
            path_length_bound(parents, p);
            // IH: path(p).len() <= p + 1
            // path(node) = path(p).push(node), so len = path(p).len() + 1 <= p + 2 <= node + 1
        }
        None => {
            // path(node) = [node], len = 1 <= node + 1
        }
    }
}

// r[verify forest.ancestor-path]
/// All elements in the path are valid indices.
proof fn path_elements_in_bounds(parents: Seq<Option<int>>, node: int)
    requires well_formed_spec(parents), 0 <= node < parents.len()
    ensures forall|k: int| 0 <= k < ancestor_path_spec(parents, node).len() ==>
        (#[trigger] ancestor_path_spec(parents, node)[k] >= 0
        && ancestor_path_spec(parents, node)[k] < parents.len()),
    decreases node,
{
    ancestor_path_unfold(parents, node);
    match parents[node] {
        Some(p) => {
            path_elements_in_bounds(parents, p);
        }
        None => {}
    }
}

// ============================================================
// depth
// ============================================================

// r[depends forest.depth]
/// Spec: depth of a node is the length of its ancestor path minus 1.
pub open spec fn depth_spec(parents: Seq<Option<int>>, node: int) -> int {
    ancestor_path_spec(parents, node).len() as int - 1
}

// r[impl forest.depth]
/// Compute depth by counting parent-pointer hops.
pub fn depth(parents: &[Option<usize>], node: usize) -> (result: usize)
    requires
        node < parents@.len(),
        well_formed_spec(parents@.map(|_i: int, p: Option<usize>| match p {
            Some(j) => Some(j as int),
            None => None::<int>,
        })),
    ensures result as int == depth_spec(
        parents@.map(|_i: int, p: Option<usize>| match p {
            Some(j) => Some(j as int),
            None => None::<int>,
        }),
        node as int,
    ),
{
    let ghost pm = parents@.map(|_i: int, p: Option<usize>| match p {
        Some(j) => Some(j as int),
        None => None::<int>,
    });
    let mut d: usize = 0;
    let mut cur: usize = node;
    while parents[cur].is_some()
        invariant
            cur < parents@.len(),
            pm =~= parents@.map(|_i: int, p: Option<usize>| match p {
                Some(j) => Some(j as int),
                None => None::<int>,
            }),
            well_formed_spec(pm),
            d as int + depth_spec(pm, cur as int) == depth_spec(pm, node as int),
            d as int <= node as int,
        decreases cur,
    {
        proof {
            ancestor_path_unfold(pm, cur as int);
            // depth_spec(pm, cur) = ancestor_path_spec(pm, cur).len() - 1
            // After unfold: path(cur) = path(p).push(cur), so len = path(p).len() + 1
            // depth(cur) = path(p).len() = depth(p) + 1
            // So: d + depth(p) + 1 = depth(node), meaning d + 1 <= depth(node) <= node
            path_length_bound(pm, node as int);
        }
        let p = parents[cur].unwrap();
        d += 1;
        cur = p;
    }
    proof { ancestor_path_unfold(pm, cur as int); }
    d
}

// r[verify forest.depth]
/// Root has depth 0.
proof fn root_depth_zero(parents: Seq<Option<int>>, node: int)
    requires well_formed_spec(parents), is_root_spec(parents, node)
    ensures depth_spec(parents, node) == 0
{}

// ============================================================
// is_ancestor
// ============================================================

// r[depends forest.is-ancestor]
/// Spec: `a` is an ancestor of `d` if `a` appears in `ancestor_path(d)`.
pub open spec fn is_ancestor_spec(
    parents: Seq<Option<int>>, a: int, d: int,
) -> bool {
    exists|k: int| 0 <= k < ancestor_path_spec(parents, d).len()
        && ancestor_path_spec(parents, d)[k] == a
}

// r[verify forest.is-ancestor]
/// Reflexive: every node is its own ancestor.
proof fn is_ancestor_reflexive(parents: Seq<Option<int>>, node: int)
    requires well_formed_spec(parents), 0 <= node < parents.len()
    ensures is_ancestor_spec(parents, node, node)
{
    path_contains_node(parents, node);
    let path = ancestor_path_spec(parents, node);
    assert(path[path.len() as int - 1] == node);
}

// ============================================================
// lca — least common ancestor
// ============================================================

// r[depends forest.lca]
/// Spec: LCA is the last element in the common prefix of two ancestor paths.
pub open spec fn lca_spec(
    parents: Seq<Option<int>>, a: int, b: int,
) -> int {
    let path_a = ancestor_path_spec(parents, a);
    let path_b = ancestor_path_spec(parents, b);
    let common_len = common_prefix_len(path_a, path_b);
    if common_len > 0 {
        path_a[common_len - 1]
    } else {
        -1 // no common ancestor (different roots)
    }
}

/// Spec: length of the common prefix of two sequences.
pub open spec fn common_prefix_len(a: Seq<int>, b: Seq<int>) -> int
    decreases if a.len() < b.len() { a.len() as int } else { b.len() as int },
{
    common_prefix_from(a, b, 0)
}

/// Helper: common prefix length starting from index `from`.
pub open spec fn common_prefix_from(a: Seq<int>, b: Seq<int>, from: int) -> int
    decreases if from < a.len() as int && from < b.len() as int
        { (a.len() - from) as int } else { 0 },
{
    if from >= a.len() || from >= b.len() || a[from] != b[from] {
        from
    } else {
        common_prefix_from(a, b, from + 1)
    }
}

// r[impl forest.lca]
/// Compute LCA by walking both ancestor paths and finding the last common element.
pub fn lca(parents: &[Option<usize>], a: usize, b: usize) -> (result: Option<usize>)
    requires
        a < parents@.len(),
        b < parents@.len(),
        well_formed_spec(parents@.map(|_i: int, p: Option<usize>| match p {
            Some(j) => Some(j as int),
            None => None::<int>,
        })),
    ensures ({
        let pm = parents@.map(|_i: int, p: Option<usize>| match p {
            Some(j) => Some(j as int),
            None => None::<int>,
        });
        let lca_val = lca_spec(pm, a as int, b as int);
        match result {
            Some(v) => v as int == lca_val && lca_val >= 0,
            None => lca_val < 0,
        }
    })
{
    let path_a = ancestor_path(parents, a);
    let path_b = ancestor_path(parents, b);

    // Find common prefix length (no break — explicit stop condition)
    let min_len = if path_a.len() < path_b.len() { path_a.len() } else { path_b.len() };
    let mut common: usize = 0;
    let mut found_diff: bool = false;
    while common < min_len && !found_diff
        invariant
            0 <= common <= min_len,
            min_len <= path_a@.len(),
            min_len <= path_b@.len(),
            forall|k: int| 0 <= k < common as int ==> path_a@[k] == path_b@[k],
            found_diff ==> common < min_len && path_a@[common as int] != path_b@[common as int],
        decreases min_len - common + if found_diff { 0usize } else { 1usize },
    {
        if path_a[common] != path_b[common] {
            found_diff = true;
        } else {
            common += 1;
        }
    }

    proof {
        let pm = parents@.map(|_i: int, p: Option<usize>| match p {
            Some(j) => Some(j as int),
            None => None::<int>,
        });
        let pa = path_a@.map(|_i: int, v: usize| v as int);
        let pb = path_b@.map(|_i: int, v: usize| v as int);
        assert forall|k: int| 0 <= k < common as int implies pa[k] == pb[k]
        by {
            assert(pa[k] == path_a@[k] as int);
            assert(pb[k] == path_b@[k] as int);
            assert(path_a@[k] == path_b@[k]);
        }
        if found_diff {
            assert(pa[common as int] == path_a@[common as int] as int);
            assert(pb[common as int] == path_b@[common as int] as int);
            assert(pa[common as int] != pb[common as int]);
        }
        common_prefix_from_matches(pa, pb, 0, common as int);
    }

    if common > 0 {
        Some(path_a[common - 1])
    } else {
        None
    }
}

/// Helper: connect iterative common prefix scan to recursive spec.
proof fn common_prefix_from_matches(a: Seq<int>, b: Seq<int>, from: int, to: int)
    requires
        0 <= from <= to,
        to <= a.len(),
        to <= b.len(),
        forall|k: int| from <= k < to ==> a[k] == b[k],
        to >= a.len() || to >= b.len() || a[to] != b[to],
    ensures common_prefix_from(a, b, from) == to
    decreases to - from,
{
    if from == to {
    } else {
        assert(a[from] == b[from]);
        common_prefix_from_matches(a, b, from + 1, to);
    }
}

// r[verify forest.lca]
/// LCA is symmetric.
proof fn lca_symmetric(parents: Seq<Option<int>>, a: int, b: int)
    requires well_formed_spec(parents), 0 <= a < parents.len(), 0 <= b < parents.len()
    ensures lca_spec(parents, a, b) == lca_spec(parents, b, a)
{
    let pa = ancestor_path_spec(parents, a);
    let pb = ancestor_path_spec(parents, b);
    common_prefix_symmetric(pa, pb);
    // common_prefix_len(pa, pb) == common_prefix_len(pb, pa)
    let cl_ab = common_prefix_len(pa, pb);
    let cl_ba = common_prefix_len(pb, pa);
    assert(cl_ab == cl_ba);
    // If cl > 0, lca_spec(a,b) = pa[cl-1], lca_spec(b,a) = pb[cl-1]
    // But pa[i] == pb[i] for i < cl (by common prefix definition)
    if cl_ab > 0 {
        common_prefix_implies_equal(pa, pb, cl_ab - 1);
    }
}

/// Elements within the common prefix are equal.
proof fn common_prefix_implies_equal(a: Seq<int>, b: Seq<int>, i: int)
    requires 0 <= i < common_prefix_len(a, b)
    ensures a[i] == b[i]
{
    common_prefix_from_implies_equal(a, b, 0, i);
}

proof fn common_prefix_from_implies_equal(a: Seq<int>, b: Seq<int>, from: int, i: int)
    requires 0 <= from <= i, i < common_prefix_from(a, b, from),
        from <= a.len(), from <= b.len()
    ensures a[i] == b[i]
    decreases i - from,
{
    // common_prefix_from(a, b, from) > i >= from means a[from] == b[from]
    // and common_prefix_from(a, b, from+1) > i
    if from == i {
        // a[from] == b[from] because common_prefix_from > from means they match
    } else {
        common_prefix_from_implies_equal(a, b, from + 1, i);
    }
}

/// Common prefix length is symmetric.
proof fn common_prefix_symmetric(a: Seq<int>, b: Seq<int>)
    ensures common_prefix_len(a, b) == common_prefix_len(b, a)
{
    common_prefix_from_symmetric(a, b, 0);
}

proof fn common_prefix_from_symmetric(a: Seq<int>, b: Seq<int>, from: int)
    requires 0 <= from
    ensures common_prefix_from(a, b, from) == common_prefix_from(b, a, from)
    decreases if from < a.len() as int && from < b.len() as int
        { (a.len() - from) as int } else { 0 },
{
    if from < a.len() as int && from < b.len() as int && a[from] == b[from] {
        common_prefix_from_symmetric(a, b, from + 1);
    }
}

// r[verify forest.lca]
/// LCA with self is self.
proof fn lca_self(parents: Seq<Option<int>>, a: int)
    requires well_formed_spec(parents), 0 <= a < parents.len()
    ensures lca_spec(parents, a, a) == a
{
    let path = ancestor_path_spec(parents, a);
    path_contains_node(parents, a);
    // path == path, so common prefix is the full path
    common_prefix_full(path);
    assert(common_prefix_len(path, path) == path.len() as int);
    assert(path[path.len() as int - 1] == a);
}

/// Common prefix of a sequence with itself is the full length.
proof fn common_prefix_full(s: Seq<int>)
    ensures common_prefix_len(s, s) == s.len() as int
{
    common_prefix_from_full(s, 0);
}

proof fn common_prefix_from_full(s: Seq<int>, from: int)
    requires 0 <= from <= s.len()
    ensures common_prefix_from(s, s, from) == s.len() as int
    decreases (s.len() - from),
{
    if from < s.len() as int {
        common_prefix_from_full(s, from + 1);
    }
}

// ============================================================
// leaves
// ============================================================

// r[depends forest.leaves]
/// Spec: all leaf nodes, scanning from index `from`.
pub open spec fn leaves_from_spec(parents: Seq<Option<int>>, from: int) -> Seq<int>
    decreases if from < parents.len() as int { (parents.len() - from) as int } else { 0 },
{
    if from >= parents.len() {
        seq![]
    } else if is_leaf_spec(parents, from) {
        seq![from] + leaves_from_spec(parents, from + 1)
    } else {
        leaves_from_spec(parents, from + 1)
    }
}

// r[depends forest.leaves]
/// Spec: all leaf nodes.
pub open spec fn leaves_spec(parents: Seq<Option<int>>) -> Seq<int> {
    leaves_from_spec(parents, 0)
}

// r[impl forest.leaves]
/// Collect all leaf nodes.
pub fn leaves(parents: &[Option<usize>]) -> (result: Vec<usize>)
    ensures ({
        let pm = parents@.map(|_i: int, p: Option<usize>| match p {
            Some(j) => Some(j as int),
            None => None::<int>,
        });
        result@.map(|_i: int, v: usize| v as int) =~= leaves_spec(pm)
    })
{
    let ghost pm = parents@.map(|_i: int, p: Option<usize>| match p {
        Some(j) => Some(j as int),
        None => None::<int>,
    });

    // Count children for each node
    let mut has_child: Vec<bool> = Vec::new();
    let mut k: usize = 0;
    while k < parents.len()
        invariant
            0 <= k <= parents@.len(),
            has_child@.len() == k as int,
            forall|j: int| 0 <= j < k as int ==> has_child@[j] == false,
        decreases parents.len() - k,
    {
        has_child.push(false);
        k += 1;
    }

    let mut j: usize = 0;
    while j < parents.len()
        invariant
            0 <= j <= parents@.len(),
            has_child@.len() == parents@.len(),
            pm =~= parents@.map(|_i: int, p: Option<usize>| match p {
                Some(j) => Some(j as int),
                None => None::<int>,
            }),
            // has_child[x] => some node in [0, j) has x as parent
            forall|x: int| 0 <= x < parents@.len() ==> (
                has_child@[x] == true ==> exists|c: int| 0 <= c < j as int && pm[c] == Some(x)
            ),
            // if node c < j has parent x, then has_child[x] == true
            forall|c: int| #![trigger pm[c]] 0 <= c < j as int ==> match pm[c] {
                Some(p) => 0 <= p < parents@.len() ==> has_child@[p] == true,
                None => true,
            },
        decreases parents.len() - j,
    {
        match parents[j] {
            Some(p) => {
                if p < parents.len() {
                    has_child.set(p, true);
                    proof {
                        assert(pm[j as int] == Some(p as int));
                    }
                }
            }
            None => {}
        }
        j += 1;
    }

    // Establish biconditional: has_child[x] <==> !is_leaf_spec(pm, x)
    proof {
        assert forall|x: int| 0 <= x < parents@.len() implies
            (has_child@[x] == true <==> !is_leaf_spec(pm, x))
        by {
            if has_child@[x] {
                // Forward: has_child[x] => exists c with pm[c] == Some(x) => !is_leaf
                let c = choose|c: int| 0 <= c < parents@.len() && pm[c] == Some(x);
                assert(pm[c] == Some(x));
                // is_leaf requires forall j: pm[j] != Some(x), but pm[c] == Some(x)
            }
            if !is_leaf_spec(pm, x) {
                // Backward: !is_leaf => exists j: pm[j] == Some(x)
                let j = choose|j: int| #![trigger pm[j]] 0 <= j < pm.len() && pm[j] == Some(x);
                // j < parents.len(), and second loop invariant says has_child[x] == true
                assert(pm[j] == Some(x));
                assert(0 <= x < parents@.len());
                assert(has_child@[x] == true);
            }
        }
    }

    // Collect nodes with no children
    let mut out: Vec<usize> = Vec::new();
    let mut i: usize = 0;
    while i < parents.len()
        invariant
            0 <= i <= parents@.len(),
            has_child@.len() == parents@.len(),
            pm =~= parents@.map(|_i: int, p: Option<usize>| match p {
                Some(j) => Some(j as int),
                None => None::<int>,
            }),
            forall|x: int| 0 <= x < parents@.len() ==> (
                has_child@[x] == true <==> !is_leaf_spec(pm, x)
            ),
            out@.map(|_i: int, v: usize| v as int) + leaves_from_spec(pm, i as int)
                =~= leaves_from_spec(pm, 0),
        decreases parents.len() - i,
    {
        if !has_child[i] {
            proof {
                assert(is_leaf_spec(pm, i as int));
            }
            out.push(i);
        }
        i += 1;
    }
    out
}

// ============================================================
// subtree — forward scan
// ============================================================

// r[depends forest.subtree]
/// Spec: is node `n` reachable from `root` via child pointers?
/// Fuel-bounded to guarantee termination without well_formed in `when`.
pub open spec fn reachable_fuel(
    parents: Seq<Option<int>>, root: int, n: int, fuel: nat,
) -> bool
    decreases fuel,
{
    if n == root {
        true
    } else if fuel == 0 || n < 0 || n >= parents.len() {
        false
    } else {
        match parents[n] {
            Some(p) => reachable_fuel(parents, root, p, (fuel - 1) as nat),
            None => false,
        }
    }
}

/// Top-level reachable spec with sufficient fuel.
pub open spec fn reachable_spec(
    parents: Seq<Option<int>>, root: int, n: int,
) -> bool {
    reachable_fuel(parents, root, n, (if n >= 0 { n + 1 } else { 0 }) as nat)
}

/// Under well_formed, reachable_spec unfolds one step.
proof fn reachable_unfold(parents: Seq<Option<int>>, root: int, n: int)
    requires well_formed_spec(parents), 0 <= n < parents.len()
    ensures reachable_spec(parents, root, n) == if n == root { true } else {
        match parents[n] {
            Some(p) => reachable_spec(parents, root, p),
            None => false,
        }
    }
    decreases n,
{
    if n == root {
        // Base: reachable_fuel(parents, root, root, fuel) = true for any fuel > 0
    } else {
        match parents[n] {
            Some(p) => {
                // well_formed: 0 <= p < n
                // reachable_spec(parents, root, n) = reachable_fuel(parents, root, n, n+1)
                // Unfolds to: reachable_fuel(parents, root, p, n)
                // Need: reachable_fuel(parents, root, p, n) == reachable_fuel(parents, root, p, p+1)
                reachable_fuel_sufficient(parents, root, p, n as nat);
            }
            None => {}
        }
    }
}

/// Extra fuel beyond n+1 doesn't change the result under well_formed.
proof fn reachable_fuel_sufficient(parents: Seq<Option<int>>, root: int, n: int, fuel: nat)
    requires well_formed_spec(parents), 0 <= n < parents.len(), fuel >= n + 1
    ensures reachable_fuel(parents, root, n, fuel) ==
        reachable_fuel(parents, root, n, (n + 1) as nat)
    decreases n,
{
    if n == root {
    } else {
        match parents[n] {
            Some(p) => {
                reachable_fuel_sufficient(parents, root, p, (fuel - 1) as nat);
                reachable_fuel_sufficient(parents, root, p, n as nat);
            }
            None => {}
        }
    }
}

/// If parent is reachable, child is reachable.
proof fn reachable_child(parents: Seq<Option<int>>, root: int, n: int)
    requires
        well_formed_spec(parents),
        0 <= n < parents.len(),
        n != root,
        parents[n].is_some(),
        reachable_spec(parents, root, parents[n].unwrap()),
    ensures reachable_spec(parents, root, n)
{
    reachable_unfold(parents, root, n);
}

/// If reachable and not root, parent must be reachable.
proof fn reachable_parent(parents: Seq<Option<int>>, root: int, n: int)
    requires
        well_formed_spec(parents),
        0 <= n < parents.len(),
        n != root,
        reachable_spec(parents, root, n),
    ensures parents[n].is_some() && reachable_spec(parents, root, parents[n].unwrap())
{
    reachable_unfold(parents, root, n);
}

// r[depends forest.subtree]
/// Spec: collect all reachable nodes from `root`, from index `from`.
pub open spec fn subtree_from_spec(
    parents: Seq<Option<int>>, root: int, from: int,
) -> Seq<int>
    decreases if from < parents.len() as int { (parents.len() - from) as int } else { 0 },
{
    if from >= parents.len() {
        seq![]
    } else if reachable_spec(parents, root, from) {
        seq![from] + subtree_from_spec(parents, root, from + 1)
    } else {
        subtree_from_spec(parents, root, from + 1)
    }
}

// r[depends forest.subtree]
/// Spec: all nodes in the subtree rooted at `root`.
pub open spec fn subtree_spec(parents: Seq<Option<int>>, root: int) -> Seq<int> {
    subtree_from_spec(parents, root, 0)
}

// r[impl forest.subtree]
/// Collect subtree via forward scan with push-based reachability.
/// Builds the marked array via push (not set) to avoid `Vec::set` quantifier issues.
pub fn subtree(parents: &[Option<usize>], root: usize) -> (result: Vec<usize>)
    requires
        root < parents@.len(),
        well_formed_spec(parents@.map(|_i: int, p: Option<usize>| match p {
            Some(j) => Some(j as int),
            None => None::<int>,
        })),
    ensures ({
        let pm = parents@.map(|_i: int, p: Option<usize>| match p {
            Some(j) => Some(j as int),
            None => None::<int>,
        });
        result@.map(|_i: int, v: usize| v as int) =~= subtree_spec(pm, root as int)
    })
{
    let ghost pm = parents@.map(|_i: int, p: Option<usize>| match p {
        Some(j) => Some(j as int),
        None => None::<int>,
    });

    // Single pass: build marked array AND result vec simultaneously via push.
    // For each index i from 0 to len-1, determine if reachable and push
    // marked[i] = true/false. Since well_formed guarantees parent < child,
    // when we process i, all potential ancestors are already in marked.
    let mut marked: Vec<bool> = Vec::new();
    let mut out: Vec<usize> = Vec::new();
    let mut i: usize = 0;

    while i < parents.len()
        invariant
            0 <= i <= parents@.len(),
            marked@.len() == i as int,
            pm =~= parents@.map(|_i: int, p: Option<usize>| match p {
                Some(j) => Some(j as int),
                None => None::<int>,
            }),
            well_formed_spec(pm),
            // marked[n] == reachable for all n < i
            forall|n: int| #![trigger marked@[n]] 0 <= n < i as int ==>
                (marked@[n] == reachable_spec(pm, root as int, n)),
            // out tracks subtree_from_spec up to i
            out@.map(|_i: int, v: usize| v as int) + subtree_from_spec(pm, root as int, i as int)
                =~= subtree_from_spec(pm, root as int, 0),
        decreases parents.len() - i,
    {
        let is_reachable: bool;
        if i == root {
            is_reachable = true;
        } else {
            match parents[i] {
                Some(p) => {
                    proof {
                        assert(pm[i as int] == Some(p as int));
                        assert(0 <= (p as int) < (i as int));
                    }
                    // Parent already processed (p < i by well_formed)
                    is_reachable = marked[p];
                    proof {
                        assert(marked@[p as int] == reachable_spec(pm, root as int, p as int));
                        reachable_unfold(pm, root as int, i as int);
                        // reachable(i) == reachable(p) when parents[i] = Some(p) and i != root
                    }
                }
                None => {
                    is_reachable = false;
                    proof {
                        reachable_unfold(pm, root as int, i as int);
                        // parents[i] == None, i != root => !reachable
                    }
                }
            }
        }

        marked.push(is_reachable);

        if is_reachable {
            out.push(i);
        }

        i += 1;
    }

    out
}

// r[verify forest.subtree]
/// Subtree always contains the root.
proof fn subtree_contains_root(parents: Seq<Option<int>>, root: int)
    requires 0 <= root < parents.len()
    ensures reachable_spec(parents, root, root)
{
    // reachable_spec(parents, root, root) unfolds to
    // reachable_fuel(parents, root, root, fuel) which hits n == root => true
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // Helper to build pm mapping for tests
    #[allow(dead_code)]
    fn to_pm(parents: &[Option<usize>]) -> Vec<Option<usize>> {
        parents.to_vec()
    }

    // r[verify forest.well-formedness]
    #[test]
    fn test_well_formed() {
        assert!(well_formed(&[None, Some(0), Some(0), Some(1)]));
        assert!(!well_formed(&[Some(0)])); // self-ref
        assert!(!well_formed(&[Some(1), None])); // forward ref
        assert!(well_formed(&[None]));
        assert!(well_formed(&[]));
    }

    // r[verify forest.root-leaf]
    #[test]
    fn test_is_leaf() {
        let p = [None, Some(0), Some(0), Some(1)];
        assert!(!is_leaf(&p, 0)); // has children 1, 2
        assert!(!is_leaf(&p, 1)); // has child 3
        assert!(is_leaf(&p, 2)); // no children
        assert!(is_leaf(&p, 3)); // no children
    }

    // r[verify forest.ancestor-path]
    #[test]
    fn test_ancestor_path() {
        let p = [None, Some(0), Some(0), Some(2)];
        let path = ancestor_path(&p, 3);
        assert_eq!(path, vec![0, 2, 3]);

        let path = ancestor_path(&p, 0);
        assert_eq!(path, vec![0]);

        let path = ancestor_path(&p, 1);
        assert_eq!(path, vec![0, 1]);
    }

    // r[verify forest.depth]
    #[test]
    fn test_depth() {
        let p = [None, Some(0), Some(1), Some(2)];
        assert_eq!(depth(&p, 0), 0);
        assert_eq!(depth(&p, 1), 1);
        assert_eq!(depth(&p, 2), 2);
        assert_eq!(depth(&p, 3), 3);
    }

    // r[verify forest.lca]
    #[test]
    fn test_lca() {
        // Fork at root
        let p = [None, Some(0), Some(0)];
        assert_eq!(lca(&p, 1, 2), Some(0));

        // Fork mid-chain
        let p = [None, Some(0), Some(1), Some(1)];
        assert_eq!(lca(&p, 2, 3), Some(1));

        // Same node
        let p = [None, Some(0), Some(1)];
        assert_eq!(lca(&p, 2, 2), Some(2));

        // Ancestor-descendant
        assert_eq!(lca(&p, 1, 2), Some(1));
    }

    // r[verify forest.leaves]
    #[test]
    fn test_leaves() {
        let p = [None, Some(0), Some(0), Some(1)];
        let l = leaves(&p);
        assert_eq!(l, vec![2, 3]);

        let p2: [Option<usize>; 1] = [None];
        let l2 = leaves(&p2);
        assert_eq!(l2, vec![0]); // single node is both root and leaf
    }

    // r[verify forest.subtree]
    #[test]
    fn test_subtree() {
        let p = [None, Some(0), Some(1), Some(1), Some(0)];
        // Subtree of node 1: 1, 2, 3
        let s = subtree(&p, 1);
        assert_eq!(s, vec![1, 2, 3]);

        // Subtree of leaf: just itself
        let s = subtree(&p, 4);
        assert_eq!(s, vec![4]);

        // Subtree of root: everything
        let s = subtree(&p, 0);
        assert_eq!(s, vec![0, 1, 2, 3, 4]);
    }

    // Generate random well-formed parent maps
    fn well_formed_parents(max_size: usize) -> impl Strategy<Value = Vec<Option<usize>>> {
        (1..=max_size).prop_flat_map(|n| {
            proptest::collection::vec(proptest::option::of(0usize..100), n..=n).prop_map(move |raw| {
                raw.into_iter()
                    .enumerate()
                    .map(|(i, opt)| {
                        if i == 0 {
                            None // first node is always root
                        } else {
                            match opt {
                                Some(v) => Some(v % i), // ensure parent < i
                                None => None,
                            }
                        }
                    })
                    .collect()
            })
        })
    }

    proptest! {
        // r[verify forest.well-formedness]
        #[test]
        fn prop_generated_is_well_formed(parents in well_formed_parents(20)) {
            prop_assert!(well_formed(&parents));
        }

        // r[verify forest.ancestor-path]
        #[test]
        fn prop_path_starts_at_root(parents in well_formed_parents(20)) {
            for node in 0..parents.len() {
                let path = ancestor_path(&parents, node);
                prop_assert!(!path.is_empty());
                prop_assert_eq!(parents[path[0]], None, "path should start at root");
                prop_assert_eq!(*path.last().unwrap(), node, "path should end at node");
            }
        }

        // r[verify forest.ancestor-path]
        #[test]
        fn prop_path_valid_links(parents in well_formed_parents(20)) {
            for node in 0..parents.len() {
                let path = ancestor_path(&parents, node);
                for k in 1..path.len() {
                    prop_assert_eq!(parents[path[k]], Some(path[k - 1]),
                        "path[{}]={} should have parent path[{}]={}", k, path[k], k-1, path[k-1]);
                }
            }
        }

        // r[verify forest.lca]
        #[test]
        fn prop_lca_symmetric(parents in well_formed_parents(20)) {
            if parents.len() >= 2 {
                for a in 0..parents.len().min(5) {
                    for b in 0..parents.len().min(5) {
                        prop_assert_eq!(lca(&parents, a, b), lca(&parents, b, a));
                    }
                }
            }
        }

        // r[verify forest.lca]
        #[test]
        fn prop_lca_on_both_paths(parents in well_formed_parents(20)) {
            if parents.len() >= 2 {
                for a in 0..parents.len().min(5) {
                    for b in 0..parents.len().min(5) {
                        if let Some(l) = lca(&parents, a, b) {
                            let pa = ancestor_path(&parents, a);
                            let pb = ancestor_path(&parents, b);
                            prop_assert!(pa.contains(&l), "lca {} not on path to {}", l, a);
                            prop_assert!(pb.contains(&l), "lca {} not on path to {}", l, b);
                        }
                    }
                }
            }
        }

        // r[verify forest.leaves]
        #[test]
        fn prop_leaves_sound_complete(parents in well_formed_parents(20)) {
            let l = leaves(&parents);
            for &leaf in &l {
                prop_assert!(is_leaf(&parents, leaf), "node {} returned as leaf but has children", leaf);
            }
            for node in 0..parents.len() {
                if is_leaf(&parents, node) {
                    prop_assert!(l.contains(&node), "leaf {} missing from result", node);
                }
            }
        }

        // r[verify forest.subtree]
        #[test]
        fn prop_subtree_contains_root(parents in well_formed_parents(20)) {
            for root in 0..parents.len().min(5) {
                let s = subtree(&parents, root);
                prop_assert!(s.contains(&root), "subtree of {} should contain root", root);
            }
        }

        // r[verify forest.subtree]
        #[test]
        fn prop_subtree_sound(parents in well_formed_parents(20)) {
            for root in 0..parents.len().min(5) {
                let s = subtree(&parents, root);
                // Every node in subtree should have root as ancestor
                for &n in &s {
                    let path = ancestor_path(&parents, n);
                    prop_assert!(path.contains(&root),
                        "node {} in subtree({}) but root not on its path", n, root);
                }
            }
        }
    }
}
