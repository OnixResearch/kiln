use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use kiln_adapter_github::profile::{ConfigEdge, ConfigEffectClass, ConfigNode};
use kiln_core::domain::Outcome;
use kiln_core::fleet::{ClaimState, FleetAgent, FleetClaim, MirrorStatus, parse_scope_text};
use kiln_core::implementation::{
    AcceptanceObligation, CandidateVerification, Enrollment, IMPLEMENTATION_WORK_SCHEMA,
    ImplementationWork, PublicationPhase, PublicationPlan, PublicationRecord, SourceRef,
    SourceStatusReceipt, TerminalPolicy, VERIFICATION_SCHEMA, VerificationOutcome, WorkBudget,
    WorkSourceKind,
};
use kiln_core::provider::CapabilityProfile;
use kiln_core::types::{ContentId, EffectId, ExternalRef};
use kiln_core::workflow::{
    WorkflowEffectPlan, WorkflowNodeObservation, WorkflowNodeState, WorkflowObservation,
    WorkflowState,
};
use kiln_implementer::Implementer;
use kiln_provider_api::{PortResult, WorkflowPort};
use kiln_web_service::access::Principal;
use kiln_web_service::engine::{ApiResponse, Engine, FleetClaimsSnapshot, Role};
use kiln_web_service::profile::ServiceProfile;
use kiln_web_service::rollups::{
    ImplementationWorkIndex, RUNBOOK_ROLLUP_SCHEMA, load_implementation_work_index,
};
use kiln_web_service::runbooks::bundle::{
    BundleVerifyError, verify_bundle_at, verify_registry_bundles,
};
use kiln_web_service::runbooks::{
    ApprovalGate, ParamBinding, ParamDeclaration, ParamType, ParamValue, RUNBOOK_PREVIEW_SCHEMA,
    RUNBOOK_REGISTRY_SCHEMA, RunbookDefinition, RunbookFanoutConfig, RunbookGrant, RunbookLattice,
    RunbookMode, RunbookRegistry, RunbookTarget,
};
use parking_lot::Mutex;
use serde_json::{Value, json};

const CA_PEM: &[u8] = include_bytes!("../../../fixtures/http-observation/tls/ca-cert.pem");
const CERT_PEM: &[u8] = include_bytes!("../../../fixtures/http-observation/tls/cert.pem");
const KEY_PEM: &[u8] = include_bytes!("../../../fixtures/http-observation/tls/key.pem");

fn checked<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|e| unreachable!("fixture failed: {e:?}"))
}

fn present<T>(opt: Option<T>) -> T {
    opt.unwrap_or_else(|| unreachable!("expected value missing"))
}

fn json_body(resp: &ApiResponse) -> Value {
    checked(serde_json::from_slice(&resp.body))
}

fn ptr<'a>(val: &'a Value, path: &str) -> &'a Value {
    present(val.pointer(path))
}

fn repo_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    present(manifest_dir.parent().and_then(Path::parent)).to_path_buf()
}

fn runbooks_dir() -> PathBuf {
    repo_root().join("runbooks")
}

#[derive(Clone, Default)]
struct Backend {
    dispatches: usize,
    plans: BTreeMap<String, WorkflowEffectPlan>,
    sequence: u64,
    terminal: bool,
}

struct SeedPort {
    profile: CapabilityProfile,
    project_id: String,
    backend: Arc<Mutex<Backend>>,
}

impl SeedPort {
    fn observation(
        &self,
        plan: &WorkflowEffectPlan,
        seq: u64,
        terminal: bool,
    ) -> WorkflowObservation {
        WorkflowObservation {
            effect_id: plan.effect_id.clone(),
            attempt_id: plan.attempt_id.clone(),
            candidate_id: plan.candidate_id.clone(),
            pipeline_revision_id: plan.pipeline_revision_id.clone(),
            workflow_policy_id: plan.workflow_policy_id.clone(),
            provider_profile: plan.provider_profile.clone(),
            idempotency_key: plan.idempotency_key.clone(),
            workflow: checked(ExternalRef::new("lattice", "workflow", "seed-wf")),
            execution: checked(ExternalRef::new(
                "lattice",
                "execution",
                &format!("exec-{}", self.project_id),
            )),
            sequence: seq,
            state: if terminal {
                WorkflowState::Completed
            } else {
                WorkflowState::Running
            },
            terminal,
            successful: terminal,
            outcome: if terminal {
                Outcome::Success
            } else {
                Outcome::Pending
            },
            nodes: plan
                .nodes
                .iter()
                .map(|n| WorkflowNodeObservation {
                    node_id: n.id.clone(),
                    state: if terminal {
                        WorkflowNodeState::Completed
                    } else {
                        WorkflowNodeState::Running
                    },
                    terminal,
                    successful: terminal,
                    adapter: checked(ExternalRef::new("lattice", "adapter", "local")),
                    receipt: terminal
                        .then(|| checked(ExternalRef::new("lattice", "receipt", "node-rcpt"))),
                })
                .collect(),
            logs: Vec::new(),
            receipt: terminal.then(|| checked(ExternalRef::new("lattice", "receipt", "exec-rcpt"))),
        }
    }
}

impl WorkflowPort for SeedPort {
    fn profile(&self) -> &CapabilityProfile {
        &self.profile
    }

    fn dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        let mut guard = self.backend.lock();
        guard.dispatches += 1;
        guard.sequence += 1;
        guard
            .plans
            .insert(plan.effect_id.as_str().to_owned(), plan.clone());
        let seq = guard.sequence;
        let terminal = guard.terminal;
        Ok(self.observation(plan, seq, terminal))
    }

    fn recover_dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        let guard = self.backend.lock();
        Ok(self.observation(plan, guard.sequence, guard.terminal))
    }

    fn observe(&mut self, effect_id: &EffectId) -> PortResult<WorkflowObservation> {
        let guard = self.backend.lock();
        let plan = present(guard.plans.get(effect_id.as_str()));
        Ok(self.observation(plan, guard.sequence, guard.terminal))
    }

    fn reconcile(&mut self, id: &EffectId) -> PortResult<WorkflowObservation> {
        self.observe(id)
    }

    fn control(
        &mut self,
        request: &kiln_core::workflow::WorkflowControlRequest,
    ) -> PortResult<WorkflowObservation> {
        self.observe(&request.effect_id)
    }

    fn read_log(
        &mut self,
        request: &kiln_core::workflow::WorkflowLogReadRequest,
    ) -> PortResult<kiln_core::workflow::WorkflowLogChunk> {
        let bytes = b"seed log\n";
        Ok(kiln_core::workflow::WorkflowLogChunk {
            reference: kiln_core::workflow::WorkflowLogReference {
                node_id: request.node_id.clone(),
                stream: request.stream,
                reference: checked(ExternalRef::new("lattice", "log", "seed-log")),
                cursor: request.cursor,
                bytes: bytes.len(),
                truncated: false,
                gap_before: false,
                retained: true,
            },
            bytes: bytes.to_vec(),
        })
    }
}

fn init_git_repo(path: &Path, files: &[(&str, &[u8])]) -> String {
    let repo = checked(git2::Repository::init(path));
    let mut index = checked(repo.index());
    for (rel, bytes) in files {
        let full = path.join(rel);
        if let Some(parent) = full.parent() {
            checked(std::fs::create_dir_all(parent));
        }
        checked(std::fs::write(&full, bytes));
        checked(index.add_path(Path::new(rel)));
    }
    let tree_id = checked(index.write_tree());
    let tree = checked(repo.find_tree(tree_id));
    let sig = checked(git2::Signature::now("seed-test", "seed@example.invalid"));
    let oid = checked(repo.commit(
        Some("refs/heads/main"),
        &sig,
        &sig,
        "seed fixture commit",
        &tree,
        &[],
    ));
    oid.to_string()
}

