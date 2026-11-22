//! Consumer-owned bootstrap decisions over supplied observations.
//!
//! This module grants no path or filesystem authority. The shell must admit
//! locators through Cap Root, verify export facts, and observe publication.
//! A plan is not evidence that publication or synchronization occurred.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

pub const BOOTSTRAP_GUIDANCE_LOCATOR: &str = ".cairn/bootstrap/guidance.json";
pub const BOOTSTRAP_LOCK_LOCATOR: &str = ".cairn-bootstrap.lock";
pub const BOOTSTRAP_STAGE_LEAF: &str = ".cairn-bootstrap-stage";
pub const BOOTSTRAP_SCHEMA: &str = "cairn.consumer-bootstrap.v1";
pub const BOOTSTRAP_PREPARED_LOCATOR: &str = ".cairn/bootstrap/prepared.json";
pub const BOOTSTRAP_COMPLETE_LOCATOR: &str = ".cairn/bootstrap/complete.json";
pub const BOOTSTRAP_SELECTION_LOCATOR: &str = ".cairn/bootstrap/selection.json";
pub const MAX_MANAGED_ARTIFACTS: usize = 64;
pub const MAX_ARTIFACT_BYTES: u64 = 16_777_216;
pub const MAX_TOTAL_BYTES: u64 = 67_108_864;
pub const MAX_LABEL_BYTES: usize = 256;
pub const MAX_UPGRADE_SOURCES: usize = 64;
const DIGEST_HEX_BYTES: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BootstrapIdentity(String);

impl TryFrom<String> for BootstrapIdentity {
    type Error = BootstrapError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() != DIGEST_HEX_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(BootstrapError::InvalidIdentity);
        }
        Ok(Self(value))
    }
}

impl BootstrapIdentity {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(blake3::hash(bytes).to_hex().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<BootstrapIdentity> for String {
    fn from(value: BootstrapIdentity) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapError {
    InvalidIdentity,
    UnsupportedCohort,
    Bounds,
    DuplicateArtifact,
    InvalidSelection,
    StaleExport,
    UnsupportedUpgrade,
    Modified,
    Unmanaged,
    MissingManaged,
    UnsafePath,
    ObservationFailed,
    StalePlan,
    RecoveryRequired,
    ManualRepair,
}

impl std::fmt::Display for BootstrapError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "consumer bootstrap rejected: {self:?}")
    }
}

impl std::error::Error for BootstrapError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicySource {
    Explicit,
    Project,
    LegacyDefault,
}

/// Selection observations distinguish absence from an invalid selected input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyObservation {
    Absent,
    Valid(BootstrapIdentity),
    Invalid,
}

