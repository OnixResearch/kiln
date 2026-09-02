use super::*;
use kiln_provider_api::PortFailure;
use serde_json::{Value, json};

const SECRET: &[u8] = b"fixture webhook secret";
const SHA: &str = "1234567890abcdef1234567890abcdef12345678";
const BASE_SHA: &str = "abcdef1234567890abcdef1234567890abcdef12";
const DELIVERY: &str = "72d3162e-cc78-11e3-81ab-4c9367dc0958";

fn policy() -> EventPolicy {
    EventPolicy {
        owner: "Example".to_owned(),
        repository: "Project".to_owned(),
        repository_id: 42,
        allow_fork_pull_requests: false,
    }
}

fn repository() -> Value {
    json!({
        "id": 42,
        "name": "Project",
        "full_name": "Example/Project",
        "owner": {"login": "Example", "other_github_field": true},
        "ignored_github_field": [1, 2, 3]
    })
}

fn push() -> Value {
    json!({
        "repository": repository(),
        "ref": "refs/heads/main",
        "after": SHA,
        "before": BASE_SHA,
        "deleted": false,
        "commits": []
    })
}

fn pull_request() -> Value {
    json!({
        "action": "opened",
        "repository": repository(),
        "number": 7,
        "pull_request": {
            "number": 7,
            "state": "open",
            "merge_commit_sha": BASE_SHA,
            "head": {"sha": SHA, "ref": "feature", "repo": repository()},
            "base": {"sha": BASE_SHA, "ref": "main", "repo": repository()}
        }
    })
}

fn signed(event: &str, body: &[u8]) -> DeliveryHeaders {
    let mut mac = Hmac::<Sha256>::new_from_slice(SECRET)
        .unwrap_or_else(|error| unreachable!("invalid event fixture: {error:?}"));
    mac.update(body);
    let digest = mac.finalize().into_bytes();
    let mut signature = String::from("sha256=");
    use std::fmt::Write;
    for byte in digest {
        write!(&mut signature, "{byte:02x}")
            .unwrap_or_else(|error| unreachable!("invalid event fixture: {error:?}"));
    }
    DeliveryHeaders {
        delivery: DELIVERY.to_owned(),
        event: event.to_owned(),
        signature,
    }
}

fn admit(policy: &EventPolicy, event: &str, body: &Value) -> PortResult<Option<VerifiedEvent>> {
    let body = serde_json::to_vec(body)
        .unwrap_or_else(|error| unreachable!("invalid event fixture: {error:?}"));
    admit_event(policy, &signed(event, &body), &body, SECRET, 64 * 1024)
}

fn run(result: PortResult<Option<VerifiedEvent>>) -> VerifiedEvent {
    match result.unwrap_or_else(|error| unreachable!("event fixture rejected: {error:?}")) {
        Some(event) => event,
        None => unreachable!("expected executable event"),
    }
}

fn code(result: PortResult<Option<VerifiedEvent>>) -> String {
    match result {
        Err(PortFailure::Domain(diagnostic)) => diagnostic.code,
        other => unreachable!("expected domain rejection: {other:?}"),
    }
}

// r[verify kiln.github.authentication]
#[test]
fn hmac_matches_rfc4231_vector_and_rejects_modified_bytes() {
    let signature = "sha256=b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7";
    assert!(authenticate(signature, b"Hi There", &[0x0b; 20]).is_ok());
    assert!(authenticate(signature, b"Hi there", &[0x0b; 20]).is_err());
}

// r[verify kiln.github.events]
#[test]
fn push_retains_native_facts_but_dispatches_exact_sha_and_canonical_repository() {
    let event = run(admit(&policy(), "push", &push()));
    assert_eq!(event.revision, SHA);
    assert_eq!(event.repository, "example/project");
    assert_eq!(event.repository_id, 42);
    assert_eq!(event.native_repository.owner, "Example");
    assert_eq!(event.native_repository.name, "Project");
    assert_eq!(event.delivery_id, DELIVERY);
    assert_eq!(
        event.selector,
        EventSelector::Push {
            reference: "refs/heads/main".to_owned()
        }
    );
    let body = serde_json::to_vec(&push())
        .unwrap_or_else(|error| unreachable!("invalid event fixture: {error:?}"));
    assert_eq!(
        event.body_identity,
        ContentId::from_blake3_hex(blake3::hash(&body).to_hex().to_string())
            .unwrap_or_else(|error| unreachable!("fixture body identity is valid: {error:?}"))
    );
}

