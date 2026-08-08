use std::collections::{BTreeMap, BTreeSet};

use kiln_core::Diagnostic;
use kiln_core::cob::{
    CobCapability, CobEffectPlan, CobObservation, CobProfile, plan_effect, select_profile,
};
use kiln_core::gating::{
    CandidateChange, CandidateSet, DependencyEdge, GateResult, MergePreflight, admit_merge,
    validate_candidate_set,
};
use kiln_core::hosting::{
    AcquisitionObservation, AcquisitionRoute, HostingProfile, HostingState, PublicationBundle,
    PublicationCandidate, validate_publication_bundle,
};
use kiln_core::task_ops::{
    ControlKind, ControlRequest, CursorDecision, LogCursor, LogStreamMeta, StatusRecord,
    StatusSelection, WaitPredicate, admit_control, evaluate_wait, filter_status, validate_cursor,
};
use kiln_core::types::{
    Alias, AttemptId, BLAKE3_HEX_BYTES, CandidateId, CobObjectId, CobProfileId, CobSemanticStateId,
    ContentId, EffectId, EvaluationId, ExternalRef, PipelineRevisionId, PolicyId, ProjectId,
    ProviderProfileId, RunId, StreamId, TaskId,
};

const MAX_CHANGES: usize = 8;
const MAX_EDGES: usize = 16;
const COB_MAPPER_VERSION: u16 = 1;
const MAX_COB_HEADS: usize = 8;
const MAX_COB_PAYLOAD_BYTES: usize = 4096;
const PUBLICATION_SEQUENCE: u64 = 2;
const PREVIOUS_PUBLICATION_SEQUENCE: u64 = 1;
const OBSERVED_AT: u64 = 10;
const CURRENT_TIME: u64 = 12;
const MAX_OBSERVATION_AGE: u64 = 5;
const RETAINED_START: u64 = 10;
const RETAINED_END: u64 = 20;

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(BLAKE3_HEX_BYTES))
        .unwrap_or_else(|_| unreachable!())
}

fn external(namespace: &str, kind: &str, value: &str) -> ExternalRef {
    ExternalRef::new(namespace, kind, value).unwrap_or_else(|_| unreachable!())
}

fn candidate() -> CandidateSet {
    let change = CandidateChange {
        change: external("radicle", "patch", "patch"),
        target_revision: external("git", "commit", "target"),
        source_snapshot: content('a'),
        pipeline_revision: PipelineRevisionId::new("pipeline").unwrap_or_else(|_| unreachable!()),
        collaboration_provider: ProviderProfileId::new("radicle")
            .unwrap_or_else(|_| unreachable!()),
    };
    CandidateSet {
        id: CandidateId::new("candidate").unwrap_or_else(|_| unreachable!()),
        ordered_changes: vec![change],
        dependencies: Vec::new(),
        frozen_sources: BTreeMap::from([(external("git", "repository", "repo"), content('a'))]),
        policy_id: PolicyId::new("gate-policy").unwrap_or_else(|_| unreachable!()),
    }
}

#[test]
fn exact_candidate_and_merge_preflight_pass() {
    let candidate = candidate();
    assert_eq!(
        validate_candidate_set(&candidate, MAX_CHANGES, MAX_EDGES),
        Ok(())
    );
    let result = GateResult {
        candidate_id: candidate.id.clone(),
        policy_id: candidate.policy_id.clone(),
        passed: true,
    };
    let target = external("git", "commit", "target");
    let preflight = MergePreflight {
        candidate_id: candidate.id.clone(),
        policy_id: candidate.policy_id.clone(),
        expected_target: target.clone(),
        observed_target: target,
        approvals_current: true,
        provider_capable: true,
    };
    assert_eq!(admit_merge(&result, &candidate, &preflight), Ok(()));
}

#[test]
fn cyclic_or_reversed_dependency_fails() {
    let mut candidate = candidate();
    candidate.ordered_changes.push(
        candidate
            .ordered_changes
            .first()
            .cloned()
            .unwrap_or_else(|| unreachable!()),
    );
    candidate.dependencies.push(DependencyEdge {
        before_index: 1,
        after_index: 0,
    });
    assert!(validate_candidate_set(&candidate, MAX_CHANGES, MAX_EDGES).is_err());
}