fn fallback_seed_registry() -> RunbookRegistry {
    RunbookRegistry {
        schema: RUNBOOK_REGISTRY_SCHEMA.into(),
        runbooks: vec![
            RunbookDefinition {
                id: "flake-update".into(),
                display_name: "Flake Update".into(),
                description: "Update flake inputs, run verification checks, and pause at an approval gate before Kiln publication.".into(),
                mode: RunbookMode::Mutating,
                approval_gate: ApprovalGate::KilnPublication,
                lattice: RunbookLattice {
                    bundle_id: Some("flake-update".into()),
                    bundle_identity:
                        "28cf2c469fe40f1eb231baef3ff86aad5ba4b890cea6415fee54aaeb8b8e75eb".into(),
                    workflow_name: "flake-update".into(),
                    workflow_revision:
                        "5bcd6a32798819bd3186f408d76932ef4f79ed6fbb29e0d485f1c1ea93791401".into(),
                    nodes: vec![
                        ConfigNode {
                            id: "update-flake".into(),
                            kind: "build".into(),
                            effect_class: ConfigEffectClass::Build,
                            adapter_kind: "local".into(),
                            operation_identity:
                                "c282c8d89783c97744bb2ef24a163a6b19aca015042a23b33be83d2e1b2e73f8"
                                    .into(),
                        },
                        ConfigNode {
                            id: "verify-flake".into(),
                            kind: "test".into(),
                            effect_class: ConfigEffectClass::Test,
                            adapter_kind: "local".into(),
                            operation_identity:
                                "c3fc0eaea96fd679f457c6c89950b0d9c24441b43213be75d4da9ed8357735ce"
                                    .into(),
                        },
                    ],
                    edges: vec![ConfigEdge {
                        source: "update-flake".into(),
                        target: "verify-flake".into(),
                    }],
                },
                params: vec![
                    ParamDeclaration {
                        name: "flake_dir".into(),
                        param_type: ParamType::RelativePath { max_bytes: 128 },
                        required: false,
                        default: Some(ParamValue::Text(".".into())),
                        bindings: vec![ParamBinding {
                            node: "update-flake".into(),
                            shell_env: "LATTICE_PARAM_FLAKE_DIR".into(),
                        }],
                    },
                    ParamDeclaration {
                        name: "update_scope".into(),
                        param_type: ParamType::Enum {
                            values: vec![
                                "all".into(),
                                "nixpkgs".into(),
                                "cairn".into(),
                                "toolchain".into(),
                            ],
                        },
                        required: false,
                        default: Some(ParamValue::Text("all".into())),
                        bindings: vec![ParamBinding {
                            node: "update-flake".into(),
                            shell_env: "LATTICE_PARAM_UPDATE_SCOPE".into(),
                        }],
                    },
                    ParamDeclaration {
                        name: "run_check".into(),
                        param_type: ParamType::Bool {},
                        required: false,
                        default: Some(ParamValue::Bool(true)),
                        bindings: vec![ParamBinding {
                            node: "update-flake".into(),
                            shell_env: "LATTICE_PARAM_RUN_CHECK".into(),
                        }],
                    },
                ],
                allowed_targets: vec![RunbookTarget {
                    repository: "onixresearch/kiln".into(),
                    project_id: "kiln-flake-update".into(),
                    references: vec!["refs/heads/main".into()],
                    allow_commit_sha: true,
                }],
                fanout: Some(RunbookFanoutConfig {
                    enabled: true,
                    default_max_concurrency: 1,
                    max_concurrency_limit: 4,
                    default_budget_cap: 8,
                    max_budget_cap: 32,
                    max_targets: 16,
                    write_scopes: vec!["flake.lock".into()],
                }),
            },
            RunbookDefinition {
                id: "security-audit".into(),
                display_name: "Security Audit".into(),
                description: "Run Cairn, cargo-audit, and Octet checks and retain a Provenance summary receipt.".into(),
                mode: RunbookMode::ReadOnly,
                approval_gate: ApprovalGate::None,
                lattice: RunbookLattice {
                    bundle_id: Some("security-audit".into()),
                    bundle_identity:
                        "86790db3549cf9ed5646e21546fd031929e687be690af967927df3fe65ceb1ba".into(),
                    workflow_name: "security-audit".into(),
                    workflow_revision:
                        "2cc6f8223cf41a03ea0affa8b2b141d9f03814371d73e2242e2c8ba8f1ef1842".into(),
                    nodes: vec![ConfigNode {
                        id: "audit".into(),
                        kind: "test".into(),
                        effect_class: ConfigEffectClass::Test,
                        adapter_kind: "local".into(),
                        operation_identity:
                            "0c7a51d71e36c886a3afa6d01877536646af25051d7ba2d4dcc2a8c1e8df040a"
                                .into(),
                    }],
                    edges: vec![],
                },
                params: vec![
                    ParamDeclaration {
                        name: "crate_root".into(),
                        param_type: ParamType::RelativePath { max_bytes: 128 },
                        required: false,
                        default: Some(ParamValue::Text(".".into())),
                        bindings: vec![ParamBinding {
                            node: "audit".into(),
                            shell_env: "LATTICE_PARAM_CRATE_ROOT".into(),
                        }],
                    },
                    ParamDeclaration {
                        name: "audit_scope".into(),
                        param_type: ParamType::Enum {
                            values: vec![
                                "all".into(),
                                "cairn".into(),
                                "cargo-audit".into(),
                                "octet".into(),
                            ],
                        },
                        required: false,
                        default: Some(ParamValue::Text("all".into())),
                        bindings: vec![ParamBinding {
                            node: "audit".into(),
                            shell_env: "LATTICE_PARAM_AUDIT_SCOPE".into(),
                        }],
                    },
                    ParamDeclaration {
                        name: "strict".into(),
                        param_type: ParamType::Bool {},
                        required: false,
                        default: Some(ParamValue::Bool(true)),
                        bindings: vec![ParamBinding {
                            node: "audit".into(),
                            shell_env: "LATTICE_PARAM_STRICT".into(),
                        }],
                    },
                ],
                allowed_targets: vec![RunbookTarget {
                    repository: "onixresearch/kiln".into(),
                    project_id: "kiln-security-audit".into(),
                    references: vec!["refs/heads/main".into()],
                    allow_commit_sha: false,
                }],
                fanout: Some(RunbookFanoutConfig {
                    enabled: true,
                    default_max_concurrency: 2,
                    max_concurrency_limit: 8,
                    default_budget_cap: 16,
                    max_budget_cap: 64,
                    max_targets: 32,
                    write_scopes: vec![],
                }),
            },
            RunbookDefinition {
                id: "license-ci-scaffold".into(),
                display_name: "License & CI Scaffold".into(),
                description: "Scaffold LICENSE and GitHub CI workflow files from pinned templates and pause at an approval gate before PR publication.".into(),
                mode: RunbookMode::Mutating,
                approval_gate: ApprovalGate::KilnPublication,
                lattice: RunbookLattice {
                    bundle_id: Some("license-ci-scaffold".into()),
                    bundle_identity:
                        "2b77bd7e6b76a848d181befb75664496ef349014e75647bbbd16f300d5e44c5e".into(),
                    workflow_name: "license-ci-scaffold".into(),
                    workflow_revision:
                        "d61e05e5ad8d240a1a3ee267480ea2a176030903cb663ad653e442c4f39a583a".into(),
                    nodes: vec![
                        ConfigNode {
                            id: "scaffold".into(),
                            kind: "build".into(),
                            effect_class: ConfigEffectClass::Build,
                            adapter_kind: "local".into(),
                            operation_identity:
                                "7d3a118489ad63d1ab82b10e472994e05994f44da8a603cb6657d7823c133604"
                                    .into(),
                        },
                        ConfigNode {
                            id: "verify-scaffold".into(),
                            kind: "test".into(),
                            effect_class: ConfigEffectClass::Test,
                            adapter_kind: "local".into(),
                            operation_identity:
                                "1ff2372e88ad772f890b6e9f583bc5d052e9e76da2f3b95236aec0a8ec262554"
                                    .into(),
                        },
                    ],
                    edges: vec![ConfigEdge {
                        source: "scaffold".into(),
                        target: "verify-scaffold".into(),
                    }],
                },
                params: vec![
                    ParamDeclaration {
                        name: "license_spdx".into(),
                        param_type: ParamType::Enum {
                            values: vec![
                                "apache-2.0".into(),
                                "mit".into(),
                                "mit-or-apache-2.0".into(),
                            ],
                        },
                        required: false,
                        default: Some(ParamValue::Text("mit-or-apache-2.0".into())),
                        bindings: vec![ParamBinding {
                            node: "scaffold".into(),
                            shell_env: "LATTICE_PARAM_LICENSE_SPDX".into(),
                        }],
                    },
                    ParamDeclaration {
                        name: "ci_template".into(),
                        param_type: ParamType::Enum {
                            values: vec![
                                "rust-cargo".into(),
                                "nix-flake".into(),
                                "rust-and-nix".into(),
                            ],
                        },
                        required: false,
                        default: Some(ParamValue::Text("rust-and-nix".into())),
                        bindings: vec![ParamBinding {
                            node: "scaffold".into(),
                            shell_env: "LATTICE_PARAM_CI_TEMPLATE".into(),
                        }],
                    },
                    ParamDeclaration {
                        name: "overwrite_existing".into(),
                        param_type: ParamType::Bool {},
                        required: false,
                        default: Some(ParamValue::Bool(false)),
                        bindings: vec![ParamBinding {
                            node: "scaffold".into(),
                            shell_env: "LATTICE_PARAM_OVERWRITE_EXISTING".into(),
                        }],
                    },
                ],
                allowed_targets: vec![RunbookTarget {
                    repository: "onixresearch/kiln".into(),
                    project_id: "kiln-license-ci-scaffold".into(),
                    references: vec!["refs/heads/main".into()],
                    allow_commit_sha: true,
                }],
                fanout: Some(RunbookFanoutConfig {
                    enabled: true,
                    default_max_concurrency: 1,
                    max_concurrency_limit: 4,
                    default_budget_cap: 8,
                    max_budget_cap: 32,
                    max_targets: 16,
                    write_scopes: vec!["LICENSE".into(), ".github/workflows/ci.yml".into()],
                }),
            },
            RunbookDefinition {
                id: "release-cut".into(),
                display_name: "Release Cut".into(),
                description: "Prepare a verified release candidate and require an approval gate before tag creation or release publication.".into(),
                mode: RunbookMode::Mutating,
                approval_gate: ApprovalGate::KilnPublication,
                lattice: RunbookLattice {
                    bundle_id: Some("release-cut".into()),
                    bundle_identity:
                        "7b0cb77a7ef9ab08366a95b6e63bfdeb669f56e4416497ba88b03e8f121878ac".into(),
                    workflow_name: "release-cut".into(),
                    workflow_revision:
                        "d8709011726f79344aa198bdd5cf01e3530797b4a975a28bb8cba434eb7b3dd5".into(),
                    nodes: vec![
                        ConfigNode {
                            id: "prepare-release".into(),
                            kind: "build".into(),
                            effect_class: ConfigEffectClass::Build,
                            adapter_kind: "local".into(),
                            operation_identity:
                                "6e48823d8aa6dcaff583655da87f6c91d758f891279065665c1ed9dd588be9fa"
                                    .into(),
                        },
                        ConfigNode {
                            id: "verify-release".into(),
                            kind: "test".into(),
                            effect_class: ConfigEffectClass::Test,
                            adapter_kind: "local".into(),
                            operation_identity:
                                "ca024fde4b9ca7bde6ded1079b30d10ab15d48cf740a6debb19c9bdf3f3b5ef4"
                                    .into(),
                        },
                    ],
                    edges: vec![ConfigEdge {
                        source: "prepare-release".into(),
                        target: "verify-release".into(),
                    }],
                },
                params: vec![
                    ParamDeclaration {
                        name: "crate_manifest".into(),
                        param_type: ParamType::RelativePath { max_bytes: 128 },
                        required: false,
                        default: Some(ParamValue::Text("Cargo.toml".into())),
                        bindings: vec![ParamBinding {
                            node: "prepare-release".into(),
                            shell_env: "LATTICE_PARAM_CRATE_MANIFEST".into(),
                        }],
                    },
                    ParamDeclaration {
                        name: "release_channel".into(),
                        param_type: ParamType::Enum {
                            values: vec![
                                "alpha".into(),
                                "beta".into(),
                                "rc".into(),
                                "stable".into(),
                            ],
                        },
                        required: false,
                        default: Some(ParamValue::Text("rc".into())),
                        bindings: vec![ParamBinding {
                            node: "prepare-release".into(),
                            shell_env: "LATTICE_PARAM_RELEASE_CHANNEL".into(),
                        }],
                    },
                    ParamDeclaration {
                        name: "bump_level".into(),
                        param_type: ParamType::Enum {
                            values: vec!["patch".into(), "minor".into(), "major".into()],
                        },
                        required: false,
                        default: Some(ParamValue::Text("patch".into())),
                        bindings: vec![ParamBinding {
                            node: "prepare-release".into(),
                            shell_env: "LATTICE_PARAM_BUMP_LEVEL".into(),
                        }],
                    },
                    ParamDeclaration {
                        name: "allow_dirty".into(),
                        param_type: ParamType::Bool {},
                        required: false,
                        default: Some(ParamValue::Bool(false)),
                        bindings: vec![ParamBinding {
                            node: "prepare-release".into(),
                            shell_env: "LATTICE_PARAM_ALLOW_DIRTY".into(),
                        }],
                    },
                ],
                allowed_targets: vec![RunbookTarget {
                    repository: "onixresearch/kiln".into(),
                    project_id: "kiln-release-cut".into(),
                    references: vec!["refs/heads/main".into()],
                    allow_commit_sha: true,
                }],
                fanout: Some(RunbookFanoutConfig {
                    enabled: true,
                    default_max_concurrency: 1,
                    max_concurrency_limit: 2,
                    default_budget_cap: 4,
                    max_budget_cap: 16,
                    max_targets: 8,
                    write_scopes: vec!["Cargo.toml".into(), "CHANGELOG.md".into()],
                }),
            },
        ],
        grants: vec![RunbookGrant {
            principal: "local:operator".into(),
            runbooks: vec![
                "flake-update".into(),
                "license-ci-scaffold".into(),
                "release-cut".into(),
            ],
            not_after_ms: Some(2_000_000_000_000),
        }],
        notifications: Vec::new(),
    }
}

