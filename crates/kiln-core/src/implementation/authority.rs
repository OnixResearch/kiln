//! Closed protected-operation authority coverage.
//!
//! Every protected path in the selected implementation profile maps to exactly one
//! reviewed Basalt operation, one enforcement owner, one effect class and the
//! candidate rule. This module admits and reports that mapping; it never verifies
//! authority, resolves credentials, executes work or performs an effect.
//!
//! An unmapped path or operation denies. No operation implies another: claim,
//! dispatch and submission authority never stand in for verification, publication,
//! integration or lifecycle authority.

// The denial is the bounded redacted audit record itself; boxing it would add an
// allocation only on the denial path and break the single record-and-return shape.
#![allow(clippy::result_large_err)]

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::types::{ContentId, CoreResult, Diagnostic};

pub const IMPLEMENTATION_AUTHORITY_SCHEMA: &str = "kiln.implementation-authority-coverage.v2";
pub const MAX_OWNER_PATH_BYTES: usize = 256;

/// Closed protected implementation operations, one-to-one with the reviewed
/// Basalt adapter vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProtectedOperation {
    ClaimAcquire,
    ClaimRenew,
    Dispatch,
    CandidateSubmit,
    VerificationAdmit,
    AttemptCancel,
    PublicationPublish,
    IntegrationLand,
    LifecycleComplete,
}

impl ProtectedOperation {
    /// Every protected operation in review order.
    pub const ALL: [Self; 9] = [
        Self::ClaimAcquire,
        Self::ClaimRenew,
        Self::Dispatch,
        Self::CandidateSubmit,
        Self::VerificationAdmit,
        Self::AttemptCancel,
        Self::PublicationPublish,
        Self::IntegrationLand,
        Self::LifecycleComplete,
    ];

    /// Position in [`Self::ALL`] and [`COVERAGE`].
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::ClaimAcquire => 0,
            Self::ClaimRenew => 1,
            Self::Dispatch => 2,
            Self::CandidateSubmit => 3,
            Self::VerificationAdmit => 4,
            Self::AttemptCancel => 5,
            Self::PublicationPublish => 6,
            Self::IntegrationLand => 7,
            Self::LifecycleComplete => 8,
        }
    }

    /// Stable Basalt operation identifier. This MUST equal the reviewed adapter's
    /// serialized operation name; the adapter and this table are reviewed together.
    #[must_use]
    pub const fn basalt_operation(self) -> &'static str {
        match self {
            Self::ClaimAcquire => "claim-acquire",
            Self::ClaimRenew => "claim-renew",
            Self::Dispatch => "dispatch",
            Self::CandidateSubmit => "candidate-submit",
            Self::VerificationAdmit => "verification-admit",
            Self::AttemptCancel => "attempt-cancel",
            Self::PublicationPublish => "publication-publish",
            Self::IntegrationLand => "integration-land",
            Self::LifecycleComplete => "lifecycle-complete",
        }
    }

    /// Whether the operation binds an exact candidate identity.
    #[must_use]
    pub const fn requires_candidate(self) -> bool {
        match self {
            Self::ClaimAcquire | Self::ClaimRenew | Self::Dispatch | Self::AttemptCancel => false,
            Self::CandidateSubmit
            | Self::VerificationAdmit
            | Self::PublicationPublish
            | Self::IntegrationLand
            | Self::LifecycleComplete => true,
        }
    }
}

/// Enforcement owner for one protected operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EnforcementBoundary {
    Admission,
    Campaign,
    Verification,
    Publication,
    Lifecycle,
}

impl EnforcementBoundary {
    /// Stable boundary label for redacted decisions.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admission => "admission",
            Self::Campaign => "campaign",
            Self::Verification => "verification",
            Self::Publication => "publication",
            Self::Lifecycle => "lifecycle",
        }
    }
}

/// Effect class governed by the operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthorityEffect {
    Mutation,
    Publication,
    Integration,
    Lifecycle,
}

impl AuthorityEffect {
    /// Stable effect label for redacted decisions.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mutation => "mutation",
            Self::Publication => "publication",
            Self::Integration => "integration",
            Self::Lifecycle => "lifecycle",
        }
    }
}

/// One reviewed coverage entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverageEntry {
    /// Protected operation.
    pub operation: ProtectedOperation,
    /// Basalt operation identifier.
    pub basalt_operation: &'static str,
    /// Enforcement boundary.
    pub owner: EnforcementBoundary,
    /// Reviewed repository path that owns enforcement.
    pub owner_path: &'static str,
    /// Effect class.
    pub effect: AuthorityEffect,
    /// Whether the operation binds an exact candidate.
    pub candidate_bound: bool,
}

// r[impl kiln.implementation.authority.coverage]
/// Every protected operation with its reviewed owner and binding rule.
pub const COVERAGE: [CoverageEntry; 9] = [
    CoverageEntry {
        operation: ProtectedOperation::ClaimAcquire,
        basalt_operation: "claim-acquire",
        owner: EnforcementBoundary::Admission,
        owner_path: "crates/kiln-implementer/src/admission.rs",
        effect: AuthorityEffect::Mutation,
        candidate_bound: false,
    },
    CoverageEntry {
        operation: ProtectedOperation::ClaimRenew,
        basalt_operation: "claim-renew",
        owner: EnforcementBoundary::Admission,
        owner_path: "crates/kiln-implementer/src/lib.rs",
        effect: AuthorityEffect::Mutation,
        candidate_bound: false,
    },
    CoverageEntry {
        operation: ProtectedOperation::Dispatch,
        basalt_operation: "dispatch",
        owner: EnforcementBoundary::Admission,
        owner_path: "crates/kiln-implementer/src/admission.rs",
        effect: AuthorityEffect::Mutation,
        candidate_bound: false,
    },
    CoverageEntry {
        operation: ProtectedOperation::CandidateSubmit,
        basalt_operation: "candidate-submit",
        owner: EnforcementBoundary::Campaign,
        owner_path: "crates/kiln-implementer/src/lib.rs",
        effect: AuthorityEffect::Mutation,
        candidate_bound: true,
    },
    CoverageEntry {
        operation: ProtectedOperation::VerificationAdmit,
        basalt_operation: "verification-admit",
        owner: EnforcementBoundary::Verification,
        owner_path: "crates/kiln-implementer/src/lib.rs",
        effect: AuthorityEffect::Mutation,
        candidate_bound: true,
    },
    CoverageEntry {
        operation: ProtectedOperation::AttemptCancel,
        basalt_operation: "attempt-cancel",
        owner: EnforcementBoundary::Campaign,
        owner_path: "crates/kiln-implementer/src/lib.rs",
        effect: AuthorityEffect::Mutation,
        candidate_bound: false,
    },
    CoverageEntry {
        operation: ProtectedOperation::PublicationPublish,
        basalt_operation: "publication-publish",
        owner: EnforcementBoundary::Publication,
        owner_path: "crates/kiln-implementer/src/publication_effects.rs",
        effect: AuthorityEffect::Publication,
        candidate_bound: true,
    },
    CoverageEntry {
        operation: ProtectedOperation::IntegrationLand,
        basalt_operation: "integration-land",
        owner: EnforcementBoundary::Publication,
        owner_path: "crates/kiln-implementer/src/native_publication.rs",
        effect: AuthorityEffect::Integration,
        candidate_bound: true,
    },
    CoverageEntry {
        operation: ProtectedOperation::LifecycleComplete,
        basalt_operation: "lifecycle-complete",
        owner: EnforcementBoundary::Lifecycle,
        owner_path: "crates/kiln-implementer/src/native_publication.rs",
        effect: AuthorityEffect::Lifecycle,
        candidate_bound: true,
    },
];

