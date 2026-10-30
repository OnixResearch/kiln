//! Rebase-aware interdiff, commit ranges, hunk viewed markers and the
//! structural comparison mode through the authenticated review shell.
use std::error::Error;
use std::os::unix::fs::PermissionsExt;

use git2::{ObjectType, Oid, Repository, Signature, Time};
use kiln_core::change_review::CiFact;
use kiln_web_service::code_review::{Profile, RequestContext, Workspace};
use kiln_web_service::engine::{ApiResponse, Role};
use rusqlite::Connection;
use serde_json::{Value, json};
use tempfile::TempDir;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const ALICE: &str = "local:author";
const BOB: &str = "local:reviewer";
const MAIN: &str = "refs/heads/main";

fn lines(edits: &[(usize, &str)]) -> String {
    (1..=20)
        .map(|n| {
            edits
                .iter()
                .find(|(line, _)| *line == n)
                .map_or_else(|| format!("l{n}\n"), |(_, text)| format!("{text}\n"))
        })
        .collect()
}
const LIB_BASE: &str = "fn a() -> u32 { 1 }\nfn b() -> u32 { 2 }\n";
// Reformats `a` and changes one literal in `b`.
const LIB_NEW: &str = "fn a() -> u32 {\n    1\n}\nfn b() -> u32 { 3 }\n";

