//! Real disposable Git + SQLite integration; no network, installed keys or real refs.
// r[verify kiln.review.workflow]
// r[verify kiln.review.authority]
use std::collections::BTreeSet;
use std::error::Error;
use std::os::unix::fs::{PermissionsExt, symlink};

use git2::{Oid, Repository, Signature, Time};
use kiln_adapter_git::change_review::LocalGitReviewProfile;
use kiln_core::change_review::{CiFact, Policy};
use kiln_provider_api::change_review::ReviewCapability;
use kiln_web_service::code_review::{Profile, Workspace};
use kiln_web_service::engine::{ApiResponse, Role};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use tempfile::TempDir;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const ALICE: &str = "local:author";
const BOB: &str = "local:reviewer";
const CAROL: &str = "local:submitter";
const MAIN: &str = "refs/heads/main";
const RELEASE: &str = "refs/heads/release";

struct Fixture {
    _temp: TempDir,
    profile: Profile,
    base: String,
    first: String,
    second: String,
}
impl Fixture {
    fn new() -> Result<Self> {
        let temp = tempfile::tempdir()?;
        std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700))?;
        let path = temp.path().join("review.git");
        let repository = Repository::init_bare(&path)?;
        let base = commit(&repository, None, "base", &[("file.txt", "one\ntwo\n")])?;
        let first = commit(
            &repository,
            Some(base),
            "first",
            &[("file.txt", "one\nchanged\n")],
        )?;
        let second = commit(
            &repository,
            Some(first),
            "second",
            &[("file.txt", "one\nchanged again\n")],
        )?;
        repository.reference(MAIN, base, false, "fixture")?;
        repository.reference(RELEASE, base, false, "fixture")?;
        let canonical = path.canonicalize()?;
        let text = canonical.to_str().ok_or("non-UTF8 fixture")?;
        let repository_id = kiln_codec::identity_from_bytes(text.as_bytes())?
            .as_str()
            .to_owned();
        let profile = Profile {
            schema: "kiln.code-review-profile.v1".into(),
            database: temp.path().join("review.sqlite"),
            project: "project".into(),
            provider: LocalGitReviewProfile {
                repository_id,
                repository_path: path,
                target_refs: vec![MAIN.into(), RELEASE.into()],
                capabilities: vec![
                    ReviewCapability::Inspect,
                    ReviewCapability::Compare,
                    ReviewCapability::Register,
                    ReviewCapability::Submit,
                    ReviewCapability::Rebase,
                    ReviewCapability::Revert,
                    ReviewCapability::CherryPick,
                ],
            },
            policy: Policy {
                id: "review-policy".into(),
                eligible_reviewers: BTreeSet::from([BOB.into()]),
                submitters: BTreeSet::from([CAROL.into()]),
                minimum_approvals: 1,
                require_ci: true,
                ownership: Default::default(),
            },
            read_principals: vec![ALICE.into(), BOB.into(), CAROL.into()],
            write_principals: vec![ALICE.into(), BOB.into(), CAROL.into()],
            signature_name: "Review service".into(),
            signature_email: "review@example.invalid".into(),
            intake: None,
            delivery: None,
            queue: None,
        };
        Ok(Self {
            _temp: temp,
            profile,
            base: base.to_string(),
            first: first.to_string(),
            second: second.to_string(),
        })
    }
    fn workspace(&self) -> Result<Workspace> {
        Ok(Workspace::new(Some(self.profile.clone()))?)
    }
    fn ci(&self, revision: &str, ordinal: u64, success: bool) -> CiFact {
        CiFact {
            project: self.profile.project.clone(),
            repository: self.profile.provider.repository_id.clone(),
            revision: revision.into(),
            run_id: format!("run-{ordinal}"),
            ordinal,
            success,
            full_run: true,
            clean: true,
        }
    }
    fn target(&self, target: &str) -> Result<String> {
        Ok(
            Repository::open_bare(&self.profile.provider.repository_path)?
                .find_reference(target)?
                .target()
                .ok_or("target not direct")?
                .to_string(),
        )
    }
}
fn commit(
    repository: &Repository,
    parent: Option<Oid>,
    message: &str,
    files: &[(&str, &str)],
) -> Result<Oid> {
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
    Ok(repository.commit(None, &signature, &signature, message, &tree, &parents)?)
}
fn value(response: &ApiResponse) -> Result<Value> {
    Ok(serde_json::from_slice(&response.body)?)
}
fn field<'a>(value: &'a Value, path: &str) -> Result<&'a Value> {
    value
        .pointer(path)
        .ok_or_else(|| format!("missing {path}: {value}").into())
}
fn text(value: &Value, path: &str) -> Result<String> {
    Ok(field(value, path)?.as_str().ok_or("not text")?.to_owned())
}
fn accepted(response: ApiResponse) -> Result<Value> {
    assert_eq!(
        response.status,
        200,
        "{}",
        String::from_utf8_lossy(&response.body)
    );
    value(&response)
}
fn get(workspace: &Workspace, path: &str, actor: &str, ci: &[CiFact]) -> ApiResponse {
    workspace.response(
        "GET",
        path,
        &[],
        kiln_web_service::code_review::RequestContext {
            role: Role::Viewer,
            principal: actor,
            now_ms: 1000,
            ci,
        },
    )
}
fn post(workspace: &Workspace, request: &Value, actor: &str, ci: &[CiFact]) -> Result<ApiResponse> {
    Ok(workspace.response(
        "POST",
        "/api/code-review/commands",
        &serde_json::to_vec(request)?,
        kiln_web_service::code_review::RequestContext {
            role: Role::Viewer,
            principal: actor,
            now_ms: 1000,
            ci,
        },
    ))
}
fn global(request: &str, action: Value) -> Value {
    json!({"request_id":request,"change_id":null,"expected_version":null,"action":action})
}
fn update(request: &str, change: &Value, action: Value) -> Result<Value> {
    Ok(
        json!({"request_id":request,"change_id":field(change,"/id")?,"expected_version":field(change,"/version")?,"action":action}),
    )
}
fn create(workspace: &Workspace, fixture: &Fixture, request: &str) -> Result<Value> {
    let result = accepted(post(
        workspace,
        &global(
            request,
            json!({"type":"create","commit":fixture.first,"target_ref":MAIN,"title":"Review this patch","description":"Exact source","wip":false}),
        ),
        ALICE,
        &[],
    )?)?;
    Ok(field(
        &detail(workspace, field(&result, "/change")?, ALICE, &[])?,
        "/change",
    )?
    .clone())
}
fn detail(workspace: &Workspace, change: &Value, actor: &str, ci: &[CiFact]) -> Result<Value> {
    accepted(get(
        workspace,
        &format!("/api/code-review/changes/{}", text(change, "/id")?),
        actor,
        ci,
    ))
}
fn change_after(
    workspace: &Workspace,
    request: &str,
    change: &Value,
    action: Value,
    actor: &str,
    ci: &[CiFact],
) -> Result<Value> {
    let result = accepted(post(
        workspace,
        &update(request, change, action)?,
        actor,
        ci,
    )?)?;
    Ok(field(
        &detail(workspace, field(&result, "/change")?, actor, ci)?,
        "/change",
    )?
    .clone())
}
fn approved(workspace: &Workspace, fixture: &Fixture) -> Result<Value> {
    let change = create(workspace, fixture, "create")?;
    let change = change_after(
        workspace,
        "reviewer",
        &change,
        json!({"type":"set_reviewer","principal":BOB,"role":"reviewer"}),
        ALICE,
        &[],
    )?;
    change_after(
        workspace,
        "approval",
        &change,
        json!({"type":"vote","value":"approve"}),
        BOB,
        &[],
    )
}

