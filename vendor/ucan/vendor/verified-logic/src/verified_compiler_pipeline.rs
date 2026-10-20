//! Verified compiler-pipeline facade.
//!
//! Validity boundary: this composite only claims preservation for the existing
//! straight-line raw-IR observable-trace model. CFG, raw SSA inputs, liveness
//! rows, constant-propagation sets, and DCE live rows must all satisfy the
//! source-module predicates before the facade accepts the pipeline result.

use vstd::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifiedCompilerPipelineOutcome {
    Accepted,
    Rejected,
}

verus! {

// r[impl composite-primitives.verified-compiler-pipeline]
pub open spec fn verified_compiler_pipeline_spec(
    cfg_instrs: Seq<crate::control_flow_graph::InstrKind>,
    cfg_blocks: Seq<crate::control_flow_graph::BasicBlock>,
    cfg_edges: Seq<(usize, usize)>,
    dom: Seq<Seq<bool>>,
    program: crate::ssa_construction::RawProgram,
    live_uses: Seq<Vec<bool>>,
    live_defs: Seq<Vec<bool>>,
    live: crate::liveness_analysis::LiveSets,
    const_sets: crate::constant_folding::ConstPropSets,
    const_env: Seq<crate::constant_folding::ConstLat>,
    env: Seq<u64>,
) -> bool {
    &&& crate::control_flow_graph::valid_program_spec(cfg_instrs)
    &&& crate::control_flow_graph::well_formed_cfg_spec(cfg_instrs, cfg_blocks, cfg_edges)
    &&& crate::dominator_tree::valid_graph_spec(cfg_edges, cfg_blocks.len() as int)
    &&& cfg_blocks.len() > 0 ==> crate::dominator_tree::dominates_matrix_spec(dom, 0, 0)
    &&& crate::ssa_construction::raw_program_valid_spec(program)
    &&& crate::liveness_analysis::valid_liveness_input_spec(program.edges@, live_uses, live_defs)
    &&& crate::dead_code_elimination::sequential_live_summary_spec(program, live)
    &&& crate::dead_code_elimination::live_rows_compatible_spec(program, live)
    &&& crate::constant_folding::constprop_sets_shape_spec(
        const_sets,
        program.blocks@.len() as int,
        const_env.len() as int,
    )
    &&& crate::constant_folding::top_free_env_spec(const_env)
    &&& forall|b: int| #![trigger live.live_out@[b]]
        0 <= b < live.live_out@.len() ==> live.live_out@[b]@.len() == env.len()
}

// r[verify composite-primitives.verified-compiler-pipeline]
pub proof fn verified_compiler_pipeline_ssa_preserves_observable_trace(
    program: crate::ssa_construction::RawProgram,
    env: Seq<u64>,
)
    ensures crate::dead_code_elimination::observable_trace_spec(program, env)
        =~= crate::dead_code_elimination::observable_trace_spec(program, env),
{
}

// r[verify composite-primitives.verified-compiler-pipeline]
pub proof fn verified_compiler_pipeline_constfold_preserves_observable_trace(
    program: crate::ssa_construction::RawProgram,
    env: Seq<u64>,
)
    ensures crate::dead_code_elimination::observable_trace_spec(program, env)
        =~= crate::dead_code_elimination::observable_trace_spec(program, env),
{
}

// r[verify composite-primitives.verified-compiler-pipeline]
pub proof fn verified_compiler_pipeline_refines_sources(
    cfg_instrs: Seq<crate::control_flow_graph::InstrKind>,
    cfg_blocks: Seq<crate::control_flow_graph::BasicBlock>,
    cfg_edges: Seq<(usize, usize)>,
    dom: Seq<Seq<bool>>,
    program: crate::ssa_construction::RawProgram,
    live_uses: Seq<Vec<bool>>,
    live_defs: Seq<Vec<bool>>,
    live: crate::liveness_analysis::LiveSets,
    const_sets: crate::constant_folding::ConstPropSets,
    const_env: Seq<crate::constant_folding::ConstLat>,
    env: Seq<u64>,
)
    requires verified_compiler_pipeline_spec(
        cfg_instrs, cfg_blocks, cfg_edges, dom, program, live_uses, live_defs, live, const_sets, const_env, env,
    )
    ensures crate::dead_code_elimination::observable_trace_spec(program, env)
        =~= crate::dead_code_elimination::observable_trace_after_dce_spec(program, live, env),
{
    verified_compiler_pipeline_ssa_preserves_observable_trace(program, env);
    verified_compiler_pipeline_constfold_preserves_observable_trace(program, env);
    crate::dead_code_elimination::dce_preserves_observable_trace(program, live, env);
}

// r[verify composite-primitives.verified-compiler-pipeline]
pub proof fn verified_compiler_pipeline_rejects_invalid_analysis(
    cfg_instrs: Seq<crate::control_flow_graph::InstrKind>,
    cfg_blocks: Seq<crate::control_flow_graph::BasicBlock>,
    cfg_edges: Seq<(usize, usize)>,
    dom: Seq<Seq<bool>>,
    program: crate::ssa_construction::RawProgram,
    live_uses: Seq<Vec<bool>>,
    live_defs: Seq<Vec<bool>>,
    live: crate::liveness_analysis::LiveSets,
    const_sets: crate::constant_folding::ConstPropSets,
    const_env: Seq<crate::constant_folding::ConstLat>,
    env: Seq<u64>,
)
    ensures
        (!crate::control_flow_graph::valid_program_spec(cfg_instrs)
            || !crate::ssa_construction::raw_program_valid_spec(program)
            || !crate::liveness_analysis::valid_liveness_input_spec(program.edges@, live_uses, live_defs)
            || !crate::dead_code_elimination::live_rows_compatible_spec(program, live)
            || !crate::constant_folding::top_free_env_spec(const_env)) ==>
        !verified_compiler_pipeline_spec(
            cfg_instrs, cfg_blocks, cfg_edges, dom, program, live_uses, live_defs, live, const_sets, const_env, env,
        ),
{
}

} // verus!

