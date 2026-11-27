use std::path::Path;

use git2::{Oid, Repository, Signature, Time};
use kiln_web_service::engine::{ApiResponse, Role};
use kiln_web_service::sources::{Browser, Profile, SourcePolicy};
use serde_json::{Value, json};

fn signature() -> Signature<'static> {
    Signature::new(
        "Source fixture",
        "source@example.invalid",
        &Time::new(1_700_000_000, 0),
    )
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
}

fn commit(
    repository: &Repository,
    files: &[(&str, &str)],
    message: &str,
    parent: Option<Oid>,
) -> Oid {
    let mut builder = repository
        .treebuilder(None)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    for (name, body) in files {
        let blob = repository
            .blob(body.as_bytes())
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        let path = Path::new(name);
        let mut parts = path.components();
        let first = parts.next();
        if parts.next().is_none() {
            let name = first
                .and_then(|part| part.as_os_str().to_str())
                .unwrap_or_else(|| unreachable!("fixture name"));
            builder
                .insert(name, blob, 0o100644)
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        } else {
            let mut directory = repository
                .treebuilder(None)
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
            let rest: Vec<_> = name.splitn(2, '/').collect();
            let leaf = rest.get(1).copied().unwrap_or("");
            directory
                .insert(leaf, blob, 0o100644)
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
            let subtree = directory
                .write()
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
            builder
                .insert(rest.first().copied().unwrap_or(""), subtree, 0o040000)
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        }
    }
    let tree_id = builder
        .write()
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let tree = repository
        .find_tree(tree_id)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let parents: Vec<_> = parent
        .into_iter()
        .filter_map(|oid| repository.find_commit(oid).ok())
        .collect();
    let borrowed: Vec<_> = parents.iter().collect();
    repository
        .commit(None, &signature(), &signature(), message, &tree, &borrowed)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
}

fn policy(id: &str, kind: &str, path: &Path) -> SourcePolicy {
    SourcePolicy {
        id: id.into(),
        label: format!("Source {id}"),
        kind: kind.into(),
        path: path.to_owned(),
        channel: None,
        executable: None,
    }
}

fn admitted(value: Value) -> Result<Profile, String> {
    let profile: Profile = serde_json::from_value(value).map_err(|error| error.to_string())?;
    profile.admit().map(|()| profile)
}

fn browser(sources: Vec<SourcePolicy>) -> Browser {
    Browser::new(Some(Profile {
        schema: "kiln.web-sources.v1".into(),
        sources,
    }))
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
}

fn value(response: &ApiResponse) -> Value {
    serde_json::from_slice(&response.body)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
}

fn get(browser: &Browser, target: &str) -> Value {
    value(&browser.response("GET", target, &[], Role::Viewer))
}

fn rejects(browser: &Browser, target: &str, status: u16, code: &str) {
    let response = browser.response("GET", target, &[], Role::Viewer);
    assert_eq!(response.status, status, "{target}");
    assert_eq!(
        value(&response).get("code").and_then(Value::as_str),
        Some(code),
        "{target}"
    );
}

fn git_fixture(root: &Path) -> (Repository, Oid, Oid) {
    let repository = Repository::init(root.join("work"))
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let first = commit(
        &repository,
        &[
            ("README.md", "# fixture\n"),
            ("src/main.rs", "fn main() {}\n"),
        ],
        "Initial fixture commit",
        None,
    );
    let second = commit(
        &repository,
        &[
            ("README.md", "# fixture v2\n"),
            ("src/main.rs", "fn main() { println!(); }\n"),
        ],
        "Second fixture commit",
        Some(first),
    );
    repository
        .reference("refs/heads/main", second, true, "fixture")
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    repository
        .set_head("refs/heads/main")
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    (repository, first, second)
}

// r[verify kiln.dashboard.source.read.scenario.git]
#[test]
fn git_source_log_tree_and_file_preserve_exact_revision_content() {
    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let (_repository, first, second) = git_fixture(root.path());
    let browser = browser(vec![policy("kiln", "git", &root.path().join("work"))]);

    let inventory = get(&browser, "/api/sources");
    assert_eq!(
        inventory.get("configured").and_then(Value::as_bool),
        Some(true)
    );
    let sources = inventory
        .get("sources")
        .and_then(Value::as_array)
        .unwrap_or_else(|| unreachable!("fixture member absent"));
    assert_eq!(sources.len(), 1);
    let source = sources
        .first()
        .unwrap_or_else(|| unreachable!("fixture absent"));
    assert_eq!(
        source.get("status").and_then(Value::as_str),
        Some("available")
    );
    assert_eq!(
        source.get("revision_scope").and_then(Value::as_str),
        Some("revision")
    );
    assert_eq!(
        source.get("head").and_then(Value::as_str),
        Some(second.to_string().as_str())
    );

    let log = get(&browser, "/api/sources/kiln/log?limit=1");
    let entries = log
        .get("entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| unreachable!("fixture member absent"));
    assert_eq!(entries.len(), 1);
    let entry = entries
        .first()
        .unwrap_or_else(|| unreachable!("fixture absent"));
    assert_eq!(
        entry.get("id").and_then(Value::as_str),
        Some(second.to_string().as_str())
    );
    assert_eq!(
        entry.get("summary").and_then(Value::as_str),
        Some("Second fixture commit")
    );
    assert_eq!(
        entry.get("recorded_at_ms").and_then(Value::as_i64),
        Some(1_700_000_000_000)
    );
    assert!(log.get("next_cursor").and_then(Value::as_str).is_some());

    let older = get(&browser, "/api/sources/kiln/log?limit=1&cursor=1");
    assert_eq!(
        older
            .get("entries")
            .and_then(Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("id"))
            .and_then(Value::as_str),
        Some(first.to_string().as_str()),
        "cursor is a bounded offset, not a revision selector"
    );

    let tree = get(
        &browser,
        &format!("/api/sources/kiln/tree?revision={first}"),
    );
    let names: Vec<_> = tree
        .get("entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| unreachable!("fixture member absent"))
        .iter()
        .filter_map(|entry| entry.get("name").and_then(Value::as_str))
        .collect();
    assert_eq!(names, ["README.md", "src"]);

    let nested = get(
        &browser,
        &format!("/api/sources/kiln/tree?revision={first}&path=src"),
    );
    assert_eq!(
        nested
            .get("entries")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );

    let head_file = get(&browser, "/api/sources/kiln/file?path=src/main.rs");
    assert_eq!(
        head_file.get("revision").and_then(Value::as_str),
        Some(second.to_string().as_str())
    );
    assert_eq!(
        head_file.get("text").and_then(Value::as_str),
        Some("fn main() { println!(); }\n")
    );
    assert_eq!(
        head_file.get("binary").and_then(Value::as_bool),
        Some(false)
    );

    let exact = get(
        &browser,
        &format!("/api/sources/kiln/file?revision={first}&path=src/main.rs"),
    );
    assert_eq!(
        exact.get("text").and_then(Value::as_str),
        Some("fn main() {}\n"),
        "an exact revision must not be substituted by the head revision"
    );

    rejects(
        &browser,
        "/api/sources/kiln/file?path=missing.rs",
        404,
        "source_path_missing",
    );
    rejects(
        &browser,
        "/api/sources/kiln/file?path=..%2Fetc%2Fpasswd",
        400,
        "source_path_invalid",
    );
    rejects(
        &browser,
        "/api/sources/kiln/tree?revision=HEAD",
        400,
        "source_revision_invalid",
    );
    rejects(&browser, "/api/sources/unknown/log", 404, "source_missing");
    rejects(
        &browser,
        "/api/sources/kiln/file?path=a&view=raw",
        400,
        "query_invalid",
    );
}

// r[verify kiln.dashboard.source.read]
#[test]
fn jj_source_reads_a_colocated_or_backend_git_store_without_mutation() {
    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let colocated_root = root.path().join("colocated");
    let (_repository, _first, head) = git_fixture(&colocated_root);
    let colocated = colocated_root.join("work");
    std::fs::create_dir_all(colocated.join(".jj").join("repo").join("store"))
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));

    let backend = root.path().join("backend");
    std::fs::create_dir_all(backend.join(".jj").join("repo").join("store"))
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let store = Repository::init_bare(backend.join(".jj").join("repo").join("store").join("git"))
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let tree_id = store
        .treebuilder(None)
        .and_then(|mut builder| {
            let blob = store.blob(b"backend\n")?;
            builder.insert("file.txt", blob, 0o100644)?;
            builder.write()
        })
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let empty = store
        .find_tree(tree_id)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let backend_head = store
        .commit(
            None,
            &signature(),
            &signature(),
            "Backend-only jj store",
            &empty,
            &[],
        )
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    store
        .reference("refs/heads/main", backend_head, true, "fixture")
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    store
        .set_head("refs/heads/main")
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));

    let browser = browser(vec![
        policy("colocated", "jj", &colocated),
        policy("backend", "jj", &backend),
    ]);
    let inventory = get(&browser, "/api/sources");
    let sources = inventory
        .get("sources")
        .and_then(Value::as_array)
        .unwrap_or_else(|| unreachable!("fixture member absent"));
    let head_of = |id: &str| {
        sources
            .iter()
            .find(|row| row.get("id").and_then(Value::as_str) == Some(id))
            .and_then(|row| row.get("head"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    };
    assert_eq!(head_of("colocated"), Some(head.to_string()));
    assert_eq!(
        head_of("backend"),
        Some(backend_head.to_string()),
        "{}",
        Value::Array(sources.clone())
    );

    let log = get(&browser, "/api/sources/backend/log");
    assert_eq!(
        log.get("entries")
            .and_then(Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("summary"))
            .and_then(Value::as_str),
        Some("Backend-only jj store")
    );
    let file = get(&browser, "/api/sources/backend/file?path=file.txt");
    assert_eq!(file.get("text").and_then(Value::as_str), Some("backend\n"));
    assert_eq!(file.get("kind").and_then(Value::as_str), Some("jj"));
    assert_eq!(
        get(&browser, "/api/sources/colocated/file?path=README.md")
            .get("text")
            .and_then(Value::as_str),
        Some("# fixture v2\n")
    );
}

// r[verify kiln.dashboard.source.authority.scenario.policy]
#[test]
fn source_policy_rejects_relative_paths_kind_drift_and_duplicates() {
    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let base = json!({
        "schema": "kiln.web-sources.v1",
        "sources": [{
            "id": "kiln",
            "label": "Kiln",
            "kind": "git",
            "path": root.path().join("kiln"),
        }],
    });
    admitted(base.clone()).unwrap_or_else(|error| unreachable!("fixture failed: {error}"));

    for (field, patch) in [
        ("path", json!("relative/path")),
        ("kind", json!("mercurial")),
        ("id", json!("../escape")),
        ("executable", json!("/bin/true")),
        ("channel", json!("main")),
        ("label", json!("")),
    ] {
        let mut candidate = base.clone();
        if let Some(object) = candidate
            .get_mut("sources")
            .and_then(Value::as_array_mut)
            .and_then(|rows| rows.first_mut())
            .and_then(Value::as_object_mut)
        {
            object.insert(field.to_owned(), patch);
        }
        assert!(admitted(candidate).is_err(), "{field} must be rejected");
    }

    let duplicate = json!({
        "schema": "kiln.web-sources.v1",
        "sources": [
            {"id": "same", "label": "A", "kind": "git", "path": root.path().join("a")},
            {"id": "same", "label": "B", "kind": "git", "path": root.path().join("b")},
        ],
    });
    assert!(admitted(duplicate).is_err());

    let relative_executable = json!({
        "schema": "kiln.web-sources.v1",
        "sources": [{
            "id": "pijul",
            "label": "Pijul",
            "kind": "pijul",
            "path": root.path().join("pijul"),
            "channel": "main",
            "executable": "pijul",
        }],
    });
    assert!(admitted(relative_executable).is_err());
    let missing_channel = json!({
        "schema": "kiln.web-sources.v1",
        "sources": [{
            "id": "pijul",
            "label": "Pijul",
            "kind": "pijul",
            "path": root.path().join("pijul"),
        }],
    });
    assert!(admitted(missing_channel).is_err());
}

// r[verify kiln.dashboard.source.read.scenario.pijul]
#[test]
fn pijul_source_reports_explicit_unavailability_instead_of_empty_success() {
    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let repository = root.path().join("pijul");
    std::fs::create_dir_all(repository.join(".pijul").join("changes"))
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    std::fs::write(repository.join(".pijul").join("pristine").join("db"), b"")
        .or_else(|_| {
            std::fs::create_dir_all(repository.join(".pijul").join("pristine"))?;
            std::fs::write(repository.join(".pijul").join("pristine").join("db"), b"")
        })
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let mut source = policy("pijul", "pijul", &repository);
    source.channel = Some("main".into());
    source.executable = Some(root.path().join("missing-pijul"));
    let browser = browser(vec![source]);

    let inventory = get(&browser, "/api/sources");
    let row = inventory
        .get("sources")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .unwrap_or_else(|| unreachable!("fixture member absent"))
        .clone();
    assert_eq!(
        row.get("status").and_then(Value::as_str),
        Some("unavailable")
    );
    assert_eq!(
        row.get("revision_scope").and_then(Value::as_str),
        Some("current_channel_state")
    );
    assert!(row.get("reason").and_then(Value::as_str).is_some());
    rejects(
        &browser,
        "/api/sources/pijul/log",
        503,
        "source_unavailable",
    );
    rejects(
        &browser,
        "/api/sources/pijul/tree",
        503,
        "source_unavailable",
    );
    rejects(
        &browser,
        "/api/sources/pijul/file?path=README.md",
        503,
        "source_unavailable",
    );
}

// r[verify kiln.dashboard.source.read.scenario.diff]
#[test]
fn source_diff_reports_exact_parent_changes_and_rejects_pijul_history() {
    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let (_repository, first, second) = git_fixture(root.path());
    let source = policy("kiln", "git", &root.path().join("work"));
    let git_browser = browser(vec![source]);

    let diff = get(
        &git_browser,
        &format!("/api/sources/kiln/diff?revision={second}"),
    );
    assert_eq!(
        diff.get("parent").and_then(Value::as_str),
        Some(first.to_string().as_str())
    );
    assert_eq!(diff.get("files_changed").and_then(Value::as_u64), Some(2));
    assert_eq!(diff.get("truncated").and_then(Value::as_bool), Some(false));
    let text = diff.get("diff").and_then(Value::as_str).unwrap_or_default();
    assert!(text.contains("+++ b/README.md"), "{text}");
    assert!(text.contains("+# fixture v2"), "{text}");
    assert!(text.contains("-# fixture"), "{text}");

    let head_diff = get(&git_browser, "/api/sources/kiln/diff");
    assert_eq!(
        head_diff.get("revision").and_then(Value::as_str),
        Some(second.to_string().as_str()),
        "an empty revision selects the current head commit"
    );

    let root_commit = get(
        &git_browser,
        &format!("/api/sources/kiln/diff?revision={first}"),
    );
    assert_eq!(root_commit.get("parent"), Some(&Value::Null));
    assert_eq!(
        root_commit.get("files_changed").and_then(Value::as_u64),
        Some(2)
    );

    rejects(
        &git_browser,
        "/api/sources/kiln/diff?revision=HEAD",
        400,
        "source_revision_invalid",
    );

    let mut pijul_source = policy("kiln", "pijul", &root.path().join("work"));
    pijul_source.channel = Some("main".into());
    pijul_source.executable = Some(root.path().join("missing-pijul"));
    let pijul_browser = browser(vec![pijul_source]);
    rejects(
        &pijul_browser,
        "/api/sources/kiln/diff",
        400,
        "source_diff_unsupported",
    );
}

// r[verify kiln.dashboard.source.read.scenario.diff]
#[test]
fn oversized_revision_changes_report_truncation_instead_of_failure() {
    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let repository = Repository::init(root.path().join("work"))
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let first = commit(&repository, &[("bulk.txt", "")], "Empty bulk", None);
    let bulk: String = (0..40_000).map(|index| format!("line {index}\n")).collect();
    let second = commit(
        &repository,
        &[("bulk.txt", &bulk)],
        "Bulk lines",
        Some(first),
    );
    repository
        .reference("refs/heads/main", second, true, "fixture")
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    repository
        .set_head("refs/heads/main")
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));

    let browser = browser(vec![policy("bulk", "git", &root.path().join("work"))]);
    let diff = get(&browser, "/api/sources/bulk/diff");
    assert_eq!(diff.get("truncated").and_then(Value::as_bool), Some(true));
    let text = diff.get("diff").and_then(Value::as_str).unwrap_or_default();
    assert!(text.len() <= 262_144, "{}", text.len());
    assert!(!text.is_empty());
}

