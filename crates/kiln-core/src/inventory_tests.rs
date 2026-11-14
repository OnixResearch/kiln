use super::*;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec;

#[derive(Deserialize)]
struct Fixture {
    profile: InventoryProfile,
    snapshot: InventorySnapshot,
}
type TestResult<T = ()> = Result<T, String>;

fn cohort() -> TestResult<(InventoryProfile, InventorySnapshot, WorkNeeds)> {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../../fixtures/inventory/synthetic-v2.json"
    ))
    .map_err(|error| format!("{error:?}"))?;
    let needs = serde_json::from_str(include_str!(
        "../../../fixtures/inventory/synthetic-work-needs-v2.json"
    ))
    .map_err(|error| format!("{error:?}"))?;
    // Test observations only; turning this flag off tests eligibility semantics, not a live claim.
    let mut snapshot = fixture.snapshot;
    snapshot.synthetic = false;
    Ok((fixture.profile, snapshot, needs))
}
fn has(explanation: &ReadinessExplanation, code: &str) -> bool {
    explanation
        .blockers
        .iter()
        .chain(
            explanation
                .candidates
                .iter()
                .flat_map(|c| c.blockers.iter()),
        )
        .any(|b| b.code == code)
}
fn reference(source: &str, id: &str) -> FactRef {
    FactRef {
        source: source.into(),
        id: id.into(),
    }
}

// r[verify kiln.inventory.separation]
#[test]
fn distinct_definitions_workers_compute_and_shared_serving() -> TestResult {
    let (profile, mut snapshot, needs) = cohort()?;
    let result = explain_readiness(&profile, &snapshot, &needs, 2000)?;
    assert_eq!(result.candidates.len(), 2);
    assert!(result.candidates.iter().all(|c| c.observed_eligible));
    assert!(!result.dispatch_authorized);
    let first_worker = snapshot.workers.first().ok_or("missing first worker")?;
    let second_worker = snapshot.workers.get(1).ok_or("missing second worker")?;
    assert_eq!(first_worker.execution, second_worker.execution);
    assert_eq!(first_worker.model_services, second_worker.model_services);
    let projection = project_inventory(&profile, &snapshot, Some(&needs), 2000)?;
    assert_eq!(projection.snapshot.models.len(), 1);
    let model = snapshot.models.first().ok_or("missing model")?;
    assert_eq!(
        projection
            .snapshot
            .pools
            .iter()
            .flat_map(|p| &p.allocations)
            .filter(|a| a.consumer == model.meta.reference)
            .count(),
        1
    );
    assert_ne!(
        snapshot
            .compute
            .first()
            .ok_or("missing first compute")?
            .host,
        snapshot
            .compute
            .get(1)
            .ok_or("missing second compute")?
            .host
    );
    snapshot.workers.clear();
    let projection = project_inventory(&profile, &snapshot, Some(&needs), 2000)?;
    assert_eq!(projection.snapshot.agents.len(), 1);
    assert_eq!(projection.snapshot.workers.len(), 0);
    assert!(has(
        projection
            .explanation
            .as_ref()
            .ok_or("missing readiness explanation")?,
        "no_observed_worker"
    ));
    Ok(())
}

#[test]
fn shared_memory_pool_is_a_single_authoritative_capacity() -> TestResult {
    let (profile, mut snapshot, needs) = cohort()?;
    *snapshot
        .compute
        .first_mut()
        .ok_or("missing compute")?
        .accounting_pools
        .get_mut(1)
        .ok_or("missing execution memory pool reference")? = reference("compute", "serving-memory");
    snapshot
        .pools
        .retain(|p| p.meta.reference.id != "execution-memory");
    let projection = project_inventory(&profile, &snapshot, Some(&needs), 2000)?;
    assert_eq!(projection.snapshot.pools.len(), 2);
    let shared = projection
        .snapshot
        .pools
        .iter()
        .find(|p| p.meta.reference.id == "serving-memory")
        .ok_or("missing serving memory pool")?;
    assert_eq!(shared.capacity, Some(16000));
    assert_eq!(shared.allocatable, Some(10000));
    assert_eq!(shared.allocations.len(), 1);
    let duplicate = shared.clone();
    snapshot.pools.push(duplicate);
    assert_eq!(
        admit_snapshot(&profile, &snapshot),
        Err("inventory_fact_ambiguous")
    );
    Ok(())
}

