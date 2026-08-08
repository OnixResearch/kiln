use kiln_core::cob::{
    CobCapability, CobEffectPlan, CobObservation, CobProfile, INITIAL_CAIRN_TYPES,
    admit_resolution, plan_effect, select_profile,
};
use kiln_core::types::{CoreResult, Diagnostic, ExternalRef};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReducedProviderObject {
    pub observation: CobObservation,
    pub payload_bytes: usize,
    pub signature_observed: bool,
    pub signature_valid: bool,
}

pub fn map_read_only<'a>(
    profiles: &'a [CobProfile],
    object: &ReducedProviderObject,
) -> CoreResult<(&'a CobProfile, CobObservation)> {
    let profile = select_profile(profiles, &object.observation, CobCapability::TypedRead)?;
    if object.payload_bytes > profile.max_payload_bytes {
        return Err(Diagnostic::new(
            "cob_payload_bound",
            "cob.payload",
            "COB payload exceeds its named bound",
        ));
    }
    if object.signature_observed && !object.signature_valid {
        return Err(Diagnostic::new(
            "cob_signature_invalid",
            "cob.signature",
            "observed COB signature is invalid",
        ));
    }
    Ok((profile, object.observation.clone()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CobAdapterKind {
    RadicleIssue,
    RadiclePatch,
    RadicleJob,
    CairnChange,
    CairnRfd,
    CairnAdr,
    CairnVerificationObligation,
    CairnReleaseDecision,
    CairnPolicyException,
}

pub fn adapter_kind(type_name: &ExternalRef) -> CoreResult<CobAdapterKind> {
    match type_name.native_id.as_str() {
        "xyz.radicle.issue" => Ok(CobAdapterKind::RadicleIssue),
        "xyz.radicle.patch" => Ok(CobAdapterKind::RadiclePatch),
        "xyz.radicle.ci-job" => Ok(CobAdapterKind::RadicleJob),
        "xyz.onixresearch.cairn-change" => Ok(CobAdapterKind::CairnChange),
        "xyz.onixresearch.rfd" => Ok(CobAdapterKind::CairnRfd),
        "xyz.onixresearch.adr" => Ok(CobAdapterKind::CairnAdr),
        "xyz.onixresearch.verification-obligation" => {
            Ok(CobAdapterKind::CairnVerificationObligation)
        }
        "xyz.onixresearch.release-decision" => Ok(CobAdapterKind::CairnReleaseDecision),
        "xyz.onixresearch.policy-exception" => Ok(CobAdapterKind::CairnPolicyException),
        _ => Err(Diagnostic::new(
            "unsupported_cob_type",
            "cob.type_name",
            "COB type has no admitted provider adapter",
        )),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MutationResult {
    Applied(Box<CobObservation>),
    Denied,
    Cancelled,
    TimedOut,
    ProviderError,
    Unknown,
}

pub trait CobMutationTransport {
    fn observe(&mut self, plan: &CobEffectPlan) -> CoreResult<CobObservation>;
    fn acquire_signer(&mut self, actor: &ExternalRef) -> CoreResult<()>;
    fn preview(&mut self, plan: &CobEffectPlan) -> CoreResult<()>;
    fn dispatch(&mut self, plan: &CobEffectPlan) -> CoreResult<MutationResult>;
    fn reconcile(&mut self, plan: &CobEffectPlan) -> CoreResult<MutationResult>;
}

pub fn dispatch_mutation<T: CobMutationTransport>(
    profile: &CobProfile,
    expected: &CobObservation,
    plan: CobEffectPlan,
    transport: &mut T,
) -> CoreResult<MutationResult> {
    let fresh = transport.observe(&plan)?;
    if &fresh != expected {
        return Err(Diagnostic::new(
            "stale_cob_preflight",
            "cob_effect.expected_heads",
            "fresh COB observation differs from the planned observation",
        ));
    }
    let admitted = plan_effect(profile, &fresh, plan)?;
    transport.acquire_signer(&admitted.actor)?;
    transport.preview(&admitted)?;
    transport.dispatch(&admitted)
}

pub fn dispatch_resolution<T: CobMutationTransport>(
    profile: &CobProfile,
    expected: &CobObservation,
    plan: CobEffectPlan,
    transport: &mut T,
) -> CoreResult<MutationResult> {
    admit_resolution(profile, expected, &plan.expected_heads)?;
    dispatch_mutation(profile, expected, plan, transport)
}

#[must_use]
pub fn cairn_profile_types() -> &'static [&'static str] {
    &INITIAL_CAIRN_TYPES
}

#[cfg(test)]
mod tests {
    use kiln_core::cob::missing_initial_profiles;

    use super::*;

    #[test]
    fn registry_requires_all_cairn_profiles() {
        assert!(!missing_initial_profiles(&[]).is_empty());
    }

    #[test]
    fn every_cairn_profile_has_a_read_only_adapter() {
        for type_name in cairn_profile_types() {
            let external = ExternalRef::new("radicle", "cob-type", type_name)
                .unwrap_or_else(|_| unreachable!());
            assert!(adapter_kind(&external).is_ok());
        }
    }

    #[test]
    fn unknown_profile_has_no_implicit_adapter() {
        let external =
            ExternalRef::new("radicle", "cob-type", "unknown").unwrap_or_else(|_| unreachable!());
        assert!(adapter_kind(&external).is_err());
    }
}
