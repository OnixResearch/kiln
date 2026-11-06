use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::str::FromStr;

use cob_git2 as git2;
use kiln_core::types::{Diagnostic, ExternalRef};
use kiln_provider_api::{InfrastructureFailure, PortFailure, PortResult};
use nonempty::NonEmpty;
use onix_radicle_cob_git_store::{GitCobStore, GitCobStoreError};
use radicle_cob::change::Storage as _;
use radicle_cob::object::Storage as _;
use radicle_cob::object::collaboration;
use radicle_cob::{Contents, Create, ObjectId, TypeName, Update, Version};
use radicle_crypto::PublicKey;
use radicle_oid::Oid;

use super::plan::{
    JobDispatch, JobObservation, JobPlan, JobState, JobUnknownObservation, admit_job_plan,
    required_actions, run_uuid,
};
use super::profile::RadicleJobProfile;
use super::reducer::{JobAction, JobCob};
use crate::status::shell::open_repository;
use crate::status::{ResolvedSigner, SignerResolver};

const COMMIT_AUTHOR_NAME: &str = "Kiln Jobs";
const COMMIT_AUTHOR_EMAIL: &str = "jobs@kiln.invalid";

const MAX_HISTORY_ENTRIES: usize = 1024;
const MAX_HISTORY_BYTES: usize = 16_777_216;
const MAX_HISTORY_HEADS: usize = 128;
const MAX_CHANGE_PARENTS: usize = 128;
const MAX_CHANGE_COMMIT_BYTES: usize = 65_536;
const MAX_CHANGE_TREE_BYTES: usize = 65_536;
const MAX_CHANGE_BLOB_BYTES: usize = 1_048_576;
const MAX_HISTORY_MEMBERS: usize = 4096;
const MAX_CHANGE_TREE_DEPTH: usize = 16;

// r[impl kiln.radicle.job.multi_writer]
// r[impl kiln.radicle.job.replay]
/// Bounded Radicle `xyz.radworks.job` publication shell.
pub struct RadicleJobShell {
    profile: RadicleJobProfile,
    allowed_rids: BTreeSet<String>,
    resolver: SignerResolver,
    storage_root: PathBuf,
}

impl std::fmt::Debug for RadicleJobShell {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RadicleJobShell")
            .field("profile", &self.profile)
            .field("storage_root", &self.storage_root)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum JobTarget {
    New,
    Existing {
        object_id: ObjectId,
        job: JobCob,
        our_head: Option<Oid>,
    },
}

impl RadicleJobShell {
    pub fn new(storage_root: PathBuf, profile: RadicleJobProfile) -> PortResult<Self> {
        let allowed_rids = profile.admit()?;
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
    pub fn new_fixture(storage_root: PathBuf, profile: RadicleJobProfile) -> PortResult<Self> {
        let allowed_rids = profile.admit_fixture()?;
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
    pub const fn profile(&self) -> &RadicleJobProfile {
        &self.profile
    }

    /// Bounded read-only multi-writer job projection; never resolves a signer.
    pub fn read(&self, repository: &ExternalRef, object: &ExternalRef) -> PortResult<JobCob> {
        if repository.provider_namespace.as_str() != "radicle"
            || repository.native_kind.as_str() != "repository"
            || object.provider_namespace.as_str() != "radicle"
            || object.native_kind.as_str() != "cob"
        {
            return Err(PortFailure::Domain(Diagnostic::new(
                "job_reference_invalid",
                "radicle_job.reference",
                "Job reads require a Radicle repository and COB reference",
            )));
        }
        if !self.allowed_rids.contains(repository.native_id.as_str()) {
            return Err(PortFailure::Domain(Diagnostic::new(
                "job_repository_denied",
                "radicle_job.repository",
                "Repository is not admitted",
            )));
        }
        let opened = open_repository(
            &self.storage_root,
            repository,
            COMMIT_AUTHOR_NAME,
            COMMIT_AUTHOR_EMAIL,
        )?;
        reduce(
            &GitCobStore::new(opened.repository()),
            JobCob::type_name_static(),
            parse_object_id(object)?,
        )
    }

    // r[impl kiln.radicle.job.authority]
    // r[impl kiln.radicle.job.identity]
    // r[impl kiln.radicle.job.bounds]
    // r[impl kiln.radicle.job.scenario.replay_conflict]
    pub fn publish(&self, plan: &JobPlan, reason: &str) -> PortResult<JobDispatch> {
        self.admit_plan(plan)?;
        let signer_identity = self.profile.signer_identity.clone();
        let namespace = PublicKey::from_str(signer_identity.native_id.as_str()).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "radicle_signer_identity_invalid",
                "radicle_job.signer",
                "Declared job signer identity did not parse as a node public key",
            ))
        })?;
        let revision = revision_oid(plan)?;
        let uuid = run_uuid(&plan.repository, &plan.revision, &plan.run_id);
        let opened = open_repository(
            &self.storage_root,
            &plan.repository,
            COMMIT_AUTHOR_NAME,
            COMMIT_AUTHOR_EMAIL,
        )?;
        let repo = opened.repository();
        let store = GitCobStore::new(repo);
        let target = self.resolve_target(&store, plan, &namespace, revision)?;

