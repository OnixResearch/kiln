//! Isolated evidence from an operator-installed, pinned Patchgate executable.
//!
//! The only invocation is selected `review --dry-run --json`; this crate neither
//! links Patchgate/Pijul nor grants source mutation or merge authority. The linked
//! bounded-exec runtime is AGPL-3.0-or-later. Executable provenance and runtime
//! closure remain operator responsibilities.

mod mapping;
mod profile;
mod runtime;

use std::sync::atomic::{AtomicBool, Ordering};

use bounded_exec::Disposition;
use kiln_core::provider::{CapabilityProfile, Operation};
use kiln_core::review::{ReviewEffectPlan, ReviewEvaluationObservation};
use kiln_core::types::Diagnostic;
use kiln_provider_api::{InfrastructureFailure, PortFailure, PortResult, ReviewPort};

pub use profile::{
    BOUNDED_EXEC_LICENSE, ConfigurationEvidence, ConfigurationValues, MainBinding,
    NATIVE_REPORT_SCHEMA, NativeBinding, PINNED_SOURCE_REVISION, PatchgateLimits, PatchgateProfile,
    PolicyBinding, candidate_root, configuration_identity, native_binding_identity,
    policy_identity,
};

#[derive(Debug)]
pub struct PatchgateAdapter {
    profile: PatchgateProfile,
    raw_report: Option<Vec<u8>>,
}

impl PatchgateAdapter {
    /// Validates declarations without dispatch or inspecting the native database.
    /// Executable identity is checked immediately before each fresh review, not
    /// on retained-report reconciliation.
    pub fn new(profile: PatchgateProfile) -> PortResult<Self> {
        profile::validate_profile(&profile)?;
        Ok(Self {
            profile,
            raw_report: None,
        })
    }

    #[must_use]
    pub fn profile_config(&self) -> &PatchgateProfile {
        &self.profile
    }

    /// Exact bounded stdout bytes from the last dispatched review, including a
    /// failed admission. Reconciliation uses the caller's bytes without caching.
    #[must_use]
    pub fn raw_report(&self) -> Option<&[u8]> {
        self.raw_report.as_deref()
    }

    /// Pure request admission; does not inspect an executable or repository.
    pub fn validate_plan(&self, plan: &ReviewEffectPlan) -> PortResult<()> {
        mapping::validate_plan(&self.profile, plan, Operation::Observe)
    }

    pub fn review(&mut self, plan: &ReviewEffectPlan) -> PortResult<ReviewEvaluationObservation> {
        self.review_inner(plan, None)
    }

    pub fn review_with_cancellation(
        &mut self,
        plan: &ReviewEffectPlan,
        cancellation: &AtomicBool,
    ) -> PortResult<ReviewEvaluationObservation> {
        self.review_inner(plan, Some(cancellation))
    }

    pub fn reconcile(
        &self,
        plan: &ReviewEffectPlan,
        prior: &ReviewEvaluationObservation,
        report: &[u8],
    ) -> PortResult<ReviewEvaluationObservation> {
        mapping::validate_plan(&self.profile, plan, Operation::Reconcile)?;
        kiln_core::review::admit_review_observation(
            plan,
            prior,
            self.profile.limits.max_changes,
            self.profile.limits.max_edges,
        )
        .map_err(PortFailure::Domain)?;
        let observation = mapping::admit_report(&self.profile, plan, report)?;
        if observation != *prior {
            return Err(domain(
                "patchgate_receipt_observation",
                "Retained native report differs from its prior normalized receipt",
            ));
        }
        Ok(observation)
    }

    // r[impl kiln.patchgate.execution]
    fn review_inner(
        &mut self,
        plan: &ReviewEffectPlan,
        cancellation: Option<&AtomicBool>,
    ) -> PortResult<ReviewEvaluationObservation> {
        self.raw_report = None;
        mapping::validate_plan(&self.profile, plan, Operation::Observe)?;
        if cancellation.is_some_and(|flag| flag.load(Ordering::Acquire)) {
            return Err(PortFailure::Cancelled(failure(
                "patchgate_cancelled",
                "Review cancelled before dispatch",
            )));
        }
        runtime::verify_executable(&self.profile)?;
        let output = runtime::execute(runtime::invocation(&self.profile, plan), cancellation)?;
        self.raw_report = Some(output.stdout);
        match output.disposition {
            Disposition::TimedOut => {
                return Err(PortFailure::TimedOut(failure(
                    "patchgate_timeout",
                    "Patchgate exceeded its process deadline",
                )));
            }
            Disposition::Cancelled => {
                return Err(PortFailure::Cancelled(failure(
                    "patchgate_cancelled",
                    "Patchgate review was cancelled",
                )));
            }
            Disposition::OutputLimitExceeded(_) => {
                return Err(PortFailure::Infrastructure(failure(
                    "patchgate_output_limit",
                    "Patchgate exceeded its stdout or stderr capture bound",
                )));
            }
            Disposition::ExitFailed => {
                return Err(PortFailure::Infrastructure(failure(
                    "patchgate_process_exit",
                    "Patchgate process failed; process failure is not a review rejection",
                )));
            }
            Disposition::Succeeded => {}
        }
        if output.status != Some(runtime::SUCCESS_EXIT_CODE) {
            return Err(PortFailure::Infrastructure(failure(
                "patchgate_process_exit",
                "Patchgate did not report a normal successful process exit",
            )));
        }
        let report = self.raw_report.as_deref().ok_or_else(|| {
            domain(
                "patchgate_report_missing",
                "Successful process returned no retained report",
            )
        })?;
        mapping::admit_report(&self.profile, plan, report)
    }
}

impl ReviewPort for PatchgateAdapter {
    fn profile(&self) -> &CapabilityProfile {
        &self.profile.provider
    }

    fn review(&mut self, plan: &ReviewEffectPlan) -> PortResult<ReviewEvaluationObservation> {
        PatchgateAdapter::review(self, plan)
    }

    fn review_with_cancellation(
        &mut self,
        plan: &ReviewEffectPlan,
        cancellation: &AtomicBool,
    ) -> PortResult<ReviewEvaluationObservation> {
        PatchgateAdapter::review_with_cancellation(self, plan, cancellation)
    }

    fn reconcile(
        &self,
        plan: &ReviewEffectPlan,
        prior: &ReviewEvaluationObservation,
        report: &[u8],
    ) -> PortResult<ReviewEvaluationObservation> {
        PatchgateAdapter::reconcile(self, plan, prior, report)
    }
}

fn domain(code: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, "patchgate", message))
}

fn failure(code: &str, message: &str) -> InfrastructureFailure {
    InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests;
