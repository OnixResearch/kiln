//! Declared authorship intake, journal replay and review-window export over a
//! disposable local Git repository and SQLite journal. No network is used.
use std::error::Error;
use std::os::unix::fs::PermissionsExt;

use git2::{Oid, Repository, Signature, Time};
use tempfile::TempDir;

use super::commands::{Completion, Pending};
use super::*;

type TestResult<T = ()> = std::result::Result<T, Box<dyn Error>>;
const ALICE: &str = "local:author";
const BOB: &str = "local:reviewer";
const OUTSIDER: &str = "local:outsider";
const MAIN: &str = "refs/heads/main";
const SOURCE: &str = "refs/heads/review/topic";
const OBSERVATION_PROFILE: &str = include_str!("../../../../fixtures/observation/profile-v1.json");
const REVIEW_WINDOW: &str = include_str!("../../../../fixtures/observation/review-window-v1.json");

/// Real provider whose `execute` applies the effect and then reports an
/// ambiguous outcome, leaving the reserved journal entry durable for recovery.
struct AmbiguousProvider<'a>(&'a LocalGitReviewProvider);
impl ChangeReviewPort for AmbiguousProvider<'_> {
    fn info(&self) -> ReviewProviderInfo {
        self.0.info()
    }
    fn inspect(&self, commit: &str, target_ref: &str) -> ReviewResult<CommitInspection> {
        self.0.inspect(commit, target_ref)
    }
    fn compare(&self, request: &CompareRequest) -> ReviewResult<CommitComparison> {
        self.0.compare(request)
    }
    fn execute(&self, request: &ReviewOperationRequest) -> ReviewResult<ReviewEffect> {
        self.0.execute(request)?;
        Err(ReviewError {
            kind: ReviewErrorKind::Uncertain,
            message: "fixture ambiguity".into(),
        })
    }
    fn reconcile(&self, request: &ReviewOperationRequest) -> ReviewResult<ReviewEffect> {
        self.0.reconcile(request)
    }
}

