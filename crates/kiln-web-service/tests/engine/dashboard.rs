use super::*;
use kiln_web_service::access::Principal;

fn id(run: &Value) -> &str {
    checked(field(run, "id").as_str().ok_or("run id"))
}
fn finish(engine: &mut Engine, backend: &Arc<Mutex<Backend>>) {
    {
        let mut state = backend.lock();
        state.terminal = true;
        state.sequence += 1;
    }
    poll(engine);
}
fn rerun(engine: &mut Engine, run: &Value, key: &str, mode: &str) -> ApiResponse {
    call(
        engine,
        "POST",
        &format!("/api/runs/{}/rerun", id(run)),
        json!({"request_id":key,"mode":mode}),
        Role::Operator,
    )
}
fn dag(profile: &mut ServiceProfile) {
    let project = checked(profile.projects.first_mut().ok_or("project"));
    let original = checked(project.workflow.nodes.first().ok_or("node")).clone();
    project.workflow.nodes = ["build", "publish", "test"]
        .iter()
        .map(|name| {
            let mut node = original.clone();
            node.id = (*name).into();
            node
        })
        .collect();
    project.workflow.edges = checked(serde_json::from_value(
        json!([{"source":"build","target":"test"},{"source":"test","target":"publish"}]),
    ));
}

#[test]
fn viewer_denials_are_attributable_and_before_effects() {
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile, backend.clone());
    let viewer = Principal {
        id: "tailscale:42".into(),
        label: "Read-only member".into(),
        role: Role::Viewer,
    };
    let response = checked(engine.api_as(
        "POST",
        "/api/projects",
        br#"{"project_id":"fixture","request_id":"forbidden"}"#,
        &viewer,
    ));
    assert_eq!(response.status, 403);
    let projects = value(&get(&mut engine, "/api/projects", Role::Viewer));
    assert_eq!(field(&projects, "enrolled"), &json!([]));
    assert_eq!(backend.lock().dispatches, 0);
    let activity = value(&get(&mut engine, "/api/activity", Role::Viewer));
    let items = checked(field(&activity, "items").as_array().ok_or("items"));
    assert!(items.iter().any(
        |item| item.get("audit").is_some_and(|audit| audit.get("actor_id")
            == Some(&json!("tailscale:42"))
            && audit.get("status") == Some(&json!(403)))
    ));
    let session = value(&checked(engine.api_as("GET", "/api/session", &[], &viewer)));
    assert_eq!(
        session
            .pointer("/permissions/operate")
            .unwrap_or(&serde_json::Value::Null),
        false
    );
    enroll(&mut engine);
    assert_eq!(start(&mut engine, "admitted").status, 201);
    assert_eq!(backend.lock().dispatches, 1);
}

#[test]
fn durable_queue_dispatches_once_in_order_and_cancellation_has_no_provider_effect() {
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    let active = value(&start(&mut engine, "active"));
    let queued_response = start(&mut engine, "queued");
    assert_eq!(queued_response.status, 202);
    let queued = value(&queued_response);
    let cancelled = value(&start(&mut engine, "cancel-me"));
    let cancel_path = format!("/api/runs/{}/control", id(&cancelled));
    let cancel = json!({"request_id":"queue-cancel","action":"cancel"});
    assert_eq!(
        call(
            &mut engine,
            "POST",
            &cancel_path,
            cancel.clone(),
            Role::Viewer
        )
        .status,
        403
    );
    let response = call(
        &mut engine,
        "POST",
        &cancel_path,
        cancel.clone(),
        Role::Operator,
    );
    assert_eq!(response.status, 200);
    assert_eq!(
        value(&response)
            .pointer("/outcome")
            .unwrap_or(&serde_json::Value::Null),
        "Cancelled"
    );
    assert_eq!(
        call(&mut engine, "POST", &cancel_path, cancel, Role::Operator).body,
        response.body
    );
    assert_eq!(backend.lock().controls, 0);
    assert_eq!(backend.lock().dispatches, 1);
    drop(engine);
    let mut engine = open(profile.clone(), backend.clone());
    poll(&mut engine);
    assert_eq!(backend.lock().dispatches, 1);
    let operations = value(&get(&mut engine, "/api/operations", Role::Viewer));
    assert_eq!(
        operations
            .pointer("/queue")
            .unwrap_or(&serde_json::Value::Null),
        &json!([{"run_id":id(&queued),"project_id":"fixture","position":1,"created_at_ms":queued.pointer("/created_at_ms").unwrap_or(&serde_json::Value::Null)}])
    );
    finish(&mut engine, &backend);
    assert_eq!(backend.lock().dispatches, 2);
    assert_eq!(
        value(&get(
            &mut engine,
            &format!("/api/runs/{}", id(&active)),
            Role::Viewer
        ))
        .pointer("/outcome")
        .unwrap_or(&serde_json::Value::Null),
        "Success"
    );
    assert_eq!(
        value(&get(
            &mut engine,
            &format!("/api/runs/{}", id(&queued)),
            Role::Viewer
        ))
        .pointer("/outcome")
        .unwrap_or(&serde_json::Value::Null),
        "Success"
    );
    drop(engine);
    let mut engine = open(profile, backend.clone());
    poll(&mut engine);
    assert_eq!(backend.lock().dispatches, 2);
    assert_eq!(start(&mut engine, "queued").body, queued_response.body);
}

