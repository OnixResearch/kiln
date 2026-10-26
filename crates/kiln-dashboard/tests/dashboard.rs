use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Stdio};
use std::time::Duration;

use kiln_codec::{encode_and_identify, identity_from_bytes};
use kiln_core::domain::{Command, CommandEnvelope, Decision, Outcome, State, decide, evolve};
use kiln_core::operations::JournalRecord;
use kiln_core::types::{
    AttemptId, CONTRACT_VERSION_V1, CommandId, EffectId, ExternalRef, Limits, ProviderProfileId,
    RunId,
};
use kiln_dashboard::journal::{JournalDocument, admit_document};
use kiln_dashboard::profile::{DashboardProfile, ProfileDocument};
use kiln_dashboard::render::{RenderStyle, render_report};
use kiln_dashboard::report::{DashboardSelection, build_html_report, build_report};
use kiln_observe_core::{
    ObservationRequest, ObservationWindow, ViewBody, ViewField, ViewRequest, project_observation,
};

const MAX_EVENT_IDENTITY_BYTES: usize = 4 * 1024 * 1024;

fn expect_error<T>(
    result: Result<T, kiln_core::types::Diagnostic>,
) -> kiln_core::types::Diagnostic {
    match result {
        Ok(_) => unreachable!(),
        Err(error) => error,
    }
}

fn identity(value: &str) -> RunId {
    RunId::new(value).unwrap_or_else(|_| unreachable!())
}

fn attempt_identity(value: &str) -> AttemptId {
    AttemptId::new(value).unwrap_or_else(|_| unreachable!())
}

fn command_identity(value: &str) -> CommandId {
    CommandId::new(value).unwrap_or_else(|_| unreachable!())
}

fn effect_identity(value: &str) -> EffectId {
    EffectId::new(value).unwrap_or_else(|_| unreachable!())
}

fn provider_identity(value: &str) -> ProviderProfileId {
    ProviderProfileId::new(value).unwrap_or_else(|_| unreachable!())
}

fn stream() -> ExternalRef {
    ExternalRef::new("kiln", "pilot-ingress", "operation-1").unwrap_or_else(|_| unreachable!())
}

/// Build one canonical Kiln journal document with the given run count.
fn journal_document(run_count: usize) -> Vec<u8> {
    journal_commands((0..run_count).map(|index| Command::StartRun {
        run_id: identity(&format!("run-{index}")),
        attempt_id: attempt_identity(&format!("attempt-{index}")),
        provider_profile: provider_identity("build-v1"),
        effect_id: effect_identity(&format!("effect-{index}")),
    }))
}

fn journal_commands(commands: impl IntoIterator<Item = Command>) -> Vec<u8> {
    let mut state = State::default();
    let mut records: Vec<JournalRecord> = Vec::new();
    for (index, command) in commands.into_iter().enumerate() {
        let envelope = CommandEnvelope {
            contract_version: CONTRACT_VERSION_V1,
            command_id: command_identity(&format!("command-{index}")),
            expected_revision: state.revision,
            command,
        };
        let decision: Decision =
            decide(&state, &envelope, Limits::default()).unwrap_or_else(|_| unreachable!());
        for event in decision.events {
            let expected_revision = state.revision;
            let (_, content) = encode_and_identify(&event, MAX_EVENT_IDENTITY_BYTES)
                .unwrap_or_else(|_| unreachable!());
            let event_identity = ExternalRef::new("kiln", "pilot-event-blake3", content.as_str())
                .unwrap_or_else(|_| unreachable!());
            state = evolve(&state, &event).unwrap_or_else(|_| unreachable!());
            records.push(JournalRecord {
                schema_version: CONTRACT_VERSION_V1,
                stream: stream(),
                expected_revision,
                event_identity,
                event,
            });
        }
    }
    let mut envelope = BTreeMap::new();
    envelope.insert("schema", serde_json::json!("kiln.onixos-pilot.journal.v1"));
    envelope.insert(
        "identity",
        serde_json::json!("0000000000000000000000000000000000000000000000000000000000000000"),
    );
    envelope.insert(
        "records",
        serde_json::to_value(&records).unwrap_or_else(|_| unreachable!()),
    );
    serde_json::to_vec(&envelope).unwrap_or_else(|_| unreachable!())
}