struct Fixture {
    temp: TempDir,
    repository: Repository,
    profile: Profile,
    base1: Oid,
    middle: Oid,
    rev1: Oid,
}
impl Fixture {
    fn new() -> Result<Self> {
        let temp = tempfile::tempdir()?;
        std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700))?;
        let path = temp.path().join("review.git");
        let repository = Repository::init_bare(&path)?;
        let base1 = commit(
            &repository,
            None,
            &[
                ("file.txt", &lines(&[])),
                ("other.txt", "target\n"),
                ("lib.rs", LIB_BASE),
            ],
        )?;
        // Revision 1 has two commits: line 2, then line 15 and lib.rs.
        let middle = commit(
            &repository,
            Some(base1),
            &[
                ("file.txt", &lines(&[(2, "author-2")])),
                ("other.txt", "target\n"),
                ("lib.rs", LIB_BASE),
            ],
        )?;
        let rev1 = commit(
            &repository,
            Some(middle),
            &[
                ("file.txt", &lines(&[(2, "author-2"), (15, "author-15")])),
                ("other.txt", "target\n"),
                ("lib.rs", LIB_NEW),
            ],
        )?;
        repository.reference(MAIN, base1, false, "fixture")?;
        let canonical = path.canonicalize()?;
        let repository_id =
            kiln_codec::identity_from_bytes(canonical.to_str().ok_or("path")?.as_bytes())?
                .as_str()
                .to_owned();
        let profile: Profile = serde_json::from_value(json!({
            "schema": "kiln.code-review-profile.v1",
            "database": temp.path().join("review.sqlite"),
            "project": "project",
            "provider": {"repository_id": repository_id, "repository_path": path, "target_refs": [MAIN],
                "capabilities": ["inspect", "compare", "register"]},
            "policy": {"id": "policy", "eligible_reviewers": [BOB], "submitters": [ALICE], "minimum_approvals": 1, "require_ci": false},
            "read_principals": [ALICE, BOB], "write_principals": [ALICE, BOB],
            "signature_name": "Review service", "signature_email": "review@example.invalid"
        }))?;
        Ok(Self {
            temp,
            repository,
            profile,
            base1,
            middle,
            rev1,
        })
    }
    fn workspace(&self) -> Result<Workspace> {
        Ok(Workspace::new(Some(self.profile.clone()))?)
    }
    fn move_main(&self, to: Oid) -> Result {
        self.repository.reference(MAIN, to, true, "target moved")?;
        Ok(())
    }
}
fn commit(repository: &Repository, parent: Option<Oid>, files: &[(&str, &str)]) -> Result<Oid> {
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
fn context(actor: &str) -> RequestContext<'_> {
    RequestContext {
        role: Role::Viewer,
        principal: actor,
        now_ms: 1000,
        ci: &[] as &[CiFact],
    }
}
fn get(workspace: &Workspace, path: &str, actor: &str) -> ApiResponse {
    workspace.response("GET", path, &[], context(actor))
}
fn ok(response: ApiResponse) -> Result<Value> {
    assert_eq!(
        response.status,
        200,
        "{}",
        String::from_utf8_lossy(&response.body)
    );
    Ok(serde_json::from_slice(&response.body)?)
}
fn status(response: &ApiResponse) -> (u16, Value) {
    (
        response.status,
        serde_json::from_slice(&response.body).unwrap_or(Value::Null),
    )
}
fn post(workspace: &Workspace, request: &Value, actor: &str) -> Result<ApiResponse> {
    Ok(workspace.response(
        "POST",
        "/api/code-review/commands",
        &serde_json::to_vec(request)?,
        context(actor),
    ))
}
fn detail(workspace: &Workspace, id: &str, actor: &str) -> Result<Value> {
    ok(get(
        workspace,
        &format!("/api/code-review/changes/{id}"),
        actor,
    ))
}
fn act(
    workspace: &Workspace,
    id: &str,
    request: &str,
    actor: &str,
    action: Value,
) -> Result<ApiResponse> {
    let change = detail(workspace, id, actor)?;
    post(
        workspace,
        &json!({"request_id":request,"change_id":id,"expected_version":at(&change, "/change/version"),"action":action}),
        actor,
    )
}
fn diff(workspace: &Workspace, id: &str, actor: &str, query: &str) -> ApiResponse {
    get(
        workspace,
        &format!("/api/code-review/changes/{id}/diff?{query}"),
        actor,
    )
}
fn changed(diff: &Value) -> Vec<String> {
    let mut rows = Vec::new();
    for file in diff["files"].as_array().into_iter().flatten() {
        let path = file["new_path"]
            .as_str()
            .or(file["old_path"].as_str())
            .unwrap_or_default();
        for hunk in file["hunks"].as_array().into_iter().flatten() {
            for line in hunk["lines"].as_array().into_iter().flatten() {
                if line["kind"] != "context" {
                    rows.push(format!(
                        "{path}:{}:{}",
                        line["kind"].as_str().unwrap_or_default(),
                        line["text"].as_str().unwrap_or_default().trim_end()
                    ));
                }
            }
        }
    }
    rows
}
/// The JSON value at `pointer`, or null when absent.
fn at(value: &Value, pointer: &str) -> Value {
    value.pointer(pointer).cloned().unwrap_or(Value::Null)
}
fn file_progress<'a>(diff: &'a Value, path: &str) -> Result<&'a Value> {
    diff.pointer("/review_hunks/files")
        .and_then(Value::as_array)
        .and_then(|files| files.iter().find(|f| f["path"] == path))
        .ok_or_else(|| format!("no progress for {path}: {diff}").into())
}

