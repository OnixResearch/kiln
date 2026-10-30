use alloc::format;
use serde::{Deserialize, Serialize};

use super::{
    CheckBounds, CheckMode, CheckerIdentity, LineagePolicy, MergeRole, RoleSource,
    SemanticMergePolicy, SemanticMergeRequest,
};
use crate::source::{GitObjectFormat, JujutsuBackend, SourceNativeFacts};
use crate::types::{ContentId, CoreResult, Diagnostic, ExternalRef};

const BINDING_DOMAIN: &[u8] = b"kiln.semantic-merge.binding.v1\0";
const ATTEMPT_DOMAIN: &[u8] = b"kiln.semantic-merge.attempt.v1\0";
const ARTIFACT_DOMAIN: &[u8] = b"kiln.semantic-merge.artifact.v1\0";

/// One independently hashed part of a semantic merge request binding.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingComponent {
    Base,
    VariantA,
    VariantB,
    Candidate,
    CandidateSupply,
    Subject,
    Property,
    Assumptions,
    Dependencies,
    Model,
    Checker,
    Policy,
    Bounds,
}

impl BindingComponent {
    pub const ALL: [Self; 13] = [
        Self::Base,
        Self::VariantA,
        Self::VariantB,
        Self::Candidate,
        Self::CandidateSupply,
        Self::Subject,
        Self::Property,
        Self::Assumptions,
        Self::Dependencies,
        Self::Model,
        Self::Checker,
        Self::Policy,
        Self::Bounds,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::VariantA => "variant_a",
            Self::VariantB => "variant_b",
            Self::Candidate => "candidate",
            Self::CandidateSupply => "candidate_supply",
            Self::Subject => "subject",
            Self::Property => "property",
            Self::Assumptions => "assumptions",
            Self::Dependencies => "dependencies",
            Self::Model => "model",
            Self::Checker => "checker",
            Self::Policy => "policy",
            Self::Bounds => "bounds",
        }
    }

    #[must_use]
    pub const fn for_role(role: MergeRole) -> Self {
        match role {
            MergeRole::Base => Self::Base,
            MergeRole::VariantA => Self::VariantA,
            MergeRole::VariantB => Self::VariantB,
            MergeRole::Candidate => Self::Candidate,
        }
    }
}

/// Role-tagged BLAKE3 identities for every input of one semantic merge check.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RequestBinding {
    pub base: ContentId,
    pub variant_a: ContentId,
    pub variant_b: ContentId,
    pub candidate: ContentId,
    pub candidate_supply: ContentId,
    pub subject: ContentId,
    pub property: ContentId,
    pub assumptions: ContentId,
    pub dependencies: ContentId,
    pub model: ContentId,
    pub checker: ContentId,
    pub policy: ContentId,
    pub bounds: ContentId,
    pub identity: ContentId,
}

impl RequestBinding {
    #[must_use]
    pub const fn component(&self, component: BindingComponent) -> &ContentId {
        match component {
            BindingComponent::Base => &self.base,
            BindingComponent::VariantA => &self.variant_a,
            BindingComponent::VariantB => &self.variant_b,
            BindingComponent::Candidate => &self.candidate,
            BindingComponent::CandidateSupply => &self.candidate_supply,
            BindingComponent::Subject => &self.subject,
            BindingComponent::Property => &self.property,
            BindingComponent::Assumptions => &self.assumptions,
            BindingComponent::Dependencies => &self.dependencies,
            BindingComponent::Model => &self.model,
            BindingComponent::Checker => &self.checker,
            BindingComponent::Policy => &self.policy,
            BindingComponent::Bounds => &self.bounds,
        }
    }

    /// The first component whose identity differs, in the fixed component order.
    #[must_use]
    pub fn first_mismatch(&self, other: &Self) -> Option<BindingComponent> {
        BindingComponent::ALL
            .into_iter()
            .find(|component| self.component(*component) != other.component(*component))
    }
}

