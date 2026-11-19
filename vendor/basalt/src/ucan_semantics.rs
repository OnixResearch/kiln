//! Typed UCAN decision fixtures replayed through Basalt's existing enforcement core.
//!
//! The fixture boundary consumes decisions produced by a UCAN integration. It does
//! not parse tokens, traverse proofs, evaluate caveats, or own replay state. Basalt
//! validates the declared decision facts, fails closed on every non-admitted fact,
//! and passes only admitted effective grants to [`crate::enforce`].

use crate::ucan_profile::{BasaltUcanProtocolProfile, admit_compact_profile};
use crate::{
    CapabilityGrant, DEFAULT_POLICY_JSON, EnforcementReceipt, EnforcementRequest, Policy,
    default_policy, enforce,
};
use serde::{Deserialize, Serialize};

pub use crate::ucan_semantics_identity::{
    BASALT_POLICY_BLAKE3, UCAN_PROTOCOL_PROFILE, UCAN_RUNTIME_REVISION,
    UCAN_SEMANTICS_FIXTURE_SCHEMA, UCAN_SUPPORT_CATALOG_BLAKE3, UCAN_SUPPORT_CATALOG_REVISION,
};

const MAXIMUM_FIXTURE_CAPABILITIES: usize = 64;
const MAXIMUM_FIXTURE_ID_BYTES: usize = 128;
const FIXTURE_INVALID_REASON: &str = "UCAN semantics fixture rejected";
const ENFORCEMENT_ERROR_REASON: &str = "UCAN semantics enforcement failed closed";

/// Claims this fixture profile explicitly does not make.
pub const REQUIRED_NON_CLAIMS: [&str; 5] = [
    "revocation-backend-freshness",
    "key-resolution-truth",
    "ucan-cryptographic-correctness",
    "consumer-replay-store-durability",
    "application-authorization-completeness",
];

/// UCAN's decision about the supplied delegation chain.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DelegationDecision {
    /// UCAN admitted the effective delegated grants.
    Accepted,
    /// A child grant exceeded its parent authority.
    OverDelegated,
    /// UCAN rejected the delegation for another bounded reason.
    Rejected,
}

/// UCAN's decision about caveat evaluation.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CaveatDecision {
    /// Every required caveat was evaluated and satisfied.
    Satisfied,
    /// The caller did not supply the required caveat policy.
    MissingPolicy,
    /// A supplied caveat evaluated to denial.
    Rejected,
}

/// UCAN's decision about bounded proof traversal.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProofTraversalDecision {
    /// The supplied proof path was admitted within configured bounds.
    Accepted,
    /// At least one proof was stale at the explicit verification time.
    Stale,
    /// The configured traversal bound was exceeded.
    LimitExceeded,
    /// Proof linkage or cycle validation failed.
    Invalid,
}

/// UCAN integration decision about replay admission.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ReplayDecision {
    /// The replay backend admitted this one-time use.
    Admitted,
    /// The replay key had already been consumed.
    Duplicate,
    /// Replay evidence was stale for the requested operation.
    Stale,
    /// The replay backend explicitly denied the operation.
    Denied,
    /// The replay backend failed or was unavailable.
    BackendError,
}

/// Stable result class used by fixtures and replay reports.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticsReasonClass {
    /// Basalt and the supplied UCAN facts allowed the operation.
    Allowed,
    /// Basalt's Nickel-authored policy denied the operation.
    PolicyDenied,
    /// The admitted UCAN grant set did not authorize the concrete operation.
    UcanCapabilityDenied,
    /// UCAN reported an invalid or excessive delegation.
    DelegationDenied,
    /// A wildcard operation request was rejected as non-concrete.
    WildcardRequestDenied,
    /// Caveat evaluation was unavailable or denied.
    CaveatDenied,
    /// Proof traversal was stale, invalid, or out of bounds.
    ProofDenied,
    /// Replay admission was denied or unavailable.
    ReplayDenied,
    /// The fixture contract itself was rejected.
    FixtureInvalid,
    /// Basalt failed closed on an unexpected enforcement error.
    EnforcementError,
}

