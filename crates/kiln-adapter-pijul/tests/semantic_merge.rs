//! Frozen semantic merge orchestration fixture, end to end:
//! exact Pijul acquisition -> checker request -> result evidence -> CI decision.
//!
//! The human-authored fixture is `fixtures/semantic-merge/quartet.ncl`; `quartet.json` is its
//! deterministic `nickel export`. Labels in the fixture are hashed with BLAKE3 into identities.
//! Only the fixture checker producer below implements `SemanticCheckerPort`.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;

use anyhow::{Context, anyhow};
use kiln_adapter_pijul::{PijulSourceProfile, admit_profile, resolve_and_acquire};
use kiln_codec::identity_from_bytes;
use kiln_core::semantic_merge::{
    ArtifactKind, BoundArtifact, CandidateSupply, CheckBounds, CheckMode, CheckerDispatchIdentity,
    CheckerIdentity, CheckerObservation, CheckerRequest, CheckerVerdict, ContextExpectation,
    EvidenceKind, ExhaustionBound, IndependentGate, IndependentGateKind, LineagePolicy, MergeRole,
    RetryPolicy, RoleSource, SEMANTIC_MERGE_CONTRACT_VERSION, SemanticCheckCommand,
    SemanticCheckDecision, SemanticCheckEffect, SemanticCheckState, SemanticInputs,
    SemanticMergePolicy, SemanticMergeRequest, SemanticOutcome, SemanticProperty, SemanticSubject,
    admit_request, artifact_binding, attempt_binding, check_revalidation, checker_workflow_plan,
    combine_gates, decide, decide_gate, evolve, reobserve_binding,
};
use kiln_core::source::{PijulEntryKind, PijulSnapshotEntry};
use kiln_core::types::{
    Alias, AttemptId, CandidateId, ContentId, Diagnostic, EffectId, EvaluationId, ExternalRef,
    PipelineRevisionId, PolicyId, ProviderProfileId,
};
use kiln_core::workflow::WorkflowBounds;
use kiln_provider_api::semantic_merge::{
    SemanticCheckerPort, dispatch_semantic_check, producer_failure,
};
use kiln_provider_api::{
    PijulReadObservation, PijulReadRequest, PijulRepositoryPort, PortFailure, PortResult,
};
use serde::Deserialize;

const FIXTURE: &str = include_str!("../../../fixtures/semantic-merge/quartet.json");
const FIXTURE_SCHEMA: &str = "kiln.semantic-merge-fixture.v1";
const FORMAT_COHORT: &str = "sanakirja-pristine-v1";
const WORKFLOW_GRAPH_BOUND: usize = 4;
const WORKFLOW_LOG_BYTES: usize = 65_536;

#[derive(Clone, Debug, Deserialize)]
struct FixtureFile {
    path: String,
    text: String,
}

#[derive(Clone, Debug, Deserialize)]
struct FixtureRole {
    channel: String,
    state: String,
    changes: Vec<String>,
    files: Vec<FixtureFile>,
}

#[derive(Clone, Debug, Deserialize)]
struct FixtureRoles {
    base: FixtureRole,
    variant_a: FixtureRole,
    variant_b: FixtureRole,
    candidate: FixtureRole,
}

#[derive(Clone, Debug, Deserialize)]
struct FixtureBounds {
    timeout_ms: u64,
    max_memory_bytes: u64,
    stdout_max_bytes: u64,
    stderr_max_bytes: u64,
    max_products: usize,
}

#[derive(Clone, Debug, Deserialize)]
struct FixtureProduct {
    name: String,
    content: String,
}

#[derive(Clone, Debug, Deserialize)]
struct FixtureProducer {
    verdict: String,
    coverage: Vec<String>,
    output_complete: bool,
    receipt: String,
    log: String,
    products: Vec<FixtureProduct>,
}

