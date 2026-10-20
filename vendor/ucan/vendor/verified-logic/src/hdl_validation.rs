//! Hardware-output collateral and external-tool validation evidence helpers.
//!
//! These helpers classify already-observed tool outcomes. Process execution,
//! filesystem writes, and evidence persistence belong in thin shells around this
//! pure core.

#![allow(clippy::struct_excessive_bools)]

use crate::hdl_emit::HdlEmitterMode;
use crate::hdl_emit::hdl_mode_label;
use crate::hdl_ir::HdlEvalCase;
use crate::hdl_ir::HdlEvalError;
use crate::hdl_ir::hdl_eval_step;

pub const HDL_VALIDATION_REASON_PASS: &str = "tool completed successfully";
pub const HDL_VALIDATION_REASON_EXIT_FAILURE: &str = "tool exited with failure";
pub const HDL_VALIDATION_REASON_TOOL_UNAVAILABLE: &str = "tool unavailable";
pub const HDL_VALIDATION_REASON_INVALID_INPUT: &str = "input artifact is not traceable";
pub const HDL_BACKEND_LIMITS: &str =
    "no timing closure, vendor implementation, or downstream synthesis-preservation claim";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdlTool {
    Verilator,
    Yosys,
    SymbiYosys,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdlToolClassification {
    Pass,
    Fail,
    Skipped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HdlToolObservation {
    pub tool: HdlTool,
    pub tool_available: bool,
    pub exit_success: bool,
    pub input_artifact_traceable: bool,
    pub stdout_captured: bool,
    pub stderr_captured: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HdlToolEvidence {
    pub tool: HdlTool,
    pub classification: HdlToolClassification,
    pub reason: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdlCollateralError {
    EvaluatorRejected(HdlEvalError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HdlVectorCollateral {
    pub module_name: &'static str,
    pub mode_label: &'static str,
    pub expected_output: u64,
    pub expected_next_state: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HdlBackendAdmissionInput {
    pub ir_admitted: bool,
    pub adapter_admitted: bool,
    pub adapter_refined: bool,
    pub emitter_supported: bool,
    pub collateral_eligible: bool,
    pub validation_recorded: bool,
    pub names_non_claim_limits: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HdlBackendAdmissionClaim {
    pub admitted: bool,
    pub limits: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HdlTestMatrixRow {
    pub has_requirement_id: bool,
    pub has_positive_artifact: bool,
    pub has_negative_artifact: bool,
    pub has_verify_marker: bool,
}

// r[impl hardware-output.testing.matrix]
// r[impl hardware-output.testing.tracey-gates]
pub fn hdl_test_matrix_row_complete(row: HdlTestMatrixRow) -> bool {
    row.has_requirement_id && row.has_positive_artifact && row.has_negative_artifact && row.has_verify_marker
}

// r[impl hardware-output.validation]
// r[impl hardware-output.testing.tool-evidence]
pub fn hdl_classify_tool_observation(observation: HdlToolObservation) -> HdlToolEvidence {
    if !observation.tool_available {
        return HdlToolEvidence {
            tool: observation.tool,
            classification: HdlToolClassification::Skipped,
            reason: HDL_VALIDATION_REASON_TOOL_UNAVAILABLE,
        };
    }
    if !observation.input_artifact_traceable {
        return HdlToolEvidence {
            tool: observation.tool,
            classification: HdlToolClassification::Fail,
            reason: HDL_VALIDATION_REASON_INVALID_INPUT,
        };
    }
    if observation.exit_success && observation.stdout_captured && observation.stderr_captured {
        HdlToolEvidence {
            tool: observation.tool,
            classification: HdlToolClassification::Pass,
            reason: HDL_VALIDATION_REASON_PASS,
        }
    } else {
        HdlToolEvidence {
            tool: observation.tool,
            classification: HdlToolClassification::Fail,
            reason: HDL_VALIDATION_REASON_EXIT_FAILURE,
        }
    }
}

// r[impl hardware-output.testbench]
pub fn hdl_generate_vector_collateral(
    case: HdlEvalCase,
    mode: HdlEmitterMode,
    module_name: &'static str,
) -> Result<HdlVectorCollateral, HdlCollateralError> {
    let result = hdl_eval_step(case).map_err(HdlCollateralError::EvaluatorRejected)?;
    Ok(HdlVectorCollateral {
        module_name,
        mode_label: hdl_mode_label(mode),
        expected_output: result.output,
        expected_next_state: result.next_state,
    })
}

// r[impl hardware-output.backend-admission]
pub fn hdl_backend_admission(input: HdlBackendAdmissionInput) -> HdlBackendAdmissionClaim {
    let admitted = input.ir_admitted
        && input.adapter_admitted
        && input.adapter_refined
        && input.emitter_supported
        && input.collateral_eligible
        && input.validation_recorded
        && input.names_non_claim_limits;
    HdlBackendAdmissionClaim {
        admitted,
        limits: HDL_BACKEND_LIMITS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hdl_emit::HdlEmitterMode;
    use crate::hdl_emit::HdlExampleKind;
    use crate::hdl_emit::hdl_example_module_name;
    use crate::hdl_ir::HDL_EMPTY_COUNT;
    use crate::hdl_ir::HDL_OPTIONAL_RESET_LIMIT;
    use crate::hdl_ir::HDL_SINGLE_CLOCK;
    use crate::hdl_ir::HdlEvalCase;
    use crate::hdl_ir::HdlExprKind;
    use crate::hdl_ir::HdlModuleShape;

    const TEST_WIDTH: u16 = 8;
    const TEST_INPUTS: u16 = 2;
    const TEST_OUTPUTS: u16 = 1;
    const TEST_EXPRS: u16 = 1;
    const TEST_LHS: u64 = 0b1111_0000;
    const TEST_RHS: u64 = 0b1010_1010;
    const TEST_EXPECTED_AND: u64 = 0b1010_0000;
    const TEST_REGISTER_COUNT: u16 = 1;

    fn module() -> HdlModuleShape {
        HdlModuleShape {
            input_count: TEST_INPUTS,
            output_count: TEST_OUTPUTS,
            expr_count: TEST_EXPRS,
            register_count: HDL_EMPTY_COUNT,
            clock_count: HDL_EMPTY_COUNT,
            reset_count: HDL_EMPTY_COUNT,
            fsm_state_count: HDL_EMPTY_COUNT,
            value_width: TEST_WIDTH,
            has_boolean_expr: true,
            canonical_order: true,
            unsupported_node_count: HDL_EMPTY_COUNT,
            malformed_width_count: HDL_EMPTY_COUNT,
        }
    }

    fn eval_case() -> HdlEvalCase {
        HdlEvalCase {
            module: module(),
            op: HdlExprKind::BitwiseAnd,
            lhs: TEST_LHS,
            rhs: TEST_RHS,
            select: false,
            input_present: true,
            input_width: TEST_WIDTH,
            current_state: HDL_EMPTY_COUNT,
        }
    }

    fn passing_observation(tool: HdlTool) -> HdlToolObservation {
        HdlToolObservation {
            tool,
            tool_available: true,
            exit_success: true,
            input_artifact_traceable: true,
            stdout_captured: true,
            stderr_captured: true,
        }
    }

    // r[verify hardware-output.testbench]
    #[test]
    fn vector_collateral_derives_expected_values_from_evaluator() {
        let collateral = hdl_generate_vector_collateral(
            eval_case(),
            HdlEmitterMode::Verilog,
            hdl_example_module_name(HdlExampleKind::BitwiseAnd),
        )
        .expect("vector collateral");
        assert_eq!(collateral.expected_output, TEST_EXPECTED_AND);
        assert_eq!(collateral.expected_next_state, HDL_EMPTY_COUNT);
        assert_eq!(collateral.mode_label, "verilog");
    }

    // r[verify hardware-output.testbench]
    // r[verify hardware-output.testing.fail-closed]
    #[test]
    fn evaluator_rejection_blocks_vector_collateral() {
        let malformed = HdlEvalCase {
            module: HdlModuleShape {
                register_count: TEST_REGISTER_COUNT,
                clock_count: HDL_SINGLE_CLOCK,
                reset_count: HDL_OPTIONAL_RESET_LIMIT,
                unsupported_node_count: TEST_EXPRS,
                ..module()
            },
            ..eval_case()
        };
        assert!(matches!(
            hdl_generate_vector_collateral(
                malformed,
                HdlEmitterMode::SystemVerilog,
                hdl_example_module_name(HdlExampleKind::RegisterStage)
            ),
            Err(HdlCollateralError::EvaluatorRejected(_))
        ));
    }

    // r[verify hardware-output.validation]
    // r[verify hardware-output.testing.tool-evidence]
    #[test]
    fn validation_classifies_pass_fail_and_skipped() {
        assert_eq!(
            hdl_classify_tool_observation(passing_observation(HdlTool::Verilator)).classification,
            HdlToolClassification::Pass
        );

        let failed = HdlToolObservation {
            exit_success: false,
            ..passing_observation(HdlTool::Yosys)
        };
        assert_eq!(hdl_classify_tool_observation(failed), HdlToolEvidence {
            tool: HdlTool::Yosys,
            classification: HdlToolClassification::Fail,
            reason: HDL_VALIDATION_REASON_EXIT_FAILURE,
        });

        let skipped = HdlToolObservation {
            tool_available: false,
            ..passing_observation(HdlTool::SymbiYosys)
        };
        assert_eq!(hdl_classify_tool_observation(skipped), HdlToolEvidence {
            tool: HdlTool::SymbiYosys,
            classification: HdlToolClassification::Skipped,
            reason: HDL_VALIDATION_REASON_TOOL_UNAVAILABLE,
        });
    }

    // r[verify hardware-output.validation]
    // r[verify hardware-output.testing.fail-closed]
    #[test]
    fn invalid_validation_input_records_failure_not_skip_or_pass() {
        let invalid = HdlToolObservation {
            input_artifact_traceable: false,
            ..passing_observation(HdlTool::Verilator)
        };
        assert_eq!(hdl_classify_tool_observation(invalid).classification, HdlToolClassification::Fail);
    }

    // r[verify hardware-output.backend-admission]
    #[test]
    fn backend_admission_requires_every_check_and_names_limits() {
        let accepted = hdl_backend_admission(HdlBackendAdmissionInput {
            ir_admitted: true,
            adapter_admitted: true,
            adapter_refined: true,
            emitter_supported: true,
            collateral_eligible: true,
            validation_recorded: true,
            names_non_claim_limits: true,
        });
        assert!(accepted.admitted);
        assert_eq!(accepted.limits, HDL_BACKEND_LIMITS);

        let rejected = hdl_backend_admission(HdlBackendAdmissionInput {
            adapter_refined: false,
            ..HdlBackendAdmissionInput {
                ir_admitted: true,
                adapter_admitted: true,
                adapter_refined: true,
                emitter_supported: true,
                collateral_eligible: true,
                validation_recorded: true,
                names_non_claim_limits: true,
            }
        });
        assert!(!rejected.admitted);
    }
}
