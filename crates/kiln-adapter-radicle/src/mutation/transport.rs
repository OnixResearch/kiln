use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, Instant};

use cob_git2 as git2;
use kiln_core::cob::{CobEffectPlan, CobObservation};
use kiln_core::types::{CobSemanticStateId, ContentId, Diagnostic, ExternalRef};
use kiln_provider_api::{
    CollaborationMutationOutcome, CollaborationMutationPort, InfrastructureFailure, PortFailure,
    PortResult,
};
use nonempty::NonEmpty;
use onix_radicle_cob_git_store::{GitCobStore, GitCobStoreError};
use radicle_cob::change::Storage as _;
use radicle_cob::object::Storage as _;
use radicle_cob::object::collaboration;
use radicle_cob::{Entry, ObjectId, TypeName, Update};
use radicle_crypto::PublicKey;
use radicle_oid::Oid;

use crate::status::{ResolvedSigner, SignerResolver};

#[cfg(feature = "status-native-fixtures")]
use super::model::admit_mutation_fixture_profile;
use super::model::{
    PATCH_SCHEMA_LINE, PATCH_TYPE_NAME, PatchReviewAction, RadicleMutationProfile,
    WriteClassification, WriteFact, admit_mutation_plan, admit_mutation_profile, classify_write,
    render_patch_review,
};

const COMMIT_AUTHOR_NAME: &str = "Kiln Mutation";
const COMMIT_AUTHOR_EMAIL: &str = "mutation@kiln.invalid";
const OPERATION_MESSAGE_PREFIX: &str = "kiln mutation";
const HISTORY_DIGEST_DOMAIN: &[u8] = b"kiln.radicle.patch-history.v1\0";
const PATCH_ACTION_TYPES: [&str; 24] = [
    "assign",
    "edit",
    "label",
    "lifecycle",
    "merge",
    "review",
    "review.comment",
    "review.comment.edit",
    "review.comment.react",
    "review.comment.redact",
    "review.comment.resolve",
    "review.comment.unresolve",
    "review.edit",
    "review.edit.v2",
    "review.react",
    "review.redact",
    "revision",
    "revision.comment",
    "revision.comment.edit",
    "revision.comment.react",
    "revision.comment.redact",
    "revision.edit",
    "revision.react",
    "revision.redact",
];

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
struct PreviewedMutation {
    plan: CobEffectPlan,
    payload: Vec<u8>,
}

struct ObservedMutation {
    observation: CobObservation,
    entries: Vec<Entry>,
    head: Oid,
    resource: Oid,
}

/// Concrete single-writer patch-review transport.
///
/// The transport only appends the pinned `review` action. It cannot create,
/// close, archive, merge, or revise a patch.
pub struct RadicleMutationTransport {
    storage_root: PathBuf,
    profile: RadicleMutationProfile,
    resolver: SignerResolver,
    signer: Option<ResolvedSigner>,
    preview: Option<PreviewedMutation>,
    #[cfg(feature = "status-native-fixtures")]
    fixture_delay_after_write: Duration,
}

impl std::fmt::Debug for RadicleMutationTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RadicleMutationTransport")
            .field("storage_root", &self.storage_root)
            .field("profile", &self.profile)
            .field("signer", &self.signer.as_ref().map(|_| "[resolved]"))
            .field("preview", &self.preview)
            .finish_non_exhaustive()
    }
}

impl RadicleMutationTransport {
    pub fn new(storage_root: PathBuf, profile: RadicleMutationProfile) -> PortResult<Self> {
        admit_mutation_profile(&profile).map_err(PortFailure::Domain)?;
        Ok(Self::from_admitted(storage_root, profile))
    }

    #[cfg(feature = "status-native-fixtures")]
    pub fn new_fixture(
        storage_root: PathBuf,
        profile: RadicleMutationProfile,
        delay_after_write: Duration,
    ) -> PortResult<Self> {
        admit_mutation_fixture_profile(&profile).map_err(PortFailure::Domain)?;
        let mut transport = Self::from_admitted(storage_root, profile);
        transport.fixture_delay_after_write = delay_after_write;
        Ok(transport)
    }