/// Expected replay result recorded by a fixture.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct SemanticsExpectation {
    /// Expected allow bit.
    pub allowed: bool,
    /// Expected stable result class.
    pub reason_class: SemanticsReasonClass,
}

/// Decision facts supplied by the UCAN integration boundary.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct UcanDecisionFacts {
    /// Delegation-chain admission outcome.
    pub delegation: DelegationDecision,
    /// Caveat evaluation outcome.
    pub caveat_evaluation: CaveatDecision,
    /// Proof traversal outcome.
    pub proof_traversal: ProofTraversalDecision,
    /// Replay admission outcome.
    pub replay: ReplayDecision,
}

/// Typed compatibility fixture for one UCAN-to-Basalt authorization decision.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct UcanSemanticsFixture {
    /// Fixture schema identifier.
    pub schema: String,
    /// Stable fixture identifier.
    pub fixture_id: String,
    /// Explicit nominal UCAN protocol profile.
    pub protocol_profile: BasaltUcanProtocolProfile,
    /// Exact UCAN revision linked by Basalt at runtime.
    pub runtime_ucan_revision: String,
    /// Exact UCAN revision that produced the reviewed support catalog.
    pub support_catalog_revision: String,
    /// BLAKE3 identity of the reviewed UCAN support catalog.
    pub support_catalog_blake3: String,
    /// BLAKE3 identity of the generated Basalt policy under test.
    pub basalt_policy_blake3: String,
    /// Basalt policy contract selected for the request.
    pub contract_id: String,
    /// Concrete requested resource.
    pub request_resource: String,
    /// Concrete requested ability.
    pub request_ability: String,
    /// Direct capabilities admitted by UCAN.
    pub direct_capabilities: Vec<CapabilityGrant>,
    /// Effective delegated capabilities admitted by UCAN.
    pub effective_delegated_capabilities: Vec<CapabilityGrant>,
    /// Caller-supplied UCAN decision facts.
    pub decisions: UcanDecisionFacts,
    /// Expected Basalt replay result.
    pub expected: SemanticsExpectation,
    /// Explicit claim boundaries carried by every fixture.
    pub non_claims: Vec<String>,
}

/// Stable validation finding for malformed or incompatible fixtures.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct SemanticsIssue {
    /// Machine-readable diagnostic code.
    pub code: String,
    /// Stable human-readable explanation.
    pub message: String,
}

impl SemanticsIssue {
    fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.to_owned(),
            message: message.to_owned(),
        }
    }
}

/// Stable replay report for one fixture.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct SemanticsReplayReport {
    /// Stable fixture identifier, or an empty string when JSON parsing failed.
    pub fixture_id: String,
    /// True only when the fixture was valid and replay matched its expectation.
    pub accepted: bool,
    /// Actual Basalt enforcement receipt.
    pub enforcement: EnforcementReceipt,
    /// Stable classification of the actual result.
    pub reason_class: SemanticsReasonClass,
    /// Deterministic validation or expectation findings.
    pub issues: Vec<SemanticsIssue>,
}

impl SemanticsReplayReport {
    /// Return true when the fixture contract and expected replay result were accepted.
    #[must_use]
    pub const fn is_accepted(&self) -> bool {
        self.accepted
    }
}

/// Return the BLAKE3 identity of the embedded default policy bytes.
#[must_use]
pub fn default_policy_blake3() -> String {
    blake3::hash(DEFAULT_POLICY_JSON.as_bytes())
        .to_hex()
        .to_string()
}