#[test]
fn stable_search_cursor_excludes_new_runs_and_rejects_changed_filters() {
    let (_root, mut profile, revision) = fixture();
    profile.bounds.max_runs = 8;
    let backend = Arc::new(Mutex::new(Backend {
        terminal: true,
        ..Backend::default()
    }));
    let mut engine = open(profile, backend);
    enroll(&mut engine);
    let first = value(&start(&mut engine, "one"));
    let second = value(&start(&mut engine, "two"));
    let third = value(&start(&mut engine, "three"));
    let response = value(&get(
        &mut engine,
        "/api/runs?project_id=fixture&outcome=success&q=fixture&limit=1",
        Role::Viewer,
    ));
    assert_eq!(
        response
            .pointer("/runs/0/id")
            .unwrap_or(&serde_json::Value::Null),
        third.pointer("/id").unwrap_or(&serde_json::Value::Null)
    );
    assert_eq!(
        response
            .pointer("/total")
            .unwrap_or(&serde_json::Value::Null),
        3
    );
    let cursor = checked(
        response
            .pointer("/next_cursor")
            .unwrap_or(&serde_json::Value::Null)
            .as_str()
            .ok_or("cursor"),
    );
    let fourth = value(&start(&mut engine, "four"));
    let next =
        format!("/api/runs?project_id=fixture&outcome=success&q=fixture&limit=1&cursor={cursor}");
    let response = value(&get(&mut engine, &next, Role::Viewer));
    assert_eq!(
        response
            .pointer("/runs/0/id")
            .unwrap_or(&serde_json::Value::Null),
        second.pointer("/id").unwrap_or(&serde_json::Value::Null)
    );
    assert_eq!(
        response
            .pointer("/total")
            .unwrap_or(&serde_json::Value::Null),
        3
    );
    assert_ne!(
        response
            .pointer("/runs/0/id")
            .unwrap_or(&serde_json::Value::Null),
        fourth.pointer("/id").unwrap_or(&serde_json::Value::Null)
    );
    let cursor = checked(
        response
            .pointer("/next_cursor")
            .unwrap_or(&serde_json::Value::Null)
            .as_str()
            .ok_or("cursor"),
    );
    let response = value(&get(
        &mut engine,
        &format!("/api/runs?project_id=fixture&outcome=success&q=fixture&limit=1&cursor={cursor}"),
        Role::Viewer,
    ));
    assert_eq!(
        response
            .pointer("/runs/0/id")
            .unwrap_or(&serde_json::Value::Null),
        first.pointer("/id").unwrap_or(&serde_json::Value::Null)
    );
    assert_eq!(
        response
            .pointer("/next_cursor")
            .unwrap_or(&serde_json::Value::Null),
        &Value::Null
    );
    assert_eq!(
        get(
            &mut engine,
            &format!("/api/runs?project_id=fixture&limit=1&cursor={cursor}"),
            Role::Viewer
        )
        .status,
        400
    );
    assert_eq!(
        get(&mut engine, "/api/runs?cursor=garbage", Role::Viewer).status,
        400
    );
    assert_eq!(
        get(&mut engine, "/api/runs?limit=0", Role::Viewer).status,
        400
    );
    assert_eq!(
        get(
            &mut engine,
            "/api/runs?project_id=fixture&project_id=fixture",
            Role::Viewer
        )
        .status,
        400
    );
    let filtered = value(&get(
        &mut engine,
        &format!("/api/runs?source_revision={revision}&order=asc"),
        Role::Viewer,
    ));
    assert_eq!(
        filtered
            .pointer("/runs/0/id")
            .unwrap_or(&serde_json::Value::Null),
        first.pointer("/id").unwrap_or(&serde_json::Value::Null)
    );
    assert_eq!(
        get(&mut engine, "/api/runs/absent", Role::Viewer).status,
        404
    );
}