    fn from_admitted(storage_root: PathBuf, profile: RadicleMutationProfile) -> Self {
        Self {
            resolver: SignerResolver::new(
                profile.signer.clone(),
                profile.executable.path.clone(),
                profile.executable.blake3.clone(),
            ),
            storage_root,
            profile,
            signer: None,
            preview: None,
            #[cfg(feature = "status-native-fixtures")]
            fixture_delay_after_write: Duration::ZERO,
        }
    }

    #[must_use]
    pub const fn profile(&self) -> &RadicleMutationProfile {
        &self.profile
    }

    // r[impl kiln.radicle.status_store_boundary]
    fn observe_bounded(&self, plan: &CobEffectPlan) -> PortResult<ObservedMutation> {
        let started = Instant::now();
        let _action = admit_mutation_plan(&self.profile, plan).map_err(PortFailure::Domain)?;
        let opened = self.open_repository(&plan.repository)?;
        let repository = GitCobStore::new(opened.repository());
        let object_id = parse_object_id(plan)?;
        let type_name = patch_type_name()?;
        let refs = repository
            .objects(&type_name, &object_id)
            .map_err(storage_failure)?;
        let mut refs_iter = refs.iter();
        let reference = refs_iter.next().ok_or_else(|| {
            PortFailure::Domain(Diagnostic::new(
                "radicle_mutation_object_missing",
                "radicle_mutation.object_id",
                "Declared patch object does not exist",
            ))
        })?;
        if refs_iter.next().is_some() {
            return Err(PortFailure::Domain(Diagnostic::new(
                "radicle_mutation_multi_head",
                "radicle_mutation.heads",
                "Patch object has multiple heads under single-writer policy",
            )));
        }
        let entries = load_history_bounded(
            &repository,
            reference.target.id,
            self.profile.max_history_entries,
            self.profile.max_history_bytes,
        )?;
        let resource = consistent_resource(&entries)?;
        let observation = observation_from_history(plan, &entries, reference.target.id)?;
        if deadline_expired(started, self.profile.deadlines.preflight_ms) {
            return Err(timeout_failure(
                "radicle_mutation_preflight_timeout",
                "Patch mutation preflight exceeded its named deadline",
            ));
        }
        Ok(ObservedMutation {
            observation,
            entries,
            head: reference.target.id,
            resource,
        })
    }

    fn open_repository(&self, repository: &ExternalRef) -> PortResult<OpenedRepository> {
        let path = self.storage_root.join(repository.native_id.as_str());
        open_repository(&path)
    }

    fn clear_authority(&mut self) {
        self.signer = None;
        self.preview = None;
    }
}

// r[impl kiln.radicle.shell_mutation]
// r[impl kiln.radicle.mutation_bounds]
impl CollaborationMutationPort for RadicleMutationTransport {
    fn observe(&mut self, plan: &CobEffectPlan) -> PortResult<CobObservation> {
        Ok(self.observe_bounded(plan)?.observation)
    }

    fn acquire_signer(&mut self, actor: &ExternalRef) -> PortResult<()> {
        self.signer = None;
        let started = Instant::now();
        let signer = self
            .resolver
            .resolve("publish declared Radicle patch review")?;
        if deadline_expired(started, self.profile.deadlines.signer_ms) {
            return Err(timeout_failure(
                "radicle_mutation_signer_timeout",
                "Patch mutation signer resolution exceeded its named deadline",
            ));
        }
        if actor.provider_namespace.as_str() != "radicle"
            || actor.native_kind.as_str() != "node"
            || actor.native_id.as_str() != signer.public_key()
        {
            return Err(PortFailure::Domain(Diagnostic::new(
                "radicle_mutation_signer_substitution",
                "radicle_mutation.actor",
                "Resolved signer does not match the exact planned actor",
            )));
        }
        self.signer = Some(signer);
        Ok(())
    }

