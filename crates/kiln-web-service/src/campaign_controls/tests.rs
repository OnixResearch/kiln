//! Campaign controls over real SQLite campaign stores and control profiles.
use super::*;
use kiln_core::implementation::campaign::{
    CAMPAIGN_SCHEMA, EnumerationState, Freshness, MemberObservation, MemberStage,
};
use serde_json::{Value, json};

const CAMPAIGN: &str = "controlled";
const MEMBER: &str = "change-a/*";
const OPERATOR: &str = "local:operator";
const NOW_MS: u64 = 1_000_000;
static NULL: Value = Value::Null;

fn id(byte: char) -> ContentId {
    ContentId::from_blake3_hex(byte.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

/// The JSON value at `pointer`, or null when it is absent.
fn at<'a>(value: &'a Value, pointer: &str) -> &'a Value {
    value.pointer(pointer).unwrap_or(&NULL)
}

fn set(value: &mut Value, pointer: &str, replacement: Value) {
    if let Some(slot) = value.pointer_mut(pointer) {
        *slot = replacement;
    }
}

fn contract(campaign_id: &str, goal: CompletionGoal) -> CampaignContract {
    CampaignContract {
        schema: CAMPAIGN_SCHEMA.into(),
        campaign_id: campaign_id.into(),
        revision: 1,
        repository: "repo".into(),
        members: vec![MemberBinding {
            selector: MemberSelector::WholeChange {
                change: "change-a".into(),
            },
            profile: WorkflowProfile::SpecDriven,
            profile_version: id('a'),
            repository_instructions: id('b'),
            policy: id('a'),
            required_checks: vec!["acceptance".into()],
            required_reviews: vec!["maintainer".into()],
            integration_policy: id('a'),
            goal,
            prerequisite_inclusion: false,
            context: Vec::new(),
        }],
        max_concurrency: 1,
        max_outstanding_decisions: 1,
        parent_budget_id: id('a'),
        agent_definition: id('a'),
        require_one_writer_per_repository: true,
    }
}

fn verified(evidence: char) -> MemberObservation {
    MemberObservation {
        member: MEMBER.into(),
        stage: MemberStage::VerifiedCandidate,
        enumeration: EnumerationState::Complete,
        source_identity: Some(id('a')),
        policy_identity: id('a'),
        subject_identity: Some(id('a')),
        integration_target_identity: None,
        profile_identity: id('a'),
        dependencies: Vec::new(),
        remaining_obligations: Default::default(),
        dependency_reason: None,
        canonical_work_key: Some(id('a')),
        work_identity: Some(id('a')),
        attempt_identity: None,
        attempt_fence: None,
        context_identity: None,
        native_context_identity: None,
        context_freshness: Freshness::NotApplicable,
        candidate_freshness: Freshness::Current,
        policy_freshness: Freshness::Current,
        verifier_freshness: Freshness::Current,
        target_freshness: Freshness::Unknown,
        evidence: vec![id(evidence)],
        blockers: Vec::new(),
        required_checks_satisfied: true,
        required_reviews_satisfied: false,
        delivery_route_supported: Some(true),
        recovery: None,
        consumed_budget: 0,
        reserved_budget: 0,
        unknown_budget: 0,
        cancelled: false,
    }
}

fn decision(identity: char, kind: DecisionKind) -> HumanDecisionRequest {
    HumanDecisionRequest {
        identity: id(identity),
        kind,
        member: MEMBER.into(),
        work_identity: Some(id('a')),
        decision_revision: None,
        policy_identity: id('a'),
        candidate_identity: Some(id('c')),
        state: DecisionRequestState::Outstanding,
        created_ms: 1,
        question: "Approve the verified candidate?".into(),
        options: vec!["approve".into(), "reject".into()],
        recommendation: Some("approve".into()),
        priority: 1,
        pending_reason: None,
        resolved_answer: None,
        active_effort_ms: None,
    }
}