#[test]
fn admission_is_effect_free_and_scopes_are_independent_from_ci_operator() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    assert!(!fixture.profile.database.exists());
    assert_eq!(
        get(&workspace, "/api/code-review", "local:stranger", &[]).status,
        403
    );
    let body = serde_json::to_vec(&global(
        "create",
        json!({"type":"create","commit":fixture.first,"target_ref":MAIN,"title":"Review","description":"","wip":false}),
    ))?;
    assert_eq!(
        workspace
            .response(
                "POST",
                "/api/code-review/commands",
                &body,
                kiln_web_service::code_review::RequestContext {
                    role: Role::Worker,
                    principal: ALICE,
                    now_ms: 1000,
                    ci: &[],
                }
            )
            .status,
        403
    );
    let created = accepted(workspace.response(
        "POST",
        "/api/code-review/commands",
        &body,
        kiln_web_service::code_review::RequestContext {
            role: Role::Viewer,
            principal: ALICE,
            now_ms: 1000,
            ci: &[],
        },
    ))?;
    let created = detail(&workspace, field(&created, "/change")?, ALICE, &[])?;
    assert_eq!(field(&created, "/change/owner")?, ALICE);
    assert_eq!(fixture.target(MAIN)?, fixture.base);
    let mut readonly = fixture.profile.clone();
    readonly.write_principals.retain(|p| p != ALICE);
    let readonly = Workspace::new(Some(readonly))?;
    assert_eq!(
        readonly
            .response(
                "POST",
                "/api/code-review/commands",
                &body,
                kiln_web_service::code_review::RequestContext {
                    role: Role::Operator,
                    principal: ALICE,
                    now_ms: 1000,
                    ci: &[],
                }
            )
            .status,
        403
    );
    Ok(())
}

#[test]
fn drafts_saved_views_and_notifications_survive_restart_without_cross_principal_disclosure()
-> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let change = create(&workspace, &fixture, "create")?;
    let change = change_after(
        &workspace,
        "reviewer",
        &change,
        json!({"type":"set_reviewer","principal":BOB,"role":"reviewer"}),
        ALICE,
        &[],
    )?;
    let anchor = json!({"revision":field(&change,"/revisions/0/id")?,"base_commit":fixture.base,"file":"file.txt","side":"revision","start_line":2,"end_line":2});
    let invalid = json!({"revision":field(&change,"/revisions/0/id")?,"base_commit":fixture.base,"file":"file.txt","side":"revision","start_line":99,"end_line":99});
    assert_eq!(post(&workspace,&update("invalid-anchor",&change,json!({"type":"save_draft","anchor":invalid,"thread":null,"body":"private","blocking":true}))?,BOB,&[])?.status,400);
    let saved = accepted(post(
        &workspace,
        &update(
            "draft",
            &change,
            json!({"type":"save_draft","anchor":anchor,"thread":null,"body":"private reviewer note","blocking":true}),
        )?,
        BOB,
        &[],
    )?)?;
    let saved = detail(&workspace, field(&saved, "/change")?, BOB, &[])?;
    let change = field(&saved, "/change")?.clone();
    let draft = text(&saved, "/drafts/0/id")?;
    accepted(post(
        &workspace,
        &global(
            "view",
            json!({"type":"save_view","name":"My queue","query":{"kind":"awaiting_review","text":"","status":"open"}}),
        ),
        BOB,
        &[],
    )?)?;
    let author_view = detail(&workspace, &change, ALICE, &[])?;
    assert_eq!(field(&author_view, "/drafts")?, &json!([]));
    let root = accepted(get(&workspace, "/api/code-review", ALICE, &[]))?;
    assert_eq!(field(&root, "/saved_views")?, &json!([]));
    let request = update(
        "publish",
        &change,
        json!({"type":"publish","draft_ids":[draft],"vote":"needs_revision"}),
    )?;
    drop(workspace);
    let restarted = fixture.workspace()?;
    let bob_root = accepted(get(
        &restarted,
        "/api/code-review?view=awaiting_review",
        BOB,
        &[],
    ))?;
    assert_eq!(field(&bob_root, "/saved_views/0/name")?, "My queue");
    let published_receipt = accepted(post(&restarted, &request, BOB, &[])?)?;
    let published = detail(&restarted, field(&published_receipt, "/change")?, BOB, &[])?;
    assert_eq!(
        field(&published, "/change/threads/0/comments/0/body")?,
        "private reviewer note"
    );
    assert_eq!(
        field(&published, "/change/votes/0/value")?,
        "needs_revision"
    );
    assert_eq!(field(&published, "/drafts")?, &json!([]));
    let author_root = accepted(get(&restarted, "/api/code-review", ALICE, &[]))?;
    assert_eq!(field(&author_root, "/notifications/0/actor")?, BOB);
    let replayed = accepted(post(&restarted, &request, BOB, &[])?)?;
    assert_eq!(field(&replayed, "/replayed")?, true);
    assert_eq!(
        field(&replayed, "/change/version")?,
        field(&published_receipt, "/change/version")?
    );
    let replay_snapshot = detail(&restarted, field(&replayed, "/change")?, BOB, &[])?;
    assert_eq!(
        field(&replay_snapshot, "/change/threads")?,
        field(&published, "/change/threads")?
    );
    assert_eq!(post(&restarted, &request, ALICE, &[])?.status, 409);
    Ok(())
}