// r[verify kiln.github.events]
#[test]
fn all_selected_pr_actions_use_head_not_base_or_merge_sha() {
    for action in ["opened", "reopened", "synchronize"] {
        let mut payload = pull_request();
        *payload
            .get_mut("action")
            .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!(action);
        let event = run(admit(&policy(), "pull_request", &payload));
        assert_eq!(event.revision, SHA);
        assert_eq!(
            event.selector,
            EventSelector::PullRequest {
                number: 7,
                head_repository: NativeRepository {
                    owner: "Example".to_owned(),
                    name: "Project".to_owned(),
                    id: 42
                },
                head_reference: "feature".to_owned(),
                base_reference: "main".to_owned()
            }
        );
    }
}

// r[verify kiln.github.authentication]
#[test]
fn authentication_precedes_json_decode_and_failure_does_not_reveal_payload() {
    let body = b"secret-bearing malformed JSON";
    let mut headers = signed("push", body);
    headers.signature.replace_range(7..8, "f");
    // Guarantee a changed signature regardless of the original first nibble.
    headers.signature.replace_range(8..9, "z");
    let result = admit_event(&policy(), &headers, body, SECRET, 4096);
    assert_eq!(code(result), "github_authentication");
    let valid_headers = signed("push", body);
    assert_eq!(
        code(admit_event(&policy(), &valid_headers, body, SECRET, 4096)),
        "github_event_payload"
    );
    let body = serde_json::to_vec(&push())
        .unwrap_or_else(|error| unreachable!("invalid event fixture: {error:?}"));
    let headers = signed("push", &body);
    let mut tampered = body.clone();
    tampered.push(b' ');
    assert_eq!(
        code(admit_event(&policy(), &headers, &tampered, SECRET, 4096)),
        "github_authentication"
    );
}

// r[verify kiln.github.authentication]
#[test]
fn malformed_signature_forms_fail_closed() {
    let body = serde_json::to_vec(&push())
        .unwrap_or_else(|error| unreachable!("invalid event fixture: {error:?}"));
    for signature in [
        String::new(),
        format!("sha1={}", "0".repeat(64)),
        format!("SHA256={}", "0".repeat(64)),
        format!("sha256={}", "A".repeat(64)),
        format!("sha256={}", "0".repeat(63)),
        format!("sha256={} ", "0".repeat(64)),
        format!("sha256={},sha256={}", "0".repeat(64), "0".repeat(64)),
    ] {
        let mut headers = signed("push", &body);
        headers.signature = signature;
        assert_eq!(
            code(admit_event(&policy(), &headers, &body, SECRET, 4096)),
            "github_authentication"
        );
    }
    assert!(authenticate(&signed("push", &body).signature, &body, b"").is_err());
}

// r[verify kiln.github.authentication]
#[test]
fn body_and_header_bounds_and_delivery_guid_are_enforced() {
    let body = serde_json::to_vec(&push())
        .unwrap_or_else(|error| unreachable!("invalid event fixture: {error:?}"));
    let headers = signed("push", &body);
    for bound in [0, body.len() - 1] {
        assert_eq!(
            code(admit_event(&policy(), &headers, &body, SECRET, bound)),
            "github_body_bound"
        );
    }
    assert!(matches!(
        admit_event(&policy(), &headers, &body, SECRET, body.len()),
        Ok(Some(_))
    ));
    for delivery in [
        "",
        "not-a-guid",
        "72d3162ecc7811e381ab4c9367dc0958",
        "72d3162e-cc78-11e3-81ab-4c9367dc095x",
        "72d3162e-cc78-11e3-81ab-4c9367dc0958\n",
    ] {
        let mut headers = headers.clone();
        headers.delivery = delivery.to_owned();
        assert_eq!(
            code(admit_event(&policy(), &headers, &body, SECRET, 4096)),
            "github_event_headers"
        );
    }
    for event in ["", "Push", "push,push", "push\r\n", "push "] {
        let mut headers = headers.clone();
        headers.event = event.to_owned();
        assert_eq!(
            code(admit_event(&policy(), &headers, &body, SECRET, 4096)),
            "github_event_headers"
        );
    }
}

