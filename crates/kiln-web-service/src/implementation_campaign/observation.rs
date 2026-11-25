//! Native Cairn source/readiness projection into the frozen campaign selection.
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use git2::CredentialType;

use kiln_core::{
    implementation::campaign::{
        CampaignOperation, EnumerationState, Freshness, MemberObservation, MemberSelector,
        MemberStage, RecoveryDisposition, selector_key,
    },
    types::{ContentId, ExternalRef},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{ImplementationCampaignDriveProfile, id, read_json, show, text};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NativeCairnProfile {
    pub cairn: kiln_adapter_cairn::campaign::CairnCampaignProfile,
    pub implementation_policy: kiln_adapter_cairn::implementation::CairnImplementationPolicy,
    pub source: ExternalRef,
}

impl NativeCairnProfile {
    fn validate(&self) -> Result<(), String> {
        self.source.validate().map_err(show)?;
        if !self.cairn.executable.is_absolute()
            || !self.cairn.repository_root.is_absolute()
            || !self.cairn.policy_path.is_absolute()
            || self.source.provider_namespace.as_str() != "cairn"
            || self.source.native_kind.as_str().is_empty()
            || self.implementation_policy.repository.trim().is_empty()
            || self.implementation_policy.project.trim().is_empty()
            || self.implementation_policy.actor.trim().is_empty()
            || self.implementation_policy.base.is_none()
            || self.implementation_policy.acceptance.is_empty()
            || self.implementation_policy.acceptance.len()
                >= kiln_core::implementation::MAX_ACCEPTANCE
            || self.implementation_policy.permitted_paths.is_empty()
        {
            return Err(
                "native Cairn profile is incomplete or outside the admitted Cairn source cohort"
                    .into(),
            );
        }
        kiln_adapter_cairn::implementation::validate_implementation_policy(
            &self.implementation_policy,
        )
        .map_err(|error| format!("Cairn policy admission: {error:?}"))?;
        Ok(())
    }
    pub(super) fn identity(&self) -> Result<ContentId, String> {
        let bytes = serde_json::to_vec(&(&self.cairn, &self.implementation_policy, &self.source))
            .map_err(text)?;
        id(blake3::hash(&bytes).to_hex().as_str())
    }
}

pub(super) fn validate(profile: &ImplementationCampaignDriveProfile) -> Result<(), String> {
    profile.native_cairn.validate()?;
    let source_id = profile.native_cairn.source.native_id.as_str();
    if source_id.is_empty() || source_id.len() > 256 || source_id.chars().any(char::is_control) {
        return Err("native Cairn source identity is malformed".into());
    }
    Ok(())
}

pub(super) fn refresh(
    profile: &ImplementationCampaignDriveProfile,
    contexts: &super::context::Projections,
    store: &mut kiln_implementer::campaign::CampaignStore,
    campaign_id: &str,
    expected_version: u64,
) -> Result<u64, String> {
    validate(profile)?;
    if store.version(campaign_id)? != Some(expected_version) {
        return Err("stale campaign version; native observations were not applied".into());
    }
    let initial = store.inspect(campaign_id)?.ok_or("campaign not found")?;
    if initial.cancelled
        || initial.paused
        || initial
            .pending_effects
            .values()
            .any(|effect| !effect.progress_applied)
    {
        return Ok(expected_version);
    }
    if initial.contract.campaign_id != campaign_id {
        return Err("native refresh campaign identity differs from the frozen selection".into());
    }
    if initial.contract.repository != profile.workspace.repository
        || initial.contract.repository != profile.native_cairn.implementation_policy.repository
    {
        return Err(
            "native source and workspace do not bind the frozen campaign repository".into(),
        );
    }
    if initial
        .contract
        .members
        .iter()
        .any(|binding| !super::delivery::route_supports_goal(profile.delivery.route, binding.goal))
    {
        return Err("selected delivery route cannot reach a frozen campaign endpoint; no canonical work was admitted".into());
    }
    let native_profile_identity = profile.native_cairn.identity()?;

    let enrollment: kiln_core::implementation::Enrollment = read_json(&profile.worker.enrollment)?;
    let mut implementer =
        kiln_implementer::Implementer::open(&profile.implementation_database, enrollment)?;
    let canonical_work = implementer.work()?;
    let canonical_ledgers = kiln_implementer::campaign_consumer::CanonicalCampaignLedgers::read(
        &profile.implementation_database,
    )?;
    let (current_source, target_ref, target_remote_identity, target_freshness_identity) =
        current_source(profile)?;
    let source_repository = match profile.delivery.route {
        super::delivery::NativeDeliveryRoute::NativeGit => profile
            .delivery
            .native_git
            .as_ref()
            .ok_or("native-Git source route is not configured")?
            .repository_path
            .clone(),
        super::delivery::NativeDeliveryRoute::GithubPullRequest => profile
            .delivery
            .github
            .as_ref()
            .ok_or("GitHub source route is not configured")?
            .repository
            .clone(),
    };
    let exact_source = super::workspace::materialize_observation(
        &profile.workspace,
        &source_repository,
        current_source.native_id.as_str(),
    )?;
    let mut cairn_profile = profile.native_cairn.cairn.clone();
    cairn_profile.repository_root = exact_source.clone();
    if let Ok(relative_policy) = cairn_profile
        .policy_path
        .strip_prefix(&profile.native_cairn.cairn.repository_root)
    {
        cairn_profile.policy_path = exact_source.join(relative_policy);
    }
    let admission_sequence = next_work_revision(&canonical_work)?;
    let mut reader = kiln_adapter_cairn::campaign::CairnCampaignReader::new(
        cairn_profile,
        kiln_adapter_cairn::BoundedCairnRuntime,
    )?;
    let mut next_version = expected_version;
    let mut collected = BTreeMap::new();
    let mut selected_changes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for binding in &initial.contract.members {
        let (change, tasks) = selector_parts(&binding.selector);
        if !valid_change(change) {
            return Err("frozen campaign contains a malformed Cairn change identity".into());
        }
        selected_changes
            .entry(change.to_owned())
            .or_default()
            .extend(tasks);
    }
    for (change, selected_tasks) in selected_changes {
        let snapshot = reader.collect(&change)?;
        let configured_source = profile
            .native_cairn
            .implementation_policy
            .base
            .as_ref()
            .ok_or("Cairn implementation policy omitted its exact source base")?;
        let (
            rechecked_source,
            rechecked_target_ref,
            rechecked_remote_identity,
            rechecked_freshness_identity,
        ) = self::current_source(profile)?;
        let target_stable = rechecked_source == current_source
            && rechecked_target_ref == target_ref
            && rechecked_remote_identity == target_remote_identity
            && rechecked_freshness_identity == target_freshness_identity;
        let source_advance_effect = if target_stable && &current_source != configured_source {
            integrated_successor_permitted(
                profile,
                &initial,
                &current_source,
                &target_ref,
                &target_remote_identity,
                &canonical_ledgers,
                &mut implementer,
            )?
        } else {
            None
        };
        let source_permitted = target_stable
            && (&current_source == configured_source || source_advance_effect.is_some());
        let mut implementation_policy = profile.native_cairn.implementation_policy.clone();
        implementation_policy.base = Some(current_source.clone());
        let (native_works, outcome) = if !source_permitted {
            (Vec::new(), "source_moved".to_owned())
        } else if snapshot
            .implementation_ready
            .get("complete")
            .and_then(Value::as_bool)
            == Some(true)
        {
            let ready_bytes = serde_json::to_vec(&snapshot.implementation_ready).map_err(text)?;
            let admission = kiln_adapter_cairn::implementation::admit_cairn_snapshot(
                &implementation_policy,
                admission_sequence,
                &ready_bytes,
            )
            .map_err(|error| format!("Cairn work admission: {error:?}"))?;
            (admission.works, admission.outcome)
        } else {
            (Vec::new(), "incomplete_limit".to_owned())
        };
        let mut work_by_native = BTreeMap::new();
        for mut work in native_works {
            let Some(alias) = work
                .aliases
                .iter()
                .find(|alias| alias.kind == kiln_core::implementation::WorkSourceKind::CairnTask)
            else {
                continue;
            };
            let alias_id = alias.native_id.clone();
            let unit = alias_id
                .split_once('/')
                .map(|(_, unit)| unit.strip_prefix("task:").unwrap_or(unit))
                .unwrap_or("");
            if selected_tasks.is_empty() || selected_tasks.contains(unit) {
                retain_work_revision(&mut implementer, &mut work)?;
                implementer.admit(&work, None)?;
                work_by_native.insert(alias_id, work);
            }
        }
        collected.insert(
            change,
            (snapshot, work_by_native, outcome, source_advance_effect),
        );
    }
    if collected.is_empty() {
        return Err("frozen campaign has no selected Cairn changes".into());
    }

    for binding in &initial.contract.members {
        let (change, tasks) = selector_parts(&binding.selector);
        let Some((snapshot, works, outcome, source_advance_effect)) = collected.get(change) else {
            continue;
        };
        let member_key = selector_key(&binding.selector);
        let prior = initial.observations.get(&member_key);
        if initial
            .pending_effects
            .values()
            .any(|effect| effect.member == member_key && !effect.progress_applied)
        {
            continue;
        }
        let old_work = prior.and_then(|observation| observation.work_identity.clone());
        let mut observation = prior.cloned().unwrap_or_else(|| {
            empty_observation(&member_key, &binding.policy, &binding.profile_version)
        });
        observation.member = member_key.clone();
        observation.policy_identity = binding.policy.clone();
        observation.profile_identity = binding.profile_version.clone();
        observation.native_context_identity = Some(bound_native_context_identity(
            &native_profile_identity,
            &snapshot.context_identity,
            &target_remote_identity,
            &target_ref,
            &current_source,
            &target_freshness_identity,
        )?);
        observation.context_identity = context_identity(binding)?;
        observation.context_freshness = super::context::freshness(binding, contexts);
        observation.integration_target_identity = Some(target_freshness_identity.clone());
        observation.target_freshness = if outcome == "source_moved" {
            Freshness::Stale
        } else {
            Freshness::Current
        };
        observation.policy_freshness = Freshness::Current;
        observation.source_identity = None;
        observation.work_identity = None;
        observation.canonical_work_key = None;
        observation.dependencies =
            native_dependencies(&snapshot.graph, change, &initial.contract.members)?;
        observation.enumeration = if snapshot
            .implementation_ready
            .get("complete")
            .and_then(Value::as_bool)
            == Some(true)
        {
            EnumerationState::Complete
        } else {
            EnumerationState::IncompleteLimit
        };
        if observation.enumeration == EnumerationState::Complete
            && prior.is_none_or(|old| old.enumeration != EnumerationState::Complete)
        {
            observation.remaining_obligations = required_native_units(snapshot, change, &tasks)?;
        }
        observation.blockers.clear();
        observation.dependency_reason = None;
        observation.recovery = None;
        observation.cancelled = false;
        // A completion-only member's adopted canonical evidence, bound below.
        let mut adopted: Option<super::adoption::Adopted> = None;
        if outcome == "source_moved" {
            observation.stage = MemberStage::Blocked;
            observation.blockers.push("native Git source moved without an exact qualified integrated/captured candidate receipt".into());
            observation.recovery = Some(RecoveryDisposition::SemanticReplan);
        } else if observation.enumeration == EnumerationState::IncompleteLimit {
            observation.stage = MemberStage::Blocked;
            observation
                .blockers
                .push("Cairn implementation-ready enumeration is incomplete".into());
        } else if snapshot
            .workflow_status
            .get("valid")
            .and_then(Value::as_bool)
            != Some(true)
        {
            let artifacts = snapshot
                .workflow_status
                .get("artifacts")
                .and_then(Value::as_array)
                .ok_or("Cairn workflow status omitted artifacts")?;
            let repairable = artifacts
                .iter()
                .filter(|artifact| {
                    artifact["state"].as_str() == Some("blocked")
                        && artifact["files"]
                            .as_array()
                            .is_some_and(|files| !files.is_empty())
                        && artifact["blockers"]
                            .as_array()
                            .is_some_and(|blockers| !blockers.is_empty())
                        && snapshot
                            .workflow_status
                            .get("issues")
                            .and_then(Value::as_array)
                            == artifact["blockers"].as_array()
                })
                .collect::<Vec<_>>();
            if let &[artifact] = repairable.as_slice() {
                let artifact_id = artifact["id"]
                    .as_str()
                    .ok_or("Cairn workflow artifact id is absent")?;
                let artifact_stage = artifact["stage"]
                    .as_str()
                    .ok_or("Cairn workflow artifact stage is absent")?;
                let native_id = format!("{change}/{artifact_stage}/{artifact_id}");
                let mut work =
                    artifact_work(profile, snapshot, artifact, &native_id, &current_source)?;
                retain_work_revision(&mut implementer, &mut work)?;
                implementer.admit(&work, None)?;
                observation.source_identity =
                    work.aliases.first().map(|alias| alias.version.clone());
                observation.canonical_work_key = Some(work.canonical_key().map_err(show)?);
                observation.work_identity = Some(work.identity().map_err(show)?);
                observation.stage = MemberStage::Planning;
                observation.recovery = Some(RecoveryDisposition::ArtifactRepair);
                observation.blockers.extend(
                    artifact["blockers"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(str::to_owned),
                );
            } else {
                observation.stage = MemberStage::Blocked;
                observation.blockers.extend(
                    snapshot
                        .workflow_status
                        .get("issues")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(str::to_owned),
                );
                if observation.blockers.is_empty() {
                    observation
                        .blockers
                        .push("native Cairn workflow status is invalid".into());
                }
            }
        } else if let Some(workflow) = snapshot
            .workflow_status
            .get("artifacts")
            .and_then(Value::as_array)
        {
            let ready = workflow
                .iter()
                .filter(|artifact| artifact["state"].as_str() == Some("ready"))
                .collect::<Vec<_>>();
            if ready.len() > 1 {
                observation.stage = MemberStage::Blocked;
                observation.blockers.push("native Cairn workflow exposes multiple ready artifact stages for one selected member".into());
            } else if let Some(artifact) = ready.first() {
                let artifact = *artifact;
                let artifact_id = artifact["id"]
                    .as_str()
                    .ok_or("Cairn workflow artifact id is absent")?;
                let artifact_stage = artifact["stage"]
                    .as_str()
                    .ok_or("Cairn workflow artifact stage is absent")?;
                let native_id = format!("{change}/{artifact_stage}/{artifact_id}");
                let mut work =
                    artifact_work(profile, snapshot, artifact, &native_id, &current_source)?;
                retain_work_revision(&mut implementer, &mut work)?;
                implementer.admit(&work, None)?;
                observation.source_identity =
                    work.aliases.first().map(|alias| alias.version.clone());
                observation.canonical_work_key = Some(work.canonical_key().map_err(show)?);
                observation.work_identity = Some(work.identity().map_err(show)?);
                observation.stage = MemberStage::Selected;
            } else {
                observation.stage =
                    stage_for_tasks(change, &tasks, works, outcome, snapshot, &mut observation)?;
                // r[impl kiln.implementation.campaign.adoption]
                if outcome == "completion_only" {
                    let units = required_native_units(snapshot, change, &tasks)?;
                    let evidence = super::adoption::unit_evidence(
                        &canonical_ledgers,
                        &initial.contract.repository,
                        &units,
                        |attempt| {
                            route_integration(
                                profile,
                                &canonical_ledgers,
                                &mut implementer,
                                attempt,
                            )
                        },
                    )?;
                    let decision = match super::adoption::TargetHistory::open(
                        &source_repository,
                        current_source.native_id.as_str(),
                        change,
                    ) {
                        Ok(target) => super::adoption::decide(&evidence, &target),
                        Err(error) => super::adoption::Adoption::Blocked(error),
                    };
                    match decision {
                        super::adoption::Adoption::Adopted { binding } => {
                            let binding = super::adoption::Adopted::new(&evidence, binding)?;
                            binding.identify(&mut observation);
                            observation.stage = MemberStage::Integrated;
                            adopted = Some(binding);
                        }
                        super::adoption::Adoption::Blocked(reason) => {
                            observation.blockers.push(format!(
                                "native Cairn reports completion-only and no implementation unit may be scheduled; adoption refused: {reason}"
                            ));
                        }
                    }
                }
            }
        } else {
            observation.enumeration = EnumerationState::Invalid;
            observation.stage = MemberStage::Blocked;
            observation
                .blockers
                .push("Cairn workflow status omitted artifact observations".into());
        }
        if observation.remaining_obligations.is_empty()
            && outcome != "source_moved"
            && observation.enumeration == EnumerationState::Complete
        {
            if let Some(old) = prior.filter(|old| {
                old.work_identity.is_some()
                    && matches!(
                        old.stage,
                        MemberStage::VerifiedCandidate
                            | MemberStage::VerifiedPullRequest
                            | MemberStage::Integrated
                            | MemberStage::Lifecycle
                            | MemberStage::Complete
                    )
            }) {
                observation.work_identity = old.work_identity.clone();
                observation.canonical_work_key = old.canonical_work_key.clone();
                observation.source_identity = old.source_identity.clone();
                observation.native_context_identity = old.native_context_identity.clone();
                observation.stage = old.stage;
                observation.blockers = old.blockers.clone();
                if !observation.evidence.contains(&snapshot.context_identity) {
                    observation.evidence.push(snapshot.context_identity.clone());
                }
            }
        }
        let preserve_progress = prior.is_some_and(|prior| {
            old_work == observation.work_identity
                && prior.attempt_identity.is_some()
                && matches!(
                    prior.stage,
                    MemberStage::Claimed
                        | MemberStage::Executing
                        | MemberStage::Candidate
                        | MemberStage::Verifying
                        | MemberStage::VerifiedCandidate
                        | MemberStage::VerifiedPullRequest
                        | MemberStage::Integrated
                        | MemberStage::Lifecycle
                        | MemberStage::Complete
                        | MemberStage::Cancelled
                )
        });
        if preserve_progress {
            observation.stage = prior.map(|prior| prior.stage).unwrap_or(observation.stage);
        }
        if old_work != observation.work_identity {
            observation.attempt_identity = None;
            observation.attempt_fence = None;
            observation.candidate_freshness = Freshness::Unknown;
            observation.verifier_freshness = Freshness::Unknown;
            observation.evidence.clear();
            observation.recovery = None;
        }
        if !preserve_progress {
            observation.required_checks_satisfied = false;
            observation.required_reviews_satisfied = false;
            observation.delivery_route_supported = Some(super::delivery::route_supports_goal(
                profile.delivery.route,
                binding.goal,
            ));
        }
        if let Some(adopted) = &adopted {
            adopted.bind(&mut observation, &binding.required_checks);
        }
        // Adoption is not source progress from this campaign's own integration.
        if let Some(effect) = source_advance_effect.as_ref().filter(|_| {
            adopted.is_none()
                && old_work != observation.work_identity
                && observation.work_identity.is_some()
        }) {
            store.observe_successor(campaign_id, next_version, observation, effect)?;
        } else {
            store.observe(campaign_id, next_version, observation)?;
        }
        next_version = store
            .version(campaign_id)?
            .ok_or("campaign disappeared after native observation")?;
    }
    Ok(next_version)
}

pub(super) fn bind_stage_request(
    profile: &ImplementationCampaignDriveProfile,
    effect: &kiln_core::implementation::campaign::PendingEffect,
    work: &kiln_core::implementation::ImplementationWork,
    request: &mut Value,
) -> Result<(), String> {
    let identity = work.identity().map_err(show)?;
    if effect.work_identity.as_ref() != Some(&identity) || effect.member.is_empty() {
        return Err("stage effect does not bind the selected canonical work".into());
    }
    let (kind, alias) = unique_cairn_alias(work)?;
    let is_artifact = kind == kiln_core::implementation::WorkSourceKind::CairnArtifact;
    if matches!(
        effect.operation,
        CampaignOperation::AuthorArtifact | CampaignOperation::RepairArtifact
    ) != is_artifact
    {
        return Err("artifact and task operations cannot be crossed".into());
    }
    let native_profile_identity = profile.native_cairn.identity()?;
    if alias.source.provider_namespace != profile.native_cairn.source.provider_namespace {
        return Err("stage source provider differs from configured Cairn source".into());
    }
    let (change, stage, subject) = parse_source(kind, &alias.native_id)?;
    if !valid_change(change) {
        return Err("Cairn stage change identity is malformed".into());
    }
    let source = work
        .base
        .as_ref()
        .ok_or("Cairn stage revalidation requires the canonical exact work base")?;
    let (observed_target, _, _, _) = current_source(profile)?;
    if observed_target != *source {
        return Err("qualified native target moved from the canonical work base before stage admission; explicit replan is required".into());
    }
    let source_repository = match profile.delivery.route {
        super::delivery::NativeDeliveryRoute::NativeGit => profile
            .delivery
            .native_git
            .as_ref()
            .ok_or("native-Git source route is not configured")?
            .repository_path
            .clone(),
        super::delivery::NativeDeliveryRoute::GithubPullRequest => profile
            .delivery
            .github
            .as_ref()
            .ok_or("GitHub source route is not configured")?
            .repository
            .clone(),
    };
    let exact_source = super::workspace::materialize_observation(
        &profile.workspace,
        &source_repository,
        source.native_id.as_str(),
    )?;
    let mut cairn_profile = profile.native_cairn.cairn.clone();
    cairn_profile.repository_root = exact_source.clone();
    if let Ok(relative_policy) = cairn_profile
        .policy_path
        .strip_prefix(&profile.native_cairn.cairn.repository_root)
    {
        cairn_profile.policy_path = exact_source.join(relative_policy);
    }
    let mut reader = kiln_adapter_cairn::campaign::CairnCampaignReader::new(
        cairn_profile,
        kiln_adapter_cairn::BoundedCairnRuntime,
    )?;
    let snapshot = reader.collect(change)?;
    let instruction_id = if is_artifact { subject } else { "tasks" };
    let instructions = snapshot
        .workflow_instructions
        .get(instruction_id)
        .ok_or("native Cairn instructions are absent for the selected stage")?;
    let instruction_artifact = instructions
        .get("artifact")
        .ok_or("native Cairn instruction artifact envelope is absent")?;
    if instruction_artifact
        .get("artifact_id")
        .and_then(Value::as_str)
        != Some(instruction_id)
        || instruction_artifact
            .get("output_path")
            .and_then(Value::as_str)
            .is_none()
        || instruction_artifact
            .get("instruction_template")
            .and_then(Value::as_str)
            .is_none()
    {
        return Err(
            "native Cairn instructions do not bind the selected stage artifact and output".into(),
        );
    }
    if is_artifact {
        let expected_state = if effect.operation == CampaignOperation::RepairArtifact {
            "blocked"
        } else {
            "ready"
        };
        if alias.version != snapshot.context_identity
            || !snapshot
                .workflow_status
                .get("artifacts")
                .and_then(Value::as_array)
                .is_some_and(|items| {
                    items.iter().any(|item| {
                        item["id"].as_str() == Some(subject)
                            && item["stage"].as_str() == Some(stage)
                            && item["state"].as_str() == Some(expected_state)
                    })
                })
        {
            return Err("Cairn artifact source is stale or no longer the exact authorable/repairable workflow stage".into());
        }
    } else {
        let ready = snapshot
            .implementation_ready
            .get("selection")
            .and_then(Value::as_array)
            .ok_or("Cairn implementation-ready selection is absent")?;
        if !ready.iter().any(|item| {
            let unit = item["unit"].as_str();
            item["change"].as_str() == Some(change)
                && item["snapshot_digest"].as_str() == Some(alias.version.as_str())
                && unit == Some(subject)
        }) {
            return Err("Cairn task/change source is stale or absent from native implementation-ready selection".into());
        }
    }
    let acceptance = kiln_core::implementation::acceptance_digest(work).map_err(show)?;
    let envelope = request
        .get_mut("envelope")
        .and_then(Value::as_object_mut)
        .ok_or("native Lattice request lacks its typed attempt envelope")?;
    let expected_kind = if is_artifact {
        "cairn_artifact"
    } else {
        "cairn_task"
    };
    if envelope.get("task_spec_digest").and_then(Value::as_str) != Some(acceptance.as_str())
        || envelope.get("source_kind").and_then(Value::as_str) != Some(expected_kind)
        || envelope.get("source_id").and_then(Value::as_str) != Some(alias.native_id.as_str())
        || envelope.get("readiness_snapshot").and_then(Value::as_str)
            != Some(alias.version.as_str())
    {
        return Err(
            "native request source or protected acceptance differs from canonical work".into(),
        );
    }
    let harness = native_prompt_profile(request)?;
    let prompt = harness
        .get("prompt")
        .and_then(Value::as_str)
        .ok_or("native harness profile prompt is not text")?;
    let change_projection = snapshot
        .implementation_ready
        .get("changes")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .find(|item| item["name"].as_str() == Some(change))
        })
        .ok_or("native implementation-ready change projection is absent")?;
    let selected_units = snapshot
        .implementation_ready
        .get("selection")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item["change"].as_str() == Some(change))
        .cloned()
        .collect::<Vec<_>>();
    let context = serde_json::json!({
        "decision_context": super::context::prompt(profile, effect)?,
        "schema": "kiln.cairn-stage-context.v1", "change": change, "stage": stage,
        "native_profile_identity": native_profile_identity,
        "subject": subject, "source_kind": expected_kind, "source_id": alias.native_id,
        "source_version": alias.version, "native_context_identity": snapshot.context_identity,
        "implementation_ready": {"change": change_projection, "selected_units": selected_units},
        "workflow_status": snapshot.workflow_status, "readiness": snapshot.readiness,
        "instructions": instructions, "acceptance": work.acceptance,
        "non_claims": ["native Cairn instructions are guidance, not artifact truth or approval", "readiness is not a grant of compute or model resources"],
    });
    let suffix = serde_json::to_string(&context).map_err(text)?;
    let marker = format!(
        "\n\n[Kiln native Cairn stage context {} {}]\n",
        native_profile_identity.as_str(),
        snapshot.context_identity.as_str()
    );
    let mut bound_prompt = prompt.to_owned();
    if let Some(index) = bound_prompt.find("\n\n[Kiln native Cairn stage context ") {
        bound_prompt.truncate(index);
    }
    bound_prompt.push_str(&marker);
    bound_prompt.push_str(&suffix);
    if bound_prompt.len() > 1_048_576 {
        return Err("native Cairn stage prompt exceeds the admitted request bound".into());
    }
    harness.insert("prompt".into(), Value::String(bound_prompt));
    Ok(())
}