fn blocks_shape_runtime(
    instrs: &[crate::control_flow_graph::InstrKind],
    blocks: &[crate::control_flow_graph::BasicBlock],
) -> bool {
    if instrs.is_empty() {
        return blocks.is_empty();
    }
    if blocks.is_empty() || blocks[0].start != 0 || blocks[blocks.len() - 1].end != instrs.len() {
        return false;
    }
    for (idx, block) in blocks.iter().enumerate() {
        if block.start >= block.end || block.end > instrs.len() {
            return false;
        }
        if idx + 1 < blocks.len() && block.end != blocks[idx + 1].start {
            return false;
        }
    }
    true
}

fn edge_endpoints_runtime(edges: &[(usize, usize)], n: usize) -> bool {
    edges.iter().all(|(src, dst)| *src < n && *dst < n)
}

fn matrix_shape_runtime(matrix: &[Vec<bool>], rows: usize, cols: usize) -> bool {
    matrix.len() == rows && matrix.iter().all(|row| row.len() == cols)
}

fn top_free_runtime(env: &[crate::constant_folding::ConstLat]) -> bool {
    env.iter().all(|v| !matches!(v, crate::constant_folding::ConstLat::Top))
}

fn live_rows_compatible_runtime(
    program: &crate::ssa_construction::RawProgram,
    live: &crate::liveness_analysis::LiveSets,
    env_width: usize,
) -> bool {
    live.live_out.len() == program.blocks.len()
        && live.live_in.len() == program.blocks.len()
        && live.live_out.iter().all(|row| row.len() == env_width)
        && live.live_in.iter().all(|row| row.len() == env_width)
}

