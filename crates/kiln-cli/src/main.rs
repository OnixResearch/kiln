use std::path::Path;

use kiln_codec::identity_from_bytes;
use kiln_core::domain::{Command, CommandEnvelope};
use kiln_core::types::{
    AttemptId, CONTRACT_VERSION_V1, CommandId, EffectId, ProviderProfileId, RunId,
};
use kiln_standalone::{CommitMode, MemoryJournal};

const COMMAND_DEMO: &str = "demo";
const COMMAND_EVIDENCE: &str = "evidence";
const EXIT_FAILURE: i32 = 1;
const EXIT_USAGE: i32 = 2;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let Some(command) = arguments.next() else {
        fail("usage: kiln demo | kiln evidence INPUT OUTPUT", EXIT_USAGE);
    };
    let result = match command.as_str() {
        COMMAND_DEMO => demo(),
        COMMAND_EVIDENCE => {
            let Some(input) = arguments.next() else {
                fail("usage: kiln evidence INPUT OUTPUT", EXIT_USAGE);
            };
            let Some(output) = arguments.next() else {
                fail("usage: kiln evidence INPUT OUTPUT", EXIT_USAGE);
            };
            generate_evidence(Path::new(&input), Path::new(&output))
        }
        _ => Err(format!("unsupported command: {command}")),
    };
    match result {
        Ok(output) if !output.is_empty() => println!("{output}"),
        Ok(_) => {}
        Err(message) => fail(&message, EXIT_FAILURE),
    }
}

fn fail(message: &str, code: i32) -> ! {
    eprintln!("{message}");
    std::process::exit(code);
}

fn demo() -> Result<String, String> {
    let command = CommandEnvelope {
        contract_version: CONTRACT_VERSION_V1,
        command_id: CommandId::new("demo-start").map_err(|error| error.message)?,
        expected_revision: 0,
        command: Command::StartRun {
            run_id: RunId::new("demo-run").map_err(|error| error.message)?,
            attempt_id: AttemptId::new("demo-attempt").map_err(|error| error.message)?,
            provider_profile: ProviderProfileId::new("standalone-build-v1")
                .map_err(|error| error.message)?,
            effect_id: EffectId::new("demo-effect").map_err(|error| error.message)?,
        },
    };
    let mut journal = MemoryJournal::default();
    journal
        .transact(&command, CommitMode::Commit)
        .map_err(|error| error.message)?;
    serde_json::to_string(&journal.snapshot().state).map_err(|error| error.to_string())
}

fn build_evidence(mut value: serde_json::Value) -> Result<(serde_json::Value, String), String> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| "evidence input must be a JSON object".to_string())?;
    object.remove("receipt_hash");
    let canonical = serde_json::to_vec(&value)
        .map_err(|error| format!("evidence canonical encoding failed: {error}"))?;
    let receipt_hash = identity_from_bytes(&canonical)
        .map_err(|error| format!("evidence BLAKE3 identity failed: {error}"))?;
    let receipt_hash_text = receipt_hash.as_str().to_string();
    let object = value
        .as_object_mut()
        .ok_or_else(|| "evidence input must remain a JSON object".to_string())?;
    object.insert(
        "receipt_hash".to_string(),
        serde_json::Value::String(receipt_hash_text.clone()),
    );
    Ok((value, receipt_hash_text))
}

fn generate_evidence(input: &Path, output: &Path) -> Result<String, String> {
    let source =
        std::fs::read(input).map_err(|error| format!("evidence input read failed: {error}"))?;
    let value: serde_json::Value = serde_json::from_slice(&source)
        .map_err(|error| format!("evidence JSON parse failed: {error}"))?;
    let (evidence, receipt_hash) = build_evidence(value)?;
    let rendered = serde_json::to_vec_pretty(&evidence)
        .map_err(|error| format!("evidence output encoding failed: {error}"))?;
    std::fs::write(output, rendered)
        .map_err(|error| format!("evidence output write failed: {error}"))?;
    Ok(receipt_hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_emits_canonical_state() {
        assert!(demo().is_ok());
    }

    #[test]
    fn evidence_identity_is_deterministic() {
        let value = serde_json::json!({"schema": "kiln.local-validation.v1", "valid": true});
        assert_eq!(build_evidence(value.clone()), build_evidence(value));
    }

    #[test]
    fn evidence_rejects_missing_input() {
        let result = generate_evidence(
            Path::new("fixtures/missing-evidence.json"),
            Path::new("target/missing-evidence-output.json"),
        );
        assert!(result.is_err());
    }
}
