use super::*;
use crate::{OperatorRequest, PROFILE_SCHEMA, prepare_request};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signer, SigningKey};
use kiln_core::build::{
    BuildCacheKind, BuildEffectPlan, BuildEvaluationObservation, BuildProductObservation,
};
use kiln_core::discovery::*;
use kiln_core::types::*;

const NOW: u64 = 1_800_000_000;
const WINDOW: u64 = 60;
const TEST_BYTES: usize = 1024 * 1024;
const TEST_RECORDS: usize = 16;
fn core<T>(value: CoreResult<T>) -> Result<T> {
    value.map_err(|error| format!("{error:?}"))
}
fn content(text: &str) -> Result<ContentId> {
    crate::identity(&text)
}
fn external(kind: &str, value: &str) -> Result<ExternalRef> {
    core(ExternalRef::new("test", kind, value))
}
fn fixture() -> Result<(PilotProfile, SignedRequest, WorkerResult)> {
    let key = SigningKey::from_bytes(&[7; crate::KEY_BYTES]);
    let discovery_plan = DiscoveryEffectPlan {
        effect_id: core(EffectId::new("discovery"))?,
        evaluation_id: core(EvaluationId::new("evaluation"))?,
        project_id: core(ProjectId::new("project"))?,
        pipeline_revision_id: core(PipelineRevisionId::new("pipeline"))?,
        discovery_policy_id: core(PolicyId::new("policy"))?,
        provider_profile: core(ProviderProfileId::new("discovery"))?,
        evaluator_cohort: None,
        build_provider_profile: core(ProviderProfileId::new("build"))?,
        idempotency_key: content("discovery-key")?,
        source_identity: content("source")?,
        source: external("source", "/nix/store/test-source")?,
        source_mode: DiscoverySourceMode::Flake,
        root_selector: core(Alias::new("jobs"))?,
        system: core(Alias::new("x86_64-linux"))?,
        options: DiscoveryOptions {
            workers: 1,
            max_memory_mebibytes: 128,
            include_metadata: false,
            include_input_derivations: true,
            check_cache_status: false,
            partial_admission: false,
        },
        bounds: DiscoveryBounds {
            max_records: TEST_RECORDS,
            max_jobs: TEST_RECORDS,
            max_job_errors: TEST_RECORDS,
            max_diagnostic_bytes: 512,
            max_outputs_per_job: TEST_RECORDS,
            max_dependencies_per_job: TEST_RECORDS,
            max_tasks: TEST_RECORDS,
        },
    };
    let build_plan = BuildEffectPlan {
        effect_id: core(EffectId::new("build"))?,
        attempt_id: core(AttemptId::new("attempt"))?,
        idempotency_key: content("build-key")?,
        candidate_id: core(CandidateId::new("candidate"))?,
        pipeline_revision: discovery_plan.pipeline_revision_id.clone(),
        plan_identity: content("plan")?,
        policy_identity: content("policy")?,
        provider_profile: discovery_plan.build_provider_profile.clone(),
        platform: discovery_plan.system.clone(),
        required_products: vec![core(Alias::new("out"))?],
    };
    let profile = PilotProfile {
        schema: PROFILE_SCHEMA.into(),
        allowed_operator_keys: vec![URL_SAFE_NO_PAD.encode(key.verifying_key().to_bytes())],
        cohort_identity: content("cohort")?,
        source_uri: "rad:test@revision".into(),
        source_identity: discovery_plan.source_identity.clone(),
        job_identity: content("job")?,
        selected_task: core(Alias::new("app"))?,
        max_request_age_seconds: WINDOW,
        max_request_bytes: TEST_BYTES,
        max_journal_bytes: TEST_BYTES,
        max_journal_records: TEST_RECORDS,
        socket_path: "/tmp/kiln-test-unconnected.sock".into(),
        transport_timeout_seconds: 1,
        maximum_frame_bytes: TEST_BYTES,
        runtime_profile: kiln_adapter_aspen::profile_fixture(),
        discovery_plan,
        build_plan,
    };
    let trigger = kiln_provider_api::TriggerRequest {
        repository: external("repository", "rad:test")?,
        actor: external("actor", "operator")?,
        before: None,
        after: external("revision", "revision")?,
        branch: None,
        patch: None,
        event: kiln_provider_api::TriggerEventKind::Push,
    };
    let request = prepare_request(&profile, trigger, NOW, NOW + WINDOW)?;
    let signed = sign_fixed(request, &key)?;
    let job = DiscoveredJob {
        attribute: profile.selected_task.clone(),
        attribute_path: vec![profile.selected_task.clone()],
        name: profile.selected_task.clone(),
        system: profile.discovery_plan.system.clone(),
        derivation: external("derivation", "/nix/store/application.drv")?,
        outputs: vec![DiscoveredOutput {
            name: core(Alias::new("out"))?,
            path: external("store-path", "/nix/store/application")?,
        }],
        input_derivations: Vec::new(),
        metadata_identity: None,
        cache: CacheAdvisory::NotBuilt,
    };
    let tasks = core(map_task_proposals(&profile.discovery_plan, &[job.clone()]))?;
    let completion = DiscoveryCompletionFacts {
        stream_complete: true,
        stream_valid: true,
        stderr_within_bound: true,
        gc_roots_ready: true,
        fatal_job_error: false,
        process: ProcessCompletionFact::Exited { status: 0 },
    };
    let roots = GcRootObservation {
        directory: Some(external("gc-root-directory", "/tmp/test-roots")?),
        derivations: vec![job.derivation.clone()],
        state: GcRootState::Retained,
    };
    let observation_identity = compute_discovery_observation_identity(
        &profile.discovery_plan,
        DiscoveryTerminal::Complete,
        &[job.clone()],
        &[],
        &tasks,
        completion,
        &roots,
    );
    let discovery = DiscoveryObservation {
        effect_id: profile.discovery_plan.effect_id.clone(),
        evaluation_id: profile.discovery_plan.evaluation_id.clone(),
        project_id: profile.discovery_plan.project_id.clone(),
        pipeline_revision_id: profile.discovery_plan.pipeline_revision_id.clone(),
        discovery_policy_id: profile.discovery_plan.discovery_policy_id.clone(),
        provider_profile: profile.discovery_plan.provider_profile.clone(),
        idempotency_key: profile.discovery_plan.idempotency_key.clone(),
        source_identity: profile.source_identity.clone(),
        observation_identity,
        terminal: DiscoveryTerminal::Complete,
        jobs: vec![job],
        job_errors: Vec::new(),
        tasks,
        completion,
        gc_roots: roots,
    };
    let plan = &profile.build_plan;
    let build = BuildEvaluationObservation {
        effect_id: plan.effect_id.clone(),
        attempt_id: plan.attempt_id.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        candidate_id: plan.candidate_id.clone(),
        pipeline_revision: plan.pipeline_revision.clone(),
        plan_identity: plan.plan_identity.clone(),
        policy_identity: plan.policy_identity.clone(),
        provider_profile: plan.provider_profile.clone(),
        platform: plan.platform.clone(),
        outcome: BuildEvaluationOutcome::Success,
        products: vec![BuildProductObservation {
            name: core(Alias::new("out"))?,
            artifact_identity: content("artifact")?,
            store_identity: content("store")?,
        }],
        builder_identity: content("builder")?,
        worker_identity: content("worker")?,
        store_identity: content("store")?,
        cache_kind: BuildCacheKind::None,
        cache_source_identity: None,
        log_identities: Vec::new(),
        metrics: Vec::new(),
        receipt_identity: Some(content("receipt")?),
    };
    let worker = WorkerResult {
        schema: WORKER_RESULT_SCHEMA.into(),
        job_identity: profile.job_identity.clone(),
        source_uri: profile.source_uri.clone(),
        discovery_plan: profile.discovery_plan.clone(),
        discovery_observation: discovery,
        build_plan: Some(profile.build_plan.clone()),
        build_observation: Some(build),
    };
    Ok((profile, signed, worker))
}
fn sign_fixed(request: OperatorRequest, key: &SigningKey) -> Result<SignedRequest> {
    let mut bytes = b"kiln.onixos-pilot.operator-request.v1\0".to_vec();
    serde_json::to_writer(&mut bytes, &request).map_err(|error| error.to_string())?;
    Ok(SignedRequest {
        public_key: URL_SAFE_NO_PAD.encode(key.verifying_key().to_bytes()),
        signature: URL_SAFE_NO_PAD.encode(key.sign(&bytes).to_bytes()),
        request,
    })
}
fn ack(journal: &PilotJournal, decision: AspenServiceDecision) -> Result<ServiceTransportOutcome> {
    Ok(ServiceTransportOutcome::Acknowledged(AspenServiceAck {
        schema: SERVICE_ACK_SCHEMA.into(),
        operation_id: journal.ingress.operation_id.clone(),
        generation: journal.ingress.generation,
        trigger_identity: journal.ingress.trigger_identity.clone(),
        run_id: Some(journal.request.request.run_id.clone()),
        decision,
        receipt_ref: external("receipt", "native-receipt")?,
    }))
}