/// Revision 1 (two commits on base 1), then the target moves and revision 2
/// rebases it, keeping the line 2 hunk and editing line 15 and line 19 (one hunk).
fn uploaded(
    fixture: &Fixture,
    workspace: &Workspace,
) -> Result<(String, String, String, Oid, Oid)> {
    let created = ok(post(
        workspace,
        &json!({"request_id":"create","change_id":null,"expected_version":null,
            "action":{"type":"create","commit":fixture.rev1.to_string(),"target_ref":MAIN,"title":"Interdiff","description":"","wip":false}}),
        ALICE,
    )?)?;
    let id = created
        .pointer("/change/id")
        .and_then(Value::as_str)
        .ok_or("id")?
        .to_owned();
    let r1 = detail(workspace, &id, ALICE)?
        .pointer("/change/revisions/0/id")
        .and_then(Value::as_str)
        .ok_or("r1")?
        .to_owned();
    // Bob views every hunk of revision 1.
    for (index, file) in ["file.txt", "lib.rs"].iter().enumerate() {
        ok(act(
            workspace,
            &id,
            &format!("mark-{index}"),
            BOB,
            json!({"type":"mark_reviewed","file":file,"reviewed":true}),
        )?)?;
    }
    let base2 = commit(
        &fixture.repository,
        Some(fixture.base1),
        &[
            ("file.txt", &lines(&[(9, "target-9")])),
            ("other.txt", "target moved\n"),
            ("lib.rs", LIB_BASE),
        ],
    )?;
    fixture.move_main(base2)?;
    let rev2 = commit(
        &fixture.repository,
        Some(base2),
        &[
            (
                "file.txt",
                &lines(&[
                    (2, "author-2"),
                    (9, "target-9"),
                    (15, "author-15-edited"),
                    (19, "author-19"),
                ]),
            ),
            ("other.txt", "target moved\n"),
            ("lib.rs", LIB_NEW),
        ],
    )?;
    ok(act(
        workspace,
        &id,
        "upload",
        ALICE,
        json!({"type":"upload","commit":rev2.to_string()}),
    )?)?;
    let r2 = detail(workspace, &id, ALICE)?
        .pointer("/change/revisions/1/id")
        .and_then(Value::as_str)
        .ok_or("r2")?
        .to_owned();
    Ok((id, r1, r2, base2, rev2))
}

// r[verify kiln.review.hunks.scenario.carry_forward]
#[test]
fn upload_carries_forward_only_unchanged_hunks_and_reports_progress() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let (id, r1, r2, _, _) = uploaded(&fixture, &workspace)?;
    let bob = detail(&workspace, &id, BOB)?;
    // file.txt line 2 and lib.rs (identical hunks) carry forward; the edited line 15 hunk (merged
    // with the new line 19 edit) does not.
    let carried: Vec<_> = bob
        .pointer("/change/reviewed_hunks")
        .and_then(Value::as_array)
        .ok_or("markers")?
        .iter()
        .map(|m| {
            (
                m["file"].as_str().unwrap_or_default().to_owned(),
                m["revision"].clone(),
            )
        })
        .collect();
    assert_eq!(carried.len(), 2, "{carried:?}");
    assert!(carried.iter().all(|(_, revision)| revision == &json!(r2)));
    assert_eq!(
        carried
            .iter()
            .filter(|(file, _)| file == "file.txt")
            .count(),
        1
    );
    assert_eq!(at(&bob, "/change/last_viewed"), json!(r1));
    // Private: the author sees none of Bob's markers.
    assert_eq!(
        at(&detail(&workspace, &id, ALICE)?, "/change/reviewed_hunks"),
        json!([])
    );
    let view = ok(diff(
        &workspace,
        &id,
        BOB,
        &format!("revision={r2}&base=base"),
    ))?;
    assert_eq!(at(&view, "/review_hunks/exact_base"), json!(true));
    assert_eq!(at(&view, "/review_hunks/last_viewed/number"), json!(1));
    let file = file_progress(&view, "file.txt")?;
    assert_eq!(
        (
            file["total"].clone(),
            file["viewed"].clone(),
            file["new_since_viewed"].clone()
        ),
        (json!(2), json!(1), json!(1))
    );
    assert_eq!(at(file, "/hunks/0/viewed"), json!(true));
    assert_eq!(at(file, "/hunks/0/new_since_viewed"), json!(false));
    // Toggle one hunk off and on through its digest; a forged digest is refused.
    let digest = file
        .pointer("/hunks/1/digest")
        .and_then(Value::as_str)
        .ok_or("digest")?
        .to_owned();
    ok(act(
        &workspace,
        &id,
        "hunk-on",
        BOB,
        json!({"type":"mark_hunk","file":"file.txt","hunk_digest":digest,"reviewed":true}),
    )?)?;
    let view = ok(diff(&workspace, &id, BOB, &format!("revision={r2}")))?;
    assert_eq!(file_progress(&view, "file.txt")?["viewed"], json!(2));
    let (code, body) = status(&act(
        &workspace,
        &id,
        "forged",
        BOB,
        json!({"type":"mark_hunk","file":"file.txt","hunk_digest":"0".repeat(64),"reviewed":true}),
    )?);
    assert_eq!((code, at(&body, "/code")), (409, json!("invalid_hunk")));
    let (code, _) = status(&act(
        &workspace,
        &id,
        "malformed",
        BOB,
        json!({"type":"mark_hunk","file":"file.txt","hunk_digest":"not-hex","reviewed":true}),
    )?);
    assert_eq!(code, 400);
    // Unmark the file: every hunk of it is dropped, other files stay.
    ok(act(
        &workspace,
        &id,
        "unmark",
        BOB,
        json!({"type":"mark_reviewed","file":"file.txt","reviewed":false}),
    )?)?;
    let bob = detail(&workspace, &id, BOB)?;
    assert!(
        bob.pointer("/change/reviewed_hunks")
            .and_then(Value::as_array)
            .ok_or("markers")?
            .iter()
            .all(|m| m["file"] == "lib.rs")
    );
    Ok(())
}

