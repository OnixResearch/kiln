//! Verified control-flow graph construction for straight-line instruction streams.
//!
//! The model uses half-open basic-block ranges over a linear instruction slice.
//! Leaders are the first instruction, any branch target, and any instruction
//! immediately following a terminator. Branch targets to instruction 0 are
//! rejected so the entry block has no internal predecessors.

#![allow(clippy::match_same_arms, clippy::single_match)]

use vstd::prelude::*;

verus! {

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstrKind {
    Normal,
    Branch { target: usize },
    ConditionalBranch { target: usize },
    Return,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BasicBlock {
    pub start: usize,
    pub end: usize,
}

// ============================================================
// Specs
// ============================================================

pub open spec fn is_terminator_spec(instr: InstrKind) -> bool {
    match instr {
        InstrKind::Normal => false,
        InstrKind::Branch { .. } | InstrKind::ConditionalBranch { .. } | InstrKind::Return => true,
    }
}

pub open spec fn branch_target_spec(instr: InstrKind) -> Option<int> {
    match instr {
        InstrKind::Branch { target } | InstrKind::ConditionalBranch { target } => Some(target as int),
        _ => None,
    }
}

// r[depends control-flow-graph.valid-program]
/// Spec: all branch targets are in range and skip the entry instruction.
pub open spec fn valid_program_spec(instrs: Seq<InstrKind>) -> bool {
    forall|i: int| #![trigger instrs[i]] 0 <= i < instrs.len() ==> match branch_target_spec(instrs[i]) {
        Some(target) => 0 < target < instrs.len(),
        None => true,
    }
}

pub open spec fn is_branch_target_spec(instrs: Seq<InstrKind>, idx: int) -> bool {
    exists|i: int| #![trigger instrs[i]]
        0 <= i < instrs.len() && branch_target_spec(instrs[i]) == Some(idx)
}

// r[depends control-flow-graph.leader]
/// Spec: leaders are entry, branch targets, and instructions after terminators.
pub open spec fn is_leader_spec(instrs: Seq<InstrKind>, idx: int) -> bool {
    0 <= idx < instrs.len()
        && (
            idx == 0
            || is_branch_target_spec(instrs, idx)
            || (0 < idx && is_terminator_spec(instrs[idx - 1]))
        )
}

pub open spec fn block_valid_spec(instrs: Seq<InstrKind>, block: BasicBlock) -> bool {
    block.start < block.end <= instrs.len()
}

pub open spec fn block_contains_spec(block: BasicBlock, idx: int) -> bool {
    block.start as int <= idx < block.end as int
}

// r[depends control-flow-graph.partition]
/// Spec: block list has contiguous, non-empty shape.
pub open spec fn blocks_shape_spec(instrs: Seq<InstrKind>, blocks: Seq<BasicBlock>) -> bool {
    if instrs.len() == 0 {
        blocks.len() == 0
    } else {
        blocks.len() > 0
            && blocks[0].start == 0
            && blocks[blocks.len() - 1].end as int == instrs.len()
            && forall|i: int| #![trigger blocks[i]] 0 <= i < blocks.len() ==> block_valid_spec(instrs, blocks[i])
            && forall|i: int| #![trigger blocks[i], blocks[i + 1]]
                0 <= i < blocks.len() - 1 ==> blocks[i].end == blocks[i + 1].start
    }
}

pub open spec fn edge_endpoints_valid_spec(blocks: Seq<BasicBlock>, edges: Seq<(usize, usize)>) -> bool {
    forall|i: int| #![trigger edges[i]] 0 <= i < edges.len() ==> {
        let (src, dst) = edges[i];
        src < blocks.len() && dst < blocks.len()
    }
}

// r[depends control-flow-graph.well-formed]
pub open spec fn well_formed_cfg_spec(
    instrs: Seq<InstrKind>,
    blocks: Seq<BasicBlock>,
    edges: Seq<(usize, usize)>,
) -> bool {
    valid_program_spec(instrs)
        && blocks_shape_spec(instrs, blocks)
        && edge_endpoints_valid_spec(blocks, edges)
}