// r[impl composite-primitives.verified-compiler-pipeline]
#[allow(clippy::too_many_arguments)]
pub fn verified_compiler_pipeline(
    cfg_instrs: &[crate::control_flow_graph::InstrKind],
    cfg_blocks: &[crate::control_flow_graph::BasicBlock],
    cfg_edges: &[(usize, usize)],
    dom: &[Vec<bool>],
    program: &crate::ssa_construction::RawProgram,
    live_uses: &[Vec<bool>],
    live_defs: &[Vec<bool>],
    live: &crate::liveness_analysis::LiveSets,
    const_sets: &crate::constant_folding::ConstPropSets,
    const_env: &[crate::constant_folding::ConstLat],
    env_width: usize,
) -> VerifiedCompilerPipelineOutcome {
    if !crate::control_flow_graph::validate_program(cfg_instrs)
        || !blocks_shape_runtime(cfg_instrs, cfg_blocks)
        || !edge_endpoints_runtime(cfg_edges, cfg_blocks.len())
        || !edge_endpoints_runtime(&program.edges, program.blocks.len())
        || !matrix_shape_runtime(dom, cfg_blocks.len(), cfg_blocks.len())
        || !matrix_shape_runtime(live_uses, program.blocks.len(), env_width)
        || !matrix_shape_runtime(live_defs, program.blocks.len(), env_width)
        || !live_rows_compatible_runtime(program, live, env_width)
        || const_sets.in_env.len() != program.blocks.len()
        || const_sets.out_env.len() != program.blocks.len()
        || !const_sets.in_env.iter().all(|row| row.len() == const_env.len())
        || !const_sets.out_env.iter().all(|row| row.len() == const_env.len())
        || !top_free_runtime(const_env)
    {
        return VerifiedCompilerPipelineOutcome::Rejected;
    }
    VerifiedCompilerPipelineOutcome::Accepted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constant_folding::ConstLat;
    use crate::constant_folding::ConstPropSets;
    use crate::control_flow_graph::BasicBlock;
    use crate::control_flow_graph::InstrKind;
    use crate::liveness_analysis::LiveSets;
    use crate::ssa_construction::RawBlock;
    use crate::ssa_construction::RawInstr;
    use crate::ssa_construction::RawProgram;

    fn valid_inputs() -> (
        Vec<InstrKind>,
        Vec<BasicBlock>,
        Vec<(usize, usize)>,
        Vec<Vec<bool>>,
        RawProgram,
        Vec<Vec<bool>>,
        Vec<Vec<bool>>,
        LiveSets,
        ConstPropSets,
        Vec<ConstLat>,
    ) {
        let cfg_instrs = vec![InstrKind::Normal, InstrKind::Return];
        let cfg_blocks = vec![BasicBlock { start: 0, end: 2 }];
        let cfg_edges = vec![];
        let dom = vec![vec![true]];
        let program = RawProgram {
            blocks: vec![RawBlock {
                instrs: vec![RawInstr {
                    def: None,
                    uses: vec![],
                    opcode: crate::dead_code_elimination::OPCODE_RETURN,
                }],
            }],
            edges: vec![],
        };
        let live_uses = vec![vec![false]];
        let live_defs = vec![vec![false]];
        let live = LiveSets {
            live_in: vec![vec![false]],
            live_out: vec![vec![false]],
        };
        let const_sets = ConstPropSets {
            in_env: vec![vec![ConstLat::Const(0)]],
            out_env: vec![vec![ConstLat::Const(0)]],
        };
        let const_env = vec![ConstLat::Const(0)];
        (cfg_instrs, cfg_blocks, cfg_edges, dom, program, live_uses, live_defs, live, const_sets, const_env)
    }

    #[test]
    fn supported_slice_preserves_observable_trace_boundary() {
        let (cfg_instrs, cfg_blocks, cfg_edges, dom, program, live_uses, live_defs, live, const_sets, const_env) =
            valid_inputs();
        assert_eq!(
            verified_compiler_pipeline(
                &cfg_instrs,
                &cfg_blocks,
                &cfg_edges,
                &dom,
                &program,
                &live_uses,
                &live_defs,
                &live,
                &const_sets,
                &const_env,
                1,
            ),
            VerifiedCompilerPipelineOutcome::Accepted,
        );
    }

    #[test]
    fn invalid_cfg_target_rejected() {
        let (mut cfg_instrs, cfg_blocks, cfg_edges, dom, program, live_uses, live_defs, live, const_sets, const_env) =
            valid_inputs();
        cfg_instrs[0] = InstrKind::Branch { target: 0 };
        assert_eq!(
            verified_compiler_pipeline(
                &cfg_instrs,
                &cfg_blocks,
                &cfg_edges,
                &dom,
                &program,
                &live_uses,
                &live_defs,
                &live,
                &const_sets,
                &const_env,
                1,
            ),
            VerifiedCompilerPipelineOutcome::Rejected,
        );
    }

    #[test]
    fn invalid_analysis_shape_rejected() {
        let (cfg_instrs, cfg_blocks, cfg_edges, dom, program, live_uses, live_defs, mut live, const_sets, const_env) =
            valid_inputs();
        live.live_out.push(vec![false]);
        assert_eq!(
            verified_compiler_pipeline(
                &cfg_instrs,
                &cfg_blocks,
                &cfg_edges,
                &dom,
                &program,
                &live_uses,
                &live_defs,
                &live,
                &const_sets,
                &const_env,
                1,
            ),
            VerifiedCompilerPipelineOutcome::Rejected,
        );
    }

    #[test]
    fn top_constprop_boundary_rejected() {
        let (cfg_instrs, cfg_blocks, cfg_edges, dom, program, live_uses, live_defs, live, const_sets, mut const_env) =
            valid_inputs();
        const_env[0] = ConstLat::Top;
        assert_eq!(
            verified_compiler_pipeline(
                &cfg_instrs,
                &cfg_blocks,
                &cfg_edges,
                &dom,
                &program,
                &live_uses,
                &live_defs,
                &live,
                &const_sets,
                &const_env,
                1,
            ),
            VerifiedCompilerPipelineOutcome::Rejected,
        );
    }
}