// r[verify kiln.dashboard.source.read]
#[test]
fn commit_lookup_reports_only_sources_that_contain_the_object() {
    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let (_repository, first, second) = git_fixture(root.path());
    let other = root.path().join("other");
    let other_repository = Repository::init(other.join("work"))
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let foreign = commit(&other_repository, &[("x", "x\n")], "Foreign", None);
    other_repository
        .reference("refs/heads/main", foreign, true, "fixture")
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    other_repository
        .set_head("refs/heads/main")
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let browser = browser(vec![
        policy("kiln", "git", &root.path().join("work")),
        policy("other", "git", &other.join("work")),
    ]);

    let found = get(&browser, &format!("/api/sources/lookup?revision={second}"));
    let matches = found
        .get("matches")
        .and_then(Value::as_array)
        .unwrap_or_else(|| unreachable!("fixture member absent"));
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches
            .first()
            .and_then(|row| row.get("id"))
            .and_then(Value::as_str),
        Some("kiln")
    );

    let foreign_match = get(&browser, &format!("/api/sources/lookup?revision={foreign}"));
    assert_eq!(
        foreign_match
            .get("matches")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );

    let absent = get(
        &browser,
        "/api/sources/lookup?revision=0123456789012345678901234567890123456789",
    );
    assert_eq!(
        absent
            .get("matches")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    rejects(
        &browser,
        "/api/sources/lookup?revision=HEAD",
        400,
        "source_revision_invalid",
    );
    rejects(
        &browser,
        "/api/sources/lookup?revision=HEAD&path=x",
        400,
        "query_invalid",
    );
    assert_eq!(first, first);
}

#[test]
fn source_routes_reject_writes_workers_and_unconfigured_policies() {
    let unconfigured = Browser::new(None).unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        value(&unconfigured.response("GET", "/api/sources", &[], Role::Viewer))
            .get("configured")
            .and_then(Value::as_bool),
        Some(false)
    );
    rejects(
        &unconfigured,
        "/api/sources/kiln/log",
        404,
        "source_missing",
    );

    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let (_repository, _first, _second) = git_fixture(root.path());
    let browser = browser(vec![policy("kiln", "git", &root.path().join("work"))]);
    let worker = browser.response("GET", "/api/sources", &[], Role::Worker);
    assert_eq!(worker.status, 403);
    let write = browser.response("POST", "/api/sources", b"{}", Role::Operator);
    assert_eq!(write.status, 405);
    let body = browser.response("GET", "/api/sources", b"{}", Role::Viewer);
    assert_eq!(body.status, 400);
    let bounds = browser.response("GET", "/api/sources/kiln/log?limit=0", &[], Role::Viewer);
    assert_eq!(bounds.status, 400);
    let unknown = browser.response("GET", "/api/sources/kiln/blob", &[], Role::Viewer);
    assert_eq!(unknown.status, 404);
}

fn without_highlight(value: &Value) -> Value {
    let mut value = value.clone();
    if let Some(object) = value.as_object_mut() {
        object.remove("highlight");
    }
    value
}

/// The first token of the line at `index`, as `[start, end, class]`.
fn first_token(value: &Value, index: usize) -> Option<&Value> {
    value.pointer(&format!("/highlight/lines/{index}/0"))
}

// r[verify kiln.highlight.api]
#[test]
fn source_file_and_diff_highlighting_is_opt_in_and_additive() {
    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let (_repository, _first, second) = git_fixture(root.path());
    let browser = browser(vec![policy("kiln", "git", &root.path().join("work"))]);

    for route in [
        "/api/sources/kiln/file?path=src/main.rs".to_owned(),
        format!("/api/sources/kiln/diff?revision={second}"),
    ] {
        let plain = browser.response("GET", &route, &[], Role::Viewer);
        assert_eq!(plain.status, 200, "{route}");
        let plain_value = value(&plain);
        assert_eq!(plain_value.get("highlight"), None, "{route}");
        let highlighted = get(&browser, &format!("{route}&highlight=1"));
        let stripped = without_highlight(&highlighted);
        assert_eq!(stripped, plain_value, "{route}: other fields unchanged");
        assert_eq!(
            serde_json::to_vec(&stripped).unwrap_or_default(),
            plain.body,
            "{route}: the plain response is the same object, byte for byte"
        );
        assert_eq!(
            highlighted.pointer("/highlight/schema"),
            Some(&json!("kiln.highlight.v1"))
        );
        assert_eq!(
            highlighted.pointer("/highlight/status"),
            Some(&json!("highlighted")),
            "{route}"
        );
    }

    let file = get(
        &browser,
        "/api/sources/kiln/file?path=src/main.rs&highlight=1",
    );
    assert_eq!(file.pointer("/highlight/language"), Some(&json!("rust")));
    assert_eq!(first_token(&file, 0), Some(&json!([0, 2, "keyword"])));
    assert_eq!(
        file.pointer("/highlight/lines")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1),
        "a trailing newline starts no extra line"
    );

    let diff = get(
        &browser,
        &format!("/api/sources/kiln/diff?revision={second}&highlight=1"),
    );
    let text = diff.get("diff").and_then(Value::as_str).unwrap_or_default();
    let rows: Vec<&str> = text
        .strip_suffix('\n')
        .unwrap_or(text)
        .split('\n')
        .collect();
    assert_eq!(diff.pointer("/highlight/language"), Some(&Value::Null));
    assert_eq!(
        diff.pointer("/highlight/lines")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(rows.len())
    );
    for (index, row) in rows.iter().enumerate() {
        if row.starts_with("+fn main") || row.starts_with("-fn main") {
            assert_eq!(
                first_token(&diff, index),
                Some(&json!([1, 3, "keyword"])),
                "{row}"
            );
        }
        if row.starts_with("diff --git") || row.starts_with("@@") || row.starts_with("+++") {
            assert_eq!(
                diff.pointer(&format!("/highlight/lines/{index}")),
                Some(&json!([])),
                "{row}"
            );
        }
    }

    for bad in ["2", "true", "", "01", "1%00"] {
        rejects(
            &browser,
            &format!("/api/sources/kiln/file?path=src/main.rs&highlight={bad}"),
            400,
            "query_invalid",
        );
        rejects(
            &browser,
            &format!("/api/sources/kiln/diff?highlight={bad}"),
            400,
            "query_invalid",
        );
    }
    rejects(
        &browser,
        "/api/sources/kiln/tree?highlight=1",
        400,
        "query_invalid",
    );
    rejects(
        &browser,
        "/api/sources/kiln/log?highlight=1",
        400,
        "query_invalid",
    );
}

// r[verify kiln.highlight.api]
#[test]
fn unsupported_and_binary_source_files_report_explicit_skips() {
    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let repository = Repository::init(root.path().join("work"))
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let head = commit(
        &repository,
        &[("notes.txt", "plain words\n"), ("logo.rs", "fn\0binary")],
        "Skips",
        None,
    );
    repository
        .reference("refs/heads/main", head, true, "fixture")
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    repository
        .set_head("refs/heads/main")
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let browser = browser(vec![policy("skips", "git", &root.path().join("work"))]);

    let notes = get(
        &browser,
        "/api/sources/skips/file?path=notes.txt&highlight=1",
    );
    assert_eq!(notes.get("text"), Some(&json!("plain words\n")));
    assert_eq!(
        notes.get("highlight"),
        Some(
            &json!({"schema":"kiln.highlight.v1","language":null,"status":"skipped","reason":"unsupported_language","lines":[]})
        )
    );
    let binary = get(&browser, "/api/sources/skips/file?path=logo.rs&highlight=1");
    assert_eq!(binary.get("binary"), Some(&json!(true)));
    assert_eq!(binary.get("text"), None);
    assert_eq!(
        binary.get("highlight"),
        Some(
            &json!({"schema":"kiln.highlight.v1","language":"rust","status":"skipped","reason":"binary","lines":[]})
        )
    );
}

