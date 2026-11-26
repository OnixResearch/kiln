use super::*;
use tempfile::TempDir;

fn repository() -> String {
    "a".repeat(64)
}

fn fixture() -> TempDir {
    let root = tempfile::tempdir().unwrap();
    for directory in [".cairn", ".cairn/changes", ".cairn/specs", ".cairn/archive"] {
        fs::create_dir(root.path().join(directory)).unwrap();
    }
    root
}

fn enrollment(root: &Path) -> RootEnrollment {
    RootEnrollment {
        repository: repository(),
        label: "Disposable Cairn fixture".into(),
        root: root.to_owned(),
    }
}

fn reply(
    authoring: Option<&Authoring>,
    method: &str,
    target: &str,
    body: &[u8],
    role: Role,
    restricted: bool,
) -> (u16, Value) {
    let response = response(authoring, method, target, body, role, restricted);
    (
        response.status,
        serde_json::from_slice(&response.body).unwrap(),
    )
}

fn request(authoring: &Authoring, method: &str, target: &str, body: &[u8]) -> (u16, Value) {
    reply(Some(authoring), method, target, body, Role::Operator, false)
}

fn read(authoring: &Authoring) -> Value {
    let (status, payload) = request(
        authoring,
        "GET",
        &format!("{ROUTE}?repository={}", repository()),
        b"",
    );
    assert_eq!(status, 200, "{payload}");
    payload["snapshot"].clone()
}

fn mutate(authoring: &Authoring, snapshot: &Value, action: Value) -> (u16, Value) {
    request(
        authoring,
        "POST",
        ROUTE,
        json!({
            "repository": repository(), "expected_revision": snapshot["revision"], "action": action,
        })
        .to_string()
        .as_bytes(),
    )
}

#[test]
fn admission_never_resolves_an_unenrolled_or_worker_supplied_path() {
    let root = fixture();
    let authoring = Authoring::new(
        std::env::current_exe().unwrap(),
        vec![enrollment(root.path())],
    )
    .unwrap();
    for (role, restricted) in [
        (Role::Worker, false),
        (Role::Viewer, false),
        (Role::Operator, true),
    ] {
        assert_eq!(
            reply(Some(&authoring), "GET", ROUTE, b"", role, restricted).0,
            403
        );
        assert_eq!(
            reply(Some(&authoring), "POST", ROUTE, b"{}", role, restricted).0,
            403
        );
    }
    let (_, index) = request(&authoring, "GET", ROUTE, b"");
    assert_eq!(index["roots"][0]["repository"], repository());
    assert!(index["roots"][0].get("root").is_none());
    assert_eq!(index["capabilities"]["edit_artifacts"], false);
    assert_eq!(reply(None, "GET", ROUTE, b"", Role::Operator, false).0, 503);
    assert_eq!(request(&authoring, "PUT", ROUTE, b"{}").0, 405);
    assert_eq!(
        request(&authoring, "GET", &format!("{ROUTE}?root=/tmp"), b"").0,
        400
    );
    assert_eq!(
        request(
            &authoring,
            "GET",
            &format!("{ROUTE}?repository={}", "b".repeat(64)),
            b""
        )
        .0,
        404
    );
    let baseline = json!({"revision": "c".repeat(64)});
    for action in [
        json!({"type": "create", "change": "../outside"}),
        json!({"type": "create", "change": "--execute"}),
        json!({"type": "create", "change": "alpha", "root": "/tmp"}),
        json!({"type": "edit_artifact", "change": "alpha", "path": "proposal.md", "text": "replacement"}),
    ] {
        assert_eq!(mutate(&authoring, &baseline, action).0, 400);
    }
    assert_eq!(
        request(&authoring, "POST", ROUTE, &vec![b' '; REQUEST_BYTES + 1]).0,
        413
    );
}

#[test]
fn profile_is_bounded_strict_and_enrolls_only_distinct_native_roots() {
    let root = fixture();
    let profile = root.path().join("profile.json");
    let executable = std::env::current_exe().unwrap();
    let value = json!({
        "schema": PROFILE_SCHEMA, "cairn_executable": executable,
        "roots": [{"repository": repository(), "root": root.path(), "label": "Fixture"}],
    });
    fs::write(&profile, value.to_string()).unwrap();
    assert_eq!(Authoring::load(&profile).unwrap().roots.len(), 1);
    let mut invalid = value.clone();
    invalid["roots"][0]["root"] = json!(".");
    fs::write(&profile, invalid.to_string()).unwrap();
    assert!(Authoring::load(&profile).is_err());
    let mut invalid = value.clone();
    invalid["roots"]
        .as_array_mut()
        .unwrap()
        .push(value["roots"][0].clone());
    invalid["roots"][1]["repository"] = json!("b".repeat(64));
    fs::write(&profile, invalid.to_string()).unwrap();
    assert!(
        Authoring::load(&profile).is_err(),
        "same worktree under two identities"
    );
    fs::write(&profile, vec![b' '; PROFILE_BYTES + 1]).unwrap();
    assert!(Authoring::load(&profile).is_err());
}

#[test]
fn cycle_check_follows_transitive_prerequisites() {
    let dependencies = BTreeMap::from([
        ("alpha".into(), BTreeSet::from(["beta".into()])),
        ("beta".into(), BTreeSet::from(["gamma".into()])),
        ("gamma".into(), BTreeSet::new()),
    ]);
    assert!(reaches(&dependencies, "alpha", "gamma"));
    assert!(!reaches(&dependencies, "gamma", "alpha"));
}

