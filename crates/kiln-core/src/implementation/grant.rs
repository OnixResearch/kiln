//! Attempt grant lineage: the pure attenuation core for delegated attempt
//! authority (`kiln.attempt-authority-binding.v1`, Kiln #70).
//!
//! A root binding ties one admitted standing-policy ceiling to the exact
//! campaign selection, work version, attempt, fence, holder and #61 budget
//! reservation. An admitted parent binding derives a child only when the
//! verified delegator holds the parent, holds explicit delegation permission
//! and requests a ceiling no broader than the parent's in resource, operation,
//! audience, lifetime, disclosure route and remaining delegation depth. A child
//! never crosses from coding authority into verification or publication
//! authority.
//!
//! Currentness walks the whole lineage: a revoked or expired ancestor, a moved
//! authority generation, a stale revocation view or a replaced fence
//! invalidates every descendant.
//!
//! This module decides over explicit facts. It never mints, signs, stores,
//! revokes or resolves a token, and a binding holds proof references only, never
//! token bytes. The shell persists an [`AdmittedGrant`] as the issuance intent
//! with its #61 reservation, mints the UCAN, and records the proof reference
//! with [`AdmittedGrant::issue`].

use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};
use core::iter::once;

use serde::{Deserialize, Serialize};

use super::authority::{
    AUTHORITY_DIGEST_BYTES, MAX_AUTHORITY_FIELD_BYTES, MAX_AUTHORITY_REFERENCE_BYTES,
    ProtectedOperation,
};
use crate::types::{ContentId, CoreResult, Diagnostic};

/// Contract schema of an attempt authority binding.
pub const ATTEMPT_AUTHORITY_BINDING_SCHEMA: &str = "kiln.attempt-authority-binding.v1";
/// Contract schema of a redacted grant-issuance explanation (`#60`, `#70`).
pub const GRANT_ISSUANCE_EXPLANATION_SCHEMA: &str = "kiln.grant-issuance-explanation.v1";
/// Maximum lineage depth below a root binding. Bounds every chain walk.
pub const MAX_GRANT_DEPTH: u8 = 8;
/// Maximum bindings in one lineage chain, root included.
pub const MAX_GRANT_LINEAGE: usize = MAX_GRANT_DEPTH as usize + 1;
/// Maximum disclosure routes one ceiling may name.
pub const MAX_DISCLOSURE_ROUTES: usize = 32;
/// Maximum concurrent child grants permitted under a single parent binding.
pub const MAX_CONCURRENT_CHILDREN_PER_PARENT: u32 = 64;
/// Maximum bytes in a redacted denial reason token.
pub const MAX_REDACTED_DENIAL_BYTES: usize = 128;
/// Hex characters in a redacted 32-byte proof reference digest.
pub const GRANT_REF_HEX_CHARS: usize = AUTHORITY_DIGEST_BYTES * 2;

const CEILING_DOMAIN: &[u8] = b"kiln.authority-ceiling.v1";
const BINDING_DOMAIN: &[u8] = b"kiln.attempt-authority-binding.v1";
const INTENT_DOMAIN: &[u8] = b"kiln.attempt-authority-grant-intent.v1";
const NO_GRANT_REF: [u8; AUTHORITY_DIGEST_BYTES] = [0; AUTHORITY_DIGEST_BYTES];

/// Separated authority roles. A delegated child holds exactly one role, and no
/// child crosses into a role its parent does not hold.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GrantRole {
    /// Claim, renewal, dispatch, candidate submission and cancellation.
    Coding,
    /// Independent verification admission.
    Verification,
    /// Publication, integration and lifecycle completion.
    Publication,
}

impl GrantRole {
    /// Role that owns one protected operation.
    #[must_use]
    pub const fn of(operation: ProtectedOperation) -> Self {
        match operation {
            ProtectedOperation::ClaimAcquire
            | ProtectedOperation::ClaimRenew
            | ProtectedOperation::Dispatch
            | ProtectedOperation::CandidateSubmit
            | ProtectedOperation::AttemptCancel => Self::Coding,
            ProtectedOperation::VerificationAdmit => Self::Verification,
            ProtectedOperation::PublicationPublish
            | ProtectedOperation::IntegrationLand
            | ProtectedOperation::LifecycleComplete => Self::Publication,
        }
    }

    const fn bit(self) -> u8 {
        match self {
            Self::Coding => 1,
            Self::Verification => 2,
            Self::Publication => 4,
        }
    }
}

fn role_bits(operations: &BTreeSet<ProtectedOperation>) -> u8 {
    operations
        .iter()
        .fold(0, |bits, operation| bits | GrantRole::of(*operation).bit())
}

/// Closed lineage refusal reasons (`kiln.attempt-authority-binding.v1`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageRefusal {
    /// The child names another project, drops the parent's work scope or adds a
    /// disclosure route the parent does not hold.
    WidenResource,
    /// The child requests an operation the parent does not hold.
    WidenOperation,
    /// The child names another service audience.
    WidenAudience,
    /// The child would outlive its parent, or an ancestor or ceiling expired.
    OutlivesParent,
    /// The child requests delegation depth the parent cannot grant, or the
    /// lineage exceeds [`MAX_GRANT_DEPTH`].
    DepthExceeded,
    /// The delegator holds no explicit delegation permission, or the parent
    /// ceiling permits no further delegation.
    DelegationNotPermitted,
    /// The child mixes roles or crosses from one role into another, such as
    /// coding into verification, publication, integration or lifecycle.
    RoleEscalation,
    /// A binding names another campaign, work, version, attempt or holder, its
    /// identity does not recompute, or its lineage chain is broken.
    ForeignLineage,
    /// No owner-confirmed #61 reservation binds the grant, or the requested
    /// reservation differs from it or from the parent's account.
    BudgetUnbound,
    /// The authority generation moved, the revocation view is older than the
    /// binding, a lineage grant is revoked, or a later fence replaced the attempt.
    StaleGeneration,
}

impl LineageRefusal {
    /// Every refusal in contract order.
    pub const ALL: [Self; 10] = [
        Self::WidenResource,
        Self::WidenOperation,
        Self::WidenAudience,
        Self::OutlivesParent,
        Self::DepthExceeded,
        Self::DelegationNotPermitted,
        Self::RoleEscalation,
        Self::ForeignLineage,
        Self::BudgetUnbound,
        Self::StaleGeneration,
    ];

    /// Stable redacted label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WidenResource => "widen_resource",
            Self::WidenOperation => "widen_operation",
            Self::WidenAudience => "widen_audience",
            Self::OutlivesParent => "outlives_parent",
            Self::DepthExceeded => "depth_exceeded",
            Self::DelegationNotPermitted => "delegation_not_permitted",
            Self::RoleEscalation => "role_escalation",
            Self::ForeignLineage => "foreign_lineage",
            Self::BudgetUnbound => "budget_unbound",
            Self::StaleGeneration => "stale_generation",
        }
    }
}

/// Why a grant was not admitted: malformed input, or a lineage refusal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LineageError {
    /// A requested field is missing, oversized or otherwise malformed.
    Invalid(Diagnostic),
    /// The request is well formed but the lineage refuses it.
    Refused(LineageRefusal),
}