/// Validate fixture shape and immutable dependency identities without performing I/O.
#[must_use]
pub fn validate_semantics_fixture(fixture: &UcanSemanticsFixture) -> Vec<SemanticsIssue> {
    let mut issues = Vec::new();

    check_exact(
        &mut issues,
        fixture.schema.as_str(),
        UCAN_SEMANTICS_FIXTURE_SCHEMA,
        "semantics.schema",
        "unsupported UCAN semantics fixture schema",
    );
    check_fixture_id(&mut issues, fixture.fixture_id.as_str());
    if admit_compact_profile(fixture.protocol_profile).is_err() {
        issues.push(SemanticsIssue::new(
            "semantics.profile.mismatch",
            "fixture protocol profile does not match the compact adapter cohort",
        ));
    }
    check_exact(
        &mut issues,
        fixture.runtime_ucan_revision.as_str(),
        UCAN_RUNTIME_REVISION,
        "semantics.identity.runtime-revision",
        "fixture UCAN runtime revision does not match Basalt's immutable dependency",
    );
    check_exact(
        &mut issues,
        fixture.support_catalog_revision.as_str(),
        UCAN_SUPPORT_CATALOG_REVISION,
        "semantics.identity.catalog-revision",
        "fixture support catalog revision is not the reviewed UCAN revision",
    );
    check_exact(
        &mut issues,
        fixture.support_catalog_blake3.as_str(),
        UCAN_SUPPORT_CATALOG_BLAKE3,
        "semantics.identity.catalog-blake3",
        "fixture support catalog BLAKE3 does not match the reviewed catalog",
    );
    check_exact(
        &mut issues,
        fixture.basalt_policy_blake3.as_str(),
        BASALT_POLICY_BLAKE3,
        "semantics.identity.policy-blake3",
        "fixture Basalt policy BLAKE3 does not match the reviewed artifact",
    );
    check_non_empty(
        &mut issues,
        fixture.contract_id.as_str(),
        "semantics.request.contract",
        "fixture contract id must be non-empty",
    );
    check_non_empty(
        &mut issues,
        fixture.request_resource.as_str(),
        "semantics.request.resource",
        "fixture request resource must be non-empty",
    );
    check_non_empty(
        &mut issues,
        fixture.request_ability.as_str(),
        "semantics.request.ability",
        "fixture request ability must be non-empty",
    );
    check_policy_identity(&mut issues);
    check_capability_limit(&mut issues, fixture);
    check_capabilities(&mut issues, fixture);
    check_non_claims(&mut issues, fixture.non_claims.as_slice());
    check_expectation(&mut issues, fixture);

    issues
}

/// Parse and replay one JSON fixture against Basalt's embedded generated policy.
#[must_use]
pub fn replay_semantics_fixture_json(input: &str) -> SemanticsReplayReport {
    let Ok(fixture) = serde_json::from_str::<UcanSemanticsFixture>(input) else {
        return rejected_report(
            String::new(),
            "semantics.fixture.json",
            "fixture JSON does not match the UCAN semantics schema",
        );
    };

    replay_semantics_fixture(&fixture)
}

/// Replay one typed fixture against Basalt's embedded generated policy.
#[must_use]
pub fn replay_semantics_fixture(fixture: &UcanSemanticsFixture) -> SemanticsReplayReport {
    let issues = validate_semantics_fixture(fixture);
    if !issues.is_empty() {
        return invalid_fixture_report(fixture.fixture_id.clone(), issues);
    }

    let Ok(policy) = default_policy() else {
        return rejected_report(
            fixture.fixture_id.clone(),
            "semantics.policy.parse",
            "embedded Basalt policy failed closed during parsing",
        );
    };

    replay_valid_fixture(&policy, fixture)
}

fn replay_valid_fixture(policy: &Policy, fixture: &UcanSemanticsFixture) -> SemanticsReplayReport {
    let (enforcement, reason_class, mut issues) = match precondition_denial(fixture) {
        Some((enforcement, reason_class)) => (enforcement, reason_class, Vec::new()),
        None => enforce_admitted_fixture(policy, fixture),
    };

    if enforcement.allowed != fixture.expected.allowed
        || reason_class != fixture.expected.reason_class
    {
        issues.push(SemanticsIssue::new(
            "semantics.expected.mismatch",
            "actual Basalt replay result does not match the fixture expectation",
        ));
    }

    SemanticsReplayReport {
        fixture_id: fixture.fixture_id.clone(),
        accepted: issues.is_empty(),
        enforcement,
        reason_class,
        issues,
    }
}

