use serde::{Deserialize, Serialize};

pub const REQUEST_SCHEMA: &str = "mantle.build-request.v1";
pub const OBSERVATION_SCHEMA: &str = "mantle.build-observation.v1";
pub const RECEIPT_SCHEMA: &str = "mantle.build-receipt.v1";
pub const NATIVE_REPORT_SCHEMA: &str = "crunch-build-report-v1";
pub const MAXIMUM_LABEL_BYTES: usize = 128;
pub const MAXIMUM_REQUIRED_PRODUCTS: usize = 256;
pub const MAXIMUM_PRODUCTS: usize = 256;
pub const MAXIMUM_LOGS: usize = 256;
pub const MAXIMUM_METRICS: usize = 256;
const BLAKE3_HEX_LENGTH: usize = 64;
const REQUEST_DOMAIN: &str = "mantle.build-request.identity.v1";
const OBSERVATION_DOMAIN: &str = "mantle.build-observation.identity.v1";

pub const REQUIRED_NON_CLAIMS: [&str; 6] = [
    "build observation does not prove build correctness",
    "build observation does not prove sandbox completeness",
    "cache observation does not prove cache truth",
    "product admission does not prove product semantics",
    "build observation does not prove reproducibility",
    "build observation does not establish release readiness",
];

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Identity(String);

