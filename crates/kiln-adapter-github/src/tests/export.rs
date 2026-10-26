use kiln_core::domain::{Command, CommandEnvelope, Outcome, State, decide, evolve};
use kiln_core::operations::{JournalRecord, replay};
use kiln_core::types::{CONTRACT_VERSION_V1, CommandId, ContentId, Limits};
use rusqlite::Connection;
use tempfile::TempDir;

use crate::export::{OBSERVATION_SCHEMA, encode_observation, export_observation};
use crate::profile::{AdmittedGithubConfig, admit_config, load_config};
use crate::server::open_store;
use crate::service::{RunPhase, RunRecord, plan_event};

fn fixture(outcome: Outcome) -> (TempDir, AdmittedGithubConfig, RunRecord) {
    let root = TempDir::new().unwrap_or_else(|_| unreachable!());
    let admitted = admit_config(super::config(&root)).unwrap_or_else(|_| unreachable!());
    let event = super::event(&admitted, 'c', 1);
    let mut record = plan_event(&admitted, &event).unwrap_or_else(|_| unreachable!());
    let mut store = open_store(&admitted).unwrap_or_else(|_| unreachable!());
    store
        .reserve(&event, record.clone())
        .unwrap_or_else(|_| unreachable!());
    if outcome != Outcome::Pending {
        let command = if outcome == Outcome::Unknown {
            Command::RecordUnknown {
                attempt_id: record.plan.attempt_id.clone(),
                external_reference: None,
            }
        } else {
            Command::CompleteAttempt {
                attempt_id: record.plan.attempt_id.clone(),
                outcome,
                external_reference: None,
                products: Vec::new(),
                metrics: Vec::new(),
            }
        };
        let decision = decide(
            &record.ci_state,
            &CommandEnvelope {
                contract_version: CONTRACT_VERSION_V1,
                command_id: CommandId::new("export-test-complete")
                    .unwrap_or_else(|_| unreachable!()),
                expected_revision: record.ci_state.revision,
                command,
            },
            Limits::default(),
        )
        .unwrap_or_else(|_| unreachable!());
        for event in decision.events {
            record.ci_state = evolve(&record.ci_state, &event).unwrap_or_else(|_| unreachable!());
            record.events.push(event);
        }
        record.phase = if outcome == Outcome::Unknown {
            RunPhase::Unresolved
        } else {
            RunPhase::Terminal
        };
        store.save(&mut record).unwrap_or_else(|_| unreachable!());
    }
    drop(store);
    (root, admitted, record)
}

fn retained_output(root: &TempDir) -> std::path::PathBuf {
    let path = root.path().join("observation.json");
    std::fs::write(&path, b"previous-good-report").unwrap_or_else(|_| unreachable!());
    path
}

fn assert_rejected_unchanged(admitted: &AdmittedGithubConfig, run: &ContentId, root: &TempDir) {
    let output = retained_output(root);
    let before = std::fs::read(&admitted.config.replay_database_path).ok();
    assert!(
        export_observation(
            admitted,
            &root.path().join("profile.json"),
            run,
            &output,
            None
        )
        .is_err()
    );
    assert_eq!(
        std::fs::read(&output).unwrap_or_else(|_| unreachable!()),
        b"previous-good-report"
    );
    assert_eq!(
        std::fs::read(&admitted.config.replay_database_path).ok(),
        before
    );
}

