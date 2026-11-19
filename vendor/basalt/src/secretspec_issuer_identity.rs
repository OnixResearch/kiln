//! Immutable identities for the SecretSpec-backed UCAN issuer-key boundary.

/// Reviewed `SecretSpec` SDK version pinned by Basalt.
pub const SECRETSPEC_VERSION: &str = "0.17.0";
/// Reviewed `SecretSpec` upstream tag commit that produced `v0.17.0`.
///
/// This is the `v0.17.0` tag: `git rev-parse v0.17.0` resolves to this commit.
/// Builds resolve the exact crates.io release, so this identity anchors review
/// and release receipts to a durable, non-moving source boundary.
pub const SECRETSPEC_REVIEWED_COMMIT: &str = "a8794e46ec9664a0e1a3869cc3105d0853937e48";
/// `SecretSpec` license expression recorded in its package metadata.
pub const SECRETSPEC_LICENSE: &str = "Apache-2.0";
/// Scope under which the UCAN issuer signing key is declared.
pub const OIDC_EXCHANGE_SCOPE: &str = "basalt-oidc-exchange";
/// Schema for the issuer-key metadata declaration.
pub const ISSUER_DECLARATION_SCHEMA: &str = "basalt.oidc-exchange.issuer-key-declaration.v1";
/// Schema for the bounded issuer-key resolution request.
pub const ISSUER_RESOLUTION_SCHEMA: &str = "basalt.oidc-exchange.issuer-key-resolution.v1";
/// Schema for the typed issuer-key resolution result.
pub const ISSUER_RESOLUTION_RESULT_SCHEMA: &str = "basalt.oidc-exchange.issuer-key-result.v1";
/// Schema for the release verifier's issuer-key receipt.
pub const ISSUER_RELEASE_RECEIPT_SCHEMA: &str =
    "basalt.oidc-exchange.issuer-key-release-receipt.v1";

/// Claim boundaries carried by every issuer-key resolution result.
pub const REQUIRED_ISSUER_NON_CLAIMS: [&str; 6] = [
    "oidc-token-truth",
    "ucan-issuance",
    "provider-secret-authority",
    "key-distribution-freshness",
    "revocation-freshness",
    "whole-system-safety",
];
