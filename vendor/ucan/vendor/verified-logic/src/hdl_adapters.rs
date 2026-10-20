//! Trellis-to-hardware adapter admission and refinement checks.
//!
//! Adapter lowering is explicit metadata, not Rust syntax compilation. The pure
//! checks below reject unsupported runtime shapes and compare supported helpers
//! with the hardware IR evaluator over admitted finite inputs.

#![allow(clippy::struct_excessive_bools)]

use crate::bitfield::extract_field;
use crate::hdl_emit::HdlEmitError;
use crate::hdl_emit::HdlEmitRequest;
use crate::hdl_emit::HdlEmitterMode;
use crate::hdl_emit::HdlExampleKind;
use crate::hdl_emit::emit_hdl;
use crate::hdl_ir::HDL_EMPTY_COUNT;
use crate::hdl_ir::HDL_MAX_WIDTH;
use crate::hdl_ir::HDL_OPTIONAL_RESET_LIMIT;
use crate::hdl_ir::HDL_SINGLE_CLOCK;
use crate::hdl_ir::HdlEvalCase;
use crate::hdl_ir::HdlEvalError;
use crate::hdl_ir::HdlExprKind;
use crate::hdl_ir::HdlModuleShape;
use crate::hdl_ir::hdl_eval_step;
use crate::hdl_ir::hdl_width_admitted;
use crate::hdl_ir::hdl_width_mask;