    fn preview(&mut self, plan: &CobEffectPlan) -> PortResult<()> {
        self.preview = None;
        let signer = self.signer.as_ref().ok_or_else(|| {
            PortFailure::Domain(Diagnostic::new(
                "radicle_mutation_signer_missing",
                "radicle_mutation.signer",
                "Patch mutation preview requires the resolved signer",
            ))
        })?;
        if signer.public_key() != plan.actor.native_id.as_str() {
            return Err(PortFailure::Domain(Diagnostic::new(
                "radicle_mutation_signer_substitution",
                "radicle_mutation.actor",
                "Patch mutation preview actor differs from the resolved signer",
            )));
        }
        let action = admit_mutation_plan(&self.profile, plan).map_err(PortFailure::Domain)?;
        let payload = render_patch_review(action).map_err(PortFailure::Domain)?;
        if blake3::hash(&payload).to_string() != plan.payload_identity.as_str() {
            return Err(PortFailure::Domain(Diagnostic::new(
                "radicle_mutation_payload_substitution",
                "radicle_mutation.payload_identity",
                "Rendered patch review differs from the planned payload identity",
            )));
        }
        self.preview = Some(PreviewedMutation {
            plan: plan.clone(),
            payload,
        });
        Ok(())
    }

    fn dispatch(&mut self, plan: &CobEffectPlan) -> PortResult<CollaborationMutationOutcome> {
        let signer = self.signer.take().ok_or_else(|| {
            PortFailure::Domain(Diagnostic::new(
                "radicle_mutation_signer_missing",
                "radicle_mutation.signer",
                "Patch mutation dispatch requires the resolved signer",
            ))
        })?;
        let preview = self.preview.take().ok_or_else(|| {
            PortFailure::Domain(Diagnostic::new(
                "radicle_mutation_preview_missing",
                "radicle_mutation.preview",
                "Patch mutation dispatch requires an accepted preview",
            ))
        })?;
        if preview.plan != *plan {
            return Err(PortFailure::Domain(Diagnostic::new(
                "radicle_mutation_preview_substitution",
                "radicle_mutation.preview",
                "Patch mutation plan differs from the accepted preview",
            )));
        }

        let started = Instant::now();
        let observed = self.observe_bounded(plan)?;
        admit_exact_heads(plan, &observed.observation)?;
        admit_owned_head(&observed.entries, observed.head, signer.public_key())?;
        let action = admit_mutation_plan(&self.profile, plan).map_err(PortFailure::Domain)?;
        admit_review_revision(action, &observed.entries)?;

        let opened = self.open_repository(&plan.repository)?;
        if let Some(replay) = find_operation(
            opened.repository(),
            &observed.entries,
            plan,
            &preview.payload,
            signer.public_key(),
        )? {
            return Ok(if replay {
                CollaborationMutationOutcome::Applied(Box::new(observed.observation))
            } else {
                CollaborationMutationOutcome::ProviderError
            });
        }
        if deadline_expired(started, self.profile.deadlines.write_ms) {
            return Ok(CollaborationMutationOutcome::TimedOut);
        }

        let namespace = PublicKey::from_str(signer.public_key()).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "radicle_mutation_signer_invalid",
                "radicle_mutation.signer",
                "Resolved mutation signer has an invalid public identity",
            ))
        })?;
        let object_id = parse_object_id(plan)?;
        let type_name = patch_type_name()?;
        let repository = GitCobStore::with_expected_ref(opened.repository(), observed.head);
        let message = operation_message(plan);
        let result = collaboration::update::<NonEmpty<Entry>, GitCobStore>(
            &repository,
            signer.signing_key(),
            Some(observed.resource),
            Vec::new(),
            &namespace,
            Update {
                changes: NonEmpty::new(preview.payload),
                object_id,
                type_name,
                message,
                embeds: Vec::new(),
            },
        );

        #[cfg(feature = "status-native-fixtures")]
        if !self.fixture_delay_after_write.is_zero() {
            std::thread::sleep(self.fixture_delay_after_write);
        }

        let expired = deadline_expired(started, self.profile.deadlines.write_ms);
        let fact = match result {
            Ok(_) => WriteFact::Applied,
            Err(collaboration::error::Update::Refs { .. }) => WriteFact::AmbiguousAfterAcceptance,
            Err(error) => return Err(update_failure(error)),
        };
        match classify_write(expired, fact) {
            WriteClassification::Applied => match self.observe_bounded(plan) {
                Ok(observation) => Ok(CollaborationMutationOutcome::Applied(Box::new(
                    observation.observation,
                ))),
                Err(_) => Ok(CollaborationMutationOutcome::Unknown),
            },
            WriteClassification::ProviderError => Ok(CollaborationMutationOutcome::ProviderError),
            WriteClassification::TimedOut => Ok(CollaborationMutationOutcome::TimedOut),
            WriteClassification::Unknown => Ok(CollaborationMutationOutcome::Unknown),
        }
    }

    fn reconcile(&mut self, plan: &CobEffectPlan) -> PortResult<CollaborationMutationOutcome> {
        self.clear_authority();
        let action = admit_mutation_plan(&self.profile, plan).map_err(PortFailure::Domain)?;
        let payload = render_patch_review(action).map_err(PortFailure::Domain)?;
        let started = Instant::now();
        let signer = self
            .resolver
            .resolve("reconcile declared Radicle patch review")?;
        if deadline_expired(started, self.profile.deadlines.signer_ms) {
            return Err(timeout_failure(
                "radicle_mutation_signer_timeout",
                "Patch mutation reconciliation signer exceeded its named deadline",
            ));
        }
        if signer.public_key() != plan.actor.native_id.as_str() {
            return Err(PortFailure::Domain(Diagnostic::new(
                "radicle_mutation_signer_substitution",
                "radicle_mutation.actor",
                "Reconciliation signer differs from the exact planned actor",
            )));
        }
        let observed = self.observe_bounded(plan)?;
        let opened = self.open_repository(&plan.repository)?;
        match find_operation(
            opened.repository(),
            &observed.entries,
            plan,
            &payload,
            signer.public_key(),
        )? {
            Some(true) => Ok(CollaborationMutationOutcome::Applied(Box::new(
                observed.observation,
            ))),
            Some(false) => Ok(CollaborationMutationOutcome::ProviderError),
            None => Ok(CollaborationMutationOutcome::Unknown),
        }
    }
}