// r[verify kiln.github.events]
#[test]
fn repository_name_id_and_internal_name_consistency_are_required() {
    for replacement in [
        json!({"id": 43, "name": "Project", "full_name": "Example/Project", "owner": {"login": "Example"}}),
        json!({"id": 42, "name": "Other", "full_name": "Example/Other", "owner": {"login": "Example"}}),
        json!({"id": 42, "name": "Project", "full_name": "Other/Project", "owner": {"login": "Other"}}),
        json!({"id": 42, "name": "Project", "full_name": "Example/Other", "owner": {"login": "Example"}}),
        json!({"id": 0, "name": "Project", "full_name": "Example/Project", "owner": {"login": "Example"}}),
    ] {
        let mut payload = push();
        *payload
            .get_mut("repository")
            .unwrap_or_else(|| unreachable!("event fixture field exists")) = replacement;
        assert!(admit(&policy(), "push", &payload).is_err());
    }
}

// r[verify kiln.github.events]
#[test]
fn malformed_sha_and_reference_never_become_executable_candidates() {
    for revision in [
        "",
        "main",
        "1234",
        "0000000000000000000000000000000000000000",
        "1234567890ABCDEF1234567890ABCDEF12345678",
        "1234567890abcdef1234567890abcdef1234567g",
        "1234567890abcdef1234567890abcdef12345678901234567890abcdef12345678",
    ] {
        let mut payload = push();
        *payload
            .get_mut("after")
            .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!(revision);
        assert!(admit(&policy(), "push", &payload).is_err());
        let mut payload = pull_request();
        *payload
            .get_mut("pull_request")
            .and_then(|value| value.get_mut("head"))
            .and_then(|value| value.get_mut("sha"))
            .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!(revision);
        assert!(admit(&policy(), "pull_request", &payload).is_err());
    }
    for reference in [
        "main",
        "refs/heads/",
        "refs/heads/a..b",
        "refs/heads/a.lock",
        "refs/heads/a b",
        "refs/heads/a@{b",
        "refs/heads/.hidden",
        "refs/heads/a//b",
    ] {
        let mut payload = push();
        *payload
            .get_mut("ref")
            .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!(reference);
        assert!(admit(&policy(), "push", &payload).is_err());
    }
    let mut payload = push();
    *payload
        .get_mut("ref")
        .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!("refs/tags/v1.0");
    assert!(matches!(admit(&policy(), "push", &payload), Ok(Some(_))));
}

// r[verify kiln.github.events]
#[test]
fn deleted_pushes_closed_prs_and_unselected_events_are_ignored() {
    let mut payload = push();
    *payload
        .get_mut("deleted")
        .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!(true);
    *payload
        .get_mut("after")
        .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!("0".repeat(40));
    assert!(matches!(admit(&policy(), "push", &payload), Ok(None)));
    *payload
        .get_mut("after")
        .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!(SHA);
    assert!(admit(&policy(), "push", &payload).is_err());
    for action in ["closed", "labeled", "edited", "ready_for_review"] {
        let mut payload = pull_request();
        *payload
            .get_mut("action")
            .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!(action);
        assert!(matches!(
            admit(&policy(), "pull_request", &payload),
            Ok(None)
        ));
    }
    let mut payload = pull_request();
    *payload
        .get_mut("pull_request")
        .and_then(|value| value.get_mut("state"))
        .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!("closed");
    assert!(matches!(
        admit(&policy(), "pull_request", &payload),
        Ok(None)
    ));
    for event in ["ping", "issues", "pull_request_target", "future_event"] {
        assert!(matches!(admit(&policy(), event, &push()), Ok(None)));
    }
}