#[test]
fn exact_and_latest_reruns_bind_new_candidates_to_selected_revisions() {
    let (_root, profile, revision) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    let original = value(&start(&mut engine, "original"));
    assert_eq!(
        rerun(&mut engine, &original, "too-early", "exact").status,
        409
    );
    finish(&mut engine, &backend);
    let repository = checked(git2::Repository::open(
        &checked(profile.projects.first().ok_or("project")).repository_path,
    ));
    let parent = checked(repository.find_commit(checked(git2::Oid::from_str(&revision))));
    let tree = checked(parent.tree());
    let signature = checked(git2::Signature::now("test", "test@example.invalid"));
    let new_revision = checked(repository.commit(
        Some("refs/heads/main"),
        &signature,
        &signature,
        "new admitted revision",
        &tree,
        &[&parent],
    ))
    .to_string();
    let exact = value(&rerun(&mut engine, &original, "exact", "exact"));
    assert_eq!(
        exact
            .pointer("/source_revision")
            .unwrap_or(&serde_json::Value::Null),
        &revision
    );
    assert_ne!(
        exact.pointer("/id").unwrap_or(&serde_json::Value::Null),
        original.pointer("/id").unwrap_or(&serde_json::Value::Null)
    );
    assert_ne!(
        exact
            .pointer("/attempt_id")
            .unwrap_or(&serde_json::Value::Null),
        original
            .pointer("/attempt_id")
            .unwrap_or(&serde_json::Value::Null)
    );
    assert_eq!(
        exact
            .pointer("/parent_run_id")
            .unwrap_or(&serde_json::Value::Null),
        original.pointer("/id").unwrap_or(&serde_json::Value::Null)
    );
    let latest = value(&rerun(&mut engine, &original, "latest", "latest"));
    assert_eq!(
        latest
            .pointer("/source_revision")
            .unwrap_or(&serde_json::Value::Null),
        &new_revision
    );
    let admitted = call(
        &mut engine,
        "POST",
        "/api/runs",
        json!({"project_id":"fixture","request_id":"old-exact","reference":"refs/heads/main","source_revision":revision}),
        Role::Operator,
    );
    assert_eq!(admitted.status, 201);
    assert_eq!(
        value(&admitted)
            .pointer("/source_revision")
            .unwrap_or(&serde_json::Value::Null),
        &revision
    );
    assert_eq!(call(&mut engine,"POST","/api/runs",json!({"project_id":"fixture","request_id":"ref-denied","reference":"refs/heads/other"}),Role::Operator).status,400);
    let dispatches = backend.lock().dispatches;
    assert_eq!(call(&mut engine,"POST","/api/runs",json!({"project_id":"fixture","request_id":"revision-denied","source_revision":"f".repeat(40)}),Role::Operator).status,400);
    assert_eq!(backend.lock().dispatches, dispatches);
}