fn open_repository(path: &Path) -> PortResult<OpenedRepository> {
    let repository = git2::Repository::open(path).map_err(|error| {
        PortFailure::Infrastructure(InfrastructureFailure {
            code: "radicle_mutation_storage_unavailable".to_string(),
            message: format!("local Radicle mutation repository could not be opened: {error}"),
        })
    })?;
    let commit_config = tempfile::NamedTempFile::new().map_err(|error| {
        PortFailure::Infrastructure(InfrastructureFailure {
            code: "radicle_mutation_commit_config".to_string(),
            message: format!("temporary mutation commit configuration failed: {error}"),
        })
    })?;
    let mut config = git2::Config::open(commit_config.path()).map_err(config_failure)?;
    config
        .set_str("user.name", COMMIT_AUTHOR_NAME)
        .map_err(config_failure)?;
    config
        .set_str("user.email", COMMIT_AUTHOR_EMAIL)
        .map_err(config_failure)?;
    repository.set_config(&config).map_err(config_failure)?;
    Ok(OpenedRepository {
        repository,
        _commit_config: commit_config,
    })
}

fn load_history_bounded(
    repository: &GitCobStore<'_>,
    head: Oid,
    max_entries: usize,
    max_bytes: usize,
) -> PortResult<Vec<Entry>> {
    let mut pending = vec![head];
    let mut visited = BTreeSet::new();
    let mut entries = Vec::new();
    let mut total_bytes = 0usize;
    while let Some(id) = pending.pop() {
        if visited.contains(&id) {
            continue;
        }
        if visited.len() == max_entries {
            return Err(bound_failure("Patch history exceeds its entry bound"));
        }
        let entry = repository.load(id).map_err(|error| {
            PortFailure::Infrastructure(InfrastructureFailure {
                code: "radicle_mutation_history_unreadable".to_string(),
                message: format!("patch history entry could not be loaded: {error}"),
            })
        })?;
        for content in entry.contents() {
            total_bytes = total_bytes.checked_add(content.len()).ok_or_else(|| {
                bound_failure("Patch history byte count overflowed its representation")
            })?;
            if total_bytes > max_bytes {
                return Err(bound_failure("Patch history exceeds its byte bound"));
            }
            admit_existing_action(content)?;
        }
        pending.extend(entry.parents.iter().copied());
        visited.insert(id);
        entries.push(entry);
    }
    entries.sort_by_key(|entry| entry.id);
    Ok(entries)
}

