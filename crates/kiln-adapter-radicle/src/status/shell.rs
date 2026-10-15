use std::collections::BTreeSet;
use std::path::PathBuf;
use std::str::FromStr;

use cob_git2 as git2;
use kiln_core::types::{Diagnostic, ExternalRef};
use kiln_provider_api::{InfrastructureFailure, PortFailure, PortResult};
use onix_radicle_cob_git_store::{GitCobStore, GitCobStoreError};
use radicle_cob::change::Storage as _;
use radicle_cob::object::Storage as _;
use radicle_cob::object::collaboration;
use radicle_cob::{Contents, Create, ObjectId, Update, Version};
use radicle_crypto::PublicKey;
use radicle_oid::Oid;

use super::cob::{CiJobAction, CiJobStatus, ProjectedOutcome};
use super::plan::{
    HeadAuthor, ObservedHeads, StatusOutcome, StatusPlan, admit_expected_head,
    admit_idempotent_replay, admit_single_writer_heads, admit_status_plan,
};
#[cfg(feature = "status-native-fixtures")]
use super::profile::admit_fixture_profile;
use super::profile::{RadicleStatusProfile, admit_profile};
use super::resolver::SignerResolver;
use super::secret::ResolvedSigner;

const COMMIT_AUTHOR_NAME: &str = "Kiln Status";
const COMMIT_AUTHOR_EMAIL: &str = "status@kiln.invalid";

// r[impl kiln.radicle.shell_status]
/// Bounded, single-writer Radicle CI status publication shell.
///
/// The shell owns storage handles and SecretSpec resolution. Pure functions
/// retain authority, allowlist, idempotency, and outcome decisions.
pub struct RadicleStatusShell {
    profile: RadicleStatusProfile,
    allowed_rids: BTreeSet<String>,
    resolver: SignerResolver,
    /// Root of the local Radicle git storage.
    storage_root: PathBuf,
}

impl std::fmt::Debug for RadicleStatusShell {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RadicleStatusShell")
            .field("profile", &self.profile)
            .field("storage_root", &self.storage_root)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct StatusWriteObservation {
    pub repository: ExternalRef,
    pub object_id: ExternalRef,
    pub head: ExternalRef,
    pub run_id: String,
    pub idempotency_key: String,
    pub outcome: StatusOutcome,
    pub signer_identity: ExternalRef,
}

/// A write may have reached storage even when its final result is unavailable.
/// The shell returns this value instead of converting uncertainty to failure.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct StatusUnknownObservation {
    pub repository: ExternalRef,
    pub object_id: Option<ExternalRef>,
    pub run_id: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(
    rename_all = "snake_case",
    tag = "disposition",
    content = "observation"
)]
pub enum StatusDispatch {
    Written(StatusWriteObservation),
    Unknown(StatusUnknownObservation),
}

struct OpenedRepository {
    repository: git2::Repository,
    _commit_config: tempfile::NamedTempFile,
}

