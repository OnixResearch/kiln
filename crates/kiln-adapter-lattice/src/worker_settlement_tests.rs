use super::*;
use serde_json::json;

const PLAN: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

fn envelope() -> Value {
    json!({"work_id": "a".repeat(64), "attempt_id": "b".repeat(64), "fence": 7})
}

/// The value with one top-level field replaced or added.
fn with(mut value: Value, field: &str, new: Value) -> Value {
    if let Some(fields) = value.as_object_mut() {
        fields.insert(field.to_owned(), new);
    }
    value
}

fn request(credential: Value) -> Value {
    json!({
        "envelope": envelope(),
        "dispatch_plan": PLAN,
        "omp": {"model_broker": {"endpoint": "http://127.0.0.1:1/v1/chat/completions", "credential": credential}},
    })
}

fn refusal_record(reason: &str, dispatched: bool) -> Value {
    json!({
        "schema": REFUSAL_SCHEMA,
        "reason": reason,
        "work_id": "a".repeat(64),
        "attempt_id": "b".repeat(64),
        "fence": 7,
        "dispatched": dispatched,
    })
}

fn refusal(reason: &str, dispatched: bool) -> Vec<u8> {
    format!("{}\n", refusal_record(reason, dispatched)).into_bytes()
}

// r[verify kiln.implementation.lattice_cli.scenario.credential]
#[test]
fn only_an_env_credential_with_a_plain_name_is_passed_through() {
    let named = request(json!({"provider": "env", "key": "MODEL_TOKEN"}));
    assert_eq!(credential_variable(&named), Ok(Some("MODEL_TOKEN")));
    let null_field = request(json!({"provider": "env", "key": "K2_TOKEN", "field": null}));
    assert_eq!(credential_variable(&null_field), Ok(Some("K2_TOKEN")));
    // No declared credential keeps the fully cleared environment.
    assert_eq!(credential_variable(&request(Value::Null)), Ok(None));
    let mut undeclared = request(Value::Null);
    if let Some(broker) = undeclared
        .pointer_mut("/omp/model_broker")
        .and_then(Value::as_object_mut)
    {
        broker.remove("credential");
    }
    assert_eq!(credential_variable(&undeclared), Ok(None));

    for (credential, refusal) in [
        (json!("MODEL_TOKEN"), CredentialRefusal::Malformed),
        (json!({"key": "MODEL_TOKEN"}), CredentialRefusal::Malformed),
        (json!({"provider": "env"}), CredentialRefusal::Malformed),
        (
            json!({"provider": "env", "key": "MODEL_TOKEN", "field": "token"}),
            CredentialRefusal::Malformed,
        ),
        (
            json!({"provider": "env", "key": "MODEL_TOKEN", "extra": 1}),
            CredentialRefusal::Malformed,
        ),
        (
            json!({"provider": "vault", "key": "MODEL_TOKEN"}),
            CredentialRefusal::UnsupportedProvider,
        ),
        (
            json!({"provider": "env", "key": ""}),
            CredentialRefusal::MalformedKey,
        ),
        (
            json!({"provider": "env", "key": "model_token"}),
            CredentialRefusal::MalformedKey,
        ),
        (
            json!({"provider": "env", "key": "1TOKEN"}),
            CredentialRefusal::MalformedKey,
        ),
        (
            json!({"provider": "env", "key": "MODEL-TOKEN"}),
            CredentialRefusal::MalformedKey,
        ),
        (
            json!({"provider": "env", "key": "MODEL TOKEN"}),
            CredentialRefusal::MalformedKey,
        ),
        (
            json!({"provider": "env", "key": "T".repeat(MAX_CREDENTIAL_KEY_BYTES + 1)}),
            CredentialRefusal::MalformedKey,
        ),
        (
            json!({"provider": "env", "key": "PATH"}),
            CredentialRefusal::ReservedName,
        ),
        (
            json!({"provider": "env", "key": "LD_PRELOAD"}),
            CredentialRefusal::ReservedName,
        ),
        (
            json!({"provider": "env", "key": "LATTICE_OMP"}),
            CredentialRefusal::ReservedName,
        ),
    ] {
        assert_eq!(
            credential_variable(&request(credential.clone())),
            Err(refusal),
            "{credential}"
        );
    }
    assert_eq!(
        credential_variable(&json!({"omp": {}})),
        Err(CredentialRefusal::Malformed)
    );
    assert!(
        CredentialRefusal::Missing
            .to_string()
            .starts_with("worker_credential_missing:")
    );
}

// r[verify kiln.implementation.lattice_cli.scenario.credential]
#[test]
fn the_environment_holds_exactly_the_named_value() {
    assert_eq!(
        credential_environment(None, Some(OsString::from("ignored"))),
        Ok(Vec::new())
    );
    assert_eq!(
        credential_environment(Some("MODEL_TOKEN"), Some(OsString::from("fake"))),
        Ok(vec![(
            OsString::from("MODEL_TOKEN"),
            OsString::from("fake")
        )])
    );
    assert_eq!(
        credential_environment(Some("MODEL_TOKEN"), None),
        Err(CredentialRefusal::Missing)
    );
    assert_eq!(
        credential_environment(Some("MODEL_TOKEN"), Some(OsString::new())),
        Err(CredentialRefusal::Missing)
    );
}

// r[verify kiln.implementation.lattice_cli.scenario.plan]
#[test]
fn the_request_must_carry_the_reserved_plan() {
    let bound = request(Value::Null);
    assert!(request_binds_plan(&bound, PLAN));
    assert!(!request_binds_plan(&bound, &"e".repeat(64)));
    let mut absent = bound.clone();
    if let Some(fields) = absent.as_object_mut() {
        fields.remove("dispatch_plan");
    }
    assert!(!request_binds_plan(&absent, PLAN));
    let numeric = with(bound, "dispatch_plan", json!(7));
    assert!(!request_binds_plan(&numeric, PLAN));
}