// ============================================================
// Program validation
// ============================================================

// r[impl control-flow-graph.valid-program]
/// Check target-range validity for the instruction stream.
pub fn validate_program(instrs: &[InstrKind]) -> (result: bool)
    ensures result == valid_program_spec(instrs@)
{
    let mut i: usize = 0;
    while i < instrs.len()
        invariant
            0 <= i <= instrs@.len(),
            forall|j: int| #![trigger instrs@[j]] 0 <= j < i as int ==> match branch_target_spec(instrs@[j]) {
                Some(target) => 0 < target < instrs@.len(),
                None => true,
            },
        decreases instrs.len() - i,
    {
        match instrs[i] {
            InstrKind::Branch { target } | InstrKind::ConditionalBranch { target } => {
                if target == 0 || target >= instrs.len() {
                    proof {
                        assert(branch_target_spec(instrs@[i as int]) == Some(target as int));
                    }
                    return false;
                }
            }
            InstrKind::Normal | InstrKind::Return => {}
        }
        i += 1;
    }
    true
}

// ============================================================
// Leader identification
// ============================================================

// r[impl control-flow-graph.leader]
/// Check whether `idx` is a basic-block leader.
pub fn is_leader(instrs: &[InstrKind], idx: usize) -> (result: bool)
    requires
        idx < instrs@.len(),
        valid_program_spec(instrs@),
    ensures
        result == is_leader_spec(instrs@, idx as int),
{
    if idx == 0 {
        return true;
    }
    if is_terminator_runtime(instrs[idx - 1]) {
        proof {
            assert(is_terminator_spec(instrs@[idx as int - 1]));
        }
        return true;
    }

    let mut i: usize = 0;
    while i < instrs.len()
        invariant
            0 <= i <= instrs@.len(),
            idx > 0,
            valid_program_spec(instrs@),
            forall|j: int| #![trigger instrs@[j]] 0 <= j < i as int ==> branch_target_spec(instrs@[j]) != Some(idx as int),
        decreases instrs.len() - i,
    {
        match instrs[i] {
            InstrKind::Branch { target } | InstrKind::ConditionalBranch { target } => {
                if target == idx {
                    proof {
                        assert(branch_target_spec(instrs@[i as int]) == Some(idx as int));
                    }
                    return true;
                }
            }
            InstrKind::Normal | InstrKind::Return => {}
        }
        i += 1;
    }
    false
}

