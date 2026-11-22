//! Source-review attribution over exact Artifact Auth signed bytes.
//!
//! This boundary owns no private keys, trust discovery, clocks, build witness or
//! release-signature authority. Policies and detached signatures are explicit inputs.
// r[impl cairn.source_review.contract]
// r[impl cairn.source_review.authentication]
// r[impl cairn.source_review.boundary]
use std::collections::BTreeSet;

use artifact_auth_core::{
    canonical_statement_bytes, policy_identity, statement_identity, ArtifactRef, ArtifactStatement,
    AuthenticationPolicy, AuthenticationScope, STATEMENT_SCHEMA_V1,
};
use artifact_auth_ed25519::{public_key_identity, verify_and_evaluate, Ed25519Evidence};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

pub const SUBJECT_SCHEMA: &str = "cairn.source-review-subject.v1";
pub const CLAIM_SCHEMA: &str = "cairn.source-review-claim.v1";
pub const ATTACHMENT_SCHEMA: &str = "cairn.source-review-attachment.v1";
pub const POLICY_SCHEMA: &str = "cairn.source-review-verification-policy.v1";
pub const PROFILE: &str = "cairn.source-review.v1";
pub const PURPOSE: &str = "source-review-approval";
pub const MAX_ROWS: usize = 64;
pub const MAX_INPUT_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("source-review: {0}")]
pub struct SourceReviewError(pub String);
pub type Result<T, E = SourceReviewError> = std::result::Result<T, E>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceReviewSubject {
    pub schema: String,
    pub source_archive_blake3: String,
    pub source_revision: Option<String>,
    pub source_tree_blake3: Option<String>,
    pub snapshot_profile: String,
    pub snapshot_profile_version: u64,
    pub projection: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimRoot {
    pub claim_identity: String,
    pub subject_root: String,
    pub revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceReviewDisposition {
    Approved,
    NeedsRevision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceReviewClaim {
    pub schema: String,
    pub subject: SourceReviewSubject,
    pub claim_root: ClaimRoot,
    pub review_policy_blake3: String,
    pub disposition: SourceReviewDisposition,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedSourceReview {
    pub claim: SourceReviewClaim,
    pub statement: ArtifactStatement,
    pub generation: u64,
    pub public_key_hex: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceReviewAttachment {
    pub schema: String,
    pub reviews: Vec<SignedSourceReview>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceReviewVerificationPolicy {
    pub schema: String,
    pub subject: SourceReviewSubject,
    pub claim_root: ClaimRoot,
    pub review_policy_blake3: String,
    pub authentication_policy: AuthenticationPolicy,
    pub verifier_context: ArtifactRef,
    pub author_key_blake3: Vec<String>,
    pub exclude_authors: bool,
    pub required_approvals: u16,
    pub non_claims: Vec<String>,
}

/// The hex field is the exact unsigned Artifact Auth canonical byte sequence,
/// not JSON bytes or a digest. An external signer must decode it before signing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceReviewSigningRequest {
    pub schema: String,
    pub claim: SourceReviewClaim,
    pub statement: ArtifactStatement,
    pub generation: u64,
    pub public_key_hex: String,
    pub canonical_statement_hex: String,
    pub statement_blake3: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceReviewOutcome {
    AcceptedManualReview,
    Insufficient,
}

/// Derived locally from actual signature verification; never input authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceReviewReport {
    pub schema: &'static str,
    pub outcome: SourceReviewOutcome,
    pub verification_role: &'static str,
    pub attachment_identity: String,
    pub approved_key_blake3: Vec<String>,
    pub needs_revision_key_blake3: Vec<String>,
    pub canonical_statement_blake3: Vec<String>,
    pub non_claims: Vec<String>,
}

pub fn required_non_claims() -> Vec<String> {
    let mut values = [
        "source-review-does-not-prove-source-correctness",
        "source-review-does-not-prove-review-quality",
        "source-review-does-not-prove-build-correctness",
        "source-review-does-not-prove-reproducibility",
        "source-review-does-not-prove-witness-independence",
        "source-review-does-not-prove-deployment-safety",
        "source-review-does-not-prove-release-eligibility",
    ]
    .map(str::to_owned)
    .to_vec();
    values.sort();
    values
}

fn require(condition: bool, code: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(SourceReviewError(code.to_owned()))
    }
}
fn digest(value: &str) -> Result<()> {
    require(
        value.len() == 64
            && value
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
        "digest_malformed",
    )
}
fn non_claims(values: &[String]) -> Result<()> {
    require(values == required_non_claims(), "non_claims_mismatch")
}
fn root(value: &ClaimRoot) -> Result<()> {
    digest(&value.claim_identity)?;
    digest(&value.subject_root)
}
fn reference(profile: &str, digest_hex: &str) -> ArtifactRef {
    ArtifactRef {
        profile: profile.to_owned(),
        algorithm: "blake3".to_owned(),
        digest_hex: digest_hex.to_owned(),
    }
}

impl SourceReviewSubject {
    pub fn new(
        source_archive_blake3: String,
        source_revision: Option<String>,
        source_tree_blake3: Option<String>,
    ) -> Result<Self> {
        let subject = Self {
            schema: SUBJECT_SCHEMA.to_owned(),
            source_archive_blake3,
            source_revision,
            source_tree_blake3,
            snapshot_profile: "mantle-release-source".to_owned(),
            snapshot_profile_version: 1,
            projection: "source".to_owned(),
        };
        subject.validate()?;
        Ok(subject)
    }
    pub fn validate(&self) -> Result<()> {
        require(self.schema == SUBJECT_SCHEMA, "subject_schema_unsupported")?;
        digest(&self.source_archive_blake3)?;
        if let Some(value) = &self.source_tree_blake3 {
            digest(value)?;
        }
        if let Some(value) = &self.source_revision {
            require(
                !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control),
                "source_revision_malformed",
            )?;
        }
        require(
            self.snapshot_profile == "mantle-release-source"
                && self.snapshot_profile_version == 1
                && self.projection == "source",
            "source_projection_unsupported",
        )
    }
    pub fn identity(&self) -> Result<String> {
        self.validate()?;
        logical_identity(SUBJECT_SCHEMA, self)
    }
}

impl SourceReviewClaim {
    pub fn new(
        subject: SourceReviewSubject,
        claim_root: ClaimRoot,
        review_policy_blake3: String,
        disposition: SourceReviewDisposition,
    ) -> Result<Self> {
        let claim = Self {
            schema: CLAIM_SCHEMA.to_owned(),
            subject,
            claim_root,
            review_policy_blake3,
            disposition,
            non_claims: required_non_claims(),
        };
        claim.validate()?;
        Ok(claim)
    }
    pub fn validate(&self) -> Result<()> {
        require(self.schema == CLAIM_SCHEMA, "claim_schema_unsupported")?;
        self.subject.validate()?;
        root(&self.claim_root)?;
        digest(&self.review_policy_blake3)?;
        non_claims(&self.non_claims)
    }
    pub fn identity(&self) -> Result<String> {
        self.validate()?;
        logical_identity(CLAIM_SCHEMA, self)
    }
    pub fn scope(&self, verifier_context: ArtifactRef) -> Result<AuthenticationScope> {
        let identity = self.identity()?;
        let mut parents = vec![
            reference(
                "mantle-release-source.v1",
                &self.subject.source_archive_blake3,
            ),
            reference("cairn.source-review-policy.v1", &self.review_policy_blake3),
        ];
        parents.sort();
        Ok(AuthenticationScope {
            domain: PROFILE.to_owned(),
            purpose: PURPOSE.to_owned(),
            profile_id: PROFILE.to_owned(),
            subject: reference(CLAIM_SCHEMA, &identity),
            parents,
            verifier_context,
        })
    }
}

impl SourceReviewVerificationPolicy {
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema == POLICY_SCHEMA,
            "verification_policy_schema_unsupported",
        )?;
        self.subject.validate()?;
        root(&self.claim_root)?;
        digest(&self.review_policy_blake3)?;
        non_claims(&self.non_claims)?;
        require(
            self.required_approvals > 0 && usize::from(self.required_approvals) <= MAX_ROWS,
            "approval_threshold_invalid",
        )?;
        require(
            self.authentication_policy.threshold == self.required_approvals
                && self.authentication_policy.profile_id == PROFILE,
            "authentication_policy_mismatch",
        )?;
        require(
            !self.authentication_policy.trusted_keys.is_empty()
                && self.authentication_policy.trusted_keys.len() <= MAX_ROWS,
            "trusted_key_count_exceeded",
        )?;
        require(
            self.author_key_blake3.len() <= MAX_ROWS,
            "author_count_exceeded",
        )?;
        let mut authors = BTreeSet::new();
        for author in &self.author_key_blake3 {
            digest(author)?;
            require(authors.insert(author), "duplicate_author_key")?;
        }
        for key in &self.authentication_policy.trusted_keys {
            require(
                key.currentness_ref == self.verifier_context,
                "currentness_context_mismatch",
            )?;
            require(
                key.allowed_purposes == [PURPOSE],
                "key_role_not_source_review_only",
            )?;
        }
        policy_identity(&self.authentication_policy)
            .map_err(|_| SourceReviewError("authentication_policy_invalid".to_owned()))?;
        // Every trusted currentness_ref equals verifier_context above; the
        // nonempty Artifact Auth policy validates that reference independently.
        Ok(())
    }
}

pub fn signing_request(
    claim: SourceReviewClaim,
    policy: &SourceReviewVerificationPolicy,
    producer_id: String,
    key_id: String,
    generation: u64,
    public_key_hex: String,
) -> Result<SourceReviewSigningRequest> {
    policy.validate()?;
    validate_links(&claim, policy)?;
    let key = decode_hex::<32>(&public_key_hex)?;
    let statement = ArtifactStatement {
        schema: STATEMENT_SCHEMA_V1.to_owned(),
        scope: claim.scope(policy.verifier_context.clone())?,
        producer_id,
        key_id,
        key_identity: public_key_identity(&key),
    };
    validate_signer(&statement, generation, policy)?;
    let bytes = canonical_statement_bytes(&statement)
        .map_err(|_| SourceReviewError("statement_invalid".to_owned()))?;
    Ok(SourceReviewSigningRequest {
        schema: "cairn.source-review-signing-request.v1".to_owned(),
        claim,
        statement,
        generation,
        public_key_hex,
        canonical_statement_hex: encode_hex(&bytes),
        statement_blake3: blake3::hash(&bytes).to_hex().to_string(),
    })
}

/// Reconstruct unsigned bytes and reject edited request carriers. This does not
/// establish trust or approval; assemble/verify additionally require policy.
pub fn signing_request_bytes(request: &SourceReviewSigningRequest) -> Result<Vec<u8>> {
    require(
        request.schema == "cairn.source-review-signing-request.v1",
        "signing_request_schema_unsupported",
    )?;
    require(
        request.statement.scope
            == request
                .claim
                .scope(request.statement.scope.verifier_context.clone())?,
        "statement_scope_mismatch",
    )?;
    require(request.generation > 0, "key_generation_stale")?;
    require(
        request.statement.key_identity
            == public_key_identity(&decode_hex::<32>(&request.public_key_hex)?),
        "public_key_identity_mismatch",
    )?;
    let bytes = canonical_statement_bytes(&request.statement)
        .map_err(|_| SourceReviewError("statement_invalid".to_owned()))?;
    require(
        request.canonical_statement_hex == encode_hex(&bytes)
            && request.statement_blake3 == blake3::hash(&bytes).to_hex().as_str(),
        "signing_request_tampered",
    )?;
    Ok(bytes)
}

/// Assemble only a genuine detached signature; rederive and compare every request
/// field, then cryptographically verify. A needs_revision signature is attribution.
pub fn assemble(
    request: SourceReviewSigningRequest,
    signature_hex: String,
    policy: &SourceReviewVerificationPolicy,
) -> Result<SignedSourceReview> {
    require(
        request.schema == "cairn.source-review-signing-request.v1",
        "signing_request_schema_unsupported",
    )?;
    let expected = signing_request(
        request.claim.clone(),
        policy,
        request.statement.producer_id.clone(),
        request.statement.key_id.clone(),
        request.generation,
        request.public_key_hex.clone(),
    )?;
    require(request == expected, "signing_request_tampered")?;
    let review = SignedSourceReview {
        claim: request.claim,
        statement: request.statement,
        generation: request.generation,
        public_key_hex: request.public_key_hex,
        signature_hex,
    };
    verify_row(&review, policy)?;
    Ok(review)
}

fn validate_links(
    claim: &SourceReviewClaim,
    policy: &SourceReviewVerificationPolicy,
) -> Result<()> {
    claim.validate()?;
    require(claim.subject == policy.subject, "source_subject_mismatch")?;
    require(claim.claim_root == policy.claim_root, "claim_root_mismatch")?;
    require(
        claim.review_policy_blake3 == policy.review_policy_blake3,
        "review_policy_mismatch",
    )
}
fn validate_signer(
    statement: &ArtifactStatement,
    generation: u64,
    policy: &SourceReviewVerificationPolicy,
) -> Result<()> {
    let key = policy
        .authentication_policy
        .trusted_keys
        .iter()
        .find(|key| {
            key.key_identity == statement.key_identity
                && key.producer_id == statement.producer_id
                && key.key_id == statement.key_id
        })
        .ok_or_else(|| SourceReviewError("key_untrusted".to_owned()))?;
    require(
        generation > 0 && generation == key.generation,
        "key_generation_stale",
    )?;
    require(
        matches!(
            key.currentness,
            artifact_auth_core::KeyCurrentness::Current
                | artifact_auth_core::KeyCurrentness::VerificationOverlap
        ),
        "key_not_current",
    )?;
    require(
        !policy.exclude_authors
            || !policy
                .author_key_blake3
                .contains(&statement.key_identity.digest_hex),
        "author_key_excluded",
    )
}
fn verify_row(
    review: &SignedSourceReview,
    policy: &SourceReviewVerificationPolicy,
) -> Result<String> {
    validate_links(&review.claim, policy)?;
    let scope = review.claim.scope(policy.verifier_context.clone())?;
    require(review.statement.scope == scope, "statement_scope_mismatch")?;
    validate_signer(&review.statement, review.generation, policy)?;
    let key = decode_hex::<32>(&review.public_key_hex)?;
    let signature = decode_hex::<64>(&review.signature_hex)?;
    let mut single_key_policy = policy.authentication_policy.clone();
    single_key_policy.threshold = 1;
    let decision = verify_and_evaluate(
        &single_key_policy,
        &scope,
        &[Ed25519Evidence {
            statement: &review.statement,
            generation: review.generation,
            public_key_bytes: &key,
            signature_bytes: &signature,
        }],
    );
    require(decision.passed, "signature_authentication_failed")?;
    statement_identity(&review.statement)
        .map_err(|_| SourceReviewError("statement_invalid".to_owned()))
}

impl SourceReviewAttachment {
    pub fn new(reviews: Vec<SignedSourceReview>) -> Result<Self> {
        let mut attachment = Self {
            schema: ATTACHMENT_SCHEMA.to_owned(),
            reviews,
            non_claims: required_non_claims(),
        };
        attachment.normalize()?;
        Ok(attachment)
    }
    fn normalize(&mut self) -> Result<()> {
        require(
            self.schema == ATTACHMENT_SCHEMA,
            "attachment_schema_unsupported",
        )?;
        require(self.reviews.len() <= MAX_ROWS, "review_count_exceeded")?;
        non_claims(&self.non_claims)?;
        let mut seen = BTreeSet::new();
        let mut rows = Vec::with_capacity(self.reviews.len());
        for review in self.reviews.drain(..) {
            review.claim.validate()?;
            let public_key = decode_hex::<32>(&review.public_key_hex)?;
            decode_hex::<64>(&review.signature_hex)?;
            require(
                review.statement.key_identity == public_key_identity(&public_key),
                "public_key_identity_mismatch",
            )?;
            require(
                seen.insert(review.statement.key_identity.digest_hex.clone()),
                "duplicate_review_key",
            )?;
            let identity = statement_identity(&review.statement)
                .map_err(|_| SourceReviewError("statement_invalid".to_owned()))?;
            rows.push((identity, review));
        }
        rows.sort_by(|(left_id, left), (right_id, right)| {
            left.statement
                .key_identity
                .digest_hex
                .cmp(&right.statement.key_identity.digest_hex)
                .then(left_id.cmp(right_id))
        });
        self.reviews = rows.into_iter().map(|(_, row)| row).collect();
        Ok(())
    }
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        require(self.reviews.len() <= MAX_ROWS, "review_count_exceeded")?;
        let mut normalized = self.clone();
        normalized.normalize()?;
        canonical_json_bytes(&normalized)
    }
    pub fn identity(&self) -> Result<String> {
        domain_identity(ATTACHMENT_SCHEMA, &self.canonical_bytes()?)
    }
}