// r[verify kiln.onixos_pilot.trigger]
#[test]
fn authentication_binds_bytes_key_cohort_plan_and_freshness() -> Result<()> {
    let (profile, signed, _) = fixture()?;
    authenticate(&profile, &signed, NOW)?;
    assert!(authenticate(&profile, &signed, NOW - 1).is_err());
    assert!(authenticate(&profile, &signed, NOW + WINDOW + 1).is_err());
    let mut changed = signed.clone();
    changed.request.job_identity = content("other-job")?;
    assert!(authenticate(&profile, &changed, NOW).is_err());
    let wrong = sign_fixed(
        signed.request.clone(),
        &SigningKey::from_bytes(&[8; crate::KEY_BYTES]),
    )?;
    assert!(authenticate(&profile, &wrong, NOW).is_err());
    let mut changed_profile = profile.clone();
    changed_profile.build_plan.plan_identity = content("other-plan")?;
    assert!(authenticate(&changed_profile, &signed, NOW).is_err());
    let mut wrong_cohort = signed.request.clone();
    wrong_cohort.cohort_identity = content("other-cohort")?;
    let wrong_cohort = sign_fixed(
        wrong_cohort,
        &SigningKey::from_bytes(&[7; crate::KEY_BYTES]),
    )?;
    assert!(authenticate(&profile, &wrong_cohort, NOW).is_err());
    let mut stale = signed.request.clone();
    stale.expires_at_unix = NOW + WINDOW + 1;
    let stale = sign_fixed(stale, &SigningKey::from_bytes(&[7; crate::KEY_BYTES]))?;
    assert!(authenticate(&profile, &stale, NOW).is_err());
    Ok(())
}

