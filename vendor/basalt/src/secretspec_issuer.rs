//! `SecretSpec`-backed resolution of the UCAN issuer signing key.
//!
//! The pure core here validates an issuer-key declaration, builds the bounded
//! `SecretSpec` resolve request, and classifies a supplied resolution response.
//! The thin `shell` submodule runs the pinned `SecretSpec` SDK inside
//! an owned bounded worker with an explicit timeout and output-size bound, and
//! always fails closed on any unsafe outcome.
//!
//! This boundary proves only that a reviewed `SecretSpec` boundary can resolve a
//! declared issuer key. It does not verify OIDC tokens, issue UCAN
//! capabilities, or prove that any provider is a correct secret authority.
//!
// Traceability:
// r[impl ucan_token_flow.oidc.secretspec_dependency]
// r[impl ucan_token_flow.oidc.secretspec_issuer_key]

pub use crate::secretspec_issuer_identity::{
    ISSUER_DECLARATION_SCHEMA, ISSUER_RELEASE_RECEIPT_SCHEMA, ISSUER_RESOLUTION_RESULT_SCHEMA,
    ISSUER_RESOLUTION_SCHEMA, OIDC_EXCHANGE_SCOPE, REQUIRED_ISSUER_NON_CLAIMS, SECRETSPEC_LICENSE,
    SECRETSPEC_REVIEWED_COMMIT, SECRETSPEC_VERSION,
};
use serde::{Deserialize, Serialize};

mod shell;
#[cfg(test)]
mod tests;

pub use shell::{
    IssuerResolutionFailure, ResolutionBoundaries, ShellResolution, resolve_issuer_key,
};

/// Maximum accepted byte length for a resolved issuer-key value.
pub const DEFAULT_MAX_RESOLVED_VALUE_BYTES: usize = 8 * 1024;
/// Default worker timeout for a bounded `SecretSpec` resolution.
pub const DEFAULT_RESOLUTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
/// Maximum byte length of a declaration field.
pub const MAX_DECLARATION_FIELD_BYTES: usize = 4_096;
/// Maximum number of overlapping verification keys in one rotation window.
pub const MAX_ROTATION_OVERLAP_KEYS: usize = 8;
/// Maximum byte length of one public verification key digest.
pub const MAX_VERIFICATION_KEY_DIGEST_BYTES: usize = 4_096;

/// Failure reason recorded on the result receipt by the bounded shell.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum IssuerResolutionReason {
    /// The declaration is invalid for this boundary.
    DeclarationRejected,
    /// The declared key value resolved from the pinned SDK.
    Loaded,
    /// A required declared key was missing from the resolution.
    Missing,
    /// The resolution did not run under the declared scope.
    WrongScope,
    /// The bounded worker exceeded its explicit timeout.
    Timeout,
    /// The provider produced more output than the bound allows.
    OversizedOutput,
    /// The selected provider failed or was unavailable.
    ProviderFailure,
}

impl IssuerResolutionReason {
    /// Return the stable serialized reason identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeclarationRejected => "declaration-rejected",
            Self::Loaded => "loaded",
            Self::Missing => "missing",
            Self::WrongScope => "wrong-scope",
            Self::Timeout => "timeout",
            Self::OversizedOutput => "oversized-output",
            Self::ProviderFailure => "provider-failure",
        }
    }
}

/// Metadata-only declaration of the UCAN issuer signing key.
///
/// This declaration carries no key value. The key is resolved through the
/// pinned `SecretSpec` SDK at the bounded shell boundary.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IssuerKeyDeclaration {
    /// Declaration schema identifier.
    pub schema: String,
    /// Reviewed `SecretSpec` version pinned by Basalt.
    pub secretspec_version: String,
    /// Reviewed `SecretSpec` upstream commit identity.
    pub secretspec_commit: String,
    /// Explicit scope; must equal [`OIDC_EXCHANGE_SCOPE`].
    pub scope: String,
    /// Explicit profile used for resolution.
    pub profile: String,
    /// Explicit key identifier resolved from the provider.
    pub key_id: String,
    /// Credential-free provider spec, or the empty string for the default.
    pub provider: String,
    /// Explicit claim boundaries.
    pub non_claims: Vec<String>,
}