// r[verify kiln.review.interdiff.scenario.rebased]
// r[verify kiln.review.interdiff.scenario.unavailable]
// r[verify kiln.review.interdiff.scenario.narrowed]
#[test]
fn interdiff_shows_only_the_authors_delta_and_falls_back_explicitly() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let (id, r1, r2, base2, rev2) = uploaded(&fixture, &workspace)?;
    let inter = ok(diff(
        &workspace,
        &id,
        BOB,
        &format!("revision={r2}&interdiff={r1}"),
    ))?;
    assert_eq!(at(&inter, "/interdiff/status"), json!("exact"));
    assert_eq!(at(&inter, "/exact_old_side"), json!(false));
    assert_eq!(
        changed(&inter),
        vec![
            "file.txt:deletion:author-15",
            "file.txt:addition:author-15-edited",
            "file.txt:deletion:l19",
            "file.txt:addition:author-19"
        ]
    );
    // The plain comparison against revision 1 is unchanged and includes target movement.
    let plain = changed(&ok(diff(
        &workspace,
        &id,
        BOB,
        &format!("revision={r2}&base={r1}"),
    ))?);
    assert!(plain.contains(&"other.txt:addition:target moved".to_string()));
    assert!(plain.contains(&"file.txt:addition:target-9".to_string()));
    // Narrowing without change-id headers is unavailable and falls back to the exact comparison.
    let narrowed = ok(diff(
        &workspace,
        &id,
        BOB,
        &format!("revision={r2}&interdiff={r1}&change=wnloytsvxwxwxtpqyuwxutmotlmslszv"),
    ))?;
    assert_eq!(at(&narrowed, "/interdiff/status"), json!("unavailable"));
    assert_eq!(at(&narrowed, "/interdiff/reason"), json!("no_change_ids"));
    assert_eq!(
        at(&narrowed, "/interdiff/fallback"),
        json!("exact_commit_comparison")
    );
    assert_eq!(
        at(&narrowed, "/old_commit"),
        json!(fixture.rev1.to_string())
    );
    assert_eq!(at(&narrowed, "/new_commit"), json!(rev2.to_string()));
    // A conflicting rebase is unavailable with the exact fallback.
    let base3 = commit(
        &fixture.repository,
        Some(base2),
        &[
            ("file.txt", &lines(&[(2, "target-2"), (9, "target-9")])),
            ("other.txt", "target moved\n"),
            ("lib.rs", LIB_BASE),
        ],
    )?;
    fixture.move_main(base3)?;
    let rev3 = commit(
        &fixture.repository,
        Some(base3),
        &[
            ("file.txt", &lines(&[(2, "author-2b"), (9, "target-9")])),
            ("other.txt", "target moved\n"),
            ("lib.rs", LIB_BASE),
        ],
    )?;
    ok(act(
        &workspace,
        &id,
        "upload-3",
        ALICE,
        json!({"type":"upload","commit":rev3.to_string()}),
    )?)?;
    let r3 = detail(&workspace, &id, ALICE)?
        .pointer("/change/revisions/2/id")
        .and_then(Value::as_str)
        .ok_or("r3")?
        .to_owned();
    let conflicted = ok(diff(
        &workspace,
        &id,
        BOB,
        &format!("revision={r3}&interdiff={r2}"),
    ))?;
    assert_eq!(
        at(&conflicted, "/interdiff/reason"),
        json!("conflicted_rebase")
    );
    assert_eq!(at(&conflicted, "/exact_old_side"), json!(true));
    assert_eq!(at(&conflicted, "/old_commit"), json!(rev2.to_string()));
    // Malformed selections are refused before any provider work.
    for (query, code) in [
        (
            format!("revision={r1}&interdiff={r2}"),
            "review_interdiff_order",
        ),
        (
            format!("revision={r2}&change=wnloytsvxwxwxtpqyuwxutmotlmslszv"),
            "review_comparison_invalid",
        ),
        (
            format!("revision={r2}&interdiff={r1}&change=NOTANID"),
            "review_change_id_invalid",
        ),
        (
            format!("revision={r2}&interdiff={r1}&base={r1}"),
            "review_comparison_invalid",
        ),
        (
            format!("revision={r2}&mode=semantic"),
            "review_mode_invalid",
        ),
    ] {
        let (status, body) = status(&diff(&workspace, &id, BOB, &query));
        assert_eq!((status, at(&body, "/code")), (400, json!(code)), "{query}");
    }
    Ok(())
}

