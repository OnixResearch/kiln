//! Mantle's cache-only preserve route as a second, separate cohort.
//!
//! The build cohort stays pinned to its own revision, schemas and profile. This
//! module admits exactly one additional operation, `foreign-import realize`,
//! over one typed profile. It verifies the frozen inputs, runs one bounded
//! command and admits a closed-world realization receipt. It performs no
//! realization correctness or cache-truth claim of its own.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use bounded_exec::ExecutionLimits;
use kiln_core::build::{
    BuildCacheKind, BuildEffectPlan, BuildEvaluationObservation, BuildEvaluationOutcome,
    BuildProductObservation, admit_build_evaluation,
};
use kiln_core::domain::Outcome;
use kiln_core::provider::{
    CapabilityProfile, Operation, OperationRequest, ProviderLimits, ProviderRole, admit_operation,
};
use kiln_core::types::{
    CONTRACT_VERSION_V1, ContentId, Diagnostic, EffectId, ExternalRef, ProviderProfileId,
};
use kiln_provider_api::{BuildPort, PortFailure, PortResult, role_matches_port};
use serde::{Deserialize, Serialize};

use crate::runtime::{MantleProcessOutput, MantleRuntime, ProcessInvocation};

// r[impl kiln.mantle.foreign_realize.profile]
pub const PINNED_REALIZE_MANTLE_SOURCE: &str = "ssh://git@github.com/OnixResearch/mantle.git";
pub const PINNED_REALIZE_MANTLE_REVISION: &str = "ead7b8dac087fad0170daca01356afbb3b43b179";
pub const PINNED_REALIZE_MANTLE_CONTRACT_LICENSE: &str = "MIT OR Apache-2.0";
pub const PINNED_REALIZE_MANTLE_CLI_LICENSE: &str = "AGPL-3.0-or-later";
pub const PINNED_REALIZE_MANTLE_PLAN_SCHEMA: &str = "mantle-foreign-executable-plan-v1";
pub const PINNED_REALIZE_MANTLE_IMPORT_SCHEMA: &str = "foreign-derivation-import-receipt-v1";
pub const PINNED_REALIZE_MANTLE_RECEIPT_SCHEMA: &str = "mantle-foreign-realization-receipt-v1";
pub const PINNED_REALIZE_MANTLE_COMMAND: &str = "foreign-import realize";
pub const PINNED_REALIZE_MANTLE_PLATFORM: &str = "x86_64-linux";
pub const PINNED_REALIZE_STORE_PREFIX: &str = "/nix/store";
pub const PINNED_REALIZE_MANTLE_EXECUTABLE_PATH: &str =
    "/nix/store/PLACEHOLDER-mantle-0.1.0/bin/mantle";
pub const PINNED_REALIZE_MANTLE_EXECUTABLE_BLAKE3: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";
pub const PINNED_REALIZE_BOUNDED_EXEC_SOURCE: &str =
    "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git";
pub const PINNED_REALIZE_BOUNDED_EXEC_REVISION: &str = "29dac88ecded94457572db3fdfaaaab95fa91525";
pub const PINNED_REALIZE_BOUNDED_EXEC_LICENSE: &str = "AGPL-3.0-or-later";

pub const MAXIMUM_REALIZE_REQUIRED_PRODUCTS: usize = 64;
pub const MAXIMUM_REALIZE_PRODUCTS: usize = 64;
pub const MAXIMUM_REALIZE_BINDINGS: usize = 256;
pub const MAXIMUM_REALIZE_ROOTS: usize = 1;
pub const MAXIMUM_REALIZE_IDENTITY_LENGTH: usize = 64;

const REALIZE_REQUEST_IDENTITY_DOMAIN: &str = "kiln.mantle.realize-request.v1";
const REALIZE_RECEIPT_IDENTITY_DOMAIN: &str = "kiln.mantle.foreign-realization-receipt.v1";
const REALIZE_ROOT_IDENTITY_DOMAIN: &str = "kiln.mantle.foreign-realization-root.v1";
const REALIZE_STORE_IDENTITY_DOMAIN: &str = "kiln.mantle.foreign-realization-store.v1";
const REALIZE_WORKER_IDENTITY_DOMAIN: &str = "kiln.mantle.foreign-realization-worker.v1";
const SHELL_RECEIPT_DOMAIN: &str = "kiln.mantle.realize-shell-receipt.v1";
const RECEIPT_DIGEST_DOMAIN: &[u8] = b"mantle-foreign-realization-receipt-v1\0";
const RECEIPT_FILENAME: &str = "realization-receipt.json";
const SUCCESS_PROCESS_STATUS: i32 = 0;

