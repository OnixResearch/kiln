use super::*;
fn report_body(run: &Value, name: &str, kind: &str, bytes: &[u8]) -> Value {
    json!({"run_id":run.pointer("/id").unwrap_or(&serde_json::Value::Null),"source_revision":run.pointer("/source_revision").unwrap_or(&serde_json::Value::Null),"attempt_id":run.pointer("/attempt_id").unwrap_or(&serde_json::Value::Null),"execution":run.pointer("/execution").unwrap_or(&serde_json::Value::Null),"node_id":"job","kind":kind,"name":name,"body_base64":base64::engine::general_purpose::STANDARD.encode(bytes),"blake3":blake3::hash(bytes).to_hex().as_str()})
}
fn report_path(run: &Value) -> String {
    format!(
        "/api/runs/{}/reports",
        checked(
            run.pointer("/id")
                .unwrap_or(&serde_json::Value::Null)
                .as_str()
                .ok_or("run id")
        )
    )
}
fn finish(engine: &mut Engine, backend: &Arc<Mutex<Backend>>) {
    {
        let mut backend = backend.lock();
        backend.terminal = true;
        backend.sequence += 1;
    }
    poll(engine);
}
#[test]
fn reports_preserve_raw_bytes_bind_exact_attempt_and_compare_real_test_coverage_history() {
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    let first = value(&start(&mut engine, "first"));
    let path = report_path(&first);
    let passed=br#"<testsuite name="unit" tests="1" failures="0"><testcase name="stable" classname="Example" time="0.1"/></testsuite>"#;
    let upload = report_body(&first, "reports/unit.xml", "junit", passed);
    assert_eq!(
        call(&mut engine, "POST", &path, upload.clone(), Role::Operator).status,
        403
    );
    assert_eq!(
        call(&mut engine, "POST", &path, upload.clone(), Role::Viewer).status,
        403
    );
    for (field, replacement) in [
        ("run_id", json!("another-run")),
        ("attempt_id", json!("old-attempt")),
        ("source_revision", json!("b".repeat(40))),
        ("node_id", json!("other-node")),
    ] {
        let mut stale = upload.clone();
        set(&mut stale, field, replacement);
        assert_eq!(
            call(&mut engine, "POST", &path, stale, Role::Worker).status,
            409
        );
    }
    let mut execution = upload.clone();
    *checked(
        execution
            .pointer_mut("/execution/native_id")
            .ok_or("native execution"),
    ) = json!("foreign-execution");
    assert_eq!(
        call(&mut engine, "POST", &path, execution, Role::Worker).status,
        409
    );
    let mut malformed = upload.clone();
    set(&mut malformed, "body_base64", json!("broken"));
    assert_eq!(
        call(&mut engine, "POST", &path, malformed, Role::Worker).status,
        400
    );
    let response = call(&mut engine, "POST", &path, upload.clone(), Role::Worker);
    assert_eq!(response.status, 201);
    let first_record = value(&response);
    assert_eq!(
        first_record
            .pointer("/report/tests/passed")
            .unwrap_or(&serde_json::Value::Null),
        1
    );
    assert_eq!(
        first_record
            .pointer("/report/tests/failed")
            .unwrap_or(&serde_json::Value::Null),
        0
    );
    let raw_path = checked(
        first_record
            .pointer("/artifact/download_path")
            .unwrap_or(&serde_json::Value::Null)
            .as_str()
            .ok_or("download path"),
    )
    .to_owned();
    assert_eq!(get(&mut engine, &raw_path, Role::Viewer).body, passed);
    assert_eq!(
        call(&mut engine, "POST", &path, upload, Role::Worker).body,
        response.body
    );
    let coverage = b"SF:src/lib.rs\nDA:1,1\nDA:2,0\nLF:2\nLH:1\nend_of_record\n";
    assert_eq!(
        call(
            &mut engine,
            "POST",
            &path,
            report_body(&first, "coverage/lcov.info", "lcov", coverage),
            Role::Worker
        )
        .status,
        201
    );
    finish(&mut engine, &backend);
    {
        let mut backend = backend.lock();
        backend.terminal = false;
        backend.sequence += 1;
    }
    let second = value(&start(&mut engine, "second"));
    let second_path = report_path(&second);
    let failed=br#"<testsuite name="unit" tests="1" failures="1"><testcase name="stable" classname="Example"><failure message="expected A">observed B</failure></testcase></testsuite>"#;
    assert_eq!(
        call(
            &mut engine,
            "POST",
            &second_path,
            report_body(&second, "reports/unit.xml", "junit", failed),
            Role::Worker
        )
        .status,
        201
    );
    let better = b"SF:src/lib.rs\nDA:1,1\nDA:2,3\nLF:2\nLH:2\nend_of_record\n";
    assert_eq!(
        call(
            &mut engine,
            "POST",
            &second_path,
            report_body(&second, "coverage/lcov.info", "lcov", better),
            Role::Worker
        )
        .status,
        201
    );
    let reports = value(&get(&mut engine, &second_path, Role::Viewer));
    assert_eq!(
        reports
            .pointer("/summary/tests/failed")
            .unwrap_or(&serde_json::Value::Null),
        1
    );
    assert_eq!(
        reports
            .pointer("/summary/coverage/0/coverage/line_rate")
            .unwrap_or(&serde_json::Value::Null),
        1.0
    );
    assert_eq!(
        reports
            .pointer("/flaky_history/0/passed")
            .unwrap_or(&serde_json::Value::Null),
        1
    );
    assert_eq!(
        reports
            .pointer("/flaky_history/0/failed")
            .unwrap_or(&serde_json::Value::Null),
        1
    );
    assert_eq!(
        reports
            .pointer("/reports/0/report/tests/cases/0/details")
            .unwrap_or(&serde_json::Value::Null),
        "observed B"
    );
    let compare = format!(
        "/api/runs/{}/compare?other={}",
        checked(
            second
                .pointer("/id")
                .unwrap_or(&serde_json::Value::Null)
                .as_str()
                .ok_or("run id")
        ),
        checked(
            first
                .pointer("/id")
                .unwrap_or(&serde_json::Value::Null)
                .as_str()
                .ok_or("run id")
        )
    );
    let comparison = value(&get(&mut engine, &compare, Role::Viewer));
    assert_eq!(
        comparison
            .pointer("/test_changes/0/current")
            .unwrap_or(&serde_json::Value::Null),
        "failed"
    );
    assert_eq!(
        comparison
            .pointer("/test_changes/0/other")
            .unwrap_or(&serde_json::Value::Null),
        "passed"
    );
    assert_eq!(
        comparison
            .pointer("/reports/other/coverage/0/coverage/line_rate")
            .unwrap_or(&serde_json::Value::Null),
        0.5
    );
    assert_eq!(
        get(
            &mut engine,
            "/api/runs/absent/compare?other=missing",
            Role::Viewer
        )
        .status,
        404
    );
    assert_eq!(
        get(
            &mut engine,
            &format!(
                "/api/runs/{}/compare?other=missing",
                checked(
                    first
                        .pointer("/id")
                        .unwrap_or(&serde_json::Value::Null)
                        .as_str()
                        .ok_or("run id")
                )
            ),
            Role::Viewer
        )
        .status,
        404
    );
    drop(engine);
    let mut engine = open(profile, backend);
    assert_eq!(get(&mut engine, &raw_path, Role::Viewer).body, passed);
    assert_eq!(
        value(&get(&mut engine, &second_path, Role::Viewer)),
        reports
    );
}
#[test]
fn unavailable_reports_and_recorded_review_never_invent_test_results_or_gate_authority() {
    let (_root, profile, _) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile, backend.clone());
    enroll(&mut engine);
    let run = value(&start(&mut engine, "evidence"));
    let path = report_path(&run);
    let absent = json!({"run_id":run.pointer("/id").unwrap_or(&serde_json::Value::Null),"source_revision":run.pointer("/source_revision").unwrap_or(&serde_json::Value::Null),"attempt_id":run.pointer("/attempt_id").unwrap_or(&serde_json::Value::Null),"execution":run.pointer("/execution").unwrap_or(&serde_json::Value::Null),"node_id":"job","kind":"junit","name":"missing.xml","unavailable":"missing"});
    assert_eq!(
        call(&mut engine, "POST", &path, absent, Role::Worker).status,
        201
    );
    let reports = value(&get(&mut engine, &path, Role::Viewer));
    assert_eq!(
        reports
            .pointer("/summary/tests")
            .unwrap_or(&serde_json::Value::Null),
        &Value::Null
    );
    assert_eq!(
        reports
            .pointer("/reports/0/artifact")
            .unwrap_or(&serde_json::Value::Null),
        &Value::Null
    );
    assert_eq!(
        reports
            .pointer("/reports/0/unavailable")
            .unwrap_or(&serde_json::Value::Null),
        "missing"
    );
    let review=br#"{"verdict":"approve","reviewer":"recorded-checker","links":{"build":"https://example.invalid/review/1"}}"#;
    let response = call(
        &mut engine,
        "POST",
        &path,
        report_body(&run, "review.json", "review", review),
        Role::Worker,
    );
    assert_eq!(response.status, 201);
    let review = value(&response);
    assert_eq!(
        review
            .pointer("/report/evidence/accepted_gate")
            .unwrap_or(&serde_json::Value::Null),
        false
    );
    let evidence = value(&get(
        &mut engine,
        &format!(
            "/api/runs/{}/evidence",
            checked(
                run.pointer("/id")
                    .unwrap_or(&serde_json::Value::Null)
                    .as_str()
                    .ok_or("run id")
            )
        ),
        Role::Viewer,
    ));
    assert_eq!(
        evidence
            .pointer("/reports/0/id")
            .unwrap_or(&serde_json::Value::Null),
        review.pointer("/id").unwrap_or(&serde_json::Value::Null)
    );
    assert_eq!(
        evidence
            .pointer("/execution/attempt_id")
            .unwrap_or(&serde_json::Value::Null),
        run.pointer("/attempt_id")
            .unwrap_or(&serde_json::Value::Null)
    );
    let stale = br#"{"attempt_id":"another-attempt","verdict":"approve"}"#;
    assert_eq!(
        call(
            &mut engine,
            "POST",
            &path,
            report_body(&run, "stale.json", "gate", stale),
            Role::Worker
        )
        .status,
        400
    );
    let entity = br#"<!DOCTYPE x [<!ENTITY x SYSTEM "file:///etc/passwd">]><testsuite/>"#;
    assert_eq!(
        call(
            &mut engine,
            "POST",
            &path,
            report_body(&run, "unsafe.xml", "junit", entity),
            Role::Worker
        )
        .status,
        400
    );
    finish(&mut engine, &backend);
    assert_eq!(
        call(
            &mut engine,
            "POST",
            &path,
            report_body(&run, "late.xml", "junit", b"<testsuite/>"),
            Role::Worker
        )
        .status,
        409
    );
}