// r[impl cairn.consumer_bootstrap.policy]
pub fn select_policy(
    explicit: &PolicyObservation,
    project: &PolicyObservation,
    default: &PolicyObservation,
) -> Result<(PolicySource, BootstrapIdentity), BootstrapError> {
    for (source, observation) in [
        (PolicySource::Explicit, explicit),
        (PolicySource::Project, project),
        (PolicySource::LegacyDefault, default),
    ] {
        match observation {
            PolicyObservation::Absent => {}
            PolicyObservation::Valid(identity) => return Ok((source, identity.clone())),
            PolicyObservation::Invalid => return Err(BootstrapError::InvalidSelection),
        }
    }
    Err(BootstrapError::InvalidSelection)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapBounds {
    pub max_artifacts: usize,
    pub max_artifact_bytes: u64,
    pub max_total_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapCohort {
    pub schema: String,
    pub tool: BootstrapIdentity,
    pub policy: BootstrapIdentity,
    pub export: BootstrapIdentity,
    pub evaluator: BootstrapIdentity,
    pub source: BootstrapIdentity,
    pub workflow: BootstrapIdentity,
    pub profile: String,
    pub managed_locators: Vec<BootstrapIdentity>,
    pub upgrade_from: Vec<BootstrapIdentity>,
    pub bounds: BootstrapBounds,
}

/// Declared input mapping for a preview. Paths do not confer host authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapSourceArtifact {
    pub source: String,
    pub destination: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapSourceBundle {
    pub schema: String,
    pub cohort: BootstrapCohort,
    pub export_request: String,
    pub export_manifest: String,
    pub workflow_source: String,
    pub policy_locator: String,
    pub artifacts: Vec<BootstrapSourceArtifact>,
}

impl BootstrapSourceBundle {
    pub fn validate(&self) -> Result<(), BootstrapError> {
        if self.schema != "cairn.bootstrap-source-bundle.v1"
            || self.artifacts.is_empty()
            || self.artifacts.len() > MAX_MANAGED_ARTIFACTS
            || self.artifacts.len() > self.cohort.bounds.max_artifacts
        {
            return Err(BootstrapError::Bounds);
        }
        let mut destinations = BTreeSet::new();
        for locator in [
            &self.export_request,
            &self.export_manifest,
            &self.workflow_source,
            &self.policy_locator,
        ]
        .into_iter()
        .chain(
            self.artifacts
                .iter()
                .flat_map(|artifact| [&artifact.source, &artifact.destination]),
        ) {
            validate_locator(locator)?;
        }
        for artifact in &self.artifacts {
            if [
                BOOTSTRAP_PREPARED_LOCATOR,
                BOOTSTRAP_COMPLETE_LOCATOR,
                BOOTSTRAP_SELECTION_LOCATOR,
                BOOTSTRAP_LOCK_LOCATOR,
                BOOTSTRAP_GUIDANCE_LOCATOR,
            ]
            .contains(&artifact.destination.as_str())
                || artifact
                    .destination
                    .split('/')
                    .any(|part| part == BOOTSTRAP_STAGE_LEAF)
            {
                return Err(BootstrapError::InvalidSelection);
            }
            if !destinations.insert(&artifact.destination) {
                return Err(BootstrapError::DuplicateArtifact);
            }
        }
        for destination in &destinations {
            if destinations
                .iter()
                .any(|other| other.starts_with(&format!("{destination}/")))
            {
                return Err(BootstrapError::InvalidSelection);
            }
        }
        if !destinations.contains(&self.policy_locator) {
            return Err(BootstrapError::InvalidSelection);
        }
        Ok(())
    }
}

pub fn admit_bootstrap_policy(
    cohort: &BootstrapCohort,
    policy: &crate::Policy,
) -> Result<(), BootstrapError> {
    if !policy
        .workflow_profile_policy
        .allowed_profiles
        .contains(&cohort.profile)
        || !policy
            .workflow_schemas
            .iter()
            .any(|schema| schema.id == cohort.profile)
    {
        Err(BootstrapError::UnsupportedCohort)
    } else {
        Ok(())
    }
}

fn validate_locator(locator: &str) -> Result<(), BootstrapError> {
    if locator.is_empty()
        || locator.len() > MAX_LABEL_BYTES
        || !locator.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
        })
    {
        Err(BootstrapError::UnsafePath)
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapGuidanceDeclaration {
    pub schema: String,
    pub adapter: String,
    pub destination: String,
}

impl BootstrapGuidanceDeclaration {
    pub fn validate(&self) -> Result<(), BootstrapError> {
        if self.schema != "cairn.bootstrap-guidance.v1"
            || self.adapter.is_empty()
            || self.adapter.len() > MAX_LABEL_BYTES
            || !self
                .adapter
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(BootstrapError::InvalidSelection);
        }
        validate_locator(&self.destination)
    }
}

/// A checked runtime projection, not a declaration of filesystem authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapProjectDeclaration {
    pub schema: String,
    pub policy_locator: String,
    pub policy: BootstrapIdentity,
    pub cohort: BootstrapIdentity,
    pub complete_manifest: BootstrapIdentity,
}

impl BootstrapProjectDeclaration {
    // r[impl cairn.consumer_bootstrap.policy]
    pub fn validate(&self) -> Result<(), BootstrapError> {
        if self.schema != "cairn.bootstrap-project.v1"
            || self.policy_locator.is_empty()
            || self.policy_locator.len() > MAX_LABEL_BYTES
            || !self.policy_locator.split('/').all(|part| {
                !part.is_empty()
                    && part != "."
                    && part != ".."
                    && part.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-')
                    })
            })
        {
            return Err(BootstrapError::InvalidSelection);
        }
        Ok(())
    }
}

/// Complete selection metadata. The shell must verify its exact bytes and every
/// selected policy artifact; this structure alone is not execution evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapCompleteSelection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<BootstrapInstalledArtifact>,
    pub schema: String,
    pub cohort: BootstrapIdentity,
    pub policy: BootstrapIdentity,
    pub policy_locator: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapInstalledArtifact {
    pub destination: String,
    pub identity: BootstrapIdentity,
    pub bytes: u64,
}