impl From<Diagnostic> for LineageError {
    fn from(diagnostic: Diagnostic) -> Self {
        Self::Invalid(diagnostic)
    }
}

impl From<LineageRefusal> for LineageError {
    fn from(refusal: LineageRefusal) -> Self {
        Self::Refused(refusal)
    }
}

/// The requested terms of one ceiling, before its generation and identity are
/// fixed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CeilingTerms {
    /// Service audience the grant is valid for.
    pub audience: String,
    /// Project scope.
    pub project: String,
    /// Exact work scope, or the whole project when absent.
    pub work: Option<ContentId>,
    /// Protected operations the grant may invoke.
    pub operations: BTreeSet<ProtectedOperation>,
    /// Last valid instant, exclusive, in host seconds.
    pub not_after_s: u64,
    /// Further delegation hops permitted below a binding with this ceiling.
    pub max_depth: u8,
    /// Disclosure routes the grant may use.
    pub disclosure_routes: BTreeSet<String>,
}

/// Immutable authority ceiling. A scope increase needs a new, independently
/// authorized ceiling and identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityCeiling {
    /// BLAKE3 identity over every other field.
    pub identity: ContentId,
    /// Service audience the grant is valid for.
    pub audience: String,
    /// Authority generation the ceiling was admitted under.
    pub generation: u64,
    /// Project scope.
    pub project: String,
    /// Exact work scope, or the whole project when absent.
    pub work: Option<ContentId>,
    /// Protected operations the grant may invoke.
    pub operations: BTreeSet<ProtectedOperation>,
    /// Last valid instant, exclusive, in host seconds.
    pub not_after_s: u64,
    /// Further delegation hops permitted below a binding with this ceiling.
    pub max_depth: u8,
    /// Disclosure routes the grant may use.
    pub disclosure_routes: BTreeSet<String>,
}

impl AuthorityCeiling {
    /// Fix one ceiling's generation and identity.
    ///
    /// # Errors
    /// Returns a diagnostic for a zero generation, malformed text, an empty
    /// operation set, a zero lifetime, a depth above [`MAX_GRANT_DEPTH`] or too
    /// many disclosure routes.
    pub fn new(generation: u64, terms: CeilingTerms) -> CoreResult<Self> {
        let view = CeilingView {
            audience: &terms.audience,
            generation,
            project: &terms.project,
            work: terms.work.as_ref(),
            operations: &terms.operations,
            not_after_s: terms.not_after_s,
            max_depth: terms.max_depth,
            disclosure_routes: &terms.disclosure_routes,
        };
        view.validate_bounds()?;
        let identity = ContentId::from_blake3_hex(view.digest_hex())?;
        Ok(Self {
            identity,
            audience: terms.audience,
            generation,
            project: terms.project,
            work: terms.work,
            operations: terms.operations,
            not_after_s: terms.not_after_s,
            max_depth: terms.max_depth,
            disclosure_routes: terms.disclosure_routes,
        })
    }

    /// Revalidate bounds and recompute the identity of a ceiling that may have
    /// arrived through serde.
    ///
    /// # Errors
    /// Returns the bounds diagnostic, or `authority_ceiling_identity` when the
    /// identity does not recompute.
    pub fn validate(&self) -> CoreResult<()> {
        self.view().validate_bounds()?;
        self.identity.validate()?;
        if self.identity.as_str() != self.view().digest_hex() {
            return Err(Diagnostic::new(
                "authority_ceiling_identity",
                "ceiling.identity",
                "authority ceiling identity does not recompute",
            ));
        }
        Ok(())
    }

    /// Whether this ceiling contains `other` in every attenuable dimension.
    #[must_use]
    pub fn covers(&self, other: &Self) -> bool {
        self.attenuation(other).is_none()
    }

    /// First dimension in which `child` is broader than this ceiling.
    fn attenuation(&self, child: &Self) -> Option<LineageRefusal> {
        if child.audience != self.audience {
            return Some(LineageRefusal::WidenAudience);
        }
        let work_widened = match (&self.work, &child.work) {
            (Some(parent), Some(child)) => parent != child,
            (Some(_), None) => true,
            (None, _) => false,
        };
        if child.project != self.project
            || work_widened
            || !child.disclosure_routes.is_subset(&self.disclosure_routes)
        {
            return Some(LineageRefusal::WidenResource);
        }
        if !child.operations.is_subset(&self.operations) {
            return Some(LineageRefusal::WidenOperation);
        }
        if child.not_after_s > self.not_after_s {
            return Some(LineageRefusal::OutlivesParent);
        }
        if child.max_depth >= self.max_depth {
            return Some(LineageRefusal::DepthExceeded);
        }
        None
    }

    fn view(&self) -> CeilingView<'_> {
        CeilingView {
            audience: &self.audience,
            generation: self.generation,
            project: &self.project,
            work: self.work.as_ref(),
            operations: &self.operations,
            not_after_s: self.not_after_s,
            max_depth: self.max_depth,
            disclosure_routes: &self.disclosure_routes,
        }
    }
}

/// Borrowed ceiling fields, so requested terms and retained ceilings share one
/// bounds check and one identity.
struct CeilingView<'a> {
    audience: &'a str,
    generation: u64,
    project: &'a str,
    work: Option<&'a ContentId>,
    operations: &'a BTreeSet<ProtectedOperation>,
    not_after_s: u64,
    max_depth: u8,
    disclosure_routes: &'a BTreeSet<String>,
}

impl CeilingView<'_> {
    fn validate_bounds(&self) -> CoreResult<()> {
        text_field("ceiling.audience", self.audience, MAX_AUTHORITY_FIELD_BYTES)?;
        text_field("ceiling.project", self.project, MAX_AUTHORITY_FIELD_BYTES)?;
        if let Some(work) = self.work {
            work.validate()?;
        }
        if self.generation == 0 {
            return Err(Diagnostic::new(
                "authority_ceiling_generation",
                "ceiling.generation",
                "authority ceiling requires a non-zero generation",
            ));
        }
        if self.operations.is_empty() {
            return Err(Diagnostic::new(
                "authority_ceiling_operations",
                "ceiling.operations",
                "authority ceiling requires at least one operation",
            ));
        }
        if self.not_after_s == 0 {
            return Err(Diagnostic::new(
                "authority_ceiling_lifetime",
                "ceiling.not_after_s",
                "authority ceiling requires a bounded non-zero lifetime",
            ));
        }
        if self.max_depth > MAX_GRANT_DEPTH {
            return Err(Diagnostic::new(
                "authority_ceiling_depth",
                "ceiling.max_depth",
                "authority ceiling delegation depth exceeds the lineage bound",
            ));
        }
        if self.disclosure_routes.len() > MAX_DISCLOSURE_ROUTES {
            return Err(Diagnostic::new(
                "authority_ceiling_routes",
                "ceiling.disclosure_routes",
                "authority ceiling names too many disclosure routes",
            ));
        }
        for route in self.disclosure_routes {
            text_field(
                "ceiling.disclosure_routes",
                route,
                MAX_AUTHORITY_FIELD_BYTES,
            )?;
        }
        Ok(())
    }

    fn digest_hex(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(CEILING_DOMAIN);
        put_text(&mut hasher, self.audience);
        put_u64(&mut hasher, self.generation);
        put_text(&mut hasher, self.project);
        put_optional_id(&mut hasher, self.work);
        put_u64(&mut hasher, self.operations.len() as u64);
        for operation in self.operations {
            put_text(&mut hasher, operation.basalt_operation());
        }
        put_u64(&mut hasher, self.not_after_s);
        hasher.update(&[self.max_depth]);
        put_u64(&mut hasher, self.disclosure_routes.len() as u64);
        for route in self.disclosure_routes {
            put_text(&mut hasher, route);
        }
        hasher.finalize().to_hex().to_string()
    }
}

