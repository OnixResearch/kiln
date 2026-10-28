//! Authenticated branch submission against disposable local Git and SQLite.
//! No network or production repository references are used.
// r[verify kiln.review.parity.intake]
use std::error::Error;
use std::os::unix::fs::PermissionsExt;

use git2::{Oid, Repository, Signature, Time};
use kiln_web_service::code_review::{Profile, RequestContext, Workspace};
use kiln_web_service::engine::{ApiResponse, Role};
use serde_json::{Value, json};
use tempfile::TempDir;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const ALICE: &str = "local:author";
const BOB: &str = "local:reviewer";
const MAIN: &str = "refs/heads/main";
const RELEASE: &str = "refs/heads/release";
const SOURCE: &str = "refs/heads/review/topic";
const EXPLICIT: &str = "refs/heads/team/topic";

struct Fixture {
    _temp: TempDir,
    profile: Profile,
    base: Oid,
    first: Oid,
    second: Oid,
}
impl Fixture {
    fn new() -> Result<Self> {
        let temp = tempfile::tempdir()?;
        std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700))?;
        let repository_path = temp.path().join("review.git");
        let repository = Repository::init_bare(&repository_path)?;
        let base = commit(&repository, None, "one\ntwo\n")?;
        let first = commit(&repository, Some(base), "one\nchanged\n")?;
        let second = commit(&repository, Some(first), "one\nchanged again\n")?;
        repository.reference(MAIN, base, false, "fixture")?;
        repository.reference(RELEASE, base, false, "fixture")?;
        repository.reference(SOURCE, first, false, "fixture")?;
        repository.reference(EXPLICIT, first, false, "fixture")?;
        let canonical = repository_path.canonicalize()?;
        let repository_id =
            kiln_codec::identity_from_bytes(canonical.to_str().ok_or("path encoding")?.as_bytes())?;
        let profile: Profile = serde_json::from_value(json!({
            "schema":"kiln.code-review-profile.v1",
            "database":temp.path().join("review.sqlite"),
            "project":"project",
            "provider":{
                "repository_id":repository_id.as_str(),
                "repository_path":repository_path,
                "target_refs":[MAIN,RELEASE],
                "capabilities":["inspect","compare","register"]
            },
            "policy":{
                "id":"review-policy", "eligible_reviewers":[BOB],
                "submitters":[BOB], "minimum_approvals":1, "require_ci":true
            },
            "read_principals":[ALICE,BOB], "write_principals":[ALICE,BOB],
            "signature_name":"Review service", "signature_email":"review@example.invalid",
            "intake":{
                "default_target_ref":MAIN,
                "mappings":[{"source_ref":EXPLICIT,"target_ref":RELEASE,"principal":ALICE}]
            }
        }))?;
        Ok(Self {
            _temp: temp,
            profile,
            base,
            first,
            second,
        })
    }
    fn workspace(&self) -> Result<Workspace> {
        Ok(Workspace::new(Some(self.profile.clone()))?)
    }
    fn move_source(&self, source_ref: &str, commit: Oid) -> Result {
        Repository::open_bare(&self.profile.provider.repository_path)?.reference(
            source_ref,
            commit,
            true,
            "fixture branch update",
        )?;
        Ok(())
    }
    fn target(&self, reference: &str) -> Result<Oid> {
        Repository::open_bare(&self.profile.provider.repository_path)?
            .find_reference(reference)?
            .target()
            .ok_or_else(|| "not a direct ref".into())
    }
}
fn commit(repository: &Repository, parent: Option<Oid>, contents: &str) -> Result<Oid> {
    let mut builder = repository.treebuilder(None)?;
    builder.insert("file.txt", repository.blob(contents.as_bytes())?, 0o100644)?;
    let tree = repository.find_tree(builder.write()?)?;
    let signature = Signature::new(
        "Fixture",
        "fixture@example.invalid",
        &Time::new(1_700_000_000, 0),
    )?;
    let parent = parent.map(|oid| repository.find_commit(oid)).transpose()?;
    let parents: Vec<_> = parent.iter().collect();
    Ok(repository.commit(None, &signature, &signature, "fixture", &tree, &parents)?)
}
fn submit(
    workspace: &Workspace,
    request_id: &str,
    source_ref: &str,
    expected_commit: Option<String>,
    actor: &str,
) -> Result<ApiResponse> {
    let body = json!({
        "request_id":request_id,"change_id":null,"expected_version":null,
        "action":{"type":"contribute","source_ref":source_ref,"expected_commit":expected_commit,
            "title":"Branch contribution","description":"Local admitted branch","wip":false}
    });
    Ok(workspace.response(
        "POST",
        "/api/code-review/commands",
        &serde_json::to_vec(&body)?,
        context(actor),
    ))
}
fn context(actor: &str) -> RequestContext<'_> {
    RequestContext {
        role: Role::Viewer,
        principal: actor,
        now_ms: 1000,
        ci: &[],
    }
}
fn read(workspace: &Workspace, path: &str, actor: &str) -> Result<Value> {
    accepted(workspace.response("GET", path, &[], context(actor)))
}
fn accepted(response: ApiResponse) -> Result<Value> {
    assert_eq!(
        response.status,
        200,
        "{}",
        String::from_utf8_lossy(&response.body)
    );
    Ok(serde_json::from_slice(&response.body)?)
}
fn field<'a>(value: &'a Value, path: &str) -> Result<&'a Value> {
    value
        .pointer(path)
        .ok_or_else(|| format!("missing {path}: {value}").into())
}
fn rejected(response: ApiResponse, status: u16, code: &str) -> Result {
    assert_eq!(
        response.status,
        status,
        "{}",
        String::from_utf8_lossy(&response.body)
    );
    let value: Value = serde_json::from_slice(&response.body)?;
    assert_eq!(field(&value, "/code")?, code);
    Ok(())
}
fn change(workspace: &Workspace, id: &str) -> Result<Value> {
    Ok(field(
        &read(workspace, &format!("/api/code-review/changes/{id}"), ALICE)?,
        "/change",
    )?
    .clone())
}
fn id(response: &Value) -> Result<&str> {
    response
        .pointer("/change/id")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing change ID".into())
}

