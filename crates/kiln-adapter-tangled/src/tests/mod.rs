mod auth;
mod e2e;
mod events;
mod pipeline;
mod profile;
mod shell;
mod workflows;

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use k256::ecdsa::signature::Signer as _;
use kiln_core::domain::Outcome;
use kiln_core::monorepo::{ChangedFallback, ChangedFiles};
use kiln_core::provider::CapabilityProfile;
use kiln_core::types::{EffectId, ExternalRef};
use kiln_core::workflow::{
    WorkflowBounds, WorkflowControlRequest, WorkflowControls, WorkflowEffectPlan, WorkflowLogChunk,
    WorkflowLogReadRequest, WorkflowNodeObservation, WorkflowNodeState, WorkflowObservation,
    WorkflowState,
};
use kiln_provider_api::{PortFailure, PortResult, WorkflowPort};
use serde_json::{Value, json};

use crate::auth::{IssuerKey, VerifiedClaims};
use crate::identity::ObjectId;
use crate::pipeline::{AcquiredSource, WorkflowFile};
use crate::profile::*;
use crate::source::{AcquireRequest, SourcePort};
use crate::status::{StatusDelivery, StatusPort};

pub const REPO: &str = "did:plc:abcdefghijklmnopqrstuvwx";
pub const OTHER_REPO: &str = "did:plc:zzzzzzzzzzzzzzzzzzzzzzzz";
pub const KNOT: &str = "did:web:knot.example.com";
pub const SPINDLE: &str = "did:web:spindle.example.com";
pub const TRUSTED: &str = "did:plc:trustedactor234567abcdef";
pub const APPROVER: &str = "did:plc:approver234567abcdefghij";
pub const OUTSIDER: &str = "did:plc:outsider234567abcdefghij";

pub fn ok<T, E: Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => unreachable!("unexpected error: {error:?}"),
    }
}

pub fn err<T: Debug, E>(result: Result<T, E>) -> E {
    match result {
        Ok(value) => unreachable!("unexpected success: {value:?}"),
        Err(error) => error,
    }
}

/// JSON pointer lookup; absent paths read as `null`.
pub fn field<'a>(value: &'a Value, pointer: &str) -> &'a Value {
    value.pointer(pointer).unwrap_or(&Value::Null)
}

pub fn set(value: &mut Value, key: &str, replacement: Value) {
    if let Some(object) = value.as_object_mut() {
        object.insert(key.to_string(), replacement);
    }
}

pub fn some<T>(value: Option<T>) -> T {
    value.unwrap_or_else(|| unreachable!("unexpected None"))
}

pub fn knot_key() -> k256::ecdsa::SigningKey {
    ok(k256::ecdsa::SigningKey::from_slice(&[7_u8; 32]))
}

pub fn actor_key() -> p256::ecdsa::SigningKey {
    ok(p256::ecdsa::SigningKey::from_slice(&[9_u8; 32]))
}

pub fn rogue_key() -> k256::ecdsa::SigningKey {
    ok(k256::ecdsa::SigningKey::from_slice(&[11_u8; 32]))
}

pub enum Signer {
    K256(k256::ecdsa::SigningKey),
    P256(p256::ecdsa::SigningKey),
}

pub fn issuer_keys() -> BTreeMap<String, IssuerKey> {
    let actor = IssuerKey::P256(*actor_key().verifying_key());
    BTreeMap::from([
        (
            KNOT.to_string(),
            IssuerKey::Secp256k1(*knot_key().verifying_key()),
        ),
        (TRUSTED.to_string(), actor.clone()),
        (APPROVER.to_string(), actor),
    ])
}

/// A compact service-auth JWT signed like a knot or PDS would sign it.
pub fn sign_token(signer: &Signer, claims: &Value) -> String {
    let alg = match signer {
        Signer::K256(_) => "ES256K",
        Signer::P256(_) => "ES256",
    };
    let header =
        URL_SAFE_NO_PAD.encode(ok(serde_json::to_vec(&json!({ "alg": alg, "typ": "JWT" }))));
    let payload = URL_SAFE_NO_PAD.encode(ok(serde_json::to_vec(claims)));
    let input = format!("{header}.{payload}");
    let signature = match signer {
        Signer::K256(key) => {
            let signature: k256::ecdsa::Signature = key.sign(input.as_bytes());
            signature
                .normalize_s()
                .unwrap_or(signature)
                .to_bytes()
                .to_vec()
        }
        Signer::P256(key) => {
            let signature: p256::ecdsa::Signature = key.sign(input.as_bytes());
            signature
                .normalize_s()
                .unwrap_or(signature)
                .to_bytes()
                .to_vec()
        }
    };
    format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature))
}