#[test]
fn hardware_artifact_loading_saturation_and_request_admission_are_distinct() -> TestResult {
    let (profile, mut snapshot, needs) = cohort()?;
    snapshot.models.first_mut().ok_or("missing model")?.state = ServingState::ArtifactCached;
    assert!(has(
        &explain_readiness(&profile, &snapshot, &needs, 2000)?,
        "model_not_ready"
    ));
    snapshot.models.first_mut().ok_or("missing model")?.state = ServingState::Loading;
    assert!(has(
        &explain_readiness(&profile, &snapshot, &needs, 2000)?,
        "model_loading"
    ));
    let model = snapshot.models.first_mut().ok_or("missing model")?;
    model.state = ServingState::Saturated;
    model.available_requests = Some(0);
    assert!(has(
        &explain_readiness(&profile, &snapshot, &needs, 2000)?,
        "inference_saturated"
    ));
    assert!(snapshot.compute.iter().all(|c| c.health == Health::Healthy));
    let model = snapshot.models.first_mut().ok_or("missing model")?;
    assert_eq!(model.artifact_available, Some(true));
    assert!(model.artifact_verification_evidence.is_none());
    model.state = ServingState::Warm;
    model.available_requests = Some(2);
    assert!(
        explain_readiness(&profile, &snapshot, &needs, 2000)?
            .candidates
            .iter()
            .all(|c| c.observed_eligible)
    );
    assert!(snapshot.admissions.is_empty());
    Ok(())
}

#[test]
fn source_missing_stale_partial_out_of_order_and_future_never_ready() -> TestResult {
    let (profile, snapshot, needs) = cohort()?;
    for state in [
        ObservationState::Missing,
        ObservationState::Stale,
        ObservationState::Incomplete,
        ObservationState::OutOfOrder,
        ObservationState::Future,
        ObservationState::Unavailable,
    ] {
        let mut changed = snapshot.clone();
        let source = changed
            .sources
            .iter_mut()
            .find(|s| s.source == "serving")
            .ok_or("missing serving source")?;
        match state {
            ObservationState::Missing => {
                changed.models.clear();
                changed.sources.retain(|s| s.source != "serving");
            }
            ObservationState::Stale => {
                source.observed_at_ms = 0;
                source.valid_until_ms = 1500;
                let model = changed.models.first_mut().ok_or("missing model")?;
                model.meta.observed_at_ms = 0;
                model.meta.valid_until_ms = 1500;
            }
            ObservationState::Incomplete => {
                source.complete = false;
                source.next_page = Some("page-two".into());
            }
            ObservationState::OutOfOrder => source.generation = 6,
            ObservationState::Future => {
                source.observed_at_ms = 3000;
            }
            ObservationState::Unavailable => {
                source.unavailable_reason = Some("provider_unreachable".into())
            }
            _ => return Err("unexpected observation state".into()),
        }
        let projection = project_inventory(&profile, &changed, Some(&needs), 2000)?;
        assert_eq!(
            projection
                .sources
                .iter()
                .find(|s| s.source == "serving")
                .ok_or("missing serving source projection")?
                .state,
            state
        );
        let explanation = projection
            .explanation
            .as_ref()
            .ok_or("missing readiness explanation")?;
        assert!(has(explanation, "provider_observation_unknown"));
        assert!(explanation.candidates.iter().all(|c| !c.observed_eligible));
    }
    Ok(())
}

