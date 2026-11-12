mod dashboard;
mod qualification;

use std::path::Path;

use kiln_codec::identity_from_bytes;
use kiln_core::domain::{Command, CommandEnvelope};
use kiln_core::types::{
    AttemptId, CONTRACT_VERSION_V1, CommandId, EffectId, ProviderProfileId, RunId,
};
use kiln_standalone::{CommitMode, MemoryJournal};

const COMMAND_DEMO: &str = "demo";
const COMMAND_EVIDENCE: &str = "evidence";
const COMMAND_HOST_QUALIFY: &str = "host-qualify";
const COMMAND_DASHBOARD: &str = "dashboard";
const COMMAND_HELP: &str = "help";
const FLAG_HELP: &str = "--help";
const FLAG_HELP_SHORT: &str = "-h";
const FLAG_VERSION: &str = "--version";
const FLAG_VERSION_SHORT: &str = "-V";
/// The line a version request prints: the binary name and the package version.
const VERSION: &str = concat!("kiln ", env!("CARGO_PKG_VERSION"));
/// The usage text of the operator CLI. It names every accepted command.
const USAGE: &str = "\
usage: kiln <command> [arguments]

commands:
  demo
      commit one StartRun command to an in-memory journal and print the state;
      contacts no provider
  evidence INPUT OUTPUT
      write the JSON object INPUT to OUTPUT with its BLAKE3 receipt_hash and
      print the hash
  host-qualify COHORT RECEIPT ARTIFACT_ROOT
      check a host qualification receipt and its retained artifacts against
      the expected cohort
  dashboard --journal PATH --profile PATH
      render the read-only dashboard; `kiln dashboard --help` lists every option
  help, --help, -h
      print this text
  --version, -V
      print the version";
const QUALIFICATION_USAGE: &str = "usage: kiln host-qualify COHORT RECEIPT ARTIFACT_ROOT";
const EXIT_FAILURE: i32 = 1;
const EXIT_USAGE: i32 = 2;

// r[impl kiln.cli.help]
// r[impl kiln.cli.help.scenario.help]
// r[impl kiln.cli.help.scenario.refused]
// r[impl kiln.cli.version]
// r[impl kiln.cli.version.scenario.version]
// r[impl kiln.cli.version.scenario.refused]
fn main() {
    let mut arguments = std::env::args().skip(1);
    let Some(command) = arguments.next() else {
        fail(USAGE, EXIT_USAGE);
    };
    let result = match command.as_str() {
        COMMAND_HELP | FLAG_HELP | FLAG_HELP_SHORT => {
            answer(arguments, USAGE);
            return;
        }
        FLAG_VERSION | FLAG_VERSION_SHORT => {
            answer(arguments, VERSION);
            return;
        }
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
        COMMAND_HOST_QUALIFY => {
            let Some(cohort) = arguments.next() else {
                fail(QUALIFICATION_USAGE, EXIT_USAGE);
            };
            let Some(receipt) = arguments.next() else {
                fail(QUALIFICATION_USAGE, EXIT_USAGE);
            };
            let Some(artifacts) = arguments.next() else {
                fail(QUALIFICATION_USAGE, EXIT_USAGE);
            };
            if arguments.next().is_some() {
                fail(QUALIFICATION_USAGE, EXIT_USAGE);
            }
            qualification::qualify(
                Path::new(&cohort),
                Path::new(&receipt),
                Path::new(&artifacts),
            )
        }
        COMMAND_DASHBOARD => {
            let code = dashboard::dashboard(arguments);
            std::process::exit(code);
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

/// Answers a request that takes no argument: prints `text`, or reports a usage
/// error when another argument follows.
fn answer(mut rest: impl Iterator<Item = String>, text: &str) {
    if rest.next().is_some() {
        fail(USAGE, EXIT_USAGE);
    }
    println!("{text}");
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