pub fn claims(issuer: &str, method: &str, now: i64, jti: &str) -> Value {
    json!({ "iss": issuer, "aud": SPINDLE, "lxm": method, "iat": now, "exp": now + 60, "jti": jti })
}

pub fn verified(issuer: &str, jti: &str) -> VerifiedClaims {
    VerifiedClaims {
        issuer: issuer.to_string(),
        jti: jti.to_string(),
        expires_unix: i64::MAX,
    }
}

pub fn profile_value() -> TangledProfile {
    TangledProfile {
        schema: PROFILE_SCHEMA.to_string(),
        spindle_did: SPINDLE.to_string(),
        listen_address: ok("127.0.0.1:0".parse()),
        database_path: "/var/lib/kiln/tangled.sqlite".into(),
        source_directory: "/var/lib/kiln/tangled-sources".into(),
        git_program: "/run/current-system/sw/bin/git".into(),
        plc_directory_url: "https://plc.directory".to_string(),
        repositories: vec![RepositoryPin {
            repo_did: REPO.to_string(),
            knot_did: KNOT.to_string(),
            clone_url: format!("https://knot.example.com/{REPO}"),
            trusted_actors: vec![TRUSTED.to_string()],
            approvers: vec![APPROVER.to_string()],
        }],
        engines: vec![EngineBinding {
            engine: "microvm".to_string(),
            images: vec!["nixos".to_string()],
            packages: vec!["go".to_string(), "nodejs".to_string()],
            platform: "linux-x86_64".to_string(),
            adapter_kind: "microvm".to_string(),
        }],
        workflow: KilnWorkflowBinding {
            policy_id: "tangled-trusted-v1".to_string(),
            untrusted_policy_id: "tangled-untrusted-v1".to_string(),
            provider_profile_id: "kiln-fixture-v1".to_string(),
            controls: WorkflowControls {
                cancel: true,
                pause: false,
                resume: false,
            },
            bounds: WorkflowBounds {
                max_nodes: 4,
                max_edges: 4,
                max_log_references: 4,
                max_log_read_bytes: 1024,
                max_total_log_bytes: 4096,
            },
        },
        status: StatusBinding {
            endpoint: "https://status.example.com/kiln".to_string(),
            secret_path: "/run/kiln/tangled-status.secret".into(),
            timeout_ms: 2000,
            maximum_attempts: 3,
        },
        limits: Limits {
            maximum_body_bytes: 65_536,
            maximum_header_bytes: 16_384,
            read_timeout_ms: 2000,
            write_timeout_ms: 2000,
            maximum_token_lifetime_seconds: 300,
            clock_skew_seconds: 60,
            maximum_tokens: 64,
            maximum_pipelines: 64,
            maximum_record_bytes: 1_048_576,
            maximum_workflow_files: 16,
            maximum_workflow_bytes: 65_536,
            maximum_changed_paths: 1024,
            maximum_changed_path_bytes: 65_536,
            acquisition_timeout_ms: 30_000,
            resolver_timeout_ms: 2000,
            maximum_did_document_bytes: 65_536,
            key_cache_seconds: 300,
            maximum_active_runs: 4,
            maximum_workflow_polls: 8,
            worker_poll_interval_ms: 10,
        },
        non_claims: REQUIRED_NON_CLAIMS.map(str::to_string).to_vec(),
    }
}

pub fn profile() -> AdmittedProfile {
    ok(admit_profile(profile_value()))
}

pub fn object(character: char, length: usize) -> ObjectId {
    some(ObjectId::parse(&character.to_string().repeat(length)))
}