#[derive(Clone, Debug, Deserialize)]
struct FixtureGate {
    name: String,
    kind: String,
    passed: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct NegativeCase {
    name: String,
    mutation: String,
    role: Option<String>,
    state: Option<String>,
    changes: Option<Vec<String>>,
    label: Option<String>,
    verdict: Option<String>,
    evidence_kind: Option<String>,
    expected_code: Option<String>,
    expected_field: Option<String>,
    expected_outcome: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct Fixture {
    schema: String,
    repository: String,
    source_profile: String,
    roles: FixtureRoles,
    supply: serde_json::Value,
    subject: serde_json::Value,
    property: serde_json::Value,
    inputs: serde_json::Value,
    checker: serde_json::Value,
    policy: serde_json::Value,
    bounds: FixtureBounds,
    producer: FixtureProducer,
    independent_gates: Vec<FixtureGate>,
    negative_cases: Vec<NegativeCase>,
    non_claims: Vec<String>,
}

fn core<T>(result: Result<T, Diagnostic>) -> anyhow::Result<T> {
    result.map_err(|error| anyhow!("{} at {}: {}", error.code, error.field, error.message))
}

fn port<T>(result: PortResult<T>) -> anyhow::Result<T> {
    result.map_err(|failure| anyhow!("port failure: {failure:?}"))
}

fn label(value: &str) -> anyhow::Result<ContentId> {
    identity_from_bytes(value.as_bytes()).map_err(|error| anyhow!("{error:?}"))
}

fn text<'a>(value: &'a serde_json::Value, pointer: &str) -> anyhow::Result<&'a str> {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_str)
        .with_context(|| format!("fixture field {pointer} is not text"))
}

fn flag(value: &serde_json::Value, pointer: &str) -> anyhow::Result<bool> {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_bool)
        .with_context(|| format!("fixture field {pointer} is not a boolean"))
}

fn number(value: &serde_json::Value, pointer: &str) -> anyhow::Result<u64> {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_u64)
        .with_context(|| format!("fixture field {pointer} is not a number"))
}

fn texts(value: &serde_json::Value, pointer: &str) -> anyhow::Result<Vec<String>> {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_array)
        .with_context(|| format!("fixture field {pointer} is not an array"))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .context("array item is not text")
        })
        .collect()
}

fn pijul(kind: &str, value: &str) -> anyhow::Result<ExternalRef> {
    core(ExternalRef::new("pijul", kind, value))
}

fn load() -> anyhow::Result<Fixture> {
    let fixture: Fixture = serde_json::from_str(FIXTURE)?;
    anyhow::ensure!(
        fixture.schema == FIXTURE_SCHEMA,
        "unexpected fixture schema"
    );
    anyhow::ensure!(
        !fixture.non_claims.is_empty(),
        "fixture must carry non-claims"
    );
    Ok(fixture)
}

/// Fixture-backed native read port: each channel has one exact recorded observation.
#[derive(Debug)]
struct FixtureRepository {
    observations: RefCell<BTreeMap<String, PijulReadObservation>>,
}

impl FixtureRepository {
    fn new(fixture: &Fixture) -> anyhow::Result<Self> {
        let repository = Self {
            observations: RefCell::new(BTreeMap::new()),
        };
        for role in [
            &fixture.roles.base,
            &fixture.roles.variant_a,
            &fixture.roles.variant_b,
            &fixture.roles.candidate,
        ] {
            repository.record(fixture, role)?;
        }
        Ok(repository)
    }

    /// Records a new channel state, as a concurrent writer outside Kiln would.
    fn record(&self, fixture: &Fixture, role: &FixtureRole) -> anyhow::Result<()> {
        let state = pijul("state", &role.state)?;
        let observation = PijulReadObservation {
            repository: pijul("repository", &fixture.repository)?,
            channel: pijul("channel", &role.channel)?,
            observed_state_before: state.clone(),
            observed_state_after: state,
            change_sequence: role
                .changes
                .iter()
                .map(|change| pijul("change", change))
                .collect::<anyhow::Result<_>>()?,
            format_cohort: pijul("format-cohort", FORMAT_COHORT)?,
            entries: role
                .files
                .iter()
                .map(|file| PijulSnapshotEntry {
                    path: file.path.clone(),
                    kind: PijulEntryKind::Regular,
                    bytes: file.text.as_bytes().to_vec(),
                })
                .collect(),
        };
        self.observations
            .borrow_mut()
            .insert(role.channel.clone(), observation);
        Ok(())
    }
}

impl PijulRepositoryPort for FixtureRepository {
    fn discover_channels(&self, _max_channels: usize) -> PortResult<Vec<ExternalRef>> {
        self.observations
            .borrow()
            .values()
            .map(|observation| Ok(observation.channel.clone()))
            .collect()
    }