fn consistent_resource(entries: &[Entry]) -> PortResult<Oid> {
    let resource = entries
        .iter()
        .find_map(|entry| entry.resource)
        .ok_or_else(|| {
            PortFailure::Domain(Diagnostic::new(
                "radicle_mutation_resource_missing",
                "radicle_mutation.history",
                "Patch history has no committed identity resource",
            ))
        })?;
    if entries.iter().any(|entry| entry.resource != Some(resource)) {
        return Err(PortFailure::Domain(Diagnostic::new(
            "radicle_mutation_resource_moved",
            "radicle_mutation.history",
            "Patch history contains inconsistent identity resources",
        )));
    }
    Ok(resource)
}

fn observation_from_history(
    plan: &CobEffectPlan,
    entries: &[Entry],
    head: Oid,
) -> PortResult<CobObservation> {
    let identity = history_identity(entries)?;
    Ok(CobObservation {
        profile_id: plan.profile_id.clone(),
        repository: plan.repository.clone(),
        type_name: ExternalRef::new("radicle", "cob-type", PATCH_TYPE_NAME)
            .map_err(PortFailure::Domain)?,
        schema_line: ExternalRef::new("radicle", "schema", PATCH_SCHEMA_LINE)
            .map_err(PortFailure::Domain)?,
        object_id: plan.object_id.clone(),
        heads: BTreeSet::from([ExternalRef::new("radicle", "operation", &head.to_string())
            .map_err(PortFailure::Domain)?]),
        semantic_state: CobSemanticStateId::new(identity.clone()).map_err(PortFailure::Domain)?,
        projection_identity: ContentId::from_blake3_hex(identity).map_err(PortFailure::Domain)?,
    })
}

fn history_identity(entries: &[Entry]) -> PortResult<String> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(HISTORY_DIGEST_DOMAIN);
    for entry in entries {
        hash_field(&mut hasher, entry.id.as_ref())?;
        let author = entry.author().to_string();
        hash_field(&mut hasher, author.as_bytes())?;
        if let Some(resource) = entry.resource {
            hash_field(&mut hasher, resource.as_ref())?;
        }
        for parent in &entry.parents {
            hash_field(&mut hasher, parent.as_ref())?;
        }
        for content in entry.contents() {
            hash_field(&mut hasher, content)?;
        }
    }
    Ok(hasher.finalize().to_string())
}

fn hash_field(hasher: &mut blake3::Hasher, bytes: &[u8]) -> PortResult<()> {
    let length =
        u64::try_from(bytes.len()).map_err(|_| bound_failure("History field is too large"))?;
    hasher.update(&length.to_be_bytes());
    hasher.update(bytes);
    Ok(())
}

fn admit_existing_action(bytes: &[u8]) -> PortResult<()> {
    let action: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "radicle_mutation_history_action",
            "radicle_mutation.history",
            "Patch history contains malformed action JSON",
        ))
    })?;
    let action_type = action
        .get("type")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            PortFailure::Domain(Diagnostic::new(
                "radicle_mutation_history_action",
                "radicle_mutation.history",
                "Patch history action has no type tag",
            ))
        })?;
    if !PATCH_ACTION_TYPES.contains(&action_type) {
        return Err(PortFailure::Domain(Diagnostic::new(
            "radicle_mutation_history_action",
            "radicle_mutation.history",
            "Patch history contains an action outside the pinned cohort",
        )));
    }
    if action_type == "revision" {
        let base = action.get("base").and_then(serde_json::Value::as_str);
        let oid = action.get("oid").and_then(serde_json::Value::as_str);
        if base.and_then(|value| Oid::from_str(value).ok()).is_none()
            || oid.and_then(|value| Oid::from_str(value).ok()).is_none()
        {
            return Err(PortFailure::Domain(Diagnostic::new(
                "radicle_mutation_history_revision",
                "radicle_mutation.history",
                "Patch history contains a malformed revision action",
            )));
        }
    }
    Ok(())
}

fn admit_exact_heads(plan: &CobEffectPlan, observed: &CobObservation) -> PortResult<()> {
    if observed.heads != plan.expected_heads {
        return Err(PortFailure::Domain(Diagnostic::new(
            "radicle_mutation_stale_head",
            "radicle_mutation.expected_heads",
            "Reloaded patch head differs from the exact planned head",
        )));
    }
    Ok(())
}

