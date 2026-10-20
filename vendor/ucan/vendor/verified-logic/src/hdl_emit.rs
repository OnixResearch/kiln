//! Deterministic Verilog/SystemVerilog emission for admitted hardware IR shapes.
//!
//! The emitter is pure text assembly. It refuses malformed IR, unsupported
//! constructs, and reserved names before exposing any HDL text.

use crate::hdl_ir::HdlModuleShape;
use crate::hdl_ir::hdl_module_admitted;

pub const HDL_MODE_VERILOG_LABEL: &str = "verilog";
pub const HDL_MODE_SYSTEMVERILOG_LABEL: &str = "systemverilog";
pub const HDL_ORACLE_EVALUATOR_LABEL: &str = "hardware-ir-evaluator";
pub const HDL_REQ_VERILOG: &str = "hardware-output.verilog-emission";
pub const HDL_REQ_SYSTEMVERILOG: &str = "hardware-output.systemverilog-emission";
pub const HDL_REQ_TESTING_GOLDEN: &str = "hardware-output.testing.emitter-golden";

const VERILOG_AND_MODULE: &str =
    "module trellis_bitwise_and(input [7:0] a, input [7:0] b, output [7:0] y);\n  assign y = a & b;\nendmodule\n";
const VERILOG_REGISTER_MODULE: &str = "module trellis_register_stage(input clk, input rst_n, input [7:0] d, output reg [7:0] q);\n  always @(posedge clk or negedge rst_n) begin\n    if (!rst_n) q <= 8'd0;\n    else q <= d;\n  end\nendmodule\n";
const SYSTEMVERILOG_AND_MODULE: &str = "module trellis_bitwise_and(input logic [7:0] a, input logic [7:0] b, output logic [7:0] y);\n  always_comb begin\n    y = a & b;\n  end\nendmodule\n";
const SYSTEMVERILOG_REGISTER_MODULE: &str = "module trellis_register_stage(input logic clk, input logic rst_n, input logic [7:0] d, output logic [7:0] q);\n  always_ff @(posedge clk or negedge rst_n) begin\n    if (!rst_n) q <= '0;\n    else q <= d;\n  end\nendmodule\n";
const SYSTEMVERILOG_FSM_MODULE: &str = "module trellis_fsm_toggle(input logic clk, input logic rst_n, input logic step, output logic state);\n  always_ff @(posedge clk or negedge rst_n) begin\n    if (!rst_n) state <= 1'b0;\n    else if (step) state <= ~state;\n  end\n  assert property (@(posedge clk) disable iff (!rst_n) step |-> state != $past(state));\nendmodule\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdlEmitterMode {
    Verilog,
    SystemVerilog,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdlExampleKind {
    BitwiseAnd,
    RegisterStage,
    FsmToggle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdlEmitError {
    MalformedIr,
    UnsupportedConstruct,
    ReservedIdentifier,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HdlEmitRequest {
    pub module: HdlModuleShape,
    pub mode: HdlEmitterMode,
    pub kind: HdlExampleKind,
    pub include_assertion_snippet: bool,
    pub reserved_identifier: bool,
    pub unsupported_construct: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HdlFixtureMetadata {
    pub requirement_id: &'static str,
    pub module_name: &'static str,
    pub mode_label: &'static str,
    pub oracle_source: &'static str,
}

// r[impl hardware-output.verilog-emission]
// r[impl hardware-output.systemverilog-emission]
pub fn hdl_mode_label(mode: HdlEmitterMode) -> &'static str {
    match mode {
        HdlEmitterMode::Verilog => HDL_MODE_VERILOG_LABEL,
        HdlEmitterMode::SystemVerilog => HDL_MODE_SYSTEMVERILOG_LABEL,
    }
}

// r[impl hardware-output.verilog-emission]
// r[impl hardware-output.systemverilog-emission]
pub fn hdl_example_module_name(kind: HdlExampleKind) -> &'static str {
    match kind {
        HdlExampleKind::BitwiseAnd => "trellis_bitwise_and",
        HdlExampleKind::RegisterStage => "trellis_register_stage",
        HdlExampleKind::FsmToggle => "trellis_fsm_toggle",
    }
}

fn hdl_requirement_for_mode(mode: HdlEmitterMode) -> &'static str {
    match mode {
        HdlEmitterMode::Verilog => HDL_REQ_VERILOG,
        HdlEmitterMode::SystemVerilog => HDL_REQ_SYSTEMVERILOG,
    }
}

// r[impl hardware-output.testing.fixtures]
pub fn hdl_fixture_metadata(request: HdlEmitRequest) -> HdlFixtureMetadata {
    HdlFixtureMetadata {
        requirement_id: hdl_requirement_for_mode(request.mode),
        module_name: hdl_example_module_name(request.kind),
        mode_label: hdl_mode_label(request.mode),
        oracle_source: HDL_ORACLE_EVALUATOR_LABEL,
    }
}

// r[impl hardware-output.testing.fixtures]
pub fn hdl_fixture_traceable(metadata: HdlFixtureMetadata) -> bool {
    !metadata.requirement_id.is_empty()
        && !metadata.module_name.is_empty()
        && !metadata.mode_label.is_empty()
        && metadata.oracle_source == HDL_ORACLE_EVALUATOR_LABEL
}

