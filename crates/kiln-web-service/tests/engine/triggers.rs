//! Web automation trigger filters and cron schedules through the real engine.
use super::*;
use hmac::{Hmac, Mac};
use kiln_adapter_github::events::DeliveryHeaders;
use kiln_web_service::automation::AutomationProfile;
use sha2::Sha256;
use std::os::unix::fs::PermissionsExt;

fn policy(root: &std::path::Path, triggers: Value, schedules: Value) -> AutomationProfile {
    let secret = root.join("github.secret");
    checked(std::fs::write(&secret, b"private-github-test-secret"));
    checked(std::fs::set_permissions(
        &secret,
        std::fs::Permissions::from_mode(0o600),
    ));
    let triggers: Vec<Value> = triggers
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|mut t| {
            set(&mut t, "secret_file", json!(secret));
            t
        })
        .collect();
    checked(serde_json::from_value(
        json!({"schema":"kiln.web-automation.v1","max_records":100,"status_limit":20,"max_event_bytes":65536,"max_tick_effects":2,
        "triggers":triggers,"schedules":schedules,"notifications":[]}),
    ))
}
fn push(revision: &str) -> Vec<u8> {
    checked(serde_json::to_vec(
        &json!({"repository":{"id":42,"name":"source","full_name":"fixture/source","owner":{"login":"fixture"}},"ref":"refs/heads/main","after":revision,"deleted":false}),
    ))
}
fn signed(delivery: &str, body: &[u8]) -> DeliveryHeaders {
    let mut mac = checked(Hmac::<Sha256>::new_from_slice(
        b"private-github-test-secret",
    ));
    mac.update(body);
    let signature: String = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    DeliveryHeaders {
        delivery: delivery.into(),
        event: "push".into(),
        signature: format!("sha256={signature}"),
    }
}
/// Commit `path` on refs/heads/main of the fixture repository.
fn commit(profile: &ServiceProfile, path: &str) -> String {
    let repository_path = &checked(profile.projects.first().ok_or("project")).repository_path;
    let repository = checked(git2::Repository::open(repository_path));
    checked(std::fs::write(repository_path.join(path), path.as_bytes()));
    let mut index = checked(repository.index());
    checked(index.add_path(std::path::Path::new(path)));
    let tree = checked(repository.find_tree(checked(index.write_tree())));
    let parent = checked(checked(repository.revparse_single("refs/heads/main")).peel_to_commit());
    let signature = checked(git2::Signature::now("test", "test@example.invalid"));
    checked(repository.commit(
        Some("refs/heads/main"),
        &signature,
        &signature,
        path,
        &tree,
        &[&parent],
    ))
    .to_string()
}
fn sources(engine: &mut Engine) -> Vec<Value> {
    value(&get(engine, "/api/automation", Role::Operator))
        .get("sources")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

// r[verify kiln.triggers.record]
// r[verify kiln.triggers.evaluate.scenario.unknown]
#[test]
fn path_filter_runs_on_unknown_files_and_records_exclusion_without_a_run() {
    let (root, profile, revision) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    let triggers = json!([{"id":"docs","project_id":"fixture","reference":"refs/heads/main","repository":{"owner":"fixture","repository":"source","repository_id":42,"allow_fork_pull_requests":false},"enabled":true,"push":true,"pull_request":false,"pull_request_base":null,"filter":{"paths":["docs/**"]}}]);
    checked(engine.configure_automation(policy(root.path(), triggers, json!([]))));
    // First push of the reference: no base, changed files unknown, so it runs.
    let body = push(&revision);
    assert_eq!(
        checked(engine.automation_trigger(
            "docs",
            &signed("00000000-0000-4000-8000-000000000011", &body),
            &body
        ))
        .status,
        202
    );
    assert_eq!(backend.lock().dispatches, 1);
    let first = sources(&mut engine);
    assert_eq!(
        first
            .first()
            .and_then(|s| s.pointer("/filter/reasons/0/filter")),
        Some(&json!("paths_unknown"))
    );
    // An exact diff outside docs/ is filtered: recorded, never dispatched.
    let outside = commit(&profile, "outside.txt");
    let body = push(&outside);
    assert_eq!(
        checked(engine.automation_trigger(
            "docs",
            &signed("00000000-0000-4000-8000-000000000012", &body),
            &body
        ))
        .status,
        202
    );
    assert_eq!(backend.lock().dispatches, 1);
    let filtered = sources(&mut engine);
    let record = checked(
        filtered
            .iter()
            .find(|s| s.get("status") == Some(&json!("filtered")))
            .ok_or("filtered record"),
    );
    assert_eq!(record.pointer("/filter/excluded_by"), Some(&json!("paths")));
    assert_eq!(record.get("run_id"), Some(&Value::Null));
    // A change under docs/ matches with the exact path recorded.
    checked(std::fs::create_dir_all(
        checked(profile.projects.first().ok_or("project"))
            .repository_path
            .join("docs"),
    ));
    let inside = commit(&profile, "docs/guide.md");
    let body = push(&inside);
    assert_eq!(
        checked(engine.automation_trigger(
            "docs",
            &signed("00000000-0000-4000-8000-000000000013", &body),
            &body
        ))
        .status,
        202
    );
    // The run is queued behind the first one (one admission slot).
    let matched = sources(&mut engine);
    let record = checked(
        matched
            .iter()
            .find(|s| s.pointer("/filter/reasons/0/path") == Some(&json!("docs/guide.md")))
            .ok_or("matched record"),
    );
    assert!(record.get("run_id").and_then(Value::as_str).is_some());
}

fn retained(profile: &ServiceProfile, pointer: &str, replacement: Value) {
    let connection = checked(rusqlite::Connection::open(
        profile.state_directory.join("service.sqlite"),
    ));
    let bytes: Vec<u8> = checked(connection.query_row(
        "SELECT state FROM service_state WHERE id=1",
        [],
        |row| row.get(0),
    ));
    let mut state: Value = checked(serde_json::from_slice(&bytes));
    *checked(state.pointer_mut(pointer).ok_or("retained field")) = replacement;
    checked(connection.execute(
        "UPDATE service_state SET state=?1 WHERE id=1",
        [checked(serde_json::to_vec(&state))],
    ));
}
fn now_ms() -> u64 {
    checked(u64::try_from(
        checked(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)).as_millis(),
    ))
}