// r[verify kiln.github.events]
#[test]
fn pr_base_number_and_head_repository_consistency_are_enforced() {
    for pointer in [
        "/repository/id",
        "/pull_request/base/repo/id",
        "/pull_request/head/repo/id",
        "/pull_request/number",
    ] {
        let mut payload = pull_request();
        *payload
            .pointer_mut(pointer)
            .unwrap_or_else(|| unreachable!("event fixture pointer exists")) = json!(99);
        let mut enabled = policy();
        enabled.allow_fork_pull_requests = true;
        assert!(admit(&enabled, "pull_request", &payload).is_err());
    }
    let mut payload = pull_request();
    *payload
        .get_mut("pull_request")
        .and_then(|value| value.get_mut("head"))
        .and_then(|value| value.get_mut("repo"))
        .and_then(|value| value.get_mut("full_name"))
        .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!("Other/Project");
    assert!(admit(&policy(), "pull_request", &payload).is_err());
    let mut payload = pull_request();
    *payload
        .get_mut("pull_request")
        .and_then(|value| value.get_mut("base"))
        .and_then(|value| value.get_mut("sha"))
        .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!("main");
    assert!(admit(&policy(), "pull_request", &payload).is_err());
}

// r[verify kiln.github.events]
#[test]
fn forks_require_explicit_policy_and_retain_exact_source_facts() {
    let mut payload = pull_request();
    *payload
        .get_mut("pull_request")
        .and_then(|value| value.get_mut("head"))
        .and_then(|value| value.get_mut("repo"))
        .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!({
        "id": 99, "name": "Fork", "full_name": "Contributor/Fork",
        "owner": {"login": "Contributor"}
    });
    assert_eq!(
        code(admit(&policy(), "pull_request", &payload)),
        "github_fork_denied"
    );
    let mut enabled = policy();
    enabled.allow_fork_pull_requests = true;
    let event = run(admit(&enabled, "pull_request", &payload));
    assert_eq!(event.repository, "example/project");
    assert_eq!(event.revision, SHA);
    match event.selector {
        EventSelector::PullRequest {
            head_repository, ..
        } => {
            assert_eq!(head_repository.id, 99);
            assert_eq!(head_repository.owner, "Contributor");
            assert_eq!(head_repository.name, "Fork");
        }
        _ => unreachable!("expected pull request selector"),
    }
    let decoded: EventPolicy = serde_json::from_value(json!({
        "owner": "Example", "repository": "Project", "repository_id": 42
    }))
    .unwrap_or_else(|error| unreachable!("invalid event fixture: {error:?}"));
    assert!(!decoded.allow_fork_pull_requests);
}

// r[verify kiln.github.authentication]
// r[verify kiln.github.events]
#[test]
fn malformed_json_missing_fields_and_duplicate_critical_fields_are_rejected() {
    let original = serde_json::to_string(&push())
        .unwrap_or_else(|error| unreachable!("invalid event fixture: {error:?}"));
    let duplicate_repository =
        original.replacen("\"repository\":", "\"repository\":{},\"repository\":", 1);
    let duplicate_sha = original.replacen("\"after\":", "\"after\":\"bad\",\"after\":", 1);
    let duplicate_owner = original.replacen("\"login\":", "\"login\":\"Other\",\"login\":", 1);
    let original_pr = serde_json::to_string(&pull_request())
        .unwrap_or_else(|error| unreachable!("invalid event fixture: {error:?}"));
    let duplicate_action =
        original_pr.replacen("\"action\":", "\"action\":\"closed\",\"action\":", 1);
    let duplicate_head = original_pr.replacen("\"head\":", "\"head\":{},\"head\":", 1);
    for (event, body) in [
        ("push", "{".to_owned()),
        ("push", "{}".to_owned()),
        ("push", format!("{original} true")),
        ("push", duplicate_repository),
        ("push", duplicate_sha),
        ("push", duplicate_owner),
        ("pull_request", duplicate_action),
        ("pull_request", duplicate_head),
    ] {
        let body = body.as_bytes();
        assert_eq!(
            code(admit_event(
                &policy(),
                &signed(event, body),
                body,
                SECRET,
                64 * 1024
            )),
            "github_event_payload"
        );
    }
}