#[test]
fn failed_retry_executes_only_failed_downstream_graph_and_restore_rejects_tampering() {
    let (_root, mut profile, _) = fixture();
    dag(&mut profile);
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    let original = value(&start(&mut engine, "original"));
    {
        let mut state = backend.lock();
        state.failed_nodes.insert("test".into());
        state.skipped_nodes.insert("publish".into());
    }
    finish(&mut engine, &backend);
    {
        let mut state = backend.lock();
        state.terminal = false;
        state.failed_nodes.clear();
        state.skipped_nodes.clear();
    }
    let retried = value(&rerun(&mut engine, &original, "failed-only", "failed"));
    assert_eq!(
        retried
            .pointer("/selected_nodes")
            .unwrap_or(&serde_json::Value::Null),
        &json!(["publish", "test"])
    );
    assert_eq!(
        retried
            .pointer("/nodes")
            .unwrap_or(&serde_json::Value::Null)
            .as_array()
            .map(|nodes| nodes
                .iter()
                .map(|n| n.pointer("/id").unwrap_or(&serde_json::Value::Null).clone())
                .collect::<Vec<_>>()),
        Some(vec![json!("publish"), json!("test")])
    );
    assert_eq!(
        retried
            .pointer("/edges")
            .unwrap_or(&serde_json::Value::Null),
        &json!([{"source":"test","target":"publish"}])
    );
    let plan = checked(backend.lock().plan.clone().ok_or("plan"));
    assert_eq!(plan.nodes.len(), 2);
    assert!(!plan.nodes.iter().any(|n| n.id.as_str() == "build"));
    drop(engine);
    let mut engine = open(profile.clone(), backend.clone());
    poll(&mut engine);
    assert_eq!(backend.lock().dispatches, 2);
    drop(engine);
    let connection = checked(rusqlite::Connection::open(
        profile.state_directory.join("service.sqlite"),
    ));
    let bytes: Vec<u8> = checked(connection.query_row(
        "SELECT state FROM service_state WHERE id=1",
        [],
        |row| row.get(0),
    ));
    let mut state: Value = checked(serde_json::from_slice(&bytes));
    *checked(
        state
            .pointer_mut(&format!("/runs/{}/selected_nodes", id(&retried)))
            .ok_or("selected nodes"),
    ) = json!(["build", "test", "publish"]);
    checked(connection.execute(
        "UPDATE service_state SET state=?1 WHERE id=1",
        [checked(serde_json::to_vec(&state))],
    ));
    drop(connection);
    assert!(try_open(profile, backend).is_err());
}

#[test]
fn missing_subset_provider_route_is_not_a_full_graph_fallback_or_success() {
    let (_root, mut profile, _) = fixture();
    dag(&mut profile);
    let backend = Arc::new(Mutex::new(Backend {
        registered_node_sets: Some([vec!["build".into(), "publish".into(), "test".into()]].into()),
        ..Backend::default()
    }));
    let mut engine = open(profile, backend.clone());
    enroll(&mut engine);
    let original = value(&start(&mut engine, "original"));
    backend.lock().failed_nodes.insert("test".into());
    backend.lock().skipped_nodes.insert("publish".into());
    finish(&mut engine, &backend);
    let retried = value(&rerun(&mut engine, &original, "missing-route", "failed"));
    assert_eq!(
        retried
            .pointer("/outcome")
            .unwrap_or(&serde_json::Value::Null),
        "Unknown"
    );
    assert_eq!(
        retried
            .pointer("/error_code")
            .unwrap_or(&serde_json::Value::Null),
        "provider_rejected"
    );
    assert_eq!(
        retried
            .pointer("/phase")
            .unwrap_or(&serde_json::Value::Null),
        "unknown"
    );
    assert_eq!(backend.lock().dispatches, 2);
    let plan = checked(backend.lock().plan.clone().ok_or("plan"));
    assert_eq!(plan.nodes.len(), 2);
    assert_eq!(
        rerun(&mut engine, &retried, "unsafe-again", "failed").status,
        409
    );
}

