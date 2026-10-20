//! Finite hardware IR admission and evaluator primitives.
//!
//! The public surface is a small pure core: structural admission decides whether
//! a Trellis-owned hardware shape is eligible for evaluation/emission, and the
//! evaluator computes one deterministic combinational or registered step.

#![allow(clippy::struct_excessive_bools)]

pub const HDL_MIN_WIDTH: u16 = 1;
pub const HDL_MAX_WIDTH: u16 = 64;
pub const HDL_MAX_PORTS: u16 = 32;
pub const HDL_MAX_NODES: u16 = 128;
pub const HDL_MAX_FSM_STATES: u16 = 32;
pub const HDL_SINGLE_CLOCK: u16 = 1;
pub const HDL_OPTIONAL_RESET_LIMIT: u16 = 1;
pub const HDL_EMPTY_COUNT: u16 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdlExprKind {
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    WrappingAdd,
    Equal,
    Mux,
    PassLhs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdlEvalError {
    MalformedIr,
    MissingInput,
    WidthMismatch,
    InvalidState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HdlModuleShape {
    pub input_count: u16,
    pub output_count: u16,
    pub expr_count: u16,
    pub register_count: u16,
    pub clock_count: u16,
    pub reset_count: u16,
    pub fsm_state_count: u16,
    pub value_width: u16,
    pub has_boolean_expr: bool,
    pub canonical_order: bool,
    pub unsupported_node_count: u16,
    pub malformed_width_count: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HdlEvalCase {
    pub module: HdlModuleShape,
    pub op: HdlExprKind,
    pub lhs: u64,
    pub rhs: u64,
    pub select: bool,
    pub input_present: bool,
    pub input_width: u16,
    pub current_state: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HdlEvalResult {
    pub output: u64,
    pub next_state: u16,
}

// r[impl hardware-output.ir]
pub fn hdl_width_admitted(width: u16) -> bool {
    (HDL_MIN_WIDTH..=HDL_MAX_WIDTH).contains(&width)
}

fn hdl_counts_fit(module: HdlModuleShape) -> bool {
    module.input_count <= HDL_MAX_PORTS
        && module.output_count <= HDL_MAX_PORTS - module.input_count
        && module.expr_count <= HDL_MAX_NODES
        && module.register_count <= HDL_MAX_NODES - module.expr_count
}

fn hdl_registered_shape_admitted(module: HdlModuleShape) -> bool {
    module.register_count == HDL_EMPTY_COUNT
        || (module.clock_count == HDL_SINGLE_CLOCK && module.reset_count <= HDL_OPTIONAL_RESET_LIMIT)
}

fn hdl_fsm_shape_admitted(module: HdlModuleShape) -> bool {
    module.fsm_state_count == HDL_EMPTY_COUNT
        || (module.register_count > HDL_EMPTY_COUNT
            && (HDL_MIN_WIDTH..=HDL_MAX_FSM_STATES).contains(&module.fsm_state_count))
}

// r[impl hardware-output.ir]
pub fn hdl_module_admitted(module: HdlModuleShape) -> bool {
    module.input_count > HDL_EMPTY_COUNT
        && module.output_count > HDL_EMPTY_COUNT
        && hdl_width_admitted(module.value_width)
        && hdl_counts_fit(module)
        && hdl_registered_shape_admitted(module)
        && hdl_fsm_shape_admitted(module)
        && module.canonical_order
        && module.unsupported_node_count == HDL_EMPTY_COUNT
        && module.malformed_width_count == HDL_EMPTY_COUNT
}

// r[impl hardware-output.evaluator]
pub fn hdl_width_mask(width: u16) -> u64 {
    if width == HDL_EMPTY_COUNT {
        HDL_EMPTY_COUNT.into()
    } else if width >= HDL_MAX_WIDTH {
        u64::MAX
    } else {
        (1u64 << u32::from(width)) - 1
    }
}

// r[impl hardware-output.evaluator]
pub fn hdl_eval_expr(op: HdlExprKind, lhs: u64, rhs: u64, select: bool, width: u16) -> u64 {
    let mask = hdl_width_mask(width);
    let left = lhs & mask;
    let right = rhs & mask;
    match op {
        HdlExprKind::BitwiseAnd => left & right,
        HdlExprKind::BitwiseOr => left | right,
        HdlExprKind::BitwiseXor => left ^ right,
        HdlExprKind::WrappingAdd => left.wrapping_add(right) & mask,
        HdlExprKind::Equal => u64::from(left == right),
        HdlExprKind::Mux => {
            if select {
                left
            } else {
                right
            }
        }
        HdlExprKind::PassLhs => left,
    }
}

fn hdl_next_state(module: HdlModuleShape, current_state: u16, output: u64) -> Result<u16, HdlEvalError> {
    if module.fsm_state_count == HDL_EMPTY_COUNT {
        return Ok(current_state);
    }
    if current_state >= module.fsm_state_count {
        return Err(HdlEvalError::InvalidState);
    }
    if output == HDL_EMPTY_COUNT.into() {
        return Ok(current_state);
    }
    let advanced = current_state + 1;
    if advanced >= module.fsm_state_count {
        Ok(HDL_EMPTY_COUNT)
    } else {
        Ok(advanced)
    }
}

// r[impl hardware-output.evaluator]
// r[impl hardware-output.testing.evaluator-oracle]
pub fn hdl_eval_step(case: HdlEvalCase) -> Result<HdlEvalResult, HdlEvalError> {
    if !hdl_module_admitted(case.module) {
        return Err(HdlEvalError::MalformedIr);
    }
    if !case.input_present {
        return Err(HdlEvalError::MissingInput);
    }
    if case.input_width != case.module.value_width {
        return Err(HdlEvalError::WidthMismatch);
    }
    let output = hdl_eval_expr(case.op, case.lhs, case.rhs, case.select, case.module.value_width);
    let next_state = hdl_next_state(case.module, case.current_state, output)?;
    Ok(HdlEvalResult { output, next_state })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_WIDTH: u16 = 8;
    const TEST_INPUTS: u16 = 2;
    const TEST_OUTPUTS: u16 = 1;
    const TEST_EXPRS: u16 = 1;
    const TEST_REGISTERS: u16 = 1;
    const TEST_STATES: u16 = 3;
    const TEST_LHS: u64 = 0b1010_1100;
    const TEST_RHS: u64 = 0b0110_1010;
    const TEST_AND_OUTPUT: u64 = 0b0010_1000;
    const OUT_OF_RANGE_WIDTH: u16 = HDL_MAX_WIDTH + 1;
    const INVALID_STATE: u16 = TEST_STATES;

    fn combinational_module() -> HdlModuleShape {
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

    fn registered_fsm_module() -> HdlModuleShape {
        HdlModuleShape {
            register_count: TEST_REGISTERS,
            clock_count: HDL_SINGLE_CLOCK,
            reset_count: HDL_OPTIONAL_RESET_LIMIT,
            fsm_state_count: TEST_STATES,
            ..combinational_module()
        }
    }

    fn eval_case(module: HdlModuleShape) -> HdlEvalCase {
        HdlEvalCase {
            module,
            op: HdlExprKind::BitwiseAnd,
            lhs: TEST_LHS,
            rhs: TEST_RHS,
            select: false,
            input_present: true,
            input_width: TEST_WIDTH,
            current_state: HDL_EMPTY_COUNT,
        }
    }

    // r[verify hardware-output.ir]
    // r[verify hardware-output.testing.evaluator-oracle]
    #[test]
    fn admits_valid_combinational_ir_and_rejects_width_mismatch() {
        let valid = combinational_module();
        assert!(hdl_module_admitted(valid));

        let invalid_width = HdlModuleShape {
            value_width: OUT_OF_RANGE_WIDTH,
            ..valid
        };
        assert!(!hdl_module_admitted(invalid_width));
    }

    // r[verify hardware-output.ir]
    // r[verify hardware-output.testing.fail-closed]
    #[test]
    fn rejects_unsupported_nodes_before_evaluation() {
        let malformed = HdlModuleShape {
            unsupported_node_count: TEST_EXPRS,
            ..combinational_module()
        };
        assert!(!hdl_module_admitted(malformed));
        assert_eq!(hdl_eval_step(eval_case(malformed)), Err(HdlEvalError::MalformedIr));
    }

    // r[verify hardware-output.evaluator]
    // r[verify hardware-output.testing.evaluator-oracle]
    #[test]
    fn evaluates_combinational_module_deterministically() {
        let first = hdl_eval_step(eval_case(combinational_module())).expect("admitted evaluator result");
        let second = hdl_eval_step(eval_case(combinational_module())).expect("repeat evaluator result");
        assert_eq!(first, second);
        assert_eq!(first.output, TEST_AND_OUTPUT);
    }

    // r[verify hardware-output.evaluator]
    // r[verify hardware-output.testing.evaluator-oracle]
    #[test]
    fn rejects_missing_inputs_width_mismatch_and_invalid_state() {
        let mut missing = eval_case(combinational_module());
        missing.input_present = false;
        assert_eq!(hdl_eval_step(missing), Err(HdlEvalError::MissingInput));

        let mut wrong_width = eval_case(combinational_module());
        wrong_width.input_width = OUT_OF_RANGE_WIDTH;
        assert_eq!(hdl_eval_step(wrong_width), Err(HdlEvalError::WidthMismatch));

        let mut invalid_state = eval_case(registered_fsm_module());
        invalid_state.current_state = INVALID_STATE;
        assert_eq!(hdl_eval_step(invalid_state), Err(HdlEvalError::InvalidState));
    }

    // r[verify hardware-output.evaluator]
    #[test]
    fn registered_fsm_step_advances_on_nonzero_output() {
        let result = hdl_eval_step(eval_case(registered_fsm_module())).expect("registered evaluator result");
        assert_eq!(result.next_state, HDL_MIN_WIDTH);
    }
}