#[test]
fn stale_individual_fact_and_explicit_unknown_are_not_capacity() -> TestResult {
    let (profile, mut snapshot, needs) = cohort()?;
    snapshot
        .models
        .first_mut()
        .ok_or("missing model")?
        .meta
        .valid_until_ms = 1500;
    assert!(has(
        &explain_readiness(&profile, &snapshot, &needs, 2000)?,
        "stale_or_unknown_serving"
    ));
    let model = snapshot.models.first_mut().ok_or("missing model")?;
    model.meta.valid_until_ms = 10000;
    model.meta.unknown_fields.push("request_shape".into());
    assert!(has(
        &explain_readiness(&profile, &snapshot, &needs, 2000)?,
        "stale_or_unknown_serving"
    ));
    let model = snapshot.models.first_mut().ok_or("missing model")?;
    model.meta.unknown_fields.clear();
    model.available_requests = None;
    assert!(has(
        &explain_readiness(&profile, &snapshot, &needs, 2000)?,
        "inference_capacity_unknown_or_saturated"
    ));
    snapshot
        .pools
        .first_mut()
        .ok_or("missing pool")?
        .allocatable = Some(500);
    assert!(has(
        &explain_readiness(&profile, &snapshot, &needs, 2000)?,
        "execution_capacity_insufficient_or_unknown"
    ));
    Ok(())
}

#[test]
fn unsupported_context_tool_backend_sandbox_and_hosted_api() -> TestResult {
    let (profile, mut snapshot, mut needs) = cohort()?;
    needs.context_tokens = 128000;
    needs.tools.push("unavailable-tool".into());
    needs.model_backend = "unsupported-backend".into();
    needs.execution_backend = "unsupported-execution-backend".into();
    needs.sandbox = "unsupported-sandbox".into();
    let explanation = explain_readiness(&profile, &snapshot, &needs, 2000)?;
    for code in [
        "unsupported_context",
        "unsupported_agent_tool",
        "unsupported_model_tool",
        "incompatible_backend",
        "incompatible_execution_backend",
        "unsupported_sandbox",
    ] {
        assert!(has(&explanation, code));
    }
    let (_, _, needs) = cohort()?;
    let model = snapshot.models.first_mut().ok_or("missing model")?;
    model.accounting_pools.clear();
    model.compute_devices.clear();
    model.artifact_available = None;
    snapshot.compute.retain(|c| c.kind != "accelerator");
    snapshot
        .pools
        .retain(|p| p.meta.reference.id != "serving-memory");
    assert!(
        explain_readiness(&profile, &snapshot, &needs, 2000)?
            .candidates
            .iter()
            .all(|c| c.observed_eligible)
    );
    assert!(
        snapshot
            .models
            .first()
            .ok_or("missing model")?
            .compute_devices
            .is_empty()
    );
    Ok(())
}

fn bound_attempt(snapshot: &mut InventorySnapshot) -> TestResult<AttemptBinding> {
    let execution_grant = reference("policy", "execution-grant");
    let request_grant = reference("policy", "request-grant");
    let worker = snapshot.workers.first_mut().ok_or("missing worker")?;
    let model = snapshot.models.first().ok_or("missing model")?;
    let binding = AttemptBinding {
        work_version: "work-r1".into(),
        attempt: "attempt-a".into(),
        fence: 4,
        worker: worker.meta.reference.clone(),
        execution_grant: execution_grant.clone(),
        model_services: worker.model_services.clone(),
        request_admissions: vec![request_grant.clone()],
    };
    worker.attempt = Some(binding.attempt.clone());
    worker.fence = Some(binding.fence);
    worker.lifecycle = WorkerLifecycle::Executing;
    worker.execution_grant = Some(execution_grant.clone());
    for (grant, resource) in [
        (execution_grant, worker.execution.clone()),
        (request_grant, model.meta.reference.clone()),
    ] {
        snapshot.admissions.push(AdmissionFact {
            meta: FactMeta {
                reference: grant,
                observed_at_ms: 1000,
                valid_until_ms: 10000,
                unknown_fields: Vec::new(),
            },
            work_version: binding.work_version.clone(),
            attempt: binding.attempt.clone(),
            fence: binding.fence,
            resource,
            state: AdmissionState::Admitted,
            owner_reason: None,
        });
    }
    snapshot.attempts.push(binding.clone());
    Ok(binding)
}

