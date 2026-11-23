#[test]
fn stages_have_closed_spellings() -> Result<(), serde_json::Error> {
    for (stage, spelling) in [
        (cairn_gate_contract::GateStage::Proposal, "proposal"),
        (cairn_gate_contract::GateStage::Design, "design"),
        (cairn_gate_contract::GateStage::Tasks, "tasks"),
    ] {
        assert_eq!(stage.as_str(), spelling);
        assert_eq!(
            serde_json::to_value(stage)?,
            serde_json::Value::String(spelling.into())
        );
    }
    assert!(serde_json::from_str::<cairn_gate_contract::GateStage>("\"release\"").is_err());
    Ok(())
}

#[test]
fn verdicts_have_closed_spellings() -> Result<(), serde_json::Error> {
    for (verdict, spelling) in [
        (cairn_gate_contract::GateVerdict::Pass, "pass"),
        (cairn_gate_contract::GateVerdict::Block, "block"),
    ] {
        assert_eq!(verdict.as_str(), spelling);
        assert_eq!(
            serde_json::to_value(verdict)?,
            serde_json::Value::String(spelling.into())
        );
    }
    assert!(serde_json::from_str::<cairn_gate_contract::GateVerdict>("\"unknown\"").is_err());
    Ok(())
}

#[test]
fn severities_have_closed_spellings() -> Result<(), serde_json::Error> {
    for (severity, spelling) in [
        (cairn_gate_contract::DiagnosticSeverity::Error, "error"),
        (cairn_gate_contract::DiagnosticSeverity::Warning, "warning"),
        (cairn_gate_contract::DiagnosticSeverity::Info, "info"),
    ] {
        assert_eq!(severity.as_str(), spelling);
        assert_eq!(
            serde_json::to_value(severity)?,
            serde_json::Value::String(spelling.into())
        );
    }
    assert!(serde_json::from_str::<cairn_gate_contract::DiagnosticSeverity>("\"fatal\"").is_err());
    Ok(())
}