#[test]
fn branch_submission_updates_stable_review_and_suppresses_durable_replays() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    // No immutable commit, change identity, or expected version is entered.
    let created = accepted(submit(&workspace, "create", SOURCE, None, ALICE)?)?;
    let change_id = id(&created)?.to_owned();
    let first = change(&workspace, &change_id)?;
    assert_eq!(field(&first, "/owner")?, ALICE);
    assert_eq!(
        field(&first, "/revisions/0/commit")?,
        fixture.first.to_string().as_str()
    );
    assert_eq!(field(&first, "/target_ref")?, MAIN);
    fixture.move_source(SOURCE, fixture.second)?;
    let uploaded = accepted(submit(&workspace, "upload", SOURCE, None, ALICE)?)?;
    assert_eq!(id(&uploaded)?, change_id);
    let second = change(&workspace, &change_id)?;
    assert_eq!(
        field(&second, "/revisions/0/commit")?,
        fixture.first.to_string().as_str()
    );
    assert_eq!(
        field(&second, "/revisions/1/commit")?,
        fixture.second.to_string().as_str()
    );
    assert_eq!(field(&second, "/revisions/1/number")?, 2);
    // An exact repeated delivery remains bound to its original journal result,
    // even after the source moved. A new delivery of the same tip is a no-op.
    let original = accepted(submit(&workspace, "create", SOURCE, None, ALICE)?)?;
    assert_eq!(field(&original, "/change")?, field(&created, "/change")?);
    assert_eq!(field(&original, "/replayed")?, true);
    drop(workspace);
    let restarted = fixture.workspace()?;
    let replay = accepted(submit(&restarted, "duplicate-tip", SOURCE, None, ALICE)?)?;
    assert_eq!(id(&replay)?, change_id);
    assert_eq!(field(&replay, "/replayed")?, true);
    assert_eq!(change(&restarted, &change_id)?, second);
    assert_eq!(fixture.target(MAIN)?, fixture.base);
    assert_eq!(fixture.target(RELEASE)?, fixture.base);
    Ok(())
}

#[test]
fn source_ownership_staleness_and_closed_ref_admission_fail_without_upload() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let created = accepted(submit(&workspace, "create", SOURCE, None, ALICE)?)?;
    let change_id = id(&created)?;
    let original = change(&workspace, change_id)?;
    fixture.move_source(SOURCE, fixture.second)?;
    rejected(
        submit(&workspace, "takeover", SOURCE, None, BOB)?,
        403,
        "review_source_owner_denied",
    )?;
    rejected(
        submit(
            &workspace,
            "stale",
            SOURCE,
            Some(fixture.first.to_string()),
            ALICE,
        )?,
        409,
        "review_source_commit_moved",
    )?;
    rejected(
        submit(&workspace, "bad-hash", SOURCE, Some("HEAD".into()), ALICE)?,
        400,
        "review_source_commit_invalid",
    )?;
    for (index, reference, status, code) in [
        (0, MAIN, 403, "review_source_ref_protected"),
        (1, RELEASE, 403, "review_source_ref_protected"),
        (
            2,
            "https://example.invalid/repo.git",
            400,
            "review_source_ref_invalid",
        ),
        (
            3,
            "refs/heads/review/topic~1",
            400,
            "review_source_ref_invalid",
        ),
        (
            4,
            "refs/heads/unconfigured",
            403,
            "review_source_ref_not_admitted",
        ),
        (
            5,
            "refs/tags/review/topic",
            400,
            "review_source_ref_invalid",
        ),
        (
            6,
            "refs/heads/review/missing",
            404,
            "review_source_ref_missing",
        ),
    ] {
        rejected(
            submit(&workspace, &format!("ref-{index}"), reference, None, ALICE)?,
            status,
            code,
        )?;
    }
    assert_eq!(change(&workspace, change_id)?, original);
    assert_eq!(fixture.target(MAIN)?, fixture.base);
    Ok(())
}

