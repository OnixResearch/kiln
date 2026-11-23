//! High-level intent-before-effect campaign continuation using the guarded
//! native Lattice worker, canonical attempt read-back, and canonical CI Engine.
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

use bounded_exec::ExecutionLimits;
use kiln_adapter_cairn::{
    BoundedCairnRuntime, CairnProcessOutput, CairnRuntime, ProcessInvocation,
};

use kiln_core::{
    implementation::{
        AttemptPhase, ImplementationWork,
        campaign::{
            CampaignOperation, CampaignState, EffectState, MAX_ID_BYTES, MemberObservation,
            MemberStage, RecoveryDisposition,
        },
        continuation::ContinuationPolicy,
    },
    types::ContentId,
};
use kiln_implementer::{
    campaign::{
        CampaignClaim, CampaignDriveShell, CampaignEffectConsumer, CampaignOwnerObservation,
        CampaignStore, drive_bounded,
    },
    campaign_consumer::{CampaignClaimOutcome, CanonicalCampaignLedgers},
};
use serde::{Deserialize, Serialize};

mod adoption;
mod binding;
mod context;
mod continuation;
mod delivery;
mod observation;
mod workspace;

pub use continuation::{continue_cli_args, wake_cli_args};
pub(crate) use delivery::{NativeDeliveryRoute, route_supports_goal};