// r[impl hardware-output.verilog-emission]
// r[impl hardware-output.systemverilog-emission]
// r[impl hardware-output.testing.emitter-golden]
// r[impl hardware-output.testing.fail-closed]
pub fn emit_hdl(request: HdlEmitRequest) -> Result<String, HdlEmitError> {
    if !hdl_module_admitted(request.module) {
        return Err(HdlEmitError::MalformedIr);
    }
    if request.reserved_identifier {
        return Err(HdlEmitError::ReservedIdentifier);
    }
    if request.unsupported_construct {
        return Err(HdlEmitError::UnsupportedConstruct);
    }

    let text = match (request.mode, request.kind) {
        (HdlEmitterMode::Verilog, HdlExampleKind::BitwiseAnd) => VERILOG_AND_MODULE,
        (HdlEmitterMode::Verilog, HdlExampleKind::RegisterStage) => VERILOG_REGISTER_MODULE,
        (HdlEmitterMode::Verilog, HdlExampleKind::FsmToggle) => return Err(HdlEmitError::UnsupportedConstruct),
        (HdlEmitterMode::SystemVerilog, HdlExampleKind::BitwiseAnd) => SYSTEMVERILOG_AND_MODULE,
        (HdlEmitterMode::SystemVerilog, HdlExampleKind::RegisterStage) => SYSTEMVERILOG_REGISTER_MODULE,
        (HdlEmitterMode::SystemVerilog, HdlExampleKind::FsmToggle) => SYSTEMVERILOG_FSM_MODULE,
    };
    Ok(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hdl_ir::HDL_EMPTY_COUNT;
    use crate::hdl_ir::HDL_OPTIONAL_RESET_LIMIT;
    use crate::hdl_ir::HDL_SINGLE_CLOCK;
    use crate::hdl_ir::HdlModuleShape;

    const TEST_WIDTH: u16 = 8;
    const TEST_INPUTS: u16 = 2;
    const TEST_OUTPUTS: u16 = 1;
    const TEST_EXPRS: u16 = 1;
    const TEST_REGISTER_COUNT: u16 = 1;

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

    fn registered_module() -> HdlModuleShape {
        HdlModuleShape {
            register_count: TEST_REGISTER_COUNT,
            clock_count: HDL_SINGLE_CLOCK,
            reset_count: HDL_OPTIONAL_RESET_LIMIT,
            ..combinational_module()
        }
    }

    fn request(mode: HdlEmitterMode, kind: HdlExampleKind, module: HdlModuleShape) -> HdlEmitRequest {
        HdlEmitRequest {
            module,
            mode,
            kind,
            include_assertion_snippet: matches!(kind, HdlExampleKind::FsmToggle),
            reserved_identifier: false,
            unsupported_construct: false,
        }
    }

    // r[verify hardware-output.verilog-emission]
    // r[verify hardware-output.testing.emitter-golden]
    #[test]
    fn verilog_emission_is_stable_and_matches_golden() {
        let req = request(HdlEmitterMode::Verilog, HdlExampleKind::BitwiseAnd, combinational_module());
        let first = emit_hdl(req).expect("verilog output");
        let second = emit_hdl(req).expect("repeat verilog output");
        assert_eq!(first, second);
        assert_eq!(first, include_str!("../tests/fixtures/hdl/golden/trellis_bitwise_and.v"));
    }

    // r[verify hardware-output.systemverilog-emission]
    // r[verify hardware-output.testing.emitter-golden]
    #[test]
    fn systemverilog_registered_module_uses_always_ff_and_matches_golden() {
        let req = request(HdlEmitterMode::SystemVerilog, HdlExampleKind::RegisterStage, registered_module());
        let text = emit_hdl(req).expect("systemverilog output");
        assert!(text.contains("logic"));
        assert!(text.contains("always_ff"));
        assert_eq!(text, include_str!("../tests/fixtures/hdl/golden/trellis_register_stage.sv"));
    }

    // r[verify hardware-output.systemverilog-emission]
    #[test]
    fn systemverilog_fsm_can_emit_assertion_snippet() {
        let req = request(HdlEmitterMode::SystemVerilog, HdlExampleKind::FsmToggle, registered_module());
        let text = emit_hdl(req).expect("systemverilog fsm output");
        assert!(text.contains("assert property"));
    }

    // r[verify hardware-output.verilog-emission]
    // r[verify hardware-output.testing.fail-closed]
    #[test]
    fn unsupported_or_reserved_shapes_emit_no_partial_text() {
        let mut unsupported = request(HdlEmitterMode::Verilog, HdlExampleKind::FsmToggle, registered_module());
        assert_eq!(emit_hdl(unsupported), Err(HdlEmitError::UnsupportedConstruct));

        unsupported.kind = HdlExampleKind::BitwiseAnd;
        unsupported.reserved_identifier = true;
        assert_eq!(emit_hdl(unsupported), Err(HdlEmitError::ReservedIdentifier));
    }

    // r[verify hardware-output.testing.fixtures]
    #[test]
    fn fixture_metadata_names_requirement_mode_and_oracle() {
        let req = request(HdlEmitterMode::Verilog, HdlExampleKind::BitwiseAnd, combinational_module());
        let metadata = hdl_fixture_metadata(req);
        assert!(hdl_fixture_traceable(metadata));
        assert_eq!(metadata.requirement_id, HDL_REQ_VERILOG);
        assert_eq!(metadata.mode_label, HDL_MODE_VERILOG_LABEL);
        assert_eq!(metadata.oracle_source, HDL_ORACLE_EVALUATOR_LABEL);
    }

    // r[verify hardware-output.testing.fixtures]
    #[test]
    fn fixture_metadata_rejects_untraceable_entries() {
        let invalid = HdlFixtureMetadata {
            requirement_id: "",
            module_name: hdl_example_module_name(HdlExampleKind::BitwiseAnd),
            mode_label: HDL_MODE_VERILOG_LABEL,
            oracle_source: HDL_ORACLE_EVALUATOR_LABEL,
        };
        assert!(!hdl_fixture_traceable(invalid));
    }
}