impl OpenedRepository {
    const fn repository(&self) -> &git2::Repository {
        &self.repository
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TargetState {
    New,
    Existing {
        object_id: ObjectId,
        current_head: Oid,
        projected: Option<ProjectedOutcome>,
    },
}

impl RadicleStatusShell {
    pub fn new(storage_root: PathBuf, profile: RadicleStatusProfile) -> PortResult<Self> {
        let allowed_rids = admit_profile(&profile)?;
        Ok(Self {
            resolver: SignerResolver::new(
                profile.signer.clone(),
                profile.executable.path.clone(),
                profile.executable.blake3.clone(),
            ),
            allowed_rids,
            profile,
            storage_root,
        })
    }

    #[cfg(feature = "status-native-fixtures")]
    pub fn new_fixture(storage_root: PathBuf, profile: RadicleStatusProfile) -> PortResult<Self> {
        let allowed_rids = admit_fixture_profile(&profile)?;
        Ok(Self {
            resolver: SignerResolver::new(
                profile.signer.clone(),
                profile.executable.path.clone(),
                profile.executable.blake3.clone(),
            ),
            allowed_rids,
            profile,
            storage_root,
        })
    }

    #[must_use]
    pub const fn profile(&self) -> &RadicleStatusProfile {
        &self.profile
    }

    // r[impl kiln.radicle.status_store_boundary]
    // r[impl kiln.radicle.status_signer.scenario.resolved]
    // r[impl kiln.radicle.status_signer.scenario.missing]
    // r[impl kiln.radicle.status_allowlist.scenario.admitted]
    // r[impl kiln.radicle.status_allowlist.scenario.foreign]
    // r[impl kiln.radicle.shell_status.scenario.unknown]
    pub fn publish(&self, plan: &StatusPlan, reason: &str) -> PortResult<StatusDispatch> {
        self.admit_plan(plan)?;
        let contents = self.status_contents(plan)?;
        let signer = self.resolver.resolve(reason)?;
        let signer_identity = signer_identity(&signer)?;
        let namespace = public_key_of(&signer)?;
        let opened = self.open_repository(&plan.repository)?;
        let repo = opened.repository();
        let repository = GitCobStore::new(repo);
        let target = self.resolve_target(&repository, plan, &signer_identity)?;

        if let TargetState::Existing {
            object_id,
            current_head,
            projected: Some(projected),
        } = &target
        {
            admit_idempotent_replay(plan, &projected.run_id, projected.outcome)
                .map_err(PortFailure::Domain)?;
            return Ok(StatusDispatch::Written(write_observation(
                plan,
                *object_id,
                *current_head,
                signer_identity,
            )?));
        }

        admit_planned_head(plan, &target)?;
        let type_name = CiJobStatus::type_name_static().clone();
        let message = format!("kiln status {}", plan.idempotency_key.native_id.as_str());
        match target {
            TargetState::New => {
                let created = collaboration::create::<CiJobStatus, GitCobStore>(
                    &repository,
                    signer.signing_key(),
                    None,
                    Vec::new(),
                    &namespace,
                    Create {
                        contents,
                        type_name,
                        message,
                        embeds: Vec::new(),
                        version: Version::default(),
                    },
                );
                match created {
                    Ok(object) => Ok(StatusDispatch::Written(write_observation(
                        plan,
                        *object.id(),
                        **object.id(),
                        signer_identity,
                    )?)),
                    Err(collaboration::error::Create::Refs { .. }) => {
                        Ok(StatusDispatch::Unknown(unknown_observation(plan, None)?))
                    }
                    Err(error) => Err(create_failure(error)),
                }
            }
            TargetState::Existing {
                object_id,
                current_head,
                projected: None,
            } => {
                // r[impl kiln.radicle.status_store_concurrency]
                let write_repository = GitCobStore::with_expected_ref(repo, current_head);
                let updated = collaboration::update::<CiJobStatus, GitCobStore>(
                    &write_repository,
                    signer.signing_key(),
                    None,
                    Vec::new(),
                    &namespace,
                    Update {
                        changes: contents,
                        object_id,
                        type_name,
                        message,
                        embeds: Vec::new(),
                    },
                );
                match updated {
                    Ok(updated) => Ok(StatusDispatch::Written(write_observation(
                        plan,
                        object_id,
                        updated.head,
                        signer_identity,
                    )?)),
                    Err(collaboration::error::Update::Refs { .. }) => Ok(StatusDispatch::Unknown(
                        unknown_observation(plan, Some(object_id))?,
                    )),
                    Err(error) => Err(update_failure(error)),
                }
            }
            TargetState::Existing {
                projected: Some(_), ..
            } => unreachable!("idempotent replay returned before write"),
        }
    }

    /// Re-read one accepted write. This operation never redispatches.
    pub fn reconcile(
        &self,
        plan: &StatusPlan,
        reason: &str,
    ) -> PortResult<Option<StatusWriteObservation>> {
        self.admit_plan(plan)?;
        let signer = self.resolver.resolve(reason)?;
        let signer_identity = signer_identity(&signer)?;
        let opened = self.open_repository(&plan.repository)?;
        let repo = opened.repository();
        let repository = GitCobStore::new(repo);
        let target = self.resolve_target(&repository, plan, &signer_identity)?;
        match target {
            TargetState::New => Ok(None),
            TargetState::Existing {
                object_id: _,
                current_head: _,
                projected: None,
            } => Ok(None),
            TargetState::Existing {
                object_id,
                current_head,
                projected: Some(projected),
            } => {
                admit_idempotent_replay(plan, &projected.run_id, projected.outcome)
                    .map_err(PortFailure::Domain)?;
                Ok(Some(write_observation(
                    plan,
                    object_id,
                    current_head,
                    signer_identity,
                )?))
            }
        }
    }

    fn admit_plan(&self, plan: &StatusPlan) -> PortResult<()> {
        admit_status_plan(plan, &self.allowed_rids).map_err(PortFailure::Domain)?;
        if plan.max_payload_bytes > self.profile.max_payload_bytes {
            return Err(PortFailure::Domain(Diagnostic::new(
                "status_payload_bound",
                "status_plan.max_payload_bytes",
                "Radicle status payload bound exceeds the profile maximum",
            )));
        }
        Ok(())
    }

    // r[impl kiln.radicle.status_store_scan]
    fn resolve_target(
        &self,
        repository: &GitCobStore<'_>,
        plan: &StatusPlan,
        signer_identity: &ExternalRef,
    ) -> PortResult<TargetState> {
        if let Some(declared) = &plan.object_id {
            let object_id = parse_object_id(declared)?;
            let observed = read_heads(repository, object_id)?;
            if observed.heads.is_empty() {
                return Err(PortFailure::Domain(Diagnostic::new(
                    "status_object_missing",
                    "status_plan.object_id",
                    "Declared ci-job object does not exist",
                )));
            }
            admit_single_writer_heads(&observed, signer_identity).map_err(PortFailure::Domain)?;
            let current_head = observed_head_id(&observed)?;
            let projected = read_projected(repository, plan, object_id)?;
            return Ok(TargetState::Existing {
                object_id,
                current_head,
                projected,
            });
        }

        let type_name = CiJobStatus::type_name_static();
        let object_ids = repository
            .object_ids_bounded(type_name, self.profile.max_objects_scanned)
            .map_err(storage_failure)?;
        let mut matched = None;
        for object_id in object_ids {
            let observed = read_heads(repository, object_id)?;
            admit_single_writer_heads(&observed, signer_identity).map_err(PortFailure::Domain)?;
            let projected = read_projected(repository, plan, object_id)?;
            if let Some(projected) = projected {
                if matched.is_some() {
                    return Err(PortFailure::Domain(Diagnostic::new(
                        "status_idempotency_ambiguous",
                        "status_plan.idempotency_key",
                        "More than one ci-job object contains the idempotency key",
                    )));
                }
                matched = Some(TargetState::Existing {
                    object_id,
                    current_head: observed_head_id(&observed)?,
                    projected: Some(projected),
                });
            }
        }
        Ok(matched.unwrap_or(TargetState::New))
    }

    fn status_contents(&self, plan: &StatusPlan) -> PortResult<Contents> {
        let action = CiJobAction::Project {
            key: plan.idempotency_key.native_id.as_str().to_string(),
            run_id: plan.run_id.as_str().to_string(),
            outcome: plan.outcome,
        };
        let payload = serde_json::to_vec(&action).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "radicle_status_action_encoding",
                "radicle_status.action",
                "The bounded status action could not be encoded",
            ))
        })?;
        if payload.len() > plan.max_payload_bytes {
            return Err(PortFailure::Domain(Diagnostic::new(
                "status_payload_bound",
                "status_plan.payload",
                "Serialized ci-job action exceeds its named bound",
            )));
        }
        non_empty_payload(payload)
    }

    fn open_repository(&self, repository: &ExternalRef) -> PortResult<OpenedRepository> {
        let rid = repository.native_id.as_str();
        let path = self.storage_root.join(rid);
        let repository = git2::Repository::open(&path).map_err(|error| {
            PortFailure::Infrastructure(InfrastructureFailure {
                code: "radicle_storage_unavailable".to_string(),
                message: format!("local Radicle repository could not be opened: {error}"),
            })
        })?;
        let commit_config = tempfile::NamedTempFile::new().map_err(|error| {
            PortFailure::Infrastructure(InfrastructureFailure {
                code: "radicle_commit_config_failed".to_string(),
                message: format!("temporary ci-job commit configuration failed: {error}"),
            })
        })?;
        let mut config = git2::Config::open(commit_config.path()).map_err(commit_config_failure)?;
        config
            .set_str("user.name", COMMIT_AUTHOR_NAME)
            .map_err(commit_config_failure)?;
        config
            .set_str("user.email", COMMIT_AUTHOR_EMAIL)
            .map_err(commit_config_failure)?;
        repository
            .set_config(&config)
            .map_err(commit_config_failure)?;
        Ok(OpenedRepository {
            repository,
            _commit_config: commit_config,
        })
    }
}