        let existing = match &target {
            JobTarget::New => None,
            JobTarget::Existing { job, .. } => job.run(&namespace.to_string(), &uuid).cloned(),
        };
        let actions = required_actions(plan, existing.as_ref()).map_err(PortFailure::Domain)?;
        if actions.is_empty() {
            let JobTarget::Existing {
                object_id,
                our_head: Some(head),
                ..
            } = &target
            else {
                return Err(PortFailure::Domain(Diagnostic::new(
                    "job_history_invalid",
                    "radicle_job.history",
                    "An idempotent replay has no Kiln namespace head",
                )));
            };
            return self
                .observe(repo, plan, *object_id, *head, &namespace)
                .map(JobDispatch::Written);
        }

        let encoded = encoded_actions(plan, &actions)?;
        let type_name = JobCob::type_name_static().clone();
        let signer = self.resolver.resolve(reason)?;
        if resolved_signer_identity(&signer)? != signer_identity {
            return Err(PortFailure::Domain(Diagnostic::new(
                "radicle_job_signer_mismatch",
                "radicle_job.signer_identity",
                "Resolved SecretSpec signer differs from the admitted job signer identity",
            )));
        }
        let resolved_namespace = public_key_of(&signer)?;
        if resolved_namespace != namespace {
            return Err(PortFailure::Domain(Diagnostic::new(
                "radicle_job_signer_mismatch",
                "radicle_job.signer_identity",
                "Resolved SecretSpec signer differs from the admitted job namespace",
            )));
        }

        let (object_id, mut head, mut expected) = match target {
            JobTarget::New => {
                let root = encode_action(plan, &JobAction::Request { oid: revision })?;
                let created = collaboration::create::<JobCob, GitCobStore>(
                    &store,
                    signer.signing_key(),
                    None,
                    Vec::new(),
                    &namespace,
                    Create {
                        contents: root,
                        type_name: type_name.clone(),
                        message: format!("kiln job {uuid}"),
                        embeds: Vec::new(),
                        version: Version::default(),
                    },
                );
                match created {
                    Ok(object) => {
                        let object_id: ObjectId = *object.id();
                        let head: Oid = *object_id;
                        (object_id, head, Some(head))
                    }
                    Err(collaboration::error::Create::Refs { .. }) => {
                        return Ok(JobDispatch::Unknown(unknown_observation(
                            plan, None, &uuid,
                        )?));
                    }
                    Err(error) => return Err(create_failure(error)),
                }
            }
            JobTarget::Existing {
                object_id,
                our_head,
                ..
            } => (object_id, our_head.unwrap_or(revision), our_head),
        };