fn native_prompt_profile(request: &mut Value) -> Result<&mut serde_json::Map<String, Value>, String> {
    let selected = match (
        request.get("omp").is_some_and(|value| !value.is_null()),
        request.get("animus_implementation").is_some_and(|value| !value.is_null()),
    ) {
        (true, false) => "omp",
        (false, true) => "animus_implementation",
        _ => return Err("native request requires exactly one implementation harness".into()),
    };
    let profile = request
        .get_mut(selected)
        .and_then(Value::as_object_mut)
        .ok_or("native Lattice request lacks its typed harness profile")?;
    if selected == "omp" {
        for key in ["revision", "executable_blake3", "base_identity", "model", "prompt"] {
            if !profile.contains_key(key) {
                return Err(format!("native OMP profile lacks supported field {key}"));
            }
        }
    }
    Ok(profile)
}

fn artifact_work(
    profile: &ImplementationCampaignDriveProfile,
    snapshot: &kiln_adapter_cairn::campaign::CairnCampaignSnapshot,
    artifact: &Value,
    native_id: &str,
    current_source: &ExternalRef,
) -> Result<kiln_core::implementation::ImplementationWork, String> {
    let instruction = snapshot
        .workflow_instructions
        .get(
            artifact["id"]
                .as_str()
                .ok_or("Cairn artifact id is absent")?,
        )
        .ok_or("Cairn artifact instructions are absent")?;
    let output_path = instruction
        .get("artifact")
        .and_then(|artifact| artifact.get("output_path"))
        .and_then(Value::as_str)
        .ok_or("Cairn artifact output path is absent")?;
    let output = PathBuf::from(output_path);
    if output.as_os_str().is_empty()
        || output.is_absolute()
        || output_path.contains('\\')
        || output_path.chars().any(char::is_control)
        || output_path
            .split('/')
            .any(|part| matches!(part, "" | "." | ".."))
        || output.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir
                    | std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err("native Cairn artifact output path is not repository-relative".into());
    }
    let change_root = match snapshot
        .workflow_status
        .get("layout")
        .and_then(Value::as_str)
    {
        Some("cairn") => ".cairn/changes",
        Some("openspec") => "openspec/changes",
        _ => return Err("native Cairn workflow status has an unsupported change layout".into()),
    };
    let canonical_output = format!("{change_root}/{}/{output_path}", snapshot.change);
    if !profile
        .native_cairn
        .implementation_policy
        .permitted_paths
        .iter()
        .any(|allowed| {
            allowed == &canonical_output
                || allowed.ends_with("/**")
                    && canonical_output.starts_with(allowed.trim_end_matches("**"))
        })
    {
        return Err(
            "native Cairn artifact output is outside the configured canonical work path authority"
                .into(),
        );
    }
    let source = kiln_core::implementation::SourceRef {
        kind: kiln_core::implementation::WorkSourceKind::CairnArtifact,
        source: ExternalRef::new(
            profile.native_cairn.source.provider_namespace.as_str(),
            "artifact",
            native_id,
        )
        .map_err(show)?,
        native_id: native_id.to_owned(),
        version: snapshot.context_identity.clone(),
    };
    let artifact_id = artifact["id"]
        .as_str()
        .ok_or("Cairn artifact id is absent")?;
    let fingerprint = blake3::hash(native_id.as_bytes()).to_hex();
    let mut acceptance = profile
        .native_cairn
        .implementation_policy
        .acceptance
        .clone();
    acceptance.push(kiln_core::implementation::AcceptanceObligation {
        id: format!("cairn-artifact-{}", &fingerprint.as_str()[..16]),
        statement: format!("Author native Cairn workflow artifact `{artifact_id}` for change `{}` at canonical repository output `{canonical_output}`; follow the bound native workflow instructions.", snapshot.change),
    });
    Ok(kiln_core::implementation::ImplementationWork {
        schema: kiln_core::implementation::IMPLEMENTATION_WORK_SCHEMA.to_owned(),
        revision: 1,
        actor: profile.native_cairn.implementation_policy.actor.clone(),
        repository: profile
            .native_cairn
            .implementation_policy
            .repository
            .clone(),
        project: profile.native_cairn.implementation_policy.project.clone(),
        base: Some(current_source.clone()),
        title: format!("Cairn artifact {native_id}"),
        requested_outcome: profile
            .native_cairn
            .implementation_policy
            .requested_outcome
            .clone(),
        acceptance,
        dependencies: Vec::new(),
        permitted_paths: vec![canonical_output],
        permitted_effects: profile
            .native_cairn
            .implementation_policy
            .permitted_effects
            .clone(),
        verification_profile: profile
            .native_cairn
            .implementation_policy
            .verification_profile
            .clone(),
        budget: profile.native_cairn.implementation_policy.budget,
        terminal_policy: profile.native_cairn.implementation_policy.terminal_policy,
        priority: profile.native_cairn.implementation_policy.priority,
        aliases: vec![source],
    })
}

