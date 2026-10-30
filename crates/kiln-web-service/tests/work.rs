//! Real disposable native stores: no installed keys, remote effects or fallback state.
use std::error::Error;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::sync::{Arc, Barrier};

use cob_git2::{Repository, Signature};
use kiln_web_service::engine::{ApiResponse, Role};
use kiln_web_service::work::{Profile, Workspace};
use onix_radicle_cob_git_store::GitCobStore;
use radicle_cob::change::Storage as _;
use serde_json::{Value, json};
use tempfile::TempDir;

type Result<T, E = Box<dyn Error>> = std::result::Result<T, E>;
const ALICE: &str = "local:operator";
const BOB: &str = "tailscale:42";

struct Fixture {
    temp: TempDir,
    profile: Profile,
}
impl Fixture {
    fn new() -> Result<Self> {
        let temp = tempfile::tempdir()?;
        let git_directory = temp.path().join("work.git");
        let repository = Repository::init_bare(&git_directory)?;
        std::fs::set_permissions(&git_directory, std::fs::Permissions::from_mode(0o700))?;
        let tree = repository.find_tree(repository.treebuilder(None)?.write()?)?;
        let signature = Signature::now("Existing owner", "owner@example.invalid")?;
        let base = repository.commit(
            Some("refs/heads/keep"),
            &signature,
            &signature,
            "unrelated",
            &tree,
            &[],
        )?;
        repository.set_head("refs/heads/keep")?;
        repository.reference("refs/foreign/preexisting", base, false, "fixture")?;
        let seed = temp.path().join("service.seed");
        std::fs::write(&seed, [7_u8; 32])?;
        std::fs::set_permissions(&seed, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self {
            temp,
            profile: Profile {
                schema: "kiln.web-work.v1".into(),
                git_directory,
                signer_seed_file: seed,
                write_principals: vec![ALICE.into(), BOB.into()],
            },
        })
    }
    fn workspace(&self) -> Result<Workspace> {
        Ok(Workspace::new(Some(self.profile.clone()))?)
    }
    fn repository(&self) -> Result<Repository> {
        Ok(Repository::open_bare(&self.profile.git_directory)?)
    }
}

fn field<'a>(value: &'a Value, path: &str) -> &'a Value {
    value
        .pointer(path)
        .unwrap_or_else(|| unreachable!("fixture field absent: {path}"))
}

fn body(response: &ApiResponse) -> Result<Value> {
    Ok(serde_json::from_slice(&response.body)?)
}
fn post(workspace: &Workspace, command: &Value, actor: &str, now: i64) -> Result<ApiResponse> {
    Ok(workspace.response(
        "POST",
        "/api/work/commands",
        &serde_json::to_vec(command)?,
        Role::Operator,
        actor,
        now,
    ))
}
fn create(request: &str, kind: &str, fields: Value) -> Value {
    json!({"request_id":request,"object_id":null,"expected_head":null,"action":{"type":"create","kind":kind,"fields":fields}})
}
fn change(request: &str, record: &Value, action: Value) -> Value {
    json!({"request_id":request,"object_id":(*field(record, "/id")),"expected_head":(*field(record, "/head")),"action":action})
}
fn accepted(response: ApiResponse) -> Result<Value> {
    assert_eq!(
        response.status,
        200,
        "{}",
        String::from_utf8_lossy(&response.body)
    );
    Ok((*field(&body(&response)?, "/object")).clone())
}
fn get(workspace: &Workspace, id: &str, actor: &str) -> ApiResponse {
    workspace.response(
        "GET",
        &format!("/api/work/objects/{id}"),
        &[],
        Role::Viewer,
        actor,
        999,
    )
}

#[test]
fn native_signed_history_survives_restart_and_replay_returns_original_result() -> Result<()> {
    let fixture = Fixture::new()?;
    let repository = fixture.repository()?;
    let original_head = repository.head()?.target();
    let original_config = std::fs::read(fixture.profile.git_directory.join("config"))?;
    let original_head_file = std::fs::read(fixture.profile.git_directory.join("HEAD"))?;
    let untouched = repository
        .find_reference("refs/foreign/preexisting")?
        .target();
    let workspace = fixture.workspace()?;
    let command = create(
        "issue-one",
        "issue",
        json!({"title":"Durable issue","description":"Native signed data"}),
    );
    let first = accepted(post(&workspace, &command, ALICE, 100)?)?;
    assert_eq!((*field(&first, "/created_by")), ALICE);
    let commented = accepted(post(
        &workspace,
        &change(
            "comment-one",
            &first,
            json!({"type":"comment","body":"Please review","mentions":[BOB]}),
        ),
        ALICE,
        200,
    )?)?;
    assert_eq!((*field(&commented, "/comments/0/author")), ALICE);
    let reacted = accepted(post(
        &workspace,
        &change(
            "reaction-one",
            &commented,
            json!({"type":"react","comment":"message:comment-one","emoji":"👍","active":true}),
        ),
        BOB,
        250,
    )?)?;
    assert_eq!((*field(&reacted, "/comments/0/reactions/👍")), json!([BOB]));
    let updated = accepted(post(
        &workspace,
        &change(
            "update-one",
            &reacted,
            json!({"type":"update","body":"Progress is retained","mentions":[]}),
        ),
        ALICE,
        300,
    )?)?;
    assert_eq!(
        (*field(&updated, "/updates/0/body")),
        "Progress is retained"
    );
    drop(workspace);
    let reopened = fixture.workspace()?;
    let observed = get(
        &reopened,
        (*field(&updated, "/id")).as_str().ok_or("id missing")?,
        ALICE,
    );
    assert_eq!(observed.status, 200);
    assert_eq!(body(&observed)?, updated);
    let replay = post(&reopened, &command, ALICE, 900)?;
    assert_eq!(replay.status, 200);
    assert_eq!((*field(&body(&replay)?, "/replayed")), true);
    assert_eq!((*field(&body(&replay)?, "/object")), first);
    let mut altered = command.clone();
    *altered
        .pointer_mut("/action/fields/title")
        .ok_or("fixture field absent")? = json!("Changed duplicate payload");
    assert_eq!(post(&reopened, &altered, ALICE, 900)?.status, 409);
    assert_eq!(post(&reopened, &command, BOB, 900)?.status, 409);
    assert_eq!(
        post(
            &reopened,
            &change(
                "stale",
                &first,
                json!({"type":"comment","body":"Stale write","mentions":[]})
            ),
            ALICE,
            900
        )?
        .status,
        409
    );
    let refs = repository
        .references_glob("refs/namespaces/*/refs/cobs/xyz.onixresearch.work.issue/*")?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    assert_eq!(refs.len(), 1);
    let store = GitCobStore::new(&repository);
    let tip = refs
        .first()
        .ok_or("fixture sequence absent")?
        .target()
        .ok_or("missing native target")?;
    let entry = store.load(tip.into())?;
    assert!(entry.valid_signatures());
    let envelope: Value = serde_json::from_slice(&entry.contents.head)?;
    assert_eq!((*field(&envelope, "/operation/actor")), ALICE);
    assert_ne!(entry.author().to_string(), ALICE);
    assert_eq!(repository.head()?.target(), original_head);
    assert_eq!(
        repository
            .find_reference("refs/foreign/preexisting")?
            .target(),
        untouched
    );
    assert_eq!(
        std::fs::read(fixture.profile.git_directory.join("config"))?,
        original_config
    );
    assert_eq!(
        std::fs::read(fixture.profile.git_directory.join("HEAD"))?,
        original_head_file
    );
    Ok(())
}