#[test]
fn changed_resource_rejection_and_stale_fence_are_visible_not_dispatch() -> TestResult {
    let (profile, mut snapshot, needs) = cohort()?;
    let old = explain_readiness(&profile, &snapshot, &needs, 2000)?;
    assert!(
        old.candidates
            .first()
            .ok_or("missing candidate")?
            .observed_eligible
    );
    let binding = bound_attempt(&mut snapshot)?;
    assert!(explain_attempt_binding(&profile, &snapshot, &binding, 2000)?.is_empty());
    let admission = snapshot
        .admissions
        .get_mut(1)
        .ok_or("missing request admission")?;
    admission.state = AdmissionState::Rejected;
    admission.owner_reason = Some("serving_queue_full".into());
    let projection = project_inventory(&profile, &snapshot, Some(&needs), 2000)?;
    assert!(
        projection
            .attempt_explanations
            .first()
            .ok_or("missing attempt explanation")?
            .blockers
            .iter()
            .any(|b| b.code == "reservation_not_admitted" && b.constraint == "serving_queue_full")
    );
    assert!(
        !projection
            .explanation
            .as_ref()
            .ok_or("missing readiness explanation")?
            .dispatch_authorized
    );
    snapshot.workers.first_mut().ok_or("missing worker")?.fence = Some(5);
    assert!(
        explain_attempt_binding(&profile, &snapshot, &binding, 2000)?
            .iter()
            .any(|b| b.code == "attempt_binding_stale_or_mismatched")
    );
    Ok(())
}

#[test]
fn preparation_failure_promotion_pinning_rollback_and_qualification_are_independent() -> TestResult
{
    let (profile, mut snapshot, needs) = cohort()?;
    let agent = snapshot.agents.first_mut().ok_or("missing agent")?;
    agent.revisions.desired = Some("profile-r2".into());
    agent.revisions.preparation_failure = Some("prepare-r2-failed".into());
    assert!(
        explain_readiness(&profile, &snapshot, &needs, 2000)?
            .candidates
            .first()
            .ok_or("missing candidate")?
            .observed_eligible
    );
    let agent = snapshot.agents.first_mut().ok_or("missing agent")?;
    agent.revisions.selected_for_new_sessions = Some("profile-r2".into());
    agent.profile_revision = "profile-r2".into();
    assert!(has(
        &explain_readiness(&profile, &snapshot, &needs, 2000)?,
        "agent_revision_mismatch"
    ));
    let mut promoted_needs = needs.clone();
    promoted_needs.profile_revision = "profile-r2".into();
    let promoted = explain_readiness(&profile, &snapshot, &promoted_needs, 2000)?;
    assert!(has(&promoted, "worker_pinned_other_revision"));
    assert!(has(&promoted, "qualification_missing_stale_or_mismatched"));
    assert_eq!(
        snapshot
            .workers
            .first()
            .ok_or("missing worker")?
            .pinned_revision,
        "profile-r1"
    );
    let agent = snapshot.agents.first_mut().ok_or("missing agent")?;
    agent.revisions.selected_for_new_sessions = Some("profile-r1".into());
    agent.profile_revision = "profile-r1".into();
    assert!(
        explain_readiness(&profile, &snapshot, &needs, 2000)?
            .candidates
            .first()
            .ok_or("missing candidate")?
            .observed_eligible
    );
    snapshot
        .agents
        .first_mut()
        .ok_or("missing agent")?
        .qualifications
        .first_mut()
        .ok_or("missing qualification")?
        .authority = Permission::Revoked;
    assert!(has(
        &explain_readiness(&profile, &snapshot, &needs, 2000)?,
        "qualification_missing_stale_or_mismatched"
    ));
    let qualification = snapshot
        .agents
        .first_mut()
        .ok_or("missing agent")?
        .qualifications
        .first_mut()
        .ok_or("missing qualification")?;
    qualification.authority = Permission::Allowed;
    qualification.valid_until_ms = 1500;
    assert!(has(
        &explain_readiness(&profile, &snapshot, &needs, 2000)?,
        "qualification_missing_stale_or_mismatched"
    ));
    let projection = project_inventory(&profile, &snapshot, Some(&needs), 2000)?;
    let qualification = projection
        .snapshot
        .agents
        .first()
        .ok_or("missing projected agent")?
        .qualifications
        .first()
        .ok_or("missing projected qualification")?;
    assert_eq!(
        (
            qualification.accepted_outcomes,
            qualification.attempted_outcomes
        ),
        (Some(8), Some(10))
    );
    assert!(qualification.unknown_use);
    assert!(!qualification.limitations.is_empty());
    Ok(())
}

