//! Write a sample canonical Kiln journal document for dashboard rehearsal.
//!
//! usage: cargo run -p kiln-dashboard --example sample_journal -- <path> [run-count]

use std::error::Error;
use std::fs;
use std::path::PathBuf;

use kiln_codec::encode_and_identify;
use kiln_core::domain::{Command, CommandEnvelope, State, decide, evolve};
use kiln_core::operations::JournalRecord;
use kiln_core::types::{
    AttemptId, CONTRACT_VERSION_V1, CommandId, EffectId, ExternalRef, Limits, ProviderProfileId,
    RunId,
};

const MAX_EVENT_IDENTITY_BYTES: usize = 4 * 1024 * 1024;

fn failure(message: String) -> Box<dyn Error> {
    Box::new(std::io::Error::other(message))
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    let Some(path) = arguments.next().map(PathBuf::from) else {
        return Err(failure(
            "usage: sample_journal <path> [run-count]".to_string(),
        ));
    };
    let run_count = match arguments.next() {
        Some(value) => value
            .parse::<usize>()
            .map_err(|error| failure(format!("run count: {error}")))?,
        None => 3,
    };
    let bytes = sample_journal(run_count)?;
    fs::write(&path, bytes).map_err(|error| failure(format!("{}: {error}", path.display())))?;
    println!("{}", path.display());
    Ok(())
}

fn sample_journal(run_count: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut state = State::default();
    let mut records: Vec<JournalRecord> = Vec::new();
    let stream = ExternalRef::new("kiln", "sample-ingress", "operation-1")
        .map_err(|error| failure(format!("stream: {}", error.message)))?;
    for index in 0..run_count {
        let run_id = RunId::new(format!("run-{index}"))
            .map_err(|error| failure(format!("run identity: {}", error.message)))?;
        let attempt_id = AttemptId::new(format!("attempt-{index}"))
            .map_err(|error| failure(format!("attempt identity: {}", error.message)))?;
        let provider_profile = ProviderProfileId::new("build-v1")
            .map_err(|error| failure(format!("provider identity: {}", error.message)))?;
        let effect_id = EffectId::new(format!("effect-{index}"))
            .map_err(|error| failure(format!("effect identity: {}", error.message)))?;
        let command_id = CommandId::new(format!("command-{index}"))
            .map_err(|error| failure(format!("command identity: {}", error.message)))?;
        let envelope = CommandEnvelope {
            contract_version: CONTRACT_VERSION_V1,
            command_id,
            expected_revision: state.revision,
            command: Command::StartRun {
                run_id,
                attempt_id,
                provider_profile,
                effect_id,
            },
        };
        let decision = decide(&state, &envelope, Limits::default())
            .map_err(|error| failure(format!("decision: {}", error.message)))?;
        for event in decision.events {
            let expected_revision = state.revision;
            let (_, content) = encode_and_identify(&event, MAX_EVENT_IDENTITY_BYTES)
                .map_err(|error| failure(format!("event encoding: {error}")))?;
            let event_identity =
                ExternalRef::new("kiln", "pilot-event-blake3", content.as_str())
                    .map_err(|error| failure(format!("event identity: {}", error.message)))?;
            state = evolve(&state, &event)
                .map_err(|error| failure(format!("evolve: {}", error.message)))?;
            records.push(JournalRecord {
                schema_version: CONTRACT_VERSION_V1,
                stream: stream.clone(),
                expected_revision,
                event_identity,
                event,
            });
        }
    }
    let document = serde_json::json!({
        "schema": "kiln.sample.journal.v1",
        "identity": "0000000000000000000000000000000000000000000000000000000000000000",
        "records": records,
    });
    serde_json::to_vec_pretty(&document).map_err(|error| failure(format!("encode: {error}")))
}
