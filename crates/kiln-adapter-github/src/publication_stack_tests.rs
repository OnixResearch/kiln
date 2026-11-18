//! A two-layer stack published through native Git and the loopback GitHub
//! fixture: first run, re-entry and refusals.
use super::tests::{serve, token};
use super::*;
use kiln_core::implementation::{StackLayerPlan, layer_branch};
use std::os::unix::fs::symlink;

type Response = (&'static str, String);

struct Captured {
    _directory: tempfile::TempDir,
    plan: StackPlan,
    source: PublicationSourceProfile,
    remote: PathBuf,
}

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn commit_files(repository: &Repository, files: &[&str], parent: Option<Oid>) -> (Oid, Oid) {
    let mut builder = repository
        .treebuilder(None)
        .unwrap_or_else(|_| unreachable!());
    for file in files {
        let blob = repository
            .blob(file.as_bytes())
            .unwrap_or_else(|_| unreachable!());
        builder
            .insert(file, blob, 0o100_644)
            .unwrap_or_else(|_| unreachable!());
    }
    let tree_oid = builder.write().unwrap_or_else(|_| unreachable!());
    let tree = repository
        .find_tree(tree_oid)
        .unwrap_or_else(|_| unreachable!());
    let signature = git2::Signature::now("Kiln fixture", "fixture@example.invalid")
        .unwrap_or_else(|_| unreachable!());
    let parents: Vec<git2::Commit<'_>> = parent
        .into_iter()
        .map(|oid| {
            repository
                .find_commit(oid)
                .unwrap_or_else(|_| unreachable!())
        })
        .collect();
    let parents: Vec<&git2::Commit<'_>> = parents.iter().collect();
    let commit = repository
        .commit(
            None,
            &signature,
            &signature,
            &files.join(","),
            &tree,
            &parents,
        )
        .unwrap_or_else(|_| unreachable!());
    (commit, tree_oid)
}

/// A captured export whose history is base, lower, upper. When `linear` is
/// false the upper commit is built on the base, so it skips the lower layer.
fn captured(linear: bool) -> Captured {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let remote = directory.path().join("remote.git");
    let _remote = Repository::init_bare(&remote).unwrap_or_else(|_| unreachable!());
    let work = content('1');
    let attempt = content('2');
    let fence = 3;
    let export_root = directory.path().join("candidate-export");
    let capture = export_root.join(format!("{}-{fence}/candidate", attempt.as_str()));
    std::fs::create_dir_all(&capture).unwrap_or_else(|_| unreachable!());
    let repository = Repository::init(&capture).unwrap_or_else(|_| unreachable!());
    let (base, _) = commit_files(&repository, &["base"], None);
    let (lower, lower_tree) = commit_files(&repository, &["base", "lower"], Some(base));
    let upper_parent = if linear { lower } else { base };
    let (upper, upper_tree) =
        commit_files(&repository, &["base", "lower", "upper"], Some(upper_parent));
    repository
        .reference("refs/heads/main", upper, true, "captured stack")
        .unwrap_or_else(|_| unreachable!());
    let remote_url = remote.to_string_lossy().into_owned();
    repository
        .remote("origin", &remote_url)
        .unwrap_or_else(|_| unreachable!());
    drop(repository);
    symlink(
        capture.parent().unwrap_or_else(|| unreachable!()),
        export_root.join("current"),
    )
    .unwrap_or_else(|_| unreachable!());
    let git = |kind: &str, oid: Oid| {
        ExternalRef::new("git", kind, &oid.to_string()).unwrap_or_else(|_| unreachable!())
    };
    let layer = |position: u32, candidate: Oid, tree: Oid, parent: Oid, base_branch: String| {
        StackLayerPlan {
            position,
            candidate: git("commit", candidate),
            tree: git("tree", tree),
            parent: git("commit", parent),
            verification: content('5'),
            branch: layer_branch(&work, &attempt, position),
            base_branch,
            title: format!("Layer {position}"),
            body: "Verified layer".to_string(),
        }
    };
    let plan = StackPlan {
        schema: STACK_SCHEMA.to_string(),
        work_identity: work.clone(),
        work_revision: 1,
        attempt: attempt.clone(),
        fence,
        handoff: content('3'),
        verification: content('4'),
        repository: "octocat/hello-world".to_string(),
        base: git("commit", base),
        trunk_branch: "main".to_string(),
        layers: vec![
            layer(1, lower, lower_tree, base, "main".to_string()),
            layer(
                2,
                upper,
                upper_tree,
                lower,
                layer_branch(&work, &attempt, 1),
            ),
        ],
    };
    let remote_profile = NativeRemoteProfile {
        remote_name: "origin".into(),
        remote_url,
        transport: NativeTransport::LocalBareFixture,
        ssh_host_key_sha256: None,
    };
    let source = PublicationSourceProfile {
        schema: PUBLICATION_SOURCE_PROFILE_SCHEMA.into(),
        repository_path: export_root.join("current/candidate"),
        remote_identity: remote_identity(&remote_profile).unwrap_or_else(|_| unreachable!()),
        remote: remote_profile,
        fixture: true,
    };
    Captured {
        _directory: directory,
        plan,
        source,
        remote,
    }
}

fn layer(plan: &StackPlan, index: usize) -> &StackLayerPlan {
    plan.layers.get(index).unwrap_or_else(|| unreachable!())
}

fn layer_pull(plan: &StackPlan, index: usize, number: u64) -> serde_json::Value {
    let layer = layer(plan, index);
    serde_json::json!({
        "number": number,
        "state": "open",
        "merged": false,
        "html_url": format!("https://github.com/octocat/hello-world/pull/{number}"),
        "head": {"ref": layer.branch, "sha": layer.candidate.native_id.as_str()},
        "base": {"ref": layer.base_branch}
    })
}

fn listed(plan: &StackPlan, index: usize, number: u64) -> Response {
    (
        "200 OK",
        serde_json::json!([layer_pull(plan, index, number)]).to_string(),
    )
}

fn stack_body(plan: &StackPlan, numbers: [u64; 2]) -> serde_json::Value {
    let members: Vec<serde_json::Value> = plan
        .layers
        .iter()
        .zip(numbers)
        .map(|(layer, number)| {
            serde_json::json!({
                "number": number,
                "state": "open",
                "draft": false,
                "merged_at": serde_json::Value::Null,
                "head": {"ref": layer.branch, "sha": layer.candidate.native_id.as_str()}
            })
        })
        .collect();
    serde_json::json!({"number": 5, "base": {"ref": "main"}, "open": true, "pull_requests": members})
}

fn empty() -> Response {
    ("200 OK", "[]".to_string())
}

fn remote_head(remote: &Path, branch: &str) -> Option<String> {
    let repository = Repository::open_bare(remote).ok()?;
    let reference = repository
        .find_reference(&format!("refs/heads/{branch}"))
        .ok()?;
    reference.target().map(|oid| oid.to_string())
}

fn publish(
    captured: &Captured,
    responses: Vec<Response>,
) -> (PortResult<StackPublication>, Vec<String>) {
    let (endpoint, server) = serve(responses);
    let client = GithubPublicationClient::new_fixture(token(), 5000, &endpoint)
        .unwrap_or_else(|_| unreachable!());
    let result = client.publish_stack(&captured.plan, &captured.source);
    (result, server.join().unwrap_or_else(|_| unreachable!()))
}

fn outcome(result: &PortResult<StackPublication>) -> String {
    match result {
        Ok(_) => "ok".to_string(),
        Err(PortFailure::Domain(diagnostic)) => format!("refused:{}", diagnostic.code),
        Err(PortFailure::Unknown(failure)) => format!("unknown:{}", failure.code),
        Err(other) => format!("{other:?}"),
    }
}

#[test]
// r[verify kiln.github.stack] r[verify kiln.github.stack.scenario.publish]
fn publishes_a_two_layer_stack_once_and_reenters_without_mutation() {
    let captured = captured(true);
    let plan = &captured.plan;
    let (first, requests) = publish(
        &captured,
        vec![
            empty(),
            empty(),
            empty(),
            ("201 Created", layer_pull(plan, 0, 11).to_string()),
            ("201 Created", layer_pull(plan, 1, 12).to_string()),
            empty(),
            ("201 Created", stack_body(plan, [11, 12]).to_string()),
        ],
    );
    let first = first.unwrap_or_else(|error| unreachable!("{error:?}"));
    assert_eq!(
        first.provider_stack.native_id.as_str(),
        "octocat/hello-world#5"
    );
    let posts: Vec<&String> = requests
        .iter()
        .filter(|request| request.starts_with("POST "))
        .collect();
    assert_eq!(posts.len(), 3);
    assert!(posts.get(1).is_some_and(|request| {
        request.contains(&format!("\"base\":\"{}\"", layer(plan, 0).branch))
    }));
    assert!(posts.get(2).is_some_and(|request| {
        request.starts_with("POST /repos/octocat/hello-world/stacks ")
            && request.contains("{\"pull_requests\":[11,12]}")
    }));
    for index in 0..2 {
        assert_eq!(
            remote_head(&captured.remote, &layer(plan, index).branch).as_deref(),
            Some(layer(plan, index).candidate.native_id.as_str())
        );
    }

    let exact = serde_json::json!([stack_body(plan, [11, 12])]).to_string();
    let (second, requests) = publish(
        &captured,
        vec![
            ("200 OK", exact.clone()),
            listed(plan, 0, 11),
            listed(plan, 1, 12),
            ("200 OK", exact),
        ],
    );
    assert_eq!(second.ok(), Some(first));
    assert!(requests.iter().all(|request| request.starts_with("GET ")));
}

#[test]
// r[verify kiln.github.stack.scenario.refused]
fn unsupported_repository_and_inexact_layers_refuse_before_any_mutation() {
    let unsupported = captured(true);
    let (result, requests) = publish(&unsupported, vec![("404 Not Found", "{}".to_string())]);
    assert_eq!(outcome(&result), "refused:github_stack_unsupported");
    assert_eq!(requests.len(), 1);

    let skipped = captured(false);
    let (result, requests) = publish(&skipped, vec![empty()]);
    assert_eq!(outcome(&result), "refused:github_stack_ancestry");
    assert_eq!(requests.len(), 1);

    let mut wrong_tree = captured(true);
    let upper_tree = layer(&wrong_tree.plan, 1).tree.clone();
    if let Some(lower) = wrong_tree.plan.layers.first_mut() {
        lower.tree = upper_tree;
    }
    let (result, _) = publish(&wrong_tree, vec![empty()]);
    assert_eq!(outcome(&result), "refused:github_publication_tree_conflict");

    // A foreign commit already holds the upper branch: refuse before the
    // lower branch is pushed.
    let conflicted = captured(true);
    let upper_branch = layer(&conflicted.plan, 1).branch.clone();
    let remote = Repository::open_bare(&conflicted.remote).unwrap_or_else(|_| unreachable!());
    let (foreign, _) = commit_files(&remote, &["foreign"], None);
    remote
        .reference(
            &format!("refs/heads/{upper_branch}"),
            foreign,
            false,
            "foreign",
        )
        .unwrap_or_else(|_| unreachable!());
    let (result, requests) = publish(&conflicted, vec![empty(), empty(), empty()]);
    assert_eq!(
        outcome(&result),
        "refused:github_publication_branch_conflict"
    );
    assert!(requests.iter().all(|request| request.starts_with("GET ")));
    assert_eq!(
        remote_head(&conflicted.remote, &layer(&conflicted.plan, 0).branch),
        None
    );
    assert_eq!(
        remote_head(&conflicted.remote, &upper_branch),
        Some(foreign.to_string())
    );

    // After a lower merge GitHub retargets the upper pull request to the
    // trunk. Matching by head alone finds it, so it is refused, not recreated.
    let retargeted = captured(true);
    let upper = layer(&retargeted.plan, 1);
    let moved = serde_json::json!([{
        "number": 12,
        "state": "open",
        "merged": false,
        "html_url": "https://github.com/octocat/hello-world/pull/12",
        "head": {"ref": upper.branch, "sha": upper.candidate.native_id.as_str()},
        "base": {"ref": "main"}
    }])
    .to_string();
    let (result, requests) = publish(
        &retargeted,
        vec![empty(), listed(&retargeted.plan, 0, 11), ("200 OK", moved)],
    );
    assert_eq!(outcome(&result), "refused:github_publication_pr_conflict");
    assert!(
        requests
            .iter()
            .all(|request| request.starts_with("GET ") && !request.contains("&base="))
    );
    for refused in [&unsupported, &skipped, &wrong_tree, &retargeted] {
        for index in 0..2 {
            assert_eq!(
                remote_head(&refused.remote, &layer(&refused.plan, index).branch),
                None
            );
        }
    }
}

#[test]
// r[verify kiln.github.stack.scenario.refused]
fn a_different_stack_is_refused_and_a_lost_creation_is_reconciled_once() {
    let captured = captured(true);
    let plan = &captured.plan;
    let (result, requests) = publish(
        &captured,
        vec![
            empty(),
            listed(plan, 0, 11),
            listed(plan, 1, 12),
            (
                "200 OK",
                serde_json::json!([stack_body(plan, [11, 13])]).to_string(),
            ),
        ],
    );
    assert_eq!(outcome(&result), "refused:github_stack_conflict");
    assert!(requests.iter().all(|request| request.starts_with("GET ")));

    for (status, body, reconciled, expected) in [
        (
            "201 Created",
            "lost or truncated body".to_string(),
            "[]".to_string(),
            "unknown:github_stack_unknown",
        ),
        (
            "409 Conflict",
            "{}".to_string(),
            serde_json::json!([stack_body(plan, [11, 12])]).to_string(),
            "ok",
        ),
    ] {
        let (result, requests) = publish(
            &captured,
            vec![
                empty(),
                listed(plan, 0, 11),
                listed(plan, 1, 12),
                empty(),
                (status, body),
                ("200 OK", reconciled),
            ],
        );
        assert_eq!(outcome(&result), expected);
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.starts_with("POST "))
                .count(),
            1
        );
        assert!(
            requests
                .last()
                .is_some_and(|request| request.contains("stacks?pull_request=11"))
        );
    }
}