fn load_seed_registry_or_export() -> RunbookRegistry {
    let seed_ncl = runbooks_dir().join("registry.seed.ncl");
    if let Ok(output) = Command::new("nickel")
        .args(["export", "--format", "json"])
        .arg(&seed_ncl)
        .output()
        && output.status.success()
    {
        let reg: RunbookRegistry = checked(serde_json::from_slice(&output.stdout));
        checked(reg.admit());
        assert_eq!(fallback_seed_registry(), reg);
        return reg;
    }

    fallback_seed_registry()
}

fn copy_dir_all(src: &Path, dst: &Path) {
    checked(std::fs::create_dir_all(dst));
    for entry in checked(std::fs::read_dir(src)) {
        let entry = checked(entry);
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if checked(entry.file_type()).is_dir() {
            copy_dir_all(&path, &target);
        } else {
            checked(std::fs::copy(&path, &target));
        }
    }
}

fn setup_service_for_seed_registry(
    root: &Path,
    registry: &RunbookRegistry,
) -> (
    ServiceProfile,
    Arc<Mutex<Backend>>,
    BTreeMap<String, String>,
) {
    let mut projects = Vec::new();
    let mut revisions = BTreeMap::new();
    for rb in &registry.runbooks {
        for target in &rb.allowed_targets {
            let repo_dir = root.join("repos").join(&target.project_id);
            let rev = init_git_repo(
                &repo_dir,
                &[
                    (
                        "Cargo.toml",
                        b"[package]\nname = \"seed-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
                    ),
                    (
                        "Cargo.lock",
                        b"# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 3\n\n[[package]]\nname = \"seed-fixture\"\nversion = \"0.1.0\"\n",
                    ),
                    ("src/lib.rs", b"#[test]\nfn smoke() {\n    assert_eq!(1 + 1, 2);\n}\n"),
                    ("flake.nix", b"{ outputs = { ... }: {}; }\n"),
                ],
            );
            revisions.insert(target.project_id.clone(), rev);
            projects.push(json!({
                "id": target.project_id,
                "label": target.project_id,
                "repository_path": repo_dir,
                "reference": "refs/heads/main",
                "source_limits": {
                    "max_entries": 64,
                    "max_path_bytes": 1024,
                    "max_file_bytes": 65536,
                    "max_total_bytes": 262144
                },
                "workflow": {
                    "socket_path": root.join(format!("{}.sock", target.project_id)),
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
                    "policy_id": "seed-policy",
                    "provider_profile_id": "lattice-seed",
                    "platform": "linux-x86_64",
                    "nodes": rb.lattice.nodes,
                    "edges": rb.lattice.edges,
                    "controls": { "cancel": true, "pause": false, "resume": false },
                    "bounds": {
                        "max_nodes": 16,
                        "max_edges": 32,
                        "max_log_references": 32,
                        "max_log_read_bytes": 65536,
                        "max_total_log_bytes": 1048576
                    }
                }
            }));
        }
    }

    checked(std::fs::write(root.join("cert.pem"), CERT_PEM));
    checked(std::fs::write(root.join("key.pem"), KEY_PEM));
    checked(std::fs::set_permissions(
        root.join("key.pem"),
        std::fs::Permissions::from_mode(0o600),
    ));
    checked(std::fs::write(root.join("operator.token"), "a".repeat(64)));
    checked(std::fs::set_permissions(
        root.join("operator.token"),
        std::fs::Permissions::from_mode(0o600),
    ));
    checked(std::fs::write(root.join("worker.token"), "b".repeat(64)));
    checked(std::fs::set_permissions(
        root.join("worker.token"),
        std::fs::Permissions::from_mode(0o600),
    ));
    checked(std::fs::create_dir_all(root.join("state")));
    checked(std::fs::set_permissions(
        root.join("state"),
        std::fs::Permissions::from_mode(0o700),
    ));

    let profile: ServiceProfile = checked(serde_json::from_value(json!({
        "schema": "kiln.web-service.v1",
        "service_id": "seed-runbooks-e2e",
        "listen_address": "127.0.0.1:9443",
        "origin": "https://localhost:9443",
        "tls": {
            "certificate_path": root.join("cert.pem"),
            "private_key_path": root.join("key.pem"),
            "min_version": "TLS1.3"
        },
        "operator_token_file": root.join("operator.token"),
        "worker_token_file": root.join("worker.token"),
        "state_directory": root.join("state"),
        "projects": projects,
        "bounds": {
            "max_request_bytes": 131072,
            "max_response_bytes": 1048576,
            "max_header_bytes": 4096,
            "max_connections": 8,
            "timeout_seconds": 5,
            "poll_interval_ms": 25,
            "max_projects": 8,
            "max_runs": 64,
            "max_artifact_bytes": 16384,
            "max_artifacts": 32,
            "max_run_artifact_bytes": 262144,
            "max_total_artifact_bytes": 1048576,
            "max_log_read_bytes": 65536,
            "max_requests": 128,
            "max_events_per_run": 64,
            "max_state_bytes": 4194304
        }
    })));
    checked(profile.admit());
    let backend = Arc::new(Mutex::new(Backend::default()));
    (profile, backend, revisions)
}

fn open_engine_with_ports(profile: ServiceProfile, backend: Arc<Mutex<Backend>>) -> Engine {
    let mut ports: BTreeMap<String, Box<dyn WorkflowPort + Send>> = BTreeMap::new();
    for project in &profile.projects {
        let caps = checked(project.execution_capabilities(profile.bounds.max_runs));
        ports.insert(
            project.id.clone(),
            Box::new(SeedPort {
                profile: caps,
                project_id: project.id.clone(),
                backend: Arc::clone(&backend),
            }),
        );
    }
    checked(Engine::open_with_ports(profile, ports))
}

fn principal(id: &str, role: Role) -> Principal {
    Principal {
        id: id.to_owned(),
        label: id.to_owned(),
        role,
        agent: None,
    }
}

fn post(engine: &mut Engine, path: &str, body: Value, p: &Principal) -> ApiResponse {
    let bytes = checked(serde_json::to_vec(&body));
    checked(engine.api_as("POST", path, &bytes, p))
}

fn get_with_work(
    engine: &mut Engine,
    path: &str,
    p: &Principal,
    work: &ImplementationWorkIndex,
) -> ApiResponse {
    checked(engine.api_as_with_work("GET", path, b"", p, work))
}

#[test]
fn on_disk_seed_bundles_pass_blake3_verification_and_reject_drift_or_symlinks() {
    let reg = load_seed_registry_or_export();
    assert_eq!(reg.runbooks.len(), 4);

    let audit = present(reg.find("security-audit"));
    assert_eq!(audit.mode, RunbookMode::ReadOnly);
    assert_eq!(audit.approval_gate, ApprovalGate::None);
    assert!(
        audit
            .fanout
            .as_ref()
            .is_some_and(|f| f.write_scopes.is_empty())
    );

    for mut_id in ["flake-update", "license-ci-scaffold", "release-cut"] {
        let rb = present(reg.find(mut_id));
        assert_eq!(rb.mode, RunbookMode::Mutating);
        assert_eq!(rb.approval_gate, ApprovalGate::KilnPublication);
    }

    // Real on-disk verification of all 4 bundles under `runbooks/`.
    let rb_root = runbooks_dir();
    checked(verify_registry_bundles(&rb_root, &reg));

    // Also verify directly against the `workflow-templates/` catalog directory layout.
    let wt_root = rb_root
        .join("packs")
        .join("kiln-seed-runbooks")
        .join("workflow-templates");
    for rb in &reg.runbooks {
        let verified = checked(verify_bundle_at(
            &wt_root,
            rb.lattice.bundle_id.as_deref().unwrap_or(&rb.id),
            &rb.lattice.bundle_identity,
        ));
        assert_eq!(verified.bundle_identity, rb.lattice.bundle_identity);
    }

    // Copy to tempdir and verify tamper, undeclared file, symlink, and missing bundle fail closed.
    let tmp = checked(tempfile::tempdir());
    let copy_root = tmp.path().join("runbooks");
    copy_dir_all(&rb_root, &copy_root);
    checked(verify_registry_bundles(&copy_root, &reg));

    // 1. Tamper asset script byte -> Drift
    let audit_script = copy_root.join(
        "packs/kiln-seed-runbooks/workflow-templates/security-audit/assets/run-security-audit.py",
    );
    let mut content = checked(std::fs::read(&audit_script));
    content.extend_from_slice(b"\n# tamper\n");
    checked(std::fs::write(&audit_script, &content));
    assert!(matches!(
        verify_bundle_at(&copy_root, "security-audit", &audit.lattice.bundle_identity),
        Err(BundleVerifyError::Drift(_))
    ));

    // 2. Undeclared extra file -> Drift
    let flake_dir = copy_root.join("packs/kiln-seed-runbooks/workflow-templates/flake-update");
    checked(std::fs::write(flake_dir.join("undeclared.txt"), b"rogue"));
    let flake = present(reg.find("flake-update"));
    assert!(matches!(
        verify_bundle_at(&copy_root, "flake-update", &flake.lattice.bundle_identity),
        Err(BundleVerifyError::Drift(_))
    ));

    // 3. Symlink inside bundle -> Drift
    let release_assets =
        copy_root.join("packs/kiln-seed-runbooks/workflow-templates/release-cut/assets");
    checked(std::os::unix::fs::symlink(
        "/etc/passwd",
        release_assets.join("evil-link"),
    ));
    let release = present(reg.find("release-cut"));
    assert!(matches!(
        verify_bundle_at(&copy_root, "release-cut", &release.lattice.bundle_identity),
        Err(BundleVerifyError::Drift(_))
    ));

    // 4. Missing bundle -> Missing
    assert!(matches!(
        verify_bundle_at(&copy_root, "missing-bundle", &"a".repeat(64)),
        Err(BundleVerifyError::Missing(_))
    ));
}