const MAX_VERIFICATION_ROUTES: usize = 64;
const MAX_DRIVE_OPERATIONS: u32 = 128;
const MAX_ROLE_ARGUMENTS: usize = 32;
const MAX_ROLE_ARGUMENT_BYTES: usize = 4096;
const MAX_ROLE_PROGRAM_BYTES: u64 = 512 * 1024 * 1024;
const MAX_LAUNCHER_BYTES: u64 = 16 * 1024 * 1024;
const ROLE_TIMEOUT_MS: u64 = 3_600_000;
const ROLE_STDOUT_BYTES: usize = 4 * 1024 * 1024;
const ROLE_STDERR_BYTES: usize = 65_536;
const ROLE_POLL_MS: u64 = 25;
const ROLE_TEARDOWN_MS: u64 = 1_000;
const PROFILE_SCHEMA: &str = "kiln.implementation-campaign-drive.v1";
const MAX_PROFILE_BYTES: u64 = 1_048_576;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleProgram {
    pub program: PathBuf,
    #[serde(default)]
    pub arguments: Vec<String>,
    pub blake3: ContentId,
    pub uid: u32,
    pub gid: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateExportProfile {
    pub output_parent: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleLauncher {
    pub setpriv: PathBuf,
    pub setpriv_blake3: ContentId,
    pub worker: RoleProgram,
    pub exporter: RoleProgram,
    pub capture: RoleProgram,
    pub verifier: RoleProgram,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationRoute {
    pub ci_service_profile: PathBuf,
    pub verification_profile: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImplementationCampaignDriveProfile {
    pub schema: String,
    pub campaign_id: String,
    pub campaign_database: PathBuf,
    pub implementation_database: PathBuf,
    pub verification_routes: std::collections::BTreeMap<ContentId, VerificationRoute>,
    pub verifier_authority: PathBuf,
    pub bound_input_directory: PathBuf,
    /// Template paths are rebound to the selected canonical attempt into the
    /// private bound-input directory before dispatch.
    pub worker: kiln_adapter_lattice::worker::WorkerInvocation,
    pub candidate_export: CandidateExportProfile,
    pub roles: RoleLauncher,
    pub native_cairn: observation::NativeCairnProfile,
    pub delivery: delivery::NativeDeliveryProfile,
    #[serde(default)]
    pub decision_contexts: std::collections::BTreeMap<ContentId, PathBuf>,
    pub claim_lease_ms: u64,
    pub maximum_operations: u32,
    pub workspace: workspace::WorkspaceProfile,
    /// Continuation pacing; absent profiles use the named defaults.
    #[serde(default)]
    pub continuation: Option<ContinuationPolicy>,
    /// Optional renews/iroh swarm claim mirror; absent or disabled is inert.
    #[serde(default)]
    pub swarm: Option<kiln_adapter_swarm::SwarmConfig>,
    /// Explicit local-operator profile selection for protected campaign effects.
    #[serde(default)]
    pub local_operator: Option<String>,
}
impl ImplementationCampaignDriveProfile {
    /// Select the explicitly configured local-operator profile, if any.
    ///
    /// # Errors
    /// Returns a message when the operator identity is invalid.
    pub(super) fn configure_authority(
        &self,
        implementer: &mut kiln_implementer::Implementer,
    ) -> Result<(), String> {
        if let Some(operator) = &self.local_operator {
            implementer.select_local_operator(operator.clone())?;
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != PROFILE_SCHEMA
            || self.campaign_id.trim().is_empty()
            || self.campaign_id.len() > MAX_ID_BYTES
            || !self.campaign_database.is_absolute()
            || !self.implementation_database.is_absolute()
            || !self.bound_input_directory.is_absolute()
        {
            return Err(
                "campaign drive profile schema, identity, or database paths are invalid".into(),
            );
        }
        let paths = [
            &self.worker.database,
            &self.worker.enrollment,
            &self.worker.dispatch_plan,
            &self.worker.request,
            &self.worker.transport_profile,
            &self.verifier_authority,
            &self.candidate_export.output_parent,
            &self.roles.setpriv,
            &self.roles.worker.program,
            &self.roles.exporter.program,
            &self.roles.capture.program,
            &self.roles.verifier.program,
        ];
        if paths.iter().any(|path| !path.is_absolute()) {
            return Err("campaign drive profile inputs must be absolute paths".into());
        }
        if self.verification_routes.is_empty()
            || self.verification_routes.len() > MAX_VERIFICATION_ROUTES
            || self.verification_routes.iter().any(|(identity, route)| {
                identity.validate().is_err()
                    || !route.ci_service_profile.is_absolute()
                    || !route.verification_profile.is_absolute()
            })
        {
            return Err("campaign requires bounded exact frozen verification routes".into());
        }
        if self.worker.database != self.implementation_database {
            return Err("campaign worker and canonical owner databases must be identical".into());
        }
        if self
            .worker
            .admission_observations
            .as_ref()
            .is_some_and(|path| !path.is_absolute())
        {
            return Err("campaign admission observation path must be absolute".into());
        }
        if self.roles.worker.uid == 0
            || self.roles.capture.uid == 0
            || self.roles.exporter.uid != self.roles.worker.uid
            || self.roles.verifier.uid != self.roles.capture.uid
            || self.roles.worker.uid == self.roles.verifier.uid
        {
            return Err("export uses the trusted worker UID; capture and verification require one distinct non-root verifier UID".into());
        }
        let gid = self.roles.worker.gid;
        if gid == 0
            || [
                &self.roles.exporter,
                &self.roles.capture,
                &self.roles.verifier,
            ]
            .iter()
            .any(|role| role.gid != gid)
        {
            return Err("campaign roles require one non-root shared data group".into());
        }
        if self.maximum_operations == 0 || self.maximum_operations > MAX_DRIVE_OPERATIONS {
            return Err("campaign operation limit is empty or exceeds its bound".into());
        }
        if self.claim_lease_ms == 0 {
            return Err("campaign claim lease must be finite and positive".into());
        }
        if let Some(policy) = &self.continuation {
            policy.validate().map_err(show)?;
        }
        if !self.roles.worker.arguments.is_empty()
            || self.roles.exporter.arguments != ["implementation-attempt", "export-candidate"]
            || self.roles.capture.arguments != ["capture-candidate"]
            || self.roles.verifier.arguments != ["verify-implementation"]
        {
            return Err(
                "campaign role command prefixes do not select the admitted native owner operations"
                    .into(),
            );
        }
        context::validate(self)?;
        workspace::validate(&self.workspace)?;
        observation::validate(self)?;
        delivery::validate(self)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DriveArguments {
    profile: PathBuf,
    campaign_id: String,
    expected_version: u64,
}

const CLI_USAGE: &str = "usage: kiln-web-service drive-implementation-campaign --profile PROFILE --campaign-id ID --expected-version N";

pub fn run_cli_args(arguments: &[String]) -> Result<String, String> {
    let [
        profile_flag,
        profile,
        campaign_flag,
        campaign_id,
        version_flag,
        expected_version,
    ] = arguments
    else {
        return Err(CLI_USAGE.into());
    };
    if profile_flag != "--profile"
        || campaign_flag != "--campaign-id"
        || version_flag != "--expected-version"
    {
        return Err(CLI_USAGE.into());
    }
    run(DriveArguments {
        profile: PathBuf::from(profile),
        campaign_id: campaign_id.clone(),
        expected_version: expected_version
            .parse()
            .map_err(|_| "expected version must be an unsigned integer")?,
    })
}

fn read_profile(path: &Path) -> Result<ImplementationCampaignDriveProfile, String> {
    let profile: ImplementationCampaignDriveProfile = read_json(path)?;
    profile.validate()?;
    Ok(profile)
}

fn run(args: DriveArguments) -> Result<String, String> {
    let (mut store, mut consumer) = open_native(
        &args.profile,
        &args.campaign_id,
        Some(args.expected_version),
    )?;
    if store.version(&args.campaign_id)? != Some(args.expected_version) {
        return Err("stale campaign version; no canonical work was claimed".into());
    }
    let maximum_operations = consumer.profile.maximum_operations;
    let drive = drive_bounded(
        &mut store,
        &args.campaign_id,
        args.expected_version,
        maximum_operations,
        &mut consumer,
    )?;
    let evaluation = drive.state.evaluate();
    serde_json::to_string(&serde_json::json!({
        "campaign": drive.state,
        "version": drive.version,
        "decision": evaluation.decision,
        "basis": evaluation.basis,
        "holds": evaluation.holds,
        "steps": drive.steps,
        "refusal": drive.refusal,
    }))
    .map_err(text)
}

/// Profile, role, campaign and canonical-ledger preflight shared by the drive
/// and continuation commands. A stale expected version fails before the
/// campaign store is opened for writing.
fn open_native(
    profile: &Path,
    campaign_id: &str,
    expected_version: Option<u64>,
) -> Result<(CampaignStore, NativeCampaignConsumer), String> {
    if campaign_id.trim().is_empty() || campaign_id.len() > MAX_ID_BYTES {
        return Err("campaign identity is malformed".into());
    }
    let profile = read_profile(profile)?;
    if campaign_id != profile.campaign_id {
        return Err("campaign id differs from the typed drive profile".into());
    }
    preflight_roles(&profile)?;
    let existing =
        kiln_implementer::campaign::CampaignObservation::open(&profile.campaign_database)?;
    let (_, existing_version) = existing
        .inspect(campaign_id)?
        .ok_or("canonical campaign is not initialized")?;
    if expected_version.is_some_and(|expected| expected != existing_version) {
        return Err("stale campaign version; no canonical database was mutated".into());
    }
    CanonicalCampaignLedgers::read(&profile.implementation_database)?;
    let contexts = context::load(&profile)?;
    let store = CampaignStore::open(&profile.campaign_database)?;
    Ok((
        store,
        NativeCampaignConsumer {
            profile,
            contexts,
            worker: None,
        },
    ))
}

struct NativeCampaignConsumer {
    profile: ImplementationCampaignDriveProfile,
    contexts: context::Projections,
    worker: Option<kiln_adapter_lattice::worker::WorkerInvocation>,
}

impl CampaignDriveShell for NativeCampaignConsumer {
    fn refresh(
        &mut self,
        store: &mut CampaignStore,
        campaign_id: &str,
        version: u64,
    ) -> Result<u64, String> {
        observation::refresh(&self.profile, &self.contexts, store, campaign_id, version)
    }

    fn recover_executing(
        &mut self,
        store: &mut CampaignStore,
        campaign_id: &str,
        version: u64,
        state: &CampaignState,
        member_key: &str,
    ) -> Result<Option<u64>, String> {
        let Some(observation) = state
            .observations
            .get(member_key)
            .filter(|o| o.stage == MemberStage::Executing)
        else {
            return Ok(None);
        };
        let (Some(work_id), Some(attempt_id), Some(fence)) = (
            &observation.work_identity,
            &observation.attempt_identity,
            observation.attempt_fence,
        ) else {
            return Ok(None);
        };
        let owner = CanonicalCampaignLedgers::read(&self.profile.implementation_database)?;
        if !owner.attempts.dispatch_requires_recovery(attempt_id) {
            return Ok(None);
        }
        if !owner.attempts.attempts().any(|attempt| {
            attempt.identity == *attempt_id
                && attempt.fence == fence
                && attempt.work_identity == *work_id
        }) {
            return Err("executing campaign attempt binding disappeared".into());
        }
        let intent = reconcile_intent_id(&state.selection_identity, member_key, attempt_id, fence)?;
        store.persist_effect_intent(
            campaign_id,
            version,
            member_key.to_owned(),
            CampaignOperation::Reconcile,
            "lattice-worker-owner".into(),
            intent,
        )?;
        let version = store
            .version(campaign_id)?
            .ok_or("campaign disappeared after reconcile intent")?;
        Ok(Some(version))
    }

    fn claim(
        &mut self,
        store: &mut CampaignStore,
        campaign_id: &str,
        mut version: u64,
        state: &CampaignState,
        member_key: &str,
    ) -> Result<CampaignClaim, String> {
        let profile = &self.profile;
        let observation = state
            .observations
            .get(member_key)
            .ok_or("campaign member observation unavailable")?;
        let work_id = observation
            .work_identity
            .as_ref()
            .ok_or("ready campaign member has no canonical work identity")?;
        let enrollment = read_json(&profile.worker.enrollment)?;
        let mut owner =
            kiln_implementer::Implementer::open(&profile.implementation_database, enrollment)?;
        profile.configure_authority(&mut owner)?;
        let now_ms = wall_clock_ms()?;
        attach_swarm_mirror(profile, &mut owner, now_ms)?;
        let work = owner
            .work()?
            .current()
            .find(|work| work.identity().is_ok_and(|id| &id == work_id))
            .cloned()
            .ok_or("selected work ceased to be current")?;
        binding::attach_work_needs(profile, &work, &mut owner)?;
        let attempt = match owner.claim_campaign_work(
            work_id,
            &state.contract.parent_budget_id,
            now_ms,
            profile.claim_lease_ms,
        )? {
            CampaignClaimOutcome::Claimed(attempt) => *attempt,
            CampaignClaimOutcome::NotWinner { selected } => {
                return Ok(CampaignClaim::Refused(format!(
                    "campaign work is not the canonical admission winner ({selected:?}); no work was claimed"
                )));
            }
            CampaignClaimOutcome::WriterBusy => {
                return Ok(CampaignClaim::Refused(
                    "repository already has an active or unreconciled writer; no work was claimed"
                        .into(),
                ));
            }
            CampaignClaimOutcome::Overlapped { cause } => {
                return Ok(CampaignClaim::Refused(cause));
            }
        };
        if observation.attempt_identity.as_ref() != Some(&attempt.identity)
            || observation.attempt_fence != Some(attempt.fence)
        {
            let mut bound = observation.clone();
            bound.attempt_identity = Some(attempt.identity.clone());
            bound.attempt_fence = Some(attempt.fence);
            store.observe(campaign_id, version, bound)?;
            version = store
                .version(campaign_id)?
                .ok_or("campaign disappeared after exact attempt binding")?;
        }
        context::retain(
            store,
            campaign_id,
            version,
            state,
            member_key,
            &attempt,
            &self.contexts,
        )
        .map(CampaignClaim::Bound)
    }
}

impl NativeCampaignConsumer {
    fn consume(
        &mut self,
        effect: &kiln_core::implementation::campaign::PendingEffect,
        reconciliation: bool,
    ) -> Result<Option<CampaignOwnerObservation>, String> {
        if matches!(
            effect.operation,
            CampaignOperation::Publish
                | CampaignOperation::Integrate
                | CampaignOperation::Lifecycle
        ) {
            return delivery::consume(&self.profile, effect, reconciliation);
        }
        if !matches!(
            effect.operation,
            CampaignOperation::AuthorArtifact
                | CampaignOperation::RepairArtifact
                | CampaignOperation::Code
                | CampaignOperation::RepairCode
                | CampaignOperation::RetryInfrastructure
                | CampaignOperation::Verify
                | CampaignOperation::Reconcile
        ) {
            return Err(format!(
                "campaign operation {:?} has no supported concrete owner in this controller profile",
                effect.operation
            ));
        }
        let (Some(work_id), Some(attempt_id), Some(fence)) = (
            &effect.work_identity,
            &effect.attempt_identity,
            effect.attempt_fence,
        ) else {
            return Err("campaign effect lacks its canonical work/attempt/fence binding".into());
        };
        let before = CanonicalCampaignLedgers::read(&self.profile.implementation_database)?;
        let work = before
            .work
            .current()
            .find(|work| work.identity().is_ok_and(|id| &id == work_id))
            .ok_or("campaign work version is no longer current")?;
        let attempt = before
            .attempts
            .attempts()
            .find(|attempt| &attempt.identity == attempt_id)
            .ok_or("campaign attempt is not retained")?;
        if attempt.work_identity != *work_id || attempt.fence != fence {
            return Err("campaign owner binding differs from current work/attempt/fence".into());
        }
        if effect.operation != CampaignOperation::Verify {
            self.worker = if reconciliation {
                binding::restore(&self.profile, work, attempt, effect)?
            } else {
                Some(binding::bind(&self.profile, work, attempt, effect)?)
            };
            if self.worker.is_none() {
                return Ok(None);
            }
        }
        if reconciliation {
            if effect.operation == CampaignOperation::Verify {
                self.verify(work, attempt_id, fence)?;
            } else if attempt.handoff.is_none() && attempt.verification.is_none() {
                if !before.attempts.dispatch_requires_recovery(attempt_id)
                    && attempt.workflow.is_none()
                {
                    return Ok(None);
                }
                self.run_worker(true)?;
                self.export_and_capture(work, attempt)?;
            }
        } else {
            match effect.operation {
                CampaignOperation::Verify => self.verify(work, attempt_id, fence)?,
                CampaignOperation::AuthorArtifact
                | CampaignOperation::RepairArtifact
                | CampaignOperation::Code
                | CampaignOperation::RepairCode
                | CampaignOperation::RetryInfrastructure => {
                    if !matches!(
                        attempt.phase,
                        AttemptPhase::Claimed | AttemptPhase::Executing
                    ) {
                        return Err("campaign coding dispatch requires the current claimed/executing attempt".into());
                    }
                    self.run_worker(false)?;
                    self.export_and_capture(work, attempt)?;
                }
                CampaignOperation::Reconcile => {
                    return Err("reconcile intent was dispatched instead of read back".into());
                }
                _ => return Err("unsupported campaign operation".into()),
            }
        }
        let after = CanonicalCampaignLedgers::read(&self.profile.implementation_database)?;
        let attempt = after
            .attempts
            .attempts()
            .find(|attempt| &attempt.identity == attempt_id)
            .ok_or("canonical attempt disappeared after owner operation")?;
        if attempt.work_identity != *work_id || attempt.fence != fence {
            return Err("canonical owner read-back changed work or attempt fence".into());
        }
        if attempt.workflow.is_none() && attempt.handoff.is_none() && attempt.verification.is_none()
        {
            return Ok(None);
        }
        let member = self.member_from_owner(&effect.member, work_id, attempt, &after)?;
        let evidence_identity = id(blake3::hash(&serde_json::to_vec(attempt).map_err(text)?)
            .to_hex()
            .as_str())?;
        let result = if matches!(
            attempt.phase,
            AttemptPhase::Blocked
                | AttemptPhase::Failed
                | AttemptPhase::Cancelled
                | AttemptPhase::Superseded
        ) {
            EffectState::ObservedFailed
        } else {
            EffectState::ObservedComplete
        };
        Ok(Some(CampaignOwnerObservation {
            result,
            evidence_identity,
            member,
        }))
    }

    fn run_worker(&self, reconciliation: bool) -> Result<(), String> {
        let worker = self
            .worker
            .as_ref()
            .ok_or("native worker binding is unavailable")?;
        let mut args = vec![
            OsString::from("--database"),
            worker.database.as_os_str().to_owned(),
            OsString::from("--enrollment"),
            worker.enrollment.as_os_str().to_owned(),
            OsString::from("--dispatch-plan"),
            worker.dispatch_plan.as_os_str().to_owned(),
            OsString::from("--worker-request"),
            worker.request.as_os_str().to_owned(),
            OsString::from("--worker-transport-profile"),
            worker.transport_profile.as_os_str().to_owned(),
        ];
        if let Some(path) = &worker.admission_observations {
            args.extend([
                OsString::from("--admission-observations"),
                path.as_os_str().to_owned(),
            ]);
        }
        if reconciliation {
            args.extend([OsString::from("--recover"), OsString::from("true")]);
        }
        run_role(
            &self.profile,
            &self.profile.roles.worker,
            &self.profile.bound_input_directory,
            &args,
        )?;
        Ok(())
    }

    fn export_and_capture(
        &self,
        work: &ImplementationWork,
        attempt: &kiln_core::implementation::ImplementationAttempt,
    ) -> Result<(), String> {
        let worker = self
            .worker
            .as_ref()
            .ok_or("native export request binding is unavailable")?;
        let output = self.profile.candidate_export.output_parent.join(format!(
            "{}-{}",
            attempt.identity.as_str(),
            attempt.fence
        ));
        let candidate_root = output.join("candidate");
        let host_receipt = output.join("host-receipt.json");
        let transport: kiln_adapter_lattice::worker::WorkerTransportProfile =
            read_json(&worker.transport_profile)?;
        let mut owner = kiln_implementer::Implementer::open(
            &self.profile.implementation_database,
            read_json(&worker.enrollment)?,
        )?;
        let launch = owner.dispatch_launch(&attempt.identity)?;
        let sent = owner.dispatch_request(&attempt.identity)?;
        let (request, state) = kiln_adapter_lattice::worker::sent_request_and_state(
            launch.as_ref(),
            sent.as_ref(),
            &transport,
            &fs::read(&worker.request).map_err(text)?,
        )?;
        let raw = run_role(
            &self.profile,
            &self.profile.roles.exporter,
            &self.profile.bound_input_directory,
            &[
                OsString::from("--request"),
                request.as_os_str().to_owned(),
                OsString::from("--state"),
                state.as_os_str().to_owned(),
                OsString::from("--output-parent"),
                self.profile
                    .candidate_export
                    .output_parent
                    .as_os_str()
                    .to_owned(),
            ],
        )?;
        if raw.is_empty() || raw.len() as u64 > MAX_PROFILE_BYTES {
            return Err("native candidate export is empty or exceeds its bound".into());
        }
        let export: serde_json::Value = serde_json::from_slice(&raw).map_err(text)?;
        if export.get("schema").and_then(serde_json::Value::as_str)
            != Some("lattice.implementation-candidate-export.v1")
            || export.get("accepted").and_then(serde_json::Value::as_bool) != Some(false)
            || export
                .get("candidate_root")
                .and_then(serde_json::Value::as_str)
                != Some(
                    candidate_root
                        .to_str()
                        .ok_or("candidate export path is not UTF-8")?,
                )
            || export
                .get("host_receipt")
                .and_then(serde_json::Value::as_str)
                != Some(
                    host_receipt
                        .to_str()
                        .ok_or("candidate receipt path is not UTF-8")?,
                )
        {
            return Err(
                "native candidate exporter returned an unexpected or accepted result".into(),
            );
        }
        validate_export_tree(
            &output,
            &candidate_root,
            &host_receipt,
            &output.join("native-observation.json"),
            &self.profile.roles.exporter,
        )?;
        let route = verification_route(&self.profile, work)?;
        let handoff_output = self.profile.bound_input_directory.join(format!(
            "campaign-{}.handoff.json",
            attempt.identity.as_str()
        ));
        let capture_args = vec![
            OsString::from("--database"),
            self.profile.implementation_database.as_os_str().to_owned(),
            OsString::from("--enrollment"),
            worker.enrollment.as_os_str().to_owned(),
            OsString::from("--attempt"),
            OsString::from(attempt.identity.as_str()),
            OsString::from("--fence"),
            OsString::from(attempt.fence.to_string()),
            OsString::from("--dispatch-plan"),
            worker.dispatch_plan.as_os_str().to_owned(),
            OsString::from("--profile"),
            route.verification_profile.as_os_str().to_owned(),
            OsString::from("--authority"),
            self.profile.verifier_authority.as_os_str().to_owned(),
            OsString::from("--candidate-root"),
            candidate_root.as_os_str().to_owned(),
            OsString::from("--host-receipt"),
            host_receipt.as_os_str().to_owned(),
            OsString::from("--handoff-output"),
            handoff_output.as_os_str().to_owned(),
        ];
        run_role(
            &self.profile,
            &self.profile.roles.capture,
            &candidate_root,
            &capture_args,
        )?;
        Ok(())
    }

    fn verify(
        &self,
        work: &ImplementationWork,
        attempt: &ContentId,
        fence: u64,
    ) -> Result<(), String> {
        let route = verification_route(&self.profile, work)?;
        let args = [
            "--profile",
            route
                .ci_service_profile
                .to_str()
                .ok_or("CI profile path is not UTF-8")?,
            "--database",
            self.profile
                .implementation_database
                .to_str()
                .ok_or("implementation database path is not UTF-8")?,
            "--enrollment",
            self.profile
                .worker
                .enrollment
                .to_str()
                .ok_or("enrollment path is not UTF-8")?,
            "--attempt",
            attempt.as_str(),
            "--fence",
            &fence.to_string(),
            "--verification-profile",
            route
                .verification_profile
                .to_str()
                .ok_or("verification profile path is not UTF-8")?,
            "--authority",
            self.profile
                .verifier_authority
                .to_str()
                .ok_or("verifier authority path is not UTF-8")?,
        ]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
        run_role(
            &self.profile,
            &self.profile.roles.verifier,
            &self.profile.bound_input_directory,
            &args,
        )?;
        Ok(())
    }

    fn member_from_owner(
        &self,
        member_key: &str,
        work_id: &ContentId,
        attempt: &kiln_core::implementation::ImplementationAttempt,
        ledgers: &CanonicalCampaignLedgers,
    ) -> Result<MemberObservation, String> {
        let observer =
            kiln_implementer::campaign::CampaignObservation::open(&self.profile.campaign_database)?;
        let (state, _) = observer
            .inspect(&self.profile.campaign_id)?
            .ok_or("campaign state unavailable")?;
        let mut member = state
            .observations
            .get(member_key)
            .cloned()
            .ok_or("campaign member observation unavailable")?;
        let work = ledgers
            .work
            .current()
            .find(|work| work.identity().is_ok_and(|id| &id == work_id))
            .ok_or("canonical work disappeared")?;
        let binding = selected_member_binding(&state.contract, member_key)?;
        member.work_identity = Some(work_id.clone());
        member.canonical_work_key = Some(work.canonical_key().map_err(show)?);
        member.attempt_identity = Some(attempt.identity.clone());
        member.attempt_fence = Some(attempt.fence);
        member.source_identity = work
            .aliases
            .iter()
            .find(|alias| {
                matches!(
                    alias.kind,
                    kiln_core::implementation::WorkSourceKind::CairnTask
                        | kiln_core::implementation::WorkSourceKind::CairnArtifact
                )
            })
            .map(|alias| alias.version.clone());
        member.subject_identity = attempt
            .handoff
            .as_ref()
            .map(|handoff| {
                kiln_core::implementation::admit_candidate_handoff(work, attempt, handoff)
                    .map_err(show)
            })
            .transpose()?;
        let verified = attempt.phase == AttemptPhase::Verified && attempt.verification.is_some();
        member.stage = if verified {
            MemberStage::VerifiedCandidate
        } else if matches!(
            attempt.phase,
            AttemptPhase::Blocked
                | AttemptPhase::Failed
                | AttemptPhase::Cancelled
                | AttemptPhase::Superseded
        ) {
            MemberStage::Blocked
        } else if attempt.handoff.is_some() {
            MemberStage::Candidate
        } else if attempt.phase == AttemptPhase::Executing {
            MemberStage::Executing
        } else if attempt.phase == AttemptPhase::Claimed {
            MemberStage::Claimed
        } else {
            MemberStage::Blocked
        };
        member.candidate_freshness = if attempt.handoff.is_some() {
            kiln_core::implementation::campaign::Freshness::Current
        } else {
            kiln_core::implementation::campaign::Freshness::Unknown
        };
        member.verifier_freshness = if verified {
            kiln_core::implementation::campaign::Freshness::Current
        } else if attempt.verification.is_some() {
            kiln_core::implementation::campaign::Freshness::Stale
        } else {
            kiln_core::implementation::campaign::Freshness::Unknown
        };
        member.required_checks_satisfied = verified
            && attempt.verification.as_ref().is_some_and(|verification| {
                required_checks_present(&binding.required_checks, &verification.run.checks)
            });
        member.delivery_route_supported = Some(delivery::route_supports_goal(
            self.profile.delivery.route,
            binding.goal,
        ));
        member.recovery = if matches!(attempt.phase, AttemptPhase::Blocked | AttemptPhase::Failed) {
            Some(if attempt.verification.is_some() {
                RecoveryDisposition::CodingRepair
            } else {
                RecoveryDisposition::SemanticReplan
            })
        } else {
            None
        };
        if let Some(verification) = &attempt.verification {
            for check in &verification.run.checks {
                if let Some(identity) = check
                    .artifact_digest
                    .as_deref()
                    .and_then(|value| ContentId::from_blake3_hex(value.to_owned()).ok())
                    && !member.evidence.contains(&identity)
                {
                    member.evidence.push(identity);
                }
            }
        }
        retain_unit_milestone(binding.goal, &mut member, work);
        Ok(member)
    }
}

fn required_checks_present(
    required: &[String],
    checks: &[kiln_core::change_review::CiCheck],
) -> bool {
    required.iter().all(|name| {
        checks
            .iter()
            .any(|check| check.name == *name && check.outcome == "pass")
    })
}

fn selected_member_goal(
    contract: &kiln_core::implementation::campaign::CampaignContract,
    member: &str,
) -> Result<kiln_core::implementation::campaign::CompletionGoal, String> {
    Ok(selected_member_binding(contract, member)?.goal)
}

fn selected_member_binding<'a>(
    contract: &'a kiln_core::implementation::campaign::CampaignContract,
    member: &str,
) -> Result<&'a kiln_core::implementation::campaign::MemberBinding, String> {
    let (change_key, unit_key) = member
        .split_once('/')
        .ok_or("campaign member key is malformed")?;
    contract
        .members
        .iter()
        .find(|binding| match &binding.selector {
            kiln_core::implementation::campaign::MemberSelector::WholeChange { change } => {
                change == change_key && unit_key == "*"
            }
            kiln_core::implementation::campaign::MemberSelector::Tasks { change, tasks } => {
                change == change_key && unit_key.split(',').eq(tasks.iter().map(String::as_str))
            }
        })
        .ok_or_else(|| "campaign member is outside the frozen selection".into())
}

fn retain_unit_milestone(
    goal: kiln_core::implementation::campaign::CompletionGoal,
    member: &mut MemberObservation,
    work: &ImplementationWork,
) {
    if goal
        .unit_goal(!member.remaining_obligations.is_empty())
        .reached_by(member.stage)
    {
        for alias in &work.aliases {
            if matches!(
                alias.kind,
                kiln_core::implementation::WorkSourceKind::CairnTask
                    | kiln_core::implementation::WorkSourceKind::CairnArtifact
            ) {
                member.remaining_obligations.remove(&alias.native_id);
            }
        }
    }
}

impl CampaignEffectConsumer for NativeCampaignConsumer {
    fn dispatch(
        &mut self,
        effect: &kiln_core::implementation::campaign::PendingEffect,
    ) -> Result<Option<CampaignOwnerObservation>, String> {
        self.consume(effect, false)
    }
    fn reconcile(
        &mut self,
        effect: &kiln_core::implementation::campaign::PendingEffect,
    ) -> Result<Option<CampaignOwnerObservation>, String> {
        self.consume(effect, true)
    }
}

fn reconcile_intent_id(
    selection: &ContentId,
    member: &str,
    attempt: &ContentId,
    fence: u64,
) -> Result<ContentId, String> {
    let bytes = serde_json::to_vec(&(
        "campaign-lattice-reconcile-v1",
        selection,
        member,
        attempt,
        fence,
    ))
    .map_err(text)?;
    id(blake3::hash(&bytes).to_hex().as_str())
}

fn verification_route<'a>(
    profile: &'a ImplementationCampaignDriveProfile,
    work: &ImplementationWork,
) -> Result<&'a VerificationRoute, String> {
    let route = profile
        .verification_routes
        .get(&work.verification_profile)
        .ok_or("selected stage has no admitted frozen verification route")?;
    let frozen: kiln_implementer::verification::VerificationProfile =
        read_json(&route.verification_profile)?;
    if frozen.repository != work.repository || frozen.identity()? != work.verification_profile {
        return Err("stage verifier route differs from the exact canonical work profile".into());
    }
    Ok(route)
}

fn write_private_idempotently(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o440);
    }
    match options.open(path) {
        Ok(mut file) => {
            file.write_all(bytes)
                .and_then(|_| file.sync_all())
                .map_err(text)?;
            fs::File::open(path.parent().ok_or("bound input has no parent")?)
                .and_then(|parent| parent.sync_all())
                .map_err(text)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(path).map_err(text)?;
            if !metadata.is_file() {
                return Err("campaign bound input replay is not an ordinary file".into());
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if metadata.uid() != nix::unistd::geteuid().as_raw()
                    || metadata.nlink() != 1
                    || metadata.mode() & 0o777 != 0o440
                {
                    return Err("campaign bound input replay must be owner-bound, singly linked and read-only".into());
                }
            }
            if fs::read(path).map_err(text)? != bytes {
                return Err("campaign bound input conflicts with prior exact attempt bytes".into());
            }
            Ok(())
        }
        Err(error) => Err(text(error)),
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let metadata = fs::symlink_metadata(path).map_err(text)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_PROFILE_BYTES {
        return Err("campaign input must be a bounded ordinary file".into());
    }
    serde_json::from_slice(&fs::read(path).map_err(text)?).map_err(text)
}
fn wall_clock_ms() -> Result<u64, String> {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(text)?
            .as_millis(),
    )
    .map_err(text)
}
fn id(hex: &str) -> Result<ContentId, String> {
    ContentId::from_blake3_hex(hex.to_owned()).map_err(show)
}
fn show(error: kiln_core::types::Diagnostic) -> String {
    format!("{}: {}", error.code, error.message)
}
fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn validate_role_directory(path: &Path, gid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::symlink_metadata(path).map_err(text)?;
        if !metadata.is_dir()
            || metadata.uid() != 0
            || metadata.gid() != gid
            || metadata.mode() & 0o7777 != 0o2770
            || fs::canonicalize(path).map_err(text)? != path
        {
            return Err(
                "campaign shared role directory must be canonical root-owned setgid mode 02770"
                    .into(),
            );
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (path, gid);
        Err("campaign role-separated directories require Unix ownership semantics".into())
    }
}

fn preflight_roles(profile: &ImplementationCampaignDriveProfile) -> Result<(), String> {
    if !nix::unistd::geteuid().is_root() {
        return Err("campaign role delegation requires root or a separately admitted role launcher; no provider effect was attempted".into());
    }
    validate_role_directory(&profile.bound_input_directory, profile.roles.worker.gid)?;
    validate_role_directory(
        &profile.candidate_export.output_parent,
        profile.roles.worker.gid,
    )?;
    workspace::preflight(&profile.workspace, profile.roles.worker.gid)?;
    let transport: kiln_adapter_lattice::worker::WorkerTransportProfile =
        read_json(&profile.worker.transport_profile)?;
    if transport.schema != "kiln.lattice-implementation-cli.v2"
        || fs::canonicalize(&transport.lattice_program).map_err(text)?
            != fs::canonicalize(&profile.roles.exporter.program).map_err(text)?
        || transport.executable_blake3 != profile.roles.exporter.blake3
    {
        return Err(
            "campaign export/preparation must use the exact selected transport-v2 Lattice program"
                .into(),
        );
    }
    let runtime = BoundedCairnRuntime;
    if runtime
        .executable_identity(
            &immutable_program(&profile.roles.setpriv)?,
            MAX_LAUNCHER_BYTES,
        )
        .map_err(|error| format!("{error:?}"))?
        != profile.roles.setpriv_blake3
    {
        return Err("setpriv launcher identity changed".into());
    }
    for role in [
        &profile.roles.worker,
        &profile.roles.exporter,
        &profile.roles.capture,
        &profile.roles.verifier,
    ] {
        if role.arguments.len() > MAX_ROLE_ARGUMENTS
            || role
                .arguments
                .iter()
                .any(|arg| arg.len() > MAX_ROLE_ARGUMENT_BYTES || arg.chars().any(char::is_control))
        {
            return Err("campaign role command prefix exceeds its bound".into());
        }
        if runtime
            .executable_identity(&immutable_program(&role.program)?, MAX_ROLE_PROGRAM_BYTES)
            .map_err(|error| format!("{error:?}"))?
            != role.blake3
        {
            return Err(format!(
                "campaign role program identity changed: {}",
                role.program.display()
            ));
        }
    }
    Ok(())
}

fn immutable_program(path: &Path) -> Result<PathBuf, String> {
    let resolved = fs::canonicalize(path).map_err(text)?;
    if !resolved.starts_with("/nix/store") || !resolved.is_file() {
        return Err("campaign role program must resolve to its immutable Nix-store cohort".into());
    }
    Ok(resolved)
}

fn run_role(
    profile: &ImplementationCampaignDriveProfile,
    role: &RoleProgram,
    working_directory: &Path,
    args: &[OsString],
) -> Result<Vec<u8>, String> {
    if !nix::unistd::geteuid().is_root() {
        return Err("campaign role launcher is unavailable to this caller".into());
    }
    let mut runtime = BoundedCairnRuntime;
    let launcher = immutable_program(&profile.roles.setpriv)?;
    let program = immutable_program(&role.program)?;
    if runtime
        .executable_identity(&launcher, MAX_LAUNCHER_BYTES)
        .map_err(|error| format!("{error:?}"))?
        != profile.roles.setpriv_blake3
        || runtime
            .executable_identity(&program, MAX_ROLE_PROGRAM_BYTES)
            .map_err(|error| format!("{error:?}"))?
            != role.blake3
    {
        return Err("campaign role executable identity changed before effect".into());
    }
    let mut command = vec![
        OsString::from("--reuid"),
        OsString::from(role.uid.to_string()),
        OsString::from("--regid"),
        OsString::from(role.gid.to_string()),
        OsString::from("--clear-groups"),
        OsString::from("--bounding-set=-all"),
        OsString::from("--inh-caps=-all"),
        OsString::from("--ambient-caps=-all"),
        OsString::from("--no-new-privs"),
        OsString::from("--"),
        program.into_os_string(),
    ];
    command.extend(role.arguments.iter().map(OsString::from));
    command.extend_from_slice(args);
    let invocation = ProcessInvocation {
        program: launcher,
        args: command,
        current_dir: working_directory.to_path_buf(),
        environment: Vec::new(),
        limits: ExecutionLimits {
            timeout_ms: ROLE_TIMEOUT_MS,
            stdin_max_bytes: 0,
            stdout_max_bytes: ROLE_STDOUT_BYTES,
            stderr_max_bytes: ROLE_STDERR_BYTES,
            poll_interval_ms: ROLE_POLL_MS,
            teardown_timeout_ms: ROLE_TEARDOWN_MS,
        },
    };
    match runtime
        .run(&invocation)
        .map_err(|error| format!("{error:?}"))?
    {
        CairnProcessOutput::Exited {
            process_status: Some(0),
            stdout,
            ..
        } => Ok(stdout),
        _ => Err("campaign role outcome is failed or unknown; retain intent and reconcile".into()),
    }
}
fn validate_export_tree(
    output: &Path,
    candidate: &Path,
    receipt: &Path,
    observation: &Path,
    exporter: &RoleProgram,
) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        for (path, directory) in [
            (output, true),
            (candidate, true),
            (receipt, false),
            (observation, false),
        ] {
            let metadata = fs::symlink_metadata(path).map_err(text)?;
            if metadata.is_dir() != directory
                || (!directory && !metadata.is_file())
                || metadata.uid() != exporter.uid
                || metadata.gid() != exporter.gid
                || metadata.mode() & 0o027 != 0
            {
                return Err(
                    "retained candidate export is not an ordinary exporter-owned read-only tree"
                        .into(),
                );
            }
            if !directory
                && (metadata.len() == 0
                    || metadata.len() > 2 * 1024 * 1024
                    || metadata.nlink() != 1)
            {
                return Err("retained candidate export file violates its bound".into());
            }
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (output, candidate, receipt, observation, exporter);
        Err("candidate export requires Unix role-separated file authority".into())
    }
}