fn cob_fixture() -> (CobProfile, CobObservation) {
    let profile_id = CobProfileId::new("cairn-adr-v1").unwrap_or_else(|_| unreachable!());
    let type_name = external("radicle", "type", "xyz.onixresearch.adr");
    let schema = external("cairn", "schema", "adr-v1");
    let profile = CobProfile {
        id: profile_id.clone(),
        provider: ProviderProfileId::new("radicle-cob").unwrap_or_else(|_| unreachable!()),
        type_name: type_name.clone(),
        schema_line: schema.clone(),
        mapper_version: COB_MAPPER_VERSION,
        capabilities: BTreeSet::from([CobCapability::TypedRead, CobCapability::Update]),
        max_heads: MAX_COB_HEADS,
        max_payload_bytes: MAX_COB_PAYLOAD_BYTES,
        complete_head_resolution: true,
    };
    let observation = CobObservation {
        profile_id,
        repository: external("radicle", "repository", "rid"),
        type_name,
        schema_line: schema,
        object_id: CobObjectId::new("object").unwrap_or_else(|_| unreachable!()),
        heads: BTreeSet::from([external("radicle", "operation", "head")]),
        semantic_state: CobSemanticStateId::new("state").unwrap_or_else(|_| unreachable!()),
        projection_identity: content('b'),
    };
    (profile, observation)
}

#[test]
fn cob_profile_and_fresh_plan_pass() {
    let (profile, observation) = cob_fixture();
    assert!(
        select_profile(
            std::slice::from_ref(&profile),
            &observation,
            CobCapability::TypedRead
        )
        .is_ok()
    );
    let plan = CobEffectPlan {
        effect_id: EffectId::new("effect").unwrap_or_else(|_| unreachable!()),
        profile_id: profile.id.clone(),
        repository: observation.repository.clone(),
        object_id: observation.object_id.clone(),
        expected_heads: observation.heads.clone(),
        action: CobCapability::Update,
        payload_identity: content('c'),
        actor: external("radicle", "actor", "did:key:actor"),
        policy_id: PolicyId::new("cob-policy").unwrap_or_else(|_| unreachable!()),
        idempotency_key: external("kiln", "effect", "effect"),
    };
    assert_eq!(plan_effect(&profile, &observation, plan.clone()), Ok(plan));
}

#[test]
fn unknown_cob_schema_fails_closed() {
    let (profile, mut observation) = cob_fixture();
    observation.schema_line = external("cairn", "schema", "unknown");
    assert!(select_profile(&[profile], &observation, CobCapability::TypedRead).is_err());
}

fn status(outcome: kiln_core::domain::Outcome) -> StatusRecord {
    StatusRecord {
        project_id: ProjectId::new("project").unwrap_or_else(|_| unreachable!()),
        evaluation_id: EvaluationId::new("evaluation").unwrap_or_else(|_| unreachable!()),
        run_id: RunId::new("run").unwrap_or_else(|_| unreachable!()),
        task_id: TaskId::new("task").unwrap_or_else(|_| unreachable!()),
        attempt_id: Some(AttemptId::new("attempt").unwrap_or_else(|_| unreachable!())),
        provider: ProviderProfileId::new("provider").unwrap_or_else(|_| unreachable!()),
        capacity_class: Alias::new("local").unwrap_or_else(|_| unreachable!()),
        readiness: kiln_core::task_ops::ReadinessFacet::Ready,
        execution: kiln_core::task_ops::ExecutionFacet::Running,
        outcome,
    }
}

#[test]
fn status_filter_covers_every_exact_selection_dimension() {
    let record = status(kiln_core::domain::Outcome::Success);
    let selection = StatusSelection {
        project_ids: BTreeSet::from([record.project_id.clone()]),
        evaluation_ids: BTreeSet::from([record.evaluation_id.clone()]),
        run_ids: BTreeSet::from([record.run_id.clone()]),
        task_ids: BTreeSet::from([record.task_id.clone()]),
        attempt_ids: record.attempt_id.iter().cloned().collect(),
        providers: BTreeSet::from([record.provider.clone()]),
        capacity_classes: BTreeSet::from([record.capacity_class.clone()]),
        outcomes: BTreeSet::from([record.outcome]),
    };
    assert_eq!(
        filter_status(std::slice::from_ref(&record), &selection, 1),
        Ok(vec![record])
    );
}