/// Return the reviewed entry for one protected operation.
#[must_use]
pub fn coverage(operation: ProtectedOperation) -> &'static CoverageEntry {
    match COVERAGE.get(operation.index()) {
        Some(entry) => entry,
        None => unreachable!("closed protected-operation index"),
    }
}

/// Return the reviewed entry for one protected path, or `None` when unmapped.
#[must_use]
pub fn coverage_for_path(path: &str) -> Option<&'static CoverageEntry> {
    COVERAGE.iter().find(|entry| entry.owner_path == path)
}

/// Verify that a coverage table is closed, consistent and unambiguous.
///
/// # Errors
/// Returns a diagnostic for a missing or duplicate operation, a duplicate Basalt
/// operation identifier, an empty or oversized owner path, a mismatched Basalt
/// name, or a candidate rule that disagrees with the operation.
pub fn validate_coverage(entries: &[CoverageEntry]) -> CoreResult<()> {
    if entries.len() != ProtectedOperation::ALL.len() {
        return Err(Diagnostic::new(
            "authority_coverage_incomplete",
            "coverage",
            "coverage must list every protected operation exactly once",
        ));
    }
    for (index, entry) in entries.iter().enumerate() {
        if entry.owner_path.is_empty()
            || entry.owner_path.len() > MAX_OWNER_PATH_BYTES
            || entry.owner_path.trim() != entry.owner_path
        {
            return Err(Diagnostic::new(
                "authority_coverage_owner",
                "coverage.owner_path",
                "owner path must be a bounded non-empty repository path",
            ));
        }
        if entry.effect != authority_effect(entry.operation) {
            return Err(Diagnostic::new(
                "authority_coverage_effect",
                "coverage.effect",
                "effect class does not match the reviewed operation",
            ));
        }
        if entry.basalt_operation != entry.operation.basalt_operation() {
            return Err(Diagnostic::new(
                "authority_coverage_operation",
                "coverage.basalt_operation",
                "Basalt operation does not match the protected operation",
            ));
        }
        if entry.candidate_bound != entry.operation.requires_candidate() {
            return Err(Diagnostic::new(
                "authority_coverage_candidate",
                "coverage.candidate_bound",
                "candidate binding does not match the reviewed operation",
            ));
        }
        for other in entries.iter().skip(index + 1) {
            if other.operation == entry.operation {
                return Err(Diagnostic::new(
                    "authority_coverage_duplicate",
                    "coverage.operation",
                    "protected operation appears more than once",
                ));
            }
            if other.basalt_operation == entry.basalt_operation {
                return Err(Diagnostic::new(
                    "authority_coverage_duplicate",
                    "coverage.basalt_operation",
                    "Basalt operation id appears more than once",
                ));
            }
        }
    }
    for (position, operation) in ProtectedOperation::ALL.iter().enumerate() {
        let Some(entry) = entries.get(position) else {
            return Err(Diagnostic::new(
                "authority_coverage_incomplete",
                "coverage.operation",
                "coverage must list every protected operation exactly once",
            ));
        };
        if entry.operation != *operation {
            return Err(Diagnostic::new(
                "authority_coverage_order",
                "coverage.operation",
                "coverage order must match the closed operation order",
            ));
        }
    }
    Ok(())
}

const fn authority_effect(operation: ProtectedOperation) -> AuthorityEffect {
    match operation {
        ProtectedOperation::ClaimAcquire
        | ProtectedOperation::ClaimRenew
        | ProtectedOperation::Dispatch
        | ProtectedOperation::CandidateSubmit
        | ProtectedOperation::VerificationAdmit
        | ProtectedOperation::AttemptCancel => AuthorityEffect::Mutation,
        ProtectedOperation::PublicationPublish => AuthorityEffect::Publication,
        ProtectedOperation::IntegrationLand => AuthorityEffect::Integration,
        ProtectedOperation::LifecycleComplete => AuthorityEffect::Lifecycle,
    }
}

mod entry_points;
pub use entry_points::{ENTRY_POINTS, IMPLEMENTER_CLI};

/// Maximum bytes in one entry-point item name.
pub const MAX_ENTRY_ITEM_BYTES: usize = 128;
/// Maximum reviewed entry points.
pub const MAX_ENTRY_POINTS: usize = 512;

/// How one externally reachable entry point relates to protected authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryClass {
    /// Admits a fresh exact invocation of the operation before its effect.
    Protected(ProtectedOperation),
    /// Records or re-observes the outcome of an effect already admitted under
    /// the operation, bound to that effect's original identity. It never
    /// starts a new effect; where it adopts a provider outcome it presents a
    /// fresh invocation for the original effect identity.
    Reconcile(ProtectedOperation),
    /// Reads state only. It confers and consumes no authority.
    ObservationOnly,
    /// Operator-owned configuration, intake or ledger maintenance that the
    /// deployment authenticates outside implementation authority.
    OperatorConfig,
}

impl EntryClass {
    /// Stable redacted class label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Protected(_) => "protected",
            Self::Reconcile(_) => "reconcile",
            Self::ObservationOnly => "observation-only",
            Self::OperatorConfig => "operator-config",
        }
    }

    /// Operation the class names, if any.
    #[must_use]
    pub const fn operation(self) -> Option<ProtectedOperation> {
        match self {
            Self::Protected(operation) | Self::Reconcile(operation) => Some(operation),
            Self::ObservationOnly | Self::OperatorConfig => None,
        }
    }
}

/// One reviewed externally reachable entry point: an `Implementer` public
/// function, an implementer CLI subcommand, a fleet API route or a web control
/// route. An entry point that multiplexes several commands lists one row per
/// class it reaches, for example `admission_apply` admits claims, admits
/// dispatch requests and applies operator ledger configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryPoint {
    /// Repository path of the item.
    pub path: &'static str,
    /// Function name, subcommand or `METHOD route`.
    pub item: &'static str,
    /// Reviewed class.
    pub class: EntryClass,
}

// r[impl kiln.implementation.authority.coverage.entry_points]
/// Verify that an entry-point table is bounded, unique and covers every
/// protected operation at its reviewed owner.
///
/// # Errors
/// Returns a diagnostic for an empty or oversized table, an invalid path or
/// item, a duplicate `(path, item, class)` row, or a protected operation with
/// no `Protected` entry at its coverage owner path.
pub fn validate_entry_points(entries: &[EntryPoint]) -> CoreResult<()> {
    if entries.is_empty() || entries.len() > MAX_ENTRY_POINTS {
        return Err(Diagnostic::new(
            "authority_entry_points_bound",
            "entry_points",
            "entry-point table must be non-empty and bounded",
        ));
    }
    for (index, entry) in entries.iter().enumerate() {
        if entry.path.is_empty()
            || entry.path.len() > MAX_OWNER_PATH_BYTES
            || entry.path.trim() != entry.path
            || entry.item.is_empty()
            || entry.item.len() > MAX_ENTRY_ITEM_BYTES
            || entry.item.trim() != entry.item
        {
            return Err(Diagnostic::new(
                "authority_entry_point_name",
                "entry_points.item",
                "entry point path and item must be bounded non-empty trimmed text",
            ));
        }
        if entries.iter().skip(index + 1).any(|other| {
            other.path == entry.path && other.item == entry.item && other.class == entry.class
        }) {
            return Err(Diagnostic::new(
                "authority_entry_point_duplicate",
                "entry_points.item",
                "entry point row appears more than once",
            ));
        }
    }
    for operation in ProtectedOperation::ALL {
        let owner = coverage(operation).owner_path;
        if !entries
            .iter()
            .any(|entry| entry.path == owner && entry.class == EntryClass::Protected(operation))
        {
            return Err(Diagnostic::new(
                "authority_entry_point_unowned",
                "entry_points.class",
                "every protected operation needs a protected entry at its owner path",
            ));
        }
    }
    Ok(())
}