fn read_projected(
    repository: &GitCobStore<'_>,
    plan: &StatusPlan,
    object_id: ObjectId,
) -> PortResult<Option<ProjectedOutcome>> {
    let type_name = CiJobStatus::type_name_static();
    let object = collaboration::get::<CiJobStatus, GitCobStore>(repository, type_name, &object_id)
        .map_err(retrieve_failure)?;
    Ok(object.and_then(|entry| entry.object().projected(plan).cloned()))
}

fn read_heads(repository: &GitCobStore<'_>, object_id: ObjectId) -> PortResult<ObservedHeads> {
    let type_name = CiJobStatus::type_name_static();
    let refs = repository
        .objects(type_name, &object_id)
        .map_err(storage_failure)?;
    let mut heads = Vec::new();
    for reference in refs.iter() {
        let entry = repository.load(reference.target.id).map_err(|error| {
            PortFailure::Infrastructure(InfrastructureFailure {
                code: "radicle_head_unreadable".to_string(),
                message: format!("a ci-job head entry could not be loaded: {error}"),
            })
        })?;
        heads.push(HeadAuthor {
            head: ExternalRef::new("radicle", "head", &reference.target.id.to_string())
                .map_err(PortFailure::Domain)?,
            author: Some(
                ExternalRef::new("radicle", "node", &entry.author().to_string())
                    .map_err(PortFailure::Domain)?,
            ),
        });
    }
    Ok(ObservedHeads { heads })
}

