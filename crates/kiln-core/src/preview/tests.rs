use super::*;
use crate::{
    build::{BuildCacheKind, BuildProductObservation, BuildStoreObjectObservation},
    types::{EffectId, PipelineRevisionId, ProviderProfileId},
};
use alloc::{
    format,
    string::{String, ToString},
    vec,
};

fn id(c: char) -> ContentId {
    ContentId::from_blake3_hex(c.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}
fn alias(s: &str) -> Alias {
    Alias::new(s).unwrap_or_else(|_| unreachable!())
}
fn fixture() -> (
    BuildEffectPlan,
    BuildEvaluationObservation,
    PreviewDescriptor,
) {
    let plan = BuildEffectPlan {
        effect_id: EffectId::new("build-effect").unwrap_or_else(|_| unreachable!()),
        attempt_id: AttemptId::new("attempt").unwrap_or_else(|_| unreachable!()),
        idempotency_key: id('1'),
        candidate_id: CandidateId::new("candidate").unwrap_or_else(|_| unreachable!()),
        pipeline_revision: PipelineRevisionId::new("pipeline").unwrap_or_else(|_| unreachable!()),
        plan_identity: id('2'),
        policy_identity: id('3'),
        provider_profile: ProviderProfileId::new("nix").unwrap_or_else(|_| unreachable!()),
        platform: alias("x86_64-linux"),
        required_products: vec![alias("out")],
        constraints: None,
    };
    let root = "/nix/store/00000000000000000000000000000000-hello";
    let member = RuntimeMember {
        path: root.into(),
        nar_sha256: "a".repeat(64),
        nar_size: 16,
        references: vec![],
        signatures: vec!["cache-1:signature".into()],
    };
    let product = BuildProductObservation {
        name: alias("out"),
        artifact_identity: id('4'),
        store_identity: id('5'),
        store: Some(BuildStoreObjectObservation {
            nar_sha256: member.nar_sha256.clone(),
            nar_size: 16,
            references: vec![],
            references_complete: true,
            deriver: None,
            content_address: None,
            signatures: member.signatures.clone(),
        }),
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
        outcome: BuildEvaluationOutcome::Success,
        products: vec![product],
        constraints: None,
        inputs: vec![],
        input_closure_identity: None,
        input_closure_complete: true,
        derivation_identity: None,
        builder_identity: id('6'),
        worker_identity: id('7'),
        store_identity: id('8'),
        cache_kind: BuildCacheKind::Local,
        cache_source_identity: None,
        log_identities: vec![],
        metrics: vec![],
        rebuild: vec![],
        receipt_identity: Some(id('9')),
    };
    let d = PreviewDescriptor {
        version: 1,
        binding: PreviewBinding {
            project: "project".into(),
            repository: "repository".into(),
            revision: "b".repeat(40),
            run_id: RunId::new("run").unwrap_or_else(|_| unreachable!()),
            attempt_id: plan.attempt_id.clone(),
            candidate_id: plan.candidate_id.clone(),
            build_receipt: id('9'),
            product: alias("out"),
            artifact_identity: id('4'),
            store_identity: id('5'),
        },
        platform: plan.platform.clone(),
        root_path: root.into(),
        executable: format!("{root}/bin/hello"),
        arguments: vec!["--version".into()],
        closure: vec![member],
        closure_complete: true,
        engine: PreviewEngine {
            protocol: alias("existing-linux-engine"),
            identity: id('a'),
            platform: plan.platform.clone(),
            guest_memory_bytes: 512 * 1024 * 1024,
            network_access: false,
            supports_exact_closure: true,
            supports_disposable_session: true,
        },
        trust_identity: id('c'),
        public_keys: vec!["cache-1:public-key".into()],
        issued_at: 100,
        limits: PreviewLimits {
            runtime_bytes: 1024,
            run_seconds: 60,
            output_bytes: 4096,
            expires_at: 200,
            retain_after_session: false,
        },
    };
    (plan, observation, d)
}
fn admitted(
    p: &BuildEffectPlan,
    o: &BuildEvaluationObservation,
    d: &PreviewDescriptor,
) -> CoreResult<ContentId> {
    admit_preview(p, o, &d.binding, d, &d.engine, 101)
}

#[test]
fn exact_build_is_admitted_and_retained_identity_is_stable() -> Result<(), String> {
    let (p, o, d) = fixture();
    let first = admitted(&p, &o, &d).map_err(|error| format!("{error:?}"))?;
    let encoded = serde_json::to_vec(&d).map_err(|error| error.to_string())?;
    let restored: PreviewDescriptor =
        serde_json::from_slice(&encoded).map_err(|error| error.to_string())?;
    assert_eq!(admitted(&p, &o, &restored), Ok(first));
    // Path-only legacy product identities do not prove the exact CI NAR bytes.
    let mut old = o.clone();
    old.products
        .first_mut()
        .ok_or("fixture has no build product")?
        .store = None;
    assert!(admitted(&p, &old, &d).is_err());
    Ok(())
}
#[test]
fn retained_candidate_cannot_be_relabelled_as_current() {
    let (p, o, d) = fixture();
    let mut current = d.binding.clone();
    current.revision = "d".repeat(40);
    assert!(admit_preview(&p, &o, &current, &d, &d.engine, 101).is_err());
    current = d.binding.clone();
    current.run_id = RunId::new("other-run").unwrap_or_else(|_| unreachable!());
    assert!(admit_preview(&p, &o, &current, &d, &d.engine, 101).is_err());
}
#[test]
fn changes_to_every_execution_identity_produce_distinct_identity_or_rejection() -> Result<(), String>
{
    let (p, o, d) = fixture();
    let original = descriptor_identity(&d).map_err(|error| format!("{error:?}"))?;
    let mut variants = vec![];
    let mut x = d.clone();
    x.binding.candidate_id = CandidateId::new("new").unwrap_or_else(|_| unreachable!());
    variants.push(x);
    let mut x = d.clone();
    x.executable.push_str("-changed");
    variants.push(x);
    let mut x = d.clone();
    x.engine.identity = id('e');
    variants.push(x);
    let mut x = d.clone();
    x.closure
        .first_mut()
        .ok_or("fixture has no closure member")?
        .nar_sha256 = "f".repeat(64);
    variants.push(x);
    for x in variants {
        assert_ne!(descriptor_identity(&x), Ok(original.clone()));
    }
    let mut selected = d.engine.clone();
    selected.identity = id('e');
    assert!(admit_preview(&p, &o, &d.binding, &d, &selected, 101).is_err());
    Ok(())
}
#[test]
fn failed_build_missing_product_and_partial_closure_are_not_launches() -> Result<(), String> {
    let (p, mut o, d) = fixture();
    o.outcome = BuildEvaluationOutcome::Failed;
    assert!(admitted(&p, &o, &d).is_err());
    o.outcome = BuildEvaluationOutcome::Success;
    o.products.clear();
    assert!(admitted(&p, &o, &d).is_err());
    let (p, o, mut d) = fixture();
    d.closure_complete = false;
    assert!(admitted(&p, &o, &d).is_err());
    d.closure_complete = true;
    d.closure
        .first_mut()
        .ok_or("fixture has no closure member")?
        .references
        .push("/nix/store/11111111111111111111111111111111-missing".into());
    assert!(admitted(&p, &o, &d).is_err());
    Ok(())
}
#[test]
fn no_build_only_member_or_unsigned_member_can_enter_export() -> Result<(), String> {
    let (_, _, mut d) = fixture();
    let mut extra = d
        .closure
        .first()
        .ok_or("fixture has no closure member")?
        .clone();
    extra.path = "/nix/store/11111111111111111111111111111111-build-input".into();
    d.closure.push(extra);
    assert!(validate_descriptor(&d, 101).is_err());
    d.closure.pop();
    d.closure
        .first_mut()
        .ok_or("fixture has no closure member")?
        .signatures
        .clear();
    assert!(validate_descriptor(&d, 101).is_err());
    Ok(())
}
#[test]
fn expiry_resource_exhaustion_network_and_redox_fail_closed() {
    let (p, o, mut d) = fixture();
    assert!(admit_preview(&p, &o, &d.binding, &d, &d.engine, 200).is_err());
    d.limits.runtime_bytes = 15;
    assert!(validate_descriptor(&d, 101).is_err());
    d.limits.runtime_bytes = 1024;
    d.engine.network_access = true;
    assert!(validate_descriptor(&d, 101).is_err());
    d.engine.network_access = false;
    d.platform = alias("x86_64-redox");
    d.engine.platform = d.platform.clone();
    assert!(validate_descriptor(&d, 101).is_err());
}
#[test]
fn preview_decision_never_changes_original_build_evidence() {
    let (p, o, mut d) = fixture();
    let before = o.clone();
    d.engine.supports_exact_closure = false;
    assert!(admitted(&p, &o, &d).is_err());
    assert_eq!(o, before);
}