// r[impl kiln.implementation.authority.selection.scenario.scoped]
/// Holder roles one reviewed entry point can sign for: the role of every
/// operation its `Protected` and `Reconcile` rows name. An entry point with no
/// such row loads no holder key.
#[must_use]
pub fn entry_point_roles(
    entries: &[EntryPoint],
    path: &str,
    item: &str,
) -> alloc::collections::BTreeSet<super::grant::GrantRole> {
    entries
        .iter()
        .filter(|entry| entry.path == path && entry.item == item)
        .filter_map(|entry| entry.class.operation())
        .map(super::grant::GrantRole::of)
        .collect()
}

/// Domain of the canonical protected-payload digest.
pub const IMPLEMENTATION_AUTHORITY_PAYLOAD_SCHEMA: &str =
    "kiln.implementation-authority-payload.v2";
/// Incarnation label before the attempt's first transport launch is recorded.
pub const PRE_LAUNCH_INCARNATION: &str = "pre-launch";
/// Domain of a worker incarnation identity.
pub const INCARNATION_SCHEMA: &str = "kiln.implementation-incarnation.v1";

/// BLAKE3 identity of one recorded transport launch: the worker incarnation
/// bound into [`payload_digest_v2`]. Receivers recompute it from the launch.
///
/// # Errors
/// Returns the content-identity diagnostic; it does not occur for a BLAKE3 digest.
pub fn incarnation_identity(launch: &super::TransportLaunch) -> CoreResult<ContentId> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(INCARNATION_SCHEMA.as_bytes());
    hash_authority_text(&mut hasher, &launch.lattice_program);
    hash_authority_text(&mut hasher, launch.executable_blake3.as_str());
    hash_authority_text(&mut hasher, &launch.state_directory);
    ContentId::from_blake3_hex(hasher.finalize().to_hex().to_string())
}

/// Expected-target label for operations that change no remote target.
pub const NO_EXPECTED_TARGET: &str = "none";

// r[impl kiln.implementation.authority.payload]
/// Canonical digest of one protected payload.
///
/// The digest binds the operation, the worker incarnation (the attempt's
/// recorded transport launch, or [`PRE_LAUNCH_INCARNATION`]), the stable
/// effect identity, the expected remote target state and the exact payload
/// parts. Every field is length-framed; the receiving owner recomputes it from
/// its own facts. A fresh invocation of the same effect reuses the effect
/// identity; only its holder nonce differs.
#[must_use]
pub fn payload_digest_v2(
    operation: ProtectedOperation,
    incarnation: Option<&ContentId>,
    effect: &str,
    expected_target: &str,
    parts: &[&str],
) -> [u8; AUTHORITY_DIGEST_BYTES] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(IMPLEMENTATION_AUTHORITY_PAYLOAD_SCHEMA.as_bytes());
    hash_authority_text(&mut hasher, operation.basalt_operation());
    hash_authority_text(
        &mut hasher,
        incarnation.map_or(PRE_LAUNCH_INCARNATION, ContentId::as_str),
    );
    hash_authority_text(&mut hasher, effect);
    hash_authority_text(&mut hasher, expected_target);
    hasher.update(&(parts.len() as u64).to_be_bytes());
    for part in parts {
        hash_authority_text(&mut hasher, part);
    }
    *hasher.finalize().as_bytes()
}

/// Digest width used by authority request and decision identities.
pub const AUTHORITY_DIGEST_BYTES: usize = 32;
/// Maximum bytes in one caller-supplied authority binding field.
pub const MAX_AUTHORITY_FIELD_BYTES: usize = 512;
/// Maximum bytes in a redacted policy or upstream decision reference.
pub const MAX_AUTHORITY_REFERENCE_BYTES: usize = 128;
/// Maximum retained redacted decisions.
pub const MAX_AUTHORITY_DECISIONS: usize = 256;
/// Maximum compact-token bytes accepted by the credential profile.
pub const MAX_AUTHORITY_TOKEN_BYTES: usize = 16_384;
/// Holder-invocation nonce width.
pub const AUTHORITY_NONCE_BYTES: usize = 32;
/// Ed25519 holder-signature width.
pub const AUTHORITY_SIGNATURE_BYTES: usize = 64;
/// Redacted decision schema version.
pub const IMPLEMENTATION_AUTHORITY_DECISION_SCHEMA: &str =
    "kiln.implementation-authority-decision.v1";

// r[impl kiln.implementation.authority.binding]
/// One exact protected-operation request built from trusted owner facts.
///
/// The deployment audience, authority generation and expected holder are
/// supplied by the verified adapter, never by this caller-supplied structure.
/// Every field here is length-framed into the signed resource.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityRequest {
    /// Protected operation.
    pub operation: ProtectedOperation,
    /// Project identity.
    pub project: String,
    /// Work identity.
    pub work: String,
    /// Work version.
    pub work_version: u64,
    /// Attempt identity, or the work identity before an attempt exists.
    pub attempt: String,
    /// Attempt fence; zero only before an attempt exists.
    pub fence: u64,
    /// Digest of the exact payload the effect intends to execute.
    pub request_digest: [u8; AUTHORITY_DIGEST_BYTES],
    /// Canonical target.
    pub target: String,
    /// Exact candidate digest, or zero when the operation binds none.
    pub candidate: [u8; AUTHORITY_DIGEST_BYTES],
}

impl AuthorityRequest {
    /// Validate field bounds and the operation's candidate rule.
    ///
    /// # Errors
    /// Returns a diagnostic for a missing or oversized field, non-ASCII or
    /// control bytes, an invalid work version, or a candidate that is present
    /// when the operation binds none or absent when it binds one.
    pub fn validate(&self) -> CoreResult<()> {
        for (field, value) in [
            ("project", self.project.as_str()),
            ("work", self.work.as_str()),
            ("attempt", self.attempt.as_str()),
            ("target", self.target.as_str()),
        ] {
            if !valid_authority_text(value) {
                return Err(Diagnostic::new(
                    "authority_request_field",
                    field,
                    "authority binding field must be bounded non-empty trimmed ASCII",
                ));
            }
        }
        if self.work_version == 0 {
            return Err(Diagnostic::new(
                "authority_request_version",
                "work_version",
                "authority request requires a non-zero work version",
            ));
        }
        if self.operation.requires_candidate() != (self.candidate != [0; AUTHORITY_DIGEST_BYTES]) {
            return Err(Diagnostic::new(
                "authority_request_candidate",
                "candidate",
                "candidate presence must match the reviewed operation rule",
            ));
        }
        Ok(())
    }

    /// Canonical BLAKE3 identity of this request.
    ///
    /// # Errors
    /// Returns the validation diagnostic before hashing.
    pub fn identity(&self) -> CoreResult<ContentId> {
        self.validate()?;
        ContentId::from_blake3_hex(self.digest_hex())
    }