struct Fixture {
    _temp: TempDir,
    profile: Profile,
    workspace: Workspace,
    first: Oid,
    other: Oid,
    other_next: Oid,
}
impl Fixture {
    fn new() -> TestResult<Self> {
        let temp = tempfile::tempdir()?;
        std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700))?;
        let path = temp.path().join("review.git");
        let repository = Repository::init_bare(&path)?;
        let base = commit(&repository, None, &[("file.txt", "one\ntwo\n")])?;
        let first = commit(&repository, Some(base), &[("file.txt", "one\nchanged\n")])?;
        let other = commit(
            &repository,
            Some(base),
            &[("file.txt", "one\ntwo\n"), ("notes.md", "notes\n")],
        )?;
        let other_next = commit(
            &repository,
            Some(other),
            &[("file.txt", "one\ntwo\n"), ("notes.md", "more notes\n")],
        )?;
        repository.reference(MAIN, base, false, "fixture")?;
        repository.reference(SOURCE, other, false, "fixture")?;
        let canonical = path.canonicalize()?;
        let identity =
            kiln_codec::identity_from_bytes(canonical.to_str().ok_or("path encoding")?.as_bytes())?;
        let profile: Profile = serde_json::from_value(json!({
            "schema":"kiln.code-review-profile.v1",
            "database":temp.path().join("review.sqlite"),
            "project":"project",
            "provider":{"repository_id":identity.as_str(),"repository_path":path,
                "target_refs":[MAIN],"capabilities":["inspect","compare","register"]},
            "policy":{"id":"review-policy","eligible_reviewers":[BOB],"submitters":[BOB],
                "minimum_approvals":1,"require_ci":true,
                "ownership":{"teams":{},"rules":[{"id":"core","paths":["file.txt"],
                    "owners":[BOB],"teams":[],"target_refs":[]}]}},
            "read_principals":[ALICE,BOB],"write_principals":[ALICE,BOB],
            "signature_name":"Review service","signature_email":"review@example.invalid",
            "intake":{"default_target_ref":MAIN,"mappings":[]}
        }))?;
        let workspace = Workspace::new(Some(profile.clone()))?;
        Ok(Self {
            _temp: temp,
            profile,
            workspace,
            first,
            other,
            other_next,
        })
    }
    fn call(
        &self,
        method: &str,
        target: &str,
        body: &[u8],
        actor: &str,
        now_ms: i64,
    ) -> ApiResponse {
        self.workspace.response(
            method,
            target,
            body,
            RequestContext {
                role: Role::Viewer,
                principal: actor,
                now_ms,
                ci: &[],
            },
        )
    }
    fn post(&self, request: &Value, actor: &str, now_ms: i64) -> TestResult<ApiResponse> {
        Ok(self.call(
            "POST",
            "/api/code-review/commands",
            &serde_json::to_vec(request)?,
            actor,
            now_ms,
        ))
    }
    fn get(&self, target: &str, actor: &str) -> ApiResponse {
        self.call("GET", target, &[], actor, 5000)
    }
    fn detail(&self, id: &str) -> TestResult<Value> {
        let detail = accepted(self.get(&format!("/api/code-review/changes/{id}"), ALICE))?;
        Ok(field(&detail, "/change")?.clone())
    }
    fn document(&self) -> TestResult<Option<Vec<u8>>> {
        use rusqlite::OptionalExtension as _;
        let connection = rusqlite::Connection::open(&self.profile.database)?;
        Ok(connection
            .query_row("SELECT document FROM review_state WHERE id=1", [], |row| {
                row.get(0)
            })
            .optional()?)
    }
    fn references(&self) -> TestResult<Vec<(String, Option<Oid>)>> {
        let repository = Repository::open_bare(&self.profile.provider.repository_path)?;
        let mut references = Vec::new();
        for reference in repository.references()? {
            let reference = reference?;
            references.push((
                reference.name().ok_or("reference name")?.to_owned(),
                reference.target(),
            ));
        }
        Ok(references)
    }
    /// Run one command through the real shell path with an ambiguous provider
    /// outcome, so the reserved `Pending` journal stays durable.
    fn ambiguous(&self, request: &Value, actor: &str, now_ms: i64) -> TestResult<ApiResponse> {
        let request: Request = serde_json::from_value(request.clone())?;
        let _gate = self.workspace.gate.lock().map_err(|_| "gate")?;
        let binding = self
            .profile
            .binding()
            .map_err(|error| format!("{error:?}"))?;
        let mut store = store::Store::open(&self.profile.database, &binding)?;
        let mut state: State = store.load()?;
        let real = LocalGitReviewProvider::open(self.profile.provider.clone())
            .map_err(|error| error.to_string())?;
        let provider = AmbiguousProvider(&real);
        let context = ProviderContext {
            profile: &self.profile,
            provider: &provider,
            binding: &binding,
        };
        let request_context = RequestContext {
            role: Role::Viewer,
            principal: actor,
            now_ms,
            ci: &[],
        };
        self.workspace
            .command(&context, &mut store, &mut state, request, &request_context)
            .map_err(|error| format!("{error:?}").into())
    }
    fn pending(&self) -> TestResult<Vec<Value>> {
        let binding = self
            .profile
            .binding()
            .map_err(|error| format!("{error:?}"))?;
        let store = store::Store::open(&self.profile.database, &binding)?;
        Ok(store
            .pending()?
            .into_iter()
            .filter_map(|journal| journal.pending)
            .collect())
    }
}
fn commit(repository: &Repository, parent: Option<Oid>, files: &[(&str, &str)]) -> TestResult<Oid> {
    let mut builder = repository.treebuilder(None)?;
    for (path, content) in files {
        builder.insert(*path, repository.blob(content.as_bytes())?, 0o100644)?;
    }
    let tree = repository.find_tree(builder.write()?)?;
    let signature = Signature::new(
        "Fixture",
        "fixture@example.invalid",
        &Time::new(1_700_000_000, 0),
    )?;
    let parent = parent.map(|id| repository.find_commit(id)).transpose()?;
    let parents: Vec<_> = parent.iter().collect();
    Ok(repository.commit(None, &signature, &signature, "fixture", &tree, &parents)?)
}
fn accepted(response: ApiResponse) -> TestResult<Value> {
    assert_eq!(
        response.status,
        200,
        "{}",
        String::from_utf8_lossy(&response.body)
    );
    Ok(serde_json::from_slice(&response.body)?)
}
fn rejected(response: &ApiResponse, status: u16, code: &str) -> TestResult {
    let body: Value = serde_json::from_slice(&response.body)?;
    assert_eq!(
        (response.status, field(&body, "/code")?.as_str()),
        (status, Some(code)),
        "{body}"
    );
    Ok(())
}
fn field<'a>(value: &'a Value, path: &str) -> TestResult<&'a Value> {
    value
        .pointer(path)
        .ok_or_else(|| format!("missing {path}: {value}").into())
}
fn text(value: &Value, path: &str) -> TestResult<String> {
    Ok(field(value, path)?.as_str().ok_or("not text")?.to_owned())
}
fn create(request_id: &str, commit: Oid, authorship: Option<Value>) -> Value {
    let mut action = json!({"type":"create","commit":commit.to_string(),"target_ref":MAIN,
        "title":"Declared patch","description":"","wip":false});
    if let (Some(authorship), Some(map)) = (authorship, action.as_object_mut()) {
        map.insert("authorship".into(), authorship);
    }
    json!({"request_id":request_id,"change_id":null,"expected_version":null,"action":action})
}
fn contribute(request_id: &str, authorship: Option<Value>) -> Value {
    let mut action = json!({"type":"contribute","source_ref":SOURCE,"expected_commit":null,
        "title":"Branch contribution","description":"","wip":false});
    if let (Some(authorship), Some(map)) = (authorship, action.as_object_mut()) {
        map.insert("authorship".into(), authorship);
    }
    json!({"request_id":request_id,"change_id":null,"expected_version":null,"action":action})
}
fn update(request_id: &str, change: &Value, action: Value) -> TestResult<Value> {
    Ok(
        json!({"request_id":request_id,"change_id":field(change,"/id")?,
        "expected_version":field(change,"/version")?,"action":action}),
    )
}
fn agent() -> Value {
    json!({"kind":"agent","harness":"omp","model":"claude"})
}
fn result_change(response: ApiResponse) -> TestResult<String> {
    text(&accepted(response)?, "/change/id")
}

