#![allow(clippy::module_name_repetitions)]

use serde::Deserialize;
use serde::Serialize;

/// Evidence-backed support label for a production-relevant surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SupportLevel {
    /// Verified for controlled integrations where the caller wires all external truth.
    ControlledIntegrationReady,
    /// Public or candidate surface that is not yet promoted to controlled production use.
    Preview,
    /// Deliberately not supplied by this crate.
    Unsupported,
    /// Turnkey production behavior supplied by this crate.
    ProductionSupported,
}

impl SupportLevel {
    /// Stable machine-readable label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ControlledIntegrationReady => "controlled-integration-ready",
            Self::Preview => "preview",
            Self::Unsupported => "unsupported",
            Self::ProductionSupported => "production-supported",
        }
    }
}

/// One production-relevant surface and its current support label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadinessSurface {
    /// Stable surface identifier used by docs and checks.
    pub id: &'static str,
    /// Current support label.
    pub level: SupportLevel,
    /// Short evidence summary for operators and integrators.
    pub evidence: &'static str,
    /// Boundary that remains owned by the consuming application or future work.
    pub boundary: &'static str,
}

/// One closed integration choice and its current support label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadinessOption {
    /// Stable option identifier used by integration profiles.
    pub id: &'static str,
    /// Current support label.
    pub level: SupportLevel,
    /// Short evidence summary for reviewers.
    pub evidence: &'static str,
    /// Boundary retained by the consuming application.
    pub boundary: &'static str,
}

/// The explicit integration profile required for controlled production use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegrationProfile {
    /// Stable profile identifier.
    pub id: &'static str,
    /// Required caller-owned inputs for production use.
    pub required_inputs: &'static [&'static str],
    /// Behaviors that must fail closed when the caller-owned backend cannot answer.
    pub fail_closed_inputs: &'static [&'static str],
}

/// Static production-readiness report for this crate version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductionReadinessReport {
    /// Report schema version.
    pub schema_version: u8,
    /// Current crate-level support label.
    pub crate_level: SupportLevel,
    /// Current production-relevant surface labels.
    pub surfaces: &'static [ReadinessSurface],
    /// Closed DID method choices.
    pub did_methods: &'static [ReadinessOption],
    /// Closed compact-token signature algorithm choices.
    pub signing_algorithms: &'static [ReadinessOption],
    /// Closed signing and verification key class choices.
    pub key_classes: &'static [ReadinessOption],
    /// Explicit integration profile for controlled production use.
    pub integration_profile: IntegrationProfile,
}

const REQUIRED_INPUTS: &[&str] = &[
    "KeyResolver or KeyResolutionContext",
    "ProofStore or ProofCollection",
    "RevocationChecker or NoRevocations",
    "VerificationTime",
    "VerificationLimits",
    "CaveatPolicySet",
    "ReplayAdmission or caller-owned replay admission policy",
    "OperationProfileAdmission and HolderInvocationReplayAdmission",
    "HolderInvocationLimits and downstream request-binding digest",
    "TokenSigner and caller-owned key custody",
];

const FAIL_CLOSED_INPUTS: &[&str] = &[
    "key-resolution backend errors",
    "proof-store missing, stale, oversized, cyclic, or backend errors",
    "revocation denial or backend errors",
    "replay admission duplicate, stale, malformed, denied, or backend errors",
    "holder signature, operation profile, nonce, and canonical limit failures",
    "missing or rejected caveat policy",
    "unsupported DID method, signing algorithm, or key type",
    "verification limit overflow",
];

const DID_METHODS: &[ReadinessOption] = &[
    ReadinessOption {
        id: "did:key",
        level: SupportLevel::ControlledIntegrationReady,
        evidence: "compact-token issuance and verification support Ed25519 did:key identifiers",
        boundary: "key provenance, custody, trust roots, and resolver freshness remain caller-owned",
    },
    ReadinessOption {
        id: "remote-did-method",
        level: SupportLevel::Unsupported,
        evidence: "no remote DID method resolver is built into this crate",
        boundary: "method admission, network resolution, trust roots, caching, and freshness remain caller-owned",
    },
];

const SIGNING_ALGORITHMS: &[ReadinessOption] = &[
    ReadinessOption {
        id: "EdDSA",
        level: SupportLevel::ControlledIntegrationReady,
        evidence: "compact-token signing and verification support Ed25519 with the EdDSA header",
        boundary: "key custody, key provenance, and deployment hardening remain caller-owned",
    },
    ReadinessOption {
        id: "non-EdDSA",
        level: SupportLevel::Unsupported,
        evidence: "no alternative compact-token signature algorithm is admitted",
        boundary: "future algorithms require explicit parsing, mismatch, signature, and failure evidence",
    },
];