fn version(database: &Path) -> u64 {
    CampaignStore::open(database)
        .and_then(|store| store.version(CAMPAIGN))
        .unwrap_or_else(|error| unreachable!("{error}"))
        .unwrap_or_else(|| unreachable!("campaign version"))
}

/// A campaign whose member awaits review `e`; preference `f` waits unissued
/// beyond the outstanding bound.
fn seed(database: &Path) {
    let mut store = CampaignStore::open(database).unwrap_or_else(|error| unreachable!("{error}"));
    store
        .start(contract(CAMPAIGN, CompletionGoal::VerifiedPullRequest))
        .unwrap_or_else(|error| unreachable!("{error}"));
    store
        .observe(CAMPAIGN, version(database), verified('c'))
        .unwrap_or_else(|error| unreachable!("{error}"));
    for (identity, kind) in [
        ('e', DecisionKind::RequiredReview),
        ('f', DecisionKind::Preference),
    ] {
        store
            .add_decision(CAMPAIGN, version(database), decision(identity, kind))
            .unwrap_or_else(|error| unreachable!("{error}"));
    }
}

fn all_commands() -> Value {
    json!([
        "start",
        "continue",
        "pause",
        "cancel",
        "issue_decision",
        "resolve_decision"
    ])
}

fn grant(principal: &str, commands: Value, roles: Value) -> Value {
    json!({
        "principal": principal,
        "campaigns": [CAMPAIGN, "previewed", "unsupported"],
        "commands": commands,
        "review_roles": roles,
    })
}

fn write_profile(directory: &Path, database: &Path, grants: Value, definitions: Value) -> PathBuf {
    let path = directory.join("controls.json");
    let profile = json!({
        "schema": PROFILE_SCHEMA,
        "campaign_database": database,
        "grants": grants,
        "definitions": definitions,
    });
    std::fs::write(&path, profile.to_string()).unwrap_or_else(|error| unreachable!("{error}"));
    path
}

struct Fixture {
    directory: tempfile::TempDir,
    database: PathBuf,
    profile: PathBuf,
    controls: Controls,
}

fn fixture(grants: Value) -> Fixture {
    let directory = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let database = directory.path().join("campaign.sqlite");
    seed(&database);
    let profile = write_profile(directory.path(), &database, grants, json!([]));
    let controls =
        Controls::new(&profile, Some(&database)).unwrap_or_else(|error| unreachable!("{error}"));
    Fixture {
        directory,
        database,
        profile,
        controls,
    }
}

fn call(
    controls: &Controls,
    method: &str,
    target: &str,
    body: &Value,
    principal: &str,
    role: Role,
) -> (u16, Value, Option<&'static str>) {
    let bytes = if body.is_null() {
        Vec::new()
    } else {
        body.to_string().into_bytes()
    };
    let result = controls.response(&ControlRequest {
        method,
        target,
        body: &bytes,
        principal,
        role,
        now_ms: NOW_MS,
    });
    let value =
        serde_json::from_slice(&result.response.body).unwrap_or_else(|_| unreachable!("json"));
    (result.response.status, value, result.denial)
}

fn code(value: &Value) -> Option<&str> {
    at(value, "/code").as_str()
}

fn inbox_as(controls: &Controls, principal: &str, role: Role) -> (u16, Value) {
    let (status, body, _) = call(
        controls,
        "GET",
        &format!("{INBOX_ROUTE}?campaign={CAMPAIGN}"),
        &Value::Null,
        principal,
        role,
    );
    (status, body)
}

fn post(controls: &Controls, body: &Value, principal: &str) -> (u16, Value, Option<&'static str>) {
    call(
        controls,
        "POST",
        COMMANDS_ROUTE,
        body,
        principal,
        Role::Operator,
    )
}

fn admission_command(command: &str, request_id: &str, expected_version: u64) -> Value {
    json!({
        "command": command,
        "request_id": request_id,
        "campaign_id": CAMPAIGN,
        "expected_version": expected_version,
    })
}

