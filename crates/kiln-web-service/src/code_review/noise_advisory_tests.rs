//! Display-only noise rules and advisory approvals through the real review
//! shell over a disposable local Git repository and SQLite journal.
use std::error::Error;
use std::os::unix::fs::PermissionsExt;

use git2::{Oid, Repository, Signature, Time};
use tempfile::TempDir;

use super::*;

type TestResult<T = ()> = std::result::Result<T, Box<dyn Error>>;
const ALICE: &str = "local:author";
const BOB: &str = "local:reviewer";
const CAROL: &str = "local:lock-owner";
/// Read-only principal: may advise, never vote or write.
const DAVE: &str = "local:reader";
const MAIN: &str = "refs/heads/main";

struct Fixture {
    _temp: TempDir,
    profile: Profile,
    workspace: Workspace,
    first: Oid,
}

fn profile_document(temp: &Path, path: &Path, identity: &str) -> Value {
    json!({
        "schema":"kiln.code-review-profile.v1",
        "database":temp.join("review.sqlite"),
        "project":"project",
        "provider":{"repository_id":identity,"repository_path":path,
            "target_refs":[MAIN],"capabilities":["inspect","compare","register"]},
        "policy":{"id":"review-policy","eligible_reviewers":[BOB,CAROL],"submitters":[ALICE],
            "minimum_approvals":1,"require_ci":false,
            // Ownership over a path that is also declared noise.
            "ownership":{"teams":{},"rules":[{"id":"lock","paths":["Cargo.lock"],
                "owners":[CAROL],"teams":[],"target_refs":[]}]}},
        "read_principals":[ALICE,BOB,CAROL,DAVE],"write_principals":[ALICE,BOB,CAROL],
        "signature_name":"Review service","signature_email":"review@example.invalid",
        "review":{"noise":[
            {"id":"lockfiles","pattern":"**/Cargo.lock","reason":"Lockfile churn"},
            {"id":"vendor","pattern":"vendor/**","reason":"Vendored sources"}]}
    })
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let temp = tempfile::tempdir()?;
        std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700))?;
        let path = temp.path().join("review.git");
        let repository = Repository::init_bare(&path)?;
        let base = commit(
            &repository,
            None,
            &[
                ("Cargo.lock", "version = 1\n"),
                ("src/lib.rs", "pub fn one() {}\n"),
                ("vendor/dep/lib.rs", "pub fn dep() {}\n"),
            ],
        )?;
        let first = commit(
            &repository,
            Some(base),
            &[
                ("Cargo.lock", "version = 2\n"),
                ("src/lib.rs", "pub fn two() {}\n"),
                ("vendor/dep/lib.rs", "pub fn dep2() {}\n"),
            ],
        )?;
        repository.reference(MAIN, base, false, "fixture")?;
        let canonical = path.canonicalize()?;
        let identity =
            kiln_codec::identity_from_bytes(canonical.to_str().ok_or("path encoding")?.as_bytes())?;
        let profile: Profile =
            serde_json::from_value(profile_document(temp.path(), &path, identity.as_str()))?;
        let workspace = Workspace::new(Some(profile.clone()))?;
        Ok(Self {
            _temp: temp,
            profile,
            workspace,
            first,
        })
    }
    fn call(&self, method: &str, target: &str, body: &[u8], actor: &str) -> ApiResponse {
        self.workspace.response(
            method,
            target,
            body,
            RequestContext {
                role: Role::Viewer,
                principal: actor,
                now_ms: 5000,
                ci: &[],
            },
        )
    }
    fn post(&self, request: &Value, actor: &str) -> TestResult<ApiResponse> {
        Ok(self.call(
            "POST",
            "/api/code-review/commands",
            &serde_json::to_vec(request)?,
            actor,
        ))
    }
    fn get(&self, target: &str, actor: &str) -> ApiResponse {
        self.call("GET", target, &[], actor)
    }
    fn detail(&self, id: &str, actor: &str) -> TestResult<Value> {
        accepted(self.get(&format!("/api/code-review/changes/{id}"), actor))
    }
    /// Act on the change at its current version.
    fn act(
        &self,
        id: &str,
        request_id: &str,
        action: Value,
        actor: &str,
    ) -> TestResult<ApiResponse> {
        let change = field(&self.detail(id, ALICE)?, "/change")?.clone();
        self.post(
            &json!({"request_id":request_id,"change_id":id,
                "expected_version":field(&change,"/version")?,"action":action}),
            actor,
        )
    }
    fn create(&self) -> TestResult<String> {
        let body = accepted(self.post(
            &json!({"request_id":"create","change_id":null,"expected_version":null,
                "action":{"type":"create","commit":self.first.to_string(),"target_ref":MAIN,
                    "title":"Bump lock, vendor and source","description":"","wip":false}}),
            ALICE,
        )?)?;
        let id = text(&body, "/change/id")?;
        for (index, reviewer) in [BOB, CAROL].iter().enumerate() {
            accepted(self.act(
                &id,
                &format!("reviewer-{index}"),
                json!({"type":"set_reviewer","principal":reviewer,"role":"reviewer"}),
                ALICE,
            )?)?;
        }
        Ok(id)
    }
    fn document(&self) -> TestResult<Vec<u8>> {
        let connection = rusqlite::Connection::open(&self.profile.database)?;
        Ok(
            connection.query_row("SELECT document FROM review_state WHERE id=1", [], |row| {
                row.get(0)
            })?,
        )
    }
}