#[test]
fn exact_ci_non_author_approval_policy_and_target_bind_submission() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let change = approved(&workspace, &fixture)?;
    assert_eq!(
        post(
            &workspace,
            &update("no-ci", &change, json!({"type":"prepare_submit"}))?,
            CAROL,
            &[]
        )?
        .status,
        409
    );
    let mut forged = fixture.ci(&fixture.first, 1, true);
    forged.repository = "another-repository".into();
    assert_eq!(
        post(
            &workspace,
            &update("wrong-source", &change, json!({"type":"prepare_submit"}))?,
            CAROL,
            &[forged]
        )?
        .status,
        409
    );
    let success = fixture.ci(&fixture.first, 1, true);
    let failed = fixture.ci(&fixture.first, 2, false);
    assert_eq!(
        post(
            &workspace,
            &update("latest-failed", &change, json!({"type":"prepare_submit"}))?,
            CAROL,
            &[success.clone(), failed]
        )?
        .status,
        409
    );
    let change = change_after(
        &workspace,
        "prepare",
        &change,
        json!({"type":"prepare_submit"}),
        CAROL,
        std::slice::from_ref(&success),
    )?;
    let candidate = text(&change, "/candidate/id")?;
    let mut altered = fixture.profile.clone();
    altered.policy.minimum_approvals = 2;
    altered.policy.eligible_reviewers.insert(CAROL.into());
    let changed_policy = Workspace::new(Some(altered))?;
    assert_eq!(
        post(
            &changed_policy,
            &update(
                "policy-stale",
                &change,
                json!({"type":"submit","candidate_id":candidate})
            )?,
            CAROL,
            std::slice::from_ref(&success)
        )?
        .status,
        409
    );
    assert_eq!(fixture.target(MAIN)?, fixture.base);
    let submitted = change_after(
        &workspace,
        "submit",
        &change,
        json!({"type":"submit","candidate_id":candidate}),
        CAROL,
        std::slice::from_ref(&success),
    )?;
    assert_eq!(field(&submitted, "/state")?, "submitted");
    assert_eq!(fixture.target(MAIN)?, fixture.first);
    let restarted = fixture.workspace()?;
    assert_eq!(
        field(
            &detail(&restarted, &submitted, BOB, &[success])?,
            "/change/state"
        )?,
        "submitted"
    );
    Ok(())
}

#[test]
fn stale_target_requires_new_revision_and_author_actions_never_bypass_review() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let change = approved(&workspace, &fixture)?;
    let ci = fixture.ci(&fixture.first, 1, true);
    let change = change_after(
        &workspace,
        "prepare",
        &change,
        json!({"type":"prepare_submit"}),
        CAROL,
        std::slice::from_ref(&ci),
    )?;
    let repository = Repository::open_bare(&fixture.profile.provider.repository_path)?;
    let base = Oid::from_str(&fixture.base)?;
    let advanced = commit(
        &repository,
        Some(base),
        "target advanced",
        &[("file.txt", "one\ntwo\n"), ("other.txt", "target\n")],
    )?;
    repository.reference(MAIN, advanced, true, "external target advance")?;
    assert_eq!(
        post(
            &workspace,
            &update(
                "stale-submit",
                &change,
                json!({"type":"submit","candidate_id":field(&change,"/candidate/id")?})
            )?,
            CAROL,
            &[ci]
        )?
        .status,
        409
    );
    assert_eq!(
        post(
            &workspace,
            &update(
                "old-confirmation",
                &change,
                json!({"type":"rebase","expected_target":fixture.base})
            )?,
            ALICE,
            &[]
        )?
        .status,
        409
    );
    let rebased = change_after(
        &workspace,
        "rebase",
        &change,
        json!({"type":"rebase","expected_target":advanced.to_string()}),
        ALICE,
        &[],
    )?;
    assert_eq!(
        field(&rebased, "/revisions/1/base")?,
        &json!(advanced.to_string())
    );
    assert_eq!(field(&rebased, "/candidate")?, &Value::Null);
    assert_eq!(fixture.target(MAIN)?, advanced.to_string());
    let abandoned = change_after(
        &workspace,
        "abandon",
        &rebased,
        json!({"type":"abandon"}),
        ALICE,
        &[],
    )?;
    assert_eq!(field(&abandoned, "/state")?, "abandoned");
    let restored = change_after(
        &workspace,
        "restore",
        &abandoned,
        json!({"type":"restore"}),
        ALICE,
        &[],
    )?;
    let picked_result = accepted(post(
        &workspace,
        &update(
            "pick",
            &restored,
            json!({"type":"cherry_pick","target_ref":RELEASE,"expected_target":fixture.base,"title":"Backport"}),
        )?,
        ALICE,
        &[],
    )?)?;
    let picked_result = detail(&workspace, field(&picked_result, "/change")?, ALICE, &[])?;
    assert_eq!(field(&picked_result, "/change/target_ref")?, RELEASE);
    assert_eq!(field(&picked_result, "/change/wip")?, true);
    assert_eq!(fixture.target(RELEASE)?, fixture.base);
    Ok(())
}

#[test]
fn historical_exact_diff_drafts_publish_atomically_and_stale_versions_conflict() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let change = create(&workspace, &fixture, "create")?;
    let old_revision = text(&change, "/revisions/0/id")?;
    let saved = accepted(post(
        &workspace,
        &update(
            "draft",
            &change,
            json!({"type":"save_draft","anchor":{"revision":old_revision,"base_commit":fixture.base,"file":"file.txt","side":"revision","start_line":2,"end_line":2},"thread":null,"body":"Historical feedback","blocking":true}),
        )?,
        BOB,
        &[],
    )?)?;
    let saved = detail(&workspace, field(&saved, "/change")?, BOB, &[])?;
    let draft = text(&saved, "/drafts/0/id")?;
    let change = field(&saved, "/change")?.clone();
    let changed = change_after(
        &workspace,
        "upload",
        &change,
        json!({"type":"upload","commit":fixture.second}),
        ALICE,
        &[],
    )?;
    assert_eq!(
        post(
            &workspace,
            &update(
                "stale-edit",
                &change,
                json!({"type":"edit","title":"Lost update","description":""})
            )?,
            ALICE,
            &[]
        )?
        .status,
        409
    );
    let exact = accepted(get(
        &workspace,
        &format!(
            "/api/code-review/changes/{}/diff?revision={}&base={}",
            text(&changed, "/id")?,
            text(&changed, "/revisions/1/id")?,
            old_revision
        ),
        BOB,
        &[],
    ))?;
    assert_eq!(field(&exact, "/old_commit")?, &json!(fixture.first));
    assert_eq!(field(&exact, "/new_commit")?, &json!(fixture.second));
    let partial = update(
        "atomic-fail",
        &changed,
        json!({"type":"publish","draft_ids":[draft,"missing-draft"],"vote":null}),
    )?;
    assert_eq!(post(&workspace, &partial, BOB, &[])?.status, 409);
    let unchanged = detail(&workspace, &changed, BOB, &[])?;
    assert_eq!(field(&unchanged, "/change/threads")?, &json!([]));
    assert_eq!(field(&unchanged, "/drafts/0/id")?, &json!(draft));
    let published = change_after(
        &workspace,
        "publish",
        &changed,
        json!({"type":"publish","draft_ids":[draft],"vote":null}),
        BOB,
        &[],
    )?;
    assert_eq!(
        field(&published, "/threads/0/anchor/revision")?,
        &json!(old_revision)
    );
    assert_eq!(
        field(&published, "/threads/0/anchor/base_commit")?,
        &json!(fixture.base)
    );
    Ok(())
}

