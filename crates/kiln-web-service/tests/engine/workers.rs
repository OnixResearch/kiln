use super::*;

fn inventory(engine: &mut Engine) -> Value {
    let response = get(engine, "/api/workers", Role::Viewer);
    assert_eq!(response.status, 200);
    value(&response)
}

fn workers(snapshot: &Value) -> &[Value] {
    checked(field(snapshot, "workers").as_array().ok_or("workers")).as_slice()
}

fn only_worker(snapshot: &Value) -> &Value {
    let items = workers(snapshot);
    assert_eq!(items.len(), 1);
    checked(items.first().ok_or("worker"))
}

fn run_id(run: &Value) -> &str {
    checked(field(run, "id").as_str().ok_or("run id"))
}

fn capacity(snapshot: &Value, enrolled: usize, active: usize, free: usize, queued: usize) {
    assert_eq!(
        field(only_worker(snapshot), "capacity"),
        &json!({"configured_slots":1,"enrolled_slots":enrolled,"active":active,"free_slots":free,"queued":queued})
    );
    assert_eq!(
        field(snapshot, "summary"),
        &json!({"targets":1,"configured_slots":1,"enrolled_slots":enrolled,"active":active,"free_slots":free,"queued":queued})
    );
}

fn open_projects(profile: ServiceProfile, backend: Arc<Mutex<Backend>>) -> Engine {
    let mut ports: BTreeMap<String, Box<dyn WorkflowPort + Send>> = BTreeMap::new();
    for project in &profile.projects {
        ports.insert(
            project.id.clone(),
            Box::new(Port {
                profile: checked(project.lattice(profile.bounds.max_runs)).provider,
                backend: backend.clone(),
            }),
        );
    }
    checked(Engine::open_with_ports(profile, ports))
}

fn stored_state(profile: &ServiceProfile) -> Vec<u8> {
    let connection = checked(rusqlite::Connection::open(
        profile.state_directory.join("service.sqlite"),
    ));
    checked(
        connection.query_row("SELECT state FROM service_state WHERE id=1", [], |row| {
            row.get(0)
        }),
    )
}

