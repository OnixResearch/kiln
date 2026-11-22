//! Stateless, source-bound continuation over admitted lifecycle observations.
//!
//! This module owns cohort identity, whole-entry ordering, page cursors,
//! unchanged comparisons, and exact differences. Shells own bounded file reads,
//! explicit prior snapshots, and guarded output. Nothing here grants mutation
//! authority or writes process-global state.
use crate::verified::evidence::{blake3_hex, canonical_json};
use crate::verified::validation::ContextSelector;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{error::Error, fmt, sync::LazyLock};

pub const OBSERVATION_SNAPSHOT_SCHEMA: &str = "cairn.incremental-observation.v1";
pub const OBSERVATION_PAGE_SCHEMA: &str = "cairn.incremental-observation-page.v1";
pub const OBSERVATION_UNCHANGED_SCHEMA: &str = "cairn.incremental-observation-unchanged.v1";
pub const OBSERVATION_DIFFERENCE_SCHEMA: &str = "cairn.incremental-observation-difference.v1";
pub const OBSERVATION_CURSOR_SCHEMA: &str = "cairn.incremental-observation-cursor.v1";
pub const OBSERVATION_COHORT_DOMAIN: &str = "cairn.incremental-observation.cohort.v1";
pub const OBSERVATION_CONTRACT_DOMAIN: &str = "cairn.incremental-observation.contract.v1";
pub const OBSERVATION_ENTRY_DOMAIN: &str = "cairn.incremental-observation.entry.v1";
pub const OBSERVATION_SNAPSHOT_DOMAIN: &str = "cairn.incremental-observation.snapshot.v1";
pub const OBSERVATION_PAGE_DOMAIN: &str = "cairn.incremental-observation.page.v1";
pub const OBSERVATION_UNCHANGED_DOMAIN: &str = "cairn.incremental-observation.unchanged.v1";
pub const OBSERVATION_DIFFERENCE_DOMAIN: &str = "cairn.incremental-observation.difference.v1";
pub const OBSERVATION_POLICY_SCHEMA: &str = "cairn.incremental-observations-policy.v1";
pub const OBSERVATION_ORDERING_VERSION: &str = "lexicographic-identity-byte-order.v1";
pub const OBSERVATION_STATE_COMPLETE: &str = "complete";
pub const OBSERVATION_STATE_PARTIAL: &str = "partial";
pub const OBSERVATION_MAX_ROOT_BYTES: usize = 4096;
pub const OBSERVATION_MAX_IDENTITY_BYTES: usize = 512;
pub const OBSERVATION_MAX_CONTRACT_BYTES: usize = 256;

const DOMAIN_SEPARATOR: u8 = 0x1f;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationPolicy {
    pub schema: String,
    pub ordering_version: String,
    pub bounds: ObservationBounds,
    pub required_non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationBounds {
    pub max_entry_bytes: usize,
    pub default_page_entries: usize,
    pub max_page_entries: usize,
    pub default_page_bytes: usize,
    pub max_page_bytes: usize,
    pub max_observation_bytes: usize,
    pub max_prior_bytes: usize,
    pub max_cursor_bytes: usize,
    pub max_context_selectors: usize,
    pub max_entries: usize,
    pub max_selector_bytes: usize,
}

static POLICY_BINDING: LazyLock<(ObservationPolicy, String)> = LazyLock::new(|| {
    let raw = include_str!("../../../../cairn-policy/generated/incremental-observations.json");
    let value: Value = serde_json::from_str(raw).expect("checked incremental observation policy");
    let policy: ObservationPolicy =
        serde_json::from_value(value).expect("typed incremental observation policy");
    assert_eq!(
        policy.schema, OBSERVATION_POLICY_SCHEMA,
        "generated incremental observation policy schema"
    );
    assert_eq!(
        policy.ordering_version, OBSERVATION_ORDERING_VERSION,
        "generated incremental observation ordering version"
    );
    let hash = blake3_hex(raw.as_bytes());
    (policy, hash)
});

pub fn policy() -> &'static ObservationPolicy {
    &POLICY_BINDING.0
}

pub fn policy_hash() -> &'static str {
    POLICY_BINDING.1.as_str()
}

pub fn bounds() -> &'static ObservationBounds {
    &POLICY_BINDING.0.bounds
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ObservationIssue {
    pub code: String,
    pub message: String,
}

impl ObservationIssue {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ObservationIssue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl Error for ObservationIssue {}

fn domain_hash_bytes(domain: &str, bytes: &[u8]) -> String {
    let mut material =
        Vec::with_capacity(domain.len().saturating_add(1).saturating_add(bytes.len()));
    material.extend_from_slice(domain.as_bytes());
    material.push(DOMAIN_SEPARATOR);
    material.extend_from_slice(bytes);
    blake3_hex(&material)
}

fn domain_hash_json<T: Serialize>(domain: &str, value: &T) -> Result<String, ObservationIssue> {
    let value = serde_json::to_value(value).map_err(|error| {
        ObservationIssue::new(
            "observation.serialization",
            format!("observation hash material could not be serialized: {error}"),
        )
    })?;
    Ok(domain_hash_bytes(domain, canonical_json(&value).as_bytes()))
}

fn require_hash(value: &str, field: &str) -> Result<(), ObservationIssue> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(ObservationIssue::new(
            "observation.identity",
            format!("{field} must be 64 lowercase hexadecimal characters"),
        ))
    }
}

