use std::path::Path;

use kiln_web_service::engine::{ApiResponse, Role};
use kiln_web_service::reviews::{Browser, Profile};
use serde_json::{Value, json};

fn receipt(decision: &str) -> Value {
    json!({
        "schema": "kiln.patchgate.review.v1",
        "request_identity": "1".repeat(64),
        "observation": {
            "contract_version": 1,
            "effect_id": "review-fixture",
            "idempotency_key": "2".repeat(64),
            "candidate_id": "candidate-fixture",
            "candidate_root": "3".repeat(64),
            "pipeline_revision": "pipeline-fixture",
            "policy_identity": "4".repeat(64),
            "provider_profile": "patchgate-fixture",
            "native_binding_identity": "5".repeat(64),
            "report_identity": "6".repeat(64),
            "isolated": true,
            "authorizes_acceptance": false,
            "reviews": [
                {"change": {"contract_version":1,"provider_namespace":"pijul","native_kind":"change","native_id":"AAA"},"decision": decision}
            ]
        },
        "native_report": "{\n  \"dry_run\": true\n}\n"
    })
}

fn profile(directory: &Path) -> Profile {
    Profile {
        schema: "kiln.web-reviews.v1".into(),
        label: "Fixture receipts".into(),
        directory: directory.to_owned(),
    }
}

fn value(response: &ApiResponse) -> Value {
    serde_json::from_slice(&response.body)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
}

fn get(browser: &Browser, target: &str) -> ApiResponse {
    browser.response("GET", target, &[], Role::Viewer)
}

// r[verify kiln.dashboard.reviews.view.scenario.retained]
#[test]
fn retained_receipts_are_listed_with_exact_identities_and_decisions() {
    let root = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    std::fs::write(
        root.path().join("review-a.json"),
        serde_json::to_vec(&receipt("accepted")).unwrap_or_else(|error| unreachable!("{error}")),
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    std::fs::write(
        root.path().join("review-b.json"),
        serde_json::to_vec(&receipt("escalated")).unwrap_or_else(|error| unreachable!("{error}")),
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    std::fs::write(root.path().join("notes.txt"), b"ignored")
        .unwrap_or_else(|error| unreachable!("{error}"));
    std::fs::write(root.path().join("broken.json"), b"{\"schema\":\"other\"}")
        .unwrap_or_else(|error| unreachable!("{error}"));
    std::os::unix::fs::symlink(
        root.path().join("review-a.json"),
        root.path().join("linked.json"),
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    let browser =
        Browser::new(Some(profile(root.path()))).unwrap_or_else(|error| unreachable!("{error}"));

    let inventory = value(&get(&browser, "/api/reviews"));
    assert_eq!(
        inventory.get("configured").and_then(Value::as_bool),
        Some(true)
    );
    let receipts = inventory
        .get("receipts")
        .and_then(Value::as_array)
        .unwrap_or_else(|| unreachable!("receipts"));
    assert_eq!(receipts.len(), 2);
    let first = receipts.first().unwrap_or_else(|| unreachable!("receipt"));
    assert_eq!(
        first.get("name").and_then(Value::as_str),
        Some("review-a.json")
    );
    assert_eq!(
        first.get("candidate_id").and_then(Value::as_str),
        Some("candidate-fixture")
    );
    assert_eq!(
        first
            .get("decision_counts")
            .and_then(|counts| counts.get("accepted"))
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        first.get("authorizes_acceptance").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        first.get("native_report_bytes").and_then(Value::as_u64),
        Some("{\n  \"dry_run\": true\n}\n".len() as u64)
    );
    let skipped = inventory
        .get("skipped")
        .and_then(Value::as_array)
        .unwrap_or_else(|| unreachable!("skipped"));
    let names: Vec<_> = skipped
        .iter()
        .filter_map(|row| row.get("name").and_then(Value::as_str))
        .collect();
    assert!(names.contains(&"notes.txt"));
    assert!(names.contains(&"broken.json"));
    assert!(names.contains(&"linked.json"));

    let detail = value(&get(&browser, "/api/reviews/review-b.json"));
    assert_eq!(
        detail
            .pointer("/document/observation/reviews/0/decision")
            .and_then(Value::as_str),
        Some("escalated")
    );
    assert_eq!(
        detail.get("bytes").and_then(Value::as_u64),
        Some(
            std::fs::metadata(root.path().join("review-b.json"))
                .map(|m| m.len())
                .unwrap_or(0)
        )
    );
}

// r[verify kiln.dashboard.reviews.view.scenario.absent]
#[test]
fn unconfigured_and_unsafe_receipt_requests_stay_explicit() {
    let unconfigured = Browser::new(None).unwrap_or_else(|error| unreachable!("{error}"));
    let inventory = value(&get(&unconfigured, "/api/reviews"));
    assert_eq!(
        inventory.get("configured").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(get(&unconfigured, "/api/reviews/x.json").status, 404);

    let root = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    std::fs::create_dir(root.path().join("nested")).unwrap_or_else(|error| unreachable!("{error}"));
    let browser =
        Browser::new(Some(profile(root.path()))).unwrap_or_else(|error| unreachable!("{error}"));
    for (target, status) in [
        ("/api/reviews/..%2Fsecret.json", 400),
        ("/api/reviews/secret.txt", 400),
        ("/api/reviews/missing.json", 404),
        ("/api/reviews?path=x", 400),
    ] {
        assert_eq!(get(&browser, target).status, status, "{target}");
    }
    assert_eq!(
        browser
            .response("GET", "/api/reviews", &[], Role::Worker)
            .status,
        403
    );
    assert_eq!(
        browser
            .response("POST", "/api/reviews", b"{}", Role::Operator)
            .status,
        405
    );
    assert_eq!(
        browser
            .response("GET", "/api/reviews", b"{}", Role::Viewer)
            .status,
        400
    );

    let missing_directory = Browser::new(Some(profile(&root.path().join("absent"))))
        .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(get(&missing_directory, "/api/reviews").status, 503);
}