// r[verify kiln.dashboard.workers.inventory]
// r[verify kiln.dashboard.workers.inventory.scenario.grouping]
#[test]
fn inventory_groups_exact_bindings_with_stable_redacted_target_identity() {
    let (root, mut profile, _) = fixture();
    profile.bounds.max_projects = 4;
    let original = checked(profile.projects.first().ok_or("project")).clone();
    for id in ["shared", "endpoint", "provider"] {
        let mut project = original.clone();
        project.id = id.into();
        project.label = format!("Public {id}");
        match id {
            "endpoint" => project.workflow.socket_path = root.path().join("other.sock"),
            "provider" => project.workflow.provider_profile_id = "another-provider".into(),
            _ => {}
        }
        profile.projects.push(project);
    }
    // The pinned adapter admits only linux-x86_64; unsupported platforms
    // must fail before a target can appear in the inventory.
    let mut unsupported = profile.clone();
    checked(unsupported.projects.first_mut().ok_or("project"))
        .workflow
        .platform = "linux-aarch64".into();
    assert!(unsupported.admit().is_err());
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open_projects(profile.clone(), backend.clone());
    let snapshot = inventory(&mut engine);
    assert_eq!(field(&snapshot, "schema"), "kiln.web-workers.v1");
    assert_eq!(field(&snapshot, "bounded"), true);
    assert_eq!(
        field(&snapshot, "summary"),
        &json!({"targets":3,"configured_slots":4,"enrolled_slots":0,"active":0,"free_slots":0,"queued":0})
    );
    let ids: Vec<_> = workers(&snapshot).iter().map(|w| field(w, "id")).collect();
    let string_ids: Vec<_> = ids
        .iter()
        .map(|id| checked(id.as_str().ok_or("id")))
        .collect();
    assert!(
        string_ids
            .windows(2)
            .all(|pair| matches!(pair, [a, b] if a < b))
    );
    assert!(
        string_ids
            .iter()
            .all(|id| id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit()))
    );
    let shared = checked(
        workers(&snapshot)
            .iter()
            .find(|w| {
                field(w, "provider_profile_id") == "reviewed-lattice"
                    && field(w, "platform") == "linux-x86_64"
                    && field(w, "projects")
                        .as_array()
                        .is_some_and(|p| p.len() == 2)
            })
            .ok_or("shared target"),
    );
    assert_eq!(
        field(shared, "projects"),
        &json!([
            {"id":"fixture","label":"Fixture","enrolled":false,"active_run_ids":[],"queued":0},
            {"id":"shared","label":"Public shared","enrolled":false,"active_run_ids":[],"queued":0}
        ])
    );
    for worker in workers(&snapshot) {
        assert_eq!(field(worker, "status"), "not_enrolled");
        assert_eq!(field(worker, "liveness"), "unknown");
        assert_eq!(field(worker, "latest_run"), &Value::Null);
        let keys: std::collections::BTreeSet<_> = checked(worker.as_object().ok_or("worker"))
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            std::collections::BTreeSet::from([
                "id",
                "provider_profile_id",
                "platform",
                "liveness",
                "status",
                "capacity",
                "projects",
                "latest_run"
            ])
        );
    }
    let encoded = checked(serde_json::to_string(&snapshot));
    assert!(!encoded.contains(checked(root.path().to_str().ok_or("root path"))));
    for private in [
        "socket_path",
        "repository_path",
        "allowed_user_ids",
        "argv",
        "operator.token",
        "worker.token",
        "reviewed-policy",
    ] {
        assert!(!encoded.contains(private), "leaked {private}");
    }
    assert_eq!(
        value(&get(&mut engine, "/api/workers", Role::Operator)),
        snapshot
    );
    drop(engine);
    let mut engine = open_projects(profile.clone(), backend.clone());
    assert_eq!(inventory(&mut engine), snapshot);
    drop(engine);

    // Config ordering, public labels, project identity, credentials, policy and
    // state location are not part of the target identity tuple.
    profile.projects.reverse();
    for p in &mut profile.projects {
        p.label.push_str(" renamed");
        p.id.push_str("-renamed");
        p.workflow.policy_id = "changed-policy".into();
    }
    profile.operator_token_file = root.path().join("new-operator.token");
    profile.state_directory = root.path().join("independent-state");
    let mut engine = open_projects(profile, backend.clone());
    let changed = inventory(&mut engine);
    assert_eq!(
        workers(&changed)
            .iter()
            .map(|w| field(w, "id"))
            .collect::<Vec<_>>(),
        ids
    );
    assert_eq!(backend.lock().dispatches, 0);
    assert_eq!(backend.lock().recoveries, 0);
}