#[test]
fn revoking_a_writer_preserves_signed_history_and_denies_new_commands() -> Result<()> {
    let mut fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let created = accepted(post(
        &workspace,
        &create(
            "before-revocation",
            "issue",
            json!({"title":"Retained prior admission"}),
        ),
        ALICE,
        100,
    )?)?;
    drop(workspace);
    fixture
        .profile
        .write_principals
        .retain(|actor| actor != ALICE);
    let reopened = fixture.workspace()?;
    let id = (*field(&created, "/id")).as_str().ok_or("id missing")?;
    let observed = get(&reopened, id, BOB);
    assert_eq!(observed.status, 200);
    assert_eq!(body(&observed)?, created);
    assert_eq!(
        post(
            &reopened,
            &create(
                "revoked-command",
                "issue",
                json!({"title":"Must not be written"})
            ),
            ALICE,
            200
        )?
        .status,
        403
    );
    let commented = accepted(post(
        &reopened,
        &change(
            "retained-writer",
            &created,
            json!({"type":"comment","body":"History remains usable","mentions":[]}),
        ),
        BOB,
        200,
    )?)?;
    assert_eq!((*field(&commented, "/comments/0/author")), BOB);
    assert_eq!(
        (*field(&commented, "/fields/title")),
        (*field(&created, "/fields/title"))
    );
    Ok(())
}

#[test]
fn private_objects_hide_existence_and_work_authority_is_independently_admitted() -> Result<()> {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let private = accepted(post(
        &workspace,
        &create(
            "private-personal",
            "personal",
            json!({
                "title":"Only Alice","personal":{"watched":[],"read":{},"favorites":[],"reminders":{}}
            }),
        ),
        ALICE,
        100,
    )?)?;
    let private_view = accepted(post(
        &workspace,
        &create(
            "private-view",
            "view",
            json!({
                "title":"Secret filter","view":{"filters":{"q":"private needle"},"sort":"updated","group_by":"status","layout":"list","shared":false}
            }),
        ),
        ALICE,
        110,
    )?)?;
    let id = (*field(&private, "/id")).as_str().ok_or("id missing")?;
    assert_eq!(get(&workspace, id, ALICE).status, 200);
    let hidden = get(&workspace, id, BOB);
    let missing = get(&workspace, "0000000000000000000000000000000000000000", BOB);
    assert_eq!(hidden.status, 404);
    assert_eq!(hidden.body, missing.body);
    assert_eq!(
        get(
            &workspace,
            (*field(&private_view, "/id"))
                .as_str()
                .ok_or("id missing")?,
            BOB
        )
        .status,
        404
    );
    let listing = workspace.response("GET", "/api/work", &[], Role::Viewer, BOB, 200);
    assert_eq!(listing.status, 200);
    assert_eq!((*field(&body(&listing)?, "/objects")), json!([]));
    assert!(!String::from_utf8_lossy(&listing.body).contains("private needle"));
    let edit = change(
        "intrusion",
        &private,
        json!({"type":"comment","body":"Intrude","mentions":[]}),
    );
    assert_eq!(post(&workspace, &edit, BOB, 200)?.status, 404);
    let request = serde_json::to_vec(&create("role-denied", "issue", json!({"title":"Denied"})))?;
    assert_eq!(
        workspace
            .response(
                "POST",
                "/api/work/commands",
                &request,
                Role::Viewer,
                "local:outsider",
                200
            )
            .status,
        403
    );
    assert_eq!(
        workspace
            .response(
                "POST",
                "/api/work/commands",
                &request,
                Role::Operator,
                "local:outsider",
                200
            )
            .status,
        403
    );
    assert_eq!(
        workspace
            .response("GET", "/api/work", &[], Role::Worker, ALICE, 200)
            .status,
        403
    );
    assert_eq!(
        workspace
            .response(
                "GET",
                &format!("/api/work/objects/{id}"),
                &[],
                Role::Worker,
                ALICE,
                200
            )
            .status,
        403
    );
    let public = accepted(post(
        &workspace,
        &create(
            "viewer-collaboration",
            "issue",
            json!({"title":"Independent work permission"}),
        ),
        ALICE,
        210,
    )?)?;
    let comment = serde_json::to_vec(&change(
        "viewer-comment",
        &public,
        json!({"type":"comment","body":"Work-only collaborator","mentions":[]}),
    ))?;
    let collaborated = accepted(workspace.response(
        "POST",
        "/api/work/commands",
        &comment,
        Role::Viewer,
        BOB,
        220,
    ))?;
    assert_eq!((*field(&collaborated, "/comments/0/author")), BOB);
    assert_eq!(
        (*field(
            &body(&workspace.response("GET", "/api/work", &[], Role::Viewer, BOB, 230))?,
            "/can_write"
        )),
        true
    );
    assert_eq!(
        (*field(
            &body(&workspace.response(
                "GET",
                "/api/work",
                &[],
                Role::Viewer,
                "local:outsider",
                230
            ))?,
            "/can_write"
        )),
        false
    );
    Ok(())
}