/// Rejects an observed binding that differs from the expected binding, naming the failed component.
// r[impl kiln.semantic_merge.binding]
pub fn require_binding(
    expected: &RequestBinding,
    observed: &RequestBinding,
    code: &str,
) -> CoreResult<()> {
    if overall_identity(observed)? != observed.identity {
        return Err(Diagnostic::new(
            "semantic_binding_identity",
            "semantic_merge.binding.identity",
            "binding identity does not cover its own role-tagged components",
        ));
    }
    if let Some(component) = expected.first_mismatch(observed) {
        let field = format!("semantic_merge.binding.{}", component.name());
        return Err(Diagnostic::new(
            code,
            &field,
            "result is bound to a different semantic merge input",
        ));
    }
    if expected.identity != observed.identity {
        return Err(Diagnostic::new(
            code,
            "semantic_merge.binding.identity",
            "result is bound to a different semantic merge request",
        ));
    }
    Ok(())
}

/// Exact identity of one checker attempt for one request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttemptBinding {
    pub request: ContentId,
    pub ordinal: u32,
    pub identity: ContentId,
}

// r[impl kiln.semantic_merge.binding]
pub fn attempt_binding(request: &ContentId, ordinal: u32) -> CoreResult<AttemptBinding> {
    if ordinal == 0 {
        return Err(Diagnostic::new(
            "semantic_attempt_ordinal",
            "semantic_merge.attempt.ordinal",
            "attempt ordinals start at one",
        ));
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(ATTEMPT_DOMAIN);
    field(&mut hasher, "request", request.as_str().as_bytes());
    field(&mut hasher, "ordinal", &ordinal.to_be_bytes());
    Ok(AttemptBinding {
        request: request.clone(),
        ordinal,
        identity: finish(&hasher)?,
    })
}

/// Kind of checker output that Kiln binds to an attempt.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Log,
    Product,
}

// r[impl kiln.semantic_merge.binding]
pub fn artifact_binding(
    request: &ContentId,
    attempt: &ContentId,
    kind: ArtifactKind,
    name: &str,
    content: &ContentId,
) -> CoreResult<ContentId> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(ARTIFACT_DOMAIN);
    field(&mut hasher, "request", request.as_str().as_bytes());
    field(&mut hasher, "attempt", attempt.as_str().as_bytes());
    let kind = match kind {
        ArtifactKind::Log => "log",
        ArtifactKind::Product => "product",
    };
    field(&mut hasher, "kind", kind.as_bytes());
    field(&mut hasher, "name", name.as_bytes());
    field(&mut hasher, "content", content.as_str().as_bytes());
    finish(&hasher)
}

pub(super) fn compute_binding(request: &SemanticMergeRequest) -> CoreResult<RequestBinding> {
    let mut binding = RequestBinding {
        base: role_identity(&request.base)?,
        variant_a: role_identity(&request.variant_a)?,
        variant_b: role_identity(&request.variant_b)?,
        candidate: role_identity(&request.candidate)?,
        candidate_supply: {
            let mut hasher = component_hasher(BindingComponent::CandidateSupply);
            let supply = &request.candidate_supply;
            field(&mut hasher, "supplier", supply.supplier.as_str().as_bytes());
            external(&mut hasher, "proposal", &supply.proposal);
            external(&mut hasher, "supplied_state", &supply.supplied_state);
            finish(&hasher)?
        },
        subject: {
            let mut hasher = component_hasher(BindingComponent::Subject);
            let subject = &request.subject;
            field(
                &mut hasher,
                "language",
                subject.language.as_str().as_bytes(),
            );
            external(&mut hasher, "unit", &subject.unit);
            count(&mut hasher, subject.required_coverage.len());
            for unit in &subject.required_coverage {
                field(&mut hasher, "coverage", unit.as_str().as_bytes());
            }
            finish(&hasher)?
        },
        property: {
            let mut hasher = component_hasher(BindingComponent::Property);
            field(&mut hasher, "id", request.property.id.as_str().as_bytes());
            field(
                &mut hasher,
                "revision",
                request.property.revision.as_str().as_bytes(),
            );
            finish(&hasher)?
        },
        assumptions: {
            let mut hasher = component_hasher(BindingComponent::Assumptions);
            count(&mut hasher, request.property.assumptions.len());
            for assumption in &request.property.assumptions {
                field(&mut hasher, "assumption", assumption.as_str().as_bytes());
            }
            finish(&hasher)?
        },
        dependencies: single(BindingComponent::Dependencies, &request.inputs.dependencies)?,
        model: single(BindingComponent::Model, &request.inputs.model)?,
        checker: {
            let mut hasher = component_hasher(BindingComponent::Checker);
            checker(&mut hasher, &request.checker);
            finish(&hasher)?
        },
        policy: policy_identity(&request.policy)?,
        bounds: {
            let mut hasher = component_hasher(BindingComponent::Bounds);
            bounds(&mut hasher, &request.bounds);
            finish(&hasher)?
        },
        identity: placeholder_identity()?,
    };
    binding.identity = overall_identity(&binding)?;
    Ok(binding)
}