    fn resolve_and_acquire(&self, request: &PijulReadRequest) -> PortResult<PijulReadObservation> {
        self.observations
            .borrow()
            .get(request.channel.native_id.as_str())
            .cloned()
            .ok_or_else(|| {
                PortFailure::Domain(Diagnostic::new(
                    "pijul_channel_missing",
                    "pijul.channel",
                    "fixture channel is missing",
                ))
            })
    }
}

fn fixture_role(fixture: &Fixture, role: MergeRole) -> &FixtureRole {
    match role {
        MergeRole::Base => &fixture.roles.base,
        MergeRole::VariantA => &fixture.roles.variant_a,
        MergeRole::VariantB => &fixture.roles.variant_b,
        MergeRole::Candidate => &fixture.roles.candidate,
    }
}

/// Acquires one role through the real read-only adapter mapping and admitted profile.
fn acquire(
    repository: &FixtureRepository,
    fixture: &Fixture,
    role: MergeRole,
) -> anyhow::Result<RoleSource> {
    let profile = PijulSourceProfile::pinned_local();
    anyhow::ensure!(profile.id.as_str() == fixture.source_profile);
    let channel = pijul("channel", &fixture_role(fixture, role).channel)?;
    let resolution = port(resolve_and_acquire(
        repository,
        channel,
        profile.id.clone(),
        &profile,
    ))?;
    Ok(RoleSource {
        role,
        resolution,
        acquisition_profile: port(admit_profile(&profile))?,
    })
}

fn bounds(value: &FixtureBounds) -> CheckBounds {
    CheckBounds {
        timeout_ms: value.timeout_ms,
        max_memory_bytes: value.max_memory_bytes,
        stdout_max_bytes: value.stdout_max_bytes,
        stderr_max_bytes: value.stderr_max_bytes,
        max_products: value.max_products,
    }
}

fn policy_bounds(policy: &serde_json::Value) -> anyhow::Result<CheckBounds> {
    Ok(CheckBounds {
        timeout_ms: number(policy, "/bound_ceilings/timeout_ms")?,
        max_memory_bytes: number(policy, "/bound_ceilings/max_memory_bytes")?,
        stdout_max_bytes: number(policy, "/bound_ceilings/stdout_max_bytes")?,
        stderr_max_bytes: number(policy, "/bound_ceilings/stderr_max_bytes")?,
        max_products: usize::try_from(number(policy, "/bound_ceilings/max_products")?)?,
    })
}

fn checker_identity(fixture: &Fixture) -> anyhow::Result<CheckerIdentity> {
    let checker = &fixture.checker;
    Ok(CheckerIdentity {
        producer_profile: core(ProviderProfileId::new(text(checker, "/producer_profile")?))?,
        checker: label(text(checker, "/checker")?)?,
        toolchain: label(text(checker, "/toolchain")?)?,
        semantic_profile: core(Alias::new(text(checker, "/semantic_profile")?))?,
        receipt_profile: core(Alias::new(text(checker, "/receipt_profile")?))?,
    })
}