#[test]
fn replay_payload_principal_and_safe_file_policy_fail_closed() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let change = create(&workspace, &fixture, "create")?;
    let request = update(
        "metadata",
        &change,
        json!({"type":"edit","title":"Updated","description":""}),
    )?;
    accepted(post(&workspace, &request, ALICE, &[])?)?;
    let conflicting = update(
        "metadata",
        &change,
        json!({"type":"edit","title":"Different payload","description":""}),
    )?;
    assert_eq!(post(&workspace, &conflicting, ALICE, &[])?.status, 409);
    assert_eq!(post(&workspace, &request, BOB, &[])?.status, 409);
    assert_eq!(
        get(&workspace, "/api/code-review?q=%GG", ALICE, &[]).status,
        400
    );
    let mut body = request.clone();
    body.as_object_mut()
        .ok_or("request object")?
        .insert("ci_success".into(), json!(true));
    assert_eq!(post(&workspace, &body, ALICE, &[])?.status, 400);
    let mut unsafe_profile = fixture.profile.clone();
    unsafe_profile.database = fixture.profile.database.with_file_name("symlink.sqlite");
    symlink(&fixture.profile.database, &unsafe_profile.database)?;
    assert!(Workspace::new(Some(unsafe_profile)).is_err());
    Ok(())
}

#[test]
fn restart_reconciles_provider_applied_upload_from_reserved_journal_exactly_once() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let change = create(&workspace, &fixture, "create")?;
    let connection = Connection::open(&fixture.profile.database)?;
    let before: Vec<u8> =
        connection.query_row("SELECT document FROM review_state WHERE id=1", [], |r| {
            r.get(0)
        })?;
    let request = update(
        "upload",
        &change,
        json!({"type":"upload","commit":fixture.second}),
    )?;
    let uploaded = accepted(post(&workspace, &request, ALICE, &[])?)?;
    let operation_id = text(&uploaded, "/operation/operation_id")?;
    let uploaded_snapshot = detail(&workspace, field(&uploaded, "/change")?, ALICE, &[])?;
    let revision = field(&uploaded_snapshot, "/change/revisions/1")?.clone();
    let id = text(&change, "/id")?;
    let journal_bytes: Vec<u8> = connection.query_row(
        "SELECT document FROM review_requests WHERE principal=?1 AND request_id='upload'",
        [ALICE],
        |r| r.get(0),
    )?;
    let mut journal: Value = serde_json::from_slice(&journal_bytes)?;
    let digest = text(&journal, "/digest")?;
    let event = blake3::hash(&serde_json::to_vec(&json!([
        "event", ALICE, "upload", digest
    ]))?)
    .to_hex()
    .to_string();
    let mut policy = serde_json::to_value(&fixture.profile.policy)?;
    let policy_id = blake3::hash(&serde_json::to_vec(&fixture.profile.policy)?)
        .to_hex()
        .to_string();
    policy
        .as_object_mut()
        .ok_or("policy object")?
        .insert("id".into(), json!(policy_id));
    let pending = json!({
        "request":{"version":1,"operation_id":operation_id,"repository_id":fixture.profile.provider.repository_id,"target_ref":MAIN,"operation":{"kind":"register","commit":fixture.second},"signature":{"name":fixture.profile.signature_name,"email":fixture.profile.signature_email,"time_seconds":1}},
        "actor":ALICE,
        "facts":{"now_ms":1000,"event_id":event,"policy":policy,"source":{"project":fixture.profile.project,"repository":fixture.profile.provider.repository_id,"target_ref":MAIN,"commit":fixture.second,"base":fixture.base,"current_target":fixture.base,"immutable":true,"target_contains_commit":false,"target_is_ancestor":true,"target_ancestors":[]},"ci":[],"anchors":[]},
        "completion":{"type":"register","command":{"op":"upload","change":id,"expected_revision":field(&change,"/revisions/0/id")?,"revision":revision},"change":id},
        "origin_change":id
    });
    let map = journal.as_object_mut().ok_or("journal object")?;
    map.insert("pending".into(), pending);
    map.insert("response".into(), Value::Null);
    // Simulate the real crash boundary: provider effect is durable, SQLite still
    // has the reserved exact request and pre-effect aggregate document.
    connection.execute("UPDATE review_state SET document=?1 WHERE id=1", [before])?;
    connection.execute("UPDATE review_requests SET pending=1,document=?1 WHERE principal=?2 AND request_id='upload'",params![serde_json::to_vec(&journal)?,ALICE])?;
    drop(connection);
    drop(workspace);
    let repository = Repository::open_bare(&fixture.profile.provider.repository_path)?;
    let provider_request_ref = format!("refs/kiln/review-operations/{operation_id}/request");
    let saved_request = repository
        .find_reference(&provider_request_ref)?
        .target()
        .ok_or("request ref direct")?;
    repository.find_reference(&provider_request_ref)?.delete()?;
    let restarted = fixture.workspace()?;
    let root = accepted(get(&restarted, "/api/code-review", ALICE, &[]))?;
    assert_eq!(field(&root, "/changes/0/id")?, field(&change, "/id")?);
    let ambiguous = detail(&restarted, &change, ALICE, &[])?;
    assert!(
        field(&ambiguous, "/operations")?
            .as_array()
            .is_some_and(|ops| ops
                .iter()
                .any(|op| op.get("state").and_then(Value::as_str) == Some("uncertain")))
    );
    assert_eq!(
        post(
            &restarted,
            &update(
                "reserved-edit",
                &change,
                json!({"type":"edit","title":"Must not mutate reserved change","description":""})
            )?,
            ALICE,
            &[]
        )?
        .status,
        409
    );
    let unrelated = create(&restarted, &fixture, "unrelated-create")?;
    let unrelated = change_after(
        &restarted,
        "unrelated-edit",
        &unrelated,
        json!({"type":"edit","title":"Unrelated history remains usable","description":""}),
        ALICE,
        &[],
    )?;
    assert_eq!(
        field(&unrelated, "/title")?,
        "Unrelated history remains usable"
    );
    repository.reference(
        &provider_request_ref,
        saved_request,
        false,
        "restore exact request proof",
    )?;
    let recovered = detail(&restarted, &change, ALICE, &[])?;
    assert_eq!(
        field(&recovered, "/change/revisions/1/commit")?,
        &json!(fixture.second)
    );
    assert!(
        field(&recovered, "/change/revisions")?
            .as_array()
            .is_some_and(|revisions| revisions.len() == 2)
    );
    let replayed = accepted(post(&restarted, &request, ALICE, &[])?)?;
    assert_eq!(field(&replayed, "/replayed")?, true);
    assert_eq!(
        field(&replayed, "/operation/operation_id")?,
        &json!(operation_id)
    );
    assert_eq!(fixture.target(MAIN)?, fixture.base);
    Ok(())
}

