//! Monorepo shell: scope admission, exact changed files and affected-only dispatch
//! over a disposable Git repository with a Cargo workspace.
use super::*;
use hmac::{Hmac, Mac};
use kiln_adapter_github::events::DeliveryHeaders;
use kiln_core::monorepo::{ChangedFallback, ChangedFiles};
use kiln_web_service::automation::AutomationProfile;
use kiln_web_service::monorepo::{ChangedBounds, changed_files};
use sha2::Sha256;
use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

const SECRET: &[u8] = b"private-monorepo-test-secret";
const CRATES: [&str; 3] = ["a", "b", "c"];

fn commit_files(repository: &git2::Repository, files: &[(&str, &str)], message: &str) -> String {
    let root = checked(repository.workdir().ok_or("workdir"));
    for (path, content) in files {
        let path = root.join(path);
        checked(std::fs::create_dir_all(checked(
            path.parent().ok_or("parent"),
        )));
        checked(std::fs::write(path, content));
    }
    let mut index = checked(repository.index());
    checked(index.add_all(["*"], git2::IndexAddOption::DEFAULT, None));
    checked(index.update_all(["*"], None));
    checked(index.write());
    let tree = checked(repository.find_tree(checked(index.write_tree())));
    let signature = checked(git2::Signature::now("test", "test@example.invalid"));
    let parent = repository
        .find_reference("refs/heads/main")
        .ok()
        .and_then(|r| r.peel_to_commit().ok());
    let parents: Vec<_> = parent.iter().collect();
    checked(repository.commit(
        Some("refs/heads/main"),
        &signature,
        &signature,
        message,
        &tree,
        &parents,
    ))
    .to_string()
}

/// Workspace with crates a, b (path dependency on a) and unrelated c.
fn workspace_repository(root: &Path) -> (git2::Repository, String) {
    let path = root.join("monorepo");
    let repository = checked(git2::Repository::init(&path));
    let package = |name: &str, dependencies: &str| {
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n{dependencies}"
        )
    };
    let b = package("b", "\n[dependencies]\na = { path = \"../a\" }\n");
    let revision = commit_files(
        &repository,
        &[
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"2\"\n",
            ),
            ("crates/a/Cargo.toml", &package("a", "")),
            ("crates/a/src/lib.rs", "pub fn a() {}\n"),
            ("crates/b/Cargo.toml", &b),
            ("crates/b/src/lib.rs", "pub fn b() { a::a() }\n"),
            ("crates/c/Cargo.toml", &package("c", "")),
            ("crates/c/src/lib.rs", "pub fn c() {}\n"),
        ],
        "workspace",
    );
    (repository, revision)
}

fn project(root: &Path, repository: &Path, id: &str, scope: Value) -> Value {
    let mut project = json!({"id":id,"label":format!("Project {id}"),"repository_path":repository,"reference":"refs/heads/main",
        "source_limits":{"max_entries":64,"max_path_bytes":1024,"max_file_bytes":4096,"max_total_bytes":65536},
        "workflow":{"socket_path":root.join(format!("{id}.sock")),"allowed_user_ids":[nix::unistd::geteuid().as_raw()],
            "transport_limits":{"maximum_frame_bytes":1048576,"maximum_log_bytes":65536,"maximum_connections":4,"read_deadline_milliseconds":1000,"write_deadline_milliseconds":1000},
            "adapter_limits":{"maximum_request_bytes":1048576,"maximum_response_bytes":1048576},
            "policy_id":"reviewed-policy","provider_profile_id":"reviewed-lattice","platform":"linux-x86_64",
            "nodes":[{"id":"job","kind":"test","effect_class":"test","adapter_kind":"local","operation_identity":"a".repeat(64)}],
            "edges":[],"controls":{"cancel":true,"pause":false,"resume":false},
            "bounds":{"max_nodes":8,"max_edges":16,"max_log_references":32,"max_log_read_bytes":4096,"max_total_log_bytes":65536}}});
    if !scope.is_null() {
        set(&mut project, "scope", scope);
    }
    project
}