impl IssuerKeyDeclaration {
    /// Construct a bounded issuer-key declaration under the reviewed scope.
    #[must_use]
    pub fn new(
        profile: impl Into<String>,
        key_id: impl Into<String>,
        provider: impl Into<String>,
    ) -> Self {
        Self {
            schema: ISSUER_DECLARATION_SCHEMA.to_owned(),
            secretspec_version: SECRETSPEC_VERSION.to_owned(),
            secretspec_commit: SECRETSPEC_REVIEWED_COMMIT.to_owned(),
            scope: OIDC_EXCHANGE_SCOPE.to_owned(),
            profile: profile.into(),
            key_id: key_id.into(),
            provider: provider.into(),
            non_claims: REQUIRED_ISSUER_NON_CLAIMS.map(str::to_owned).to_vec(),
        }
    }
}

/// Stable deterministic finding on a declaration or resolution.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct IssuerIssue {
    /// Machine-readable diagnostic code.
    pub code: String,
    /// Stable human-readable explanation.
    pub message: String,
}

impl IssuerIssue {
    fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.to_owned(),
            message: message.to_owned(),
        }
    }
}

/// Pure result of classifying one bounded resolution response.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IssuerResolutionResult {
    /// Result schema identifier.
    pub schema: String,
    /// Stable reason class.
    pub reason: IssuerResolutionReason,
    /// Whether the declared key load was admitted.
    pub allowed: bool,
    /// Deterministic issues; empty on a clean load.
    pub issues: Vec<IssuerIssue>,
    /// Explicit non-claims preserved on every result.
    pub non_claims: Vec<String>,
}

impl IssuerResolutionResult {
    /// Return true only when the declared key resolved cleanly.
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        self.allowed
    }
}

/// Validate an issuer-key declaration without performing I/O.
///
/// Returns the ordered issues when the declaration is not admissible.
// r[impl ucan_token_flow.oidc.secretspec_issuer_key]
#[must_use]
pub fn validate_declaration(declaration: &IssuerKeyDeclaration) -> Vec<IssuerIssue> {
    let mut issues = Vec::new();
    if declaration.schema != ISSUER_DECLARATION_SCHEMA {
        issues.push(IssuerIssue::new(
            "issuer.schema",
            "unsupported issuer-key declaration schema",
        ));
    }
    if declaration.scope != OIDC_EXCHANGE_SCOPE {
        issues.push(IssuerIssue::new(
            "issuer.scope",
            "issuer-key declaration must use the reviewed exchange scope",
        ));
    }
    if !bounded_non_empty(&declaration.profile) {
        issues.push(IssuerIssue::new(
            "issuer.profile",
            "issuer-key declaration profile must be non-empty and bounded",
        ));
    }
    if !bounded_non_empty(&declaration.key_id) {
        issues.push(IssuerIssue::new(
            "issuer.key-id",
            "issuer key identifier must be non-empty and bounded",
        ));
    }
    if declaration.secretspec_version != SECRETSPEC_VERSION
        || declaration.secretspec_commit != SECRETSPEC_REVIEWED_COMMIT
    {
        issues.push(IssuerIssue::new(
            "issuer.dependency",
            "declaration does not pin the reviewed SecretSpec identity",
        ));
    }
    if declaration.non_claims != REQUIRED_ISSUER_NON_CLAIMS.map(str::to_owned) {
        issues.push(IssuerIssue::new(
            "issuer.non-claims",
            "declaration must preserve the complete ordered non-claim boundary",
        ));
    }
    issues
}

/// Build the deterministic JSON request consumed by `secretspec::resolve_json`.
///
/// The request requests only the value bound to the declared scope and key
/// identifier. It never enables a write or a plaintext value cache path.
#[must_use]
pub fn build_resolve_request(declaration: &IssuerKeyDeclaration, config_path: &str) -> String {
    let mut request = serde_json::json!({
        "path": config_path,
        "scope": declaration.scope,
        "profile": declaration.profile,
        "reason": format!("resolve:{}", declaration.key_id),
        "no_values": false,
        "mode": "resolve",
    });
    if !declaration.provider.is_empty() {
        request["provider"] = serde_json::json!(declaration.provider);
    }
    serde_json::to_string(&request).unwrap_or_else(|_| "{}".to_owned())
}