#[test]
fn logs_search_across_chunk_boundary_and_download_exact_bytes_with_metadata() {
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    let run = value(&start(&mut engine, "logs"));
    let path = log_path(&run);
    assert_eq!(
        call(
            &mut engine,
            "POST",
            &path,
            log_upload(&run, "stdout", 0, b"first needle", false),
            Role::Worker
        )
        .status,
        201
    );
    assert_eq!(
        call(
            &mut engine,
            "POST",
            &path,
            log_upload(&run, "stdout", 12, b"-cross\nlast", true),
            Role::Worker
        )
        .status,
        201
    );
    let search = value(&get(
        &mut engine,
        &format!("{path}?node_id=job&stream=stdout&cursor=0&max_bytes=64&q=needle-cross"),
        Role::Viewer,
    ));
    assert_eq!(
        search
            .pointer("/cursor")
            .unwrap_or(&serde_json::Value::Null),
        6
    );
    assert_eq!(
        search
            .pointer("/matches")
            .unwrap_or(&serde_json::Value::Null),
        &json!([{"cursor":6,"bytes":12}])
    );
    let metadata = value(&get(
        &mut engine,
        &format!("{path}/download?node_id=job&stream=stdout&metadata=true"),
        Role::Viewer,
    ));
    assert_eq!(
        metadata
            .pointer("/truncated")
            .unwrap_or(&serde_json::Value::Null),
        true
    );
    assert_eq!(
        metadata
            .pointer("/retained")
            .unwrap_or(&serde_json::Value::Null),
        true
    );
    assert_eq!(
        metadata
            .pointer("/bytes")
            .unwrap_or(&serde_json::Value::Null),
        23
    );
    let download = get(
        &mut engine,
        &format!("{path}/download?node_id=job&stream=stdout"),
        Role::Viewer,
    );
    assert_eq!(download.body, b"first needle-cross\nlast");
    assert_eq!(
        get(
            &mut engine,
            &format!("{path}/download?node_id=unknown&stream=stdout"),
            Role::Viewer
        )
        .status,
        400
    );
    assert_eq!(
        get(
            &mut engine,
            &format!("{path}/download?node_id=job&stream=diagnostic"),
            Role::Viewer
        )
        .status,
        400
    );
    drop(engine);
    let mut engine = open(profile, backend);
    assert_eq!(
        get(
            &mut engine,
            &format!("{path}/download?node_id=job&stream=stdout"),
            Role::Viewer
        )
        .body,
        download.body
    );
}