        for payload in encoded {
            let write_store = match expected {
                Some(expected) => GitCobStore::with_expected_ref(repo, expected),
                None => GitCobStore::new(repo),
            };
            let updated = collaboration::update::<JobCob, GitCobStore>(
                &write_store,
                signer.signing_key(),
                None,
                Vec::new(),
                &namespace,
                Update {
                    changes: payload,
                    object_id,
                    type_name: type_name.clone(),
                    message: format!("kiln job {uuid}"),
                    embeds: Vec::new(),
                },
            );
            match updated {
                Ok(updated) => {
                    head = updated.head;
                    expected = Some(head);
                }
                Err(_) => {
                    return Ok(JobDispatch::Unknown(unknown_observation(
                        plan,
                        Some(object_id),
                        &uuid,
                    )?));
                }
            }
        }

        self.observe(repo, plan, object_id, head, &namespace)
            .map(JobDispatch::Written)
    }

    pub fn reconcile(&self, plan: &JobPlan) -> PortResult<Option<JobObservation>> {
        self.admit_plan(plan)?;
        let signer_identity = self.profile.signer_identity.clone();
        let namespace = PublicKey::from_str(signer_identity.native_id.as_str()).map_err(|_| {
            PortFailure::Domain(Diagnostic::new(
                "radicle_signer_identity_invalid",
                "radicle_job.signer",
                "Declared job signer identity did not parse as a node public key",
            ))
        })?;
        let revision = revision_oid(plan)?;
        let opened = open_repository(
            &self.storage_root,
            &plan.repository,
            COMMIT_AUTHOR_NAME,
            COMMIT_AUTHOR_EMAIL,
        )?;
        let repo = opened.repository();
        let store = GitCobStore::new(repo);
        let target = self.resolve_target(&store, plan, &namespace, revision)?;
        let JobTarget::Existing {
            object_id,
            job,
            our_head: Some(head),
        } = target
        else {
            return Ok(None);
        };
        let uuid = run_uuid(&plan.repository, &plan.revision, &plan.run_id);
        let existing = job.run(&namespace.to_string(), &uuid).cloned();
        if !required_actions(plan, existing.as_ref())
            .map_err(PortFailure::Domain)?
            .is_empty()
        {
            return Ok(None);
        }
        self.observe(repo, plan, object_id, head, &namespace)
            .map(Some)
    }

    fn admit_plan(&self, plan: &JobPlan) -> PortResult<()> {
        admit_job_plan(plan, &self.allowed_rids).map_err(PortFailure::Domain)?;
        if plan.max_payload_bytes > self.profile.max_payload_bytes {
            return Err(PortFailure::Domain(Diagnostic::new(
                "job_payload_bound",
                "job_plan.max_payload_bytes",
                "Radicle job payload bound exceeds the profile maximum",
            )));
        }
        Ok(())
    }

    fn resolve_target(
        &self,
        store: &GitCobStore<'_>,
        plan: &JobPlan,
        namespace: &PublicKey,
        revision: Oid,
    ) -> PortResult<JobTarget> {
        let type_name = JobCob::type_name_static().clone();
        if let Some(declared) = &plan.object_id {
            let object_id = parse_object_id(declared)?;
            let job = reduce(store, &type_name, object_id)?;
            if job.oid() != revision {
                return Err(PortFailure::Domain(Diagnostic::new(
                    "job_commit_mismatch",
                    "job_plan.object_id",
                    "Declared job object is bound to another commit",
                )));
            }
            let our_head = our_head(store, namespace, &type_name, &object_id)?;
            return Ok(JobTarget::Existing {
                object_id,
                job,
                our_head,
            });
        }

        let object_ids = store
            .object_ids_bounded(&type_name, self.profile.max_objects_scanned)
            .map_err(storage_failure)?;
        let mut matched = Vec::new();
        for object_id in object_ids {
            let job = reduce(store, &type_name, object_id)?;
            if job.oid() == revision {
                let our_head = our_head(store, namespace, &type_name, &object_id)?;
                matched.push((object_id, job, our_head));
            }
        }
        match matched.len() {
            0 => Ok(JobTarget::New),
            1 => {
                let (object_id, job, our_head) = matched.pop().unwrap_or_else(|| unreachable!());
                Ok(JobTarget::Existing {
                    object_id,
                    job,
                    our_head,
                })
            }
            _ => Err(PortFailure::Domain(Diagnostic::new(
                "job_identity_ambiguous",
                "job_plan.object_id",
                "More than one job object binds the requested commit",
            ))),
        }
    }

    fn observe(
        &self,
        repo: &git2::Repository,
        plan: &JobPlan,
        object_id: ObjectId,
        head: Oid,
        namespace: &PublicKey,
    ) -> PortResult<JobObservation> {
        let store = GitCobStore::new(repo);
        let type_name = JobCob::type_name_static().clone();
        let job = reduce(&store, &type_name, object_id)?;
        let uuid = run_uuid(&plan.repository, &plan.revision, &plan.run_id);
        let run = job.run(&namespace.to_string(), &uuid).ok_or_else(|| {
            PortFailure::Domain(Diagnostic::new(
                "job_run_missing",
                "radicle_job.run",
                "Published job object does not contain the Kiln run",
            ))
        })?;
        let state = match run.finished {
            None => JobState::Started,
            Some(reason) => JobState::Finished {
                succeeded: reason.succeeded(),
            },
        };
        if state != plan.state {
            return Err(PortFailure::Domain(Diagnostic::new(
                "job_run_state_mismatch",
                "radicle_job.run",
                "Reduced job run state differs from the admitted plan",
            )));
        }
        Ok(JobObservation {
            repository: plan.repository.clone(),
            revision: plan.revision.clone(),
            object_id: object_external_ref(object_id)?,
            head: head_external_ref(head)?,
            run_uuid: uuid,
            run_id: plan.run_id.as_str().to_string(),
            state,
            signer_identity: self.profile.signer_identity.clone(),
        })
    }
}