#[test]
fn all_work_kinds_use_distinct_native_types_and_editing_preserves_discussion() -> Result<()> {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    for kind in [
        "issue",
        "project",
        "milestone",
        "cycle",
        "initiative",
        "team",
        "request",
        "view",
        "personal",
    ] {
        let mut fields = json!({"title":format!("Native {kind}")});
        if kind == "view" {
            fields.as_object_mut().ok_or("fixture object absent")?.insert("view".into(), json!({"filters":{},"sort":"updated","group_by":"none","layout":"board","shared":true}));
        }
        if kind == "personal" {
            fields
                .as_object_mut()
                .ok_or("fixture object absent")?
                .insert(
                    "personal".into(),
                    json!({"watched":[],"read":{},"favorites":[],"reminders":{}}),
                );
        }
        if kind == "request" {
            fields
                .as_object_mut()
                .ok_or("fixture object absent")?
                .insert("customer".into(), json!("Customer"));
            fields
                .as_object_mut()
                .ok_or("fixture object absent")?
                .insert(
                    "source_url".into(),
                    json!("https://example.invalid/request/1"),
                );
        }
        let record = accepted(post(
            &workspace,
            &create(&format!("create-{kind}"), kind, fields),
            ALICE,
            100,
        )?)?;
        assert_eq!((*field(&record, "/kind")), kind);
        let repository = fixture.repository()?;
        let refs = repository
            .references_glob(&format!(
                "refs/namespaces/*/refs/cobs/xyz.onixresearch.work.{kind}/*"
            ))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        assert_eq!(refs.len(), 1);
    }
    let issue = accepted(post(
        &workspace,
        &create("editable", "issue", json!({"title":"Before"})),
        ALICE,
        200,
    )?)?;
    let discussion = accepted(post(
        &workspace,
        &change(
            "discussion",
            &issue,
            json!({"type":"comment","body":"Retain me","mentions":[]}),
        ),
        ALICE,
        250,
    )?)?;
    let mut fields = (*field(&discussion, "/fields")).clone();
    fields
        .as_object_mut()
        .ok_or("fixture object absent")?
        .insert("title".into(), json!("After"));
    fields
        .as_object_mut()
        .ok_or("fixture object absent")?
        .insert("status".into(), json!("started"));
    let edited = accepted(post(
        &workspace,
        &change("edit", &discussion, json!({"type":"edit","fields":fields})),
        ALICE,
        300,
    )?)?;
    assert_eq!((*field(&edited, "/fields/title")), "After");
    assert_eq!(
        (*field(&edited, "/comments")),
        (*field(&discussion, "/comments"))
    );
    let bad_relation = create(
        "bad-relation",
        "issue",
        json!({"title":"Broken project","project":"0000000000000000000000000000000000000000"}),
    );
    assert_eq!(post(&workspace, &bad_relation, ALICE, 400)?.status, 400);
    Ok(())
}

#[test]
fn competing_workspaces_never_overwrite_an_exact_head() -> Result<()> {
    let fixture = Fixture::new()?;
    let first_workspace = fixture.workspace()?;
    let record = accepted(post(
        &first_workspace,
        &create("race-root", "issue", json!({"title":"Concurrent"})),
        ALICE,
        100,
    )?)?;
    let barrier = Arc::new(Barrier::new(2));
    let mut writers = Vec::new();
    for number in 0..2 {
        let workspace = fixture.workspace()?;
        let barrier = barrier.clone();
        let command = change(
            &format!("race-{number}"),
            &record,
            json!({"type":"comment","body":format!("writer {number}"),"mentions":[]}),
        );
        writers.push(std::thread::spawn(move || {
            barrier.wait();
            workspace
                .response(
                    "POST",
                    "/api/work/commands",
                    &serde_json::to_vec(&command)
                        .unwrap_or_else(|error| unreachable!("fixture JSON failed: {error:?}")),
                    Role::Operator,
                    ALICE,
                    200,
                )
                .status
        }));
    }
    let statuses: Vec<_> = writers
        .into_iter()
        .map(|writer| {
            writer
                .join()
                .unwrap_or_else(|error| unreachable!("fixture writer failed: {error:?}"))
        })
        .collect();
    assert_eq!(statuses.iter().filter(|status| **status == 200).count(), 1);
    assert!(
        statuses
            .iter()
            .all(|status| [200, 409, 503].contains(status))
    );
    let observed = get(
        &first_workspace,
        (*field(&record, "/id")).as_str().ok_or("id missing")?,
        ALICE,
    );
    assert_eq!(observed.status, 200);
    assert_eq!(
        (*field(&body(&observed)?, "/comments"))
            .as_array()
            .ok_or("comments missing")?
            .len(),
        1
    );
    Ok(())
}