/// Builds the request from the fixture and four exact acquisitions.
fn build_request(
    fixture: &Fixture,
    repository: &FixtureRepository,
) -> anyhow::Result<SemanticMergeRequest> {
    let policy = &fixture.policy;
    let mode = match text(policy, "/mode")? {
        "required" => CheckMode::Required,
        "advisory" => CheckMode::Advisory,
        other => anyhow::bail!("unknown mode {other}"),
    };
    let checker = checker_identity(fixture)?;
    let candidate = acquire(repository, fixture, MergeRole::Candidate)?;
    Ok(SemanticMergeRequest {
        contract_version: SEMANTIC_MERGE_CONTRACT_VERSION,
        evaluation: core(EvaluationId::new("semantic-merge-fixture-evaluation"))?,
        base: acquire(repository, fixture, MergeRole::Base)?,
        variant_a: acquire(repository, fixture, MergeRole::VariantA)?,
        variant_b: acquire(repository, fixture, MergeRole::VariantB)?,
        candidate_supply: CandidateSupply {
            supplier: core(ProviderProfileId::new(text(&fixture.supply, "/supplier")?))?,
            proposal: core(ExternalRef::new(
                "fixture",
                "proposal",
                text(&fixture.supply, "/proposal")?,
            ))?,
            supplied_state: candidate.resolution.exact_revision.clone(),
        },
        candidate,
        subject: SemanticSubject {
            language: core(Alias::new(text(&fixture.subject, "/language")?))?,
            unit: core(ExternalRef::new(
                "fixture",
                "crate",
                text(&fixture.subject, "/unit")?,
            ))?,
            required_coverage: texts(&fixture.subject, "/required_coverage")?
                .into_iter()
                .map(|unit| core(Alias::new(unit)))
                .collect::<anyhow::Result<_>>()?,
        },
        property: SemanticProperty {
            id: core(Alias::new(text(&fixture.property, "/id")?))?,
            revision: label(text(&fixture.property, "/revision")?)?,
            assumptions: {
                let mut assumptions = texts(&fixture.property, "/assumptions")?
                    .iter()
                    .map(|assumption| label(assumption))
                    .collect::<anyhow::Result<Vec<_>>>()?;
                assumptions.sort();
                assumptions
            },
        },
        inputs: SemanticInputs {
            dependencies: label(text(&fixture.inputs, "/dependencies")?)?,
            model: label(text(&fixture.inputs, "/model")?)?,
        },
        policy: SemanticMergePolicy {
            id: core(PolicyId::new(text(policy, "/id")?))?,
            mode,
            lineage: LineagePolicy::PijulChangeContainment {
                require_greatest_common_base: flag(
                    policy,
                    "/lineage/require_greatest_common_base",
                )?,
                max_candidate_extra_changes: usize::try_from(number(
                    policy,
                    "/lineage/max_candidate_extra_changes",
                )?)?,
            },
            admitted_checkers: vec![checker.clone()],
            admitted_source_profiles: vec![PijulSourceProfile::pinned_local().id],
            admitted_candidate_suppliers: texts(policy, "/admitted_candidate_suppliers")?
                .into_iter()
                .map(|supplier| core(ProviderProfileId::new(supplier)))
                .collect::<anyhow::Result<_>>()?,
            retry: RetryPolicy {
                max_attempts: u32::try_from(number(policy, "/retry/max_attempts")?)?,
                retry_tool_error: flag(policy, "/retry/retry_tool_error")?,
                retry_exhausted: flag(policy, "/retry/retry_exhausted")?,
            },
            bound_ceilings: policy_bounds(policy)?,
        },
        checker,
        bounds: bounds(&fixture.bounds),
    })
}

fn verdict(value: &str) -> anyhow::Result<CheckerVerdict> {
    Ok(match value {
        "verified" => CheckerVerdict::Verified,
        "counterexample" => CheckerVerdict::Counterexample,
        "unknown" => CheckerVerdict::Unknown,
        "unsupported" => CheckerVerdict::Unsupported,
        "exhausted_wall_time" => CheckerVerdict::Exhausted(ExhaustionBound::WallTime),
        "exhausted_memory" => CheckerVerdict::Exhausted(ExhaustionBound::Memory),
        "cancelled" => CheckerVerdict::Cancelled,
        "tool_error" => CheckerVerdict::ToolError,
        other => anyhow::bail!("unknown verdict {other}"),
    })
}

fn outcome_name(outcome: SemanticOutcome) -> &'static str {
    match outcome {
        SemanticOutcome::Verified => "verified",
        SemanticOutcome::Counterexample => "counterexample",
        SemanticOutcome::Unknown => "unknown",
        SemanticOutcome::Unsupported => "unsupported",
        SemanticOutcome::Exhausted(ExhaustionBound::WallTime) => "exhausted_wall_time",
        SemanticOutcome::Exhausted(ExhaustionBound::Memory) => "exhausted_memory",
        SemanticOutcome::Exhausted(_) => "exhausted",
        SemanticOutcome::Cancelled => "cancelled",
        SemanticOutcome::Stale(_) => "stale",
        SemanticOutcome::ToolError => "tool_error",
    }
}