fn encoded_actions(plan: &JobPlan, actions: &[JobAction]) -> PortResult<Vec<Contents>> {
    actions
        .iter()
        .map(|action| encode_action(plan, action))
        .collect()
}

fn encode_action(plan: &JobPlan, action: &JobAction) -> PortResult<Contents> {
    let payload = serde_json::to_vec(action).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "job_action_encoding",
            "radicle_job.action",
            "The bounded job action could not be encoded",
        ))
    })?;
    if payload.is_empty() || payload.len() > plan.max_payload_bytes {
        return Err(PortFailure::Domain(Diagnostic::new(
            "job_payload_bound",
            "job_plan.payload",
            "Serialized job action is empty or exceeds its named bound",
        )));
    }
    Ok(NonEmpty::new(payload))
}

fn reduce(
    store: &GitCobStore<'_>,
    type_name: &TypeName,
    object_id: ObjectId,
) -> PortResult<JobCob> {
    // `objects` loads entries, so inspect raw refs and ODB headers first.
    let heads = raw_heads(store, type_name, &object_id)?;
    let mut pending: Vec<_> = heads.values().copied().collect();
    let mut parents = BTreeMap::new();
    let mut bytes = 0usize;
    let mut members = 0usize;
    while let Some(id) = pending.pop() {
        if parents.contains_key(&id) {
            continue;
        }
        if parents.len() >= MAX_HISTORY_ENTRIES {
            return Err(history_bound());
        }
        inspect_change(store.raw(), id, &mut bytes, &mut members)?;
        let entry = store.load(id).map_err(|_| history_bound())?;
        if !entry.valid_signatures() {
            return Err(history_bound());
        }
        for content in entry.contents() {
            serde_json::from_slice::<JobAction>(content).map_err(|_| history_bound())?;
        }
        pending.extend(entry.parents.iter().copied());
        parents.insert(id, entry.parents);
    }
    // A union containing the root is insufficient: every advertised branch
    // must independently descend from this exact object, even if signed.
    for head in heads.values() {
        let mut pending = vec![*head];
        let mut visited = BTreeSet::new();
        let mut connected = false;
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            if id == *object_id {
                connected = true;
                break;
            }
            pending.extend(parents.get(&id).ok_or_else(history_bound)?.iter().copied());
        }
        if !connected {
            return Err(history_bound());
        }
    }
    // Apply the shared store's manifest, signature and namespace/tip-author
    // checks only after the entire advertised history is bounded.
    store
        .objects(type_name, &object_id)
        .map_err(storage_failure)?;
    let object = collaboration::get::<JobCob, GitCobStore>(store, type_name, &object_id)
        .map_err(retrieve_failure)?
        .ok_or_else(|| {
            PortFailure::Domain(Diagnostic::new(
                "job_object_missing",
                "job_plan.object_id",
                "The job object has no reducible history",
            ))
        })?;
    // Compare the complete namespace/head map, not only our own ref. Native
    // actor-permission rejection may legitimately prune typed actions.
    if raw_heads(store, type_name, &object_id)? != heads {
        return Err(history_bound());
    }
    Ok(object.object().clone())
}

