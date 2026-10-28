use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use git2::{Oid, Repository, Signature, Time};
use kiln_core::cob::INITIAL_CAIRN_TYPES;
use kiln_web_service::cobs::{Browser, Profile, RepositoryPolicy};
use kiln_web_service::engine::{ApiResponse, Role};
use serde_json::{Value, json};

const NATIVE_ID: &str = "rad:zFixtureOnix";
const RECORDED_AT_MS: i64 = 1_700_000_000_000;

fn signature() -> Signature<'static> {
    Signature::new(
        "COB fixture",
        "cob@example.invalid",
        &Time::new(RECORDED_AT_MS / 1000, 0),
    )
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
}

fn operation(
    repository: &Repository,
    entries: &[(&str, Vec<u8>)],
    message: &str,
    parents: &[Oid],
) -> Oid {
    let mut builder = repository
        .treebuilder(None)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    for (name, bytes) in entries {
        let blob = repository
            .blob(bytes)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        builder
            .insert(*name, blob, 0o100644)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    }
    let tree_id = builder
        .write()
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let tree = repository
        .find_tree(tree_id)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let parents: Vec<_> = parents
        .iter()
        .map(|id| {
            repository
                .find_commit(*id)
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
        })
        .collect();
    let parents: Vec<_> = parents.iter().collect();
    repository
        .commit(None, &signature(), &signature(), message, &tree, &parents)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
}

fn entries(type_name: &str, actions: &[Value]) -> Vec<(String, Vec<u8>)> {
    let mut entries = vec![(
        "manifest".into(),
        serde_json::to_vec(&json!({"typeName": type_name, "version": 1}))
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
    )];
    entries.extend(actions.iter().enumerate().map(|(index, value)| {
        (
            index.to_string(),
            serde_json::to_vec(value)
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
        )
    }));
    entries
}

fn cob_operation(
    repository: &Repository,
    type_name: &str,
    message: &str,
    actions: &[Value],
    parents: &[Oid],
) -> Oid {
    let entries = entries(type_name, actions);
    let borrowed: Vec<_> = entries
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.clone()))
        .collect();
    operation(repository, &borrowed, message, parents)
}

fn policy(id: &str, native_id: &str, path: &Path) -> RepositoryPolicy {
    RepositoryPolicy {
        id: id.into(),
        label: format!("Onix {id}"),
        repository_id: native_id.into(),
        git_directory: path.to_owned(),
    }
}

fn browser(repositories: Vec<RepositoryPolicy>) -> Browser {
    Browser::new(Some(Profile {
        schema: "kiln.web-cobs.v1".into(),
        repositories,
    }))
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
}

fn value(response: &ApiResponse) -> Value {
    serde_json::from_slice(&response.body)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
}

fn member<I: serde_json::value::Index>(value: &Value, key: I) -> &Value {
    value
        .get(key)
        .unwrap_or_else(|| unreachable!("fixture member absent"))
}

fn insert(value: &mut Value, path: &str, key: &str, item: Value) {
    value
        .pointer_mut(path)
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| unreachable!("fixture object absent"))
        .insert(key.to_owned(), item);
}

fn get(browser: &Browser, target: &str) -> Value {
    let response = browser.response("GET", target, &[], Role::Viewer);
    assert_eq!(response.status, 200, "{target}: {}", value(&response));
    value(&response)
}

fn rejects(browser: &Browser, target: &str, status: u16, code: &str) {
    let response = browser.response("GET", target, &[], Role::Viewer);
    assert_eq!(response.status, status, "{target}: {}", value(&response));
    assert_eq!((*member(&value(&response), "code")), code, "{target}");
}

fn detail_target(type_name: &str, root: Oid) -> String {
    format!("/api/cobs/object?repository=onix&type={type_name}&object={root}")
}