/// The #61 budget reservation one grant is charged against.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantBudget {
    /// Parent budget account the reservation was transferred from.
    pub parent_budget_identity: ContentId,
    /// Account the grant charges: the parent's account under shared-parent
    /// charging, or a disjoint #61 `Transfer` child account.
    pub account: ContentId,
}

/// The exact attempt lineage a binding is valid for, read from the current
/// attempt owner. The shell builds it only for the current, active attempt of
/// a work unit; it never builds facts for a non-current attempt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptLineage {
    /// Selected campaign revision, when the attempt runs under a campaign.
    pub campaign_revision: Option<ContentId>,
    /// Project of the admitted work (`ImplementationWork.project`).
    pub project: String,
    /// Work identity.
    pub work: ContentId,
    /// Work version.
    pub work_version: u64,
    /// Attempt identity.
    pub attempt: ContentId,
    /// Current attempt fence.
    pub fence: u64,
}

/// One attempt's admitted authority binding
/// (`kiln.attempt-authority-binding.v1`). It holds proof references only.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptAuthorityBinding {
    /// Contract schema.
    pub schema: String,
    /// Immutable ceiling of this binding.
    pub ceiling: AuthorityCeiling,
    /// Standing root policy reference the lineage descends from.
    pub root_policy_ref: String,
    /// UCAN proof reference: BLAKE3 of the compact token. Never the token.
    pub grant_ref: [u8; AUTHORITY_DIGEST_BYTES],
    /// Holder DID.
    pub holder: String,
    /// Worker incarnation the grant is bound to, when one exists.
    pub incarnation: Option<ContentId>,
    /// Selected campaign revision.
    pub campaign_revision: Option<ContentId>,
    /// Work identity.
    pub work: ContentId,
    /// Work version.
    pub work_version: u64,
    /// Attempt identity.
    pub attempt: ContentId,
    /// Attempt fence.
    pub fence: u64,
    /// #61 reservation the grant is charged against.
    pub budget: GrantBudget,
    /// Parent binding identity, absent for a root binding.
    pub parent: Option<ContentId>,
    /// Distance from the root binding.
    pub depth: u8,
    /// Revocation generation observed when the grant was admitted.
    pub revocation_generation: u64,
    /// BLAKE3 identity over every other field.
    pub identity: ContentId,
}

impl AttemptAuthorityBinding {
    /// Revalidate bounds, ceiling and identity of a binding that may have
    /// arrived through serde.
    ///
    /// # Errors
    /// Returns a diagnostic for a wrong schema, malformed field, a zero proof
    /// reference, an inconsistent depth, or an identity that does not recompute.
    pub fn validate(&self) -> CoreResult<()> {
        if self.grant_ref == NO_GRANT_REF {
            return Err(Diagnostic::new(
                "authority_binding_grant_ref",
                "binding.grant_ref",
                "authority binding requires a proof reference",
            ));
        }
        self.fields().validate()?;
        self.identity.validate()?;
        if self.identity.as_str() != self.fields().binding_digest_hex(&self.grant_ref) {
            return Err(Diagnostic::new(
                "authority_binding_identity",
                "binding.identity",
                "authority binding identity does not recompute",
            ));
        }
        Ok(())
    }

    /// The attempt lineage this binding is valid for. The project is the
    /// ceiling's, which admission requires to equal the work's project.
    #[must_use]
    pub fn lineage(&self) -> AttemptLineage {
        self.fields().lineage()
    }

    fn fields(&self) -> GrantFields<'_> {
        GrantFields {
            schema: &self.schema,
            ceiling: &self.ceiling,
            root_policy_ref: &self.root_policy_ref,
            holder: &self.holder,
            incarnation: self.incarnation.as_ref(),
            campaign_revision: self.campaign_revision.as_ref(),
            work: &self.work,
            work_version: self.work_version,
            attempt: &self.attempt,
            fence: self.fence,
            budget: &self.budget,
            parent: self.parent.as_ref(),
            depth: self.depth,
            revocation_generation: self.revocation_generation,
        }
    }

    /// Refusal when this binding does not bind exactly `current`. Another
    /// campaign revision, project, work or version is foreign; with those
    /// equal, another fence is a replaced attempt and therefore stale; with the
    /// fence equal too, another attempt identity is foreign.
    fn lineage_refusal(&self, current: &AttemptLineage) -> Option<LineageRefusal> {
        if self.campaign_revision != current.campaign_revision
            || self.ceiling.project != current.project
            || self.work != current.work
            || self.work_version != current.work_version
        {
            return Some(LineageRefusal::ForeignLineage);
        }
        if self.fence != current.fence {
            return Some(LineageRefusal::StaleGeneration);
        }
        if self.attempt != current.attempt {
            return Some(LineageRefusal::ForeignLineage);
        }
        None
    }
}

/// An admitted, not yet issued grant: the issuance intent. Its `intent`
/// identity is deterministic over every binding field except the proof
/// reference. Recovery resumes the stored intent record rather than
/// re-deriving it from fresh facts, so it never issues a second grant.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmittedGrant {
    /// Contract schema of the binding this intent issues.
    pub schema: String,
    /// Immutable ceiling.
    pub ceiling: AuthorityCeiling,
    /// Standing root policy reference.
    pub root_policy_ref: String,
    /// Holder DID.
    pub holder: String,
    /// Worker incarnation.
    pub incarnation: Option<ContentId>,
    /// Selected campaign revision.
    pub campaign_revision: Option<ContentId>,
    /// Work identity.
    pub work: ContentId,
    /// Work version.
    pub work_version: u64,
    /// Attempt identity.
    pub attempt: ContentId,
    /// Attempt fence.
    pub fence: u64,
    /// #61 reservation.
    pub budget: GrantBudget,
    /// Parent binding identity, absent for a root grant.
    pub parent: Option<ContentId>,
    /// Distance from the root binding.
    pub depth: u8,
    /// Revocation generation observed at admission.
    pub revocation_generation: u64,
    /// BLAKE3 intent identity over every other field.
    pub intent: ContentId,
}

impl AdmittedGrant {
    /// Revalidate bounds and recompute the intent identity.
    ///
    /// # Errors
    /// Returns a diagnostic for a malformed field or an intent that does not
    /// recompute.
    pub fn validate(&self) -> CoreResult<()> {
        self.fields().validate()?;
        self.intent.validate()?;
        if self.intent.as_str() != self.fields().intent_digest_hex() {
            return Err(Diagnostic::new(
                "authority_grant_intent",
                "grant.intent",
                "authority grant intent does not recompute",
            ));
        }
        Ok(())
    }

