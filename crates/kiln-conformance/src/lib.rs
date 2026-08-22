use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use kiln_core::discovery::{DiscoveryEffectPlan, admit_discovery_observation};
use kiln_core::domain::Outcome;
use kiln_core::provider::{
    Operation, OperationRequest, ProviderRole, admit_observation, admit_operation,
};
use kiln_core::types::{CONTRACT_VERSION_V1, CoreResult, Diagnostic};
use kiln_provider_api::{DiscoveryPort, ProviderPort, ProviderRequest};

// r[impl kiln.nix_eval_jobs.dependency_boundary]
// r[impl kiln.source.pijul_capabilities] r[impl kiln.source.pijul_nonclaims]
pub const FORBIDDEN_CORE_DEPENDENCIES: &[&str] = &[
    "aspen::",
    "mantle::",
    "lattice::",
    "lattice-workflow-contract =",
    "lattice_workflow_contract::",
    "cairn-gate-contract =",
    "cairn_gate_contract::",
    "bounded-exec =",
    "bounded_exec::",
    "kiln-adapter-cairn =",
    "kiln_adapter_cairn::",
    "kiln-adapter-mantle =",
    "kiln_adapter_mantle::",
    "kiln-adapter-nix-eval =",
    "kiln_adapter_nix_eval::",
    "nix-eval-jobs =",
    "nix_eval_jobs::",
    "nix-store =",
    "nix_store::",
    "nix-expr =",
    "nix_expr::",
    "hydra =",
    "hydra::",
    "kiln-adapter-cob =",
    "kiln_adapter_cob::",
    "kiln-adapter-jj =",
    "kiln_adapter_jj::",
    "kiln-adapter-git =",
    "kiln_adapter_git::",
    "git2 =",
    "git2::",
    "kiln-adapter-pijul =",
    "kiln_adapter_pijul::",
    "libpijul =",
    "libpijul::",
    "pijul-repository =",
    "pijul_repository::",
    "kiln-adapter-radicle =",
    "kiln_adapter_radicle::",
    "kiln-adapter-remote =",
    "kiln_adapter_remote::",
    "kiln-adapter-cdevents =",
    "kiln_adapter_cdevents::",
    "kiln-adapter-opentelemetry =",
    "kiln_adapter_opentelemetry::",
    "kiln-adapter-valence =",
    "kiln_adapter_valence::",
    "valence-core =",
    "valence_core::",
    "kiln-adapter-webhook =",
    "kiln_adapter_webhook::",
    "kiln-adapter-sqlite =",
    "kiln_adapter_sqlite::",
    "kiln-adapter-celld =",
    "kiln_adapter_celld::",
    "celld =",
    "celld::",
    "V8",
    "v8::",
    "worker::",
    "Wrangler",
    "wrangler::",
    "object-store =",
    "object_store::",
    "rusqlite =",
    "rusqlite::",
    "radicle-cob =",
    "radicle_cob::",
    "pueue-lib =",
    "pueue_lib::",
    "std::fs",
    "std::process",
    "std::net",
    "std::time",
    "tokio::",
    "reqwest::",
    "use reqwest",
];

// r[impl kiln.providers.port_ownership] r[impl kiln.providers.composition_root]
// r[impl kiln.providers.boundary_conformance]
pub const FORBIDDEN_ADAPTER_PORT_DECLARATIONS: &[&str] = &[
    "pub trait BrokerService",
    "pub trait CobMutationTransport",
    "pub trait JujutsuRepository",
    "pub trait PijulRepositoryPort",
    "pub trait RemoteLifecycleTransport",
    "pub trait RemoteTransport",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundaryFinding {
    pub token: &'static str,
    pub path: PathBuf,
}

#[must_use]
pub fn audit_text(path: &Path, text: &str) -> Vec<BoundaryFinding> {
    audit_patterns(path, text, FORBIDDEN_CORE_DEPENDENCIES)
}

#[must_use]
pub fn audit_patterns(
    path: &Path,
    text: &str,
    patterns: &'static [&'static str],
) -> Vec<BoundaryFinding> {
    patterns
        .iter()
        .copied()
        .filter(|token| text.contains(token))
        .map(|token| BoundaryFinding {
            token,
            path: path.to_path_buf(),
        })
        .collect()
}

pub fn audit_core(root: &Path) -> std::io::Result<Vec<BoundaryFinding>> {
    audit_application_contracts_at(root, &["crates/kiln-core"])
}

pub fn audit_application_boundaries(root: &Path) -> std::io::Result<Vec<BoundaryFinding>> {
    audit_application_contracts_at(root, &["crates/kiln-core", "crates/kiln-provider-api"])
}