#[test]
fn historical_runs_restore_without_invented_timing_and_profile_serialization_is_unchanged() {
    let (_root, profile, _) = fixture();
    assert!(
        checked(serde_json::to_value(&profile))
            .pointer("/projects/0")
            .unwrap_or(&serde_json::Value::Null)
            .get("admitted_references")
            .is_none()
    );
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    let run = value(&start(&mut engine, "historical"));
    drop(engine);
    let connection = checked(rusqlite::Connection::open(
        profile.state_directory.join("service.sqlite"),
    ));
    let bytes: Vec<u8> = checked(connection.query_row(
        "SELECT state FROM service_state WHERE id=1",
        [],
        |row| row.get(0),
    ));
    let mut state: Value = checked(serde_json::from_slice(&bytes));
    let record = checked(
        checked(
            state
                .pointer_mut(&format!("/runs/{}", id(&run)))
                .ok_or("run"),
        )
        .as_object_mut()
        .ok_or("record"),
    );
    for field in [
        "created_at_ms",
        "started_at_ms",
        "completed_at_ms",
        "parent_run_id",
        "selected_nodes",
        "node_timing",
        "reports",
        "event_recorded_at_ms",
    ] {
        record.remove(field);
    }
    let state_object = checked(state.as_object_mut().ok_or("state"));
    for field in ["audit", "automation", "access_policy_identity"] {
        state_object.remove(field);
    }
    checked(connection.execute(
        "UPDATE service_state SET state=?1 WHERE id=1",
        [checked(serde_json::to_vec(&state))],
    ));
    drop(connection);
    let mut engine = open(profile, backend.clone());
    finish(&mut engine, &backend);
    let restored = value(&get(
        &mut engine,
        &format!("/api/runs/{}", id(&run)),
        Role::Viewer,
    ));
    for field in [
        "created_at_ms",
        "started_at_ms",
        "completed_at_ms",
        "duration_ms",
    ] {
        assert_eq!(restored.get(field).unwrap_or(&Value::Null), &Value::Null);
    }
    let overview = value(&get(&mut engine, "/api/overview", Role::Viewer));
    assert_eq!(
        overview
            .pointer("/totals/succeeded")
            .unwrap_or(&serde_json::Value::Null),
        1
    );
    assert_eq!(
        overview
            .pointer("/timing")
            .unwrap_or(&serde_json::Value::Null),
        &json!({"recorded":0,"total":1})
    );
    assert_eq!(backend.lock().dispatches, 1);
}
#[test]
fn overview_records_observed_timing_and_operations_warns_before_admission_exhaustion() {
    let (_root, mut profile, _) = fixture();
    profile.bounds.max_runs = 2;
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile, backend.clone());
    enroll(&mut engine);
    let first = value(&start(&mut engine, "timed"));
    assert!(
        first
            .pointer("/created_at_ms")
            .unwrap_or(&serde_json::Value::Null)
            .as_u64()
            .is_some()
    );
    assert!(
        first
            .pointer("/started_at_ms")
            .unwrap_or(&serde_json::Value::Null)
            .as_u64()
            .is_some()
    );
    assert_eq!(
        first
            .pointer("/completed_at_ms")
            .unwrap_or(&serde_json::Value::Null),
        &Value::Null
    );
    assert!(
        first
            .pointer("/nodes/0/started_at_ms")
            .unwrap_or(&serde_json::Value::Null)
            .as_u64()
            .is_some()
    );
    finish(&mut engine, &backend);
    let completed = value(&get(
        &mut engine,
        &format!("/api/runs/{}", id(&first)),
        Role::Viewer,
    ));
    let started = checked(
        completed
            .pointer("/started_at_ms")
            .unwrap_or(&serde_json::Value::Null)
            .as_u64()
            .ok_or("started"),
    );
    let end = checked(
        completed
            .pointer("/completed_at_ms")
            .unwrap_or(&serde_json::Value::Null)
            .as_u64()
            .ok_or("completed"),
    );
    assert_eq!(
        completed
            .pointer("/duration_ms")
            .unwrap_or(&serde_json::Value::Null),
        &json!(end - started)
    );
    let overview = value(&get(&mut engine, "/api/overview", Role::Viewer));
    assert_eq!(
        overview
            .pointer("/timing")
            .unwrap_or(&serde_json::Value::Null),
        &json!({"recorded":1,"total":1})
    );
    assert_eq!(
        overview
            .pointer("/duration_trend/0/run_id")
            .unwrap_or(&serde_json::Value::Null),
        first.pointer("/id").unwrap_or(&serde_json::Value::Null)
    );
    assert_eq!(
        overview
            .pointer("/totals/succeeded")
            .unwrap_or(&serde_json::Value::Null),
        1
    );
    assert_eq!(start(&mut engine, "at-limit").status, 201);
    checked(engine.configure_access_identity(&"a".repeat(64)));
    assert!(engine.configure_access_identity("not-an-identity").is_err());
    let operations = value(&get(&mut engine, "/api/operations", Role::Viewer));
    assert_eq!(
        operations
            .pointer("/access_policy_identity")
            .unwrap_or(&serde_json::Value::Null),
        &"a".repeat(64)
    );
    assert!(
        checked(
            operations
                .pointer("/warnings")
                .unwrap_or(&serde_json::Value::Null)
                .as_array()
                .ok_or("warnings")
        )
        .iter()
        .any(
            |w| w.pointer("/resource").unwrap_or(&serde_json::Value::Null) == "runs"
                && w.pointer("/used").unwrap_or(&serde_json::Value::Null) == 2
                && w.pointer("/maximum").unwrap_or(&serde_json::Value::Null) == 2
        )
    );
    assert_eq!(start(&mut engine, "over-limit").status, 409);
    assert_eq!(backend.lock().dispatches, 2);
    assert_eq!(
        get(
            &mut engine,
            &format!("/api/runs/{}", id(&first)),
            Role::Viewer
        )
        .status,
        200
    );
}
