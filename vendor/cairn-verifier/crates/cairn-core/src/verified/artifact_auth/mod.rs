//! Cairn's bounded consumer boundary for the immutable standalone artifact-auth source.
//!
//! These `verify` links bind requirements checked by the exact standalone revision plus
//! Cairn's source-pin/conformance rail. Traceability is linkage evidence only; this consumer
//! module does not independently prove the standalone implementation.
// r[verify cairn.artifact_auth.repository_extraction.inventory]
// r[verify cairn.artifact_auth.repository_extraction.inventory.complete]
// r[verify cairn.artifact_auth.repository_extraction.inventory.incomplete]
// r[verify cairn.artifact_auth.repository_extraction.repository]
// r[verify cairn.artifact_auth.repository_extraction.repository.neutral]
// r[verify cairn.artifact_auth.repository_extraction.repository.product_dependency]
// r[verify cairn.artifact_auth.repository_extraction.functional_core]
// r[verify cairn.artifact_auth.repository_extraction.functional_core.deterministic]
// r[verify cairn.artifact_auth.repository_extraction.functional_core.no_authority]
// r[verify cairn.artifact_auth.repository_extraction.canonical_statement]
// r[verify cairn.artifact_auth.repository_extraction.canonical_statement.stable]
// r[verify cairn.artifact_auth.repository_extraction.canonical_statement.domain_separation]
// r[verify cairn.artifact_auth.repository_extraction.ed25519]
// r[verify cairn.artifact_auth.repository_extraction.ed25519.positive]
// r[verify cairn.artifact_auth.repository_extraction.ed25519.negative]
// r[verify cairn.artifact_auth.repository_extraction.policy]
// r[verify cairn.artifact_auth.repository_extraction.policy.positive]
// r[verify cairn.artifact_auth.repository_extraction.policy.negative]
// r[verify cairn.artifact_auth.repository_extraction.conformance.positive]
// r[verify cairn.artifact_auth.repository_extraction.conformance.negative]
// r[verify cairn.artifact_auth.repository_extraction.non_claims]
// r[verify cairn.artifact_auth.repository_extraction.non_claims.preserved]
// r[verify cairn.artifact_auth.repository_extraction.non_claims.overclaim]

pub mod adapter;