// r[verify kiln.review.commit_range]
#[test]
fn commit_ranges_inside_a_revision_are_exact_and_bounded_to_its_commits() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let (id, r1, _, _, _) = uploaded(&fixture, &workspace)?;
    let view = ok(diff(&workspace, &id, BOB, &format!("revision={r1}")))?;
    let commits: Vec<_> = view
        .pointer("/revision_commits/commits")
        .and_then(Value::as_array)
        .ok_or("commits")?
        .iter()
        .map(|c| c["commit"].clone())
        .collect();
    assert_eq!(
        commits,
        vec![
            json!(fixture.middle.to_string()),
            json!(fixture.rev1.to_string())
        ]
    );
    let range = ok(diff(
        &workspace,
        &id,
        BOB,
        &format!("revision={r1}&commits={}..{}", fixture.middle, fixture.rev1),
    ))?;
    assert!(changed(&range).iter().all(|row| !row.contains("author-2")));
    assert!(changed(&range).contains(&"file.txt:addition:author-15".to_string()));
    assert_eq!(at(&range, "/commit_range/anchorable"), json!(true));
    let first = ok(diff(
        &workspace,
        &id,
        BOB,
        &format!(
            "revision={r1}&commits={}..{}",
            fixture.base1, fixture.middle
        ),
    ))?;
    assert_eq!(
        changed(&first),
        vec!["file.txt:deletion:l2", "file.txt:addition:author-2"]
    );
    assert_eq!(at(&first, "/commit_range/anchorable"), json!(false));
    for query in [
        format!("revision={r1}&commits={}..{}", fixture.rev1, fixture.middle),
        format!(
            "revision={r1}&commits={}..{}",
            fixture.middle, fixture.middle
        ),
        format!("revision={r1}&commits=main..{}", fixture.rev1),
    ] {
        assert_eq!(
            at(&status(&diff(&workspace, &id, BOB, &query)).1, "/code"),
            json!("review_commits_invalid"),
            "{query}"
        );
    }
    Ok(())
}