#[test]
fn blocking_threads_self_votes_and_missing_provider_capabilities_block_effects() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let change = approved(&workspace, &fixture)?;
    assert_eq!(
        post(
            &workspace,
            &update(
                "self-vote",
                &change,
                json!({"type":"vote","value":"approve"})
            )?,
            ALICE,
            &[]
        )?
        .status,
        409
    );
    let change = change_after(
        &workspace,
        "thread",
        &change,
        json!({"type":"start_thread","anchor":{"revision":field(&change,"/revisions/0/id")?,"base_commit":fixture.base,"file":"file.txt","side":"revision","start_line":2,"end_line":2},"body":"Must fix","blocking":true}),
        BOB,
        &[],
    )?;
    let ci = fixture.ci(&fixture.first, 1, true);
    assert_eq!(
        post(
            &workspace,
            &update("blocked", &change, json!({"type":"prepare_submit"}))?,
            CAROL,
            std::slice::from_ref(&ci)
        )?
        .status,
        409
    );
    let thread = text(&change, "/threads/0/id")?;
    let change = change_after(
        &workspace,
        "resolve",
        &change,
        json!({"type":"resolve","thread":thread,"resolved":true}),
        BOB,
        &[],
    )?;
    let change = change_after(
        &workspace,
        "prepare",
        &change,
        json!({"type":"prepare_submit"}),
        CAROL,
        std::slice::from_ref(&ci),
    )?;
    let mut disabled = fixture.profile.clone();
    disabled
        .provider
        .capabilities
        .retain(|cap| *cap != ReviewCapability::Submit);
    let disabled = Workspace::new(Some(disabled))?;
    let request = update(
        "denied-submit",
        &change,
        json!({"type":"submit","candidate_id":field(&change,"/candidate/id")?}),
    )?;
    assert_eq!(
        post(&disabled, &request, CAROL, std::slice::from_ref(&ci))?.status,
        403
    );
    assert_eq!(fixture.target(MAIN)?, fixture.base);
    Ok(())
}

#[test]
fn revert_registers_new_wip_review_without_reverting_submitted_target() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let change = approved(&workspace, &fixture)?;
    let ci = fixture.ci(&fixture.first, 1, true);
    let prepared = change_after(
        &workspace,
        "prepare",
        &change,
        json!({"type":"prepare_submit"}),
        CAROL,
        std::slice::from_ref(&ci),
    )?;
    let submitted = change_after(
        &workspace,
        "submit",
        &prepared,
        json!({"type":"submit","candidate_id":field(&prepared,"/candidate/id")?}),
        CAROL,
        std::slice::from_ref(&ci),
    )?;
    let result = accepted(post(
        &workspace,
        &update(
            "revert",
            &submitted,
            json!({"type":"revert","expected_target":fixture.first,"title":"Revert review"}),
        )?,
        ALICE,
        &[],
    )?)?;
    let result = detail(&workspace, field(&result, "/change")?, ALICE, &[])?;
    assert_eq!(field(&result, "/change/state")?, "open");
    assert_eq!(field(&result, "/change/wip")?, true);
    assert_ne!(field(&result, "/change/id")?, field(&submitted, "/id")?);
    assert_eq!(
        field(&result, "/change/revisions/0/base")?,
        &json!(fixture.first)
    );
    assert_eq!(fixture.target(MAIN)?, fixture.first);
    let original = detail(&workspace, &submitted, ALICE, &[])?;
    assert_eq!(field(&original, "/change/maintenance/0/kind")?, "revert");
    Ok(())
}

#[test]
fn process_writer() -> Result {
    let Some(directory) = std::env::var_os("KILN_REVIEW_PROCESS_FIXTURE") else {
        return Ok(());
    };
    let directory = std::path::PathBuf::from(directory);
    let slot = std::env::var("KILN_REVIEW_PROCESS_SLOT")?;
    let profile = Profile::load(&directory.join("profile.json"))?;
    let request: Value = serde_json::from_slice(&std::fs::read(
        directory.join(format!("request-{slot}.json")),
    )?)?;
    let workspace = Workspace::new(Some(profile))?;
    let response = post(&workspace, &request, ALICE, &[])?;
    std::fs::write(
        directory.join(format!("result-{slot}.json")),
        serde_json::to_vec(&json!({"status":response.status,"body":value(&response)?}))?,
    )?;
    Ok(())
}

#[test]
fn separate_processes_cannot_overwrite_the_same_expected_version() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let change = create(&workspace, &fixture, "create")?;
    let directory = fixture.profile.database.parent().ok_or("fixture parent")?;
    std::fs::write(
        directory.join("profile.json"),
        serde_json::to_vec(&fixture.profile)?,
    )?;
    for slot in ["one", "two"] {
        std::fs::write(
            directory.join(format!("request-{slot}.json")),
            serde_json::to_vec(&update(
                slot,
                &change,
                json!({"type":"edit","title":slot,"description":""}),
            )?)?,
        )?;
    }
    let executable = std::env::current_exe()?;
    let spawn = |slot: &str| {
        std::process::Command::new(&executable)
            .args(["--exact", "process_writer", "--nocapture"])
            .env("KILN_REVIEW_PROCESS_FIXTURE", directory)
            .env("KILN_REVIEW_PROCESS_SLOT", slot)
            .spawn()
    };
    let mut one = spawn("one")?;
    let mut two = spawn("two")?;
    assert!(one.wait()?.success());
    assert!(two.wait()?.success());
    let mut statuses = Vec::new();
    for slot in ["one", "two"] {
        let result: Value = serde_json::from_slice(&std::fs::read(
            directory.join(format!("result-{slot}.json")),
        )?)?;
        statuses.push(field(&result, "/status")?.as_u64().ok_or("status")?);
    }
    assert_eq!(statuses.iter().filter(|status| **status == 200).count(), 1);
    assert!(
        statuses
            .iter()
            .all(|status| matches!(status, 200 | 409 | 503))
    );
    let result = detail(&workspace, &change, ALICE, &[])?;
    assert_eq!(field(&result, "/change/version")?, &json!(2));
    assert!(matches!(
        field(&result, "/change/title")?.as_str(),
        Some("one" | "two")
    ));
    Ok(())
}