// r[verify kiln.dashboard.workers.capacity]
// r[verify kiln.dashboard.workers.capacity.scenario.transitions]
#[test]
fn enrollment_execution_queue_cancellation_and_completion_preserve_capacity_and_latest() {
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    let initial = inventory(&mut engine);
    let target_id = field(only_worker(&initial), "id").clone();
    capacity(&initial, 0, 0, 0, 0);
    assert_eq!(field(only_worker(&initial), "latest_run"), &Value::Null);
    enroll(&mut engine);
    let enrolled = inventory(&mut engine);
    capacity(&enrolled, 1, 0, 1, 0);
    assert_eq!(field(only_worker(&enrolled), "status"), "slots_available");
    let active = value(&start(&mut engine, "active"));
    let running = inventory(&mut engine);
    capacity(&running, 1, 1, 0, 0);
    assert_eq!(field(only_worker(&running), "status"), "busy");
    assert_eq!(
        only_worker(&running).pointer("/projects/0/active_run_ids"),
        Some(&json!([run_id(&active)]))
    );
    let latest = field(only_worker(&running), "latest_run");
    assert_eq!(
        latest,
        &json!({
            "id":run_id(&active),"project_id":"fixture","phase":field(&active,"phase"),
            "outcome":"Pending","sequence":field(&active,"sequence"),"error_code":null,
            "created_at_ms":field(&active,"created_at_ms"),"started_at_ms":field(&active,"started_at_ms"),"completed_at_ms":null
        })
    );
    let queued_response = start(&mut engine, "queued");
    assert_eq!(queued_response.status, 202);
    let queued = value(&queued_response);
    let waiting = inventory(&mut engine);
    capacity(&waiting, 1, 1, 0, 1);
    assert_eq!(field(only_worker(&waiting), "latest_run"), latest);
    assert_eq!(
        only_worker(&waiting).pointer("/projects/0/queued"),
        Some(&json!(1))
    );
    let cancelled = call(
        &mut engine,
        "POST",
        &format!("/api/runs/{}/control", run_id(&queued)),
        json!({"request_id":"cancel-queue","action":"cancel"}),
        Role::Operator,
    );
    assert_eq!(cancelled.status, 200);
    let after_cancel = inventory(&mut engine);
    capacity(&after_cancel, 1, 1, 0, 0);
    assert_eq!(field(only_worker(&after_cancel), "latest_run"), latest);
    assert_eq!(backend.lock().dispatches, 1);
    assert_eq!(backend.lock().controls, 0);
    {
        let mut state = backend.lock();
        state.sequence = 4;
        state.terminal = true;
    }
    poll(&mut engine);
    let complete = inventory(&mut engine);
    capacity(&complete, 1, 0, 1, 0);
    assert_eq!(field(only_worker(&complete), "status"), "slots_available");
    assert_eq!(
        only_worker(&complete).pointer("/projects/0/active_run_ids"),
        Some(&json!([]))
    );
    let terminal = field(only_worker(&complete), "latest_run");
    assert_eq!(field(terminal, "id"), run_id(&active));
    assert_eq!(field(terminal, "outcome"), "Success");
    assert_eq!(field(terminal, "phase"), "terminal");
    assert_eq!(field(terminal, "sequence"), 4);
    assert!(field(terminal, "completed_at_ms").as_u64().is_some());
    assert_eq!(field(only_worker(&complete), "id"), &target_id);
    assert!(field(&complete, "version").as_u64() > field(&initial, "version").as_u64());
    drop(engine);
    let mut engine = open(profile, backend.clone());
    assert_eq!(inventory(&mut engine), complete);
    assert_eq!(backend.lock().dispatches, 1);
    let newer = value(&start(&mut engine, "newer-execution"));
    let latest_snapshot = inventory(&mut engine);
    capacity(&latest_snapshot, 1, 0, 1, 0);
    assert_eq!(
        only_worker(&latest_snapshot).pointer("/latest_run/id"),
        Some(&json!(run_id(&newer)))
    );
    assert_ne!(run_id(&newer), run_id(&active));
    assert_eq!(field(only_worker(&latest_snapshot), "id"), &target_id);
    assert_eq!(backend.lock().dispatches, 2);
}

// r[verify kiln.dashboard.workers.capacity.scenario.unknown]
#[test]
fn ambiguous_attempts_hold_slots_without_fabricating_observations() {
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend {
        disconnect: true,
        ..Backend::default()
    }));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    let unknown = value(&start(&mut engine, "unknown"));
    let queued = start(&mut engine, "waiting");
    assert_eq!(queued.status, 202);
    let snapshot = inventory(&mut engine);
    capacity(&snapshot, 1, 1, 0, 1);
    let worker = only_worker(&snapshot);
    assert_eq!(field(worker, "status"), "unknown");
    assert_eq!(field(worker, "liveness"), "unknown");
    let latest = field(worker, "latest_run");
    assert_eq!(field(latest, "id"), run_id(&unknown));
    assert_eq!(field(latest, "outcome"), "Unknown");
    assert_eq!(field(latest, "sequence"), &Value::Null);
    assert_eq!(field(latest, "error_code"), "provider_unknown");
    assert_eq!(field(latest, "started_at_ms"), &Value::Null);
    assert_eq!(field(latest, "completed_at_ms"), &Value::Null);
    assert_eq!(
        worker.pointer("/projects/0/active_run_ids"),
        Some(&json!([run_id(&unknown)]))
    );
    drop(engine);
    let mut engine = open(profile, backend.clone());
    assert_eq!(inventory(&mut engine), snapshot);
    assert_eq!(backend.lock().dispatches, 1);
    assert_eq!(backend.lock().recoveries, 0);
}