pub const TEST_WORKFLOW: &str = "when:\n  - event: push\n    branch: main\n  - event: [pull_request, manual]\n    branch: main\nengine: microvm\nimage: nixos\ndependencies: [go]\nsteps:\n  - name: test\n    command: go test ./...\n";
pub const DOCS_WORKFLOW: &str = "when:\n  - event: push\n    branch: main\n    paths: [\"docs/**\"]\nengine: microvm\nsteps:\n  - name: docs\n    command: make docs\n";
pub const CACHE_WORKFLOW: &str = "engine: microvm\ncache:\n  - key: go\n    paths: [.gocache]\nsteps:\n  - name: x\n    command: true\n";

pub fn workflow_files() -> Vec<WorkflowFile> {
    vec![
        WorkflowFile {
            path: ".tangled/workflows/test.yml".into(),
            bytes: TEST_WORKFLOW.as_bytes().to_vec(),
        },
        WorkflowFile {
            path: ".tangled/workflows/docs.yml".into(),
            bytes: DOCS_WORKFLOW.as_bytes().to_vec(),
        },
        WorkflowFile {
            path: ".tangled/workflows/cache.yml".into(),
            bytes: CACHE_WORKFLOW.as_bytes().to_vec(),
        },
    ]
}

pub fn acquired(commit: &ObjectId) -> AcquiredSource {
    AcquiredSource {
        commit: commit.clone(),
        files: workflow_files(),
        changed: ChangedFiles::Exact(["src/main.go".to_string()].into()),
    }
}

/// In-memory knot source.
#[derive(Debug, Default)]
pub struct FixtureSource {
    pub acquisitions: usize,
    pub failure: Option<PortFailure>,
    pub head: Option<ObjectId>,
}

impl SourcePort for FixtureSource {
    fn acquire(&mut self, request: &AcquireRequest<'_>) -> PortResult<AcquiredSource> {
        self.acquisitions += 1;
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        let mut source = acquired(request.commit);
        if request.base.is_none() {
            source.changed = ChangedFiles::Fallback(ChangedFallback::NoBase);
        }
        Ok(source)
    }

    fn resolve(&mut self, _: &str, _: &str, reference: &str) -> PortResult<Option<ObjectId>> {
        Ok(matches!(reference, "HEAD" | "main")
            .then(|| self.head.clone())
            .flatten())
    }
}

#[derive(Debug, Default)]
pub struct Remote {
    pub dispatched: Vec<WorkflowEffectPlan>,
    pub recoveries: usize,
}

/// Kiln workflow port that accepts a dispatch and completes on observation.
#[derive(Debug, Clone)]
pub struct FixtureWorkflow {
    pub profile: CapabilityProfile,
    pub remote: Arc<Mutex<Remote>>,
    pub lose_dispatch: bool,
    pub outcome: Outcome,
}

impl FixtureWorkflow {
    pub fn new() -> Self {
        Self {
            profile: CapabilityProfile {
                contract_version: kiln_core::types::CONTRACT_VERSION_V1,
                profile_id: ok(kiln_core::types::ProviderProfileId::new("kiln-fixture-v1")),
                role: kiln_core::provider::ProviderRole::Workflow,
                namespace: ok(ExternalRef::new("fixture", "workflow", "fixture")),
                operations: std::collections::BTreeSet::new(),
                outcomes: std::collections::BTreeSet::new(),
                limits: kiln_core::provider::ProviderLimits {
                    max_request_bytes: 65_536,
                    max_result_bytes: 65_536,
                    max_in_flight: 4,
                },
            },
            remote: Arc::new(Mutex::new(Remote::default())),
            lose_dispatch: false,
            outcome: Outcome::Success,
        }
    }

    pub fn dispatched(&self) -> Vec<WorkflowEffectPlan> {
        self.remote
            .lock()
            .map(|remote| remote.dispatched.clone())
            .unwrap_or_default()
    }
}