fn answer(request_id: &str, expected_version: u64, decision: char) -> Value {
    json!({
        "command": "resolve_decision",
        "request_id": request_id,
        "campaign_id": CAMPAIGN,
        "expected_version": expected_version,
        "decision_identity": id(decision),
        "work_identity": id('a'),
        "policy_identity": id('a'),
        "candidate_identity": id('c'),
        "answer": "approve",
    })
}

fn decision_entry(inbox: &Value, identity: char) -> &Value {
    at(inbox, "/decisions")
        .as_array()
        .and_then(|decisions| {
            decisions
                .iter()
                .find(|entry| at(entry, "/identity") == &json!(id(identity)))
        })
        .unwrap_or_else(|| unreachable!("decision {identity} listed"))
}

#[test]
fn profile_admission_refuses_wildcards_duplicates_and_relative_paths() {
    let valid = json!({
        "schema": PROFILE_SCHEMA,
        "campaign_database": "/var/lib/kiln/implementation.sqlite",
        "grants": [grant(OPERATOR, all_commands(), json!(["maintainer"]))],
        "definitions": [{"contract": "/etc/kiln/campaign.json", "delivery_route": "native_git"}],
    });
    let admit = |value: &Value| {
        serde_json::from_value::<Profile>(value.clone())
            .map_err(|_| "malformed")
            .and_then(|profile| profile.admit())
    };
    assert_eq!(admit(&valid), Ok(()));
    let mut broken = Vec::new();
    for (pointer, replacement) in [
        ("/schema", json!("kiln.web-campaign-controls.v0")),
        ("/campaign_database", json!("relative.sqlite")),
        ("/grants", json!([])),
        ("/grants/0/principal", json!("*")),
        ("/grants/0/principal", json!("two words")),
        ("/grants/0/campaigns", json!([])),
        ("/grants/0/campaigns", json!([CAMPAIGN, CAMPAIGN])),
        ("/grants/0/commands", json!([])),
        ("/grants/0/commands", json!(["pause", "pause"])),
        ("/grants/0/commands", json!(["deploy"])),
        ("/grants/0/review_roles", json!(["*"])),
        ("/definitions/0/contract", json!("campaign.json")),
        ("/definitions/0/delivery_route", json!("ftp")),
    ] {
        let mut value = valid.clone();
        set(&mut value, pointer, replacement);
        broken.push(value);
    }
    let mut duplicate = valid.clone();
    if let Some(grants) = duplicate
        .pointer_mut("/grants")
        .and_then(Value::as_array_mut)
    {
        grants.push(grant(OPERATOR, json!(["pause"]), json!([])));
    }
    broken.push(duplicate);
    let mut unknown = valid.clone();
    if let Some(grant) = unknown
        .pointer_mut("/grants/0")
        .and_then(Value::as_object_mut)
    {
        grant.insert("owner".into(), json!(true));
    }
    broken.push(unknown);
    for value in broken {
        assert!(admit(&value).is_err(), "{value}");
    }
}

