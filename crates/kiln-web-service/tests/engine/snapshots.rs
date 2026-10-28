use super::*;

#[test]
fn dirty_candidates_download_and_exact_retry_preserve_provenance_and_survive_restart() {
    let (_root, profile, base) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    let clean = value(&start(&mut engine, "clean"));
    {
        let mut state = backend.lock();
        state.terminal = true;
        state.sequence += 1;
    }
    poll(&mut engine);
    backend.lock().terminal = false;
    let manifest = json!({"schema":"kiln.dirty-snapshot.v1","project_id":"fixture","reference":"refs/heads/main","base_revision":base,
        "files":[{"path":"source.txt","executable":false,"body_base64":base64::engine::general_purpose::STANDARD.encode(b"dirty submitted bytes\n")}]});
    let upload = call(
        &mut engine,
        "POST",
        "/api/snapshots",
        manifest.clone(),
        Role::Operator,
    );
    assert_eq!(upload.status, 201);
    let record = value(&upload);
    let snapshot_id = field(field(&record, "provenance"), "manifest_id")
        .as_str()
        .unwrap_or_else(|| unreachable!("{}", "snapshot id"));
    let revision = field(field(&record, "provenance"), "synthetic_revision")
        .as_str()
        .unwrap_or_else(|| unreachable!("{}", "snapshot revision"));
    let download = format!("/api/snapshots/fixture/{revision}");
    assert_eq!(get(&mut engine, &download, Role::Worker).status, 404);
    let request = json!({"project_id":"fixture","request_id":"dirty","reference":"refs/heads/main","source_revision":base,"snapshot_id":snapshot_id});
    let started = call(
        &mut engine,
        "POST",
        "/api/runs",
        request.clone(),
        Role::Operator,
    );
    assert_eq!(started.status, 201);
    let dirty = value(&started);
    assert_eq!(field(&dirty, "source_revision").as_str(), Some(revision));
    assert_ne!(
        field(&dirty, "source_revision").as_str(),
        Some(base.as_str())
    );
    assert_eq!(field(&dirty, "repository"), field(&clean, "repository"));
    assert_ne!(
        field(&dirty, "canonical_tree"),
        field(&clean, "canonical_tree")
    );
    assert_eq!(
        field(&dirty, "dirty_snapshot"),
        field(&record, "provenance")
    );
    assert_eq!(value(&get(&mut engine, &download, Role::Worker)), record);
    assert_eq!(get(&mut engine, &download, Role::Viewer).status, 403);
    {
        let mut state = backend.lock();
        state.terminal = true;
        state.sequence += 1;
    }
    poll(&mut engine);
    let id = field(&dirty, "id")
        .as_str()
        .unwrap_or_else(|| unreachable!("{}", "dirty run"));
    let rejected = call(
        &mut engine,
        "POST",
        &format!("/api/runs/{id}/rerun"),
        json!({"request_id":"dirty-latest","mode":"latest"}),
        Role::Operator,
    );
    assert_eq!(rejected.status, 409);
    backend.lock().terminal = false;
    let retry = call(
        &mut engine,
        "POST",
        &format!("/api/runs/{id}/rerun"),
        json!({"request_id":"dirty-retry","mode":"exact"}),
        Role::Operator,
    );
    assert_eq!(retry.status, 201);
    let retry = value(&retry);
    assert_ne!(field(&retry, "id"), field(&dirty, "id"));
    assert_eq!(
        field(&retry, "source_revision"),
        field(&dirty, "source_revision")
    );
    assert_eq!(
        field(&retry, "dirty_snapshot"),
        field(&dirty, "dirty_snapshot")
    );
    assert_eq!(field(&retry, "parent_run_id"), field(&dirty, "id"));
    drop(engine);
    let mut restarted = open(profile, backend);
    assert_eq!(value(&get(&mut restarted, &download, Role::Worker)), record);
    let replay = value(&call(
        &mut restarted,
        "POST",
        "/api/runs",
        request,
        Role::Operator,
    ));
    assert_eq!(field(&replay, "id"), field(&dirty, "id"));
}