impl BootstrapCompleteSelection {
    pub fn validate_bounds(&self, bounds: &BootstrapBounds) -> Result<(), BootstrapError> {
        let mut total = 0_u64;
        if self.artifacts.len() > bounds.max_artifacts {
            return Err(BootstrapError::Bounds);
        }
        for artifact in &self.artifacts {
            total = total
                .checked_add(artifact.bytes)
                .ok_or(BootstrapError::Bounds)?;
            if artifact.bytes > bounds.max_artifact_bytes || total > bounds.max_total_bytes {
                return Err(BootstrapError::Bounds);
            }
        }
        Ok(())
    }

    pub fn validate_for(
        &self,
        declaration: &BootstrapProjectDeclaration,
    ) -> Result<(), BootstrapError> {
        declaration.validate()?;
        if self.artifacts.len() > MAX_MANAGED_ARTIFACTS {
            return Err(BootstrapError::Bounds);
        }
        let mut destinations = BTreeSet::new();
        let mut total = 0_u64;
        for artifact in &self.artifacts {
            validate_locator(&artifact.destination)?;
            total = total
                .checked_add(artifact.bytes)
                .ok_or(BootstrapError::Bounds)?;
            if artifact.bytes > MAX_ARTIFACT_BYTES || total > MAX_TOTAL_BYTES {
                return Err(BootstrapError::Bounds);
            }
            if !destinations.insert(&artifact.destination) {
                return Err(BootstrapError::DuplicateArtifact);
            }
        }
        if self.schema != "cairn.bootstrap-complete-selection.v1"
            || self.cohort != declaration.cohort
            || self.policy != declaration.policy
            || self.policy_locator != declaration.policy_locator
        {
            return Err(BootstrapError::InvalidSelection);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapOwnership {
    pub locator: BootstrapIdentity,
    pub identity: BootstrapIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapUpgrade {
    pub from: BootstrapIdentity,
    pub to: BootstrapIdentity,
}

/// Independently observed identities, not assertions copied from the cohort.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CohortObservations {
    pub tool: BootstrapIdentity,
    pub policy: BootstrapIdentity,
    pub export: BootstrapIdentity,
    pub evaluator: BootstrapIdentity,
    pub source: BootstrapIdentity,
    pub workflow: BootstrapIdentity,
    pub export_current: bool,
}

// r[impl cairn.consumer_bootstrap.compatibility]
pub fn admit_cohort(
    cohort: &BootstrapCohort,
    observed: &CohortObservations,
    prior_cohort: Option<&BootstrapIdentity>,
) -> Result<(), BootstrapError> {
    let bounds = &cohort.bounds;
    if bounds.max_artifacts == 0
        || bounds.max_artifacts > MAX_MANAGED_ARTIFACTS
        || bounds.max_artifact_bytes == 0
        || bounds.max_artifact_bytes > MAX_ARTIFACT_BYTES
        || bounds.max_total_bytes < bounds.max_artifact_bytes
        || bounds.max_total_bytes > MAX_TOTAL_BYTES
        || cohort.upgrade_from.len() > MAX_UPGRADE_SOURCES
        || cohort.managed_locators.is_empty()
        || cohort.managed_locators.len() > bounds.max_artifacts
    {
        return Err(BootstrapError::Bounds);
    }
    if cohort.schema != BOOTSTRAP_SCHEMA
        || cohort.profile.is_empty()
        || cohort.profile.len() > MAX_LABEL_BYTES
        || !cohort
            .profile
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        || cohort.tool != observed.tool
        || cohort.policy != observed.policy
        || cohort.export != observed.export
        || cohort.evaluator != observed.evaluator
        || cohort.source != observed.source
        || cohort.workflow != observed.workflow
        || cohort.upgrade_from.iter().collect::<BTreeSet<_>>().len() != cohort.upgrade_from.len()
        || cohort
            .managed_locators
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != cohort.managed_locators.len()
    {
        return Err(BootstrapError::UnsupportedCohort);
    }
    if !observed.export_current {
        return Err(BootstrapError::StaleExport);
    }
    if let Some(prior) = prior_cohort {
        if prior != &cohort_identity(cohort) && !cohort.upgrade_from.contains(prior) {
            return Err(BootstrapError::UnsupportedUpgrade);
        }
    }
    Ok(())
}

fn frame(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

pub fn cohort_identity(cohort: &BootstrapCohort) -> BootstrapIdentity {
    let mut hasher = blake3::Hasher::new();
    frame(&mut hasher, BOOTSTRAP_SCHEMA.as_bytes());
    for text in [
        &cohort.schema,
        &cohort.tool.0,
        &cohort.policy.0,
        &cohort.export.0,
        &cohort.evaluator.0,
        &cohort.source.0,
        &cohort.workflow.0,
        &cohort.profile,
    ] {
        frame(&mut hasher, text.as_bytes());
    }
    frame(
        &mut hasher,
        &(cohort.managed_locators.len() as u64).to_le_bytes(),
    );
    for identity in &cohort.managed_locators {
        frame(&mut hasher, identity.0.as_bytes());
    }
    frame(
        &mut hasher,
        &(cohort.upgrade_from.len() as u64).to_le_bytes(),
    );
    for identity in &cohort.upgrade_from {
        frame(&mut hasher, identity.0.as_bytes());
    }
    for bound in [
        cohort.bounds.max_artifacts as u64,
        cohort.bounds.max_artifact_bytes,
        cohort.bounds.max_total_bytes,
    ] {
        frame(&mut hasher, &bound.to_le_bytes());
    }
    BootstrapIdentity(hasher.finalize().to_hex().to_string())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileObservation {
    Missing,
    Regular {
        identity: BootstrapIdentity,
        bytes: u64,
    },
    UnsafePath,
    Unreadable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedArtifact {
    /// Opaque shell-admitted locator identity, not a host path.
    pub locator: BootstrapIdentity,
    pub desired: BootstrapIdentity,
    pub desired_bytes: u64,
    pub previously_owned: Option<BootstrapIdentity>,
    pub observed: FileObservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationAction {
    Create,
    ReplaceOwned,
    Preserve,
}

// r[impl cairn.consumer_bootstrap.publication]
pub fn classify_artifact(artifact: &ManagedArtifact) -> Result<PublicationAction, BootstrapError> {
    match (&artifact.previously_owned, &artifact.observed) {
        (_, FileObservation::UnsafePath) => Err(BootstrapError::UnsafePath),
        (_, FileObservation::Unreadable) => Err(BootstrapError::ObservationFailed),
        (None, FileObservation::Missing) => Ok(PublicationAction::Create),
        (None, FileObservation::Regular { .. }) => Err(BootstrapError::Unmanaged),
        (Some(_), FileObservation::Missing) => Err(BootstrapError::MissingManaged),
        (Some(owned), FileObservation::Regular { identity, bytes }) => {
            if owned != identity {
                return Err(BootstrapError::Modified);
            }
            if identity == &artifact.desired {
                if *bytes != artifact.desired_bytes {
                    return Err(BootstrapError::ObservationFailed);
                }
                Ok(PublicationAction::Preserve)
            } else {
                Ok(PublicationAction::ReplaceOwned)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapPlan {
    identity: BootstrapIdentity,
    artifacts: Vec<ManagedArtifact>,
    actions: Vec<PublicationAction>,
}

impl BootstrapPlan {
    pub fn identity(&self) -> &BootstrapIdentity {
        &self.identity
    }
    pub fn actions(&self) -> &[PublicationAction] {
        &self.actions
    }
}

/// Bind the prior complete manifest, exact cohort, and all destination facts.
/// An existing prepared transaction blocks a new plan, including upgrades.
pub fn plan_bootstrap(
    cohort: &BootstrapCohort,
    observed: &CohortObservations,
    prior_cohort: Option<&BootstrapIdentity>,
    prior_manifest: Option<&BootstrapIdentity>,
    prepared: bool,
    artifacts: &[ManagedArtifact],
) -> Result<BootstrapPlan, BootstrapError> {
    admit_cohort(cohort, observed, prior_cohort)?;
    if prepared {
        return Err(BootstrapError::RecoveryRequired);
    }
    if prior_cohort.is_some() != prior_manifest.is_some() {
        return Err(BootstrapError::InvalidSelection);
    }
    if artifacts.is_empty() || artifacts.len() > cohort.bounds.max_artifacts {
        return Err(BootstrapError::Bounds);
    }
    if artifacts
        .iter()
        .map(|artifact| &artifact.locator)
        .collect::<Vec<_>>()
        != cohort.managed_locators.iter().collect::<Vec<_>>()
    {
        return Err(BootstrapError::InvalidSelection);
    }
    let mut total = 0_u64;
    let mut locators = BTreeSet::new();
    let mut actions = Vec::with_capacity(artifacts.len());
    let mut hasher = blake3::Hasher::new();
    frame(&mut hasher, b"cairn.bootstrap-plan.v1");
    frame(&mut hasher, cohort_identity(cohort).0.as_bytes());
    frame(
        &mut hasher,
        prior_manifest.map_or(&[][..], |id| id.0.as_bytes()),
    );
    frame(
        &mut hasher,
        prior_cohort.map_or(&[][..], |id| id.0.as_bytes()),
    );
    for artifact in artifacts {
        if !locators.insert(&artifact.locator) {
            return Err(BootstrapError::DuplicateArtifact);
        }
        total = total
            .checked_add(artifact.desired_bytes)
            .ok_or(BootstrapError::Bounds)?;
        if artifact.desired_bytes > cohort.bounds.max_artifact_bytes
            || total > cohort.bounds.max_total_bytes
        {
            return Err(BootstrapError::Bounds);
        }
        if prior_manifest.is_none() && artifact.previously_owned.is_some() {
            return Err(BootstrapError::InvalidSelection);
        }
        let action = classify_artifact(artifact)?;
        frame(&mut hasher, artifact.locator.0.as_bytes());
        frame(&mut hasher, artifact.desired.0.as_bytes());
        frame(&mut hasher, &artifact.desired_bytes.to_le_bytes());
        frame(
            &mut hasher,
            artifact
                .previously_owned
                .as_ref()
                .map_or(&[][..], |id| id.0.as_bytes()),
        );
        match &artifact.observed {
            FileObservation::Missing => frame(&mut hasher, b"missing"),
            FileObservation::Regular { identity, bytes } => {
                frame(&mut hasher, b"regular");
                frame(&mut hasher, identity.0.as_bytes());
                frame(&mut hasher, &bytes.to_le_bytes());
            }
            _ => return Err(BootstrapError::ObservationFailed),
        }
        actions.push(action);
    }
    Ok(BootstrapPlan {
        identity: BootstrapIdentity(hasher.finalize().to_hex().to_string()),
        artifacts: artifacts.to_vec(),
        actions,
    })
}

/// Keep the selected policy generation unchanged until the final pointer switch.
pub fn admit_generation_write(
    prior: Option<&BootstrapCompleteSelection>,
    destination: &str,
    action: PublicationAction,
) -> Result<(), BootstrapError> {
    if prior.is_some_and(|prior| prior.policy_locator == destination)
        && action == PublicationAction::ReplaceOwned
    {
        Err(BootstrapError::InvalidSelection)
    } else {
        Ok(())
    }
}

pub fn recheck_plan(
    reviewed: &BootstrapIdentity,
    current: &BootstrapPlan,
) -> Result<(), BootstrapError> {
    if reviewed == current.identity() {
        Ok(())
    } else {
        Err(BootstrapError::StalePlan)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryDecision {
    ResumeUnchanged,
    PublishCompleteManifest,
    RequireSynchronization,
    ManualRepair,
}

/// The shell must verify the saved plan identity before reconstructing this
/// plan. Durability evidence comes from the publication adapter, never hashes.
// r[impl cairn.consumer_bootstrap.publication]
pub fn recover_bootstrap(
    plan: &BootstrapPlan,
    prepared_plan: &BootstrapIdentity,
    current: &[FileObservation],
    synchronized: &[bool],
) -> RecoveryDecision {
    if prepared_plan != plan.identity()
        || current.len() != plan.artifacts.len()
        || synchronized.len() != current.len()
    {
        return RecoveryDecision::ManualRepair;
    }
    let mut incomplete = false;
    let mut needs_sync = false;
    for ((artifact, observed), durable) in plan.artifacts.iter().zip(current).zip(synchronized) {
        let is_desired = matches!(observed, FileObservation::Regular { identity, bytes }
            if identity == &artifact.desired && *bytes == artifact.desired_bytes);
        if is_desired {
            needs_sync |= !durable;
        } else if observed == &artifact.observed {
            incomplete = true;
        } else {
            return RecoveryDecision::ManualRepair;
        }
    }
    if incomplete {
        RecoveryDecision::ResumeUnchanged
    } else if needs_sync {
        RecoveryDecision::RequireSynchronization
    } else {
        RecoveryDecision::PublishCompleteManifest
    }
}

#[cfg(test)]
mod tests;