/// Classify one bounded resolution response into a deterministic result.
///
/// The response JSON is the `{"ok":true,"response":...}` / `{"ok":false,...}`
/// envelope returned by `secretspec::resolve_json`. This core never inspects
/// provider internals, clocks, processes, or environments.
#[must_use]
pub fn classify_resolution(
    declaration: &IssuerKeyDeclaration,
    response_json: &str,
    output_bytes: usize,
) -> IssuerResolutionResult {
    let declaration_issues = validate_declaration(declaration);
    if !declaration_issues.is_empty() {
        return result_with(
            IssuerResolutionReason::DeclarationRejected,
            declaration_issues,
        );
    }

    if output_bytes > DEFAULT_MAX_RESOLVED_VALUE_BYTES {
        return result_with(IssuerResolutionReason::OversizedOutput, Vec::new());
    }

    let envelope: serde_json::Value = match serde_json::from_str(response_json) {
        Ok(value) => value,
        Err(_) => return result_with(IssuerResolutionReason::ProviderFailure, Vec::new()),
    };

    if envelope.get("ok") != Some(&serde_json::json!(true)) {
        return result_with(IssuerResolutionReason::ProviderFailure, Vec::new());
    }

    let Some(response) = envelope.get("response") else {
        return result_with(IssuerResolutionReason::ProviderFailure, Vec::new());
    };

    if response.get("scope").and_then(serde_json::Value::as_str) != Some(OIDC_EXCHANGE_SCOPE) {
        return result_with(IssuerResolutionReason::WrongScope, Vec::new());
    }

    let missing_count = response
        .get("missing_required")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, std::vec::Vec::len);
    if missing_count > 0 {
        return result_with(IssuerResolutionReason::Missing, Vec::new());
    }

    let has_key = response
        .get("secrets")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|map| map.contains_key(declaration.key_id.as_str()));
    if !has_key {
        return result_with(IssuerResolutionReason::Missing, Vec::new());
    }

    result_with(IssuerResolutionReason::Loaded, Vec::new())
}

pub(crate) fn result_with(
    reason: IssuerResolutionReason,
    issues: Vec<IssuerIssue>,
) -> IssuerResolutionResult {
    IssuerResolutionResult {
        schema: ISSUER_RESOLUTION_RESULT_SCHEMA.to_owned(),
        reason,
        allowed: reason == IssuerResolutionReason::Loaded,
        issues,
        non_claims: REQUIRED_ISSUER_NON_CLAIMS.map(str::to_owned).to_vec(),
    }
}

/// Public verification-key window carried during rotation.
///
/// This window carries only public verification material (key identifiers and
/// public digests). It never carries private signing material.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RotationWindow {
    /// Public digest of the currently active verification key.
    pub active_verification_digest: String,
    /// Public digests of overlapping verification keys still admitted.
    pub overlap_digests: Vec<String>,
    /// The private signing key identifier currently selected.
    pub signing_key_id: String,
}

impl RotationWindow {
    /// Construct a rotation window from public verification material.
    #[must_use]
    pub fn new(
        active_verification_digest: impl Into<String>,
        overlap_digests: Vec<String>,
        signing_key_id: impl Into<String>,
    ) -> Self {
        Self {
            active_verification_digest: active_verification_digest.into(),
            overlap_digests,
            signing_key_id: signing_key_id.into(),
        }
    }

    /// Return whether this window admits the supplied key identifier.
    #[must_use]
    pub fn admits(&self, signing_key_id: &str) -> bool {
        self.signing_key_id == signing_key_id
    }
}

/// Pure check that a rotation window is admissible.
///
/// The window must carry only public, bounded verification material and at
/// most [`MAX_ROTATION_OVERLAP_KEYS`] overlapping digests. The signing key
/// identifier must be non-empty and bounded. Private material is never
/// admitted into the window.
#[must_use]
pub fn validate_rotation_window(window: &RotationWindow) -> Vec<IssuerIssue> {
    let mut issues = Vec::new();
    if !bounded_non_empty(&window.signing_key_id) {
        issues.push(IssuerIssue::new(
            "issuer.rotation.signing-key",
            "rotation signing key identifier must be non-empty and bounded",
        ));
    }
    if window.overlap_digests.len() > MAX_ROTATION_OVERLAP_KEYS {
        issues.push(IssuerIssue::new(
            "issuer.rotation.overlap-limit",
            "rotation overlap window exceeds the bounded key count",
        ));
    }
    for digest in std::iter::once(window.active_verification_digest.as_str())
        .chain(window.overlap_digests.iter().map(String::as_str))
    {
        if !bounded_non_empty(digest) {
            issues.push(IssuerIssue::new(
                "issuer.rotation.digest",
                "rotation window carries invalid public verification digest material",
            ));
        }
    }
    if window
        .overlap_digests
        .contains(&window.active_verification_digest)
    {
        issues.push(IssuerIssue::new(
            "issuer.rotation.active-duplicate",
            "active verification digest must not repeat in the overlap window",
        ));
    }
    issues
}

fn bounded_non_empty(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_DECLARATION_FIELD_BYTES
        && !value.chars().any(char::is_control)
}