fn service_value(root: &Path, projects: Vec<Value>) -> Value {
    json!({"schema":"kiln.web-service.v1","service_id":"monorepo-tests","listen_address":"127.0.0.1:9443","origin":"https://localhost:9443",
        "tls":{"certificate_path":root.join("cert.pem"),"private_key_path":root.join("key.pem"),"min_version":"TLS1.3"},
        "operator_token_file":root.join("operator.token"),"worker_token_file":root.join("worker.token"),"state_directory":root.join("state"),
        "projects":projects,
        "bounds":{"max_request_bytes":16384,"max_response_bytes":1048576,"max_header_bytes":4096,"max_connections":4,"timeout_seconds":2,"poll_interval_ms":10,
            "max_projects":8,"max_runs":32,"max_artifact_bytes":1024,"max_artifacts":2,"max_run_artifact_bytes":2048,"max_total_artifact_bytes":4096,
            "max_log_read_bytes":4096,"max_requests":64,"max_events_per_run":8,"max_state_bytes":4194304}})
}

fn crate_scope(name: &str) -> Value {
    json!({"root":format!("crates/{name}"),"graphs":["cargo"]})
}

/// Three Cargo-scoped crate projects plus one unscoped project on one repository.
fn monorepo_profile(root: &Path, repository: &Path) -> ServiceProfile {
    let mut projects: Vec<Value> = CRATES
        .iter()
        .map(|name| project(root, repository, name, crate_scope(name)))
        .collect();
    projects.push(project(root, repository, "whole", Value::Null));
    checked(serde_json::from_value(service_value(root, projects)))
}

fn open_projects(profile: ServiceProfile) -> (Engine, BTreeMap<String, Arc<Mutex<Backend>>>) {
    let mut ports: BTreeMap<String, Box<dyn WorkflowPort + Send>> = BTreeMap::new();
    let mut backends = BTreeMap::new();
    for project in &profile.projects {
        let backend = Arc::new(Mutex::new(Backend {
            terminal: true,
            ..Backend::default()
        }));
        backends.insert(project.id.clone(), backend.clone());
        ports.insert(
            project.id.clone(),
            Box::new(Port {
                profile: checked(project.lattice(profile.bounds.max_runs)).provider,
                backend,
            }),
        );
    }
    (checked(Engine::open_with_ports(profile, ports)), backends)
}

fn automation(root: &Path, projects: &[&str]) -> AutomationProfile {
    let secret = root.join("github.secret");
    checked(std::fs::write(&secret, SECRET));
    checked(std::fs::set_permissions(
        &secret,
        std::fs::Permissions::from_mode(0o600),
    ));
    let triggers: Vec<Value> = projects
        .iter()
        .map(|id| json!({"id":format!("push-{id}"),"project_id":id,"reference":"refs/heads/main",
            "repository":{"owner":"fixture","repository":"monorepo","repository_id":7,"allow_fork_pull_requests":false},
            "secret_file":secret,"enabled":true,"push":true,"pull_request":false,"pull_request_base":null}))
        .collect();
    checked(serde_json::from_value(
        json!({"schema":"kiln.web-automation.v1","max_records":100,"status_limit":20,"max_event_bytes":65536,"max_tick_effects":2,
        "triggers":triggers,"schedules":[],"notifications":[]}),
    ))
}

fn push_event(delivery: &str, revision: &str) -> (DeliveryHeaders, Vec<u8>) {
    let body = checked(serde_json::to_vec(&json!({
        "repository":{"id":7,"name":"monorepo","full_name":"fixture/monorepo","owner":{"login":"fixture"}},
        "ref":"refs/heads/main","after":revision,"deleted":false})));
    let mut mac = checked(Hmac::<Sha256>::new_from_slice(SECRET));
    mac.update(&body);
    let signature: String = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    (
        DeliveryHeaders {
            delivery: delivery.into(),
            event: "push".into(),
            signature: format!("sha256={signature}"),
        },
        body,
    )
}