// r[impl control-flow-graph.partition]
/// Collect leaders in increasing index order.
pub fn identify_leaders(instrs: &[InstrKind]) -> (result: Vec<usize>)
    requires valid_program_spec(instrs@)
    ensures
        forall|i: int| #![trigger result@[i]] 0 <= i < result@.len() ==> (
            result@[i] < instrs@.len() && is_leader_spec(instrs@, result@[i] as int)
        ),
        forall|i: int, j: int| #![trigger result@[i], result@[j]] 0 <= i < j < result@.len() ==> result@[i] < result@[j],
        instrs@.len() == 0 ==> result@.len() == 0,
        instrs@.len() > 0 ==> result@.len() > 0 && result@[0] == 0,
{
    let mut marks: Vec<bool> = Vec::new();
    let mut i: usize = 0;
    while i < instrs.len()
        invariant
            0 <= i <= instrs@.len(),
            marks@.len() == i as int,
            forall|j: int| 0 <= j < i as int ==> marks@[j] == false,
        decreases instrs.len() - i,
    {
        marks.push(false);
        i += 1;
    }

    if instrs.len() > 0 {
        marks.set(0, true);
    }

    let mut scan: usize = 0;
    while scan < instrs.len()
        invariant
            0 <= scan <= instrs@.len(),
            marks@.len() == instrs@.len(),
            valid_program_spec(instrs@),
            forall|j: int| #![trigger marks@[j]] 0 <= j < marks@.len() && marks@[j] ==> is_leader_spec(instrs@, j),
            instrs@.len() > 0 ==> marks@[0],
        decreases instrs.len() - scan,
    {
        match instrs[scan] {
            InstrKind::Branch { target } | InstrKind::ConditionalBranch { target } => {
                marks.set(target, true);
                proof {
                    assert(branch_target_spec(instrs@[scan as int]) == Some(target as int));
                    assert(is_branch_target_spec(instrs@, target as int));
                    assert(is_leader_spec(instrs@, target as int));
                }
            }
            InstrKind::Normal | InstrKind::Return => {}
        }
        if scan + 1 < instrs.len() && is_terminator_runtime(instrs[scan]) {
            marks.set(scan + 1, true);
            proof {
                assert(is_terminator_spec(instrs@[scan as int]));
                assert(is_leader_spec(instrs@, scan as int + 1));
            }
        }
        scan += 1;
    }

    let mut out: Vec<usize> = Vec::new();
    let mut k: usize = 0;
    if instrs.len() > 0 {
        out.push(0);
        k = 1;
    }
    while k < marks.len()
        invariant
            0 <= k <= marks@.len(),
            marks@.len() == instrs@.len(),
            forall|j: int| #![trigger marks@[j]] 0 <= j < marks@.len() && marks@[j] ==> is_leader_spec(instrs@, j),
            forall|j: int| #![trigger out@[j]] 0 <= j < out@.len() ==> (
                out@[j] < instrs@.len() && is_leader_spec(instrs@, out@[j] as int)
            ),
            forall|a: int, b: int| #![trigger out@[a], out@[b]] 0 <= a < b < out@.len() ==> out@[a] < out@[b],
            forall|j: int| #![trigger out@[j]] 0 <= j < out@.len() ==> out@[j] < k,
            instrs@.len() == 0 ==> out@.len() == 0,
            instrs@.len() > 0 ==> out@.len() > 0 && out@[0] == 0,
        decreases marks.len() - k,
    {
        if marks[k] {
            out.push(k);
        }
        k += 1;
    }
    out
}

// ============================================================
// Block construction
// ============================================================

// r[impl control-flow-graph.partition]
/// Build half-open basic blocks from the leader set.
pub fn build_blocks(instrs: &[InstrKind]) -> (result: Vec<BasicBlock>)
    requires valid_program_spec(instrs@)
    ensures
        forall|j: int| #![trigger result@[j]] 0 <= j < result@.len() ==> block_valid_spec(instrs@, result@[j]),
        instrs@.len() == 0 ==> result@.len() == 0,
{
    let leaders = identify_leaders(instrs);
    let mut out: Vec<BasicBlock> = Vec::new();
    let mut i: usize = 0;
    while i < leaders.len()
        invariant
            0 <= i <= leaders@.len(),
            valid_program_spec(instrs@),
            forall|a: int, b: int| #![trigger leaders@[a], leaders@[b]] 0 <= a < b < leaders@.len() ==> leaders@[a] < leaders@[b],
            forall|j: int| #![trigger leaders@[j]] 0 <= j < leaders@.len() ==> (
                leaders@[j] < instrs@.len() && is_leader_spec(instrs@, leaders@[j] as int)
            ),
            instrs@.len() == 0 ==> leaders@.len() == 0,
            instrs@.len() > 0 ==> leaders@.len() > 0 && leaders@[0] == 0,
            out@.len() == i as int,
            instrs@.len() == 0 ==> out@.len() == 0,
            instrs@.len() > 0 && i > 0 ==> out@[0].start == 0,
            forall|j: int| #![trigger out@[j]] 0 <= j < out@.len() ==> block_valid_spec(instrs@, out@[j]),
            forall|j: int| #![trigger out@[j], out@[j + 1]] 0 <= j < out@.len() - 1 ==> out@[j].end == out@[j + 1].start,
            0 < i < leaders@.len() ==> out@[i as int - 1].end == leaders@[i as int],
        decreases leaders.len() - i,
    {
        let start = leaders[i];
        let end = if i + 1 < leaders.len() { leaders[i + 1] } else { instrs.len() };
        out.push(BasicBlock { start, end });
        i += 1;
    }
    out
}

