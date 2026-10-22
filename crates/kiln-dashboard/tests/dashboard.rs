use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Stdio};
use std::time::Duration;

use kiln_codec::{encode_and_identify, identity_from_bytes};
use kiln_core::domain::{Command, CommandEnvelope, Decision, State, decide, evolve};
use kiln_core::operations::JournalRecord;
use kiln_core::types::{
    AttemptId, CONTRACT_VERSION_V1, CommandId, EffectId, ExternalRef, Limits, ProviderProfileId,
    RunId,
};
use kiln_dashboard::journal::{JournalDocument, admit_document};
use kiln_dashboard::profile::{DashboardProfile, ProfileDocument};
use kiln_dashboard::render::{RenderStyle, render_report};
use kiln_dashboard::report::{DashboardSelection, build_report};
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
    let mut state = State::default();
    let mut records: Vec<JournalRecord> = Vec::new();
    for index in 0..run_count {
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

// r[verify kiln.observe.dashboard.html]
#[test]
fn html_report_states_the_same_facts_as_the_text_report() {
    let journal = admitted(2);
    let profile = dashboard_profile();
    let report = build_report(&profile, &journal, &DashboardSelection::default())
        .unwrap_or_else(|_| unreachable!());
    let html = kiln_dashboard::render::html::render_html(&report);
    let text = render_report(&report, RenderStyle::Plain);
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("state revision"));
    assert!(html.contains("<dt>state revision</dt><dd>2</dd>"));
    assert!(html.contains("kiln.observe.view.v1 identity"));
    assert!(html.contains("non-claims"));
    for non_claim in &profile.non_claims {
        assert!(html.contains(non_claim), "missing non-claim: {non_claim}");
        assert!(text.contains(non_claim), "missing non-claim: {non_claim}");
    }
    for view in [&report.views.summary, &report.views.events] {
        assert!(html.contains(view.identity.as_str()));
    }
    for view in &report.views.runs.views {
        assert!(html.contains(view.identity.as_str()));
    }
}

// r[verify kiln.observe.dashboard.html.scenario.self_contained]
#[test]
fn html_report_is_self_contained_and_deterministic() {
    let journal = admitted(2);
    let profile = dashboard_profile();
    let report = build_report(&profile, &journal, &DashboardSelection::default())
        .unwrap_or_else(|_| unreachable!());
    let html = kiln_dashboard::render::html::render_html(&report);
    assert_eq!(html, kiln_dashboard::render::html::render_html(&report));
    assert!(!html.contains("<script"));
    assert!(!html.contains("http://"));
    assert!(!html.contains("https://"));
    assert!(html.contains("<style>"));
}

// r[verify kiln.observe.dashboard.html.scenario.escaping]
#[test]
fn html_report_escapes_dynamic_content() {
    let document = JournalDocument::parse(&journal_document(1)).unwrap_or_else(|_| unreachable!());
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
    variant.insert(
        "command_id".to_string(),
        serde_json::json!("<script>alert(1)</script>"),
    );
    let bytes = serde_json::to_vec(&value).unwrap_or_else(|_| unreachable!());
    let tampered = JournalDocument::parse(&bytes).unwrap_or_else(|_| unreachable!());
    let error = match admit_document(&tampered) {
        Ok(_) => unreachable!(),
        Err(error) => error,
    };
    assert_eq!(error.code, "event_identity_mismatch");
    let _ = document;

    let journal = admitted(1);
    let profile = dashboard_profile();
    let report = build_report(&profile, &journal, &DashboardSelection::default())
        .unwrap_or_else(|_| unreachable!());
    let html = kiln_dashboard::render::html::render_html(&report);
    assert!(!html.contains("<script>alert(1)</script>"));
}