// r[verify kiln.source.projection.disclosure.restricted]
// r[verify kiln.source.projection.disclosure.handles]
// r[verify kiln.source.projection.disclosure.unavailable]
#[test]
fn restricted_projection_sources_enforce_basalt_authority_handles_and_unavailable_oracles() {
    use kiln_adapter_basalt::test_support::{
        COORDINATOR_SEED, EXPIRED_AT_S, EXPIRES_S, GENERATION, SourceFixture, VERIFY_AT_S,
    };
    use kiln_core::implementation::authority::{AttemptCurrentness, AuthorityProfile};
    use kiln_core::projection::authority::{
        PathScope, SnapshotObservation, SourceAuthorityRequest, SourceCurrentness,
        SourceViewOperation,
    };
    use kiln_core::projection::disclosure::{HandleKey, OpaqueHandle};
    use kiln_core::projection::{
        ComponentVersion, Coverage, EntryKind, GeneratedInputs, Mount, ProjectionDefinition,
        ProjectionFacts, ProjectionProfile, ProviderBinding, RepositoryEnrollment,
        RepositoryLocator, Selection, SourceCapability, SourceEntry, TargetProfile,
        admit_definition, admit_repositories,
    };
    use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};
    use kiln_web_service::sources::{BrowserMode, RestrictedSourceAccess, RestrictedSourceRepo};
    use kiln_web_service::worker::source_auth::RequestProfile;

    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let (_repo_a, _first_a, second_a) = git_fixture(&root.path().join("a"));
    let repo_b = Repository::init(root.path().join("b").join("work"))
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let head_b = commit(
        &repo_b,
        &[("secret.md", "hidden\n"), ("src/lib.rs", "pub fn x() {}\n")],
        "Secret commit",
        None,
    );
    repo_b
        .reference("refs/heads/main", head_b, true, "fixture")
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    repo_b
        .set_head("refs/heads/main")
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));

    let profile = Profile {
        schema: "kiln.web-sources.v1".into(),
        sources: vec![
            policy("kiln", "git", &root.path().join("a/work")),
            policy("secret", "git", &root.path().join("b/work")),
        ],
    };
    let browser = Browser::restricted(Some(profile))
        .unwrap_or_else(|error| unreachable!("restricted browser: {error:?}"));
    assert_eq!(browser.mode(), BrowserMode::RestrictedProjection);

    // Un-scoped Browser::response fails closed in RestrictedProjection mode.
    let unscoped = browser.response("GET", "/api/sources", &[], Role::Viewer);
    assert_eq!(unscoped.status, 404);
    assert_eq!(value(&unscoped), json!({"error": "unavailable"}));
    assert_eq!(
        browser
            .response("GET", "/api/sources", &[], Role::Worker)
            .status,
        403
    );
    assert_eq!(
        browser
            .response("POST", "/api/sources", &[], Role::Viewer)
            .status,
        405
    );

    let enroll = |id: &str, path: &Path| {
        admit_repositories(
            &[RepositoryEnrollment {
                repository_id: id.into(),
                binding: ProviderBinding {
                    source_provider: ProviderProfileId::new("git-source-v1")
                        .unwrap_or_else(|e| unreachable!("{e:?}")),
                    native_repository: ExternalRef::new("git", "repository", id)
                        .unwrap_or_else(|e| unreachable!("{e:?}")),
                },
                locators: vec![RepositoryLocator::LocalPath {
                    path: path.to_string_lossy().into_owned(),
                }],
            }],
            &[],
        )
        .unwrap_or_else(|e| unreachable!("{e:?}"))
        .pop()
        .unwrap_or_else(|| unreachable!("admitted repo"))
    };
    let adm_a = enroll("kiln", &root.path().join("a/work"));
    let adm_b = enroll("secret", &root.path().join("b/work"));

    let def = admit_definition(&ProjectionDefinition {
        schema: kiln_core::projection::DEFINITION_SCHEMA.into(),
        id: "restricted-browser".into(),
        profile: ProjectionProfile::Runnable,
        selection: Selection {
            root: String::new(),
            include: vec!["src/**".into(), "README.md".into()],
            exclude: vec!["secret.txt".into()],
        },
        mounts: vec![Mount {
            source_prefix: String::new(),
            destination_prefix: String::new(),
        }],
        dependencies: Vec::new(),
        creation_destinations: Vec::new(),
        required_capabilities: vec![SourceCapability::ExactTree],
        generated_inputs: GeneratedInputs::Refused,
        target: TargetProfile::POSIX,
        compatibility: None,
    })
    .unwrap_or_else(|e| unreachable!("{e:?}"));

    let rev_a = ExternalRef::new("git", "commit", &second_a.to_string())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let rev_b = ExternalRef::new("git", "commit", &head_b.to_string())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let blob_ref = |bytes: &[u8]| {
        let oid = Oid::hash_object(git2::ObjectType::Blob, bytes)
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        ExternalRef::new("git", "blob", &oid.to_string()).unwrap_or_else(|e| unreachable!("{e:?}"))
    };
    let component = |name: &str| ComponentVersion {
        name: name.into(),
        version: "1".into(),
    };
    let tree_digest = ContentId::from_blake3_hex(blake3::hash(b"tree").to_hex().to_string())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let facts_a = ProjectionFacts {
        repository: adm_a.identity().clone(),
        exact_revision: rev_a.clone(),
        canonical_tree: tree_digest.clone(),
        evaluator: component("kiln-projection"),
        adapter: component("git-source"),
        source_capabilities: vec![SourceCapability::ExactTree],
        coverage: Coverage::Complete,
        entries: vec![
            SourceEntry {
                path: "README.md".into(),
                object: blob_ref(b"# fixture v2\n"),
                kind: EntryKind::File,
            },
            SourceEntry {
                path: "secret.txt".into(),
                object: blob_ref(b"excluded"),
                kind: EntryKind::File,
            },
            SourceEntry {
                path: "src/main.rs".into(),
                object: blob_ref(b"fn main() { println!(); }\n"),
                kind: EntryKind::File,
            },
        ],
        dependencies: Vec::new(),
        generated: Vec::new(),
    };
    let facts_b = ProjectionFacts {
        repository: adm_b.identity().clone(),
        exact_revision: rev_b.clone(),
        canonical_tree: tree_digest,
        evaluator: component("kiln-projection"),
        adapter: component("git-source"),
        source_capabilities: vec![SourceCapability::ExactTree],
        coverage: Coverage::Complete,
        entries: vec![SourceEntry {
            path: "src/lib.rs".into(),
            object: blob_ref(b"pub fn x() {}\n"),
            kind: EntryKind::File,
        }],
        dependencies: Vec::new(),
        generated: Vec::new(),
    };
    let cur_a = SourceCurrentness {
        generation: GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: rev_a,
            ref_name: Some("refs/heads/main".into()),
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };
    let cur_b = SourceCurrentness {
        generation: GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: rev_b,
            ref_name: Some("refs/heads/main".into()),
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };

    // Grant access ONLY to repo `kiln` (Discover + ReadContent on src/main.rs and root Discover),
    // leaving repo `secret` completely unauthorized.
    let fixture = SourceFixture::direct(
        COORDINATOR_SEED,
        adm_a.identity(),
        None,
        &[
            (PathScope::root(), vec![SourceViewOperation::Discover]),
            (
                PathScope::from_path("src/main.rs", false).unwrap_or_else(|| unreachable!("path")),
                vec![SourceViewOperation::ReadContent],
            ),
        ],
        EXPIRES_S,
    );
    let mut authority = fixture.authority();
    let signer = fixture.signer();
    let sign = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let handle_key = HandleKey::from_bytes([0x42; 32]);
    let repos = [
        RestrictedSourceRepo {
            policy_id: "kiln",
            admitted: &adm_a,
            definition: &def,
            facts: &facts_a,
            current: &cur_a,
        },
        RestrictedSourceRepo {
            policy_id: "secret",
            admitted: &adm_b,
            definition: &def,
            facts: &facts_b,
            current: &cur_b,
        },
    ];

    let mut call = |target: &str, role: Role, now_unix_s: u64| -> ApiResponse {
        let mut access = RestrictedSourceAccess {
            authority: &mut authority,
            profile: RequestProfile::Signer(&sign),
            repos: &repos,
            handle_key: &handle_key,
            now_unix_s,
        };
        browser.restricted_response("GET", target, &[], role, &mut access)
    };

    // 1. Worker role denied (403).
    assert_eq!(call("/api/sources", Role::Worker, VERIFY_AT_S).status, 403);

    // 2. Inventory omits unauthorized repo `secret`.
    let inv = call("/api/sources", Role::Viewer, VERIFY_AT_S);
    assert_eq!(inv.status, 200);
    let inv_json = value(&inv);
    let listed = inv_json
        .get("sources")
        .and_then(Value::as_array)
        .unwrap_or_else(|| unreachable!("sources array"));
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed
            .first()
            .and_then(|s| s.get("id"))
            .and_then(Value::as_str),
        Some("kiln")
    );

    // 3. Lookup omits unauthorized repo `secret`.
    let lookup_b = call(
        &format!("/api/sources/lookup?revision={head_b}"),
        Role::Viewer,
        VERIFY_AT_S,
    );
    assert_eq!(lookup_b.status, 200);
    assert_eq!(
        value(&lookup_b)
            .get("matches")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );

    // 4. Hidden repo and nonexistent repo return identical 404 PublicUnavailable (no oracle).
    for hidden_route in [
        "/api/sources/secret/tree",
        "/api/sources/secret/file?path=src/lib.rs",
        "/api/sources/secret/log",
        "/api/sources/secret/diff",
        "/api/sources/nonexistent/tree",
        "/api/sources/nonexistent/file?path=src/lib.rs",
    ] {
        let resp = call(hidden_route, Role::Viewer, VERIFY_AT_S);
        assert_eq!(resp.status, 404, "{hidden_route}");
        assert_eq!(
            value(&resp),
            json!({"error": "unavailable"}),
            "{hidden_route}"
        );
    }

    // 5. Excluded ViewCeiling path and ungranted sibling path return 404 unavailable.
    for blocked in [
        "/api/sources/kiln/file?path=secret.txt",
        "/api/sources/kiln/file?path=README.md",
        "/api/sources/kiln/file?path=missing.txt",
    ] {
        let resp = call(blocked, Role::Viewer, VERIFY_AT_S);
        assert_eq!(resp.status, 404, "{blocked}");
        assert_eq!(value(&resp), json!({"error": "unavailable"}), "{blocked}");
    }

    // 6. Tree lists admitted `src` and extracts opaque handle for `src/main.rs`, then
    //    `file?handle=<hex>` resolves via AudienceView::resolve + revalidate.
    let tree_resp = call("/api/sources/kiln/tree", Role::Viewer, VERIFY_AT_S);
    assert_eq!(tree_resp.status, 200);
    let tree_json = value(&tree_resp);
    let handle_hex = tree_json
        .pointer("/view/entries")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries.iter().find(|entry| {
                entry.get("display_path").and_then(Value::as_str) == Some("src/main.rs")
            })
        })
        .and_then(|entry| entry.get("handle").and_then(Value::as_str))
        .unwrap_or_else(|| unreachable!("handle for src/main.rs in {tree_json}"))
        .to_owned();
    let by_handle = call(
        &format!("/api/sources/kiln/file?handle={handle_hex}"),
        Role::Viewer,
        VERIFY_AT_S,
    );
    assert_eq!(by_handle.status, 200);
    assert_eq!(
        value(&by_handle).get("text").and_then(Value::as_str),
        Some("fn main() { println!(); }\n")
    );

    // 7. Forged handle fails closed with 404 unavailable.
    let forged = OpaqueHandle::from_hex(&"11".repeat(32))
        .unwrap_or_else(|| unreachable!("forged handle"))
        .to_hex();
    let forged_resp = call(
        &format!("/api/sources/kiln/file?handle={forged}"),
        Role::Viewer,
        VERIFY_AT_S,
    );
    assert_eq!(forged_resp.status, 404);
    assert_eq!(value(&forged_resp), json!({"error": "unavailable"}));

    // 8. Expired time fails closed.
    let expired = call("/api/sources/kiln/tree", Role::Viewer, EXPIRED_AT_S);
    assert_eq!(expired.status, 404);
    assert_eq!(value(&expired), json!({"error": "unavailable"}));

    // 9. Revoked token fails closed and disappears from inventory.
    fixture.revocations.revoke(&fixture.token);
    let revoked_file = call(
        "/api/sources/kiln/file?path=src/main.rs",
        Role::Viewer,
        VERIFY_AT_S,
    );
    assert_eq!(revoked_file.status, 404);
    assert_eq!(value(&revoked_file), json!({"error": "unavailable"}));
    let revoked_inv = call("/api/sources", Role::Viewer, VERIFY_AT_S);
    assert_eq!(
        value(&revoked_inv)
            .get("sources")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
}

fn commit_nested(
    repository: &Repository,
    files: &[(&str, &str)],
    message: &str,
    parent: Option<Oid>,
) -> Oid {
    let mut update = git2::build::TreeUpdateBuilder::new();
    for (path, body) in files {
        let blob = repository
            .blob(body.as_bytes())
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        update.upsert(*path, blob, git2::FileMode::Blob);
    }
    let empty_oid = repository
        .treebuilder(None)
        .and_then(|tb| tb.write())
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let empty_tree = repository
        .find_tree(empty_oid)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let tree_oid = update
        .create_updated(repository, &empty_tree)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let tree = repository
        .find_tree(tree_oid)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let parents: Vec<_> = parent
        .into_iter()
        .filter_map(|oid| repository.find_commit(oid).ok())
        .collect();
    let borrowed: Vec<_> = parents.iter().collect();
    let oid = repository
        .commit(None, &signature(), &signature(), message, &tree, &borrowed)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    repository
        .reference("refs/heads/main", oid, true, "fixture")
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    repository
        .set_head("refs/heads/main")
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    oid
}

