//! End-to-end integration test linking `kiln-adapter-github` label and
//! `/runbook <id>` comment routing to a real `kiln-web-service` TLS server
//! (`#97` dispatch + `#98` fan-out).

use hmac::{Hmac, Mac};
use kiln_adapter_github::comments::{
    GithubCommentPort, IssueCommentPollPage, IssueCommentPollRecord, IssueRoutingPollPage,
    PostedIssueComment,
};
use kiln_adapter_github::events::{DeliveryHeaders, EventPolicy};
use kiln_adapter_github::implementation::ImplementationIssuePolicy;
use kiln_adapter_github::profile::ImplementationQueueConfig;
use kiln_adapter_github::runbook_dispatch::{WebDispatchConfig, WebRunbookDispatchClient};
use kiln_adapter_github::runbook_routing::{
    CommandRunbookRoute, LabelRunbookRoute, PullRequestHead, PullRequestHeadRepo,
    PullRequestOriginCheck, RoutingClaimPolicy, RoutingFanoutOptions, RoutingParamValue,
    RunbookRoutingMode, RunbookRoutingPolicy, drain_runbooks_queue, poll_runbooks,
};
use kiln_adapter_github::secret::ResolvedSecret;
use kiln_core::implementation::{AcceptanceObligation, TerminalPolicy, WorkBudget};
use kiln_core::types::ContentId;
use kiln_implementer::{DeliveryQueue, RunbookRoutingStore};
use kiln_provider_api::PortResult;
use kiln_web_service::profile::ServiceProfile;
use kiln_web_service::runbooks::{
    ParamBinding, ParamDeclaration, ParamType, ParamValue, RUNBOOK_REGISTRY_SCHEMA,
    RunbookDefinition, RunbookFanoutConfig, RunbookGrant, RunbookLattice, RunbookMode,
    RunbookRegistry, RunbookTarget,
};
use serde_json::{Value, json};
use sha2::Sha256;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixDatagram;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

const CA: &[u8] = include_bytes!("../../../fixtures/http-observation/tls/ca-cert.pem");
const CERT: &[u8] = include_bytes!("../../../fixtures/http-observation/tls/cert.pem");
const KEY: &[u8] = include_bytes!("../../../fixtures/http-observation/tls/key.pem");
const WEBHOOK_SECRET: &[u8] = b"live-routing-webhook-secret";
const TOKEN_LENGTH: usize = 64;

struct Service(Child);
impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn checked<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sign(body: &[u8], event: &str, delivery: &str) -> DeliveryHeaders {
    let mut mac = checked(Hmac::<Sha256>::new_from_slice(WEBHOOK_SECRET));
    mac.update(body);
    DeliveryHeaders {
        delivery: delivery.to_string(),
        event: event.to_string(),
        signature: format!("sha256={}", hex(&mac.finalize().into_bytes())),
    }
}

fn tls_get(addr: SocketAddr, target: &str, token: &str) -> (u16, Value) {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut roots = rustls::RootCertStore::empty();
    let certs = checked(
        rustls_pemfile::certs(&mut std::io::Cursor::new(CA)).collect::<Result<Vec<_>, _>>(),
    );
    assert!(roots.add_parsable_certificates(certs).0 > 0);
    let tls = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = checked(rustls::pki_types::ServerName::try_from("localhost"));
    let conn = checked(rustls::ClientConnection::new(Arc::new(tls), name));
    let sock = checked(TcpStream::connect_timeout(&addr, Duration::from_secs(3)));
    checked(sock.set_read_timeout(Some(Duration::from_secs(5))));
    checked(sock.set_write_timeout(Some(Duration::from_secs(5))));
    let mut stream = rustls::StreamOwned::new(conn, sock);
    let req = format!(
        "GET {target} HTTP/1.1\r\nHost: localhost:{}\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n",
        addr.port()
    );
    checked(stream.write_all(req.as_bytes()));
    checked(stream.flush());
    let mut bytes = Vec::new();
    let mut buf = [0_u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => bytes.extend_from_slice(buf.get(..n).unwrap_or_default()),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => unreachable!("read failed: {e}"),
        }
    }
    let split = bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or_else(|| unreachable!("missing headers"));
    let head = std::str::from_utf8(bytes.get(..split).unwrap_or_default())
        .unwrap_or_else(|_| unreachable!());
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or_else(|| unreachable!());
    let body: Value = checked(serde_json::from_slice(
        bytes.get(split + 4..).unwrap_or_default(),
    ));
    (status, body)
}

