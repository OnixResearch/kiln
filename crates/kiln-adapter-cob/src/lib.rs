use kiln_core::cob::{
    CobCapability, CobEffectPlan, CobObservation, CobProfile, INITIAL_CAIRN_TYPES,
    admit_cob_freshness, admit_resolution, plan_effect, select_profile,
};
use kiln_core::types::{CoreResult, Diagnostic, ExternalRef};
use kiln_provider_api::{CollaborationMutationOutcome, CollaborationMutationPort, PortResult};

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

pub fn dispatch_mutation<T: CollaborationMutationPort>(
    profile: &CobProfile,
    expected: &CobObservation,
    plan: CobEffectPlan,
    transport: &mut T,
) -> PortResult<CollaborationMutationOutcome> {
    let fresh = transport.observe(&plan)?;
    admit_cob_freshness(expected, &fresh)?;
    let admitted = plan_effect(profile, &fresh, plan)?;
    transport.acquire_signer(&admitted.actor)?;
    transport.preview(&admitted)?;
    transport.dispatch(&admitted)
}

pub fn dispatch_resolution<T: CollaborationMutationPort>(
    profile: &CobProfile,
    expected: &CobObservation,
    plan: CobEffectPlan,
    transport: &mut T,
) -> PortResult<CollaborationMutationOutcome> {
    admit_resolution(profile, expected, &plan.expected_heads)?;
    dispatch_mutation(profile, expected, plan, transport)
}

#[must_use]
pub fn cairn_profile_types() -> &'static [&'static str] {
    &INITIAL_CAIRN_TYPES
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use kiln_core::cob::missing_initial_profiles;
    use kiln_core::types::{
        CobObjectId, CobProfileId, CobSemanticStateId, ContentId, EffectId, PolicyId,
        ProviderProfileId,
    };
    use kiln_provider_api::{InfrastructureFailure, PortFailure};

    use super::*;

    const MAPPER_VERSION: u16 = 1;
    const MAX_HEADS: usize = 4;
    const MAX_PAYLOAD_BYTES: usize = 4096;

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

    struct FixtureTransport {
        observation: CobObservation,
        calls: Vec<&'static str>,
        signer_fails: bool,
    }

    impl CollaborationMutationPort for FixtureTransport {
        fn observe(&mut self, _plan: &CobEffectPlan) -> PortResult<CobObservation> {
            self.calls.push("observe");
            Ok(self.observation.clone())
        }

        fn acquire_signer(&mut self, _actor: &ExternalRef) -> PortResult<()> {
            self.calls.push("signer");
            if self.signer_fails {
                return Err(PortFailure::Infrastructure(InfrastructureFailure {
                    code: "signer_missing".to_string(),
                    message: "fixture signer is unavailable".to_string(),
                }));
            }
            Ok(())
        }

        fn preview(&mut self, _plan: &CobEffectPlan) -> PortResult<()> {
            self.calls.push("preview");
            Ok(())
        }

        fn dispatch(&mut self, _plan: &CobEffectPlan) -> PortResult<CollaborationMutationOutcome> {
            self.calls.push("dispatch");
            Ok(CollaborationMutationOutcome::Applied(Box::new(
                self.observation.clone(),
            )))
        }

        fn reconcile(&mut self, _plan: &CobEffectPlan) -> PortResult<CollaborationMutationOutcome> {
            self.calls.push("reconcile");
            Ok(CollaborationMutationOutcome::Unknown)
        }
    }

    fn profile_and_observation() -> (CobProfile, CobObservation) {
        let profile = CobProfile {
            id: CobProfileId::new("profile").unwrap_or_else(|_| unreachable!()),
            provider: ProviderProfileId::new("provider").unwrap_or_else(|_| unreachable!()),
            type_name: ExternalRef::new("radicle", "cob-type", "xyz.radicle.issue")
                .unwrap_or_else(|_| unreachable!()),
            schema_line: ExternalRef::new("radicle", "schema", "v1")
                .unwrap_or_else(|_| unreachable!()),
            mapper_version: MAPPER_VERSION,
            capabilities: BTreeSet::from([CobCapability::Update]),
            max_heads: MAX_HEADS,
            max_payload_bytes: MAX_PAYLOAD_BYTES,
            complete_head_resolution: true,
        };
        let observation = CobObservation {
            profile_id: profile.id.clone(),
            repository: ExternalRef::new("radicle", "repository", "rid")
                .unwrap_or_else(|_| unreachable!()),
            type_name: profile.type_name.clone(),
            schema_line: profile.schema_line.clone(),
            object_id: CobObjectId::new("object").unwrap_or_else(|_| unreachable!()),
            heads: BTreeSet::from([
                ExternalRef::new("radicle", "operation", "head").unwrap_or_else(|_| unreachable!())
            ]),
            semantic_state: CobSemanticStateId::new("state").unwrap_or_else(|_| unreachable!()),
            projection_identity: ContentId::from_blake3_hex(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .unwrap_or_else(|_| unreachable!()),
        };
        (profile, observation)
    }

    fn effect_plan(profile: &CobProfile, observation: &CobObservation) -> CobEffectPlan {
        CobEffectPlan {
            effect_id: EffectId::new("effect").unwrap_or_else(|_| unreachable!()),
            profile_id: profile.id.clone(),
            repository: observation.repository.clone(),
            object_id: observation.object_id.clone(),
            expected_heads: observation.heads.clone(),
            action: CobCapability::Update,
            payload_identity: ContentId::from_blake3_hex(
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            )
            .unwrap_or_else(|_| unreachable!()),
            actor: ExternalRef::new("radicle", "actor", "actor").unwrap_or_else(|_| unreachable!()),
            policy_id: PolicyId::new("policy").unwrap_or_else(|_| unreachable!()),
            idempotency_key: ExternalRef::new("kiln", "command", "command")
                .unwrap_or_else(|_| unreachable!()),
        }
    }

    #[test]
    fn dispatch_orders_observation_signer_preview_and_effect() {
        let (profile, observation) = profile_and_observation();
        let mut transport = FixtureTransport {
            observation: observation.clone(),
            calls: Vec::new(),
            signer_fails: false,
        };
        let result = dispatch_mutation(
            &profile,
            &observation,
            effect_plan(&profile, &observation),
            &mut transport,
        );
        assert!(result.is_ok());
        assert_eq!(
            transport.calls,
            ["observe", "signer", "preview", "dispatch"]
        );
    }

    // r[verify kiln.providers.boundary_conformance]
    #[test]
    fn stale_observation_and_missing_signer_block_effects() {
        let (profile, observation) = profile_and_observation();
        let mut stale = observation.clone();
        stale.heads =
            BTreeSet::from([ExternalRef::new("radicle", "operation", "moved")
                .unwrap_or_else(|_| unreachable!())]);
        let mut stale_transport = FixtureTransport {
            observation: stale,
            calls: Vec::new(),
            signer_fails: false,
        };
        let stale_result = dispatch_mutation(
            &profile,
            &observation,
            effect_plan(&profile, &observation),
            &mut stale_transport,
        );
        assert!(stale_result.is_err());
        assert_eq!(stale_transport.calls, ["observe"]);

        let mut missing_signer = FixtureTransport {
            observation: observation.clone(),
            calls: Vec::new(),
            signer_fails: true,
        };
        let signer_result = dispatch_mutation(
            &profile,
            &observation,
            effect_plan(&profile, &observation),
            &mut missing_signer,
        );
        assert!(signer_result.is_err());
        assert_eq!(missing_signer.calls, ["observe", "signer"]);
    }
}
