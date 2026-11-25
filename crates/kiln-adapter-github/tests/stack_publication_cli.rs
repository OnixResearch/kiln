use std::collections::BTreeSet;
#[cfg(feature = "protocol-fixtures")]
use std::io::{Read, Write};
#[cfg(feature = "protocol-fixtures")]
use std::net::TcpListener;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
#[cfg(feature = "protocol-fixtures")]
use std::sync::Arc;
#[cfg(feature = "protocol-fixtures")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "protocol-fixtures")]
use std::thread::JoinHandle;
#[cfg(feature = "protocol-fixtures")]
use std::time::Duration;

use git2::{Oid, Repository};
use kiln_adapter_git::native_publication::{NativeRemoteProfile, NativeTransport, remote_identity};
use kiln_adapter_github::publication::PUBLICATION_SOURCE_PROFILE_SCHEMA;
use kiln_core::change_review::{CiCheck, CiFact};
use kiln_core::implementation::{
    AcceptanceObligation, CANDIDATE_HANDOFF_SCHEMA, CandidateEvidence, CandidateHandoff,
    CandidateVerification, Enrollment, EvidenceKind, EvidenceOutcome, IMPLEMENTATION_WORK_SCHEMA,
    ImplementationWork, LAYER_VERIFICATION_SCHEMA, LayerVerification, PUBLICATION_SCHEMA,
    PublicationPlan, STACK_SCHEMA, SourceRef, StackLayerPlan, StackPlan, TerminalPolicy,
    VERIFICATION_SCHEMA, WorkBudget, WorkSourceKind, acceptance_digest, admit_candidate_handoff,
    admit_candidate_verification, admit_layer_verification, branch_for, layer_branch,
};
use kiln_core::types::{ContentId, ExternalRef};
use kiln_implementer::Implementer;
#[cfg(feature = "protocol-fixtures")]
use kiln_implementer::stack_publication::StackEffectKind;
use kiln_implementer::stack_publication::StackPublicationInput;

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn write_bytes(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap_or_else(|error| unreachable!("{error}"));
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) {
    write_bytes(
        path,
        &serde_json::to_vec_pretty(value).unwrap_or_else(|error| unreachable!("{error}")),
    );
}

fn commit_files(repository: &Repository, files: &[&str], parents: &[Oid]) -> (Oid, Oid) {
    let mut builder = repository
        .treebuilder(None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    for file in files {
        let blob = repository
            .blob(file.as_bytes())
            .unwrap_or_else(|error| unreachable!("{error}"));
        builder
            .insert(file, blob, 0o100_644)
            .unwrap_or_else(|error| unreachable!("{error}"));
    }
    let tree_oid = builder
        .write()
        .unwrap_or_else(|error| unreachable!("{error}"));
    let tree = repository
        .find_tree(tree_oid)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let signature = git2::Signature::new(
        "Kiln fixture",
        "fixture@example.invalid",
        &git2::Time::new(1_700_000_000, 0),
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    let parent_commits: Vec<git2::Commit<'_>> = parents
        .iter()
        .map(|oid| {
            repository
                .find_commit(*oid)
                .unwrap_or_else(|error| unreachable!("{error}"))
        })
        .collect();
    let parent_refs: Vec<&git2::Commit<'_>> = parent_commits.iter().collect();
    let commit = repository
        .commit(
            None,
            &signature,
            &signature,
            &files.join(","),
            &tree,
            &parent_refs,
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    (commit, tree_oid)
}

#[cfg(feature = "protocol-fixtures")]
fn read_http_request(stream: &mut std::net::TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    let header_end = loop {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            return String::from_utf8_lossy(&buffer).into_owned();
        }
        if let Some(slice) = chunk.get(..read) {
            buffer.extend_from_slice(slice);
        }
        if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
    };
    let head = String::from_utf8_lossy(buffer.get(..header_end).unwrap_or_default()).to_string();
    let content_length: usize = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().ok())
                .flatten()
        })
        .unwrap_or(0);
    while buffer.len() < header_end + content_length {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            break;
        }
        if let Some(slice) = chunk.get(..read) {
            buffer.extend_from_slice(slice);
        }
    }
    String::from_utf8_lossy(&buffer).into_owned()
}

#[cfg(feature = "protocol-fixtures")]
struct LoopbackServer {
    endpoint: String,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<Vec<String>>>,
}

#[cfg(feature = "protocol-fixtures")]
impl LoopbackServer {
    fn start(handler: impl Fn(&str) -> (&'static str, String) + Send + Sync + 'static) -> Self {
        let listener =
            TcpListener::bind("127.0.0.1:0").unwrap_or_else(|error| unreachable!("{error}"));
        listener
            .set_nonblocking(true)
            .unwrap_or_else(|error| unreachable!("{error}"));
        let address = listener
            .local_addr()
            .unwrap_or_else(|error| unreachable!("{error}"));
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            let mut requests = Vec::new();
            while !stop_flag.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        let request = read_http_request(&mut stream);
                        let (status, body) = handler(&request);
                        requests.push(request);
                        let head = format!(
                            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(head.as_bytes());
                        let _ = stream.write_all(body.as_bytes());
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
            requests
        });
        Self {
            endpoint: format!("http://{address}"),
            stop,
            handle: Some(handle),
        }
    }

    fn finish(mut self) -> Vec<String> {
        self.stop.store(true, Ordering::Release);
        self.handle
            .take()
            .map(|handle| handle.join().unwrap_or_else(|_| unreachable!()))
            .unwrap_or_default()
    }
}

#[cfg(feature = "protocol-fixtures")]
impl Drop for LoopbackServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg_attr(not(feature = "protocol-fixtures"), expect(dead_code))]
struct VerifiedStackFixture {
    _dir: tempfile::TempDir,
    database: PathBuf,
    enrollment_file: PathBuf,
    secret_file: PathBuf,
    remote_profile_file: PathBuf,
    publication_file: PathBuf,
    stack_file: PathBuf,
    remote_repo: PathBuf,
    work: ImplementationWork,
    attempt_id: ContentId,
    fence: u64,
    base_oid: Oid,
    lower_oid: Oid,
    lower_tree_oid: Oid,
    top_oid: Oid,
    top_tree_oid: Oid,
    stack_input: StackPublicationInput,
    single_plan: PublicationPlan,
}