fn safe_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= OBSERVATION_MAX_IDENTITY_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'/')
        })
        && !value.starts_with('/')
        && !value.ends_with('/')
        && !value.split('/').any(|part| part == ".." || part == ".")
}

fn validate_identity(identity: &str) -> Result<(), ObservationIssue> {
    if safe_identity(identity) {
        Ok(())
    } else {
        Err(ObservationIssue::new(
            "observation.identity",
            format!("unsafe or unbounded observation entry identity: {identity}"),
        ))
    }
}

fn view_allows_identity(view: &ObservationView, identity: &str) -> bool {
    match view {
        ObservationView::ActiveChanges => identity.starts_with("change:"),
        ObservationView::Readiness { .. } => {
            identity.starts_with("stage:")
                || identity.starts_with("blocker:")
                || identity.starts_with("action:")
        }
        ObservationView::Context { .. } => {
            identity.starts_with("context:requirement:")
                || identity.starts_with("context:task:")
                || identity.starts_with("context:section:")
        }
    }
}

fn safe_root(root: &str) -> bool {
    !root.is_empty()
        && root.len() <= OBSERVATION_MAX_ROOT_BYTES
        && !root.chars().any(char::is_control)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "view", rename_all = "snake_case", deny_unknown_fields)]
pub enum ObservationView {
    ActiveChanges,
    Readiness {
        change: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_bytes: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_entries: Option<usize>,
    },
    Context {
        scope: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        change: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spec: Option<String>,
        artifact: String,
        selectors: Vec<ContextSelector>,
    },
}

impl ObservationView {
    pub fn producer_contract(&self) -> &'static str {
        match self {
            Self::ActiveChanges => "cairn.active-change-listing.v1",
            Self::Readiness { .. } => "cairn.readiness-agent.v1",
            Self::Context { .. } => "cairn.targeted-context-read.v1",
        }
    }
}

fn safe_simple_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        && value != "."
        && value != ".."
        && !value.starts_with('.')
}

fn validate_selector(selector: &ContextSelector) -> Result<(), ObservationIssue> {
    match selector {
        ContextSelector::Requirement { id } | ContextSelector::Task { id } => {
            if safe_simple_id(id) {
                Ok(())
            } else {
                Err(ObservationIssue::new(
                    "observation.selector",
                    "context selector id is unsafe or unbounded",
                ))
            }
        }
        ContextSelector::Section { path } => {
            if path.is_empty()
                || path.iter().any(|part| {
                    part.is_empty() || part.len() > 512 || part.chars().any(char::is_control)
                })
            {
                return Err(ObservationIssue::new(
                    "observation.selector",
                    "context heading selector is empty, unsafe, or unbounded",
                ));
            }
            Ok(())
        }
    }
}