fn required_native_units(
    snapshot: &kiln_adapter_cairn::campaign::CairnCampaignSnapshot,
    change: &str,
    tasks: &[String],
) -> Result<BTreeSet<String>, String> {
    if !tasks.is_empty() {
        return Ok(tasks
            .iter()
            .map(|task| format!("{change}/task:{task}"))
            .collect());
    }
    let record = snapshot
        .implementation_ready
        .get("changes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|record| record["name"].as_str() == Some(change))
        .ok_or("selected whole change is absent from complete native enumeration")?;
    let task_ordering = record.get("task_ordering");
    if task_ordering
        .and_then(|ordering| ordering.get("active"))
        .and_then(Value::as_bool)
        == Some(true)
    {
        let order = task_ordering
            .and_then(|ordering| ordering.get("order"))
            .and_then(Value::as_array)
            .ok_or("native ordered change omitted its full task order")?;
        if order.is_empty() || order.len() > kiln_core::implementation::campaign::MAX_MEMBERS {
            return Err("native task order is empty or exceeds selected-scope bounds".into());
        }
        order
            .iter()
            .map(|task| {
                task.as_str()
                    .filter(|task| !task.is_empty())
                    .map(|task| format!("{change}/task:{task}"))
                    .ok_or_else(|| "native task order contains a malformed identity".into())
            })
            .collect()
    } else {
        Ok(BTreeSet::from([format!("{change}/change:{change}")]))
    }
}

fn native_ordered_unit(
    snapshot: &kiln_adapter_cairn::campaign::CairnCampaignSnapshot,
    change: &str,
    mut has_work: impl FnMut(&str) -> bool,
) -> Option<String> {
    snapshot
        .implementation_ready
        .get("selection")?
        .as_array()?
        .iter()
        .filter(|selected| selected["change"].as_str() == Some(change))
        .filter_map(|selected| {
            selected["unit"]
                .as_str()
                .map(|unit| format!("{change}/{unit}"))
        })
        .find(|native_id| has_work(native_id))
}

fn stage_for_tasks(
    change: &str,
    tasks: &[String],
    works: &BTreeMap<String, kiln_core::implementation::ImplementationWork>,
    outcome: &str,
    snapshot: &kiln_adapter_cairn::campaign::CairnCampaignSnapshot,
    observation: &mut MemberObservation,
) -> Result<MemberStage, String> {
    let selected_work = native_ordered_unit(snapshot, change, |native_id| {
        works.contains_key(native_id) && observation.remaining_obligations.contains(native_id)
    })
    .and_then(|native_id| works.get(&native_id));
    if let Some(work) = selected_work {
        observation.source_identity = work
            .aliases
            .iter()
            .find(|alias| alias.kind == kiln_core::implementation::WorkSourceKind::CairnTask)
            .map(|alias| alias.version.clone());
        observation.canonical_work_key = Some(work.canonical_key().map_err(show)?);
        observation.work_identity = Some(work.identity().map_err(show)?);
        return Ok(MemberStage::Ready);
    }
    if outcome == "completion_only" {
        // Native observation decides whether existing evidence is adopted.
        return Ok(MemberStage::Blocked);
    }
    if outcome == "incomplete_limit" {
        observation.enumeration = EnumerationState::IncompleteLimit;
        observation
            .blockers
            .push("native Cairn implementation-ready enumeration is incomplete".into());
        return Ok(MemberStage::Blocked);
    }
    if let Some(record) = snapshot
        .implementation_ready
        .get("changes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|item| item["name"].as_str() == Some(change))
    {
        observation
            .blockers
            .extend(
                record["blockers"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|blocker| {
                        blocker["message"]
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| blocker.to_string())
                    }),
            );
        for blocked in record["blocked_tasks"].as_array().into_iter().flatten() {
            let task = blocked["task"].as_str().unwrap_or_default();
            if tasks.is_empty() || tasks.iter().any(|selected| selected == task) {
                observation.blockers.push(format!(
                    "Cairn task {task} is blocked by {}",
                    blocked["blocked_by"]
                ));
            }
        }
        if record["completed_tasks"].as_array().is_some_and(|done| {
            !tasks.is_empty()
                && tasks
                    .iter()
                    .all(|task| done.iter().any(|item| item.as_str() == Some(task)))
        }) {
            observation.blockers.push("selected Cairn task is already complete in the native projection; this is not campaign verification".into());
        }
    }
    if tasks.is_empty() {
        return Ok(MemberStage::Blocked);
    }
    if observation.blockers.is_empty() {
        observation
            .blockers
            .push("selected Cairn task is not in the native ready selection".into());
    }
    observation.recovery = Some(RecoveryDisposition::SemanticReplan);
    Ok(MemberStage::Blocked)
}

fn native_dependencies(
    graph: &Value,
    change: &str,
    members: &[kiln_core::implementation::campaign::MemberBinding],
) -> Result<Vec<String>, String> {
    let edges = graph["edges"]
        .as_array()
        .ok_or("Cairn change graph omitted dependencies")?;
    let mut dependencies = BTreeSet::new();
    for edge in edges
        .iter()
        .filter(|edge| edge["dependent"].as_str() == Some(change))
    {
        let dependency = edge["dependency"]
            .as_str()
            .ok_or("Cairn dependency edge omitted its identity")?;
        if let Some(selected) = members
            .iter()
            .find(|member| selector_parts(&member.selector).0 == dependency)
        {
            dependencies.insert(selector_key(&selected.selector));
        } else if edge["satisfied"].as_bool() != Some(true) {
            dependencies.insert(format!("{dependency}/*"));
        }
    }
    Ok(dependencies.into_iter().collect())
}

fn context_identity(
    binding: &kiln_core::implementation::campaign::MemberBinding,
) -> Result<Option<ContentId>, String> {
    if binding.context.is_empty() {
        return Ok(None);
    }
    let bytes = serde_json::to_vec(&binding.context).map_err(text)?;
    hash(&bytes).map(Some)
}

fn valid_change(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn selector_parts(selector: &MemberSelector) -> (&str, Vec<String>) {
    match selector {
        MemberSelector::WholeChange { change } => (change, Vec::new()),
        MemberSelector::Tasks { change, tasks } => (change, tasks.clone()),
    }
}
fn unique_cairn_alias(
    work: &kiln_core::implementation::ImplementationWork,
) -> Result<
    (
        kiln_core::implementation::WorkSourceKind,
        kiln_core::implementation::SourceRef,
    ),
    String,
> {
    let aliases = work
        .aliases
        .iter()
        .filter(|alias| {
            matches!(
                alias.kind,
                kiln_core::implementation::WorkSourceKind::CairnArtifact
                    | kiln_core::implementation::WorkSourceKind::CairnTask
            )
        })
        .collect::<Vec<_>>();
    let &[alias] = aliases.as_slice() else {
        return Err("Cairn stage work must have exactly one task or artifact source alias".into());
    };
    Ok((alias.kind, alias.clone()))
}
fn parse_source(
    kind: kiln_core::implementation::WorkSourceKind,
    value: &str,
) -> Result<(&str, &str, &str), String> {
    let parts = value.split('/').collect::<Vec<_>>();
    match (kind, parts.as_slice()) {
        (kiln_core::implementation::WorkSourceKind::CairnArtifact, &[change, stage, subject]) => {
            Ok((change, stage, subject))
        }
        (kiln_core::implementation::WorkSourceKind::CairnTask, &[change, unit])
            if unit.starts_with("task:") || unit.starts_with("change:") =>
        {
            Ok((change, "tasks", unit))
        }
        _ => Err("Cairn source identity must preserve exact change/stage/task distinction".into()),
    }
}
fn empty_observation(member: &str, policy: &ContentId, profile: &ContentId) -> MemberObservation {
    MemberObservation {
        member: member.to_owned(),
        stage: MemberStage::Selected,
        enumeration: EnumerationState::NotObserved,
        source_identity: None,
        policy_identity: policy.clone(),
        subject_identity: None,
        integration_target_identity: None,
        profile_identity: profile.clone(),
        dependencies: Vec::new(),
        dependency_reason: None,
        remaining_obligations: Default::default(),
        canonical_work_key: None,
        work_identity: None,
        attempt_identity: None,
        attempt_fence: None,
        context_identity: None,
        native_context_identity: None,
        context_freshness: Freshness::Unknown,
        candidate_freshness: Freshness::Unknown,
        policy_freshness: Freshness::Unknown,
        verifier_freshness: Freshness::Unknown,
        target_freshness: Freshness::Unknown,
        evidence: Vec::new(),
        blockers: Vec::new(),
        required_checks_satisfied: false,
        required_reviews_satisfied: false,
        delivery_route_supported: None,
        recovery: None,
        consumed_budget: 0,
        reserved_budget: 0,
        unknown_budget: 0,
        cancelled: false,
    }
}
fn current_source(
    profile: &ImplementationCampaignDriveProfile,
) -> Result<(ExternalRef, String, ContentId, ContentId), String> {
    match profile.delivery.route {
        super::delivery::NativeDeliveryRoute::NativeGit => {
            let route = profile
                .delivery
                .native_git
                .as_ref()
                .ok_or("native-Git source route is not configured")?;
            let repository = git2::Repository::open(&route.repository_path)
                .map_err(|error| format!("native Git target repository is unavailable: {error}"))?;
            let remote_identity =
                kiln_adapter_git::native_publication::remote_identity(&route.remote)
                    .map_err(text)?;
            let credential = |url: &str, _username: Option<&str>, _allowed: CredentialType| {
                if url != route.remote.remote_url {
                    Err(git2::Error::from_str(
                        "credential URL differs from admitted native remote",
                    ))
                } else {
                    Err(git2::Error::from_str(
                        "native target observation has no scoped credential authority; authenticated target read is unavailable",
                    ))
                }
            };
            let target = kiln_adapter_git::native_publication::observe_remote_target(
                &repository,
                &route.remote,
                &remote_identity,
                &route.target_ref,
                credential,
            )
            .map_err(|error| format!("qualified native target read failed: {error}"))?;
            Ok((
                target.current_commit,
                target.target_ref,
                target.remote_identity,
                target.freshness_identity,
            ))
        }
        super::delivery::NativeDeliveryRoute::GithubPullRequest => {
            let route = profile
                .delivery
                .github
                .as_ref()
                .ok_or("GitHub source route is not configured")?;
            let source: kiln_adapter_github::publication::PublicationSourceProfile =
                read_json(&route.remote_profile)?;
            if kiln_adapter_git::native_publication::remote_identity(&source.remote)
                .map_err(text)?
                != source.remote_identity
            {
                return Err("GitHub target remote profile identity is stale".into());
            }
            let target_ref = format!("refs/heads/{}", route.publication_policy.base_branch);
            let repository = git2::Repository::open(&route.repository).map_err(|error| {
                format!("GitHub target object repository is unavailable: {error}")
            })?;
            let remote_identity = source.remote_identity.clone();
            let credential = |url: &str, _username: Option<&str>, _allowed: CredentialType| {
                if url != source.remote.remote_url {
                    Err(git2::Error::from_str(
                        "credential URL differs from admitted GitHub remote",
                    ))
                } else {
                    Err(git2::Error::from_str(
                        "GitHub target observation has no scoped credential authority; private target read is unavailable",
                    ))
                }
            };
            let target = kiln_adapter_git::native_publication::observe_remote_target(
                &repository,
                &source.remote,
                &remote_identity,
                &target_ref,
                credential,
            )
            .map_err(|error| format!("qualified GitHub target read failed: {error}"))?;
            Ok((
                target.current_commit,
                target.target_ref,
                target.remote_identity,
                target.freshness_identity,
            ))
        }
    }
}

fn integrated_successor_permitted(
    profile: &ImplementationCampaignDriveProfile,
    campaign: &kiln_core::implementation::campaign::CampaignState,
    current_source: &ExternalRef,
    target_ref: &str,
    target_remote_identity: &ContentId,
    ledgers: &kiln_implementer::campaign_consumer::CanonicalCampaignLedgers,
    implementer: &mut kiln_implementer::Implementer,
) -> Result<Option<ContentId>, String> {
    for attempt in ledgers
        .attempts
        .attempts()
        .filter(|attempt| attempt.phase == kiln_core::implementation::AttemptPhase::Verified)
    {
        let work = ledgers
            .work
            .current()
            .chain(ledgers.work.superseded().iter())
            .find(|work| {
                work.identity()
                    .is_ok_and(|identity| identity == attempt.work_identity)
            });
        let Some(work) = work else { continue };
        if work.repository != campaign.contract.repository
            || !work.aliases.iter().any(|alias| {
                matches!(
                    alias.kind,
                    kiln_core::implementation::WorkSourceKind::CairnTask
                        | kiln_core::implementation::WorkSourceKind::CairnArtifact
                )
            })
        {
            continue;
        }
        let Some(effect) = campaign.pending_effects.values().find(|effect| {
            effect.operation == CampaignOperation::Integrate
                && effect.state
                    == kiln_core::implementation::campaign::EffectState::ObservedComplete
                && effect.progress_applied
                && effect.selection_identity == campaign.selection_identity
                && effect.evidence_identity.is_some()
                && effect.work_identity.as_ref() == Some(&attempt.work_identity)
                && effect.attempt_identity.as_ref() == Some(&attempt.identity)
                && effect.attempt_fence == Some(attempt.fence)
        }) else {
            continue;
        };
        let Some(base) = &work.base else { continue };
        match profile.delivery.route {
            super::delivery::NativeDeliveryRoute::NativeGit => {
                let Some(receipt) = implementer.native_git_receipt(&attempt.identity)? else {
                    continue;
                };
                if receipt.work_identity == attempt.work_identity
                    && receipt.remote_identity == *target_remote_identity
                    && receipt.work_revision == attempt.work_revision
                    && receipt.attempt == attempt.identity
                    && receipt.fence == attempt.fence
                    && receipt.target_ref == target_ref
                    && receipt.expected_old_target == *base
                    && receipt.resulting_commit == *current_source
                {
                    return Ok(Some(effect.effect_id.clone()));
                }
            }
            super::delivery::NativeDeliveryRoute::GithubPullRequest => {
                let Some(receipt) = work_publication_integration(ledgers, &attempt.identity) else {
                    continue;
                };
                let config = profile
                    .delivery
                    .github
                    .as_ref()
                    .ok_or("GitHub source route is not configured")?;
                if receipt.work_identity == attempt.work_identity
                    && receipt.work_revision == attempt.work_revision
                    && receipt.attempt == attempt.identity
                    && receipt.fence == attempt.fence
                    && receipt.base_ref == config.publication_policy.base_branch
                    && receipt.base_head == *base
                    && receipt.merge_commit == *current_source
                {
                    return Ok(Some(effect.effect_id.clone()));
                }
            }
        }
    }
    Ok(None)
}

/// The integration of exactly `attempt` through the selected delivery route,
/// bound to its work identity, revision, attempt and fence.
fn route_integration(
    profile: &ImplementationCampaignDriveProfile,
    ledgers: &kiln_implementer::campaign_consumer::CanonicalCampaignLedgers,
    implementer: &mut kiln_implementer::Implementer,
    attempt: &kiln_core::implementation::ImplementationAttempt,
) -> Result<Option<super::adoption::Integration>, String> {
    match profile.delivery.route {
        super::delivery::NativeDeliveryRoute::NativeGit => {
            let Some(receipt) = implementer.native_git_receipt(&attempt.identity)? else {
                return Ok(None);
            };
            if receipt.work_identity != attempt.work_identity
                || receipt.work_revision != attempt.work_revision
                || receipt.attempt != attempt.identity
                || receipt.fence != attempt.fence
            {
                return Ok(None);
            }
            let Some(evidence) = super::delivery::native_git_evidence(
                profile,
                &attempt.identity,
                attempt.fence,
                &attempt.work_identity,
            ) else {
                return Ok(None);
            };
            Ok(Some(super::adoption::Integration {
                commit: receipt.resulting_commit.native_id.as_str().to_owned(),
                evidence,
            }))
        }
        super::delivery::NativeDeliveryRoute::GithubPullRequest => {
            let Some(receipt) = work_publication_integration(ledgers, &attempt.identity) else {
                return Ok(None);
            };
            if receipt.work_identity != attempt.work_identity
                || receipt.work_revision != attempt.work_revision
                || receipt.attempt != attempt.identity
                || receipt.fence != attempt.fence
            {
                return Ok(None);
            }
            let record = attempt
                .publication
                .as_ref()
                .ok_or("integrated attempt lost its publication record")?;
            Ok(Some(super::adoption::Integration {
                commit: receipt.merge_commit.native_id.as_str().to_owned(),
                evidence: hash(&serde_json::to_vec(record).map_err(text)?)?,
            }))
        }
    }
}

fn work_publication_integration<'a>(
    ledgers: &'a kiln_implementer::campaign_consumer::CanonicalCampaignLedgers,
    identity: &ContentId,
) -> Option<&'a kiln_core::implementation::IntegrationReceipt> {
    ledgers
        .attempts
        .get(identity)?
        .publication
        .as_ref()?
        .integration
        .as_ref()
}