fn setup_verified_fixture() -> VerifiedStackFixture {
    let dir = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let database = dir.path().join("work.sqlite");
    let enrollment_file = dir.path().join("enrollment.json");
    let secret_file = dir.path().join("secret");
    let remote_profile_file = dir.path().join("remote-profile.json");
    let publication_file = dir.path().join("publication.json");
    let stack_file = dir.path().join("stack.json");
    let remote_repo = dir.path().join("remote.git");
    Repository::init_bare(&remote_repo).unwrap_or_else(|error| unreachable!("{error}"));

    write_bytes(&secret_file, b"fixture-github-token");
    let enrollment = Enrollment {
        repositories: BTreeSet::from(["octocat/hello-world".to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    write_json(&enrollment_file, &enrollment);

    let seed_repo_dir = dir.path().join("seed-repo");
    let seed_repo =
        Repository::init(&seed_repo_dir).unwrap_or_else(|error| unreachable!("{error}"));
    let (base_oid, _) = commit_files(&seed_repo, &["base"], &[]);
    let (lower_oid, lower_tree_oid) = commit_files(&seed_repo, &["base", "lower"], &[base_oid]);
    let (top_oid, top_tree_oid) =
        commit_files(&seed_repo, &["base", "lower", "upper"], &[lower_oid]);

    let git = |kind: &str, oid: Oid| {
        ExternalRef::new("git", kind, &oid.to_string()).unwrap_or_else(|_| unreachable!())
    };
    let work = ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
        revision: 1,
        actor: "operator".to_string(),
        repository: "octocat/hello-world".to_string(),
        project: "kiln".to_string(),
        base: Some(git("commit", base_oid)),
        title: "Ordered stack CLI".to_string(),
        requested_outcome: "implement".to_string(),
        acceptance: vec![AcceptanceObligation {
            id: "accepted".to_string(),
            statement: "verified".to_string(),
        }],
        dependencies: Vec::new(),
        permitted_paths: vec!["crates".to_string()],
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        verification_profile: content('a'),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedIntegrated,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::GitHubIssue,
            source: ExternalRef::new("github", "issue", "octocat/hello-world#89")
                .unwrap_or_else(|_| unreachable!()),
            native_id: "octocat/hello-world#89".to_string(),
            version: content('1'),
        }],
    };
    let mut implementer =
        Implementer::open_with_local_operator(&database, enrollment, "test-operator")
            .unwrap_or_else(|error| unreachable!("{error}"));
    implementer
        .admit(&work, None)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let now_ms = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| unreachable!())
            .as_millis(),
    )
    .unwrap_or_else(|_| unreachable!());
    let attempt = implementer
        .claim_next(now_ms, 600_000)
        .unwrap_or_else(|error| unreachable!("{error}"))
        .unwrap_or_else(|| unreachable!("claim"));
    implementer
        .reserve_dispatch(
            &attempt.identity,
            attempt.fence,
            now_ms,
            &content('d'),
            &kiln_core::types::ProviderProfileId::new("lattice-implementation")
                .unwrap_or_else(|_| unreachable!()),
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    implementer
        .start_execution(
            &attempt.identity,
            attempt.fence,
            now_ms,
            &ExternalRef::new("lattice", "run", "one").unwrap_or_else(|_| unreachable!()),
        )
        .unwrap_or_else(|error| unreachable!("{error}"));

    let export_root = dir.path().join("candidate-export");
    let capture_dir = export_root.join(format!(
        "{}-{}/candidate",
        attempt.identity.as_str(),
        attempt.fence
    ));
    std::fs::create_dir_all(&capture_dir).unwrap_or_else(|error| unreachable!("{error}"));
    let capture_repo =
        Repository::init(&capture_dir).unwrap_or_else(|error| unreachable!("{error}"));
    let (cap_base, _) = commit_files(&capture_repo, &["base"], &[]);
    let (cap_lower, cap_lower_tree) = commit_files(&capture_repo, &["base", "lower"], &[cap_base]);
    let (cap_top, cap_top_tree) =
        commit_files(&capture_repo, &["base", "lower", "upper"], &[cap_lower]);
    assert_eq!(cap_base, base_oid);
    assert_eq!(cap_lower, lower_oid);
    assert_eq!(cap_lower_tree, lower_tree_oid);
    assert_eq!(cap_top, top_oid);
    assert_eq!(cap_top_tree, top_tree_oid);
    let remote_url = remote_repo.to_string_lossy().into_owned();
    capture_repo
        .remote("origin", &remote_url)
        .unwrap_or_else(|error| unreachable!("{error}"));
    drop(capture_repo);
    drop(seed_repo);
    symlink(
        capture_dir.parent().unwrap_or_else(|| unreachable!()),
        export_root.join("current"),
    )
    .unwrap_or_else(|error| unreachable!("{error}"));

    let work_identity = work.identity().unwrap_or_else(|_| unreachable!());
    let handoff = CandidateHandoff {
        schema: CANDIDATE_HANDOFF_SCHEMA.to_string(),
        work_identity: work_identity.clone(),
        work_revision: work.revision,
        attempt_identity: attempt.identity.clone(),
        fence: attempt.fence,
        repository: work.repository.clone(),
        base: work.base.clone(),
        candidate: git("commit", top_oid),
        tree: git("tree", top_tree_oid),
        changed_paths: vec!["crates/src/lib.rs".to_string()],
        acceptance_digest: acceptance_digest(&work).unwrap_or_else(|_| unreachable!()),
        verification_profile: work.verification_profile.clone(),
        toolchain: content('7'),
        environment: content('8'),
        evidence: vec![CandidateEvidence {
            identity: content('e'),
            kind: EvidenceKind::Check,
            outcome: EvidenceOutcome::Passed,
            reference: ExternalRef::new("kiln", "run", "candidate-check")
                .unwrap_or_else(|_| unreachable!()),
        }],
    };
    implementer
        .admit_candidate_handoff(
            &work,
            &attempt.identity,
            attempt.fence,
            now_ms + 1,
            &handoff,
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    implementer
        .begin_verification(&attempt.identity, attempt.fence, now_ms + 2)
        .unwrap_or_else(|error| unreachable!("{error}"));
    let handoff_identity =
        admit_candidate_handoff(&work, &attempt, &handoff).unwrap_or_else(|_| unreachable!());
    let ci = |rev: Oid| CiFact {
        project: work.project.clone(),
        repository: work.repository.clone(),
        revision: rev.to_string(),
        run_id: format!("run-{rev}"),
        ordinal: 1,
        success: true,
        full_run: true,
        clean: true,
        scope_identity: None,
        terminal: true,
        untrusted: None,
        checks: vec![CiCheck {
            name: "test".to_string(),
            kind: "test".to_string(),
            outcome: "pass".to_string(),
            artifact_digest: Some("f".repeat(64)),
        }],
    };
    let verification = CandidateVerification {
        schema: VERIFICATION_SCHEMA.to_string(),
        handoff: handoff_identity.clone(),
        work_identity: work_identity.clone(),
        work_revision: work.revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        candidate: handoff.candidate.clone(),
        tree: handoff.tree.clone(),
        base: handoff.base.clone(),
        verification_profile: work.verification_profile.clone(),
        run: ci(top_oid),
    };
    implementer
        .record_verification(
            &work,
            &attempt.identity,
            attempt.fence,
            now_ms + 3,
            &handoff,
            &verification,
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    let verified_attempt = implementer
        .attempt(&attempt.identity)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    let lower_verification = LayerVerification {
        schema: LAYER_VERIFICATION_SCHEMA.to_string(),
        handoff: handoff_identity.clone(),
        work_identity: work_identity.clone(),
        work_revision: work.revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        position: 1,
        candidate: git("commit", lower_oid),
        tree: git("tree", lower_tree_oid),
        parent: git("commit", base_oid),
        verification_profile: work.verification_profile.clone(),
        run: ci(lower_oid),
    };
    let lower_id =
        admit_layer_verification(&work, &verified_attempt, &handoff, &lower_verification)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
    let top_id = admit_candidate_verification(&work, &verified_attempt, &handoff, &verification)
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let branch1 = layer_branch(&work_identity, &attempt.identity, 1);
    let branch2 = layer_branch(&work_identity, &attempt.identity, 2);
    let stack_plan = StackPlan {
        schema: STACK_SCHEMA.to_string(),
        work_identity: work_identity.clone(),
        work_revision: work.revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        handoff: handoff_identity.clone(),
        verification: top_id.clone(),
        repository: work.repository.clone(),
        base: git("commit", base_oid),
        trunk_branch: "main".to_string(),
        layers: vec![
            StackLayerPlan {
                position: 1,
                candidate: git("commit", lower_oid),
                tree: git("tree", lower_tree_oid),
                parent: git("commit", base_oid),
                verification: lower_id,
                branch: branch1.clone(),
                base_branch: "main".to_string(),
                title: "Layer 1".to_string(),
                body: "First layer".to_string(),
            },
            StackLayerPlan {
                position: 2,
                candidate: git("commit", top_oid),
                tree: git("tree", top_tree_oid),
                parent: git("commit", lower_oid),
                verification: top_id.clone(),
                branch: branch2,
                base_branch: branch1,
                title: "Layer 2".to_string(),
                body: "Second layer".to_string(),
            },
        ],
    };
    let stack_input = StackPublicationInput {
        plan: stack_plan,
        layer_verifications: vec![lower_verification],
    };
    write_json(&stack_file, &stack_input);

    let single_plan = PublicationPlan {
        schema: PUBLICATION_SCHEMA.to_string(),
        work_identity: work_identity.clone(),
        work_revision: work.revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        handoff: handoff_identity,
        verification: top_id,
        repository: work.repository.clone(),
        base: git("commit", base_oid),
        base_branch: "main".to_string(),
        candidate: git("commit", top_oid),
        tree: git("tree", top_tree_oid),
        branch: branch_for(&work_identity, &attempt.identity),
        title: "Single PR".to_string(),
        body: "Fallback single PR".to_string(),
    };
    write_json(&publication_file, &single_plan);

    let remote_profile = NativeRemoteProfile {
        remote_name: "origin".into(),
        remote_url,
        transport: NativeTransport::LocalBareFixture,
        ssh_host_key_sha256: None,
    };
    let remote_id = remote_identity(&remote_profile).unwrap_or_else(|_| unreachable!());
    write_json(
        &remote_profile_file,
        &serde_json::json!({
            "schema": PUBLICATION_SOURCE_PROFILE_SCHEMA,
            "repository_path": export_root.join("current/candidate"),
            "remote": remote_profile,
            "remote_identity": remote_id,
            "fixture": true,
        }),
    );

    VerifiedStackFixture {
        _dir: dir,
        database,
        enrollment_file,
        secret_file,
        remote_profile_file,
        publication_file,
        stack_file,
        remote_repo,
        work,
        attempt_id: attempt.identity,
        fence: attempt.fence,
        base_oid,
        lower_oid,
        lower_tree_oid,
        top_oid,
        top_tree_oid,
        stack_input,
        single_plan,
    }
}

fn run_cli(args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_kiln-implementer"));
    cmd.args(args);
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd.output().unwrap_or_else(|error| unreachable!("{error}"))
}

#[test]
fn publish_route_origin_stack_is_refused_pre_effect() {
    let fixture = setup_verified_fixture();
    let db = fixture.database.to_string_lossy();
    let enroll = fixture.enrollment_file.to_string_lossy();
    let stack = fixture.stack_file.to_string_lossy();
    let out = run_cli(
        &[
            "publish",
            "--route",
            "origin_stack",
            "--local-operator",
            "test-operator",
            "--database",
            &db,
            "--enrollment",
            &enroll,
            "--stack-file",
            &stack,
        ],
        &[],
    );
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("implementation_stack_destination_unsupported"),
        "{stderr}"
    );
    assert!(out.stdout.is_empty());
}

#[cfg(feature = "protocol-fixtures")]
#[test]
fn default_pull_request_route_and_github_stack_404_fallback_publish_single_pr() {
    // 1. Default route (no --route flag) publishes single PR unchanged.
    {
        let fixture = setup_verified_fixture();
        let db = fixture.database.to_string_lossy();
        let enroll = fixture.enrollment_file.to_string_lossy();
        let pub_path = fixture.publication_file.to_string_lossy();
        let secret = fixture.secret_file.to_string_lossy();
        let remote = fixture.remote_profile_file.to_string_lossy();

        let prep = run_cli(
            &[
                "prepare-publication",
                "--local-operator",
                "test-operator",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--publication-file",
                &pub_path,
            ],
            &[],
        );
        assert!(
            prep.status.success(),
            "{}",
            String::from_utf8_lossy(&prep.stderr)
        );

        let branch = fixture.single_plan.branch.clone();
        let sha = fixture.top_oid.to_string();
        let server = LoopbackServer::start(move |req| {
            let pr = serde_json::json!({
                "number": 41,
                "state": "open",
                "merged": false,
                "html_url": "https://github.com/octocat/hello-world/pull/41",
                "head": {"ref": branch, "sha": sha},
                "base": {"ref": "main"}
            });
            if req.starts_with("POST ") {
                ("201 Created", pr.to_string())
            } else if req.contains("pulls?state=all") && !req.contains("after_create") {
                ("200 OK", serde_json::json!([pr]).to_string())
            } else {
                ("200 OK", "[]".to_string())
            }
        });
        let env = [(
            "KILN_GITHUB_FIXTURE_PUBLICATION_ENDPOINT",
            server.endpoint.as_str(),
        )];
        let published = run_cli(
            &[
                "publish",
                "--local-operator",
                "test-operator",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--publication-file",
                &pub_path,
                "--secret-file",
                &secret,
                "--remote-profile",
                &remote,
            ],
            &env,
        );
        assert!(
            published.status.success(),
            "{}",
            String::from_utf8_lossy(&published.stderr)
        );
        let json: serde_json::Value =
            serde_json::from_slice(&published.stdout).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            json.pointer("/observed/pull_request/pull_request/native_id")
                .and_then(serde_json::Value::as_str),
            Some("octocat/hello-world#41")
        );
        let _ = server.finish();
    }

    // 2. --route github_stack on a 404 host falls back cleanly to single PR when prepared.
    {
        let fixture = setup_verified_fixture();
        let db = fixture.database.to_string_lossy();
        let enroll = fixture.enrollment_file.to_string_lossy();
        let pub_path = fixture.publication_file.to_string_lossy();
        let secret = fixture.secret_file.to_string_lossy();
        let remote = fixture.remote_profile_file.to_string_lossy();

        let prep = run_cli(
            &[
                "prepare-publication",
                "--local-operator",
                "test-operator",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--publication-file",
                &pub_path,
            ],
            &[],
        );
        assert!(prep.status.success());

        let branch = fixture.single_plan.branch.clone();
        let sha = fixture.top_oid.to_string();
        let pr_created = Arc::new(AtomicBool::new(false));
        let pr_created_flag = Arc::clone(&pr_created);
        let server = LoopbackServer::start(move |req| {
            if req.contains("/stacks") {
                return ("404 Not Found", "{}".to_string());
            }
            let pr = serde_json::json!({
                "number": 42,
                "state": "open",
                "merged": false,
                "html_url": "https://github.com/octocat/hello-world/pull/42",
                "head": {"ref": branch, "sha": sha},
                "base": {"ref": "main"}
            });
            if req.starts_with("POST ") {
                pr_created_flag.store(true, Ordering::Release);
                ("201 Created", pr.to_string())
            } else if pr_created_flag.load(Ordering::Acquire) {
                ("200 OK", serde_json::json!([pr]).to_string())
            } else {
                ("200 OK", "[]".to_string())
            }
        });
        let env = [(
            "KILN_GITHUB_FIXTURE_PUBLICATION_ENDPOINT",
            server.endpoint.as_str(),
        )];
        let fallback = run_cli(
            &[
                "publish",
                "--route",
                "github_stack",
                "--local-operator",
                "test-operator",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--publication-file",
                &pub_path,
                "--secret-file",
                &secret,
                "--remote-profile",
                &remote,
            ],
            &env,
        );
        assert!(
            fallback.status.success(),
            "{}",
            String::from_utf8_lossy(&fallback.stderr)
        );
        let json: serde_json::Value =
            serde_json::from_slice(&fallback.stdout).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            json.pointer("/observed/pull_request/pull_request/native_id")
                .and_then(serde_json::Value::as_str),
            Some("octocat/hello-world#42")
        );
        let requests = server.finish();
        assert!(requests.iter().any(|r| r.contains("/stacks")));
        assert!(
            !requests
                .iter()
                .any(|r| r.starts_with("POST /repos/octocat/hello-world/stacks"))
        );
    }

    // 3. --route github_stack on a 404 host refuses cleanly when no single-PR plan was prepared.
    {
        let fixture = setup_verified_fixture();
        let db = fixture.database.to_string_lossy();
        let enroll = fixture.enrollment_file.to_string_lossy();
        let stack = fixture.stack_file.to_string_lossy();
        let secret = fixture.secret_file.to_string_lossy();
        let remote = fixture.remote_profile_file.to_string_lossy();

        assert!(
            run_cli(
                &[
                    "prepare-stack-publication",
                    "--local-operator",
                    "test-operator",
                    "--database",
                    &db,
                    "--enrollment",
                    &enroll,
                    "--stack-file",
                    &stack,
                ],
                &[],
            )
            .status
            .success()
        );
        let server = LoopbackServer::start(|_| ("404 Not Found", "{}".to_string()));
        let env = [(
            "KILN_GITHUB_FIXTURE_PUBLICATION_ENDPOINT",
            server.endpoint.as_str(),
        )];
        let refused = run_cli(
            &[
                "publish",
                "--route",
                "github_stack",
                "--local-operator",
                "test-operator",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--stack-file",
                &stack,
                "--secret-file",
                &secret,
                "--remote-profile",
                &remote,
            ],
            &env,
        );
        assert!(!refused.status.success());
        assert!(
            String::from_utf8_lossy(&refused.stderr).contains("github_stack_unsupported"),
            "{}",
            String::from_utf8_lossy(&refused.stderr)
        );
        let _ = server.finish();
    }
}