// r[verify kiln.dashboard.workers.capacity.scenario.unknown]
#[test]
fn shared_target_free_enrolled_slot_precedes_ambiguous_status() {
    let (_root, mut profile, _) = fixture();
    let mut sibling = checked(profile.projects.first().ok_or("project")).clone();
    sibling.id = "sibling".into();
    sibling.label = "Sibling".into();
    profile.projects.push(sibling);
    let backend = Arc::new(Mutex::new(Backend {
        disconnect: true,
        ..Backend::default()
    }));
    let mut engine = open_projects(profile, backend.clone());
    enroll(&mut engine);
    let unknown = value(&start(&mut engine, "unknown"));
    let before = inventory(&mut engine);
    assert_eq!(field(only_worker(&before), "status"), "unknown");
    assert_eq!(
        only_worker(&before).pointer("/capacity/free_slots"),
        Some(&json!(0))
    );
    assert_eq!(
        call(
            &mut engine,
            "POST",
            "/api/projects",
            json!({"project_id":"sibling","request_id":"enroll-sibling"}),
            Role::Operator,
        )
        .status,
        200
    );
    let snapshot = inventory(&mut engine);
    assert_eq!(field(only_worker(&snapshot), "status"), "slots_available");
    assert_eq!(
        field(only_worker(&snapshot), "capacity"),
        &json!({"configured_slots":2,"enrolled_slots":2,"active":1,"free_slots":1,"queued":0})
    );
    assert_eq!(field(only_worker(&snapshot), "liveness"), "unknown");
    assert_eq!(
        only_worker(&snapshot).pointer("/latest_run/id"),
        Some(&json!(run_id(&unknown)))
    );
    assert_eq!(backend.lock().dispatches, 1);
}

// r[verify kiln.dashboard.workers.inventory.scenario.denied]
#[test]
fn worker_role_queries_bodies_and_nonread_methods_fail_closed() {
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile, backend.clone());
    let before = inventory(&mut engine);
    assert_eq!(get(&mut engine, "/api/workers", Role::Worker).status, 403);
    for query in [
        "?q=fixture",
        "?worker=fixture",
        "?status=busy",
        "?q=a&q=b",
        "?unknown",
    ] {
        assert_eq!(
            get(&mut engine, &format!("/api/workers{query}"), Role::Viewer).status,
            400
        );
    }
    for body in [b"{}".as_slice(), b"null".as_slice(), b" ".as_slice()] {
        assert_eq!(
            checked(engine.api("GET", "/api/workers", body, Role::Viewer)).status,
            400
        );
    }
    for method in ["POST", "PUT", "PATCH", "DELETE", "HEAD"] {
        assert_eq!(
            checked(engine.api(method, "/api/workers", &[], Role::Operator)).status,
            404
        );
    }
    assert_eq!(
        checked(engine.api("POST", "/api/workers", &[], Role::Viewer)).status,
        403
    );
    let after = inventory(&mut engine);
    // Denial auditing may change snapshot version, never inventory facts.
    assert_eq!(field(&after, "workers"), field(&before, "workers"));
    assert_eq!(field(&after, "summary"), field(&before, "summary"));
    assert_eq!(backend.lock().dispatches, 0);
    assert_eq!(backend.lock().recoveries, 0);
    assert_eq!(backend.lock().controls, 0);
}