fn admit_owned_head(entries: &[Entry], head: Oid, signer: &str) -> PortResult<()> {
    let entry = entries
        .iter()
        .find(|entry| entry.id == head)
        .ok_or_else(|| {
            PortFailure::Domain(Diagnostic::new(
                "radicle_mutation_head_missing",
                "radicle_mutation.heads",
                "Current patch head is absent from the bounded history",
            ))
        })?;
    if entry.author().to_string() != signer {
        return Err(PortFailure::Domain(Diagnostic::new(
            "radicle_mutation_foreign_head",
            "radicle_mutation.heads",
            "Current patch head was not authored by the resolved Kiln signer",
        )));
    }
    Ok(())
}

fn admit_review_revision(action: &PatchReviewAction, entries: &[Entry]) -> PortResult<()> {
    let exists = entries.iter().any(|entry| {
        entry.contents().iter().any(|content| {
            serde_json::from_slice::<serde_json::Value>(content)
                .ok()
                .is_some_and(|value| {
                    value.get("type").and_then(serde_json::Value::as_str) == Some("revision")
                        && entry.id.to_string() == action.revision
                })
        })
    });
    if !exists {
        return Err(PortFailure::Domain(Diagnostic::new(
            "radicle_mutation_revision_missing",
            "radicle_mutation.review.revision",
            "Declared review revision is absent from patch history",
        )));
    }
    Ok(())
}

fn find_operation(
    repository: &git2::Repository,
    entries: &[Entry],
    plan: &CobEffectPlan,
    payload: &[u8],
    expected_author: &str,
) -> PortResult<Option<bool>> {
    let prefix = format!(
        "{OPERATION_MESSAGE_PREFIX} {} ",
        plan.idempotency_key.native_id.as_str()
    );
    let expected = operation_message(plan);
    for entry in entries {
        let commit = repository.find_commit(entry.id.into()).map_err(|error| {
            PortFailure::Infrastructure(InfrastructureFailure {
                code: "radicle_mutation_commit_unreadable".to_string(),
                message: format!("patch operation commit could not be read: {error}"),
            })
        })?;
        let message = commit
            .summary()
            .map_err(|error| {
                PortFailure::Infrastructure(InfrastructureFailure {
                    code: "radicle_mutation_commit_unreadable".to_string(),
                    message: format!("patch operation summary could not be read: {error}"),
                })
            })?
            .unwrap_or_default();
        if message == expected {
            let exact_payload = entry.contents().len() == 1
                && entry.contents().head.as_slice() == payload
                && blake3::hash(payload).to_string() == plan.payload_identity.as_str()
                && entry.author().to_string() == expected_author;
            return Ok(Some(exact_payload));
        }
        if message.starts_with(&prefix) {
            return Ok(Some(false));
        }
    }
    Ok(None)
}

fn operation_message(plan: &CobEffectPlan) -> String {
    format!(
        "{OPERATION_MESSAGE_PREFIX} {} {}",
        plan.idempotency_key.native_id.as_str(),
        plan.payload_identity.as_str()
    )
}

fn parse_object_id(plan: &CobEffectPlan) -> PortResult<ObjectId> {
    ObjectId::from_str(plan.object_id.as_str()).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "radicle_mutation_object_identity",
            "radicle_mutation.object_id",
            "Patch object identity is not a valid COB object ID",
        ))
    })
}

fn patch_type_name() -> PortResult<TypeName> {
    TypeName::from_str(PATCH_TYPE_NAME).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "radicle_mutation_type",
            "radicle_mutation.object_type",
            "Pinned patch object type is invalid",
        ))
    })
}

fn deadline_expired(started: Instant, deadline_ms: u64) -> bool {
    started.elapsed() > Duration::from_millis(deadline_ms)
}

fn timeout_failure(code: &str, message: &str) -> PortFailure {
    PortFailure::TimedOut(InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    })
}

fn update_failure(error: collaboration::error::Update) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: "radicle_mutation_update_failed".to_string(),
        message: format!("the pinned COB store rejected the patch review update: {error}"),
    })
}

fn storage_failure(error: GitCobStoreError) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: "radicle_mutation_refs".to_string(),
        message: error.to_string(),
    })
}

fn config_failure(error: git2::Error) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: "radicle_mutation_commit_config".to_string(),
        message: format!("deterministic mutation commit metadata failed: {error}"),
    })
}

fn bound_failure(message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(
        "radicle_mutation_bound",
        "radicle_mutation.history",
        message,
    ))
}
