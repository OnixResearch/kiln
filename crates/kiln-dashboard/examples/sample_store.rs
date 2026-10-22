//! Write a sample canonical SQLite event store for dashboard rehearsal.
//!
//! usage: cargo run -p kiln-dashboard --example sample_store -- <path>

use std::path::PathBuf;
use std::process::ExitCode;

use kiln_adapter_sqlite::{
    AppendCommitMode, SqliteEventStore, build_journal_record, canonical_profile,
};
use kiln_core::domain::{Command, CommandEnvelope, State, decide, evolve};
use kiln_core::types::{
    AttemptId, CONTRACT_VERSION_V1, CommandId, EffectId, ExternalRef, Limits, ProviderProfileId,
    RunId,
};
use kiln_provider_api::EventAppendRequest;

const RUNS: usize = 2;

fn failure(message: String) -> ExitCode {
    eprintln!("sample_store: {message}");
    ExitCode::FAILURE
}

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let Some(path) = arguments.next().map(PathBuf::from) else {
        eprintln!("usage: sample_store <path>");
        return ExitCode::FAILURE;
    };
    match write(&path) {
        Ok(()) => {
            println!("{}", path.display());
            ExitCode::SUCCESS
        }
        Err(message) => failure(message),
    }
}

fn write(path: &std::path::Path) -> Result<(), String> {
    let profile = canonical_profile();
    let stream =
        ExternalRef::new("kiln", "event-stream", "main").map_err(|error| format!("{error:?}"))?;
    let mut state = State::default();
    let mut records = Vec::new();
    for index in 0..RUNS {
        let run_id = RunId::new(format!("run-{index}")).map_err(|error| format!("{error:?}"))?;
        let attempt_id =
            AttemptId::new(format!("attempt-{index}")).map_err(|error| format!("{error:?}"))?;
        let provider_profile =
            ProviderProfileId::new("build-v1").map_err(|error| format!("{error:?}"))?;
        let effect_id =
            EffectId::new(format!("effect-{index}")).map_err(|error| format!("{error:?}"))?;
        let command_id =
            CommandId::new(format!("command-{index}")).map_err(|error| format!("{error:?}"))?;
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
        let decision =
            decide(&state, &envelope, Limits::default()).map_err(|error| format!("{error:?}"))?;
        for event in decision.events {
            let expected_revision = state.revision;
            state = evolve(&state, &event).map_err(|error| format!("{error:?}"))?;
            let record = build_journal_record(stream.clone(), expected_revision, event)
                .map_err(|error| format!("{error:?}"))?;
            records.push(record);
        }
    }
    let mut store = SqliteEventStore::open(path, profile).map_err(|error| format!("{error:?}"))?;
    store
        .append_with_mode(
            &EventAppendRequest {
                stream,
                expected_revision: 0,
                records,
                outbox: Vec::new(),
            },
            AppendCommitMode::Commit,
        )
        .map_err(|error| format!("{error:?}"))?;
    Ok(())
}