#[cfg(unix)]
#[test]
fn manifest_refuses_symlinks_and_hard_links_and_serializes_authoring() {
    use std::os::unix::fs::symlink;
    let root = fixture();
    let root_entry = enrollment(root.path());
    let lock = lock_root(&root_entry).unwrap();
    assert_eq!(
        lock_root(&root_entry).unwrap_err().1,
        "cairn_authoring_root_busy"
    );
    drop(lock);
    assert!(lock_root(&root_entry).is_ok());
    let outside = root.path().join("not-enrolled.txt");
    fs::write(&outside, "outside").unwrap();
    let inside = root.path().join(".cairn/changes/linked");
    symlink(&outside, &inside).unwrap();
    assert!(read_tree(root.path()).is_err());
    fs::remove_file(&inside).unwrap();
    fs::hard_link(&outside, &inside).unwrap();
    assert!(read_tree(root.path()).is_err());
}

// r[verify kiln.sdlc.cairn_authoring.scenario.operator]
#[test]
#[ignore = "requires KILN_TEST_CAIRN pointing to the flake-pinned Cairn executable"]
fn native_create_add_remove_and_stale_refusal() {
    let executable = std::env::var_os("KILN_TEST_CAIRN")
        .map(PathBuf::from)
        .expect("set KILN_TEST_CAIRN to the pinned executable");
    let root = fixture();
    let authoring = Authoring::new(executable, vec![enrollment(root.path())]).unwrap();
    let initial = read(&authoring);
    let (status, created) = mutate(
        &authoring,
        &initial,
        json!({"type": "create", "change": "alpha"}),
    );
    assert_eq!(status, 200, "{created}");
    assert_eq!(created["outcome"], "applied", "{created}");
    assert_eq!(created["provider"]["result"]["command"], "change_create");
    assert_eq!(created["snapshot"]["change"]["name"], "alpha");
    assert_ne!(created["snapshot"]["revision"], initial["revision"]);
    let (status, refused) = mutate(
        &authoring,
        &initial,
        json!({"type": "create", "change": "beta"}),
    );
    assert_eq!(status, 409);
    assert_eq!(refused["code"], STALE.1);
    assert!(!root.path().join(".cairn/changes/beta").exists());
    let (_, beta) = mutate(
        &authoring,
        &created["snapshot"],
        json!({"type": "create", "change": "beta"}),
    );
    assert_eq!(beta["outcome"], "applied", "{beta}");
    // A prose edit can leave the native graph unchanged, but not its revision.
    let proposal = root.path().join(".cairn/changes/alpha/proposal.md");
    fs::write(
        &proposal,
        "## Why\n\nA separate author changed these bytes.\n",
    )
    .unwrap();
    let add = json!({"type": "add_dependency", "change": "alpha", "dependency": "beta"});
    assert_eq!(mutate(&authoring, &beta["snapshot"], add.clone()).0, 409);
    let current = read(&authoring);
    let (status, added) = mutate(&authoring, &current, add);
    assert_eq!(status, 200, "{added}");
    assert_eq!(added["outcome"], "applied", "{added}");
    assert_eq!(added["provider"]["result"]["command"], "change depend add");
    assert_eq!(
        added["snapshot"]["change"]["metadata"]["depends_on"],
        json!(["beta"])
    );
    assert_eq!(
        added["snapshot"]["graph"]["edges"],
        json!([{"dependent": "alpha", "dependency": "beta", "satisfied": false}])
    );
    let (status, cycle) = mutate(
        &authoring,
        &added["snapshot"],
        json!({"type": "add_dependency", "change": "beta", "dependency": "alpha"}),
    );
    assert_eq!(status, 409);
    assert_eq!(cycle["code"], "cairn_authoring_dependency_cycle");
    let (status, removed) = mutate(
        &authoring,
        &added["snapshot"],
        json!({"type": "remove_dependency", "change": "alpha", "dependency": "beta"}),
    );
    assert_eq!(status, 200, "{removed}");
    assert_eq!(removed["outcome"], "applied", "{removed}");
    assert_eq!(removed["snapshot"]["graph"]["edges"], json!([]));
    assert_ne!(
        removed["snapshot"]["revision"],
        added["snapshot"]["revision"]
    );
    assert!(!root.path().join(".git").exists());
}

#[cfg(unix)]
#[test]
#[ignore = "requires KILN_TEST_CAIRN pointing to the flake-pinned Cairn executable"]
fn native_effect_is_reconciled_when_the_machine_receipt_is_lost() {
    use std::os::unix::fs::PermissionsExt;
    let executable =
        std::env::var("KILN_TEST_CAIRN").expect("set KILN_TEST_CAIRN to the pinned executable");
    let root = fixture();
    let wrapper = root.path().join("receipt-loss");
    // Fixed test-only wrapper: Cairn applies create normally, but its stdout is
    // lost afterward. Subsequent reads still use the actual producer.
    let executable = executable.replace('\'', "'\\''");
    fs::write(&wrapper, format!("#!/bin/sh\nif [ \"$3\" = create ]; then\n  '{executable}' \"$@\" >/dev/null\n  exit 1\nfi\nexec '{executable}' \"$@\"\n")).unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
    let authoring = Authoring::new(wrapper, vec![enrollment(root.path())]).unwrap();
    let initial = read(&authoring);
    let (status, result) = mutate(
        &authoring,
        &initial,
        json!({"type": "create", "change": "alpha"}),
    );
    assert_eq!(status, 200, "{result}");
    assert_eq!(result["outcome"], "reconciled", "{result}");
    assert!(result["provider"].is_null());
    assert_eq!(result["snapshot"]["change"]["name"], "alpha");
    assert_eq!(
        mutate(
            &authoring,
            &initial,
            json!({"type": "create", "change": "other"})
        )
        .0,
        409
    );
}