pub const HDL_ADAPTER_LOW_FIELD_OFFSET: u16 = 0;
pub const HDL_ADAPTER_BITFIELD_MAX_WIDTH: u16 = HDL_MAX_WIDTH - 1;
pub const HDL_ADAPTER_SINGLE_OUTPUT: u16 = 1;
pub const HDL_ADAPTER_BINARY_INPUTS: u16 = 2;
pub const HDL_ADAPTER_SINGLE_EXPR: u16 = 1;
pub const HDL_ADAPTER_SINGLE_REGISTER: u16 = 1;
pub const HDL_ADAPTER_FSM_STATES: u16 = 2;
pub const HDL_ADAPTER_ZERO_STATE: u16 = 0;
pub const HDL_ADAPTER_ONE_STATE: u16 = 1;
pub const HDL_ADAPTER_EMITTED_BITWISE_WIDTH: u16 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdlAdapterKind {
    BitwiseAnd,
    BitfieldLowExtract,
    WrappingAdd,
    SmallFsmToggle,
    UnsupportedHeap,
    UnsupportedIo,
    UnsupportedAsync,
    UnsupportedCrypto,
    UnsupportedDistributed,
    UnsupportedSpecOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdlAdapterError {
    UnsupportedShape,
    OutOfDomain,
    EvaluatorRejected(HdlEvalError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdlAdapterEmissionError {
    UnsupportedShape,
    OutOfDomain,
    UnsupportedOutput,
    EvaluatorRejected(HdlEvalError),
    EmitterRejected(HdlEmitError),
    RefinementFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HdlAdapterEmissionPlan {
    pub request: HdlEmitRequest,
    pub expected_observable: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HdlAdapterCandidate {
    pub kind: HdlAdapterKind,
    pub width: u16,
    pub bit_offset: u16,
    pub uses_heap: bool,
    pub uses_unbounded_vector: bool,
    pub uses_io: bool,
    pub uses_async: bool,
    pub crosses_crypto_boundary: bool,
    pub executes_distributed_protocol: bool,
    pub recursive_generated_hardware: bool,
    pub spec_only: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HdlAdapterInput {
    pub lhs: u64,
    pub rhs: u64,
    pub select: bool,
    pub current_state: u16,
}

fn hdl_adapter_unsupported_flag(candidate: HdlAdapterCandidate) -> bool {
    candidate.uses_heap
        || candidate.uses_unbounded_vector
        || candidate.uses_io
        || candidate.uses_async
        || candidate.crosses_crypto_boundary
        || candidate.executes_distributed_protocol
        || candidate.recursive_generated_hardware
        || candidate.spec_only
}

fn hdl_adapter_kind_supported(kind: HdlAdapterKind) -> bool {
    matches!(
        kind,
        HdlAdapterKind::BitwiseAnd
            | HdlAdapterKind::BitfieldLowExtract
            | HdlAdapterKind::WrappingAdd
            | HdlAdapterKind::SmallFsmToggle
    )
}

fn hdl_adapter_emission_error(error: HdlAdapterError) -> HdlAdapterEmissionError {
    match error {
        HdlAdapterError::UnsupportedShape => HdlAdapterEmissionError::UnsupportedShape,
        HdlAdapterError::OutOfDomain => HdlAdapterEmissionError::OutOfDomain,
        HdlAdapterError::EvaluatorRejected(error) => HdlAdapterEmissionError::EvaluatorRejected(error),
    }
}

// r[impl hardware-output.adapter-subset]
pub fn hdl_adapter_eligible(candidate: HdlAdapterCandidate) -> bool {
    hdl_adapter_kind_supported(candidate.kind)
        && !hdl_adapter_unsupported_flag(candidate)
        && hdl_width_admitted(candidate.width)
        && match candidate.kind {
            HdlAdapterKind::BitfieldLowExtract => {
                candidate.bit_offset == HDL_ADAPTER_LOW_FIELD_OFFSET
                    && candidate.width <= HDL_ADAPTER_BITFIELD_MAX_WIDTH
            }
            HdlAdapterKind::SmallFsmToggle => candidate.width == HDL_ADAPTER_SINGLE_OUTPUT,
            HdlAdapterKind::BitwiseAnd | HdlAdapterKind::WrappingAdd => true,
            HdlAdapterKind::UnsupportedHeap
            | HdlAdapterKind::UnsupportedIo
            | HdlAdapterKind::UnsupportedAsync
            | HdlAdapterKind::UnsupportedCrypto
            | HdlAdapterKind::UnsupportedDistributed
            | HdlAdapterKind::UnsupportedSpecOnly => false,
        }
}

fn base_module(candidate: HdlAdapterCandidate) -> HdlModuleShape {
    HdlModuleShape {
        input_count: HDL_ADAPTER_BINARY_INPUTS,
        output_count: HDL_ADAPTER_SINGLE_OUTPUT,
        expr_count: HDL_ADAPTER_SINGLE_EXPR,
        register_count: HDL_EMPTY_COUNT,
        clock_count: HDL_EMPTY_COUNT,
        reset_count: HDL_EMPTY_COUNT,
        fsm_state_count: HDL_EMPTY_COUNT,
        value_width: candidate.width,
        has_boolean_expr: true,
        canonical_order: true,
        unsupported_node_count: HDL_EMPTY_COUNT,
        malformed_width_count: HDL_EMPTY_COUNT,
    }
}

// r[impl hardware-output.adapter-subset]
pub fn hdl_lower_adapter(candidate: HdlAdapterCandidate) -> Result<HdlModuleShape, HdlAdapterError> {
    if !hdl_adapter_eligible(candidate) {
        return Err(HdlAdapterError::UnsupportedShape);
    }
    let module = match candidate.kind {
        HdlAdapterKind::SmallFsmToggle => HdlModuleShape {
            register_count: HDL_ADAPTER_SINGLE_REGISTER,
            clock_count: HDL_SINGLE_CLOCK,
            reset_count: HDL_OPTIONAL_RESET_LIMIT,
            fsm_state_count: HDL_ADAPTER_FSM_STATES,
            ..base_module(candidate)
        },
        HdlAdapterKind::BitwiseAnd | HdlAdapterKind::BitfieldLowExtract | HdlAdapterKind::WrappingAdd => {
            base_module(candidate)
        }
        HdlAdapterKind::UnsupportedHeap
        | HdlAdapterKind::UnsupportedIo
        | HdlAdapterKind::UnsupportedAsync
        | HdlAdapterKind::UnsupportedCrypto
        | HdlAdapterKind::UnsupportedDistributed
        | HdlAdapterKind::UnsupportedSpecOnly => return Err(HdlAdapterError::UnsupportedShape),
    };
    Ok(module)
}

fn hdl_adapter_op(kind: HdlAdapterKind) -> Result<HdlExprKind, HdlAdapterError> {
    match kind {
        HdlAdapterKind::BitwiseAnd => Ok(HdlExprKind::BitwiseAnd),
        HdlAdapterKind::BitfieldLowExtract | HdlAdapterKind::SmallFsmToggle => Ok(HdlExprKind::PassLhs),
        HdlAdapterKind::WrappingAdd => Ok(HdlExprKind::WrappingAdd),
        HdlAdapterKind::UnsupportedHeap
        | HdlAdapterKind::UnsupportedIo
        | HdlAdapterKind::UnsupportedAsync
        | HdlAdapterKind::UnsupportedCrypto
        | HdlAdapterKind::UnsupportedDistributed
        | HdlAdapterKind::UnsupportedSpecOnly => Err(HdlAdapterError::UnsupportedShape),
    }
}

fn hdl_adapter_emitted_example(
    candidate: HdlAdapterCandidate,
    mode: HdlEmitterMode,
) -> Result<HdlExampleKind, HdlAdapterEmissionError> {
    match candidate.kind {
        HdlAdapterKind::BitwiseAnd if candidate.width == HDL_ADAPTER_EMITTED_BITWISE_WIDTH => {
            Ok(HdlExampleKind::BitwiseAnd)
        }
        HdlAdapterKind::SmallFsmToggle if mode == HdlEmitterMode::SystemVerilog => Ok(HdlExampleKind::FsmToggle),
        HdlAdapterKind::BitwiseAnd
        | HdlAdapterKind::BitfieldLowExtract
        | HdlAdapterKind::WrappingAdd
        | HdlAdapterKind::SmallFsmToggle => Err(HdlAdapterEmissionError::UnsupportedOutput),
        HdlAdapterKind::UnsupportedHeap
        | HdlAdapterKind::UnsupportedIo
        | HdlAdapterKind::UnsupportedAsync
        | HdlAdapterKind::UnsupportedCrypto
        | HdlAdapterKind::UnsupportedDistributed
        | HdlAdapterKind::UnsupportedSpecOnly => Err(HdlAdapterEmissionError::UnsupportedShape),
    }
}

fn hdl_source_result(candidate: HdlAdapterCandidate, input: HdlAdapterInput) -> Result<u64, HdlAdapterError> {
    if !hdl_adapter_eligible(candidate) {
        return Err(HdlAdapterError::UnsupportedShape);
    }
    let mask = hdl_width_mask(candidate.width);
    let result = match candidate.kind {
        HdlAdapterKind::BitwiseAnd => (input.lhs & input.rhs) & mask,
        HdlAdapterKind::BitfieldLowExtract => {
            extract_field(input.lhs, u64::from(HDL_ADAPTER_LOW_FIELD_OFFSET), u64::from(candidate.width))
        }
        HdlAdapterKind::WrappingAdd => input.lhs.wrapping_add(input.rhs) & mask,
        HdlAdapterKind::SmallFsmToggle => {
            if input.lhs & mask == HDL_EMPTY_COUNT.into() {
                u64::from(input.current_state)
            } else if input.current_state == HDL_ADAPTER_ZERO_STATE {
                u64::from(HDL_ADAPTER_ONE_STATE)
            } else {
                u64::from(HDL_ADAPTER_ZERO_STATE)
            }
        }
        HdlAdapterKind::UnsupportedHeap
        | HdlAdapterKind::UnsupportedIo
        | HdlAdapterKind::UnsupportedAsync
        | HdlAdapterKind::UnsupportedCrypto
        | HdlAdapterKind::UnsupportedDistributed
        | HdlAdapterKind::UnsupportedSpecOnly => return Err(HdlAdapterError::UnsupportedShape),
    };
    Ok(result)
}

fn hdl_evaluator_result(candidate: HdlAdapterCandidate, input: HdlAdapterInput) -> Result<u64, HdlAdapterError> {
    let module = hdl_lower_adapter(candidate)?;
    let op = hdl_adapter_op(candidate.kind)?;
    let step = hdl_eval_step(HdlEvalCase {
        module,
        op,
        lhs: input.lhs,
        rhs: input.rhs,
        select: input.select,
        input_present: true,
        input_width: candidate.width,
        current_state: input.current_state,
    })
    .map_err(HdlAdapterError::EvaluatorRejected)?;

    if candidate.kind == HdlAdapterKind::SmallFsmToggle {
        Ok(u64::from(step.next_state))
    } else {
        Ok(step.output)
    }
}

// r[impl hardware-output.adapter-refinement]
// r[impl hardware-output.testing.adapter-property]
pub fn hdl_adapter_refines_source(
    candidate: HdlAdapterCandidate,
    input: HdlAdapterInput,
) -> Result<bool, HdlAdapterError> {
    if candidate.kind == HdlAdapterKind::SmallFsmToggle && input.current_state >= HDL_ADAPTER_FSM_STATES {
        return Err(HdlAdapterError::OutOfDomain);
    }
    let source = hdl_source_result(candidate, input)?;
    let lowered = hdl_evaluator_result(candidate, input)?;
    Ok(source == lowered)
}

// r[impl hardware-output.adapter-emission]
pub fn hdl_adapter_emission_plan(
    candidate: HdlAdapterCandidate,
    input: HdlAdapterInput,
    mode: HdlEmitterMode,
) -> Result<HdlAdapterEmissionPlan, HdlAdapterEmissionError> {
    if candidate.kind == HdlAdapterKind::SmallFsmToggle && input.current_state >= HDL_ADAPTER_FSM_STATES {
        return Err(HdlAdapterEmissionError::OutOfDomain);
    }

    let source = hdl_source_result(candidate, input).map_err(hdl_adapter_emission_error)?;
    let lowered = hdl_evaluator_result(candidate, input).map_err(hdl_adapter_emission_error)?;
    if source != lowered {
        return Err(HdlAdapterEmissionError::RefinementFailed);
    }

    let module = hdl_lower_adapter(candidate).map_err(hdl_adapter_emission_error)?;
    let kind = hdl_adapter_emitted_example(candidate, mode)?;
    let request = HdlEmitRequest {
        module,
        mode,
        kind,
        include_assertion_snippet: kind == HdlExampleKind::FsmToggle,
        reserved_identifier: false,
        unsupported_construct: false,
    };

    match emit_hdl(request) {
        Ok(_) => Ok(HdlAdapterEmissionPlan {
            request,
            expected_observable: lowered,
        }),
        Err(error) => Err(HdlAdapterEmissionError::EmitterRejected(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_WIDTH: u16 = 8;
    const TEST_BITFIELD_WIDTH: u16 = 4;
    const TEST_LHS: u64 = 0b1010_1100;
    const TEST_RHS: u64 = 0b0110_1010;
    const TEST_LOW_FIELD: u64 = 0b1100;
    const TEST_AND: u64 = 0b0010_1000;
    const TEST_ADD: u64 = 0b0001_0110;
    const OUT_OF_DOMAIN_STATE: u16 = HDL_ADAPTER_FSM_STATES;

    fn candidate(kind: HdlAdapterKind, width: u16) -> HdlAdapterCandidate {
        HdlAdapterCandidate {
            kind,
            width,
            bit_offset: HDL_ADAPTER_LOW_FIELD_OFFSET,
            uses_heap: false,
            uses_unbounded_vector: false,
            uses_io: false,
            uses_async: false,
            crosses_crypto_boundary: false,
            executes_distributed_protocol: false,
            recursive_generated_hardware: false,
            spec_only: false,
        }
    }

    fn input() -> HdlAdapterInput {
        HdlAdapterInput {
            lhs: TEST_LHS,
            rhs: TEST_RHS,
            select: false,
            current_state: HDL_ADAPTER_ZERO_STATE,
        }
    }

    // r[verify hardware-output.adapter-subset]
    // r[verify hardware-output.testing.adapter-property]
    #[test]
    fn admits_supported_bitwise_bitfield_arithmetic_and_fsm_adapters() {
        assert!(hdl_adapter_eligible(candidate(HdlAdapterKind::BitwiseAnd, TEST_WIDTH)));
        assert!(hdl_adapter_eligible(candidate(HdlAdapterKind::BitfieldLowExtract, TEST_BITFIELD_WIDTH)));
        assert!(hdl_adapter_eligible(candidate(HdlAdapterKind::WrappingAdd, TEST_WIDTH)));
        assert!(hdl_adapter_eligible(candidate(HdlAdapterKind::SmallFsmToggle, HDL_ADAPTER_SINGLE_OUTPUT)));
    }

    // r[verify hardware-output.adapter-subset]
    // r[verify hardware-output.testing.fail-closed]
    #[test]
    fn rejects_heap_io_async_crypto_distributed_and_spec_only_candidates() {
        let rejected_kinds = [
            HdlAdapterKind::UnsupportedHeap,
            HdlAdapterKind::UnsupportedIo,
            HdlAdapterKind::UnsupportedAsync,
            HdlAdapterKind::UnsupportedCrypto,
            HdlAdapterKind::UnsupportedDistributed,
            HdlAdapterKind::UnsupportedSpecOnly,
        ];
        for kind in rejected_kinds {
            let unsupported = candidate(kind, TEST_WIDTH);
            assert!(!hdl_adapter_eligible(unsupported));
            assert_eq!(hdl_lower_adapter(unsupported), Err(HdlAdapterError::UnsupportedShape));
        }

        let heap_backed = HdlAdapterCandidate {
            uses_heap: true,
            ..candidate(HdlAdapterKind::BitwiseAnd, TEST_WIDTH)
        };
        assert!(!hdl_adapter_eligible(heap_backed));
    }

    // r[verify hardware-output.adapter-refinement]
    // r[verify hardware-output.testing.adapter-property]
    #[test]
    fn supported_adapters_match_source_semantics() {
        assert_eq!(hdl_source_result(candidate(HdlAdapterKind::BitwiseAnd, TEST_WIDTH), input()), Ok(TEST_AND));
        assert_eq!(
            hdl_source_result(candidate(HdlAdapterKind::BitfieldLowExtract, TEST_BITFIELD_WIDTH), input()),
            Ok(TEST_LOW_FIELD)
        );
        assert_eq!(hdl_source_result(candidate(HdlAdapterKind::WrappingAdd, TEST_WIDTH), input()), Ok(TEST_ADD));

        assert_eq!(hdl_adapter_refines_source(candidate(HdlAdapterKind::BitwiseAnd, TEST_WIDTH), input()), Ok(true));
        assert_eq!(
            hdl_adapter_refines_source(candidate(HdlAdapterKind::BitfieldLowExtract, TEST_BITFIELD_WIDTH), input()),
            Ok(true)
        );
        assert_eq!(hdl_adapter_refines_source(candidate(HdlAdapterKind::WrappingAdd, TEST_WIDTH), input()), Ok(true));
    }

    // r[verify hardware-output.adapter-refinement]
    // r[verify hardware-output.testing.adapter-property]
    #[test]
    fn finite_fsm_adapter_matches_source_next_state() {
        let fsm = candidate(HdlAdapterKind::SmallFsmToggle, HDL_ADAPTER_SINGLE_OUTPUT);
        assert_eq!(hdl_adapter_refines_source(fsm, input()), Ok(true));

        let invalid = HdlAdapterInput {
            current_state: OUT_OF_DOMAIN_STATE,
            ..input()
        };
        assert_eq!(hdl_adapter_refines_source(fsm, invalid), Err(HdlAdapterError::OutOfDomain));
    }

    // r[verify hardware-output.adapter-emission]
    // r[verify hardware-output.testing.adapter-property]
    #[test]
    fn adapter_emission_plan_accepts_supported_emitted_shapes() {
        let bitwise = hdl_adapter_emission_plan(
            candidate(HdlAdapterKind::BitwiseAnd, TEST_WIDTH),
            input(),
            HdlEmitterMode::Verilog,
        )
        .expect("bitwise adapter emission plan");
        assert_eq!(bitwise.request.kind, HdlExampleKind::BitwiseAnd);
        assert_eq!(bitwise.request.mode, HdlEmitterMode::Verilog);
        assert_eq!(bitwise.expected_observable, TEST_AND);
        assert!(emit_hdl(bitwise.request).is_ok());

        let fsm_input = HdlAdapterInput {
            lhs: u64::from(HDL_ADAPTER_ONE_STATE),
            ..input()
        };
        let fsm = hdl_adapter_emission_plan(
            candidate(HdlAdapterKind::SmallFsmToggle, HDL_ADAPTER_SINGLE_OUTPUT),
            fsm_input,
            HdlEmitterMode::SystemVerilog,
        )
        .expect("fsm adapter emission plan");
        assert_eq!(fsm.request.kind, HdlExampleKind::FsmToggle);
        assert_eq!(fsm.request.mode, HdlEmitterMode::SystemVerilog);
        assert!(fsm.request.include_assertion_snippet);
        assert_eq!(fsm.expected_observable, u64::from(HDL_ADAPTER_ONE_STATE));
        assert!(emit_hdl(fsm.request).is_ok());
    }

    // r[verify hardware-output.adapter-emission]
    // r[verify hardware-output.testing.fail-closed]
    #[test]
    fn adapter_emission_plan_rejects_unemitted_or_invalid_shapes() {
        assert_eq!(
            hdl_adapter_emission_plan(
                candidate(HdlAdapterKind::WrappingAdd, TEST_WIDTH),
                input(),
                HdlEmitterMode::Verilog
            ),
            Err(HdlAdapterEmissionError::UnsupportedOutput)
        );
        assert_eq!(
            hdl_adapter_emission_plan(
                candidate(HdlAdapterKind::BitfieldLowExtract, TEST_BITFIELD_WIDTH),
                input(),
                HdlEmitterMode::SystemVerilog
            ),
            Err(HdlAdapterEmissionError::UnsupportedOutput)
        );
        assert_eq!(
            hdl_adapter_emission_plan(
                candidate(HdlAdapterKind::SmallFsmToggle, HDL_ADAPTER_SINGLE_OUTPUT),
                input(),
                HdlEmitterMode::Verilog
            ),
            Err(HdlAdapterEmissionError::UnsupportedOutput)
        );
        assert_eq!(
            hdl_adapter_emission_plan(
                candidate(HdlAdapterKind::BitwiseAnd, TEST_BITFIELD_WIDTH),
                input(),
                HdlEmitterMode::Verilog
            ),
            Err(HdlAdapterEmissionError::UnsupportedOutput)
        );
        assert_eq!(
            hdl_adapter_emission_plan(
                candidate(HdlAdapterKind::UnsupportedHeap, TEST_WIDTH),
                input(),
                HdlEmitterMode::Verilog
            ),
            Err(HdlAdapterEmissionError::UnsupportedShape)
        );

        let invalid = HdlAdapterInput {
            current_state: OUT_OF_DOMAIN_STATE,
            ..input()
        };
        assert_eq!(
            hdl_adapter_emission_plan(
                candidate(HdlAdapterKind::SmallFsmToggle, HDL_ADAPTER_SINGLE_OUTPUT),
                invalid,
                HdlEmitterMode::SystemVerilog
            ),
            Err(HdlAdapterEmissionError::OutOfDomain)
        );
    }
}