// r[verify kiln.dashboard.repositories.github_export]
// r[verify kiln.dashboard.repositories.github_export.scenario.admission]
#[test]
fn exported_success_failure_unresolved_and_pending_replay_exactly() {
    for outcome in [
        Outcome::Success,
        Outcome::DomainFailure,
        Outcome::Unknown,
        Outcome::Pending,
    ] {
        let (root, admitted, record) = fixture(outcome);
        let before =
            std::fs::read(&admitted.config.replay_database_path).unwrap_or_else(|_| unreachable!());
        let bytes = encode_observation(&admitted, &record.run_identity, None)
            .unwrap_or_else(|_| unreachable!());
        let envelope: serde_json::Value =
            serde_json::from_slice(&bytes).unwrap_or_else(|_| unreachable!());
        let records: Vec<JournalRecord> = serde_json::from_value(
            envelope
                .get("records")
                .cloned()
                .unwrap_or_else(|| unreachable!()),
        )
        .unwrap_or_else(|_| unreachable!());
        let state = replay(&State::default(), &records).unwrap_or_else(|_| unreachable!());
        assert_eq!(state, record.ci_state);
        assert_eq!(
            state.runs.get(&record.run_id).map(|run| run.outcome),
            Some(outcome)
        );
        assert_eq!(
            state
                .attempts
                .get(&record.plan.attempt_id)
                .map(|attempt| attempt.outcome),
            Some(outcome)
        );
        assert_eq!(
            envelope.get("schema").and_then(serde_json::Value::as_str),
            Some(OBSERVATION_SCHEMA)
        );
        assert_eq!(
            envelope.get("run_identity"),
            Some(&serde_json::to_value(&record.run_identity).unwrap_or_else(|_| unreachable!()))
        );
        assert_eq!(
            envelope.get("profile_identity"),
            Some(&serde_json::to_value(&admitted.identity).unwrap_or_else(|_| unreachable!()))
        );
        assert_eq!(
            envelope.get("declared_source"),
            Some(&serde_json::to_value(&record.plan.source).unwrap_or_else(|_| unreachable!()))
        );
        assert_eq!(records.len(), record.events.len());
        for (index, (framed, original)) in records.iter().zip(&record.events).enumerate() {
            assert_eq!(&framed.event, original);
            assert_eq!(framed.expected_revision, index as u64);
            assert_eq!(
                framed.stream.native_id.as_str(),
                record.run_identity.as_str()
            );
            assert_eq!(
                framed.event_identity.native_kind.as_str(),
                "ci-event-blake3"
            );
            let (_, expected) = kiln_codec::encode_and_identify(
                &framed.event,
                crate::export::MAXIMUM_EVENT_IDENTITY_BYTES,
            )
            .unwrap_or_else(|_| unreachable!());
            assert_eq!(framed.event_identity.native_id.as_str(), expected.as_str());
        }
        let output = retained_output(&root);
        export_observation(
            &admitted,
            &root.path().join("profile.json"),
            &record.run_identity,
            &output,
            None,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            std::fs::read(&output).unwrap_or_else(|_| unreachable!()),
            bytes
        );
        // A new independent connection observes the same exact persisted run.
        assert_eq!(
            encode_observation(&admitted, &record.run_identity, None)
                .unwrap_or_else(|_| unreachable!()),
            bytes
        );
        assert_eq!(
            std::fs::read(&admitted.config.replay_database_path).unwrap_or_else(|_| unreachable!()),
            before
        );
        let text = String::from_utf8(bytes).unwrap_or_else(|_| unreachable!());
        assert!(!text.contains("secret_key"));
        assert!(!text.contains("WEBHOOK_SECRET"));
        assert!(!text.contains("statuses"));
    }
}

// r[verify kiln.dashboard.repositories.github_export.scenario.admission]
#[test]
fn missing_and_stale_profiles_preserve_prior_output_and_database() {
    let (root, admitted, record) = fixture(Outcome::Success);
    assert_rejected_unchanged(&admitted, &super::hash('f'), &root);
    let mut stale = admitted.config.clone();
    stale.worker_poll_interval_ms += 1;
    let stale = admit_config(stale).unwrap_or_else(|_| unreachable!());
    assert_rejected_unchanged(&stale, &record.run_identity, &root);

    let empty = TempDir::new().unwrap_or_else(|_| unreachable!());
    let missing = admit_config(super::config(&empty)).unwrap_or_else(|_| unreachable!());
    assert_rejected_unchanged(&missing, &record.run_identity, &empty);
    assert!(!missing.config.replay_database_path.exists());
    assert!(!empty.path().join("github.sqlite-wal").exists());
    assert!(!empty.path().join("github.sqlite-shm").exists());
}

// r[verify kiln.dashboard.repositories.github_export.scenario.admission]
#[test]
fn malformed_and_corrupt_stores_preserve_prior_report() {
    let corruptions = [
        "PRAGMA user_version=2",
        "UPDATE github_meta SET schema='unknown'",
        "UPDATE github_runs SET record=x'ff'",
        "UPDATE github_runs SET record=CAST(replace(CAST(record AS TEXT), 'github-run-', 'different-run-') AS BLOB)",
        "UPDATE github_runs SET phase='unresolved'",
        "UPDATE github_runs SET immutable_identity='wrong'",
        "UPDATE github_runs SET version=-1",
        "UPDATE github_runs SET record=zeroblob(65537)",
    ];
    for sql in corruptions {
        let (root, admitted, record) = fixture(Outcome::Success);
        let connection = Connection::open(&admitted.config.replay_database_path)
            .unwrap_or_else(|_| unreachable!());
        connection
            .execute_batch(sql)
            .unwrap_or_else(|_| unreachable!());
        drop(connection);
        assert_rejected_unchanged(&admitted, &record.run_identity, &root);
    }
    let (root, admitted, record) = fixture(Outcome::Success);
    std::fs::write(&admitted.config.replay_database_path, b"not-sqlite")
        .unwrap_or_else(|_| unreachable!());
    assert_rejected_unchanged(&admitted, &record.run_identity, &root);

    let (root, admitted, record) = fixture(Outcome::Success);
    std::fs::write(&admitted.config.replay_database_path, b"").unwrap_or_else(|_| unreachable!());
    assert_rejected_unchanged(&admitted, &record.run_identity, &root);
}