#[test]
fn snapshot_api_rejects_unauthorized_unsafe_and_mismatched_inputs() {
    let (_root, profile, base) = fixture();
    let mut engine = open(profile, Arc::new(Mutex::new(Backend::default())));
    enroll(&mut engine);
    let manifest = json!({"schema":"kiln.dirty-snapshot.v1","project_id":"fixture","reference":"refs/heads/main","base_revision":base,
        "files":[{"path":"source.txt","executable":false,"body_base64":base64::engine::general_purpose::STANDARD.encode(b"explicit dirty\n")}]});
    assert_eq!(
        call(
            &mut engine,
            "POST",
            "/api/snapshots",
            manifest.clone(),
            Role::Viewer
        )
        .status,
        403
    );
    assert_eq!(
        call(
            &mut engine,
            "POST",
            "/api/snapshots",
            manifest.clone(),
            Role::Worker
        )
        .status,
        403
    );
    let mut unsafe_manifest = manifest.clone();
    *checked(
        unsafe_manifest
            .pointer_mut("/files/0/path")
            .ok_or("fixture file path"),
    ) = json!("../escape");
    assert_eq!(
        call(
            &mut engine,
            "POST",
            "/api/snapshots",
            unsafe_manifest,
            Role::Operator
        )
        .status,
        400
    );
    let mut stale = manifest.clone();
    set(&mut stale, "base_revision", json!("f".repeat(40)));
    assert_eq!(
        call(&mut engine, "POST", "/api/snapshots", stale, Role::Operator).status,
        400
    );
    let record = value(&call(
        &mut engine,
        "POST",
        "/api/snapshots",
        manifest,
        Role::Operator,
    ));
    let snapshot_id = field(field(&record, "provenance"), "manifest_id")
        .as_str()
        .unwrap_or_else(|| unreachable!("{}", "snapshot id"));
    let mismatch = json!({"project_id":"fixture","request_id":"mismatch","reference":"refs/heads/main","source_revision":"f".repeat(40),"snapshot_id":snapshot_id});
    assert_eq!(
        call(&mut engine, "POST", "/api/runs", mismatch, Role::Operator).status,
        400
    );
    let missing = json!({"project_id":"fixture","request_id":"missing","source_revision":base,"snapshot_id":"f".repeat(64)});
    assert_eq!(
        call(&mut engine, "POST", "/api/runs", missing, Role::Operator).status,
        404
    );
}

#[test]
fn durable_snapshot_tampering_rejects_restart_without_rewriting_retained_state() {
    for tamper in ["manifest", "base", "missing", "run"] {
        let (_root, profile, base) = fixture();
        let backend = Arc::new(Mutex::new(Backend::default()));
        let mut engine = open(profile.clone(), backend.clone());
        enroll(&mut engine);
        let manifest = json!({"schema":"kiln.dirty-snapshot.v1","project_id":"fixture","reference":"refs/heads/main","base_revision":base,
            "files":[{"path":"source.txt","executable":false,"body_base64":base64::engine::general_purpose::STANDARD.encode(b"retained dirty bytes\n")}]});
        let record = value(&call(
            &mut engine,
            "POST",
            "/api/snapshots",
            manifest,
            Role::Operator,
        ));
        let snapshot_id = field(field(&record, "provenance"), "manifest_id")
            .as_str()
            .unwrap_or_else(|| unreachable!("{}", "snapshot id"));
        let run = value(&call(
            &mut engine,
            "POST",
            "/api/runs",
            json!({"project_id":"fixture","request_id":"retained-dirty","reference":"refs/heads/main","source_revision":base,"snapshot_id":snapshot_id}),
            Role::Operator,
        ));
        let run_id = field(&run, "id")
            .as_str()
            .unwrap_or_else(|| unreachable!("{}", "dirty run id"));
        drop(engine);
        let database = checked(rusqlite::Connection::open(
            profile.state_directory.join("service.sqlite"),
        ));
        let bytes: Vec<u8> = checked(database.query_row(
            "SELECT state FROM service_state WHERE id=1",
            [],
            |row| row.get(0),
        ));
        let mut state: Value = checked(serde_json::from_slice(&bytes));
        match tamper {
            "manifest" => {
                *checked(
                    state
                        .pointer_mut(&format!(
                            "/snapshots/{snapshot_id}/manifest/files/0/body_base64"
                        ))
                        .ok_or("retained snapshot file"),
                ) = json!("Y29ycnVwdA==");
            }
            "base" => {
                *checked(
                    state
                        .pointer_mut(&format!(
                            "/snapshots/{snapshot_id}/provenance/base_revision"
                        ))
                        .ok_or("retained snapshot base"),
                ) = json!("f".repeat(40));
            }
            "missing" => set(&mut state, "snapshots", json!({})),
            "run" => {
                *checked(
                    state
                        .pointer_mut(&format!("/runs/{run_id}/dirty_snapshot/synthetic_revision"))
                        .ok_or("retained dirty run"),
                ) = json!("f".repeat(40));
            }
            _ => unreachable!("bounded tamper case"),
        }
        let corrupted = checked(serde_json::to_vec(&state));
        checked(database.execute("UPDATE service_state SET state=?1 WHERE id=1", [&corrupted]));
        assert!(
            try_open(profile, backend).is_err(),
            "corrupt snapshot case {tamper} must fail closed"
        );
        let retained: Vec<u8> = checked(database.query_row(
            "SELECT state FROM service_state WHERE id=1",
            [],
            |row| row.get(0),
        ));
        assert_eq!(
            retained, corrupted,
            "recovery must preserve unknown retained data"
        );
    }
}