#[test]
// r[verify kiln.implementation.campaign.controls.scenario.roles]
fn viewers_read_workers_are_excluded_and_operators_see_only_granted_operations() {
    let fixture = fixture(json!([grant(
        OPERATOR,
        all_commands(),
        json!(["maintainer"])
    )]));
    let (status, viewer) = inbox_as(&fixture.controls, "local:viewer", Role::Viewer);
    assert_eq!(status, 200);
    assert_eq!(at(&viewer, "/operations"), &json!([]));
    assert_eq!(at(&viewer, "/admission"), "open");
    let viewed = decision_entry(&viewer, 'e');
    assert_eq!(at(viewed, "/actions"), &json!([]));
    assert_eq!(at(viewed, "/question"), "Approve the verified candidate?");

    let (status, worker, denial) = call(
        &fixture.controls,
        "GET",
        &format!("{INBOX_ROUTE}?campaign={CAMPAIGN}"),
        &Value::Null,
        "local:worker",
        Role::Worker,
    );
    assert_eq!(
        (status, code(&worker), denial),
        (
            403,
            Some("campaign_controls_forbidden"),
            Some("campaign_controls_forbidden")
        )
    );

    let (_, operator) = inbox_as(&fixture.controls, OPERATOR, Role::Operator);
    assert_eq!(
        at(&operator, "/operations"),
        &json!(["pause", "cancel", "resolve_decision"])
    );
    let review = decision_entry(&operator, 'e');
    for (pointer, expected) in [
        ("/state", json!("outstanding")),
        ("/category", json!("required_review")),
        ("/scope", json!("current")),
        ("/actions", json!(["resolve"])),
        ("/review_roles", json!(["maintainer"])),
        ("/recommendation", json!("approve")),
    ] {
        assert_eq!(at(review, pointer), &expected, "{pointer}");
    }
    let preference = decision_entry(&operator, 'f');
    assert_eq!(at(preference, "/state"), "unissued");
    assert_eq!(
        at(preference, "/actions"),
        &json!([]),
        "the outstanding bound is full"
    );
    let (_, outsider) = inbox_as(&fixture.controls, "local:other", Role::Operator);
    assert_eq!(at(&outsider, "/operations"), &json!([]));

    let before = version(&fixture.database);
    for (principal, role, expected) in [
        ("local:viewer", Role::Viewer, "campaign_command_denied"),
        ("local:worker", Role::Worker, "campaign_controls_forbidden"),
        ("local:other", Role::Operator, "campaign_command_denied"),
    ] {
        let (status, body, denial) = call(
            &fixture.controls,
            "POST",
            COMMANDS_ROUTE,
            &admission_command("pause", "pause-by-someone", before),
            principal,
            role,
        );
        assert_eq!(
            (status, code(&body), denial),
            (403, Some(expected), Some(expected))
        );
    }
    assert_eq!(version(&fixture.database), before);
}

#[test]
// r[verify kiln.implementation.campaign.controls.scenario.replay]
// r[verify kiln.implementation.campaign.controls.scenario.conflict]
fn commands_apply_once_conflicts_change_nothing_and_grants_are_reread() {
    let fixture = fixture(json!([grant(
        OPERATOR,
        json!(["pause", "continue"]),
        json!([])
    )]));
    let current = version(&fixture.database);
    let pause = admission_command("pause", "console-pause-0001", current);
    let (status, first, _) = post(&fixture.controls, &pause, OPERATOR);
    assert_eq!(status, 200);
    assert_eq!(
        (
            at(&first, "/replayed").as_bool(),
            at(&first, "/admission").as_str(),
            at(&first, "/store_version").as_u64(),
            at(&first, "/actor").as_str(),
        ),
        (
            Some(false),
            Some("paused"),
            Some(current + 1),
            Some(OPERATOR)
        )
    );
    let (status, replayed, _) = post(&fixture.controls, &pause, OPERATOR);
    assert_eq!(status, 200);
    assert_eq!(at(&replayed, "/replayed"), true);
    assert_eq!(
        at(&replayed, "/store_version"),
        at(&first, "/store_version")
    );
    assert_eq!(version(&fixture.database), current + 1);

    for (body, expected) in [
        (
            admission_command("continue", "console-continue-stale", current),
            "campaign_version_conflict",
        ),
        (
            admission_command("continue", "console-pause-0001", current + 1),
            "campaign_request_conflict",
        ),
    ] {
        let (status, refused, _) = post(&fixture.controls, &body, OPERATOR);
        assert_eq!((status, code(&refused)), (409, Some(expected)));
    }
    let (status, narrowed, denial) = post(
        &fixture.controls,
        &admission_command("cancel", "console-cancel-0001", current + 1),
        OPERATOR,
    );
    assert_eq!(
        (status, code(&narrowed), denial),
        (
            403,
            Some("campaign_command_denied"),
            Some("campaign_command_denied")
        )
    );
    assert_eq!(version(&fixture.database), current + 1);

    // An expiring grant stops at its limit and a removed grant at once,
    // without restarting the service.
    let mut expiring = grant(OPERATOR, json!(["pause", "continue"]), json!([]));
    if let Some(grant) = expiring.as_object_mut() {
        grant.insert("not_after_ms".into(), json!(NOW_MS));
    }
    write_profile(
        fixture.directory.path(),
        &fixture.database,
        json!([expiring]),
        json!([]),
    );
    let resume = admission_command("continue", "console-continue-0001", current + 1);
    assert_eq!(post(&fixture.controls, &resume, OPERATOR).0, 403);
    write_profile(
        fixture.directory.path(),
        &fixture.database,
        json!([grant("local:other", json!(["continue"]), json!([]))]),
        json!([]),
    );
    assert_eq!(post(&fixture.controls, &resume, OPERATOR).0, 403);
    assert_eq!(version(&fixture.database), current + 1);

    // A restarted service replays the lost response from the owner record.
    write_profile(
        fixture.directory.path(),
        &fixture.database,
        json!([grant(OPERATOR, json!(["pause", "continue"]), json!([]))]),
        json!([]),
    );
    let restarted = Controls::new(&fixture.profile, Some(&fixture.database))
        .unwrap_or_else(|error| unreachable!("{error}"));
    let (status, after_restart, _) = post(&restarted, &pause, OPERATOR);
    assert_eq!(
        (status, at(&after_restart, "/replayed").as_bool()),
        (200, Some(true))
    );
    std::fs::remove_file(&fixture.profile).unwrap_or_else(|error| unreachable!("{error}"));
    let (status, unavailable, _) = post(&restarted, &resume, OPERATOR);
    assert_eq!(
        (status, code(&unavailable)),
        (503, Some("campaign_controls_unavailable"))
    );
}