fn enroll_project(engine: &mut Engine, id: &str) {
    assert_eq!(
        call(
            engine,
            "POST",
            "/api/projects",
            json!({"project_id":id,"request_id":format!("enroll-{id}")}),
            Role::Operator
        )
        .status,
        200
    );
}

fn sources_by_project(engine: &mut Engine) -> BTreeMap<String, Vec<Value>> {
    let view = value(&get(engine, "/api/automation", Role::Operator));
    let mut sources: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for source in checked(field(&view, "sources").as_array().ok_or("sources")) {
        sources
            .entry(checked(field(source, "project_id").as_str().ok_or("project")).to_owned())
            .or_default()
            .push(source.clone());
    }
    sources
}

fn run_view(engine: &mut Engine, id: &str) -> Value {
    value(&get(engine, &format!("/api/runs/{id}"), Role::Operator))
}

// r[verify kiln.monorepo.scope]
// r[verify kiln.monorepo.scope.scenario.invalid]
#[test]
fn scope_admission_names_project_and_field_and_old_profiles_load() {
    let root = checked(tempfile::tempdir());
    let (repository, _) = workspace_repository(root.path());
    let path = checked(repository.workdir().ok_or("workdir")).to_path_buf();
    let profile = monorepo_profile(root.path(), &path);
    checked(profile.admit());
    let keys: BTreeSet<String> = profile
        .projects
        .iter()
        .map(|p| p.repository_key())
        .collect();
    assert_eq!(keys.len(), 1, "projects on one repository share one key");
    // An unscoped profile written before monorepo support still admits unchanged.
    let old = service_value(
        root.path(),
        vec![project(root.path(), &path, "whole", Value::Null)],
    );
    assert!(field(&old, "projects")[0].get("scope").is_none());
    let old: ServiceProfile = checked(serde_json::from_value(old));
    checked(old.admit());
    assert_eq!(
        checked(checked(old.projects.first().ok_or("project")).scope_view()),
        Value::Null
    );
    let invalid = |id: &str, scope: Value| {
        let profile: ServiceProfile = checked(serde_json::from_value(service_value(
            root.path(),
            vec![
                project(root.path(), &path, "whole", Value::Null),
                project(root.path(), &path, id, scope),
            ],
        )));
        checked(profile.admit().err().ok_or("invalid scope admitted"))
    };
    let parent = invalid("escape", json!({"root":"crates/../secret"}));
    assert!(
        parent.contains("\"escape\"") && parent.contains("scope.root"),
        "{parent}"
    );
    let absolute = invalid("absolute", json!({"root":"/etc"}));
    assert!(
        absolute.contains("\"absolute\"") && absolute.contains("scope.root"),
        "{absolute}"
    );
    let limit = kiln_core::monorepo::MAX_SCOPE_PATTERNS + 1;
    let many: Vec<String> = (0..limit).map(|i| format!("src/{i}/**")).collect();
    let include = invalid("wide", json!({"root":"crates/a","include":many}));
    assert!(
        include.contains("\"wide\"") && include.contains("scope.include"),
        "{include}"
    );
    let at_limit: Vec<String> = (0..limit - 1).map(|i| format!("src/{i}/**")).collect();
    let profile: ServiceProfile = checked(serde_json::from_value(service_value(
        root.path(),
        vec![project(
            root.path(),
            &path,
            "edge",
            json!({"root":"crates/a","include":at_limit}),
        )],
    )));
    checked(profile.admit());
    // Unknown scope fields never reach admission.
    let mut unknown = service_value(
        root.path(),
        vec![project(
            root.path(),
            &path,
            "a",
            json!({"root":"crates/a","languages":["rust"]}),
        )],
    );
    assert!(serde_json::from_value::<ServiceProfile>(unknown.clone()).is_err());
    set(
        &mut unknown,
        "projects",
        json!([project(
            root.path(),
            &path,
            "a",
            json!({"root":"crates/a","graphs":["bazel"]})
        )]),
    );
    assert!(serde_json::from_value::<ServiceProfile>(unknown).is_err());
    // One repository reached through an alias must use the canonical spelling.
    let alias = root.path().join("alias");
    checked(std::os::unix::fs::symlink(&path, &alias));
    let aliased: ServiceProfile = checked(serde_json::from_value(service_value(
        root.path(),
        vec![
            project(root.path(), &path, "a", crate_scope("a")),
            project(root.path(), &alias, "b", crate_scope("b")),
        ],
    )));
    let error = checked(aliased.admit().err().ok_or("aliased repository admitted"));
    assert!(error.contains("project b repository_path"), "{error}");
}

