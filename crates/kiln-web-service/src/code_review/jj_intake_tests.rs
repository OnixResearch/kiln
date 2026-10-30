//! Change-ID intake through the real shell path over fixture repositories
//! built by a real `jj` (feature `jj-fixtures`). Kiln only observes them.
#![allow(clippy::too_many_lines)]
use std::error::Error;
use std::os::unix::fs::PermissionsExt;

use kiln_adapter_jj::fixtures::Fixture as JjRepo;
use tempfile::TempDir;

use super::jj_intake::kiln_change;
use super::*;

type TestResult<T = ()> = std::result::Result<T, Box<dyn Error>>;
const ALICE: &str = "local:author";
const BOB: &str = "local:reviewer";
const MAIN: &str = "refs/heads/main";

struct Harness {
    _temp: TempDir,
    jj: JjRepo,
    profile: Profile,
    workspace: Workspace,
    clock: std::cell::Cell<i64>,
}

impl Harness {
    fn new() -> TestResult<Self> {
        let temp = tempfile::tempdir()?;
        std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700))?;
        let jj = JjRepo::new(&temp.path().join("repo"));
        let root = jj.root.canonicalize()?;
        let identity =
            kiln_codec::identity_from_bytes(root.to_str().ok_or("path encoding")?.as_bytes())?;
        let profile: Profile = serde_json::from_value(json!({
            "schema":"kiln.code-review-profile.v1",
            "database":temp.path().join("review.sqlite"),
            "project":"project",
            "provider":{"repository_id":identity.as_str(),"repository_path":root,
                "target_refs":[MAIN],"capabilities":["inspect","compare","register"]},
            "policy":{"id":"review-policy","eligible_reviewers":[BOB],"submitters":[BOB],
                "minimum_approvals":1,"require_ci":true,
                "ownership":{"teams":{},"rules":[]}},
            "read_principals":[ALICE,BOB],"write_principals":[ALICE,BOB],
            "signature_name":"Review service","signature_email":"review@example.invalid",
            "intake":{"default_target_ref":MAIN,"mappings":[],
                "jujutsu":{"workspace_path":root,"executable":jj.executable,
                    "change_id_header":true,"max_stack":8}}
        }))?;
        let workspace = Workspace::new(Some(profile.clone()))?;
        Ok(Self {
            _temp: temp,
            jj,
            profile,
            workspace,
            clock: std::cell::Cell::new(1_000),
        })
    }

    fn post(&self, request_id: &str, action: Value, actor: &str) -> TestResult<ApiResponse> {
        let now = self.clock.get() + 1_000;
        self.clock.set(now);
        let body = json!({"request_id":request_id,"change_id":null,"expected_version":null,"action":action});
        Ok(self.workspace.response(
            "POST",
            "/api/code-review/commands",
            &serde_json::to_vec(&body)?,
            RequestContext {
                role: Role::Viewer,
                principal: actor,
                now_ms: now,
                ci: &[],
            },
        ))
    }

    fn submit(&self, request_id: &str, ids: &[&str], stack: bool) -> TestResult<ApiResponse> {
        self.post(
            request_id,
            json!({"type":"jj_contribute","change_ids":ids,"stack":stack,"wip":true}),
            ALICE,
        )
    }

    fn detail(&self, id: &str) -> TestResult<Value> {
        let response = self.workspace.response(
            "GET",
            &format!("/api/code-review/changes/{id}"),
            &[],
            RequestContext {
                role: Role::Viewer,
                principal: ALICE,
                now_ms: self.clock.get(),
                ci: &[],
            },
        );
        accepted(response)
    }

    fn change(&self, jj_change: &str) -> TestResult<String> {
        kiln_change(&self.profile.provider.repository_id, jj_change)
            .map_err(|error| format!("{error:?}").into())
    }

    fn revisions(&self, jj_change: &str) -> TestResult<usize> {
        let detail = self.detail(&self.change(jj_change)?)?;
        Ok(field(&detail, "/change/revisions")?
            .as_array()
            .ok_or("revisions")?
            .len())
    }

    fn dependencies(&self, jj_change: &str) -> TestResult<Vec<String>> {
        let detail = self.detail(&self.change(jj_change)?)?;
        Ok(field(&detail, "/change/dependencies")?
            .as_array()
            .ok_or("dependencies")?
            .iter()
            .filter_map(|edge| {
                edge.get("change")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .collect())
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

    fn kiln_refs(&self) -> TestResult<usize> {
        let repository = git2::Repository::open(&self.jj.root)?;
        Ok(repository.references_glob("refs/kiln/*")?.count())
    }
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

fn refused(response: &ApiResponse, status: u16, code: &str) -> TestResult {
    let body: Value = serde_json::from_slice(&response.body)?;
    assert_eq!(
        (response.status, body.get("code").and_then(Value::as_str)),
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

fn outcomes(body: &Value) -> TestResult<Vec<String>> {
    Ok(field(body, "/jj_intake/members")?
        .as_array()
        .ok_or("members")?
        .iter()
        .map(|member| {
            member
                .get("outcome")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned()
        })
        .collect())
}

// r[verify kiln.review.jj_intake.scenario.rewritten]
// r[verify kiln.review.jj_intake.scenario.replay]
// r[verify kiln.review.jj_intake.observations]
// r[verify kiln.review.jj_intake.evolog]
// r[verify kiln.review.jj_intake.read_only.scenario.unchanged]
#[test]
fn rewritten_change_gives_one_change_with_three_revisions_and_predecessors() -> TestResult {
    let h = Harness::new()?;
    let change =
        h.jj.commit("a.txt", "one\n", "Add a\n\nFirst version body.");
    let first = h.jj.commit_id(&change);
    let before = h.jj.fingerprint();
    let created = accepted(h.submit("r1", &[&change], false)?)?;
    assert_eq!(outcomes(&created)?, ["created"]);
    assert_eq!(h.jj.fingerprint(), before, "Kiln must not mutate jj state");
    let id = h.change(&change)?;
    assert_eq!(text(&created, "/change/id")?, id);
    h.jj.rewrite(&change, "a.txt", "two\n");
    let second = h.jj.commit_id(&change);
    assert_eq!(
        outcomes(&accepted(h.submit("r2", &[&change], false)?)?)?,
        ["appended"]
    );
    h.jj.rewrite(&change, "a.txt", "three\n");
    let third = h.jj.commit_id(&change);
    let before = h.jj.fingerprint();
    assert_eq!(
        outcomes(&accepted(h.submit("r3", &[&change], false)?)?)?,
        ["appended"]
    );
    assert_eq!(h.jj.fingerprint(), before);

    let detail = h.detail(&id)?;
    assert_eq!(text(&detail, "/change/title")?, "Add a");
    assert_eq!(text(&detail, "/change/description")?, "First version body.");
    let revisions = field(&detail, "/change/revisions")?
        .as_array()
        .ok_or("revisions")?;
    let commits: Vec<_> = revisions
        .iter()
        .map(|r| r.get("commit").and_then(Value::as_str).unwrap_or(""))
        .collect();
    assert_eq!(commits, [first.as_str(), second.as_str(), third.as_str()]);
    let evolog = field(&detail, "/jj_evolog")?.as_array().ok_or("evolog")?;
    assert_eq!(evolog.len(), 3);
    let mut previous: Option<&Value> = None;
    for (index, (row, revision)) in evolog.iter().zip(revisions).enumerate() {
        assert_eq!(text(row, "/jj/source")?, "jj");
        assert_eq!(text(row, "/jj/change_id")?, change);
        assert_eq!(text(row, "/jj/operation")?.len(), 128);
        assert_eq!(text(row, "/jj/author_email")?, "author@example.org");
        if let Some(previous) = previous {
            let commit = field(previous, "/commit")?;
            assert!(
                field(row, "/jj/predecessors")?
                    .as_array()
                    .is_some_and(|p| p.contains(commit)),
                "revision {index} must record the previous commit as its jj predecessor: {row}"
            );
            assert_eq!(field(row, "/predecessor_matches_previous")?, true);
            assert_eq!(
                text(row, "/interdiff")?,
                format!("{}..{}", text(previous, "/id")?, text(revision, "/id")?)
            );
        } else {
            assert!(field(row, "/interdiff")?.is_null());
        }
        previous = Some(revision);
    }
    assert_eq!(text(&detail, "/jj_stack/jj_change_id")?, change);

    // Duplicate submissions append nothing: a new request id, then a replay.
    let duplicate = accepted(h.submit("r4", &[&change], false)?)?;
    assert_eq!(outcomes(&duplicate)?, ["unchanged"]);
    let replay = accepted(h.submit("r3", &[&change], false)?)?;
    assert_eq!(field(&replay, "/replayed")?, true);
    assert_eq!(h.revisions(&change)?, 3);
    // The same request id with a different body is a conflict, not a revision.
    refused(
        &h.submit("r3", &[&change], true)?,
        409,
        "review_request_conflict",
    )?;
    assert_eq!(h.revisions(&change)?, 3);
    Ok(())
}

fn text(value: &Value, path: &str) -> TestResult<String> {
    Ok(field(value, path)?.as_str().ok_or("not text")?.to_owned())
}

// r[verify kiln.review.jj_intake.admission]
// r[verify kiln.review.jj_intake.admission.scenario.divergent]
// r[verify kiln.review.jj_intake.admission.scenario.conflicted]
#[test]
fn divergent_conflicted_and_malformed_inputs_are_refused_before_any_effect() -> TestResult {
    let h = Harness::new()?;
    let healthy = h.jj.commit("ok.txt", "ok\n", "Healthy");
    let leaf = h.jj.commit("a.txt", "a\n", "Leaf");
    let operation = h.jj.jj(&["op", "log", "--no-graph", "-n1", "-T", "id"]);
    h.jj.jj(&["describe", "-r", &leaf, "-m", "Leaf one"]);
    h.jj.jj(&[
        "--at-op", &operation, "describe", "-r", &leaf, "-m", "Leaf two",
    ]);
    // Two operation heads: refused without merging them.
    refused(
        &h.submit("d0", &[&leaf], false)?,
        409,
        "review_jj_operation_divergent",
    )?;
    h.jj.jj(&["op", "log", "-n1"]);
    let before = h.jj.fingerprint();
    refused(
        &h.submit("d1", &[&leaf], false)?,
        409,
        "review_jj_divergent_change",
    )?;
    // One refused member refuses the whole stack before any effect.
    refused(
        &h.submit("d2", &[&healthy, &leaf], true)?,
        409,
        "review_jj_divergent_change",
    )?;
    assert_eq!(h.jj.fingerprint(), before);

    h.jj.jj(&["new", "main"]);
    let left = h.jj.commit("c.txt", "left\n", "Left");
    h.jj.jj(&["new", "main"]);
    let right = h.jj.commit("c.txt", "right\n", "Right");
    h.jj.jj(&["bookmark", "create", "topic", "-r", &healthy]);
    let operation = h.jj.jj(&["op", "log", "--no-graph", "-n1", "-T", "id"]);
    h.jj.jj(&["bookmark", "set", "topic", "-r", &left, "--allow-backwards"]);
    h.jj.jj(&[
        "--at-op",
        &operation,
        "bookmark",
        "set",
        "topic",
        "-r",
        &right,
        "--allow-backwards",
    ]);
    h.jj.jj(&["op", "log", "-n1"]);
    refused(
        &h.submit("c1", &[&right], false)?,
        409,
        "review_jj_conflicted_bookmark",
    )?;
    h.jj.jj(&[
        "bookmark",
        "set",
        "topic",
        "-r",
        &right,
        "--allow-backwards",
    ]);
    h.jj.jj(&["rebase", "-r", &left, "-d", &right]);
    refused(
        &h.submit("c2", &[&left], false)?,
        409,
        "review_jj_conflicted_commit",
    )?;

    refused(
        &h.submit(
            "m1",
            &[&healthy.chars().take(12).collect::<String>()],
            false,
        )?,
        400,
        "review_jj_change_id_invalid",
    )?;
    refused(
        &h.submit("m2", &["all()"], false)?,
        400,
        "review_jj_change_id_invalid",
    )?;
    refused(
        &h.submit("m3", &[&healthy, &right], false)?,
        400,
        "review_jj_stack_required",
    )?;
    refused(
        &h.submit("m4", &[&healthy, &healthy], true)?,
        400,
        "review_jj_change_duplicate",
    )?;
    refused(
        &h.submit("m5", &["kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk"], false)?,
        409,
        "review_jj_change_missing",
    )?;
    // Out of ancestor order (top before bottom).
    let stacked = h.jj.commit("d.txt", "d\n", "Above right");
    refused(
        &h.submit("m6", &[&stacked, &right], true)?,
        409,
        "review_jj_stack_order",
    )?;
    // The internal member action is never deserializable from a client.
    refused(
        &h.post("w2", json!({"type":"jj_member","member":{}}), ALICE)?,
        400,
        "review_command_invalid",
    )?;
    // Nothing was reserved, journaled into state or registered.
    assert_eq!(h.document()?, None);
    assert_eq!(h.kiln_refs()?, 0);
    Ok(())
}

// r[verify kiln.review.jj_intake.stack]
// r[verify kiln.review.jj_intake.stack.scenario.create]
// r[verify kiln.review.jj_intake.stack.scenario.partial]
// r[verify kiln.review.jj_intake.stack.scenario.reorder]
// r[verify kiln.review.jj_intake.stack.scenario.removed]
#[test]
fn four_change_stack_maps_by_change_id_across_edit_reorder_and_removal() -> TestResult {
    let h = Harness::new()?;
    let a = h.jj.commit("a.txt", "a\n", "A");
    let b = h.jj.commit("b.txt", "b\n", "B");
    let c = h.jj.commit("c.txt", "c\n", "C");
    let d = h.jj.commit("d.txt", "d\n", "D");
    let ids = [a.as_str(), b.as_str(), c.as_str(), d.as_str()];
    let created = accepted(h.submit("s1", &ids, true)?)?;
    assert_eq!(outcomes(&created)?, ["created"; 4]);
    let stack = text(&created, "/jj_intake/stack")?;
    assert!(h.dependencies(&a)?.is_empty());
    assert_eq!(h.dependencies(&b)?, [h.change(&a)?]);
    assert_eq!(h.dependencies(&c)?, [h.change(&b)?]);
    assert_eq!(h.dependencies(&d)?, [h.change(&c)?]);

    // Replay with a new request id creates and appends nothing.
    let replay = accepted(h.submit("s2", &ids, true)?)?;
    assert_eq!(outcomes(&replay)?, ["unchanged"; 4]);
    assert!(
        field(&replay, "/jj_intake/members")?
            .as_array()
            .ok_or("members")?
            .iter()
            .all(|member| member.get("dependencies_updated") == Some(&json!(false)))
    );
    for id in ids {
        assert_eq!(h.revisions(id)?, 1);
    }

    // Edit exactly two changes (C, then D on top): exactly those two append.
    h.jj.rewrite(&c, "c.txt", "c2\n");
    h.jj.rewrite(&d, "d.txt", "d2\n");
    let edited = accepted(h.submit("s3", &ids, true)?)?;
    assert_eq!(
        outcomes(&edited)?,
        ["unchanged", "unchanged", "appended", "appended"]
    );
    assert_eq!(
        [
            h.revisions(&a)?,
            h.revisions(&b)?,
            h.revisions(&c)?,
            h.revisions(&d)?
        ],
        [1, 1, 2, 2]
    );
    // D's edge rebinds to C's new current revision.
    let detail = h.detail(&h.change(&d)?)?;
    let c_detail = h.detail(&h.change(&c)?)?;
    assert_eq!(
        field(&detail, "/change/dependencies/0/revision")?,
        field(&c_detail, "/change/revisions/1/id")?
    );

    // Reorder in jj (D below C) and resubmit in the new order.
    h.jj.jj(&["rebase", "-r", &d, "--insert-before", &c]);
    let reordered = [a.as_str(), b.as_str(), d.as_str(), c.as_str()];
    accepted(h.submit("s4", &reordered, true)?)?;
    assert_eq!(h.dependencies(&d)?, [h.change(&b)?]);
    assert_eq!(h.dependencies(&c)?, [h.change(&d)?]);

    // Drop C from the stack: flagged on its change, never abandoned.
    let shortened = accepted(h.submit("s5", &[&a, &b, &d], true)?)?;
    assert_eq!(text(&shortened, "/jj_intake/stack")?, stack);
    assert_eq!(text(&shortened, "/jj_intake/removed/0/jj_change_id")?, c);
    let removed = h.detail(&h.change(&c)?)?;
    assert_eq!(field(&removed, "/change/state")?, "open");
    assert_eq!(field(&removed, "/jj_stack/removed")?, true);
    let kept = h.detail(&h.change(&d)?)?;
    assert_eq!(field(&kept, "/jj_stack/removed")?, false);
    assert_eq!(field(&kept, "/jj_stack/position")?, 3);
    // Re-adding C clears the flag.
    accepted(h.submit("s6", &reordered, true)?)?;
    let restored = h.detail(&h.change(&c)?)?;
    assert_eq!(field(&restored, "/jj_stack/removed")?, false);
    Ok(())
}

// r[verify kiln.review.jj_intake.header]
// r[verify kiln.review.jj_intake.header.scenario.stack]
#[test]
fn change_id_header_branch_maps_to_the_same_changes_without_jj() -> TestResult {
    let h = Harness::new()?;
    let e = h.jj.commit("e.txt", "e\n", "E");
    let f = h.jj.commit("f.txt", "f\n", "F");
    h.jj.jj(&["bookmark", "create", "review/stack", "-r", &f]);
    let header = |id: &str, stack: bool| {
        h.post(
            id,
            json!({"type":"jj_contribute","source_ref":"refs/heads/review/stack","stack":stack}),
            ALICE,
        )
    };
    let created = accepted(header("h1", true)?)?;
    assert_eq!(outcomes(&created)?, ["created", "created"]);
    assert_eq!(text(&created, "/jj_intake/source")?, "git_change_id_header");
    assert_eq!(h.dependencies(&f)?, [h.change(&e)?]);
    let evolog = h.detail(&h.change(&e)?)?;
    assert_eq!(
        field(&evolog, "/jj_evolog/0/jj/source")?,
        "git_change_id_header"
    );
    // jj intake reaches the very same changes.
    assert_eq!(
        outcomes(&accepted(h.submit("h2", &[&e, &f], true)?)?)?,
        ["unchanged"; 2]
    );
    // Single mode takes the tip only.
    assert_eq!(outcomes(&accepted(header("h3", false)?)?)?, ["unchanged"]);

    // A commit without the header on an admitted branch is refused.
    let repository = git2::Repository::open(&h.jj.root)?;
    let tip = repository
        .revparse_single("refs/heads/review/stack")?
        .peel_to_commit()?;
    let signature = git2::Signature::now("Plain", "plain@example.org")?;
    let plain = repository.commit(
        None,
        &signature,
        &signature,
        "plain git",
        &tip.tree()?,
        &[&tip],
    )?;
    repository.reference("refs/heads/review/plain", plain, false, "fixture")?;
    refused(
        &h.post(
            "h4",
            json!({"type":"jj_contribute","source_ref":"refs/heads/review/plain","stack":true}),
            ALICE,
        )?,
        409,
        "review_jj_header_missing",
    )?;
    refused(
        &h.post(
            "h5",
            json!({"type":"jj_contribute","source_ref":"refs/heads/elsewhere","stack":true}),
            ALICE,
        )?,
        403,
        "review_source_ref_not_admitted",
    )?;
    Ok(())
}