fn placeholder_identity() -> CoreResult<ContentId> {
    ContentId::from_blake3_hex(blake3::hash(BINDING_DOMAIN).to_hex().as_str())
}

fn overall_identity(binding: &RequestBinding) -> CoreResult<ContentId> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(BINDING_DOMAIN);
    field(&mut hasher, "component", b"request");
    for component in BindingComponent::ALL {
        field(
            &mut hasher,
            component.name(),
            binding.component(component).as_str().as_bytes(),
        );
    }
    finish(&hasher)
}

fn role_identity(source: &RoleSource) -> CoreResult<ContentId> {
    let mut hasher = component_hasher(BindingComponent::for_role(source.role));
    field(&mut hasher, "role", source.role.tag().as_bytes());
    let resolution = &source.resolution;
    field(
        &mut hasher,
        "source_provider",
        resolution.source_provider.as_str().as_bytes(),
    );
    external(&mut hasher, "repository", &resolution.repository);
    external(&mut hasher, "symbolic", &resolution.symbolic_reference);
    external(&mut hasher, "exact", &resolution.exact_revision);
    field(
        &mut hasher,
        "tree",
        resolution.canonical_tree.as_str().as_bytes(),
    );
    match &resolution.native_facts {
        SourceNativeFacts::Pijul(facts) => {
            field(&mut hasher, "native", b"pijul");
            field(
                &mut hasher,
                "observation",
                facts.observation_identity.as_str().as_bytes(),
            );
            external(&mut hasher, "channel", &facts.channel);
            external(&mut hasher, "state", &facts.state);
            external(&mut hasher, "before", &facts.observed_state_before);
            external(&mut hasher, "after", &facts.observed_state_after);
            external(&mut hasher, "format", &facts.format_cohort);
            count(&mut hasher, facts.change_sequence.len());
            for change in &facts.change_sequence {
                external(&mut hasher, "change", change);
            }
            count(&mut hasher, facts.change_bound);
        }
        SourceNativeFacts::Git(facts) => {
            field(&mut hasher, "native", b"git");
            external(&mut hasher, "commit", &facts.exact_commit);
            external(&mut hasher, "native_tree", &facts.native_tree);
            external(&mut hasher, "before", &facts.observed_ref_before);
            external(&mut hasher, "after", &facts.observed_ref_after);
            let format = match facts.object_format {
                GitObjectFormat::Sha1 => "sha1",
                GitObjectFormat::Other => "other",
            };
            field(&mut hasher, "object_format", format.as_bytes());
            for flag in [
                facts.partial_repository,
                facts.replacement_objects,
                facts.alternate_object_database,
                facts.signature_observed,
            ] {
                field(&mut hasher, "flag", &[u8::from(flag)]);
            }
        }
        SourceNativeFacts::Jujutsu(facts) => {
            field(&mut hasher, "native", b"jujutsu");
            let backend = match facts.backend {
                JujutsuBackend::Git => "git",
                JujutsuBackend::Other => "other",
            };
            field(&mut hasher, "backend", backend.as_bytes());
            external(&mut hasher, "operation", &facts.operation_identity);
            external(&mut hasher, "change", &facts.change_id);
            if let Some(bookmark) = &facts.bookmark {
                field(&mut hasher, "bookmark", bookmark.as_str().as_bytes());
            }
            count(&mut hasher, facts.resolved_commits.len());
            for commit in &facts.resolved_commits {
                external(&mut hasher, "resolved", commit);
            }
            field(
                &mut hasher,
                "flags",
                &[
                    u8::from(facts.conflicted_bookmark),
                    u8::from(facts.divergent_change),
                ],
            );
        }
    }
    finish(&hasher)
}