// r[verify kiln.dashboard.workers.inventory.scenario.grouping]
#[test]
fn successful_reads_neither_persist_nor_refresh_provider_observations() {
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    let active = value(&start(&mut engine, "active"));
    let before = inventory(&mut engine);
    let durable = stored_state(&profile);
    let connection = checked(rusqlite::Connection::open(
        profile.state_directory.join("service.sqlite"),
    ));
    checked(connection.execute_batch("CREATE TRIGGER reject_state BEFORE UPDATE ON service_state BEGIN SELECT RAISE(ABORT, 'reads must not persist'); END;"));
    {
        let mut state = backend.lock();
        state.terminal = true;
        state.sequence = 9;
    }
    assert_eq!(inventory(&mut engine), before);
    let operator = get(&mut engine, "/api/workers", Role::Operator);
    assert_eq!(operator.status, 200);
    assert_eq!(value(&operator), before);
    assert_eq!(stored_state(&profile), durable);
    assert_eq!(
        only_worker(&before).pointer("/latest_run/id"),
        Some(&json!(run_id(&active)))
    );
    assert_eq!(
        only_worker(&before).pointer("/latest_run/outcome"),
        Some(&json!("Pending"))
    );
    assert_eq!(backend.lock().dispatches, 1);
    assert_eq!(backend.lock().recoveries, 0);
    assert_eq!(backend.lock().controls, 0);
}

// r[verify kiln.dashboard.workers.capacity.scenario.unknown]
#[test]
fn failed_storage_denies_inventory_without_reporting_usable_capacity() {
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    let connection = checked(rusqlite::Connection::open(
        profile.state_directory.join("service.sqlite"),
    ));
    checked(connection.execute_batch("CREATE TRIGGER reject_state BEFORE UPDATE ON service_state BEGIN SELECT RAISE(ABORT, 'isolated disk failure'); END;"));
    assert!(
        engine
            .api(
                "POST",
                "/api/runs",
                br#"{"project_id":"fixture","request_id":"fail"}"#,
                Role::Operator
            )
            .is_err()
    );
    for role in [Role::Viewer, Role::Operator, Role::Worker] {
        let response = get(&mut engine, "/api/workers", role);
        assert_eq!(response.status, 503);
        let failure = value(&response);
        assert_eq!(field(&failure, "code"), "durable_state_unavailable");
        assert!(failure.get("workers").is_none());
        assert!(failure.get("summary").is_none());
    }
    assert_eq!(backend.lock().dispatches, 0);
}

// r[verify kiln.dashboard.workers.capacity.scenario.unknown]
#[test]
fn legacy_missing_timestamps_remain_null_in_latest_execution() {
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend {
        terminal: true,
        ..Backend::default()
    }));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    let completed = value(&start(&mut engine, "legacy"));
    drop(engine);
    let mut state: Value = checked(serde_json::from_slice(&stored_state(&profile)));
    let record = checked(
        state
            .get_mut("runs")
            .and_then(|runs| runs.get_mut(run_id(&completed)))
            .and_then(Value::as_object_mut)
            .ok_or("run record"),
    );
    for name in [
        "created_at_ms",
        "started_at_ms",
        "completed_at_ms",
        "event_recorded_at_ms",
        "node_timing",
    ] {
        record.remove(name);
    }
    let connection = checked(rusqlite::Connection::open(
        profile.state_directory.join("service.sqlite"),
    ));
    checked(connection.execute(
        "UPDATE service_state SET state=?1 WHERE id=1",
        [checked(serde_json::to_vec(&state))],
    ));
    drop(connection);
    let mut engine = open(profile, backend);
    let snapshot = inventory(&mut engine);
    capacity(&snapshot, 1, 0, 1, 0);
    let latest = field(only_worker(&snapshot), "latest_run");
    assert_eq!(field(latest, "id"), run_id(&completed));
    assert_eq!(field(latest, "outcome"), "Success");
    assert_eq!(field(latest, "sequence"), 0);
    assert_eq!(field(latest, "error_code"), &Value::Null);
    for name in ["created_at_ms", "started_at_ms", "completed_at_ms"] {
        assert_eq!(latest.get(name), Some(&Value::Null));
    }
}