pub fn verify(
    attachment: &SourceReviewAttachment,
    policy: &SourceReviewVerificationPolicy,
) -> Result<SourceReviewReport> {
    policy.validate()?;
    let attachment_identity = attachment.identity()?;
    let mut approved = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    let mut statements = BTreeSet::new();
    for review in &attachment.reviews {
        statements.insert(verify_row(review, policy)?);
        let keys = match review.claim.disposition {
            SourceReviewDisposition::Approved => &mut approved,
            SourceReviewDisposition::NeedsRevision => &mut rejected,
        };
        keys.insert(review.statement.key_identity.digest_hex.clone());
    }
    Ok(SourceReviewReport {
        schema: "cairn.source-review-verification.v1",
        outcome: if approved.len() >= usize::from(policy.required_approvals) {
            SourceReviewOutcome::AcceptedManualReview
        } else {
            SourceReviewOutcome::Insufficient
        },
        verification_role: "manual_review",
        attachment_identity,
        approved_key_blake3: approved.into_iter().collect(),
        needs_revision_key_blake3: rejected.into_iter().collect(),
        canonical_statement_blake3: statements.into_iter().collect(),
        non_claims: required_non_claims(),
    })
}

/// Decode directly to closed DTOs (not through Value, which loses duplicate fields).
pub fn parse<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    require(bytes.len() <= MAX_INPUT_BYTES, "input_bytes_exceeded")?;
    serde_json::from_slice(bytes)
        .map_err(|error| SourceReviewError(format!("json_invalid: {error}")))
}