fn validate_view(view: &ObservationView) -> Result<(), ObservationIssue> {
    let bounds = bounds();
    match view {
        ObservationView::ActiveChanges => Ok(()),
        ObservationView::Readiness {
            change,
            max_bytes,
            max_entries,
        } => {
            crate::validate_change_name(change)
                .map_err(|error| ObservationIssue::new("observation.selector", error))?;
            if max_bytes.is_some_and(|value| value == 0) {
                return Err(ObservationIssue::new(
                    "observation.bounds",
                    "readiness view max_bytes must be positive",
                ));
            }
            if max_entries.is_some_and(|value| value > bounds.max_entries) {
                return Err(ObservationIssue::new(
                    "observation.bounds",
                    "readiness view max_entries exceeds the observation bound",
                ));
            }
            Ok(())
        }
        ObservationView::Context {
            scope,
            change,
            spec,
            artifact,
            selectors,
        } => {
            match scope.as_str() {
                "accepted" => {
                    if change.is_some() {
                        return Err(ObservationIssue::new(
                            "observation.selector",
                            "accepted context view must not declare a change",
                        ));
                    }
                    if !spec.as_deref().is_some_and(safe_simple_id) {
                        return Err(ObservationIssue::new(
                            "observation.selector",
                            "accepted context view requires a safe spec id",
                        ));
                    }
                }
                "active" | "archived" => {
                    if !change.as_deref().is_some_and(safe_simple_id) {
                        return Err(ObservationIssue::new(
                            "observation.selector",
                            "change context view requires a safe change id",
                        ));
                    }
                    if spec.as_deref().is_some_and(|spec| !safe_simple_id(spec)) {
                        return Err(ObservationIssue::new(
                            "observation.selector",
                            "unsafe context spec id",
                        ));
                    }
                }
                _ => {
                    return Err(ObservationIssue::new(
                        "observation.selector",
                        "unsupported context lifecycle scope",
                    ))
                }
            }
            if !safe_simple_id(artifact) {
                return Err(ObservationIssue::new(
                    "observation.selector",
                    "unsafe context artifact id",
                ));
            }
            if selectors.is_empty() || selectors.len() > bounds.max_context_selectors {
                return Err(ObservationIssue::new(
                    "observation.bounds",
                    "context view requires a bounded non-empty selector list",
                ));
            }
            let mut seen = std::collections::BTreeSet::new();
            for selector in selectors {
                validate_selector(selector)?;
                let encoded = canonical_json(&serde_json::to_value(selector).map_err(|error| {
                    ObservationIssue::new(
                        "observation.selector",
                        format!("context selector could not be serialized: {error}"),
                    )
                })?);
                if encoded.len() > bounds.max_selector_bytes {
                    return Err(ObservationIssue::new(
                        "observation.selector",
                        "context selector exceeds the selector byte bound",
                    ));
                }
                if !seen.insert(encoded) {
                    return Err(ObservationIssue::new(
                        "observation.selector",
                        "duplicate context selector in one observation view",
                    ));
                }
            }
            Ok(())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationEntry {
    pub identity: String,
    pub content_hash: String,
    pub content: Value,
}

impl ObservationEntry {
    pub fn new(identity: impl Into<String>, content: Value) -> Result<Self, ObservationIssue> {
        let identity = identity.into();
        validate_identity(&identity)?;
        let bytes = serde_json::to_vec(&content).map_err(|error| {
            ObservationIssue::new(
                "observation.serialization",
                format!("observation entry could not be serialized: {error}"),
            )
        })?;
        if bytes.len() > bounds().max_entry_bytes {
            return Err(ObservationIssue::new(
                "observation.entry_bounds",
                format!("observation entry {identity} exceeds the policy byte bound"),
            ));
        }
        let content_hash = domain_hash_json(OBSERVATION_ENTRY_DOMAIN, &content)?;
        Ok(Self {
            identity,
            content_hash,
            content,
        })
    }

    fn validate(&self) -> Result<(), ObservationIssue> {
        validate_identity(&self.identity)?;
        require_hash(&self.content_hash, "entry content hash")?;
        let bytes = serde_json::to_vec(&self.content).map_err(|error| {
            ObservationIssue::new(
                "observation.serialization",
                format!("observation entry could not be serialized: {error}"),
            )
        })?;
        if bytes.len() > bounds().max_entry_bytes {
            return Err(ObservationIssue::new(
                "observation.entry_bounds",
                format!(
                    "observation entry {} exceeds the policy byte bound",
                    self.identity
                ),
            ));
        }
        let expected = domain_hash_json(OBSERVATION_ENTRY_DOMAIN, &self.content)?;
        if expected != self.content_hash {
            return Err(ObservationIssue::new(
                "observation.hash_mismatch",
                format!(
                    "observation entry {} content hash does not match its bytes",
                    self.identity
                ),
            ));
        }
        Ok(())
    }
}

pub fn observation_entry(
    identity: impl Into<String>,
    content: Value,
) -> Result<ObservationEntry, ObservationIssue> {
    ObservationEntry::new(identity, content)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationCohort {
    pub root: String,
    pub store: Option<String>,
    pub source_manifest: String,
    pub policy_hash: String,
    pub producer_policy_hash: String,
    pub evidence_hash: String,
    pub producer_contract: String,
    pub view: ObservationView,
    pub ordering: String,
}

// r[impl cairn.incremental_observations.identity]
impl ObservationCohort {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        root: impl Into<String>,
        store: Option<String>,
        source_manifest: impl Into<String>,
        policy_hash: impl Into<String>,
        producer_policy_hash: impl Into<String>,
        evidence_hash: impl Into<String>,
        view: ObservationView,
    ) -> Result<Self, ObservationIssue> {
        let cohort = Self {
            root: root.into(),
            store,
            source_manifest: source_manifest.into(),
            policy_hash: policy_hash.into(),
            producer_policy_hash: producer_policy_hash.into(),
            evidence_hash: evidence_hash.into(),
            producer_contract: view.producer_contract().into(),
            view,
            ordering: policy().ordering_version.clone(),
        };
        validate_cohort(&cohort)?;
        Ok(cohort)
    }

    /// Continuation identity binds the complete cohort, including source and
    /// evidence identity. Cursors and unchanged comparisons use this value.
    pub fn continuation_hash(&self) -> Result<String, ObservationIssue> {
        domain_hash_json(OBSERVATION_COHORT_DOMAIN, self)
    }

    /// Difference admission binds the observation contract only. Source and
    /// evidence drift is exactly what a difference reports; policy, root,
    /// producer, view, selectors, and ordering drift remain errors.
    pub fn contract_hash(&self) -> Result<String, ObservationIssue> {
        domain_hash_json(
            OBSERVATION_CONTRACT_DOMAIN,
            &json!({
                "root": self.root,
                "store": self.store,
                "policy_hash": self.policy_hash,
                "producer_policy_hash": self.producer_policy_hash,
                "producer_contract": self.producer_contract,
                "view": self.view,
                "ordering": self.ordering,
            }),
        )
    }
}

fn validate_cohort(cohort: &ObservationCohort) -> Result<(), ObservationIssue> {
    if !safe_root(&cohort.root) {
        return Err(ObservationIssue::new(
            "observation.root",
            "observation root identity is empty, unsafe, or unbounded",
        ));
    }
    if cohort
        .store
        .as_deref()
        .is_some_and(|store| !safe_root(store))
    {
        return Err(ObservationIssue::new(
            "observation.root",
            "observation store identity is unsafe or unbounded",
        ));
    }
    for (field, value) in [
        ("source manifest", cohort.source_manifest.as_str()),
        ("policy hash", cohort.policy_hash.as_str()),
        ("producer policy hash", cohort.producer_policy_hash.as_str()),
        ("evidence hash", cohort.evidence_hash.as_str()),
    ] {
        require_hash(value, field)?;
    }
    if cohort.producer_contract.is_empty()
        || cohort.producer_contract.len() > OBSERVATION_MAX_CONTRACT_BYTES
        || !cohort
            .producer_contract
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(ObservationIssue::new(
            "observation.contract",
            "observation producer contract is empty, unsafe, or unbounded",
        ));
    }
    if cohort.producer_contract != cohort.view.producer_contract() {
        return Err(ObservationIssue::new(
            "observation.contract",
            "producer contract does not match the selected observation view",
        ));
    }
    if cohort.ordering != policy().ordering_version {
        return Err(ObservationIssue::new(
            "observation.ordering",
            "observation ordering version is unsupported; require a complete refresh",
        ));
    }
    validate_view(&cohort.view)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationSnapshot {
    pub schema: String,
    pub cohort: ObservationCohort,
    pub cohort_hash: String,
    pub contract_hash: String,
    pub entries: Vec<ObservationEntry>,
    pub total: usize,
    pub included: usize,
    pub omitted: usize,
    pub complete: bool,
    pub state: String,
    pub snapshot_hash: String,
    pub read_only: bool,
    pub mutated: bool,
    pub non_claims: Vec<String>,
}

fn snapshot_hash(snapshot: &ObservationSnapshot) -> Result<String, ObservationIssue> {
    let mut body = snapshot.clone();
    body.snapshot_hash.clear();
    domain_hash_json(OBSERVATION_SNAPSHOT_DOMAIN, &body)
}

fn observation_state(complete: bool) -> &'static str {
    if complete {
        OBSERVATION_STATE_COMPLETE
    } else {
        OBSERVATION_STATE_PARTIAL
    }
}

// r[impl cairn.incremental_observations.identity]
pub fn build_snapshot(
    cohort: ObservationCohort,
    mut entries: Vec<ObservationEntry>,
    omitted: usize,
    view_complete: bool,
) -> Result<ObservationSnapshot, ObservationIssue> {
    validate_cohort(&cohort)?;
    if entries.len() > bounds().max_entries {
        return Err(ObservationIssue::new(
            "observation.bounds",
            "observation contains more entries than the policy bound",
        ));
    }
    for entry in &entries {
        entry.validate()?;
        if !view_allows_identity(&cohort.view, &entry.identity) {
            return Err(ObservationIssue::new(
                "observation.schema",
                format!(
                    "observation entry {} is not valid for the selected view",
                    entry.identity
                ),
            ));
        }
    }
    entries.sort_by(|left, right| left.identity.as_bytes().cmp(right.identity.as_bytes()));
    for pair in entries.windows(2) {
        if pair[0].identity == pair[1].identity {
            return Err(ObservationIssue::new(
                "observation.duplicate_entry",
                format!("duplicate observation entry identity: {}", pair[0].identity),
            ));
        }
    }
    let included = entries.len();
    let total = included.checked_add(omitted).ok_or_else(|| {
        ObservationIssue::new("observation.bounds", "observation counts overflow")
    })?;
    if total > bounds().max_entries {
        return Err(ObservationIssue::new(
            "observation.bounds",
            "observation total exceeds the policy entry bound",
        ));
    }
    let complete = view_complete && omitted == 0;
    let mut snapshot = ObservationSnapshot {
        schema: OBSERVATION_SNAPSHOT_SCHEMA.into(),
        cohort_hash: cohort.continuation_hash()?,
        contract_hash: cohort.contract_hash()?,
        cohort,
        entries,
        total,
        included,
        omitted,
        complete,
        state: observation_state(complete).into(),
        snapshot_hash: String::new(),
        read_only: true,
        mutated: false,
        non_claims: policy().required_non_claims.clone(),
    };
    let serialized = serde_json::to_vec(&snapshot).map_err(|error| {
        ObservationIssue::new(
            "observation.serialization",
            format!("observation snapshot could not be serialized: {error}"),
        )
    })?;
    if serialized.len() > bounds().max_observation_bytes {
        return Err(ObservationIssue::new(
            "observation.bounds",
            "observation snapshot exceeds the policy byte bound",
        ));
    }
    snapshot.snapshot_hash = snapshot_hash(&snapshot)?;
    Ok(snapshot)
}

fn validate_snapshot(snapshot: &ObservationSnapshot) -> Result<(), ObservationIssue> {
    if snapshot.schema != OBSERVATION_SNAPSHOT_SCHEMA {
        return Err(ObservationIssue::new(
            "observation.schema",
            "unsupported prior observation schema; require a complete refresh",
        ));
    }
    if !snapshot.read_only || snapshot.mutated {
        return Err(ObservationIssue::new(
            "observation.authority",
            "observation snapshots are read-only and cannot claim mutation",
        ));
    }
    if snapshot.non_claims != policy().required_non_claims {
        return Err(ObservationIssue::new(
            "observation.schema",
            "observation non-claims do not match the admitted policy",
        ));
    }
    validate_cohort(&snapshot.cohort)?;
    if snapshot.included != snapshot.entries.len() {
        return Err(ObservationIssue::new(
            "observation.forged_counts",
            "observation included count does not match its entries",
        ));
    }
    let total = snapshot
        .included
        .checked_add(snapshot.omitted)
        .ok_or_else(|| {
            ObservationIssue::new("observation.forged_counts", "observation counts overflow")
        })?;
    if total != snapshot.total {
        return Err(ObservationIssue::new(
            "observation.forged_counts",
            "observation total, included, and omitted counts disagree",
        ));
    }
    if snapshot.total > bounds().max_entries {
        return Err(ObservationIssue::new(
            "observation.bounds",
            "observation total exceeds the policy entry bound",
        ));
    }
    if !matches!(
        snapshot.state.as_str(),
        OBSERVATION_STATE_COMPLETE | OBSERVATION_STATE_PARTIAL
    ) {
        return Err(ObservationIssue::new(
            "observation.schema",
            "observation completeness state is unsupported",
        ));
    }
    if snapshot.complete != (snapshot.state == OBSERVATION_STATE_COMPLETE) {
        return Err(ObservationIssue::new(
            "observation.forged_counts",
            "observation complete flag and state disagree",
        ));
    }
    if snapshot.complete && snapshot.omitted != 0 {
        return Err(ObservationIssue::new(
            "observation.forged_counts",
            "a complete observation cannot omit entries",
        ));
    }
    let mut previous: Option<&[u8]> = None;
    for entry in &snapshot.entries {
        entry.validate()?;
        if !view_allows_identity(&snapshot.cohort.view, &entry.identity) {
            return Err(ObservationIssue::new(
                "observation.schema",
                format!(
                    "observation entry {} is not valid for the promised view",
                    entry.identity
                ),
            ));
        }
        if let Some(previous) = previous {
            if previous >= entry.identity.as_bytes() {
                return Err(ObservationIssue::new(
                    "observation.ordering",
                    "observation entries are duplicated or out of deterministic order",
                ));
            }
        }
        previous = Some(entry.identity.as_bytes());
    }
    let expected_cohort = snapshot.cohort.continuation_hash()?;
    if snapshot.cohort_hash != expected_cohort {
        return Err(ObservationIssue::new(
            "observation.mixed_cohort",
            "observation cohort identity does not match its bound inputs",
        ));
    }
    let expected_contract = snapshot.cohort.contract_hash()?;
    if snapshot.contract_hash != expected_contract {
        return Err(ObservationIssue::new(
            "observation.contract_drift",
            "observation contract identity does not match its view and policy",
        ));
    }
    if snapshot.snapshot_hash != snapshot_hash(snapshot)? {
        return Err(ObservationIssue::new(
            "observation.hash_mismatch",
            "observation snapshot hash does not match its content",
        ));
    }
    Ok(())
}

pub fn admit_snapshot(value: &Value) -> Result<ObservationSnapshot, ObservationIssue> {
    let snapshot: ObservationSnapshot = serde_json::from_value(value.clone()).map_err(|error| {
        ObservationIssue::new(
            "observation.invalid",
            format!("prior observation is malformed or truncated: {error}"),
        )
    })?;
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut rendered = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        rendered.push(HEX[(byte >> 4) as usize] as char);
        rendered.push(HEX[(byte & 0x0f) as usize] as char);
    }
    rendered
}

fn hex_decode(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    let mut bytes = Vec::with_capacity(text.len() / 2);
    let raw = text.as_bytes();
    let mut index = 0;
    while index < raw.len() {
        let high = (raw[index] as char).to_digit(16)?;
        let low = (raw[index + 1] as char).to_digit(16)?;
        bytes.push(((high << 4) | low) as u8);
        index += 2;
    }
    Some(bytes)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationCursor {
    pub schema: String,
    pub cohort_hash: String,
    pub contract_hash: String,
    pub snapshot_hash: String,
    pub page_contract: String,
    pub last_identity: String,
    pub max_entries: usize,
    pub max_bytes: usize,
}

fn encode_cursor(cursor: &ObservationCursor) -> Result<String, ObservationIssue> {
    let bytes = serde_json::to_vec(cursor).map_err(|error| {
        ObservationIssue::new(
            "observation.serialization",
            format!("observation cursor could not be serialized: {error}"),
        )
    })?;
    let token = hex_encode(&bytes);
    if token.len() > bounds().max_cursor_bytes {
        return Err(ObservationIssue::new(
            "observation.bounds",
            "observation cursor exceeds the policy byte bound",
        ));
    }
    Ok(token)
}

fn decode_cursor(token: &str) -> Result<ObservationCursor, ObservationIssue> {
    if token.is_empty() || token.len() > bounds().max_cursor_bytes {
        return Err(ObservationIssue::new(
            "observation.invalid_cursor",
            "observation cursor is empty or exceeds the policy byte bound",
        ));
    }
    let bytes = hex_decode(token).ok_or_else(|| {
        ObservationIssue::new(
            "observation.invalid_cursor",
            "observation cursor is not valid hexadecimal continuation data",
        )
    })?;
    let cursor: ObservationCursor = serde_json::from_slice(&bytes).map_err(|error| {
        ObservationIssue::new(
            "observation.invalid_cursor",
            format!("observation cursor is malformed: {error}"),
        )
    })?;
    if cursor.schema != OBSERVATION_CURSOR_SCHEMA || cursor.page_contract != OBSERVATION_PAGE_SCHEMA
    {
        return Err(ObservationIssue::new(
            "observation.invalid_cursor",
            "observation cursor schema or page contract is unsupported",
        ));
    }
    require_hash(&cursor.cohort_hash, "cursor cohort hash")?;
    require_hash(&cursor.contract_hash, "cursor contract hash")?;
    require_hash(&cursor.snapshot_hash, "cursor snapshot hash")?;
    validate_identity(&cursor.last_identity)?;
    if cursor.max_entries == 0
        || cursor.max_entries > bounds().max_page_entries
        || cursor.max_bytes == 0
        || cursor.max_bytes > bounds().max_page_bytes
    {
        return Err(ObservationIssue::new(
            "observation.invalid_cursor",
            "observation cursor page limits are zero or exceed the policy bounds",
        ));
    }
    Ok(cursor)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationPageRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_entries: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationPage {
    pub schema: String,
    pub view: ObservationView,
    pub cohort_hash: String,
    pub contract_hash: String,
    pub snapshot_hash: String,
    pub page_contract: String,
    pub request_cursor: Option<String>,
    pub entries: Vec<ObservationEntry>,
    pub total: usize,
    pub included: usize,
    pub omitted: usize,
    pub remaining: usize,
    pub complete: bool,
    pub state: String,
    pub first_identity: Option<String>,
    pub last_identity: Option<String>,
    pub next_cursor: Option<String>,
    pub page_hash: String,
    pub read_only: bool,
    pub mutated: bool,
    pub non_claims: Vec<String>,
}

fn page_hash(page: &ObservationPage) -> Result<String, ObservationIssue> {
    let mut body = page.clone();
    body.page_hash.clear();
    domain_hash_json(OBSERVATION_PAGE_DOMAIN, &body)
}

// r[impl cairn.incremental_observations.pages]
pub fn page(
    snapshot: &ObservationSnapshot,
    request: ObservationPageRequest,
) -> Result<ObservationPage, ObservationIssue> {
    let (start, max_entries, max_bytes, request_cursor) = match request.cursor {
        Some(token) => {
            if request.max_entries.is_some() || request.max_bytes.is_some() {
                return Err(ObservationIssue::new(
                    "observation.page_request",
                    "a continuation cursor owns the page entry and byte limits",
                ));
            }
            let cursor = decode_cursor(&token)?;
            if cursor.contract_hash != snapshot.contract_hash {
                return Err(ObservationIssue::new(
                    "observation.contract_drift",
                    "continuation cursor belongs to a different observation contract",
                ));
            }
            if cursor.snapshot_hash != snapshot.snapshot_hash
                || cursor.cohort_hash != snapshot.cohort_hash
            {
                return Err(ObservationIssue::new(
                    "observation.stale_cursor",
                    "observation source, policy, evidence, or cohort changed; request a complete refresh",
                ));
            }
            let index = snapshot
                .entries
                .binary_search_by(|entry| {
                    entry
                        .identity
                        .as_bytes()
                        .cmp(cursor.last_identity.as_bytes())
                })
                .map_err(|_| {
                    ObservationIssue::new(
                        "observation.invalid_cursor",
                        "observation cursor last identity is not in the current cohort",
                    )
                })?;
            (index + 1, cursor.max_entries, cursor.max_bytes, Some(token))
        }
        None => (
            0,
            request.max_entries.unwrap_or(bounds().default_page_entries),
            request.max_bytes.unwrap_or(bounds().default_page_bytes),
            None,
        ),
    };
    if max_entries == 0 || max_entries > bounds().max_page_entries {
        return Err(ObservationIssue::new(
            "observation.bounds",
            "observation page entry limit is zero or exceeds the policy bound",
        ));
    }
    if max_bytes == 0 || max_bytes > bounds().max_page_bytes {
        return Err(ObservationIssue::new(
            "observation.bounds",
            "observation page byte limit is zero or exceeds the policy bound",
        ));
    }
    let mut entries = Vec::new();
    // The byte budget covers the serialized `entries` array, including its
    // brackets, so consumers can apply it to the emitted page directly.
    let mut used = 2usize;
    for entry in &snapshot.entries[start..] {
        if entries.len() >= max_entries {
            break;
        }
        let entry_bytes = serde_json::to_vec(entry).map_err(|error| {
            ObservationIssue::new(
                "observation.serialization",
                format!("observation page entry could not be serialized: {error}"),
            )
        })?;
        let separator = if entries.is_empty() { 0 } else { 1 };
        let candidate = used
            .checked_add(entry_bytes.len())
            .and_then(|value| value.checked_add(separator))
            .ok_or_else(|| {
                ObservationIssue::new("observation.bounds", "observation page byte count overflow")
            })?;
        if candidate > max_bytes {
            break;
        }
        used = candidate;
        entries.push(entry.clone());
    }
    if entries.is_empty() && start < snapshot.entries.len() {
        return Err(ObservationIssue::new(
            "observation.page_budget",
            "observation page byte limit cannot hold the next whole entry",
        ));
    }
    let next_index = start + entries.len();
    let next_cursor = if next_index < snapshot.entries.len() {
        let cursor = ObservationCursor {
            schema: OBSERVATION_CURSOR_SCHEMA.into(),
            cohort_hash: snapshot.cohort_hash.clone(),
            contract_hash: snapshot.contract_hash.clone(),
            snapshot_hash: snapshot.snapshot_hash.clone(),
            page_contract: OBSERVATION_PAGE_SCHEMA.into(),
            last_identity: entries
                .last()
                .map(|entry| entry.identity.clone())
                .expect("non-empty page has a last identity"),
            max_entries,
            max_bytes,
        };
        Some(encode_cursor(&cursor)?)
    } else {
        None
    };
    let included = entries.len();
    let complete = snapshot.complete && next_index == snapshot.entries.len();
    let mut page = ObservationPage {
        schema: OBSERVATION_PAGE_SCHEMA.into(),
        view: snapshot.cohort.view.clone(),
        cohort_hash: snapshot.cohort_hash.clone(),
        contract_hash: snapshot.contract_hash.clone(),
        snapshot_hash: snapshot.snapshot_hash.clone(),
        page_contract: OBSERVATION_PAGE_SCHEMA.into(),
        request_cursor,
        first_identity: entries.first().map(|entry| entry.identity.clone()),
        last_identity: entries.last().map(|entry| entry.identity.clone()),
        entries,
        total: snapshot.total,
        included,
        omitted: snapshot.total.saturating_sub(included),
        remaining: snapshot.entries.len() - next_index,
        complete,
        state: observation_state(complete).into(),
        next_cursor,
        page_hash: String::new(),
        read_only: true,
        mutated: false,
        non_claims: policy().required_non_claims.clone(),
    };
    page.page_hash = page_hash(&page)?;
    Ok(page)
}

fn ensure_complete<'a>(
    snapshot: &'a ObservationSnapshot,
    role: &str,
) -> Result<&'a ObservationSnapshot, ObservationIssue> {
    if !snapshot.complete || snapshot.omitted != 0 {
        return Err(ObservationIssue::new(
            "observation.partial_baseline",
            format!(
                "{role} observation is partial; a complete refresh is required before comparison"
            ),
        ));
    }
    Ok(snapshot)
}

fn ensure_same_contract(
    current: &ObservationSnapshot,
    prior: &ObservationSnapshot,
) -> Result<(), ObservationIssue> {
    if current.contract_hash != prior.contract_hash {
        return Err(ObservationIssue::new(
            "observation.mixed_cohort",
            "prior observation has a different root, policy, view, selector set, or producer contract",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationUnchanged {
    pub schema: String,
    pub view: ObservationView,
    pub cohort_hash: String,
    pub contract_hash: String,
    pub current_hash: String,
    pub prior_hash: String,
    pub unchanged: bool,
    pub complete: bool,
    pub total: usize,
    pub included: usize,
    pub omitted: usize,
    pub comparison_hash: String,
    pub read_only: bool,
    pub mutated: bool,
    pub non_claims: Vec<String>,
}

fn comparison_hash(value: &ObservationUnchanged) -> Result<String, ObservationIssue> {
    let mut body = value.clone();
    body.comparison_hash.clear();
    domain_hash_json(OBSERVATION_UNCHANGED_DOMAIN, &body)
}

// r[impl cairn.incremental_observations.delta]
pub fn unchanged(
    current: &ObservationSnapshot,
    prior: &ObservationSnapshot,
) -> Result<ObservationUnchanged, ObservationIssue> {
    ensure_complete(current, "current")?;
    ensure_complete(prior, "prior")?;
    ensure_same_contract(current, prior)?;
    let unchanged = current.cohort_hash == prior.cohort_hash && current.entries == prior.entries;
    let mut result = ObservationUnchanged {
        schema: OBSERVATION_UNCHANGED_SCHEMA.into(),
        view: current.cohort.view.clone(),
        cohort_hash: current.cohort_hash.clone(),
        contract_hash: current.contract_hash.clone(),
        current_hash: current.snapshot_hash.clone(),
        prior_hash: prior.snapshot_hash.clone(),
        unchanged,
        complete: true,
        total: current.total,
        included: current.included,
        omitted: current.omitted,
        comparison_hash: String::new(),
        read_only: true,
        mutated: false,
        non_claims: policy().required_non_claims.clone(),
    };
    result.comparison_hash = comparison_hash(&result)?;
    Ok(result)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationChange {
    pub identity: String,
    pub prior_content_hash: String,
    pub content_hash: String,
    pub content: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationRemoval {
    pub identity: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationDifference {
    pub schema: String,
    pub view: ObservationView,
    pub cohort_hash: String,
    pub contract_hash: String,
    pub current_hash: String,
    pub prior_hash: String,
    pub added: Vec<ObservationEntry>,
    pub changed: Vec<ObservationChange>,
    pub removed: Vec<ObservationRemoval>,
    pub added_count: usize,
    pub changed_count: usize,
    pub removed_count: usize,
    pub unchanged: bool,
    pub complete: bool,
    pub difference_hash: String,
    pub read_only: bool,
    pub mutated: bool,
    pub non_claims: Vec<String>,
}

fn difference_hash(value: &ObservationDifference) -> Result<String, ObservationIssue> {
    let mut body = value.clone();
    body.difference_hash.clear();
    domain_hash_json(OBSERVATION_DIFFERENCE_DOMAIN, &body)
}

// r[impl cairn.incremental_observations.delta]
pub fn difference(
    current: &ObservationSnapshot,
    prior: &ObservationSnapshot,
) -> Result<ObservationDifference, ObservationIssue> {
    ensure_complete(current, "current")?;
    ensure_complete(prior, "prior")?;
    ensure_same_contract(current, prior)?;
    let prior_entries = prior
        .entries
        .iter()
        .map(|entry| (entry.identity.as_str(), entry))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut added = Vec::new();
    let mut changed = Vec::new();
    for entry in &current.entries {
        match prior_entries.get(entry.identity.as_str()) {
            None => added.push(entry.clone()),
            Some(prior_entry) if prior_entry.content_hash != entry.content_hash => {
                changed.push(ObservationChange {
                    identity: entry.identity.clone(),
                    prior_content_hash: prior_entry.content_hash.clone(),
                    content_hash: entry.content_hash.clone(),
                    content: entry.content.clone(),
                });
            }
            Some(_) => {}
        }
    }
    let current_identities = current
        .entries
        .iter()
        .map(|entry| entry.identity.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let removed = prior
        .entries
        .iter()
        .filter(|entry| !current_identities.contains(entry.identity.as_str()))
        .map(|entry| ObservationRemoval {
            identity: entry.identity.clone(),
            content_hash: entry.content_hash.clone(),
        })
        .collect::<Vec<_>>();
    let unchanged = added.is_empty()
        && changed.is_empty()
        && removed.is_empty()
        && current.cohort_hash == prior.cohort_hash;
    let mut result = ObservationDifference {
        schema: OBSERVATION_DIFFERENCE_SCHEMA.into(),
        view: current.cohort.view.clone(),
        cohort_hash: current.cohort_hash.clone(),
        contract_hash: current.contract_hash.clone(),
        current_hash: current.snapshot_hash.clone(),
        prior_hash: prior.snapshot_hash.clone(),
        added_count: added.len(),
        changed_count: changed.len(),
        removed_count: removed.len(),
        added,
        changed,
        removed,
        unchanged,
        complete: true,
        difference_hash: String::new(),
        read_only: true,
        mutated: false,
        non_claims: policy().required_non_claims.clone(),
    };
    result.difference_hash = difference_hash(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests;