    /// Whether this intent binds exactly `current`.
    #[must_use]
    pub fn binds(&self, current: &AttemptLineage) -> bool {
        self.campaign_revision == current.campaign_revision
            && self.ceiling.project == current.project
            && self.work == current.work
            && self.work_version == current.work_version
            && self.attempt == current.attempt
            && self.fence == current.fence
    }

    /// Identity of the binding this intent issues under `grant_ref`.
    ///
    /// # Errors
    /// Returns a diagnostic for a zero proof reference.
    pub fn binding_identity(
        &self,
        grant_ref: &[u8; AUTHORITY_DIGEST_BYTES],
    ) -> CoreResult<ContentId> {
        if *grant_ref == NO_GRANT_REF {
            return Err(Diagnostic::new(
                "authority_binding_grant_ref",
                "binding.grant_ref",
                "authority binding requires a proof reference",
            ));
        }
        ContentId::from_blake3_hex(self.fields().binding_digest_hex(grant_ref))
    }

    /// Record the minted token's proof reference and produce the binding.
    ///
    /// # Errors
    /// Returns a diagnostic for a zero proof reference.
    pub fn issue(
        self,
        grant_ref: [u8; AUTHORITY_DIGEST_BYTES],
    ) -> CoreResult<AttemptAuthorityBinding> {
        let identity = self.binding_identity(&grant_ref)?;
        Ok(AttemptAuthorityBinding {
            schema: self.schema,
            ceiling: self.ceiling,
            root_policy_ref: self.root_policy_ref,
            grant_ref,
            holder: self.holder,
            incarnation: self.incarnation,
            campaign_revision: self.campaign_revision,
            work: self.work,
            work_version: self.work_version,
            attempt: self.attempt,
            fence: self.fence,
            budget: self.budget,
            parent: self.parent,
            depth: self.depth,
            revocation_generation: self.revocation_generation,
            identity,
        })
    }

    fn fields(&self) -> GrantFields<'_> {
        GrantFields {
            schema: &self.schema,
            ceiling: &self.ceiling,
            root_policy_ref: &self.root_policy_ref,
            holder: &self.holder,
            incarnation: self.incarnation.as_ref(),
            campaign_revision: self.campaign_revision.as_ref(),
            work: &self.work,
            work_version: self.work_version,
            attempt: &self.attempt,
            fence: self.fence,
            budget: &self.budget,
            parent: self.parent.as_ref(),
            depth: self.depth,
            revocation_generation: self.revocation_generation,
        }
    }
}

/// Borrowed binding fields shared by the intent and binding identities.
struct GrantFields<'a> {
    schema: &'a str,
    ceiling: &'a AuthorityCeiling,
    root_policy_ref: &'a str,
    holder: &'a str,
    incarnation: Option<&'a ContentId>,
    campaign_revision: Option<&'a ContentId>,
    work: &'a ContentId,
    work_version: u64,
    attempt: &'a ContentId,
    fence: u64,
    budget: &'a GrantBudget,
    parent: Option<&'a ContentId>,
    depth: u8,
    revocation_generation: u64,
}

// r[impl kiln.attempt.grant_lineage.identity]
impl GrantFields<'_> {
    fn lineage(&self) -> AttemptLineage {
        AttemptLineage {
            campaign_revision: self.campaign_revision.cloned(),
            project: self.ceiling.project.clone(),
            work: self.work.clone(),
            work_version: self.work_version,
            attempt: self.attempt.clone(),
            fence: self.fence,
        }
    }

    fn validate(&self) -> CoreResult<()> {
        if self.schema != ATTEMPT_AUTHORITY_BINDING_SCHEMA {
            return Err(Diagnostic::new(
                "authority_binding_schema",
                "binding.schema",
                "authority binding schema is not supported",
            ));
        }
        self.ceiling.validate()?;
        text_field(
            "binding.root_policy_ref",
            self.root_policy_ref,
            MAX_AUTHORITY_REFERENCE_BYTES,
        )?;
        text_field("binding.holder", self.holder, MAX_AUTHORITY_FIELD_BYTES)?;
        for id in [self.incarnation, self.campaign_revision, self.parent]
            .into_iter()
            .flatten()
            .chain([
                self.work,
                self.attempt,
                &self.budget.parent_budget_identity,
                &self.budget.account,
            ])
        {
            id.validate()?;
        }
        if self.work_version == 0 {
            return Err(Diagnostic::new(
                "authority_binding_version",
                "binding.work_version",
                "authority binding requires a non-zero work version",
            ));
        }
        if self
            .ceiling
            .work
            .as_ref()
            .is_some_and(|work| work != self.work)
        {
            return Err(Diagnostic::new(
                "authority_binding_work",
                "binding.work",
                "authority binding work lies outside its ceiling",
            ));
        }
        if (self.depth == 0) != self.parent.is_none()
            || self.depth.saturating_add(self.ceiling.max_depth) > MAX_GRANT_DEPTH
        {
            return Err(Diagnostic::new(
                "authority_binding_depth",
                "binding.depth",
                "authority binding depth must be zero exactly for a root and stay within the lineage bound",
            ));
        }
        Ok(())
    }

    fn hash_into(&self, hasher: &mut blake3::Hasher) {
        put_text(hasher, self.schema);
        put_id(hasher, &self.ceiling.identity);
        put_text(hasher, self.root_policy_ref);
        put_text(hasher, self.holder);
        put_optional_id(hasher, self.incarnation);
        put_optional_id(hasher, self.campaign_revision);
        put_id(hasher, self.work);
        put_u64(hasher, self.work_version);
        put_id(hasher, self.attempt);
        put_u64(hasher, self.fence);
        put_id(hasher, &self.budget.parent_budget_identity);
        put_id(hasher, &self.budget.account);
        put_optional_id(hasher, self.parent);
        hasher.update(&[self.depth]);
        put_u64(hasher, self.revocation_generation);
    }

    fn intent_digest_hex(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(INTENT_DOMAIN);
        self.hash_into(&mut hasher);
        hasher.finalize().to_hex().to_string()
    }

    fn binding_digest_hex(&self, grant_ref: &[u8; AUTHORITY_DIGEST_BYTES]) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(BINDING_DOMAIN);
        self.hash_into(&mut hasher);
        hasher.update(grant_ref);
        hasher.finalize().to_hex().to_string()
    }

    fn admit(self) -> CoreResult<AdmittedGrant> {
        self.validate()?;
        let intent = ContentId::from_blake3_hex(self.intent_digest_hex())?;
        Ok(AdmittedGrant {
            schema: self.schema.to_string(),
            ceiling: self.ceiling.clone(),
            root_policy_ref: self.root_policy_ref.to_string(),
            holder: self.holder.to_string(),
            incarnation: self.incarnation.cloned(),
            campaign_revision: self.campaign_revision.cloned(),
            work: self.work.clone(),
            work_version: self.work_version,
            attempt: self.attempt.clone(),
            fence: self.fence,
            budget: self.budget.clone(),
            parent: self.parent.cloned(),
            depth: self.depth,
            revocation_generation: self.revocation_generation,
            intent,
        })
    }
}

