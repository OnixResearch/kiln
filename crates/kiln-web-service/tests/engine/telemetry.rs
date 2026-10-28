use super::*;

fn telemetry(run: &Value, sequence: u64, index: u32) -> Value {
    json!({"binding":{"run_id":field(run,"id"),"source_revision":field(run,"source_revision"),"attempt_id":field(run,"attempt_id"),"execution":field(run,"execution"),"node_id":"job"},"sequence":sequence,"step":{"step_index":index,"started_at_ms":1000,"completed_at_ms":1010,"duration_ms":9,"exit_code":0,"cpu_time_us":1234,"peak_memory_bytes":8192}})
}
fn path(run: &Value) -> String {
    format!(
        "/api/runs/{}/telemetry",
        checked(field(run, "id").as_str().ok_or("run id"))
    )
}

#[test]
fn measured_steps_survive_recovery_and_reject_stale_binding_replay_and_gaps() {
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    let run = value(&start(&mut engine, "telemetry"));
    let target = path(&run);
    let first = telemetry(&run, 1, 0);
    for role in [Role::Viewer, Role::Operator] {
        assert_eq!(
            call(&mut engine, "POST", &target, first.clone(), role).status,
            403
        );
    }
    for (pointer, replacement) in [
        ("/binding/run_id", json!("foreign-run")),
        ("/binding/source_revision", json!("foreign-source")),
        ("/binding/attempt_id", json!("foreign-attempt")),
        ("/binding/execution/native_id", json!("foreign-execution")),
        ("/binding/node_id", json!("foreign-node")),
        ("/sequence", json!(2)),
    ] {
        let mut invalid = first.clone();
        *checked(invalid.pointer_mut(pointer).ok_or("binding field")) = replacement;
        assert_eq!(
            call(&mut engine, "POST", &target, invalid, Role::Worker).status,
            409
        );
    }
    assert_eq!(
        call(&mut engine, "POST", &target, first.clone(), Role::Worker).status,
        201
    );
    assert_eq!(
        call(&mut engine, "POST", &target, first.clone(), Role::Worker).status,
        200
    );
    let mut conflicting = first.clone();
    *checked(
        conflicting
            .pointer_mut("/step/exit_code")
            .ok_or("exit code"),
    ) = json!(7);
    assert_eq!(
        call(&mut engine, "POST", &target, conflicting, Role::Worker).status,
        409
    );
    assert_eq!(
        call(
            &mut engine,
            "POST",
            &target,
            telemetry(&run, 2, 0),
            Role::Worker
        )
        .status,
        409
    );
    let mut second = telemetry(&run, 2, 1);
    *checked(second.pointer_mut("/step/cpu_time_us").ok_or("CPU time")) = Value::Null;
    *checked(
        second
            .pointer_mut("/step/peak_memory_bytes")
            .ok_or("peak memory"),
    ) = Value::Null;
    *checked(second.pointer_mut("/step/exit_code").ok_or("exit code")) = json!(7);
    assert_eq!(
        call(&mut engine, "POST", &target, second, Role::Worker).status,
        201
    );
    drop(engine);
    let mut engine = open(profile, backend.clone());
    let retained = value(&get(&mut engine, &target, Role::Viewer));
    assert_eq!(
        checked(
            retained
                .pointer("/steps/0/observation/step/duration_ms")
                .ok_or("first duration")
        ),
        9
    );
    assert_eq!(
        checked(
            retained
                .pointer("/steps/0/observation/step/cpu_time_us")
                .ok_or("first CPU time")
        ),
        1234
    );
    assert_eq!(
        checked(
            retained
                .pointer("/steps/1/observation/step/peak_memory_bytes")
                .ok_or("second peak memory")
        ),
        &Value::Null
    );
    assert_eq!(
        checked(
            retained
                .pointer("/steps/1/observation/step/exit_code")
                .ok_or("second exit code")
        ),
        7
    );
    {
        let mut state = backend.lock();
        state.terminal = true;
        state.sequence += 1;
    }
    poll(&mut engine);
    assert_eq!(
        call(
            &mut engine,
            "POST",
            &target,
            telemetry(&run, 3, 2),
            Role::Worker
        )
        .status,
        409
    );
    assert_eq!(
        call(&mut engine, "POST", &target, first, Role::Worker).status,
        200
    );
}

#[test]
fn malformed_and_unbounded_telemetry_never_become_measurements() {
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile, backend);
    enroll(&mut engine);
    let run = value(&start(&mut engine, "invalid-telemetry"));
    let target = path(&run);
    let mut invalid = telemetry(&run, 1, 0);
    set(&mut invalid, "extra_identity", json!("not admitted"));
    assert_eq!(
        call(&mut engine, "POST", &target, invalid, Role::Worker).status,
        400
    );
    let mut invalid = telemetry(&run, 1, 0);
    *checked(invalid.pointer_mut("/step/duration_ms").ok_or("duration")) =
        json!(kiln_web_service::telemetry::MAX_STEP_DURATION_MS + 1);
    assert_eq!(
        call(&mut engine, "POST", &target, invalid, Role::Worker).status,
        409
    );
    let retained = value(&get(&mut engine, &target, Role::Viewer));
    assert_eq!(field(&retained, "steps"), &json!([]));
    let run = value(&get(
        &mut engine,
        &format!(
            "/api/runs/{}",
            checked(field(&run, "id").as_str().ok_or("run id"))
        ),
        Role::Viewer,
    ));
    assert_eq!(field(&run, "step_telemetry"), &json!([]));
}

#[test]
fn durable_replay_rejects_changed_measurements_and_stale_exact_bindings() {
    for pointer in [
        "/observation/binding/source_revision",
        "/observation/binding/attempt_id",
        "/observation/binding/execution/native_id",
        "/observation/binding/node_id",
        "/observation/sequence",
        "/observation/step/duration_ms",
        "/id",
    ] {
        let (_root, profile, _) = fixture();
        let backend = Arc::new(Mutex::new(Backend::default()));
        let mut engine = open(profile.clone(), backend.clone());
        enroll(&mut engine);
        let run = value(&start(&mut engine, "replay-tamper"));
        assert_eq!(
            call(
                &mut engine,
                "POST",
                &path(&run),
                telemetry(&run, 1, 0),
                Role::Worker
            )
            .status,
            201
        );
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
        let record = checked(
            state
                .pointer_mut(&format!(
                    "/runs/{}/step_telemetry/0",
                    checked(field(&run, "id").as_str().ok_or("run id"))
                ))
                .ok_or("stored record"),
        );
        let slot = checked(record.pointer_mut(pointer).ok_or("telemetry field"));
        *slot = if slot.is_number() {
            json!(42)
        } else {
            json!("foreign")
        };
        checked(database.execute(
            "UPDATE service_state SET state=?1 WHERE id=1",
            [checked(serde_json::to_vec(&state))],
        ));
        drop(database);
        assert!(
            try_open(profile, backend).is_err(),
            "admitted tampered {pointer}"
        );
    }
}