// r[verify kiln.review.authorship.scenario.declared]
// r[verify kiln.review.authorship]
#[test]
fn declared_authorship_is_journaled_replayed_and_shown() -> TestResult {
    let fixture = Fixture::new()?;
    let request = create("agent-create", fixture.first, Some(agent()));
    let ambiguous = fixture.ambiguous(&request, ALICE, 1000)?;
    rejected(&ambiguous, 503, "review_operation_uncertain")?;
    let pending = fixture.pending()?;
    assert_eq!(pending.len(), 1);
    let journaled = pending.first().ok_or("pending journal")?;
    assert_eq!(
        field(journaled, "/completion/command/revision/authorship")?,
        &agent()
    );
    let decoded: Pending = serde_json::from_value(journaled.clone())?;
    let Completion::Register {
        command: domain::Command::Create { revision, .. },
        ..
    } = decoded.completion
    else {
        return Err("register create completion expected".into());
    };
    assert_eq!(
        revision.authorship,
        domain::Authorship::Agent {
            harness: "omp".into(),
            model: Some("claude".into())
        }
    );
    // Recovery replays the durable journal through the real provider.
    let summary = accepted(fixture.get("/api/code-review", ALICE))?;
    assert!(fixture.pending()?.is_empty());
    assert_eq!(field(&summary, "/changes/0/authorship")?, "agent");
    let id = text(&summary, "/changes/0/id")?;
    let change = fixture.detail(&id)?;
    assert_eq!(field(&change, "/authorship")?, "agent");
    assert_eq!(field(&change, "/revisions/0/authorship")?, &agent());
    // The declaration is part of the request digest: same id, different declaration conflicts.
    let replay = accepted(fixture.post(&request, ALICE, 1000)?)?;
    assert_eq!(field(&replay, "/replayed")?, true);
    rejected(
        &fixture.post(&create("agent-create", fixture.first, None), ALICE, 1000)?,
        409,
        "review_request_conflict",
    )?;

    // Branch intake carries the declaration into both Create and Upload rewrites.
    let created = result_change(fixture.post(
        &contribute("branch-create", Some(json!({"kind":"human"}))),
        ALICE,
        2000,
    )?)?;
    let change = fixture.detail(&created)?;
    assert_eq!(field(&change, "/authorship")?, "human");
    assert_eq!(
        field(&change, "/revisions/0/authorship")?,
        &json!({"kind":"human"})
    );
    Repository::open_bare(&fixture.profile.provider.repository_path)?.reference(
        SOURCE,
        fixture.other_next,
        true,
        "fixture branch update",
    )?;
    let uploaded = result_change(fixture.post(
        &contribute(
            "branch-upload",
            Some(json!({"kind":"agent","harness":"omp"})),
        ),
        ALICE,
        3000,
    )?)?;
    assert_eq!(uploaded, created);
    let change = fixture.detail(&created)?;
    assert_eq!(
        field(&change, "/revisions/1/authorship")?,
        &json!({"kind":"agent","harness":"omp"})
    );
    assert_eq!(field(&change, "/authorship")?, "mixed");
    assert_eq!(
        fixture.other.to_string(),
        text(&change, "/revisions/0/commit")?
    );
    Ok(())
}