// r[verify kiln.onixos_pilot.trigger] r[verify kiln.onixos_pilot.journal]
#[test]
fn invalid_transport_does_not_reserve_an_undispatchable_intent() -> Result<()> {
    let (mut profile, signed, _) = fixture()?;
    profile.socket_path = "relative-aspen.sock".into();
    let now = now_unix()?;
    let request = prepare_request(&profile, signed.request.trigger, now, now + WINDOW)?;
    let signed = sign_fixed(request, &SigningKey::from_bytes(&[7; crate::KEY_BYTES]))?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory.path().join("journal.json");
    assert!(start(&profile, signed, &path).is_err());
    assert!(!path.exists());
    Ok(())
}

// r[verify kiln.onixos_pilot.journal]
// r[verify kiln.onixos_pilot.recovery]
#[test]
fn durable_intent_replays_without_evidence_and_rejects_record_substitution() -> Result<()> {
    let (profile, signed, _) = fixture()?;
    let mut journal = PilotJournal::fresh(&profile, signed)?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory.path().join("journal.json");
    {
        let lock = JournalLock::acquire(&path)?;
        lock.store(&journal, profile.max_journal_bytes)?;
    }
    let state = replay(&profile, &path)?;
    assert_eq!(
        state
            .runs
            .get(&journal.request.request.run_id)
            .map(|run| run.outcome),
        Some(Outcome::Pending)
    );
    journal
        .records
        .first_mut()
        .ok_or("missing start")?
        .expected_revision = 1;
    journal.seal()?;
    assert!(journal.validate(&profile).is_err());
    Ok(())
}