// r[verify kiln.github.replay]
#[test]
fn delivery_and_incidental_payload_changes_do_not_change_semantic_candidate() {
    let first = run(admit(&policy(), "push", &push()));
    let mut payload = push();
    payload
        .as_object_mut()
        .unwrap_or_else(|| unreachable!("event fixture is an object"))
        .insert("sender".into(), json!({"login": "another-actor"}));
    let bytes = serde_json::to_vec(&payload)
        .unwrap_or_else(|error| unreachable!("invalid event fixture: {error:?}"));
    let mut headers = signed("push", &bytes);
    headers.delivery = "82d3162e-cc78-11e3-81ab-4c9367dc0958".to_owned();
    let second = run(admit_event(&policy(), &headers, &bytes, SECRET, 64 * 1024));
    assert_ne!(first.delivery_id, second.delivery_id);
    assert_ne!(first.body_identity, second.body_identity);
    assert_eq!(first.candidate_identity, second.candidate_identity);

    let mut payload = push();
    *payload
        .get_mut("ref")
        .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!("refs/heads/other");
    let other_branch = run(admit(&policy(), "push", &payload));
    assert_ne!(first.candidate_identity, other_branch.candidate_identity);
    let mut payload = push();
    *payload
        .get_mut("after")
        .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!(BASE_SHA);
    let other_revision = run(admit(&policy(), "push", &payload));
    assert_ne!(first.candidate_identity, other_revision.candidate_identity);
}

// r[verify kiln.github.replay]
#[test]
fn pr_action_redelivery_deduplicates_but_distinct_prs_and_event_kinds_do_not() {
    let first = run(admit(&policy(), "pull_request", &pull_request()));
    let mut payload = pull_request();
    *payload
        .get_mut("action")
        .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!("synchronize");
    let second = run(admit(&policy(), "pull_request", &payload));
    assert_eq!(first.candidate_identity, second.candidate_identity);
    *payload
        .get_mut("number")
        .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!(8);
    *payload
        .get_mut("pull_request")
        .and_then(|value| value.get_mut("number"))
        .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!(8);
    let other_pr = run(admit(&policy(), "pull_request", &payload));
    assert_ne!(first.candidate_identity, other_pr.candidate_identity);
    let push = run(admit(&policy(), "push", &push()));
    assert_ne!(first.candidate_identity, push.candidate_identity);
}

// r[verify kiln.github.replay]
#[test]
fn case_normalization_preserves_native_spelling_without_dedup_bypass() {
    let first = run(admit(&policy(), "push", &push()));
    let mut payload = push();
    *payload
        .get_mut("repository")
        .and_then(|value| value.get_mut("owner"))
        .and_then(|value| value.get_mut("login"))
        .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!("EXAMPLE");
    *payload
        .get_mut("repository")
        .and_then(|value| value.get_mut("name"))
        .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!("PROJECT");
    *payload
        .get_mut("repository")
        .and_then(|value| value.get_mut("full_name"))
        .unwrap_or_else(|| unreachable!("event fixture field exists")) = json!("EXAMPLE/PROJECT");
    let bytes = serde_json::to_vec(&payload)
        .unwrap_or_else(|error| unreachable!("invalid event fixture: {error:?}"));
    let mut headers = signed("push", &bytes);
    headers.delivery = DELIVERY.to_ascii_uppercase();
    let second = run(admit_event(&policy(), &headers, &bytes, SECRET, 64 * 1024));
    assert_eq!(first.delivery_id, second.delivery_id);
    assert_eq!(first.candidate_identity, second.candidate_identity);
    assert_eq!(second.native_repository.owner, "EXAMPLE");
    assert_eq!(second.native_repository.name, "PROJECT");
}

// r[verify kiln.github.events]
#[test]
fn repository_policy_respects_core_native_identity_boundary() {
    let mut admitted = policy();
    admitted.owner = "o".repeat(39);
    admitted.repository = "r".repeat(88);
    assert!(admit_event_policy(&admitted).is_ok());
    admitted.repository.push('r');
    assert!(admit_event_policy(&admitted).is_err());

    let body = serde_json::to_vec(&push())
        .unwrap_or_else(|error| unreachable!("invalid event fixture: {error:?}"));
    assert_eq!(
        code(admit_event(
            &admitted,
            &signed("push", &body),
            &body,
            SECRET,
            4096
        )),
        "github_event_policy"
    );
}