fn evidence_kind(value: &str) -> anyhow::Result<EvidenceKind> {
    Ok(match value {
        "clean_text_merge" => EvidenceKind::CleanTextMerge,
        "equal_source_trees" => EvidenceKind::EqualSourceTrees,
        "passing_tests" => EvidenceKind::PassingTests,
        "evidence_envelope" => EvidenceKind::EvidenceEnvelope,
        other => anyhow::bail!("unknown evidence kind {other}"),
    })
}

/// Fixture checker producer. It is the only `SemanticCheckerPort` implementation admitted here.
#[derive(Debug)]
struct FixtureCheckerProducer {
    identity: CheckerIdentity,
    script: FixtureProducer,
    verdict: CheckerVerdict,
    calls: Cell<usize>,
}

impl FixtureCheckerProducer {
    fn new(fixture: &Fixture) -> anyhow::Result<Self> {
        Ok(Self {
            identity: checker_identity(fixture)?,
            script: fixture.producer.clone(),
            verdict: verdict(&fixture.producer.verdict)?,
            calls: Cell::new(0),
        })
    }

    fn observe(&self, request: &CheckerRequest) -> anyhow::Result<CheckerObservation> {
        let request_identity = &request.request.binding().identity;
        let bind = |kind: ArtifactKind, name: &str, content: &str| -> anyhow::Result<_> {
            let content = label(content)?;
            Ok(BoundArtifact {
                kind,
                name: core(Alias::new(name))?,
                binding: core(artifact_binding(
                    request_identity,
                    &request.attempt.identity,
                    kind,
                    name,
                    &content,
                ))?,
                content,
            })
        };
        let mut artifacts = vec![bind(ArtifactKind::Log, "checker-log", &self.script.log)?];
        for product in &self.script.products {
            artifacts.push(bind(
                ArtifactKind::Product,
                &product.name,
                &product.content,
            )?);
        }
        Ok(CheckerObservation {
            attempt: request.attempt.identity.clone(),
            binding: request.request.binding().clone(),
            checker: self.identity.clone(),
            evidence_kind: EvidenceKind::SemanticCheckerReceipt,
            verdict: self.verdict,
            coverage: self
                .script
                .coverage
                .iter()
                .map(|unit| core(Alias::new(unit.as_str())))
                .collect::<anyhow::Result<_>>()?,
            output_complete: self.script.output_complete,
            receipt: Some(label(&self.script.receipt)?),
            artifacts,
        })
    }
}

impl SemanticCheckerPort for FixtureCheckerProducer {
    fn producer(&self) -> &CheckerIdentity {
        &self.identity
    }

    fn check(
        &mut self,
        request: &CheckerRequest,
        cancellation: &AtomicBool,
    ) -> PortResult<CheckerObservation> {
        self.calls.set(self.calls.get() + 1);
        if cancellation.load(std::sync::atomic::Ordering::Acquire) {
            return Err(PortFailure::Cancelled(
                kiln_provider_api::InfrastructureFailure {
                    code: "fixture_cancelled".to_string(),
                    message: "fixture producer observed cancellation".to_string(),
                },
            ));
        }
        self.observe(request).map_err(|error| {
            PortFailure::Infrastructure(kiln_provider_api::InfrastructureFailure {
                code: "fixture_producer".to_string(),
                message: error.to_string(),
            })
        })
    }
}

fn apply(
    state: &SemanticCheckState,
    decision: &SemanticCheckDecision,
) -> anyhow::Result<SemanticCheckState> {
    let mut next = state.clone();
    for event in &decision.events {
        next = core(evolve(&next, event))?;
    }
    Ok(next)
}

fn dispatch(state: &SemanticCheckState) -> anyhow::Result<(SemanticCheckState, CheckerRequest)> {
    let decision = core(decide(state, &SemanticCheckCommand::Dispatch))?;
    let Some(SemanticCheckEffect::DispatchChecker(request)) = decision.effects.first() else {
        anyhow::bail!("dispatch requested no checker effect");
    };
    Ok((apply(state, &decision)?, request.as_ref().clone()))
}

fn observe(
    state: &SemanticCheckState,
    observation: CheckerObservation,
) -> Result<SemanticCheckDecision, Diagnostic> {
    decide(
        state,
        &SemanticCheckCommand::ObserveResult(Box::new(observation)),
    )
}

