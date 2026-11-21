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
        "admitted_envelope": "f".repeat(64),
        "dispatch_plan": PLAN,
        "omp": {"model_broker": {"endpoint": "http://127.0.0.1:1/v1/chat/completions", "credential": credential}},
    })
}

/// One refusal record exactly as Lattice `db64cd65` prints it, including the
/// lifecycle refusal the agent-revision reasons carry.
fn refusal_record(reason: &str, dispatched: bool) -> Value {
    let record = json!({
        "schema": REFUSAL_SCHEMA,
        "reason": reason,
        "work_id": "a".repeat(64),
        "attempt_id": "b".repeat(64),
        "fence": 7,
        "dispatched": dispatched,
    });
    if REVISION_REASONS.contains(&reason) {
        with(
            record,
            "revision_refusal",
            json!({"code": "session_revoked"}),
        )
    } else {
        record
    }
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
fn only_run_refusals_lattice_proves_undispatched_settle() {
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
            // stale_fence:false is not a no-start proof while Lattice's fence
            // check races its intent creation.
            let settles = !dispatched && emitted.contains(&false) && reason != "stale_fence";
            let expected = if settles {
                RefusalOutcome::BeforeEffect {
                    reason: reason.to_owned(),
                }
            } else {
                RefusalOutcome::Retain
            };
            let record = refusal(reason, dispatched);
            assert_eq!(
                classify_refusal(&envelope, &record, Launch::Run),
                expected,
                "{reason} dispatched:{dispatched}"
            );
            // A recover refusal describes the recover invocation only.
            assert_eq!(
                classify_refusal(&envelope, &record, Launch::Recover),
                RefusalOutcome::Retain,
                "recover {reason} dispatched:{dispatched}"
            );
        }
    }
}

// r[verify kiln.implementation.lattice_cli.scenario.refusal]
#[test]
fn crossed_unknown_contradictory_or_malformed_refusals_are_retained() {
    let envelope = envelope();
    let base = refusal_record("crossed_attempt", false);
    let classify =
        |record: &Value| classify_refusal(&envelope, record.to_string().as_bytes(), Launch::Run);
    assert_eq!(
        classify(&base),
        RefusalOutcome::BeforeEffect {
            reason: "crossed_attempt".into()
        }
    );
    let changed = |field: &str, new: Value| classify(&with(base.clone(), field, new));
    let mut without_dispatched = base.clone();
    if let Some(fields) = without_dispatched.as_object_mut() {
        fields.remove("dispatched");
    }
    let mut without_lifecycle = refusal_record("revision_blocked", false);
    if let Some(fields) = without_lifecycle.as_object_mut() {
        fields.remove("revision_refusal");
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
        ("missing dispatched", classify(&without_dispatched)),
        ("missing reason", changed("reason", Value::Null)),
        // Execution evidence or any other extension contradicts a no-start refusal.
        (
            "result",
            changed("result", json!({"disposition": "failed"})),
        ),
        (
            "artifacts",
            changed("artifacts", json!({"raw_events": "/x"})),
        ),
        ("accepted", changed("accepted", json!(false))),
        (
            "lifecycle refusal on another reason",
            changed("revision_refusal", json!({"code": "session_revoked"})),
        ),
        (
            "revision reason without its lifecycle refusal",
            classify(&without_lifecycle),
        ),
        (
            "non-object lifecycle refusal",
            classify(&with(
                refusal_record("revision_blocked", false),
                "revision_refusal",
                json!("session_revoked"),
            )),
        ),
    ] {
        assert_eq!(outcome, RefusalOutcome::Retain, "{label}");
    }
    // Duplicate keys are refused rather than resolved by the parser.
    let a = "a".repeat(64);
    let b = "b".repeat(64);
    let duplicated = format!(
        r#"{{"schema":"{REFUSAL_SCHEMA}","reason":"substituted_request","reason":"crossed_attempt","work_id":"{a}","attempt_id":"{b}","fence":7,"dispatched":false}}"#
    );
    assert_eq!(
        classify_refusal(&envelope, duplicated.as_bytes(), Launch::Run),
        RefusalOutcome::Retain
    );
    let dispatched_twice = format!(
        r#"{{"schema":"{REFUSAL_SCHEMA}","reason":"crossed_attempt","work_id":"{a}","attempt_id":"{b}","fence":7,"dispatched":true,"dispatched":false}}"#
    );
    assert_eq!(
        classify_refusal(&envelope, dispatched_twice.as_bytes(), Launch::Run),
        RefusalOutcome::Retain
    );
    for stdout in [
        &b""[..],
        b"not json",
        b"{\"schema\":",
        b"[]",
        b"{\"schema\":\"lattice.implementation-attempt-refusal.v1\"}",
    ] {
        assert_eq!(
            classify_refusal(&envelope, stdout, Launch::Run),
            RefusalOutcome::Retain
        );
    }
    let twice = [
        refusal("crossed_attempt", false),
        refusal("crossed_attempt", false),
    ]
    .concat();
    assert_eq!(
        classify_refusal(&envelope, &twice, Launch::Run),
        RefusalOutcome::Retain
    );
    // An envelope without the bound fields never matches a record.
    assert_eq!(
        classify_refusal(&json!({}), &refusal("crossed_attempt", false), Launch::Run),
        RefusalOutcome::Retain
    );
}