#[test]
fn preview_path_enforces_rbac_zero_side_effects_claim_overlap_and_bundle_drift() {
    let reg = load_seed_registry_or_export();
    let dir = checked(tempfile::tempdir());
    let (profile, backend, revisions) = setup_service_for_seed_registry(dir.path(), &reg);
    let mut engine = open_engine_with_ports(profile, Arc::clone(&backend));

    for rb in &reg.runbooks {
        for target in &rb.allowed_targets {
            let enroll = post(
                &mut engine,
                "/api/projects",
                json!({
                    "project_id": target.project_id,
                    "request_id": format!("enroll-{}", target.project_id)
                }),
                &principal("local:operator", Role::Operator),
            );
            assert_eq!(enroll.status, 200);
        }
    }

    // Copy runbooks to tempdir so we can test verified + tampered bundles against Engine.
    let rb_copy = dir.path().join("runbooks");
    copy_dir_all(&runbooks_dir(), &rb_copy);
    checked(engine.configure_runbooks(reg.clone()));
    checked(engine.configure_runbooks_dir(&rb_copy));

    let op_granted = principal("local:operator", Role::Operator);
    let op_ungranted = principal("local:other", Role::Operator);
    let viewer = principal("local:viewer", Role::Viewer);
    let worker = principal("worker:w1", Role::Worker);

    // RBAC: Viewer -> 403 operator_required; Worker -> 403 worker_scope
    let v_resp = post(
        &mut engine,
        "/api/runbooks/security-audit/preview",
        json!({ "target": "onixresearch/kiln@refs/heads/main" }),
        &viewer,
    );
    assert_eq!(v_resp.status, 403);
    assert_eq!(
        ptr(&json_body(&v_resp), "/code"),
        &json!("operator_required")
    );

    let w_resp = post(
        &mut engine,
        "/api/runbooks/security-audit/preview",
        json!({ "target": "onixresearch/kiln@refs/heads/main" }),
        &worker,
    );
    assert_eq!(w_resp.status, 403);
    assert_eq!(ptr(&json_body(&w_resp), "/code"), &json!("worker_scope"));

    // Ungranted operator can preview mutating runbook and sees can_dispatch=false, dispatch_refusal=runbook_grant_required
    let ungranted_resp = post(
        &mut engine,
        "/api/runbooks/flake-update/preview",
        json!({
            "target": "onixresearch/kiln@refs/heads/main",
            "params": { "update_scope": "nixpkgs", "run_check": true }
        }),
        &op_ungranted,
    );
    assert_eq!(ungranted_resp.status, 200);
    let ungranted_json = json_body(&ungranted_resp);
    assert_eq!(
        ptr(&ungranted_json, "/schema"),
        &json!(RUNBOOK_PREVIEW_SCHEMA)
    );
    assert_eq!(ptr(&ungranted_json, "/dry_run"), &json!(true));
    assert_eq!(ptr(&ungranted_json, "/grants_authority"), &json!(false));
    assert_eq!(ptr(&ungranted_json, "/can_dispatch"), &json!(false));
    assert_eq!(
        ptr(&ungranted_json, "/dispatch_refusal"),
        &json!("runbook_grant_required")
    );
    assert_eq!(
        ptr(&ungranted_json, "/approval_gate"),
        &json!("kiln_publication")
    );
    assert_eq!(ptr(&ungranted_json, "/bundle_pin_verified"), &json!(true));

    let ungranted_ro = post(
        &mut engine,
        "/api/runbooks/security-audit/preview",
        json!({ "target": "onixresearch/kiln@refs/heads/main" }),
        &op_ungranted,
    );
    assert_eq!(ungranted_ro.status, 200);
    let ungranted_ro_json = json_body(&ungranted_ro);
    assert_eq!(ptr(&ungranted_ro_json, "/dry_run"), &json!(true));
    assert_eq!(ptr(&ungranted_ro_json, "/grants_authority"), &json!(false));
    assert_eq!(ptr(&ungranted_ro_json, "/can_dispatch"), &json!(true));
    assert!(ptr(&ungranted_ro_json, "/dispatch_refusal").is_null());

    let granted_single = post(
        &mut engine,
        "/api/runbooks/flake-update/preview",
        json!({
            "target": "onixresearch/kiln@refs/heads/main",
            "params": { "update_scope": "nixpkgs", "run_check": true }
        }),
        &op_granted,
    );
    assert_eq!(granted_single.status, 200);
    let granted_single_json = json_body(&granted_single);
    assert_eq!(ptr(&granted_single_json, "/dry_run"), &json!(true));
    assert_eq!(
        ptr(&granted_single_json, "/grants_authority"),
        &json!(false)
    );
    assert_eq!(ptr(&granted_single_json, "/can_dispatch"), &json!(true));
    assert!(ptr(&granted_single_json, "/dispatch_refusal").is_null());
    assert_eq!(
        ptr(&granted_single_json, "/bundle_pin_verified"),
        &json!(true)
    );

    // Granted operator previews fan-out with live fleet claim overlap!
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(1_000_000);
    engine.set_fleet_claims_snapshot(FleetClaimsSnapshot::Available(vec![FleetClaim {
        identity: checked(ContentId::from_blake3_hex("f".repeat(64))),
        scope: checked(parse_scope_text("onixresearch/kiln flake.lock")),
        holder: FleetAgent {
            tool: "codex".into(),
            host: "aspen3".into(),
            session: "other-agent".into(),
        },
        purpose: "update flake lock".into(),
        fence: 706,
        initial_fence: None,
        granted_ms: now_ms.saturating_sub(1_000),
        expires_ms: now_ms.saturating_add(600_000),
        state: ClaimState::Open,
        attempt: None,
        claim_mirror: MirrorStatus::Posted { message_id: None },
        release_mirror: MirrorStatus::Pending,
    }]));
    let fo_preview = post(
        &mut engine,
        "/api/runbooks/flake-update/preview",
        json!({
            "targets": ["onixresearch/kiln@refs/heads/main"],
            "max_concurrency": 2,
            "budget_cap": 4,
            "claim_policy": "refuse"
        }),
        &op_granted,
    );
    assert_eq!(fo_preview.status, 200);
    let fo_json = json_body(&fo_preview);
    assert_eq!(ptr(&fo_json, "/can_dispatch"), &json!(true));
    assert_eq!(
        ptr(&fo_json, "/targets/0/source_revision"),
        &json!(present(revisions.get("kiln-flake-update")))
    );
    assert_eq!(
        ptr(&fo_json, "/targets/0/write_scopes"),
        &json!(["onixresearch/kiln flake.lock"])
    );
    assert!(
        ptr(&fo_json, "/targets/0/claim_conflict")
            .as_str()
            .is_some_and(|s| s.contains("other-agent"))
    );
    assert_eq!(ptr(&fo_json, "/budget/estimated_units"), &json!(1));
    assert_eq!(ptr(&fo_json, "/budget/budget_cap"), &json!(4));
    assert_eq!(ptr(&fo_json, "/budget/within_budget"), &json!(true));

    // Zero side effects: 0 runs, 0 WorkflowPort dispatches!
    assert_eq!(backend.lock().dispatches, 0);
    let runs_resp = get_with_work(
        &mut engine,
        "/api/runs",
        &viewer,
        &ImplementationWorkIndex::Unconfigured,
    );
    assert_eq!(ptr(&json_body(&runs_resp), "/runs"), &json!([]));

    // Tamper bundle on disk -> both preview and dispatch fail closed with 409 runbook_bundle_drift!
    let wf_path =
        rb_copy.join("packs/kiln-seed-runbooks/workflow-templates/flake-update/workflow.ncl");
    checked(std::fs::write(&wf_path, b"{ tampered = true }\n"));
    let drifted_preview = post(
        &mut engine,
        "/api/runbooks/flake-update/preview",
        json!({ "target": "onixresearch/kiln@refs/heads/main" }),
        &op_granted,
    );
    assert_eq!(drifted_preview.status, 409);
    assert_eq!(
        ptr(&json_body(&drifted_preview), "/code"),
        &json!("runbook_bundle_drift")
    );

    let drifted_dispatch = post(
        &mut engine,
        "/api/runbooks/flake-update/dispatch",
        json!({
            "request_id": "should-fail-drift",
            "target": "onixresearch/kiln@refs/heads/main"
        }),
        &op_granted,
    );
    assert_eq!(drifted_dispatch.status, 409);
    assert_eq!(
        ptr(&json_body(&drifted_dispatch), "/code"),
        &json!("runbook_bundle_drift")
    );
    assert_eq!(backend.lock().dispatches, 0);
}

#[test]
fn security_audit_executes_read_only_script_retains_provenance_receipt_and_surfaces_in_rollup() {
    let reg = load_seed_registry_or_export();
    let dir = checked(tempfile::tempdir());
    let (profile, backend, revisions) = setup_service_for_seed_registry(dir.path(), &reg);
    let mut engine = open_engine_with_ports(profile, Arc::clone(&backend));

    let op = principal("local:operator", Role::Operator);
    let worker = principal("worker:w1", Role::Worker);
    let viewer = principal("local:viewer", Role::Viewer);

    assert_eq!(
        post(
            &mut engine,
            "/api/projects",
            json!({
                "project_id": "kiln-security-audit",
                "request_id": "enroll-audit"
            }),
            &op
        )
        .status,
        200
    );
    checked(engine.configure_runbooks(reg));
    checked(engine.configure_runbooks_dir(&runbooks_dir()));

    let disp = post(
        &mut engine,
        "/api/runbooks/security-audit/dispatch",
        json!({
            "request_id": "audit-e2e-1",
            "target": "onixresearch/kiln@refs/heads/main",
            "params": { "audit_scope": "all", "strict": false }
        }),
        &op,
    );
    assert_eq!(disp.status, 201);
    let disp_json = json_body(&disp);
    let run_id = present(ptr(&disp_json, "/run_id").as_str()).to_owned();
    assert_eq!(ptr(&disp_json, "/mode"), &json!("read_only"));
    assert_eq!(ptr(&disp_json, "/approval_gate"), &json!("none"));
    assert_eq!(
        ptr(&disp_json, "/run/phase"),
        &json!("active"),
        "unexpected run phase: {disp_json}"
    );
    assert!(
        !ptr(&disp_json, "/run/execution").is_null(),
        "missing execution: {disp_json}"
    );

    // Fetch execution-binding as worker and run the REAL `assets/run-security-audit.py` script!
    let binding_resp = get_with_work(
        &mut engine,
        &format!("/api/runs/{run_id}/execution-binding"),
        &worker,
        &ImplementationWorkIndex::Unconfigured,
    );
    assert_eq!(binding_resp.status, 200);
    let binding = json_body(&binding_resp);
    let node_id = present(ptr(&binding, "/plan/nodes/0/id").as_str()).to_owned();
    let attempt_id = present(ptr(&disp_json, "/run/attempt_id").as_str()).to_owned();
    let source_revision = present(ptr(&disp_json, "/run/source_revision").as_str()).to_owned();
    let execution = ptr(&disp_json, "/run/execution").clone();

    let script_path = runbooks_dir().join(
        "packs/kiln-seed-runbooks/workflow-templates/security-audit/assets/run-security-audit.py",
    );
    let target_checkout = dir.path().join("repos/kiln-security-audit");
    let receipt_out = dir.path().join("security-audit-receipt.json");
    let expected_sec_rev = present(revisions.get("kiln-security-audit"));

    let script_output = checked(
        Command::new("python3")
            .arg(&script_path)
            .args([".", "all", "false"])
            .env("KILN_WORK_DIR", &target_checkout)
            .env("KILN_RECEIPT_PATH", &receipt_out)
            .env("LATTICE_SOURCE_REPOSITORY", "onixresearch/kiln")
            .env("LATTICE_SOURCE_REVISION", expected_sec_rev)
            .output(),
    );
    assert!(
        script_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&script_output.stderr)
    );
    let receipt_bytes = checked(std::fs::read(&receipt_out));
    let receipt_json: Value = checked(serde_json::from_slice(&receipt_bytes));
    assert_eq!(
        ptr(&receipt_json, "/schema"),
        &json!("kiln.runbook.security-audit-receipt.v1")
    );
    assert_eq!(ptr(&receipt_json, "/grants_authority"), &json!(false));
    assert_eq!(ptr(&receipt_json, "/mode"), &json!("read_only"));
    assert_eq!(
        ptr(&receipt_json, "/source_revision"),
        &json!(expected_sec_rev)
    );

    // Upload the receipt as ReportKind::Provenance via worker report endpoint!
    let report_resp = post(
        &mut engine,
        &format!("/api/runs/{run_id}/reports"),
        json!({
            "run_id": run_id,
            "source_revision": source_revision,
            "attempt_id": attempt_id,
            "execution": execution,
            "node_id": node_id,
            "kind": "provenance",
            "name": "security-audit-receipt.json",
            "blake3": blake3::hash(&receipt_bytes).to_hex().to_string(),
            "body_base64": base64::engine::general_purpose::STANDARD.encode(&receipt_bytes),
        }),
        &worker,
    );
    assert_eq!(report_resp.status, 201, "body: {}", json_body(&report_resp));

    // Complete the run via backend + Engine::poll() and verify #100 roll-up surfaces the Provenance receipt cleanly!
    {
        let mut guard = backend.lock();
        guard.terminal = true;
        guard.sequence += 1;
    }
    checked(engine.poll());

    let rollup_resp = get_with_work(
        &mut engine,
        &format!("/api/runbooks/dispatches/{run_id}/rollup"),
        &viewer,
        &ImplementationWorkIndex::Unconfigured,
    );
    assert_eq!(rollup_resp.status, 200);
    let rollup = json_body(&rollup_resp);
    assert_eq!(ptr(&rollup, "/schema"), &json!(RUNBOOK_ROLLUP_SCHEMA));
    assert_eq!(ptr(&rollup, "/grants_authority"), &json!(false));
    assert_eq!(
        ptr(&rollup, "/targets/0/verification_receipt/grants_authority"),
        &json!(false)
    );
    assert_eq!(
        ptr(
            &rollup,
            "/targets/0/verification_receipt/run_reports/0/kind"
        ),
        &json!("provenance")
    );
    assert_eq!(
        ptr(
            &rollup,
            "/targets/0/verification_receipt/run_reports/0/name"
        ),
        &json!("security-audit-receipt.json")
    );
}