pub fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    fn sorted(value: Value) -> Value {
        match value {
            Value::Object(map) => {
                let ordered: std::collections::BTreeMap<_, _> = map
                    .into_iter()
                    .map(|(key, value)| (key, sorted(value)))
                    .collect();
                Value::Object(ordered.into_iter().collect())
            }
            Value::Array(values) => Value::Array(values.into_iter().map(sorted).collect()),
            value => value,
        }
    }
    let value =
        serde_json::to_value(value).map_err(|error| SourceReviewError(error.to_string()))?;
    serde_json::to_vec(&sorted(value)).map_err(|error| SourceReviewError(error.to_string()))
}
fn logical_identity<T: Serialize>(domain: &str, value: &T) -> Result<String> {
    domain_identity(domain, &canonical_json_bytes(value)?)
}
fn domain_identity(domain: &str, bytes: &[u8]) -> Result<String> {
    let mut hash = blake3::Hasher::new();
    hash.update(domain.as_bytes());
    hash.update(&[0]);
    hash.update(bytes);
    Ok(hash.finalize().to_hex().to_string())
}
pub fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 15)]));
    }
    output
}
fn decode_hex<const N: usize>(value: &str) -> Result<[u8; N]> {
    require(value.len() == N * 2, "hex_length_invalid")?;
    fn nibble(value: u8) -> Result<u8> {
        match value {
            b'0'..=b'9' => Ok(value - b'0'),
            b'a'..=b'f' => Ok(value - b'a' + 10),
            _ => Err(SourceReviewError("hex_not_lowercase".to_owned())),
        }
    }
    let mut output = [0; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        output[index] = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