#[test]
fn declared_stack_submits_in_dependency_order_with_fresh_exact_target_candidates() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let parent = approved(&workspace, &fixture)?;
    let created = accepted(post(
        &workspace,
        &global(
            "child",
            json!({"type":"create","commit":fixture.second,"target_ref":MAIN,"title":"Dependent patch","description":"","wip":false}),
        ),
        ALICE,
        &[],
    )?)?;
    let created = detail(&workspace, field(&created, "/change")?, ALICE, &[])?;
    let child = field(&created, "/change")?.clone();
    let child = change_after(
        &workspace,
        "child-reviewer",
        &child,
        json!({"type":"set_reviewer","principal":BOB,"role":"reviewer"}),
        ALICE,
        &[],
    )?;
    let child = change_after(
        &workspace,
        "child-vote",
        &child,
        json!({"type":"vote","value":"approve"}),
        BOB,
        &[],
    )?;
    let child = change_after(
        &workspace,
        "child-dependency",
        &child,
        json!({"type":"set_dependencies","dependencies":[{"change":field(&parent,"/id")?,"revision":field(&parent,"/revisions/0/id")?}]}),
        ALICE,
        &[],
    )?;
    assert_eq!(post(&workspace,&update("cycle",&parent,json!({"type":"set_dependencies","dependencies":[{"change":field(&child,"/id")?,"revision":field(&child,"/revisions/0/id")?}]}))?,ALICE,&[])?.status,409);
    let child_ci = fixture.ci(&fixture.second, 2, true);
    assert_eq!(
        post(
            &workspace,
            &update("dependency-open", &child, json!({"type":"prepare_submit"}))?,
            CAROL,
            std::slice::from_ref(&child_ci)
        )?
        .status,
        409
    );
    let parent_ci = fixture.ci(&fixture.first, 1, true);
    let parent = change_after(
        &workspace,
        "parent-prepare",
        &parent,
        json!({"type":"prepare_submit"}),
        CAROL,
        std::slice::from_ref(&parent_ci),
    )?;
    let parent = change_after(
        &workspace,
        "parent-submit",
        &parent,
        json!({"type":"submit","candidate_id":field(&parent,"/candidate/id")?}),
        CAROL,
        &[parent_ci],
    )?;
    assert_eq!(fixture.target(MAIN)?, fixture.first);
    let child = change_after(
        &workspace,
        "child-prepare",
        &child,
        json!({"type":"prepare_submit"}),
        CAROL,
        std::slice::from_ref(&child_ci),
    )?;
    assert_eq!(field(&child, "/candidate/target")?, &json!(fixture.first));
    assert_eq!(field(&child, "/revisions/0/base")?, &json!(fixture.base));
    assert_eq!(
        field(&child, "/candidate/dependencies/0/revision")?,
        field(&parent, "/revisions/0/id")?
    );
    let child = change_after(
        &workspace,
        "child-submit",
        &child,
        json!({"type":"submit","candidate_id":field(&child,"/candidate/id")?}),
        CAROL,
        &[child_ci],
    )?;
    assert_eq!(field(&child, "/state")?, "submitted");
    assert_eq!(fixture.target(MAIN)?, fixture.second);
    let stack = detail(&workspace, &child, BOB, &[])?;
    assert_eq!(
        field(&stack, "/stack/dependencies/0/id")?,
        field(&parent, "/id")?
    );
    assert_eq!(field(&stack, "/stack/dependencies/0/state")?, "submitted");
    Ok(())
}

#[test]
fn reviewed_file_preferences_are_private_and_new_revision_invalidates_them() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let change = create(&workspace, &fixture, "create")?;
    let marked = change_after(
        &workspace,
        "mark",
        &change,
        json!({"type":"mark_reviewed","file":"file.txt","reviewed":true}),
        BOB,
        &[],
    )?;
    assert_eq!(field(&marked, "/reviewed/0/actor")?, BOB);
    let author = detail(&workspace, &marked, ALICE, &[])?;
    assert_eq!(field(&author, "/change/reviewed")?, &json!([]));
    let reviewer = detail(&workspace, &marked, BOB, &[])?;
    assert_eq!(field(&reviewer, "/change/reviewed/0/file")?, "file.txt");
    let uploaded = change_after(
        &workspace,
        "upload",
        &marked,
        json!({"type":"upload","commit":fixture.second}),
        ALICE,
        &[],
    )?;
    let reviewer = detail(&workspace, &uploaded, BOB, &[])?;
    assert_eq!(field(&reviewer, "/change/reviewed")?, &json!([]));
    Ok(())
}

#[test]
fn detail_ci_runs_are_exact_source_sorted_bounded_and_not_browser_assertions() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let change = create(&workspace, &fixture, "create")?;
    let mut facts: Vec<_> = (1..=30)
        .map(|ordinal| fixture.ci(&fixture.first, ordinal, ordinal % 2 == 0))
        .collect();
    let mut partial = fixture.ci(&fixture.first, 31, false);
    partial.full_run = false;
    facts.push(partial);
    let mut dirty = fixture.ci(&fixture.first, 32, false);
    dirty.clean = false;
    facts.push(dirty);
    let mut foreign = fixture.ci(&fixture.first, 33, true);
    foreign.repository = "foreign".into();
    facts.push(foreign);
    facts.push(fixture.ci(&fixture.second, 34, true));
    facts.reverse();
    let snapshot = detail(&workspace, &change, BOB, &facts)?;
    assert_eq!(field(&snapshot, "/ci_runs_total")?, &json!(32));
    assert_eq!(field(&snapshot, "/ci_runs_truncated")?, true);
    assert_eq!(field(&snapshot, "/ci_runs/0/ordinal")?, &json!(32));
    assert_eq!(field(&snapshot, "/ci_runs/0/clean")?, false);
    assert_eq!(field(&snapshot, "/ci_runs/1/full_run")?, false);
    let runs = field(&snapshot, "/ci_runs")?
        .as_array()
        .ok_or("CI runs array")?;
    assert_eq!(runs.len(), 25);
    assert!(
        runs.iter()
            .all(|run| run.get("repository").and_then(Value::as_str)
                == Some(fixture.profile.provider.repository_id.as_str())
                && run.get("revision").and_then(Value::as_str) == Some(fixture.first.as_str()))
    );
    let request = update(
        "browser-ci",
        &change,
        json!({"type":"prepare_submit","ci_runs":[fixture.ci(&fixture.first,99,true)]}),
    )?;
    assert_eq!(post(&workspace, &request, CAROL, &[])?.status, 400);
    assert_eq!(fixture.target(MAIN)?, fixture.base);
    Ok(())
}