// r[verify kiln.dashboard.repositories.github_export.scenario.admission]
#[test]
fn malformed_profile_does_not_create_a_database() {
    let root = TempDir::new().unwrap_or_else(|_| unreachable!());
    let profile = root.path().join("profile.json");
    std::fs::write(&profile, b"{bad-json").unwrap_or_else(|_| unreachable!());
    assert!(load_config(&profile).is_err());
    assert!(!root.path().join("github.sqlite").exists());
}

// r[verify kiln.dashboard.repositories.github_export.scenario.admission]
#[test]
fn export_rejects_source_aliases_and_sidecars_without_replacement() {
    let (root, admitted, record) = fixture(Outcome::Success);
    let profile = root.path().join("profile.json");
    std::fs::write(
        &profile,
        serde_json::to_vec(&admitted.config).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());
    let database_before =
        std::fs::read(&admitted.config.replay_database_path).unwrap_or_else(|_| unreachable!());
    let profile_before = std::fs::read(&profile).unwrap_or_else(|_| unreachable!());
    for output in [&profile, &admitted.config.replay_database_path] {
        assert!(
            export_observation(&admitted, &profile, &record.run_identity, output, None).is_err()
        );
    }
    for suffix in ["-wal", "-shm", "-journal"] {
        let output = root.path().join(format!("github.sqlite{suffix}"));
        assert!(
            export_observation(&admitted, &profile, &record.run_identity, &output, None).is_err()
        );
    }
    let alias = root.path().join("alias.sqlite");
    std::fs::hard_link(&admitted.config.replay_database_path, &alias)
        .unwrap_or_else(|_| unreachable!());
    assert!(export_observation(&admitted, &profile, &record.run_identity, &alias, None).is_err());
    #[cfg(unix)]
    {
        let link = root.path().join("symlink.sqlite");
        std::os::unix::fs::symlink(&admitted.config.replay_database_path, &link)
            .unwrap_or_else(|_| unreachable!());
        assert!(
            export_observation(&admitted, &profile, &record.run_identity, &link, None).is_err()
        );
        let db_link = root.path().join("database-link.sqlite");
        std::os::unix::fs::symlink(&admitted.config.replay_database_path, &db_link)
            .unwrap_or_else(|_| unreachable!());
        // Exercise protection of the real database sidecars through a DB alias.
        let mut config = admitted.config.clone();
        config.replay_database_path = db_link;
        let alias_profile = admit_config(config).unwrap_or_else(|_| unreachable!());
        let sidecar = root.path().join("github.sqlite-wal");
        assert!(
            export_observation(
                &alias_profile,
                &profile,
                &record.run_identity,
                &sidecar,
                None
            )
            .is_err()
        );
    }
    assert_eq!(
        std::fs::read(&admitted.config.replay_database_path).unwrap_or_else(|_| unreachable!()),
        database_before
    );
    assert_eq!(
        std::fs::read(&profile).unwrap_or_else(|_| unreachable!()),
        profile_before
    );
    let output = root.path().join("missing-parent").join("journal.json");
    assert!(export_observation(&admitted, &profile, &record.run_identity, &output, None).is_err());
    assert!(!output.exists());
}