// ============================================================
// Edge construction helpers
// ============================================================

fn is_terminator_runtime(instr: InstrKind) -> (result: bool)
    ensures result == is_terminator_spec(instr)
{
    match instr {
        InstrKind::Normal => false,
        InstrKind::Branch { .. } | InstrKind::ConditionalBranch { .. } | InstrKind::Return => true,
    }
}

fn index_of_block_start(blocks: &[BasicBlock], start: usize) -> (result: Option<usize>)
    ensures
        match result {
            Some(idx) => idx < blocks@.len() && blocks@[idx as int].start == start,
            None => forall|i: int| #![trigger blocks@[i]] 0 <= i < blocks@.len() ==> blocks@[i].start != start,
        },
{
    let mut i: usize = 0;
    while i < blocks.len()
        invariant
            0 <= i <= blocks@.len(),
            forall|j: int| #![trigger blocks@[j]] 0 <= j < i as int ==> blocks@[j].start != start,
        decreases blocks.len() - i,
    {
        if blocks[i].start == start {
            return Some(i);
        }
        i += 1;
    }
    None
}

// r[impl control-flow-graph.edges]
/// Build block-to-block edges from block terminators.
pub fn build_edges(instrs: &[InstrKind], blocks: &[BasicBlock]) -> (result: Vec<(usize, usize)>)
    requires
        valid_program_spec(instrs@),
        forall|j: int| #![trigger blocks@[j]] 0 <= j < blocks@.len() ==> block_valid_spec(instrs@, blocks@[j]),
    ensures
        edge_endpoints_valid_spec(blocks@, result@),
{
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut i: usize = 0;
    while i < blocks.len()
        invariant
            0 <= i <= blocks@.len(),
            valid_program_spec(instrs@),
            forall|j: int| #![trigger blocks@[j]] 0 <= j < blocks@.len() ==> block_valid_spec(instrs@, blocks@[j]),
            edge_endpoints_valid_spec(blocks@, out@),
        decreases blocks.len() - i,
    {
        let block = blocks[i];
        let term_idx = block.end - 1;
        match instrs[term_idx] {
            InstrKind::Normal => {
                if i + 1 < blocks.len() {
                    out.push((i, i + 1));
                }
            }
            InstrKind::Branch { target } => {
                let dst = index_of_block_start(blocks, target);
                match dst {
                    Some(j) => { out.push((i, j)); }
                    None => {}
                }
            }
            InstrKind::ConditionalBranch { target } => {
                let dst = index_of_block_start(blocks, target);
                match dst {
                    Some(j) => {
                        out.push((i, j));
                        if i + 1 < blocks.len() && j != i + 1 {
                            out.push((i, i + 1));
                        }
                    }
                    None => {
                        if i + 1 < blocks.len() {
                            out.push((i, i + 1));
                        }
                    }
                }
            }
            InstrKind::Return => {}
        }
        i += 1;
    }
    out
}

// r[impl control-flow-graph.entry]
/// Entry block is block 0 for non-empty CFGs.
pub fn entry_block(blocks: &[BasicBlock]) -> (result: Option<usize>)
    ensures
        match result {
            Some(idx) => idx == 0 && blocks@.len() > 0,
            None => blocks@.len() == 0,
        },
{
    if blocks.is_empty() { None } else { Some(0) }
}