// r[verify kiln.review.structural]
// r[verify kiln.review.structural.scenario.fallback]
// r[verify kiln.review.structural.scenario.anchors]
#[test]
fn structural_mode_shows_the_semantic_delta_with_line_mode_anchors() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let (id, r1, r2, _, _) = uploaded(&fixture, &workspace)?;
    let line = ok(diff(&workspace, &id, BOB, &format!("revision={r1}")))?;
    let structural = ok(diff(
        &workspace,
        &id,
        BOB,
        &format!("revision={r1}&mode=structural"),
    ))?;
    // Same exact comparison: anchors bind identical commits in both modes.
    assert_eq!(
        (&at(&line, "/old_commit"), &at(&line, "/new_commit")),
        (
            &at(&structural, "/old_commit"),
            &at(&structural, "/new_commit")
        )
    );
    let files = structural
        .pointer("/structural/files")
        .and_then(Value::as_array)
        .ok_or("files")?;
    let lib = files
        .iter()
        .find(|f| f["path"] == "lib.rs")
        .ok_or("lib.rs")?;
    assert_eq!(lib["status"], json!("structural"));
    let rows: Vec<_> = lib["hunks"]
        .as_array()
        .ok_or("hunks")?
        .iter()
        .flat_map(|h| h["rows"].as_array().cloned().unwrap_or_default())
        .filter(|r| r["side"] != "both")
        .collect();
    // Only `2` → `3` is novel: the reformatted `a` is not shown.
    let [old, new] = rows.as_slice() else {
        return Err(format!("{rows:?}").into());
    };
    assert_eq!((&old["side"], &old["old_line"]), (&json!("old"), &json!(2)));
    assert_eq!((&new["side"], &new["new_line"]), (&json!("new"), &json!(4)));
    // The same lines in line mode carry the same coordinates.
    let lib_line = line
        .pointer("/files")
        .and_then(Value::as_array)
        .ok_or("files")?
        .iter()
        .find(|f| f["new_path"] == "lib.rs")
        .ok_or("lib")?;
    let line_rows: Vec<_> = lib_line
        .pointer("/hunks/0/lines")
        .and_then(Value::as_array)
        .ok_or("lines")?
        .clone();
    assert!(line_rows.iter().any(|l| l["kind"] == "deletion"
        && l["old_line"] == 2
        && l["text"].as_str().is_some_and(|t| t.contains("{ 2 }"))));
    assert!(line_rows.iter().any(|l| l["kind"] == "addition"
        && l["new_line"] == 4
        && l["text"].as_str().is_some_and(|t| t.contains("{ 3 }"))));
    // An anchor created from the structural row is admitted exactly like a line-mode anchor.
    let anchor = json!({"revision":r1,"base_commit":at(&structural, "/old_commit"),"file":"lib.rs","side":"revision","start_line":4,"end_line":4});
    ok(act(
        &workspace,
        &id,
        "draft",
        BOB,
        json!({"type":"save_draft","anchor":anchor,"thread":null,"body":"Why 3?","blocking":false}),
    )?)?;
    let drafts = at(&detail(&workspace, &id, BOB)?, "/drafts");
    assert_eq!(at(&drafts, "/0/anchor"), anchor);
    // Files without a grammar, and inexact old sides, fall back explicitly.
    let text = files
        .iter()
        .find(|f| f["path"] == "file.txt")
        .ok_or("file.txt")?;
    assert_eq!(
        (&text["status"], &text["reason"]),
        (&json!("fallback"), &json!("unsupported_language"))
    );
    let inter = ok(diff(
        &workspace,
        &id,
        BOB,
        &format!("revision={r2}&interdiff={r1}&mode=structural"),
    ))?;
    assert!(
        inter
            .pointer("/structural/files")
            .and_then(Value::as_array)
            .ok_or("files")?
            .iter()
            .all(|f| f["reason"] == "not_exact_comparison")
    );
    let filtered = ok(diff(
        &workspace,
        &id,
        BOB,
        &format!("revision={r1}&mode=structural&whitespace=ignore_all"),
    ))?;
    assert!(
        filtered
            .pointer("/structural/files")
            .and_then(Value::as_array)
            .ok_or("files")?
            .iter()
            .all(|f| f["reason"] == "whitespace_filtered")
    );
    Ok(())
}