pub fn observation(
    plan: &WorkflowEffectPlan,
    completed: bool,
    outcome: Outcome,
    sequence: u64,
) -> WorkflowObservation {
    let reference = |kind: &str| ok(ExternalRef::new("fixture", kind, "execution-1"));
    let successful = completed && outcome == Outcome::Success;
    WorkflowObservation {
        effect_id: plan.effect_id.clone(),
        attempt_id: plan.attempt_id.clone(),
        candidate_id: plan.candidate_id.clone(),
        pipeline_revision_id: plan.pipeline_revision_id.clone(),
        workflow_policy_id: plan.workflow_policy_id.clone(),
        provider_profile: plan.provider_profile.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        workflow: reference("workflow"),
        execution: reference("execution"),
        sequence,
        state: match (completed, successful) {
            (false, _) => WorkflowState::Accepted,
            (true, true) => WorkflowState::Completed,
            (true, false) => WorkflowState::Failed,
        },
        terminal: completed,
        successful,
        outcome: if completed { outcome } else { Outcome::Pending },
        nodes: plan
            .nodes
            .iter()
            .map(|node| WorkflowNodeObservation {
                node_id: node.id.clone(),
                state: match (completed, successful) {
                    (false, _) => WorkflowNodeState::Pending,
                    (true, true) => WorkflowNodeState::Completed,
                    (true, false) => WorkflowNodeState::Failed,
                },
                terminal: completed,
                successful,
                adapter: reference("adapter"),
                receipt: completed.then(|| reference("receipt")),
            })
            .collect(),
        logs: Vec::new(),
        receipt: completed.then(|| reference("receipt")),
    }
}

impl WorkflowPort for FixtureWorkflow {
    fn profile(&self) -> &CapabilityProfile {
        &self.profile
    }
    fn dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        if let Ok(mut remote) = self.remote.lock() {
            remote.dispatched.push(plan.clone());
        }
        if self.lose_dispatch {
            return Err(crate::error::unknown(
                "fixture_lost",
                "Dispatch response was lost",
            ));
        }
        Ok(observation(plan, false, Outcome::Pending, 0))
    }
    fn recover_dispatch(&mut self, plan: &WorkflowEffectPlan) -> PortResult<WorkflowObservation> {
        if let Ok(mut remote) = self.remote.lock() {
            remote.recoveries += 1;
        }
        Ok(observation(plan, false, Outcome::Pending, 0))
    }
    fn observe(&mut self, effect_id: &EffectId) -> PortResult<WorkflowObservation> {
        let plan = self.remote.lock().ok().and_then(|remote| {
            remote
                .dispatched
                .iter()
                .rev()
                .find(|plan| plan.effect_id == *effect_id)
                .cloned()
        });
        let plan = plan.ok_or_else(|| {
            crate::error::infrastructure("fixture_unknown_effect", "No such effect")
        })?;
        Ok(observation(&plan, true, self.outcome, 1))
    }
    fn control(&mut self, _: &WorkflowControlRequest) -> PortResult<WorkflowObservation> {
        Err(crate::error::domain(
            "fixture_control",
            "test",
            "Not selected by this fixture",
        ))
    }
    fn read_log(&mut self, _: &WorkflowLogReadRequest) -> PortResult<WorkflowLogChunk> {
        Err(crate::error::domain(
            "fixture_log",
            "test",
            "Not selected by this fixture",
        ))
    }
    fn reconcile(&mut self, effect_id: &EffectId) -> PortResult<WorkflowObservation> {
        self.observe(effect_id)
    }
}

/// Records deliveries; optionally loses responses.
#[derive(Debug, Default, Clone)]
pub struct FixtureStatus {
    pub deliveries: Arc<Mutex<Vec<StatusDelivery>>>,
    pub lose_response: bool,
}

impl StatusPort for FixtureStatus {
    fn publish(&mut self, delivery: &StatusDelivery) -> PortResult<()> {
        if let Ok(mut deliveries) = self.deliveries.lock() {
            deliveries.push(delivery.clone());
        }
        if self.lose_response {
            return Err(crate::error::unknown(
                "fixture_lost_response",
                "Status response was lost",
            ));
        }
        Ok(())
    }
}

impl FixtureStatus {
    pub fn views(&self) -> Vec<Value> {
        self.deliveries
            .lock()
            .map(|deliveries| {
                deliveries
                    .iter()
                    .map(|delivery| ok(serde_json::from_slice(&delivery.body)))
                    .collect()
            })
            .unwrap_or_default()
    }
}