fn observed_head_id(observed: &ObservedHeads) -> PortResult<Oid> {
    let head = observed.heads.first().ok_or_else(|| {
        PortFailure::Domain(Diagnostic::new(
            "status_head_missing",
            "status_plan.heads",
            "Existing ci-job object has no current head",
        ))
    })?;
    Oid::from_str(head.head.native_id.as_str()).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "status_head_identity_invalid",
            "status_plan.heads",
            "Current ci-job head did not parse as an object identity",
        ))
    })
}

fn admit_planned_head(plan: &StatusPlan, target: &TargetState) -> PortResult<()> {
    match target {
        TargetState::New => {
            admit_expected_head(None, plan.expected_head.as_ref()).map_err(PortFailure::Domain)
        }
        TargetState::Existing { current_head, .. } => {
            let observed = head_external_ref(*current_head)?;
            admit_expected_head(Some(&observed), plan.expected_head.as_ref())
                .map_err(PortFailure::Domain)
        }
    }
}

fn write_observation(
    plan: &StatusPlan,
    object_id: ObjectId,
    head: Oid,
    signer_identity: ExternalRef,
) -> PortResult<StatusWriteObservation> {
    Ok(StatusWriteObservation {
        repository: plan.repository.clone(),
        object_id: object_external_ref(object_id)?,
        head: head_external_ref(head)?,
        run_id: plan.run_id.as_str().to_string(),
        idempotency_key: plan.idempotency_key.native_id.as_str().to_string(),
        outcome: plan.outcome,
        signer_identity,
    })
}