// r[verify kiln.review.hunks.scenario.replay]
#[test]
fn legacy_file_markers_in_stored_state_replay_byte_identically() -> Result {
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let created = ok(post(
        &workspace,
        &json!({"request_id":"create","change_id":null,"expected_version":null,
            "action":{"type":"create","commit":fixture.rev1.to_string(),"target_ref":MAIN,"title":"Legacy","description":"","wip":false}}),
        ALICE,
    )?)?;
    let id = created
        .pointer("/change/id")
        .and_then(Value::as_str)
        .ok_or("id")?
        .to_owned();
    let r1 = detail(&workspace, &id, ALICE)?
        .pointer("/change/revisions/0/id")
        .and_then(Value::as_str)
        .ok_or("r1")?
        .to_owned();
    let connection = Connection::open(fixture.temp.path().join("review.sqlite"))?;
    let document: Vec<u8> =
        connection.query_row("SELECT document FROM review_state WHERE id=1", [], |row| {
            row.get(0)
        })?;
    let text = String::from_utf8(document)?;
    assert!(!text.contains("reviewed_hunks") && !text.contains("last_viewed"));
    let legacy_marker = format!(
        "\"reviewed\":[{{\"actor\":\"{BOB}\",\"revision\":\"{r1}\",\"file\":\"file.txt\"}}]"
    );
    let legacy = text.replacen("\"reviewed\":[]", &legacy_marker, 1);
    connection.execute(
        "UPDATE review_state SET document=?1 WHERE id=1",
        [legacy.as_bytes()],
    )?;
    // A committed command reloads and re-serializes the document.
    ok(act(
        &workspace,
        &id,
        "attention",
        ALICE,
        json!({"type":"set_attention","principal":ALICE,"attention":true}),
    )?)?;
    let after: Vec<u8> =
        connection.query_row("SELECT document FROM review_state WHERE id=1", [], |row| {
            row.get(0)
        })?;
    let after = String::from_utf8(after)?;
    assert!(after.contains(&legacy_marker));
    assert!(!after.contains("reviewed_hunks"));
    // The legacy file marker means "viewed" for every hunk of that file on its own revision.
    let view = ok(diff(&workspace, &id, BOB, &format!("revision={r1}")))?;
    let file = file_progress(&view, "file.txt")?;
    assert_eq!(file["viewed"], file["total"]);
    Ok(())
}

/// A commit carrying a Jujutsu `change-id` header, as jj writes it.
fn change_commit(
    repository: &Repository,
    parent: Oid,
    files: &[(&str, &str)],
    change_id: &str,
) -> Result<Oid> {
    let mut builder = repository.treebuilder(None)?;
    for (path, content) in files {
        builder.insert(*path, repository.blob(content.as_bytes())?, 0o100644)?;
    }
    let tree = builder.write()?;
    let text = format!(
        "tree {tree}\nparent {parent}\nauthor Fixture <f@example.invalid> 1700000000 +0000\ncommitter Fixture <f@example.invalid> 1700000000 +0000\nchange-id {change_id}\n\n{change_id}\n"
    );
    Ok(repository
        .odb()?
        .write(ObjectType::Commit, text.as_bytes())?)
}

