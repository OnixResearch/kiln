use base64::Engine as _;
use kiln_web_service::reports::{self, CaseStatus, ReportBinding, ReportKind, ReportUpload};
use serde_json::json;

const MIXED: &[u8] = include_bytes!("fixtures/reports/junit-mixed.xml");
const PASS: &[u8] = include_bytes!("fixtures/reports/junit-pass.xml");
const COBERTURA: &[u8] = include_bytes!("fixtures/reports/cobertura.xml");
const LCOV: &[u8] = include_bytes!("fixtures/reports/lcov.info");

fn binding() -> ReportBinding {
    ReportBinding {
        run_id: "run-1".into(),
        source_revision: "0123456789abcdef".into(),
        attempt_id: "attempt-1".into(),
        execution: json!({"provider_namespace":"lattice","native_id":"execution-1"}),
        node_id: "test".into(),
    }
}
fn upload(bytes: &[u8], kind: ReportKind) -> ReportUpload {
    ReportUpload {
        binding: binding(),
        kind,
        name: "results/report.xml".into(),
        body_base64: Some(base64::engine::general_purpose::STANDARD.encode(bytes)),
        blake3: Some(blake3::hash(bytes).to_hex().to_string()),
        unavailable: None,
    }
}

#[test]
fn junit_preserves_real_outcomes_failure_text_skips_and_source_identity() {
    let report = reports::parse(ReportKind::Junit, MIXED, &binding())
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert_eq!(
        report.source_blake3,
        blake3::hash(MIXED).to_hex().to_string()
    );
    assert_eq!(report.source_bytes, MIXED.len());
    let tests = report
        .tests
        .unwrap_or_else(|| unreachable!("test report absent"));
    assert_eq!(
        (
            tests.total,
            tests.passed,
            tests.failed,
            tests.errors,
            tests.skipped
        ),
        (4, 1, 1, 1, 1)
    );
    let [passed, failed, errored, skipped] = tests.cases.as_slice() else {
        unreachable!("mixed report case count differs")
    };
    assert_eq!(failed.status, CaseStatus::Failed);
    assert_eq!(failed.message.as_deref(), Some("expected 403 & got 200"));
    assert_eq!(
        failed.details.as_deref(),
        Some("assertion failed\nexpected: 403\nactual: 200")
    );
    assert_eq!(errored.status, CaseStatus::Error);
    assert_eq!(skipped.status, CaseStatus::Skipped);
    assert_eq!(passed.file.as_deref(), Some("src/api.rs"));
    assert_eq!(passed.line, Some(7));
    let pass = reports::parse(ReportKind::Junit, PASS, &binding())
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
        .tests
        .unwrap_or_else(|| unreachable!("test report absent"));
    assert_eq!(
        (
            pass.total,
            pass.passed,
            pass.failed,
            pass.errors,
            pass.skipped
        ),
        (2, 2, 0, 0, 0)
    );
    assert_eq!(pass.duration_seconds, Some(0.4));
}

#[test]
fn junit_rejects_malformed_entities_ambiguous_cases_and_invalid_metrics() {
    for bytes in [
        include_bytes!("fixtures/reports/junit-xxe.xml").as_slice(),
        include_bytes!("fixtures/reports/junit-malformed.xml").as_slice(),
        b"<testsuite><testcase name='x'/><testcase name='x'/></testsuite>",
        b"<testsuite tests='2'><testcase name='x'/></testsuite>",
        b"<testsuite><testcase name='x'><failure/><skipped/></testcase></testsuite>",
        b"<testsuite><testcase name='x' time='NaN'/></testsuite>",
        b"<testsuite><testcase name='x' time='inf'/></testsuite>",
        b"<testsuite><testcase name='x' time='-1'/></testsuite>",
        b"<testsuite><testcase name='x' name='y'/></testsuite>",
        b"<testsuite><testcase name='x' status='failed'/></testsuite>",
        b"<testsuite><testcase name='x'><unknown/></testcase></testsuite>",
        b"<testsuite><testcase name='&unknown;'/></testsuite>",
    ] {
        assert!(reports::parse(ReportKind::Junit, bytes, &binding()).is_err());
    }
}

#[test]
fn report_bounds_reject_bytes_depth_fields_and_case_count() {
    assert!(
        reports::parse(
            ReportKind::Junit,
            &vec![b'x'; reports::MAX_REPORT_BYTES + 1],
            &binding()
        )
        .is_err()
    );
    let deep = format!(
        "<testsuite>{}<testcase name='x'/>{}</testsuite>",
        "<testsuite>".repeat(65),
        "</testsuite>".repeat(65)
    );
    assert!(reports::parse(ReportKind::Junit, deep.as_bytes(), &binding()).is_err());
    let field = format!(
        "<testsuite><testcase name='{}'/></testsuite>",
        "x".repeat(65_537)
    );
    assert!(reports::parse(ReportKind::Junit, field.as_bytes(), &binding()).is_err());
    let mut cases = String::from("<testsuite>");
    for i in 0..=reports::MAX_ITEMS {
        cases.push_str(&format!("<testcase name='{i}'/>"));
    }
    cases.push_str("</testsuite>");
    assert!(reports::parse(ReportKind::Junit, cases.as_bytes(), &binding()).is_err());
}