#[test]
fn idempotency_and_private_query_namespaces_are_per_authenticated_principal() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let request = global(
        "same-key",
        json!({"type":"save_view","name":"Private owned view","query":{"kind":"all","text":"review","status":null}}),
    );
    accepted(post(&workspace, &request, ALICE, &[])?)?;
    accepted(post(&workspace, &request, BOB, &[])?)?;
    let alice = accepted(get(&workspace, "/api/code-review", ALICE, &[]))?;
    let bob = accepted(get(&workspace, "/api/code-review", BOB, &[]))?;
    assert_ne!(
        field(&alice, "/saved_views/0/id")?,
        field(&bob, "/saved_views/0/id")?
    );
    let replayed = accepted(post(&workspace, &request, BOB, &[])?)?;
    assert_eq!(field(&replayed, "/replayed")?, true);
    let deleted = global(
        "delete-own-view",
        json!({"type":"delete_view","id":field(&alice,"/saved_views/0/id")?}),
    );
    assert_eq!(post(&workspace, &deleted, BOB, &[])?.status, 409);
    accepted(post(&workspace, &deleted, ALICE, &[])?)?;
    let alice = accepted(get(&workspace, "/api/code-review", ALICE, &[]))?;
    assert_eq!(field(&alice, "/saved_views")?, &json!([]));
    let bob = accepted(get(&workspace, "/api/code-review", BOB, &[]))?;
    assert_eq!(field(&bob, "/saved_views/0/name")?, "Private owned view");
    Ok(())
}

#[test]
fn nested_policy_provider_anchor_dependency_and_query_schemas_are_closed() -> Result {
    let fixture = Fixture::new()?;
    let directory = fixture
        .profile
        .database
        .parent()
        .ok_or("profile directory")?;
    let profile_path = directory.join("profile.json");
    std::fs::write(&profile_path, serde_json::to_vec(&fixture.profile)?)?;
    Profile::load(&profile_path)?;
    assert!(!fixture.profile.database.exists());
    let mut malformed = serde_json::to_value(&fixture.profile)?;
    malformed
        .pointer_mut("/policy")
        .and_then(Value::as_object_mut)
        .ok_or("policy object")?
        .insert("allow_author_approval".into(), json!(true));
    std::fs::write(&profile_path, serde_json::to_vec(&malformed)?)?;
    assert!(Profile::load(&profile_path).is_err());
    let mut malformed = serde_json::to_value(&fixture.profile)?;
    malformed
        .pointer_mut("/provider")
        .and_then(Value::as_object_mut)
        .ok_or("provider object")?
        .insert("allow_force".into(), json!(true));
    std::fs::write(&profile_path, serde_json::to_vec(&malformed)?)?;
    assert!(Profile::load(&profile_path).is_err());
    let workspace = fixture.workspace()?;
    let change = create(&workspace, &fixture, "create")?;
    let anchor = json!({"revision":field(&change,"/revisions/0/id")?,"base_commit":fixture.base,"file":"file.txt","side":"revision","start_line":2,"end_line":2});
    let mut bad_anchor = anchor.clone();
    bad_anchor
        .as_object_mut()
        .ok_or("anchor object")?
        .insert("ci_success".into(), json!(true));
    assert_eq!(post(&workspace,&update("nested-anchor",&change,json!({"type":"start_thread","anchor":bad_anchor,"body":"Must not accept authority options","blocking":true}))?,BOB,&[])?.status,400);
    assert_eq!(post(&workspace,&update("nested-dependency",&change,json!({"type":"set_dependencies","dependencies":[{"change":field(&change,"/id")?,"revision":field(&change,"/revisions/0/id")?,"ignore_cycle":true}]}))?,ALICE,&[])?.status,400);
    assert_eq!(post(&workspace,&global("nested-query",json!({"type":"save_view","name":"Leaky query","query":{"kind":"all","text":"","status":null,"include_private":true}})),BOB,&[])?.status,400);
    let valid = change_after(
        &workspace,
        "valid-anchor",
        &change,
        json!({"type":"start_thread","anchor":anchor,"body":"Exactly anchored feedback","blocking":true}),
        BOB,
        &[],
    )?;
    assert_eq!(
        field(&valid, "/threads/0/anchor/base_commit")?,
        &json!(fixture.base)
    );
    let denied = post(
        &workspace,
        &update("owner-only", &valid, json!({"type":"abandon"}))?,
        BOB,
        &[],
    )?;
    assert_eq!(denied.status, 409);
    assert_eq!(field(&value(&denied)?, "/code")?, "forbidden");
    Ok(())
}