#[test]
fn explicit_mapping_binds_owner_target_and_expected_current_commit() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    rejected(
        submit(&workspace, "wrong-owner", EXPLICIT, None, BOB)?,
        403,
        "review_source_owner_denied",
    )?;
    let response = accepted(submit(
        &workspace,
        "explicit",
        EXPLICIT,
        Some(fixture.first.to_string()),
        ALICE,
    )?)?;
    let observed = change(&workspace, id(&response)?)?;
    assert_eq!(field(&observed, "/target_ref")?, RELEASE);
    assert_eq!(
        field(&observed, "/revisions/0/commit")?,
        fixture.first.to_string().as_str()
    );
    assert_eq!(fixture.target(RELEASE)?, fixture.base);
    Ok(())
}

#[test]
fn symbolic_noncommit_and_rolled_back_sources_are_not_new_revisions() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let repository = Repository::open_bare(&fixture.profile.provider.repository_path)?;
    let alias = "refs/heads/review/alias";
    repository.reference_symbolic(alias, SOURCE, false, "fixture alias")?;
    rejected(
        submit(&workspace, "symbolic", alias, None, ALICE)?,
        400,
        "review_source_ref_symbolic",
    )?;
    let blob_ref = "refs/heads/review/blob";
    repository.reference(
        blob_ref,
        repository.blob(b"not a commit")?,
        false,
        "fixture blob",
    )?;
    rejected(
        submit(&workspace, "blob", blob_ref, None, ALICE)?,
        400,
        "review_source_ref_not_commit",
    )?;
    let first = accepted(submit(&workspace, "first", SOURCE, None, ALICE)?)?;
    fixture.move_source(SOURCE, fixture.second)?;
    accepted(submit(&workspace, "second", SOURCE, None, ALICE)?)?;
    let before_rollback = change(&workspace, id(&first)?)?;
    fixture.move_source(SOURCE, fixture.first)?;
    rejected(
        submit(&workspace, "rollback", SOURCE, None, ALICE)?,
        409,
        "review_source_commit_replayed",
    )?;
    assert_eq!(change(&workspace, id(&first)?)?, before_rollback);
    assert_eq!(fixture.target(MAIN)?, fixture.base);
    Ok(())
}

#[test]
fn configuration_cannot_admit_protected_or_duplicate_source_mappings() -> Result {
    for (source, duplicate) in [(MAIN, false), (SOURCE, true)] {
        let fixture = Fixture::new()?;
        let mut value = serde_json::to_value(&fixture.profile)?;
        let mapping = json!({"source_ref":source,"target_ref":RELEASE,"principal":ALICE});
        *value
            .pointer_mut("/intake/mappings")
            .ok_or("missing mappings")? = if duplicate {
            json!([mapping, mapping])
        } else {
            json!([mapping])
        };
        let profile: Profile = serde_json::from_value(value)?;
        assert!(Workspace::new(Some(profile)).is_err());
    }
    Ok(())
}

#[test]
fn branch_picker_exposes_current_commits_only_to_admitted_owners() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let before = read(&workspace, "/api/code-review", ALICE)?;
    let sources = field(&before, "/contribution_sources")?
        .as_array()
        .ok_or("missing sources")?;
    let explicit = sources
        .iter()
        .find(|source| source["source_ref"] == EXPLICIT)
        .ok_or("missing explicit source")?;
    assert_eq!(explicit["target_ref"], RELEASE);
    assert_eq!(explicit["commit"], fixture.first.to_string());
    assert!(
        sources
            .iter()
            .all(|source| source["source_ref"] != MAIN && source["source_ref"] != RELEASE)
    );
    let bob = read(&workspace, "/api/code-review", BOB)?;
    let bob_sources = field(&bob, "/contribution_sources")?
        .as_array()
        .ok_or("missing sources")?;
    assert!(
        bob_sources
            .iter()
            .any(|source| source["source_ref"] == SOURCE)
    );
    assert!(
        bob_sources
            .iter()
            .all(|source| source["source_ref"] != EXPLICIT)
    );
    let created = accepted(submit(&workspace, "claim", SOURCE, None, ALICE)?)?;
    fixture.move_source(SOURCE, fixture.second)?;
    let alice = read(&workspace, "/api/code-review", ALICE)?;
    let source = field(&alice, "/contribution_sources")?
        .as_array()
        .ok_or("missing sources")?
        .iter()
        .find(|source| source["source_ref"] == SOURCE)
        .ok_or("missing claimed source")?;
    assert_eq!(source["commit"], fixture.second.to_string());
    assert_eq!(source["change_id"], id(&created)?);
    let bob = read(&workspace, "/api/code-review", BOB)?;
    assert!(
        field(&bob, "/contribution_sources")?
            .as_array()
            .ok_or("missing sources")?
            .iter()
            .all(|source| source["source_ref"] != SOURCE)
    );
    Ok(())
}