// Compare actual ref targets and every repository file, not just API counts.
// A read must not pack refs, rewrite configuration, add objects, or leave locks.
fn repository_files(path: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, path: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(path)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
        {
            let entry = entry.unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
            let path = entry.path();
            if entry
                .file_type()
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
                .is_dir()
            {
                visit(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root)
                        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
                        .to_owned(),
                    std::fs::read(path)
                        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(path, path, &mut files);
    files
}

fn refs(repository: &Repository) -> BTreeMap<String, Oid> {
    repository
        .references()
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
        .map(|reference| {
            let reference =
                reference.unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
            (
                reference
                    .name()
                    .unwrap_or_else(|| unreachable!("fixture value absent"))
                    .into(),
                reference
                    .target()
                    .unwrap_or_else(|| unreachable!("fixture value absent")),
            )
        })
        .collect()
}

struct ObjectFixture {
    type_name: &'static str,
    root: Oid,
    left: Oid,
    right: Oid,
    expected_operations: BTreeMap<String, Value>,
    expected_tips: Value,
}

fn expected_operation(
    repository: &Repository,
    id: Oid,
    message: &str,
    git_parents: &[Oid],
    history_parents: &[Oid],
    actions: &[Value],
) -> Value {
    let tree = repository
        .find_commit(id)
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
        .tree()
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let actions: Vec<_> = actions
        .iter()
        .enumerate()
        .map(|(index, action)| {
            let entry = tree.get_name(&index.to_string()).unwrap_or_else(|| unreachable!("fixture value absent"));
            json!({
                "index": index,
                "blob_id": entry.id().to_string(),
                "bytes": serde_json::to_vec(action).unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")).len(),
                "value": action,
            })
        })
        .collect();
    json!({
        "id": id.to_string(),
        "git_parents": git_parents.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "history_parents": history_parents.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "message": message,
        "author_name": "COB fixture",
        "author_email": "cob@example.invalid",
        "recorded_at_ms": RECORDED_AT_MS,
        "signature_status": "not_verified",
        "actions": actions,
    })
}

struct Fixture {
    _root: tempfile::TempDir,
    repository: Repository,
    objects: Vec<ObjectFixture>,
}

impl Fixture {
    fn new() -> Self {
        let root =
            tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        let repository = Repository::init_bare(root.path().join("private-cob-store.git"))
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        // Deliberately not COB commits. Following these parents would fail manifest admission.
        let resource = operation(
            &repository,
            &[("source", b"resource".to_vec())],
            "resource",
            &[],
        );
        let related = operation(
            &repository,
            &[("source", b"related".to_vec())],
            "related",
            &[],
        );
        let mut objects = Vec::new();
        for (index, type_name) in INITIAL_CAIRN_TYPES.into_iter().enumerate() {
            let root_message =
                format!("Object {index}: {type_name}\n\nInitial observation, not reduced state.\n");
            let root_actions = vec![
                json!({"type":"create", "title":format!("Fixture {index}"), "body":"<script>alert('inert')</script>\nraw text"}),
                json!({"type":"metadata", "nested":{"number":index,"enabled":true,"unset":null},"items":[1,"two"]}),
            ];
            // Root has a non-COB parent without a trailer: history must stop at the selected ID.
            let root_id = cob_operation(
                &repository,
                type_name,
                &root_message,
                &root_actions,
                &[resource],
            );
            let left_actions = vec![json!({"type":"comment", "body":"left update"})];
            let left_message = format!(
                "Left update {index}\n\nRad-Resource: {resource}\nRad-Related: {related}\n"
            );
            let left = cob_operation(
                &repository,
                type_name,
                &left_message,
                &left_actions,
                &[root_id, resource, related],
            );
            let right_actions = vec![json!({"type":"comment", "body":"right update"})];
            let right_message = format!("Right update {index}\n");
            let right = cob_operation(
                &repository,
                type_name,
                &right_message,
                &right_actions,
                &[root_id],
            );
            let tips = [
                (format!("refs/cobs/{type_name}/{root_id}"), left),
                (
                    format!("refs/namespaces/alice/refs/cobs/{type_name}/{root_id}"),
                    right,
                ),
                (
                    format!("refs/namespaces/bob/refs/cobs/{type_name}/{root_id}"),
                    root_id,
                ),
            ];
            for (name, tip) in &tips {
                repository
                    .reference(name, *tip, false, "fixture")
                    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
            }
            let expected_tips = json!(
                tips.iter()
                    .map(|(name, tip)| { json!({"reference":name,"operation_id":tip.to_string()}) })
                    .collect::<Vec<_>>()
            );
            let expected_operations = [
                (
                    root_id,
                    expected_operation(
                        &repository,
                        root_id,
                        &root_message,
                        &[resource],
                        &[resource],
                        &root_actions,
                    ),
                ),
                (
                    left,
                    expected_operation(
                        &repository,
                        left,
                        &left_message,
                        &[root_id, resource, related],
                        &[root_id],
                        &left_actions,
                    ),
                ),
                (
                    right,
                    expected_operation(
                        &repository,
                        right,
                        &right_message,
                        &[root_id],
                        &[root_id],
                        &right_actions,
                    ),
                ),
            ]
            .into_iter()
            .map(|(id, value)| (id.to_string(), value))
            .collect();
            objects.push(ObjectFixture {
                type_name,
                root: root_id,
                left,
                right,
                expected_operations,
                expected_tips,
            });
        }
        // An unrelated producer's refs and ordinary source refs are not Onix objects.
        repository
            .reference(
                "refs/cobs/org.example.other/not-an-onix-id",
                resource,
                false,
                "fixture",
            )
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        repository
            .reference("refs/heads/main", resource, false, "fixture")
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        Self {
            _root: root,
            repository,
            objects,
        }
    }

    fn browser(&self) -> Browser {
        browser(vec![policy("onix", NATIVE_ID, self.repository.path())])
    }

    fn assert_private(&self, response: &ApiResponse) {
        let body = String::from_utf8(response.body.clone())
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        assert!(
            !body.contains(
                self.repository
                    .path()
                    .to_str()
                    .unwrap_or_else(|| unreachable!("fixture value absent"))
            )
        );
        assert!(!body.contains("private-cob-store.git"));
        assert!(!body.contains("git_directory"));
    }
}

// r[verify kiln.dashboard.cobs.read.scenario.present]
// r[verify kiln.dashboard.cobs.authority.scenario.reader]
#[test]
fn six_types_preserve_exact_multitip_history_actions_and_leave_git_unchanged() {
    let fixture = Fixture::new();
    let before_files = repository_files(fixture.repository.path());
    let before_refs = refs(&fixture.repository);
    let browser = fixture.browser();
    let inventory = get(&browser, "/api/cobs");
    assert_eq!((*member(&inventory, "configured")), true);
    assert_eq!((*member(&inventory, "complete")), true);
    assert_eq!((*member(&inventory, "total")), 6);
    assert_eq!((*member(&inventory, "next_cursor")), Value::Null);
    assert_eq!(
        (*member(&inventory, "repositories")),
        json!([{"id":"onix","label":"Onix onix","repository_id":NATIVE_ID}])
    );
    assert_eq!(
        (*member(&inventory, "sources")),
        json!([{"id":"onix","label":"Onix onix","repository_id":NATIVE_ID,"status":"available","objects":6}])
    );
    let type_rows: Vec<_> = (*member(&inventory, "types"))
        .as_array()
        .unwrap_or_else(|| unreachable!("fixture value absent"))
        .iter()
        .map(|row| {
            (
                (*member(row, "id"))
                    .as_str()
                    .unwrap_or_else(|| unreachable!("fixture value absent")),
                (*member(row, "label"))
                    .as_str()
                    .unwrap_or_else(|| unreachable!("fixture value absent")),
            )
        })
        .collect();
    assert_eq!(
        type_rows,
        [
            ("xyz.onixresearch.cairn-change", "Cairn changes"),
            ("xyz.onixresearch.rfd", "RFDs"),
            ("xyz.onixresearch.adr", "ADRs"),
            (
                "xyz.onixresearch.verification-obligation",
                "Verification obligations"
            ),
            ("xyz.onixresearch.release-decision", "Release decisions"),
            ("xyz.onixresearch.policy-exception", "Policy exceptions"),
            ("xyz.radicle.issue", "Radicle issues"),
            ("xyz.radicle.patch", "Radicle patches"),
            ("xyz.radworks.job", "Radicle CI jobs"),
        ]
    );
    let rows = (*member(&inventory, "objects"))
        .as_array()
        .unwrap_or_else(|| unreachable!("fixture value absent"));
    let mut public_ids = std::collections::BTreeSet::new();
    for object in &fixture.objects {
        let row = rows
            .iter()
            .find(|row| (*member(row, "object_id")) == object.root.to_string())
            .unwrap_or_else(|| unreachable!("fixture value absent"));
        assert_eq!((*member(row, "repository")), "onix");
        assert_eq!((*member(row, "repository_id")), NATIVE_ID);
        assert_eq!((*member(row, "type_name")), object.type_name);
        assert_eq!((*member(row, "ref_tips")), object.expected_tips);
        assert!(
            public_ids.insert(
                row["id"]
                    .as_str()
                    .unwrap_or_else(|| unreachable!("fixture value absent"))
            )
        );
        let target = detail_target(object.type_name, object.root);
        let response = browser.response("GET", &target, &[], Role::Operator);
        assert_eq!(response.status, 200, "{}", value(&response));
        fixture.assert_private(&response);
        let detail = value(&response);
        assert_eq!(
            (*member(&detail, "non_claims")),
            (*member(&inventory, "non_claims"))
        );
        let detail = member(&detail, "object");
        assert_eq!((*member(detail, "identity")), *row);
        assert_eq!((*member(detail, "operation_count")), 3);
        assert_eq!((*member(detail, "observation")), "raw_local_history");
        assert_eq!((*member(detail, "signatures_verified")), false);
        assert_eq!((*member(detail, "reduced_state")), Value::Null);
        let actual: BTreeMap<_, _> = (*member(detail, "operations"))
            .as_array()
            .unwrap_or_else(|| unreachable!("fixture value absent"))
            .iter()
            .map(|operation| {
                (
                    (*member(operation, "id"))
                        .as_str()
                        .unwrap_or_else(|| unreachable!("fixture value absent"))
                        .to_owned(),
                    operation.clone(),
                )
            })
            .collect();
        assert_eq!(actual, object.expected_operations);
        let retained: u64 = actual
            .values()
            .flat_map(|operation| {
                (*member(operation, "actions"))
                    .as_array()
                    .unwrap_or_else(|| unreachable!("fixture value absent"))
            })
            .map(|action| {
                (*member(action, "bytes"))
                    .as_u64()
                    .unwrap_or_else(|| unreachable!("fixture value absent"))
            })
            .sum();
        assert_eq!((*member(detail, "retained_action_bytes")), retained);
    }
    fixture.assert_private(&browser.response("GET", "/api/cobs", &[], Role::Viewer));
    assert_eq!(refs(&fixture.repository), before_refs);
    assert_eq!(repository_files(fixture.repository.path()), before_files);
}

// r[verify kiln.dashboard.cobs.read.scenario.unavailable]
#[test]
fn paging_search_and_cursor_identity_follow_filters_and_observed_ref_tips() {
    let fixture = Fixture::new();
    let browser = fixture.browser();
    let complete = get(&browser, "/api/cobs");
    let first = get(&browser, "/api/cobs?limit=2");
    let cursor = (*member(&first, "next_cursor"))
        .as_str()
        .unwrap_or_else(|| unreachable!("fixture value absent"));
    let second = get(&browser, &format!("/api/cobs?limit=2&cursor={cursor}"));
    let third = get(
        &browser,
        &format!(
            "/api/cobs?limit=2&cursor={}",
            member(&second, "next_cursor")
                .as_str()
                .unwrap_or_else(|| unreachable!("fixture value absent"))
        ),
    );
    let paged: Vec<_> = [&first, &second, &third]
        .into_iter()
        .flat_map(|page| {
            (*member(page, "objects"))
                .as_array()
                .unwrap_or_else(|| unreachable!("fixture value absent"))
                .iter()
                .cloned()
        })
        .collect();
    assert_eq!(
        paged,
        *member(&complete, "objects")
            .as_array()
            .unwrap_or_else(|| unreachable!("fixture value absent"))
    );
    assert_eq!(
        (*member(&first, "snapshot")),
        (*member(&second, "snapshot"))
    );
    assert_eq!(
        (*member(&second, "snapshot")),
        (*member(&third, "snapshot"))
    );
    assert_eq!((*member(&third, "next_cursor")), Value::Null);
    assert_eq!(first, get(&browser, "/api/cobs?limit=2"));
    let object = fixture
        .objects
        .first()
        .unwrap_or_else(|| unreachable!("fixture object absent"));
    for filter in [
        "repository=onix".to_owned(),
        format!("type={}", object.type_name),
        "q=Object".to_owned(),
        "limit=3".to_owned(),
    ] {
        let target = if filter.starts_with("limit=") {
            format!("/api/cobs?{filter}&cursor={cursor}")
        } else {
            format!("/api/cobs?limit=2&{filter}&cursor={cursor}")
        };
        rejects(&browser, &target, 409, "cob_snapshot_changed");
    }
    for offset in ["0", "1", "6", "not-a-number"] {
        rejects(
            &browser,
            &format!(
                "/api/cobs?limit=2&cursor={}:{offset}",
                member(&first, "snapshot")
                    .as_str()
                    .unwrap_or_else(|| unreachable!("fixture value absent"))
            ),
            400,
            "cursor_invalid",
        );
    }
    let by_title = get(&browser, "/api/cobs?q=oBjEcT+0");
    let by_id = get(&browser, &format!("/api/cobs?q={}", object.root));
    let by_type = get(&browser, &format!("/api/cobs?type={}", object.type_name));
    assert_eq!(
        (*member(&by_title, "objects")),
        (*member(&by_id, "objects"))
    );
    assert_eq!(
        (*member(&by_title, "objects")),
        (*member(&by_type, "objects"))
    );
    assert_eq!((*member(&by_title, "total")), 1);
    assert_eq!(
        (*member(&get(&browser, "/api/cobs?q=ONIX+ONIX"), "objects")),
        (*member(&complete, "objects"))
    );
    assert_eq!(
        (*member(
            &get(&browser, &format!("/api/cobs?q={NATIVE_ID}")),
            "objects"
        )),
        (*member(&complete, "objects"))
    );
    assert_eq!(
        (*member(&get(&browser, "/api/cobs?q=never-present"), "total")),
        0
    );
    let name = format!("refs/cobs/{}/{}", object.type_name, object.root);
    fixture
        .repository
        .reference(&name, object.right, true, "move fixture tip")
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let after_move = repository_files(fixture.repository.path());
    let after_refs = refs(&fixture.repository);
    rejects(
        &browser,
        &format!("/api/cobs?limit=2&cursor={cursor}"),
        409,
        "cob_snapshot_changed",
    );
    let fresh = get(&browser, "/api/cobs?limit=2");
    assert_ne!((*member(&fresh, "snapshot")), (*member(&first, "snapshot")));
    assert_eq!((*member(&fresh, "total")), (*member(&first, "total")));
    assert_eq!(refs(&fixture.repository), after_refs);
    assert_eq!(repository_files(fixture.repository.path()), after_move);
}

#[test]
fn unconfigured_empty_missing_and_unavailable_stores_are_distinct() {
    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let unconfigured =
        Browser::new(None).unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let absent = get(&unconfigured, "/api/cobs");
    assert_eq!((*member(&absent, "configured")), false);
    assert_eq!((*member(&absent, "complete")), true);
    assert_eq!((*member(&absent, "sources")), json!([]));
    assert_eq!((*member(&absent, "objects")), json!([]));
    assert_eq!((*member(&absent, "total")), 0);
    rejects(
        &unconfigured,
        "/api/cobs?repository=onix",
        404,
        "cob_repository_missing",
    );
    let empty = Repository::init_bare(root.path().join("empty.git"))
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let nongit = root.path().join("nongit");
    std::fs::create_dir(&nongit).unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let browser = browser(vec![
        policy("empty", "native-empty", empty.path()),
        policy(
            "missing",
            "native-missing",
            &root.path().join("missing.git"),
        ),
        policy("nongit", "native-nongit", &nongit),
    ]);
    let all = get(&browser, "/api/cobs");
    assert_eq!((*member(&all, "configured")), true);
    assert_eq!((*member(&all, "complete")), false);
    assert_eq!((*member(&all, "total")), 0);
    assert_eq!(
        (*member(member(member(&all, "sources"), 0), "status")),
        "available"
    );
    assert_eq!((*member(member(member(&all, "sources"), 0), "objects")), 0);
    for index in [1, 2] {
        assert_eq!(
            (*member(member(member(&all, "sources"), index), "status")),
            "unavailable"
        );
        assert_eq!(
            (*member(member(member(&all, "sources"), index), "objects")),
            Value::Null
        );
    }
    let selected = get(&browser, "/api/cobs?repository=empty");
    assert_eq!((*member(&selected, "complete")), true);
    assert_eq!(
        (*member(&selected, "repositories")),
        (*member(&all, "repositories"))
    );
    assert_eq!(
        (*member(&selected, "sources")),
        json!([member(member(&all, "sources"), 0)])
    );
    rejects(
        &browser,
        "/api/cobs?repository=not-admitted",
        404,
        "cob_repository_missing",
    );
    let oid = "0123456789012345678901234567890123456789";
    let kind = INITIAL_CAIRN_TYPES
        .first()
        .copied()
        .unwrap_or_else(|| unreachable!("fixture type absent"));
    rejects(
        &browser,
        &format!("/api/cobs/object?repository=empty&type={kind}&object={oid}"),
        404,
        "cob_object_missing",
    );
    rejects(
        &browser,
        &format!("/api/cobs/object?repository=missing&type={kind}&object={oid}"),
        503,
        "cob_store_unavailable",
    );
    assert!(
        !serde_json::to_string(&all)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
            .contains(
                root.path()
                    .to_str()
                    .unwrap_or_else(|| unreachable!("fixture value absent"))
            )
    );
}

#[test]
fn policy_rejects_ambiguous_identity_paths_unknown_fields_and_excess_repositories() {
    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let valid = json!({"schema":"kiln.web-cobs.v1","repositories":[
        {"id":"onix","label":"Onix","repository_id":NATIVE_ID,"git_directory":root.path().join("onix.git")},
        {"id":"other","label":"Other","repository_id":"native-other","git_directory":root.path().join("other.git")},
    ]});
    let admitted: Profile = serde_json::from_value(valid.clone())
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    admitted
        .admit()
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let path = root.path().join("policy.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&valid)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
    )
    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    Profile::load(&path).unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let mut invalid = Vec::new();
    for field in ["id", "repository_id", "git_directory"] {
        let mut duplicate = valid.clone();
        let original = valid
            .pointer(&format!("/repositories/0/{field}"))
            .unwrap_or_else(|| unreachable!("fixture member absent"))
            .clone();
        insert(&mut duplicate, "/repositories/1", field, original);
        invalid.push(duplicate);
    }
    for (field, value) in [
        ("id", json!("../onix")),
        ("repository_id", json!("")),
        ("repository_id", json!("native\ncredential")),
        ("label", json!("")),
        ("git_directory", json!("relative/store.git")),
        (
            "git_directory",
            json!(root.path().join("nested/../outside.git")),
        ),
    ] {
        let mut profile = valid.clone();
        insert(&mut profile, "/repositories/0", field, value);
        invalid.push(profile);
    }
    let mut schema = valid.clone();
    insert(&mut schema, "", "schema", json!("unknown.v1"));
    invalid.push(schema);
    let mut too_many = valid.clone();
    let repositories = json!((0..65).map(|index| json!({
        "id":format!("store-{index}"), "label":"Store", "repository_id":format!("native-{index}"),
        "git_directory":root.path().join(format!("store-{index}.git")),
    })).collect::<Vec<_>>());
    insert(&mut too_many, "", "repositories", repositories);
    invalid.push(too_many);
    for candidate in invalid {
        let parsed: Profile = serde_json::from_value(candidate.clone())
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        assert!(parsed.admit().is_err(), "{candidate}");
        assert!(Browser::new(Some(parsed)).is_err(), "{candidate}");
    }
    for nested in [false, true] {
        let mut candidate = valid.clone();
        if nested {
            insert(
                &mut candidate,
                "/repositories/0",
                "credentials",
                json!("not-admitted"),
            );
        } else {
            insert(&mut candidate, "", "discover", json!(true));
        }
        assert!(serde_json::from_value::<Profile>(candidate.clone()).is_err());
        std::fs::write(
            &path,
            serde_json::to_vec(&candidate)
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
        )
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        assert!(Profile::load(&path).is_err());
    }
    std::fs::write(&path, b"{not-json")
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    assert!(Profile::load(&path).is_err());
    assert!(Profile::load(&root.path().join("absent.json")).is_err());
}

// r[verify kiln.dashboard.cobs.authority.scenario.denied]
#[test]
fn authority_query_and_exact_selection_rejections_do_not_expose_paths_or_write() {
    let fixture = Fixture::new();
    let before = repository_files(fixture.repository.path());
    let before_refs = refs(&fixture.repository);
    let browser = fixture.browser();
    let object = fixture
        .objects
        .first()
        .unwrap_or_else(|| unreachable!("fixture object absent"));
    let detail = detail_target(object.type_name, object.root);
    for (method, body, role, status, code) in [
        ("GET", b"".as_slice(), Role::Worker, 403, "worker_scope"),
        (
            "POST",
            b"".as_slice(),
            Role::Operator,
            405,
            "cobs_read_only",
        ),
        (
            "DELETE",
            b"".as_slice(),
            Role::Operator,
            405,
            "cobs_read_only",
        ),
        (
            "GET",
            b"{}".as_slice(),
            Role::Viewer,
            400,
            "unexpected_body",
        ),
    ] {
        for target in ["/api/cobs", detail.as_str()] {
            let response = browser.response(method, target, body, role);
            assert_eq!(response.status, status);
            assert_eq!((*member(&value(&response), "code")), code);
            fixture.assert_private(&response);
        }
    }
    for (target, status, code) in [
        ("/api/cobs?path=/etc/passwd", 400, "query_invalid"),
        (
            "/api/cobs?repository=onix&repository=other",
            400,
            "query_invalid",
        ),
        ("/api/cobs?q=%ZZ", 400, "query_invalid"),
        ("/api/cobs?q=%FF", 400, "query_invalid"),
        ("/api/cobs?q=%00", 400, "query_invalid"),
        ("/api/cobs?repository=..%2Fonix", 400, "repository_invalid"),
        ("/api/cobs?type=org.example.other", 400, "cob_type_invalid"),
        ("/api/cobs?limit=0", 400, "page_limit_invalid"),
        ("/api/cobs?limit=101", 400, "page_limit_invalid"),
        ("/api/cobs?limit=two", 400, "page_limit_invalid"),
        ("/api/cobs/../object", 404, "route_missing"),
        ("/api/cobs/%2e%2e/object", 404, "route_missing"),
        ("/api/cobs/object", 400, "cob_selection_invalid"),
        (
            "/api/cobs/object?repository=onix",
            400,
            "cob_selection_invalid",
        ),
    ] {
        rejects(&browser, target, status, code);
        fixture.assert_private(&browser.response("GET", target, &[], Role::Viewer));
    }
    rejects(
        &browser,
        &format!("{detail}&q=ignored"),
        400,
        "query_invalid",
    );
    rejects(
        &browser,
        &format!("/api/cobs?q={}", "x".repeat(257)),
        400,
        "query_invalid",
    );
    for id in [
        "0".repeat(39),
        "g".repeat(40),
        "A".repeat(40),
        "../private-store".into(),
    ] {
        rejects(
            &browser,
            &format!(
                "/api/cobs/object?repository=onix&type={}&object={id}",
                object.type_name
            ),
            400,
            "cob_selection_invalid",
        );
    }
    rejects(
        &browser,
        &detail_target(object.type_name, object.left),
        404,
        "cob_object_missing",
    );
    let other_type = fixture
        .objects
        .get(1)
        .unwrap_or_else(|| unreachable!("fixture object absent"))
        .type_name;
    rejects(
        &browser,
        &detail_target(other_type, object.root),
        404,
        "cob_object_missing",
    );
    assert_eq!(refs(&fixture.repository), before_refs);
    assert_eq!(repository_files(fixture.repository.path()), before);
}

#[test]
fn malformed_native_stores_are_explicit_not_false_empty_or_substituted_objects() {
    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let kind = INITIAL_CAIRN_TYPES
        .first()
        .copied()
        .unwrap_or_else(|| unreachable!("fixture type absent"));
    for case in [
        "wrong-type",
        "wrong-version",
        "noncommit-tip",
        "invalid-ref-id",
        "bad-json",
        "action-gap",
        "action-overflow",
        "unrelated-tip",
    ] {
        let repository = Repository::init_bare(root.path().join(case))
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        let actions = [json!({"type":"create","title":"Fixture"})];
        let root_id = cob_operation(&repository, kind, "Root", &actions, &[]);
        let mut stored = entries(kind, &actions);
        match case {
            "wrong-type" => {
                stored
                    .first_mut()
                    .unwrap_or_else(|| unreachable!("fixture manifest absent"))
                    .1 = serde_json::to_vec(&json!({"typeName":INITIAL_CAIRN_TYPES[1],"version":1}))
                    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
            }
            "wrong-version" => {
                stored
                    .first_mut()
                    .unwrap_or_else(|| unreachable!("fixture manifest absent"))
                    .1 = serde_json::to_vec(&json!({"typeName":kind,"version":2}))
                    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
            }
            "bad-json" => {
                stored
                    .get_mut(1)
                    .unwrap_or_else(|| unreachable!("fixture action absent"))
                    .1 = b"{broken".to_vec()
            }
            "action-gap" => {
                stored
                    .get_mut(1)
                    .unwrap_or_else(|| unreachable!("fixture action absent"))
                    .0 = "1".into()
            }
            // Retain valid action 0: ignoring an overflowing numeric name must not succeed.
            "action-overflow" => stored.push(("256".into(), b"{\"type\":\"comment\"}".to_vec())),
            _ => {}
        }
        let borrowed: Vec<_> = stored
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.clone()))
            .collect();
        let parents = if case == "unrelated-tip" {
            vec![]
        } else {
            vec![root_id]
        };
        let mut tip = operation(&repository, &borrowed, "Update", &parents);
        if case == "noncommit-tip" {
            tip = repository
                .blob(b"not a commit")
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        }
        let ref_id = if case == "invalid-ref-id" {
            "not-an-object".into()
        } else {
            root_id.to_string()
        };
        repository
            .reference(&format!("refs/cobs/{kind}/{ref_id}"), tip, false, "fixture")
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        let before = repository_files(repository.path());
        let browser = browser(vec![policy("onix", NATIVE_ID, repository.path())]);
        let inventory = get(&browser, "/api/cobs");
        if matches!(
            case,
            "bad-json" | "action-gap" | "action-overflow" | "unrelated-tip"
        ) {
            assert_eq!((*member(&inventory, "complete")), true, "{case}");
            assert_eq!((*member(&inventory, "total")), 1, "{case}");
        } else {
            assert_eq!((*member(&inventory, "complete")), false, "{case}");
            assert_eq!(
                (*member(member(member(&inventory, "sources"), 0), "status")),
                "unavailable",
                "{case}"
            );
            assert_eq!(
                (*member(member(member(&inventory, "sources"), 0), "objects")),
                Value::Null,
                "{case}"
            );
        }
        rejects(
            &browser,
            &detail_target(kind, root_id),
            503,
            "cob_store_unavailable",
        );
        assert!(
            !serde_json::to_string(&inventory)
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
                .contains(
                    root.path()
                        .to_str()
                        .unwrap_or_else(|| unreachable!("fixture value absent"))
                )
        );
        assert_eq!(repository_files(repository.path()), before, "{case}");
    }
}