#[test]
// r[verify kiln.implementation.campaign.controls.scenario.decisions]
fn required_review_answers_need_a_review_role_and_the_current_scope() {
    let fixture = fixture(json!([
        grant(OPERATOR, all_commands(), json!(["maintainer"])),
        grant("local:delegate", all_commands(), json!(["other-role"])),
    ]));
    let current = version(&fixture.database);
    let (status, _, denial) = post(
        &fixture.controls,
        &answer("delegate-answer-1", current, 'e'),
        "local:delegate",
    );
    assert_eq!((status, denial), (403, Some("campaign_command_denied")));
    let (_, delegate_view) = inbox_as(&fixture.controls, "local:delegate", Role::Operator);
    assert_eq!(
        at(decision_entry(&delegate_view, 'e'), "/actions"),
        &json!([])
    );

    // A superseding candidate makes the outstanding review stale.
    let mut store =
        CampaignStore::open(&fixture.database).unwrap_or_else(|error| unreachable!("{error}"));
    store
        .observe(CAMPAIGN, current, verified('d'))
        .unwrap_or_else(|error| unreachable!("{error}"));
    let superseded = version(&fixture.database);
    let (_, changed) = inbox_as(&fixture.controls, OPERATOR, Role::Operator);
    let stale_review = decision_entry(&changed, 'e');
    assert_eq!(at(stale_review, "/scope"), "changed");
    assert_eq!(at(stale_review, "/actions"), &json!([]));
    let (status, stale, _) = post(
        &fixture.controls,
        &answer("operator-answer-stale", superseded, 'e'),
        OPERATOR,
    );
    assert_eq!(
        (status, code(&stale)),
        (409, Some("campaign_decision_scope_changed"))
    );
    store
        .observe(CAMPAIGN, superseded, verified('c'))
        .unwrap_or_else(|error| unreachable!("{error}"));

    let (status, other, _) = post(
        &fixture.controls,
        &answer("operator-answer-other", version(&fixture.database), 'f'),
        OPERATOR,
    );
    assert_eq!(
        (status, code(&other)),
        (409, Some("campaign_decision_stale")),
        "an unissued decision cannot be answered"
    );
    let (status, resolved, _) = post(
        &fixture.controls,
        &answer("operator-answer-1", version(&fixture.database), 'e'),
        OPERATOR,
    );
    assert_eq!(status, 200);
    assert_eq!(at(&resolved, "/decision/state"), "resolved");
    let (_, after) = inbox_as(&fixture.controls, OPERATOR, Role::Operator);
    let review = decision_entry(&after, 'e');
    assert_eq!(
        (
            at(review, "/state").as_str(),
            at(review, "/resolved_answer").as_str()
        ),
        (Some("resolved"), Some("approve"))
    );
    assert_eq!(
        at(decision_entry(&after, 'f'), "/actions"),
        &json!(["issue"])
    );
}