// r[verify kiln.onixos_pilot.journal]
// r[verify kiln.onixos_pilot.failure]
#[test]
fn terminal_requires_exact_native_and_real_admitted_worker_facts() -> Result<()> {
    let (profile, signed, worker) = fixture()?;
    let mut journal = PilotJournal::fresh(&profile, signed)?;
    assert!(completion(&profile, &journal)?.is_none());
    journal.worker_result = Some(worker.clone());
    assert!(completion(&profile, &journal)?.is_none());
    let native = ack(&journal, AspenServiceDecision::Completed)?;
    observe(&profile, &mut journal, native)?;
    let state = journal.validate(&profile)?;
    assert_eq!(
        state
            .runs
            .get(&journal.request.request.run_id)
            .map(|run| run.outcome),
        Some(Outcome::Success)
    );
    let attempt = state
        .attempts
        .get(&profile.build_plan.attempt_id)
        .ok_or("missing attempt")?;
    assert_eq!(
        attempt
            .products
            .first()
            .map(|product| product.identity.clone()),
        Some(content("artifact")?)
    );
    let mut substitution = worker.clone();
    substitution.source_uri = "rad:other".into();
    assert!(admitted_worker(&profile, &substitution).is_err());
    let mut missing_product = worker.clone();
    missing_product
        .build_observation
        .as_mut()
        .ok_or("missing build")?
        .products
        .clear();
    assert!(admitted_worker(&profile, &missing_product).is_err());
    let mut stale = worker.clone();
    stale.discovery_observation.observation_identity = content("wrong-observation")?;
    assert!(admitted_worker(&profile, &stale).is_err());
    let mut conflict = PilotJournal::fresh(&profile, journal.request.clone())?;
    conflict.worker_result = Some(worker);
    let failed = ack(&conflict, AspenServiceDecision::Failed)?;
    assert!(observe(&profile, &mut conflict, failed).is_err());
    let mut native = ack(&journal, AspenServiceDecision::Completed)?;
    if let ServiceTransportOutcome::Acknowledged(ack) = &mut native {
        ack.operation_id = external("operation", "other")?;
    }
    assert!(validate_native(&journal.ingress, &journal.request, &native).is_err());
    Ok(())
}

#[test]
fn negative_builds_preserve_terminal_class_and_unknown_never_completes() -> Result<()> {
    let (profile, signed, worker) = fixture()?;
    for (build_outcome, native_phase, expected) in [
        (
            BuildEvaluationOutcome::Failed,
            AspenServiceDecision::Failed,
            Outcome::DomainFailure,
        ),
        (
            BuildEvaluationOutcome::ProviderError,
            AspenServiceDecision::Failed,
            Outcome::ProviderError,
        ),
        (
            BuildEvaluationOutcome::TimedOut,
            AspenServiceDecision::Failed,
            Outcome::TimedOut,
        ),
        (
            BuildEvaluationOutcome::Cancelled,
            AspenServiceDecision::Failed,
            Outcome::Cancelled,
        ),
    ] {
        let mut result = worker.clone();
        let build = result.build_observation.as_mut().ok_or("missing build")?;
        build.outcome = build_outcome;
        build.products.clear();
        let mut journal = PilotJournal::fresh(&profile, signed.clone())?;
        journal.worker_result = Some(result);
        let native = ack(&journal, native_phase)?;
        observe(&profile, &mut journal, native)?;
        let state = journal.validate(&profile)?;
        assert_eq!(
            state
                .runs
                .get(&signed.request.run_id)
                .map(|run| run.outcome),
            Some(expected)
        );
        let again = ack(&journal, native_phase)?;
        assert!(observe(&profile, &mut journal, again).is_err());
    }
    let mut result = worker;
    let build = result.build_observation.as_mut().ok_or("missing build")?;
    build.outcome = BuildEvaluationOutcome::Unknown;
    build.receipt_identity = None;
    build.products.clear();
    let mut journal = PilotJournal::fresh(&profile, signed)?;
    journal.worker_result = Some(result);
    let native = ack(&journal, AspenServiceDecision::Unknown)?;
    observe(&profile, &mut journal, native)?;
    assert!(completion(&profile, &journal)?.is_none());
    assert_eq!(
        journal
            .validate(&profile)?
            .runs
            .get(&journal.request.request.run_id)
            .map(|run| run.outcome),
        Some(Outcome::Pending)
    );
    Ok(())
}

#[test]
fn partial_and_symlink_journals_fail_closed() -> Result<()> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory.path().join("journal.json");
    crate::publish_new(&path.with_extension("pending"), &"partial")?;
    assert!(JournalLock::acquire(&path).is_err());
    let link = directory.path().join("link.json");
    std::os::unix::fs::symlink(path.with_extension("pending"), &link)
        .map_err(|error| error.to_string())?;
    assert!(load_json::<serde_json::Value>(&link, TEST_BYTES).is_err());
    Ok(())
}