fn policy_identity(policy: &SemanticMergePolicy) -> CoreResult<ContentId> {
    let mut hasher = component_hasher(BindingComponent::Policy);
    field(&mut hasher, "id", policy.id.as_str().as_bytes());
    let mode = match policy.mode {
        CheckMode::Advisory => "advisory",
        CheckMode::Required => "required",
    };
    field(&mut hasher, "mode", mode.as_bytes());
    match policy.lineage {
        LineagePolicy::DeclaredRoles => field(&mut hasher, "lineage", b"declared-roles"),
        LineagePolicy::PijulChangeContainment {
            require_greatest_common_base,
            max_candidate_extra_changes,
        } => {
            field(&mut hasher, "lineage", b"pijul-change-containment");
            field(
                &mut hasher,
                "greatest_common_base",
                &[u8::from(require_greatest_common_base)],
            );
            count(&mut hasher, max_candidate_extra_changes);
        }
    }
    count(&mut hasher, policy.admitted_checkers.len());
    for admitted in &policy.admitted_checkers {
        checker(&mut hasher, admitted);
    }
    count(&mut hasher, policy.admitted_source_profiles.len());
    for profile in &policy.admitted_source_profiles {
        field(&mut hasher, "source_profile", profile.as_str().as_bytes());
    }
    count(&mut hasher, policy.admitted_candidate_suppliers.len());
    for supplier in &policy.admitted_candidate_suppliers {
        field(&mut hasher, "supplier", supplier.as_str().as_bytes());
    }
    field(
        &mut hasher,
        "max_attempts",
        &policy.retry.max_attempts.to_be_bytes(),
    );
    field(
        &mut hasher,
        "retry",
        &[
            u8::from(policy.retry.retry_tool_error),
            u8::from(policy.retry.retry_exhausted),
        ],
    );
    bounds(&mut hasher, &policy.bound_ceilings);
    finish(&hasher)
}

fn component_hasher(component: BindingComponent) -> blake3::Hasher {
    let mut hasher = blake3::Hasher::new();
    hasher.update(BINDING_DOMAIN);
    field(&mut hasher, "component", component.name().as_bytes());
    hasher
}

fn single(component: BindingComponent, value: &ContentId) -> CoreResult<ContentId> {
    let mut hasher = component_hasher(component);
    field(&mut hasher, "value", value.as_str().as_bytes());
    finish(&hasher)
}

fn checker(hasher: &mut blake3::Hasher, checker: &CheckerIdentity) {
    field(
        hasher,
        "producer_profile",
        checker.producer_profile.as_str().as_bytes(),
    );
    field(hasher, "checker", checker.checker.as_str().as_bytes());
    field(hasher, "toolchain", checker.toolchain.as_str().as_bytes());
    field(
        hasher,
        "semantic_profile",
        checker.semantic_profile.as_str().as_bytes(),
    );
    field(
        hasher,
        "receipt_profile",
        checker.receipt_profile.as_str().as_bytes(),
    );
}

fn bounds(hasher: &mut blake3::Hasher, bounds: &CheckBounds) {
    field(hasher, "timeout_ms", &bounds.timeout_ms.to_be_bytes());
    field(
        hasher,
        "max_memory_bytes",
        &bounds.max_memory_bytes.to_be_bytes(),
    );
    field(
        hasher,
        "stdout_max_bytes",
        &bounds.stdout_max_bytes.to_be_bytes(),
    );
    field(
        hasher,
        "stderr_max_bytes",
        &bounds.stderr_max_bytes.to_be_bytes(),
    );
    count(hasher, bounds.max_products);
}

fn external(hasher: &mut blake3::Hasher, label: &str, reference: &ExternalRef) {
    field(hasher, label, &reference.contract_version.to_be_bytes());
    field(
        hasher,
        "namespace",
        reference.provider_namespace.as_str().as_bytes(),
    );
    field(hasher, "kind", reference.native_kind.as_str().as_bytes());
    field(hasher, "id", reference.native_id.as_str().as_bytes());
}

fn count(hasher: &mut blake3::Hasher, value: usize) {
    // usize is at most 64 bits on every supported target.
    let value = u64::try_from(value).unwrap_or(u64::MAX);
    field(hasher, "count", &value.to_be_bytes());
}

fn field(hasher: &mut blake3::Hasher, label: &str, value: &[u8]) {
    let label_length = u64::try_from(label.len()).unwrap_or(u64::MAX);
    let value_length = u64::try_from(value.len()).unwrap_or(u64::MAX);
    hasher.update(&label_length.to_be_bytes());
    hasher.update(label.as_bytes());
    hasher.update(&value_length.to_be_bytes());
    hasher.update(value);
}

fn finish(hasher: &blake3::Hasher) -> CoreResult<ContentId> {
    ContentId::from_blake3_hex(hasher.finalize().to_hex().as_str())
}