// Explicit branching keeps the fail-closed error path visible.
#[allow(clippy::option_if_let_else)]
fn enforce_admitted_fixture(
    policy: &Policy,
    fixture: &UcanSemanticsFixture,
) -> (
    EnforcementReceipt,
    SemanticsReasonClass,
    Vec<SemanticsIssue>,
) {
    let mut capabilities = fixture.direct_capabilities.clone();
    capabilities.extend_from_slice(fixture.effective_delegated_capabilities.as_slice());
    let request = EnforcementRequest::new(
        fixture.contract_id.clone(),
        fixture.request_resource.clone(),
        fixture.request_ability.clone(),
    )
    .with_capabilities(capabilities);

    match enforce(policy, &request) {
        Ok(enforcement) => {
            let reason_class = classify_enforcement(&enforcement);
            (enforcement, reason_class, Vec::new())
        }
        Err(_) => (
            EnforcementReceipt::denied(ENFORCEMENT_ERROR_REASON),
            SemanticsReasonClass::EnforcementError,
            vec![SemanticsIssue::new(
                "semantics.enforcement.error",
                "Basalt enforcement returned an error and the adapter failed closed",
            )],
        ),
    }
}

// Keep the no-precondition return visible instead of hiding it behind `?`.
#[allow(clippy::question_mark)]
fn precondition_denial(
    fixture: &UcanSemanticsFixture,
) -> Option<(EnforcementReceipt, SemanticsReasonClass)> {
    let Some(reason_class) = precondition_reason_class(fixture) else {
        return None;
    };
    let reason = match reason_class {
        SemanticsReasonClass::WildcardRequestDenied => "UCAN request ability must be concrete",
        SemanticsReasonClass::DelegationDenied => "UCAN delegation decision denied the fixture",
        SemanticsReasonClass::CaveatDenied => "UCAN caveat decision denied the fixture",
        SemanticsReasonClass::ProofDenied => "UCAN proof traversal decision denied the fixture",
        SemanticsReasonClass::ReplayDenied => "UCAN replay decision denied the fixture",
        _ => "UCAN semantics precondition denied the fixture",
    };
    Some((EnforcementReceipt::denied(reason), reason_class))
}

// Explicit branching keeps invalid UCAN requests and decision ordering visible.
#[allow(clippy::option_if_let_else)]
fn precondition_reason_class(fixture: &UcanSemanticsFixture) -> Option<SemanticsReasonClass> {
    let request_is_concrete = match ucan::VerifiedCapability::new(
        fixture.request_resource.clone(),
        fixture.request_ability.clone(),
    ) {
        Ok(request) => request.is_concrete(),
        Err(_) => false,
    };
    if !request_is_concrete {
        return Some(SemanticsReasonClass::WildcardRequestDenied);
    }
    if fixture.decisions.delegation != DelegationDecision::Accepted {
        return Some(SemanticsReasonClass::DelegationDenied);
    }
    if fixture.decisions.caveat_evaluation != CaveatDecision::Satisfied {
        return Some(SemanticsReasonClass::CaveatDenied);
    }
    if fixture.decisions.proof_traversal != ProofTraversalDecision::Accepted {
        return Some(SemanticsReasonClass::ProofDenied);
    }
    if fixture.decisions.replay != ReplayDecision::Admitted {
        return Some(SemanticsReasonClass::ReplayDenied);
    }
    None
}

fn classify_enforcement(enforcement: &EnforcementReceipt) -> SemanticsReasonClass {
    if enforcement.is_allowed() {
        SemanticsReasonClass::Allowed
    } else if enforcement.reason().contains("ucan capability set") {
        SemanticsReasonClass::UcanCapabilityDenied
    } else {
        SemanticsReasonClass::PolicyDenied
    }
}

fn check_exact(
    issues: &mut Vec<SemanticsIssue>,
    actual: &str,
    expected: &str,
    code: &str,
    message: &str,
) {
    if actual != expected {
        issues.push(SemanticsIssue::new(code, message));
    }
}

fn check_fixture_id(issues: &mut Vec<SemanticsIssue>, fixture_id: &str) {
    if fixture_id.is_empty() || fixture_id.len() > MAXIMUM_FIXTURE_ID_BYTES {
        issues.push(SemanticsIssue::new(
            "semantics.fixture-id",
            "fixture id must be non-empty and within the bounded byte length",
        ));
    }
}

fn check_non_empty(issues: &mut Vec<SemanticsIssue>, value: &str, code: &str, message: &str) {
    if value.is_empty() {
        issues.push(SemanticsIssue::new(code, message));
    }
}