// r[verify kiln.review.parity.ergonomics]
#[test]
fn published_suggestion_creates_exact_revision_invalidates_approval_and_replays() -> Result {
    let mut fixture = Fixture::new()?;
    fixture
        .profile
        .provider
        .capabilities
        .push(ReviewCapability::ApplySuggestions);
    let workspace = fixture.workspace()?;
    let change = approved(&workspace, &fixture)?;
    let revision = field(&change, "/revisions/0")?;
    let anchor = json!({"revision":field(revision,"/id")?,"base_commit":field(revision,"/base")?,
        "file":"file.txt","side":"revision","start_line":2,"end_line":2});
    let change = change_after(
        &workspace,
        "suggest-thread",
        &change,
        json!({"type":"start_thread","anchor":anchor,
        "body":"```suggestion\nreplacement\n```","blocking":false}),
        BOB,
        &[],
    )?;
    let thread = text(&change, "/threads/0/id")?;
    let request = update(
        "apply-suggestion",
        &change,
        json!({"type":"apply_suggestions","thread_ids":[thread]}),
    )?;
    assert_eq!(post(&workspace, &request, BOB, &[])?.status, 403);
    let receipt = accepted(post(&workspace, &request, ALICE, &[])?)?;
    let updated = detail(&workspace, field(&receipt, "/change")?, ALICE, &[])?;
    let result = text(&updated, "/change/revisions/1/commit")?;
    assert_ne!(result, fixture.first);
    assert_eq!(fixture.target(MAIN)?, fixture.base);
    let repository = Repository::open_bare(&fixture.profile.provider.repository_path)?;
    let commit = repository.find_commit(Oid::from_str(&result)?)?;
    assert_eq!(commit.parent_id(0)?.to_string(), fixture.first);
    let blob = commit
        .tree()?
        .get_path(std::path::Path::new("file.txt"))?
        .id();
    assert_eq!(repository.find_blob(blob)?.content(), b"one\nreplacement\n");
    assert!(
        field(&updated, "/checklist/items")?
            .as_array()
            .ok_or("items")?
            .iter()
            .any(|item| item["key"] == "approvals" && item["satisfied"] == false)
    );
    let replay = accepted(post(&workspace, &request, ALICE, &[])?)?;
    assert_eq!(field(&replay, "/change")?, field(&receipt, "/change")?);
    let current = field(&updated, "/change")?;
    assert_eq!(
        post(
            &workspace,
            &update(
                "old-suggestion",
                current,
                json!({"type":"apply_suggestions","thread_ids":[thread]})
            )?,
            ALICE,
            &[]
        )?
        .status,
        409
    );
    Ok(())
}

// r[verify kiln.review.parity.ownership]
#[test]
fn path_owner_auto_request_is_enforced_by_real_service_source_observations() -> Result {
    let mut fixture = Fixture::new()?;
    fixture.profile.policy.ownership = serde_json::from_value(
        json!({"teams":{"maintainers":[BOB]},"rules":[
        {"id":"files","paths":["*.txt"],"owners":[],"teams":["maintainers"],"target_refs":[MAIN]}]}),
    )?;
    let workspace = fixture.workspace()?;
    let change = create(&workspace, &fixture, "owned-create")?;
    assert_eq!(
        field(&change, "/reviewers")?.get(BOB),
        Some(&json!("reviewer"))
    );
    let ci = [fixture.ci(&fixture.first, 1, true)];
    let before = detail(&workspace, &change, ALICE, &ci)?;
    assert!(
        field(&before, "/checklist/items")?
            .as_array()
            .ok_or("items")?
            .iter()
            .any(|item| item["key"] == "owner:files" && item["satisfied"] == false)
    );
    let approved = change_after(
        &workspace,
        "owner-vote",
        &change,
        json!({"type":"vote","value":"approve"}),
        BOB,
        &ci,
    )?;
    assert_eq!(
        field(
            &detail(&workspace, &approved, ALICE, &ci)?,
            "/checklist/ready"
        )?,
        true
    );
    let changed = change_after(
        &workspace,
        "owner-new-revision",
        &approved,
        json!({"type":"upload","commit":fixture.second}),
        ALICE,
        &ci,
    )?;
    let after = detail(
        &workspace,
        &changed,
        ALICE,
        &[fixture.ci(&fixture.second, 2, true)],
    )?;
    assert!(
        field(&after, "/checklist/items")?
            .as_array()
            .ok_or("items")?
            .iter()
            .any(|item| item["key"] == "owner:files" && item["satisfied"] == false)
    );
    Ok(())
}

// r[verify kiln.highlight.api]
#[test]
fn review_diff_highlighting_is_opt_in_row_aligned_and_rejects_other_values() -> Result {
    let fixture = Fixture::new()?;
    let repository = Repository::open_bare(&fixture.profile.provider.repository_path)?;
    let patch = commit(
        &repository,
        Some(Oid::from_str(&fixture.base)?),
        "rust",
        &[
            ("file.txt", "one\ntwo\n"),
            ("lib.rs", "// é😀\nfn main() { let s = \"x\"; }\n"),
        ],
    )?;
    let workspace = fixture.workspace()?;
    let created = accepted(post(
        &workspace,
        &global(
            "create-rust",
            json!({"type":"create","commit":patch.to_string(),"target_ref":MAIN,"title":"Rust","description":"","wip":false}),
        ),
        ALICE,
        &[],
    )?)?;
    let route = format!(
        "/api/code-review/changes/{}/diff",
        text(&created, "/change/id")?
    );
    let plain = get(&workspace, &route, BOB, &[]);
    let plain_value = accepted(get(&workspace, &route, BOB, &[]))?;
    assert_eq!(plain_value.get("highlight"), None);
    let highlighted = accepted(get(&workspace, &format!("{route}?highlight=1"), BOB, &[]))?;
    let mut stripped = highlighted.clone();
    if let Some(object) = stripped.as_object_mut() {
        object.remove("highlight");
    }
    assert_eq!(stripped, plain_value);
    assert_eq!(serde_json::to_vec(&stripped)?, plain.body);
    assert_eq!(
        field(&highlighted, "/highlight/schema")?,
        &json!("kiln.highlight.v1")
    );
    assert_eq!(
        field(&highlighted, "/highlight/status")?,
        &json!("highlighted")
    );
    let rows: Vec<&Value> = field(&highlighted, "/files")?
        .as_array()
        .ok_or("files")?
        .iter()
        .filter_map(|file| file.get("hunks").and_then(Value::as_array))
        .flatten()
        .filter_map(|hunk| hunk.get("lines").and_then(Value::as_array))
        .flatten()
        .collect();
    let lines = field(&highlighted, "/highlight/lines")?
        .as_array()
        .ok_or("lines")?;
    assert_eq!(lines.len(), rows.len());
    let texts: Vec<&str> = rows
        .iter()
        .filter_map(|row| row.get("text").and_then(Value::as_str))
        .collect();
    let comment = texts
        .iter()
        .position(|text| text.starts_with("// é😀"))
        .ok_or("comment row")?;
    // "// é😀" is six UTF-16 units; offsets are relative to the unprefixed text.
    assert_eq!(
        field(&highlighted, &format!("/highlight/lines/{comment}"))?,
        &json!([[0, 6, "comment"]])
    );
    let code = texts
        .iter()
        .position(|text| text.starts_with("fn main"))
        .ok_or("code row")?;
    assert_eq!(
        field(&highlighted, &format!("/highlight/lines/{code}/0"))?,
        &json!([0, 2, "keyword"])
    );
    for bad in ["2", "true", "", "yes"] {
        let response = get(&workspace, &format!("{route}?highlight={bad}"), BOB, &[]);
        assert_eq!(response.status, 400, "{bad}");
        assert_eq!(
            field(&value(&response)?, "/code")?,
            &json!("review_query_invalid")
        );
    }
    Ok(())
}