#[test]
fn native_action_operation_and_tip_bounds_fail_closed_without_mutation() {
    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let kind = INITIAL_CAIRN_TYPES
        .first()
        .copied()
        .unwrap_or_else(|| unreachable!("fixture type absent"));
    for case in ["action-bytes", "operation-count", "ref-tips"] {
        let repository = Repository::init_bare(root.path().join(case))
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        let action = json!({"type":"create"});
        let root_id = cob_operation(
            &repository,
            kind,
            "Root",
            std::slice::from_ref(&action),
            &[],
        );
        let tip = match case {
            "action-bytes" => {
                let oversized = json!({"body":"x".repeat(262_144)});
                cob_operation(
                    &repository,
                    kind,
                    "Oversized action",
                    &[oversized],
                    &[root_id],
                )
            }
            "operation-count" => {
                let mut tip = root_id;
                // Root plus 128 descendants exceeds the admitted 128-operation history.
                for index in 0..128 {
                    tip = cob_operation(
                        &repository,
                        kind,
                        &format!("Update {index}"),
                        std::slice::from_ref(&action),
                        &[tip],
                    );
                }
                tip
            }
            "ref-tips" => {
                for index in 0..128 {
                    repository
                        .reference(
                            &format!("refs/namespaces/peer-{index}/refs/cobs/{kind}/{root_id}"),
                            root_id,
                            false,
                            "fixture",
                        )
                        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
                }
                root_id
            }
            _ => unreachable!(),
        };
        repository
            .reference(
                &format!("refs/cobs/{kind}/{root_id}"),
                tip,
                false,
                "fixture",
            )
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        let before = repository_files(repository.path());
        let before_refs = refs(&repository);
        let browser = browser(vec![policy("onix", NATIVE_ID, repository.path())]);
        rejects(
            &browser,
            &detail_target(kind, root_id),
            503,
            "cob_store_unavailable",
        );
        let inventory = get(&browser, "/api/cobs");
        if case == "ref-tips" {
            assert_eq!((*member(&inventory, "complete")), false);
            assert_eq!(
                (*member(member(member(&inventory, "sources"), 0), "status")),
                "unavailable"
            );
        } else {
            assert_eq!((*member(&inventory, "complete")), true);
            assert_eq!((*member(&inventory, "total")), 1);
        }
        assert_eq!(refs(&repository), before_refs, "{case}");
        assert_eq!(repository_files(repository.path()), before, "{case}");
    }
}