fn dashboard_profile() -> DashboardProfile {
    let path = format!(
        "{}/../../fixtures/dashboard/profile-v1.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let bytes = std::fs::read(path).unwrap_or_else(|_| unreachable!());
    let document: ProfileDocument =
        serde_json::from_slice(&bytes).unwrap_or_else(|_| unreachable!());
    document.profile
}

fn admitted(run_count: usize) -> kiln_dashboard::journal::AdmittedJournal {
    let document =
        JournalDocument::parse(&journal_document(run_count)).unwrap_or_else(|_| unreachable!());
    admit_document(&document).unwrap_or_else(|_| unreachable!())
}

// r[verify kiln.observe.dashboard.profile]
// r[verify kiln.observe.dashboard.profile.scenario.match]
#[test]
fn exported_profile_is_admitted() {
    let path = format!(
        "{}/../../fixtures/dashboard/profile-v1.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let bytes = std::fs::read(path).unwrap_or_else(|_| unreachable!());
    let document: ProfileDocument =
        serde_json::from_slice(&bytes).unwrap_or_else(|_| unreachable!());
    document.admit().unwrap_or_else(|_| unreachable!());
    assert_eq!(document.profile.profile_id, "dashboard-v1");
}

// r[verify kiln.observe.dashboard.profile.scenario.zero_bound]
// r[verify kiln.observe.dashboard.profile.scenario.missing_non_claim]
#[test]
fn profile_rejects_zero_bound_and_missing_non_claim() {
    let mut profile = dashboard_profile();
    profile.run_page_limit = 0;
    let error = expect_error(profile.admit());
    assert_eq!(error.code, "zero_bound");

    let mut profile = dashboard_profile();
    profile.max_run_pages = 0;
    let error = expect_error(profile.admit());
    assert_eq!(error.code, "zero_bound");

    let mut profile = dashboard_profile();
    profile
        .non_claims
        .retain(|value| value != "dashboard rendering does not prove CI success");
    let error = expect_error(profile.admit());
    assert_eq!(error.code, "missing_required_non_claim");
}

// r[verify kiln.observe.dashboard.projection]
#[test]
fn summary_reports_admitted_run_and_attempt_counts() {
    let journal = admitted(3);
    let profile = dashboard_profile();
    let report = build_report(&profile, &journal, &DashboardSelection::default())
        .unwrap_or_else(|_| unreachable!());
    let ViewBody::StateSummary(summary) = &report.views.summary.body else {
        unreachable!()
    };
    assert_eq!(summary.run_count, 3);
    assert_eq!(summary.attempt_count, 3);
    assert_eq!(summary.hold_count, 0);
    assert_eq!(journal.state.revision, 3);
}

#[test]
fn run_list_pages_with_the_admitted_cursor() {
    let journal = admitted(5);
    let profile = dashboard_profile();
    let report = build_report(&profile, &journal, &DashboardSelection::default())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(report.views.runs.total_runs, 5);
    assert_eq!(report.views.runs.views.len(), 3);
    assert!(report.views.runs.capped);
    assert!(!report.views.runs.has_more);
    assert!(!report.views.runs.truncated);
}

#[test]
fn run_list_truncation_is_reported() {
    let journal = admitted(9);
    let mut profile = dashboard_profile();
    profile.max_run_pages = 2;
    let report = build_report(&profile, &journal, &DashboardSelection::default())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(report.views.runs.views.len(), 2);
    assert!(report.views.runs.has_more);
    assert!(report.views.runs.truncated);
}

#[test]
fn selected_run_and_attempt_are_detailed() {
    let journal = admitted(2);
    let profile = dashboard_profile();
    let selection = DashboardSelection {
        run: Some(identity("run-1")),
        attempt: Some(attempt_identity("attempt-1")),
    };
    let report = build_report(&profile, &journal, &selection).unwrap_or_else(|_| unreachable!());
    let Some(view) = report.views.run_detail.as_ref() else {
        unreachable!()
    };
    let ViewBody::RunDetail(detail) = &view.body else {
        unreachable!()
    };
    assert_eq!(detail.run.run_id.as_str(), "run-1");
    let Some(view) = report.views.attempt_detail.as_ref() else {
        unreachable!()
    };
    let ViewBody::AttemptDetail(detail) = &view.body else {
        unreachable!()
    };
    assert_eq!(detail.attempt_id.as_str(), "attempt-1");
    assert_eq!(detail.run_id.as_str(), "run-1");
}

// r[verify kiln.observe.dashboard.projection.scenario.unknown_target]
#[test]
fn unknown_selection_reports_not_found() {
    let journal = admitted(1);
    let profile = dashboard_profile();
    let selection = DashboardSelection {
        run: Some(identity("run-absent")),
        attempt: None,
    };
    let report = build_report(&profile, &journal, &selection).unwrap_or_else(|_| unreachable!());
    let Some(view) = report.views.run_detail.as_ref() else {
        unreachable!()
    };
    assert!(matches!(view.body, ViewBody::NotFound(_)));
    assert!(report.views.attempt_detail.is_none());
}

// r[verify kiln.observe.dashboard.projection.scenario.parity]
#[test]
fn dashboard_view_matches_the_accepted_projection() {
    let journal = admitted(2);
    let profile = dashboard_profile();
    let report = build_report(&profile, &journal, &DashboardSelection::default())
        .unwrap_or_else(|_| unreachable!());
    let direct = project_observation(
        &profile.observation,
        &journal.state,
        &ObservationRequest {
            state_revision: journal.state.revision,
            view: ViewRequest::RunList {
                after: None,
                limit: profile.run_page_limit,
            },
        },
        &ObservationWindow::empty(),
    )
    .unwrap_or_else(|_| unreachable!());
    let dashboard = report
        .views
        .runs
        .views
        .first()
        .unwrap_or_else(|| unreachable!());
    assert_eq!(dashboard.identity, direct.identity);
    assert_eq!(dashboard.body, direct.body);
}

// r[verify kiln.observe.dashboard.projection.scenario.redaction]
#[test]
fn redacted_fields_are_named_and_absent() {
    let journal = admitted(1);
    let mut profile = dashboard_profile();
    profile
        .observation
        .redaction
        .visible_fields
        .remove(&ViewField::Products);
    let report = build_report(&profile, &journal, &DashboardSelection::default())
        .unwrap_or_else(|_| unreachable!());
    assert!(report.redacted_fields.contains(&ViewField::Products));
    let Some(view) = report.views.attempt_detail.as_ref() else {
        unreachable!()
    };
    let ViewBody::AttemptDetail(detail) = &view.body else {
        unreachable!()
    };
    assert!(detail.products.is_empty());
    let text = render_report(&report, RenderStyle::Plain);
    assert!(text.contains("products"));
    assert!(text.contains("redacted"));
}

// r[verify kiln.observe.dashboard.report.scenario.bounded]
#[test]
fn event_tail_reports_the_bounded_window_tail() {
    let journal = admitted(6);
    let profile = dashboard_profile();
    let report = build_report(&profile, &journal, &DashboardSelection::default())
        .unwrap_or_else(|_| unreachable!());
    let ViewBody::EventTail(events) = &report.views.events.body else {
        unreachable!()
    };
    assert_eq!(events.page.returned, 4);
    assert!(events.page.has_earlier);
    assert!(events.page.capped);
    assert_eq!(events.page.end_revision, journal.state.revision);
}

// r[verify kiln.observe.dashboard.journal]
// r[verify kiln.observe.dashboard.journal.scenario.match]
#[test]
fn canonical_journal_is_admitted() {
    let journal = admitted(2);
    assert_eq!(journal.records.len(), 2);
    assert_eq!(journal.state.revision, 2);
}

// r[verify kiln.observe.dashboard.journal.scenario.substituted_event]
#[test]
fn tampered_event_bytes_are_rejected() {
    let mut value: serde_json::Value =
        serde_json::from_slice(&journal_document(1)).unwrap_or_else(|_| unreachable!());
    let records = value
        .get_mut("records")
        .and_then(|value| value.as_array_mut())
        .unwrap_or_else(|| unreachable!());
    let record = records.first_mut().unwrap_or_else(|| unreachable!());
    let event = record
        .get_mut("event")
        .and_then(|value| value.as_object_mut())
        .unwrap_or_else(|| unreachable!());
    let variant = event
        .values_mut()
        .next()
        .and_then(|value| value.as_object_mut())
        .unwrap_or_else(|| unreachable!());
    let command = variant
        .get("command_id")
        .and_then(|value| value.as_str())
        .unwrap_or_else(|| unreachable!())
        .to_string();
    variant.insert(
        "command_id".to_string(),
        serde_json::json!(format!("{command}-tampered")),
    );
    let bytes = serde_json::to_vec(&value).unwrap_or_else(|_| unreachable!());
    let document = JournalDocument::parse(&bytes).unwrap_or_else(|_| unreachable!());
    let error = expect_error(admit_document(&document));
    assert_eq!(error.code, "event_identity_mismatch");
}

// r[verify kiln.observe.dashboard.journal.scenario.unsupported_identity_kind]
#[test]
fn unknown_event_identity_kind_is_rejected() {
    let mut value: serde_json::Value =
        serde_json::from_slice(&journal_document(1)).unwrap_or_else(|_| unreachable!());
    let records = value
        .get_mut("records")
        .and_then(|value| value.as_array_mut())
        .unwrap_or_else(|| unreachable!());
    let record = records.first_mut().unwrap_or_else(|| unreachable!());
    let identity = record
        .get_mut("event_identity")
        .and_then(|value| value.as_object_mut())
        .unwrap_or_else(|| unreachable!());
    identity.insert(
        "native_kind".to_string(),
        serde_json::json!("unknown-event-blake3"),
    );
    let bytes = serde_json::to_vec(&value).unwrap_or_else(|_| unreachable!());
    let document = JournalDocument::parse(&bytes).unwrap_or_else(|_| unreachable!());
    let error = expect_error(admit_document(&document));
    assert_eq!(error.code, "unsupported_event_identity");
}

// r[verify kiln.observe.dashboard.journal.scenario.replay_failure]
#[test]
fn reordered_journal_is_rejected() {
    let mut value: serde_json::Value =
        serde_json::from_slice(&journal_document(2)).unwrap_or_else(|_| unreachable!());
    let records = value
        .get_mut("records")
        .and_then(|value| value.as_array_mut())
        .unwrap_or_else(|| unreachable!());
    records.swap(0, 1);
    let bytes = serde_json::to_vec(&value).unwrap_or_else(|_| unreachable!());
    let document = JournalDocument::parse(&bytes).unwrap_or_else(|_| unreachable!());
    let error = expect_error(admit_document(&document));
    assert_eq!(error.code, "journal_replay_rejected");
}

// r[verify kiln.observe.dashboard.journal.scenario.truncated_prefix]
#[test]
fn canonical_prefix_is_admitted_with_its_observed_revision() {
    let mut value: serde_json::Value =
        serde_json::from_slice(&journal_document(3)).unwrap_or_else(|_| unreachable!());
    let records = value
        .get_mut("records")
        .and_then(|value| value.as_array_mut())
        .unwrap_or_else(|| unreachable!());
    records.pop();
    let bytes = serde_json::to_vec(&value).unwrap_or_else(|_| unreachable!());
    let document = JournalDocument::parse(&bytes).unwrap_or_else(|_| unreachable!());
    let journal = admit_document(&document).unwrap_or_else(|_| unreachable!());
    assert_eq!(journal.records.len(), 2);
    assert_eq!(journal.state.revision, 2);
    let profile = dashboard_profile();
    let report = build_report(&profile, &journal, &DashboardSelection::default())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(report.journal.state_revision, 2);
    assert_eq!(report.journal.record_count, 2);
}

// r[verify kiln.observe.dashboard.journal.scenario.match]
#[test]
fn canonical_json_identity_convention_is_admitted() {
    let mut state = State::default();
    let run_id = identity("run-json");
    let attempt_id = attempt_identity("attempt-json");
    let envelope = CommandEnvelope {
        contract_version: CONTRACT_VERSION_V1,
        command_id: command_identity("command-json"),
        expected_revision: state.revision,
        command: Command::StartRun {
            run_id,
            attempt_id,
            provider_profile: provider_identity("build-v1"),
            effect_id: effect_identity("effect-json"),
        },
    };
    let decision: Decision =
        decide(&state, &envelope, Limits::default()).unwrap_or_else(|_| unreachable!());
    let mut records: Vec<JournalRecord> = Vec::new();
    for event in decision.events {
        let expected_revision = state.revision;
        let bytes = serde_json::to_vec(&event).unwrap_or_else(|_| unreachable!());
        let content = identity_from_bytes(&bytes).unwrap_or_else(|_| unreachable!());
        let event_identity = ExternalRef::new("kiln", "event-blake3", content.as_str())
            .unwrap_or_else(|_| unreachable!());
        state = evolve(&state, &event).unwrap_or_else(|_| unreachable!());
        records.push(JournalRecord {
            schema_version: CONTRACT_VERSION_V1,
            stream: stream(),
            expected_revision,
            event_identity,
            event,
        });
    }
    let document = serde_json::json!({
        "schema": "kiln.sqlite-journal.v1",
        "records": records,
    });
    let bytes = serde_json::to_vec(&document).unwrap_or_else(|_| unreachable!());
    let parsed = JournalDocument::parse(&bytes).unwrap_or_else(|_| unreachable!());
    let journal = admit_document(&parsed).unwrap_or_else(|_| unreachable!());
    assert_eq!(journal.state.revision, 1);
}

#[test]
fn journal_without_canonical_records_is_rejected() {
    let bytes = serde_json::to_vec(&serde_json::json!({"schema": "kiln.test.v1"}))
        .unwrap_or_else(|_| unreachable!());
    let error = expect_error(JournalDocument::parse(&bytes));
    assert_eq!(error.code, "malformed_journal_document");
}

// r[verify kiln.observe.dashboard.report]
// r[verify kiln.observe.dashboard.report.scenario.deterministic]
// r[verify kiln.observe.dashboard.nonclaims]
// r[verify kiln.observe.dashboard.nonclaims.scenario.scope]
#[test]
fn rendering_is_deterministic() {
    let journal = admitted(2);
    let profile = dashboard_profile();
    let report = build_report(&profile, &journal, &DashboardSelection::default())
        .unwrap_or_else(|_| unreachable!());
    let plain = render_report(&report, RenderStyle::Plain);
    let again = render_report(&report, RenderStyle::Plain);
    assert_eq!(plain, again);
    let ansi = render_report(&report, RenderStyle::Ansi);
    assert!(ansi.contains("\u{1b}["));
    assert!(plain.contains("kiln dashboard"));
    assert!(plain.contains("run-0"));
    assert!(plain.contains("non-claims"));
    assert!(plain.contains("dashboard rendering does not prove CI success"));
}

fn write_case(name: &str, journal: &[u8], profile: &DashboardProfile) -> (PathBuf, PathBuf) {
    let directory = std::env::temp_dir().join(format!("kiln-dashboard-{name}"));
    std::fs::create_dir_all(&directory).unwrap_or_else(|_| unreachable!());
    let journal_path = directory.join("journal.json");
    let profile_path = directory.join("profile.json");
    std::fs::write(&journal_path, journal).unwrap_or_else(|_| unreachable!());
    let document = ProfileDocument {
        profile: profile.clone(),
    };
    std::fs::write(
        &profile_path,
        serde_json::to_vec(&document).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());
    (journal_path, profile_path)
}

fn path_text(path: &Path) -> &str {
    path.to_str().unwrap_or_else(|| unreachable!())
}

// r[verify kiln.observe.dashboard.shell]
#[test]
fn shell_renders_the_named_journal() {
    let profile = dashboard_profile();
    let (journal_path, profile_path) = write_case("render", &journal_document(2), &profile);
    let output = Process::new(env!("CARGO_BIN_EXE_kiln-dashboard"))
        .args([
            "--journal",
            path_text(&journal_path),
            "--profile",
            path_text(&profile_path),
            "--plain",
        ])
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap_or_else(|_| unreachable!());
    assert!(text.contains("kiln dashboard"));
    assert!(text.contains("run-0"));
    assert!(text.contains("run-1"));
}

#[test]
fn shell_rejects_a_tampered_journal() {
    let mut value: serde_json::Value =
        serde_json::from_slice(&journal_document(1)).unwrap_or_else(|_| unreachable!());
    let records = value
        .get_mut("records")
        .and_then(|value| value.as_array_mut())
        .unwrap_or_else(|| unreachable!());
    let record = records.first_mut().unwrap_or_else(|| unreachable!());
    let event = record
        .get_mut("event")
        .and_then(|value| value.as_object_mut())
        .unwrap_or_else(|| unreachable!());
    let variant = event
        .values_mut()
        .next()
        .and_then(|value| value.as_object_mut())
        .unwrap_or_else(|| unreachable!());
    let command = variant
        .get("command_id")
        .and_then(|value| value.as_str())
        .unwrap_or_else(|| unreachable!())
        .to_string();
    variant.insert(
        "command_id".to_string(),
        serde_json::json!(format!("{command}-tampered")),
    );
    let bytes = serde_json::to_vec(&value).unwrap_or_else(|_| unreachable!());
    let profile = dashboard_profile();
    let (journal_path, profile_path) = write_case("tampered", &bytes, &profile);
    let output = Process::new(env!("CARGO_BIN_EXE_kiln-dashboard"))
        .args([
            "--journal",
            path_text(&journal_path),
            "--profile",
            path_text(&profile_path),
            "--plain",
        ])
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(!output.status.success());
    let text = String::from_utf8(output.stderr).unwrap_or_else(|_| unreachable!());
    assert!(text.contains("event_identity_mismatch"));
}

// r[verify kiln.observe.dashboard.shell.scenario.watch]
#[test]
fn shell_watch_mode_redraws_the_report() {
    let mut profile = dashboard_profile();
    profile.watch_interval_ms = 50;
    let (journal_path, profile_path) = write_case("watch", &journal_document(2), &profile);
    let mut child = Process::new(env!("CARGO_BIN_EXE_kiln-dashboard"))
        .args([
            "--journal",
            path_text(&journal_path),
            "--profile",
            path_text(&profile_path),
            "--watch",
            "--plain",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|_| unreachable!());
    std::thread::sleep(Duration::from_millis(400));
    child.kill().unwrap_or_else(|_| unreachable!());
    let output = child.wait_with_output().unwrap_or_else(|_| unreachable!());
    let text = String::from_utf8(output.stdout).unwrap_or_else(|_| unreachable!());
    assert!(text.matches("kiln dashboard").count() >= 2);
}

// r[verify kiln.observe.dashboard.store]
// r[verify kiln.observe.dashboard.store.scenario.match]
#[test]
fn sqlite_event_store_is_admitted_and_projected() {
    use kiln_adapter_sqlite::{
        AppendCommitMode, SqliteEventStore, build_journal_record, canonical_profile,
    };
    use kiln_provider_api::EventAppendRequest;

    let directory = std::env::temp_dir().join("kiln-dashboard-store");
    std::fs::create_dir_all(&directory).unwrap_or_else(|_| unreachable!());
    let path = directory.join("events.db");
    let _ = std::fs::remove_file(&path);
    let profile = canonical_profile();
    let stream =
        ExternalRef::new("kiln", "event-stream", "main").unwrap_or_else(|_| unreachable!());

    let mut state = State::default();
    let mut records = Vec::new();
    for index in 0..2 {
        let run_id = identity(&format!("run-{index}"));
        let attempt_id = attempt_identity(&format!("attempt-{index}"));
        let envelope = CommandEnvelope {
            contract_version: CONTRACT_VERSION_V1,
            command_id: command_identity(&format!("command-{index}")),
            expected_revision: state.revision,
            command: Command::StartRun {
                run_id,
                attempt_id,
                provider_profile: provider_identity("build-v1"),
                effect_id: effect_identity(&format!("effect-{index}")),
            },
        };
        let decision: Decision =
            decide(&state, &envelope, Limits::default()).unwrap_or_else(|_| unreachable!());
        for event in decision.events {
            let expected_revision = state.revision;
            state = evolve(&state, &event).unwrap_or_else(|_| unreachable!());
            let record = build_journal_record(stream.clone(), expected_revision, event)
                .unwrap_or_else(|_| unreachable!());
            records.push(record);
        }
    }

    let mut store =
        SqliteEventStore::open(&path, profile.clone()).unwrap_or_else(|_| unreachable!());
    store
        .append_with_mode(
            &EventAppendRequest {
                stream: stream.clone(),
                expected_revision: 0,
                records: records.clone(),
                outbox: Vec::new(),
            },
            AppendCommitMode::Commit,
        )
        .unwrap_or_else(|_| unreachable!());
    drop(store);

    let journal = kiln_dashboard::store::admit_store(&path, &profile, &stream)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(journal.records.len(), 2);
    assert_eq!(journal.state.revision, 2);
    let report = build_report(
        &dashboard_profile(),
        &journal,
        &DashboardSelection::default(),
    )
    .unwrap_or_else(|_| unreachable!());
    let ViewBody::StateSummary(summary) = &report.views.summary.body else {
        unreachable!()
    };
    assert_eq!(summary.run_count, 2);
    assert_eq!(summary.attempt_count, 2);
}

// r[verify kiln.observe.dashboard.store.scenario.read_only]
#[test]
fn sqlite_event_store_is_read_only_for_the_dashboard() {
    use kiln_adapter_sqlite::{SqliteEventStore, canonical_profile};

    let directory = std::env::temp_dir().join("kiln-dashboard-store-read-only");
    std::fs::create_dir_all(&directory).unwrap_or_else(|_| unreachable!());
    let path = directory.join("events.db");
    let _ = std::fs::remove_file(&path);
    let profile = canonical_profile();
    let stream =
        ExternalRef::new("kiln", "event-stream", "main").unwrap_or_else(|_| unreachable!());
    {
        let store =
            SqliteEventStore::open(&path, profile.clone()).unwrap_or_else(|_| unreachable!());
        drop(store);
    }
    let before = std::fs::metadata(&path).unwrap_or_else(|_| unreachable!());
    let journal = kiln_dashboard::store::admit_store(&path, &profile, &stream)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(journal.records.len(), 0);
    let after = std::fs::metadata(&path).unwrap_or_else(|_| unreachable!());
    assert_eq!(before.len(), after.len());
    assert_eq!(
        before.modified().unwrap_or_else(|_| unreachable!()),
        after.modified().unwrap_or_else(|_| unreachable!())
    );

    // A missing store must be refused, never created by a read.
    let absent = directory.join("absent.db");
    let _ = std::fs::remove_file(&absent);
    let error = match kiln_dashboard::store::admit_store(&absent, &profile, &stream) {
        Ok(_) => unreachable!(),
        Err(error) => error,
    };
    assert!(
        error.contains("event store open"),
        "unexpected error: {error}"
    );
    assert!(!absent.exists(), "the dashboard must not create a store");
}

// r[verify kiln.observe.dashboard.html.scenario.determinism]
#[test]
fn html_generation_is_deterministic() {
    let journal = admitted(2);
    let profile = dashboard_profile();
    let first = build_html_report(&profile, &journal, &DashboardSelection::default())
        .unwrap_or_else(|_| unreachable!());
    let second = build_html_report(&profile, &journal, &DashboardSelection::default())
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        kiln_dashboard::render::html::render_html(&first),
        kiln_dashboard::render::html::render_html(&second)
    );
}

// r[verify kiln.observe.dashboard.html.scenario.escaping]
#[test]
fn admitted_hostile_identifiers_are_escaped_in_text_and_attributes() {
    let hostile = "run-\"><img/src=x/onerror=alert(1)>&'";
    let bytes = journal_commands([Command::StartRun {
        run_id: identity(hostile),
        attempt_id: attempt_identity("attempt-</script><script>alert(1)</script>"),
        provider_profile: provider_identity("build-v1"),
        effect_id: effect_identity("effect-hostile"),
    }]);
    let document = JournalDocument::parse(&bytes).unwrap_or_else(|_| unreachable!());
    let journal = admit_document(&document).unwrap_or_else(|_| unreachable!());
    let report = build_html_report(
        &dashboard_profile(),
        &journal,
        &DashboardSelection::default(),
    )
    .unwrap_or_else(|_| unreachable!());
    let html = kiln_dashboard::render::html::render_html(&report);
    assert!(html.contains("run-&quot;&gt;&lt;img/src=x/onerror=alert(1)&gt;&amp;&#39;"));
    assert!(!html.contains(hostile));
    assert!(!html.contains("<script>alert(1)</script>"));
}

// r[verify kiln.observe.dashboard.html.interactive]
// r[verify kiln.observe.dashboard.html.interactive.scenario.bounded]
#[test]
fn html_details_cover_only_paged_runs_and_explicit_selection_with_exact_views() {
    let journal = admitted(9);
    let mut profile = dashboard_profile();
    profile.max_run_pages = 1;
    let html = build_html_report(
        &profile,
        &journal,
        &DashboardSelection {
            run: Some(identity("run-8")),
            attempt: None,
        },
    )
    .unwrap_or_else(|_| unreachable!());
    assert!(html.report.views.runs.truncated);
    let runs: Vec<_> = html
        .report
        .views
        .run_detail
        .iter()
        .chain(&html.runs)
        .map(|view| {
            let ViewBody::RunDetail(detail) = &view.body else {
                unreachable!()
            };
            let direct = project_observation(
                &profile.observation,
                &journal.state,
                &ObservationRequest {
                    state_revision: journal.state.revision,
                    view: ViewRequest::RunDetail {
                        run_id: detail.run.run_id.clone(),
                    },
                },
                &ObservationWindow::empty(),
            )
            .unwrap_or_else(|_| unreachable!());
            assert_eq!(view, &direct);
            detail.run.run_id.as_str()
        })
        .collect();
    assert_eq!(runs, ["run-8", "run-0", "run-1"]);
    let attempts: Vec<_> = html
        .report
        .views
        .attempt_detail
        .iter()
        .chain(&html.attempts)
        .map(|view| {
            let ViewBody::AttemptDetail(detail) = &view.body else {
                unreachable!()
            };
            let direct = project_observation(
                &profile.observation,
                &journal.state,
                &ObservationRequest {
                    state_revision: journal.state.revision,
                    view: ViewRequest::AttemptDetail {
                        attempt_id: detail.attempt_id.clone(),
                    },
                },
                &ObservationWindow::empty(),
            )
            .unwrap_or_else(|_| unreachable!());
            assert_eq!(view, &direct);
            detail.attempt_id.as_str()
        })
        .collect();
    assert_eq!(attempts, ["attempt-8", "attempt-0", "attempt-1"]);
}

// r[verify kiln.observe.dashboard.html.interactive.scenario.bounded]
#[test]
fn html_attempt_details_respect_history_bound_active_attempt_and_redaction() {
    let mut commands = vec![Command::StartRun {
        run_id: identity("run-0"),
        attempt_id: attempt_identity("attempt-0"),
        provider_profile: provider_identity("private-provider"),
        effect_id: effect_identity("effect-0"),
    }];
    for index in 0..3 {
        commands.push(Command::CompleteAttempt {
            attempt_id: attempt_identity(&format!("attempt-{index}")),
            outcome: Outcome::DomainFailure,
            external_reference: Some(
                ExternalRef::new("provider", "attempt", "private-reference")
                    .unwrap_or_else(|_| unreachable!()),
            ),
            products: Vec::new(),
            metrics: Vec::new(),
        });
        commands.push(Command::Retry {
            run_id: identity("run-0"),
            prior_attempt_id: attempt_identity(&format!("attempt-{index}")),
            next_attempt_id: attempt_identity(&format!("attempt-{}", index + 1)),
            provider_profile: provider_identity("private-provider"),
            effect_id: effect_identity(&format!("retry-{index}")),
        });
    }
    let document =
        JournalDocument::parse(&journal_commands(commands)).unwrap_or_else(|_| unreachable!());
    let journal = admit_document(&document).unwrap_or_else(|_| unreachable!());
    let mut profile = dashboard_profile();
    profile.observation.redaction.visible_fields.clear();
    let html = build_html_report(
        &profile,
        &journal,
        &DashboardSelection {
            run: None,
            attempt: Some(attempt_identity("attempt-2")),
        },
    )
    .unwrap_or_else(|_| unreachable!());
    let ViewBody::RunDetail(run) = &html
        .report
        .views
        .run_detail
        .as_ref()
        .unwrap_or_else(|| unreachable!())
        .body
    else {
        unreachable!()
    };
    assert_eq!(run.attempts_truncated, 3);
    assert_eq!(
        run.attempts
            .first()
            .map(|attempt| attempt.attempt_id.as_str()),
        Some("attempt-0")
    );
    assert_eq!(run.run.active_attempt.as_str(), "attempt-3");
    let attempts: Vec<_> = html
        .report
        .views
        .attempt_detail
        .iter()
        .chain(&html.attempts)
        .map(|view| {
            let ViewBody::AttemptDetail(detail) = &view.body else {
                unreachable!()
            };
            assert_eq!(detail.provider_profile, None);
            assert_eq!(detail.external_reference, None);
            assert!(view.redacted_fields.contains(&ViewField::ProviderProfile));
            detail.attempt_id.as_str()
        })
        .collect();
    assert_eq!(attempts, ["attempt-2", "attempt-0", "attempt-3"]);
    assert!(
        html.report
            .redacted_fields
            .contains(&ViewField::ProviderProfile)
    );
    let rendered = kiln_dashboard::render::html::render_html(&html);
    assert!(!rendered.contains("private-provider"));
    assert!(!rendered.contains("private-reference"));
}

#[test]
fn html_empty_history_and_missing_selections_do_not_invent_details() {
    let journal = admitted(0);
    let profile = dashboard_profile();
    let empty = build_html_report(&profile, &journal, &DashboardSelection::default())
        .unwrap_or_else(|_| unreachable!());
    assert!(empty.runs.is_empty());
    assert!(empty.attempts.is_empty());
    assert!(empty.report.views.run_detail.is_none());
    assert!(empty.report.views.attempt_detail.is_none());
    let missing = build_html_report(
        &profile,
        &journal,
        &DashboardSelection {
            run: Some(identity("missing-run")),
            attempt: Some(attempt_identity("missing-attempt")),
        },
    )
    .unwrap_or_else(|_| unreachable!());
    assert!(missing.runs.is_empty());
    assert!(missing.attempts.is_empty());
    assert!(matches!(
        missing
            .report
            .views
            .run_detail
            .as_ref()
            .unwrap_or_else(|| unreachable!())
            .body,
        ViewBody::NotFound(_)
    ));
    assert!(matches!(
        missing
            .report
            .views
            .attempt_detail
            .as_ref()
            .unwrap_or_else(|| unreachable!())
            .body,
        ViewBody::NotFound(_)
    ));
}

fn catalog_value() -> serde_json::Value {
    serde_json::json!({
        "schema_version": 1,
        "bounds": {
            "max_catalog_bytes": 1048576, "max_projects": 32, "max_histories": 128,
            "max_input_bytes": 268435456, "max_rendered_bytes": 67108864
        },
        "projects": [{"id": "kiln", "label": "Declared repository", "histories": [
            {"id": "first", "label": "First history", "input": {"kind": "journal", "path": "journal.json"}},
            {"id": "second", "label": "Second history", "input": {"kind": "journal", "path": "second.json"}}
        ]}]
    })
}

// r[verify kiln.dashboard.repositories.catalog]
#[test]
fn catalog_rejects_ambiguous_identities_versions_unknown_fields_and_bounds() {
    use kiln_dashboard::catalog::Catalog;
    let valid = catalog_value();
    let parse = |value: &serde_json::Value| {
        Catalog::parse(&serde_json::to_vec(value).unwrap_or_else(|_| unreachable!()))
    };
    assert!(parse(&valid).is_ok());
    let mut invalid = valid.clone();
    *invalid
        .get_mut("schema_version")
        .unwrap_or_else(|| unreachable!()) = serde_json::json!(2);
    assert!(parse(&invalid).is_err());
    let mut invalid = valid.clone();
    invalid
        .pointer_mut("/projects/0/histories/0/input")
        .and_then(serde_json::Value::as_object_mut)
        .unwrap_or_else(|| unreachable!())
        .insert("stream".into(), serde_json::json!("kiln:ci:main"));
    assert!(parse(&invalid).is_err());
    let mut invalid = valid.clone();
    *invalid
        .pointer_mut("/projects/0/histories/1/id")
        .unwrap_or_else(|| unreachable!()) = serde_json::json!("first");
    assert!(parse(&invalid).is_err());
    let mut invalid = valid.clone();
    invalid
        .get_mut("projects")
        .and_then(serde_json::Value::as_array_mut)
        .unwrap_or_else(|| unreachable!())
        .push(
            valid
                .pointer("/projects/0")
                .cloned()
                .unwrap_or_else(|| unreachable!()),
        );
    assert!(parse(&invalid).is_err());
    for (name, value) in [
        ("max_histories", 0),
        ("max_histories", 1),
        ("max_projects", 33),
        ("max_catalog_bytes", 1),
    ] {
        let mut invalid = valid.clone();
        *invalid
            .get_mut("bounds")
            .and_then(|bounds| bounds.get_mut(name))
            .unwrap_or_else(|| unreachable!()) = serde_json::json!(value);
        assert!(parse(&invalid).is_err(), "{name}={value}");
    }
    for field in ["label", "source", "revision"] {
        let pointer = format!("/projects/0/histories/0/{field}");
        let mut boundary = valid.clone();
        boundary
            .pointer_mut("/projects/0/histories/0")
            .and_then(serde_json::Value::as_object_mut)
            .unwrap_or_else(|| unreachable!())
            .insert(field.to_string(), serde_json::json!("é".repeat(512)));
        assert!(parse(&boundary).is_ok(), "1024-byte {field}");
        for value in ["é".repeat(513), String::new(), "line\nbreak".into()] {
            *boundary
                .pointer_mut(&pointer)
                .unwrap_or_else(|| unreachable!()) = serde_json::json!(value);
            assert!(parse(&boundary).is_err(), "invalid {field}");
        }
    }
    for id in [
        String::new(),
        "a".repeat(129),
        "répository".into(),
        "../input/path".into(),
    ] {
        let mut invalid = valid.clone();
        *invalid
            .pointer_mut("/projects/0/histories/0/id")
            .unwrap_or_else(|| unreachable!()) = serde_json::json!(id);
        assert!(parse(&invalid).is_err());
    }
    let mut invalid = valid;
    *invalid
        .pointer_mut("/projects/0/histories/0/input")
        .unwrap_or_else(|| unreachable!()) =
        serde_json::json!({"kind": "store", "path": "events.db", "stream": "not-a-stream"});
    assert!(parse(&invalid).is_err());
}

// r[verify kiln.dashboard.repositories.catalog]
// r[verify kiln.dashboard.repositories.catalog.scenario.navigation]
#[test]
fn catalog_resolves_relative_histories_and_preserves_independent_view_identities() {
    let profile = dashboard_profile();
    let (journal_path, _) = write_case("catalog-independent", &journal_document(1), &profile);
    let directory = journal_path.parent().unwrap_or_else(|| unreachable!());
    std::fs::write(directory.join("second.json"), journal_document(2))
        .unwrap_or_else(|_| unreachable!());
    let catalog_path = directory.join("catalog.json");
    std::fs::write(
        &catalog_path,
        serde_json::to_vec(&catalog_value()).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());
    let rendered = kiln_dashboard::shell::render_catalog(&catalog_path, &profile, None)
        .unwrap_or_else(|_| unreachable!());
    let documents: Vec<_> = rendered
        .split(" srcdoc=\"")
        .skip(1)
        .map(|value| {
            value
                .split('"')
                .next()
                .unwrap_or_else(|| unreachable!())
                .replace("&quot;", "\"")
                .replace("&#39;", "'")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&amp;", "&")
        })
        .collect();
    let expected: Vec<_> = [1, 2]
        .into_iter()
        .map(|count| {
            let original =
                build_html_report(&profile, &admitted(count), &DashboardSelection::default())
                    .unwrap_or_else(|_| unreachable!());
            kiln_dashboard::render::html::render_html(&original)
        })
        .collect();
    assert_eq!(documents, expected);
    // A corrupt second history fails the entire artifact, not just that panel.
    let mut corrupt: serde_json::Value =
        serde_json::from_slice(&journal_document(2)).unwrap_or_else(|_| unreachable!());
    *corrupt
        .pointer_mut("/records/0/expected_revision")
        .unwrap_or_else(|| unreachable!()) = serde_json::json!(7);
    std::fs::write(
        directory.join("second.json"),
        serde_json::to_vec(&corrupt).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());
    assert!(kiln_dashboard::shell::render_catalog(&catalog_path, &profile, None).is_err());
}

// r[verify kiln.dashboard.repositories.catalog]
#[test]
fn catalog_rejects_aggregate_and_output_overflow_without_replacing_last_good_report() {
    let profile = dashboard_profile();
    let (journal_path, profile_path) =
        write_case("catalog-overflow", &journal_document(1), &profile);
    let directory = journal_path.parent().unwrap_or_else(|| unreachable!());
    std::fs::write(directory.join("second.json"), journal_document(2))
        .unwrap_or_else(|_| unreachable!());
    let catalog_path = directory.join("catalog.json");
    let output_path = directory.join("report.html");
    std::fs::write(&output_path, "previous admitted report").unwrap_or_else(|_| unreachable!());
    std::fs::write(
        &catalog_path,
        serde_json::to_vec(&catalog_value()).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());
    let admitted_bytes = kiln_dashboard::shell::render_catalog(&catalog_path, &profile, None)
        .unwrap_or_else(|_| unreachable!())
        .len();
    for (bound, limit) in [
        ("max_input_bytes", 1),
        ("max_rendered_bytes", admitted_bytes - 1),
    ] {
        let mut catalog = catalog_value();
        *catalog
            .get_mut("bounds")
            .and_then(|bounds| bounds.get_mut(bound))
            .unwrap_or_else(|| unreachable!()) = serde_json::json!(limit);
        std::fs::write(
            &catalog_path,
            serde_json::to_vec(&catalog).unwrap_or_else(|_| unreachable!()),
        )
        .unwrap_or_else(|_| unreachable!());
        let args = [
            "--catalog",
            path_text(&catalog_path),
            "--profile",
            path_text(&profile_path),
            "--format",
            "html",
            "--output",
            path_text(&output_path),
        ];
        let kiln_dashboard::shell::Parsed::Options(options) =
            kiln_dashboard::shell::parse_options(args.into_iter().map(String::from))
                .unwrap_or_else(|_| unreachable!())
        else {
            unreachable!()
        };
        let document = ProfileDocument {
            profile: profile.clone(),
        };
        assert!(
            kiln_dashboard::shell::render_frame(&options, &document, RenderStyle::Plain).is_err()
        );
        let retained = std::fs::read_to_string(&output_path).unwrap_or_else(|_| unreachable!());
        assert_eq!(retained, "previous admitted report");
    }
    let mut exact = catalog_value();
    *exact
        .pointer_mut("/bounds/max_rendered_bytes")
        .unwrap_or_else(|| unreachable!()) = serde_json::json!(admitted_bytes);
    std::fs::write(
        &catalog_path,
        serde_json::to_vec(&exact).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        kiln_dashboard::shell::render_catalog(&catalog_path, &profile, None)
            .unwrap_or_else(|_| unreachable!())
            .len(),
        admitted_bytes
    );
}

#[test]
fn catalog_cli_rejects_input_and_selection_modes_that_could_lose_history_boundaries() {
    use kiln_dashboard::shell::parse_options;
    let base = [
        "--catalog",
        "catalog.json",
        "--profile",
        "profile.json",
        "--format",
        "html",
        "--output",
        "report.html",
    ];
    assert!(parse_options(base.into_iter().map(String::from)).is_ok());
    for extra in [
        vec!["--journal", "journal.json"],
        vec!["--store", "events.db"],
        vec!["--stream", "kiln:ci:main"],
        vec!["--run", "run-0"],
        vec!["--attempt", "attempt-0"],
        vec!["--plain"],
        vec!["--format", "text"],
        vec!["--refresh-seconds", "0"],
        vec!["--refresh-seconds", "1", "--watch"],
    ] {
        assert!(parse_options(base.into_iter().chain(extra).map(String::from)).is_err());
    }
}

#[test]
fn catalog_output_cannot_replace_inputs_through_symlink_aliases() {
    let profile = dashboard_profile();
    let (journal_path, profile_path) =
        write_case("catalog-output-protection", &journal_document(1), &profile);
    let directory = journal_path.parent().unwrap_or_else(|| unreachable!());
    std::fs::write(directory.join("second.json"), journal_document(2))
        .unwrap_or_else(|_| unreachable!());
    let catalog_path = directory.join("catalog.json");
    std::fs::write(
        &catalog_path,
        serde_json::to_vec(&catalog_value()).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());
    for source in [&catalog_path, &profile_path, &journal_path] {
        assert!(
            kiln_dashboard::shell::protect_output(&catalog_path, &profile_path, source).is_err()
        );
    }
    #[cfg(unix)]
    {
        let alias = directory.join("input-alias.html");
        let _ = std::fs::remove_file(&alias);
        std::os::unix::fs::symlink(&journal_path, &alias).unwrap_or_else(|_| unreachable!());
        assert!(
            kiln_dashboard::shell::protect_output(&catalog_path, &profile_path, &alias).is_err()
        );
    }
    assert!(
        kiln_dashboard::shell::protect_output(
            &catalog_path,
            &profile_path,
            &directory.join("safe-report.html")
        )
        .is_ok()
    );
}