/// Current owner facts every lineage check reads. The shell reads them from
/// the attempt owner and the authority state; callers cannot supply a binding
/// in their place.
#[derive(Clone, Copy, Debug)]
pub struct CurrentFacts<'a> {
    /// Host clock, in seconds.
    pub now_s: u64,
    /// Current authority generation.
    pub authority_generation: u64,
    /// Generation of the revocation view the shell read.
    pub revocation_generation: u64,
    /// Revoked proof references in that view.
    pub revoked: &'a BTreeSet<[u8; AUTHORITY_DIGEST_BYTES]>,
    /// The current attempt lineage.
    pub attempt: &'a AttemptLineage,
}

/// Facts for admitting a root grant from a standing-policy ceiling.
#[derive(Clone, Copy, Debug)]
pub struct RootFacts<'a> {
    /// Currentness facts.
    pub current: CurrentFacts<'a>,
    /// The #61 reservation the budget owner confirmed for this grant.
    pub budget: Option<&'a GrantBudget>,
}

/// Facts for deriving a child grant from an admitted parent.
#[derive(Clone, Copy, Debug)]
pub struct DelegationFacts<'a> {
    /// Currentness facts.
    pub current: CurrentFacts<'a>,
    /// Verified subject presenting the parent grant.
    pub delegator: &'a str,
    /// Whether the delegator's verified session holds explicit delegation
    /// permission over the parent's scope.
    pub delegate_permitted: bool,
    /// The parent's ancestors, root first, excluding the parent.
    pub ancestors: &'a [AttemptAuthorityBinding],
    /// The #61 reservation the budget owner confirmed for this child.
    pub budget: Option<&'a GrantBudget>,
}

/// Request to bind a standing-policy ceiling to the current attempt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootGrantRequest {
    /// Admitted standing-policy ceiling.
    pub ceiling: AuthorityCeiling,
    /// Standing root policy reference.
    pub root_policy_ref: String,
    /// Holder DID.
    pub holder: String,
    /// Worker incarnation, when one exists.
    pub incarnation: Option<ContentId>,
    /// Requested #61 reservation.
    pub budget: GrantBudget,
}

/// Request to derive one child grant from an admitted parent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChildGrantRequest {
    /// Requested child ceiling. Its generation is the parent's.
    pub terms: CeilingTerms,
    /// Child holder DID.
    pub holder: String,
    /// Worker incarnation, when one exists.
    pub incarnation: Option<ContentId>,
    /// Requested #61 reservation.
    pub budget: GrantBudget,
}

// r[impl kiln.attempt.grant_lineage.root]
/// Bind one admitted standing-policy ceiling to the current attempt.
///
/// # Errors
/// Returns [`LineageError::Invalid`] for malformed input and
/// [`LineageError::Refused`] when the ceiling generation moved
/// (`stale_generation`), the ceiling expired (`outlives_parent`), the ceiling
/// names another project or other work (`foreign_lineage`) or no
/// owner-confirmed reservation matches the request (`budget_unbound`).
pub fn admit_root(
    request: RootGrantRequest,
    facts: &RootFacts<'_>,
) -> Result<AdmittedGrant, LineageError> {
    let current = &facts.current;
    request.ceiling.validate()?;
    if request.ceiling.generation != current.authority_generation {
        return Err(LineageRefusal::StaleGeneration.into());
    }
    if current.now_s >= request.ceiling.not_after_s {
        return Err(LineageRefusal::OutlivesParent.into());
    }
    if request.ceiling.project != current.attempt.project
        || request
            .ceiling
            .work
            .as_ref()
            .is_some_and(|work| *work != current.attempt.work)
    {
        return Err(LineageRefusal::ForeignLineage.into());
    }
    if facts.budget != Some(&request.budget) {
        return Err(LineageRefusal::BudgetUnbound.into());
    }
    let attempt = current.attempt;
    Ok(GrantFields {
        schema: ATTEMPT_AUTHORITY_BINDING_SCHEMA,
        ceiling: &request.ceiling,
        root_policy_ref: &request.root_policy_ref,
        holder: &request.holder,
        incarnation: request.incarnation.as_ref(),
        campaign_revision: attempt.campaign_revision.as_ref(),
        work: &attempt.work,
        work_version: attempt.work_version,
        attempt: &attempt.attempt,
        fence: attempt.fence,
        budget: &request.budget,
        parent: None,
        depth: 0,
        revocation_generation: current.revocation_generation,
    }
    .admit()?)
}

// r[impl kiln.attempt.grant_lineage.attenuation] r[impl kiln.attempt.grant_lineage.foreign] r[impl kiln.attempt.grant_lineage.budget]
/// Derive one child grant from an admitted parent.
///
/// Checks run in a fixed order, so one request always yields the same
/// refusal: lineage currentness, delegator and delegation permission, role,
/// then audience, resource, operation, lifetime and depth attenuation, and
/// finally the #61 reservation.
///
/// # Errors
/// Returns [`LineageError::Invalid`] for malformed input and
/// [`LineageError::Refused`] with the first [`LineageRefusal`] that applies.
pub fn admit_child(
    parent: &AttemptAuthorityBinding,
    request: ChildGrantRequest,
    facts: &DelegationFacts<'_>,
) -> Result<AdmittedGrant, LineageError> {
    check_current(parent, facts.ancestors, &facts.current)?;
    if parent.holder != facts.delegator {
        return Err(LineageRefusal::ForeignLineage.into());
    }
    if !facts.delegate_permitted || parent.ceiling.max_depth == 0 {
        return Err(LineageRefusal::DelegationNotPermitted.into());
    }
    let ceiling = AuthorityCeiling::new(parent.ceiling.generation, request.terms)?;
    if !role_derives(&parent.ceiling, &ceiling) {
        return Err(LineageRefusal::RoleEscalation.into());
    }
    if ceiling
        .work
        .as_ref()
        .is_some_and(|work| *work != parent.work)
    {
        return Err(LineageRefusal::ForeignLineage.into());
    }
    if let Some(refusal) = parent.ceiling.attenuation(&ceiling) {
        return Err(refusal.into());
    }
    if facts.current.now_s >= ceiling.not_after_s {
        return Err(Diagnostic::new(
            "authority_grant_expired",
            "request.terms.not_after_s",
            "a child grant must be valid now",
        )
        .into());
    }
    if facts.budget != Some(&request.budget) || !budget_derives(&parent.budget, &request.budget) {
        return Err(LineageRefusal::BudgetUnbound.into());
    }
    let depth = parent
        .depth
        .checked_add(1)
        .ok_or(LineageRefusal::DepthExceeded)?;
    Ok(GrantFields {
        schema: ATTEMPT_AUTHORITY_BINDING_SCHEMA,
        ceiling: &ceiling,
        root_policy_ref: &parent.root_policy_ref,
        holder: &request.holder,
        incarnation: request.incarnation.as_ref(),
        campaign_revision: parent.campaign_revision.as_ref(),
        work: &parent.work,
        work_version: parent.work_version,
        attempt: &parent.attempt,
        fence: parent.fence,
        budget: &request.budget,
        parent: Some(&parent.identity),
        depth,
        revocation_generation: facts.current.revocation_generation,
    }
    .admit()?)
}