#[test]
fn restricted_file_path_requires_discover_and_read_and_pins_exact_revision_against_moved_head() {
    use kiln_adapter_basalt::test_support::{
        COORDINATOR_SEED, EXPIRES_S, GENERATION, SourceFixture, VERIFY_AT_S,
    };
    use kiln_core::implementation::authority::{AttemptCurrentness, AuthorityProfile};
    use kiln_core::projection::authority::{
        PathScope, SnapshotObservation, SourceAuthorityRequest, SourceCurrentness,
        SourceViewOperation,
    };
    use kiln_core::projection::disclosure::HandleKey;
    use kiln_core::projection::{
        ComponentVersion, Coverage, EntryKind, GeneratedInputs, Mount, ProjectionDefinition,
        ProjectionFacts, ProjectionProfile, ProviderBinding, RepositoryEnrollment,
        RepositoryLocator, Selection, SourceCapability, SourceEntry, TargetProfile,
        admit_definition, admit_repositories,
    };
    use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};
    use kiln_web_service::sources::{RestrictedSourceAccess, RestrictedSourceRepo};
    use kiln_web_service::worker::source_auth::RequestProfile;

    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let work_a = root.path().join("a/work");
    let repo_a =
        Repository::init(&work_a).unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let first_a = commit_nested(
        &repo_a,
        &[
            ("README.md", "# v1\n"),
            ("src/main.rs", "const OLD_SECRET: &str = \"REV1\";\n"),
        ],
        "rev1",
        None,
    );
    let second_a = commit_nested(
        &repo_a,
        &[
            ("README.md", "# v2\n"),
            ("src/main.rs", "fn main() { /* v2 */ }\n"),
        ],
        "rev2",
        Some(first_a),
    );
    // Advance HEAD to third_a AFTER facts are pinned to second_a.
    let third_a = commit_nested(
        &repo_a,
        &[
            ("README.md", "# v3\n"),
            ("src/main.rs", "const FUTURE_SECRET: &str = \"REV3\";\n"),
        ],
        "rev3",
        Some(second_a),
    );

    let profile = Profile {
        schema: "kiln.web-sources.v1".into(),
        sources: vec![policy("kiln", "git", &work_a)],
    };
    let browser = Browser::restricted(Some(profile))
        .unwrap_or_else(|error| unreachable!("restricted browser: {error:?}"));
    let adm_a = admit_repositories(
        &[RepositoryEnrollment {
            repository_id: "kiln".into(),
            binding: ProviderBinding {
                source_provider: ProviderProfileId::new("git-source-v1")
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
                native_repository: ExternalRef::new("git", "repository", "kiln")
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
            },
            locators: vec![RepositoryLocator::LocalPath {
                path: work_a.to_string_lossy().into_owned(),
            }],
        }],
        &[],
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"))
    .pop()
    .unwrap_or_else(|| unreachable!("admitted repo"));

    let def = admit_definition(&ProjectionDefinition {
        schema: kiln_core::projection::DEFINITION_SCHEMA.into(),
        id: "full-pinned".into(),
        profile: ProjectionProfile::Runnable,
        selection: Selection {
            root: String::new(),
            include: vec!["**".into()],
            exclude: Vec::new(),
        },
        mounts: vec![Mount {
            source_prefix: String::new(),
            destination_prefix: String::new(),
        }],
        dependencies: Vec::new(),
        creation_destinations: Vec::new(),
        required_capabilities: vec![SourceCapability::ExactTree],
        generated_inputs: GeneratedInputs::Refused,
        target: TargetProfile::POSIX,
        compatibility: None,
    })
    .unwrap_or_else(|e| unreachable!("{e:?}"));

    let rev_a = ExternalRef::new("git", "commit", &second_a.to_string())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let blob_ref = |bytes: &[u8]| {
        let oid = Oid::hash_object(git2::ObjectType::Blob, bytes)
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        ExternalRef::new("git", "blob", &oid.to_string()).unwrap_or_else(|e| unreachable!("{e:?}"))
    };
    let component = |name: &str| ComponentVersion {
        name: name.into(),
        version: "1".into(),
    };
    let tree_digest = ContentId::from_blake3_hex(blake3::hash(b"t2").to_hex().to_string())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let facts_a = ProjectionFacts {
        repository: adm_a.identity().clone(),
        exact_revision: rev_a.clone(),
        canonical_tree: tree_digest,
        evaluator: component("kiln-projection"),
        adapter: component("git-source"),
        source_capabilities: vec![SourceCapability::ExactTree],
        coverage: Coverage::Complete,
        entries: vec![
            SourceEntry {
                path: "README.md".into(),
                object: blob_ref(b"# v2\n"),
                kind: EntryKind::File,
            },
            SourceEntry {
                path: "src/main.rs".into(),
                object: blob_ref(b"fn main() { /* v2 */ }\n"),
                kind: EntryKind::File,
            },
        ],
        dependencies: Vec::new(),
        generated: Vec::new(),
    };
    let cur_a = SourceCurrentness {
        generation: GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: rev_a,
            ref_name: Some("refs/heads/main".into()),
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };
    let handle_key = HandleKey::from_bytes([0x42; 32]);
    let repos = [RestrictedSourceRepo {
        policy_id: "kiln",
        admitted: &adm_a,
        definition: &def,
        facts: &facts_a,
        current: &cur_a,
    }];

    // 1. Full Discover + ReadContent + ReadHistory pinned to second_a
    let full_fix = SourceFixture::direct(
        COORDINATOR_SEED,
        adm_a.identity(),
        None,
        &[(
            PathScope::root(),
            vec![
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
                SourceViewOperation::ReadHistory,
            ],
        )],
        EXPIRES_S,
    );
    let mut full_auth = full_fix.authority();
    let full_signer = full_fix.signer();
    let sign_full = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        full_signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let mut call_full = |target: &str| {
        let mut access = RestrictedSourceAccess {
            authority: &mut full_auth,
            profile: RequestProfile::Signer(&sign_full),
            repos: &repos,
            handle_key: &handle_key,
            now_unix_s: VERIFY_AT_S,
        };
        browser.restricted_response("GET", target, &[], Role::Viewer, &mut access)
    };

    // file?path= works with omitted revision and exact revision, pinned to second_a
    for target in [
        "/api/sources/kiln/file?path=src/main.rs".to_string(),
        format!("/api/sources/kiln/file?revision={second_a}&path=src/main.rs"),
    ] {
        let resp = call_full(&target);
        assert_eq!(resp.status, 200, "{target}");
        let body = value(&resp);
        assert_eq!(
            body.get("revision").and_then(Value::as_str),
            Some(second_a.to_string().as_str())
        );
        assert_eq!(
            body.get("text").and_then(Value::as_str),
            Some("fn main() { /* v2 */ }\n")
        );
    }

    // Extract handle for handle-based checks
    let tree_resp = call_full("/api/sources/kiln/tree");
    assert_eq!(tree_resp.status, 200);
    let tree_json = value(&tree_resp);
    assert_eq!(
        tree_json.get("revision").and_then(Value::as_str),
        Some(second_a.to_string().as_str())
    );
    let handle_hex = tree_json
        .pointer("/view/entries")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries.iter().find(|entry| {
                entry.get("display_path").and_then(Value::as_str) == Some("src/main.rs")
            })
        })
        .and_then(|entry| entry.get("handle").and_then(Value::as_str))
        .unwrap_or_else(|| unreachable!("handle for src/main.rs"))
        .to_owned();

    // Omitted revision on file?handle=, diff, log, inventory all stay pinned to second_a
    let fh_omitted = call_full(&format!("/api/sources/kiln/file?handle={handle_hex}"));
    assert_eq!(fh_omitted.status, 200);
    let fh_body = value(&fh_omitted);
    assert_eq!(
        fh_body.get("revision").and_then(Value::as_str),
        Some(second_a.to_string().as_str())
    );
    assert_eq!(
        fh_body.get("text").and_then(Value::as_str),
        Some("fn main() { /* v2 */ }\n")
    );

    let diff_omitted = call_full("/api/sources/kiln/diff");
    assert_eq!(diff_omitted.status, 200);
    assert_eq!(
        value(&diff_omitted).get("revision").and_then(Value::as_str),
        Some(second_a.to_string().as_str())
    );

    let log_resp = call_full("/api/sources/kiln/log");
    assert_eq!(log_resp.status, 200);
    let log_json = value(&log_resp);
    assert_eq!(
        log_json.get("head").and_then(Value::as_str),
        Some(second_a.to_string().as_str())
    );
    assert!(
        !String::from_utf8_lossy(&log_resp.body).contains(&third_a.to_string()),
        "moved HEAD leaked in restricted_log"
    );

    let inv_resp = call_full("/api/sources");
    assert_eq!(inv_resp.status, 200);
    assert_eq!(
        value(&inv_resp)
            .pointer("/sources/0/head")
            .and_then(Value::as_str),
        Some(second_a.to_string().as_str())
    );

    // Mismatched revisions (first_a and third_a) are rejected across file, tree, diff, and lookup(third_a)
    for bad_rev in [first_a, third_a] {
        for bad_route in [
            format!("/api/sources/kiln/file?handle={handle_hex}&revision={bad_rev}"),
            format!("/api/sources/kiln/file?path=src/main.rs&revision={bad_rev}"),
            format!("/api/sources/kiln/tree?revision={bad_rev}"),
            format!("/api/sources/kiln/diff?revision={bad_rev}"),
        ] {
            let resp = call_full(&bad_route);
            assert_eq!(resp.status, 404, "{bad_route}");
            assert_eq!(value(&resp), json!({"error": "unavailable"}), "{bad_route}");
        }
    }
    let lookup_future = call_full(&format!("/api/sources/lookup?revision={third_a}"));
    assert_eq!(lookup_future.status, 200);
    assert_eq!(
        value(&lookup_future)
            .get("matches")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );

    // 2. ReadContent-only (without Discover) MUST fail closed on file?path=
    let read_only_fix = SourceFixture::direct(
        COORDINATOR_SEED,
        adm_a.identity(),
        None,
        &[(
            PathScope::from_path("src/main.rs", false).unwrap_or_else(|| unreachable!("path")),
            vec![SourceViewOperation::ReadContent],
        )],
        EXPIRES_S,
    );
    let mut read_only_auth = read_only_fix.authority();
    let read_only_signer = read_only_fix.signer();
    let sign_ro = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        read_only_signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let mut ro_access = RestrictedSourceAccess {
        authority: &mut read_only_auth,
        profile: RequestProfile::Signer(&sign_ro),
        repos: &repos,
        handle_key: &handle_key,
        now_unix_s: VERIFY_AT_S,
    };
    let ro_resp = browser.restricted_response(
        "GET",
        "/api/sources/kiln/file?path=src/main.rs",
        &[],
        Role::Viewer,
        &mut ro_access,
    );
    assert_eq!(ro_resp.status, 404);
    assert_eq!(value(&ro_resp), json!({"error": "unavailable"}));
}

#[test]
fn restricted_sources_eliminate_400_vs_404_oracle_on_invalid_queries() {
    use kiln_adapter_basalt::test_support::{
        COORDINATOR_SEED, EXPIRES_S, GENERATION, SourceFixture, VERIFY_AT_S,
    };
    use kiln_core::implementation::authority::{AttemptCurrentness, AuthorityProfile};
    use kiln_core::projection::authority::{
        PathScope, SnapshotObservation, SourceAuthorityRequest, SourceCurrentness,
        SourceViewOperation,
    };
    use kiln_core::projection::disclosure::HandleKey;
    use kiln_core::projection::{
        ComponentVersion, Coverage, EntryKind, GeneratedInputs, Mount, ProjectionDefinition,
        ProjectionFacts, ProjectionProfile, ProviderBinding, RepositoryEnrollment,
        RepositoryLocator, Selection, SourceCapability, SourceEntry, TargetProfile,
        admit_definition, admit_repositories,
    };
    use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};
    use kiln_web_service::sources::{RestrictedSourceAccess, RestrictedSourceRepo};
    use kiln_web_service::worker::source_auth::RequestProfile;

    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let (_repo_a, _, second_a) = git_fixture(&root.path().join("a"));
    let (_repo_b, _, head_b) = git_fixture(&root.path().join("b"));
    let profile = Profile {
        schema: "kiln.web-sources.v1".into(),
        sources: vec![
            policy("kiln", "git", &root.path().join("a/work")),
            policy("secret", "git", &root.path().join("b/work")),
        ],
    };
    let browser = Browser::restricted(Some(profile))
        .unwrap_or_else(|error| unreachable!("restricted browser: {error:?}"));
    let enroll = |id: &str, path: &Path| {
        admit_repositories(
            &[RepositoryEnrollment {
                repository_id: id.into(),
                binding: ProviderBinding {
                    source_provider: ProviderProfileId::new("git-source-v1")
                        .unwrap_or_else(|e| unreachable!("{e:?}")),
                    native_repository: ExternalRef::new("git", "repository", id)
                        .unwrap_or_else(|e| unreachable!("{e:?}")),
                },
                locators: vec![RepositoryLocator::LocalPath {
                    path: path.to_string_lossy().into_owned(),
                }],
            }],
            &[],
        )
        .unwrap_or_else(|e| unreachable!("{e:?}"))
        .pop()
        .unwrap_or_else(|| unreachable!("admitted repo"))
    };
    let adm_a = enroll("kiln", &root.path().join("a/work"));
    let adm_b = enroll("secret", &root.path().join("b/work"));
    let def = admit_definition(&ProjectionDefinition {
        schema: kiln_core::projection::DEFINITION_SCHEMA.into(),
        id: "oracle-def".into(),
        profile: ProjectionProfile::Runnable,
        selection: Selection {
            root: String::new(),
            include: vec!["src/**".into()],
            exclude: Vec::new(),
        },
        mounts: vec![Mount {
            source_prefix: String::new(),
            destination_prefix: String::new(),
        }],
        dependencies: Vec::new(),
        creation_destinations: Vec::new(),
        required_capabilities: vec![SourceCapability::ExactTree],
        generated_inputs: GeneratedInputs::Refused,
        target: TargetProfile::POSIX,
        compatibility: None,
    })
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    let mk_facts = |adm: &kiln_core::projection::AdmittedRepository, rev: Oid| {
        let ext = ExternalRef::new("git", "commit", &rev.to_string())
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        let blob = ExternalRef::new(
            "git",
            "blob",
            &Oid::hash_object(git2::ObjectType::Blob, b"fn main() {}\n")
                .unwrap_or_else(|e| unreachable!("{e:?}"))
                .to_string(),
        )
        .unwrap_or_else(|e| unreachable!("{e:?}"));
        let tree_id = ContentId::from_blake3_hex(blake3::hash(b"t").to_hex().to_string())
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        ProjectionFacts {
            repository: adm.identity().clone(),
            exact_revision: ext.clone(),
            canonical_tree: tree_id,
            evaluator: ComponentVersion {
                name: "kiln-projection".into(),
                version: "1".into(),
            },
            adapter: ComponentVersion {
                name: "git-source".into(),
                version: "1".into(),
            },
            source_capabilities: vec![SourceCapability::ExactTree],
            coverage: Coverage::Complete,
            entries: vec![SourceEntry {
                path: "src/main.rs".into(),
                object: blob,
                kind: EntryKind::File,
            }],
            dependencies: Vec::new(),
            generated: Vec::new(),
        }
    };
    let facts_a = mk_facts(&adm_a, second_a);
    let facts_b = mk_facts(&adm_b, head_b);
    let mk_cur = |facts: &ProjectionFacts| SourceCurrentness {
        generation: GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: facts.exact_revision.clone(),
            ref_name: None,
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };
    let cur_a = mk_cur(&facts_a);
    let cur_b = mk_cur(&facts_b);
    let fixture = SourceFixture::direct(
        COORDINATOR_SEED,
        adm_a.identity(),
        None,
        &[(PathScope::root(), vec![SourceViewOperation::Discover])],
        EXPIRES_S,
    );
    let mut authority = fixture.authority();
    let signer = fixture.signer();
    let sign = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let handle_key = HandleKey::from_bytes([0x42; 32]);
    let repos = [
        RestrictedSourceRepo {
            policy_id: "kiln",
            admitted: &adm_a,
            definition: &def,
            facts: &facts_a,
            current: &cur_a,
        },
        RestrictedSourceRepo {
            policy_id: "secret",
            admitted: &adm_b,
            definition: &def,
            facts: &facts_b,
            current: &cur_b,
        },
    ];
    let mut call = |target: &str| {
        let mut access = RestrictedSourceAccess {
            authority: &mut authority,
            profile: RequestProfile::Signer(&sign),
            repos: &repos,
            handle_key: &handle_key,
            now_unix_s: VERIFY_AT_S,
        };
        browser.restricted_response("GET", target, &[], Role::Viewer, &mut access)
    };

    for (secret_path, missing_path) in [
        (
            "/api/sources/secret/tree?path=../x",
            "/api/sources/nonexistent/tree?path=../x",
        ),
        (
            "/api/sources/secret/file?path=../x",
            "/api/sources/nonexistent/file?path=../x",
        ),
        ("/api/sources/secret/file", "/api/sources/nonexistent/file"),
        (
            "/api/sources/secret/log?limit=0",
            "/api/sources/nonexistent/log?limit=0",
        ),
    ] {
        let r_secret = call(secret_path);
        let r_missing = call(missing_path);
        assert_eq!(r_secret.status, 404, "{secret_path}");
        assert_eq!(r_missing.status, 404, "{missing_path}");
        assert_eq!(
            r_secret.body, r_missing.body,
            "{secret_path} vs {missing_path}"
        );
    }

    let r_secret_both = call(&format!(
        "/api/sources/secret/file?handle={}&path=../x",
        "11".repeat(32)
    ));
    let r_missing_both = call(&format!(
        "/api/sources/nonexistent/file?handle={}&path=../x",
        "11".repeat(32)
    ));
    assert_eq!(r_secret_both.status, 400);
    assert_eq!(r_missing_both.status, 400);
    assert_eq!(
        value(&r_secret_both).get("code").and_then(Value::as_str),
        Some("query_invalid")
    );
    assert_eq!(r_secret_both.body, r_missing_both.body);
}