fn next_work_revision(ledger: &kiln_core::implementation::WorkLedger) -> Result<u64, String> {
    ledger
        .current()
        .chain(ledger.superseded().iter())
        .map(|work| work.revision)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| "canonical implementation work revision is exhausted".into())
}

fn retain_work_revision(
    implementer: &mut kiln_implementer::Implementer,
    work: &mut kiln_core::implementation::ImplementationWork,
) -> Result<(), String> {
    let ledger = implementer.work()?;
    let key = work.canonical_key().map_err(show)?;
    let previous = ledger
        .current()
        .chain(ledger.superseded().iter())
        .filter(|row| row.canonical_key().is_ok_and(|candidate| candidate == key))
        .max_by_key(|row| row.revision);
    if let Some(previous) = previous {
        let revision = previous.revision;
        work.revision = revision;
        if previous.identity().map_err(show)? != work.identity().map_err(show)? {
            work.revision = revision
                .checked_add(1)
                .ok_or("canonical implementation work revision is exhausted")?;
        }
    } else if work.revision == 0 {
        work.revision = 1;
    }
    Ok(())
}

fn bound_native_context_identity(
    profile: &ContentId,
    context: &ContentId,
    remote: &ContentId,
    target_ref: &str,
    commit: &ExternalRef,
    freshness: &ContentId,
) -> Result<ContentId, String> {
    hash(
        &serde_json::to_vec(&(profile, context, remote, target_ref, commit, freshness))
            .map_err(text)?,
    )
}

