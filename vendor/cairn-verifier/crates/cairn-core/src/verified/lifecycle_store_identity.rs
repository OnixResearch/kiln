//! Pure lifecycle-store identity admission and local locator transitions.
//!
//! The shell supplies canonical source identities and normalized checkout-path
//! text after it performs filesystem, symlink, capability, and lock checks.
//! This module treats checkout paths as opaque local display facts. It performs
//! no I/O and makes no claim about path existence, ownership, or safety.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::evidence::{blake3_hex, canonical_json, canonical_json_hash};

// r[impl cairn.lifecycle_stores.identity]
// r[impl cairn.lifecycle_stores.locator]
// r[impl cairn.lifecycle_stores.functional_boundary.pure]
// r[impl cairn.lifecycle_stores.receipts]

pub const LIFECYCLE_STORE_IDENTITY_SCHEMA_V1: &str = "cairn.lifecycle-store-identity.v1";
pub const LIFECYCLE_STORE_LOCATOR_SCHEMA_V1: &str = "cairn.lifecycle-store-locator.v1";
pub const LIFECYCLE_STORE_LOCATOR_PLAN_SCHEMA_V1: &str = "cairn.lifecycle-store-locator-plan.v1";
pub const LIFECYCLE_STORE_LOCAL_EVIDENCE_CLASS: &str = "local-only";
pub const LIFECYCLE_STORE_PORTABLE_FACT_CLASS: &str = "portable-facts";
pub const LIFECYCLE_STORE_IDENTITY_OBSERVATION_NON_CLAIM: &str =
    "store identity proves only the observed checked projection and native lifecycle identity";
pub const LIFECYCLE_STORE_IDENTITY_REMOTE_NON_CLAIM: &str =
    "store identity does not prove remote freshness, ownership, trust, or VCS synchronization";
pub const LIFECYCLE_STORE_IDENTITY_READINESS_NON_CLAIM: &str = "store identity does not prove requirement truth, implementation, lifecycle readiness, or release eligibility";
pub const LIFECYCLE_STORE_IDENTITY_NON_CLAIMS: &[&str] = &[
    LIFECYCLE_STORE_IDENTITY_OBSERVATION_NON_CLAIM,
    LIFECYCLE_STORE_IDENTITY_REMOTE_NON_CLAIM,
    LIFECYCLE_STORE_IDENTITY_READINESS_NON_CLAIM,
];
pub const LIFECYCLE_STORE_LOCATOR_NON_CLAIM: &str =
    "registration proves only one machine-local locator mapping";
pub const LIFECYCLE_STORE_REQUIRED_NON_CLAIMS: &[&str] = &[
    "a valid store proves only a bounded native lifecycle identity at one observed local path",
    LIFECYCLE_STORE_LOCATOR_NON_CLAIM,
    "references do not prove truth, freshness, ownership, implementation, or requirement satisfaction",
    "references do not satisfy dependencies, gates, evidence, lifecycle policy, or release policy",
    "worksets do not select lifecycle roots or authorize member mutation",
    "opener plans and process observations do not prove editor state, retained folders, or saved work",
    "store and context commands do not perform VCS, network, credential, or hosting-service transport",
    "local locator and workset receipts are local-only and are not release evidence by default",
    "portable hashes exclude absolute host paths and do not prove cross-machine trust",
    "lifecycle store outputs do not prove lifecycle readiness or release eligibility",
];

pub const LIFECYCLE_STORE_ID_MAX_BYTES: usize = 64;
pub const LIFECYCLE_STORE_REMOTE_HINT_MAX_BYTES: usize = 2_048;
pub const LIFECYCLE_STORE_NON_CLAIM_MAX_BYTES: usize = 512;
pub const LIFECYCLE_STORE_NON_CLAIMS_MAX_COUNT: usize = 32;
pub const LIFECYCLE_STORE_CHECKOUT_PATH_MAX_BYTES: usize = 4_096;
pub const LIFECYCLE_STORE_LOCATOR_MAX_MAPPINGS: usize = 256;
pub const LIFECYCLE_STORE_DIAGNOSTIC_MAX_COUNT: usize = 256;
pub const BLAKE3_IDENTITY_HEX_LENGTH: usize = 64;

const STORE_IDENTITY_DOMAIN: &str = "cairn-lifecycle-store-identity-v1";
const LOCATOR_LOCAL_STATE_DOMAIN: &str = "cairn-lifecycle-store-locator-local-state-v1";
const LOCATOR_PORTABLE_STATE_DOMAIN: &str = "cairn-lifecycle-store-locator-portable-state-v1";

pub const DIAGNOSTIC_SCHEMA_UNSUPPORTED: &str = "lifecycle_store.identity.invalid";
pub const DIAGNOSTIC_STORE_ID_INVALID: &str = "lifecycle_store.identity.invalid";
pub const DIAGNOSTIC_REMOTE_HINT_UNSAFE: &str = "lifecycle_store.remote.unsafe";
pub const DIAGNOSTIC_SOURCE_IDENTITY_INVALID: &str = "lifecycle_store.identity.invalid";
pub const DIAGNOSTIC_NON_CLAIM_BOUND_EXCEEDED: &str = "lifecycle_store.identity.invalid";
pub const DIAGNOSTIC_NON_CLAIM_UNSAFE: &str = "lifecycle_store.identity.invalid";
pub const DIAGNOSTIC_NON_CLAIM_DUPLICATE: &str = "lifecycle_store.identity.invalid";
pub const DIAGNOSTIC_NON_CLAIM_MISSING: &str = "lifecycle_store.identity.invalid";
pub const DIAGNOSTIC_LOCATOR_SCHEMA_UNSUPPORTED: &str = "lifecycle_store.identity.invalid";
pub const DIAGNOSTIC_LOCATOR_MAPPING_BOUND_EXCEEDED: &str = "lifecycle_store.identity.invalid";
pub const DIAGNOSTIC_CHECKOUT_PATH_UNSAFE: &str = "lifecycle_store.identity.invalid";
pub const DIAGNOSTIC_STORE_IDENTITY_INVALID: &str = "lifecycle_store.identity.invalid";
pub const DIAGNOSTIC_LOCATOR_STATE_IDENTITY_INVALID: &str = "lifecycle_store.identity.invalid";
pub const DIAGNOSTIC_LOCATOR_STATE_IDENTITY_STALE: &str = "lifecycle_store.identity.invalid";
pub const DIAGNOSTIC_DUPLICATE_ID: &str = "lifecycle_store.locator.duplicate_id";
pub const DIAGNOSTIC_DUPLICATE_PATH: &str = "lifecycle_store.locator.duplicate_path";
pub const DIAGNOSTIC_IDENTITY_CHANGED: &str = "lifecycle_store.locator.identity_changed";
pub const DIAGNOSTIC_STORE_UNKNOWN: &str = "lifecycle_store.identity.invalid";
pub const DIAGNOSTIC_PLAN_BLOCKED: &str = "lifecycle_store.locator.stale_plan";
pub const DIAGNOSTIC_PLAN_HASH_MISMATCH: &str = "lifecycle_store.locator.stale_plan";
pub const DIAGNOSTIC_PORTABLE_HASH_MISMATCH: &str = "lifecycle_store.locator.stale_plan";
pub const DIAGNOSTIC_PLAN_STALE: &str = "lifecycle_store.locator.stale_plan";
pub const DIAGNOSTIC_PLAN_NEXT_STATE_MISSING: &str = "lifecycle_store.locator.stale_plan";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct LifecycleStoreId(String);