#[test]
fn cobertura_and_lcov_preserve_equivalent_real_line_and_branch_counts() {
    let xml = reports::parse(ReportKind::Cobertura, COBERTURA, &binding())
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
        .coverage
        .unwrap_or_else(|| unreachable!("coverage report absent"));
    let lcov = reports::parse(ReportKind::Lcov, LCOV, &binding())
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
        .coverage
        .unwrap_or_else(|| unreachable!("coverage report absent"));
    for coverage in [&xml, &lcov] {
        assert_eq!((coverage.lines_found, coverage.lines_hit), (3, 2));
        assert_eq!(
            (coverage.branches_found, coverage.branches_hit),
            (Some(2), Some(1))
        );
        assert_eq!(coverage.line_rate, Some(2.0 / 3.0));
        assert_eq!(coverage.branch_rate, Some(0.5));
        let [file] = coverage.files.as_slice() else {
            unreachable!("coverage file count differs")
        };
        let [_, missed, _] = file.lines.as_slice() else {
            unreachable!("coverage line count differs")
        };
        assert_eq!(file.path, "src/api.rs");
        assert_eq!(missed.number, 8);
        assert_eq!(missed.hits, 0);
    }
    assert_eq!(xml.files, lcov.files);
}

#[test]
fn coverage_rejects_false_totals_duplicate_lines_nonfinite_and_traversal() {
    for text in [
        String::from_utf8(COBERTURA.to_vec())
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
            .replace("lines-covered=\"2\"", "lines-covered=\"3\""),
        String::from_utf8(COBERTURA.to_vec())
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
            .replace("line-rate=\"0.666667\"", "line-rate=\"NaN\""),
        String::from_utf8(COBERTURA.to_vec())
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
            .replace("src/api.rs", "../secret"),
        String::from_utf8(COBERTURA.to_vec())
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
            .replace("50% (1/2)", "150% (3/2)"),
        String::from_utf8(COBERTURA.to_vec())
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
            .replace("number=\"9\"", "number=\"7\""),
    ] {
        assert!(reports::parse(ReportKind::Cobertura, text.as_bytes(), &binding()).is_err());
    }
    for text in [
        "SF:src/a.rs\nDA:1,0\nLF:1\nLH:1\nend_of_record\n",
        "SF:src/a.rs\nDA:1,0\nDA:1,1\nend_of_record\n",
        "SF:src/a.rs\nBRDA:1,0,0,NaN\nend_of_record\n",
        "SF:src/a.rs\nFNF:1\nFNH:2\nend_of_record\n",
        "SF:src/a.rs\nFN:1,f\nFN:2,f\nend_of_record\n",
        "SF:src/a.rs\nDA:1,1\n",
        "SF:../outside\nDA:1,1\nend_of_record\n",
    ] {
        assert!(reports::parse(ReportKind::Lcov, text.as_bytes(), &binding()).is_err());
    }
    let unknown = reports::parse(
        ReportKind::Lcov,
        b"SF:src/empty.rs\nLF:0\nLH:0\nend_of_record\n",
        &binding(),
    )
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
    .coverage
    .unwrap_or_else(|| unreachable!("coverage report absent"));
    assert_eq!(unknown.line_rate, None);
    assert_eq!(unknown.branch_rate, None);
    assert_eq!(unknown.branches_found, None);
    assert_eq!(unknown.branches_hit, None);
    assert!(reports::parse(ReportKind::Cobertura, b"<coverage/>", &binding()).is_err());
    let no_branches = reports::parse(ReportKind::Cobertura,b"<coverage><packages><package><classes><class filename='src/a.rs'><lines><line number='1' hits='0'/></lines></class></classes></package></packages></coverage>",&binding()).unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")).coverage.unwrap_or_else(|| unreachable!("coverage report absent"));
    assert_eq!(no_branches.lines_hit, 0);
    assert_eq!(no_branches.line_rate, Some(0.0));
    assert_eq!(no_branches.branches_found, None);
}