    /// Canonical BLAKE3 hex digest. Invalid requests still produce a stable
    /// redacted reference for denial decisions.
    #[must_use]
    pub fn digest_hex(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"kiln.implementation-authority-request.v1");
        hash_authority_text(&mut hasher, self.operation.basalt_operation());
        hash_authority_text(&mut hasher, &self.project);
        hash_authority_text(&mut hasher, &self.work);
        hasher.update(&self.work_version.to_be_bytes());
        hash_authority_text(&mut hasher, &self.attempt);
        hasher.update(&self.fence.to_be_bytes());
        hasher.update(&self.request_digest);
        hash_authority_text(&mut hasher, &self.target);
        hasher.update(&self.candidate);
        hasher.finalize().to_hex().to_string()
    }
}

/// One caller-presented authority profile, selected explicitly at composition.
pub enum AuthorityProfile {
    /// No credential was selected. Every protected operation denies.
    Missing,
    /// A signed UCAN holder invocation with its exact nonce and signature.
    VerifiedUcan(AuthorityCredential),
    /// An explicitly selected local-operator compatibility profile.
    LocalOperator {
        /// Operator identity retained in the decision provenance.
        operator: String,
    },
}

impl core::fmt::Debug for AuthorityProfile {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Missing => formatter.write_str("AuthorityProfile::Missing"),
            Self::VerifiedUcan(credential) => formatter
                .debug_struct("AuthorityProfile::VerifiedUcan")
                .field("subject", &credential.subject)
                .field("token_bytes", &credential.compact_token.len())
                .finish_non_exhaustive(),
            Self::LocalOperator { operator } => formatter
                .debug_struct("AuthorityProfile::LocalOperator")
                .field("operator", operator)
                .finish(),
        }
    }
}

// r[impl kiln.implementation.authority.selection]
/// Shell port that signs one exact request as the holder of the request's
/// role ([`GrantRole::of`](super::grant::GrantRole::of)).
///
/// Each call produces a single-use holder invocation with a fresh nonce over
/// the exact canonical request. Role keys stay inside the implementation;
/// a key for one role cannot sign another role's operations.
pub trait HolderInvocationSigner {
    /// Sign one exact request.
    ///
    /// # Errors
    /// Returns a diagnostic when the role has no key or grant, or signing fails.
    fn sign(&self, request: &AuthorityRequest) -> CoreResult<AuthorityCredential>;

    /// Whether this process loaded the holder key of `role`. An entry point
    /// loads only the roles its own protected operations need.
    fn holds(&self, role: super::grant::GrantRole) -> bool;
}

/// Authority profile selected once at composition for every protected
/// operation of one owner.
pub enum AuthoritySelection {
    /// No profile was selected. Every protected operation denies.
    Absent,
    /// The explicit local-operator compatibility profile.
    LocalOperator {
        /// Selected operator identity.
        operator: String,
    },
    /// The verified UCAN profile. Every request is signed afresh.
    Verified(Box<dyn HolderInvocationSigner>),
}

impl AuthoritySelection {
    /// The presented profile for one exact request. A signer failure presents
    /// no evidence, so the request denies as missing authority.
    #[must_use]
    pub fn profile_for(&self, request: &AuthorityRequest) -> AuthorityProfile {
        match self {
            Self::Absent => AuthorityProfile::Missing,
            Self::LocalOperator { operator } => AuthorityProfile::LocalOperator {
                operator: operator.clone(),
            },
            Self::Verified(signer) => signer
                .sign(request)
                .map_or(AuthorityProfile::Missing, AuthorityProfile::VerifiedUcan),
        }
    }

    // r[impl kiln.implementation.authority.selection.scenario.scoped]
    /// Whether this selection can present evidence for `operation` at all:
    /// the verified profile only for the holder roles this entry point
    /// loaded. The absent and local-operator profiles load no holder key.
    #[must_use]
    pub fn holds(&self, operation: ProtectedOperation) -> bool {
        match self {
            Self::Absent | Self::LocalOperator { .. } => true,
            Self::Verified(signer) => signer.holds(super::grant::GrantRole::of(operation)),
        }
    }

    /// Stable redacted profile label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::LocalOperator { .. } => "local-operator",
            Self::Verified(_) => "verified-ucan",
        }
    }
}

impl core::fmt::Debug for AuthoritySelection {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::LocalOperator { operator } => formatter
                .debug_struct("AuthoritySelection::LocalOperator")
                .field("operator", operator)
                .finish(),
            Self::Absent | Self::Verified(_) => formatter.write_str(self.label()),
        }
    }
}

/// Credential-bearing holder invocation. It has no serialization or clone surface.
pub struct AuthorityCredential {
    subject: String,
    compact_token: String,
    nonce: [u8; AUTHORITY_NONCE_BYTES],
    holder_signature: [u8; AUTHORITY_SIGNATURE_BYTES],
}

impl core::fmt::Debug for AuthorityCredential {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AuthorityCredential")
            .field("subject", &self.subject)
            .field("token_bytes", &self.compact_token.len())
            .finish_non_exhaustive()
    }
}

impl AuthorityCredential {
    /// Retain one exact holder invocation.
    ///
    /// # Errors
    /// Returns a diagnostic for a missing or oversized subject or token, or for
    /// non-ASCII token bytes.
    pub fn new(
        subject: impl Into<String>,
        compact_token: impl Into<String>,
        nonce: [u8; AUTHORITY_NONCE_BYTES],
        holder_signature: [u8; AUTHORITY_SIGNATURE_BYTES],
    ) -> CoreResult<Self> {
        let subject = subject.into();
        let compact_token = compact_token.into();
        if !valid_authority_text(&subject) {
            return Err(Diagnostic::new(
                "authority_credential_subject",
                "subject",
                "holder subject must be bounded non-empty trimmed ASCII",
            ));
        }
        if compact_token.is_empty()
            || compact_token.len() > MAX_AUTHORITY_TOKEN_BYTES
            || !compact_token.is_ascii()
            || compact_token.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(Diagnostic::new(
                "authority_credential_token",
                "compact_token",
                "compact token must be bounded printable ASCII",
            ));
        }
        Ok(Self {
            subject,
            compact_token,
            nonce,
            holder_signature,
        })
    }

    /// Exact asserted holder subject. The adapter checks it against the token.
    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// Compact token bytes.
    #[must_use]
    pub fn compact_token(&self) -> &str {
        &self.compact_token
    }

    /// Exact holder-invocation nonce.
    #[must_use]
    pub const fn nonce(&self) -> &[u8; AUTHORITY_NONCE_BYTES] {
        &self.nonce
    }

    /// Exact holder signature.
    #[must_use]
    pub const fn holder_signature(&self) -> &[u8; AUTHORITY_SIGNATURE_BYTES] {
        &self.holder_signature
    }
}

/// Currentness of the attempt named by an authority request.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttemptCurrentness {
    /// The attempt exists and is current, or the operation precedes an attempt.
    Current,
    /// The attempt was cancelled.
    Cancelled,
    /// The attempt was superseded.
    Superseded,
    /// A later fence replaced the attempt.
    FenceReplaced,
    /// The attempt lease expired.
    LeaseExpired,
    /// Currentness could not be read. New effects fail closed.
    Unavailable,
}

impl AttemptCurrentness {
    /// Stable redacted label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Cancelled => "cancelled",
            Self::Superseded => "superseded",
            Self::FenceReplaced => "fence-replaced",
            Self::LeaseExpired => "lease-expired",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Provenance of an allowed or denied decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthorityProvenance {
    /// No authority profile produced the decision.
    None,
    /// Verified UCAN holder invocation.
    VerifiedUcan,
    /// Explicitly selected local-operator compatibility profile.
    LocalOperator,
}

impl AuthorityProvenance {
    /// Stable redacted label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::VerifiedUcan => "verified-ucan",
            Self::LocalOperator => "local-operator",
        }
    }
}