struct CampaignSwarmMirror {
    client: kiln_adapter_swarm::SwarmClient,
    journal_path: PathBuf,
    host: String,
    from: String,
    read_limit: usize,
}

impl kiln_implementer::MirrorObserver for CampaignSwarmMirror {
    fn reconcile(
        &mut self,
        snapshots: &[kiln_core::implementation::swarm::AttemptSnapshot],
        now_ms: u64,
    ) -> Result<(), String> {
        let mut journal = kiln_adapter_swarm::MirrorJournal::load(&self.journal_path)?;
        let context = kiln_adapter_swarm::MirrorContext {
            from: &self.from,
            host: &self.host,
            now_ms,
            read_limit: self.read_limit,
        };
        let _actions =
            kiln_adapter_swarm::mirror::reconcile(&self.client, &mut journal, snapshots, &context);
        journal.save(&self.journal_path)
    }
}

fn attach_swarm_mirror(
    profile: &ImplementationCampaignDriveProfile,
    owner: &mut kiln_implementer::Implementer,
    now_ms: u64,
) -> Result<(), String> {
    let Some(config) = profile.swarm.clone() else {
        return Ok(());
    };
    if !config.enabled {
        return Ok(());
    }
    let journal_path = config.state_dir.join("kiln-mirror.json");
    let read_limit = config.read_limit;
    let from = config.from.clone();
    let client = kiln_adapter_swarm::SwarmClient::new(config)?;
    let views: Vec<kiln_core::implementation::swarm::ArticleView> = client
        .read_recent(read_limit)
        .map(|articles| {
            articles
                .iter()
                .filter_map(|article| article.view())
                .collect()
        })
        .unwrap_or_default();
    let closure_from = from.clone();
    owner.set_overlap(Some(Box::new(move |work| {
        let scope = kiln_core::implementation::swarm::ClaimScope {
            repository: work.repository.clone(),
            scope: work
                .permitted_paths
                .first()
                .cloned()
                .unwrap_or_else(|| work.project.clone()),
        };
        match kiln_core::implementation::swarm::evaluate_overlap(
            &views,
            &closure_from,
            &scope,
            now_ms,
        ) {
            kiln_core::implementation::swarm::OverlapDecision::Deferred {
                message_id,
                author,
                age_ms,
            } => Some(format!(
                "open swarm claim {message_id} by {author} ({age_ms} ms)"
            )),
            kiln_core::implementation::swarm::OverlapDecision::Allowed => None,
        }
    })));
    owner.set_mirror(Some(Box::new(CampaignSwarmMirror {
        client,
        journal_path,
        host: "kiln".to_string(),
        from,
        read_limit,
    })));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_required_checks_need_the_exact_passing_canonical_checks() {
        use kiln_core::change_review::CiCheck;
        let required = vec!["acceptance".to_owned()];
        let check = |name: &str, outcome: &str| CiCheck {
            name: name.into(),
            kind: "test".into(),
            outcome: outcome.into(),
            artifact_digest: Some("a".repeat(64)),
        };
        assert!(required_checks_present(
            &required,
            &[check("acceptance", "pass")]
        ));
        assert!(!required_checks_present(
            &required,
            &[check("unrelated", "pass")]
        ));
        assert!(!required_checks_present(
            &required,
            &[check("acceptance", "fail")]
        ));
        assert!(!required_checks_present(
            &required,
            &[check("acceptance", "unknown")]
        ));
        assert!(!required_checks_present(&required, &[]));
    }

    #[test]
    fn bound_attempt_inputs_are_exactly_replayable_but_never_substitutable() {
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let path = directory.path().join("attempt.request.json");
        let first = br#"{"attempt":"one"}"#;
        write_private_idempotently(&path, first).unwrap_or_else(|_| unreachable!());
        write_private_idempotently(&path, first).unwrap_or_else(|_| unreachable!());
        assert!(write_private_idempotently(&path, br#"{"attempt":"two"}"#).is_err());
        assert_eq!(fs::read(path).unwrap_or_default(), first);
    }

    #[cfg(unix)]
    #[test]
    fn bound_attempt_input_replay_rejects_symlinks() {
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let target = directory.path().join("target");
        let link = directory.path().join("attempt.plan.json");
        fs::write(&target, b"same bytes").unwrap_or_else(|_| unreachable!());
        std::os::unix::fs::symlink(&target, &link).unwrap_or_else(|_| unreachable!());
        assert!(write_private_idempotently(&link, b"same bytes").is_err());
    }
}