fn hash(bytes: &[u8]) -> Result<ContentId, String> {
    id(blake3::hash(bytes).to_hex().as_str())
}

#[cfg(test)]
mod tests {
    use super::{native_dependencies, native_ordered_unit, native_prompt_profile, parse_source};
    use kiln_core::implementation::WorkSourceKind;
    use serde_json::json;

    #[test]
    fn native_prompt_binding_refuses_ambiguous_harness_authoring() {
        let mut seed = json!({"animus_implementation":{"prompt":"base"}});
        native_prompt_profile(&mut seed).unwrap().insert("prompt".into(), json!("bound"));
        assert_eq!(seed.pointer("/animus_implementation/prompt"), Some(&json!("bound")));
        seed["omp"] = json!({"prompt":"second route"});
        assert!(native_prompt_profile(&mut seed).is_err());
    }

    #[test]
    fn task_and_artifact_source_identities_do_not_masquerade() {
        assert_eq!(
            parse_source(WorkSourceKind::CairnTask, "change-1/task:build"),
            Ok(("change-1", "tasks", "task:build"))
        );
        assert_eq!(
            parse_source(WorkSourceKind::CairnArtifact, "change-1/design/design"),
            Ok(("change-1", "design", "design"))
        );
        assert!(parse_source(WorkSourceKind::CairnTask, "change-1/design/design").is_err());
        assert!(parse_source(WorkSourceKind::CairnArtifact, "change-1/task:build").is_err());
    }

