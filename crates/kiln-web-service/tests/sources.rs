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

// r[verify kiln.dashboard.source.read.scenario.jj]
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

// r[verify kiln.dashboard.source.read.scenario.lookup]
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