#[test]
fn foreign_heads_unsigned_history_and_unavailable_store_fail_closed() -> Result<()> {
    let foreign_fixture = Fixture::new()?;
    let workspace = foreign_fixture.workspace()?;
    let record = accepted(post(
        &workspace,
        &create("foreign-root", "issue", json!({"title":"Foreign conflict"})),
        ALICE,
        100,
    )?)?;
    let repository = foreign_fixture.repository()?;
    let foreign = radicle_crypto::SigningKey::from_seed(radicle_crypto::Seed::new([8; 32]));
    use radicle_crypto::signature::Keypair as _;
    let namespace = *foreign.verifying_key().public_key();
    repository.reference(
        &format!(
            "refs/namespaces/{namespace}/refs/cobs/xyz.onixresearch.work.issue/{}",
            (*field(&record, "/id")).as_str().ok_or("id missing")?
        ),
        cob_git2::Oid::from_str((*field(&record, "/head")).as_str().ok_or("head missing")?)?,
        false,
        "foreign competing head",
    )?;
    assert_eq!(
        workspace
            .response("GET", "/api/work", &[], Role::Viewer, ALICE, 200)
            .status,
        409
    );
    let unsigned_fixture = Fixture::new()?;
    let unsigned_workspace = unsigned_fixture.workspace()?;
    let record = accepted(post(
        &unsigned_workspace,
        &create(
            "unsigned-root",
            "issue",
            json!({"title":"Corrupt signature"}),
        ),
        ALICE,
        100,
    )?)?;
    let repository = unsigned_fixture.repository()?;
    let native = repository.find_commit(cob_git2::Oid::from_str(
        (*field(&record, "/head")).as_str().ok_or("head missing")?,
    )?)?;
    let signature = Signature::now("Unsigned", "unsigned@example.invalid")?;
    let unsigned = repository.commit(
        None,
        &signature,
        &signature,
        "unsigned forged entry",
        &native.tree()?,
        &[],
    )?;
    let mut references =
        repository.references_glob("refs/namespaces/*/refs/cobs/xyz.onixresearch.work.issue/*")?;
    let mut reference = references.next().ok_or("native ref missing")??;
    reference.set_target(unsigned, "corruption fixture")?;
    assert_eq!(
        unsigned_workspace
            .response("GET", "/api/work", &[], Role::Viewer, ALICE, 200)
            .status,
        503
    );
    let unavailable_fixture = Fixture::new()?;
    let unavailable_workspace = unavailable_fixture.workspace()?;
    std::fs::rename(
        &unavailable_fixture.profile.git_directory,
        unavailable_fixture.temp.path().join("moved.git"),
    )?;
    assert_eq!(
        unavailable_workspace
            .response("GET", "/api/work", &[], Role::Viewer, ALICE, 200)
            .status,
        503
    );
    Ok(())
}

#[test]
fn explicit_profile_and_owned_private_raw_seed_are_required() -> Result<()> {
    let fixture = Fixture::new()?;
    let mut profile = fixture.profile.clone();
    profile.schema = "kiln.web-work.v2".into();
    assert!(profile.admit().is_err());
    profile = fixture.profile.clone();
    profile.git_directory = "relative.git".into();
    assert!(profile.admit().is_err());
    profile = fixture.profile.clone();
    profile.write_principals.push(ALICE.into());
    assert!(profile.admit().is_err());
    let profile_file = fixture.temp.path().join("work.json");
    std::fs::write(&profile_file, serde_json::to_vec(&fixture.profile)?)?;
    assert_eq!(
        Profile::load(&profile_file)?.git_directory,
        fixture.profile.git_directory
    );
    std::fs::set_permissions(
        &fixture.profile.signer_seed_file,
        std::fs::Permissions::from_mode(0o644),
    )?;
    assert!(fixture.workspace().is_err());
    std::fs::set_permissions(
        &fixture.profile.signer_seed_file,
        std::fs::Permissions::from_mode(0o600),
    )?;
    let linked_seed = fixture.temp.path().join("linked.seed");
    symlink(&fixture.profile.signer_seed_file, &linked_seed)?;
    profile = fixture.profile.clone();
    profile.signer_seed_file = linked_seed;
    assert!(Workspace::new(Some(profile)).is_err());
    std::fs::write(&fixture.profile.signer_seed_file, [7_u8; 31])?;
    assert!(fixture.workspace().is_err());
    let absent = Workspace::new(None)?;
    let response = absent.response("GET", "/api/work", &[], Role::Viewer, ALICE, 100);
    assert_eq!(response.status, 200);
    assert_eq!((*field(&body(&response)?, "/configured")), false);
    assert_eq!((*field(&body(&response)?, "/can_write")), false);
    Ok(())
}

#[test]
fn same_millisecond_cross_object_dependencies_replay_in_signed_causal_order() -> Result<()> {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let team = accepted(post(
        &workspace,
        &create(
            "causal-team",
            "team",
            json!({
                "title":"Team","workflow":["backlog","doing","done"]
            }),
        ),
        ALICE,
        500,
    )?)?;
    let project = accepted(post(
        &workspace,
        &create(
            "causal-project",
            "project",
            json!({
                "title":"Project","team":(*field(&team, "/id"))
            }),
        ),
        ALICE,
        500,
    )?)?;
    let milestone = accepted(post(
        &workspace,
        &create(
            "causal-milestone",
            "milestone",
            json!({
                "title":"Milestone","project":(*field(&project, "/id"))
            }),
        ),
        ALICE,
        500,
    )?)?;
    let issue = accepted(post(
        &workspace,
        &create(
            "causal-issue",
            "issue",
            json!({
                "title":"Related issue","team":(*field(&team, "/id")),"project":(*field(&project, "/id")),
                "milestone":(*field(&milestone, "/id")),"status":"doing"
            }),
        ),
        ALICE,
        500,
    )?)?;
    let private = accepted(post(
        &workspace,
        &create(
            "causal-personal",
            "personal",
            json!({
                "title":"My selections","personal":{"watched":[(*field(&issue, "/id"))],
                    "favorites":[(*field(&issue, "/id"))],"read":{},"reminders":{}}
            }),
        ),
        ALICE,
        500,
    )?)?;
    assert_eq!(
        (*field(&private, "/fields/personal/watched")),
        json!([(*field(&issue, "/id"))])
    );
    let before = workspace.response("GET", "/api/work", &[], Role::Viewer, ALICE, 500);
    assert_eq!(before.status, 200);
    drop(workspace);
    let reopened = fixture.workspace()?;
    let after = reopened.response("GET", "/api/work", &[], Role::Viewer, ALICE, 500);
    assert_eq!(after.status, 200);
    assert_eq!(body(&after)?, body(&before)?);
    let repository = fixture.repository()?;
    let references =
        repository.references_glob("refs/namespaces/*/refs/cobs/xyz.onixresearch.work.*/*")?;
    let store = GitCobStore::new(&repository);
    let mut ordered = Vec::new();
    for reference in references {
        let reference = reference?;
        let head = reference.target().ok_or("target missing")?;
        let entry = store.load(head.into())?;
        let envelope: Value = serde_json::from_slice(&entry.contents.head)?;
        ordered.push((
            (*field(&envelope, "/sequence"))
                .as_u64()
                .ok_or("sequence missing")?,
            head.to_string(),
            (*field(&envelope, "/previous_operation_head")).clone(),
        ));
    }
    ordered.sort_by_key(|event| event.0);
    assert_eq!(
        ordered.first().ok_or("fixture sequence absent")?.2,
        Value::Null
    );
    for pair in ordered.windows(2) {
        assert_eq!(
            pair.get(1).ok_or("fixture sequence absent")?.0,
            pair.first().ok_or("fixture sequence absent")?.0 + 1
        );
        assert_eq!(
            pair.get(1).ok_or("fixture sequence absent")?.2,
            pair.first().ok_or("fixture sequence absent")?.1
        );
    }
    Ok(())
}