// r[verify kiln.dashboard.cobs.read.scenario.present]
#[test]
fn radicle_observation_types_render_with_author_and_ignore_unregistered_types() {
    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let repository = Repository::init_bare(root.path().join("radicle-store.git"))
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let mut expected = Vec::new();
    for type_name in ["xyz.radicle.issue", "xyz.radicle.patch", "xyz.radworks.job"] {
        let message = format!("{type_name} example\n");
        let id = cob_operation(
            &repository,
            type_name,
            &message,
            &[json!({"type":"create","title":"example"})],
            &[],
        );
        repository
            .reference(&format!("refs/cobs/{type_name}/{id}"), id, false, "fixture")
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        expected.push((type_name, id));
    }
    // A native type outside the observation registry is skipped, never rejected.
    let identity = cob_operation(
        &repository,
        "xyz.radicle.id",
        "repository identity\n",
        &[json!({"type":"init"})],
        &[],
    );
    repository
        .reference(
            &format!("refs/cobs/xyz.radicle.id/{identity}"),
            identity,
            false,
            "fixture",
        )
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let before_refs = refs(&repository);
    let browser = browser(vec![policy("radicle", "rad:zNative", repository.path())]);
    let inventory = get(&browser, "/api/cobs");
    assert_eq!((*member(&inventory, "complete")), true);
    assert_eq!((*member(&inventory, "total")), 3);
    let rows = (*member(&inventory, "objects"))
        .as_array()
        .unwrap_or_else(|| unreachable!("fixture value absent"));
    for (type_name, id) in &expected {
        let row = rows
            .iter()
            .find(|row| (*member(row, "object_id")) == id.to_string())
            .unwrap_or_else(|| unreachable!("fixture value absent"));
        assert_eq!((*member(row, "type_name")), *type_name);
        assert_eq!(
            (*member(row, "author")),
            "COB fixture <cob@example.invalid>"
        );
        assert_eq!((*member(row, "recorded_at_ms")), RECORDED_AT_MS);
        assert_eq!((*member(row, "excerpt")), "example");
    }
    assert!(
        !rows
            .iter()
            .any(|row| (*member(row, "type_name")) == "xyz.radicle.id")
    );
    let (_, patch_object) = expected
        .iter()
        .find(|(type_name, _)| *type_name == "xyz.radicle.patch")
        .unwrap_or_else(|| unreachable!("fixture value absent"));
    let detail = get(
        &browser,
        &format!(
            "/api/cobs/object?repository=radicle&type=xyz.radicle.patch&object={patch_object}"
        ),
    );
    assert_eq!((*member(member(&detail, "object"), "operation_count")), 1);
    rejects(
        &browser,
        "/api/cobs?type=xyz.radicle.id",
        400,
        "cob_type_invalid",
    );
    let searched = get(&browser, "/api/cobs?q=cob%40example.invalid");
    assert_eq!((*member(&searched, "total")), 3);
    assert_eq!(refs(&repository), before_refs);
}