// r[impl kiln.attempt.grant_lineage.currentness]
/// Check that `binding` and every ancestor are still current.
///
/// `ancestors` lists the binding's ancestors root first, excluding the binding.
/// Every link must recompute, link to its predecessor by identity, depth and
/// root policy, be covered by the predecessor's ceiling, hold exactly one role
/// its predecessor holds, and charge the predecessor's account or one
/// transferred from it. A revoked or expired ancestor invalidates every
/// descendant.
///
/// # Errors
/// Returns `foreign_lineage` for a forged, broken or wider chain or another
/// project, work or attempt, `stale_generation` for a moved authority
/// generation, an older revocation view, a revoked lineage grant or a replaced
/// fence, `outlives_parent` for an expired lineage grant, and `depth_exceeded`
/// for a chain over the bound.
pub fn check_current(
    binding: &AttemptAuthorityBinding,
    ancestors: &[AttemptAuthorityBinding],
    facts: &CurrentFacts<'_>,
) -> Result<(), LineageRefusal> {
    if ancestors.len() >= MAX_GRANT_LINEAGE {
        return Err(LineageRefusal::DepthExceeded);
    }
    let mut previous: Option<&AttemptAuthorityBinding> = None;
    for link in ancestors.iter().chain(once(binding)) {
        if link.validate().is_err() {
            return Err(LineageRefusal::ForeignLineage);
        }
        let linked = match previous {
            None => link.parent.is_none(),
            Some(parent) => {
                link.parent.as_ref() == Some(&parent.identity)
                    && Some(link.depth) == parent.depth.checked_add(1)
                    && link.root_policy_ref == parent.root_policy_ref
                    && parent.ceiling.covers(&link.ceiling)
                    && role_derives(&parent.ceiling, &link.ceiling)
                    && budget_derives(&parent.budget, &link.budget)
            }
        };
        if !linked {
            return Err(LineageRefusal::ForeignLineage);
        }
        if let Some(refusal) = link.lineage_refusal(facts.attempt) {
            return Err(refusal);
        }
        if link.ceiling.generation != facts.authority_generation
            || link.revocation_generation > facts.revocation_generation
            || facts.revoked.contains(&link.grant_ref)
        {
            return Err(LineageRefusal::StaleGeneration);
        }
        if facts.now_s >= link.ceiling.not_after_s {
            return Err(LineageRefusal::OutlivesParent);
        }
        previous = Some(link);
    }
    Ok(())
}

/// Whether `child` holds exactly one role, and one that `parent` holds.
fn role_derives(parent: &AuthorityCeiling, child: &AuthorityCeiling) -> bool {
    let child_roles = role_bits(&child.operations);
    child_roles.count_ones() == 1 && child_roles & role_bits(&parent.operations) != 0
}

/// Whether `child` shares the parent's account or charges a disjoint account
/// transferred from it.
fn budget_derives(parent: &GrantBudget, child: &GrantBudget) -> bool {
    child == parent
        || (child.parent_budget_identity == parent.account && child.account != parent.account)
}

fn text_field(field: &str, value: &str, max_bytes: usize) -> CoreResult<()> {
    if value.is_empty()
        || value.len() > max_bytes
        || !value.is_ascii()
        || value.trim() != value
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(Diagnostic::new(
            "authority_grant_field",
            field,
            "authority grant field must be bounded non-empty trimmed ASCII",
        ));
    }
    Ok(())
}

fn put_text(hasher: &mut blake3::Hasher, value: &str) {
    put_u64(hasher, value.len() as u64);
    hasher.update(value.as_bytes());
}

fn put_u64(hasher: &mut blake3::Hasher, value: u64) {
    hasher.update(&value.to_be_bytes());
}

fn put_id(hasher: &mut blake3::Hasher, value: &ContentId) {
    put_text(hasher, value.as_str());
}

fn put_optional_id(hasher: &mut blake3::Hasher, value: Option<&ContentId>) {
    match value {
        None => {
            hasher.update(&[0]);
        }
        Some(value) => {
            hasher.update(&[1]);
            put_id(hasher, value);
        }
    }
}

/// Bounded concurrent child grant limit for a single parent binding (#61, #70).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChildConcurrencyLimit {
    /// Maximum simultaneous pending, active, and unresolved-unknown children.
    pub max_concurrent_children: u32,
}

impl ChildConcurrencyLimit {
    /// Construct and validate a concurrent child limit.
    ///
    /// # Errors
    /// Returns a diagnostic if zero or above [`MAX_CONCURRENT_CHILDREN_PER_PARENT`].
    pub fn new(max_concurrent_children: u32) -> CoreResult<Self> {
        let limit = Self {
            max_concurrent_children,
        };
        limit.validate()?;
        Ok(limit)
    }

    /// Validate that the limit is positive and within [`MAX_CONCURRENT_CHILDREN_PER_PARENT`].
    ///
    /// # Errors
    /// Returns a diagnostic if the limit is out of bounds.
    pub fn validate(&self) -> CoreResult<()> {
        if self.max_concurrent_children == 0
            || self.max_concurrent_children > MAX_CONCURRENT_CHILDREN_PER_PARENT
        {
            return Err(Diagnostic::new(
                "grant_child_concurrency_bound",
                "limit.max_concurrent_children",
                "child concurrency limit must be positive and bounded",
            ));
        }
        Ok(())
    }
}

/// Observed child grant slots held under one parent binding.
///
/// Unresolved `unknown_children` count toward held capacity so an unknown
/// termination cannot silently free a slot for duplicate child capacity.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChildConcurrencyState {
    pub active_children: u32,
    pub pending_children: u32,
    pub unknown_children: u32,
}

impl ChildConcurrencyState {
    /// Total child slots currently held (active + pending + unresolved unknown).
    #[must_use]
    pub const fn total_held(&self) -> Option<u32> {
        match self.active_children.checked_add(self.pending_children) {
            Some(sum) => sum.checked_add(self.unknown_children),
            None => None,
        }
    }

    /// Check whether one additional child slot fits within `limit`.
    ///
    /// # Errors
    /// Returns `grant_child_concurrency_bound` for an invalid limit, or
    /// `grant_child_concurrency_full` when held capacity equals or exceeds it.
    pub fn check_available(&self, limit: ChildConcurrencyLimit) -> CoreResult<()> {
        limit.validate()?;
        let held = self.total_held().ok_or_else(|| {
            Diagnostic::new(
                "grant_child_concurrency_full",
                "concurrency.held",
                "child grant concurrency count overflowed",
            )
        })?;
        if held >= limit.max_concurrent_children {
            return Err(Diagnostic::new(
                "grant_child_concurrency_full",
                "concurrency.held",
                "parent grant concurrent child budget exhausted",
            ));
        }
        Ok(())
    }
}

/// Refusal returned when admitting a child grant with concurrency enforcement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChildGrantError {
    /// Lineage, attenuation, currentness, or #61 budget check failed.
    Lineage(LineageError),
    /// Parent grant concurrent child cap (including unknown-held slots) is full.
    ConcurrencyFull,
    /// Parent binding is revoked, released, or fenced by an unknown termination.
    ParentFenced,
}

impl From<LineageError> for ChildGrantError {
    fn from(error: LineageError) -> Self {
        Self::Lineage(error)
    }
}

impl From<LineageRefusal> for ChildGrantError {
    fn from(refusal: LineageRefusal) -> Self {
        Self::Lineage(LineageError::Refused(refusal))
    }
}

impl From<Diagnostic> for ChildGrantError {
    fn from(diagnostic: Diagnostic) -> Self {
        Self::Lineage(LineageError::Invalid(diagnostic))
    }
}