#[test]
fn native_bounds_and_message_authorship_reject_without_changing_accepted_state() -> Result<()> {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let root = accepted(post(
        &workspace,
        &create("author-root", "issue", json!({"title":"Authorship"})),
        ALICE,
        100,
    )?)?;
    let comment = accepted(post(
        &workspace,
        &change(
            "author-comment",
            &root,
            json!({"type":"comment","body":"Original","mentions":[]}),
        ),
        ALICE,
        200,
    )?)?;
    let change_comment = change(
        "foreign-edit",
        &comment,
        json!({"type":"edit_comment","comment":"message:author-comment","body":"Forged author","mentions":[]}),
    );
    assert_eq!(post(&workspace, &change_comment, BOB, 300)?.status, 400);
    let valid_edit = change(
        "author-edit",
        &comment,
        json!({"type":"edit_comment","comment":"message:author-comment","body":"Revised","mentions":[]}),
    );
    let edited = accepted(post(&workspace, &valid_edit, ALICE, 300)?)?;
    assert_eq!((*field(&edited, "/comments/0/body")), "Revised");
    assert_eq!((*field(&edited, "/comments/0/edited_at_ms")), 300);
    let oversized = vec![b' '; 262_145];
    assert_eq!(
        workspace
            .response(
                "POST",
                "/api/work/commands",
                &oversized,
                Role::Operator,
                ALICE,
                400
            )
            .status,
        400
    );
    let malformed = br#"{"request_id":"unknown","object_id":null,"expected_head":null,"action":{"type":"create","kind":"issue","fields":{"title":"Rejected","authority":"operator"}}}"#;
    assert_eq!(
        workspace
            .response(
                "POST",
                "/api/work/commands",
                malformed,
                Role::Operator,
                ALICE,
                400
            )
            .status,
        400
    );
    assert_eq!(
        body(&get(
            &workspace,
            (*field(&edited, "/id")).as_str().ok_or("id missing")?,
            ALICE
        ))?,
        edited
    );
    let repository = fixture.repository()?;
    let target =
        cob_git2::Oid::from_str((*field(&edited, "/head")).as_str().ok_or("head missing")?)?;
    let current = repository.find_commit(target)?;
    let mut builder = repository.treebuilder(Some(&current.tree()?))?;
    builder.insert("0", repository.blob(&oversized)?, 0o100644)?;
    let tree = repository.find_tree(builder.write()?)?;
    let signature = Signature::now("Bound fixture", "bound@example.invalid")?;
    let corrupt = repository.commit(
        None,
        &signature,
        &signature,
        "oversized entry",
        &tree,
        &[&current],
    )?;
    let mut refs =
        repository.references_glob("refs/namespaces/*/refs/cobs/xyz.onixresearch.work.issue/*")?;
    refs.next()
        .ok_or("native ref missing")??
        .set_target(corrupt, "bounded corruption")?;
    assert_eq!(
        workspace
            .response("GET", "/api/work", &[], Role::Viewer, ALICE, 500)
            .status,
        503
    );
    Ok(())
}

fn board(workspace: &Workspace, query: &str, actor: &str) -> ApiResponse {
    let target = if query.is_empty() {
        "/api/work/board".to_owned()
    } else {
        format!("/api/work/board?{query}")
    };
    workspace.response("GET", &target, &[], Role::Viewer, actor, 999)
}
fn text<'a>(value: &'a Value, path: &str) -> Result<&'a str> {
    Ok(field(value, path)
        .as_str()
        .ok_or_else(|| format!("{path} is not text"))?)
}
fn column_statuses(projection: &Value) -> Result<Vec<String>> {
    Ok(field(projection, "/columns")
        .as_array()
        .ok_or("columns missing")?
        .iter()
        .map(|lane| {
            field(lane, "/status")
                .as_str()
                .unwrap_or_default()
                .to_owned()
        })
        .collect())
}
fn lane<'a>(projection: &'a Value, status: &str) -> Result<&'a Value> {
    Ok(field(projection, "/columns")
        .as_array()
        .ok_or("columns missing")?
        .iter()
        .find(|lane| field(lane, "/status") == status)
        .ok_or_else(|| format!("column {status} missing"))?)
}
fn card_ids(lane: &Value) -> Vec<String> {
    field(lane, "/cards")
        .as_array()
        .map(|cards| {
            cards
                .iter()
                .map(|card| field(card, "/id").as_str().unwrap_or_default().to_owned())
                .collect()
        })
        .unwrap_or_default()
}
fn set(value: &mut Value, path: &str, replacement: Value) -> Result<()> {
    let (parent, key) = path.rsplit_once('/').ok_or("path must start with /")?;
    value
        .pointer_mut(parent)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| format!("{parent} is not an object"))?
        .insert(key.to_owned(), replacement);
    Ok(())
}
fn set_status(request: &str, record: &Value, status: &str) -> Value {
    change(
        request,
        record,
        json!({"type":"set_status","status":status}),
    )
}