// r[impl control-flow-graph.exit]
/// Check whether a block ends in a return instruction.
pub fn is_exit_block(instrs: &[InstrKind], blocks: &[BasicBlock], block_idx: usize) -> (result: bool)
    requires
        block_idx < blocks@.len(),
        block_valid_spec(instrs@, blocks@[block_idx as int]),
    ensures
        result == match instrs@[blocks@[block_idx as int].end as int - 1] {
            InstrKind::Return => true,
            _ => false,
        },
{
    matches!(instrs[blocks[block_idx].end - 1], InstrKind::Return)
}

// r[impl control-flow-graph.well-formed]
/// Check basic structural CFG invariants.
pub fn check_well_formed(instrs: &[InstrKind], blocks: &[BasicBlock], edges: &[(usize, usize)]) -> (result: bool)
    ensures result ==> valid_program_spec(instrs@) && edge_endpoints_valid_spec(blocks@, edges@)
{
    if !validate_program(instrs) {
        return false;
    }
    if !blocks_shape_spec_runtime(instrs, blocks) {
        return false;
    }
    let mut i: usize = 0;
    while i < edges.len()
        invariant
            0 <= i <= edges@.len(),
            forall|j: int| #![trigger edges@[j]] 0 <= j < i as int ==> {
                let (src, dst) = edges@[j];
                src < blocks@.len() && dst < blocks@.len()
            },
        decreases edges.len() - i,
    {
        let (src, dst) = edges[i];
        if src >= blocks.len() || dst >= blocks.len() {
            return false;
        }
        i += 1;
    }
    true
}

fn blocks_shape_spec_runtime(instrs: &[InstrKind], blocks: &[BasicBlock]) -> (result: bool) {
    if instrs.is_empty() {
        return blocks.is_empty();
    }
    if blocks.is_empty() || blocks[0].start != 0 {
        return false;
    }
    let mut i: usize = 0;
    while i < blocks.len()
        invariant
            0 <= i <= blocks@.len(),
            instrs@.len() > 0,
            blocks@.len() > 0,
        decreases blocks.len() - i,
    {
        let block = blocks[i];
        if !(block.start < block.end && block.end <= instrs.len()) {
            return false;
        }
        if i + 1 < blocks.len() && block.end != blocks[i + 1].start {
            return false;
        }
        i += 1;
    }
    blocks[blocks.len() - 1].end == instrs.len()
}

// ============================================================
// Reachability / dead blocks
// ============================================================

// r[impl control-flow-graph.reachability]
/// Compute reachable blocks from the entry block.
pub fn find_reachable(blocks: &[BasicBlock], edges: &[(usize, usize)]) -> (result: Vec<bool>)
    requires
        edge_endpoints_valid_spec(blocks@, edges@),
        blocks@.len() + edges@.len() <= usize::MAX,
    ensures
        result@.len() == blocks@.len(),
        blocks@.len() > 0 ==> result@[0],
        blocks@.len() == 0 ==> result@.len() == 0,
{
    if blocks.is_empty() {
        Vec::new()
    } else {
        crate::reachability::reachable_from(edges, blocks.len(), 0)
    }
}