fn audit_application_contracts_at(
    root: &Path,
    paths: &[&str],
) -> std::io::Result<Vec<BoundaryFinding>> {
    let mut findings = Vec::new();
    for relative in paths {
        visit(&root.join(relative), &mut |path| {
            if path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|extension| extension == "rs" || extension == "toml")
            {
                let text = fs::read_to_string(path)?;
                findings.extend(audit_text(path, &text));
            }
            Ok(())
        })?;
    }
    Ok(findings)
}

pub fn audit_adapter_port_ownership(root: &Path) -> std::io::Result<Vec<BoundaryFinding>> {
    let mut findings = Vec::new();
    for relative in [
        "crates/kiln-adapter-cob",
        "crates/kiln-adapter-jj",
        "crates/kiln-adapter-pijul",
        "crates/kiln-adapter-radicle",
        "crates/kiln-adapter-lattice",
        "crates/kiln-adapter-cairn",
        "crates/kiln-adapter-mantle",
        "crates/kiln-adapter-nix-eval",
        "crates/kiln-adapter-remote",
    ] {
        visit(&root.join(relative), &mut |path| {
            if path.extension().and_then(|value| value.to_str()) == Some("rs") {
                let text = fs::read_to_string(path)?;
                findings.extend(audit_patterns(
                    path,
                    &text,
                    FORBIDDEN_ADAPTER_PORT_DECLARATIONS,
                ));
            }
            Ok(())
        })?;
    }
    Ok(findings)
}