// r[verify kiln.schedules.no_catch_up.scenario.restart]
// r[verify kiln.schedules.replay]
#[test]
fn cron_restart_in_missed_window_never_catches_up_and_fire_binds_exact_revision() {
    let (root, profile, revision) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let schedules = json!([{"id":"nightly","project_id":"fixture","reference":"refs/heads/main","enabled":true,"cron":{"expression":"0 2 * * *","timezone":"America/New_York"},"min_interval_ms":300000,"max_interval_ms":31536000000_u64,"next_due_ms":null}]);
    let automation = policy(root.path(), json!([]), schedules);
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    checked(engine.configure_automation(automation.clone()));
    let configured = value(&get(&mut engine, "/api/automation", Role::Operator));
    let due = checked(
        configured
            .pointer("/schedules/0/next_due_ms")
            .and_then(Value::as_u64)
            .ok_or("due"),
    );
    assert!(due > now_ms());
    drop(engine);
    // The service was down across an occurrence: restart must not run it.
    retained(
        &profile,
        "/automation/settings/schedule:nightly/next_due_ms",
        json!(now_ms() - 3_600_000),
    );
    let mut engine = open(profile.clone(), backend.clone());
    checked(engine.configure_automation(automation.clone()));
    checked(engine.tick_automation());
    assert_eq!(backend.lock().dispatches, 0);
    let after = value(&get(&mut engine, "/api/automation", Role::Operator));
    assert_eq!(
        after.pointer("/schedules/0/last_status"),
        Some(&json!("missed"))
    );
    assert!(
        checked(
            after
                .pointer("/schedules/0/next_due_ms")
                .and_then(Value::as_u64)
                .ok_or("next")
        ) > now_ms()
    );
    assert!(sources(&mut engine).is_empty());
    drop(engine);
    // A just-due occurrence fires once with the exact reference revision.
    retained(
        &profile,
        "/automation/settings/schedule:nightly/next_due_ms",
        json!(now_ms() - 1_000),
    );
    let mut engine = open(profile.clone(), backend.clone());
    checked(engine.configure_automation(automation));
    checked(engine.tick_automation());
    checked(engine.tick_automation());
    assert_eq!(backend.lock().dispatches, 1);
    let fired = sources(&mut engine);
    assert_eq!(fired.len(), 1);
    assert_eq!(
        fired.first().and_then(|s| s.get("source_revision")),
        Some(&json!(revision))
    );
}