#[test]
fn wait_predicate_is_read_only() {
    assert!(evaluate_wait(
        &[status(kiln_core::domain::Outcome::Success)],
        WaitPredicate::AllTerminal
    ));
    assert!(!evaluate_wait(
        &[status(kiln_core::domain::Outcome::Pending)],
        WaitPredicate::AllTerminal
    ));
}

#[test]
fn cursor_gap_is_explicit() {
    let stream_id = StreamId::new("stream").unwrap_or_else(|_| unreachable!());
    let metadata = LogStreamMeta {
        stream_id: stream_id.clone(),
        attempt_id: AttemptId::new("attempt").unwrap_or_else(|_| unreachable!()),
        segments: Vec::new(),
        retained_start: RETAINED_START,
        retained_end: RETAINED_END,
        complete: false,
        truncated: true,
    };
    let cursor = LogCursor {
        stream_id,
        offset: 0,
    };
    assert_eq!(
        validate_cursor(&metadata, &cursor),
        Ok(CursorDecision::Gap {
            retained_start: RETAINED_START,
        })
    );
}

#[test]
fn unsupported_pause_dispatches_nothing() {
    let request = ControlRequest {
        kind: ControlKind::Pause,
        attempt_id: Some(AttemptId::new("attempt").unwrap_or_else(|_| unreachable!())),
        active_attempt: true,
        declared_operations: BTreeSet::new(),
    };
    assert_eq!(
        admit_control(&request),
        Err(Diagnostic::new(
            "unsupported_control",
            "control.kind",
            "provider does not declare this control capability"
        ))
    );
}

fn hosting_profile(rid: ExternalRef) -> HostingProfile {
    HostingProfile {
        state: HostingState::Published,
        profile_id: PolicyId::new("hosting").unwrap_or_else(|_| unreachable!()),
        canonical_branch: external("git", "branch", "main"),
        rid: Some(rid),
        delegates: vec![external("radicle", "delegate", "did:key:one")],
        threshold: 1,
        required_replicas: 1,
        license: external("spdx", "license", "MIT OR Apache-2.0"),
        rollback_revision: external("git", "commit", "rollback"),
        signed_ref_features: BTreeSet::from([external(
            "radicle",
            "signed-ref-feature",
            "canonical-branch",
        )]),
        seeds: vec![external("radicle", "seed", "seed-one")],
        acquisition_routes: vec![AcquisitionRoute {
            route: external("https", "source-archive", "route"),
            writable: false,
        }],
        freeze: false,
        mirrors: Vec::new(),
    }
}

fn publication_fixture() -> (HostingProfile, PublicationBundle) {
    let rid = external("radicle", "repository", "rad:zrepo");
    let profile = hosting_profile(rid.clone());
    let source = external("git", "commit", "source");
    let candidate = PublicationCandidate {
        source_revision: source.clone(),
        archive_identity: content('d'),
        license: profile.license.clone(),
        lifecycle_evidence: content('e'),
        rollback_revision: profile.rollback_revision.clone(),
        hosting_profile_identity: content('f'),
    };
    let bundle = PublicationBundle {
        sequence: PUBLICATION_SEQUENCE,
        candidate: candidate.clone(),
        rid,
        signed_revision: source.clone(),
        observed_delegates: profile.delegates.clone(),
        observed_threshold: profile.threshold,
        acquisitions: vec![AcquisitionObservation {
            route: external("https", "source-archive", "route"),
            revision: source,
            archive_identity: candidate.archive_identity,
            observed_at: OBSERVED_AT,
        }],
    };
    (profile, bundle)
}

#[test]
fn consistent_publication_bundle_passes() {
    let (profile, bundle) = publication_fixture();
    assert_eq!(
        validate_publication_bundle(
            &profile,
            &bundle,
            Some(PREVIOUS_PUBLICATION_SEQUENCE),
            CURRENT_TIME,
            MAX_OBSERVATION_AGE,
        ),
        Ok(())
    );
}

#[test]
fn rollback_sequence_fails() {
    let (profile, mut bundle) = publication_fixture();
    bundle.sequence = PREVIOUS_PUBLICATION_SEQUENCE;
    assert!(
        validate_publication_bundle(
            &profile,
            &bundle,
            Some(PREVIOUS_PUBLICATION_SEQUENCE),
            CURRENT_TIME,
            MAX_OBSERVATION_AGE,
        )
        .is_err()
    );
}