fn check_policy_identity(issues: &mut Vec<SemanticsIssue>) {
    if default_policy_blake3() != BASALT_POLICY_BLAKE3 {
        issues.push(SemanticsIssue::new(
            "semantics.identity.embedded-policy-drift",
            "embedded Basalt policy bytes do not match the bound BLAKE3 identity",
        ));
    }
}

fn check_capability_limit(issues: &mut Vec<SemanticsIssue>, fixture: &UcanSemanticsFixture) {
    let capability_count = fixture
        .direct_capabilities
        .len()
        .saturating_add(fixture.effective_delegated_capabilities.len());
    if capability_count > MAXIMUM_FIXTURE_CAPABILITIES {
        issues.push(SemanticsIssue::new(
            "semantics.capabilities.limit",
            "fixture capability count exceeds the bounded maximum",
        ));
    }
}

fn check_capabilities(issues: &mut Vec<SemanticsIssue>, fixture: &UcanSemanticsFixture) {
    for capability in &fixture.direct_capabilities {
        if ucan::VerifiedCapability::new(capability.resource.clone(), capability.ability.clone())
            .is_err()
        {
            issues.push(SemanticsIssue::new(
                "semantics.capabilities.direct-invalid",
                "fixture contains an invalid direct UCAN capability",
            ));
        }
    }
    for capability in &fixture.effective_delegated_capabilities {
        if ucan::VerifiedCapability::new(capability.resource.clone(), capability.ability.clone())
            .is_err()
        {
            issues.push(SemanticsIssue::new(
                "semantics.capabilities.delegated-invalid",
                "fixture contains an invalid effective delegated UCAN capability",
            ));
        }
    }
    if ucan::VerifiedCapability::new(
        fixture.request_resource.clone(),
        fixture.request_ability.clone(),
    )
    .is_err()
    {
        issues.push(SemanticsIssue::new(
            "semantics.request.invalid",
            "fixture request is not a valid UCAN resource and ability pair",
        ));
    }
}

fn check_non_claims(issues: &mut Vec<SemanticsIssue>, non_claims: &[String]) {
    let expected = REQUIRED_NON_CLAIMS.map(str::to_owned);
    if non_claims != expected {
        issues.push(SemanticsIssue::new(
            "semantics.non-claims",
            "fixture must preserve the complete ordered non-claim boundary",
        ));
    }
}

fn check_expectation(issues: &mut Vec<SemanticsIssue>, fixture: &UcanSemanticsFixture) {
    let expected_allow_bit = fixture.expected.reason_class == SemanticsReasonClass::Allowed;
    if fixture.expected.allowed != expected_allow_bit {
        issues.push(SemanticsIssue::new(
            "semantics.expected.allow-bit",
            "fixture allow bit does not match its expected reason class",
        ));
    }

    match precondition_reason_class(fixture) {
        Some(reason_class) if fixture.expected.reason_class != reason_class => {
            issues.push(SemanticsIssue::new(
                "semantics.expected.precondition",
                "fixture expectation is incompatible with its UCAN decision facts",
            ));
        }
        None if !matches!(
            fixture.expected.reason_class,
            SemanticsReasonClass::Allowed
                | SemanticsReasonClass::PolicyDenied
                | SemanticsReasonClass::UcanCapabilityDenied
        ) =>
        {
            issues.push(SemanticsIssue::new(
                "semantics.expected.reason-class",
                "fixture expectation uses a result class unavailable to admitted facts",
            ));
        }
        Some(_) | None => {}
    }
}

fn rejected_report(fixture_id: String, code: &str, message: &str) -> SemanticsReplayReport {
    invalid_fixture_report(fixture_id, vec![SemanticsIssue::new(code, message)])
}

fn invalid_fixture_report(
    fixture_id: String,
    issues: Vec<SemanticsIssue>,
) -> SemanticsReplayReport {
    SemanticsReplayReport {
        fixture_id,
        accepted: false,
        enforcement: EnforcementReceipt::denied(FIXTURE_INVALID_REASON),
        reason_class: SemanticsReasonClass::FixtureInvalid,
        issues,
    }
}