// r[impl kiln.mantle.foreign_realize.nonclaims]
pub const REALIZE_NON_CLAIMS: [&str; 5] = [
    "realization observation does not prove realization correctness",
    "realization observation does not prove cache truth",
    "realization observation does not prove signature or key authority beyond Mantle's checks",
    "product admission does not prove product semantics",
    "realization observation does not establish release readiness",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MantleRealizeLimits {
    pub max_required_products: usize,
    pub max_products: usize,
    pub max_bindings: usize,
    pub max_roots: usize,
    pub max_request_bytes: usize,
    pub max_result_bytes: usize,
    pub max_stderr_bytes: usize,
    pub max_receipt_bytes: usize,
    pub max_input_bytes: u64,
    pub max_executable_bytes: u64,
    pub max_in_flight: usize,
    pub stdin_max_bytes: usize,
    pub timeout_milliseconds: u64,
    pub poll_interval_milliseconds: u64,
    pub teardown_timeout_milliseconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MantleRealizeRuntimeBinding {
    pub executable_path: PathBuf,
    pub executable_identity: ContentId,
    pub store_prefix: PathBuf,
    pub store_path: PathBuf,
    pub state_path: PathBuf,
}

/// Every frozen input that is not in the effect plan, bound by BLAKE3 identity
/// and keyed by the plan identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MantleRealizeBinding {
    pub plan_path: PathBuf,
    /// BLAKE3 of the plan file bytes, verified before dispatch.
    pub plan_file_identity: ContentId,
    /// The plan's own `plan_identity.value`, bound in the receipt.
    pub plan_identity: ContentId,
    pub import_receipt_path: PathBuf,
    /// BLAKE3 of the import-receipt file bytes, verified before dispatch.
    pub import_receipt_file_identity: ContentId,
    /// The import receipt's `receipt_digest_blake3`, bound in the receipt.
    pub import_receipt_identity: ContentId,
    pub source_bundle_path: PathBuf,
    /// BLAKE3 of the source-bundle file bytes, verified before dispatch.
    pub source_bundle_file_identity: ContentId,
    /// The `--source-bundle-blake3` manifest identity.
    pub source_bundle_manifest_identity: ContentId,
    pub execution_profile_path: PathBuf,
    pub execution_profile_file_identity: ContentId,
    pub cache_closure_policy_path: PathBuf,
    /// BLAKE3 of the policy file bytes, verified before dispatch.
    pub cache_closure_policy_file_identity: ContentId,
    /// The cache-closure policy identity bound in the receipt.
    pub cache_closure_policy_identity: ContentId,
    pub remote_binding_config_path: PathBuf,
    pub remote_binding_config_file_identity: ContentId,
    /// Explicit product-name to selected-root-path map. Never inferred.
    pub product_roots: BTreeMap<String, PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MantleRealizeProfile {
    pub source: String,
    pub revision: String,
    pub contract_license: String,
    pub cli_license: String,
    pub bounded_exec_source: String,
    pub bounded_exec_revision: String,
    pub bounded_exec_license: String,
    pub plan_schema: String,
    pub import_receipt_schema: String,
    pub receipt_schema: String,
    pub command: String,
    pub platform: String,
    pub provider: CapabilityProfile,
    pub runtime: MantleRealizeRuntimeBinding,
    pub limits: MantleRealizeLimits,
    pub required_non_claims: Vec<String>,
    pub bindings: Vec<MantleRealizeBinding>,
}

impl MantleRealizeProfile {
    pub fn pinned(
        profile_id: ProviderProfileId,
        runtime: MantleRealizeRuntimeBinding,
        limits: MantleRealizeLimits,
        bindings: Vec<MantleRealizeBinding>,
    ) -> PortResult<Self> {
        Ok(Self {
            source: PINNED_REALIZE_MANTLE_SOURCE.to_string(),
            revision: PINNED_REALIZE_MANTLE_REVISION.to_string(),
            contract_license: PINNED_REALIZE_MANTLE_CONTRACT_LICENSE.to_string(),
            cli_license: PINNED_REALIZE_MANTLE_CLI_LICENSE.to_string(),
            bounded_exec_source: PINNED_REALIZE_BOUNDED_EXEC_SOURCE.to_string(),
            bounded_exec_revision: PINNED_REALIZE_BOUNDED_EXEC_REVISION.to_string(),
            bounded_exec_license: PINNED_REALIZE_BOUNDED_EXEC_LICENSE.to_string(),
            plan_schema: PINNED_REALIZE_MANTLE_PLAN_SCHEMA.to_string(),
            import_receipt_schema: PINNED_REALIZE_MANTLE_IMPORT_SCHEMA.to_string(),
            receipt_schema: PINNED_REALIZE_MANTLE_RECEIPT_SCHEMA.to_string(),
            command: PINNED_REALIZE_MANTLE_COMMAND.to_string(),
            platform: PINNED_REALIZE_MANTLE_PLATFORM.to_string(),
            provider: CapabilityProfile {
                contract_version: CONTRACT_VERSION_V1,
                profile_id,
                role: ProviderRole::Build,
                namespace: ExternalRef::new(
                    "mantle",
                    "foreign-realize-provider",
                    PINNED_REALIZE_MANTLE_REVISION,
                )?,
                operations: required_operations(),
                outcomes: required_outcomes(),
                limits: ProviderLimits {
                    max_request_bytes: limits.max_request_bytes,
                    max_result_bytes: limits.max_result_bytes,
                    max_in_flight: limits.max_in_flight,
                },
            },
            runtime,
            limits,
            required_non_claims: REALIZE_NON_CLAIMS
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            bindings,
        })
    }

    fn binding_for(&self, plan: &BuildEffectPlan) -> PortResult<&MantleRealizeBinding> {
        if plan.provider_profile != self.provider.profile_id {
            return Err(domain(
                "mantle_realize_request_profile",
                "mantle.realize.provider_profile",
                "realize plan names another provider profile",
            ));
        }
        if plan.platform.as_str() != self.platform {
            return Err(domain(
                "mantle_realize_request_platform",
                "mantle.realize.platform",
                "realize plan names another platform",
            ));
        }
        let mut matching = self
            .bindings
            .iter()
            .filter(|binding| binding.plan_identity == plan.plan_identity);
        let binding = matching.next().ok_or_else(|| {
            domain(
                "mantle_realize_binding_missing",
                "mantle.realize.plan_identity",
                "realize profile binds no input for this plan identity",
            )
        })?;
        if matching.next().is_some() {
            return Err(domain(
                "mantle_realize_binding_duplicate",
                "mantle.realize.plan_identity",
                "realize profile binds the same plan identity twice",
            ));
        }
        for product in &plan.required_products {
            if !binding.product_roots.contains_key(product.as_str()) {
                return Err(domain(
                    "mantle_realize_product_unmapped",
                    "mantle.realize.product_roots",
                    "a required product has no explicit selected-root mapping",
                ));
            }
        }
        Ok(binding)
    }
}

// r[impl kiln.mantle.foreign_realize.profile]
pub fn admit_realize_profile(profile: &MantleRealizeProfile) -> PortResult<()> {
    if profile.source != PINNED_REALIZE_MANTLE_SOURCE
        || profile.revision != PINNED_REALIZE_MANTLE_REVISION
        || profile.contract_license != PINNED_REALIZE_MANTLE_CONTRACT_LICENSE
        || profile.cli_license != PINNED_REALIZE_MANTLE_CLI_LICENSE
        || profile.bounded_exec_source != PINNED_REALIZE_BOUNDED_EXEC_SOURCE
        || profile.bounded_exec_revision != PINNED_REALIZE_BOUNDED_EXEC_REVISION
        || profile.bounded_exec_license != PINNED_REALIZE_BOUNDED_EXEC_LICENSE
        || profile.plan_schema != PINNED_REALIZE_MANTLE_PLAN_SCHEMA
        || profile.import_receipt_schema != PINNED_REALIZE_MANTLE_IMPORT_SCHEMA
        || profile.receipt_schema != PINNED_REALIZE_MANTLE_RECEIPT_SCHEMA
        || profile.command != PINNED_REALIZE_MANTLE_COMMAND
        || profile.platform != PINNED_REALIZE_MANTLE_PLATFORM
    {
        return Err(domain(
            "mantle_realize_profile_cohort",
            "mantle.realize.profile",
            "Mantle realize profile differs from the pinned contract cohort",
        ));
    }
    if profile.runtime.executable_path != PathBuf::from(PINNED_REALIZE_MANTLE_EXECUTABLE_PATH)
        || profile.runtime.executable_identity.as_str() != PINNED_REALIZE_MANTLE_EXECUTABLE_BLAKE3
        || profile.runtime.store_prefix != PathBuf::from(PINNED_REALIZE_STORE_PREFIX)
        || profile.required_non_claims
            != REALIZE_NON_CLAIMS
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>()
        || profile.provider.operations != required_operations()
        || profile.provider.outcomes != required_outcomes()
        || profile.provider.limits
            != (ProviderLimits {
                max_request_bytes: profile.limits.max_request_bytes,
                max_result_bytes: profile.limits.max_result_bytes,
                max_in_flight: profile.limits.max_in_flight,
            })
    {
        return Err(domain(
            "mantle_realize_profile_binding",
            "mantle.realize.profile",
            "Mantle realize profile identities, capabilities, or non-claims differ from the cohort",
        ));
    }
    validate_realize_paths(profile)?;
    validate_realize_limits(profile.limits)?;
    role_matches_port(&profile.provider, ProviderRole::Build)?;
    for operation in &profile.provider.operations {
        admit_operation(
            &profile.provider,
            &OperationRequest {
                contract_version: CONTRACT_VERSION_V1,
                selected_profile: profile.provider.profile_id.clone(),
                role: ProviderRole::Build,
                operation: *operation,
                request_bytes: 0,
                required_outcomes: profile.provider.outcomes.clone(),
            },
        )?;
    }
    validate_bindings(profile)
}

fn validate_realize_paths(profile: &MantleRealizeProfile) -> PortResult<()> {
    let runtime = &profile.runtime;
    if !runtime.executable_path.is_absolute()
        || !runtime.store_prefix.is_absolute()
        || !runtime.store_path.is_absolute()
        || !runtime.state_path.is_absolute()
    {
        return Err(domain(
            "mantle_realize_profile_path",
            "mantle.realize.profile.runtime",
            "Mantle realize executable, store prefix, store and state paths must be absolute",
        ));
    }
    Ok(())
}

fn validate_realize_limits(limits: MantleRealizeLimits) -> PortResult<()> {
    if limits.max_required_products == 0
        || limits.max_required_products > MAXIMUM_REALIZE_REQUIRED_PRODUCTS
        || limits.max_products < limits.max_required_products
        || limits.max_products > MAXIMUM_REALIZE_PRODUCTS
        || limits.max_bindings == 0
        || limits.max_bindings > MAXIMUM_REALIZE_BINDINGS
        || limits.max_roots != MAXIMUM_REALIZE_ROOTS
        || limits.max_request_bytes == 0
        || limits.max_result_bytes == 0
        || limits.max_stderr_bytes == 0
        || limits.max_receipt_bytes == 0
        || limits.max_input_bytes == 0
        || limits.max_executable_bytes == 0
        || limits.max_in_flight == 0
        || limits.stdin_max_bytes == 0
        || limits.timeout_milliseconds == 0
        || limits.poll_interval_milliseconds == 0
        || limits.poll_interval_milliseconds > limits.timeout_milliseconds
        || limits.teardown_timeout_milliseconds == 0
    {
        return Err(domain(
            "mantle_realize_profile_limit",
            "mantle.realize.profile.limits",
            "Mantle realize limits are zero, inconsistent, or exceed the contract",
        ));
    }
    Ok(())
}

fn validate_bindings(profile: &MantleRealizeProfile) -> PortResult<()> {
    if profile.bindings.is_empty() || profile.bindings.len() > profile.limits.max_bindings {
        return Err(domain(
            "mantle_realize_profile_limit",
            "mantle.realize.profile.bindings",
            "Mantle realize binding count is empty or exceeds its named bound",
        ));
    }
    let mut seen = BTreeSet::new();
    for binding in &profile.bindings {
        for path in [
            &binding.plan_path,
            &binding.import_receipt_path,
            &binding.source_bundle_path,
            &binding.execution_profile_path,
            &binding.cache_closure_policy_path,
            &binding.remote_binding_config_path,
        ] {
            if !path.is_absolute() {
                return Err(domain(
                    "mantle_realize_profile_path",
                    "mantle.realize.binding",
                    "Mantle realize input paths must be absolute",
                ));
            }
        }
        for identity in [
            &binding.plan_file_identity,
            &binding.plan_identity,
            &binding.import_receipt_file_identity,
            &binding.import_receipt_identity,
            &binding.source_bundle_file_identity,
            &binding.source_bundle_manifest_identity,
            &binding.execution_profile_file_identity,
            &binding.cache_closure_policy_file_identity,
            &binding.cache_closure_policy_identity,
            &binding.remote_binding_config_file_identity,
        ] {
            identity.validate()?;
        }
        if !seen.insert(binding.plan_identity.as_str()) {
            return Err(domain(
                "mantle_realize_profile_binding",
                "mantle.realize.binding.plan_identity",
                "Mantle realize profile repeats a plan identity",
            ));
        }
        if binding.product_roots.is_empty()
            || binding.product_roots.len() > profile.limits.max_products
        {
            return Err(domain(
                "mantle_realize_profile_product",
                "mantle.realize.binding.product_roots",
                "Mantle realize product map is empty or exceeds its named bound",
            ));
        }
        for (name, path) in &binding.product_roots {
            if name.is_empty()
                || name.len() > crate::wire::MAXIMUM_LABEL_BYTES
                || name.chars().any(char::is_control)
            {
                return Err(domain(
                    "mantle_realize_profile_product",
                    "mantle.realize.binding.product_roots",
                    "Mantle realize product name is empty, oversized or contains controls",
                ));
            }
            if !safe_logical_path(path, &profile.runtime.store_prefix) {
                return Err(domain(
                    "mantle_realize_profile_product",
                    "mantle.realize.binding.product_roots",
                    "Mantle realize product root is not under the logical store prefix",
                ));
            }
        }
    }
    Ok(())
}

fn required_operations() -> BTreeSet<Operation> {
    BTreeSet::from([
        Operation::Dispatch,
        Operation::Realize,
        Operation::Observe,
        Operation::Cancel,
        Operation::Reconcile,
    ])
}

fn required_outcomes() -> BTreeSet<Outcome> {
    BTreeSet::from([
        Outcome::Success,
        Outcome::DomainFailure,
        Outcome::Cancelled,
        Outcome::TimedOut,
        Outcome::ProviderError,
        Outcome::Unknown,
    ])
}

#[derive(Debug)]
struct RealizeRecord {
    plan: BuildEffectPlan,
    observation: BuildEvaluationObservation,
}

#[derive(Debug)]
pub struct MantleRealizeAdapter<R> {
    profile: MantleRealizeProfile,
    runtime: R,
    realizations: BTreeMap<EffectId, RealizeRecord>,
}

impl<R: MantleRuntime> MantleRealizeAdapter<R> {
    pub fn new(profile: MantleRealizeProfile, runtime: R) -> PortResult<Self> {
        admit_realize_profile(&profile)?;
        verify_executable(&profile, &runtime)?;
        Ok(Self {
            profile,
            runtime,
            realizations: BTreeMap::new(),
        })
    }

    #[must_use]
    pub fn runtime(&self) -> &R {
        &self.runtime
    }

    pub fn runtime_mut(&mut self) -> &mut R {
        &mut self.runtime
    }

    #[must_use]
    pub fn into_runtime(self) -> R {
        self.runtime
    }

    pub fn restore(
        &mut self,
        plan: &BuildEffectPlan,
        observation: &BuildEvaluationObservation,
    ) -> PortResult<()> {
        self.profile.binding_for(plan)?;
        admit_build_evaluation(plan, observation)?;
        if let Some(record) = self.realizations.get(&plan.effect_id) {
            if record.plan == *plan && record.observation == *observation {
                return Ok(());
            }
            return Err(domain(
                "mantle_realize_restore_conflict",
                "mantle.realize",
                "Restored Mantle realization conflicts with the recorded request",
            ));
        }
        self.realizations.insert(
            plan.effect_id.clone(),
            RealizeRecord {
                plan: plan.clone(),
                observation: observation.clone(),
            },
        );
        Ok(())
    }

    fn dispatch_inner(
        &mut self,
        plan: &BuildEffectPlan,
        cancellation: Option<&AtomicBool>,
    ) -> PortResult<BuildEvaluationObservation> {
        // r[impl kiln.mantle.foreign_realize.recovery]
        let binding = self.profile.binding_for(plan)?;
        if let Some(record) = self.realizations.get(&plan.effect_id) {
            if record.plan != *plan {
                return Err(domain(
                    "mantle_realize_conflict",
                    "mantle.realize",
                    "Repeated Mantle realization differs from the recorded plan",
                ));
            }
            return Ok(record.observation.clone());
        }
        let unknown_count = self
            .realizations
            .values()
            .filter(|record| record.observation.outcome == BuildEvaluationOutcome::Unknown)
            .count();
        if unknown_count >= self.profile.limits.max_in_flight {
            return Err(PortFailure::Retryable(
                kiln_provider_api::InfrastructureFailure {
                    code: "mantle_realize_in_flight_bound".to_string(),
                    message: "Mantle realize adapter reached its unknown realization bound"
                        .to_string(),
                },
            ));
        }
        verify_runtime(&self.profile, &self.runtime, binding)?;
        let receipt_path = receipt_path(&self.profile, plan)?;
        if let Some(parent) = receipt_path.parent() {
            self.runtime.prepare_directory(parent)?;
        }
        let invocation = invocation(&self.profile, binding, &receipt_path);
        let output = match cancellation {
            Some(cancellation) => self
                .runtime
                .run_with_cancellation(&invocation, cancellation)?,
            None => self.runtime.run(&invocation)?,
        };
        let observation = interpret_output(
            &self.profile,
            plan,
            binding,
            &self.runtime,
            &receipt_path,
            output,
        )?;
        admit_build_evaluation(plan, &observation)?;
        self.realizations.insert(
            plan.effect_id.clone(),
            RealizeRecord {
                plan: plan.clone(),
                observation: observation.clone(),
            },
        );
        Ok(observation)
    }
}

impl<R: MantleRuntime> BuildPort for MantleRealizeAdapter<R> {
    fn profile(&self) -> &CapabilityProfile {
        &self.profile.provider
    }

    fn dispatch(&mut self, plan: &BuildEffectPlan) -> PortResult<BuildEvaluationObservation> {
        self.dispatch_inner(plan, None)
    }

    fn dispatch_with_cancellation(
        &mut self,
        plan: &BuildEffectPlan,
        cancellation: &AtomicBool,
    ) -> PortResult<BuildEvaluationObservation> {
        self.dispatch_inner(plan, Some(cancellation))
    }

    fn observe(&mut self, effect_id: &EffectId) -> PortResult<BuildEvaluationObservation> {
        self.realizations
            .get(effect_id)
            .map(|record| record.observation.clone())
            .ok_or_else(|| {
                domain(
                    "mantle_realize_missing",
                    "mantle.realize",
                    "Mantle realization has no recorded observation",
                )
            })
    }

    fn read_log(
        &mut self,
        _request: &kiln_core::build::BuildLogReadRequest,
    ) -> PortResult<kiln_core::build::BuildLogChunk> {
        Err(domain(
            "mantle_realize_log_missing",
            "mantle.realize.log",
            "Mantle realization binds no provider log identity",
        ))
    }

    fn reconcile(&mut self, plan: &BuildEffectPlan) -> PortResult<BuildEvaluationObservation> {
        let _binding = self.profile.binding_for(plan)?;
        let record = self.realizations.get(&plan.effect_id).ok_or_else(|| {
            domain(
                "mantle_realize_missing",
                "mantle.realize",
                "Mantle realization has no recorded outcome to reconcile",
            )
        })?;
        if record.plan != *plan {
            return Err(domain(
                "mantle_realize_reconciliation_conflict",
                "mantle.realize",
                "Mantle reconciliation differs from the recorded realization plan",
            ));
        }
        Ok(record.observation.clone())
    }
}

fn verify_executable<R: MantleRuntime>(
    profile: &MantleRealizeProfile,
    runtime: &R,
) -> PortResult<()> {
    let executable = runtime.file_identity(
        &profile.runtime.executable_path,
        profile.limits.max_executable_bytes,
    )?;
    if executable != profile.runtime.executable_identity {
        return Err(domain(
            "mantle_realize_executable_identity",
            "mantle.realize.executable",
            "Mantle realize executable identity differs from the admitted profile",
        ));
    }
    Ok(())
}

fn verify_runtime<R: MantleRuntime>(
    profile: &MantleRealizeProfile,
    runtime: &R,
    binding: &MantleRealizeBinding,
) -> PortResult<()> {
    verify_executable(profile, runtime)?;
    for (path, expected, code) in [
        (
            &binding.plan_path,
            &binding.plan_file_identity,
            "mantle_realize_plan_identity",
        ),
        (
            &binding.import_receipt_path,
            &binding.import_receipt_file_identity,
            "mantle_realize_import_receipt_identity",
        ),
        (
            &binding.source_bundle_path,
            &binding.source_bundle_file_identity,
            "mantle_realize_source_bundle_identity",
        ),
        (
            &binding.execution_profile_path,
            &binding.execution_profile_file_identity,
            "mantle_realize_execution_profile_identity",
        ),
        (
            &binding.cache_closure_policy_path,
            &binding.cache_closure_policy_file_identity,
            "mantle_realize_cache_closure_policy_identity",
        ),
        (
            &binding.remote_binding_config_path,
            &binding.remote_binding_config_file_identity,
            "mantle_realize_binding_config_identity",
        ),
    ] {
        let observed = runtime.file_identity(path, profile.limits.max_input_bytes)?;
        if observed != *expected {
            return Err(PortFailure::Domain(Diagnostic::new(
                code,
                "mantle.realize.input",
                "Mantle realize bound input identity differs from the admitted profile",
            )));
        }
    }
    Ok(())
}

fn receipt_path(profile: &MantleRealizeProfile, plan: &BuildEffectPlan) -> PortResult<PathBuf> {
    let attempt = plan.attempt_id.as_str();
    if attempt.is_empty()
        || attempt == "."
        || attempt == ".."
        || attempt.contains('/')
        || attempt.contains('\\')
        || attempt.chars().any(char::is_control)
    {
        return Err(domain(
            "mantle_realize_attempt",
            "mantle.realize.attempt",
            "Mantle realization attempt identity is not one safe path segment",
        ));
    }
    Ok(profile
        .runtime
        .state_path
        .join(attempt)
        .join(RECEIPT_FILENAME))
}

fn invocation(
    profile: &MantleRealizeProfile,
    binding: &MantleRealizeBinding,
    receipt_path: &Path,
) -> ProcessInvocation {
    let current_dir = binding
        .plan_path
        .parent()
        .map_or_else(|| PathBuf::from("/"), PathBuf::from);
    let path_directory = profile
        .runtime
        .executable_path
        .parent()
        .map_or_else(|| PathBuf::from("/"), PathBuf::from);
    ProcessInvocation {
        program: profile.runtime.executable_path.clone(),
        args: vec![
            OsString::from("--json"),
            OsString::from("--state-dir"),
            profile.runtime.state_path.clone().into_os_string(),
            OsString::from("--store"),
            profile.runtime.store_path.clone().into_os_string(),
            OsString::from("--store-prefix"),
            profile.runtime.store_prefix.clone().into_os_string(),
            OsString::from("--remote-binding-config"),
            binding.remote_binding_config_path.clone().into_os_string(),
            OsString::from("foreign-import"),
            OsString::from("realize"),
            OsString::from("--plan"),
            binding.plan_path.clone().into_os_string(),
            OsString::from("--import-receipt"),
            binding.import_receipt_path.clone().into_os_string(),
            OsString::from("--source-bundle"),
            binding.source_bundle_path.clone().into_os_string(),
            OsString::from("--source-bundle-blake3"),
            OsString::from(binding.source_bundle_manifest_identity.as_str()),
            OsString::from("--execution-profile"),
            binding.execution_profile_path.clone().into_os_string(),
            OsString::from("--cache-closure-policy"),
            binding.cache_closure_policy_path.clone().into_os_string(),
            OsString::from("--receipt-out"),
            receipt_path.to_path_buf().into_os_string(),
            OsString::from("--substitute"),
        ],
        current_dir,
        environment: vec![(OsString::from("PATH"), path_directory.into_os_string())],
        limits: ExecutionLimits {
            timeout_ms: profile.limits.timeout_milliseconds,
            stdin_max_bytes: profile.limits.stdin_max_bytes,
            stdout_max_bytes: profile.limits.max_result_bytes,
            stderr_max_bytes: profile.limits.max_stderr_bytes,
            poll_interval_ms: profile.limits.poll_interval_milliseconds,
            teardown_timeout_ms: profile.limits.teardown_timeout_milliseconds,
        },
    }
}

fn interpret_output<R: MantleRuntime>(
    profile: &MantleRealizeProfile,
    plan: &BuildEffectPlan,
    binding: &MantleRealizeBinding,
    runtime: &R,
    receipt_path: &Path,
    output: MantleProcessOutput,
) -> PortResult<BuildEvaluationObservation> {
    match output {
        MantleProcessOutput::Exited {
            process_status,
            stdout: _,
            stderr: _,
        } => {
            let Some(process_status) = process_status else {
                return shell_outcome(profile, plan, binding, BuildEvaluationOutcome::Unknown);
            };
            let receipt = runtime.read_bounded_file(receipt_path, profile.limits.max_receipt_bytes);
            match receipt {
                Ok(bytes) => match admit_realization_receipt(profile, plan, binding, &bytes) {
                    Ok(observation)
                        if process_status == SUCCESS_PROCESS_STATUS
                            || observation.outcome == BuildEvaluationOutcome::Failed =>
                    {
                        Ok(observation)
                    }
                    Ok(_) | Err(_) => shell_outcome(
                        profile,
                        plan,
                        binding,
                        BuildEvaluationOutcome::ProviderError,
                    ),
                },
                Err(_) => shell_outcome(
                    profile,
                    plan,
                    binding,
                    BuildEvaluationOutcome::ProviderError,
                ),
            }
        }
        MantleProcessOutput::TimedOut => {
            shell_outcome(profile, plan, binding, BuildEvaluationOutcome::TimedOut)
        }
        MantleProcessOutput::Cancelled => {
            shell_outcome(profile, plan, binding, BuildEvaluationOutcome::Cancelled)
        }
        MantleProcessOutput::OutputLimitExceeded => shell_outcome(
            profile,
            plan,
            binding,
            BuildEvaluationOutcome::ProviderError,
        ),
        MantleProcessOutput::Unknown => {
            shell_outcome(profile, plan, binding, BuildEvaluationOutcome::Unknown)
        }
    }
}

// r[impl kiln.mantle.foreign_realize.completion]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RealizationReceipt {
    schema: String,
    status: String,
    strongest_state: String,
    receipt_blake3: String,
    plan_blake3: String,
    import_receipt_blake3: String,
    source_bundle_manifest_blake3: String,
    build_report_blake3: String,
    selected_root_node_ids: Vec<String>,
    selected_root_paths: Vec<String>,
    execution_profiles: Vec<RealizedExecutionProfile>,
    cache_policy: RealizedCachePolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cache_closure_policy_blake3: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    cache_closure: Vec<ForeignCacheClosureFact>,
    sources: Vec<ForeignSourceStoreFact>,
    units: Vec<ForeignRealizedUnit>,
    failure: Option<ForeignRealizationFailure>,
    non_claims: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RealizedExecutionProfile {
    profile_id: String,
    digest_blake3: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RealizedCachePolicy {
    substitution_enabled: bool,
    offline: bool,
    ordered_cache_urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    realization_route: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ForeignCacheClosureFact {
    logical_path: String,
    nar_sha256: String,
    nar_size: u64,
    references: Vec<String>,
    signature_names: Vec<String>,
    disposition: String,
    depth: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ForeignSourceStoreFact {
    payload_id: String,
    foreign_path: String,
    target_path: String,
    descriptor_blake3: String,
    source_record_identity: String,
    source_content_blake3: String,
    nar_sha256: String,
    nar_size: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ForeignRealizedUnit {
    sequence: usize,
    node_id: String,
    target_derivation: String,
    execution_profile_id: String,
    execution_profile_digest_blake3: String,
    execution_class: String,
    fetch_attempts: Vec<ForeignFetchAttempt>,
    outputs: Vec<ForeignRealizedOutput>,
    failure: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ForeignRealizedOutput {
    output_name: String,
    target_path: String,
    nar_sha256: String,
    nar_size: u64,
    substitution_mode: Option<String>,
    substitution_transferred_bytes: Option<u64>,
    substitution_reused_bytes: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ForeignRealizationFailure {
    class: String,
    message: String,
    affected_roots: Vec<String>,
    completed_unit_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ForeignFetchAttempt {
    index: usize,
    candidate: String,
    classification: String,
}

/// Admit one bounded realization receipt against the exact plan and binding.
///
/// The function is pure: it decides from the supplied bytes. A successful
/// receipt returns a terminal success observation; a partial-failure receipt
/// returns a terminal `Failed` observation; every drift or identity mismatch is
/// refused with a stable code.
pub fn admit_realization_receipt(
    profile: &MantleRealizeProfile,
    plan: &BuildEffectPlan,
    binding: &MantleRealizeBinding,
    bytes: &[u8],
) -> PortResult<BuildEvaluationObservation> {
    if bytes.len() > profile.limits.max_receipt_bytes {
        return Err(domain(
            "mantle_realize_receipt_bound",
            "mantle.realize.receipt",
            "Mantle realization receipt exceeds its named byte bound",
        ));
    }
    let receipt: RealizationReceipt = serde_json::from_slice(bytes).map_err(|_| {
        domain(
            "mantle_realize_receipt_parse",
            "mantle.realize.receipt",
            "Mantle realization receipt is not valid closed-world JSON",
        )
    })?;
    if receipt.schema != PINNED_REALIZE_MANTLE_RECEIPT_SCHEMA {
        return Err(domain(
            "mantle_realize_receipt_schema",
            "mantle.realize.receipt.schema",
            "Mantle realization receipt names another schema",
        ));
    }
    let expected_receipt_digest = receipt_digest(&receipt)?;
    if expected_receipt_digest != receipt.receipt_blake3 {
        return Err(domain(
            "mantle_realize_receipt_digest",
            "mantle.realize.receipt.receipt_blake3",
            "Mantle realization receipt digest does not recompute",
        ));
    }
    let receipt_identity = tagged_identity(REALIZE_RECEIPT_IDENTITY_DOMAIN, &[bytes])?;
    if receipt.plan_blake3 != binding.plan_identity.as_str() {
        return Err(domain(
            "mantle_realize_receipt_plan",
            "mantle.realize.receipt.plan_blake3",
            "Mantle realization receipt names another plan identity",
        ));
    }
    if receipt.import_receipt_blake3 != binding.import_receipt_identity.as_str() {
        return Err(domain(
            "mantle_realize_receipt_import",
            "mantle.realize.receipt.import_receipt_blake3",
            "Mantle realization receipt names another import receipt",
        ));
    }
    if receipt.source_bundle_manifest_blake3 != binding.source_bundle_manifest_identity.as_str() {
        return Err(domain(
            "mantle_realize_receipt_source_bundle",
            "mantle.realize.receipt.source_bundle_manifest_blake3",
            "Mantle realization receipt names another source bundle manifest",
        ));
    }
    if receipt.cache_closure_policy_blake3.as_deref()
        != Some(binding.cache_closure_policy_identity.as_str())
    {
        return Err(domain(
            "mantle_realize_receipt_policy",
            "mantle.realize.receipt.cache_closure_policy_blake3",
            "Mantle realization receipt names another cache-closure policy",
        ));
    }
    let failed = receipt.failure.is_some()
        || receipt.status != "complete"
        || receipt.strongest_state != "realized";
    if receipt.failure.is_some() {
        return observation(
            profile,
            plan,
            receipt_identity,
            BuildEvaluationOutcome::Failed,
            None,
        );
    }
    if failed {
        return Err(domain(
            "mantle_realize_receipt_status",
            "mantle.realize.receipt.status",
            "Mantle realization receipt is not complete and realized",
        ));
    }
    if receipt.selected_root_paths.len() != profile.limits.max_roots
        || receipt.selected_root_node_ids.len() != profile.limits.max_roots
    {
        return Err(domain(
            "mantle_realize_receipt_root_count",
            "mantle.realize.receipt.selected_root_paths",
            "Mantle realization receipt does not select exactly one root",
        ));
    }
    let Some(root) = receipt.selected_root_paths.first() else {
        return Err(domain(
            "mantle_realize_receipt_root_count",
            "mantle.realize.receipt.selected_root_paths",
            "Mantle realization receipt does not select exactly one root",
        ));
    };
    if !safe_logical_path(Path::new(root), &profile.runtime.store_prefix) {
        return Err(domain(
            "mantle_realize_receipt_root_prefix",
            "mantle.realize.receipt.selected_root_paths",
            "Mantle realization root is not under the logical store prefix",
        ));
    }
    for product in &plan.required_products {
        let Some(mapped) = binding.product_roots.get(product.as_str()) else {
            return Err(domain(
                "mantle_realize_receipt_product",
                "mantle.realize.receipt.products",
                "a required product has no explicit selected-root mapping",
            ));
        };
        if mapped != Path::new(root) {
            return Err(domain(
                "mantle_realize_receipt_product",
                "mantle.realize.receipt.products",
                "a required product maps to another selected root",
            ));
        }
    }
    observation(
        profile,
        plan,
        receipt_identity,
        BuildEvaluationOutcome::Success,
        Some(root.as_str()),
    )
}

fn observation(
    profile: &MantleRealizeProfile,
    plan: &BuildEffectPlan,
    receipt_identity: ContentId,
    outcome: BuildEvaluationOutcome,
    selected_root: Option<&str>,
) -> PortResult<BuildEvaluationObservation> {
    let builder_identity = profile.runtime.executable_identity.clone();
    let worker_identity = tagged_identity(
        REALIZE_WORKER_IDENTITY_DOMAIN,
        &[
            profile.runtime.executable_identity.as_str().as_bytes(),
            profile.platform.as_bytes(),
        ],
    )?;
    let store_identity = tagged_identity(
        REALIZE_STORE_IDENTITY_DOMAIN,
        &[
            profile.runtime.store_path.to_string_lossy().as_bytes(),
            profile.runtime.store_prefix.to_string_lossy().as_bytes(),
        ],
    )?;
    let products = match (outcome, selected_root) {
        (BuildEvaluationOutcome::Success, Some(root)) => plan
            .required_products
            .iter()
            .map(|product| {
                Ok(BuildProductObservation {
                    name: product.clone(),
                    artifact_identity: tagged_identity(
                        REALIZE_ROOT_IDENTITY_DOMAIN,
                        &[receipt_identity.as_str().as_bytes(), root.as_bytes()],
                    )?,
                    store_identity: tagged_identity(
                        REALIZE_STORE_IDENTITY_DOMAIN,
                        &[root.as_bytes()],
                    )?,
                    store: None,
                })
            })
            .collect::<PortResult<Vec<_>>>()?,
        _ => Vec::new(),
    };
    let observation = BuildEvaluationObservation {
        effect_id: plan.effect_id.clone(),
        attempt_id: plan.attempt_id.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        candidate_id: plan.candidate_id.clone(),
        pipeline_revision: plan.pipeline_revision.clone(),
        plan_identity: plan.plan_identity.clone(),
        policy_identity: plan.policy_identity.clone(),
        provider_profile: plan.provider_profile.clone(),
        platform: plan.platform.clone(),
        outcome,
        constraints: None,
        inputs: Vec::new(),
        input_closure_identity: None,
        input_closure_complete: true,
        derivation_identity: None,
        products,
        builder_identity,
        worker_identity,
        store_identity,
        cache_kind: BuildCacheKind::Substitution,
        cache_source_identity: Some(receipt_identity.clone()),
        log_identities: Vec::new(),
        metrics: Vec::new(),
        rebuild: Vec::new(),
        receipt_identity: Some(receipt_identity),
    };
    Ok(observation)
}

fn shell_outcome(
    profile: &MantleRealizeProfile,
    plan: &BuildEffectPlan,
    binding: &MantleRealizeBinding,
    outcome: BuildEvaluationOutcome,
) -> PortResult<BuildEvaluationObservation> {
    if outcome == BuildEvaluationOutcome::Success {
        return Err(domain(
            "mantle_realize_shell_success",
            "mantle.realize.shell",
            "Mantle realization success requires one admitted receipt",
        ));
    }
    let request_identity = tagged_identity(
        REALIZE_REQUEST_IDENTITY_DOMAIN,
        &[
            plan.effect_id.as_str().as_bytes(),
            plan.attempt_id.as_str().as_bytes(),
            plan.plan_identity.as_str().as_bytes(),
            binding.plan_path.to_string_lossy().as_bytes(),
        ],
    )?;
    let receipt_identity = if outcome == BuildEvaluationOutcome::Unknown {
        None
    } else {
        Some(tagged_identity(
            SHELL_RECEIPT_DOMAIN,
            &[
                request_identity.as_str().as_bytes(),
                outcome_name(outcome).as_bytes(),
            ],
        )?)
    };
    let mut observation = observation(profile, plan, request_identity, outcome, None)?;
    observation.receipt_identity = receipt_identity;
    observation.cache_kind = BuildCacheKind::None;
    observation.cache_source_identity = None;
    Ok(observation)
}

const fn outcome_name(outcome: BuildEvaluationOutcome) -> &'static str {
    match outcome {
        BuildEvaluationOutcome::Success => "success",
        BuildEvaluationOutcome::Failed => "failed",
        BuildEvaluationOutcome::Cancelled => "cancelled",
        BuildEvaluationOutcome::TimedOut => "timed_out",
        BuildEvaluationOutcome::ProviderError => "provider_error",
        BuildEvaluationOutcome::Unknown => "unknown",
    }
}

fn receipt_digest(receipt: &RealizationReceipt) -> PortResult<String> {
    let mut material = receipt.clone();
    material.receipt_blake3.clear();
    let canonical = serde_json::to_vec(&material).map_err(|_| {
        infrastructure(
            "mantle_realize_receipt_encoding",
            "Mantle realization receipt could not be canonically encoded",
        )
    })?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(RECEIPT_DIGEST_DOMAIN);
    hasher.update(&canonical);
    Ok(hasher.finalize().to_hex().to_string())
}

fn tagged_identity(domain: &str, parts: &[&[u8]]) -> PortResult<ContentId> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain.as_bytes());
    for part in parts {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    ContentId::from_blake3_hex(hasher.finalize().to_hex().to_string()).map_err(PortFailure::Domain)
}

fn safe_logical_path(path: &Path, prefix: &Path) -> bool {
    let Some(text) = path.to_str() else {
        return false;
    };
    if !path.is_absolute() || !path.starts_with(prefix) || text.contains('\0') {
        return false;
    }
    text.split('/')
        .skip(1)
        .all(|segment| segment != "." && segment != "..")
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}

fn infrastructure(code: &str, message: &str) -> PortFailure {
    PortFailure::Infrastructure(kiln_provider_api::InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    })
}

#[cfg(test)]
mod tests;