/// Closed decision reason. `Allowed` is the only admitting reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthorityDenialReason {
    /// The exact request passed every check.
    Allowed,
    /// No authority profile was selected.
    MissingEvidence,
    /// A caller supplied a role label or other non-credential.
    RoleOnly,
    /// The local-operator profile was used but not selected.
    LocalOperatorNotSelected,
    /// The local-operator identity differs from the selected operator.
    PrincipalMismatch,
    /// Projection-enabled routes refuse the local-operator profile.
    ProjectionRequiresOperator,
    /// The verified adapter has no configured verifier services.
    VerifierUnavailable,
    /// The credential was malformed, unsigned or otherwise denied upstream.
    CredentialDenied,
    /// A bound field differs from the admitted session.
    BindingMismatch,
    /// Currentness is cancelled, superseded, replaced, expired or unavailable.
    AttemptNotCurrent,
    /// The session is absent or already closed.
    SessionNotFound,
    /// Revalidation failed.
    RevalidationDenied,
    /// Request fields are invalid.
    RequestInvalid,
    /// A bounded decision log or session cache limit was exceeded.
    LimitExceeded,
}

impl AuthorityDenialReason {
    /// Stable redacted label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::MissingEvidence => "missing-evidence",
            Self::RoleOnly => "role-only",
            Self::LocalOperatorNotSelected => "local-operator-not-selected",
            Self::PrincipalMismatch => "principal-mismatch",
            Self::ProjectionRequiresOperator => "projection-requires-operator",
            Self::VerifierUnavailable => "verifier-unavailable",
            Self::CredentialDenied => "credential-denied",
            Self::BindingMismatch => "binding-mismatch",
            Self::AttemptNotCurrent => "attempt-not-current",
            Self::SessionNotFound => "session-not-found",
            Self::RevalidationDenied => "revalidation-denied",
            Self::RequestInvalid => "request-invalid",
            Self::LimitExceeded => "limit-exceeded",
        }
    }
}

/// Opaque admitted-session handle retained by the adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuthoritySessionId(pub u64);

// r[impl kiln.implementation.authority.enforcement.scenario.replay]
/// Bounded, redacted authority decision. It contains no capability material.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityDecision {
    /// Decision schema.
    pub schema: String,
    /// Whether the exact request was admitted.
    pub allowed: bool,
    /// Protected operation.
    pub operation: ProtectedOperation,
    /// Enforcement boundary.
    pub boundary: EnforcementBoundary,
    /// Effect class.
    pub effect: AuthorityEffect,
    /// Decision provenance.
    pub provenance: AuthorityProvenance,
    /// Redacted principal or local operator.
    pub principal: String,
    /// Deployment audience.
    pub audience: String,
    /// Authority generation.
    pub generation: u64,
    /// Reviewed policy artifact reference.
    pub policy_ref: String,
    /// Upstream redacted receipt reference.
    pub upstream_decision: String,
    /// BLAKE3 identity of the exact request.
    pub request_identity: String,
    /// Admitted-session handle, present only when allowed.
    pub session: Option<AuthoritySessionId>,
    /// Closed reason.
    pub reason: AuthorityDenialReason,
}

impl AuthorityDecision {
    /// Build an allowed decision for one exact request and live session.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn allowed(
        request: &AuthorityRequest,
        session: AuthoritySessionId,
        provenance: AuthorityProvenance,
        principal: String,
        audience: String,
        generation: u64,
        policy_ref: String,
        upstream_decision: String,
    ) -> Self {
        let entry = coverage(request.operation);
        Self {
            schema: IMPLEMENTATION_AUTHORITY_DECISION_SCHEMA.into(),
            allowed: true,
            operation: request.operation,
            boundary: entry.owner,
            effect: entry.effect,
            provenance,
            principal,
            audience,
            generation,
            policy_ref,
            upstream_decision,
            request_identity: request.digest_hex(),
            session: Some(session),
            reason: AuthorityDenialReason::Allowed,
        }
    }

    /// Build a denied decision for one request without any credential material.
    #[must_use]
    pub fn deny(request: &AuthorityRequest, reason: AuthorityDenialReason) -> Self {
        let entry = coverage(request.operation);
        Self {
            schema: IMPLEMENTATION_AUTHORITY_DECISION_SCHEMA.into(),
            allowed: false,
            operation: request.operation,
            boundary: entry.owner,
            effect: entry.effect,
            provenance: AuthorityProvenance::None,
            principal: String::new(),
            audience: String::new(),
            generation: 0,
            policy_ref: String::new(),
            upstream_decision: String::new(),
            request_identity: request.digest_hex(),
            session: None,
            reason,
        }
    }

    /// Canonical BLAKE3 identity of the decision.
    #[must_use]
    pub fn digest_hex(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.schema.as_bytes());
        hasher.update(&[u8::from(self.allowed)]);
        hash_authority_text(&mut hasher, self.operation.basalt_operation());
        hash_authority_text(&mut hasher, self.boundary.as_str());
        hash_authority_text(&mut hasher, self.effect.as_str());
        hash_authority_text(&mut hasher, self.provenance.as_str());
        hash_authority_text(&mut hasher, &self.principal);
        hash_authority_text(&mut hasher, &self.audience);
        hasher.update(&self.generation.to_be_bytes());
        hash_authority_text(&mut hasher, &self.policy_ref);
        hash_authority_text(&mut hasher, &self.upstream_decision);
        hash_authority_text(&mut hasher, &self.request_identity);
        hash_authority_text(&mut hasher, self.reason.as_str());
        hasher.finalize().to_hex().to_string()
    }
}

// r[impl kiln.implementation.authority.enforcement]
/// Provider-neutral protected-authority port.
///
/// The adapter owns live sessions; the shell owns current work, attempt and
/// fence facts. Neither a request, a decision nor a receipt can be replayed as
/// authority.
pub trait ProtectedAuthority {
    /// Verify one exact request and retain a live session when allowed.
    ///
    /// # Errors
    /// Returns the redacted denial decision before any effect.
    fn admit(
        &mut self,
        request: &AuthorityRequest,
        profile: &AuthorityProfile,
        currentness: AttemptCurrentness,
        now_unix_s: u64,
    ) -> core::result::Result<AuthorityDecision, AuthorityDecision>;

    /// Recheck one live session without consuming replay again.
    ///
    /// # Errors
    /// Returns the redacted denial decision before the next effect.
    fn revalidate(
        &mut self,
        session: AuthoritySessionId,
        request: &AuthorityRequest,
        currentness: AttemptCurrentness,
        now_unix_s: u64,
    ) -> core::result::Result<(), AuthorityDecision>;

    /// Discard one live session.
    fn close(&mut self, session: AuthoritySessionId);

    /// Bounded redacted decision log.
    fn decisions(&self) -> &[AuthorityDecision];
}

/// Maximum live sessions retained by one adapter.
pub const MAX_AUTHORITY_SESSIONS: usize = 64;

struct LocalOperatorSession {
    request: AuthorityRequest,
    principal: String,
}

/// Explicit local-operator compatibility guard.
///
/// This guard admits only the selected local-operator principal. It denies
/// projection-enabled routes, verified credentials and every missing profile.
pub struct LocalOperatorAuthority {
    operator: String,
    projection_enabled: bool,
    sessions: BTreeMap<u64, LocalOperatorSession>,
    next_session: u64,
    decisions: Vec<AuthorityDecision>,
}

impl core::fmt::Debug for LocalOperatorAuthority {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("LocalOperatorAuthority")
            .field("operator", &self.operator)
            .field("projection_enabled", &self.projection_enabled)
            .field("sessions", &self.sessions.len())
            .field("decisions", &self.decisions.len())
            .finish_non_exhaustive()
    }
}