#[test]
fn mutating_seed_runbooks_enforce_approval_gate_publisher_boundary_and_surface_rollup() {
    let reg = load_seed_registry_or_export();
    let dir = checked(tempfile::tempdir());
    let (profile, backend, revisions) = setup_service_for_seed_registry(dir.path(), &reg);
    let mut engine = open_engine_with_ports(profile, Arc::clone(&backend));

    let op_granted = principal("local:operator", Role::Operator);
    let op_ungranted = principal("local:other", Role::Operator);
    let viewer = principal("local:viewer", Role::Viewer);

    for id in [
        "kiln-flake-update",
        "kiln-license-ci-scaffold",
        "kiln-release-cut",
    ] {
        assert_eq!(
            post(
                &mut engine,
                "/api/projects",
                json!({ "project_id": id, "request_id": format!("enroll-{id}") }),
                &op_granted
            )
            .status,
            200
        );
    }
    checked(engine.configure_runbooks(reg));
    checked(engine.configure_runbooks_dir(&runbooks_dir()));

    // 1. Ungranted operator cannot dispatch mutating runbooks.
    for id in ["flake-update", "license-ci-scaffold", "release-cut"] {
        let denied = post(
            &mut engine,
            &format!("/api/runbooks/{id}/dispatch"),
            json!({
                "request_id": format!("denied-{id}"),
                "target": "onixresearch/kiln@refs/heads/main"
            }),
            &op_ungranted,
        );
        assert_eq!(denied.status, 403);
        assert_eq!(
            ptr(&json_body(&denied), "/code"),
            &json!("runbook_grant_required")
        );
    }

    // 2. Execute real worker asset scripts for license-ci-scaffold and release-cut without push/PR credentials!
    let scaffold_repo = dir.path().join("repos/kiln-license-ci-scaffold");
    let scaffold_script = runbooks_dir()
        .join("packs/kiln-seed-runbooks/workflow-templates/license-ci-scaffold/assets/run-license-ci-scaffold.py");
    let scaffold_out = checked(
        Command::new("python3")
            .arg(&scaffold_script)
            .args(["mit-or-apache-2.0", "rust-and-nix", "false"])
            .env("KILN_WORK_DIR", &scaffold_repo)
            .output(),
    );
    assert!(
        scaffold_out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&scaffold_out.stderr)
    );
    assert!(scaffold_repo.join("LICENSE").is_file());
    assert!(scaffold_repo.join(".github/workflows/ci.yml").is_file());

    let release_repo = dir.path().join("repos/kiln-release-cut");
    let release_script = runbooks_dir()
        .join("packs/kiln-seed-runbooks/workflow-templates/release-cut/assets/run-release-cut.py");
    let release_out = checked(
        Command::new("python3")
            .arg(&release_script)
            .args(["Cargo.toml", "rc", "patch", "true"])
            .env("KILN_WORK_DIR", &release_repo)
            .output(),
    );
    assert!(
        release_out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&release_out.stderr)
    );
    let release_receipt: Value = checked(serde_json::from_slice(&checked(std::fs::read(
        release_repo.join("release-cut-receipt.json"),
    ))));
    assert_eq!(
        ptr(&release_receipt, "/approval_gate"),
        &json!("kiln_publication")
    );
    assert_eq!(ptr(&release_receipt, "/grants_authority"), &json!(false));
    assert_eq!(
        ptr(&release_receipt, "/candidate_tag"),
        &json!("v0.1.1-rc.1")
    );

    // 3. Dispatch flake-update as granted operator, verify approval_gate=kiln_publication in binding.
    let disp = post(
        &mut engine,
        "/api/runbooks/flake-update/dispatch",
        json!({
            "request_id": "flake-e2e-1",
            "target": "onixresearch/kiln@refs/heads/main",
            "params": { "update_scope": "nixpkgs", "run_check": true }
        }),
        &op_granted,
    );
    assert_eq!(disp.status, 201);
    let disp_json = json_body(&disp);
    let run_id = present(ptr(&disp_json, "/run_id").as_str()).to_owned();
    assert_eq!(
        ptr(&disp_json, "/approval_gate"),
        &json!("kiln_publication")
    );

    // 4. Separate Implementer publisher boundary: cannot publish without GrantRole::Publisher,
    //    and once verified + published, surfaces PR link in #100 roll-up!
    let impl_db = dir.path().join("impl-work.sqlite");
    let sha = present(revisions.get("kiln-flake-update")).clone();
    let enrollment = Enrollment {
        repositories: BTreeSet::from(["onixresearch/kiln".to_owned()]),
        actors: BTreeSet::from(["local:operator".to_owned()]),
    };
    let mut impl_store = checked(Implementer::open_with_local_operator(
        &impl_db,
        enrollment,
        "local:operator",
    ));
    let work = ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.into(),
        revision: 1,
        actor: "local:operator".into(),
        repository: "onixresearch/kiln".into(),
        project: "kiln-flake-update".into(),
        base: None,
        title: "flake-update".into(),
        requested_outcome: "update flake lock".into(),
        acceptance: vec![AcceptanceObligation {
            id: "a1".into(),
            statement: "verified".into(),
        }],
        dependencies: Vec::new(),
        permitted_paths: vec!["flake.lock".into()],
        permitted_effects: BTreeSet::from(["write_source".to_owned()]),
        verification_profile: checked(ContentId::from_blake3_hex("9".repeat(64))),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 30,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::CairnTask,
            source: checked(ExternalRef::new("cairn", "task", "seed-flake/task-1")),
            native_id: "seed-flake/task-1".into(),
            version: checked(ContentId::from_blake3_hex("1".repeat(64))),
        }],
    };
    checked(impl_store.admit(&work, None));
    let attempt = present(checked(impl_store.claim_next(1, 60_000)));
    let candidate_ref = checked(ExternalRef::new("git", "commit", &sha));
    let tree_ref = checked(ExternalRef::new("git", "tree", &"d".repeat(40)));
    let base_ref = checked(ExternalRef::new("git", "commit", &"e".repeat(40)));
    let handoff_id = checked(ContentId::from_blake3_hex("2".repeat(64)));
    let verification_id = checked(ContentId::from_blake3_hex("3".repeat(64)));
    let verification = CandidateVerification {
        schema: VERIFICATION_SCHEMA.into(),
        handoff: handoff_id.clone(),
        work_identity: attempt.work_identity.clone(),
        work_revision: attempt.work_revision,
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        candidate: candidate_ref.clone(),
        tree: tree_ref.clone(),
        base: Some(base_ref.clone()),
        verification_profile: work.verification_profile.clone(),
        run: kiln_core::change_review::CiFact {
            project: "kiln-flake-update".into(),
            repository: "onixresearch/kiln".into(),
            revision: sha.clone(),
            run_id: run_id.clone(),
            ordinal: 1,
            success: true,
            full_run: true,
            clean: true,
            scope_identity: None,
            terminal: true,
            untrusted: None,
            checks: Vec::new(),
        },
    };
    let pub_rec = PublicationRecord {
        schema: kiln_core::implementation::PUBLICATION_SCHEMA.into(),
        plan: PublicationPlan {
            schema: "kiln.implementation-publication-plan.v1".into(),
            work_identity: attempt.work_identity.clone(),
            work_revision: attempt.work_revision,
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
            handoff: handoff_id,
            verification: verification_id,
            repository: "onixresearch/kiln".into(),
            base: base_ref,
            base_branch: "main".into(),
            candidate: candidate_ref,
            tree: tree_ref,
            branch: format!("kiln/impl-{}", &attempt.identity.as_str()[..12]),
            title: "Seed flake update".into(),
            body: "Automated flake update".into(),
        },
        phase: PublicationPhase::PullRequestPublished,
        remote_head: None,
        pull_request: Some(checked(ExternalRef::new(
            "github",
            "pull_request",
            "onixresearch/kiln#999",
        ))),
        integration: None,
        source_status: Some(SourceStatusReceipt {
            schema: kiln_core::implementation::SOURCE_STATUS_SCHEMA.into(),
            work_identity: attempt.work_identity.clone(),
            work_revision: attempt.work_revision,
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
            integration: checked(ContentId::from_blake3_hex("4".repeat(64))),
            source: present(work.aliases.first().cloned()),
            state: "open".into(),
            observed_revision: 1,
        }),
    };

    // Update attempt ledger to mirror verified + publisher-opened PR without raw publisher secrets in Engine!
    {
        let conn = checked(rusqlite::Connection::open(&impl_db));
        let bytes: Vec<u8> = checked(conn.query_row(
            "SELECT bytes FROM implementation_state WHERE kind='attempts'",
            [],
            |r| r.get(0),
        ));
        let mut ledger: kiln_core::implementation::AttemptLedger =
            checked(serde_json::from_slice(&bytes));
        let mut val = checked(serde_json::to_value(&ledger));
        let obj = present(
            val.pointer_mut(&format!("/attempts/{}", attempt.identity.as_str()))
                .and_then(Value::as_object_mut),
        );
        obj.insert(
            "candidate".into(),
            checked(serde_json::to_value(checked(ExternalRef::new(
                "git", "commit", &sha,
            )))),
        );
        obj.insert(
            "verification".into(),
            checked(serde_json::to_value(&verification)),
        );
        obj.insert(
            "verification_outcome".into(),
            checked(serde_json::to_value(VerificationOutcome::Verified)),
        );
        obj.insert(
            "publication".into(),
            checked(serde_json::to_value(&pub_rec)),
        );
        ledger = checked(serde_json::from_value(val));
        checked(conn.execute(
            "UPDATE implementation_state SET bytes=?1 WHERE kind='attempts'",
            rusqlite::params![checked(serde_json::to_vec(&ledger))],
        ));
    }

    let impl_work = load_implementation_work_index(&impl_db);
    let rollup_resp = get_with_work(
        &mut engine,
        &format!("/api/runbooks/dispatches/{run_id}/rollup"),
        &viewer,
        &impl_work,
    );
    assert_eq!(rollup_resp.status, 200);
    let rollup = json_body(&rollup_resp);
    assert_eq!(ptr(&rollup, "/grants_authority"), &json!(false));
    assert_eq!(
        ptr(&rollup, "/targets/0/pull_requests/0/url"),
        &json!("https://github.com/onixresearch/kiln/pull/999")
    );
    assert_eq!(
        ptr(
            &rollup,
            "/targets/0/verification_receipt/implementation_observation_status"
        ),
        &json!("available")
    );
    assert_eq!(
        ptr(
            &rollup,
            "/targets/0/verification_receipt/implementation_verification/verification_outcome"
        ),
        &json!("Verified")
    );
    assert_eq!(
        ptr(
            &rollup,
            "/targets/0/verification_receipt/implementation_verification/run_id"
        ),
        &json!(run_id)
    );
}