// r[verify kiln.review.interdiff.scenario.narrowed]
#[test]
fn narrowed_interdiff_shows_only_that_changes_delta() -> Result {
    const X: &str = "wnloytsvxwxwxtpqyuwxutmotlmslszv";
    const Y: &str = "kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk";
    let fixture = Fixture::new()?;
    let workspace = fixture.workspace()?;
    let repo = &fixture.repository;
    let base = fixture.base1;
    let x1 = change_commit(
        repo,
        base,
        &[
            ("file.txt", &lines(&[])),
            ("other.txt", "target\n"),
            ("lib.rs", LIB_BASE),
            ("x.txt", "x\n"),
        ],
        X,
    )?;
    let y1 = change_commit(
        repo,
        x1,
        &[
            ("file.txt", &lines(&[])),
            ("other.txt", "target\n"),
            ("lib.rs", LIB_BASE),
            ("x.txt", "x\n"),
            ("y.txt", "y\n"),
        ],
        Y,
    )?;
    let created = ok(post(
        &workspace,
        &json!({"request_id":"create","change_id":null,"expected_version":null,
            "action":{"type":"create","commit":y1.to_string(),"target_ref":MAIN,"title":"Stack","description":"","wip":false}}),
        ALICE,
    )?)?;
    let id = created
        .pointer("/change/id")
        .and_then(Value::as_str)
        .ok_or("id")?
        .to_owned();
    // The target moves; the stack is rebased with X unchanged and Y edited.
    let base2 = commit(
        repo,
        Some(base),
        &[
            ("file.txt", &lines(&[])),
            ("other.txt", "moved\n"),
            ("lib.rs", LIB_BASE),
        ],
    )?;
    fixture.move_main(base2)?;
    let x2 = change_commit(
        repo,
        base2,
        &[
            ("file.txt", &lines(&[])),
            ("other.txt", "moved\n"),
            ("lib.rs", LIB_BASE),
            ("x.txt", "x\n"),
        ],
        X,
    )?;
    let y2 = change_commit(
        repo,
        x2,
        &[
            ("file.txt", &lines(&[])),
            ("other.txt", "moved\n"),
            ("lib.rs", LIB_BASE),
            ("x.txt", "x\n"),
            ("y.txt", "y edited\n"),
        ],
        Y,
    )?;
    ok(act(
        &workspace,
        &id,
        "upload",
        ALICE,
        json!({"type":"upload","commit":y2.to_string()}),
    )?)?;
    let revisions = at(&detail(&workspace, &id, ALICE)?, "/change/revisions");
    let (r1, r2) = (
        revisions
            .pointer("/0/id")
            .and_then(Value::as_str)
            .ok_or("r1")?,
        revisions
            .pointer("/1/id")
            .and_then(Value::as_str)
            .ok_or("r2")?,
    );
    let narrowed = ok(diff(
        &workspace,
        &id,
        BOB,
        &format!("revision={r2}&interdiff={r1}&change={Y}"),
    ))?;
    assert_eq!(at(&narrowed, "/interdiff/status"), json!("exact"));
    assert_eq!(at(&narrowed, "/interdiff/change"), json!(Y));
    assert_eq!(
        (&at(&narrowed, "/old_commit"), &at(&narrowed, "/new_commit")),
        (&json!(y1.to_string()), &json!(y2.to_string()))
    );
    assert_eq!(
        changed(&narrowed),
        vec!["y.txt:deletion:y", "y.txt:addition:y edited"]
    );
    let unchanged = ok(diff(
        &workspace,
        &id,
        BOB,
        &format!("revision={r2}&interdiff={r1}&change={X}"),
    ))?;
    assert_eq!(at(&unchanged, "/interdiff/status"), json!("exact"));
    assert!(changed(&unchanged).is_empty());
    // The listed commits carry their change IDs for the narrowing control.
    let ids: Vec<_> = narrowed
        .pointer("/revision_commits/commits")
        .and_then(Value::as_array)
        .ok_or("commits")?
        .iter()
        .map(|c| c["change_id"].clone())
        .collect();
    assert_eq!(ids, vec![json!(X), json!(Y)]);
    // A change ID missing from both revisions is unavailable with the exact fallback.
    let missing = ok(diff(
        &workspace,
        &id,
        BOB,
        &format!("revision={r2}&interdiff={r1}&change=mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm"),
    ))?;
    assert_eq!(
        (
            &at(&missing, "/interdiff/reason"),
            &at(&missing, "/interdiff/fallback")
        ),
        (&json!("change_missing"), &json!("exact_commit_comparison"))
    );
    Ok(())
}