// r[verify kiln.dashboard.cobs.presentation]
#[test]
fn root_action_titles_are_searchable_without_reducing_later_edits() {
    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let repository = Repository::init_bare(root.path().join("titles.git"))
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let kind = "xyz.radicle.issue";
    let initial = cob_operation(
        &repository,
        kind,
        "Create issue\n",
        &[
            json!({"type":"comment","body":"Recorded description\nwith another line"}),
            json!({"type":"edit","title":"Second-action title"}),
        ],
        &[],
    );
    let update = cob_operation(
        &repository,
        kind,
        "Edit issue\n",
        &[json!({"type":"edit","title":"Later title, not reduced"})],
        &[initial],
    );
    repository
        .reference(
            &format!("refs/cobs/{kind}/{initial}"),
            update,
            false,
            "fixture",
        )
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let browser = browser(vec![policy("onix", NATIVE_ID, repository.path())]);
    let inventory = get(&browser, "/api/cobs?q=second-action");
    assert_eq!(*member(&inventory, "total"), 1);
    let row = member(member(&inventory, "objects"), 0);
    assert_eq!(*member(row, "title"), "Second-action title");
    assert_eq!(
        *member(row, "excerpt"),
        "Recorded description with another line"
    );
    assert_eq!(
        *member(&get(&browser, "/api/cobs?q=Later%20title"), "total"),
        0
    );
    let detail = get(&browser, &detail_target(kind, initial));
    let object = member(&detail, "object");
    let operations = member(object, "operations")
        .as_array()
        .unwrap_or_else(|| unreachable!());
    let initial_id = initial.to_string();
    let root_operation = operations
        .iter()
        .find(|op| member(op, "id").as_str() == Some(initial_id.as_str()))
        .unwrap_or_else(|| unreachable!());
    assert_eq!(
        *member(
            member(member(member(root_operation, "actions"), 0), "value"),
            "body"
        ),
        "Recorded description\nwith another line"
    );
    assert_eq!(
        *member(
            member(member(member(root_operation, "actions"), 1), "value"),
            "title"
        ),
        "Second-action title"
    );
    assert_eq!(*member(object, "reduced_state"), Value::Null);
}