// r[verify kiln.review.authorship.scenario.legacy]
#[test]
fn legacy_requests_and_journals_replay_as_undeclared() -> TestResult {
    // Literal pre-authorship request bytes keep their exact digest.
    for literal in [
        r#"{"request_id":"r1","change_id":null,"expected_version":null,"action":{"type":"create","commit":"c","target_ref":"refs/heads/main","title":"T","description":"D","wip":false}}"#,
        r#"{"request_id":"r2","change_id":"x","expected_version":3,"action":{"type":"upload","commit":"c"}}"#,
        r#"{"request_id":"r3","change_id":null,"expected_version":null,"action":{"type":"contribute","source_ref":"refs/heads/review/a","expected_commit":null,"title":"T","description":"D","wip":true}}"#,
    ] {
        let request: Request = serde_json::from_str(literal)?;
        let (Action::Create { authorship, .. }
        | Action::Upload { authorship, .. }
        | Action::Contribute { authorship, .. }) = &request.action
        else {
            return Err("intake action expected".into());
        };
        assert!(authorship.is_undeclared());
        assert_eq!(
            hash(&request).map_err(|error| format!("{error:?}"))?,
            blake3::hash(literal.as_bytes()).to_hex().to_string()
        );
    }
    let revision = r#"{"id":"r","number":1,"commit":"c","base":"b","files":["f"],"author":"a","created_ms":1}"#;
    let decoded: domain::Revision = serde_json::from_str(revision)?;
    assert!(decoded.authorship.is_undeclared());
    assert_eq!(serde_json::to_string(&decoded)?, revision);

    // An undeclared reservation journals and replays exactly as before the field existed.
    let fixture = Fixture::new()?;
    let ambiguous = fixture.ambiguous(&create("legacy", fixture.first, None), ALICE, 1000)?;
    rejected(&ambiguous, 503, "review_operation_uncertain")?;
    let pending = fixture.pending()?;
    let journaled = pending.first().ok_or("pending journal")?;
    assert!(!journaled.to_string().contains("authorship"));
    let decoded: Pending = serde_json::from_value(journaled.clone())?;
    assert_eq!(&serde_json::to_value(&decoded)?, journaled);
    let summary = accepted(fixture.get("/api/code-review", ALICE))?;
    assert_eq!(field(&summary, "/changes/0/authorship")?, "undeclared");
    let document = fixture.document()?.ok_or("state document")?;
    assert!(!String::from_utf8_lossy(&document).contains("authorship"));
    let state: State = serde_json::from_slice(&document)?;
    assert_eq!(
        serde_json::to_value(&state)?,
        serde_json::from_slice::<Value>(&document)?
    );
    let change = state.review.changes.values().next().ok_or("change")?;
    assert!(
        change
            .revisions
            .iter()
            .all(|revision| revision.authorship.is_undeclared())
    );
    let detail = fixture.detail(&change.id)?;
    assert_eq!(field(&detail, "/authorship")?, "undeclared");
    assert!(detail.pointer("/revisions/0/authorship").is_none());
    Ok(())
}