fn dispatch_identity() -> anyhow::Result<CheckerDispatchIdentity> {
    Ok(CheckerDispatchIdentity {
        effect_id: core(EffectId::new("semantic-merge-effect-1"))?,
        attempt_id: core(AttemptId::new("semantic-merge-attempt-1"))?,
        candidate_id: core(CandidateId::new("semantic-merge-candidate-1"))?,
        pipeline_revision_id: core(PipelineRevisionId::new("semantic-merge-pipeline-1"))?,
        provider_profile: core(ProviderProfileId::new("lattice-workflow-v1"))?,
        platform: core(Alias::new("x86_64-linux"))?,
        bounds: WorkflowBounds {
            max_nodes: WORKFLOW_GRAPH_BOUND,
            max_edges: WORKFLOW_GRAPH_BOUND,
            max_log_references: WORKFLOW_GRAPH_BOUND,
            max_log_read_bytes: WORKFLOW_LOG_BYTES,
            max_total_log_bytes: WORKFLOW_LOG_BYTES,
        },
    })
}

fn independent_gates(fixture: &Fixture) -> anyhow::Result<Vec<IndependentGate>> {
    fixture
        .independent_gates
        .iter()
        .map(|gate| {
            Ok(IndependentGate {
                name: core(Alias::new(gate.name.as_str()))?,
                kind: match gate.kind.as_str() {
                    "build" => IndependentGateKind::Build,
                    "test" => IndependentGateKind::Test,
                    "lint" => IndependentGateKind::Lint,
                    "review" => IndependentGateKind::Review,
                    other => anyhow::bail!("unknown gate kind {other}"),
                },
                required: true,
                passed: gate.passed,
            })
        })
        .collect()
}

/// Re-acquires every role through the adapter and returns the fresh binding and context.
fn reobserve(
    fixture: &Fixture,
    repository: &FixtureRepository,
) -> anyhow::Result<(
    kiln_core::semantic_merge::RequestBinding,
    Vec<ContextExpectation>,
)> {
    let request = build_request(fixture, repository)?;
    let context = MergeRole::ALL
        .into_iter()
        .map(|role| {
            let resolution = &request.role(role).resolution;
            ContextExpectation {
                role,
                state: resolution.exact_revision.clone(),
                tree: resolution.canonical_tree.clone(),
            }
        })
        .collect();
    Ok((core(reobserve_binding(&request))?, context))
}

// r[verify kiln.semantic_merge.dispatch.scenario.e2e]
// r[verify kiln.semantic_merge.dispatch]
#[test]
fn frozen_quartet_reaches_a_ci_decision_through_provider_ports() -> anyhow::Result<()> {
    let fixture = load()?;
    let repository = FixtureRepository::new(&fixture)?;

    // Exact source acquisition: four channel states through the read-only adapter mapping.
    let request = core(admit_request(build_request(&fixture, &repository)?))?;
    let state = SemanticCheckState::new(request.clone());

    // Checker request: one attempt, mapped onto the versioned Lattice workflow plan.
    let (state, checker_request) = dispatch(&state)?;
    let plan = core(checker_workflow_plan(
        &checker_request,
        &dispatch_identity()?,
    ))?;
    anyhow::ensure!(plan.idempotency_key == checker_request.attempt.identity);

    // Result evidence from the fixture producer port.
    let mut producer = FixtureCheckerProducer::new(&fixture)?;
    let observation = port(dispatch_semantic_check(
        &mut producer,
        &checker_request,
        &AtomicBool::new(false),
    ))?;
    let state = apply(&state, &core(observe(&state, observation))?)?;
    let result = state.result().context("result was not recorded")?;
    anyhow::ensure!(result.receipt == Some(label(&fixture.producer.receipt)?));
    anyhow::ensure!(result.artifacts.len() == 1 + fixture.producer.products.len());

    // CI decision against a fresh re-acquisition of all four roles.
    let (fresh, context) = reobserve(&fixture, &repository)?;
    let verdict = core(decide_gate(&state, &fresh))?;
    assert_eq!(verdict.outcome, Some(SemanticOutcome::Verified));
    assert!(verdict.satisfies_required && !verdict.blocks && !verdict.conflict_demonstrated);
    let combined = core(combine_gates(&verdict, &independent_gates(&fixture)?))?;
    assert!(combined.passed);

    // Downstream operations must revalidate; the verdict grants no authority.
    assert!(!verdict.revalidation.grants_mutation_authority());
    core(check_revalidation(&verdict.revalidation, &context))?;
    assert_eq!(producer.calls.get(), 1);
    Ok(())
}