impl Identity {
    pub fn new(value: String) -> Result<Self, ContractError> {
        let digest = value.strip_prefix("b3:");
        if value.len() == BLAKE3_HEX_LENGTH + "b3:".len()
            && digest.is_some_and(|digest| {
                digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
        {
            Ok(Self(value))
        } else {
            Err(ContractError::InvalidIdentity)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn from_hash(value: blake3::Hash) -> Self {
        Self(format!("b3:{}", value.to_hex()))
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Label(String);

impl Label {
    pub fn new(value: String) -> Result<Self, ContractError> {
        if !value.is_empty()
            && value.len() <= MAXIMUM_LABEL_BYTES
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b':' | b'-')
            })
            && value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        {
            Ok(Self(value))
        } else {
            Err(ContractError::InvalidLabel)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildOutcome {
    Success,
    Failed,
    Cancelled,
    TimedOut,
    Unknown,
}

impl BuildOutcome {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheKind {
    None,
    Local,
    Substitution,
}

impl CacheKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Local => "local",
            Self::Substitution => "substitution",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildRequest {
    pub schema: String,
    pub identity: Identity,
    pub idempotency_key: Identity,
    pub effect_identity: Identity,
    pub attempt_identity: Identity,
    pub candidate_identity: Identity,
    pub pipeline_revision_identity: Identity,
    pub plan_identity: Identity,
    pub policy_identity: Identity,
    pub platform: Label,
    pub required_products: Vec<Label>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductObservation {
    pub name: Label,
    pub artifact_identity: Identity,
    pub store_identity: Identity,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricObservation {
    pub name: Label,
    pub value: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheObservation {
    pub kind: CacheKind,
    pub source_identity: Option<Identity>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildObservation {
    pub schema: String,
    pub identity: Identity,
    pub request_identity: Identity,
    pub outcome: BuildOutcome,
    pub products: Vec<ProductObservation>,
    pub builder_identity: Identity,
    pub worker_identity: Identity,
    pub store_identity: Identity,
    pub cache: CacheObservation,
    pub logs: Vec<Identity>,
    pub metrics: Vec<MetricObservation>,
    pub receipt_identity: Option<Identity>,
    pub non_claims: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractError {
    InvalidIdentity,
    InvalidLabel,
    UnsupportedSchema,
    IdentityMismatch,
    RequiredProductsNotCanonical,
    CollectionLimitExceeded,
    RequestMismatch,
    BuilderMismatch,
    ProductMismatch,
    OutcomeContradiction,
    CacheContradiction,
    NonClaimsMismatch,
}

pub fn validate_request(request: &BuildRequest) -> Result<(), ContractError> {
    if request.schema != REQUEST_SCHEMA {
        return Err(ContractError::UnsupportedSchema);
    }
    if request.required_products.len() > MAXIMUM_REQUIRED_PRODUCTS
        || !strictly_sorted(request.required_products.iter().map(Label::as_str))
    {
        return Err(ContractError::RequiredProductsNotCanonical);
    }
    if request.identity != request_identity(request) {
        return Err(ContractError::IdentityMismatch);
    }
    Ok(())
}

pub fn validate_observation(
    request: &BuildRequest,
    expected_builder: &Identity,
    observation: &BuildObservation,
) -> Result<(), ContractError> {
    validate_request(request)?;
    if observation.schema != OBSERVATION_SCHEMA {
        return Err(ContractError::UnsupportedSchema);
    }
    if observation.request_identity != request.identity {
        return Err(ContractError::RequestMismatch);
    }
    if &observation.builder_identity != expected_builder {
        return Err(ContractError::BuilderMismatch);
    }
    if observation.products.len() > MAXIMUM_PRODUCTS
        || observation.logs.len() > MAXIMUM_LOGS
        || observation.metrics.len() > MAXIMUM_METRICS
    {
        return Err(ContractError::CollectionLimitExceeded);
    }
    if !strictly_sorted(
        observation
            .products
            .iter()
            .map(|product| product.name.as_str()),
    ) {
        return Err(ContractError::ProductMismatch);
    }
    if observation.outcome == BuildOutcome::Success
        && (request.required_products.len() != observation.products.len()
            || request
                .required_products
                .iter()
                .zip(&observation.products)
                .any(|(required, observed)| required != &observed.name))
    {
        return Err(ContractError::ProductMismatch);
    }
    match observation.outcome {
        BuildOutcome::Success
        | BuildOutcome::Failed
        | BuildOutcome::Cancelled
        | BuildOutcome::TimedOut
            if observation.receipt_identity.is_some() => {}
        BuildOutcome::Unknown if observation.receipt_identity.is_none() => {}
        _ => return Err(ContractError::OutcomeContradiction),
    }
    match observation.cache.kind {
        CacheKind::None if observation.cache.source_identity.is_none() => {}
        CacheKind::Local | CacheKind::Substitution
            if observation.cache.source_identity.is_some() => {}
        _ => return Err(ContractError::CacheContradiction),
    }
    if observation.non_claims
        != REQUIRED_NON_CLAIMS
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>()
    {
        return Err(ContractError::NonClaimsMismatch);
    }
    if observation.identity != observation_identity(observation) {
        return Err(ContractError::IdentityMismatch);
    }
    Ok(())
}

#[must_use]
pub fn request_identity(request: &BuildRequest) -> Identity {
    let mut frame = Frame::new(REQUEST_DOMAIN);
    frame.text("schema", &request.schema);
    frame.text("idempotency", request.idempotency_key.as_str());
    frame.text("effect", request.effect_identity.as_str());
    frame.text("attempt", request.attempt_identity.as_str());
    frame.text("candidate", request.candidate_identity.as_str());
    frame.text("pipeline", request.pipeline_revision_identity.as_str());
    frame.text("plan", request.plan_identity.as_str());
    frame.text("policy", request.policy_identity.as_str());
    frame.text("platform", request.platform.as_str());
    frame.count("required_product_count", request.required_products.len());
    for product in &request.required_products {
        frame.text("required_product", product.as_str());
    }
    frame.finish()
}

#[must_use]
pub fn observation_identity(observation: &BuildObservation) -> Identity {
    let mut frame = Frame::new(OBSERVATION_DOMAIN);
    frame.text("schema", &observation.schema);
    frame.text("request", observation.request_identity.as_str());
    frame.text("outcome", observation.outcome.as_str());
    frame.count("product_count", observation.products.len());
    for product in &observation.products {
        frame.text("product_name", product.name.as_str());
        frame.text("product_artifact", product.artifact_identity.as_str());
        frame.text("product_store", product.store_identity.as_str());
    }
    frame.text("builder", observation.builder_identity.as_str());
    frame.text("worker", observation.worker_identity.as_str());
    frame.text("store", observation.store_identity.as_str());
    frame.text("cache_kind", observation.cache.kind.as_str());
    if let Some(source) = &observation.cache.source_identity {
        frame.text("cache_source", source.as_str());
    }
    frame.count("log_count", observation.logs.len());
    for log in &observation.logs {
        frame.text("log", log.as_str());
    }
    frame.count("metric_count", observation.metrics.len());
    for metric in &observation.metrics {
        frame.text("metric_name", metric.name.as_str());
        frame.bytes("metric_value", &metric.value.to_le_bytes());
    }
    if let Some(receipt) = &observation.receipt_identity {
        frame.text("receipt", receipt.as_str());
    }
    frame.count("non_claim_count", observation.non_claims.len());
    for non_claim in &observation.non_claims {
        frame.text("non_claim", non_claim);
    }
    frame.finish()
}

fn strictly_sorted<'a>(values: impl IntoIterator<Item = &'a str>) -> bool {
    let mut previous = None;
    for value in values {
        if previous.is_some_and(|previous| previous >= value) {
            return false;
        }
        previous = Some(value);
    }
    true
}

struct Frame(blake3::Hasher);

impl Frame {
    fn new(domain: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain.as_bytes());
        Self(hasher)
    }

    fn bytes(&mut self, label: &str, value: &[u8]) {
        self.part(label.as_bytes());
        self.part(value);
    }

    fn text(&mut self, label: &str, value: &str) {
        self.bytes(label, value.as_bytes());
    }

    fn count(&mut self, label: &str, count: usize) {
        match u64::try_from(count) {
            Ok(count) => self.bytes(label, &count.to_le_bytes()),
            Err(_) => self.text(label, "unrepresentable-count"),
        }
    }

    fn part(&mut self, value: &[u8]) {
        match u64::try_from(value.len()) {
            Ok(length) => {
                self.0.update(&length.to_le_bytes());
                self.0.update(value);
            }
            Err(_) => {
                self.0.update(b"unrepresentable-length");
            }
        }
    }

    fn finish(self) -> Identity {
        Identity::from_hash(self.0.finalize())
    }
}