#[test]
fn budget_wait_queue_permission_and_saturated_qualified_service() -> TestResult {
    let (profile, mut snapshot, needs) = cohort()?;
    let policy = snapshot.policies.first_mut().ok_or("missing policy")?;
    policy.parent_budget_remaining = Some(0);
    policy.tenant_limit_remaining = Some(0);
    policy.campaign_limit_remaining = None;
    policy.waiting_until_ms = Some(3000);
    policy.owner_reason = Some("campaign_waiting".into());
    policy.permission = Permission::Revoked;
    let model = snapshot.models.first_mut().ok_or("missing model")?;
    model.state = ServingState::Saturated;
    model.available_requests = Some(0);
    model.queue_reason = Some("serving_queue_full".into());
    let explanation = explain_readiness(&profile, &snapshot, &needs, 2000)?;
    for code in [
        "parent_budget_exhausted",
        "tenant_limit",
        "campaign_limit",
        "bounded_wait",
        "policy_denied_or_unknown",
        "inference_saturated",
    ] {
        assert!(has(&explanation, code));
    }
    assert!(!has(
        &explanation,
        "qualification_missing_stale_or_mismatched"
    ));
    Ok(())
}

#[test]
fn admission_rejects_unsupported_ambiguous_provenance_bounds_and_private_data() -> TestResult {
    let (profile, snapshot, _) = cohort()?;
    let mut bad = snapshot.clone();
    bad.schema = "kiln.observe.inventory.v3".into();
    assert_eq!(
        admit_snapshot(&profile, &bad),
        Err("inventory_snapshot_unsupported")
    );
    bad = snapshot.clone();
    bad.principal = "another-principal".into();
    assert_eq!(
        admit_snapshot(&profile, &bad),
        Err("inventory_principal_denied")
    );
    bad = snapshot.clone();
    bad.sources.first_mut().ok_or("missing source")?.producer = "impersonator".into();
    assert_eq!(
        admit_snapshot(&profile, &bad),
        Err("inventory_provenance_mismatch")
    );
    bad = snapshot.clone();
    let duplicate = bad.sources.first().ok_or("missing source")?.clone();
    bad.sources.push(duplicate);
    assert!(admit_snapshot(&profile, &bad).is_err());
    bad = snapshot.clone();
    bad.models
        .first_mut()
        .ok_or("missing model")?
        .endpoint_alias = "https://secret.example/token".into();
    assert_eq!(
        admit_snapshot(&profile, &bad),
        Err("inventory_public_identifier_invalid")
    );
    bad = snapshot.clone();
    bad.workers.first_mut().ok_or("missing worker")?.workspace = Some("/private/workspace".into());
    assert_eq!(
        admit_snapshot(&profile, &bad),
        Err("inventory_public_identifier_invalid")
    );
    bad = snapshot.clone();
    bad.agents
        .first_mut()
        .ok_or("missing agent")?
        .qualifications
        .first_mut()
        .ok_or("missing qualification")?
        .accepted_outcomes = Some(11);
    assert_eq!(
        admit_snapshot(&profile, &bad),
        Err("inventory_outcome_denominator_invalid")
    );
    let mut bounded = profile.clone();
    bounded.max_items = 1;
    assert_eq!(
        admit_snapshot(&bounded, &snapshot),
        Err("inventory_item_bound")
    );
    let mut value = serde_json::to_value(&snapshot).map_err(|error| format!("{error:?}"))?;
    value
        .get_mut("models")
        .ok_or("missing models JSON")?
        .get_mut(0)
        .ok_or("missing model JSON")?
        .as_object_mut()
        .ok_or("model JSON is not an object")?
        .insert("credentials".into(), serde_json::json!("secret"));
    assert!(serde_json::from_value::<InventorySnapshot>(value).is_err());
    Ok(())
}