const KEY_CLASSES: &[ReadinessOption] = &[
    ReadinessOption {
        id: "Ed25519",
        level: SupportLevel::ControlledIntegrationReady,
        evidence: "Ed25519 signing and verification are covered by compact-token tests and examples",
        boundary: "secret material storage, rotation, provenance, and custody remain caller-owned",
    },
    ReadinessOption {
        id: "non-Ed25519",
        level: SupportLevel::Unsupported,
        evidence: "no other signing or verification key class is admitted",
        boundary: "future key classes require explicit format, algorithm, mismatch, and failure evidence",
    },
];

const SURFACES: &[ReadinessSurface] = &[
    ReadinessSurface {
        id: "ed25519-did-key-compact-token",
        level: SupportLevel::ControlledIntegrationReady,
        evidence: "issue/parse/verify APIs, signer abstraction, examples, tests, and nix flake checks cover Ed25519 did:key compact tokens",
        boundary: "caller owns key custody, key provenance, request policy, and deployment hardening",
    },
    ReadinessSurface {
        id: "ucan-wg-1.0.0-authorization",
        level: SupportLevel::ControlledIntegrationReady,
        evidence: "pinned final-profile wire fixtures, command and policy matrices, ordered CID-bound Delegation chains, exact executor admission, one replay commit, private live values, and redacted receipt tests pass",
        boundary: "callers own key truth, proof transport and availability, executor identity, clock truth, revocation freshness, replay durability, policy completeness, transport security, resource ownership, and effect correctness",
    },
    ReadinessSurface {
        id: "caller-supplied-key-resolution",
        level: SupportLevel::ControlledIntegrationReady,
        evidence: "KeyResolver injection and KeyResolutionContext support deterministic local resolution; backend errors fail closed",
        boundary: "remote DID method support, trust roots, freshness, and cache behavior are application-owned",
    },
    ReadinessSurface {
        id: "caller-supplied-proof-store",
        level: SupportLevel::ControlledIntegrationReady,
        evidence: "ProofStore injection, ProofCollection, bounded traversal, proof-link checks, and cycle/limit failures are tested",
        boundary: "network, filesystem, database, IPFS, content-addressed freshness, and proof provenance are application-owned",
    },
    ReadinessSurface {
        id: "caller-supplied-revocation",
        level: SupportLevel::ControlledIntegrationReady,
        evidence: "RevocationChecker injection, NoRevocations, RevocationList, and leaf/proof-chain denial failures are covered",
        boundary: "registry truth, persistence, duplicate semantics, freshness, and backend availability are application-owned",
    },
    ReadinessSurface {
        id: "generic-consumer-integration-hooks",
        level: SupportLevel::ControlledIntegrationReady,
        evidence: "generic key, proof, revocation, profile, caveat, and replay hooks are public and tested without consumer-specific adapters",
        boundary: "consumer-specific resource, ability, request, identity, envelope, backend, freshness, retry, persistence, and execution semantics are application-owned",
    },
    ReadinessSurface {
        id: "caller-supplied-replay-admission",
        level: SupportLevel::ControlledIntegrationReady,
        evidence: "ReplayAdmission and verify_invocation_with_replay allow caller-owned duplicate, stale, malformed, denial, and backend policies to fail closed",
        boundary: "durable replay detection, crash recovery, backend corruption handling, and cross-process freshness are application-owned",
    },
    ReadinessSurface {
        id: "holder-signed-invocation-admission",
        level: SupportLevel::ControlledIntegrationReady,
        evidence: "borrowed direct and delegated tests bind the complete token reference, verified audience, resource, ability, operation profile, request digest, and fixed nonce to an audience-key signature before one final replay commit",
        boundary: "profile registry truth, request canonicalizer provenance, nonce durability, transport identity, action semantics, consumptive rate limits, and execution effects remain application-owned",
    },
    ReadinessSurface {
        id: "verified-replay-admission-transcript",
        level: SupportLevel::ControlledIntegrationReady,
        evidence: "ReplayAdmission outcomes are routed through a vendored verified finite transcript that accepts only allowed outcomes and reports the first duplicate, stale, malformed, denied, backend, or unknown failure",
        boundary: "nonce uniqueness truth, request identity provenance, durable replay storage, crash consistency, backend freshness, and consumer envelope semantics are application-owned",
    },
    ReadinessSurface {
        id: "verified-proof-traversal-orchestration",
        level: SupportLevel::ControlledIntegrationReady,
        evidence: "Proof traversal budget, cycle/visited-reference, proof-link, and effective-delegation transcript facts are routed through vendored verified finite traversal predicates before delegated authorization is accepted",
        boundary: "proof-store truth, parser output, signature checks, revocation freshness, backend provenance, and runtime recursion remain caller/runtime-owned boundaries",
    },
    ReadinessSurface {
        id: "verified-compact-token-codec-structure",
        level: SupportLevel::ControlledIntegrationReady,
        evidence: "Compact segment framing, exact signing-input/compact reconstruction, base64url segment admission, and decoded JSON object-envelope index discovery route through vendored verified structural predicates",
        boundary: "base64url decoding, serde JSON semantics, duplicate-key behavior, canonicalization, UTF-8/string handling, and Ed25519 correctness remain runtime boundaries",
    },
    ReadinessSurface {
        id: "verified-core-structural-validators",
        level: SupportLevel::ControlledIntegrationReady,
        evidence: "DID claim shape, required-claim presence, temporal order, proof-reference/capability/caveat list bounds, and parsed caveat payload shape route through vendored verified finite validators",
        boundary: "serde extraction truth, runtime parser behavior, DID method semantics, clock truth, proof/backend truth, and consumer caveat policy remain outside the structural claim",
    },
    ReadinessSurface {
        id: "verified-ability-helper-predicates",
        level: SupportLevel::ControlledIntegrationReady,
        evidence: "VerifiedCapability wildcard-segment and concrete-ability helpers route full-segment wildcard classification through vendored verified ability predicates",
        boundary: "consumer-specific resource/action/session policy and mapping for Aspen, Clankers, or other integrations remain downstream-owned",
    },
    ReadinessSurface {
        id: "verified-backend-transcript-aggregation",
        level: SupportLevel::ControlledIntegrationReady,
        evidence: "Key-resolution, proof-load, revocation-check, and replay-admission backend operation facts route through vendored verified transcript aggregation with first-failure semantics",
        boundary: "actual key material, proof bytes, registry state, nonce-store freshness, storage durability, network availability, and backend authorization semantics are caller-owned",
    },
    ReadinessSurface {
        id: "opaque-caveat-policy-evaluation",
        level: SupportLevel::ControlledIntegrationReady,
        evidence: "CaveatPolicySet requires every caveat to have an explicit non-consuming policy observation; missing or rejected policies deny authorization",
        boundary: "arbitrary caveat semantics, clocks, counters, consumptive quota or rate-limit effects, and external policy truth are application-owned",
    },
    ReadinessSurface {
        id: "finite-typed-caveat-helpers",
        level: SupportLevel::Preview,
        evidence: "verified finite time-bound and rate-limit helper predicates are exposed for advanced callers",
        boundary: "adopting those helpers as a production vocabulary requires integration-specific policy and provenance evidence",
    },
    ReadinessSurface {
        id: "web-of-trust-helpers",
        level: SupportLevel::Preview,
        evidence: "finite WOT evidence, threshold, and UCAN authorization helpers are exposed below the compact-token shell",
        boundary: "DID/CID decoding, evidence fetching, trust-store discovery, revocation freshness, and persistence are not supplied",
    },
    // r[impl ucan.nostr_identity_adapter.support_profile]
    ReadinessSurface {
        id: "nostr-identity-adapter",
        level: SupportLevel::Preview,
        evidence: "the optional ucan-nostr package verifies bounded NIP-01, NIP-42, NIP-98, and application challenge facts with exact digest roles",
        boundary: "relay access, DNS, signing, clocks, replay storage, subject resolution, and authorization remain caller-owned",
    },
    ReadinessSurface {
        id: "remote-did-resolution",
        level: SupportLevel::Unsupported,
        evidence: "no built-in remote resolver is provided",
        boundary: "integrators must supply resolver trust, method admission, freshness, cache, and fail-closed behavior",
    },
    ReadinessSurface {
        id: "proof-transport-backends",
        level: SupportLevel::Unsupported,
        evidence: "no built-in network, filesystem, database, IPFS, or content-addressed proof transport is provided",
        boundary: "integrators must supply proof provenance, freshness, bounds, and reference matching",
    },
    ReadinessSurface {
        id: "registry-backed-revocation",
        level: SupportLevel::Unsupported,
        evidence: "no built-in registry client or persistence layer is provided",
        boundary: "integrators must supply registry truth, freshness, duplicate handling, and outage behavior",
    },
    ReadinessSurface {
        id: "replay-persistence-store",
        level: SupportLevel::Unsupported,
        evidence: "receipt validation helpers do not store receipts or reject duplicate invocations across restarts",
        boundary: "integrators must supply durable replay detection, recovery, corruption handling, and freshness policy",
    },
    ReadinessSurface {
        id: "non-ed25519-key-algorithms",
        level: SupportLevel::Unsupported,
        evidence: "compact-token signing and verification admit Ed25519 did:key with EdDSA only",
        boundary: "future algorithms need parsing, admission, mismatch, signature, and unsupported-algorithm evidence",
    },
];