// r[verify kiln.dashboard.repositories.github_export]
#[cfg(unix)]
#[test]
fn export_never_resolves_credentials_or_contacts_workflow() {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    let root = TempDir::new().unwrap_or_else(|_| unreachable!());
    let marker = root.path().join("credential-access");
    let program = root.path().join("secretspec");
    let script = format!(
        "#!/bin/sh\ntouch '{}'\nprintf 'secret-must-not-leak'\n",
        marker.display()
    );
    std::fs::write(&program, &script).unwrap_or_else(|_| unreachable!());
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700))
        .unwrap_or_else(|_| unreachable!());
    let mut config = super::config(&root);
    config.webhook_secret.program_path = program.clone();
    config.webhook_secret.program_blake3 = blake3::hash(script.as_bytes()).to_hex().to_string();
    config.status_secret = config.webhook_secret.clone();
    let listener =
        UnixListener::bind(&config.workflow.socket_path).unwrap_or_else(|_| unreachable!());
    listener
        .set_nonblocking(true)
        .unwrap_or_else(|_| unreachable!());
    let admitted = admit_config(config).unwrap_or_else(|_| unreachable!());
    let event = super::event(&admitted, 'd', 2);
    let record = plan_event(&admitted, &event).unwrap_or_else(|_| unreachable!());
    let mut store = open_store(&admitted).unwrap_or_else(|_| unreachable!());
    store
        .reserve(&event, record.clone())
        .unwrap_or_else(|_| unreachable!());
    drop(store);
    let profile = root.path().join("profile.json");
    std::fs::write(
        &profile,
        serde_json::to_vec(&admitted.config).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());
    let loaded = load_config(&profile).unwrap_or_else(|_| unreachable!());
    let output = root.path().join("journal.json");
    export_observation(&loaded, &profile, &record.run_identity, &output, None)
        .unwrap_or_else(|_| unreachable!());
    assert!(!marker.exists());
    assert_eq!(
        listener
            .accept()
            .err()
            .unwrap_or_else(|| unreachable!())
            .kind(),
        std::io::ErrorKind::WouldBlock
    );
    let text = std::fs::read_to_string(output).unwrap_or_else(|_| unreachable!());
    assert!(!text.contains("secret-must-not-leak"));
    assert!(!text.contains(program.to_str().unwrap_or_else(|| unreachable!())));
}

// r[verify kiln.dashboard.repositories.github_export.scenario.admission]
#[test]
fn snapshot_export_preserves_original_profile_and_protects_snapshot() {
    let (root, admitted, record) = fixture(Outcome::Success);
    let expected = encode_observation(&admitted, &record.run_identity, None)
        .unwrap_or_else(|_| unreachable!());
    let snapshot = root.path().join("retained-snapshot.sqlite");
    std::fs::copy(&admitted.config.replay_database_path, &snapshot)
        .unwrap_or_else(|_| unreachable!());
    let snapshot_before = std::fs::read(&snapshot).unwrap_or_else(|_| unreachable!());
    std::fs::remove_file(&admitted.config.replay_database_path).unwrap_or_else(|_| unreachable!());
    let profile = root.path().join("profile.json");
    std::fs::write(
        &profile,
        serde_json::to_vec(&admitted.config).unwrap_or_else(|_| unreachable!()),
    )
    .unwrap_or_else(|_| unreachable!());
    let loaded = load_config(&profile).unwrap_or_else(|_| unreachable!());
    let output = retained_output(&root);
    export_observation(
        &loaded,
        &profile,
        &record.run_identity,
        &output,
        Some(&snapshot),
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        std::fs::read(&output).unwrap_or_else(|_| unreachable!()),
        expected
    );
    assert_eq!(loaded.identity, admitted.identity);
    assert!(!admitted.config.replay_database_path.exists());
    assert_eq!(
        std::fs::read(&snapshot).unwrap_or_else(|_| unreachable!()),
        snapshot_before
    );
    assert!(
        export_observation(
            &loaded,
            &profile,
            &record.run_identity,
            &snapshot,
            Some(&snapshot)
        )
        .is_err()
    );
    let hardlink = root.path().join("snapshot-alias.sqlite");
    std::fs::hard_link(&snapshot, &hardlink).unwrap_or_else(|_| unreachable!());
    assert!(
        export_observation(
            &loaded,
            &profile,
            &record.run_identity,
            &hardlink,
            Some(&snapshot)
        )
        .is_err()
    );
    for suffix in ["-wal", "-shm", "-journal"] {
        let sidecar = root
            .path()
            .join(format!("retained-snapshot.sqlite{suffix}"));
        assert!(
            export_observation(
                &loaded,
                &profile,
                &record.run_identity,
                &sidecar,
                Some(&snapshot)
            )
            .is_err()
        );
    }
    let mut changed = loaded.config.clone();
    changed.worker_poll_interval_ms += 1;
    let changed = admit_config(changed).unwrap_or_else(|_| unreachable!());
    assert!(
        export_observation(
            &changed,
            &profile,
            &record.run_identity,
            &output,
            Some(&snapshot)
        )
        .is_err()
    );
    assert_eq!(
        std::fs::read(&output).unwrap_or_else(|_| unreachable!()),
        expected
    );
    let missing = root.path().join("missing-snapshot.sqlite");
    assert!(
        export_observation(
            &loaded,
            &profile,
            &record.run_identity,
            &output,
            Some(&missing)
        )
        .is_err()
    );
    assert!(!missing.exists());
    assert_eq!(
        std::fs::read(&output).unwrap_or_else(|_| unreachable!()),
        expected
    );
    assert_eq!(
        std::fs::read(&snapshot).unwrap_or_else(|_| unreachable!()),
        snapshot_before
    );
}