// r[verify kiln.review.authorship.scenario.invalid]
#[test]
fn invalid_authorship_is_rejected_before_effects() -> TestResult {
    let fixture = Fixture::new()?;
    accepted(fixture.get("/api/code-review", ALICE))?;
    let oversized = "h".repeat(domain::MAX_AUTHORSHIP_TEXT_BYTES + 1);
    let cases = [
        (json!({"kind":"robot"}), "review_command_invalid"),
        (
            json!({"kind":"agent","harness":"omp","extra":1}),
            "review_command_invalid",
        ),
        (
            json!({"kind":"human","harness":"omp"}),
            "review_command_invalid",
        ),
        (json!({"kind":"agent"}), "review_command_invalid"),
        (json!("agent"), "review_command_invalid"),
        (
            json!({"kind":"agent","harness":""}),
            "review_authorship_invalid",
        ),
        (
            json!({"kind":"agent","harness":" omp"}),
            "review_authorship_invalid",
        ),
        (
            json!({"kind":"agent","harness":"om\u{7}p"}),
            "review_authorship_invalid",
        ),
        (
            json!({"kind":"agent","harness":oversized}),
            "review_authorship_invalid",
        ),
        (
            json!({"kind":"agent","harness":"omp","model":oversized}),
            "review_authorship_invalid",
        ),
    ];
    let document = fixture.document()?;
    let references = fixture.references()?;
    for (index, (authorship, code)) in cases.iter().enumerate() {
        for request in [
            create(
                &format!("bad-create-{index}"),
                fixture.first,
                Some(authorship.clone()),
            ),
            contribute(&format!("bad-branch-{index}"), Some(authorship.clone())),
        ] {
            rejected(&fixture.post(&request, ALICE, 1000)?, 400, code)?;
        }
    }
    assert_eq!(fixture.document()?, document);
    assert_eq!(fixture.references()?, references);
    assert!(fixture.pending()?.is_empty());

    let bounded = "h".repeat(domain::MAX_AUTHORSHIP_TEXT_BYTES);
    let id = result_change(fixture.post(
        &create(
            "bounded",
            fixture.first,
            Some(json!({"kind":"agent","harness":bounded,"model":bounded})),
        ),
        ALICE,
        1000,
    )?)?;
    let change = fixture.detail(&id)?;
    let document = fixture.document()?;
    let references = fixture.references()?;
    for (index, (authorship, code)) in cases.iter().enumerate() {
        let request = update(
            &format!("bad-upload-{index}"),
            &change,
            json!({"type":"upload","commit":fixture.other.to_string(),"authorship":authorship}),
        )?;
        rejected(&fixture.post(&request, ALICE, 2000)?, 400, code)?;
    }
    assert_eq!(fixture.document()?, document);
    assert_eq!(fixture.references()?, references);
    Ok(())
}

fn window(fixture: &Fixture, query: &str) -> ApiResponse {
    fixture.get(&format!("/api/code-review/review-window{query}"), BOB)
}
fn keys(value: &Value) -> Vec<String> {
    value
        .as_object()
        .map(|map| map.keys().cloned().collect())
        .unwrap_or_default()
}