impl LifecycleStoreId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct LifecycleStoreRemoteHint(String);

impl LifecycleStoreRemoteHint {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct LifecycleStoreNonClaim(String);

impl LifecycleStoreNonClaim {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct LifecycleStoreBlake3Identity(String);

impl LifecycleStoreBlake3Identity {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn from_computed(value: String) -> Self {
        debug_assert!(is_blake3_identity(&value));
        Self(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct LifecycleStoreCheckoutPath(String);

impl LifecycleStoreCheckoutPath {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleStoreIdentityProjection {
    pub schema: String,
    pub store_id: String,
    pub remote_hint: Option<String>,
    #[serde(default)]
    pub non_claims: Vec<String>,
    #[serde(rename = "source_blake3")]
    pub source_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LifecycleStoreIdentity {
    pub schema: String,
    pub store_id: LifecycleStoreId,
    pub remote_hint: Option<LifecycleStoreRemoteHint>,
    pub non_claims: Vec<LifecycleStoreNonClaim>,
    #[serde(rename = "source_blake3")]
    pub source_identity: LifecycleStoreBlake3Identity,
    pub store_identity: LifecycleStoreBlake3Identity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LifecycleStoreDiagnostic {
    pub code: String,
    #[serde(rename = "field_path")]
    pub field: String,
    pub subject: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LifecycleStoreIdentityAdmission {
    pub valid: bool,
    pub identity: Option<LifecycleStoreIdentity>,
    pub diagnostics: Vec<LifecycleStoreDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleStoreLocatorMappingProjection {
    pub store_id: String,
    pub checkout_path: String,
    pub store_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleStoreLocatorProjection {
    pub schema: String,
    #[serde(default, rename = "entries")]
    pub mappings: Vec<LifecycleStoreLocatorMappingProjection>,
    #[serde(rename = "state_hash")]
    pub state_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct LifecycleStoreLocatorMapping {
    pub store_id: LifecycleStoreId,
    pub checkout_path: LifecycleStoreCheckoutPath,
    pub store_identity: LifecycleStoreBlake3Identity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LifecycleStoreLocatorState {
    pub schema: String,
    pub mappings: Vec<LifecycleStoreLocatorMapping>,
    pub state_identity: LifecycleStoreBlake3Identity,
    pub portable_state_identity: LifecycleStoreBlake3Identity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LifecycleStoreLocatorAdmission {
    pub valid: bool,
    pub state: Option<LifecycleStoreLocatorState>,
    pub diagnostics: Vec<LifecycleStoreDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleStoreLocatorOperation {
    Register,
    Unregister,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LifecycleStoreLocatorLocalFacts {
    pub evidence_class: String,
    pub operation: LifecycleStoreLocatorOperation,
    pub store_id: String,
    pub checkout_path: Option<String>,
    #[serde(rename = "expected_prior_state_hash")]
    pub expected_prior_state_identity: String,
    #[serde(rename = "next_state_hash")]
    pub next_state_identity: Option<String>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LifecycleStoreLocatorPortableFacts {
    pub fact_class: String,
    pub operation: LifecycleStoreLocatorOperation,
    pub store_id: String,
    pub store_identity: Option<String>,
    pub expected_prior_portable_state_identity: String,
    pub next_portable_state_identity: Option<String>,
    pub changed: bool,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LifecycleStoreLocatorPlan {
    pub schema: String,
    pub blocked: bool,
    pub changed: bool,
    #[serde(rename = "expected_prior_state_hash")]
    pub expected_prior_state_identity: String,
    pub expected_prior_portable_state_identity: String,
    pub next_state: Option<LifecycleStoreLocatorState>,
    pub local_facts: LifecycleStoreLocatorLocalFacts,
    pub portable_facts: LifecycleStoreLocatorPortableFacts,
    pub diagnostics: Vec<LifecycleStoreDiagnostic>,
    pub portable_hash: String,
    pub plan_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LifecycleStoreLocatorApplication {
    pub accepted: bool,
    pub next_state: Option<LifecycleStoreLocatorState>,
    pub diagnostics: Vec<LifecycleStoreDiagnostic>,
}

/// Admit a checked runtime projection without reading its Nickel source.
#[must_use]
pub fn admit_lifecycle_store_identity(
    projection: &LifecycleStoreIdentityProjection,
) -> LifecycleStoreIdentityAdmission {
    let mut diagnostics = Vec::new();
    validate_identity_schema(&projection.schema, &mut diagnostics);
    let store_id = parse_store_id(&projection.store_id, "store_id", &mut diagnostics);
    let remote_hint = parse_remote_hint(&projection.remote_hint, &mut diagnostics);
    let non_claims = parse_non_claims(&projection.non_claims, &mut diagnostics);
    let source_identity = parse_blake3_identity(
        &projection.source_identity,
        "source_blake3",
        DIAGNOSTIC_SOURCE_IDENTITY_INVALID,
        &mut diagnostics,
    );
    sort_diagnostics(&mut diagnostics);

    let identity = if diagnostics.is_empty() {
        build_store_identity(store_id, remote_hint, non_claims, source_identity)
    } else {
        None
    };
    let admission = LifecycleStoreIdentityAdmission {
        valid: identity.is_some(),
        identity,
        diagnostics,
    };
    debug_assert_eq!(admission.valid, admission.diagnostics.is_empty());
    admission
}

/// Return canonical JSON bytes as text for the admitted store identity.
#[must_use]
pub fn lifecycle_store_identity_material(identity: &LifecycleStoreIdentity) -> String {
    canonical_store_identity_material(
        &identity.schema,
        identity.store_id.as_str(),
        identity.remote_hint.as_ref().map(|value| value.as_str()),
        &identity
            .non_claims
            .iter()
            .map(|value| value.as_str().to_string())
            .collect::<Vec<_>>(),
        identity.source_identity.as_str(),
    )
}

/// Hash canonical source material that the shell or export boundary supplies.
#[must_use]
pub fn lifecycle_store_source_identity(canonical_source_material: &[u8]) -> String {
    blake3_hex(canonical_source_material)
}

#[must_use]
pub fn empty_lifecycle_store_locator() -> LifecycleStoreLocatorState {
    build_locator_state(Vec::new())
}

#[must_use]
pub fn lifecycle_store_locator_projection(
    state: &LifecycleStoreLocatorState,
) -> LifecycleStoreLocatorProjection {
    let mappings = state
        .mappings
        .iter()
        .map(|mapping| LifecycleStoreLocatorMappingProjection {
            store_id: mapping.store_id.as_str().to_string(),
            checkout_path: mapping.checkout_path.as_str().to_string(),
            store_identity: mapping.store_identity.as_str().to_string(),
        })
        .collect();
    LifecycleStoreLocatorProjection {
        schema: state.schema.clone(),
        mappings,
        state_identity: state.state_identity.as_str().to_string(),
    }
}

/// Admit persisted locator values after the shell loads bytes.
#[must_use]
pub fn admit_lifecycle_store_locator(
    projection: &LifecycleStoreLocatorProjection,
) -> LifecycleStoreLocatorAdmission {
    let mut diagnostics = Vec::new();
    validate_locator_schema(&projection.schema, &mut diagnostics);
    validate_mapping_count(projection.mappings.len(), &mut diagnostics);
    let mappings = parse_locator_mappings(&projection.mappings, &mut diagnostics);
    validate_mapping_uniqueness(&mappings, &mut diagnostics);
    let stated_identity = parse_blake3_identity(
        &projection.state_identity,
        "state_identity",
        DIAGNOSTIC_LOCATOR_STATE_IDENTITY_INVALID,
        &mut diagnostics,
    );
    let expected_identity = local_locator_state_identity(&mappings);
    if stated_identity
        .as_ref()
        .is_some_and(|identity| identity.as_str() != expected_identity)
    {
        push_diagnostic(
            &mut diagnostics,
            DIAGNOSTIC_LOCATOR_STATE_IDENTITY_STALE,
            "state_identity",
            "",
            "locator state identity does not bind the canonical local mappings",
        );
    }
    sort_diagnostics(&mut diagnostics);
    let state = diagnostics
        .is_empty()
        .then(|| build_locator_state(mappings));
    let admission = LifecycleStoreLocatorAdmission {
        valid: state.is_some(),
        state,
        diagnostics,
    };
    debug_assert_eq!(admission.valid, admission.diagnostics.is_empty());
    admission
}

/// Plan one registration over already admitted identity and shell-normalized path text.
#[must_use]
pub fn plan_lifecycle_store_registration(
    current: &LifecycleStoreLocatorState,
    identity: &LifecycleStoreIdentity,
    checkout_path: &str,
) -> LifecycleStoreLocatorPlan {
    let mut diagnostics = Vec::new();
    validate_admitted_locator_state(current, &mut diagnostics);
    validate_admitted_store_identity(identity, &mut diagnostics);
    let checkout_path = parse_checkout_path(checkout_path, "checkout_path", &mut diagnostics);
    let proposed_mapping = checkout_path.map(|checkout_path| LifecycleStoreLocatorMapping {
        store_id: identity.store_id.clone(),
        checkout_path,
        store_identity: identity.store_identity.clone(),
    });
    let exact_mapping_exists = proposed_mapping
        .as_ref()
        .is_some_and(|mapping| inspect_registration_conflicts(current, mapping, &mut diagnostics));
    if proposed_mapping.is_some()
        && !exact_mapping_exists
        && current.mappings.len() >= LIFECYCLE_STORE_LOCATOR_MAX_MAPPINGS
    {
        push_diagnostic(
            &mut diagnostics,
            DIAGNOSTIC_LOCATOR_MAPPING_BOUND_EXCEEDED,
            "mappings",
            identity.store_id.as_str(),
            "registration would exceed the reviewed locator mapping bound",
        );
    }
    sort_diagnostics(&mut diagnostics);

    let next_state = if diagnostics.is_empty() {
        next_registration_state(current, proposed_mapping.as_ref(), exact_mapping_exists)
    } else {
        None
    };
    build_locator_plan(
        current,
        LifecycleStoreLocatorOperation::Register,
        identity.store_id.as_str(),
        proposed_mapping.as_ref(),
        next_state,
        diagnostics,
    )
}

/// Plan one unregister operation without changing the supplied state.
#[must_use]
pub fn plan_lifecycle_store_unregistration(
    current: &LifecycleStoreLocatorState,
    store_id: &str,
) -> LifecycleStoreLocatorPlan {
    let mut diagnostics = Vec::new();
    validate_admitted_locator_state(current, &mut diagnostics);
    let store_id = parse_store_id(store_id, "store_id", &mut diagnostics);
    let mapping = store_id.as_ref().and_then(|store_id| {
        current
            .mappings
            .iter()
            .find(|mapping| &mapping.store_id == store_id)
            .cloned()
    });
    if store_id.is_some() && mapping.is_none() {
        push_diagnostic(
            &mut diagnostics,
            DIAGNOSTIC_STORE_UNKNOWN,
            "store_id",
            store_id.as_ref().map_or("", LifecycleStoreId::as_str),
            "store ID does not resolve to a local locator mapping",
        );
    }
    sort_diagnostics(&mut diagnostics);
    let next_state = if diagnostics.is_empty() {
        mapping
            .as_ref()
            .map(|mapping| next_unregistration_state(current, mapping))
    } else {
        None
    };
    build_locator_plan(
        current,
        LifecycleStoreLocatorOperation::Unregister,
        store_id.as_ref().map_or("", LifecycleStoreId::as_str),
        mapping.as_ref(),
        next_state,
        diagnostics,
    )
}

/// Recheck a reviewed plan against the current admitted state.
///
/// The returned state is a value for the shell to serialize atomically. This
/// function does not lock, write, rename, or otherwise mutate local state.
#[must_use]
pub fn apply_lifecycle_store_locator_plan(
    current: &LifecycleStoreLocatorState,
    plan: &LifecycleStoreLocatorPlan,
) -> LifecycleStoreLocatorApplication {
    let mut diagnostics = plan.diagnostics.clone();
    diagnostics.extend(validate_plan_integrity(plan));
    validate_admitted_locator_state(current, &mut diagnostics);
    if let Some(next_state) = &plan.next_state {
        validate_admitted_locator_state(next_state, &mut diagnostics);
    }
    if plan.blocked {
        push_diagnostic(
            &mut diagnostics,
            DIAGNOSTIC_PLAN_BLOCKED,
            "plan.blocked",
            "",
            "a blocked locator plan cannot produce a replacement state",
        );
    }
    if current.state_identity.as_str() != plan.expected_prior_state_identity
        || current.portable_state_identity.as_str() != plan.expected_prior_portable_state_identity
    {
        push_diagnostic(
            &mut diagnostics,
            DIAGNOSTIC_PLAN_STALE,
            "expected_prior_state_identity",
            "",
            "current locator state does not match the reviewed expected prior state",
        );
    }
    if plan.next_state.is_none() {
        push_diagnostic(
            &mut diagnostics,
            DIAGNOSTIC_PLAN_NEXT_STATE_MISSING,
            "next_state",
            "",
            "an accepted locator plan must contain one canonical next state",
        );
    }
    sort_diagnostics(&mut diagnostics);
    let next_state = if diagnostics.is_empty() {
        plan.next_state.clone()
    } else {
        None
    };
    let application = LifecycleStoreLocatorApplication {
        accepted: next_state.is_some(),
        next_state,
        diagnostics,
    };
    debug_assert_eq!(application.accepted, application.diagnostics.is_empty());
    application
}

#[must_use]
pub fn lifecycle_store_locator_plan_hash(plan: &LifecycleStoreLocatorPlan) -> String {
    let mut value = serde_json::to_value(plan).expect("locator plan serialization is infallible");
    if let Some(object) = value.as_object_mut() {
        object.remove("plan_hash");
    }
    canonical_json_hash(&value)
}

#[must_use]
pub fn lifecycle_store_locator_portable_hash(facts: &LifecycleStoreLocatorPortableFacts) -> String {
    let value = serde_json::to_value(facts).expect("portable fact serialization is infallible");
    canonical_json_hash(&value)
}

fn validate_identity_schema(schema: &str, diagnostics: &mut Vec<LifecycleStoreDiagnostic>) {
    if schema != LIFECYCLE_STORE_IDENTITY_SCHEMA_V1 {
        push_diagnostic(
            diagnostics,
            DIAGNOSTIC_SCHEMA_UNSUPPORTED,
            "schema",
            "",
            "lifecycle-store identity schema is unsupported",
        );
    }
}

fn validate_admitted_store_identity(
    identity: &LifecycleStoreIdentity,
    diagnostics: &mut Vec<LifecycleStoreDiagnostic>,
) {
    let projection = LifecycleStoreIdentityProjection {
        schema: identity.schema.clone(),
        store_id: identity.store_id.as_str().to_string(),
        remote_hint: identity
            .remote_hint
            .as_ref()
            .map(|value| value.as_str().to_string()),
        non_claims: identity
            .non_claims
            .iter()
            .map(|value| value.as_str().to_string())
            .collect(),
        source_identity: identity.source_identity.as_str().to_string(),
    };
    let admission = admit_lifecycle_store_identity(&projection);
    diagnostics.extend(admission.diagnostics);
    if admission.identity.as_ref() != Some(identity) {
        push_diagnostic(
            diagnostics,
            DIAGNOSTIC_STORE_IDENTITY_INVALID,
            "store_identity",
            identity.store_id.as_str(),
            "admitted store identity does not bind its current canonical fields",
        );
    }
}

fn parse_store_id(
    value: &str,
    field: &str,
    diagnostics: &mut Vec<LifecycleStoreDiagnostic>,
) -> Option<LifecycleStoreId> {
    let length_valid = !value.is_empty() && value.len() <= LIFECYCLE_STORE_ID_MAX_BYTES;
    let prefix_valid = value
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit());
    let alphabet_valid = value.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
    });
    if length_valid && prefix_valid && alphabet_valid {
        Some(LifecycleStoreId(value.to_string()))
    } else {
        push_diagnostic(
            diagnostics,
            DIAGNOSTIC_STORE_ID_INVALID,
            field,
            "",
            "store ID must match the bounded lowercase ASCII store-ID policy",
        );
        None
    }
}

fn parse_remote_hint(
    value: &Option<String>,
    diagnostics: &mut Vec<LifecycleStoreDiagnostic>,
) -> Option<LifecycleStoreRemoteHint> {
    let Some(value) = value else {
        return None;
    };
    if is_safe_remote_hint(value) {
        Some(LifecycleStoreRemoteHint(value.clone()))
    } else {
        push_diagnostic(
            diagnostics,
            DIAGNOSTIC_REMOTE_HINT_UNSAFE,
            "remote_hint",
            "",
            "remote hint must match the bounded informational remote policy",
        );
        None
    }
}

fn parse_non_claims(
    values: &[String],
    diagnostics: &mut Vec<LifecycleStoreDiagnostic>,
) -> Vec<LifecycleStoreNonClaim> {
    if values.len() > LIFECYCLE_STORE_NON_CLAIMS_MAX_COUNT {
        push_diagnostic(
            diagnostics,
            DIAGNOSTIC_NON_CLAIM_BOUND_EXCEEDED,
            "non_claims",
            "",
            "non-claim count exceeds the reviewed bound",
        );
    }
    let mut accepted = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        let field = format!("non_claims[{index}]");
        if !is_safe_bounded_text(value, LIFECYCLE_STORE_NON_CLAIM_MAX_BYTES) {
            push_diagnostic(
                diagnostics,
                DIAGNOSTIC_NON_CLAIM_UNSAFE,
                &field,
                "",
                "non-claim must be non-empty bounded text without controls or surrounding whitespace",
            );
            continue;
        }
        if !seen.insert(value.as_str()) {
            push_diagnostic(
                diagnostics,
                DIAGNOSTIC_NON_CLAIM_DUPLICATE,
                &field,
                "",
                "non-claim values must be unique",
            );
            continue;
        }
        accepted.push(LifecycleStoreNonClaim(value.clone()));
    }
    accepted.sort();
    for required in LIFECYCLE_STORE_IDENTITY_NON_CLAIMS {
        if !accepted.iter().any(|value| value.as_str() == *required) {
            push_diagnostic(
                diagnostics,
                DIAGNOSTIC_NON_CLAIM_MISSING,
                "non_claims",
                "",
                &format!("required lifecycle-store identity non-claim is missing: {required}"),
            );
        }
    }
    accepted
}

fn build_store_identity(
    store_id: Option<LifecycleStoreId>,
    remote_hint: Option<LifecycleStoreRemoteHint>,
    non_claims: Vec<LifecycleStoreNonClaim>,
    source_identity: Option<LifecycleStoreBlake3Identity>,
) -> Option<LifecycleStoreIdentity> {
    let (Some(store_id), Some(source_identity)) = (store_id, source_identity) else {
        return None;
    };
    let non_claim_text = non_claims
        .iter()
        .map(|value| value.as_str().to_string())
        .collect::<Vec<_>>();
    let material = canonical_store_identity_material(
        LIFECYCLE_STORE_IDENTITY_SCHEMA_V1,
        store_id.as_str(),
        remote_hint.as_ref().map(LifecycleStoreRemoteHint::as_str),
        &non_claim_text,
        source_identity.as_str(),
    );
    let store_identity =
        LifecycleStoreBlake3Identity::from_computed(blake3_hex(material.as_bytes()));
    Some(LifecycleStoreIdentity {
        schema: LIFECYCLE_STORE_IDENTITY_SCHEMA_V1.to_string(),
        store_id,
        remote_hint,
        non_claims,
        source_identity,
        store_identity,
    })
}

fn canonical_store_identity_material(
    schema: &str,
    store_id: &str,
    remote_hint: Option<&str>,
    non_claims: &[String],
    source_identity: &str,
) -> String {
    canonical_json(&json!({
        "domain": STORE_IDENTITY_DOMAIN,
        "schema": schema,
        "store_id": store_id,
        "remote_hint": remote_hint,
        "non_claims": non_claims,
        "source_blake3": source_identity,
    }))
}

fn validate_locator_schema(schema: &str, diagnostics: &mut Vec<LifecycleStoreDiagnostic>) {
    if schema != LIFECYCLE_STORE_LOCATOR_SCHEMA_V1 {
        push_diagnostic(
            diagnostics,
            DIAGNOSTIC_LOCATOR_SCHEMA_UNSUPPORTED,
            "schema",
            "",
            "lifecycle-store locator schema is unsupported",
        );
    }
}

fn validate_mapping_count(count: usize, diagnostics: &mut Vec<LifecycleStoreDiagnostic>) {
    if count > LIFECYCLE_STORE_LOCATOR_MAX_MAPPINGS {
        push_diagnostic(
            diagnostics,
            DIAGNOSTIC_LOCATOR_MAPPING_BOUND_EXCEEDED,
            "mappings",
            "",
            "locator mapping count exceeds the reviewed bound",
        );
    }
}

fn validate_admitted_locator_state(
    state: &LifecycleStoreLocatorState,
    diagnostics: &mut Vec<LifecycleStoreDiagnostic>,
) {
    validate_locator_schema(&state.schema, diagnostics);
    validate_mapping_count(state.mappings.len(), diagnostics);
    validate_mapping_uniqueness(&state.mappings, diagnostics);
    let expected_local = local_locator_state_identity(&state.mappings);
    if state.state_identity.as_str() != expected_local {
        push_diagnostic(
            diagnostics,
            DIAGNOSTIC_LOCATOR_STATE_IDENTITY_STALE,
            "state_identity",
            "",
            "admitted locator state identity does not bind its current local mappings",
        );
    }
    let expected_portable = portable_locator_state_identity(&state.mappings);
    if state.portable_state_identity.as_str() != expected_portable {
        push_diagnostic(
            diagnostics,
            DIAGNOSTIC_LOCATOR_STATE_IDENTITY_STALE,
            "portable_state_identity",
            "",
            "admitted portable state identity does not bind its current semantic mappings",
        );
    }
}

fn parse_locator_mappings(
    values: &[LifecycleStoreLocatorMappingProjection],
    diagnostics: &mut Vec<LifecycleStoreDiagnostic>,
) -> Vec<LifecycleStoreLocatorMapping> {
    let mut mappings = Vec::new();
    for (index, value) in values.iter().enumerate() {
        let store_id = parse_store_id(
            &value.store_id,
            &format!("mappings[{index}].store_id"),
            diagnostics,
        );
        let checkout_path = parse_checkout_path(
            &value.checkout_path,
            &format!("mappings[{index}].checkout_path"),
            diagnostics,
        );
        let store_identity = parse_blake3_identity(
            &value.store_identity,
            &format!("mappings[{index}].store_identity"),
            DIAGNOSTIC_STORE_IDENTITY_INVALID,
            diagnostics,
        );
        if let (Some(store_id), Some(checkout_path), Some(store_identity)) =
            (store_id, checkout_path, store_identity)
        {
            mappings.push(LifecycleStoreLocatorMapping {
                store_id,
                checkout_path,
                store_identity,
            });
        }
    }
    mappings.sort();
    mappings
}

fn parse_checkout_path(
    value: &str,
    field: &str,
    diagnostics: &mut Vec<LifecycleStoreDiagnostic>,
) -> Option<LifecycleStoreCheckoutPath> {
    if is_safe_bounded_text(value, LIFECYCLE_STORE_CHECKOUT_PATH_MAX_BYTES) {
        Some(LifecycleStoreCheckoutPath(value.to_string()))
    } else {
        push_diagnostic(
            diagnostics,
            DIAGNOSTIC_CHECKOUT_PATH_UNSAFE,
            field,
            "",
            "checkout path text must be non-empty, bounded, and free of controls or surrounding whitespace",
        );
        None
    }
}

fn parse_blake3_identity(
    value: &str,
    field: &str,
    code: &str,
    diagnostics: &mut Vec<LifecycleStoreDiagnostic>,
) -> Option<LifecycleStoreBlake3Identity> {
    if is_blake3_identity(value) {
        Some(LifecycleStoreBlake3Identity(value.to_string()))
    } else {
        push_diagnostic(
            diagnostics,
            code,
            field,
            "",
            "identity must be 64 lowercase BLAKE3 hexadecimal characters",
        );
        None
    }
}

fn validate_mapping_uniqueness(
    mappings: &[LifecycleStoreLocatorMapping],
    diagnostics: &mut Vec<LifecycleStoreDiagnostic>,
) {
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for mapping in mappings {
        if !ids.insert(mapping.store_id.as_str()) {
            push_diagnostic(
                diagnostics,
                DIAGNOSTIC_DUPLICATE_ID,
                "mappings.store_id",
                mapping.store_id.as_str(),
                "locator store IDs must be unique",
            );
        }
        if !paths.insert(mapping.checkout_path.as_str()) {
            push_diagnostic(
                diagnostics,
                DIAGNOSTIC_DUPLICATE_PATH,
                "mappings.checkout_path",
                mapping.store_id.as_str(),
                "locator checkout paths must be unique after shell normalization",
            );
        }
    }
}

fn inspect_registration_conflicts(
    current: &LifecycleStoreLocatorState,
    proposed: &LifecycleStoreLocatorMapping,
    diagnostics: &mut Vec<LifecycleStoreDiagnostic>,
) -> bool {
    let mut exact_mapping_exists = false;
    for existing in &current.mappings {
        let same_id = existing.store_id == proposed.store_id;
        let same_path = existing.checkout_path == proposed.checkout_path;
        let same_identity = existing.store_identity == proposed.store_identity;
        if same_id && same_path && same_identity {
            exact_mapping_exists = true;
            continue;
        }
        if same_id && !same_identity {
            push_diagnostic(
                diagnostics,
                DIAGNOSTIC_IDENTITY_CHANGED,
                "store_identity",
                proposed.store_id.as_str(),
                "registered store identity differs from the admitted store identity",
            );
        }
        if same_id && !same_path {
            push_diagnostic(
                diagnostics,
                DIAGNOSTIC_DUPLICATE_ID,
                "store_id",
                proposed.store_id.as_str(),
                "store ID already maps to a different normalized checkout path",
            );
        }
        if same_path && !same_id {
            push_diagnostic(
                diagnostics,
                DIAGNOSTIC_DUPLICATE_PATH,
                "checkout_path",
                proposed.store_id.as_str(),
                "normalized checkout path already maps to a different store ID",
            );
        }
    }
    exact_mapping_exists
}

fn next_registration_state(
    current: &LifecycleStoreLocatorState,
    proposed: Option<&LifecycleStoreLocatorMapping>,
    exact_mapping_exists: bool,
) -> Option<LifecycleStoreLocatorState> {
    let proposed = proposed?;
    if exact_mapping_exists {
        return Some(current.clone());
    }
    if current.mappings.len() >= LIFECYCLE_STORE_LOCATOR_MAX_MAPPINGS {
        return None;
    }
    let mut mappings = current.mappings.clone();
    mappings.push(proposed.clone());
    Some(build_locator_state(mappings))
}

fn next_unregistration_state(
    current: &LifecycleStoreLocatorState,
    removed: &LifecycleStoreLocatorMapping,
) -> LifecycleStoreLocatorState {
    let mappings = current
        .mappings
        .iter()
        .filter(|mapping| *mapping != removed)
        .cloned()
        .collect();
    build_locator_state(mappings)
}

fn build_locator_state(
    mut mappings: Vec<LifecycleStoreLocatorMapping>,
) -> LifecycleStoreLocatorState {
    mappings.sort();
    debug_assert!(mappings.len() <= LIFECYCLE_STORE_LOCATOR_MAX_MAPPINGS);
    debug_assert!(mapping_ids_are_unique(&mappings));
    debug_assert!(mapping_paths_are_unique(&mappings));
    let state_identity = local_locator_state_identity(&mappings);
    let portable_state_identity = portable_locator_state_identity(&mappings);
    LifecycleStoreLocatorState {
        schema: LIFECYCLE_STORE_LOCATOR_SCHEMA_V1.to_string(),
        mappings,
        state_identity: LifecycleStoreBlake3Identity::from_computed(state_identity),
        portable_state_identity: LifecycleStoreBlake3Identity::from_computed(
            portable_state_identity,
        ),
    }
}

fn local_locator_state_identity(mappings: &[LifecycleStoreLocatorMapping]) -> String {
    canonical_json_hash(&json!({
        "domain": LOCATOR_LOCAL_STATE_DOMAIN,
        "schema": LIFECYCLE_STORE_LOCATOR_SCHEMA_V1,
        "mappings": mappings,
    }))
}

fn portable_locator_state_identity(mappings: &[LifecycleStoreLocatorMapping]) -> String {
    let portable = mappings
        .iter()
        .map(|mapping| {
            json!({
                "store_id": mapping.store_id.as_str(),
                "store_identity": mapping.store_identity.as_str(),
            })
        })
        .collect::<Vec<_>>();
    canonical_json_hash(&json!({
        "domain": LOCATOR_PORTABLE_STATE_DOMAIN,
        "schema": LIFECYCLE_STORE_LOCATOR_SCHEMA_V1,
        "mappings": portable,
    }))
}

fn mapping_ids_are_unique(mappings: &[LifecycleStoreLocatorMapping]) -> bool {
    let ids = mappings
        .iter()
        .map(|mapping| mapping.store_id.as_str())
        .collect::<BTreeSet<_>>();
    ids.len() == mappings.len()
}

fn mapping_paths_are_unique(mappings: &[LifecycleStoreLocatorMapping]) -> bool {
    let paths = mappings
        .iter()
        .map(|mapping| mapping.checkout_path.as_str())
        .collect::<BTreeSet<_>>();
    paths.len() == mappings.len()
}

fn build_locator_plan(
    current: &LifecycleStoreLocatorState,
    operation: LifecycleStoreLocatorOperation,
    store_id: &str,
    mapping: Option<&LifecycleStoreLocatorMapping>,
    next_state: Option<LifecycleStoreLocatorState>,
    diagnostics: Vec<LifecycleStoreDiagnostic>,
) -> LifecycleStoreLocatorPlan {
    let changed = next_state
        .as_ref()
        .is_some_and(|state| state.state_identity != current.state_identity);
    let next_state_identity = next_state
        .as_ref()
        .map(|state| state.state_identity.as_str().to_string());
    let next_portable_state_identity = next_state
        .as_ref()
        .map(|state| state.portable_state_identity.as_str().to_string());
    let local_facts = LifecycleStoreLocatorLocalFacts {
        evidence_class: LIFECYCLE_STORE_LOCAL_EVIDENCE_CLASS.to_string(),
        operation,
        store_id: store_id.to_string(),
        checkout_path: mapping.map(|mapping| mapping.checkout_path.as_str().to_string()),
        expected_prior_state_identity: current.state_identity.as_str().to_string(),
        next_state_identity,
        non_claims: lifecycle_store_required_non_claims(),
    };
    let portable_facts = LifecycleStoreLocatorPortableFacts {
        fact_class: LIFECYCLE_STORE_PORTABLE_FACT_CLASS.to_string(),
        operation,
        store_id: store_id.to_string(),
        store_identity: mapping.map(|mapping| mapping.store_identity.as_str().to_string()),
        expected_prior_portable_state_identity: current
            .portable_state_identity
            .as_str()
            .to_string(),
        next_portable_state_identity,
        changed,
        non_claims: lifecycle_store_required_non_claims(),
    };
    let portable_hash = lifecycle_store_locator_portable_hash(&portable_facts);
    let mut plan = LifecycleStoreLocatorPlan {
        schema: LIFECYCLE_STORE_LOCATOR_PLAN_SCHEMA_V1.to_string(),
        blocked: !diagnostics.is_empty(),
        changed,
        expected_prior_state_identity: current.state_identity.as_str().to_string(),
        expected_prior_portable_state_identity: current
            .portable_state_identity
            .as_str()
            .to_string(),
        next_state,
        local_facts,
        portable_facts,
        diagnostics,
        portable_hash,
        plan_hash: String::new(),
    };
    plan.plan_hash = lifecycle_store_locator_plan_hash(&plan);
    debug_assert_eq!(plan.blocked, !plan.diagnostics.is_empty());
    debug_assert_eq!(plan.changed, plan.portable_facts.changed);
    plan
}

fn validate_plan_integrity(plan: &LifecycleStoreLocatorPlan) -> Vec<LifecycleStoreDiagnostic> {
    let mut diagnostics = Vec::new();
    let expected_portable_hash = lifecycle_store_locator_portable_hash(&plan.portable_facts);
    if expected_portable_hash != plan.portable_hash {
        push_diagnostic(
            &mut diagnostics,
            DIAGNOSTIC_PORTABLE_HASH_MISMATCH,
            "portable_hash",
            &plan.portable_facts.store_id,
            "portable hash does not bind canonical portable receipt facts",
        );
    }
    let expected_plan_hash = lifecycle_store_locator_plan_hash(plan);
    if expected_plan_hash != plan.plan_hash {
        push_diagnostic(
            &mut diagnostics,
            DIAGNOSTIC_PLAN_HASH_MISMATCH,
            "plan_hash",
            &plan.portable_facts.store_id,
            "plan hash does not bind canonical local and portable facts",
        );
    }
    diagnostics
}

fn lifecycle_store_required_non_claims() -> Vec<String> {
    LIFECYCLE_STORE_REQUIRED_NON_CLAIMS
        .iter()
        .map(|value| (*value).to_string())
        .collect()
}

fn is_safe_remote_hint(value: &str) -> bool {
    let prefix_valid = value
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphanumeric);
    prefix_valid
        && value.len() <= LIFECYCLE_STORE_REMOTE_HINT_MAX_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'+' | b'.' | b'_' | b':' | b'/' | b'@' | b'~' | b'-')
        })
}

fn is_safe_bounded_text(value: &str, maximum_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum_bytes
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn is_blake3_identity(value: &str) -> bool {
    value.len() == BLAKE3_IDENTITY_HEX_LENGTH
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn push_diagnostic(
    diagnostics: &mut Vec<LifecycleStoreDiagnostic>,
    code: &str,
    field: &str,
    subject: &str,
    message: &str,
) {
    diagnostics.push(LifecycleStoreDiagnostic {
        code: code.to_string(),
        field: field.to_string(),
        subject: subject.to_string(),
        message: message.to_string(),
    });
}

fn sort_diagnostics(diagnostics: &mut Vec<LifecycleStoreDiagnostic>) {
    diagnostics.sort();
    diagnostics.dedup();
    if diagnostics.len() > LIFECYCLE_STORE_DIAGNOSTIC_MAX_COUNT {
        diagnostics.truncate(LIFECYCLE_STORE_DIAGNOSTIC_MAX_COUNT - 1);
        push_diagnostic(
            diagnostics,
            DIAGNOSTIC_NON_CLAIM_BOUND_EXCEEDED,
            "diagnostics",
            "",
            "diagnostic count exceeded the reviewed bound and was deterministically truncated",
        );
        diagnostics.sort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE_HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const STORE_ALPHA: &str = "alpha";
    const STORE_BETA: &str = "beta";
    const PATH_ALPHA: &str = "/home/operator/alpha";
    const PATH_ALPHA_OTHER: &str = "/srv/lifecycle/alpha";
    const PATH_BETA: &str = "/home/operator/beta";

    fn projection(store_id: &str) -> LifecycleStoreIdentityProjection {
        LifecycleStoreIdentityProjection {
            schema: LIFECYCLE_STORE_IDENTITY_SCHEMA_V1.to_string(),
            store_id: store_id.to_string(),
            remote_hint: Some(format!("https://example.invalid/{store_id}.git")),
            non_claims: LIFECYCLE_STORE_IDENTITY_NON_CLAIMS
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            source_identity: SOURCE_HASH_A.to_string(),
        }
    }

    fn identity(store_id: &str) -> LifecycleStoreIdentity {
        let admission = admit_lifecycle_store_identity(&projection(store_id));
        assert!(admission.valid, "{:?}", admission.diagnostics);
        admission.identity.expect("valid identity")
    }

    fn apply(plan: &LifecycleStoreLocatorPlan) -> LifecycleStoreLocatorState {
        let application =
            apply_lifecycle_store_locator_plan(&empty_lifecycle_store_locator(), plan);
        assert!(application.accepted, "{:?}", application.diagnostics);
        application.next_state.expect("accepted next state")
    }

    fn codes(diagnostics: &[LifecycleStoreDiagnostic]) -> Vec<&str> {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect()
    }

    #[test]
    // r[verify cairn.lifecycle_stores.identity.valid]
    fn valid_identity_admission_is_typed_canonical_and_deterministic() {
        let first = admit_lifecycle_store_identity(&projection(STORE_ALPHA));
        let second = admit_lifecycle_store_identity(&projection(STORE_ALPHA));
        assert!(first.valid, "{:?}", first.diagnostics);
        assert_eq!(first, second);
        let first = first.identity.expect("first identity");
        let second = second.identity.expect("second identity");
        assert_eq!(
            lifecycle_store_identity_material(&first),
            lifecycle_store_identity_material(&second)
        );
        assert_eq!(first.store_identity, second.store_identity);
        assert_eq!(first.store_id.as_str(), STORE_ALPHA);
        assert!(is_blake3_identity(first.store_identity.as_str()));
        let serialized =
            serde_json::to_value(projection(STORE_ALPHA)).expect("identity projection serializes");
        assert_eq!(serialized["source_blake3"], SOURCE_HASH_A);
        assert!(serialized.get("source_identity").is_none());
    }

    #[test]
    // r[verify cairn.lifecycle_stores.identity.invalid]
    fn unsafe_identity_data_fails_with_sorted_diagnostics() {
        let candidate = LifecycleStoreIdentityProjection {
            schema: "unsupported".to_string(),
            store_id: "../Bad\n".to_string(),
            remote_hint: Some("https://example.invalid/repo\ncommand".to_string()),
            non_claims: vec!["promotes trust\n".to_string()],
            source_identity: SOURCE_HASH_A.to_ascii_uppercase(),
        };
        let report = admit_lifecycle_store_identity(&candidate);
        let repeated = admit_lifecycle_store_identity(&candidate);
        assert!(!report.valid);
        assert!(report.identity.is_none());
        assert_eq!(report, repeated);
        let mut sorted = report.diagnostics.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(report.diagnostics, sorted);
        let emitted_codes = codes(&report.diagnostics)
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            emitted_codes,
            BTreeSet::from([DIAGNOSTIC_SCHEMA_UNSUPPORTED, DIAGNOSTIC_REMOTE_HINT_UNSAFE,])
        );
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.field == "store_id"));
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.field == "source_blake3"));
    }

    #[test]
    // r[verify cairn.lifecycle_stores.locator.register]
    // r[verify cairn.lifecycle_stores.receipts.local_only]
    fn valid_registration_and_unregistration_return_new_values() {
        let empty = empty_lifecycle_store_locator();
        let register =
            plan_lifecycle_store_registration(&empty, &identity(STORE_ALPHA), PATH_ALPHA);
        assert!(!register.blocked, "{:?}", register.diagnostics);
        assert!(register.changed);
        assert_eq!(
            register.local_facts.evidence_class,
            LIFECYCLE_STORE_LOCAL_EVIDENCE_CLASS
        );
        let registered = apply(&register);
        assert_eq!(registered.mappings.len(), 1);
        assert_eq!(registered.mappings[0].store_id.as_str(), STORE_ALPHA);
        let locator_projection = lifecycle_store_locator_projection(&registered);
        let read_back = admit_lifecycle_store_locator(&locator_projection);
        assert!(read_back.valid, "{:?}", read_back.diagnostics);
        assert_eq!(read_back.state.as_ref(), Some(&registered));
        let serialized =
            serde_json::to_value(locator_projection).expect("locator projection serializes");
        assert!(serialized.get("entries").is_some());
        assert!(serialized.get("state_hash").is_some());
        assert!(serialized.get("mappings").is_none());

        let unregister = plan_lifecycle_store_unregistration(&registered, STORE_ALPHA);
        assert!(!unregister.blocked, "{:?}", unregister.diagnostics);
        let application = apply_lifecycle_store_locator_plan(&registered, &unregister);
        assert!(application.accepted, "{:?}", application.diagnostics);
        assert!(application
            .next_state
            .expect("unregistered state")
            .mappings
            .is_empty());
    }

    #[test]
    // r[verify cairn.lifecycle_stores.locator.conflict]
    fn duplicate_store_ids_fail_without_a_next_state() {
        let first = plan_lifecycle_store_registration(
            &empty_lifecycle_store_locator(),
            &identity(STORE_ALPHA),
            PATH_ALPHA,
        );
        let current = apply(&first);
        let conflict =
            plan_lifecycle_store_registration(&current, &identity(STORE_ALPHA), PATH_ALPHA_OTHER);
        assert!(conflict.blocked);
        assert!(conflict.next_state.is_none());
        assert_eq!(codes(&conflict.diagnostics), vec![DIAGNOSTIC_DUPLICATE_ID]);
    }

    #[test]
    // r[verify cairn.lifecycle_stores.locator.conflict]
    fn duplicate_checkout_paths_fail_without_a_next_state() {
        let first = plan_lifecycle_store_registration(
            &empty_lifecycle_store_locator(),
            &identity(STORE_ALPHA),
            PATH_ALPHA,
        );
        let current = apply(&first);
        let conflict =
            plan_lifecycle_store_registration(&current, &identity(STORE_BETA), PATH_ALPHA);
        assert!(conflict.blocked);
        assert!(conflict.next_state.is_none());
        assert_eq!(
            codes(&conflict.diagnostics),
            vec![DIAGNOSTIC_DUPLICATE_PATH]
        );
    }

    #[test]
    // r[verify cairn.lifecycle_stores.locator.conflict]
    fn changed_store_identity_fails_closed() {
        let original_identity = identity(STORE_ALPHA);
        let first = plan_lifecycle_store_registration(
            &empty_lifecycle_store_locator(),
            &original_identity,
            PATH_ALPHA,
        );
        let current = apply(&first);
        let mut changed_projection = projection(STORE_ALPHA);
        changed_projection.remote_hint = Some("https://mirror.invalid/alpha.git".to_string());
        let changed_admission = admit_lifecycle_store_identity(&changed_projection);
        assert!(
            changed_admission.valid,
            "{:?}",
            changed_admission.diagnostics
        );
        let changed_identity = changed_admission.identity.expect("changed identity");
        assert_ne!(
            original_identity.store_identity,
            changed_identity.store_identity
        );

        let conflict = plan_lifecycle_store_registration(&current, &changed_identity, PATH_ALPHA);
        assert!(conflict.blocked);
        assert!(conflict.next_state.is_none());
        assert_eq!(
            codes(&conflict.diagnostics),
            vec![DIAGNOSTIC_IDENTITY_CHANGED]
        );
    }

    #[test]
    // r[verify cairn.lifecycle_stores.locator.conflict]
    // r[verify cairn.lifecycle_stores.functional_boundary.shell]
    fn stale_plan_is_rejected_without_returning_replacement_state() {
        let empty = empty_lifecycle_store_locator();
        let stale_plan =
            plan_lifecycle_store_registration(&empty, &identity(STORE_ALPHA), PATH_ALPHA);
        let intervening_plan =
            plan_lifecycle_store_registration(&empty, &identity(STORE_BETA), PATH_BETA);
        let current = apply(&intervening_plan);

        let application = apply_lifecycle_store_locator_plan(&current, &stale_plan);
        assert!(!application.accepted);
        assert!(application.next_state.is_none());
        assert_eq!(codes(&application.diagnostics), vec![DIAGNOSTIC_PLAN_STALE]);
        assert_eq!(current.mappings[0].store_id.as_str(), STORE_BETA);
    }

    #[test]
    // r[verify cairn.lifecycle_stores.locator.conflict]
    fn tampered_admitted_values_fail_before_a_transition_is_returned() {
        let empty = empty_lifecycle_store_locator();
        let mut tampered_identity = identity(STORE_ALPHA);
        tampered_identity.store_identity = identity(STORE_BETA).store_identity;
        let blocked = plan_lifecycle_store_registration(&empty, &tampered_identity, PATH_ALPHA);
        assert!(blocked.blocked);
        assert!(blocked.next_state.is_none());
        assert!(codes(&blocked.diagnostics).contains(&DIAGNOSTIC_STORE_IDENTITY_INVALID));

        let register =
            plan_lifecycle_store_registration(&empty, &identity(STORE_ALPHA), PATH_ALPHA);
        let current = apply(&register);
        let unregister = plan_lifecycle_store_unregistration(&current, STORE_ALPHA);
        let mut tampered_current = current;
        tampered_current.mappings.clear();
        let application = apply_lifecycle_store_locator_plan(&tampered_current, &unregister);
        assert!(!application.accepted);
        assert!(application.next_state.is_none());
        assert!(codes(&application.diagnostics).contains(&DIAGNOSTIC_LOCATOR_STATE_IDENTITY_STALE));
    }

    #[test]
    // r[verify cairn.lifecycle_stores.receipts.portable]
    fn local_paths_change_full_plan_hash_but_not_portable_hash() {
        let empty = empty_lifecycle_store_locator();
        let store = identity(STORE_ALPHA);
        let home_plan = plan_lifecycle_store_registration(&empty, &store, PATH_ALPHA);
        let server_plan = plan_lifecycle_store_registration(&empty, &store, PATH_ALPHA_OTHER);
        assert_ne!(home_plan.local_facts, server_plan.local_facts);
        assert_ne!(home_plan.plan_hash, server_plan.plan_hash);
        assert_eq!(home_plan.portable_facts, server_plan.portable_facts);
        assert_eq!(home_plan.portable_hash, server_plan.portable_hash);
    }

    #[test]
    // r[verify cairn.lifecycle_stores.locator.conflict]
    fn locator_projection_rejects_duplicate_ids_paths_and_stale_identity() {
        let first = identity(STORE_ALPHA);
        let second = identity(STORE_BETA);
        let projection = LifecycleStoreLocatorProjection {
            schema: LIFECYCLE_STORE_LOCATOR_SCHEMA_V1.to_string(),
            mappings: vec![
                LifecycleStoreLocatorMappingProjection {
                    store_id: first.store_id.as_str().to_string(),
                    checkout_path: PATH_ALPHA.to_string(),
                    store_identity: first.store_identity.as_str().to_string(),
                },
                LifecycleStoreLocatorMappingProjection {
                    store_id: first.store_id.as_str().to_string(),
                    checkout_path: PATH_BETA.to_string(),
                    store_identity: first.store_identity.as_str().to_string(),
                },
                LifecycleStoreLocatorMappingProjection {
                    store_id: second.store_id.as_str().to_string(),
                    checkout_path: PATH_ALPHA.to_string(),
                    store_identity: second.store_identity.as_str().to_string(),
                },
            ],
            state_identity: SOURCE_HASH_A.to_string(),
        };
        let report = admit_lifecycle_store_locator(&projection);
        assert!(!report.valid);
        assert!(report.state.is_none());
        assert_eq!(
            codes(&report.diagnostics)
                .into_iter()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                DIAGNOSTIC_DUPLICATE_ID,
                DIAGNOSTIC_DUPLICATE_PATH,
                DIAGNOSTIC_LOCATOR_STATE_IDENTITY_STALE,
            ])
        );
    }
}