#[test]
fn restricted_sources_support_multi_ceiling_and_sub_ceiling_discovery_and_handles() {
    use kiln_adapter_basalt::test_support::{
        COORDINATOR_SEED, EXPIRES_S, GENERATION, SourceFixture, VERIFY_AT_S,
    };
    use kiln_core::implementation::authority::{AttemptCurrentness, AuthorityProfile};
    use kiln_core::projection::authority::{
        PathScope, SnapshotObservation, SourceAuthorityRequest, SourceCurrentness,
        SourceViewOperation,
    };
    use kiln_core::projection::disclosure::HandleKey;
    use kiln_core::projection::{
        ComponentVersion, Coverage, EntryKind, GeneratedInputs, Mount, ProjectionDefinition,
        ProjectionFacts, ProjectionProfile, ProviderBinding, RepositoryEnrollment,
        RepositoryLocator, Selection, SourceCapability, SourceEntry, TargetProfile,
        admit_definition, admit_repositories,
    };
    use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};
    use kiln_web_service::sources::{RestrictedSourceAccess, RestrictedSourceRepo};
    use kiln_web_service::worker::source_auth::RequestProfile;

    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let work = root.path().join("work");
    let repo =
        Repository::init(&work).unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let rev = commit_nested(
        &repo,
        &[
            ("docs/a.md", "doc a\n"),
            ("src/public/mod.rs", "pub fn ok() {}\n"),
            (
                "src/private/secret.rs",
                "const SECRET: &str = \"hidden\";\n",
            ),
        ],
        "multi",
        None,
    );
    let browser = Browser::restricted(Some(Profile {
        schema: "kiln.web-sources.v1".into(),
        sources: vec![policy("kiln", "git", &work)],
    }))
    .unwrap_or_else(|error| unreachable!("restricted browser: {error:?}"));
    let adm = admit_repositories(
        &[RepositoryEnrollment {
            repository_id: "kiln".into(),
            binding: ProviderBinding {
                source_provider: ProviderProfileId::new("git-source-v1")
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
                native_repository: ExternalRef::new("git", "repository", "kiln")
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
            },
            locators: vec![RepositoryLocator::LocalPath {
                path: work.to_string_lossy().into_owned(),
            }],
        }],
        &[],
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"))
    .pop()
    .unwrap_or_else(|| unreachable!("admitted repo"));
    let def = admit_definition(&ProjectionDefinition {
        schema: kiln_core::projection::DEFINITION_SCHEMA.into(),
        id: "multi-ceiling".into(),
        profile: ProjectionProfile::Runnable,
        selection: Selection {
            root: String::new(),
            include: vec!["docs/**".into(), "src/**".into()],
            exclude: Vec::new(),
        },
        mounts: vec![Mount {
            source_prefix: String::new(),
            destination_prefix: String::new(),
        }],
        dependencies: Vec::new(),
        creation_destinations: Vec::new(),
        required_capabilities: vec![SourceCapability::ExactTree],
        generated_inputs: GeneratedInputs::Refused,
        target: TargetProfile::POSIX,
        compatibility: None,
    })
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    let ext_rev = ExternalRef::new("git", "commit", &rev.to_string())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let blob_ref = |bytes: &[u8]| {
        let oid = Oid::hash_object(git2::ObjectType::Blob, bytes)
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        ExternalRef::new("git", "blob", &oid.to_string()).unwrap_or_else(|e| unreachable!("{e:?}"))
    };
    let component = |name: &str| ComponentVersion {
        name: name.into(),
        version: "1".into(),
    };
    let tree_id = ContentId::from_blake3_hex(blake3::hash(b"t-multi").to_hex().to_string())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let facts = ProjectionFacts {
        repository: adm.identity().clone(),
        exact_revision: ext_rev.clone(),
        canonical_tree: tree_id,
        evaluator: component("kiln-projection"),
        adapter: component("git-source"),
        source_capabilities: vec![SourceCapability::ExactTree],
        coverage: Coverage::Complete,
        entries: vec![
            SourceEntry {
                path: "docs/a.md".into(),
                object: blob_ref(b"doc a\n"),
                kind: EntryKind::File,
            },
            SourceEntry {
                path: "src/public/mod.rs".into(),
                object: blob_ref(b"pub fn ok() {}\n"),
                kind: EntryKind::File,
            },
            SourceEntry {
                path: "src/private/secret.rs".into(),
                object: blob_ref(b"const SECRET: &str = \"hidden\";\n"),
                kind: EntryKind::File,
            },
        ],
        dependencies: Vec::new(),
        generated: Vec::new(),
    };
    let cur = SourceCurrentness {
        generation: GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: ext_rev,
            ref_name: None,
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };
    let fixture = SourceFixture::direct(
        COORDINATOR_SEED,
        adm.identity(),
        None,
        &[
            (
                PathScope::from_path("docs", true).unwrap_or_else(|| unreachable!("path")),
                vec![
                    SourceViewOperation::Discover,
                    SourceViewOperation::ReadContent,
                ],
            ),
            (
                PathScope::from_path("src/public", true).unwrap_or_else(|| unreachable!("path")),
                vec![
                    SourceViewOperation::Discover,
                    SourceViewOperation::ReadContent,
                ],
            ),
        ],
        EXPIRES_S,
    );
    let mut authority = fixture.authority();
    let signer = fixture.signer();
    let sign = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let handle_key = HandleKey::from_bytes([0x42; 32]);
    let repos = [RestrictedSourceRepo {
        policy_id: "kiln",
        admitted: &adm,
        definition: &def,
        facts: &facts,
        current: &cur,
    }];
    let mut call = |target: &str| {
        let mut access = RestrictedSourceAccess {
            authority: &mut authority,
            profile: RequestProfile::Signer(&sign),
            repos: &repos,
            handle_key: &handle_key,
            now_unix_s: VERIFY_AT_S,
        };
        browser.restricted_response("GET", target, &[], Role::Viewer, &mut access)
    };

    let inv = call("/api/sources");
    assert_eq!(inv.status, 200);
    let inv_str = String::from_utf8_lossy(&inv.body);
    assert!(inv_str.contains("docs/a.md"), "{inv_str}");
    assert!(inv_str.contains("src/public/mod.rs"), "{inv_str}");
    assert!(!inv_str.contains("src/private"), "{inv_str}");

    let root_tree = call("/api/sources/kiln/tree");
    assert_eq!(root_tree.status, 200);
    let root_json = value(&root_tree);
    let root_entries: Vec<&str> = root_json
        .get("entries")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|e| e.get("path").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(root_entries, vec!["docs", "src"]);

    let docs_tree = call("/api/sources/kiln/tree?path=docs");
    assert_eq!(docs_tree.status, 200);
    assert_eq!(
        value(&docs_tree)
            .pointer("/entries/0/path")
            .and_then(Value::as_str),
        Some("docs/a.md")
    );

    let src_tree = call("/api/sources/kiln/tree?path=src");
    assert_eq!(src_tree.status, 200);
    let src_json = value(&src_tree);
    let src_entries: Vec<&str> = src_json
        .get("entries")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|e| e.get("path").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(src_entries, vec!["src/public"]);

    let pub_tree = call("/api/sources/kiln/tree?path=src/public");
    assert_eq!(pub_tree.status, 200);
    let pub_json = value(&pub_tree);
    assert_eq!(
        pub_json.pointer("/entries/0/path").and_then(Value::as_str),
        Some("src/public/mod.rs")
    );
    let pub_handle = pub_json
        .pointer("/view/entries")
        .and_then(Value::as_array)
        .and_then(|arr| {
            arr.iter().find(|e| {
                e.get("display_path").and_then(Value::as_str) == Some("src/public/mod.rs")
            })
        })
        .and_then(|e| e.get("handle").and_then(Value::as_str))
        .unwrap_or_else(|| unreachable!("handle for src/public/mod.rs in {pub_json}"))
        .to_owned();

    let priv_tree = call("/api/sources/kiln/tree?path=src/private");
    assert_eq!(priv_tree.status, 404);
    assert_eq!(value(&priv_tree), json!({"error": "unavailable"}));

    let pub_file = call(&format!("/api/sources/kiln/file?handle={pub_handle}"));
    assert_eq!(pub_file.status, 200);
    assert_eq!(
        value(&pub_file).get("text").and_then(Value::as_str),
        Some("pub fn ok() {}\n")
    );
}