impl LocalOperatorAuthority {
    /// Select the local-operator compatibility profile explicitly.
    ///
    /// # Errors
    /// Returns a diagnostic for an invalid operator identity.
    pub fn new(operator: impl Into<String>, projection_enabled: bool) -> CoreResult<Self> {
        let operator = operator.into();
        if !valid_authority_text(&operator) {
            return Err(Diagnostic::new(
                "authority_local_operator",
                "operator",
                "local operator must be bounded non-empty trimmed ASCII",
            ));
        }
        Ok(Self {
            operator,
            projection_enabled,
            sessions: BTreeMap::new(),
            next_session: 1,
            decisions: Vec::new(),
        })
    }

    fn policy_reference(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"kiln.local-operator-policy.v1");
        hasher.update(self.operator.as_bytes());
        hasher.update(&[u8::from(self.projection_enabled)]);
        format!("local-operator:{}", hasher.finalize().to_hex())
    }

    fn upstream_reference(&self, request: &AuthorityRequest) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"kiln.local-operator-decision.v1");
        hasher.update(self.operator.as_bytes());
        hasher.update(request.digest_hex().as_bytes());
        hasher.finalize().to_hex().to_string()
    }

    fn record(&mut self, decision: AuthorityDecision) -> AuthorityDecision {
        if self.decisions.len() >= MAX_AUTHORITY_DECISIONS {
            self.decisions.remove(0);
        }
        self.decisions.push(decision.clone());
        decision
    }

    fn allowed(&self, request: &AuthorityRequest, session: u64) -> AuthorityDecision {
        AuthorityDecision::allowed(
            request,
            AuthoritySessionId(session),
            AuthorityProvenance::LocalOperator,
            self.operator.clone(),
            "local-operator".into(),
            0,
            self.policy_reference(),
            self.upstream_reference(request),
        )
    }

    fn find_session(&self, request: &AuthorityRequest) -> Option<u64> {
        let digest = request.digest_hex();
        self.sessions
            .iter()
            .find(|(_, live)| live.request.digest_hex() == digest)
            .map(|(id, _)| *id)
    }
}

// r[impl kiln.implementation.authority.compatibility]
impl ProtectedAuthority for LocalOperatorAuthority {
    fn admit(
        &mut self,
        request: &AuthorityRequest,
        profile: &AuthorityProfile,
        currentness: AttemptCurrentness,
        _now_unix_s: u64,
    ) -> core::result::Result<AuthorityDecision, AuthorityDecision> {
        if request.validate().is_err() {
            return Err(self.record(AuthorityDecision::deny(
                request,
                AuthorityDenialReason::RequestInvalid,
            )));
        }
        if currentness != AttemptCurrentness::Current {
            return Err(self.record(AuthorityDecision::deny(
                request,
                AuthorityDenialReason::AttemptNotCurrent,
            )));
        }
        let AuthorityProfile::LocalOperator { operator } = profile else {
            let reason = match profile {
                AuthorityProfile::Missing => AuthorityDenialReason::MissingEvidence,
                AuthorityProfile::VerifiedUcan(_) | AuthorityProfile::LocalOperator { .. } => {
                    AuthorityDenialReason::VerifierUnavailable
                }
            };
            return Err(self.record(AuthorityDecision::deny(request, reason)));
        };
        if self.projection_enabled {
            return Err(self.record(AuthorityDecision::deny(
                request,
                AuthorityDenialReason::ProjectionRequiresOperator,
            )));
        }
        if operator != &self.operator {
            return Err(self.record(AuthorityDecision::deny(
                request,
                AuthorityDenialReason::PrincipalMismatch,
            )));
        }
        if let Some(existing) = self.find_session(request) {
            return match self.revalidate(
                AuthoritySessionId(existing),
                request,
                currentness,
                _now_unix_s,
            ) {
                Ok(()) => Ok(self.allowed(request, existing)),
                Err(decision) => Err(decision),
            };
        }
        if self.sessions.len() >= MAX_AUTHORITY_SESSIONS
            && let Some(oldest) = self.sessions.keys().next().copied()
        {
            self.sessions.remove(&oldest);
        }
        let id = self.next_session;
        self.next_session = self.next_session.saturating_add(1);
        let decision = self.allowed(request, id);
        self.sessions.insert(
            id,
            LocalOperatorSession {
                request: request.clone(),
                principal: self.operator.clone(),
            },
        );
        Ok(self.record(decision))
    }

    fn revalidate(
        &mut self,
        session: AuthoritySessionId,
        request: &AuthorityRequest,
        currentness: AttemptCurrentness,
        _now_unix_s: u64,
    ) -> core::result::Result<(), AuthorityDecision> {
        if currentness != AttemptCurrentness::Current {
            self.sessions.remove(&session.0);
            return Err(self.record(AuthorityDecision::deny(
                request,
                AuthorityDenialReason::AttemptNotCurrent,
            )));
        }
        let Some(live) = self.sessions.get(&session.0) else {
            return Err(self.record(AuthorityDecision::deny(
                request,
                AuthorityDenialReason::SessionNotFound,
            )));
        };
        if live.request.digest_hex() != request.digest_hex()
            || live.request.operation != request.operation
            || live.principal != self.operator
        {
            self.sessions.remove(&session.0);
            return Err(self.record(AuthorityDecision::deny(
                request,
                AuthorityDenialReason::BindingMismatch,
            )));
        }
        let decision = self.allowed(request, session.0);
        self.record(decision);
        Ok(())
    }

    fn close(&mut self, session: AuthoritySessionId) {
        self.sessions.remove(&session.0);
    }

    fn decisions(&self) -> &[AuthorityDecision] {
        &self.decisions
    }
}

fn valid_authority_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_AUTHORITY_FIELD_BYTES
        && value.is_ascii()
        && value.trim() == value
        && !value.bytes().any(|byte| byte.is_ascii_control())
}