fn tls_post(addr: SocketAddr, target: &str, token: &str, payload: &Value) -> (u16, Value) {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut roots = rustls::RootCertStore::empty();
    let certs = checked(
        rustls_pemfile::certs(&mut std::io::Cursor::new(CA)).collect::<Result<Vec<_>, _>>(),
    );
    assert!(roots.add_parsable_certificates(certs).0 > 0);
    let tls = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = checked(rustls::pki_types::ServerName::try_from("localhost"));
    let conn = checked(rustls::ClientConnection::new(Arc::new(tls), name));
    let sock = checked(TcpStream::connect_timeout(&addr, Duration::from_secs(3)));
    checked(sock.set_read_timeout(Some(Duration::from_secs(5))));
    checked(sock.set_write_timeout(Some(Duration::from_secs(5))));
    let mut stream = rustls::StreamOwned::new(conn, sock);
    let body = checked(serde_json::to_vec(payload));
    let req = format!(
        "POST {target} HTTP/1.1\r\nHost: localhost:{}\r\nAuthorization: Bearer {token}\r\nOrigin: https://localhost:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        addr.port(),
        addr.port(),
        body.len()
    );
    checked(stream.write_all(req.as_bytes()));
    checked(stream.write_all(&body));
    checked(stream.flush());
    let mut bytes = Vec::new();
    let mut buf = [0_u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => bytes.extend_from_slice(buf.get(..n).unwrap_or_default()),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => unreachable!("read failed: {e}"),
        }
    }
    let split = bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or_else(|| unreachable!("missing headers"));
    let head = std::str::from_utf8(bytes.get(..split).unwrap_or_default())
        .unwrap_or_else(|_| unreachable!());
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or_else(|| unreachable!());
    let parsed: Value = checked(serde_json::from_slice(
        bytes.get(split + 4..).unwrap_or_default(),
    ));
    (status, parsed)
}

struct LoopbackGithubMock {
    pr_heads: BTreeMap<u64, PullRequestHead>,
    posted_comments: Vec<(u64, String)>,
    polled_comments: Vec<IssueCommentPollRecord>,
    next_id: u64,
}

impl PullRequestOriginCheck for LoopbackGithubMock {
    fn check_pull_request_head(
        &mut self,
        _owner: &str,
        _repository: &str,
        pull_request_number: u64,
    ) -> PortResult<PullRequestHead> {
        Ok(self
            .pr_heads
            .get(&pull_request_number)
            .cloned()
            .unwrap_or(PullRequestHead { repo: None }))
    }
}

impl GithubCommentPort for LoopbackGithubMock {
    fn list_issue_comments(
        &mut self,
        _owner: &str,
        _repository: &str,
        _since_ms: u64,
        page: u32,
        _per_page: u8,
    ) -> PortResult<IssueCommentPollPage> {
        Ok(IssueCommentPollPage {
            records: if page == 1 {
                self.polled_comments.clone()
            } else {
                Vec::new()
            },
            next_page: None,
        })
    }

    fn list_issues_for_routing(
        &mut self,
        _owner: &str,
        _repository: &str,
        _since_ms: u64,
        _page: u32,
        _per_page: u8,
    ) -> PortResult<IssueRoutingPollPage> {
        Ok(IssueRoutingPollPage {
            records: Vec::new(),
            next_page: None,
        })
    }

    fn post_issue_comment(
        &mut self,
        _owner: &str,
        _repository: &str,
        issue_number: u64,
        body: &str,
    ) -> PortResult<PostedIssueComment> {
        self.next_id += 1;
        self.posted_comments.push((issue_number, body.to_string()));
        Ok(PostedIssueComment {
            comment_id: self.next_id,
        })
    }