struct Board {
    alpha: Value,
    review: Value,
    deploying: Value,
    request: Value,
    view: Value,
    private: Value,
}

/// Two teams with their own statuses, three issues, a request and two views:
/// a shared team-scoped board with configured columns and Alice's private list view.
fn board_fixture(workspace: &Workspace) -> Result<Board> {
    let alpha = accepted(post(
        workspace,
        &create(
            "team-alpha",
            "team",
            json!({"title":"Alpha","workflow":["review"]}),
        ),
        ALICE,
        100,
    )?)?;
    let beta = accepted(post(
        workspace,
        &create(
            "team-beta",
            "team",
            json!({"title":"Beta","workflow":["deploying"]}),
        ),
        ALICE,
        101,
    )?)?;
    let alpha_id = text(&alpha, "/id")?;
    let backlog = accepted(post(
        workspace,
        &create(
            "issue-backlog",
            "issue",
            json!({"title":"Fix login","team":alpha_id,"labels":["bug"]}),
        ),
        ALICE,
        102,
    )?)?;
    let review = accepted(post(
        workspace,
        &create(
            "issue-review",
            "issue",
            json!({"title":"Review docs","team":alpha_id,"status":"review"}),
        ),
        ALICE,
        103,
    )?)?;
    let deploying = accepted(post(
        workspace,
        &create(
            "issue-deploying",
            "issue",
            json!({"title":"Deploy","team":text(&beta, "/id")?,"status":"deploying"}),
        ),
        ALICE,
        104,
    )?)?;
    let request = accepted(post(
        workspace,
        &create(
            "request-one",
            "request",
            json!({"title":"Customer ask","status":"triage","customer":"Acme"}),
        ),
        ALICE,
        105,
    )?)?;
    let view = accepted(post(
        workspace,
        &create(
            "view-board",
            "view",
            json!({"title":"Alpha board","view":{"filters":{"team":alpha_id},"sort":"title","group_by":"none","layout":"board","shared":true,
                "board":{"columns":["triage","backlog","review","completed"]}}}),
        ),
        ALICE,
        106,
    )?)?;
    let private = accepted(post(
        workspace,
        &create(
            "view-private",
            "view",
            json!({"title":"Mine","view":{"filters":{},"sort":"updated","group_by":"status","layout":"list","shared":false}}),
        ),
        ALICE,
        107,
    )?)?;
    let _ = backlog;
    Ok(Board {
        alpha,
        review,
        deploying,
        request,
        view,
        private,
    })
}

// r[verify kiln.work.kanban.config] r[verify kiln.work.kanban.columns] r[verify kiln.work.kanban.projection]
// r[verify kiln.work.kanban.boundary]
#[test]
fn kanban_view_survives_restart_is_shared_and_shows_the_list_eligible_issues() -> Result<()> {
    let fixture = Fixture::new()?;
    let made = board_fixture(&fixture.workspace()?)?;
    let view_id = text(&made.view, "/id")?;
    // A fresh workspace replays the signed history: the configuration is durable.
    let workspace = fixture.workspace()?;
    let shared = body(&board(&workspace, &format!("view={view_id}"), BOB))?;
    assert_eq!(field(&shared, "/schema"), "kiln.work.board.v1");
    assert_eq!(field(&shared, "/principal"), BOB);
    assert_eq!(field(&shared, "/view/id"), view_id);
    assert_eq!(field(&shared, "/view/head"), field(&made.view, "/head"));
    assert_eq!(field(&shared, "/view/configured_columns"), true);
    assert_eq!(
        column_statuses(&shared)?,
        ["triage", "backlog", "review", "completed"]
    );
    // Empty configured columns stay visible in configured order.
    assert_eq!(field(lane(&shared, "triage")?, "/total"), 0);
    assert_eq!(field(lane(&shared, "completed")?, "/total"), 0);
    assert_eq!(
        field(lane(&shared, "review")?, "/teams/0"),
        field(&made.alpha, "/id")
    );
    assert_eq!(
        card_ids(lane(&shared, "review")?),
        [text(&made.review, "/id")?]
    );
    // List eligibility over the same snapshot: Alpha issues only.
    let listing = body(&workspace.response("GET", "/api/work", &[], Role::Viewer, BOB, 999))?;
    let mut expected: Vec<String> = field(&listing, "/objects")
        .as_array()
        .ok_or("objects missing")?
        .iter()
        .filter(|record| {
            field(record, "/kind") == "issue"
                && record.pointer("/fields/team") == Some(field(&made.alpha, "/id"))
        })
        .map(|record| field(record, "/id").as_str().unwrap_or_default().to_owned())
        .collect();
    expected.sort();
    let mut shown: Vec<String> = field(&shared, "/columns")
        .as_array()
        .ok_or("columns missing")?
        .iter()
        .flat_map(card_ids)
        .collect();
    shown.sort();
    assert_eq!(shown, expected);
    assert_eq!(field(&shared, "/matched_issues"), 2);
    assert_eq!(field(&shared, "/complete"), true);
    assert_eq!(field(&shared, "/filtered"), true);
    assert_eq!(field(&shared, "/object_limit"), 1024);
    assert!(
        field(&shared, "/non_claims/2")
            .as_str()
            .is_some_and(|claim| claim.contains("no execution lease"))
    );
    // URL overrides narrow the saved filters without writing the view.
    let narrowed = body(&board(
        &workspace,
        &format!("view={view_id}&status=review&sort=updated"),
        BOB,
    ))?;
    assert_eq!(field(&narrowed, "/matched_issues"), 1);
    assert_eq!(field(&narrowed, "/sort"), "updated");
    // Without a saved view the Work page defaults apply: issues as cards,
    // the matching request only in the list, every default column present.
    let unsaved = body(&board(&workspace, "", BOB))?;
    assert_eq!(field(&unsaved, "/view"), &Value::Null);
    assert_eq!(field(&unsaved, "/matched_issues"), 3);
    assert_eq!(field(&unsaved, "/list_only"), 1);
    assert_eq!(
        column_statuses(&unsaved)?,
        [
            "triage",
            "backlog",
            "started",
            "review",
            "deploying",
            "completed",
            "cancelled",
            "snoozed",
            "duplicate"
        ]
    );
    let _ = (&made.deploying, &made.request);
    Ok(())
}