impl ChildGrantError {
    /// Closed redacted code for logs and `#60` explanations.
    #[must_use]
    pub fn redacted_code(&self) -> &str {
        match self {
            Self::Lineage(LineageError::Invalid(diagnostic)) => &diagnostic.code,
            Self::Lineage(LineageError::Refused(refusal)) => refusal.as_str(),
            Self::ConcurrencyFull => "concurrency_full",
            Self::ParentFenced => "parent_fenced",
        }
    }

    /// Convert to a bounded diagnostic.
    #[must_use]
    pub fn into_diagnostic(self) -> Diagnostic {
        match self {
            Self::Lineage(LineageError::Invalid(diagnostic)) => diagnostic,
            Self::Lineage(LineageError::Refused(refusal)) => Diagnostic::new(
                refusal.as_str(),
                "grant.lineage",
                "child grant refused by lineage policy",
            ),
            Self::ConcurrencyFull => Diagnostic::new(
                "grant_child_concurrency_full",
                "grant.concurrency",
                "parent grant concurrent child budget exhausted",
            ),
            Self::ParentFenced => Diagnostic::new(
                "grant_parent_fenced",
                "grant.parent",
                "parent grant is revoked or fenced",
            ),
        }
    }
}

/// Admit a child grant while enforcing parent-child attenuation, #61 budget
/// binding, and concurrent child slot availability.
///
/// # Errors
/// Returns [`ChildGrantError`] if the limit is invalid, the lineage/budget
/// checks fail, or concurrent held slots (active + pending + unknown) are full.
pub fn admit_child_with_concurrency(
    parent: &AttemptAuthorityBinding,
    request: ChildGrantRequest,
    facts: &DelegationFacts<'_>,
    concurrency: ChildConcurrencyState,
    limit: ChildConcurrencyLimit,
) -> Result<AdmittedGrant, ChildGrantError> {
    limit.validate()?;
    let grant = admit_child(parent, request, facts)?;
    if concurrency.check_available(limit).is_err() {
        return Err(ChildGrantError::ConcurrencyFull);
    }
    Ok(grant)
}

/// Durable lifecycle phase of a grant issuance record for `#60` projections.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantLifecyclePhase {
    /// No grant intent exists for this work yet.
    None,
    /// Admitted grant intent persisted prior to token minting.
    Intended,
    /// Token minted and proof reference persisted.
    Issued,
    /// Proof reference durably revoked (directly or via ancestor).
    Revoked,
    /// Terminated in `Unknown`: grant fenced/revoked, slot held until resolved.
    UnknownFenced,
    /// Unknown termination independently observed; grant remains revoked, slot freed.
    UnknownResolved,
    /// Cleanly completed/settled and released.
    Released,
    /// Unissued intent abandoned before minting.
    Abandoned,
}

impl GrantLifecyclePhase {
    /// Stable redacted label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Intended => "intended",
            Self::Issued => "issued",
            Self::Revoked => "revoked",
            Self::UnknownFenced => "unknown_fenced",
            Self::UnknownResolved => "unknown_resolved",
            Self::Released => "released",
            Self::Abandoned => "abandoned",
        }
    }

    /// Whether this phase blocks new dispatch or issuance.
    #[must_use]
    pub const fn is_blocking(self) -> bool {
        matches!(
            self,
            Self::Revoked | Self::UnknownFenced | Self::UnknownResolved | Self::Abandoned
        )
    }
}

/// Redacted, non-credential grant-issuance and authority status (`#60`, `#70`).
///
/// Carries only content identities, proof reference digests (never tokens),
/// bounded enum/count fields, and strictly validated closed denial tokens.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RedactedGrantExplanation {
    pub schema: String,
    pub work_version: ContentId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<ContentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fence: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<ContentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_binding: Option<ContentId>,
    /// 64-char lowercase hex BLAKE3 digest of the proof reference, never token bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant_ref_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<GrantRole>,
    pub authority_generation: u64,
    pub revocation_generation: u64,
    pub phase: GrantLifecyclePhase,
    /// Closed redacted denial token, never free text, paths, or secret material.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub denial_reason: Option<String>,
    pub active_children: u32,
    pub pending_children: u32,
    pub unknown_children: u32,
    pub max_concurrent_children: u32,
}

impl RedactedGrantExplanation {
    /// Validate that all identifiers, digests, and denial tokens are strictly redacted.
    ///
    /// # Errors
    /// Returns a diagnostic if the schema, identities, hex digest, depth, or
    /// denial token contain non-redacted, oversized, path-like, or secret-like content.
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema != GRANT_ISSUANCE_EXPLANATION_SCHEMA {
            return Err(Diagnostic::new(
                "grant_explanation_schema",
                "explanation.schema",
                "unsupported grant explanation schema",
            ));
        }
        self.work_version.validate()?;
        for id in [&self.attempt, &self.intent, &self.parent_binding]
            .into_iter()
            .flatten()
        {
            id.validate()?;
        }
        if let Some(digest) = &self.grant_ref_digest
            && (digest.len() != GRANT_REF_HEX_CHARS
                || !digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
        {
            return Err(Diagnostic::new(
                "grant_explanation_digest",
                "explanation.grant_ref_digest",
                "grant reference digest must be 64 lowercase hex chars",
            ));
        }
        if let Some(depth) = self.depth
            && depth > MAX_GRANT_DEPTH
        {
            return Err(Diagnostic::new(
                "grant_explanation_depth",
                "explanation.depth",
                "grant depth exceeds lineage bound",
            ));
        }
        if let Some(reason) = &self.denial_reason
            && !is_redacted_token(reason)
        {
            return Err(Diagnostic::new(
                "grant_explanation_redaction",
                "explanation.denial_reason",
                "denial reason must be a bounded redacted token without paths or secrets",
            ));
        }
        if self.max_concurrent_children > MAX_CONCURRENT_CHILDREN_PER_PARENT {
            return Err(Diagnostic::new(
                "grant_explanation_bound",
                "explanation.max_concurrent_children",
                "max concurrent children exceeds bound",
            ));
        }
        Ok(())
    }
}

/// Whether `value` is a safe redacted identifier/reason token: bounded ASCII
/// of `[a-zA-Z0-9_-.:]` with no `..`, whitespace, slashes, `=`, or token delimiters.
#[must_use]
pub fn is_redacted_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REDACTED_DENIAL_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
        && !value.contains("..")
}