fn raw_heads(
    store: &GitCobStore<'_>,
    type_name: &TypeName,
    object_id: &ObjectId,
) -> PortResult<BTreeMap<String, Oid>> {
    let pattern = format!("refs/namespaces/*/refs/cobs/{type_name}/{object_id}");
    let mut heads = BTreeMap::new();
    for reference in store
        .raw()
        .references_glob(&pattern)
        .map_err(|_| history_bound())?
    {
        let reference = reference.map_err(|_| history_bound())?;
        if heads.len() >= MAX_HISTORY_HEADS {
            return Err(history_bound());
        }
        let name = reference.name().map_err(|_| history_bound())?;
        let namespace = name.split('/').nth(2).ok_or_else(history_bound)?;
        let namespace = PublicKey::from_str(namespace).map_err(|_| history_bound())?;
        if store
            .ref_name(&namespace, type_name, object_id)
            .map_err(storage_failure)?
            != name
        {
            return Err(history_bound());
        }
        heads.insert(
            name.to_string(),
            reference.target().ok_or_else(history_bound)?.into(),
        );
    }
    Ok(heads)
}

fn inspect_header(
    odb: &git2::Odb<'_>,
    id: git2::Oid,
    kind: git2::ObjectType,
    limit: usize,
    bytes: &mut usize,
) -> PortResult<()> {
    let (size, actual_kind) = odb.read_header(id).map_err(|_| history_bound())?;
    *bytes = bytes.checked_add(size).ok_or_else(history_bound)?;
    if actual_kind != kind || size > limit || *bytes > MAX_HISTORY_BYTES {
        return Err(history_bound());
    }
    Ok(())
}

fn inspect_change(
    repo: &git2::Repository,
    id: Oid,
    bytes: &mut usize,
    members: &mut usize,
) -> PortResult<()> {
    let odb = repo.odb().map_err(|_| history_bound())?;
    inspect_header(
        &odb,
        id.into(),
        git2::ObjectType::Commit,
        MAX_CHANGE_COMMIT_BYTES,
        bytes,
    )?;
    let commit = repo.find_commit(id.into()).map_err(|_| history_bound())?;
    if commit.parent_count() > MAX_CHANGE_PARENTS {
        return Err(history_bound());
    }
    let mut trees = vec![(commit.tree_id(), 0usize)];
    while let Some((id, depth)) = trees.pop() {
        if depth > MAX_CHANGE_TREE_DEPTH {
            return Err(history_bound());
        }
        inspect_header(
            &odb,
            id,
            git2::ObjectType::Tree,
            MAX_CHANGE_TREE_BYTES,
            bytes,
        )?;
        let tree = repo.find_tree(id).map_err(|_| history_bound())?;
        *members = members.checked_add(tree.len()).ok_or_else(history_bound)?;
        if *members > MAX_HISTORY_MEMBERS {
            return Err(history_bound());
        }
        for member in tree.iter() {
            match member.kind() {
                Some(git2::ObjectType::Tree) => trees.push((member.id(), depth + 1)),
                Some(git2::ObjectType::Blob) => inspect_header(
                    &odb,
                    member.id(),
                    git2::ObjectType::Blob,
                    MAX_CHANGE_BLOB_BYTES,
                    bytes,
                )?,
                _ => return Err(history_bound()),
            }
        }
    }
    Ok(())
}