// r[verify kiln.work.kanban.projection] r[verify kiln.work.kanban.config.scenario.migration]
#[test]
fn private_views_stay_private_and_old_views_use_default_columns() -> Result<()> {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let made = board_fixture(&workspace)?;
    let private_id = text(&made.private, "/id")?;
    // The private view has no board configuration (written as before boards existed).
    assert_eq!(made.private.pointer("/fields/view/board"), None);
    let missing = board(&workspace, &format!("view={private_id}"), BOB);
    assert_eq!(missing.status, 404);
    assert_eq!(field(&body(&missing)?, "/code"), "work_object_missing");
    let absent = board(
        &workspace,
        "view=0123456789abcdef0123456789abcdef01234567",
        BOB,
    );
    assert_eq!(
        absent.body, missing.body,
        "missing and private are indistinguishable"
    );
    let mine = body(&board(&workspace, &format!("view={private_id}"), ALICE))?;
    assert_eq!(field(&mine, "/view/configured_columns"), false);
    assert_eq!(column_statuses(&mine)?.len(), 9);
    assert_eq!(field(&mine, "/matched_issues"), 3);
    // Saved views list every non-personal kind: two teams, the request and both views.
    assert_eq!(field(&mine, "/list_only"), 5);
    // A non-view object is not a board.
    let not_view = board(
        &workspace,
        &format!("view={}", text(&made.review, "/id")?),
        ALICE,
    );
    assert_eq!(not_view.status, 404);
    // Malformed, unknown and repeated parameters are rejected, never ignored.
    for query in [
        "view=nope",
        "team=x",
        "sort=random",
        "q=a&q=b",
        "q=%0A",
        "q",
    ] {
        let response = board(&workspace, query, ALICE);
        assert_eq!(response.status, 400, "{query}");
        assert_eq!(field(&body(&response)?, "/code"), "work_query_invalid");
    }
    let posted = workspace.response("POST", "/api/work/board", b"{}", Role::Operator, ALICE, 999);
    assert_eq!(posted.status, 405);
    let worker = workspace.response("GET", "/api/work/board", &[], Role::Worker, ALICE, 999);
    assert_eq!(worker.status, 403);
    let unconfigured = Workspace::new(None)?;
    assert_eq!(board(&unconfigured, "", ALICE).status, 404);
    Ok(())
}

// r[verify kiln.work.kanban.move] r[verify kiln.work.kanban.move.scenario.retry]
#[test]
fn moving_a_card_writes_one_issue_status_operation_seen_by_every_view() -> Result<()> {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let made = board_fixture(&workspace)?;
    let view_id = text(&made.view, "/id")?;
    let private_id = text(&made.private, "/id")?;
    let before = body(&board(&workspace, &format!("view={view_id}"), ALICE))?;
    let card = (*field(lane(&before, "review")?, "/cards/0")).clone();
    let command = set_status("move-one", &card, "completed");
    let moved = accepted(post(&workspace, &command, ALICE, 500)?)?;
    assert_eq!(field(&moved, "/fields/status"), "completed");
    assert_eq!(field(&moved, "/fields/title"), "Review docs");
    assert_eq!(field(&moved, "/fields/team"), field(&made.alpha, "/id"));
    // Exactly one native operation: the new head's only parent is the prior head.
    let repository = fixture.repository()?;
    let head = repository.find_commit(cob_git2::Oid::from_str(text(&moved, "/head")?)?)?;
    assert_eq!(head.parent_count(), 1);
    assert_eq!(head.parent_id(0)?.to_string(), text(&card, "/head")?);
    // An identical retry returns the original result and writes nothing.
    let retried = post(&workspace, &command, ALICE, 600)?;
    assert_eq!(retried.status, 200);
    let retried = body(&retried)?;
    assert_eq!(field(&retried, "/replayed"), true);
    assert_eq!(field(&retried, "/object/head"), field(&moved, "/head"));
    let current = body(&get(&workspace, text(&moved, "/id")?, ALICE))?;
    assert_eq!(field(&current, "/head"), field(&moved, "/head"));
    // The same request ID with a different target is a request conflict.
    let changed = post(
        &workspace,
        &set_status("move-one", &card, "backlog"),
        ALICE,
        600,
    )?;
    assert_eq!(changed.status, 409);
    assert_eq!(field(&body(&changed)?, "/code"), "work_request_conflict");
    // Every including view sees the move; no view was written.
    let shared = body(&board(&workspace, &format!("view={view_id}"), BOB))?;
    assert_eq!(
        card_ids(lane(&shared, "completed")?),
        [text(&moved, "/id")?]
    );
    assert_eq!(field(lane(&shared, "review")?, "/total"), 0);
    assert_eq!(field(&shared, "/view/head"), field(&made.view, "/head"));
    let private = body(&board(&workspace, &format!("view={private_id}"), ALICE))?;
    assert_eq!(field(&private, "/view/head"), field(&made.private, "/head"));
    assert!(card_ids(lane(&private, "completed")?).contains(&text(&moved, "/id")?.to_owned()));
    // The target must belong to the issue's team workflow; requests do not move.
    let wrong_team = post(
        &workspace,
        &set_status("move-wrong-team", &moved, "deploying"),
        ALICE,
        700,
    )?;
    assert_eq!(wrong_team.status, 400);
    let request = post(
        &workspace,
        &set_status("move-request", &made.request, "backlog"),
        ALICE,
        700,
    )?;
    assert_eq!(request.status, 400);
    let unchanged = body(&get(&workspace, text(&moved, "/id")?, ALICE))?;
    assert_eq!(field(&unchanged, "/head"), field(&moved, "/head"));
    let _ = &made.deploying;
    Ok(())
}