#[test]
// r[verify kiln.implementation.campaign.controls.scenario.start]
// r[verify kiln.implementation.campaign.controls.scenario.endpoints]
fn start_previews_bind_frozen_definitions_and_their_delivery_route() {
    let directory = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let database = directory.path().join("campaign.sqlite");
    CampaignStore::open(&database).unwrap_or_else(|error| unreachable!("{error}"));
    let previewed = directory.path().join("previewed.json");
    let unsupported = directory.path().join("unsupported.json");
    let write_contract = |path: &Path, contract: &CampaignContract| {
        std::fs::write(
            path,
            serde_json::to_vec(contract).unwrap_or_else(|error| unreachable!("{error}")),
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    };
    write_contract(
        &previewed,
        &contract("previewed", CompletionGoal::NativeLifecycleComplete),
    );
    write_contract(
        &unsupported,
        &contract("unsupported", CompletionGoal::VerifiedPullRequest),
    );
    let profile = write_profile(
        directory.path(),
        &database,
        json!([grant(OPERATOR, all_commands(), json!(["maintainer"]))]),
        json!([
            {"contract": previewed, "delivery_route": "native_git"},
            {"contract": unsupported, "delivery_route": "native_git"},
        ]),
    );
    let controls =
        Controls::new(&profile, Some(&database)).unwrap_or_else(|error| unreachable!("{error}"));
    let list = |principal: &str, role: Role| {
        let (status, body, _) = call(
            &controls,
            "GET",
            DEFINITIONS_ROUTE,
            &Value::Null,
            principal,
            role,
        );
        assert_eq!(status, 200);
        body
    };
    let preview = list(OPERATOR, Role::Operator);
    let ready = at(&preview, "/definitions/0");
    let refused = at(&preview, "/definitions/1");
    assert_eq!(at(&preview, "/definitions/2"), &Value::Null);
    for (pointer, expected) in [
        ("/campaign_id", json!("previewed")),
        ("/can_start", json!(true)),
        ("/delivery_route", json!("native_git")),
        (
            "/offered_goals",
            json!([
                "verified_candidate",
                "integrated_change",
                "native_lifecycle_complete"
            ]),
        ),
        ("/members/0/member", json!(MEMBER)),
        ("/members/0/goal_supported", json!(true)),
        ("/members/0/prerequisite_inclusion", json!(false)),
        ("/members/0/repository_instructions", json!(id('b'))),
        ("/members/0/required_checks", json!(["acceptance"])),
        ("/members/0/required_reviews", json!(["maintainer"])),
        ("/parent_budget_id", json!(id('a'))),
    ] {
        assert_eq!(at(ready, pointer), &expected, "{pointer}");
    }
    assert_eq!(at(refused, "/members/0/goal_supported"), false);
    assert_eq!(at(refused, "/refusals"), &json!(["endpoint_unsupported"]));
    assert_eq!(
        at(
            &list("local:viewer", Role::Viewer),
            "/definitions/0/refusals"
        ),
        &json!(["start_not_granted"])
    );

    let start = |request_id: &str, identity: &Value| json!({"command": "start", "request_id": request_id, "definition_identity": identity});
    let (status, unsupported_start, _) = post(
        &controls,
        &start("start-unsupported", at(refused, "/definition_identity")),
        OPERATOR,
    );
    assert_eq!(
        (status, code(&unsupported_start)),
        (409, Some("campaign_endpoint_unsupported"))
    );
    let started_body = start("start-previewed", at(ready, "/definition_identity"));
    let (status, started, _) = post(&controls, &started_body, OPERATOR);
    assert_eq!(
        (
            status,
            at(&started, "/store_version").as_u64(),
            at(&started, "/admission").as_str()
        ),
        (200, Some(1), Some("open"))
    );
    assert_eq!(
        at(&post(&controls, &started_body, OPERATOR).1, "/replayed"),
        true
    );
    let listed = list(OPERATOR, Role::Operator);
    assert_eq!(at(&listed, "/definitions/0/existing/same_selection"), true);
    assert_eq!(
        at(&listed, "/definitions/0/refusals"),
        &json!(["campaign_exists"])
    );
    let (status, again, _) = post(
        &controls,
        &start("start-previewed-again", at(ready, "/definition_identity")),
        OPERATOR,
    );
    assert_eq!((status, code(&again)), (409, Some("campaign_exists")));

    // A definition edited after its preview is a new selection, not a join.
    let mut widened = contract("unsupported", CompletionGoal::VerifiedCandidate);
    widened.revision = 2;
    write_contract(&unsupported, &widened);
    let (status, changed, _) = post(
        &controls,
        &start("start-stale-preview", at(refused, "/definition_identity")),
        OPERATOR,
    );
    assert_eq!(
        (status, code(&changed)),
        (409, Some("campaign_definition_changed"))
    );
}

#[test]
fn routes_admit_only_their_method_query_and_body() {
    let fixture = fixture(json!([grant(OPERATOR, all_commands(), json!([]))]));
    let inbox = format!("{INBOX_ROUTE}?campaign={CAMPAIGN}");
    let repeated = format!("{INBOX_ROUTE}?campaign={CAMPAIGN}&campaign=x");
    let queried = format!("{DEFINITIONS_ROUTE}?all=1");
    let absent = format!("{INBOX_ROUTE}?campaign=absent");
    let pause = admission_command("pause", "route-check-0001", version(&fixture.database));
    let mut unknown_field = pause.clone();
    if let Some(body) = unknown_field.as_object_mut() {
        body.insert("database".into(), json!("/tmp/other.sqlite"));
    }
    for (method, target, body, status, expected) in [
        (
            "GET",
            COMMANDS_ROUTE,
            Value::Null,
            405,
            "campaign_controls_method_denied",
        ),
        (
            "POST",
            DEFINITIONS_ROUTE,
            pause.clone(),
            405,
            "campaign_controls_method_denied",
        ),
        (
            "POST",
            inbox.as_str(),
            pause.clone(),
            405,
            "campaign_controls_method_denied",
        ),
        (
            "GET",
            INBOX_ROUTE,
            Value::Null,
            400,
            "campaign_controls_query_invalid",
        ),
        (
            "GET",
            repeated.as_str(),
            Value::Null,
            400,
            "campaign_controls_query_invalid",
        ),
        (
            "GET",
            queried.as_str(),
            Value::Null,
            400,
            "campaign_controls_query_invalid",
        ),
        ("GET", inbox.as_str(), json!({}), 400, "unexpected_body"),
        (
            "POST",
            COMMANDS_ROUTE,
            unknown_field,
            400,
            "campaign_command_invalid",
        ),
        (
            "POST",
            COMMANDS_ROUTE,
            admission_command("pause", "short", 1),
            400,
            "campaign_command_invalid",
        ),
        (
            "GET",
            "/api/implementation/campaigns/other",
            Value::Null,
            404,
            "route_missing",
        ),
        (
            "GET",
            absent.as_str(),
            Value::Null,
            404,
            "campaign_not_found",
        ),
    ] {
        let (actual, value, _) = call(
            &fixture.controls,
            method,
            target,
            &body,
            OPERATOR,
            Role::Operator,
        );
        assert_eq!(
            (actual, code(&value)),
            (status, Some(expected)),
            "{method} {target}"
        );
    }
    let elsewhere = fixture.directory.path().join("elsewhere.sqlite");
    CampaignStore::open(&elsewhere).unwrap_or_else(|error| unreachable!("{error}"));
    assert!(Controls::new(&fixture.profile, Some(&elsewhere)).is_err());
    assert!(Controls::new(&fixture.profile, None).is_err());
}