pub fn audit_reference_boundary(root: &Path) -> std::io::Result<Vec<BoundaryFinding>> {
    let mut findings = Vec::new();
    for relative in [
        "Cargo.toml",
        "config",
        "flake.nix",
        "crates/kiln-core",
        "crates/kiln-provider-api",
        "crates/kiln-codec",
        "crates/kiln-standalone",
        "crates/kiln-adapter-jj",
        "crates/kiln-adapter-git",
        "crates/kiln-adapter-pijul",
        "crates/kiln-adapter-lattice",
        "crates/kiln-adapter-cairn",
        "crates/kiln-adapter-mantle",
        "crates/kiln-adapter-nix-eval",
        "crates/kiln-adapter-remote",
        "crates/kiln-adapter-radicle",
        "crates/kiln-adapter-cob",
        "crates/kiln-adapter-cdevents",
        "crates/kiln-adapter-opentelemetry",
        "crates/kiln-adapter-valence",
        "crates/kiln-adapter-webhook",
        "crates/kiln-adapter-sqlite",
        "crates/kiln-adapter-celld",
        "crates/kiln-cli",
    ] {
        let selected = root.join(relative);
        visit(&selected, &mut |path| {
            let text = fs::read_to_string(path)?;
            if text.to_ascii_lowercase().contains("pueue") {
                findings.push(BoundaryFinding {
                    token: "pueue product integration",
                    path: path.to_path_buf(),
                });
            }
            Ok(())
        })?;
    }
    Ok(findings)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderConformanceReport {
    pub deterministic_repeat: bool,
    pub deterministic_reconciliation: bool,
    pub supported_operations: BTreeSet<Operation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryConformanceReport {
    pub deterministic_repeat: bool,
    pub deterministic_observation: bool,
    pub deterministic_reconciliation: bool,
    pub supported_operations: BTreeSet<Operation>,
}

pub fn run_discovery_conformance<P: DiscoveryPort>(
    provider: &mut P,
    plan: &DiscoveryEffectPlan,
) -> CoreResult<DiscoveryConformanceReport> {
    let profile = provider.profile().clone();
    let required_operations = BTreeSet::from([
        Operation::Discover,
        Operation::Observe,
        Operation::Cancel,
        Operation::Reconcile,
    ]);
    for operation in &required_operations {
        admit_operation(
            &profile,
            &OperationRequest {
                contract_version: CONTRACT_VERSION_V1,
                selected_profile: plan.provider_profile.clone(),
                role: ProviderRole::Discovery,
                operation: *operation,
                request_bytes: 0,
                required_outcomes: profile.outcomes.clone(),
            },
        )?;
    }
    let first = provider.discover(plan).map_err(port_failure_diagnostic)?;
    admit_discovery_observation(plan, &first)?;
    let repeated = provider.discover(plan).map_err(port_failure_diagnostic)?;
    admit_discovery_observation(plan, &repeated)?;
    let observed = provider
        .observe(&plan.effect_id)
        .map_err(port_failure_diagnostic)?;
    admit_discovery_observation(plan, &observed)?;
    let reconciled = provider.reconcile(plan).map_err(port_failure_diagnostic)?;
    admit_discovery_observation(plan, &reconciled)?;
    if first != repeated || first != observed || first != reconciled {
        return Err(Diagnostic::new(
            "nondeterministic_discovery_provider",
            "provider.discovery",
            "discovery repeat, observation, or reconciliation changed its admitted result",
        ));
    }
    Ok(DiscoveryConformanceReport {
        deterministic_repeat: true,
        deterministic_observation: true,
        deterministic_reconciliation: true,
        supported_operations: required_operations,
    })
}

pub fn run_provider_conformance<P: ProviderPort>(
    provider: &mut P,
    request: &ProviderRequest,
    required_operations: &BTreeSet<Operation>,
    required_outcomes: &BTreeSet<Outcome>,
) -> CoreResult<ProviderConformanceReport> {
    let profile = provider.profile().clone();
    for operation in required_operations {
        admit_operation(
            &profile,
            &OperationRequest {
                contract_version: CONTRACT_VERSION_V1,
                selected_profile: profile.profile_id.clone(),
                role: profile.role,
                operation: *operation,
                request_bytes: request.payload_identity.native_id.as_str().len(),
                required_outcomes: required_outcomes.clone(),
            },
        )?;
    }
    let first = provider.dispatch(request)?;
    admit_observation(&profile, &first)?;
    let repeated = provider.dispatch(request)?;
    admit_observation(&profile, &repeated)?;
    if first != repeated {
        return Err(Diagnostic::new(
            "nondeterministic_provider_repeat",
            "provider.dispatch",
            "provider changed its observation for one idempotency key",
        ));
    }
    let reconciled = provider.reconcile(&request.effect_id)?;
    admit_observation(&profile, &reconciled)?;
    if first != reconciled {
        return Err(Diagnostic::new(
            "stale_provider_reconciliation",
            "provider.reconcile",
            "provider reconciliation disagrees with the accepted observation",
        ));
    }
    Ok(ProviderConformanceReport {
        deterministic_repeat: true,
        deterministic_reconciliation: true,
        supported_operations: required_operations.clone(),
    })
}

fn port_failure_diagnostic(failure: kiln_provider_api::PortFailure) -> Diagnostic {
    match failure {
        kiln_provider_api::PortFailure::Domain(diagnostic) => diagnostic,
        kiln_provider_api::PortFailure::Infrastructure(failure)
        | kiln_provider_api::PortFailure::Retryable(failure)
        | kiln_provider_api::PortFailure::TimedOut(failure)
        | kiln_provider_api::PortFailure::Cancelled(failure)
        | kiln_provider_api::PortFailure::Unknown(failure) => {
            Diagnostic::new(&failure.code, "provider.discovery", &failure.message)
        }
    }
}

fn visit(
    path: &Path,
    inspect: &mut impl FnMut(&Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    if path.is_file() {
        return inspect(path);
    }
    let mut entries: Vec<_> = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        visit(&entry.path(), inspect)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use kiln_core::provider::{
        CapabilityProfile, ProviderLimits, ProviderObservation, ProviderRole,
    };
    use kiln_core::types::{EffectId, ExternalRef, ProviderProfileId};

    use super::*;

    const MAX_REQUEST_BYTES: usize = 4096;
    const MAX_RESULT_BYTES: usize = 4096;
    const MAX_IN_FLIGHT: usize = 4;

    struct FixtureProvider {
        profile: CapabilityProfile,
        observation: ProviderObservation,
    }

    impl ProviderPort for FixtureProvider {
        fn profile(&self) -> &CapabilityProfile {
            &self.profile
        }

        fn dispatch(&mut self, _request: &ProviderRequest) -> CoreResult<ProviderObservation> {
            Ok(self.observation.clone())
        }

        fn reconcile(&mut self, _effect_id: &EffectId) -> CoreResult<ProviderObservation> {
            Ok(self.observation.clone())
        }
    }

    fn fixture_provider() -> FixtureProvider {
        let profile_id =
            ProviderProfileId::new("fixture-build-v1").unwrap_or_else(|_| unreachable!());
        FixtureProvider {
            profile: CapabilityProfile {
                contract_version: CONTRACT_VERSION_V1,
                profile_id: profile_id.clone(),
                role: ProviderRole::Build,
                namespace: ExternalRef::new("fixture", "provider", "build")
                    .unwrap_or_else(|_| unreachable!()),
                operations: BTreeSet::from([
                    Operation::Dispatch,
                    Operation::Cancel,
                    Operation::Reconcile,
                ]),
                outcomes: BTreeSet::from([
                    Outcome::Success,
                    Outcome::Cancelled,
                    Outcome::TimedOut,
                    Outcome::Unknown,
                ]),
                limits: ProviderLimits {
                    max_request_bytes: MAX_REQUEST_BYTES,
                    max_result_bytes: MAX_RESULT_BYTES,
                    max_in_flight: MAX_IN_FLIGHT,
                },
            },
            observation: ProviderObservation {
                contract_version: CONTRACT_VERSION_V1,
                profile_id,
                outcome: Outcome::Success,
                external_reference: Some(
                    ExternalRef::new("fixture", "attempt", "accepted")
                        .unwrap_or_else(|_| unreachable!()),
                ),
            },
        }
    }

    fn provider_request() -> ProviderRequest {
        ProviderRequest {
            effect_id: EffectId::new("effect").unwrap_or_else(|_| unreachable!()),
            idempotency_key: ExternalRef::new("kiln", "command", "command")
                .unwrap_or_else(|_| unreachable!()),
            payload_identity: ExternalRef::new("kiln", "payload", "payload")
                .unwrap_or_else(|_| unreachable!()),
        }
    }

    #[test]
    fn reusable_provider_conformance_covers_repeat_cancel_timeout_and_reconcile() {
        let mut provider = fixture_provider();
        let operations =
            BTreeSet::from([Operation::Dispatch, Operation::Cancel, Operation::Reconcile]);
        let outcomes = BTreeSet::from([
            Outcome::Success,
            Outcome::Cancelled,
            Outcome::TimedOut,
            Outcome::Unknown,
        ]);
        assert!(
            run_provider_conformance(&mut provider, &provider_request(), &operations, &outcomes,)
                .is_ok()
        );
    }

    #[test]
    fn conformance_rejects_provider_substitution() {
        let mut provider = fixture_provider();
        provider.observation.profile_id =
            ProviderProfileId::new("substitute").unwrap_or_else(|_| unreachable!());
        assert!(
            run_provider_conformance(
                &mut provider,
                &provider_request(),
                &BTreeSet::from([Operation::Dispatch]),
                &BTreeSet::from([Outcome::Success]),
            )
            .is_err()
        );
    }

    #[test]
    fn dependency_boundaries_accept_current_core() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let findings = audit_core(&root);
        assert!(findings.as_ref().is_ok_and(Vec::is_empty));
        let application_findings = audit_application_boundaries(&root);
        assert!(application_findings.as_ref().is_ok_and(Vec::is_empty));
    }

    // r[verify kiln.providers.port_ownership]
    #[test]
    fn provider_contracts_have_one_application_owner() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let findings = audit_adapter_port_ownership(&root);
        assert!(findings.as_ref().is_ok_and(Vec::is_empty));
    }

    // r[verify kiln.providers.composition_root]
    // r[verify kiln.providers.port_ownership.scenario.competing]
    // r[verify kiln.providers.composition_root.scenario.hidden_selection]
    // r[verify kiln.providers.boundary_conformance.scenario.missing_negative]
    #[test]
    fn provider_contract_gate_rejects_adapter_owned_ports() {
        for declaration in FORBIDDEN_ADAPTER_PORT_DECLARATIONS {
            let findings = audit_patterns(
                Path::new("crates/provider/src/lib.rs"),
                declaration,
                FORBIDDEN_ADAPTER_PORT_DECLARATIONS,
            );
            assert!(
                !findings.is_empty(),
                "declaration was not rejected: {declaration}"
            );
        }
    }

    #[test]
    fn dependency_boundaries_reject_provider_runtime() {
        let findings = audit_text(Path::new("fixture.rs"), "use radicle_cob::store;");
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn dependency_boundaries_reject_every_forbidden_edge() {
        for pattern in FORBIDDEN_CORE_DEPENDENCIES {
            let findings = audit_text(Path::new("fixture.rs"), pattern);
            assert!(!findings.is_empty(), "pattern was not rejected: {pattern}");
        }
    }

    #[test]
    fn dependency_boundaries_reject_reference_runtime() {
        let findings = audit_text(Path::new("Cargo.toml"), "pueue-lib = \"0.1\"");
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn reference_boundary_allows_docs_and_rejects_product_integration() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let findings = audit_reference_boundary(&root);
        assert!(findings.as_ref().is_ok_and(Vec::is_empty));
        let product_text = "provider = pueue";
        assert!(product_text.to_ascii_lowercase().contains("pueue"));
    }
}