fn history_bound() -> PortFailure {
    PortFailure::Domain(Diagnostic::new(
        "job_history_bound_or_signature",
        "radicle_job.history",
        "Job history exceeds its finite bound, is unreadable or has an invalid change signature",
    ))
}

fn our_head(
    store: &GitCobStore<'_>,
    namespace: &PublicKey,
    type_name: &TypeName,
    object_id: &ObjectId,
) -> PortResult<Option<Oid>> {
    let name = store
        .ref_name(namespace, type_name, object_id)
        .map_err(storage_failure)?;
    match store.raw().find_reference(&name) {
        Ok(reference) => Ok(reference.target().map(Oid::from)),
        Err(error) if error.code() == git2::ErrorCode::NotFound => Ok(None),
        Err(error) => Err(PortFailure::Infrastructure(InfrastructureFailure {
            code: "radicle_job_ref_unreadable".to_string(),
            message: format!("the Kiln job namespace reference could not be read: {error}"),
        })),
    }
}

fn revision_oid(plan: &JobPlan) -> PortResult<Oid> {
    Oid::from_str(plan.revision.native_id.as_str()).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "job_revision_identity",
            "job_plan.revision",
            "Radicle job revision did not parse as a Git commit identity",
        ))
    })
}

fn parse_object_id(object_id: &ExternalRef) -> PortResult<ObjectId> {
    ObjectId::from_str(object_id.native_id.as_str()).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "job_object_identity_invalid",
            "job_plan.object_id",
            "Declared job object identity did not parse",
        ))
    })
}

fn object_external_ref(object_id: ObjectId) -> PortResult<ExternalRef> {
    ExternalRef::new("radicle", "cob", &object_id.to_string()).map_err(PortFailure::Domain)
}

fn head_external_ref(head: Oid) -> PortResult<ExternalRef> {
    ExternalRef::new("radicle", "head", &head.to_string()).map_err(PortFailure::Domain)
}

fn resolved_signer_identity(signer: &ResolvedSigner) -> PortResult<ExternalRef> {
    ExternalRef::new("radicle", "node", signer.public_key()).map_err(PortFailure::Domain)
}

fn public_key_of(signer: &ResolvedSigner) -> PortResult<PublicKey> {
    PublicKey::from_str(signer.public_key()).map_err(|_| {
        PortFailure::Domain(Diagnostic::new(
            "radicle_signer_identity_invalid",
            "radicle_job.signer",
            "Resolved signer identity did not parse as a node public key",
        ))
    })
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
        "radicle_job.refs",
        &error.to_string(),
    ))
}

fn create_failure(error: collaboration::error::Create) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: "radicle_store_create_failed".to_string(),
        message: format!("the pinned COB store rejected the job create: {error}"),
    })
}

fn unknown_observation(
    plan: &JobPlan,
    object_id: Option<ObjectId>,
    uuid: &str,
) -> PortResult<JobUnknownObservation> {
    Ok(JobUnknownObservation {
        repository: plan.repository.clone(),
        revision: plan.revision.clone(),
        object_id: object_id.map(object_external_ref).transpose()?,
        run_uuid: uuid.to_string(),
        run_id: plan.run_id.as_str().to_string(),
    })
}

fn retrieve_failure(error: collaboration::error::Retrieve) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: "radicle_store_error".to_string(),
        message: format!("the pinned COB store rejected the job read: {error}"),
    })
}