// r[verify kiln.dashboard.cobs.presentation]
#[test]
fn nested_root_titles_and_metadata_fallback_remain_bounded() {
    let root =
        tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let repository = Repository::init_bare(root.path().join("metadata.git"))
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    let kind = "xyz.onixresearch.rfd";
    let cases = [
        (
            json!({"type":"init","state":{"artifacts":{"title":"Nested document title"}}}),
            "Nested document title",
        ),
        (
            json!({"type":"init","state":{"document_id":"rfd-recorded-identity"}}),
            "rfd-recorded-identity",
        ),
        (json!({"title":42}), "Root summary"),
        (json!(null), "Root summary"),
        (json!({"title":"   "}), "Root summary"),
        (
            json!({"title":"must not be read","body":"x".repeat(8193)}),
            "Root summary",
        ),
    ];
    for (action, expected) in cases {
        let initial = cob_operation(&repository, kind, "Root summary\n", &[action], &[]);
        repository
            .reference(
                &format!("refs/cobs/{kind}/{initial}"),
                initial,
                false,
                "fixture",
            )
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        let browser = browser(vec![policy("onix", NATIVE_ID, repository.path())]);
        let detail = get(&browser, &detail_target(kind, initial));
        assert_eq!(
            *member(member(member(&detail, "object"), "identity"), "title"),
            expected
        );
    }
}