/// Format a 32-byte proof reference digest into 64 lowercase hex characters.
#[must_use]
pub fn format_grant_ref_digest(grant_ref: &[u8; AUTHORITY_DIGEST_BYTES]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(GRANT_REF_HEX_CHARS);
    for byte in grant_ref {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// Map an internal error or diagnostic string into a safe redacted token.
///
/// Never echoes file paths, tokens, URLs, or free-form messages; extracts a
/// closed/safe code prefix if present, otherwise returns `"denied"`.
#[must_use]
pub fn sanitize_redacted_denial_code(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("authority:") {
        let code = rest
            .trim()
            .split(|ch: char| ch.is_ascii_whitespace() || ch == '(')
            .next()
            .unwrap_or_default();
        if is_redacted_token(code) {
            return code.to_string();
        }
        return "authority_denied".to_string();
    }
    let prefix = trimmed
        .split([':', ' '])
        .next()
        .unwrap_or_default()
        .trim();
    if is_redacted_token(prefix) {
        prefix.to_string()
    } else {
        "denied".to_string()
    }
}

#[cfg(test)]
#[path = "grant_tests.rs"]
mod tests;

#[cfg(test)]
mod wiring_tests {
    use super::*;
    use crate::implementation::authority::ProtectedOperation as Op;
    use crate::implementation::{
        AcceptanceObligation, AttemptLedger, DEFAULT_LEASE_MS, IMPLEMENTATION_WORK_SCHEMA,
        ImplementationWork, SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind,
    };
    use crate::types::ExternalRef;
    use alloc::vec;

    fn test_id(ch: char) -> ContentId {
        ContentId::from_blake3_hex(ch.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
    }

    fn test_lineage() -> AttemptLineage {
        let mut attempts = AttemptLedger::default();
        let work = ImplementationWork {
            schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
            revision: 1,
            actor: "operator".to_string(),
            repository: "onix/kiln".to_string(),
            project: "kiln".to_string(),
            base: None,
            title: "Concurrency test".to_string(),
            requested_outcome: "implement".to_string(),
            acceptance: vec![AcceptanceObligation {
                id: "ok".to_string(),
                statement: "verified".to_string(),
            }],
            dependencies: vec![],
            permitted_paths: vec!["crates".to_string()],
            permitted_effects: BTreeSet::from(["write_source".to_string()]),
            verification_profile: test_id('9'),
            budget: WorkBudget {
                attempts: 2,
                minutes_per_attempt: 60,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 1,
            aliases: vec![SourceRef {
                kind: WorkSourceKind::GitHubIssue,
                source: ExternalRef::new("github", "issue", "onix/kiln#70")
                    .unwrap_or_else(|_| unreachable!()),
                native_id: "onix/kiln#70".to_string(),
                version: test_id('1'),
            }],
        };
        let attempt = attempts
            .claim(&work, 0, DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        AttemptLineage {
            campaign_revision: None,
            project: work.project,
            work: attempt.work_identity,
            work_version: attempt.work_revision,
            attempt: attempt.identity,
            fence: attempt.fence,
        }
    }

    #[test]
    fn child_concurrency_enforces_cap_including_unknown_slots() {
        let attempt = test_lineage();
        let revoked = BTreeSet::new();
        let root_budget = GrantBudget {
            parent_budget_identity: test_id('a'),
            account: test_id('b'),
        };
        let root = admit_root(
            RootGrantRequest {
                ceiling: AuthorityCeiling::new(
                    1,
                    CeilingTerms {
                        audience: "did:web:kiln".to_string(),
                        project: "kiln".to_string(),
                        work: Some(attempt.work.clone()),
                        operations: BTreeSet::from([Op::Dispatch]),
                        not_after_s: 1_000,
                        max_depth: 2,
                        disclosure_routes: BTreeSet::new(),
                    },
                )
                .unwrap_or_else(|_| unreachable!()),
                root_policy_ref: "policy:kiln".to_string(),
                holder: "did:key:coordinator".to_string(),
                incarnation: None,
                budget: root_budget.clone(),
            },
            &RootFacts {
                current: CurrentFacts {
                    now_s: 100,
                    authority_generation: 1,
                    revocation_generation: 0,
                    revoked: &revoked,
                    attempt: &attempt,
                },
                budget: Some(&root_budget),
            },
        )
        .unwrap_or_else(|_| unreachable!())
        .issue([1; AUTHORITY_DIGEST_BYTES])
        .unwrap_or_else(|_| unreachable!());

        let child_budget = GrantBudget {
            parent_budget_identity: test_id('b'),
            account: test_id('c'),
        };
        let limit = ChildConcurrencyLimit::new(2).unwrap_or_else(|_| unreachable!());
        let make_req = || ChildGrantRequest {
            terms: CeilingTerms {
                audience: "did:web:kiln".to_string(),
                project: "kiln".to_string(),
                work: Some(attempt.work.clone()),
                operations: BTreeSet::from([Op::Dispatch]),
                not_after_s: 900,
                max_depth: 0,
                disclosure_routes: BTreeSet::new(),
            },
            holder: "did:key:worker".to_string(),
            incarnation: None,
            budget: child_budget.clone(),
        };
        let facts = DelegationFacts {
            current: CurrentFacts {
                now_s: 100,
                authority_generation: 1,
                revocation_generation: 0,
                revoked: &revoked,
                attempt: &attempt,
            },
            delegator: "did:key:coordinator",
            delegate_permitted: true,
            ancestors: &[],
            budget: Some(&child_budget),
        };

        // 1 active + 0 pending + 1 unknown == 2 == limit -> refused
        let err = admit_child_with_concurrency(
            &root,
            make_req(),
            &facts,
            ChildConcurrencyState {
                active_children: 1,
                pending_children: 0,
                unknown_children: 1,
            },
            limit,
        )
        .unwrap_err();
        assert_eq!(err, ChildGrantError::ConcurrencyFull);

        // Once unknown resolves (unknown_children: 0), slot is free.
        assert!(
            admit_child_with_concurrency(
                &root,
                make_req(),
                &facts,
                ChildConcurrencyState {
                    active_children: 1,
                    pending_children: 0,
                    unknown_children: 0,
                },
                limit,
            )
            .is_ok()
        );
        assert!(ChildConcurrencyLimit::new(0).is_err());
        assert!(ChildConcurrencyLimit::new(MAX_CONCURRENT_CHILDREN_PER_PARENT + 1).is_err());
    }

    #[test]
    fn redacted_grant_explanation_rejects_paths_spaces_and_tokens() {
        let valid = RedactedGrantExplanation {
            schema: GRANT_ISSUANCE_EXPLANATION_SCHEMA.to_string(),
            work_version: test_id('1'),
            attempt: Some(test_id('2')),
            fence: Some(1),
            intent: Some(test_id('3')),
            parent_binding: None,
            grant_ref_digest: Some(format_grant_ref_digest(&[0xab; AUTHORITY_DIGEST_BYTES])),
            depth: Some(1),
            role: Some(GrantRole::Coding),
            authority_generation: 1,
            revocation_generation: 2,
            phase: GrantLifecyclePhase::Revoked,
            denial_reason: Some("stale_generation".to_string()),
            active_children: 0,
            pending_children: 0,
            unknown_children: 1,
            max_concurrent_children: 4,
        };
        assert!(valid.validate().is_ok());

        for bad in [
            "/home/brittonr/secrets/coordinator.seed",
            "https://secret.example/token=xyz",
            "bearer eyJhbGciOiJFZERTQSJ9.secret",
            "../private/repo",
            "space in reason",
        ] {
            let mut invalid = valid.clone();
            invalid.denial_reason = Some(bad.to_string());
            assert!(invalid.validate().is_err(), "must reject {bad}");
        }

        let sanitized =
            sanitize_redacted_denial_code("authority: credential-denied (/tmp/secret.ucan)");
        assert_eq!(sanitized, "credential-denied");
        assert!(!sanitized.contains("/tmp"));
        assert!(!sanitized.contains("secret.ucan"));
    }
}