// r[verify kiln.monorepo.changed]
#[test]
fn changed_files_are_exact_with_renames_new_references_and_bounds() {
    let root = checked(tempfile::tempdir());
    let (repository, base) = workspace_repository(root.path());
    let workdir = checked(repository.workdir().ok_or("workdir")).to_path_buf();
    checked(std::fs::rename(
        workdir.join("crates/c/src/lib.rs"),
        workdir.join("crates/c/src/renamed.rs"),
    ));
    let renamed = commit_files(
        &repository,
        &[("crates/a/src/lib.rs", "pub fn a2() {}\n")],
        "rename",
    );
    let bounds = kiln_web_service::monorepo::CHANGED_BOUNDS;
    let ChangedFiles::Exact(paths) = changed_files(&repository, Some(&base), &renamed, bounds)
    else {
        unreachable!("exact diff expected");
    };
    let expected: BTreeSet<String> = [
        "crates/a/src/lib.rs",
        "crates/c/src/lib.rs",
        "crates/c/src/renamed.rs",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(paths, expected, "a rename contributes both paths");
    // A new reference has no previous head: explicit fallback, never a partial list.
    assert_eq!(
        changed_files(&repository, None, &renamed, bounds),
        ChangedFiles::Fallback(ChangedFallback::NoBase)
    );
    // Exactly at the path bound is exact; one more path is truncated.
    let at = ChangedBounds {
        max_paths: 3,
        max_path_bytes: usize::MAX,
    };
    assert!(matches!(
        changed_files(&repository, Some(&base), &renamed, at),
        ChangedFiles::Exact(_)
    ));
    assert_eq!(
        changed_files(
            &repository,
            Some(&base),
            &renamed,
            ChangedBounds { max_paths: 2, ..at }
        ),
        ChangedFiles::Fallback(ChangedFallback::Truncated)
    );
    let bytes: usize = expected.iter().map(String::len).sum();
    assert!(matches!(
        changed_files(
            &repository,
            Some(&base),
            &renamed,
            ChangedBounds {
                max_paths: 3,
                max_path_bytes: bytes
            }
        ),
        ChangedFiles::Exact(_)
    ));
    assert_eq!(
        changed_files(
            &repository,
            Some(&base),
            &renamed,
            ChangedBounds {
                max_paths: 3,
                max_path_bytes: bytes - 1
            }
        ),
        ChangedFiles::Fallback(ChangedFallback::Truncated)
    );
    // A base outside the object store is a read error, not an empty diff.
    assert_eq!(
        changed_files(&repository, Some(&"b".repeat(40)), &renamed, bounds),
        ChangedFiles::Fallback(ChangedFallback::ReadError)
    );
}

// r[verify kiln.monorepo.dispatch]
// r[verify kiln.monorepo.affected.scenario.graph]
// r[verify kiln.monorepo.affected.scenario.fallback]
#[test]
fn push_dispatches_only_affected_projects_and_records_skipped_ones() {
    let root = checked(tempfile::tempdir());
    let (repository, first) = workspace_repository(root.path());
    let path = checked(repository.workdir().ok_or("workdir")).to_path_buf();
    let profile = monorepo_profile(root.path(), &path);
    let (mut engine, backends) = open_projects(profile.clone());
    let ids = ["a", "b", "c", "whole"];
    for id in ids {
        enroll_project(&mut engine, id);
    }
    let policy = automation(root.path(), &ids);
    checked(engine.configure_automation(policy.clone()));
    let projects = value(&get(&mut engine, "/api/projects", Role::Operator));
    let enrolled = checked(field(&projects, "enrolled").as_array().ok_or("enrolled"));
    let key = field(
        checked(enrolled.first().ok_or("enrolled")),
        "repository_key",
    )
    .clone();
    assert!(enrolled.iter().all(|p| field(p, "repository_key") == &key));
    assert_eq!(
        projects.pointer("/enrolled/0/scope/root"),
        Some(&json!("crates/a"))
    );
    assert_eq!(
        projects.pointer("/enrolled/0/scope/graphs"),
        Some(&json!(["cargo"]))
    );
    assert_eq!(projects.pointer("/enrolled/3/scope"), Some(&Value::Null));
    let triggers = value(&get(&mut engine, "/api/automation", Role::Operator));
    assert_eq!(
        triggers.pointer("/triggers/1/scope/root"),
        Some(&json!("crates/b"))
    );

    // First push of the reference: no recorded base, so every project runs.
    let (headers, body) = push_event("00000000-0000-4000-8000-00000000a001", &first);
    assert_eq!(
        checked(engine.automation_trigger("push-a", &headers, &body)).status,
        202
    );
    for id in ids {
        assert_eq!(
            checked(backends.get(id).ok_or("backend")).lock().dispatches,
            1,
            "{id} runs on no_base"
        );
    }
    let sources = sources_by_project(&mut engine);
    let a_run = checked(
        sources
            .get("a")
            .and_then(|s| s.first())
            .and_then(|s| s.get("run_id"))
            .and_then(Value::as_str)
            .ok_or("run"),
    )
    .to_owned();
    let a = run_view(&mut engine, &a_run);
    assert_eq!(
        a.pointer("/affected/changed"),
        Some(&json!({"fallback":"no_base"}))
    );
    assert_eq!(
        a.pointer("/affected/reasons/0/kind"),
        Some(&json!("fallback"))
    );
    assert_eq!(a.pointer("/project_root"), Some(&json!("crates/a")));
    assert!(a.pointer("/scope_identity").is_some_and(Value::is_string));

    // A change only under crate a: a by path, b by graph, c skipped, whole unscoped.
    let second = commit_files(
        &repository,
        &[("crates/a/src/lib.rs", "pub fn a() { }\n")],
        "a",
    );
    let (headers, body) = push_event("00000000-0000-4000-8000-00000000a002", &second);
    let response = checked(engine.automation_trigger("push-c", &headers, &body));
    assert_eq!(response.status, 202);
    let received = value(&response);
    assert_eq!(field(&received, "status"), &json!("skipped"));
    assert_eq!(field(&received, "run_id"), &Value::Null);
    for (id, runs) in [("a", 2), ("b", 2), ("c", 1), ("whole", 2)] {
        assert_eq!(
            checked(backends.get(id).ok_or("backend")).lock().dispatches,
            runs,
            "{id} dispatch count"
        );
    }
    let sources = sources_by_project(&mut engine);
    // The automation view lists sources newest first.
    let latest = |id: &str| {
        sources
            .get(id)
            .and_then(|sources| sources.first())
            .cloned()
            .unwrap_or_else(|| unreachable!("latest source"))
    };
    let skipped = latest("c");
    assert_eq!(field(&skipped, "status"), &json!("skipped"));
    let decisions = checked(
        skipped
            .pointer("/affected/decisions")
            .and_then(Value::as_array)
            .ok_or("decisions"),
    );
    let decision = |id: &str| {
        decisions
            .iter()
            .find(|d| d.get("project_id") == Some(&json!(id)))
            .cloned()
            .unwrap_or_else(|| unreachable!("decision {id}"))
    };
    assert_eq!(
        decision("c"),
        json!({"project_id":"c","outcome":"skipped","reason":"no_changes_in_scope"})
    );
    assert_eq!(
        decision("a").pointer("/reasons/0/kind"),
        Some(&json!("path"))
    );
    assert_eq!(
        decision("whole").pointer("/reasons/0/kind"),
        Some(&json!("unscoped"))
    );
    let graph = checked(
        decision("b")
            .pointer("/reasons/0")
            .cloned()
            .ok_or("graph reason"),
    );
    assert_eq!(field(&graph, "kind"), &json!("graph"));
    assert_eq!(field(&graph, "graph"), &json!("cargo"));
    let b_run = checked(
        latest("b")
            .get("run_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or("b run"),
    );
    let b = run_view(&mut engine, &b_run);
    assert_eq!(
        b.pointer("/affected/changed"),
        Some(&json!({"exact_count":1}))
    );
    assert_eq!(b.pointer("/affected/reasons/0"), Some(&graph));
    assert_eq!(
        b.pointer("/affected/set_identity"),
        skipped.pointer("/affected/identity")
    );
    let whole_run = checked(
        latest("whole")
            .get("run_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or("run"),
    );
    assert!(
        run_view(&mut engine, &whole_run)
            .get("project_root")
            .is_none()
    );

    // Replays never dispatch again; restart keeps every retained decision.
    assert_eq!(
        checked(engine.automation_trigger("push-a", &headers, &body)).status,
        409
    );
    drop(engine);
    let (mut engine, _) = open_projects(profile);
    checked(engine.configure_automation(policy));
    assert_eq!(
        run_view(&mut engine, &b_run).pointer("/affected/reasons/0"),
        Some(&graph)
    );
    // Manual runs keep today's behaviour: scoped projects record no_base only.
    let manual = value(&call(
        &mut engine,
        "POST",
        "/api/runs",
        json!({"project_id":"c","request_id":"manual-c"}),
        Role::Operator,
    ));
    assert_eq!(
        manual.pointer("/affected/changed"),
        Some(&json!({"fallback":"no_base"}))
    );
    let unscoped = value(&call(
        &mut engine,
        "POST",
        "/api/runs",
        json!({"project_id":"whole","request_id":"manual-whole"}),
        Role::Operator,
    ));
    assert!(unscoped.get("affected").is_none() && unscoped.get("scope_identity").is_none());
}

#[test]
fn unscoped_repository_state_keeps_the_previous_record_shape() {
    let (root, profile, revision) = fixture();
    let backend = Arc::new(Mutex::new(Backend::default()));
    let mut engine = open(profile.clone(), backend.clone());
    enroll(&mut engine);
    let run = value(&start(&mut engine, "unscoped"));
    assert!(run.get("affected").is_none() && run.get("project_root").is_none());
    assert_eq!(field(&run, "source_revision"), &json!(revision));
    drop(engine);
    let connection = checked(rusqlite::Connection::open(
        profile.state_directory.join("service.sqlite"),
    ));
    let state: Vec<u8> = checked(connection.query_row(
        "SELECT state FROM service_state WHERE id=1",
        [],
        |row| row.get(0),
    ));
    drop(connection);
    let text = String::from_utf8_lossy(&state);
    for key in ["\"affected\"", "\"scope\":{\"identity\"", "\"heads\""] {
        assert!(!text.contains(key), "unscoped state gained {key}");
    }
    let mut engine = open(profile, backend);
    assert_eq!(
        value(&get(&mut engine, "/api/projects", Role::Operator)).pointer("/available/0/scope"),
        Some(&Value::Null)
    );
    drop(root);
}

/// Commit `files` over `parent`'s tree and move `reference` to the result.
fn commit_over(
    repository: &git2::Repository,
    reference: &str,
    parent: &str,
    files: &[(&str, &str)],
) -> String {
    let parent = checked(repository.find_commit(checked(git2::Oid::from_str(parent))));
    let mut builder = git2::build::TreeUpdateBuilder::new();
    for (path, content) in files {
        builder.upsert(
            *path,
            checked(repository.blob(content.as_bytes())),
            git2::FileMode::Blob,
        );
    }
    let tree = checked(repository.find_tree(checked(
        builder.create_updated(repository, &checked(parent.tree())),
    )));
    let signature = checked(git2::Signature::now("test", "test@example.invalid"));
    checked(repository.commit(
        Some(reference),
        &signature,
        &signature,
        "update",
        &tree,
        &[&parent],
    ))
    .to_string()
}

fn pull_request_event(
    delivery: &str,
    number: u64,
    head: (&str, &str),
    base: (&str, &str),
) -> (DeliveryHeaders, Vec<u8>) {
    let repository = json!({"id":7,"name":"monorepo","full_name":"fixture/monorepo","owner":{"login":"fixture"}});
    let body = checked(serde_json::to_vec(
        &json!({"action":"synchronize","repository":repository,"number":number,
        "pull_request":{"number":number,"state":"open",
            "head":{"ref":head.0,"sha":head.1,"repo":repository},
            "base":{"ref":base.0,"sha":base.1,"repo":repository}}}),
    ));
    let (mut headers, _) = push_event(delivery, head.1);
    let mut mac = checked(Hmac::<Sha256>::new_from_slice(SECRET));
    mac.update(&body);
    headers.signature = format!(
        "sha256={}",
        mac.finalize()
            .into_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    headers.event = "pull_request".into();
    (headers, body)
}

fn decision_reasons(source: &Value, project: &str) -> Value {
    checked(
        source
            .pointer("/affected/decisions")
            .and_then(Value::as_array)
            .and_then(|decisions| {
                decisions
                    .iter()
                    .find(|d| d.get("project_id") == Some(&json!(project)))
            })
            .cloned()
            .ok_or("decision"),
    )
}

// r[verify kiln.monorepo.dispatch]
// r[verify kiln.monorepo.affected.scenario.fallback]
#[test]
fn pull_requests_diff_from_the_merge_base_and_fall_back_without_a_base_branch() {
    let root = checked(tempfile::tempdir());
    let (repository, first) = workspace_repository(root.path());
    let path = checked(repository.workdir().ok_or("workdir")).to_path_buf();
    // feature changes only crate a; main then advances with a crate c change,
    // which a diff against the base-branch head (not the merge base) would show.
    let feature = commit_over(
        &repository,
        "refs/heads/feature",
        &first,
        &[("crates/a/src/lib.rs", "pub fn a() { }\n")],
    );
    let main = commit_over(
        &repository,
        "refs/heads/main",
        &first,
        &[("crates/c/src/lib.rs", "pub fn c() { }\n")],
    );
    // feature2 targets a base branch that does not exist in the repository.
    let feature2 = commit_over(
        &repository,
        "refs/heads/feature2",
        &first,
        &[("crates/c/src/lib.rs", "pub fn c2() {}\n")],
    );
    let mut profile_json = checked(serde_json::to_value(monorepo_profile(root.path(), &path)));
    for project in checked(
        profile_json
            .pointer_mut("/projects")
            .and_then(Value::as_array_mut)
            .ok_or("projects"),
    ) {
        set(
            project,
            "admitted_references",
            json!(["refs/heads/feature", "refs/heads/feature2"]),
        );
    }
    let profile: ServiceProfile = checked(serde_json::from_value(profile_json));
    let (mut engine, backends) = open_projects(profile);
    for id in CRATES {
        enroll_project(&mut engine, id);
    }
    let secret = root.path().join("github.secret");
    checked(std::fs::write(&secret, SECRET));
    checked(std::fs::set_permissions(
        &secret,
        std::fs::Permissions::from_mode(0o600),
    ));
    let trigger = |prefix: &str, id: &str, reference: &str, base: &str| {
        json!({"id":format!("{prefix}-{id}"),"project_id":id,"reference":reference,
            "repository":{"owner":"fixture","repository":"monorepo","repository_id":7,"allow_fork_pull_requests":false},
            "secret_file":secret,"enabled":true,"push":false,"pull_request":true,"pull_request_base":base})
    };
    let triggers: Vec<Value> = CRATES
        .iter()
        .map(|id| trigger("pr", id, "refs/heads/feature", "main"))
        .chain(
            CRATES
                .iter()
                .map(|id| trigger("rel", id, "refs/heads/feature2", "release")),
        )
        .collect();
    let policy: AutomationProfile = checked(serde_json::from_value(
        json!({"schema":"kiln.web-automation.v1","max_records":100,"status_limit":20,"max_event_bytes":65536,"max_tick_effects":2,
        "triggers":triggers,"schedules":[],"notifications":[]}),
    ));
    checked(engine.configure_automation(policy));
    let dispatches = |id: &str| checked(backends.get(id).ok_or("backend")).lock().dispatches;

    let (headers, body) = pull_request_event(
        "00000000-0000-4000-8000-00000000b001",
        7,
        ("feature", &feature),
        ("main", &main),
    );
    let response = checked(engine.automation_trigger("pr-a", &headers, &body));
    assert_eq!(response.status, 202);
    let received = value(&response);
    assert_eq!(field(&received, "status"), &json!("queued"));
    assert_eq!(
        received.pointer("/changed"),
        Some(&json!({"exact_count":1})),
        "the diff is taken from the merge base, not the advanced base branch"
    );
    assert_eq!(
        decision_reasons(&received, "a").pointer("/reasons/0/kind"),
        Some(&json!("path"))
    );
    assert_eq!(
        decision_reasons(&received, "b").pointer("/reasons/0/kind"),
        Some(&json!("graph"))
    );
    assert_eq!(
        decision_reasons(&received, "c"),
        json!({"project_id":"c","outcome":"skipped","reason":"no_changes_in_scope"})
    );
    assert_eq!(
        (dispatches("a"), dispatches("b"), dispatches("c")),
        (1, 1, 0)
    );
    let sources = sources_by_project(&mut engine);
    let skipped = checked(sources.get("c").and_then(|s| s.first()).ok_or("c source"));
    assert_eq!(field(skipped, "status"), &json!("skipped"));
    assert_eq!(field(skipped, "run_id"), &Value::Null);
    let b_run = checked(
        sources
            .get("b")
            .and_then(|s| s.first())
            .and_then(|s| s.get("run_id"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or("b run"),
    );
    let b = run_view(&mut engine, &b_run);
    assert_eq!(field(&b, "source_revision"), &json!(feature));
    assert_eq!(b.pointer("/affected/reasons/0/kind"), Some(&json!("graph")));

    // An unknown base branch has no exact base: every scoped project runs.
    let (headers, body) = pull_request_event(
        "00000000-0000-4000-8000-00000000b002",
        8,
        ("feature2", &feature2),
        ("release", &first),
    );
    let response = checked(engine.automation_trigger("rel-b", &headers, &body));
    assert_eq!(response.status, 202);
    let received = value(&response);
    assert_eq!(
        received.pointer("/changed"),
        Some(&json!({"fallback":"no_base"}))
    );
    for id in CRATES {
        assert_eq!(
            decision_reasons(&received, id).pointer("/reasons/0"),
            Some(&json!({"kind":"fallback","reason":"no_base"})),
            "{id}"
        );
    }
    assert_eq!(
        (dispatches("a"), dispatches("b"), dispatches("c")),
        (2, 2, 1)
    );
}