// r[verify kiln.work.kanban.move] r[verify kiln.work.kanban.projection]
#[test]
fn board_configuration_and_moves_keep_existing_write_authority() -> Result<()> {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let made = board_fixture(&workspace)?;
    let command = set_status("move-denied", &made.review, "completed");
    // An authenticated principal outside write_principals may read but not move.
    let outsider = "local:outsider";
    assert_eq!(board(&workspace, "", outsider).status, 200);
    assert_eq!(
        field(&body(&board(&workspace, "", outsider))?, "/can_write"),
        false
    );
    let denied = post(&workspace, &command, outsider, 500)?;
    assert_eq!(denied.status, 403);
    assert_eq!(field(&body(&denied)?, "/code"), "work_write_denied");
    let worker = workspace.response(
        "POST",
        "/api/work/commands",
        &serde_json::to_vec(&command)?,
        Role::Worker,
        ALICE,
        500,
    );
    assert_eq!(worker.status, 403);
    // A board column change on another principal's private view stays hidden.
    let mut fields = (*field(&made.private, "/fields")).clone();
    set(&mut fields, "/view/board", json!({"columns":["backlog"]}))?;
    let hidden = post(
        &workspace,
        &change(
            "bob-edits-private",
            &made.private,
            json!({"type":"edit","fields":fields}),
        ),
        BOB,
        500,
    )?;
    assert_eq!(hidden.status, 404);
    // Extending the configuration does not unlock the immutable visibility.
    let mut shared = (*field(&made.private, "/fields")).clone();
    set(&mut shared, "/view/shared", json!(true))?;
    set(&mut shared, "/view/board", json!({"columns":["backlog"]}))?;
    let exposed = post(
        &workspace,
        &change(
            "share-private",
            &made.private,
            json!({"type":"edit","fields":shared}),
        ),
        ALICE,
        500,
    )?;
    assert_eq!(exposed.status, 400);
    // Undefined columns are rejected when introduced.
    let mut undefined = (*field(&made.view, "/fields")).clone();
    set(
        &mut undefined,
        "/view/board",
        json!({"columns":["backlog","nonexistent"]}),
    )?;
    let rejected = post(
        &workspace,
        &change(
            "undefined-column",
            &made.view,
            json!({"type":"edit","fields":undefined}),
        ),
        ALICE,
        500,
    )?;
    assert_eq!(rejected.status, 400);
    let unchanged = body(&get(&workspace, text(&made.view, "/id")?, ALICE))?;
    assert_eq!(field(&unchanged, "/head"), field(&made.view, "/head"));
    let issue = body(&get(&workspace, text(&made.review, "/id")?, ALICE))?;
    assert_eq!(field(&issue, "/fields/status"), "review");
    Ok(())
}

// r[verify kiln.work.kanban.move.scenario.stale]
#[test]
fn stale_and_concurrent_moves_never_overwrite_a_newer_head() -> Result<()> {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let made = board_fixture(&workspace)?;
    let card = made.review.clone();
    // Bob edits the title first; Alice's board still holds the old head.
    let mut fields = (*field(&card, "/fields")).clone();
    set(&mut fields, "/title", json!("Review docs (renamed)"))?;
    let renamed = accepted(post(
        &workspace,
        &change("bob-rename", &card, json!({"type":"edit","fields":fields})),
        BOB,
        500,
    )?)?;
    let stale = post(
        &workspace,
        &set_status("alice-stale", &card, "completed"),
        ALICE,
        600,
    )?;
    assert_eq!(stale.status, 409);
    assert_eq!(field(&body(&stale)?, "/code"), "work_head_conflict");
    let current = body(&get(&workspace, text(&card, "/id")?, ALICE))?;
    assert_eq!(field(&current, "/head"), field(&renamed, "/head"));
    assert_eq!(field(&current, "/fields/status"), "review");
    // After reloading, the user's explicit new request applies to the new head
    // and keeps the concurrent rename.
    let resolved = accepted(post(
        &workspace,
        &set_status("alice-resolved", &current, "completed"),
        ALICE,
        700,
    )?)?;
    assert_eq!(field(&resolved, "/fields/title"), "Review docs (renamed)");
    // Two workspaces moving the same head concurrently: exactly one applies.
    let barrier = Arc::new(Barrier::new(2));
    let mut writers = Vec::new();
    for (number, status) in ["backlog", "triage"].into_iter().enumerate() {
        let racer = fixture.workspace()?;
        let barrier = barrier.clone();
        let command =
            serde_json::to_vec(&set_status(&format!("race-{number}"), &resolved, status))?;
        writers.push(std::thread::spawn(move || {
            barrier.wait();
            racer
                .response(
                    "POST",
                    "/api/work/commands",
                    &command,
                    Role::Operator,
                    ALICE,
                    800,
                )
                .status
        }));
    }
    let statuses: Vec<u16> = writers
        .into_iter()
        .map(|writer| {
            writer
                .join()
                .unwrap_or_else(|error| unreachable!("fixture writer failed: {error:?}"))
        })
        .collect();
    assert_eq!(
        statuses.iter().filter(|status| **status == 200).count(),
        1,
        "{statuses:?}"
    );
    assert!(
        statuses
            .iter()
            .all(|status| [200, 409, 503].contains(status))
    );
    let settled = body(&get(&workspace, text(&card, "/id")?, ALICE))?;
    let repository = fixture.repository()?;
    let head = repository.find_commit(cob_git2::Oid::from_str(text(&settled, "/head")?)?)?;
    assert_eq!(head.parent_id(0)?.to_string(), text(&resolved, "/head")?);
    assert!(["backlog", "triage"].contains(&text(&settled, "/fields/status")?));
    Ok(())
}