#[test]
fn upload_checks_all_binding_dimensions_hash_and_absence_without_inventing_tests() {
    let original = upload(MIXED, ReportKind::Junit);
    let wire = serde_json::to_value(&original)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let decoded: ReportUpload = serde_json::from_value(wire.clone())
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let (report, retained_bytes) = reports::validate_upload(&decoded, &binding())
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
        .unwrap_or_else(|| unreachable!("retained report absent"));
    assert_eq!(retained_bytes, MIXED);
    assert_eq!(report.source_bytes, retained_bytes.len());
    assert_eq!(
        report
            .tests
            .unwrap_or_else(|| unreachable!("test report absent"))
            .failed,
        1
    );
    let mut unknown = wire.clone();
    unknown
        .as_object_mut()
        .unwrap_or_else(|| unreachable!("upload object absent"))
        .insert("foreign_run_id".into(), json!("other-run"));
    assert!(serde_json::from_value::<ReportUpload>(unknown).is_err());
    let encoded = serde_json::to_string(&wire)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let duplicate = format!(
        "{{\"run_id\":\"other-run\",{}",
        encoded
            .strip_prefix('{')
            .unwrap_or_else(|| unreachable!("JSON object absent"))
    );
    assert!(serde_json::from_str::<ReportUpload>(&duplicate).is_err());
    for key in [
        "run_id",
        "source_revision",
        "attempt_id",
        "node_id",
        "execution",
    ] {
        let mut changed = wire.clone();
        *changed
            .get_mut(key)
            .unwrap_or_else(|| unreachable!("binding field absent")) = if key == "execution" {
            json!({"native_id":"other"})
        } else {
            json!("other")
        };
        let changed: ReportUpload = serde_json::from_value(changed)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        assert!(reports::validate_upload(&changed, &binding()).is_err());
    }
    let mut changed = original.clone();
    changed.blake3 = Some("0".repeat(64));
    assert!(reports::validate_upload(&changed, &binding()).is_err());
    changed = original.clone();
    changed.name = "../report.xml".into();
    assert!(reports::validate_upload(&changed, &binding()).is_err());
    let mut missing = original;
    missing.body_base64 = None;
    missing.blake3 = None;
    missing.unavailable = Some("missing".into());
    assert_eq!(
        reports::validate_upload(&missing, &binding())
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
        None
    );
    missing.body_base64 = Some(String::new());
    assert!(reports::validate_upload(&missing, &binding()).is_err());
}

#[test]
fn native_gate_review_provenance_retain_verdicts_identities_nonclaims_without_acceptance() {
    let native: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/reports/gate.json"))
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    for kind in [ReportKind::Gate, ReportKind::Review, ReportKind::Provenance] {
        let report = reports::parse(
            kind,
            include_bytes!("fixtures/reports/gate.json"),
            &binding(),
        )
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        assert_eq!(report.tests, None);
        let evidence = report
            .evidence
            .unwrap_or_else(|| unreachable!("evidence report absent"));
        assert_eq!(evidence.document, native);
        assert!(!evidence.accepted_gate);
        assert!(
            evidence
                .fields
                .iter()
                .any(|f| f.path == "/result/verdict" && f.value == "block")
        );
        assert!(evidence.fields.iter().any(|f| {
            f.path == "/non_claims/0"
                && &f.value
                    == native
                        .pointer("/non_claims/0")
                        .unwrap_or(&serde_json::Value::Null)
        }));
        assert!(evidence.links.iter().any(|f| {
            f.path == "/identity"
                && &f.value
                    == native
                        .pointer("/identity")
                        .unwrap_or(&serde_json::Value::Null)
        }));
    }
    let envelope = json!({"binding":binding(),"document":native});
    assert!(
        reports::parse(
            ReportKind::Gate,
            &serde_json::to_vec(&envelope)
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
            &binding()
        )
        .is_ok()
    );
    let mut stale = envelope;
    *stale
        .pointer_mut("/binding/attempt_id")
        .unwrap_or_else(|| unreachable!("binding attempt absent")) = json!("other-attempt");
    assert!(
        reports::parse(
            ReportKind::Gate,
            &serde_json::to_vec(&stale)
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
            &binding()
        )
        .is_err()
    );
    let stale_native = json!({"attempt_id":"other-attempt","verdict":"pass"});
    assert!(
        reports::parse(
            ReportKind::Review,
            &serde_json::to_vec(&stale_native)
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
            &binding()
        )
        .is_err()
    );
    assert!(
        reports::parse(
            ReportKind::Provenance,
            b"{\"verdict\":\"block\",\"verdict\":\"pass\"}",
            &binding()
        )
        .is_err()
    );
    assert!(reports::parse(ReportKind::Provenance, b"{\"value\":1e999}", &binding()).is_err());
    let containers = serde_json::to_vec(&json!({"items": vec![json!({}); reports::MAX_ITEMS]}))
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert!(reports::parse(ReportKind::Provenance, &containers, &binding()).is_err());
}

#[test]
fn declaration_paths_reject_escapes_and_empty_reports_leave_old_step_serialization_unchanged() {
    for path in ["../x", "/x", "a/../x", "a/./x", "a//x", "a\\x", "a/\nx"] {
        assert!(!reports::safe_relative_path(std::path::Path::new(path)));
    }
    assert!(reports::safe_relative_path(std::path::Path::new(
        "build/test-results.xml"
    )));
    let old =
        json!({"node_id":"test","program":"/bin/true","argv":[],"environment":{},"artifacts":[]});
    let step: kiln_web_service::worker::Step = serde_json::from_value(old.clone())
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert_eq!(
        serde_json::to_value(step)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
        old
    );
}
