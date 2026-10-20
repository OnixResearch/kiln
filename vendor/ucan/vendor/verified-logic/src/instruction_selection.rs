use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Op {
    Const,
    Add,
    Mul,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExprNode {
    pub op: Op,
    pub children: Vec<ExprNode>,
    pub value: Option<u64>,
}

pub type NodePath = Vec<usize>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TilePattern {
    Any,
    Specific {
        op: Op,
        const_value: Option<u64>,
        children: Vec<TilePattern>,
    },
}

#[derive(Clone)]
pub enum TileSemantics {
    ConstValue(u64),
    Add,
    Mul,
    FusedAddMul,
}

impl std::fmt::Debug for TileSemantics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TileSemantics::ConstValue(value) => f.debug_tuple("ConstValue").field(value).finish(),
            TileSemantics::Add => f.write_str("Add"),
            TileSemantics::Mul => f.write_str("Mul"),
            TileSemantics::FusedAddMul => f.write_str("FusedAddMul"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Tile {
    pub pattern: TilePattern,
    pub cost: u32,
    pub tie_break_rank: u32,
    pub semantics: TileSemantics,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub root_path: NodePath,
    pub tile_index: usize,
    pub uncovered_roots: Vec<NodePath>,
    pub covered_paths: Vec<NodePath>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tiling {
    pub selections: Vec<Selection>,
    pub total_cost: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionError {
    MalformedTree,
    InvalidTilePattern { tile_index: usize },
    ArithmeticOverflow,
    NoMatchingTile { root_path: NodePath },
}

const CONST_ARITY: usize = 0;
const BINARY_ARITY: usize = 2;
const ROOT_PATH_LEN: usize = 0;

// r[impl instruction-selection.well-formed-tree]
pub fn is_well_formed_tree(node: &ExprNode) -> bool {
    let shape_ok = match node.op {
        Op::Const => node.children.len() == CONST_ARITY && node.value.is_some(),
        Op::Add | Op::Mul => node.children.len() == BINARY_ARITY && node.value.is_none(),
    };
    shape_ok && subtree_eval(node).is_some() && node.children.iter().all(is_well_formed_tree)
}

// r[impl instruction-selection.valid-tile-pattern]
pub fn is_valid_tile_pattern(pattern: &TilePattern) -> bool {
    match pattern {
        TilePattern::Any => false,
        TilePattern::Specific {
            op,
            const_value,
            children,
        } => {
            valid_root_literal_boundary(*op, *const_value, children.len())
                && children.iter().all(is_valid_tile_pattern_child)
        }
    }
}

fn is_valid_tile_pattern_child(pattern: &TilePattern) -> bool {
    match pattern {
        TilePattern::Any => true,
        TilePattern::Specific {
            op,
            const_value,
            children,
        } => {
            valid_root_literal_boundary(*op, *const_value, children.len())
                && children.iter().all(is_valid_tile_pattern_child)
        }
    }
}

// r[impl instruction-selection.subtree-eval]
pub fn subtree_eval(node: &ExprNode) -> Option<u64> {
    match node.op {
        Op::Const => {
            if node.children.len() != CONST_ARITY {
                return None;
            }
            node.value
        }
        Op::Add => {
            if node.children.len() != BINARY_ARITY || node.value.is_some() {
                return None;
            }
            let left_value = subtree_eval(&node.children[0])?;
            let right_value = subtree_eval(&node.children[1])?;
            left_value.checked_add(right_value)
        }
        Op::Mul => {
            if node.children.len() != BINARY_ARITY || node.value.is_some() {
                return None;
            }
            let left_value = subtree_eval(&node.children[0])?;
            let right_value = subtree_eval(&node.children[1])?;
            left_value.checked_mul(right_value)
        }
    }
}

// r[impl instruction-selection.tiling-cost]
pub fn tiling_cost(tree: &ExprNode, tiling: &Tiling) -> Result<u32, SelectionError> {
    if !is_well_formed_tree(tree) {
        return Err(SelectionError::MalformedTree);
    }
    Ok(tiling.total_cost)
}

// r[impl instruction-selection.tile-matches]
pub fn tile_matches(
    tile: &Tile,
    node: &ExprNode,
    root_path: &[usize],
) -> Result<Option<Vec<NodePath>>, SelectionError> {
    if !is_well_formed_tree(node) {
        return Err(SelectionError::MalformedTree);
    }
    if !is_valid_tile_pattern(&tile.pattern) {
        return Err(SelectionError::InvalidTilePattern { tile_index: 0 });
    }
    Ok(match_pattern(&tile.pattern, node, root_path))
}

fn match_pattern(pattern: &TilePattern, node: &ExprNode, root_path: &[usize]) -> Option<Vec<NodePath>> {
    match pattern {
        TilePattern::Any => Some(vec![root_path.to_vec()]),
        TilePattern::Specific {
            op,
            const_value,
            children,
        } => {
            if node.op != *op
                || node.children.len() != children.len()
                || !node_literal_matches(*op, *const_value, node.value)
            {
                return None;
            }
            let mut uncovered_roots = Vec::new();
            let child_count = children.len();
            for child_index in 0..child_count {
                let child_pattern = &children[child_index];
                let child_node = &node.children[child_index];
                let child_path = child_path(root_path, child_index);
                let child_uncovered = match child_pattern {
                    TilePattern::Any => vec![child_path],
                    TilePattern::Specific { .. } => match_pattern(child_pattern, child_node, &child_path)?,
                };
                uncovered_roots.extend(child_uncovered);
            }
            Some(uncovered_roots)
        }
    }
}

// r[impl instruction-selection.optimal-tiling]
pub fn optimal_tiling(node: &ExprNode, tiles: &[Tile]) -> Result<Tiling, SelectionError> {
    if !is_well_formed_tree(node) {
        return Err(SelectionError::MalformedTree);
    }
    validate_tiles(tiles)?;
    let best = best_tiling_at(node, tiles, &Vec::with_capacity(ROOT_PATH_LEN))?;
    Ok(best.tiling)
}

// r[impl instruction-selection.coverage-check]
pub fn check_coverage(tree: &ExprNode, tiling: &Tiling) -> Result<bool, SelectionError> {
    if !is_well_formed_tree(tree) {
        return Err(SelectionError::MalformedTree);
    }
    let all_paths = collect_paths(tree, &Vec::with_capacity(ROOT_PATH_LEN));
    let mut covered = Vec::new();
    for selection in &tiling.selections {
        let selected_node =
            node_at_path(tree, &Vec::with_capacity(ROOT_PATH_LEN), &selection.root_path).ok_or_else(|| {
                SelectionError::NoMatchingTile {
                    root_path: selection.root_path.clone(),
                }
            })?;
        let recomputed_paths = covered_paths_for_selection(selected_node, selection);
        if recomputed_paths != selection.covered_paths {
            return Ok(false);
        }
        for path in recomputed_paths {
            if covered.contains(&path) {
                return Ok(false);
            }
            covered.push(path);
        }
    }
    sort_paths(&mut covered);
    Ok(covered == all_paths)
}

// r[impl instruction-selection.soundness-check]
pub fn check_soundness(tree: &ExprNode, tiles: &[Tile], tiling: &Tiling) -> Result<bool, SelectionError> {
    if !is_well_formed_tree(tree) {
        return Err(SelectionError::MalformedTree);
    }
    validate_tiles(tiles)?;
    let selected_value = evaluate_tiling(tree, tiles, tiling, &Vec::with_capacity(ROOT_PATH_LEN))?;
    let tree_value = subtree_eval(tree).ok_or(SelectionError::ArithmeticOverflow)?;
    Ok(selected_value == tree_value)
}

fn validate_tiles(tiles: &[Tile]) -> Result<(), SelectionError> {
    for (tile_index, tile) in tiles.iter().enumerate() {
        if !is_valid_tile_pattern(&tile.pattern) {
            return Err(SelectionError::InvalidTilePattern { tile_index });
        }
    }
    Ok(())
}

struct BestTiling {
    tiling: Tiling,
}

fn best_tiling_at(node: &ExprNode, tiles: &[Tile], root_path: &NodePath) -> Result<BestTiling, SelectionError> {
    let mut best_candidate: Option<Candidate> = None;
    let mut best_failure: Option<NodePath> = None;

    for (tile_index, tile) in tiles.iter().enumerate() {
        let Some(uncovered_roots) = match_pattern(&tile.pattern, node, root_path) else {
            continue;
        };
        let covered_paths = covered_paths_for_pattern(&tile.pattern, node, root_path);
        let mut child_results = Vec::new();
        let mut child_cost_total: u32 = 0;
        let mut child_failure: Option<NodePath> = None;

        for uncovered_root in &uncovered_roots {
            let uncovered_node =
                node_at_path(node, root_path, uncovered_root).ok_or_else(|| SelectionError::NoMatchingTile {
                    root_path: uncovered_root.clone(),
                })?;
            match best_tiling_at(uncovered_node, tiles, uncovered_root) {
                Ok(result) => {
                    child_cost_total = child_cost_total
                        .checked_add(result.tiling.total_cost)
                        .ok_or(SelectionError::ArithmeticOverflow)?;
                    child_results.push(result);
                }
                Err(SelectionError::NoMatchingTile { root_path }) => {
                    merge_failure(&mut child_failure, root_path);
                }
                Err(other) => return Err(other),
            }
        }

        if let Some(root_failure) = child_failure {
            merge_failure(&mut best_failure, root_failure);
            continue;
        }

        let total_cost = tile.cost.checked_add(child_cost_total).ok_or(SelectionError::ArithmeticOverflow)?;
        let mut selections = vec![Selection {
            root_path: root_path.clone(),
            tile_index,
            uncovered_roots: uncovered_roots.clone(),
            covered_paths,
        }];
        for child_result in child_results {
            selections.extend(child_result.tiling.selections);
        }
        sort_selections(&mut selections);
        let candidate = Candidate {
            tiling: Tiling { selections, total_cost },
            tie_break_rank: tile.tie_break_rank,
            tile_index,
        };
        if candidate_beats(&candidate, best_candidate.as_ref()) {
            best_candidate = Some(candidate);
        }
    }

    if let Some(candidate) = best_candidate {
        return Ok(BestTiling {
            tiling: candidate.tiling,
        });
    }

    let failure_root = best_failure.unwrap_or_else(|| root_path.clone());
    Err(SelectionError::NoMatchingTile {
        root_path: failure_root,
    })
}

struct Candidate {
    tiling: Tiling,
    tie_break_rank: u32,
    tile_index: usize,
}

fn valid_root_literal_boundary(op: Op, const_value: Option<u64>, child_count: usize) -> bool {
    match op {
        Op::Const => child_count == CONST_ARITY && const_value.is_some(),
        Op::Add | Op::Mul => child_count == BINARY_ARITY && const_value.is_none(),
    }
}

fn node_literal_matches(op: Op, pattern_value: Option<u64>, node_value: Option<u64>) -> bool {
    match op {
        Op::Const => pattern_value == node_value,
        Op::Add | Op::Mul => pattern_value.is_none() && node_value.is_none(),
    }
}

fn candidate_beats(candidate: &Candidate, current_best: Option<&Candidate>) -> bool {
    match current_best {
        None => true,
        Some(best) => match candidate.tiling.total_cost.cmp(&best.tiling.total_cost) {
            Ordering::Less => true,
            Ordering::Greater => false,
            Ordering::Equal => match candidate.tie_break_rank.cmp(&best.tie_break_rank) {
                Ordering::Less => true,
                Ordering::Greater => false,
                Ordering::Equal => candidate.tile_index < best.tile_index,
            },
        },
    }
}

fn merge_failure(current: &mut Option<NodePath>, candidate: NodePath) {
    match current {
        None => *current = Some(candidate),
        Some(existing) => {
            if compare_paths(&candidate, existing) == Ordering::Less {
                *existing = candidate;
            }
        }
    }
}

fn covered_paths_for_pattern(pattern: &TilePattern, node: &ExprNode, root_path: &NodePath) -> Vec<NodePath> {
    let _ = node;
    match pattern {
        TilePattern::Any => Vec::new(),
        TilePattern::Specific { children, .. } => {
            let mut covered = vec![root_path.clone()];
            let child_count = children.len();
            for child_index in 0..child_count {
                let child_pattern = &children[child_index];
                if let TilePattern::Specific { .. } = child_pattern {
                    let child_path = child_path(root_path, child_index);
                    covered.extend(covered_paths_for_pattern(child_pattern, &node.children[child_index], &child_path));
                }
            }
            covered
        }
    }
}

fn covered_paths_for_selection(node: &ExprNode, selection: &Selection) -> Vec<NodePath> {
    let mut covered = collect_paths(node, &selection.root_path);
    for uncovered_root in &selection.uncovered_roots {
        covered.retain(|path| !path.starts_with(uncovered_root));
    }
    sort_paths(&mut covered);
    covered
}

fn evaluate_tiling(
    tree: &ExprNode,
    tiles: &[Tile],
    tiling: &Tiling,
    root_path: &NodePath,
) -> Result<u64, SelectionError> {
    let selection = tiling.selections.iter().find(|selection| selection.root_path == *root_path).ok_or_else(|| {
        SelectionError::NoMatchingTile {
            root_path: root_path.clone(),
        }
    })?;
    let tile = tiles.get(selection.tile_index).ok_or(SelectionError::InvalidTilePattern {
        tile_index: selection.tile_index,
    })?;
    let mut inputs = Vec::new();
    for uncovered_root in &selection.uncovered_roots {
        let child_value = evaluate_tiling(tree, tiles, tiling, uncovered_root)?;
        inputs.push(child_value);
    }
    let tile_value = evaluate_tile_semantics(&tile.semantics, &inputs).ok_or(SelectionError::ArithmeticOverflow)?;
    let subtree = node_at_path(tree, &Vec::with_capacity(ROOT_PATH_LEN), root_path).ok_or_else(|| {
        SelectionError::NoMatchingTile {
            root_path: root_path.clone(),
        }
    })?;
    let expected = subtree_eval(subtree).ok_or(SelectionError::ArithmeticOverflow)?;
    if tile_value == expected {
        Ok(tile_value)
    } else {
        Err(SelectionError::NoMatchingTile {
            root_path: root_path.clone(),
        })
    }
}

fn evaluate_tile_semantics(semantics: &TileSemantics, inputs: &[u64]) -> Option<u64> {
    match semantics {
        TileSemantics::ConstValue(value) => {
            if inputs.is_empty() {
                Some(*value)
            } else {
                None
            }
        }
        TileSemantics::Add => {
            if inputs.len() != BINARY_ARITY {
                return None;
            }
            inputs[0].checked_add(inputs[1])
        }
        TileSemantics::Mul => {
            if inputs.len() != BINARY_ARITY {
                return None;
            }
            inputs[0].checked_mul(inputs[1])
        }
        TileSemantics::FusedAddMul => {
            const FUSED_INPUTS: usize = 3;
            if inputs.len() != FUSED_INPUTS {
                return None;
            }
            inputs[0].checked_mul(inputs[1])?.checked_add(inputs[2])
        }
    }
}

fn node_at_path<'a>(root: &'a ExprNode, base_path: &[usize], target_path: &[usize]) -> Option<&'a ExprNode> {
    if !target_path.starts_with(base_path) {
        return None;
    }
    let mut current = root;
    for &index in &target_path[base_path.len()..] {
        current = current.children.get(index)?;
    }
    Some(current)
}

fn collect_paths(node: &ExprNode, root_path: &NodePath) -> Vec<NodePath> {
    let mut paths = vec![root_path.clone()];
    for (child_index, child) in node.children.iter().enumerate() {
        let next_path = child_path(root_path, child_index);
        paths.extend(collect_paths(child, &next_path));
    }
    sort_paths(&mut paths);
    paths
}

fn sort_paths(paths: &mut [NodePath]) {
    paths.sort_by(compare_paths);
}

fn sort_selections(selections: &mut [Selection]) {
    selections.sort_by(|left, right| compare_paths(&left.root_path, &right.root_path));
}

fn compare_paths(left: &NodePath, right: &NodePath) -> Ordering {
    let shared_len = left.len().min(right.len());
    for index in 0..shared_len {
        let ordering = left[index].cmp(&right[index]);
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

fn child_path(root_path: &[usize], child_index: usize) -> NodePath {
    let mut next_path = root_path.to_vec();
    next_path.push(child_index);
    next_path
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZERO_COST: u32 = 0;
    const LOW_COST: u32 = 1;
    const MID_COST: u32 = 2;
    const HIGH_COST: u32 = 3;
    const FIRST_RANK: u32 = 0;
    const SECOND_RANK: u32 = 1;
    const CONST_VALUE_A: u64 = 2;
    const CONST_VALUE_B: u64 = 3;
    const CONST_VALUE_C: u64 = 5;
    const ADD_RESULT: u64 = CONST_VALUE_A + CONST_VALUE_B;
    const MUL_RESULT: u64 = CONST_VALUE_A * CONST_VALUE_B;

    fn const_node(value: u64) -> ExprNode {
        ExprNode {
            op: Op::Const,
            children: Vec::new(),
            value: Some(value),
        }
    }

    fn add_node(left: ExprNode, right: ExprNode) -> ExprNode {
        ExprNode {
            op: Op::Add,
            children: vec![left, right],
            value: None,
        }
    }

    fn mul_node(left: ExprNode, right: ExprNode) -> ExprNode {
        ExprNode {
            op: Op::Mul,
            children: vec![left, right],
            value: None,
        }
    }

    fn exact_const_tile(value: u64, cost: u32, rank: u32) -> Tile {
        Tile {
            pattern: TilePattern::Specific {
                op: Op::Const,
                const_value: Some(value),
                children: Vec::new(),
            },
            cost,
            tie_break_rank: rank,
            semantics: TileSemantics::ConstValue(value),
        }
    }

    fn add_tile(cost: u32, rank: u32) -> Tile {
        Tile {
            pattern: TilePattern::Specific {
                op: Op::Add,
                const_value: None,
                children: vec![TilePattern::Any, TilePattern::Any],
            },
            cost,
            tie_break_rank: rank,
            semantics: TileSemantics::Add,
        }
    }

    fn mul_tile(cost: u32, rank: u32) -> Tile {
        Tile {
            pattern: TilePattern::Specific {
                op: Op::Mul,
                const_value: None,
                children: vec![TilePattern::Any, TilePattern::Any],
            },
            cost,
            tie_break_rank: rank,
            semantics: TileSemantics::Mul,
        }
    }

    fn fused_add_mul_tile(cost: u32, rank: u32) -> Tile {
        Tile {
            pattern: TilePattern::Specific {
                op: Op::Add,
                const_value: None,
                children: vec![
                    TilePattern::Specific {
                        op: Op::Mul,
                        const_value: None,
                        children: vec![TilePattern::Any, TilePattern::Any],
                    },
                    TilePattern::Any,
                ],
            },
            cost,
            tie_break_rank: rank,
            semantics: TileSemantics::FusedAddMul,
        }
    }

    // r[verify instruction-selection.tile-matches]
    #[test]
    fn pattern_match_single_node_tile() {
        let tile = exact_const_tile(CONST_VALUE_A, LOW_COST, FIRST_RANK);
        let node = const_node(CONST_VALUE_A);
        let matched = tile_matches(&tile, &node, &[]).unwrap();
        assert_eq!(matched, Some(Vec::new()));
    }

    // r[verify instruction-selection.tile-matches]
    #[test]
    fn pattern_match_deep_tile_returns_preorder_uncovered_roots() {
        let tile = fused_add_mul_tile(LOW_COST, FIRST_RANK);
        let node = add_node(mul_node(const_node(CONST_VALUE_A), const_node(CONST_VALUE_B)), const_node(CONST_VALUE_C));
        let matched = tile_matches(&tile, &node, &[]).unwrap().unwrap();
        assert_eq!(matched, vec![vec![0, 0], vec![0, 1], vec![1]]);
    }

    // r[verify instruction-selection.tile-matches]
    #[test]
    fn pattern_mismatch_wrong_opcode_returns_none() {
        let tile = add_tile(LOW_COST, FIRST_RANK);
        let node = mul_node(const_node(CONST_VALUE_A), const_node(CONST_VALUE_B));
        assert_eq!(tile_matches(&tile, &node, &[]).unwrap(), None);
    }

    // r[verify instruction-selection.well-formed-tree]
    // r[verify instruction-selection.tile-matches]
    #[test]
    fn pattern_mismatch_wrong_arity_returns_none() {
        let tile = add_tile(LOW_COST, FIRST_RANK);
        let node = ExprNode {
            op: Op::Add,
            children: vec![const_node(CONST_VALUE_A)],
            value: None,
        };
        assert!(!is_well_formed_tree(&node));
        assert_eq!(tile_matches(&tile, &node, &[]), Err(SelectionError::MalformedTree));
    }

    // r[verify instruction-selection.optimal-tiling]
    #[test]
    fn missing_tile_failure_on_root_reports_empty_path() {
        let node = add_node(const_node(CONST_VALUE_A), const_node(CONST_VALUE_B));
        let tiles = vec![exact_const_tile(CONST_VALUE_A, LOW_COST, FIRST_RANK)];
        assert_eq!(optimal_tiling(&node, &tiles), Err(SelectionError::NoMatchingTile { root_path: vec![] }));
    }

    // r[verify instruction-selection.optimal-tiling]
    #[test]
    fn missing_tile_failure_reports_first_unmatched_root() {
        let node = add_node(mul_node(const_node(CONST_VALUE_A), const_node(CONST_VALUE_B)), const_node(CONST_VALUE_C));
        let tiles = vec![
            add_tile(LOW_COST, FIRST_RANK),
            exact_const_tile(CONST_VALUE_A, LOW_COST, FIRST_RANK),
        ];
        assert_eq!(optimal_tiling(&node, &tiles), Err(SelectionError::NoMatchingTile { root_path: vec![0] }));
    }

    // r[verify instruction-selection.optimal-tiling]
    #[test]
    fn dp_prefers_lower_total_cost() {
        let node = add_node(mul_node(const_node(CONST_VALUE_A), const_node(CONST_VALUE_B)), const_node(CONST_VALUE_C));
        let tiles = vec![
            exact_const_tile(CONST_VALUE_A, LOW_COST, FIRST_RANK),
            exact_const_tile(CONST_VALUE_B, LOW_COST, SECOND_RANK),
            exact_const_tile(CONST_VALUE_C, LOW_COST, HIGH_COST),
            add_tile(HIGH_COST, FIRST_RANK),
            mul_tile(MID_COST, FIRST_RANK),
            fused_add_mul_tile(LOW_COST, SECOND_RANK),
        ];
        let tiling = optimal_tiling(&node, &tiles).unwrap();
        assert_eq!(tiling.total_cost, LOW_COST + LOW_COST + LOW_COST + LOW_COST);
        assert_eq!(tiling.selections[0].tile_index, 5);
    }

    // r[verify instruction-selection.optimal-tiling]
    #[test]
    fn deterministic_tie_break_uses_rank_then_index() {
        let node = add_node(const_node(CONST_VALUE_A), const_node(CONST_VALUE_B));
        let tiles = vec![
            exact_const_tile(CONST_VALUE_A, LOW_COST, FIRST_RANK),
            exact_const_tile(CONST_VALUE_B, LOW_COST, SECOND_RANK),
            add_tile(LOW_COST, SECOND_RANK),
            add_tile(LOW_COST, FIRST_RANK),
        ];
        let tiling = optimal_tiling(&node, &tiles).unwrap();
        assert_eq!(tiling.selections[0].tile_index, 3);
    }

    // r[verify instruction-selection.optimal-tiling]
    #[test]
    fn repeated_runs_produce_identical_tiling() {
        let node = add_node(const_node(CONST_VALUE_A), const_node(CONST_VALUE_B));
        let tiles = vec![
            exact_const_tile(CONST_VALUE_A, LOW_COST, FIRST_RANK),
            exact_const_tile(CONST_VALUE_B, LOW_COST, SECOND_RANK),
            add_tile(LOW_COST, FIRST_RANK),
        ];
        let first = optimal_tiling(&node, &tiles).unwrap();
        let second = optimal_tiling(&node, &tiles).unwrap();
        assert_eq!(first, second);
    }

    // r[verify instruction-selection.coverage-check]
    #[test]
    fn check_coverage_accepts_complete_base_tiles() {
        let node = add_node(const_node(CONST_VALUE_A), const_node(CONST_VALUE_B));
        let tiles = vec![
            exact_const_tile(CONST_VALUE_A, LOW_COST, FIRST_RANK),
            exact_const_tile(CONST_VALUE_B, LOW_COST, SECOND_RANK),
            add_tile(LOW_COST, FIRST_RANK),
        ];
        let tiling = optimal_tiling(&node, &tiles).unwrap();
        assert!(check_coverage(&node, &tiling).unwrap());
    }

    // r[verify instruction-selection.soundness-check]
    #[test]
    fn check_soundness_accepts_small_tree() {
        let node = add_node(const_node(CONST_VALUE_A), const_node(CONST_VALUE_A));
        let tiles = vec![
            exact_const_tile(CONST_VALUE_A, LOW_COST, FIRST_RANK),
            add_tile(LOW_COST, FIRST_RANK),
        ];
        let tiling = optimal_tiling(&node, &tiles).unwrap();
        assert!(check_soundness(&node, &tiles, &tiling).unwrap());
    }

    // r[verify instruction-selection.tiling-cost]
    #[test]
    fn tiling_cost_projects_total_cost() {
        let node = add_node(const_node(CONST_VALUE_A), const_node(CONST_VALUE_B));
        let tiles = vec![
            exact_const_tile(CONST_VALUE_A, LOW_COST, FIRST_RANK),
            exact_const_tile(CONST_VALUE_B, LOW_COST, SECOND_RANK),
            add_tile(LOW_COST, FIRST_RANK),
        ];
        let tiling = optimal_tiling(&node, &tiles).unwrap();
        assert_eq!(tiling_cost(&node, &tiling).unwrap(), tiling.total_cost);
    }

    // r[verify instruction-selection.valid-tile-pattern]
    #[test]
    fn invalid_root_any_tile_is_rejected() {
        let node = const_node(CONST_VALUE_A);
        let tile = Tile {
            pattern: TilePattern::Any,
            cost: ZERO_COST,
            tie_break_rank: FIRST_RANK,
            semantics: TileSemantics::ConstValue(CONST_VALUE_A),
        };
        assert!(!is_valid_tile_pattern(&tile.pattern));
        assert_eq!(tile_matches(&tile, &node, &[]), Err(SelectionError::InvalidTilePattern { tile_index: 0 }));
        assert_eq!(optimal_tiling(&node, &[tile]), Err(SelectionError::InvalidTilePattern { tile_index: 0 }));
    }

    // r[verify instruction-selection.valid-tile-pattern]
    #[test]
    fn tile_pattern_validity_checks_root_shape_and_recursive_children() {
        assert!(is_valid_tile_pattern(&exact_const_tile(CONST_VALUE_A, LOW_COST, FIRST_RANK).pattern));
        assert!(is_valid_tile_pattern(&add_tile(LOW_COST, FIRST_RANK).pattern));
        assert!(is_valid_tile_pattern(&fused_add_mul_tile(LOW_COST, FIRST_RANK).pattern));

        let const_with_child = TilePattern::Specific {
            op: Op::Const,
            const_value: Some(CONST_VALUE_A),
            children: vec![TilePattern::Any],
        };
        let add_with_literal = TilePattern::Specific {
            op: Op::Add,
            const_value: Some(CONST_VALUE_A),
            children: vec![TilePattern::Any, TilePattern::Any],
        };

        assert!(!is_valid_tile_pattern(&const_with_child));
        assert!(!is_valid_tile_pattern(&add_with_literal));
    }

    // r[verify instruction-selection.well-formed-tree]
    // r[verify instruction-selection.subtree-eval]
    #[test]
    fn overflowing_tree_is_malformed_for_modeled_surface() {
        let node = add_node(const_node(u64::MAX), const_node(CONST_VALUE_A));
        assert_eq!(subtree_eval(&node), None);
        assert!(!is_well_formed_tree(&node));
        assert_eq!(optimal_tiling(&node, &[]), Err(SelectionError::MalformedTree));
    }

    // r[verify instruction-selection.coverage-check]
    #[test]
    fn check_coverage_rejects_tampered_covered_paths() {
        let node = add_node(const_node(CONST_VALUE_A), const_node(CONST_VALUE_B));
        let tiles = vec![
            exact_const_tile(CONST_VALUE_A, LOW_COST, FIRST_RANK),
            exact_const_tile(CONST_VALUE_B, LOW_COST, SECOND_RANK),
            add_tile(LOW_COST, FIRST_RANK),
        ];
        let mut tiling = optimal_tiling(&node, &tiles).unwrap();
        tiling.selections[0].covered_paths = vec![vec![1]];
        assert!(!check_coverage(&node, &tiling).unwrap());
    }

    // r[verify instruction-selection.soundness-check]
    #[test]
    fn check_soundness_rejects_tampered_selection_semantics() {
        let node = add_node(const_node(CONST_VALUE_A), const_node(CONST_VALUE_B));
        let mut tiles = vec![
            exact_const_tile(CONST_VALUE_A, LOW_COST, FIRST_RANK),
            exact_const_tile(CONST_VALUE_B, LOW_COST, SECOND_RANK),
            add_tile(LOW_COST, FIRST_RANK),
        ];
        let tiling = optimal_tiling(&node, &tiles).unwrap();
        tiles[2].semantics = TileSemantics::Mul;
        assert_eq!(check_soundness(&node, &tiles, &tiling), Err(SelectionError::NoMatchingTile { root_path: vec![] }));
    }

    // r[verify instruction-selection.tile-matches]
    #[test]
    fn const_literal_mismatch_returns_none() {
        let tile = exact_const_tile(CONST_VALUE_A, LOW_COST, FIRST_RANK);
        let node = const_node(CONST_VALUE_B);
        assert_eq!(tile_matches(&tile, &node, &[]).unwrap(), None);
    }

    // r[verify instruction-selection.subtree-eval]
    #[test]
    fn subtree_eval_matches_reference_arithmetic() {
        let add_tree = add_node(const_node(CONST_VALUE_A), const_node(CONST_VALUE_B));
        let mul_tree = mul_node(const_node(CONST_VALUE_A), const_node(CONST_VALUE_B));
        assert_eq!(subtree_eval(&add_tree), Some(ADD_RESULT));
        assert_eq!(subtree_eval(&mul_tree), Some(MUL_RESULT));
    }
}