// r[verify kiln.implementation.lattice_cli.scenario.closed]
#[test]
fn recovery_closure_is_no_execution_only_as_the_exact_closure_record() {
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
    let classify = |observed: &Value, plan: &str| {
        classify_observation(&request, observed.to_string().as_bytes(), plan)
    };
    assert_eq!(classify(&closed, PLAN), Ok(Observed::ClosedBeforeExecution));
    let mut executed = closed.clone();
    if let Some(fields) = executed.as_object_mut() {
        fields.remove("status");
    }
    let executed = with(executed, "result", json!({"disposition": "failed"}));
    assert_eq!(classify(&executed, PLAN), Ok(Observed::Execution));
    let mut missing_result = closed.clone();
    if let Some(fields) = missing_result.as_object_mut() {
        fields.remove("result");
    }
    for (label, observed, plan) in [
        (
            "result",
            with(closed.clone(), "result", json!({"disposition": "failed"})),
            PLAN,
        ),
        ("missing result", missing_result, PLAN),
        (
            "artifacts",
            with(closed.clone(), "artifacts", json!({"raw_events": "/x"})),
            PLAN,
        ),
        (
            "accepted",
            with(closed.clone(), "accepted", json!(true)),
            PLAN,
        ),
        (
            "integrated",
            with(closed.clone(), "integrated", json!(true)),
            PLAN,
        ),
        (
            "unchanged observation",
            with(closed.clone(), "unchanged", json!({})),
            PLAN,
        ),
        (
            "another request identity",
            with(closed.clone(), "request_identity", json!("e".repeat(64))),
            PLAN,
        ),
        (
            "crossed envelope",
            with(
                closed.clone(),
                "envelope",
                with(envelope(), "fence", json!(8)),
            ),
            PLAN,
        ),
        (
            "other schema",
            with(closed.clone(), "schema", json!(REFUSAL_SCHEMA)),
            PLAN,
        ),
        ("other plan", closed.clone(), "eeee"),
    ] {
        assert!(classify(&observed, plan).is_err(), "{label}");
    }
    let mut unplanned = closed;
    if let Some(fields) = unplanned.as_object_mut() {
        fields.remove("dispatch_plan");
    }
    assert!(classify(&unplanned, PLAN).is_err());
    // Duplicate keys in a closure are refused rather than resolved.
    let envelope_text = envelope().to_string();
    let identity = "f".repeat(64);
    let duplicated = format!(
        r#"{{"schema":"{OBSERVATION_SCHEMA}","envelope":{envelope_text},"request_identity":"{identity}","status":"{CLOSED_STATUS}","result":null,"result":{{"disposition":"failed"}},"accepted":false,"integrated":false,"dispatch_plan":"{PLAN}"}}"#
    );
    assert!(classify_observation(&request, duplicated.as_bytes(), PLAN).is_err());
    let duplicated_null = format!(
        r#"{{"schema":"{OBSERVATION_SCHEMA}","envelope":{envelope_text},"request_identity":"{identity}","status":"{CLOSED_STATUS}","result":{{"disposition":"failed"}},"result":null,"accepted":false,"integrated":false,"dispatch_plan":"{PLAN}"}}"#
    );
    assert!(classify_observation(&request, duplicated_null.as_bytes(), PLAN).is_err());
}