#[cfg(feature = "protocol-fixtures")]
#[test]
fn two_layer_stack_publish_reentry_crash_reconcile_integration_and_gate_e2e() {
    let fixture = setup_verified_fixture();
    let db = fixture.database.to_string_lossy().into_owned();
    let enroll = fixture.enrollment_file.to_string_lossy().into_owned();
    let stack = fixture.stack_file.to_string_lossy().into_owned();
    let secret = fixture.secret_file.to_string_lossy().into_owned();
    let remote = fixture.remote_profile_file.to_string_lossy().into_owned();

    // 1. prepare-stack-publication
    let prep = run_cli(
        &[
            "prepare-stack-publication",
            "--local-operator",
            "test-operator",
            "--database",
            &db,
            "--enrollment",
            &enroll,
            "--stack-file",
            &stack,
        ],
        &[],
    );
    assert!(
        prep.status.success(),
        "{}",
        String::from_utf8_lossy(&prep.stderr)
    );

    let layer0 = fixture
        .stack_input
        .plan
        .layers
        .first()
        .unwrap_or_else(|| unreachable!())
        .clone();
    let layer1 = fixture
        .stack_input
        .plan
        .layers
        .get(1)
        .unwrap_or_else(|| unreachable!())
        .clone();
    let pr1 = serde_json::json!({
        "number": 11,
        "state": "open",
        "merged": false,
        "html_url": "https://github.com/octocat/hello-world/pull/11",
        "head": {"ref": layer0.branch, "sha": layer0.candidate.native_id.as_str()},
        "base": {"ref": layer0.base_branch}
    });
    let pr2 = serde_json::json!({
        "number": 12,
        "state": "open",
        "merged": false,
        "html_url": "https://github.com/octocat/hello-world/pull/12",
        "head": {"ref": layer1.branch, "sha": layer1.candidate.native_id.as_str()},
        "base": {"ref": layer1.base_branch}
    });
    let stack_json = serde_json::json!({
        "number": 5,
        "base": {"ref": "main"},
        "open": true,
        "pull_requests": [
            {"number": 11, "state": "open", "draft": false, "merged_at": null, "head": {"ref": layer0.branch, "sha": layer0.candidate.native_id.as_str()}},
            {"number": 12, "state": "open", "draft": false, "merged_at": null, "head": {"ref": layer1.branch, "sha": layer1.candidate.native_id.as_str()}},
        ]
    });

    // 2. publish --route github_stack
    let created_pr1 = Arc::new(AtomicBool::new(false));
    let created_pr2 = Arc::new(AtomicBool::new(false));
    let created_stack = Arc::new(AtomicBool::new(false));
    let pub_server = {
        let pr1 = pr1.clone();
        let pr2 = pr2.clone();
        let stack_json = stack_json.clone();
        let b1 = layer0.branch.clone();
        let b2 = layer1.branch.clone();
        let enc1 = b1.replace('/', "%2F");
        let enc2 = b2.replace('/', "%2F");
        let c1 = Arc::clone(&created_pr1);
        let c2 = Arc::clone(&created_pr2);
        let cs = Arc::clone(&created_stack);
        LoopbackServer::start(move |req| {
            if req.starts_with("GET /repos/octocat/hello-world/stacks?per_page=1") {
                return ("200 OK", "[]".to_string());
            }
            if req.starts_with("GET /repos/octocat/hello-world/pulls?") {
                if (req.contains(&b2) || req.contains(&enc2)) && c2.load(Ordering::Acquire) {
                    return ("200 OK", serde_json::json!([pr2]).to_string());
                }
                if (req.contains(&b1) || req.contains(&enc1)) && c1.load(Ordering::Acquire) {
                    return ("200 OK", serde_json::json!([pr1]).to_string());
                }
                return ("200 OK", "[]".to_string());
            }
            if req.starts_with("POST /repos/octocat/hello-world/pulls ") {
                if req.contains(&b2) {
                    c2.store(true, Ordering::Release);
                    return ("201 Created", pr2.to_string());
                }
                c1.store(true, Ordering::Release);
                return ("201 Created", pr1.to_string());
            }
            if req.starts_with("GET /repos/octocat/hello-world/stacks?pull_request=11") {
                if cs.load(Ordering::Acquire) {
                    return ("200 OK", serde_json::json!([stack_json]).to_string());
                }
                return ("200 OK", "[]".to_string());
            }
            if req.starts_with("POST /repos/octocat/hello-world/stacks ") {
                cs.store(true, Ordering::Release);
                return ("201 Created", stack_json.to_string());
            }
            ("404 Not Found", "{}".to_string())
        })
    };
    let pub_env = [(
        "KILN_GITHUB_FIXTURE_PUBLICATION_ENDPOINT",
        pub_server.endpoint.as_str(),
    )];
    let published = run_cli(
        &[
            "publish",
            "--route",
            "github_stack",
            "--local-operator",
            "test-operator",
            "--database",
            &db,
            "--enrollment",
            &enroll,
            "--stack-file",
            &stack,
            "--secret-file",
            &secret,
            "--remote-profile",
            &remote,
        ],
        &pub_env,
    );
    assert!(
        published.status.success(),
        "{}",
        String::from_utf8_lossy(&published.stderr)
    );
    let pub_out: serde_json::Value =
        serde_json::from_slice(&published.stdout).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        pub_out
            .pointer("/observed/publish/provider_stack/native_id")
            .and_then(serde_json::Value::as_str),
        Some("octocat/hello-world#5")
    );
    let pub_requests = pub_server.finish();
    assert_eq!(
        pub_requests
            .iter()
            .filter(|r| r.starts_with("POST "))
            .count(),
        3
    );

    // 3. Idempotent re-entry issues zero HTTP requests.
    let idle_server = LoopbackServer::start(|_| ("500 Error", "{}".to_string()));
    let idle_env = [(
        "KILN_GITHUB_FIXTURE_PUBLICATION_ENDPOINT",
        idle_server.endpoint.as_str(),
    )];
    let reentered = run_cli(
        &[
            "publish-stack",
            "--local-operator",
            "test-operator",
            "--database",
            &db,
            "--enrollment",
            &enroll,
            "--stack-file",
            &stack,
            "--secret-file",
            &secret,
            "--remote-profile",
            &remote,
        ],
        &idle_env,
    );
    assert!(
        reentered.status.success(),
        "{}",
        String::from_utf8_lossy(&reentered.stderr)
    );
    assert_eq!(idle_server.finish().len(), 0);

    // 4. Crash recovery (`publish-stack` when `stack-effect:<attempt>:publish` was reserved
    //    with `observed: None`) reconciles with GET-only requests and zero POSTs.
    {
        let conn = rusqlite::Connection::open(&fixture.database)
            .unwrap_or_else(|error| unreachable!("{error}"));
        let key = format!("stack-effect:{}:publish", fixture.attempt_id.as_str());
        let bytes: Vec<u8> = conn
            .query_row(
                "SELECT bytes FROM implementation_state WHERE kind = ?1",
                rusqlite::params![&key],
                |row| row.get(0),
            )
            .unwrap_or_else(|error| unreachable!("{error}"));
        let mut res: Option<kiln_implementer::stack_publication::StackEffectReservation> =
            serde_json::from_slice(&bytes).unwrap_or_else(|_| unreachable!());
        if let Some(inner) = res.as_mut() {
            inner.observed = None;
        }
        conn.execute(
            "UPDATE implementation_state SET bytes = ?1 WHERE kind = ?2",
            rusqlite::params![
                serde_json::to_vec(&res).unwrap_or_else(|_| unreachable!()),
                &key
            ],
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
        conn.execute(
            "DELETE FROM implementation_state WHERE kind = ?1",
            rusqlite::params![format!("stack-publication:{}", fixture.attempt_id.as_str())],
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    }
    let reconcile_server = {
        let pr1 = pr1.clone();
        let pr2 = pr2.clone();
        let stack_json = stack_json.clone();
        let b1 = layer0.branch.clone();
        let b2 = layer1.branch.clone();
        let enc1 = b1.replace('/', "%2F");
        let enc2 = b2.replace('/', "%2F");
        LoopbackServer::start(move |req| {
            if req.starts_with("GET /repos/octocat/hello-world/stacks?per_page=1") {
                return ("200 OK", "[]".to_string());
            }
            if req.starts_with("GET /repos/octocat/hello-world/pulls?") {
                if req.contains(&b2) || req.contains(&enc2) {
                    return ("200 OK", serde_json::json!([pr2]).to_string());
                }
                if req.contains(&b1) || req.contains(&enc1) {
                    return ("200 OK", serde_json::json!([pr1]).to_string());
                }
            }
            if req.starts_with("GET /repos/octocat/hello-world/stacks?pull_request=11") {
                return ("200 OK", serde_json::json!([stack_json]).to_string());
            }
            ("500 Unexpected Mutation", "{}".to_string())
        })
    };
    let rec_env = [(
        "KILN_GITHUB_FIXTURE_PUBLICATION_ENDPOINT",
        reconcile_server.endpoint.as_str(),
    )];
    let reconciled = run_cli(
        &[
            "publish-stack",
            "--local-operator",
            "test-operator",
            "--database",
            &db,
            "--enrollment",
            &enroll,
            "--stack-file",
            &stack,
            "--secret-file",
            &secret,
            "--remote-profile",
            &remote,
        ],
        &rec_env,
    );
    assert!(
        reconciled.status.success(),
        "{}",
        String::from_utf8_lossy(&reconciled.stderr)
    );
    let rec_requests = reconcile_server.finish();
    assert!(!rec_requests.is_empty());
    assert!(rec_requests.iter().all(|req| req.starts_with("GET ")));

    // 5. recover-stack-integration (layer 1 merged only, then both layers merged) + stack-gate
    let bare =
        Repository::open_bare(&fixture.remote_repo).unwrap_or_else(|error| unreachable!("{error}"));
    let (merge1, _) = commit_files(
        &bare,
        &["base", "lower"],
        &[fixture.base_oid, fixture.lower_oid],
    );
    let (merge2, _) = commit_files(
        &bare,
        &["base", "lower", "upper"],
        &[merge1, fixture.top_oid],
    );

    let lower_sha = fixture.lower_oid.to_string();
    let lower_tree = fixture.lower_tree_oid.to_string();
    let top_sha = fixture.top_oid.to_string();
    let top_tree = fixture.top_tree_oid.to_string();
    let base_sha = fixture.base_oid.to_string();
    let m1_sha = merge1.to_string();
    let m2_sha = merge2.to_string();

    let partial_server = {
        let lower_sha = lower_sha.clone();
        let lower_tree = lower_tree.clone();
        let top_sha = top_sha.clone();
        let base_sha = base_sha.clone();
        let m1_sha = m1_sha.clone();
        let layer1_base = layer1.base_branch.clone();
        LoopbackServer::start(move |req| {
            if req.starts_with("GET /repos/octocat/hello-world/pulls/11 ") {
                return (
                    "200 OK",
                    serde_json::json!({
                        "number": 11,
                        "user": {"login": "author"},
                        "state": "closed",
                        "merged": true,
                        "merge_commit_sha": m1_sha,
                        "mergeable": null,
                        "head": {"sha": lower_sha},
                        "base": {"ref": "main"}
                    })
                    .to_string(),
                );
            }
            if req.starts_with("GET /repos/octocat/hello-world/pulls/12 ") {
                return (
                    "200 OK",
                    serde_json::json!({
                        "number": 12,
                        "user": {"login": "author"},
                        "state": "open",
                        "merged": false,
                        "merge_commit_sha": null,
                        "mergeable": true,
                        "head": {"sha": top_sha},
                        "base": {"ref": layer1_base}
                    })
                    .to_string(),
                );
            }
            if req.contains(&format!("/git/commits/{m1_sha} ")) {
                return (
                    "200 OK",
                    serde_json::json!({
                        "sha": m1_sha,
                        "tree": {"sha": lower_tree},
                        "parents": [{"sha": base_sha}, {"sha": lower_sha}]
                    })
                    .to_string(),
                );
            }
            if req.contains(&format!("/git/commits/{lower_sha} ")) {
                return (
                    "200 OK",
                    serde_json::json!({
                        "sha": lower_sha,
                        "tree": {"sha": lower_tree},
                        "parents": [{"sha": base_sha}]
                    })
                    .to_string(),
                );
            }
            if req.contains("/git/ref/heads/main ") {
                return (
                    "200 OK",
                    serde_json::json!({"object": {"sha": m1_sha}}).to_string(),
                );
            }
            ("404 Not Found", "{}".to_string())
        })
    };
    let partial_env = [(
        "KILN_GITHUB_FIXTURE_LANDING_ENDPOINT",
        partial_server.endpoint.as_str(),
    )];
    let partial = run_cli(
        &[
            "recover-stack-integration",
            "--local-operator",
            "test-operator",
            "--database",
            &db,
            "--enrollment",
            &enroll,
            "--stack-file",
            &stack,
            "--secret-file",
            &secret,
        ],
        &partial_env,
    );
    assert!(
        partial.status.success(),
        "{}",
        String::from_utf8_lossy(&partial.stderr)
    );
    let partial_json: serde_json::Value =
        serde_json::from_slice(&partial.stdout).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        partial_json
            .get("integrated_layers")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        partial_json
            .get("complete")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    let _ = partial_server.finish();

    // 6. stack-gate observes layer 1 as Integrated and layer 2 (failing check) as Blocked.
    let landing_plans_file = fixture._dir.path().join("landing-plans.json");
    let landing_plans = serde_json::json!([
        {
            "schema": "kiln.github-landing-plan.v1",
            "owner": "octocat",
            "repository": "hello-world",
            "pull_number": 11,
            "base_ref": "main",
            "base_head": base_sha,
            "head_sha": lower_sha,
            "required_checks": ["test"],
            "required_approvals": 0,
            "operation": content('c')
        },
        {
            "schema": "kiln.github-landing-plan.v1",
            "owner": "octocat",
            "repository": "hello-world",
            "pull_number": 12,
            "base_ref": layer1.base_branch,
            "base_head": lower_sha,
            "head_sha": top_sha,
            "required_checks": ["test"],
            "required_approvals": 0,
            "operation": content('d')
        }
    ]);
    write_json(&landing_plans_file, &landing_plans);
    let landing_path = landing_plans_file.to_string_lossy().into_owned();

    let gate_server = {
        let top_sha = top_sha.clone();
        let lower_sha = lower_sha.clone();
        let base_ref = layer1.base_branch.clone();
        LoopbackServer::start(move |req| {
            if req.starts_with("GET /repos/octocat/hello-world/pulls/12 ") {
                return (
                    "200 OK",
                    serde_json::json!({
                        "number": 12,
                        "user": {"login": "author"},
                        "state": "open",
                        "merged": false,
                        "mergeable": true,
                        "head": {"sha": top_sha},
                        "base": {"ref": base_ref}
                    })
                    .to_string(),
                );
            }
            if req.contains("/check-runs") {
                return (
                    "200 OK",
                    serde_json::json!({
                        "total_count": 1,
                        "check_runs": [{
                            "name": "test",
                            "head_sha": top_sha,
                            "status": "completed",
                            "conclusion": "failure"
                        }]
                    })
                    .to_string(),
                );
            }
            if req.contains("/reviews") {
                return ("200 OK", "[]".to_string());
            }
            if req.contains("/git/ref/heads/") {
                return (
                    "200 OK",
                    serde_json::json!({"object": {"sha": lower_sha}}).to_string(),
                );
            }
            ("404 Not Found", "{}".to_string())
        })
    };
    let gate_env = [(
        "KILN_GITHUB_FIXTURE_LANDING_ENDPOINT",
        gate_server.endpoint.as_str(),
    )];
    // Notice `stack-gate` is ObservationOnly and works even without `--local-operator`!
    let gated = run_cli(
        &[
            "stack-gate",
            "--database",
            &db,
            "--enrollment",
            &enroll,
            "--stack-file",
            &stack,
            "--landing-plans",
            &landing_path,
            "--secret-file",
            &secret,
        ],
        &gate_env,
    );
    assert!(
        gated.status.success(),
        "{}",
        String::from_utf8_lossy(&gated.stderr)
    );
    let gates: serde_json::Value =
        serde_json::from_slice(&gated.stdout).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        gates.pointer("/0/gate").and_then(serde_json::Value::as_str),
        Some("integrated")
    );
    assert_eq!(
        gates.pointer("/1/gate").and_then(serde_json::Value::as_str),
        Some("blocked")
    );
    assert_eq!(
        gates.pointer("/1/code").and_then(serde_json::Value::as_str),
        Some("github_landing_check_failed")
    );
    let _ = gate_server.finish();

    // 7. Complete stack integration (both layers merged), then idempotent replay.
    let complete_server = LoopbackServer::start(move |req| {
        if req.starts_with("GET /repos/octocat/hello-world/pulls/11 ") {
            return (
                "200 OK",
                serde_json::json!({
                    "number": 11,
                    "user": {"login": "author"},
                    "state": "closed",
                    "merged": true,
                    "merge_commit_sha": m1_sha,
                    "mergeable": null,
                    "head": {"sha": lower_sha},
                    "base": {"ref": "main"}
                })
                .to_string(),
            );
        }
        if req.starts_with("GET /repos/octocat/hello-world/pulls/12 ") {
            return (
                "200 OK",
                serde_json::json!({
                    "number": 12,
                    "user": {"login": "author"},
                    "state": "closed",
                    "merged": true,
                    "merge_commit_sha": m2_sha,
                    "mergeable": null,
                    "head": {"sha": top_sha},
                    "base": {"ref": "main"}
                })
                .to_string(),
            );
        }
        if req.contains(&format!("/git/commits/{m1_sha} ")) {
            return (
                "200 OK",
                serde_json::json!({
                    "sha": m1_sha,
                    "tree": {"sha": lower_tree},
                    "parents": [{"sha": base_sha}, {"sha": lower_sha}]
                })
                .to_string(),
            );
        }
        if req.contains(&format!("/git/commits/{lower_sha} ")) {
            return (
                "200 OK",
                serde_json::json!({
                    "sha": lower_sha,
                    "tree": {"sha": lower_tree},
                    "parents": [{"sha": base_sha}]
                })
                .to_string(),
            );
        }
        if req.contains(&format!("/git/commits/{m2_sha} ")) {
            return (
                "200 OK",
                serde_json::json!({
                    "sha": m2_sha,
                    "tree": {"sha": top_tree},
                    "parents": [{"sha": m1_sha}, {"sha": top_sha}]
                })
                .to_string(),
            );
        }
        if req.contains(&format!("/git/commits/{top_sha} ")) {
            return (
                "200 OK",
                serde_json::json!({
                    "sha": top_sha,
                    "tree": {"sha": top_tree},
                    "parents": [{"sha": lower_sha}]
                })
                .to_string(),
            );
        }
        if req.contains("/git/ref/heads/main ") {
            return (
                "200 OK",
                serde_json::json!({"object": {"sha": m2_sha}}).to_string(),
            );
        }
        ("404 Not Found", "{}".to_string())
    });
    let complete_env = [(
        "KILN_GITHUB_FIXTURE_LANDING_ENDPOINT",
        complete_server.endpoint.as_str(),
    )];
    let complete = run_cli(
        &[
            "recover-stack-integration",
            "--local-operator",
            "test-operator",
            "--database",
            &db,
            "--enrollment",
            &enroll,
            "--stack-file",
            &stack,
            "--secret-file",
            &secret,
        ],
        &complete_env,
    );
    assert!(
        complete.status.success(),
        "{}",
        String::from_utf8_lossy(&complete.stderr)
    );
    let complete_json: serde_json::Value =
        serde_json::from_slice(&complete.stdout).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        complete_json
            .get("integrated_layers")
            .and_then(serde_json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        complete_json
            .get("complete")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    let _ = complete_server.finish();

    // Retained complete stack integration never calls landing endpoint again.
    let no_http = LoopbackServer::start(|_| ("500 Error", "{}".to_string()));
    let no_http_env = [(
        "KILN_GITHUB_FIXTURE_LANDING_ENDPOINT",
        no_http.endpoint.as_str(),
    )];
    let replay = run_cli(
        &[
            "recover-stack-integration",
            "--local-operator",
            "test-operator",
            "--database",
            &db,
            "--enrollment",
            &enroll,
            "--stack-file",
            &stack,
            "--secret-file",
            &secret,
        ],
        &no_http_env,
    );
    assert!(replay.status.success());
    assert_eq!(no_http.finish().len(), 0);

    // Confirm durable readback via Implementer.
    let mut reader = Implementer::open(
        &fixture.database,
        Enrollment {
            repositories: BTreeSet::from(["octocat/hello-world".to_string()]),
            actors: BTreeSet::from(["operator".to_string()]),
        },
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    let retained = reader
        .stack_publication_record(&fixture.attempt_id)
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
    assert!(retained.publication.is_some());
    assert_eq!(
        retained.integration.as_ref().map(|i| i.layers.len()),
        Some(2)
    );
    assert!(
        reader
            .read_stack_effect(
                &fixture.work,
                &fixture.attempt_id,
                fixture.fence,
                StackEffectKind::Publish
            )
            .unwrap_or_else(|_| unreachable!())
            .is_some()
    );
    assert!(
        reader
            .read_stack_effect(
                &fixture.work,
                &fixture.attempt_id,
                fixture.fence,
                StackEffectKind::Integration
            )
            .unwrap_or_else(|_| unreachable!())
            .is_some()
    );
}

fn supersede_and_claim_newer_attempt(fixture: &VerifiedStackFixture) {
    let enrollment = Enrollment {
        repositories: BTreeSet::from(["octocat/hello-world".to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    let mut implementer =
        Implementer::open_with_local_operator(&fixture.database, enrollment, "test-operator")
            .unwrap_or_else(|error| unreachable!("{error}"));
    let mut work2 = fixture.work.clone();
    work2.revision = 2;
    work2.requested_outcome = "superseded".to_string();
    let now_ms = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| unreachable!())
            .as_millis(),
    )
    .unwrap_or_else(|_| unreachable!());
    implementer
        .admit_with_supersession(
            &work2,
            None,
            kiln_core::implementation::SupersessionRule::AbortActive,
            now_ms,
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    let newer = implementer
        .claim_next(now_ms, 600_000)
        .unwrap_or_else(|error| unreachable!("{error}"))
        .unwrap_or_else(|| unreachable!("newer attempt claim"));
    assert!(newer.fence > fixture.fence);
}

fn write_absent_authority_config(dir: &Path) -> PathBuf {
    let config = dir.join("absent-authority.json");
    write_json(
        &config,
        &serde_json::json!({
            "schema": "kiln.implementation-authority-config.v1",
            "profile": "absent",
        }),
    );
    config
}

#[test]
fn publish_route_github_stack_refuses_absent_authority_before_reading_secret() {
    // 1. Prepared --stack-file with omitted authority flags and explicit `"profile": "absent"`.
    {
        let fixture = setup_verified_fixture();
        let db = fixture.database.to_string_lossy();
        let enroll = fixture.enrollment_file.to_string_lossy();
        let stack = fixture.stack_file.to_string_lossy();
        let remote = fixture.remote_profile_file.to_string_lossy();
        let missing_secret = fixture._dir.path().join("missing-secret");
        let missing_secret_str = missing_secret.to_string_lossy();

        let prep = run_cli(
            &[
                "prepare-stack-publication",
                "--local-operator",
                "test-operator",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--stack-file",
                &stack,
            ],
            &[],
        );
        assert!(
            prep.status.success(),
            "{}",
            String::from_utf8_lossy(&prep.stderr)
        );

        let omitted = run_cli(
            &[
                "publish",
                "--route",
                "github_stack",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--stack-file",
                &stack,
                "--secret-file",
                &missing_secret_str,
                "--remote-profile",
                &remote,
            ],
            &[],
        );
        assert!(!omitted.status.success());
        assert!(omitted.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&omitted.stderr);
        assert!(stderr.contains("no authority profile selected"), "{stderr}");
        assert!(!stderr.contains("secret file is unreadable"), "{stderr}");

        let absent_cfg = write_absent_authority_config(fixture._dir.path());
        let absent_cfg_str = absent_cfg.to_string_lossy();
        let explicit = run_cli(
            &[
                "publish",
                "--route",
                "github_stack",
                "--authority-config",
                &absent_cfg_str,
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--stack-file",
                &stack,
                "--secret-file",
                &missing_secret_str,
                "--remote-profile",
                &remote,
            ],
            &[],
        );
        assert!(!explicit.status.success());
        assert!(explicit.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&explicit.stderr);
        assert!(stderr.contains("no authority profile selected"), "{stderr}");
        assert!(!stderr.contains("secret file is unreadable"), "{stderr}");
    }

    // 2. Prepared --publication-file only with omitted authority flags and `"profile": "absent"`.
    {
        let fixture = setup_verified_fixture();
        let db = fixture.database.to_string_lossy();
        let enroll = fixture.enrollment_file.to_string_lossy();
        let pub_path = fixture.publication_file.to_string_lossy();
        let remote = fixture.remote_profile_file.to_string_lossy();
        let missing_secret = fixture._dir.path().join("missing-secret");
        let missing_secret_str = missing_secret.to_string_lossy();

        let prep = run_cli(
            &[
                "prepare-publication",
                "--local-operator",
                "test-operator",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--publication-file",
                &pub_path,
            ],
            &[],
        );
        assert!(
            prep.status.success(),
            "{}",
            String::from_utf8_lossy(&prep.stderr)
        );

        let omitted = run_cli(
            &[
                "publish",
                "--route",
                "github_stack",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--publication-file",
                &pub_path,
                "--secret-file",
                &missing_secret_str,
                "--remote-profile",
                &remote,
            ],
            &[],
        );
        assert!(!omitted.status.success());
        assert!(omitted.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&omitted.stderr);
        assert!(stderr.contains("no authority profile selected"), "{stderr}");
        assert!(!stderr.contains("secret file is unreadable"), "{stderr}");

        let absent_cfg = write_absent_authority_config(fixture._dir.path());
        let absent_cfg_str = absent_cfg.to_string_lossy();
        let explicit = run_cli(
            &[
                "publish",
                "--route",
                "github_stack",
                "--authority-config",
                &absent_cfg_str,
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--publication-file",
                &pub_path,
                "--secret-file",
                &missing_secret_str,
                "--remote-profile",
                &remote,
            ],
            &[],
        );
        assert!(!explicit.status.success());
        assert!(explicit.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&explicit.stderr);
        assert!(stderr.contains("no authority profile selected"), "{stderr}");
        assert!(!stderr.contains("secret file is unreadable"), "{stderr}");
    }
}

#[test]
fn publish_route_github_stack_refuses_stale_fence_and_superseded_work_before_reading_secret() {
    // 1. Prepared --stack-file + mismatched fence+1 AND superseded work + newer claim.
    {
        let fixture = setup_verified_fixture();
        let db = fixture.database.to_string_lossy();
        let enroll = fixture.enrollment_file.to_string_lossy();
        let stack = fixture.stack_file.to_string_lossy();
        let remote = fixture.remote_profile_file.to_string_lossy();
        let missing_secret = fixture._dir.path().join("missing-secret");
        let missing_secret_str = missing_secret.to_string_lossy();

        let prep = run_cli(
            &[
                "prepare-stack-publication",
                "--local-operator",
                "test-operator",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--stack-file",
                &stack,
            ],
            &[],
        );
        assert!(
            prep.status.success(),
            "{}",
            String::from_utf8_lossy(&prep.stderr)
        );

        let mut stale_stack = fixture.stack_input.clone();
        stale_stack.plan.fence = fixture.fence + 1;
        let stale_stack_file = fixture._dir.path().join("stale-stack.json");
        write_json(&stale_stack_file, &stale_stack);
        let stale_stack_str = stale_stack_file.to_string_lossy();

        let mismatched = run_cli(
            &[
                "publish",
                "--route",
                "github_stack",
                "--local-operator",
                "test-operator",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--stack-file",
                &stale_stack_str,
                "--secret-file",
                &missing_secret_str,
                "--remote-profile",
                &remote,
            ],
            &[],
        );
        assert!(!mismatched.status.success());
        assert!(mismatched.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&mismatched.stderr);
        assert!(stderr.contains("publication is not prepared"), "{stderr}");
        assert!(!stderr.contains("secret file is unreadable"), "{stderr}");

        supersede_and_claim_newer_attempt(&fixture);
        let superseded = run_cli(
            &[
                "publish",
                "--route",
                "github_stack",
                "--local-operator",
                "test-operator",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--stack-file",
                &stack,
                "--secret-file",
                &missing_secret_str,
                "--remote-profile",
                &remote,
            ],
            &[],
        );
        assert!(!superseded.status.success());
        assert!(superseded.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&superseded.stderr);
        assert!(!stderr.contains("secret file is unreadable"), "{stderr}");
    }

    // 2. Prepared --publication-file only + superseded work + newer claim.
    {
        let fixture = setup_verified_fixture();
        let db = fixture.database.to_string_lossy();
        let enroll = fixture.enrollment_file.to_string_lossy();
        let pub_path = fixture.publication_file.to_string_lossy();
        let remote = fixture.remote_profile_file.to_string_lossy();
        let missing_secret = fixture._dir.path().join("missing-secret");
        let missing_secret_str = missing_secret.to_string_lossy();

        let prep = run_cli(
            &[
                "prepare-publication",
                "--local-operator",
                "test-operator",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--publication-file",
                &pub_path,
            ],
            &[],
        );
        assert!(
            prep.status.success(),
            "{}",
            String::from_utf8_lossy(&prep.stderr)
        );

        supersede_and_claim_newer_attempt(&fixture);
        let superseded = run_cli(
            &[
                "publish",
                "--route",
                "github_stack",
                "--local-operator",
                "test-operator",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--publication-file",
                &pub_path,
                "--secret-file",
                &missing_secret_str,
                "--remote-profile",
                &remote,
            ],
            &[],
        );
        assert!(!superseded.status.success());
        assert!(superseded.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&superseded.stderr);
        assert!(!stderr.contains("secret file is unreadable"), "{stderr}");
    }
}

#[cfg(feature = "protocol-fixtures")]
#[test]
fn publish_route_github_stack_refuses_absent_authority_with_zero_http_requests() {
    // 1. Prepared --stack-file (valid secret + missing secret; omitted flags + `"profile": "absent"`).
    {
        let fixture = setup_verified_fixture();
        let db = fixture.database.to_string_lossy();
        let enroll = fixture.enrollment_file.to_string_lossy();
        let stack = fixture.stack_file.to_string_lossy();
        let secret = fixture.secret_file.to_string_lossy();
        let remote = fixture.remote_profile_file.to_string_lossy();
        let missing_secret = fixture._dir.path().join("missing-secret");
        let missing_secret_str = missing_secret.to_string_lossy();
        let absent_cfg = write_absent_authority_config(fixture._dir.path());
        let absent_cfg_str = absent_cfg.to_string_lossy();

        let prep = run_cli(
            &[
                "prepare-stack-publication",
                "--local-operator",
                "test-operator",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--stack-file",
                &stack,
            ],
            &[],
        );
        assert!(prep.status.success());

        for (extra_args, secret_arg) in [
            (vec![], secret.as_ref()),
            (vec![], missing_secret_str.as_ref()),
            (
                vec!["--authority-config", absent_cfg_str.as_ref()],
                secret.as_ref(),
            ),
            (
                vec!["--authority-config", absent_cfg_str.as_ref()],
                missing_secret_str.as_ref(),
            ),
        ] {
            let server = LoopbackServer::start(|_| ("500 Unexpected HTTP", "{}".to_string()));
            let env = [(
                "KILN_GITHUB_FIXTURE_PUBLICATION_ENDPOINT",
                server.endpoint.as_str(),
            )];
            let mut args = vec![
                "publish",
                "--route",
                "github_stack",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--stack-file",
                &stack,
                "--secret-file",
                secret_arg,
                "--remote-profile",
                &remote,
            ];
            args.extend(extra_args);
            let out = run_cli(&args, &env);
            assert!(!out.status.success());
            assert!(out.stdout.is_empty());
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(stderr.contains("no authority profile selected"), "{stderr}");
            assert!(!stderr.contains("secret file is unreadable"), "{stderr}");
            assert!(server.finish().is_empty());
        }
    }

    // 2. Prepared --publication-file only (valid secret + missing secret; omitted flags + `"profile": "absent"`).
    {
        let fixture = setup_verified_fixture();
        let db = fixture.database.to_string_lossy();
        let enroll = fixture.enrollment_file.to_string_lossy();
        let pub_path = fixture.publication_file.to_string_lossy();
        let secret = fixture.secret_file.to_string_lossy();
        let remote = fixture.remote_profile_file.to_string_lossy();
        let missing_secret = fixture._dir.path().join("missing-secret");
        let missing_secret_str = missing_secret.to_string_lossy();
        let absent_cfg = write_absent_authority_config(fixture._dir.path());
        let absent_cfg_str = absent_cfg.to_string_lossy();

        let prep = run_cli(
            &[
                "prepare-publication",
                "--local-operator",
                "test-operator",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--publication-file",
                &pub_path,
            ],
            &[],
        );
        assert!(prep.status.success());

        for (extra_args, secret_arg) in [
            (vec![], secret.as_ref()),
            (vec![], missing_secret_str.as_ref()),
            (
                vec!["--authority-config", absent_cfg_str.as_ref()],
                secret.as_ref(),
            ),
            (
                vec!["--authority-config", absent_cfg_str.as_ref()],
                missing_secret_str.as_ref(),
            ),
        ] {
            let server = LoopbackServer::start(|_| ("500 Unexpected HTTP", "{}".to_string()));
            let env = [(
                "KILN_GITHUB_FIXTURE_PUBLICATION_ENDPOINT",
                server.endpoint.as_str(),
            )];
            let mut args = vec![
                "publish",
                "--route",
                "github_stack",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--publication-file",
                &pub_path,
                "--secret-file",
                secret_arg,
                "--remote-profile",
                &remote,
            ];
            args.extend(extra_args);
            let out = run_cli(&args, &env);
            assert!(!out.status.success());
            assert!(out.stdout.is_empty());
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(stderr.contains("no authority profile selected"), "{stderr}");
            assert!(!stderr.contains("secret file is unreadable"), "{stderr}");
            assert!(server.finish().is_empty());
        }
    }
}

#[cfg(feature = "protocol-fixtures")]
#[test]
fn publish_route_github_stack_refuses_stale_fence_and_superseded_work_with_zero_http_requests() {
    // 1. --stack-file: mismatched fence+1 AND superseded work + newer claim.
    {
        let fixture = setup_verified_fixture();
        let db = fixture.database.to_string_lossy();
        let enroll = fixture.enrollment_file.to_string_lossy();
        let stack = fixture.stack_file.to_string_lossy();
        let secret = fixture.secret_file.to_string_lossy();
        let remote = fixture.remote_profile_file.to_string_lossy();
        let missing_secret = fixture._dir.path().join("missing-secret");
        let missing_secret_str = missing_secret.to_string_lossy();

        let prep = run_cli(
            &[
                "prepare-stack-publication",
                "--local-operator",
                "test-operator",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--stack-file",
                &stack,
            ],
            &[],
        );
        assert!(prep.status.success());

        let mut stale_stack = fixture.stack_input.clone();
        stale_stack.plan.fence = fixture.fence + 1;
        let stale_stack_file = fixture._dir.path().join("stale-stack.json");
        write_json(&stale_stack_file, &stale_stack);
        let stale_stack_str = stale_stack_file.to_string_lossy();

        let server = LoopbackServer::start(|_| ("500 Unexpected HTTP", "{}".to_string()));
        let env = [(
            "KILN_GITHUB_FIXTURE_PUBLICATION_ENDPOINT",
            server.endpoint.as_str(),
        )];
        let mismatched = run_cli(
            &[
                "publish",
                "--route",
                "github_stack",
                "--local-operator",
                "test-operator",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--stack-file",
                &stale_stack_str,
                "--secret-file",
                &secret,
                "--remote-profile",
                &remote,
            ],
            &env,
        );
        assert!(!mismatched.status.success());
        assert!(mismatched.stdout.is_empty());
        assert!(server.finish().is_empty());

        supersede_and_claim_newer_attempt(&fixture);
        for secret_arg in [secret.as_ref(), missing_secret_str.as_ref()] {
            let server = LoopbackServer::start(|_| ("500 Unexpected HTTP", "{}".to_string()));
            let env = [(
                "KILN_GITHUB_FIXTURE_PUBLICATION_ENDPOINT",
                server.endpoint.as_str(),
            )];
            let out = run_cli(
                &[
                    "publish",
                    "--route",
                    "github_stack",
                    "--local-operator",
                    "test-operator",
                    "--database",
                    &db,
                    "--enrollment",
                    &enroll,
                    "--stack-file",
                    &stack,
                    "--secret-file",
                    secret_arg,
                    "--remote-profile",
                    &remote,
                ],
                &env,
            );
            assert!(!out.status.success());
            assert!(out.stdout.is_empty());
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(!stderr.contains("secret file is unreadable"), "{stderr}");
            assert!(server.finish().is_empty());
        }
    }

    // 2. --publication-file only: superseded work + newer claim.
    {
        let fixture = setup_verified_fixture();
        let db = fixture.database.to_string_lossy();
        let enroll = fixture.enrollment_file.to_string_lossy();
        let pub_path = fixture.publication_file.to_string_lossy();
        let secret = fixture.secret_file.to_string_lossy();
        let remote = fixture.remote_profile_file.to_string_lossy();
        let missing_secret = fixture._dir.path().join("missing-secret");
        let missing_secret_str = missing_secret.to_string_lossy();

        let prep = run_cli(
            &[
                "prepare-publication",
                "--local-operator",
                "test-operator",
                "--database",
                &db,
                "--enrollment",
                &enroll,
                "--publication-file",
                &pub_path,
            ],
            &[],
        );
        assert!(prep.status.success());

        supersede_and_claim_newer_attempt(&fixture);
        for secret_arg in [secret.as_ref(), missing_secret_str.as_ref()] {
            let server = LoopbackServer::start(|_| ("500 Unexpected HTTP", "{}".to_string()));
            let env = [(
                "KILN_GITHUB_FIXTURE_PUBLICATION_ENDPOINT",
                server.endpoint.as_str(),
            )];
            let out = run_cli(
                &[
                    "publish",
                    "--route",
                    "github_stack",
                    "--local-operator",
                    "test-operator",
                    "--database",
                    &db,
                    "--enrollment",
                    &enroll,
                    "--publication-file",
                    &pub_path,
                    "--secret-file",
                    secret_arg,
                    "--remote-profile",
                    &remote,
                ],
                &env,
            );
            assert!(!out.status.success());
            assert!(out.stdout.is_empty());
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(!stderr.contains("secret file is unreadable"), "{stderr}");
            assert!(server.finish().is_empty());
        }
    }
}

#[cfg(feature = "protocol-fixtures")]
#[test]
fn stack_gate_rejects_mismatched_pr_number_and_tampered_repo_prefix_without_http() {
    let fixture = setup_verified_fixture();
    let db = fixture.database.to_string_lossy().into_owned();
    let enroll = fixture.enrollment_file.to_string_lossy().into_owned();
    let stack = fixture.stack_file.to_string_lossy().into_owned();
    let secret = fixture.secret_file.to_string_lossy().into_owned();
    let remote = fixture.remote_profile_file.to_string_lossy().into_owned();

    let prep = run_cli(
        &[
            "prepare-stack-publication",
            "--local-operator",
            "test-operator",
            "--database",
            &db,
            "--enrollment",
            &enroll,
            "--stack-file",
            &stack,
        ],
        &[],
    );
    assert!(prep.status.success());

    let layer0 = fixture
        .stack_input
        .plan
        .layers
        .first()
        .unwrap_or_else(|| unreachable!())
        .clone();
    let layer1 = fixture
        .stack_input
        .plan
        .layers
        .get(1)
        .unwrap_or_else(|| unreachable!())
        .clone();
    let pr1 = serde_json::json!({
        "number": 11,
        "state": "open",
        "merged": false,
        "html_url": "https://github.com/octocat/hello-world/pull/11",
        "head": {"ref": layer0.branch, "sha": layer0.candidate.native_id.as_str()},
        "base": {"ref": layer0.base_branch}
    });
    let pr2 = serde_json::json!({
        "number": 12,
        "state": "open",
        "merged": false,
        "html_url": "https://github.com/octocat/hello-world/pull/12",
        "head": {"ref": layer1.branch, "sha": layer1.candidate.native_id.as_str()},
        "base": {"ref": layer1.base_branch}
    });
    let stack_json = serde_json::json!({
        "number": 5,
        "base": {"ref": "main"},
        "open": true,
        "pull_requests": [
            {"number": 11, "state": "open", "draft": false, "merged_at": null, "head": {"ref": layer0.branch, "sha": layer0.candidate.native_id.as_str()}},
            {"number": 12, "state": "open", "draft": false, "merged_at": null, "head": {"ref": layer1.branch, "sha": layer1.candidate.native_id.as_str()}},
        ]
    });
    let created_pr1 = Arc::new(AtomicBool::new(false));
    let created_pr2 = Arc::new(AtomicBool::new(false));
    let created_stack = Arc::new(AtomicBool::new(false));
    let pub_server = {
        let pr1 = pr1.clone();
        let pr2 = pr2.clone();
        let stack_json = stack_json.clone();
        let b1 = layer0.branch.clone();
        let b2 = layer1.branch.clone();
        let enc1 = b1.replace('/', "%2F");
        let enc2 = b2.replace('/', "%2F");
        let c1 = Arc::clone(&created_pr1);
        let c2 = Arc::clone(&created_pr2);
        let cs = Arc::clone(&created_stack);
        LoopbackServer::start(move |req| {
            if req.starts_with("GET /repos/octocat/hello-world/stacks?per_page=1") {
                return ("200 OK", "[]".to_string());
            }
            if req.starts_with("GET /repos/octocat/hello-world/pulls?") {
                if (req.contains(&b2) || req.contains(&enc2)) && c2.load(Ordering::Acquire) {
                    return ("200 OK", serde_json::json!([pr2]).to_string());
                }
                if (req.contains(&b1) || req.contains(&enc1)) && c1.load(Ordering::Acquire) {
                    return ("200 OK", serde_json::json!([pr1]).to_string());
                }
                return ("200 OK", "[]".to_string());
            }
            if req.starts_with("POST /repos/octocat/hello-world/pulls ") {
                if req.contains(&b2) {
                    c2.store(true, Ordering::Release);
                    return ("201 Created", pr2.to_string());
                }
                c1.store(true, Ordering::Release);
                return ("201 Created", pr1.to_string());
            }
            if req.starts_with("GET /repos/octocat/hello-world/stacks?pull_request=11") {
                if cs.load(Ordering::Acquire) {
                    return ("200 OK", serde_json::json!([stack_json]).to_string());
                }
                return ("200 OK", "[]".to_string());
            }
            if req.starts_with("POST /repos/octocat/hello-world/stacks ") {
                cs.store(true, Ordering::Release);
                return ("201 Created", stack_json.to_string());
            }
            ("404 Not Found", "{}".to_string())
        })
    };
    let pub_env = [(
        "KILN_GITHUB_FIXTURE_PUBLICATION_ENDPOINT",
        pub_server.endpoint.as_str(),
    )];
    let published = run_cli(
        &[
            "publish-stack",
            "--local-operator",
            "test-operator",
            "--database",
            &db,
            "--enrollment",
            &enroll,
            "--stack-file",
            &stack,
            "--secret-file",
            &secret,
            "--remote-profile",
            &remote,
        ],
        &pub_env,
    );
    assert!(
        published.status.success(),
        "{}",
        String::from_utf8_lossy(&published.stderr)
    );
    let _ = pub_server.finish();

    let base_sha = fixture.base_oid.to_string();
    let lower_sha = fixture.lower_oid.to_string();
    let top_sha = fixture.top_oid.to_string();

    // Case A: Wrong PR number (111 instead of 11) in landing plans.
    let wrong_pr_file = fixture._dir.path().join("wrong-pr-landing.json");
    write_json(
        &wrong_pr_file,
        &serde_json::json!([
            {
                "schema": "kiln.github-landing-plan.v1",
                "owner": "octocat",
                "repository": "hello-world",
                "pull_number": 111,
                "base_ref": "main",
                "base_head": base_sha,
                "head_sha": lower_sha,
                "required_checks": ["test"],
                "required_approvals": 0,
                "operation": content('c')
            },
            {
                "schema": "kiln.github-landing-plan.v1",
                "owner": "octocat",
                "repository": "hello-world",
                "pull_number": 12,
                "base_ref": layer1.base_branch,
                "base_head": lower_sha,
                "head_sha": top_sha,
                "required_checks": ["test"],
                "required_approvals": 0,
                "operation": content('d')
            }
        ]),
    );
    let wrong_pr_path = wrong_pr_file.to_string_lossy().into_owned();
    let gate_server = LoopbackServer::start(|_| ("500 Unexpected HTTP", "{}".to_string()));
    let gate_env = [(
        "KILN_GITHUB_FIXTURE_LANDING_ENDPOINT",
        gate_server.endpoint.as_str(),
    )];
    let wrong_pr_out = run_cli(
        &[
            "stack-gate",
            "--database",
            &db,
            "--enrollment",
            &enroll,
            "--stack-file",
            &stack,
            "--landing-plans",
            &wrong_pr_path,
            "--secret-file",
            &secret,
        ],
        &gate_env,
    );
    assert!(!wrong_pr_out.status.success());
    let stderr = String::from_utf8_lossy(&wrong_pr_out.stderr);
    assert!(
        stderr.contains("landing plan does not match the prepared stack layer"),
        "{stderr}"
    );
    assert!(gate_server.finish().is_empty());

    // Case B: Matching PR number 11, but tampered repo prefix `other/hello-world#11` in retained publication.
    let valid_landing_file = fixture._dir.path().join("valid-landing.json");
    write_json(
        &valid_landing_file,
        &serde_json::json!([
            {
                "schema": "kiln.github-landing-plan.v1",
                "owner": "octocat",
                "repository": "hello-world",
                "pull_number": 11,
                "base_ref": "main",
                "base_head": base_sha,
                "head_sha": lower_sha,
                "required_checks": ["test"],
                "required_approvals": 0,
                "operation": content('c')
            },
            {
                "schema": "kiln.github-landing-plan.v1",
                "owner": "octocat",
                "repository": "hello-world",
                "pull_number": 12,
                "base_ref": layer1.base_branch,
                "base_head": lower_sha,
                "head_sha": top_sha,
                "required_checks": ["test"],
                "required_approvals": 0,
                "operation": content('d')
            }
        ]),
    );
    let valid_landing_path = valid_landing_file.to_string_lossy().into_owned();
    {
        let conn = rusqlite::Connection::open(&fixture.database)
            .unwrap_or_else(|error| unreachable!("{error}"));
        let pub_key = format!("stack-publication:{}", fixture.attempt_id.as_str());
        let bytes: Vec<u8> = conn
            .query_row(
                "SELECT bytes FROM implementation_state WHERE kind = ?1",
                rusqlite::params![&pub_key],
                |row| row.get(0),
            )
            .unwrap_or_else(|error| unreachable!("{error}"));
        let mut stored: kiln_core::implementation::StackPublication =
            serde_json::from_slice(&bytes).unwrap_or_else(|_| unreachable!());
        let first = stored
            .pull_requests
            .first_mut()
            .unwrap_or_else(|| unreachable!("expected stored pull request"));
        *first = ExternalRef::new("github", "pull_request", "other/hello-world#11")
            .unwrap_or_else(|_| unreachable!());
        conn.execute(
            "UPDATE implementation_state SET bytes = ?1 WHERE kind = ?2",
            rusqlite::params![
                serde_json::to_vec(&stored).unwrap_or_else(|_| unreachable!()),
                &pub_key
            ],
        )
        .unwrap_or_else(|error| unreachable!("{error}"));
    }
    let tampered_server = LoopbackServer::start(|_| ("500 Unexpected HTTP", "{}".to_string()));
    let tampered_env = [(
        "KILN_GITHUB_FIXTURE_LANDING_ENDPOINT",
        tampered_server.endpoint.as_str(),
    )];
    let tampered_out = run_cli(
        &[
            "stack-gate",
            "--database",
            &db,
            "--enrollment",
            &enroll,
            "--stack-file",
            &stack,
            "--landing-plans",
            &valid_landing_path,
            "--secret-file",
            &secret,
        ],
        &tampered_env,
    );
    assert!(!tampered_out.status.success());
    let stderr = String::from_utf8_lossy(&tampered_out.stderr);
    assert!(
        stderr.contains("landing plan does not match the prepared stack layer"),
        "{stderr}"
    );
    assert!(tampered_server.finish().is_empty());
}