// r[verify kiln.implementation.lattice_cli.scenario.refusal]
#[test]
fn every_lattice_refusal_reason_settles_only_when_it_cannot_have_dispatched() {
    let envelope = envelope();
    // Lattice's refusal table: (reason, dispatched values Lattice can emit).
    let table: [(&str, &[bool]); 7] = [
        ("stale_fence", &[false]),
        ("crossed_attempt", &[false]),
        ("substituted_request", &[true]),
        ("profile_revision_superseded", &[true, false]),
        ("revision_not_admitted", &[false]),
        ("revision_blocked", &[false]),
        ("export_revoked", &[true]),
    ];
    for (reason, emitted) in table {
        for dispatched in [false, true] {
            let outcome = classify_refusal(&envelope, &refusal(reason, dispatched));
            let settles = !dispatched && emitted.contains(&false);
            let expected = if settles {
                RefusalOutcome::BeforeEffect {
                    reason: reason.to_owned(),
                }
            } else {
                RefusalOutcome::Retain
            };
            assert_eq!(outcome, expected, "{reason} dispatched:{dispatched}");
        }
    }
    // The agent-revision reasons carry the lifecycle refusal; it is ignored.
    let revision = with(
        refusal_record("revision_blocked", false),
        "revision_refusal",
        json!({"code": "session_revoked"}),
    );
    assert_eq!(
        classify_refusal(&envelope, revision.to_string().as_bytes()),
        RefusalOutcome::BeforeEffect {
            reason: "revision_blocked".into()
        }
    );
}

// r[verify kiln.implementation.lattice_cli.scenario.refusal]
#[test]
fn crossed_unknown_or_malformed_refusals_are_retained() {
    let envelope = envelope();
    let base = refusal_record("stale_fence", false);
    let changed = |field: &str, new: Value| {
        classify_refusal(
            &envelope,
            with(base.clone(), field, new).to_string().as_bytes(),
        )
    };
    let mut without_dispatched = base.clone();
    if let Some(fields) = without_dispatched.as_object_mut() {
        fields.remove("dispatched");
    }
    for (label, outcome) in [
        ("unknown reason", changed("reason", json!("lease_lost"))),
        ("other schema", changed("schema", json!("lattice.other.v1"))),
        ("crossed work", changed("work_id", json!("c".repeat(64)))),
        (
            "crossed attempt",
            changed("attempt_id", json!("c".repeat(64))),
        ),
        ("crossed fence", changed("fence", json!(8))),
        ("string fence", changed("fence", json!("7"))),
        ("string dispatched", changed("dispatched", json!("false"))),
        (
            "missing dispatched",
            classify_refusal(&envelope, without_dispatched.to_string().as_bytes()),
        ),
        ("missing reason", changed("reason", Value::Null)),
    ] {
        assert_eq!(outcome, RefusalOutcome::Retain, "{label}");
    }
    for stdout in [
        &b""[..],
        b"not json",
        b"{\"schema\":",
        b"[]",
        b"{\"schema\":\"lattice.implementation-attempt-refusal.v1\"}",
    ] {
        assert_eq!(classify_refusal(&envelope, stdout), RefusalOutcome::Retain);
    }
    let twice = [refusal("stale_fence", false), refusal("stale_fence", false)].concat();
    assert_eq!(classify_refusal(&envelope, &twice), RefusalOutcome::Retain);
    // An envelope without the bound fields never matches a record.
    assert_eq!(
        classify_refusal(&json!({}), &refusal("stale_fence", false)),
        RefusalOutcome::Retain
    );
}

// r[verify kiln.implementation.lattice_cli.scenario.closed]
#[test]
fn recovery_closure_is_no_execution_only_for_the_exact_envelope_and_plan() {
    let request = request(Value::Null);
    let closed = json!({
        "schema": OBSERVATION_SCHEMA,
        "envelope": envelope(),
        "request_identity": "f".repeat(64),
        "status": CLOSED_STATUS,
        "result": null,
        "accepted": false,
        "integrated": false,
        "dispatch_plan": PLAN,
    });
    assert_eq!(
        classify_observation(&request, &closed, PLAN),
        Ok(Observed::ClosedBeforeExecution)
    );
    let mut executed = closed.clone();
    if let Some(fields) = executed.as_object_mut() {
        fields.remove("status");
    }
    let executed = with(executed, "result", json!({"disposition": "failed"}));
    assert_eq!(
        classify_observation(&request, &executed, PLAN),
        Ok(Observed::Execution)
    );
    let contradictory = with(closed.clone(), "result", json!({"disposition": "failed"}));
    assert!(classify_observation(&request, &contradictory, PLAN).is_err());
    let mut missing_result = closed.clone();
    if let Some(fields) = missing_result.as_object_mut() {
        fields.remove("result");
    }
    assert!(classify_observation(&request, &missing_result, PLAN).is_err());
    let crossed = with(
        closed.clone(),
        "envelope",
        with(envelope(), "fence", json!(8)),
    );
    assert!(classify_observation(&request, &crossed, PLAN).is_err());
    let other_schema = with(closed.clone(), "schema", json!(REFUSAL_SCHEMA));
    assert!(classify_observation(&request, &other_schema, PLAN).is_err());
    assert!(classify_observation(&request, &closed, &"e".repeat(64)).is_err());
    let mut unplanned = closed;
    if let Some(fields) = unplanned.as_object_mut() {
        fields.remove("dispatch_plan");
    }
    assert!(classify_observation(&request, &unplanned, PLAN).is_err());
}