#[test]
fn deterministic_bounded_redacted_same_projection_and_synthetic_disconnection() -> TestResult {
    let (mut profile, mut snapshot, needs) = cohort()?;
    profile.max_candidates = 1;
    let first = project_inventory(&profile, &snapshot, Some(&needs), 2000)?;
    let explanation = first
        .explanation
        .as_ref()
        .ok_or("missing readiness explanation")?;
    assert!(explanation.truncated);
    assert_eq!(explanation.combinations_considered, 2);
    snapshot.workers.reverse();
    snapshot.sources.reverse();
    assert_eq!(
        first,
        project_inventory(&profile, &snapshot, Some(&needs), 2000)?
    );
    profile.include_evidence = false;
    let redacted = project_inventory(&profile, &snapshot, Some(&needs), 2000)?;
    assert_ne!(first.identity, redacted.identity);
    assert!(
        redacted
            .snapshot
            .workers
            .iter()
            .all(|w| w.workspace.is_none() && w.session.is_none())
    );
    assert_eq!(
        redacted
            .snapshot
            .agents
            .first()
            .ok_or("missing redacted agent")?
            .qualifications
            .first()
            .ok_or("missing redacted qualification")?
            .evidence,
        "redacted"
    );
    snapshot.synthetic = true;
    let synthetic = project_inventory(&profile, &snapshot, Some(&needs), 2000)?;
    let explanation = synthetic
        .explanation
        .as_ref()
        .ok_or("missing synthetic readiness explanation")?;
    assert!(has(explanation, "synthetic_disconnected"));
    assert!(explanation.candidates.iter().all(|c| !c.observed_eligible));
    assert!(project_inventory(&profile, &snapshot, None, 2000)?.work_requirements_unknown);
    Ok(())
}

#[test]
fn attachment_is_exact_current_work_revision_policy_and_identity() -> TestResult {
    let (_, _, mut needs) = cohort()?;
    let work = crate::types::ContentId::from_blake3_hex("a".repeat(64))
        .map_err(|error| format!("{error:?}"))?;
    let policy = crate::types::ContentId::from_blake3_hex("b".repeat(64))
        .map_err(|error| format!("{error:?}"))?;
    needs.work_version = work.as_str().into();
    needs.policy_ref = policy.as_str().into();
    let mut attachment = WorkNeedsAttachment::new(work.clone(), 3, policy.clone(), needs)?;
    assert!(attachment.validate_for(&work, 3, &policy).is_ok());
    assert!(attachment.identity().is_ok());
    assert_eq!(
        attachment.validate_for(&work, 4, &policy),
        Err("inventory_requirements_binding_mismatch")
    );
    let other = crate::types::ContentId::from_blake3_hex("c".repeat(64))
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        attachment.validate_for(&other, 3, &policy),
        Err("inventory_requirements_binding_mismatch")
    );
    assert_eq!(
        attachment.validate_for(&work, 3, &other),
        Err("inventory_requirements_binding_mismatch")
    );
    attachment.needs.context_tokens += 1;
    assert_eq!(
        attachment.validate_for(&work, 3, &policy),
        Err("inventory_requirements_identity_mismatch")
    );
    Ok(())
}

