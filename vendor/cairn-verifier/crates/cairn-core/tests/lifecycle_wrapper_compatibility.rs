//! Root API controls for lifecycle-store type compatibility.

const STORE_ID: &str = "alpha";
const REMOTE_HINT: &str = "https://example.invalid/alpha.git";
const CHECKOUT_PATH: &str = "/home/operator/alpha";
const SOURCE_MATERIAL: &[u8] = b"lifecycle wrapper compatibility";
const CONTEXT_ROOT: &str = "/stores/context";
const CONTEXT_STORE: &str = "context";
const CONTEXT_DIGEST: &str = "b3:context";
const CONTEXT_SPEC: &str = "context-spec";
const RELATED_ROOT: &str = "/stores/related";
const RELATED_STORE: &str = "related";
const RELATED_DIGEST: &str = "b3:related";

fn projection() -> cairn_core::LifecycleStoreIdentityProjection {
    cairn_core::LifecycleStoreIdentityProjection {
        schema: cairn_core::LIFECYCLE_STORE_IDENTITY_SCHEMA_V1.to_string(),
        store_id: STORE_ID.to_string(),
        remote_hint: Some(REMOTE_HINT.to_string()),
        non_claims: cairn_core::LIFECYCLE_STORE_IDENTITY_NON_CLAIMS
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        source_identity: cairn_core::lifecycle_store_source_identity(SOURCE_MATERIAL),
    }
}

#[test]
fn root_api_remains_usable() {
    // r[verify cairn.lifecycle_stores.identity.valid]
    let admission = cairn_core::admit_lifecycle_store_identity(&projection());
    let _: &cairn_core::LifecycleStoreIdentityAdmission = &admission;
    assert!(admission.valid, "{:?}", admission.diagnostics);
    let identity = admission.identity.expect("valid identity");
    let _: &cairn_core::LifecycleStoreIdentity = &identity;
    let _: &cairn_core::LifecycleStoreId = &identity.store_id;
    let _: &cairn_core::LifecycleStoreRemoteHint =
        identity.remote_hint.as_ref().expect("remote hint");
    let _: &cairn_core::LifecycleStoreNonClaim =
        identity.non_claims.first().expect("required non-claim");
    let _: &cairn_core::LifecycleStoreBlake3Identity = &identity.store_identity;
    let identity_material = cairn_core::lifecycle_store_identity_material(&identity);
    assert!(!identity_material.is_empty());

    let current = cairn_core::empty_lifecycle_store_locator();
    let _: &cairn_core::LifecycleStoreLocatorState = &current;
    let plan = cairn_core::plan_lifecycle_store_registration(&current, &identity, CHECKOUT_PATH);
    let _: &cairn_core::LifecycleStoreLocatorPlan = &plan;
    let _: &cairn_core::LifecycleStoreLocatorLocalFacts = &plan.local_facts;
    let _: &cairn_core::LifecycleStoreLocatorPortableFacts = &plan.portable_facts;
    let _: cairn_core::LifecycleStoreLocatorOperation = plan.local_facts.operation;
    assert_eq!(
        cairn_core::lifecycle_store_locator_plan_hash(&plan),
        plan.plan_hash
    );
    assert_eq!(
        cairn_core::lifecycle_store_locator_portable_hash(&plan.portable_facts),
        plan.portable_hash
    );

    let locator_projection = cairn_core::lifecycle_store_locator_projection(&current);
    let _: &cairn_core::LifecycleStoreLocatorProjection = &locator_projection;
    let locator_admission = cairn_core::admit_lifecycle_store_locator(&locator_projection);
    let _: &cairn_core::LifecycleStoreLocatorAdmission = &locator_admission;
    assert!(
        locator_admission.valid,
        "{:?}",
        locator_admission.diagnostics
    );

    let application = cairn_core::apply_lifecycle_store_locator_plan(&current, &plan);
    let _: &cairn_core::LifecycleStoreLocatorApplication = &application;
    assert!(application.accepted, "{:?}", application.diagnostics);
    let state = application.next_state.expect("accepted next state");
    let mapping = state.mappings.first().expect("registered mapping");
    let _: &cairn_core::LifecycleStoreLocatorMapping = mapping;
    let _: &cairn_core::LifecycleStoreCheckoutPath = &mapping.checkout_path;
    assert_eq!(mapping.store_id.as_str(), STORE_ID);

    let unregister = cairn_core::plan_lifecycle_store_unregistration(&state, STORE_ID);
    assert!(!unregister.blocked, "{:?}", unregister.diagnostics);

    let persisted = cairn_core::lifecycle_store_locator_projection(&state);
    let mapping_projection = persisted.mappings.first().expect("registered projection");
    let _: &cairn_core::LifecycleStoreLocatorMappingProjection = mapping_projection;
}

