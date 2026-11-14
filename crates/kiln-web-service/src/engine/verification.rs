//! Trusted in-process verification -> the existing canonical Engine journal.
//! No posted success JSON, second CI store, executor dispatch or merge authority.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;

use kiln_adapter_git::{GitSourceProfile, LocalGitAdapter, resolve_and_acquire};
use kiln_core::change_review::CiCheck;
use kiln_core::domain::{Command, Outcome, State};
use kiln_core::implementation::{
    CandidateHandoff, CandidateVerification, ImplementationAttempt, ImplementationWork,
    WorkSourceKind, admit_candidate_verification,
};
use kiln_core::source::SourceNativeFacts;
use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId, RunId};
use kiln_core::workflow::{
    WorkflowNodeObservation, WorkflowNodeState, WorkflowObservation, WorkflowSourceContext,
    WorkflowState, admit_workflow_observation,
};
use kiln_implementer::{
    Implementer,
    verification::{VerificationProfile, VerificationRun, VerifierAuthority, verify_candidate},
};

use super::model::{ArtifactView, ReportRecord, RunRecord, workflow_plan};
use super::{Engine, host_time_ms, identity};

/// Recorded observations must never enter generic Lattice polling/retry/control.
pub(super) const VERIFIER_PROVIDER: &str = "kiln-independent-verifier-v1";
const NATIVE_CHECK: &str = "cairn-authoritative-completion";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationProvenance {
    pub service_id: String,
    pub repository_alias: String,
    pub acquisition_root: std::path::PathBuf,
    pub handoff: CandidateHandoff,
    pub subject_hash: String,
    pub observation_identity: ContentId,
    pub independent_receipt: Option<ContentId>,
    pub binding: ContentId,
}

impl Engine {
    /// Reviewed project nodes must bind these exact frozen operation identities.
    pub fn implementation_verification_operation(
        profile: &VerificationProfile,
        check: &str,
    ) -> Result<ContentId, String> {
        identity(&(
            "kiln.independent-verification.operation.v1",
            profile.identity()?,
            check,
        ))
    }

    /// Pure configuration admission, not check execution or candidate success.
    /// Used by the dedicated fixture constructor and the same all-check graph
    /// relation that canonical runtime ingestion enforces.
    pub fn validate_implementation_verification_project(
        profile: &VerificationProfile,
        project: &crate::profile::ProjectProfile,
    ) -> Result<(), String> {
        let mut checks: Vec<_> = profile
            .checks
            .iter()
            .map(|check| CiCheck {
                name: check.id.clone(),
                kind: check.kind.clone(),
                outcome: "unknown".into(),
                artifact_digest: None,
            })
            .collect();
        if profile.cairn.is_some() {
            checks.push(CiCheck {
                name: NATIVE_CHECK.into(),
                kind: "cairn-agent-completion-v2".into(),
                outcome: "unknown".into(),
                artifact_digest: None,
            });
        }
        let pipeline = identity(&project.workflow)?;
        let plan = workflow_plan(project, &profile.identity()?, &pipeline, None)?;
        validate_reviewed_nodes(profile, &checks, &plan)
    }

    /// Execute the actual frozen profile in the controller-owned verifier and
    /// journal its real observation before admitting the current implementation.
    /// HTTP/CLI callers select only the current attempt; profile/authority must
    /// come from trusted service configuration, never from a worker request.
    pub fn verify_implementation_candidate(
        &mut self,
        ledger: &mut Implementer,
        attempt_id: &ContentId,
        fence: u64,
        profile: &VerificationProfile,
        authority: &VerifierAuthority,
    ) -> Result<CandidateVerification, String> {
        if let Some(retained) = self.retained_implementation_verification(
            ledger,
            attempt_id,
            fence,
            &profile.identity()?,
            authority,
        )? {
            return Ok(retained);
        }
        let attempt = ledger.authorize_current(attempt_id, fence, host_time_ms()?)?;
        let work = ledger.active_work(attempt_id, fence, host_time_ms()?)?;
        let handoff = ledger
            .candidate_handoff(attempt_id)?
            .ok_or("candidate handoff is not retained")?;
        ledger.begin_verification(attempt_id, fence, host_time_ms()?)?;
        let executed = verify_candidate(&work, &attempt, &handoff, profile, authority)?;
        self.record_implementation_verification(ledger, &executed, attempt_id, fence)
    }