    fn find_issue_comment_by_marker(
        &mut self,
        _owner: &str,
        _repository: &str,
        issue_number: u64,
        marker: &str,
    ) -> PortResult<Option<PostedIssueComment>> {
        Ok(self
            .posted_comments
            .iter()
            .enumerate()
            .find(|(_, (num, body))| *num == issue_number && body.contains(marker))
            .map(|(idx, _)| PostedIssueComment {
                comment_id: 1 + idx as u64,
            }))
    }
}

#[test]
fn github_label_and_comment_routing_dispatches_to_live_kiln_web_service() {
    let root = checked(tempfile::tempdir());
    let repository_path = root.path().join("repository");
    let repository = checked(git2::Repository::init(&repository_path));
    checked(fs::write(
        repository_path.join("source.txt"),
        b"runbook source\n",
    ));
    let mut index = checked(repository.index());
    checked(index.add_path(std::path::Path::new("source.txt")));
    let tree_id = checked(index.write_tree());
    let tree = checked(repository.find_tree(tree_id));
    let signature = checked(git2::Signature::now("test", "test@example.invalid"));
    checked(repository.commit(
        Some("refs/heads/main"),
        &signature,
        &signature,
        "fixture",
        &tree,
        &[],
    ));

    let listener = checked(TcpListener::bind("127.0.0.1:0"));
    let addr = checked(listener.local_addr());
    drop(listener);
    let origin = format!("https://localhost:{}", addr.port());

    let cert_path = root.path().join("cert.pem");
    let key_path = root.path().join("key.pem");
    let op_token_path = root.path().join("operator.token");
    let wk_token_path = root.path().join("worker.token");
    checked(fs::write(&cert_path, CERT));
    checked(fs::write(&key_path, KEY));
    checked(fs::set_permissions(
        &key_path,
        fs::Permissions::from_mode(0o400),
    ));
    let op_token = "a".repeat(TOKEN_LENGTH);
    let wk_token = "b".repeat(TOKEN_LENGTH);
    checked(fs::write(&op_token_path, &op_token));
    checked(fs::write(&wk_token_path, &wk_token));
    checked(fs::set_permissions(
        &op_token_path,
        fs::Permissions::from_mode(0o400),
    ));
    checked(fs::set_permissions(
        &wk_token_path,
        fs::Permissions::from_mode(0o400),
    ));

    let profile: ServiceProfile = checked(serde_json::from_value(json!({
        "schema": "kiln.web-service.v1",
        "service_id": "github-routing-tests",
        "listen_address": addr,
        "origin": origin,
        "tls": {
            "certificate_path": cert_path,
            "private_key_path": key_path,
            "min_version": "TLS1.3"
        },
        "operator_token_file": op_token_path,
        "worker_token_file": wk_token_path,
        "state_directory": root.path().join("state"),
        "projects": [{
            "id": "kiln",
            "label": "Kiln",
            "repository_path": repository_path,
            "reference": "refs/heads/main",
            "source_limits": {
                "max_entries": 32,
                "max_path_bytes": 1024,
                "max_file_bytes": 4096,
                "max_total_bytes": 16384
            },
            "workflow": {
                "socket_path": root.path().join("lattice.sock"),
                "allowed_user_ids": [nix::unistd::geteuid().as_raw()],
                "transport_limits": {
                    "maximum_frame_bytes": 1048576,
                    "maximum_log_bytes": 65536,
                    "maximum_connections": 4,
                    "read_deadline_milliseconds": 1000,
                    "write_deadline_milliseconds": 1000
                },
                "adapter_limits": {
                    "maximum_request_bytes": 1048576,
                    "maximum_response_bytes": 1048576
                },
                "policy_id": "reviewed-policy",
                "provider_profile_id": "reviewed-lattice",
                "platform": "linux-x86_64",
                "nodes": [{
                    "id": "job",
                    "kind": "test",
                    "effect_class": "test",
                    "adapter_kind": "local",
                    "operation_identity": "a".repeat(64)
                }],
                "edges": [],
                "controls": { "cancel": true, "pause": false, "resume": false },
                "bounds": {
                    "max_nodes": 8,
                    "max_edges": 16,
                    "max_log_references": 32,
                    "max_log_read_bytes": 4096,
                    "max_total_log_bytes": 65536
                }
            }
        }],
        "bounds": {
            "max_request_bytes": 16384,
            "max_response_bytes": 1048576,
            "max_header_bytes": 4096,
            "max_connections": 8,
            "timeout_seconds": 2,
            "poll_interval_ms": 10,
            "max_projects": 2,
            "max_runs": 16,
            "max_artifact_bytes": 1024,
            "max_artifacts": 2,
            "max_run_artifact_bytes": 2048,
            "max_total_artifact_bytes": 4096,
            "max_log_read_bytes": 4096,
            "max_requests": 32,
            "max_events_per_run": 8,
            "max_state_bytes": 1048576
        }
    })));
    let profile_path = root.path().join("service.json");
    checked(fs::write(
        &profile_path,
        checked(serde_json::to_vec(&profile)),
    ));

    let nodes = vec![kiln_adapter_github::profile::ConfigNode {
        id: "job".into(),
        kind: "test".into(),
        effect_class: kiln_adapter_github::profile::ConfigEffectClass::Test,
        adapter_kind: "local".into(),
        operation_identity: "a".repeat(64),
    }];
    let target_rule = RunbookTarget {
        repository: "OnixResearch/kiln".into(),
        project_id: "kiln".into(),
        references: vec!["refs/heads/main".into()],
        allow_commit_sha: false,
    };
    let registry = RunbookRegistry {
        schema: RUNBOOK_REGISTRY_SCHEMA.into(),
        runbooks: vec![
            RunbookDefinition {
                id: "security-audit".into(),
                display_name: "Security Audit".into(),
                description: "Read-only audit".into(),
                mode: RunbookMode::ReadOnly,
                lattice: RunbookLattice {
                    bundle_id: Some("security-audit".into()),
                    bundle_identity: "b".repeat(64),
                    workflow_name: "security-audit".into(),
                    workflow_revision: "c".repeat(64),
                    nodes: nodes.clone(),
                    edges: vec![],
                },
                params: vec![ParamDeclaration {
                    name: "dry_run".into(),
                    param_type: ParamType::Bool {},
                    required: false,
                    default: Some(ParamValue::Bool(true)),
                    bindings: vec![ParamBinding {
                        node: "job".into(),
                        shell_env: "LATTICE_PARAM_DRY_RUN".into(),
                    }],
                }],
                allowed_targets: vec![target_rule.clone()],
                fanout: Some(RunbookFanoutConfig {
                    enabled: true,
                    default_max_concurrency: 1,
                    max_concurrency_limit: 4,
                    default_budget_cap: 8,
                    max_budget_cap: 16,
                    max_targets: 8,
                    write_scopes: vec![],
                }),
            },
            RunbookDefinition {
                id: "granted-mutating".into(),
                display_name: "Granted Mutating".into(),
                description: "Mutating with grant".into(),
                mode: RunbookMode::Mutating,
                lattice: RunbookLattice {
                    bundle_id: Some("granted-mutating".into()),
                    bundle_identity: "d".repeat(64),
                    workflow_name: "granted-mutating".into(),
                    workflow_revision: "e".repeat(64),
                    nodes: nodes.clone(),
                    edges: vec![],
                },
                params: vec![],
                allowed_targets: vec![target_rule.clone()],
                fanout: None,
            },
            RunbookDefinition {
                id: "ungranted-mutating".into(),
                display_name: "Ungranted Mutating".into(),
                description: "Mutating without grant".into(),
                mode: RunbookMode::Mutating,
                lattice: RunbookLattice {
                    bundle_id: Some("ungranted-mutating".into()),
                    bundle_identity: "f".repeat(64),
                    workflow_name: "ungranted-mutating".into(),
                    workflow_revision: "0".repeat(64),
                    nodes,
                    edges: vec![],
                },
                params: vec![],
                allowed_targets: vec![target_rule],
                fanout: None,
            },
        ],
        grants: vec![RunbookGrant {
            principal: "local:operator".into(),
            runbooks: vec!["granted-mutating".into()],
            not_after_ms: None,
        }],
        notifications: Vec::new(),
    };
    let runbooks_path = root.path().join("runbooks.json");
    checked(fs::write(
        &runbooks_path,
        checked(serde_json::to_vec(&registry)),
    ));

    let notify_path = root.path().join("notify.sock");
    let receiver = checked(UnixDatagram::bind(&notify_path));
    checked(receiver.set_read_timeout(Some(Duration::from_secs(8))));
    let mut child = Service(checked(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args(["serve", "--profile"])
            .arg(&profile_path)
            .arg("--runbooks")
            .arg(&runbooks_path)
            .env("NOTIFY_SOCKET", &notify_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn(),
    ));
    let mut notification = [0; 32];
    match receiver.recv(&mut notification) {
        Ok(n) => assert_eq!(notification.get(..n), Some(b"READY=1".as_slice())),
        Err(error) => {
            let exit = checked(child.0.try_wait());
            unreachable!("serve readiness failed: {error}; exit={exit:?}");
        }
    }

    let (status, _) = tls_post(
        addr,
        "/api/projects",
        &op_token,
        &json!({"project_id": "kiln", "request_id": "enroll-1"}),
    );
    assert_eq!(status, 200);

    let queue_path = root.path().join("delivery-queue.sqlite");
    let routing_path = root.path().join("routing-store.sqlite");
    let queue_cfg = ImplementationQueueConfig {
        path: queue_path.clone(),
        maximum_deliveries: 32,
    };

    let policy = ImplementationIssuePolicy {
        repository: EventPolicy {
            owner: "OnixResearch".into(),
            repository: "kiln".into(),
            repository_id: 42,
            allow_fork_pull_requests: false,
            untrusted_pull_requests: Default::default(),
        },
        project: "kiln".into(),
        base: None,
        requested_outcome: "implement".into(),
        acceptance: vec![AcceptanceObligation {
            id: "accepted".into(),
            statement: "verified".into(),
        }],
        dependencies: Vec::new(),
        permitted_paths: vec!["crates".into()],
        permitted_effects: BTreeSet::from(["write_source".into()]),
        verification_profile: ContentId::from_blake3_hex("a".repeat(64))
            .unwrap_or_else(|_| unreachable!()),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        selection: None,
        runbook_routing: Some(RunbookRoutingPolicy {
            allowed_actors: vec!["maintainer".into()],
            allow_issue_comments: true,
            allow_pull_request_comments: true,
            require_push_association: true,
            console_base_url: Some(origin.clone()),
            labels: vec![
                LabelRunbookRoute {
                    label: "runbook:security-audit".into(),
                    runbook_id: "security-audit".into(),
                    target: Some("OnixResearch/kiln@refs/heads/main".into()),
                    target_ref: None,
                    params: BTreeMap::from([("dry_run".into(), RoutingParamValue::Bool(false))]),
                    mode: RunbookRoutingMode::Dispatch,
                    fanout: None,
                },
                LabelRunbookRoute {
                    label: "runbook:security-fanout".into(),
                    runbook_id: "security-audit".into(),
                    target: None,
                    target_ref: None,
                    params: BTreeMap::new(),
                    mode: RunbookRoutingMode::Fanout,
                    fanout: Some(RoutingFanoutOptions {
                        targets: vec!["OnixResearch/kiln@refs/heads/main".into()],
                        target_globs: Vec::new(),
                        max_concurrency: Some(1),
                        budget_cap: Some(4),
                        claim_policy: Some(RoutingClaimPolicy::Refuse),
                        scope_path: None,
                    }),
                },
            ],
            commands: vec![
                CommandRunbookRoute {
                    command: "granted-mutating".into(),
                    runbook_id: "granted-mutating".into(),
                    target: None,
                    target_ref: Some("refs/heads/main".into()),
                    params: BTreeMap::new(),
                    mode: RunbookRoutingMode::Dispatch,
                    fanout: None,
                },
                CommandRunbookRoute {
                    command: "ungranted-mutating".into(),
                    runbook_id: "ungranted-mutating".into(),
                    target: Some("OnixResearch/kiln@refs/heads/main".into()),
                    target_ref: None,
                    params: BTreeMap::new(),
                    mode: RunbookRoutingMode::Dispatch,
                    fanout: None,
                },
                CommandRunbookRoute {
                    command: "security-audit".into(),
                    runbook_id: "security-audit".into(),
                    target: Some("OnixResearch/kiln@refs/heads/main".into()),
                    target_ref: None,
                    params: BTreeMap::new(),
                    mode: RunbookRoutingMode::Dispatch,
                    fanout: None,
                },
            ],
        }),
    };

    let mut queue = checked(DeliveryQueue::open(&queue_path, 32));
    let mut record = |delivery: &str, event: &str, value: Value, ts: u64| {
        let bytes = checked(serde_json::to_vec(&value));
        let headers = sign(&bytes, event, delivery);
        checked(queue.record(
            &headers.delivery,
            &headers.event,
            &headers.signature,
            &bytes,
            ts,
        ));
    };

    // 1. Open issue label -> #97 dispatch
    record(
        "00000000-0000-0000-0000-000000000001",
        "issues",
        json!({
            "action": "labeled",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "User"},
            "label": {"name": "runbook:security-audit"},
            "issue": {"number": 10, "state": "open", "updated_at": "2026-11-11T10:00:00Z", "author_association": "OWNER"}
        }),
        1,
    );
    // 2. Open issue label -> #98 fanout
    record(
        "00000000-0000-0000-0000-000000000002",
        "issues",
        json!({
            "action": "labeled",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "User"},
            "label": {"name": "runbook:security-fanout"},
            "issue": {"number": 11, "state": "open", "updated_at": "2026-11-11T10:01:00Z", "author_association": "MEMBER"}
        }),
        2,
    );
    // 3. Issue comment -> granted mutating #97 dispatch
    record(
        "00000000-0000-0000-0000-000000000003",
        "issue_comment",
        json!({
            "action": "created",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "User"},
            "issue": {"number": 12, "state": "open", "updated_at": "2026-11-11T10:02:00Z"},
            "comment": {"id": 3001, "body": "/runbook granted-mutating", "user": {"login": "maintainer", "type": "User"}, "author_association": "COLLABORATOR"}
        }),
        3,
    );
    // 4. Non-fork PR comment -> ungranted mutating -> 403 runbook_grant_required from live service
    record(
        "00000000-0000-0000-0000-000000000004",
        "issue_comment",
        json!({
            "action": "created",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "User"},
            "issue": {"number": 13, "state": "open", "updated_at": "2026-11-11T10:03:00Z", "pull_request": {}},
            "comment": {"id": 3002, "body": "/runbook ungranted-mutating", "user": {"login": "maintainer", "type": "User"}, "author_association": "OWNER"}
        }),
        4,
    );
    // 5. Fork PR comment -> refused before calling kiln-web-service
    record(
        "00000000-0000-0000-0000-000000000005",
        "issue_comment",
        json!({
            "action": "created",
            "repository": {"id": 42, "name": "kiln", "full_name": "OnixResearch/kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "maintainer", "type": "User"},
            "issue": {"number": 14, "state": "open", "updated_at": "2026-11-11T10:04:00Z", "pull_request": {}},
            "comment": {"id": 3003, "body": "/runbook security-audit", "user": {"login": "maintainer", "type": "User"}, "author_association": "OWNER"}
        }),
        5,
    );

    let mut gh = LoopbackGithubMock {
        pr_heads: BTreeMap::from([
            (
                13,
                PullRequestHead {
                    repo: Some(PullRequestHeadRepo {
                        id: 42,
                        name: "kiln".into(),
                        full_name: Some("OnixResearch/kiln".into()),
                        owner_login: "OnixResearch".into(),
                        fork: false,
                    }),
                },
            ),
            (
                14,
                PullRequestHead {
                    repo: Some(PullRequestHeadRepo {
                        id: 999,
                        name: "kiln".into(),
                        full_name: Some("stranger/kiln".into()),
                        owner_login: "stranger".into(),
                        fork: true,
                    }),
                },
            ),
        ]),
        posted_comments: Vec::new(),
        polled_comments: vec![IssueCommentPollRecord {
            comment_id: 3001,
            issue_number: 12,
            body: "/runbook granted-mutating".into(),
            actor: "maintainer".into(),
            actor_type: Some("User".into()),
            author_association: Some("COLLABORATOR".into()),
            created_at_ms: 3,
            updated_at_ms: 3,
            is_pull_request: false,
        }],
        next_id: 0,
    };

    let web_token = checked(ResolvedSecret::from_bytes(op_token.as_bytes().to_vec()));
    let mut dispatcher = checked(WebRunbookDispatchClient::new(
        WebDispatchConfig {
            service_origin: origin.clone(),
            timeout_ms: 10_000,
            maximum_response_bytes: 1_048_576,
        },
        web_token,
        CA,
    ));

    let mut store = checked(RunbookRoutingStore::open(&routing_path, 32));
    let enrolled = kiln_core::implementation::Enrollment {
        repositories: BTreeSet::from(["onixresearch/kiln".into()]),
        actors: BTreeSet::from(["maintainer".into()]),
    };

    let summary = checked(drain_runbooks_queue(
        &queue_cfg,
        WEBHOOK_SECRET,
        &policy,
        &enrolled,
        &mut store,
        &mut gh,
        &mut dispatcher,
        16,
        10_000,
    ));
    assert_eq!(summary.drained, 5, "{summary:?}");
    assert_eq!(summary.matched, 4, "{summary:?}");
    assert_eq!(summary.dispatched, 3, "{summary:?}");
    assert_eq!(summary.refused, 2, "{summary:?}");
    assert_eq!(summary.dispatch_errors, 1, "{summary:?}");
    assert_eq!(summary.comments_posted, 3, "{summary:?}");

    // Verify reply comments contain console links & markers and never leak tokens/secrets!
    for (_, comment) in &gh.posted_comments {
        assert!(comment.contains(&origin));
        assert!(comment.contains("<!-- kiln-runbook-dispatch:gh-rb-"));
        assert!(!comment.contains(&op_token));
        assert!(!comment.contains("live-routing-webhook-secret"));
        assert!(!comment.contains("LATTICE_PARAM_"));
    }
    assert!(
        gh.posted_comments
            .iter()
            .any(|(n, c)| *n == 10 && c.contains("/?page=runs&run="))
    );
    assert!(
        gh.posted_comments
            .iter()
            .any(|(n, c)| *n == 11 && c.contains("/?page=runbooks&runbook=security-audit&fanout="))
    );

    // Overlap poll on comment 3001 must replay cleanly without creating a duplicate run or comment!
    let poll_sum = checked(poll_runbooks(
        &policy,
        &enrolled,
        &mut store,
        &mut gh,
        &mut dispatcher,
        0,
        1,
        10_001,
    ));
    assert_eq!(poll_sum.dispatched, 0);
    assert_eq!(poll_sum.replayed, 1);
    assert_eq!(poll_sum.comments_posted, 0);
    assert_eq!(gh.posted_comments.len(), 3);

    // Verify live kiln-web-service recorded the 2 direct runs and 1 fanout
    // (its child for project `kiln` stays queued as pending because an earlier
    // direct run on `kiln` is already non-terminal).
    let (runs_status, runs_body) = tls_get(addr, "/api/runs", &op_token);
    assert_eq!(runs_status, 200);
    let runs_arr = runs_body
        .get("runs")
        .and_then(Value::as_array)
        .unwrap_or_else(|| unreachable!());
    assert_eq!(
        runs_arr.len(),
        2,
        "expected 2 direct live runs, got {runs_body}"
    );

    let (fanout_status, fanout_body) = tls_get(addr, "/api/runbooks/fanouts", &op_token);
    assert_eq!(fanout_status, 200);
    let fanouts = fanout_body
        .get("fanouts")
        .and_then(Value::as_array)
        .unwrap_or_else(|| unreachable!());
    assert_eq!(fanouts.len(), 1);
    let fanout_children = fanouts
        .first()
        .and_then(|f| f.get("children"))
        .and_then(Value::as_array)
        .unwrap_or_else(|| unreachable!());
    assert_eq!(fanout_children.len(), 1);
    assert_eq!(
        fanout_children.first().and_then(|c| c.get("project_id")),
        Some(&json!("kiln"))
    );
    assert_eq!(
        fanout_children.first().and_then(|c| c.get("status")),
        Some(&json!("pending"))
    );

    // Verify audit records in RunbookRoutingStore
    let audit = checked(store.list_audit(20));
    assert_eq!(audit.len(), 5);
    assert!(
        audit
            .iter()
            .any(|a| a.decision_code == "github_fork_denied")
    );
    assert!(
        audit
            .iter()
            .any(|a| a.decision_code == "runbook_grant_required")
    );
}