#[test]
fn substituted_producer_and_port_failures_fail_before_success() -> anyhow::Result<()> {
    let fixture = load()?;
    let repository = FixtureRepository::new(&fixture)?;
    let request = core(admit_request(build_request(&fixture, &repository)?))?;
    let (state, checker_request) = dispatch(&SemanticCheckState::new(request))?;

    let mut substitute = FixtureCheckerProducer::new(&fixture)?;
    substitute.identity.toolchain = label("unreviewed toolchain")?;
    let refused =
        dispatch_semantic_check(&mut substitute, &checker_request, &AtomicBool::new(false));
    assert!(matches!(
        refused,
        Err(PortFailure::Domain(Diagnostic { ref code, .. })) if code == "semantic_checker_substitution"
    ));
    assert_eq!(substitute.calls.get(), 0);

    let mut producer = FixtureCheckerProducer::new(&fixture)?;
    let cancelled =
        dispatch_semantic_check(&mut producer, &checker_request, &AtomicBool::new(true));
    let Err(failure) = cancelled else {
        anyhow::bail!("cancelled check returned a result");
    };
    let state = apply(
        &state,
        &core(decide(
            &state,
            &SemanticCheckCommand::ObserveFailure {
                attempt: checker_request.attempt.identity.clone(),
                failure: producer_failure(&failure),
            },
        ))?,
    )?;
    let verdict = core(decide_gate(&state, state.request().binding()))?;
    assert_eq!(verdict.outcome, Some(SemanticOutcome::Cancelled));
    assert!(verdict.blocks);
    Ok(())
}

fn expect_rejection(
    case: &NegativeCase,
    result: Result<SemanticCheckDecision, Diagnostic>,
) -> anyhow::Result<()> {
    let Err(diagnostic) = result else {
        anyhow::bail!("negative case {} was admitted", case.name);
    };
    assert_eq!(
        Some(&diagnostic.code),
        case.expected_code.as_ref(),
        "{}",
        case.name
    );
    assert_eq!(
        Some(&diagnostic.field),
        case.expected_field.as_ref(),
        "{}",
        case.name
    );
    Ok(())
}

fn moved_role(case: &NegativeCase, fixture: &Fixture) -> anyhow::Result<(MergeRole, FixtureRole)> {
    let role = match case.role.as_deref() {
        Some("base") => MergeRole::Base,
        Some("variant_a") => MergeRole::VariantA,
        Some("variant_b") => MergeRole::VariantB,
        Some("candidate") => MergeRole::Candidate,
        other => anyhow::bail!("negative case {} names role {other:?}", case.name),
    };
    let mut moved = fixture_role(fixture, role).clone();
    moved.state = case.state.clone().context("moved role needs a state")?;
    moved.changes = case.changes.clone().context("moved role needs changes")?;
    Ok((role, moved))
}