    #[test]
    fn native_dependencies_keep_unsatisfied_edges_only() {
        let graph = json!({ "edges": [
            { "dependent": "selected", "dependency": "blocked", "satisfied": false },
            { "dependent": "selected", "dependency": "satisfied", "satisfied": true },
            { "dependent": "other", "dependency": "ignored", "satisfied": false }
        ]});
        assert_eq!(
            native_dependencies(&graph, "selected", &[]).unwrap_or_default(),
            vec!["blocked/*"]
        );
        assert!(native_dependencies(&json!({}), "selected", &[]).is_err());
    }

    #[test]
    fn whole_change_selects_first_native_ordered_admitted_unit_only() {
        let id = kiln_core::types::ContentId::from_blake3_hex("a".repeat(64))
            .unwrap_or_else(|_| unreachable!());
        let snapshot = kiln_adapter_cairn::campaign::CairnCampaignSnapshot {
            change: "change-a".into(),
            executable_identity: id.clone(),
            policy_identity: id.clone(),
            graph: json!({}),
            workflow_status: json!({}),
            workflow_instructions: Default::default(),
            readiness: json!({}),
            implementation_ready: json!({"selection": [
                {"change":"change-a", "unit":"task:blocked"},
                {"change":"change-a", "unit":"task:ready"},
                {"change":"other", "unit":"task:foreign"}
            ]}),
            context_identity: id,
        };
        let admitted = std::collections::BTreeSet::from(["change-a/task:ready".to_owned()]);
        assert_eq!(
            native_ordered_unit(&snapshot, "change-a", |native| admitted.contains(native)),
            Some("change-a/task:ready".into())
        );
        assert_eq!(native_ordered_unit(&snapshot, "change-a", |_| false), None);
        assert_eq!(
            native_ordered_unit(&snapshot, "other", |_| true),
            Some("other/task:foreign".into())
        );
    }
}
