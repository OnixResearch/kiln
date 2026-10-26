//! Generate an offline dashboard preview from representative canonical sample events.
//!
//! usage: cargo run -p kiln-dashboard --example preview -- <output-directory> [--live]
//! `--live` reloads the page at the admitted profile interval; it does not watch source files.

use std::error::Error;
use std::fs;
use std::path::PathBuf;

use kiln_codec::encode_and_identify;
use kiln_core::domain::{Command, CommandEnvelope, Outcome, State, decide, evolve};
use kiln_core::operations::JournalRecord;
use kiln_core::types::{
    AttemptId, CONTRACT_VERSION_V1, CommandId, CoreResult, Diagnostic, EffectId, ExternalRef,
    Limits, ProviderProfileId, RunId,
};
use kiln_dashboard::journal::{JournalDocument, MAX_EVENT_IDENTITY_BYTES, admit_document};
use kiln_dashboard::profile::ProfileDocument;
use kiln_dashboard::render::html::render_html_with_refresh;
use kiln_dashboard::report::{DashboardSelection, build_html_report};

const USAGE: &str = "usage: preview <output-directory> [--live]";
const MILLISECONDS_PER_SECOND: u64 = 1000;

fn failure(message: &str) -> Box<dyn Error> {
    Box::new(std::io::Error::other(message))
}

fn checked<T>(result: CoreResult<T>) -> Result<T, Box<dyn Error>> {
    result.map_err(|error| {
        failure(&format!(
            "{} at {}: {}",
            error.code, error.field, error.message
        ))
    })
}

// r[impl kiln.observe.dashboard.html.preview]
// r[impl kiln.observe.dashboard.html.preview.scenario.generate]
fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    let directory = arguments.next().ok_or_else(|| failure(USAGE))?;
    if directory == "--help" {
        println!("{USAGE}");
        return Ok(());
    }
    let live = match arguments.next().as_deref() {
        None => false,
        Some("--live") => true,
        Some(_) => return Err(failure(USAGE)),
    };
    if arguments.next().is_some() || directory.starts_with('-') {
        return Err(failure(USAGE));
    }
    let directory = PathBuf::from(directory);
    let profile: ProfileDocument =
        serde_json::from_str(include_str!("../../../fixtures/dashboard/profile-v1.json"))?;
    checked(profile.admit())?;
    let bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "schema": "kiln.dashboard-preview.synthetic.v1",
        "records": checked(preview_records())?,
    }))?;
    let document = checked(JournalDocument::parse(&bytes))?;
    let journal = checked(admit_document(&document))?;
    let report = checked(build_html_report(
        &profile.profile,
        &journal,
        &DashboardSelection {
            run: Some(checked(RunId::new("api-checks"))?),
            attempt: Some(checked(AttemptId::new("api-checks-2"))?),
        },
    ))?;
    let refresh = live.then(|| {
        profile
            .profile
            .watch_interval_ms
            .div_ceil(MILLISECONDS_PER_SECOND)
    });
    let html = render_html_with_refresh(&report, refresh);
    fs::create_dir_all(&directory)?;
    fs::write(directory.join("journal.json"), bytes)?;
    // Readers of a live preview must never observe a partially written document.
    let staged = directory.join("dashboard.html.next");
    fs::write(&staged, html)?;
    fs::rename(&staged, directory.join("dashboard.html"))?;
    let directory = fs::canonicalize(directory)?;
    println!("Synthetic local preview; no provider was contacted.");
    println!("Journal: {}", directory.join("journal.json").display());
    println!("Open: {}", directory.join("dashboard.html").display());
    if live {
        println!(
            "Page reload enabled. Re-run this command after source changes, or use the documented watchexec workflow."
        );
    }
    Ok(())
}

fn preview_records() -> CoreResult<Vec<JournalRecord>> {
    let mut commands = Vec::new();
    for (name, outcome) in [
        ("api-checks", Outcome::DomainFailure),
        ("integration", Outcome::DomainFailure),
        ("linux-build", Outcome::Pending),
        ("package", Outcome::Unknown),
        ("static-analysis", Outcome::Success),
        ("web-checks", Outcome::Cancelled),
    ] {
        let attempt_id = AttemptId::new(format!("{name}-1"))?;
        commands.push(Command::StartRun {
            run_id: RunId::new(name)?,
            attempt_id: attempt_id.clone(),
            provider_profile: ProviderProfileId::new("preview-build-v1")?,
            effect_id: EffectId::new(format!("start-{name}"))?,
        });
        match outcome {
            Outcome::Pending => {}
            Outcome::Unknown => commands.push(Command::RecordUnknown {
                attempt_id,
                external_reference: None,
            }),
            outcome => commands.push(Command::CompleteAttempt {
                attempt_id,
                outcome,
                external_reference: None,
                products: Vec::new(),
                metrics: Vec::new(),
            }),
        }
    }
    commands.push(Command::Retry {
        run_id: RunId::new("api-checks")?,
        prior_attempt_id: AttemptId::new("api-checks-1")?,
        next_attempt_id: AttemptId::new("api-checks-2")?,
        provider_profile: ProviderProfileId::new("preview-build-v1")?,
        effect_id: EffectId::new("retry-api-checks")?,
    });
    commands.push(Command::CompleteAttempt {
        attempt_id: AttemptId::new("api-checks-2")?,
        outcome: Outcome::Success,
        external_reference: None,
        products: Vec::new(),
        metrics: Vec::new(),
    });
    let mut state = State::default();
    let mut records = Vec::new();
    let stream = ExternalRef::new("kiln", "sample-ingress", "dashboard-preview")?;
    for (index, command) in commands.into_iter().enumerate() {
        let decision = decide(
            &state,
            &CommandEnvelope {
                contract_version: CONTRACT_VERSION_V1,
                command_id: CommandId::new(format!("preview-{index}"))?,
                expected_revision: state.revision,
                command,
            },
            Limits::default(),
        )?;
        // The preview records decisions only; requested provider effects are never executed.
        for event in decision.events {
            let expected_revision = state.revision;
            let (_, identity) =
                encode_and_identify(&event, MAX_EVENT_IDENTITY_BYTES).map_err(|error| {
                    Diagnostic::new("preview_event_encoding", "event", &error.to_string())
                })?;
            state = evolve(&state, &event)?;
            records.push(JournalRecord {
                schema_version: CONTRACT_VERSION_V1,
                stream: stream.clone(),
                expected_revision,
                event_identity: ExternalRef::new("kiln", "pilot-event-blake3", identity.as_str())?,
                event,
            });
        }
    }
    Ok(records)
}