fn commit(repository: &Repository, parent: Option<Oid>, files: &[(&str, &str)]) -> TestResult<Oid> {
    let mut root = repository.treebuilder(None)?;
    let mut nested: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    for (path, content) in files {
        match path.split_once('/') {
            Some((dir, rest)) => nested.entry(dir).or_default().push((rest, content)),
            None => {
                root.insert(*path, repository.blob(content.as_bytes())?, 0o100644)?;
            }
        }
    }
    for (dir, entries) in nested {
        root.insert(dir, tree(repository, &entries)?, 0o040000)?;
    }
    let tree = repository.find_tree(root.write()?)?;
    let signature = Signature::new(
        "Fixture",
        "fixture@example.invalid",
        &Time::new(1_700_000_000, 0),
    )?;
    let parent = parent.map(|id| repository.find_commit(id)).transpose()?;
    let parents: Vec<_> = parent.iter().collect();
    Ok(repository.commit(None, &signature, &signature, "fixture", &tree, &parents)?)
}

/// Write a (possibly nested) tree for `dir/...` entries.
fn tree(repository: &Repository, entries: &[(&str, &str)]) -> TestResult<Oid> {
    let mut builder = repository.treebuilder(None)?;
    let mut nested: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    for (path, content) in entries {
        match path.split_once('/') {
            Some((dir, rest)) => nested.entry(dir).or_default().push((rest, content)),
            None => {
                builder.insert(*path, repository.blob(content.as_bytes())?, 0o100644)?;
            }
        }
    }
    for (dir, children) in nested {
        builder.insert(dir, tree(repository, &children)?, 0o040000)?;
    }
    Ok(builder.write()?)
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
fn item(checklist: &Value, key: &str) -> TestResult<bool> {
    checklist
        .pointer("/items")
        .and_then(Value::as_array)
        .and_then(|items| items.iter().find(|item| item["key"] == key))
        .and_then(|item| item["satisfied"].as_bool())
        .ok_or_else(|| format!("missing checklist item {key}: {checklist}").into())
}

// r[verify kiln.review.noise.scenario.fixture]
// r[verify kiln.review.noise.display]
// r[verify kiln.review.noise.identity]
#[test]
fn noise_fixture_annotates_lockfile_and_vendored_files_only() -> TestResult {
    let fixture = Fixture::new()?;
    let id = fixture.create()?;
    let diff = accepted(fixture.get(&format!("/api/code-review/changes/{id}/diff"), BOB))?;
    let shown: Vec<String> = field(&diff, "/files")?
        .as_array()
        .ok_or("files")?
        .iter()
        .filter_map(|file| file["new_path"].as_str().map(String::from))
        .collect();
    assert_eq!(shown, ["Cargo.lock", "src/lib.rs", "vendor/dep/lib.rs"]);
    assert_eq!(
        field(&diff, "/noise/files")?,
        &json!({"Cargo.lock":"lockfiles","vendor/dep/lib.rs":"vendor"})
    );
    let identity = fixture
        .profile
        .review
        .as_ref()
        .and_then(noise::ReviewDisplay::identity)
        .ok_or("identity")?;
    assert_eq!(text(&diff, "/noise/identity")?, identity);
    // The same identity and rules are published on the summary and detail.
    let summary = accepted(fixture.get("/api/code-review", DAVE))?;
    assert_eq!(text(&summary, "/noise/identity")?, identity);
    assert_eq!(field(&summary, "/noise/rules/1/id")?, "vendor");
    let detail = fixture.detail(&id, DAVE)?;
    assert_eq!(text(&detail, "/noise/identity")?, identity);
    // Display only: nothing is stored as a viewed marker for any principal.
    assert_eq!(field(&detail, "/change/reviewed")?, &json!([]));
    let document = String::from_utf8(fixture.document()?)?;
    assert!(!document.contains("lockfiles") && !document.contains("noise"));
    Ok(())
}

// r[verify kiln.review.noise.scenario.ownership]
// r[verify kiln.review.noise.authority]
#[test]
fn ownership_over_a_noise_path_still_blocks_submit_without_the_owner_vote() -> TestResult {
    let fixture = Fixture::new()?;
    let id = fixture.create()?;
    accepted(fixture.act(
        &id,
        "bob-approves",
        json!({"type":"vote","value":"approve"}),
        BOB,
    )?)?;
    let checklist = field(&fixture.detail(&id, ALICE)?, "/checklist")?.clone();
    assert!(item(&checklist, "approvals")?);
    assert!(
        !item(&checklist, "owner:lock")?,
        "noise never satisfies ownership"
    );
    assert_eq!(field(&checklist, "/ready")?, false);
    rejected(
        &fixture.act(
            &id,
            "prepare-early",
            json!({"type":"prepare_submit"}),
            ALICE,
        )?,
        409,
        "submit_blocked",
    )?;
    accepted(fixture.act(
        &id,
        "carol-approves",
        json!({"type":"vote","value":"approve"}),
        CAROL,
    )?)?;
    let checklist = field(&fixture.detail(&id, ALICE)?, "/checklist")?.clone();
    assert!(item(&checklist, "owner:lock")?);
    assert_eq!(field(&checklist, "/ready")?, true);
    accepted(fixture.act(&id, "prepare", json!({"type":"prepare_submit"}), ALICE)?)?;
    Ok(())
}

// r[verify kiln.review.noise.scenario.invalid]
// r[verify kiln.review.noise]
#[test]
fn check_rejects_invalid_noise_rules_and_keeps_legacy_profiles_unchanged() -> TestResult {
    let fixture = Fixture::new()?;
    let base = serde_json::to_value(&fixture.profile)?;
    // The same loader `kiln-web-service check` uses.
    let load = |document: &Value| -> TestResult<Result<Profile, String>> {
        let path = fixture._temp.path().join("profile.json");
        std::fs::write(&path, serde_json::to_vec(document)?)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        Ok(Profile::load(&path))
    };
    assert!(load(&base)?.is_ok());
    let rule = |id: &str, pattern: &str| json!({"id":id,"pattern":pattern,"reason":"Noise"});
    let too_many: Vec<Value> = (0..=noise::MAX_NOISE_RULES)
        .map(|index| rule(&format!("rule-{index}"), "vendor/**"))
        .collect();
    for noise in [
        json!([rule("escape", "../outside/**")]),
        json!([rule("absolute", "/etc/**")]),
        json!([rule("class", "vendor/[ab]/**")]),
        json!([rule("partial", "vendor/a**")]),
        json!([rule(
            "long",
            &"a".repeat(noise::MAX_NOISE_PATTERN_BYTES + 1)
        )]),
        json!([rule("dup", "a/**"), rule("dup", "b/**")]),
        json!([rule("has space", "a/**")]),
        json!([{"id":"blank","pattern":"a/**","reason":" "}]),
        json!([{"id":"extra","pattern":"a/**","reason":"r","skip_ownership":true}]),
        Value::Array(too_many),
    ] {
        let mut document = base.clone();
        document
            .as_object_mut()
            .ok_or("profile document is an object")?
            .entry("review")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or("review is an object")?
            .insert("noise".into(), noise.clone());
        assert!(load(&document)?.is_err(), "{noise} must be rejected");
    }
    // A profile without `review` serializes without it, and noise rules never
    // change the store binding or the effective policy identity.
    let mut legacy = base.clone();
    legacy.as_object_mut().ok_or("profile")?.remove("review");
    let legacy_profile = load(&legacy)?.map_err(|error| error.to_string())?;
    assert_eq!(serde_json::to_value(&legacy_profile)?, legacy);
    assert!(legacy_profile.review.is_none());
    let binding = |profile: &Profile| profile.binding().map_err(|error| format!("{error:?}"));
    let policy = |profile: &Profile| {
        profile
            .effective_policy()
            .map_err(|error| format!("{error:?}"))
    };
    assert_eq!(binding(&legacy_profile)?, binding(&fixture.profile)?);
    assert_eq!(policy(&legacy_profile)?, policy(&fixture.profile)?);
    Ok(())
}

// r[verify kiln.review.advisory.scenario.forbidden]
// r[verify kiln.review.advisory.view]
// r[verify kiln.review.advisory.authority]
#[test]
fn read_principals_may_advise_but_never_vote_and_checklist_is_unchanged() -> TestResult {
    let fixture = Fixture::new()?;
    let id = fixture.create()?;
    let before = field(&fixture.detail(&id, ALICE)?, "/checklist")?.clone();
    accepted(fixture.act(
        &id,
        "dave-advises",
        json!({"type":"advisory_approve"}),
        DAVE,
    )?)?;
    accepted(fixture.act(&id, "bob-advises", json!({"type":"advisory_approve"}), BOB)?)?;
    let detail = fixture.detail(&id, ALICE)?;
    assert_eq!(field(&detail, "/checklist")?, &before);
    assert_eq!(field(&detail, "/change/votes")?, &json!([]));
    assert_eq!(field(&detail, "/change/advisory_current")?, 2);
    assert_eq!(field(&detail, "/change/advisory/0/actor")?, DAVE);
    // Read-only principals may send only the advisory actions.
    rejected(
        &fixture.act(
            &id,
            "dave-votes",
            json!({"type":"vote","value":"approve"}),
            DAVE,
        )?,
        403,
        "review_write_denied",
    )?;
    rejected(
        &fixture.post(&json!({"not":"a request"}), DAVE)?,
        403,
        "review_write_denied",
    )?;
    // Owners may not advise on their own change.
    rejected(
        &fixture.act(
            &id,
            "alice-advises",
            json!({"type":"advisory_approve"}),
            ALICE,
        )?,
        409,
        "advisory_forbidden",
    )?;
    // Search filter.
    let with = accepted(fixture.get("/api/code-review?advisory=with", DAVE))?;
    assert_eq!(field(&with, "/total")?, 1);
    assert_eq!(field(&with, "/changes/0/advisory_current")?, 2);
    let without = accepted(fixture.get("/api/code-review?advisory=without", DAVE))?;
    assert_eq!(field(&without, "/total")?, 0);
    rejected(
        &fixture.get("/api/code-review?advisory=maybe", DAVE),
        400,
        "review_query_invalid",
    )?;
    accepted(fixture.act(
        &id,
        "dave-withdraws",
        json!({"type":"advisory_withdraw"}),
        DAVE,
    )?)?;
    rejected(
        &fixture.act(
            &id,
            "dave-withdraws-again",
            json!({"type":"advisory_withdraw"}),
            DAVE,
        )?,
        409,
        "advisory_missing",
    )?;
    assert_eq!(
        field(&fixture.detail(&id, DAVE)?, "/change/advisory_current")?,
        1
    );
    Ok(())
}

// r[verify kiln.observe.review.advisory]
// r[verify kiln.review.advisory.legacy]
#[test]
fn review_window_export_counts_advisory_and_legacy_documents_stay_clean() -> TestResult {
    let fixture = Fixture::new()?;
    let id = fixture.create()?;
    accepted(fixture.act(
        &id,
        "bob-votes",
        json!({"type":"vote","value":"approve"}),
        BOB,
    )?)?;
    // A journal without advisory approvals carries no advisory bytes at all.
    let document = String::from_utf8(fixture.document()?)?;
    assert!(!document.contains("advisory"));
    let window = || -> TestResult<Value> {
        accepted(fixture.get(
            "/api/code-review/review-window?start_ms=0&end_ms=10000",
            DAVE,
        ))
    };
    assert!(
        field(&window()?, "/changes/0")?
            .get("advisory_approvals")
            .is_none()
    );
    accepted(fixture.act(
        &id,
        "dave-advises",
        json!({"type":"advisory_approve"}),
        DAVE,
    )?)?;
    let exported = window()?;
    assert_eq!(field(&exported, "/changes/0/advisory_approvals")?, 1);
    let parsed: kiln_observe_core::ReviewWindow = serde_json::from_value(exported)?;
    let first = parsed.changes.first().ok_or("exported change")?;
    assert_eq!(first.advisory_approvals, 1);
    assert!(first.approval.is_some(), "the vote, not the advice");
    Ok(())
}