fn find_exe(name: &str) -> PathBuf {
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return candidate;
        }
    }
    unreachable!("required executable missing from PATH: {name}")
}

#[test]
fn license_ci_scaffold_refuses_dangling_license_and_symlinked_workflow_dir() {
    let python3 = find_exe("python3");
    let scaffold_script = runbooks_dir().join(
        "packs/kiln-seed-runbooks/workflow-templates/license-ci-scaffold/assets/run-license-ci-scaffold.py",
    );
    let tmp = checked(tempfile::tempdir());
    let outside = tmp.path().join("outside");
    checked(std::fs::create_dir_all(&outside));

    // 1. Dangling LICENSE symlink (both overwrite=false and overwrite=true)
    for (idx, overwrite) in ["false", "true"].into_iter().enumerate() {
        let repo = tmp.path().join(format!("repo-license-{idx}"));
        checked(std::fs::create_dir_all(&repo));
        let pwned = outside.join(format!("pwned-license-{idx}"));
        checked(std::os::unix::fs::symlink(&pwned, repo.join("LICENSE")));
        let out = checked(
            Command::new(&python3)
                .arg(&scaffold_script)
                .args(["mit", "rust-cargo", overwrite])
                .env("KILN_WORK_DIR", &repo)
                .env("PATH", "")
                .output(),
        );
        assert_eq!(
            out.status.code(),
            Some(2),
            "expected exit 2 on dangling LICENSE symlink (overwrite={overwrite}), stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(!pwned.exists());
        assert!(!repo.join(".github").exists());
    }

    // 2. Symlinked .github/workflows directory (must fail before writing LICENSE too)
    let repo2 = tmp.path().join("repo-workflows-link");
    checked(std::fs::create_dir_all(repo2.join(".github")));
    checked(std::os::unix::fs::symlink(
        &outside,
        repo2.join(".github/workflows"),
    ));
    let out2 = checked(
        Command::new(&python3)
            .arg(&scaffold_script)
            .args(["mit", "rust-cargo", "false"])
            .env("KILN_WORK_DIR", &repo2)
            .env("PATH", "")
            .output(),
    );
    assert_eq!(out2.status.code(), Some(2));
    assert!(!outside.join("ci.yml").exists());
    assert!(!repo2.join("LICENSE").exists());

    // 3. Symlinked .github directory
    let repo3 = tmp.path().join("repo-dot-github-link");
    checked(std::fs::create_dir_all(&repo3));
    checked(std::os::unix::fs::symlink(&outside, repo3.join(".github")));
    let out3 = checked(
        Command::new(&python3)
            .arg(&scaffold_script)
            .args(["mit", "rust-cargo", "false"])
            .env("KILN_WORK_DIR", &repo3)
            .env("PATH", "")
            .output(),
    );
    assert_eq!(out3.status.code(), Some(2));
    assert!(!outside.join("workflows/ci.yml").exists());
    assert!(!repo3.join("LICENSE").exists());

    // 4. Dangling .github/workflows/ci.yml symlink
    let repo4 = tmp.path().join("repo-ci-yml-link");
    checked(std::fs::create_dir_all(repo4.join(".github/workflows")));
    let pwned_ci = outside.join("pwned-ci.yml");
    checked(std::os::unix::fs::symlink(
        &pwned_ci,
        repo4.join(".github/workflows/ci.yml"),
    ));
    let out4 = checked(
        Command::new(&python3)
            .arg(&scaffold_script)
            .args(["mit", "rust-cargo", "true"])
            .env("KILN_WORK_DIR", &repo4)
            .env("PATH", "")
            .output(),
    );
    assert_eq!(out4.status.code(), Some(2));
    assert!(!pwned_ci.exists());
    assert!(!repo4.join("LICENSE").exists());
}

#[test]
fn seed_scripts_refuse_symlinked_default_and_explicit_receipt_paths() {
    let python3 = find_exe("python3");
    let wt_dir = runbooks_dir().join("packs/kiln-seed-runbooks/workflow-templates");
    let tmp = checked(tempfile::tempdir());
    let outside = tmp.path().join("outside");
    checked(std::fs::create_dir_all(&outside));

    let cases: [(&str, &str, &str, &[&str]); 4] = [
        (
            "flake-update",
            "assets/run-flake-update.py",
            "flake-update-receipt.json",
            &[".", "all", "true"],
        ),
        (
            "security-audit",
            "assets/run-security-audit.py",
            "security-audit-receipt.json",
            &[".", "all", "false"],
        ),
        (
            "license-ci-scaffold",
            "assets/run-license-ci-scaffold.py",
            "license-ci-scaffold-receipt.json",
            &["mit", "rust-cargo", "false"],
        ),
        (
            "release-cut",
            "assets/run-release-cut.py",
            "release-cut-receipt.json",
            &["Cargo.toml", "rc", "patch", "false"],
        ),
    ];

    for (name, rel_script, default_receipt, args) in cases {
        let script = wt_dir.join(name).join(rel_script);

        // Subcase A: default relative receipt inside work_root is a dangling symlink
        let repo_a = tmp.path().join(format!("{name}-default"));
        checked(std::fs::create_dir_all(&repo_a));
        checked(std::fs::write(
            repo_a.join("flake.nix"),
            b"{ outputs = { ... }: {}; }\n",
        ));
        checked(std::fs::write(
            repo_a.join("Cargo.toml"),
            b"[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        ));
        let escaped_a = outside.join(format!("{name}-default.json"));
        checked(std::os::unix::fs::symlink(
            &escaped_a,
            repo_a.join(default_receipt),
        ));
        let out_a = checked(
            Command::new(&python3)
                .arg(&script)
                .args(args)
                .env("KILN_WORK_DIR", &repo_a)
                .env("PATH", "")
                .output(),
        );
        assert_eq!(
            out_a.status.code(),
            Some(2),
            "{name} default symlink receipt did not exit 2, stderr: {}",
            String::from_utf8_lossy(&out_a.stderr)
        );
        assert!(!escaped_a.exists());

        // Subcase B: explicit KILN_RECEIPT_PATH pointing to a symlink
        let repo_b = tmp.path().join(format!("{name}-explicit"));
        checked(std::fs::create_dir_all(&repo_b));
        checked(std::fs::write(
            repo_b.join("flake.nix"),
            b"{ outputs = { ... }: {}; }\n",
        ));
        checked(std::fs::write(
            repo_b.join("Cargo.toml"),
            b"[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        ));
        let escaped_b = outside.join(format!("{name}-explicit.json"));
        let explicit_link = tmp.path().join(format!("{name}-explicit-link.json"));
        checked(std::os::unix::fs::symlink(&escaped_b, &explicit_link));
        let out_b = checked(
            Command::new(&python3)
                .arg(&script)
                .args(args)
                .env("KILN_WORK_DIR", &repo_b)
                .env("KILN_RECEIPT_PATH", &explicit_link)
                .env("PATH", "")
                .output(),
        );
        assert_eq!(
            out_b.status.code(),
            Some(2),
            "{name} explicit symlink receipt did not exit 2, stderr: {}",
            String::from_utf8_lossy(&out_b.stderr)
        );
        assert!(!escaped_b.exists());
    }
}

#[test]
fn release_cut_and_license_scaffold_report_unavailable_when_git_or_cargo_missing() {
    let python3 = find_exe("python3");
    let cargo_bin = find_exe("cargo");
    let git_bin = find_exe("git");
    let wt_dir = runbooks_dir().join("packs/kiln-seed-runbooks/workflow-templates");
    let release_script = wt_dir.join("release-cut/assets/run-release-cut.py");
    let scaffold_script = wt_dir.join("license-ci-scaffold/assets/run-license-ci-scaffold.py");

    let tmp = checked(tempfile::tempdir());
    let bin_only_cargo = tmp.path().join("bin-only-cargo");
    let bin_only_git = tmp.path().join("bin-only-git");
    checked(std::fs::create_dir_all(&bin_only_cargo));
    checked(std::fs::create_dir_all(&bin_only_git));
    checked(std::os::unix::fs::symlink(
        &cargo_bin,
        bin_only_cargo.join("cargo"),
    ));
    checked(std::os::unix::fs::symlink(
        &git_bin,
        bin_only_git.join("git"),
    ));

    let cargo_files: [(&str, &[u8]); 3] = [
        (
            "Cargo.toml",
            b"[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        ),
        (
            "Cargo.lock",
            b"# This file is automatically @generated by Cargo.\nversion = 3\n\n[[package]]\nname = \"x\"\nversion = \"0.1.0\"\n",
        ),
        ("src/lib.rs", b"#[test]\nfn ok() { assert_eq!(1, 1); }\n"),
    ];

    // 1. release-cut with .git present and git missing on PATH
    let repo_a = tmp.path().join("repo-missing-git");
    let _ = init_git_repo(&repo_a, &cargo_files);
    let out_a = checked(
        Command::new(&python3)
            .arg(&release_script)
            .args(["Cargo.toml", "rc", "patch", "false"])
            .env("KILN_WORK_DIR", &repo_a)
            .env("PATH", &bin_only_cargo)
            .output(),
    );
    assert!(!out_a.status.success());
    let rcpt_a: Value = checked(serde_json::from_slice(&checked(std::fs::read(
        repo_a.join("release-cut-receipt.json"),
    ))));
    assert_ne!(ptr(&rcpt_a, "/overall_status").as_str(), Some("pass"));
    assert!(ptr(&rcpt_a, "/working_tree_dirty").is_null());
    assert!(present(ptr(&rcpt_a, "/checks").as_array()).iter().any(|c| {
        c.get("status").and_then(Value::as_str) == Some("unavailable")
            && c.get("check")
                .and_then(Value::as_str)
                .is_some_and(|s| s.contains("git"))
    }));

    // 2. release-cut with git present and cargo missing on PATH
    let repo_b = tmp.path().join("repo-missing-cargo");
    let _ = init_git_repo(&repo_b, &cargo_files);
    let out_b = checked(
        Command::new(&python3)
            .arg(&release_script)
            .args(["Cargo.toml", "rc", "patch", "true"])
            .env("KILN_WORK_DIR", &repo_b)
            .env("PATH", &bin_only_git)
            .output(),
    );
    assert!(!out_b.status.success());
    let rcpt_b: Value = checked(serde_json::from_slice(&checked(std::fs::read(
        repo_b.join("release-cut-receipt.json"),
    ))));
    assert_ne!(ptr(&rcpt_b, "/overall_status").as_str(), Some("pass"));
    assert!(present(ptr(&rcpt_b, "/checks").as_array()).iter().any(|c| {
        c.get("status").and_then(Value::as_str) == Some("unavailable")
            && c.get("check").and_then(Value::as_str) == Some("cargo test --locked")
    }));

    // 3. release-cut with PATH="" (both git and cargo missing)
    let repo_both = tmp.path().join("repo-missing-both");
    let _ = init_git_repo(&repo_both, &cargo_files);
    let out_both = checked(
        Command::new(&python3)
            .arg(&release_script)
            .args(["Cargo.toml", "rc", "patch", "false"])
            .env("KILN_WORK_DIR", &repo_both)
            .env("PATH", "")
            .output(),
    );
    assert!(!out_both.status.success());
    let rcpt_both: Value = checked(serde_json::from_slice(&checked(std::fs::read(
        repo_both.join("release-cut-receipt.json"),
    ))));
    assert_ne!(ptr(&rcpt_both, "/overall_status").as_str(), Some("pass"));
    assert!(
        present(ptr(&rcpt_both, "/checks").as_array())
            .iter()
            .any(|c| c.get("status").and_then(Value::as_str) == Some("unavailable"))
    );

    // 4. license-ci-scaffold with Cargo.toml present and cargo missing
    let repo_c = tmp.path().join("repo-scaffold-missing-cargo");
    checked(std::fs::create_dir_all(&repo_c));
    checked(std::fs::write(
        repo_c.join("Cargo.toml"),
        b"[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    ));
    let out_c = checked(
        Command::new(&python3)
            .arg(&scaffold_script)
            .args(["mit-or-apache-2.0", "rust-and-nix", "false"])
            .env("KILN_WORK_DIR", &repo_c)
            .env("PATH", "")
            .output(),
    );
    assert!(!out_c.status.success());
    let rcpt_c: Value = checked(serde_json::from_slice(&checked(std::fs::read(
        repo_c.join("license-ci-scaffold-receipt.json"),
    ))));
    assert_ne!(ptr(&rcpt_c, "/overall_status").as_str(), Some("pass"));
    assert!(present(ptr(&rcpt_c, "/checks").as_array()).iter().any(|c| {
        c.get("status").and_then(Value::as_str) == Some("unavailable")
            && c.get("check").and_then(Value::as_str) == Some("cargo check --locked")
    }));
}

#[test]
fn manifest_parser_rejects_excessive_nesting_depth_without_stack_overflow() {
    let tmp = checked(tempfile::tempdir());
    let expected = "a".repeat(64);

    let deep_array_dir = tmp.path().join("deep-array");
    checked(std::fs::create_dir_all(&deep_array_dir));
    checked(std::fs::write(
        deep_array_dir.join("manifest.ncl"),
        format!("{}{}", "[".repeat(65), "]".repeat(65)),
    ));
    let res_arr = verify_bundle_at(tmp.path(), "deep-array", &expected);
    assert!(
        matches!(res_arr, Err(BundleVerifyError::Drift(ref msg)) if msg.contains("bundle manifest invalid")),
        "unexpected result: {res_arr:?}"
    );

    let deep_record_dir = tmp.path().join("deep-record");
    checked(std::fs::create_dir_all(&deep_record_dir));
    checked(std::fs::write(
        deep_record_dir.join("manifest.ncl"),
        format!("{}1{}", "{ a = ".repeat(65), " }".repeat(65)),
    ));
    let res_rec = verify_bundle_at(tmp.path(), "deep-record", &expected);
    assert!(
        matches!(res_rec, Err(BundleVerifyError::Drift(ref msg)) if msg.contains("bundle manifest invalid")),
        "unexpected result: {res_rec:?}"
    );

    let huge_array_dir = tmp.path().join("huge-array");
    checked(std::fs::create_dir_all(&huge_array_dir));
    checked(std::fs::write(
        huge_array_dir.join("manifest.ncl"),
        format!("{}{}", "[".repeat(10_000), "]".repeat(10_000)),
    ));
    let res_huge = verify_bundle_at(tmp.path(), "huge-array", &expected);
    assert!(
        matches!(res_huge, Err(BundleVerifyError::Drift(_))),
        "unexpected result: {res_huge:?}"
    );
}

#[test]
fn preview_single_target_enforces_scope_path_containment() {
    let mut reg = load_seed_registry_or_export();
    let dir = checked(tempfile::tempdir());
    let (mut profile, backend, _revisions) = setup_service_for_seed_registry(dir.path(), &reg);

    // Add a scoped project `kiln-scoped` with scope.root = "crates/kiln-core"
    let mut scoped_project = present(profile.projects.first()).clone();
    let scoped_repo = dir.path().join("repos/kiln-scoped");
    let _ = init_git_repo(
        &scoped_repo,
        &[(
            "crates/kiln-core/Cargo.toml",
            b"[package]\nname = \"scoped\"\nversion = \"0.1.0\"\n",
        )],
    );
    scoped_project.id = "kiln-scoped".into();
    scoped_project.label = "kiln-scoped".into();
    scoped_project.repository_path = scoped_repo;
    scoped_project.workflow.socket_path = dir.path().join("kiln-scoped.sock");
    scoped_project.scope = Some(checked(serde_json::from_value(json!({
        "root": "crates/kiln-core"
    }))));
    profile.projects.push(scoped_project);

    let mut scoped_rb = present(reg.find("flake-update")).clone();
    scoped_rb.id = "scoped-rb".into();
    scoped_rb.allowed_targets = vec![RunbookTarget {
        repository: "onixresearch/kiln".into(),
        project_id: "kiln-scoped".into(),
        references: vec!["refs/heads/main".into()],
        allow_commit_sha: true,
    }];
    if let Some(fanout) = scoped_rb.fanout.as_mut() {
        fanout.write_scopes = vec!["crates/kiln-core".into()];
    }
    reg.runbooks.push(scoped_rb);

    let mut engine = open_engine_with_ports(profile, Arc::clone(&backend));
    let op = principal("local:operator", Role::Operator);
    for id in ["kiln-flake-update", "kiln-security-audit", "kiln-scoped"] {
        assert_eq!(
            post(
                &mut engine,
                "/api/projects",
                json!({ "project_id": id, "request_id": format!("enroll-{id}") }),
                &op
            )
            .status,
            200
        );
    }
    checked(engine.configure_runbooks(reg));

    // 1. Scoped project with escaping scope_path -> 400 fanout_scope_widening
    for bad_scope in ["docs", "."] {
        let resp = post(
            &mut engine,
            "/api/runbooks/scoped-rb/preview",
            json!({
                "target": "onixresearch/kiln@refs/heads/main",
                "scope_path": bad_scope
            }),
            &op,
        );
        assert_eq!(resp.status, 400);
        assert_eq!(
            ptr(&json_body(&resp), "/code"),
            &json!("fanout_scope_widening")
        );
    }

    // 2. Unscoped project with flake-update (write_scopes=["flake.lock"]) and escaping scope_path -> 400 fanout_scope_widening
    let resp_unscoped = post(
        &mut engine,
        "/api/runbooks/flake-update/preview",
        json!({
            "target": "onixresearch/kiln@refs/heads/main",
            "scope_path": "Cargo.toml"
        }),
        &op,
    );
    assert_eq!(resp_unscoped.status, 400);
    assert_eq!(
        ptr(&json_body(&resp_unscoped), "/code"),
        &json!("fanout_scope_widening")
    );

    // 3. ReadOnly security-audit with scope_path -> 400 fanout_scope_invalid
    let resp_ro = post(
        &mut engine,
        "/api/runbooks/security-audit/preview",
        json!({
            "target": "onixresearch/kiln@refs/heads/main",
            "scope_path": "docs"
        }),
        &op,
    );
    assert_eq!(resp_ro.status, 400);
    assert_eq!(
        ptr(&json_body(&resp_ro), "/code"),
        &json!("fanout_scope_invalid")
    );

    // 4. Scoped project with contained scope_path -> 200 and derived write scope
    let resp_ok = post(
        &mut engine,
        "/api/runbooks/scoped-rb/preview",
        json!({
            "target": "onixresearch/kiln@refs/heads/main",
            "scope_path": "crates/kiln-core/src"
        }),
        &op,
    );
    assert_eq!(resp_ok.status, 200);
    assert_eq!(
        ptr(&json_body(&resp_ok), "/targets/0/write_scopes"),
        &json!(["onixresearch/kiln crates/kiln-core/src"])
    );
}

#[test]
fn rerun_refuses_drifted_or_missing_on_disk_bundle_when_runbooks_dir_configured() {
    let reg = load_seed_registry_or_export();
    let dir = checked(tempfile::tempdir());
    let (profile, backend, _revisions) = setup_service_for_seed_registry(dir.path(), &reg);
    let mut engine = open_engine_with_ports(profile, Arc::clone(&backend));

    let op = principal("local:operator", Role::Operator);
    assert_eq!(
        post(
            &mut engine,
            "/api/projects",
            json!({
                "project_id": "kiln-security-audit",
                "request_id": "enroll-rr-audit"
            }),
            &op
        )
        .status,
        200
    );

    let rb_copy = dir.path().join("runbooks");
    copy_dir_all(&runbooks_dir(), &rb_copy);
    checked(engine.configure_runbooks(reg));
    checked(engine.configure_runbooks_dir(&rb_copy));

    let disp = post(
        &mut engine,
        "/api/runbooks/security-audit/dispatch",
        json!({
            "request_id": "rr-seed-1",
            "target": "onixresearch/kiln@refs/heads/main",
            "params": { "audit_scope": "all", "strict": false }
        }),
        &op,
    );
    assert_eq!(disp.status, 201);
    let run_id = present(ptr(&json_body(&disp), "/run_id").as_str()).to_owned();
    assert_eq!(backend.lock().dispatches, 1);

    {
        let mut guard = backend.lock();
        guard.terminal = true;
        guard.sequence += 1;
    }
    checked(engine.poll());

    // 1. Tamper on-disk bundle asset -> 409 runbook_bundle_drift
    let bundle_dir = rb_copy.join("packs/kiln-seed-runbooks/workflow-templates/security-audit");
    let script = bundle_dir.join("assets/run-security-audit.py");
    let mut bytes = checked(std::fs::read(&script));
    bytes.extend_from_slice(b"\n# tampered\n");
    checked(std::fs::write(&script, &bytes));

    let drift_resp = post(
        &mut engine,
        &format!("/api/runs/{run_id}/rerun"),
        json!({ "request_id": "rr-drift", "mode": "exact" }),
        &op,
    );
    assert_eq!(drift_resp.status, 409);
    assert_eq!(
        ptr(&json_body(&drift_resp), "/code"),
        &json!("runbook_bundle_drift")
    );
    assert_eq!(backend.lock().dispatches, 1);

    // 2. Remove bundle directory -> 404 runbook_bundle_missing
    checked(std::fs::remove_dir_all(&bundle_dir));
    let missing_resp = post(
        &mut engine,
        &format!("/api/runs/{run_id}/rerun"),
        json!({ "request_id": "rr-missing", "mode": "exact" }),
        &op,
    );
    assert_eq!(missing_resp.status, 404);
    assert_eq!(
        ptr(&json_body(&missing_resp), "/code"),
        &json!("runbook_bundle_missing")
    );
    assert_eq!(backend.lock().dispatches, 1);

    // 3. Restore bundle directory -> 201 rerun succeeds
    copy_dir_all(
        &runbooks_dir().join("packs/kiln-seed-runbooks/workflow-templates/security-audit"),
        &bundle_dir,
    );
    let ok_resp = post(
        &mut engine,
        &format!("/api/runs/{run_id}/rerun"),
        json!({ "request_id": "rr-ok", "mode": "exact" }),
        &op,
    );
    assert_eq!(ok_resp.status, 201);
    assert_eq!(backend.lock().dispatches, 2);
}

#[test]
fn live_serve_preview_enforces_rbac_grants_and_zero_mutations() {
    const EXPECTED_VIEWER_CODE: &str = "operator_required";
    let reg = load_seed_registry_or_export();
    let dir = checked(tempfile::tempdir());
    let (mut profile, _backend, _revisions) = setup_service_for_seed_registry(dir.path(), &reg);

    let listener = checked(TcpListener::bind("127.0.0.1:0"));
    let addr = checked(listener.local_addr());
    drop(listener);
    profile.listen_address = addr;
    let origin = format!("https://localhost:{}", addr.port());
    profile.origin = origin.clone();

    let op_token = "a".repeat(64);
    let worker_token = "b".repeat(64);
    let viewer_token = "c".repeat(64);
    let proxy_key = "d".repeat(64);

    let viewer_token_path = dir.path().join("viewer.token");
    checked(std::fs::write(&viewer_token_path, &viewer_token));
    checked(std::fs::set_permissions(
        &viewer_token_path,
        std::fs::Permissions::from_mode(0o600),
    ));
    let proxy_key_path = dir.path().join("proxy.key");
    checked(std::fs::write(&proxy_key_path, &proxy_key));
    checked(std::fs::set_permissions(
        &proxy_key_path,
        std::fs::Permissions::from_mode(0o600),
    ));

    let profile_path = dir.path().join("service.json");
    checked(std::fs::write(
        &profile_path,
        checked(serde_json::to_vec(&profile)),
    ));
    let runbooks_path = dir.path().join("runbooks.json");
    checked(std::fs::write(
        &runbooks_path,
        checked(serde_json::to_vec(&reg)),
    ));
    let access_path = dir.path().join("access.json");
    checked(std::fs::write(
        &access_path,
        checked(serde_json::to_vec(&json!({
            "schema": "kiln.web-access.v1",
            "viewers": [{
                "id": "reader",
                "label": "Seed reader",
                "token_file": viewer_token_path
            }],
            "proxy": {
                "listen_address": "127.0.0.1:8099",
                "credential_file": proxy_key_path,
                "tailscaled_socket_path": dir.path().join("tailscaled.sock"),
                "principals": [{
                    "user_id": "99999",
                    "login_name": "other@example.invalid",
                    "label": "Ungranted operator",
                    "role": "operator"
                }]
            }
        }))),
    ));

    struct ServiceGuard(std::process::Child);
    impl Drop for ServiceGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    let notify_path = dir.path().join("notify.sock");
    let receiver = checked(UnixDatagram::bind(&notify_path));
    checked(receiver.set_read_timeout(Some(Duration::from_secs(8))));
    let mut child = ServiceGuard(checked(
        Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
            .args([
                "serve",
                "--profile",
                &profile_path.to_string_lossy(),
                "--access",
                &access_path.to_string_lossy(),
                "--runbooks",
                &runbooks_path.to_string_lossy(),
                "--runbooks-dir",
                &runbooks_dir().to_string_lossy(),
            ])
            .env("NOTIFY_SOCKET", &notify_path)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn(),
    ));
    let mut notification = [0; 32];
    match receiver.recv(&mut notification) {
        Ok(n) => assert_eq!(notification.get(..n), Some(b"READY=1".as_slice())),
        Err(err) => {
            let exit = checked(child.0.try_wait());
            let mut stderr = String::new();
            if let Some(mut pipe) = child.0.stderr.take() {
                let _ = pipe.read_to_string(&mut stderr);
            }
            unreachable!("serve readiness failed: {err}; exit={exit:?}; stderr={stderr}");
        }
    }

    let ca_cert = checked(reqwest::Certificate::from_pem(CA_PEM));
    let client = checked(
        reqwest::blocking::Client::builder()
            .add_root_certificate(ca_cert)
            .https_only(true)
            .no_proxy()
            .resolve("localhost", addr)
            .build(),
    );

    // Enroll read-only and mutating seed projects as granted operator.
    for project_id in ["kiln-security-audit", "kiln-flake-update"] {
        let enroll = checked(
            client
                .post(format!("{origin}/api/projects"))
                .header("Authorization", format!("Bearer {op_token}"))
                .header("Origin", &origin)
                .json(&json!({
                    "project_id": project_id,
                    "request_id": format!("enroll-live-{project_id}")
                }))
                .send(),
        );
        assert_eq!(enroll.status().as_u16(), 200);
    }

    let payload = json!({ "target": "onixresearch/kiln@refs/heads/main" });

    // 1. Role::Viewer -> 403 operator_required across preview, dispatch, and fanout.
    for id in ["security-audit", "flake-update"] {
        let resp = checked(
            client
                .post(format!("{origin}/api/runbooks/{id}/preview"))
                .header("Authorization", format!("Bearer {viewer_token}"))
                .header("Origin", &origin)
                .json(&payload)
                .send(),
        );
        assert_eq!(resp.status().as_u16(), 403, "viewer preview {id}");
        let body: Value = checked(resp.json());
        assert_eq!(ptr(&body, "/code"), &json!(EXPECTED_VIEWER_CODE));
    }
    for (route, body) in [
        (
            "/api/runbooks/flake-update/dispatch",
            json!({
                "request_id": "viewer-live-dispatch",
                "target": "onixresearch/kiln@refs/heads/main"
            }),
        ),
        (
            "/api/runbooks/flake-update/fanout",
            json!({
                "request_id": "viewer-live-fanout",
                "targets": ["onixresearch/kiln@refs/heads/main"]
            }),
        ),
    ] {
        let resp = checked(
            client
                .post(format!("{origin}{route}"))
                .header("Authorization", format!("Bearer {viewer_token}"))
                .header("Origin", &origin)
                .json(&body)
                .send(),
        );
        assert_eq!(resp.status().as_u16(), 403, "viewer {route}");
        let resp_body: Value = checked(resp.json());
        assert_eq!(ptr(&resp_body, "/code"), &json!(EXPECTED_VIEWER_CODE));
    }

    // 2. Role::Worker -> 403 worker_scope across read-only and mutating runbooks.
    for id in ["security-audit", "flake-update"] {
        let resp = checked(
            client
                .post(format!("{origin}/api/runbooks/{id}/preview"))
                .header("Authorization", format!("Bearer {worker_token}"))
                .header("Origin", &origin)
                .json(&payload)
                .send(),
        );
        assert_eq!(resp.status().as_u16(), 403, "worker preview {id}");
        let body: Value = checked(resp.json());
        assert_eq!(ptr(&body, "/code"), &json!("worker_scope"));
    }

    // 3. Ungranted Operator (tailscale:99999) -> 200 on both:
    //    mutating flake-update has can_dispatch=false, dispatch_refusal=runbook_grant_required;
    //    read-only security-audit has can_dispatch=true, dispatch_refusal=null.
    let ungranted_mut = checked(
        client
            .post(format!("{origin}/api/runbooks/flake-update/preview"))
            .header("X-Kiln-Proxy-Key", &proxy_key)
            .header("X-Kiln-Principal", "tailscale:99999")
            .header("Origin", &origin)
            .json(&payload)
            .send(),
    );
    assert_eq!(ungranted_mut.status().as_u16(), 200);
    let ungranted_mut_body: Value = checked(ungranted_mut.json());
    assert_eq!(ptr(&ungranted_mut_body, "/dry_run"), &json!(true));
    assert_eq!(ptr(&ungranted_mut_body, "/grants_authority"), &json!(false));
    assert_eq!(
        ptr(&ungranted_mut_body, "/bundle_pin_verified"),
        &json!(true)
    );
    assert_eq!(ptr(&ungranted_mut_body, "/can_dispatch"), &json!(false));
    assert_eq!(
        ptr(&ungranted_mut_body, "/dispatch_refusal"),
        &json!("runbook_grant_required")
    );

    let ungranted_ro = checked(
        client
            .post(format!("{origin}/api/runbooks/security-audit/preview"))
            .header("X-Kiln-Proxy-Key", &proxy_key)
            .header("X-Kiln-Principal", "tailscale:99999")
            .header("Origin", &origin)
            .json(&payload)
            .send(),
    );
    assert_eq!(ungranted_ro.status().as_u16(), 200);
    let ungranted_ro_body: Value = checked(ungranted_ro.json());
    assert_eq!(ptr(&ungranted_ro_body, "/dry_run"), &json!(true));
    assert_eq!(ptr(&ungranted_ro_body, "/grants_authority"), &json!(false));
    assert_eq!(
        ptr(&ungranted_ro_body, "/bundle_pin_verified"),
        &json!(true)
    );
    assert_eq!(ptr(&ungranted_ro_body, "/can_dispatch"), &json!(true));
    assert!(ptr(&ungranted_ro_body, "/dispatch_refusal").is_null());

    // 4. Granted Operator (local:operator) -> 200 with can_dispatch=true on both.
    for id in ["security-audit", "flake-update"] {
        let resp = checked(
            client
                .post(format!("{origin}/api/runbooks/{id}/preview"))
                .header("Authorization", format!("Bearer {op_token}"))
                .header("Origin", &origin)
                .json(&payload)
                .send(),
        );
        assert_eq!(resp.status().as_u16(), 200, "granted preview {id}");
        let body: Value = checked(resp.json());
        assert_eq!(ptr(&body, "/dry_run"), &json!(true));
        assert_eq!(ptr(&body, "/grants_authority"), &json!(false));
        assert_eq!(ptr(&body, "/bundle_pin_verified"), &json!(true));
        assert_eq!(ptr(&body, "/can_dispatch"), &json!(true));
        assert!(ptr(&body, "/dispatch_refusal").is_null());
    }

    // 5. Verify zero durable runs, dispatches, or fanouts were created.
    for (path, key) in [
        ("/api/runs", "runs"),
        ("/api/runbooks/dispatches", "dispatches"),
        ("/api/runbooks/fanouts", "fanouts"),
    ] {
        let resp = checked(
            client
                .get(format!("{origin}{path}"))
                .header("Authorization", format!("Bearer {op_token}"))
                .send(),
        );
        assert_eq!(resp.status().as_u16(), 200, "GET {path}");
        let body: Value = checked(resp.json());
        assert_eq!(ptr(&body, &format!("/{key}")), &json!([]));
    }
}