fn unknown_observation(
    plan: &StatusPlan,
    object_id: Option<ObjectId>,
) -> PortResult<StatusUnknownObservation> {
    let object_id = object_id.map(object_external_ref).transpose()?;
    Ok(StatusUnknownObservation {
        repository: plan.repository.clone(),
        object_id,
        run_id: plan.run_id.as_str().to_string(),
        idempotency_key: plan.idempotency_key.native_id.as_str().to_string(),
    })
}

fn object_external_ref(object_id: ObjectId) -> PortResult<ExternalRef> {
    let native_id = object_id.to_string();
    ExternalRef::new("radicle", "cob", &native_id).map_err(PortFailure::Domain)
}

fn head_external_ref(head: Oid) -> PortResult<ExternalRef> {
    let native_id = head.to_string();
    ExternalRef::new("radicle", "head", &native_id).map_err(PortFailure::Domain)
}

fn signer_identity(signer: &ResolvedSigner) -> PortResult<ExternalRef> {
    ExternalRef::new("radicle", "node", signer.public_key()).map_err(PortFailure::Domain)
}

fn public_key_of(signer: &ResolvedSigner) -> PortResult<PublicKey> {
    PublicKey::from_str(signer.public_key()).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "radicle_signer_identity_invalid",
            "radicle_status.signer",
            "Resolved signer identity did not parse as a node public key",
        ))
    })
}

fn parse_object_id(object_id: &ExternalRef) -> PortResult<ObjectId> {
    ObjectId::from_str(object_id.native_id.as_str()).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "radicle_object_id_invalid",
            "radicle_status.object_id",
            "ci-job object identity did not parse as a COB object id",
        ))
    })
}

fn non_empty_payload(payload: Vec<u8>) -> PortResult<Contents> {
    use nonempty::NonEmpty;
    if payload.is_empty() {
        return Err(PortFailure::Domain(Diagnostic::new(
            "status_payload_empty",
            "status_plan.payload",
            "Serialized ci-job action was empty",
        )));
    }
    Ok(NonEmpty::new(payload))
}

fn storage_failure(error: GitCobStoreError) -> PortFailure {
    let code = match &error {
        GitCobStoreError::ZeroObjectScanBound
        | GitCobStoreError::ObjectScanBoundExceeded { .. } => "radicle_object_scan_bound",
        GitCobStoreError::NonUtf8Reference | GitCobStoreError::RefFormat(_) => {
            "radicle_ref_name_invalid"
        }
        GitCobStoreError::InvalidCobReference { .. } => "radicle_ref_unparsable",
        GitCobStoreError::Git(_)
        | GitCobStoreError::Reference(_)
        | GitCobStoreError::InvalidCobChange { .. }
        | GitCobStoreError::ReferenceDrift { .. } => "radicle_refs_unreadable",
    };
    PortFailure::Domain(Diagnostic::new(
        code,
        "radicle_status.refs",
        &error.to_string(),
    ))
}

fn commit_config_failure(error: git2::Error) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: "radicle_commit_config_failed".to_string(),
        message: format!("deterministic ci-job commit metadata could not be configured: {error}"),
    })
}

fn create_failure(error: collaboration::error::Create) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: "radicle_store_create_failed".to_string(),
        message: format!("the pinned COB store rejected the ci-job create: {error}"),
    })
}

fn update_failure(error: collaboration::error::Update) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: "radicle_store_update_failed".to_string(),
        message: format!("the pinned COB store rejected the ci-job update: {error}"),
    })
}

fn retrieve_failure(error: collaboration::error::Retrieve) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: "radicle_store_error".to_string(),
        message: format!("the pinned COB store rejected the ci-job read: {error}"),
    })
}