fn hash_authority_text(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // r[verify kiln.implementation.authority.coverage.scenario.complete]
    fn coverage_is_complete_closed_and_unique() {
        assert_eq!(COVERAGE.len(), ProtectedOperation::ALL.len());
        validate_coverage(&COVERAGE).unwrap_or_else(|error| unreachable!("{error:?}"));
        for (position, operation) in ProtectedOperation::ALL.iter().enumerate() {
            let entry = coverage(*operation);
            assert_eq!(entry.operation, *operation);
            assert_eq!(entry.basalt_operation, operation.basalt_operation());
            assert_eq!(entry.candidate_bound, operation.requires_candidate());
            assert!(!entry.owner_path.is_empty());
            assert_eq!(
                COVERAGE.get(position).map(|entry| entry.operation),
                Some(*operation)
            );
        }
        assert_eq!(
            COVERAGE
                .iter()
                .map(|entry| entry.basalt_operation)
                .collect::<alloc::vec::Vec<_>>(),
            alloc::vec![
                "claim-acquire",
                "claim-renew",
                "dispatch",
                "candidate-submit",
                "verification-admit",
                "attempt-cancel",
                "publication-publish",
                "integration-land",
                "lifecycle-complete",
            ]
        );
    }

    #[test]
    // r[verify kiln.implementation.authority.coverage.scenario.unmapped]
    fn unmapped_protected_path_denies() {
        assert!(coverage_for_path("crates/kiln-implementer/src/unknown.rs").is_none());
        assert_eq!(
            coverage_for_path("crates/kiln-implementer/src/admission.rs")
                .map(|entry| entry.operation),
            Some(ProtectedOperation::ClaimAcquire)
        );
    }

    #[test]
    fn inconsistent_coverage_is_rejected() {
        let mut duplicate = COVERAGE;
        duplicate[1].operation = ProtectedOperation::ClaimAcquire;
        assert_eq!(
            validate_coverage(&duplicate).err().map(|error| error.code),
            Some("authority_coverage_duplicate".into())
        );
        let mut candidate = COVERAGE;
        candidate[0].candidate_bound = true;
        assert_eq!(
            validate_coverage(&candidate).err().map(|error| error.code),
            Some("authority_coverage_candidate".into())
        );
        let mut effect = COVERAGE;
        effect[3].effect = AuthorityEffect::Publication;
        assert_eq!(
            validate_coverage(&effect).err().map(|error| error.code),
            Some("authority_coverage_effect".into())
        );
        assert_eq!(
            validate_coverage(&[]).err().map(|error| error.code),
            Some("authority_coverage_incomplete".into())
        );
    }

    #[test]
    // r[verify kiln.implementation.authority.coverage.entry_points]
    fn reviewed_entry_points_cover_every_operation_owner() {
        validate_entry_points(ENTRY_POINTS).unwrap_or_else(|error| unreachable!("{error:?}"));
        for operation in ProtectedOperation::ALL {
            assert!(
                ENTRY_POINTS
                    .iter()
                    .any(|entry| entry.class == EntryClass::Protected(operation)),
                "{operation:?}"
            );
        }
        let first = ENTRY_POINTS
            .first()
            .copied()
            .unwrap_or_else(|| unreachable!("reviewed table is non-empty"));
        let mut duplicate = ENTRY_POINTS.to_vec();
        duplicate.push(first);
        assert_eq!(
            validate_entry_points(&duplicate)
                .err()
                .map(|error| error.code),
            Some("authority_entry_point_duplicate".into())
        );
        let unowned: alloc::vec::Vec<_> = ENTRY_POINTS
            .iter()
            .copied()
            .filter(|entry| entry.class != EntryClass::Protected(ProtectedOperation::ClaimRenew))
            .collect();
        assert_eq!(
            validate_entry_points(&unowned)
                .err()
                .map(|error| error.code),
            Some("authority_entry_point_unowned".into())
        );
        let mut unnamed = ENTRY_POINTS.to_vec();
        if let Some(entry) = unnamed.first_mut() {
            entry.item = " open";
        }
        assert_eq!(
            validate_entry_points(&unnamed)
                .err()
                .map(|error| error.code),
            Some("authority_entry_point_name".into())
        );
        assert_eq!(
            validate_entry_points(&[]).err().map(|error| error.code),
            Some("authority_entry_points_bound".into())
        );
    }

    #[test]
    // r[verify kiln.implementation.authority.payload]
    // r[verify kiln.implementation.authority.payload.scenario.fields]
    fn payload_v2_binds_every_field_with_length_framing() {
        let launch = ContentId::from_blake3_hex("ab".repeat(32))
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let digest = |operation, incarnation, effect, target, parts: &[&str]| {
            payload_digest_v2(operation, incarnation, effect, target, parts)
        };
        let publish = ProtectedOperation::PublicationPublish;
        let target = "refs/heads/kiln@absent";
        let base = digest(publish, Some(&launch), "effect-1", target, &["ab", "c"]);
        let variants = [
            digest(
                ProtectedOperation::IntegrationLand,
                Some(&launch),
                "effect-1",
                target,
                &["ab", "c"],
            ),
            digest(publish, None, "effect-1", target, &["ab", "c"]),
            digest(publish, Some(&launch), "effect-2", target, &["ab", "c"]),
            digest(
                publish,
                Some(&launch),
                "effect-1",
                "refs/heads/kiln@0123",
                &["ab", "c"],
            ),
            digest(publish, Some(&launch), "effect-1", target, &["a", "bc"]),
            digest(publish, Some(&launch), "effect-1", target, &["ab", "c", ""]),
        ];
        for variant in variants {
            assert_ne!(variant, base);
        }
        assert_eq!(
            digest(publish, Some(&launch), "effect-1", target, &["ab", "c"]),
            base,
            "a fresh invocation of the same effect recomputes the same digest"
        );
    }

    fn authority_request(operation: ProtectedOperation) -> AuthorityRequest {
        AuthorityRequest {
            operation,
            project: "OnixResearch/kiln".into(),
            work: "kiln.implementation.work".into(),
            work_version: 3,
            attempt: "attempt-a".into(),
            fence: 5,
            request_digest: [41; AUTHORITY_DIGEST_BYTES],
            target: "candidate/approved".into(),
            candidate: if operation.requires_candidate() {
                [43; AUTHORITY_DIGEST_BYTES]
            } else {
                [0; AUTHORITY_DIGEST_BYTES]
            },
        }
    }

    #[test]
    // r[verify kiln.implementation.authority.binding.scenario.candidate]
    fn authority_request_is_bounded_and_candidate_exact() {
        for operation in ProtectedOperation::ALL {
            let request = authority_request(operation);
            request
                .validate()
                .unwrap_or_else(|error| unreachable!("{error:?}"));
            assert!(request.identity().is_ok());
        }
        let mut missing_candidate = authority_request(ProtectedOperation::CandidateSubmit);
        missing_candidate.candidate = [0; AUTHORITY_DIGEST_BYTES];
        assert_eq!(
            missing_candidate.validate().err().map(|error| error.code),
            Some("authority_request_candidate".into())
        );
        let mut extra_candidate = authority_request(ProtectedOperation::Dispatch);
        extra_candidate.candidate = [1; AUTHORITY_DIGEST_BYTES];
        assert_eq!(
            extra_candidate.validate().err().map(|error| error.code),
            Some("authority_request_candidate".into())
        );
        let mut empty = authority_request(ProtectedOperation::Dispatch);
        empty.target.clear();
        assert_eq!(
            empty.validate().err().map(|error| error.code),
            Some("authority_request_field".into())
        );
        let mut version = authority_request(ProtectedOperation::Dispatch);
        version.work_version = 0;
        assert_eq!(
            version.validate().err().map(|error| error.code),
            Some("authority_request_version".into())
        );
    }

    #[test]
    // r[verify kiln.implementation.authority.binding.scenario.substitution]
    fn authority_request_identity_changes_with_every_bound_field() {
        let original = authority_request(ProtectedOperation::CandidateSubmit);
        let baseline = original.digest_hex();
        let mut alterations = alloc::vec::Vec::new();
        let mut altered = original.clone();
        altered.project.push('x');
        alterations.push(altered);
        let mut altered = original.clone();
        altered.work.push('x');
        alterations.push(altered);
        let mut altered = original.clone();
        altered.work_version += 1;
        alterations.push(altered);
        let mut altered = original.clone();
        altered.attempt.push('x');
        alterations.push(altered);
        let mut altered = original.clone();
        altered.fence += 1;
        alterations.push(altered);
        let mut altered = original.clone();
        altered.request_digest[0] ^= 1;
        alterations.push(altered);
        let mut altered = original.clone();
        altered.target.push('x');
        alterations.push(altered);
        let mut altered = original.clone();
        altered.candidate[0] ^= 1;
        alterations.push(altered);
        for altered in alterations {
            assert_ne!(altered.digest_hex(), baseline);
        }
    }

    #[test]
    fn denial_decision_is_bounded_and_credential_free() {
        let request = authority_request(ProtectedOperation::PublicationPublish);
        let decision = AuthorityDecision::deny(&request, AuthorityDenialReason::CredentialDenied);
        assert!(!decision.allowed);
        assert_eq!(decision.operation, ProtectedOperation::PublicationPublish);
        assert_eq!(decision.boundary, EnforcementBoundary::Publication);
        assert_eq!(decision.effect, AuthorityEffect::Publication);
        assert_eq!(decision.provenance, AuthorityProvenance::None);
        assert!(decision.session.is_none());
        assert_eq!(decision.request_identity, request.digest_hex());
        assert_eq!(decision.digest_hex().len(), 64);
        let encoded =
            serde_json::to_string(&decision).unwrap_or_else(|error| unreachable!("{error}"));
        assert!(!encoded.contains("compact_token"));
        assert!(!encoded.contains("holder_signature"));
    }

    #[test]
    fn credential_profile_redacts_and_validates() {
        let credential = AuthorityCredential::new(
            "did:key:zHolder",
            "compact.token.value",
            [7; AUTHORITY_NONCE_BYTES],
            [9; AUTHORITY_SIGNATURE_BYTES],
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        let profile = AuthorityProfile::VerifiedUcan(credential);
        let debug = alloc::format!("{profile:?}");
        assert!(debug.contains("did:key:zHolder"));
        assert!(!debug.contains("compact.token.value"));
        assert!(AuthorityCredential::new("", "token", [0; 32], [0; 64]).is_err());
        assert!(AuthorityCredential::new("did:key:z", "", [0; 32], [0; 64]).is_err());
    }

    #[test]
    fn local_operator_profile_admits_and_records_provenance() {
        let mut guard = LocalOperatorAuthority::new("operator-a", false)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let request = authority_request(ProtectedOperation::ClaimAcquire);
        let profile = AuthorityProfile::LocalOperator {
            operator: "operator-a".into(),
        };
        let decision = guard
            .admit(&request, &profile, AttemptCurrentness::Current, 1)
            .unwrap_or_else(|denial| unreachable!("{denial:?}"));
        assert!(decision.allowed);
        assert_eq!(decision.provenance, AuthorityProvenance::LocalOperator);
        assert_eq!(decision.principal, "operator-a");
        let session = decision
            .session
            .unwrap_or_else(|| unreachable!("allowed decision requires a session"));
        guard
            .revalidate(session, &request, AttemptCurrentness::Current, 1)
            .unwrap_or_else(|denial| unreachable!("{denial:?}"));
        assert_eq!(guard.decisions().len(), 2);
        assert!(
            guard
                .decisions()
                .iter()
                .all(|decision| decision.reason == AuthorityDenialReason::Allowed)
        );
        let mut substituted = request.clone();
        substituted.target.push('x');
        assert_eq!(
            guard
                .revalidate(session, &substituted, AttemptCurrentness::Current, 1)
                .err()
                .map(|denial| denial.reason),
            Some(AuthorityDenialReason::BindingMismatch)
        );
    }

    #[test]
    // r[verify kiln.implementation.authority.compatibility.scenario.role]
    fn local_operator_profile_denies_roles_projection_and_missing_authority() {
        let request = authority_request(ProtectedOperation::Dispatch);
        let operator = AuthorityProfile::LocalOperator {
            operator: "operator-a".into(),
        };
        let mut guard = LocalOperatorAuthority::new("operator-a", true)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(
            guard
                .admit(&request, &operator, AttemptCurrentness::Current, 1)
                .err()
                .map(|denial| denial.reason),
            Some(AuthorityDenialReason::ProjectionRequiresOperator)
        );
        let mut guard = LocalOperatorAuthority::new("operator-a", false)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(
            guard
                .admit(
                    &request,
                    &AuthorityProfile::Missing,
                    AttemptCurrentness::Current,
                    1
                )
                .err()
                .map(|denial| denial.reason),
            Some(AuthorityDenialReason::MissingEvidence)
        );
        let other = AuthorityProfile::LocalOperator {
            operator: "operator-b".into(),
        };
        assert_eq!(
            guard
                .admit(&request, &other, AttemptCurrentness::Current, 1)
                .err()
                .map(|denial| denial.reason),
            Some(AuthorityDenialReason::PrincipalMismatch)
        );
        assert_eq!(
            guard
                .admit(&request, &operator, AttemptCurrentness::Cancelled, 1)
                .err()
                .map(|denial| denial.reason),
            Some(AuthorityDenialReason::AttemptNotCurrent)
        );
    }

    #[test]
    // r[verify kiln.implementation.authority.separation]
    // r[verify kiln.implementation.authority.separation.scenario.widen]
    // r[verify kiln.implementation.authority.currentness.scenario.blocked]
    // r[verify kiln.implementation.authority.currentness.scenario.unavailable]
    fn dispatch_session_denies_widened_and_non_current_effects() {
        let operator = AuthorityProfile::LocalOperator {
            operator: "operator-a".into(),
        };
        let dispatch = authority_request(ProtectedOperation::Dispatch);
        for operation in [
            ProtectedOperation::VerificationAdmit,
            ProtectedOperation::PublicationPublish,
            ProtectedOperation::IntegrationLand,
            ProtectedOperation::LifecycleComplete,
        ] {
            let mut guard = LocalOperatorAuthority::new("operator-a", false)
                .unwrap_or_else(|error| unreachable!("{error:?}"));
            let session = guard
                .admit(&dispatch, &operator, AttemptCurrentness::Current, 1)
                .unwrap_or_else(|denial| unreachable!("{denial:?}"))
                .session
                .unwrap_or_else(|| unreachable!("allowed dispatch requires a session"));
            assert_eq!(
                guard
                    .revalidate(
                        session,
                        &authority_request(operation),
                        AttemptCurrentness::Current,
                        2
                    )
                    .err()
                    .map(|denial| denial.reason),
                Some(AuthorityDenialReason::BindingMismatch),
                "{operation:?}"
            );
        }
        for currentness in [
            AttemptCurrentness::Cancelled,
            AttemptCurrentness::Superseded,
            AttemptCurrentness::FenceReplaced,
            AttemptCurrentness::LeaseExpired,
            AttemptCurrentness::Unavailable,
        ] {
            let mut guard = LocalOperatorAuthority::new("operator-a", false)
                .unwrap_or_else(|error| unreachable!("{error:?}"));
            let session = guard
                .admit(&dispatch, &operator, AttemptCurrentness::Current, 1)
                .unwrap_or_else(|denial| unreachable!("{denial:?}"))
                .session
                .unwrap_or_else(|| unreachable!("allowed dispatch requires a session"));
            assert_eq!(
                guard
                    .revalidate(session, &dispatch, currentness, 2)
                    .err()
                    .map(|denial| denial.reason),
                Some(AuthorityDenialReason::AttemptNotCurrent),
                "{currentness:?}"
            );
            assert!(
                guard
                    .decisions()
                    .last()
                    .is_some_and(|decision| !decision.allowed),
                "{currentness:?}"
            );
        }
    }

    #[test]
    fn local_operator_profile_refuses_verified_credentials() {
        let mut guard = LocalOperatorAuthority::new("operator-a", false)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let credential = AuthorityCredential::new(
            "did:key:zHolder",
            "compact.token.value",
            [7; AUTHORITY_NONCE_BYTES],
            [9; AUTHORITY_SIGNATURE_BYTES],
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        let request = authority_request(ProtectedOperation::VerificationAdmit);
        assert_eq!(
            guard
                .admit(
                    &request,
                    &AuthorityProfile::VerifiedUcan(credential),
                    AttemptCurrentness::Current,
                    1
                )
                .err()
                .map(|denial| denial.reason),
            Some(AuthorityDenialReason::VerifierUnavailable)
        );
    }
}