#[test]
fn restricted_diff_enforces_full_ceiling_and_empty_excludes_and_inventory_revalidates() {
    use kiln_adapter_basalt::test_support::{
        COORDINATOR_SEED, EXPIRES_S, GENERATION, SourceFixture, VERIFY_AT_S,
    };
    use kiln_core::implementation::authority::{
        AttemptCurrentness, AuthorityProfile, AuthoritySessionId,
    };
    use kiln_core::projection::authority::{
        EffectiveSourceScope, PathScope, SnapshotObservation, SourceAuthority,
        SourceAuthorityDecision, SourceAuthorityRequest, SourceCurrentness, SourceDenialReason,
        SourceViewOperation,
    };
    use kiln_core::projection::disclosure::HandleKey;
    use kiln_core::projection::{
        ComponentVersion, Coverage, EntryKind, GeneratedInputs, Mount, ProjectionDefinition,
        ProjectionFacts, ProjectionProfile, ProviderBinding, RepositoryEnrollment,
        RepositoryLocator, Selection, SourceCapability, SourceEntry, TargetProfile,
        admit_definition, admit_repositories,
    };
    use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};
    use kiln_web_service::sources::{RestrictedSourceAccess, RestrictedSourceRepo};
    use kiln_web_service::worker::source_auth::RequestProfile;

    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let (_repo_a, _, second_a) = git_fixture(&root.path().join("a"));
    let profile = Profile {
        schema: "kiln.web-sources.v1".into(),
        sources: vec![policy("kiln", "git", &root.path().join("a/work"))],
    };
    let browser = Browser::restricted(Some(profile))
        .unwrap_or_else(|error| unreachable!("restricted browser: {error:?}"));
    let adm = admit_repositories(
        &[RepositoryEnrollment {
            repository_id: "kiln".into(),
            binding: ProviderBinding {
                source_provider: ProviderProfileId::new("git-source-v1")
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
                native_repository: ExternalRef::new("git", "repository", "kiln")
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
            },
            locators: vec![RepositoryLocator::LocalPath {
                path: root.path().join("a/work").to_string_lossy().into_owned(),
            }],
        }],
        &[],
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"))
    .pop()
    .unwrap_or_else(|| unreachable!("admitted repo"));

    let make_def = |id: &str, include: Vec<String>, exclude: Vec<String>| {
        admit_definition(&ProjectionDefinition {
            schema: kiln_core::projection::DEFINITION_SCHEMA.into(),
            id: id.into(),
            profile: ProjectionProfile::Runnable,
            selection: Selection {
                root: String::new(),
                include,
                exclude,
            },
            mounts: vec![Mount {
                source_prefix: String::new(),
                destination_prefix: String::new(),
            }],
            dependencies: Vec::new(),
            creation_destinations: Vec::new(),
            required_capabilities: vec![SourceCapability::ExactTree],
            generated_inputs: GeneratedInputs::Refused,
            target: TargetProfile::POSIX,
            compatibility: None,
        })
        .unwrap_or_else(|e| unreachable!("{e:?}"))
    };
    let def_partial = make_def("partial", vec!["src/**".into()], Vec::new());
    let def_excluded = make_def("excluded", vec!["**".into()], vec!["README.md".into()]);
    let def_full = make_def("full", vec!["**".into()], Vec::new());

    let rev = ExternalRef::new("git", "commit", &second_a.to_string())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let blob = ExternalRef::new(
        "git",
        "blob",
        &Oid::hash_object(git2::ObjectType::Blob, b"fn main() { println!(); }\n")
            .unwrap_or_else(|e| unreachable!("{e:?}"))
            .to_string(),
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    let component = |name: &str| ComponentVersion {
        name: name.into(),
        version: "1".into(),
    };
    let tree_id = ContentId::from_blake3_hex(blake3::hash(b"t-diff").to_hex().to_string())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let facts = ProjectionFacts {
        repository: adm.identity().clone(),
        exact_revision: rev.clone(),
        canonical_tree: tree_id,
        evaluator: component("kiln-projection"),
        adapter: component("git-source"),
        source_capabilities: vec![SourceCapability::ExactTree],
        coverage: Coverage::Complete,
        entries: vec![SourceEntry {
            path: "src/main.rs".into(),
            object: blob,
            kind: EntryKind::File,
        }],
        dependencies: Vec::new(),
        generated: Vec::new(),
    };
    let cur = SourceCurrentness {
        generation: GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: rev,
            ref_name: None,
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };
    let fixture = SourceFixture::direct(
        COORDINATOR_SEED,
        adm.identity(),
        None,
        &[(
            PathScope::root(),
            vec![
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
                SourceViewOperation::ReadHistory,
            ],
        )],
        EXPIRES_S,
    );
    let signer = fixture.signer();
    let sign = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        signer
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let handle_key = HandleKey::from_bytes([0x42; 32]);

    // 1. Partial ceiling & non-empty exclude both deny restricted_diff
    for bad_def in [&def_partial, &def_excluded] {
        let mut auth = fixture.authority();
        let repos = [RestrictedSourceRepo {
            policy_id: "kiln",
            admitted: &adm,
            definition: bad_def,
            facts: &facts,
            current: &cur,
        }];
        let mut access = RestrictedSourceAccess {
            authority: &mut auth,
            profile: RequestProfile::Signer(&sign),
            repos: &repos,
            handle_key: &handle_key,
            now_unix_s: VERIFY_AT_S,
        };
        let resp = browser.restricted_response(
            "GET",
            "/api/sources/kiln/diff",
            &[],
            Role::Viewer,
            &mut access,
        );
        assert_eq!(resp.status, 404);
        assert_eq!(value(&resp), json!({"error": "unavailable"}));
    }

    // 2. Full ceiling allows restricted_diff, and inventory invokes revalidate
    struct RevalidateSpy<A> {
        inner: A,
        revalid_calls: usize,
        deny_revalid: bool,
    }
    impl<A: SourceAuthority> SourceAuthority for RevalidateSpy<A> {
        fn admit(
            &mut self,
            request: &SourceAuthorityRequest,
            profile: &AuthorityProfile,
            current: &SourceCurrentness,
            now_unix_s: u64,
        ) -> Result<SourceAuthorityDecision, SourceAuthorityDecision> {
            self.inner.admit(request, profile, current, now_unix_s)
        }
        fn revalidate(
            &mut self,
            session: AuthoritySessionId,
            request: &SourceAuthorityRequest,
            current: &SourceCurrentness,
            now_unix_s: u64,
        ) -> Result<(), SourceAuthorityDecision> {
            self.revalid_calls = self.revalid_calls.saturating_add(1);
            if self.deny_revalid {
                return Err(SourceAuthorityDecision::deny(
                    request,
                    SourceDenialReason::Revoked,
                ));
            }
            self.inner.revalidate(session, request, current, now_unix_s)
        }
        fn close(&mut self, session: AuthoritySessionId) {
            self.inner.close(session);
        }
        fn effective_scope(&self, session: AuthoritySessionId) -> Option<&EffectiveSourceScope> {
            self.inner.effective_scope(session)
        }
        fn decisions(&self) -> &[SourceAuthorityDecision] {
            self.inner.decisions()
        }
    }

    let mut spy = RevalidateSpy {
        inner: fixture.authority(),
        revalid_calls: 0,
        deny_revalid: false,
    };
    let repos_full = [RestrictedSourceRepo {
        policy_id: "kiln",
        admitted: &adm,
        definition: &def_full,
        facts: &facts,
        current: &cur,
    }];
    {
        let mut access = RestrictedSourceAccess {
            authority: &mut spy,
            profile: RequestProfile::Signer(&sign),
            repos: &repos_full,
            handle_key: &handle_key,
            now_unix_s: VERIFY_AT_S,
        };
        let diff_ok = browser.restricted_response(
            "GET",
            "/api/sources/kiln/diff",
            &[],
            Role::Viewer,
            &mut access,
        );
        assert_eq!(diff_ok.status, 200);
        let inv_ok =
            browser.restricted_response("GET", "/api/sources", &[], Role::Viewer, &mut access);
        assert_eq!(inv_ok.status, 200);
        assert_eq!(
            value(&inv_ok)
                .get("sources")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1)
        );
    }
    assert!(spy.revalid_calls >= 2, "expected revalidate on inventory");

    spy.deny_revalid = true;
    let mut access_denied = RestrictedSourceAccess {
        authority: &mut spy,
        profile: RequestProfile::Signer(&sign),
        repos: &repos_full,
        handle_key: &handle_key,
        now_unix_s: VERIFY_AT_S,
    };
    let inv_denied =
        browser.restricted_response("GET", "/api/sources", &[], Role::Viewer, &mut access_denied);
    assert_eq!(inv_denied.status, 200);
    assert_eq!(
        value(&inv_denied)
            .get("sources")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
}

#[test]
fn restricted_sources_support_over_64_files_under_local_and_basalt_authorities() {
    use std::cell::Cell;
    use std::collections::BTreeSet;

    use kiln_adapter_basalt::test_support::{
        COORDINATOR_SEED, EXPIRES_S, GENERATION, SourceFixture, VERIFY_AT_S, seed,
    };
    use kiln_adapter_basalt::{SourceHolderKey, SourceSigner};
    use kiln_core::implementation::authority::{
        AttemptCurrentness, AuthorityProfile, AuthoritySessionId,
    };
    use kiln_core::projection::authority::{
        EffectiveSourceScope, LocalSourceAuthority, LocalSourceGrant, PathScope,
        SnapshotObservation, SourceAuthority, SourceAuthorityDecision, SourceAuthorityRequest,
        SourceCurrentness, SourceViewOperation,
    };
    use kiln_core::projection::disclosure::HandleKey;
    use kiln_core::projection::{
        ComponentVersion, Coverage, EntryKind, GeneratedInputs, Mount, ProjectionDefinition,
        ProjectionFacts, ProjectionProfile, ProviderBinding, RepositoryEnrollment,
        RepositoryLocator, Selection, SourceCapability, SourceEntry, TargetProfile,
        admit_definition, admit_repositories,
    };
    use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};
    use kiln_web_service::sources::{RestrictedSourceAccess, RestrictedSourceRepo};
    use kiln_web_service::worker::source_auth::RequestProfile;

    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let work = root.path().join("work");
    let repo =
        Repository::init(&work).unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let file_specs: Vec<(String, String)> = (0..70)
        .map(|i| {
            (
                format!("src/f{i:02}.rs"),
                format!("pub const F{i}: usize = {i};\n"),
            )
        })
        .collect();
    let file_refs: Vec<(&str, &str)> = file_specs
        .iter()
        .map(|(p, b)| (p.as_str(), b.as_str()))
        .collect();
    let rev = commit_nested(&repo, &file_refs, "70 files", None);
    let browser = Browser::restricted(Some(Profile {
        schema: "kiln.web-sources.v1".into(),
        sources: vec![policy("kiln", "git", &work)],
    }))
    .unwrap_or_else(|error| unreachable!("restricted browser: {error:?}"));
    let adm = admit_repositories(
        &[RepositoryEnrollment {
            repository_id: "kiln".into(),
            binding: ProviderBinding {
                source_provider: ProviderProfileId::new("git-source-v1")
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
                native_repository: ExternalRef::new("git", "repository", "kiln")
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
            },
            locators: vec![RepositoryLocator::LocalPath {
                path: work.to_string_lossy().into_owned(),
            }],
        }],
        &[],
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"))
    .pop()
    .unwrap_or_else(|| unreachable!("admitted repo"));
    let def = admit_definition(&ProjectionDefinition {
        schema: kiln_core::projection::DEFINITION_SCHEMA.into(),
        id: "over-64".into(),
        profile: ProjectionProfile::Runnable,
        selection: Selection {
            root: String::new(),
            include: vec!["**".into()],
            exclude: Vec::new(),
        },
        mounts: vec![Mount {
            source_prefix: String::new(),
            destination_prefix: String::new(),
        }],
        dependencies: Vec::new(),
        creation_destinations: Vec::new(),
        required_capabilities: vec![SourceCapability::ExactTree],
        generated_inputs: GeneratedInputs::Refused,
        target: TargetProfile::POSIX,
        compatibility: None,
    })
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    let ext_rev = ExternalRef::new("git", "commit", &rev.to_string())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let blob_ref = |bytes: &[u8]| {
        let oid = Oid::hash_object(git2::ObjectType::Blob, bytes)
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        ExternalRef::new("git", "blob", &oid.to_string()).unwrap_or_else(|e| unreachable!("{e:?}"))
    };
    let component = |name: &str| ComponentVersion {
        name: name.into(),
        version: "1".into(),
    };
    let tree_id = ContentId::from_blake3_hex(blake3::hash(b"t-70").to_hex().to_string())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let facts = ProjectionFacts {
        repository: adm.identity().clone(),
        exact_revision: ext_rev.clone(),
        canonical_tree: tree_id,
        evaluator: component("kiln-projection"),
        adapter: component("git-source"),
        source_capabilities: vec![SourceCapability::ExactTree],
        coverage: Coverage::Complete,
        entries: file_specs
            .iter()
            .map(|(path, body)| SourceEntry {
                path: path.clone(),
                object: blob_ref(body.as_bytes()),
                kind: EntryKind::File,
            })
            .collect(),
        dependencies: Vec::new(),
        generated: Vec::new(),
    };
    let cur = SourceCurrentness {
        generation: GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: ext_rev,
            ref_name: None,
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };
    let handle_key = HandleKey::from_bytes([0x42; 32]);
    let repos = [RestrictedSourceRepo {
        policy_id: "kiln",
        admitted: &adm,
        definition: &def,
        facts: &facts,
        current: &cur,
    }];

    // Helper for BasaltSourceAuthority with u64 nonces (>255 signatures).
    let make_u64_signer = |fixture: &SourceFixture| {
        let nonce_counter = Cell::new(0_u64);
        let holder = SourceHolderKey::new(seed(COORDINATOR_SEED), fixture.token.clone())
            .unwrap_or_else(|e| unreachable!("{e}"));
        SourceSigner::new(
            &fixture.config(),
            holder,
            fixture.max_delegation_depth,
            Box::new(move || {
                let next = nonce_counter.get().checked_add(1)?;
                nonce_counter.set(next);
                let mut bytes = [0_u8; 32];
                if let Some(head) = bytes.get_mut(..8) {
                    head.copy_from_slice(&next.to_le_bytes());
                }
                Some(bytes)
            }),
        )
        .unwrap_or_else(|e| unreachable!("{e}"))
    };

    // Part A1: BasaltSourceAuthority with full root grant across 70 files.
    let fixture_full = SourceFixture::direct(
        COORDINATOR_SEED,
        adm.identity(),
        None,
        &[(
            PathScope::root(),
            vec![
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
            ],
        )],
        EXPIRES_S,
    );
    let mut auth_full = fixture_full.authority();
    let signer_full = make_u64_signer(&fixture_full);
    let sign_full = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        signer_full
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let mut call_full = |target: &str| {
        let mut access = RestrictedSourceAccess {
            authority: &mut auth_full,
            profile: RequestProfile::Signer(&sign_full),
            repos: &repos,
            handle_key: &handle_key,
            now_unix_s: VERIFY_AT_S,
        };
        browser.restricted_response("GET", target, &[], Role::Viewer, &mut access)
    };

    let inv = call_full("/api/sources");
    assert_eq!(inv.status, 200);
    let inv_json = value(&inv);
    assert_eq!(
        inv_json
            .get("sources")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    let inv_entries = inv_json
        .pointer("/sources/0/view/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| unreachable!("inv view entries in {inv_json}"));
    assert_eq!(inv_entries.len(), 70);
    assert_eq!(
        inv_entries
            .iter()
            .find(|e| e.get("display_path").and_then(Value::as_str) == Some("src/f00.rs"))
            .and_then(|e| e.get("readable").and_then(Value::as_bool)),
        Some(true)
    );
    assert_eq!(
        inv_entries
            .iter()
            .find(|e| e.get("display_path").and_then(Value::as_str) == Some("src/f69.rs"))
            .and_then(|e| e.get("readable").and_then(Value::as_bool)),
        Some(true)
    );

    let tree = call_full("/api/sources/kiln/tree?path=src");
    assert_eq!(tree.status, 200);
    let tree_json = value(&tree);
    assert_eq!(
        tree_json
            .get("entries")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(70)
    );
    let f69_handle = tree_json
        .pointer("/view/entries")
        .and_then(Value::as_array)
        .and_then(|arr| {
            arr.iter()
                .find(|e| e.get("display_path").and_then(Value::as_str) == Some("src/f69.rs"))
        })
        .and_then(|e| {
            assert_eq!(e.get("readable").and_then(Value::as_bool), Some(true));
            e.get("handle").and_then(Value::as_str)
        })
        .unwrap_or_else(|| unreachable!("src/f69.rs handle in {tree_json}"))
        .to_owned();
    assert!(!f69_handle.is_empty());

    let file_by_handle = call_full(&format!("/api/sources/kiln/file?handle={f69_handle}"));
    assert_eq!(file_by_handle.status, 200);
    assert_eq!(
        value(&file_by_handle).get("text").and_then(Value::as_str),
        Some("pub const F69: usize = 69;\n")
    );

    let file_by_path = call_full("/api/sources/kiln/file?path=src/f69.rs");
    assert_eq!(file_by_path.status, 200);
    assert_eq!(
        value(&file_by_path).get("text").and_then(Value::as_str),
        Some("pub const F69: usize = 69;\n")
    );

    // Part A2: Partial ReadContent negative/mixed case on the 70-file repo.
    let f00_scope =
        PathScope::from_path("src/f00.rs", false).unwrap_or_else(|| unreachable!("f00"));
    let f69_scope =
        PathScope::from_path("src/f69.rs", false).unwrap_or_else(|| unreachable!("f69"));
    let fixture_partial = SourceFixture::direct(
        COORDINATOR_SEED,
        adm.identity(),
        None,
        &[
            (PathScope::root(), vec![SourceViewOperation::Discover]),
            (f00_scope, vec![SourceViewOperation::ReadContent]),
            (f69_scope, vec![SourceViewOperation::ReadContent]),
        ],
        EXPIRES_S,
    );
    let mut auth_partial = fixture_partial.authority();
    let signer_partial = make_u64_signer(&fixture_partial);
    let sign_partial = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        signer_partial
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let mut call_partial = |target: &str| {
        let mut access = RestrictedSourceAccess {
            authority: &mut auth_partial,
            profile: RequestProfile::Signer(&sign_partial),
            repos: &repos,
            handle_key: &handle_key,
            now_unix_s: VERIFY_AT_S,
        };
        browser.restricted_response("GET", target, &[], Role::Viewer, &mut access)
    };
    let partial_tree = call_partial("/api/sources/kiln/tree?path=src");
    assert_eq!(partial_tree.status, 200);
    let partial_json = value(&partial_tree);
    let partial_view_entries = partial_json
        .pointer("/view/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| unreachable!("partial view entries in {partial_json}"));
    let partial_f69 = partial_view_entries
        .iter()
        .find(|e| e.get("display_path").and_then(Value::as_str) == Some("src/f69.rs"))
        .unwrap_or_else(|| unreachable!("f69 in {partial_json}"));
    assert_eq!(
        partial_f69.get("readable").and_then(Value::as_bool),
        Some(true)
    );
    let partial_f69_handle = partial_f69
        .get("handle")
        .and_then(Value::as_str)
        .unwrap_or_else(|| unreachable!("f69 handle"))
        .to_owned();
    let partial_f01 = partial_view_entries
        .iter()
        .find(|e| e.get("display_path").and_then(Value::as_str) == Some("src/f01.rs"))
        .unwrap_or_else(|| unreachable!("f01 in {partial_json}"));
    assert_eq!(
        partial_f01.get("readable").and_then(Value::as_bool),
        Some(false)
    );
    let partial_f01_handle = partial_f01
        .get("handle")
        .and_then(Value::as_str)
        .unwrap_or_else(|| unreachable!("f01 handle"))
        .to_owned();
    let f69_ok = call_partial(&format!(
        "/api/sources/kiln/file?handle={partial_f69_handle}"
    ));
    assert_eq!(f69_ok.status, 200);
    assert_eq!(
        value(&f69_ok).get("text").and_then(Value::as_str),
        Some("pub const F69: usize = 69;\n")
    );
    let f01_denied = call_partial(&format!(
        "/api/sources/kiln/file?handle={partial_f01_handle}"
    ));
    assert_eq!(f01_denied.status, 404);
    assert_eq!(value(&f01_denied), json!({"error": "unavailable"}));

    // Part B: LocalSourceAuthority collapses Discover + ReadContent to <= 3 admit calls.
    struct AdmitSpy<A> {
        inner: A,
        admit_calls: usize,
    }
    impl<A: SourceAuthority> SourceAuthority for AdmitSpy<A> {
        fn admit(
            &mut self,
            request: &SourceAuthorityRequest,
            profile: &AuthorityProfile,
            current: &SourceCurrentness,
            now_unix_s: u64,
        ) -> Result<SourceAuthorityDecision, SourceAuthorityDecision> {
            self.admit_calls = self.admit_calls.saturating_add(1);
            self.inner.admit(request, profile, current, now_unix_s)
        }
        fn revalidate(
            &mut self,
            session: AuthoritySessionId,
            request: &SourceAuthorityRequest,
            current: &SourceCurrentness,
            now_unix_s: u64,
        ) -> Result<(), SourceAuthorityDecision> {
            self.inner.revalidate(session, request, current, now_unix_s)
        }
        fn close(&mut self, session: AuthoritySessionId) {
            self.inner.close(session);
        }
        fn effective_scope(&self, session: AuthoritySessionId) -> Option<&EffectiveSourceScope> {
            self.inner.effective_scope(session)
        }
        fn decisions(&self) -> &[SourceAuthorityDecision] {
            self.inner.decisions()
        }
    }

    let local_auth = LocalSourceAuthority::new(
        "operator@host",
        LocalSourceGrant {
            repositories: BTreeSet::from([adm.identity().clone()]),
            operations: BTreeSet::from([
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
            ]),
        },
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    let mut local_spy = AdmitSpy {
        inner: local_auth,
        admit_calls: 0,
    };
    let local_profile = AuthorityProfile::LocalOperator {
        operator: "operator@host".into(),
    };
    let local_tree = {
        let mut access = RestrictedSourceAccess {
            authority: &mut local_spy,
            profile: RequestProfile::Static(&local_profile),
            repos: &repos,
            handle_key: &handle_key,
            now_unix_s: VERIFY_AT_S,
        };
        browser.restricted_response(
            "GET",
            "/api/sources/kiln/tree?path=src",
            &[],
            Role::Viewer,
            &mut access,
        )
    };
    assert_eq!(local_tree.status, 200);
    assert!(
        local_spy.admit_calls <= 3,
        "expected LocalSourceAuthority subtree collapse (<= 3 admit calls), got {}",
        local_spy.admit_calls
    );
    let local_tree_json = value(&local_tree);
    let local_f69_handle = local_tree_json
        .pointer("/view/entries")
        .and_then(Value::as_array)
        .and_then(|arr| {
            arr.iter()
                .find(|e| e.get("display_path").and_then(Value::as_str) == Some("src/f69.rs"))
        })
        .and_then(|e| {
            assert_eq!(e.get("readable").and_then(Value::as_bool), Some(true));
            e.get("handle").and_then(Value::as_str)
        })
        .unwrap_or_else(|| unreachable!("local f69 handle in {local_tree_json}"))
        .to_owned();
    let local_file = {
        let mut access = RestrictedSourceAccess {
            authority: &mut local_spy,
            profile: RequestProfile::Static(&local_profile),
            repos: &repos,
            handle_key: &handle_key,
            now_unix_s: VERIFY_AT_S,
        };
        browser.restricted_response(
            "GET",
            &format!("/api/sources/kiln/file?handle={local_f69_handle}"),
            &[],
            Role::Viewer,
            &mut access,
        )
    };
    assert_eq!(local_file.status, 200);
    assert_eq!(
        value(&local_file).get("text").and_then(Value::as_str),
        Some("pub const F69: usize = 69;\n")
    );
}

#[test]
fn restricted_tree_hides_excluded_directories_uniformly_with_aliased_mounts() {
    use kiln_adapter_basalt::test_support::{
        COORDINATOR_SEED, EXPIRES_S, GENERATION, SourceFixture, VERIFY_AT_S,
    };
    use kiln_core::implementation::authority::{AttemptCurrentness, AuthorityProfile};
    use kiln_core::projection::authority::{
        PathScope, SnapshotObservation, SourceAuthorityRequest, SourceCurrentness,
        SourceViewOperation,
    };
    use kiln_core::projection::disclosure::HandleKey;
    use kiln_core::projection::{
        ComponentVersion, Coverage, EntryKind, GeneratedInputs, Mount, ProjectionDefinition,
        ProjectionFacts, ProjectionProfile, ProviderBinding, RepositoryEnrollment,
        RepositoryLocator, Selection, SourceCapability, SourceEntry, TargetProfile,
        admit_definition, admit_repositories,
    };
    use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};
    use kiln_web_service::sources::{RestrictedSourceAccess, RestrictedSourceRepo};
    use kiln_web_service::worker::source_auth::RequestProfile;

    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let work = root.path().join("work");
    let repo =
        Repository::init(&work).unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let rev = commit_nested(
        &repo,
        &[
            ("docs/a.md", "doc a\n"),
            ("src/public/mod.rs", "pub fn ok() {}\n"),
            ("src/private/secret.rs", "const S: u8 = 1;\n"),
            ("src/excluded/hidden.rs", "const H: u8 = 2;\n"),
            ("mounted/src/shadow.rs", "const M: u8 = 3;\n"),
        ],
        "excluded-dir-oracle",
        None,
    );
    let browser = Browser::restricted(Some(Profile {
        schema: "kiln.web-sources.v1".into(),
        sources: vec![policy("kiln", "git", &work)],
    }))
    .unwrap_or_else(|error| unreachable!("restricted browser: {error:?}"));
    let adm = admit_repositories(
        &[RepositoryEnrollment {
            repository_id: "kiln".into(),
            binding: ProviderBinding {
                source_provider: ProviderProfileId::new("git-source-v1")
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
                native_repository: ExternalRef::new("git", "repository", "kiln")
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
            },
            locators: vec![RepositoryLocator::LocalPath {
                path: work.to_string_lossy().into_owned(),
            }],
        }],
        &[],
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"))
    .pop()
    .unwrap_or_else(|| unreachable!("admitted repo"));
    let def = admit_definition(&ProjectionDefinition {
        schema: kiln_core::projection::DEFINITION_SCHEMA.into(),
        id: "exclude-aliased".into(),
        profile: ProjectionProfile::Runnable,
        selection: Selection {
            root: String::new(),
            include: vec!["docs/**".into(), "src/**".into()],
            exclude: vec!["src/excluded/**".into(), "mounted/**".into()],
        },
        mounts: vec![
            Mount {
                source_prefix: "docs".into(),
                destination_prefix: "mounted/docs".into(),
            },
            Mount {
                source_prefix: "src".into(),
                destination_prefix: "mounted/src".into(),
            },
            Mount {
                source_prefix: "src/public".into(),
                destination_prefix: "alias/public".into(),
            },
        ],
        dependencies: Vec::new(),
        creation_destinations: Vec::new(),
        required_capabilities: vec![SourceCapability::ExactTree],
        generated_inputs: GeneratedInputs::Refused,
        target: TargetProfile::POSIX,
        compatibility: None,
    })
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    let ext_rev = ExternalRef::new("git", "commit", &rev.to_string())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let blob_ref = |bytes: &[u8]| {
        let oid = Oid::hash_object(git2::ObjectType::Blob, bytes)
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        ExternalRef::new("git", "blob", &oid.to_string()).unwrap_or_else(|e| unreachable!("{e:?}"))
    };
    let component = |name: &str| ComponentVersion {
        name: name.into(),
        version: "1".into(),
    };
    let tree_id = ContentId::from_blake3_hex(blake3::hash(b"t-excl").to_hex().to_string())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let facts = ProjectionFacts {
        repository: adm.identity().clone(),
        exact_revision: ext_rev.clone(),
        canonical_tree: tree_id,
        evaluator: component("kiln-projection"),
        adapter: component("git-source"),
        source_capabilities: vec![SourceCapability::ExactTree],
        coverage: Coverage::Complete,
        entries: vec![
            SourceEntry {
                path: "docs/a.md".into(),
                object: blob_ref(b"doc a\n"),
                kind: EntryKind::File,
            },
            SourceEntry {
                path: "src/public/mod.rs".into(),
                object: blob_ref(b"pub fn ok() {}\n"),
                kind: EntryKind::File,
            },
            SourceEntry {
                path: "src/private/secret.rs".into(),
                object: blob_ref(b"const S: u8 = 1;\n"),
                kind: EntryKind::File,
            },
            SourceEntry {
                path: "src/excluded/hidden.rs".into(),
                object: blob_ref(b"const H: u8 = 2;\n"),
                kind: EntryKind::File,
            },
            SourceEntry {
                path: "mounted/src/shadow.rs".into(),
                object: blob_ref(b"const M: u8 = 3;\n"),
                kind: EntryKind::File,
            },
        ],
        dependencies: Vec::new(),
        generated: Vec::new(),
    };
    let cur = SourceCurrentness {
        generation: GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: ext_rev,
            ref_name: None,
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };
    let handle_key = HandleKey::from_bytes([0x42; 32]);
    let repos = [RestrictedSourceRepo {
        policy_id: "kiln",
        admitted: &adm,
        definition: &def,
        facts: &facts,
        current: &cur,
    }];

    // 1. Root Discover + ReadContent holder.
    let fixture_root = SourceFixture::direct(
        COORDINATOR_SEED,
        adm.identity(),
        None,
        &[(
            PathScope::root(),
            vec![
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
            ],
        )],
        EXPIRES_S,
    );
    let mut auth_root = fixture_root.authority();
    let signer_root = fixture_root.signer();
    let sign_root = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        signer_root
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let mut call_root = |target: &str| {
        let mut access = RestrictedSourceAccess {
            authority: &mut auth_root,
            profile: RequestProfile::Signer(&sign_root),
            repos: &repos,
            handle_key: &handle_key,
            now_unix_s: VERIFY_AT_S,
        };
        browser.restricted_response("GET", target, &[], Role::Viewer, &mut access)
    };

    let excl = call_root("/api/sources/kiln/tree?path=src/excluded");
    let nonex = call_root("/api/sources/kiln/tree?path=src/nonexistent");
    assert_eq!(excl.status, 404);
    assert_eq!(value(&excl), json!({"error": "unavailable"}));
    assert_eq!(nonex.status, 404);
    assert_eq!(excl.body, nonex.body);

    let mounted = call_root("/api/sources/kiln/tree?path=mounted/src");
    assert_eq!(mounted.status, 404);
    assert_eq!(value(&mounted), json!({"error": "unavailable"}));

    let src = call_root("/api/sources/kiln/tree?path=src");
    assert_eq!(src.status, 200);
    let src_json = value(&src);
    let src_paths: Vec<&str> = src_json
        .get("entries")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|e| e.get("path").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    assert!(!src_paths.contains(&"src/excluded"), "{src_paths:?}");
    assert!(src_paths.contains(&"src/public"), "{src_paths:?}");

    let pub_tree = call_root("/api/sources/kiln/tree?path=src/public");
    assert_eq!(pub_tree.status, 200);
    let pub_json = value(&pub_tree);
    assert_eq!(
        pub_json.pointer("/entries/0/path").and_then(Value::as_str),
        Some("src/public/mod.rs")
    );
    let view_entries = pub_json
        .pointer("/view/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| unreachable!("view entries in {pub_json}"));
    let h_mounted = view_entries
        .iter()
        .find(|e| {
            e.get("display_path").and_then(Value::as_str) == Some("mounted/src/public/mod.rs")
        })
        .and_then(|e| e.get("handle").and_then(Value::as_str))
        .unwrap_or_else(|| unreachable!("mounted handle in {pub_json}"))
        .to_owned();
    let h_alias = view_entries
        .iter()
        .find(|e| e.get("display_path").and_then(Value::as_str) == Some("alias/public/mod.rs"))
        .and_then(|e| e.get("handle").and_then(Value::as_str))
        .unwrap_or_else(|| unreachable!("alias handle in {pub_json}"))
        .to_owned();
    for h in [h_mounted, h_alias] {
        let f = call_root(&format!("/api/sources/kiln/file?handle={h}"));
        assert_eq!(f.status, 200);
        assert_eq!(
            value(&f).get("text").and_then(Value::as_str),
            Some("pub fn ok() {}\n")
        );
    }

    // 2. Narrow sub-ceiling holder (docs/** and src/public/**).
    let fixture_narrow = SourceFixture::direct(
        COORDINATOR_SEED,
        adm.identity(),
        None,
        &[
            (
                PathScope::from_path("docs", true).unwrap_or_else(|| unreachable!("docs")),
                vec![
                    SourceViewOperation::Discover,
                    SourceViewOperation::ReadContent,
                ],
            ),
            (
                PathScope::from_path("src/public", true)
                    .unwrap_or_else(|| unreachable!("src/public")),
                vec![
                    SourceViewOperation::Discover,
                    SourceViewOperation::ReadContent,
                ],
            ),
        ],
        EXPIRES_S,
    );
    let mut auth_narrow = fixture_narrow.authority();
    let signer_narrow = fixture_narrow.signer();
    let sign_narrow = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        signer_narrow
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let mut call_narrow = |target: &str| {
        let mut access = RestrictedSourceAccess {
            authority: &mut auth_narrow,
            profile: RequestProfile::Signer(&sign_narrow),
            repos: &repos,
            handle_key: &handle_key,
            now_unix_s: VERIFY_AT_S,
        };
        browser.restricted_response("GET", target, &[], Role::Viewer, &mut access)
    };

    let narrow_src = call_narrow("/api/sources/kiln/tree?path=src");
    assert_eq!(narrow_src.status, 200);
    let narrow_src_json = value(&narrow_src);
    let narrow_src_paths: Vec<&str> = narrow_src_json
        .get("entries")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|e| e.get("path").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(narrow_src_paths, vec!["src/public"]);
    assert_eq!(
        call_narrow("/api/sources/kiln/tree?path=src/public").status,
        200
    );
    for hidden_path in ["src/private", "src/excluded", "src/nonexistent"] {
        let resp = call_narrow(&format!("/api/sources/kiln/tree?path={hidden_path}"));
        assert_eq!(resp.status, 404, "{hidden_path}");
        assert_eq!(value(&resp), json!({"error": "unavailable"}));
    }

    // 3. Excluded-only narrow holder vs nonexistent-file narrow holder must match.
    let call_only = |scope_path: &str| {
        let fixture = SourceFixture::direct(
            COORDINATOR_SEED,
            adm.identity(),
            None,
            &[(
                PathScope::from_path(scope_path, false).unwrap_or_else(|| unreachable!("path")),
                vec![SourceViewOperation::Discover],
            )],
            EXPIRES_S,
        );
        let mut auth = fixture.authority();
        let signer = fixture.signer();
        let sign = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
            signer
                .sign(req)
                .map(AuthorityProfile::VerifiedUcan)
                .map_err(|e| e.message)
        };
        let mut access = RestrictedSourceAccess {
            authority: &mut auth,
            profile: RequestProfile::Signer(&sign),
            repos: &repos,
            handle_key: &handle_key,
            now_unix_s: VERIFY_AT_S,
        };
        let inv_resp =
            browser.restricted_response("GET", "/api/sources", &[], Role::Viewer, &mut access);
        let tree_resp = browser.restricted_response(
            "GET",
            "/api/sources/kiln/tree",
            &[],
            Role::Viewer,
            &mut access,
        );
        (inv_resp, tree_resp)
    };
    let (excl_inv, excl_tree) = call_only("src/excluded/hidden.rs");
    let (miss_inv, miss_tree) = call_only("src/nonexistent/file.rs");
    assert_eq!(excl_inv.status, 200);
    assert_eq!(value(&excl_inv).get("sources"), Some(&json!([])));
    assert_eq!(excl_inv.body, miss_inv.body);
    assert_eq!(excl_tree.status, 404);
    assert_eq!(value(&excl_tree), json!({"error": "unavailable"}));
    assert_eq!(excl_tree.body, miss_tree.body);
}

#[test]
fn restricted_search_download_and_tree_redaction_close_s5_oracles_and_cursor_replay() {
    use kiln_adapter_basalt::test_support::{
        COORDINATOR_SEED, EXPIRES_S, GENERATION, SourceFixture, VERIFY_AT_S,
    };
    use kiln_core::implementation::authority::{AttemptCurrentness, AuthorityProfile};
    use kiln_core::projection::authority::{
        PathScope, SnapshotObservation, SourceAuthorityRequest, SourceCurrentness,
        SourceViewOperation,
    };
    use kiln_core::projection::disclosure::HandleKey;
    use kiln_core::projection::{
        ComponentVersion, Coverage, EntryKind, GeneratedInputs, Mount, ProjectionDefinition,
        ProjectionFacts, ProjectionProfile, ProviderBinding, RepositoryEnrollment,
        RepositoryLocator, Selection, SourceCapability, SourceEntry, TargetProfile,
        admit_definition, admit_repositories,
    };
    use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};
    use kiln_web_service::sources::{RestrictedSourceAccess, RestrictedSourceRepo};
    use kiln_web_service::worker::source_auth::RequestProfile;

    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let work = root.path().join("work");
    let repo =
        Repository::init(&work).unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));

    let mut files: Vec<(String, String)> = vec![
        (
            "src/public/alpha.rs".into(),
            "pub fn needle_alpha() -> u32 { 1 }\n".into(),
        ),
        (
            "src/public/beta.rs".into(),
            "pub fn needle_beta() -> u32 { 2 }\n".into(),
        ),
        (
            "src/discover_only/namesake.rs".into(),
            "pub fn secret_token_in_discover_only() {}\n".into(),
        ),
        (
            "src/private/secret.rs".into(),
            "pub fn needle_hidden_secret() {}\n".into(),
        ),
    ];
    // Add > MAX_TREE_ENTRIES (512) hidden files in `src/hidden_bulk/` and root-sibling
    // entries to verify hidden sibling counts never trigger `source_tree_bound` in restricted mode.
    for i in 0..520_usize {
        files.push((
            format!("hidden_bulk_{i:04}.txt"),
            format!("hidden needle_{i}\n"),
        ));
    }
    let file_refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(p, c)| (p.as_str(), c.as_str()))
        .collect();
    let rev = commit_nested(&repo, &file_refs, "s5-search-download", None);

    let browser = Browser::restricted(Some(Profile {
        schema: "kiln.web-sources.v1".into(),
        sources: vec![policy("kiln", "git", &work)],
    }))
    .unwrap_or_else(|error| unreachable!("restricted browser: {error:?}"));
    let adm = admit_repositories(
        &[RepositoryEnrollment {
            repository_id: "kiln".into(),
            binding: ProviderBinding {
                source_provider: ProviderProfileId::new("git-source-v1")
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
                native_repository: ExternalRef::new("git", "repository", "kiln")
                    .unwrap_or_else(|e| unreachable!("{e:?}")),
            },
            locators: vec![RepositoryLocator::LocalPath {
                path: work.to_string_lossy().into_owned(),
            }],
        }],
        &[],
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"))
    .pop()
    .unwrap_or_else(|| unreachable!("admitted repo"));

    let def = admit_definition(&ProjectionDefinition {
        schema: kiln_core::projection::DEFINITION_SCHEMA.into(),
        id: "s5-def".into(),
        profile: ProjectionProfile::Runnable,
        selection: Selection {
            root: String::new(),
            include: vec!["src/**".into()],
            exclude: vec!["src/private/**".into()],
        },
        mounts: vec![Mount {
            source_prefix: String::new(),
            destination_prefix: String::new(),
        }],
        dependencies: Vec::new(),
        creation_destinations: Vec::new(),
        required_capabilities: vec![SourceCapability::ExactTree],
        generated_inputs: GeneratedInputs::Refused,
        target: TargetProfile::POSIX,
        compatibility: None,
    })
    .unwrap_or_else(|e| unreachable!("{e:?}"));

    let ext_rev = ExternalRef::new("git", "commit", &rev.to_string())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let blob_ref = |bytes: &[u8]| {
        let oid = Oid::hash_object(git2::ObjectType::Blob, bytes)
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        ExternalRef::new("git", "blob", &oid.to_string()).unwrap_or_else(|e| unreachable!("{e:?}"))
    };
    let component = |name: &str| ComponentVersion {
        name: name.into(),
        version: "1".into(),
    };
    let tree_id = ContentId::from_blake3_hex(blake3::hash(b"t-s5").to_hex().to_string())
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    let entries: Vec<SourceEntry> = files
        .iter()
        .map(|(p, c)| SourceEntry {
            path: p.clone(),
            object: blob_ref(c.as_bytes()),
            kind: EntryKind::File,
        })
        .collect();
    let facts = ProjectionFacts {
        repository: adm.identity().clone(),
        exact_revision: ext_rev.clone(),
        canonical_tree: tree_id,
        evaluator: component("kiln-projection"),
        adapter: component("git-source"),
        source_capabilities: vec![SourceCapability::ExactTree],
        coverage: Coverage::Complete,
        entries,
        dependencies: Vec::new(),
        generated: Vec::new(),
    };
    let cur = SourceCurrentness {
        generation: GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: ext_rev,
            ref_name: None,
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };
    let handle_key = HandleKey::from_bytes([0x55; 32]);
    let repos = [RestrictedSourceRepo {
        policy_id: "kiln",
        admitted: &adm,
        definition: &def,
        facts: &facts,
        current: &cur,
    }];

    let broad_fix = SourceFixture::direct(
        COORDINATOR_SEED,
        adm.identity(),
        None,
        &[
            (
                PathScope::from_path("src/public", true).unwrap_or_else(|| unreachable!("path")),
                vec![
                    SourceViewOperation::Discover,
                    SourceViewOperation::ReadContent,
                ],
            ),
            (
                PathScope::from_path("src/discover_only", true)
                    .unwrap_or_else(|| unreachable!("path")),
                vec![SourceViewOperation::Discover],
            ),
        ],
        EXPIRES_S,
    );
    let mut auth_broad = broad_fix.authority();
    let signer_broad = broad_fix.signer();
    let sign_broad = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        signer_broad
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let mut call_broad = |target: &str| {
        let mut access = RestrictedSourceAccess {
            authority: &mut auth_broad,
            profile: RequestProfile::Signer(&sign_broad),
            repos: &repos,
            handle_key: &handle_key,
            now_unix_s: VERIFY_AT_S,
        };
        browser.restricted_response("GET", target, &[], Role::Viewer, &mut access)
    };

    // 1. Restricted tree succeeds despite >512 hidden sibling files and redacts dir oid and discover-only oid/bytes.
    let root_tree = call_broad("/api/sources/kiln/tree");
    assert_eq!(root_tree.status, 200);
    let root_json = value(&root_tree);
    let root_entries = root_json
        .get("entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| unreachable!("entries"));
    assert_eq!(root_entries.len(), 1);
    assert_eq!(
        root_entries[0].get("path").and_then(Value::as_str),
        Some("src")
    );
    assert_eq!(root_entries[0].get("oid"), Some(&Value::Null));

    let disc_tree = call_broad("/api/sources/kiln/tree?path=src/discover_only");
    assert_eq!(disc_tree.status, 200);
    let disc_json = value(&disc_tree);
    let disc_entry = &disc_json["entries"][0];
    assert_eq!(
        disc_entry.get("path").and_then(Value::as_str),
        Some("src/discover_only/namesake.rs")
    );
    assert_eq!(disc_entry.get("oid"), Some(&Value::Null));
    assert_eq!(disc_entry.get("bytes"), Some(&Value::Null));

    // 2. Restricted search returns only authorized matches, omits hidden files, and never searches content of discover-only files.
    let search_secret = call_broad("/api/sources/kiln/search?q=secret_token_in_discover_only");
    assert_eq!(search_secret.status, 200);
    let search_secret_json = value(&search_secret);
    assert_eq!(
        search_secret_json.get("total_matches").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        search_secret_json
            .get("matches")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );

    let search_p1 = call_broad("/api/sources/kiln/search?q=needle&limit=1");
    assert_eq!(search_p1.status, 200);
    let p1_json = value(&search_p1);
    assert_eq!(p1_json.get("total_matches").and_then(Value::as_u64), Some(2));
    let cursor = p1_json
        .get("next_cursor")
        .and_then(Value::as_str)
        .unwrap_or_else(|| unreachable!("next_cursor in {p1_json}"))
        .to_owned();
    assert_eq!(
        p1_json["matches"][0].get("path").and_then(Value::as_str),
        Some("src/public/alpha.rs")
    );

    let search_p2 = call_broad(&format!("/api/sources/kiln/search?q=needle&limit=1&cursor={cursor}"));
    assert_eq!(search_p2.status, 200);
    let p2_json = value(&search_p2);
    assert_eq!(
        p2_json["matches"][0].get("path").and_then(Value::as_str),
        Some("src/public/beta.rs")
    );
    assert_eq!(p2_json.get("next_cursor"), Some(&Value::Null));

    // Changing query or replaying cursor under a narrower scope MUST fail closed with 404 unavailable.
    let wrong_query = call_broad(&format!("/api/sources/kiln/search?q=other&cursor={cursor}"));
    assert_eq!(wrong_query.status, 404);
    assert_eq!(value(&wrong_query), json!({"error": "unavailable"}));

    let narrow_fix = SourceFixture::direct(
        COORDINATOR_SEED,
        adm.identity(),
        None,
        &[(
            PathScope::from_path("src/public/alpha.rs", false)
                .unwrap_or_else(|| unreachable!("path")),
            vec![
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
            ],
        )],
        EXPIRES_S,
    );
    let mut auth_narrow = narrow_fix.authority();
    let signer_narrow = narrow_fix.signer();
    let sign_narrow = |req: &SourceAuthorityRequest| -> Result<AuthorityProfile, String> {
        signer_narrow
            .sign(req)
            .map(AuthorityProfile::VerifiedUcan)
            .map_err(|e| e.message)
    };
    let mut call_narrow = |target: &str| {
        let mut access = RestrictedSourceAccess {
            authority: &mut auth_narrow,
            profile: RequestProfile::Signer(&sign_narrow),
            repos: &repos,
            handle_key: &handle_key,
            now_unix_s: VERIFY_AT_S,
        };
        browser.restricted_response("GET", target, &[], Role::Viewer, &mut access)
    };
    let replay_cursor = call_narrow(&format!("/api/sources/kiln/search?q=needle&cursor={cursor}"));
    assert_eq!(replay_cursor.status, 404);
    assert_eq!(value(&replay_cursor), json!({"error": "unavailable"}));

    // Hidden vs nonexistent repo search oracle check
    let hidden_search = call_broad("/api/sources/hidden_repo/search?q=needle");
    let bad_q_hidden = call_broad("/api/sources/hidden_repo/search?q=");
    let bad_q_real = call_broad("/api/sources/kiln/search?q=");
    assert_eq!(hidden_search.status, 404);
    assert_eq!(bad_q_hidden.status, 404);
    assert_eq!(bad_q_real.status, 404);
    assert_eq!(hidden_search.body, bad_q_hidden.body);

    // 3. Restricted download succeeds for authorized handle/path, refuses discover-only, hidden, and path escape.
    let alpha_handle = p1_json["matches"][0]
        .get("handle")
        .and_then(Value::as_str)
        .unwrap_or_else(|| unreachable!("alpha handle"));
    let dl_handle = call_broad(&format!("/api/sources/kiln/download?handle={alpha_handle}"));
    assert_eq!(dl_handle.status, 200);
    assert_eq!(dl_handle.content_type, "application/octet-stream");
    assert_eq!(dl_handle.download_name.as_deref(), Some("alpha.rs"));
    assert_eq!(dl_handle.body, b"pub fn needle_alpha() -> u32 { 1 }\n");

    let dl_path = call_broad("/api/sources/kiln/download?path=src/public/alpha.rs");
    assert_eq!(dl_path.status, 200);
    assert_eq!(dl_path.body, b"pub fn needle_alpha() -> u32 { 1 }\n");

    for denied in [
        "/api/sources/kiln/download?path=src/discover_only/namesake.rs",
        "/api/sources/kiln/download?path=src/private/secret.rs",
        "/api/sources/kiln/download?path=../../etc/passwd",
        "/api/sources/kiln/download?path=src/missing.rs",
        "/api/sources/hidden/download?path=src/public/alpha.rs",
    ] {
        let resp = call_broad(denied);
        assert_eq!(resp.status, 404, "{denied}");
        assert_eq!(value(&resp), json!({"error": "unavailable"}), "{denied}");
    }
}