// r[verify kiln.observe.review.window]
// r[verify kiln.observe.review.window.scenario.violation]
#[test]
fn review_window_export_is_bounded_and_read_only() -> TestResult {
    let fixture = Fixture::new()?;
    let agent_id = result_change(fixture.post(
        &create("agent", fixture.first, Some(agent())),
        ALICE,
        1000,
    )?)?;
    let change = fixture.detail(&agent_id)?;
    let reviewer = update(
        "reviewer",
        &change,
        json!({"type":"set_reviewer","principal":BOB,"role":"reviewer"}),
    )?;
    accepted(fixture.post(&reviewer, ALICE, 1050)?)?;
    let change = fixture.detail(&agent_id)?;
    let anchor = json!({"revision":field(&change,"/revisions/0/id")?,"base_commit":field(&change,"/revisions/0/base")?,
        "file":"file.txt","side":"revision","start_line":2,"end_line":2});
    let thread = update(
        "suggest",
        &change,
        json!({"type":"start_thread","anchor":anchor,"body":"Please rename","blocking":false}),
    )?;
    accepted(fixture.post(&thread, BOB, 1100)?)?;
    let change = fixture.detail(&agent_id)?;
    accepted(fixture.post(
        &update("approve", &change, json!({"type":"vote","value":"approve"}))?,
        BOB,
        1200,
    )?)?;
    let human_id = result_change(fixture.post(
        &create("human", fixture.other, Some(json!({"kind":"human"}))),
        ALICE,
        2000,
    )?)?;

    let document = fixture.document()?;
    let exported = accepted(window(&fixture, "?start_ms=1000&end_ms=3000"))?;
    assert_eq!(
        fixture.document()?,
        document,
        "export must not mutate state"
    );
    let parsed: kiln_observe_core::ReviewWindow = serde_json::from_value(exported.clone())?;
    assert_eq!((parsed.start_ms, parsed.end_ms), (1000, 3000));
    let agent_fact = parsed
        .changes
        .iter()
        .find(|fact| fact.change_id == agent_id)
        .ok_or("agent change exported")?;
    assert_eq!(
        agent_fact,
        &kiln_observe_core::ReviewChangeFact {
            change_id: agent_id.clone(),
            authorship: domain::AuthorshipClass::Agent,
            patch_sets_ms: vec![1000],
            approval: Some(kiln_observe_core::ReviewApprovalFact {
                patch_set: 1,
                approved_ms: 1200
            }),
            suggestions: vec![kiln_observe_core::ReviewSuggestionFact {
                path: "file.txt".into(),
                owner: Some("core".into()),
                created_ms: 1100,
            }],
            merged_ms: None,
        }
    );
    let human_fact = parsed
        .changes
        .iter()
        .find(|fact| fact.change_id == human_id)
        .ok_or("human change exported")?;
    assert_eq!(human_fact.authorship, domain::AuthorshipClass::Human);
    assert!(human_fact.suggestions.is_empty() && human_fact.approval.is_none());
    assert_eq!(parsed.changes.len(), 2);

    // Same document shape the dashboard and HTTPS/iroh snapshots consume.
    let canonical: Value = serde_json::from_str(REVIEW_WINDOW)?;
    assert_eq!(keys(&exported), keys(&canonical));
    assert_eq!(
        keys(field(&exported, "/changes/0")?),
        keys(field(&canonical, "/changes/0")?)
    );
    let observation: Value = serde_json::from_str(OBSERVATION_PROFILE)?;
    let profile: kiln_observe_core::ObservationProfile =
        serde_json::from_value(field(&observation, "/profile")?.clone())?;
    kiln_observe_core::admit_review_window(&profile, &parsed)
        .map_err(|error| format!("{error:?}"))?;
    let canonical_window: kiln_observe_core::ReviewWindow = serde_json::from_str(REVIEW_WINDOW)?;
    kiln_observe_core::admit_review_window(&profile, &canonical_window)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        profile
            .bounds
            .review
            .as_ref()
            .map(|bounds| bounds.max_review_window_ms),
        u64::try_from(MAX_REVIEW_WINDOW_MS).ok()
    );

    // Window membership follows the first patch set; later windows are empty.
    let later: kiln_observe_core::ReviewWindow =
        serde_json::from_value(accepted(window(&fixture, "?start_ms=1500&end_ms=3000"))?)?;
    assert_eq!(
        later
            .changes
            .iter()
            .map(|fact| fact.change_id.as_str())
            .collect::<Vec<_>>(),
        vec![human_id.as_str()]
    );
    let empty = accepted(window(&fixture, "?end_ms=4000&start_ms=3000"))?;
    assert_eq!(field(&empty, "/changes")?, &json!([]));
    let widest = format!("?start_ms=0&end_ms={MAX_REVIEW_WINDOW_MS}");
    accepted(window(&fixture, &widest))?;

    let over_span = format!("?start_ms=0&end_ms={}", MAX_REVIEW_WINDOW_MS + 1);
    for (query, code) in [
        ("", "review_window_required"),
        ("?start_ms=1000", "review_window_required"),
        ("?end_ms=3000", "review_window_required"),
        ("?start_ms=abc&end_ms=3000", "review_query_invalid"),
        ("?start_ms=1000&end_ms=1e3", "review_query_invalid"),
        ("?start_ms=1000&end_ms=3000&q=x", "review_query_invalid"),
        (
            "?start_ms=1000&end_ms=3000&start_ms=1",
            "review_query_invalid",
        ),
        ("?start_ms=3000&end_ms=3000", "review_window_range"),
        ("?start_ms=3000&end_ms=1000", "review_window_range"),
        (over_span.as_str(), "review_window_span"),
        (
            "?start_ms=-9223372036854775808&end_ms=9223372036854775807",
            "review_window_span",
        ),
    ] {
        rejected(&window(&fixture, query), 400, code)?;
    }
    let target = "/api/code-review/review-window?start_ms=1000&end_ms=3000";
    rejected(&fixture.get(target, OUTSIDER), 403, "review_read_denied")?;
    rejected(
        &fixture.call("POST", target, b"{}", BOB, 5000),
        405,
        "review_method_denied",
    )?;
    rejected(
        &fixture.call("GET", target, b"{}", BOB, 5000),
        400,
        "unexpected_body",
    )?;
    let worker = fixture.workspace.response(
        "GET",
        target,
        &[],
        RequestContext {
            role: Role::Worker,
            principal: BOB,
            now_ms: 5000,
            ci: &[],
        },
    );
    rejected(&worker, 403, "review_principal_denied")?;
    assert_eq!(fixture.document()?, document);
    Ok(())
}