fn run_negative_case(fixture: &Fixture, case: &NegativeCase) -> anyhow::Result<()> {
    let repository = FixtureRepository::new(fixture)?;
    let base_request = build_request(fixture, &repository)?;
    let request = core(admit_request(base_request.clone()))?;
    let (state, checker_request) = dispatch(&SemanticCheckState::new(request))?;
    let producer = FixtureCheckerProducer::new(fixture)?;
    let mut observation = producer.observe(&checker_request)?;
    match case.mutation.as_str() {
        "swap_variants" => {
            let mut swapped = base_request;
            std::mem::swap(
                &mut swapped.variant_a.resolution,
                &mut swapped.variant_b.resolution,
            );
            observation.binding = core(reobserve_binding(&swapped))?;
            expect_rejection(case, observe(&state, observation))
        }
        "result_for_moved_role" => {
            let (_, moved) = moved_role(case, fixture)?;
            let other = FixtureRepository::new(fixture)?;
            other.record(fixture, &moved)?;
            observation.binding = core(reobserve_binding(&build_request(fixture, &other)?))?;
            expect_rejection(case, observe(&state, observation))
        }
        "change_dependencies" => {
            let mut changed = base_request;
            changed.inputs.dependencies = label(case.label.as_deref().context("label")?)?;
            observation.binding = core(reobserve_binding(&changed))?;
            expect_rejection(case, observe(&state, observation))
        }
        "change_policy" => {
            let mut changed = base_request;
            changed.policy.mode = CheckMode::Advisory;
            observation.binding = core(reobserve_binding(&changed))?;
            expect_rejection(case, observe(&state, observation))
        }
        "missing_coverage" => {
            observation.coverage.pop();
            expect_rejection(case, observe(&state, observation))
        }
        "substitute_evidence" => {
            observation.evidence_kind =
                evidence_kind(case.evidence_kind.as_deref().context("evidence kind")?)?;
            expect_rejection(case, observe(&state, observation))
        }
        "replay_attempt" => {
            // A complete, correctly bound result replayed from another evaluation's attempt.
            observation.attempt = core(attempt_binding(&label("another evaluation")?, 1))?.identity;
            expect_rejection(case, observe(&state, observation))
        }
        "verdict" | "partial_output" => {
            if case.mutation == "partial_output" {
                observation.output_complete = false;
            } else {
                observation.verdict = verdict(case.verdict.as_deref().context("verdict")?)?;
            }
            let state = apply(&state, &core(observe(&state, observation))?)?;
            let gate = core(decide_gate(&state, state.request().binding()))?;
            let outcome = gate.outcome.context("outcome missing")?;
            assert_eq!(
                Some(outcome_name(outcome)),
                case.expected_outcome.as_deref(),
                "{}",
                case.name
            );
            assert!(gate.blocks && !gate.satisfies_required, "{}", case.name);
            assert_eq!(
                gate.conflict_demonstrated,
                outcome == SemanticOutcome::Counterexample,
                "{}",
                case.name
            );
            if let Some(code) = &case.expected_code {
                assert_eq!(
                    gate.diagnostic
                        .as_ref()
                        .map(|diagnostic| diagnostic.code.as_str()),
                    Some(code.as_str()),
                    "{}",
                    case.name
                );
            }
            let combined = core(combine_gates(&gate, &independent_gates(fixture)?))?;
            assert!(!combined.passed, "{}", case.name);
            Ok(())
        }
        "cancel_then_result" => {
            let cancelled = apply(
                &state,
                &core(decide(&state, &SemanticCheckCommand::Cancel))?,
            )?;
            expect_rejection(case, observe(&cancelled, observation))?;
            let gate = core(decide_gate(&cancelled, cancelled.request().binding()))?;
            assert_eq!(
                gate.outcome.map(outcome_name),
                case.expected_outcome.as_deref()
            );
            assert!(gate.blocks);
            Ok(())
        }
        "concurrent_source_change" => {
            // Another writer moves a channel while the check runs; Kiln re-acquires every role.
            let (_, moved) = moved_role(case, fixture)?;
            repository.record(fixture, &moved)?;
            let (fresh, _) = reobserve(fixture, &repository)?;
            let stale = apply(
                &state,
                &core(decide(
                    &state,
                    &SemanticCheckCommand::ObserveInputs(Box::new(fresh.clone())),
                ))?,
            )?;
            expect_rejection(case, observe(&stale, observation))?;
            let gate = core(decide_gate(&stale, &fresh))?;
            assert_eq!(
                gate.outcome.map(outcome_name),
                case.expected_outcome.as_deref()
            );
            assert!(gate.blocks);
            Ok(())
        }
        other => anyhow::bail!("unknown mutation {other}"),
    }
}

// r[verify kiln.semantic_merge.binding.scenario.changed]
// r[verify kiln.semantic_merge.binding.scenario.coverage]
// r[verify kiln.semantic_merge.outcomes.scenario.nonverified]
// r[verify kiln.semantic_merge.freshness.scenario.late]
// r[verify kiln.semantic_merge.freshness.scenario.concurrent]
// r[verify kiln.semantic_merge.independence.scenario.substitution]
#[test]
fn every_negative_fixture_fails_closed_with_a_named_binding() -> anyhow::Result<()> {
    let fixture = load()?;
    anyhow::ensure!(!fixture.negative_cases.is_empty());
    for case in &fixture.negative_cases {
        run_negative_case(&fixture, case).with_context(|| format!("case {}", case.name))?;
    }
    Ok(())
}