#[test]
fn typed_fair_admission_reasons_keep_work_policy_and_freshness_exact() -> TestResult {
    use crate::fair_admission::{AdmissionExplanation, AdmissionPhase, AdmissionReason};
    let (profile, mut snapshot, mut needs) = cohort()?;
    let work = crate::types::ContentId::from_blake3_hex("a".repeat(64))
        .map_err(|error| format!("{error:?}"))?;
    let policy = crate::types::ContentId::from_blake3_hex("b".repeat(64))
        .map_err(|error| format!("{error:?}"))?;
    needs.work_version = work.as_str().into();
    needs.policy_ref = policy.as_str().into();
    let policy_fact = snapshot.policies.first_mut().ok_or("missing policy")?;
    policy_fact.work_version = needs.work_version.clone();
    policy_fact.policy_ref = needs.policy_ref.clone();
    let resource =
        crate::types::Alias::new("cpu_millicores").map_err(|error| format!("{error:?}"))?;
    let mut held = BTreeMap::new();
    held.insert(resource.clone(), 1000);
    policy_fact.admission_explanation = Some(AdmissionExplanation {
        work_version: work,
        campaign: crate::types::Alias::new("pilot").map_err(|error| format!("{error:?}"))?,
        policy_revision: policy,
        phase: AdmissionPhase::PostClaim,
        reason: AdmissionReason::QueueFull,
        position: Some(4),
        eligible_at_ms: Some(3000),
        held,
        measured: BTreeMap::new(),
        observation_at_ms: Some(1000),
    });
    let result = explain_readiness(&profile, &snapshot, &needs, 2000)?;
    assert!(
        result
            .blockers
            .iter()
            .any(|b| b.code == "fair_admission_blocked" && b.constraint == "queue_full")
    );
    assert!(has(&result, "bounded_wait"));
    assert!(!result.dispatch_authorized);
    let projection = project_inventory(&profile, &snapshot, Some(&needs), 2000)?;
    let admission = projection
        .snapshot
        .policies
        .first()
        .ok_or("missing projected policy")?
        .admission_explanation
        .as_ref()
        .ok_or("missing projected admission explanation")?;
    assert_eq!(admission.held.get(&resource), Some(&1000));
    assert!(admission.measured.is_empty());
    snapshot
        .policies
        .first_mut()
        .ok_or("missing policy")?
        .admission_explanation
        .as_mut()
        .ok_or("missing admission explanation")?
        .observation_at_ms = None;
    assert!(has(
        &explain_readiness(&profile, &snapshot, &needs, 2000)?,
        "admission_observation_unknown"
    ));
    snapshot
        .policies
        .first_mut()
        .ok_or("missing policy")?
        .admission_explanation
        .as_mut()
        .ok_or("missing admission explanation")?
        .policy_revision = crate::types::ContentId::from_blake3_hex("c".repeat(64))
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        admit_snapshot(&profile, &snapshot),
        Err("inventory_admission_binding_mismatch")
    );
    Ok(())
}

#[test]
fn invalid_pool_allocation_and_inexact_json_numbers_are_rejected() -> TestResult {
    let (profile, mut snapshot, _) = cohort()?;
    let pool = snapshot
        .pools
        .get_mut(2)
        .ok_or("missing serving memory pool")?;
    let duplicate = pool
        .allocations
        .first()
        .ok_or("missing allocation")?
        .clone();
    pool.allocations.push(duplicate);
    assert_eq!(
        admit_snapshot(&profile, &snapshot),
        Err("inventory_allocation_duplicate")
    );
    let pool = snapshot
        .pools
        .get_mut(2)
        .ok_or("missing serving memory pool")?;
    pool.allocations.pop();
    pool.allocatable = Some(16000);
    assert_eq!(
        admit_snapshot(&profile, &snapshot),
        Err("inventory_accounting_invalid")
    );
    snapshot
        .pools
        .get_mut(2)
        .ok_or("missing serving memory pool")?
        .allocatable = Some(10000);
    snapshot
        .sources
        .first_mut()
        .ok_or("missing source")?
        .generation = MAX_EXACT_NUMBER + 1;
    assert_eq!(
        admit_snapshot(&profile, &snapshot),
        Err("inventory_number_not_exact")
    );
    Ok(())
}

#[test]
fn warm_service_and_free_capacity_do_not_override_routing_permission() -> TestResult {
    let (profile, mut snapshot, needs) = cohort()?;
    snapshot
        .models
        .first_mut()
        .ok_or("missing model")?
        .routing_permission = Permission::Denied;
    assert!(has(
        &explain_readiness(&profile, &snapshot, &needs, 2000)?,
        "model_policy_denied_or_unknown"
    ));
    let model = snapshot.models.first_mut().ok_or("missing model")?;
    model.routing_permission = Permission::Allowed;
    model.routing_policy_ref = Some("different-policy".into());
    assert!(has(
        &explain_readiness(&profile, &snapshot, &needs, 2000)?,
        "model_policy_denied_or_unknown"
    ));
    Ok(())
}