/// Return the static production-readiness report for this crate version.
#[must_use]
pub const fn production_readiness_report() -> ProductionReadinessReport {
    ProductionReadinessReport {
        schema_version: 1,
        crate_level: SupportLevel::ControlledIntegrationReady,
        surfaces: SURFACES,
        did_methods: DID_METHODS,
        signing_algorithms: SIGNING_ALGORITHMS,
        key_classes: KEY_CLASSES,
        integration_profile: IntegrationProfile {
            id: "caller-wired-controlled-integration",
            required_inputs: REQUIRED_INPUTS,
            fail_closed_inputs: FAIL_CLOSED_INPUTS,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::SupportLevel;
    use super::production_readiness_report;

    #[test]
    fn report_labels_current_crate_as_controlled_integration_ready() {
        let report = production_readiness_report();
        assert_eq!(report.crate_level, SupportLevel::ControlledIntegrationReady);
        assert_eq!(report.integration_profile.id, "caller-wired-controlled-integration");
        assert!(report.surfaces.iter().any(|surface| {
            surface.id == "ed25519-did-key-compact-token" && surface.level == SupportLevel::ControlledIntegrationReady
        }));
    }

    #[test]
    fn no_surface_is_promoted_to_production_supported_without_evidence() {
        let report = production_readiness_report();
        assert!(report.surfaces.iter().all(|surface| surface.level != SupportLevel::ProductionSupported));
    }

    #[test]
    fn unsupported_platform_surfaces_remain_explicit() {
        let report = production_readiness_report();
        for surface_id in [
            "remote-did-resolution",
            "proof-transport-backends",
            "registry-backed-revocation",
            "replay-persistence-store",
            "non-ed25519-key-algorithms",
        ] {
            assert!(
                report
                    .surfaces
                    .iter()
                    .any(|surface| { surface.id == surface_id && surface.level == SupportLevel::Unsupported })
            );
        }
    }

    #[test]
    fn closed_integration_options_preserve_supported_and_unsupported_choices() {
        let report = production_readiness_report();
        assert!(
            report
                .did_methods
                .iter()
                .any(|option| { option.id == "did:key" && option.level == SupportLevel::ControlledIntegrationReady })
        );
        assert!(
            report
                .signing_algorithms
                .iter()
                .any(|option| { option.id == "non-EdDSA" && option.level == SupportLevel::Unsupported })
        );
        assert!(
            report
                .key_classes
                .iter()
                .any(|option| { option.id == "non-Ed25519" && option.level == SupportLevel::Unsupported })
        );
    }

    // r[verify ucan.nostr_identity_adapter.support_profile]
    #[test]
    fn nostr_adapter_stays_preview_and_caller_owned() {
        let report = production_readiness_report();
        let surface = report
            .surfaces
            .iter()
            .find(|surface| surface.id == "nostr-identity-adapter")
            .expect("Nostr adapter surface must exist");

        assert_eq!(surface.level, SupportLevel::Preview);
        assert!(surface.boundary.contains("replay storage"));
        assert!(surface.boundary.contains("authorization"));
    }

    #[test]
    fn integration_profile_requires_caller_owned_truth_inputs() {
        let report = production_readiness_report();
        for input in [
            "KeyResolver or KeyResolutionContext",
            "ProofStore or ProofCollection",
            "RevocationChecker or NoRevocations",
            "ReplayAdmission or caller-owned replay admission policy",
            "VerificationTime",
            "VerificationLimits",
            "CaveatPolicySet",
            "TokenSigner and caller-owned key custody",
        ] {
            assert!(report.integration_profile.required_inputs.contains(&input));
        }
    }
}