// r[impl control-flow-graph.reachability]
/// Collect unreachable block indices.
pub fn find_dead_blocks(blocks: &[BasicBlock], edges: &[(usize, usize)]) -> (result: Vec<usize>)
    requires
        edge_endpoints_valid_spec(blocks@, edges@),
        blocks@.len() + edges@.len() <= usize::MAX,
    ensures
        forall|i: int| #![trigger result@[i]] 0 <= i < result@.len() ==> result@[i] < blocks@.len(),
{
    let reachable = find_reachable(blocks, edges);
    let mut out: Vec<usize> = Vec::new();
    let mut i: usize = 0;
    while i < reachable.len()
        invariant
            0 <= i <= reachable@.len(),
            reachable@.len() == blocks@.len(),
            forall|j: int| #![trigger out@[j]] 0 <= j < out@.len() ==> out@[j] < i,
        decreases reachable.len() - i,
    {
        if !reachable[i] {
            out.push(i);
        }
        i += 1;
    }
    out
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn expected_edges(instrs: &[InstrKind], blocks: &[BasicBlock]) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let block_index = |target: usize| blocks.iter().position(|b| b.start == target);
        for (i, block) in blocks.iter().copied().enumerate() {
            match instrs[block.end - 1] {
                InstrKind::Normal => {
                    if i + 1 < blocks.len() {
                        out.push((i, i + 1));
                    }
                }
                InstrKind::Branch { target } => {
                    if let Some(j) = block_index(target) {
                        out.push((i, j));
                    }
                }
                InstrKind::ConditionalBranch { target } => {
                    if let Some(j) = block_index(target) {
                        out.push((i, j));
                        if i + 1 < blocks.len() && j != i + 1 {
                            out.push((i, i + 1));
                        }
                    } else if i + 1 < blocks.len() {
                        out.push((i, i + 1));
                    }
                }
                InstrKind::Return => {}
            }
        }
        out
    }

    fn manual_reachable(blocks: &[BasicBlock], edges: &[(usize, usize)]) -> Vec<bool> {
        if blocks.is_empty() {
            return Vec::new();
        }
        let mut seen = vec![false; blocks.len()];
        let mut queue = std::collections::VecDeque::new();
        seen[0] = true;
        queue.push_back(0usize);
        while let Some(u) = queue.pop_front() {
            for &(src, dst) in edges {
                if src == u && !seen[dst] {
                    seen[dst] = true;
                    queue.push_back(dst);
                }
            }
        }
        seen
    }

    fn valid_program_strategy(max_len: usize) -> impl Strategy<Value = Vec<InstrKind>> {
        (1..=max_len).prop_flat_map(|n| {
            proptest::collection::vec((0u8..4u8, 0usize..n.max(1)), n..=n).prop_map(move |raw| {
                raw.into_iter()
                    .enumerate()
                    .map(|(_i, (tag, raw_target))| match tag {
                        0 => InstrKind::Normal,
                        1 => InstrKind::Return,
                        2 => {
                            let target = if n <= 1 { 0 } else { 1 + (raw_target % (n - 1)) };
                            if n <= 1 {
                                InstrKind::Return
                            } else {
                                InstrKind::Branch { target }
                            }
                        }
                        _ => {
                            let target = if n <= 1 { 0 } else { 1 + (raw_target % (n - 1)) };
                            if n <= 1 {
                                InstrKind::Normal
                            } else {
                                InstrKind::ConditionalBranch { target }
                            }
                        }
                    })
                    .collect()
            })
        })
    }

    // r[verify control-flow-graph.valid-program]
    #[test]
    fn test_validate_program_rejects_entry_target() {
        let instrs = [InstrKind::Branch { target: 0 }, InstrKind::Return];
        assert!(!validate_program(&instrs));
    }

    // r[verify control-flow-graph.leader]
    #[test]
    fn test_identify_leaders() {
        let instrs = [
            InstrKind::ConditionalBranch { target: 3 },
            InstrKind::Normal,
            InstrKind::Branch { target: 4 },
            InstrKind::Normal,
            InstrKind::Return,
        ];
        assert!(validate_program(&instrs));
        let leaders = identify_leaders(&instrs);
        assert_eq!(leaders, vec![0, 1, 3, 4]);
        for &idx in &leaders {
            assert!(is_leader(&instrs, idx));
        }
    }

    // r[verify control-flow-graph.partition]
    #[test]
    fn test_build_blocks_partition() {
        let instrs = [
            InstrKind::ConditionalBranch { target: 3 },
            InstrKind::Normal,
            InstrKind::Branch { target: 4 },
            InstrKind::Normal,
            InstrKind::Return,
        ];
        let blocks = build_blocks(&instrs);
        assert_eq!(blocks, vec![
            BasicBlock { start: 0, end: 1 },
            BasicBlock { start: 1, end: 3 },
            BasicBlock { start: 3, end: 4 },
            BasicBlock { start: 4, end: 5 },
        ]);
    }

    // r[verify control-flow-graph.edges]
    #[test]
    fn test_build_edges() {
        let instrs = [
            InstrKind::ConditionalBranch { target: 3 },
            InstrKind::Normal,
            InstrKind::Branch { target: 4 },
            InstrKind::Normal,
            InstrKind::Return,
        ];
        let blocks = build_blocks(&instrs);
        let edges = build_edges(&instrs, &blocks);
        assert_eq!(edges, vec![(0, 2), (0, 1), (1, 3), (2, 3)]);
    }

    // r[verify control-flow-graph.entry]
    #[test]
    fn test_entry_block() {
        let instrs = [InstrKind::Normal, InstrKind::Return];
        let blocks = build_blocks(&instrs);
        assert_eq!(entry_block(&blocks), Some(0));
        assert_eq!(blocks[0].start, 0);
    }

    // r[verify control-flow-graph.exit]
    #[test]
    fn test_exit_block() {
        let instrs = [InstrKind::Normal, InstrKind::Return];
        let blocks = build_blocks(&instrs);
        assert!(is_exit_block(&instrs, &blocks, 0));
    }

    // r[verify control-flow-graph.well-formed]
    #[test]
    fn test_check_well_formed() {
        let instrs = [InstrKind::Normal, InstrKind::Return];
        let blocks = build_blocks(&instrs);
        let edges = build_edges(&instrs, &blocks);
        assert!(check_well_formed(&instrs, &blocks, &edges));
    }

    // r[verify control-flow-graph.reachability]
    #[test]
    fn test_find_dead_blocks() {
        let instrs = [InstrKind::Branch { target: 2 }, InstrKind::Return, InstrKind::Return];
        let blocks = build_blocks(&instrs);
        let edges = build_edges(&instrs, &blocks);
        let dead = find_dead_blocks(&blocks, &edges);
        assert_eq!(dead, vec![1]);
    }

    proptest! {
        // r[verify control-flow-graph.partition]
        #[test]
        fn prop_blocks_cover_all_instrs(instrs in valid_program_strategy(12)) {
            prop_assume!(validate_program(&instrs));
            let blocks = build_blocks(&instrs);
            prop_assert!(!blocks.is_empty());
            prop_assert_eq!(blocks[0].start, 0);
            prop_assert_eq!(blocks.last().unwrap().end, instrs.len());
            for block in &blocks {
                prop_assert!(block.start < block.end);
                prop_assert!(block.end <= instrs.len());
            }
            for w in blocks.windows(2) {
                prop_assert_eq!(w[0].end, w[1].start);
            }
            for idx in 0..instrs.len() {
                let count = blocks.iter().filter(|b| b.start <= idx && idx < b.end).count();
                prop_assert_eq!(count, 1, "instruction {} should be in exactly one block", idx);
            }
        }

        // r[verify control-flow-graph.edges]
        #[test]
        fn prop_edges_match_manual(instrs in valid_program_strategy(12)) {
            prop_assume!(validate_program(&instrs));
            let blocks = build_blocks(&instrs);
            let edges = build_edges(&instrs, &blocks);
            prop_assert_eq!(edges, expected_edges(&instrs, &blocks));
        }

        // r[verify control-flow-graph.reachability]
        #[test]
        fn prop_reachability_matches_manual(instrs in valid_program_strategy(10)) {
            prop_assume!(validate_program(&instrs));
            let blocks = build_blocks(&instrs);
            let edges = build_edges(&instrs, &blocks);
            let reachable = find_reachable(&blocks, &edges);
            let manual = manual_reachable(&blocks, &edges);
            prop_assert_eq!(reachable, manual.clone());
            let dead = find_dead_blocks(&blocks, &edges);
            for idx in 0..blocks.len() {
                prop_assert_eq!(dead.contains(&idx), !manual[idx]);
            }
        }
    }
}