#[test]
fn root_api_rejects_invalid_projection() {
    // r[verify cairn.lifecycle_stores.identity.invalid]
    let mut candidate = projection();
    candidate.store_id = "../escape".to_string();
    let report = cairn_core::admit_lifecycle_store_identity(&candidate);
    assert!(!report.valid);
    assert!(report.identity.is_none());
    let diagnostic = report
        .diagnostics
        .first()
        .expect("invalid identifier diagnostic");
    let _: &cairn_core::LifecycleStoreDiagnostic = diagnostic;
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.field == "store_id"));
}

#[test]
fn context_root_api_remains_usable() {
    let selector = cairn_core::ExplicitLifecycleStoreSelector {
        store_id: CONTEXT_STORE.to_string(),
    };
    let _: &cairn_core::ExplicitLifecycleStoreSelector = &selector;
    let observed = cairn_core::StoreIdentityObservation {
        store_id: CONTEXT_STORE.to_string(),
        store_identity: CONTEXT_DIGEST.to_string(),
    };
    let _: &cairn_core::StoreIdentityObservation = &observed;
    let entry = cairn_core::LocatorStoreObservation {
        store_id: CONTEXT_STORE.to_string(),
        root: CONTEXT_ROOT.to_string(),
        store_identity: CONTEXT_DIGEST.to_string(),
        registration_current: true,
    };
    let _: &cairn_core::LocatorStoreObservation = &entry;
    let related_entry = cairn_core::LocatorStoreObservation {
        store_id: RELATED_STORE.to_string(),
        root: RELATED_ROOT.to_string(),
        store_identity: RELATED_DIGEST.to_string(),
        registration_current: true,
    };
    let locator = cairn_core::LocatorSnapshot {
        stores: vec![entry, related_entry],
    };
    let root_observations = cairn_core::RootResolutionObservations {
        explicit_root: None,
        explicit_lifecycle_store: Some(selector),
        vcs_boundary: cairn_core::VcsBoundaryFacts::Unknown,
        native_roots: Vec::new(),
        project_pointers: Vec::new(),
        locator: locator.clone(),
        fallback: cairn_core::FallbackFacts {
            current_directory: CONTEXT_ROOT.to_string(),
            classic_root: CONTEXT_ROOT.to_string(),
        },
    };
    let resolution = cairn_core::resolve_lifecycle_root(&root_observations);
    assert!(resolution.valid, "{:?}", resolution.findings);
    let selected = resolution.selected.as_ref().expect("resolved root");
    let _: &cairn_core::ResolvedLifecycleRoot = selected;

    let references = vec![cairn_core::DirectStoreReference {
        store_id: RELATED_STORE.to_string(),
        remote_hint: None,
    }];
    let reference = references.first().expect("direct reference");
    let _: &cairn_core::DirectStoreReference = reference;
    let observations = cairn_core::ContextObservations {
        resolution,
        direct_references: references,
        locator,
        accepted_specs: vec![cairn_core::AcceptedSpecSnapshot {
            store_id: RELATED_STORE.to_string(),
            store_identity: RELATED_DIGEST.to_string(),
            specs: vec![cairn_core::AcceptedSpecMetadata {
                spec_id: CONTEXT_SPEC.to_string(),
                title: "Context specification".to_string(),
                purpose_summary: "Root API compatibility control".to_string(),
            }],
        }],
    };
    let _: &cairn_core::ContextObservations = &observations;
    let result = cairn_core::build_lifecycle_store_context(&observations);
    let _: &cairn_core::ContextResult = &result;
    assert!(result.valid, "{:?}", result.doctor_findings);
    assert!(result.healthy, "{:?}", result.doctor_findings);
    let _: &cairn_core::ContextBoundary = &result.boundary;
    let _: cairn_core::LifecycleAuthorityEffect = result.boundary.authority_effect;
    let spec = result.specs.first().expect("context specification");
    let _: &cairn_core::ContextSpecResult = spec;
    assert_eq!(
        spec.fetch_command,
        cairn_core::store_qualified_spec_fetch_command(CONTEXT_SPEC, RELATED_STORE)
    );
}

#[test]
fn context_root_api_rejects_self_reference() {
    let references = vec![cairn_core::DirectStoreReference {
        store_id: CONTEXT_STORE.to_string(),
        remote_hint: None,
    }];
    let normalized = cairn_core::normalize_direct_references(Some(CONTEXT_STORE), &references);
    assert!(!normalized.valid);
    assert!(normalized.references.is_empty());
    let finding = normalized.findings.first().expect("self-reference finding");
    let _: &cairn_core::LifecycleStoreDoctorFinding = finding;
    assert_eq!(finding.code, cairn_core::REFERENCE_SELF);
}