    /// Ingest only the non-deserializable product of real trusted execution.
    /// Retained native SourceResolution remains untouched. Only the returned
    /// implementation envelope projects its explicitly enrolled repository alias.
    pub fn record_implementation_verification(
        &mut self,
        ledger: &mut Implementer,
        executed: &VerificationRun,
        attempt_id: &ContentId,
        fence: u64,
    ) -> Result<CandidateVerification, String> {
        if self.storage_failed {
            return Err("durable canonical CI state is unavailable".into());
        }
        let attempt = ledger
            .attempt(attempt_id)?
            .ok_or("implementation attempt missing")?;
        let work = if attempt.verification.is_some() {
            current_work(ledger, &attempt)?
        } else {
            ledger.active_work(attempt_id, fence, host_time_ms()?)?
        };
        let handoff = ledger
            .candidate_handoff(attempt_id)?
            .ok_or("candidate handoff missing")?;
        executed.assert_binding(&work, &attempt, &handoff)?;
        if fence != attempt.fence || executed.authority().repository != work.repository {
            return Err("verification source enrollment or fence differs".into());
        }
        let project = self
            .profile
            .projects
            .iter()
            .find(|project| project.id == work.project)
            .cloned()
            .ok_or("implementation project not configured")?;
        if !self.state.enrolled.contains(&project.id)
            || fs::canonicalize(&project.repository_path).map_err(text)?
                != executed.authority().repository_root
        {
            return Err("verifier source is not the enrolled canonical project acquisition".into());
        }
        let run_identity = identity(&(
            "kiln.implementation.canonical-verification.v1",
            &self.profile.service_id,
            &project.id,
            &handoff,
            executed.profile().identity()?,
        ))?;
        let run_id = format!("web-verification-{}", run_identity.as_str());
        if let Some(retained) = self.retained_implementation_verification(
            ledger,
            attempt_id,
            fence,
            &executed.profile().identity()?,
            executed.authority(),
        )? {
            if retained.run.run_id != run_id {
                return Err("attempt already admitted another canonical run".into());
            }
            return Ok(retained);
        }
        ledger.authorize_current(attempt_id, fence, host_time_ms()?)?;
        let cairn = work
            .aliases
            .iter()
            .any(|alias| alias.kind == WorkSourceKind::CairnTask);
        let admission = if cairn {
            kiln_adapter_cairn::acceptance::verify_cairn_completion(
                executed, &work, &attempt, &handoff,
            )
        } else {
            executed.candidate_verification(&work, &attempt, &handoff)
        };
        let mut checks = admission
            .as_ref()
            .map(|receipt| receipt.run.checks.clone())
            .unwrap_or_else(|_| executed.run().checks.clone());
        let mut native_unmet = None;
        if cairn && !checks.iter().any(|check| check.name == NATIVE_CHECK) {
            // A native evaluator refusal remains unmet. If it emitted no native
            // report, retain the actual adapter refusal, never a passing receipt.
            let bytes = match read_material(&executed.evidence_directory().join("cairn-completion-material.json"),
                self.profile.bounds.max_artifact_bytes) {
                Ok(bytes) => bytes,
                Err(_) => serde_json::to_vec(&serde_json::json!({"completion_unmet":admission.as_ref().err(),
                    "checks_executed":executed.checks_executed(), "subject":executed.subject_hash()})).map_err(text)?,
            };
            let hash = blake3::hash(&bytes).to_hex().to_string();
            native_unmet = Some(bytes);
            checks.push(CiCheck {
                name: NATIVE_CHECK.into(),
                kind: "cairn-agent-completion-v2".into(),
                outcome: "fail".into(),
                artifact_digest: Some(hash),
            });
        }
        let source_provider =
            ProviderProfileId::new(format!("web-git-{}", project.id)).map_err(text)?;
        let adapter = LocalGitAdapter::new(
            project.repository_path.clone(),
            handoff.candidate.native_id.as_str().into(),
            GitSourceProfile::pinned_local(source_provider.clone(), project.source_limits.clone()),
        )
        .map_err(|_| "canonical verifier source adapter unavailable")?;
        let source = resolve_and_acquire(&adapter, source_provider)
            .map_err(|_| "canonical verifier source acquisition unavailable")?;
        if source.exact_revision != handoff.candidate
            || !matches!(&source.native_facts, SourceNativeFacts::Git(facts) if facts.native_tree == handoff.tree)
        {
            return Err(
                "canonical source acquisition differs from independently verified commit/tree"
                    .into(),
            );
        }
        let pipeline = identity(&project.workflow)?;
        let context = WorkflowSourceContext {
            provider: kiln_core::types::Alias::new("git").map_err(text)?,
            repository: source.repository.clone(),
            revision: ExternalRef::new("git", "revision", handoff.candidate.native_id.as_str())
                .map_err(text)?,
        };
        let mut plan = workflow_plan(&project, &run_identity, &pipeline, Some(context))?;
        // Use exactly the reviewed project node set. Acceptance-only checks can
        // never masquerade as an unrelated project's complete configured CI.
        validate_reviewed_nodes(executed.profile(), &checks, &plan)?;
        plan.provider_profile = ProviderProfileId::new(VERIFIER_PROVIDER).map_err(text)?;
        kiln_core::workflow::validate_workflow_plan(&plan).map_err(text)?;
        let execution = ExternalRef::new("kiln", "independent-verification", run_identity.as_str())
            .map_err(text)?;
        let nodes = plan
            .nodes
            .iter()
            .map(|node| {
                let check = checks
                    .iter()
                    .find(|check| check.name == node.id.as_str())
                    .ok_or("canonical node check missing")?;
                let state = match check.outcome.as_str() {
                    "pass" => WorkflowNodeState::Completed,
                    "fail" => WorkflowNodeState::Failed,
                    "cancelled" => WorkflowNodeState::Cancelled,
                    _ => WorkflowNodeState::Unknown,
                };
                Ok(WorkflowNodeObservation {
                    node_id: node.id.clone(),
                    state,
                    terminal: state.is_terminal(),
                    successful: state.is_successful(),
                    adapter: ExternalRef::new(
                        "kiln",
                        "verifier",
                        executed.profile().identity()?.as_str(),
                    )
                    .map_err(text)?,
                    receipt: if state.is_terminal() {
                        Some(
                            ExternalRef::new(
                                "kiln",
                                "check-evidence",
                                check
                                    .artifact_digest
                                    .as_deref()
                                    .ok_or("terminal check has no retained artifact")?,
                            )
                            .map_err(text)?,
                        )
                    } else {
                        None
                    },
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let unknown = nodes
            .iter()
            .any(|node| node.state == WorkflowNodeState::Unknown);
        let successful = admission.is_ok();
        let outcome = if unknown {
            Outcome::Unknown
        } else if successful {
            Outcome::Success
        } else {
            Outcome::DomainFailure
        };
        let observation = WorkflowObservation {
            effect_id: plan.effect_id.clone(),
            attempt_id: plan.attempt_id.clone(),
            candidate_id: plan.candidate_id.clone(),
            pipeline_revision_id: plan.pipeline_revision_id.clone(),
            workflow_policy_id: plan.workflow_policy_id.clone(),
            provider_profile: plan.provider_profile.clone(),
            idempotency_key: plan.idempotency_key.clone(),
            workflow: ExternalRef::new(
                "kiln",
                "verification-profile",
                executed.profile().identity()?.as_str(),
            )
            .map_err(text)?,
            execution: execution.clone(),
            sequence: 1,
            state: if unknown {
                WorkflowState::Unknown
            } else if successful {
                WorkflowState::Completed
            } else {
                WorkflowState::Failed
            },
            terminal: !unknown,
            successful,
            outcome,
            nodes,
            logs: vec![],
            receipt: if unknown {
                None
            } else {
                Some(
                    ExternalRef::new("kiln", "verification-observation", run_identity.as_str())
                        .map_err(text)?,
                )
            },
        };
        admit_workflow_observation(&plan, &observation).map_err(text)?;
        let now = host_time_ms()?;
        let independent_receipt = admission
            .as_ref()
            .ok()
            .map(|receipt| {
                admit_candidate_verification(&work, &attempt, &handoff, receipt).map_err(text)
            })
            .transpose()?;
        let observation_identity = identity(&observation)?;
        let binding = provenance_binding(
            &self.profile.service_id,
            &work.repository,
            &executed.authority().repository_root,
            &handoff,
            executed.subject_hash(),
            &observation_identity,
            independent_receipt.as_ref(),
        )?;
        let provenance = VerificationProvenance {
            service_id: self.profile.service_id.clone(),
            repository_alias: work.repository.clone(),
            acquisition_root: executed.authority().repository_root.clone(),
            handoff: handoff.clone(),
            subject_hash: executed.subject_hash().into(),
            observation_identity,
            independent_receipt,
            binding,
        };
        if let Some(existing) = self.state.runs.get(&run_id) {
            if existing.plan != plan || existing.observation.as_ref() != Some(&observation) {
                return Err("canonical verification replay substituted execution evidence".into());
            }
        } else {
            if self.state.runs.len() >= self.profile.bounds.max_runs {
                return Err("canonical run retention exceeded".into());
            }
            let mut record = RunRecord {
                id: run_id.clone(),
                ordinal: self
                    .state
                    .version
                    .checked_add(1)
                    .ok_or("canonical ordinal overflow")?,
                project_id: project.id.clone(),
                pipeline_identity: pipeline,
                source,
                requested_reference: None,
                requested_pull_request: None,
                requested_trigger: None,
                plan: plan.clone(),
                events: vec![],
                event_recorded_at_ms: vec![],
                phase: if unknown { "unknown" } else { "terminal" }.into(),
                observation: Some(observation),
                error_code: admission
                    .as_ref()
                    .err()
                    .map(|_| "implementation_verification_unmet".into()),
                superseded_by: None,
                superseded_revision: None,
                artifacts: vec![],
                logs: vec![],
                created_at_ms: Some(now),
                started_at_ms: None,
                completed_at_ms: (!unknown).then_some(now),
                parent_run_id: None,
                auto_retry: None,
                selected_nodes: vec![],
                node_timing: BTreeMap::new(),
                reports: vec![],
                step_telemetry: vec![],
                dirty_snapshot: None,
                affected: None,
                scope: crate::monorepo::run_scope(&project)?,
                untrusted: None,
                state: State::default(),
                implementation_verification: Some(provenance),
            };
            let mut total_bytes = self
                .state
                .runs
                .values()
                .flat_map(|run| &run.artifacts)
                .try_fold(0_usize, |sum, artifact| sum.checked_add(artifact.bytes))
                .ok_or("artifact total overflow")?;
            let mut run_bytes = 0_usize;
            for check in &checks {
                let bytes = if check.name == NATIVE_CHECK {
                    match native_unmet.take() {
                        Some(bytes) => bytes,
                        None => read_material(
                            &executed
                                .evidence_directory()
                                .join("cairn-completion-material.json"),
                            self.profile.bounds.max_artifact_bytes,
                        )?,
                    }
                } else {
                    executed.check_artifact(&check.name)?
                };
                let digest = blake3::hash(&bytes).to_hex().to_string();
                if check.artifact_digest.as_deref() != Some(digest.as_str()) {
                    return Err("canonical retained check artifact substituted".into());
                }
                run_bytes = run_bytes
                    .checked_add(bytes.len())
                    .ok_or("run artifact byte overflow")?;
                total_bytes = total_bytes
                    .checked_add(bytes.len())
                    .ok_or("total artifact byte overflow")?;
                let bounds = &self.profile.bounds;
                if record.artifacts.len() >= bounds.max_artifacts
                    || bytes.len() > bounds.max_artifact_bytes
                    || run_bytes > bounds.max_run_artifact_bytes
                    || total_bytes > bounds.max_total_artifact_bytes
                {
                    return Err("canonical verification artifact retention exceeded".into());
                }
                let name = format!("{}-verification.json", check.name);
                let artifact_id = identity(&(
                    "kiln.web-artifact.v1",
                    &run_id,
                    &plan.attempt_id,
                    &execution,
                    &check.name,
                    &name,
                    &digest,
                ))?
                .as_str()
                .to_owned();
                if let Err(error) = super::storage::publish_retained(
                    &self.profile.state_directory.join("artifacts"),
                    &artifact_id,
                    &bytes,
                ) {
                    self.storage_failed = true;
                    return Err(error);
                }
                let artifact = ArtifactView {
                    id: artifact_id.clone(),
                    name: name.clone(),
                    node_id: check.name.clone(),
                    attempt_id: plan.attempt_id.clone(),
                    bytes: bytes.len(),
                    blake3: digest,
                    download_path: format!("/api/runs/{run_id}/artifacts/{artifact_id}"),
                };
                record.artifacts.push(artifact.clone());
                record.reports.push(ReportRecord {
                    id: artifact_id,
                    name,
                    node_id: check.name.clone(),
                    attempt_id: plan.attempt_id.clone(),
                    source_revision: handoff.candidate.native_id.as_str().into(),
                    execution: execution.clone(),
                    kind: crate::reports::ReportKind::Provenance,
                    artifact: Some(artifact),
                    report: None,
                    unavailable: None,
                    recorded_at_ms: now,
                });
            }
            record.apply(Command::StartRun {
                run_id: RunId::new(run_id.clone()).map_err(text)?,
                attempt_id: plan.attempt_id.clone(),
                provider_profile: plan.provider_profile.clone(),
                effect_id: plan.effect_id.clone(),
            })?;
            if unknown {
                record.mark_unknown()?;
            } else {
                record.complete(outcome, Some(execution))?;
            }
            self.state.runs.insert(run_id.clone(), record);
            self.persist()?;
        }
        let canonical = self
            .code_review_ci()
            .into_iter()
            .find(|fact| fact.run_id == run_id)
            .ok_or("canonical verification projection missing")?;
        if canonical.repository != work.repository {
            return Err(
                "native canonical source repository differs from enrolled verifier acquisition"
                    .into(),
            );
        }
        let mut verification = admission.map_err(|error| {
            format!("canonical run {run_id} retained unmet verification: {error}")
        })?;
        verification.run = canonical;
        admit_candidate_verification(&work, &attempt, &handoff, &verification).map_err(text)?;
        ledger.record_verification(
            &work,
            attempt_id,
            fence,
            host_time_ms()?,
            &handoff,
            &verification,
        )?;
        Ok(verification)
    }

    fn retained_implementation_verification(
        &self,
        ledger: &mut Implementer,
        attempt_id: &ContentId,
        fence: u64,
        profile: &ContentId,
        authority: &VerifierAuthority,
    ) -> Result<Option<CandidateVerification>, String> {
        if self.storage_failed {
            return Err("durable canonical CI state unavailable".into());
        }
        let Some(retained) = ledger.verification_receipt(attempt_id)? else {
            return Ok(None);
        };
        let attempt = ledger
            .attempt(attempt_id)?
            .ok_or("retained implementation attempt missing")?;
        let work = current_work(ledger, &attempt)?;
        let handoff = ledger
            .candidate_handoff(attempt_id)?
            .ok_or("retained handoff missing")?;
        admit_candidate_verification(&work, &attempt, &handoff, &retained).map_err(text)?;
        if fence != attempt.fence
            || profile != &work.verification_profile
            || authority.repository != work.repository
        {
            return Err("canonical replay work/profile/fence differs".into());
        }
        let record = self
            .state
            .runs
            .get(&retained.run.run_id)
            .ok_or("retained verification lost canonical run")?;
        validate_record_provenance(record)?;
        let provenance = record
            .implementation_verification
            .as_ref()
            .ok_or("retained verification provenance missing")?;
        let project = self
            .profile
            .projects
            .iter()
            .find(|project| project.id == work.project)
            .ok_or("replay project missing")?;
        if !self.state.enrolled.contains(&work.project)
            || provenance.acquisition_root != authority.repository_root
            || fs::canonicalize(&project.repository_path).map_err(text)?
                != authority.repository_root
            || provenance.handoff != handoff
        {
            return Err("canonical replay enrolled source differs".into());
        }
        let canonical = self
            .code_review_ci()
            .into_iter()
            .find(|fact| fact.run_id == retained.run.run_id)
            .ok_or("retained canonical projection missing")?;
        if canonical != retained.run {
            return Err("retained canonical verification facts differ".into());
        }
        for artifact in &record.artifacts {
            super::storage::read_retained(
                &self.profile.state_directory.join("artifacts"),
                &artifact.id,
                artifact.bytes,
                &artifact.blake3,
            )?;
        }
        Ok(Some(retained))
    }
}

pub(super) fn validate_record_provenance(run: &RunRecord) -> Result<(), String> {
    let Some(provenance) = &run.implementation_verification else {
        return if run.is_independent_verification() {
            Err("independent verification provenance is absent".into())
        } else {
            Ok(())
        };
    };
    if !run.is_independent_verification()
        || provenance.repository_alias != provenance.handoff.repository
        || run.source.repository != native_repository_reference(&provenance.acquisition_root)?
        || run.source.exact_revision != provenance.handoff.candidate
        || !matches!(&run.source.native_facts, SourceNativeFacts::Git(facts) if facts.native_tree == provenance.handoff.tree)
        || provenance.observation_identity
            != identity(
                &run.observation
                    .as_ref()
                    .ok_or("verification observation missing")?,
            )?
        || (run.outcome() == Outcome::Success && provenance.independent_receipt.is_none())
        || !matches!(run.phase.as_str(), "terminal" | "unknown")
    {
        return Err("canonical verification repository/receipt/source binding differs".into());
    }
    let expected = provenance_binding(
        &provenance.service_id,
        &provenance.repository_alias,
        &provenance.acquisition_root,
        &provenance.handoff,
        &provenance.subject_hash,
        &provenance.observation_identity,
        provenance.independent_receipt.as_ref(),
    )?;
    let run_identity = identity(&(
        "kiln.implementation.canonical-verification.v1",
        &provenance.service_id,
        &run.project_id,
        &provenance.handoff,
        &provenance.handoff.verification_profile,
    ))?;
    if provenance.binding != expected
        || run.id != format!("web-verification-{}", run_identity.as_str())
        || run.plan.idempotency_key != run_identity
        || provenance.subject_hash.len() != 64
        || !provenance
            .subject_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("canonical verification durable provenance was substituted".into());
    }
    for node in &run.plan.nodes {
        let mut reports = run
            .reports
            .iter()
            .filter(|report| report.node_id == node.id.as_str());
        let report = reports
            .next()
            .ok_or("canonical verification node evidence missing")?;
        if reports.next().is_some() {
            return Err("canonical verification node evidence duplicated".into());
        }
        let artifact = report
            .artifact
            .as_ref()
            .ok_or("verification report artifact missing")?;
        let observed = run
            .observation
            .as_ref()
            .and_then(|observation| {
                observation
                    .nodes
                    .iter()
                    .find(|observed| observed.node_id == node.id)
            })
            .ok_or("verification node observation missing")?;
        if report.attempt_id != run.plan.attempt_id
            || report.source_revision != run.source.exact_revision.native_id.as_str()
            || run
                .observation
                .as_ref()
                .is_none_or(|observation| report.execution != observation.execution)
            || artifact.node_id != node.id.as_str()
            || artifact.attempt_id != run.plan.attempt_id
            || observed
                .receipt
                .as_ref()
                .is_some_and(|receipt| receipt.native_id.as_str() != artifact.blake3)
        {
            return Err("canonical verification check artifact/receipt linkage differs".into());
        }
        let operation = identity(&(
            "kiln.independent-verification.operation.v1",
            &provenance.handoff.verification_profile,
            node.id.as_str(),
        ))?;
        if node.operation_identity != operation {
            return Err("stored verification operation identity differs".into());
        }
    }
    Ok(())
}

fn native_repository_reference(root: &std::path::Path) -> Result<ExternalRef, String> {
    // LocalGitAdapter identifies a repository with BLAKE3 of the canonical
    // UTF-8 path bytes, not the path itself or packed-Preserves encoding.
    let path = root.to_str().ok_or("non UTF8 verifier acquisition root")?;
    let identity = kiln_codec::identity_from_bytes(path.as_bytes()).map_err(text)?;
    ExternalRef::new("git", "repository", identity.as_str()).map_err(text)
}

fn provenance_binding(
    service: &str,
    alias: &str,
    root: &std::path::Path,
    handoff: &CandidateHandoff,
    subject: &str,
    observation: &ContentId,
    receipt: Option<&ContentId>,
) -> Result<ContentId, String> {
    identity(&(
        "kiln.implementation.repository-provenance.v1",
        service,
        alias,
        root,
        handoff,
        subject,
        observation,
        receipt,
    ))
}

// Terminal retained replay deliberately remains exact-current-only. Historical
// FinishActive authority is available only through active_work while fenced.
fn current_work(
    ledger: &mut Implementer,
    attempt: &ImplementationAttempt,
) -> Result<ImplementationWork, String> {
    let work = ledger
        .work()?
        .get(&attempt.canonical_key)
        .cloned()
        .ok_or("current implementation work missing")?;
    if work.identity().map_err(text)? != attempt.work_identity
        || work.revision != attempt.work_revision
    {
        return Err("implementation work version superseded".into());
    }
    Ok(work)
}

fn validate_reviewed_nodes(
    profile: &VerificationProfile,
    checks: &[CiCheck],
    plan: &kiln_core::workflow::WorkflowEffectPlan,
) -> Result<(), String> {
    if checks.len() != plan.nodes.len() {
        return Err("reviewed CI workflow does not cover exact verification check set".into());
    }
    for node in &plan.nodes {
        let check = checks
            .iter()
            .find(|check| check.name == node.id.as_str())
            .ok_or("reviewed CI node is not an executed verification check")?;
        if node.kind.as_str() != check.kind
            || node.operation_identity
                != Engine::implementation_verification_operation(profile, &check.name)?
        {
            return Err(
                "reviewed CI kind/operation differs from frozen independently executed profile"
                    .into(),
            );
        }
    }
    let mut expected: BTreeSet<(String, String)> = profile
        .checks
        .iter()
        .flat_map(|check| {
            check
                .prerequisites
                .iter()
                .map(move |parent| (parent.clone(), check.id.clone()))
        })
        .collect();
    if checks.iter().any(|check| check.name == NATIVE_CHECK) {
        expected.extend(
            profile
                .checks
                .iter()
                .map(|check| (check.id.clone(), NATIVE_CHECK.into())),
        );
    }
    let actual: BTreeSet<_> = plan
        .edges
        .iter()
        .map(|edge| (edge.source.as_str().into(), edge.target.as_str().into()))
        .collect();
    if actual != expected {
        return Err("reviewed CI prerequisites differ from frozen verification profile".into());
    }
    Ok(())
}
fn read_material(path: &std::path::Path, bound: usize) -> Result<Vec<u8>, String> {
    if fs::symlink_metadata(path)
        .map_err(text)?
        .file_type()
        .is_symlink()
    {
        return Err("symlink verification material refused".into());
    }
    let file = fs::File::open(path).map_err(text)?;
    if !file.metadata().map_err(text)?.is_file() {
        return Err("verification material not regular".into());
    }
    let mut bytes = Vec::new();
    file.take(bound as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(text)?;
    if bytes.len() > bound {
        return Err("verification material exceeds canonical bound".into());
    }
    Ok(bytes)
}
fn text(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
#[path = "verification_tests.rs"]
mod tests;
