//! Pure lifecycle-store root resolution and direct-reference context.
//!
//! The shell supplies every path, boundary, locator, pointer, and spec observation.
//! This module has no global-registry input and no transitive-reference input.

use std::collections::BTreeSet;

pub const ROOT_SELECTOR_CONFLICT: &str = "lifecycle_store.root.selector_conflict";
pub const ROOT_STORE_MISSING: &str = "lifecycle_store.root.store_missing";
pub const ROOT_STORE_AMBIGUOUS: &str = "lifecycle_store.root.store_ambiguous";
pub const ROOT_STORE_STALE: &str = "lifecycle_store.root.store_stale";
pub const ROOT_PROJECT_POINTER_STALE: &str = "lifecycle_store.root.project_pointer_stale";
pub const REFERENCE_DUPLICATE: &str = "lifecycle_store.reference.duplicate";
pub const REFERENCE_SELF: &str = "lifecycle_store.reference.self";
pub const REFERENCE_STORE_MISSING: &str = "lifecycle_store.reference.store_missing";
pub const REFERENCE_STORE_AMBIGUOUS: &str = "lifecycle_store.reference.store_ambiguous";
pub const REFERENCE_STORE_STALE: &str = "lifecycle_store.reference.store_stale";
pub const REFERENCE_SNAPSHOT_MISSING: &str = "lifecycle_store.reference.snapshot_missing";
pub const REFERENCE_SNAPSHOT_AMBIGUOUS: &str = "lifecycle_store.reference.snapshot_ambiguous";
pub const REFERENCE_SNAPSHOT_STALE: &str = "lifecycle_store.reference.snapshot_stale";
pub const REFERENCE_SPEC_DUPLICATE: &str = "lifecycle_store.reference.spec_duplicate";
pub const REFERENCE_AVAILABLE: &str = "lifecycle_store.reference.available";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplicitRootSelector {
    pub root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplicitLifecycleStoreSelector {
    pub store_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VcsBoundaryFacts {
    Known {
        root: String,
        max_ancestor_distance: usize,
    },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreIdentityObservation {
    pub store_id: String,
    pub store_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeRootCandidate {
    pub root: String,
    pub ancestor_distance: usize,
    pub store: Option<StoreIdentityObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPointerObservation {
    pub project_root: String,
    pub project_identity: String,
    pub store_id: String,
    pub ancestor_distance: usize,
    pub projection_current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatorStoreObservation {
    pub store_id: String,
    pub root: String,
    pub store_identity: String,
    pub registration_current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LocatorSnapshot {
    pub stores: Vec<LocatorStoreObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackFacts {
    pub current_directory: String,
    pub classic_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootResolutionObservations {
    pub explicit_root: Option<ExplicitRootSelector>,
    pub explicit_lifecycle_store: Option<ExplicitLifecycleStoreSelector>,
    pub vcs_boundary: VcsBoundaryFacts,
    pub native_roots: Vec<NativeRootCandidate>,
    pub project_pointers: Vec<ProjectPointerObservation>,
    pub locator: LocatorSnapshot,
    pub fallback: FallbackFacts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootResolutionSource {
    ExplicitRoot,
    ExplicitLifecycleStore,
    NativeRoot,
    ProjectPointer,
    ClassicFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLifecycleRoot {
    pub root: String,
    pub source: RootResolutionSource,
    pub store_id: Option<String>,
    pub store_identity: Option<String>,
    pub project_identity: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DoctorFindingStatus {
    Fail,
    Warn,
    Ok,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleStoreDoctorFinding {
    pub code: String,
    pub status: DoctorFindingStatus,
    pub subject: String,
    pub message: String,
    pub repair_command: Option<String>,
    pub remote_hint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryLookupBoundary {
    LocalLocatorOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootResolutionResult {
    pub valid: bool,
    pub selected: Option<ResolvedLifecycleRoot>,
    pub findings: Vec<LifecycleStoreDoctorFinding>,
    pub locator_consulted: bool,
    pub registry_lookup: RegistryLookupBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectStoreReference {
    pub store_id: String,
    pub remote_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectReferenceNormalization {
    pub valid: bool,
    pub references: Vec<DirectStoreReference>,
    pub findings: Vec<LifecycleStoreDoctorFinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedSpecMetadata {
    pub spec_id: String,
    pub title: String,
    pub purpose_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedSpecSnapshot {
    pub store_id: String,
    pub store_identity: String,
    pub specs: Vec<AcceptedSpecMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextObservations {
    pub resolution: RootResolutionResult,
    pub direct_references: Vec<DirectStoreReference>,
    pub locator: LocatorSnapshot,
    pub accepted_specs: Vec<AcceptedSpecSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSpecResult {
    pub store_id: String,
    pub store_identity: String,
    pub spec_id: String,
    pub title: String,
    pub purpose_summary: String,
    pub fetch_command: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceTraversalBoundary {
    DirectOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceContentBoundary {
    AcceptedSpecsOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleAuthorityEffect {
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextBoundary {
    pub traversal: ReferenceTraversalBoundary,
    pub content: ReferenceContentBoundary,
    pub registry_lookup: RegistryLookupBoundary,
    pub authority_effect: LifecycleAuthorityEffect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextResult {
    pub valid: bool,
    pub healthy: bool,
    pub selected_root: Option<ResolvedLifecycleRoot>,
    pub direct_references: Vec<DirectStoreReference>,
    pub specs: Vec<ContextSpecResult>,
    pub doctor_findings: Vec<LifecycleStoreDoctorFinding>,
    pub boundary: ContextBoundary,
}

// r[impl cairn.lifecycle_stores.root_resolution]
// r[impl cairn.lifecycle_stores.functional_boundary]
#[must_use]
pub fn resolve_lifecycle_root(observations: &RootResolutionObservations) -> RootResolutionResult {
    if observations.explicit_root.is_some() && observations.explicit_lifecycle_store.is_some() {
        return root_resolution_result(
            None,
            vec![doctor_finding(
                ROOT_SELECTOR_CONFLICT,
                DoctorFindingStatus::Fail,
                "root selectors",
                "--root and --lifecycle-store cannot be used together",
            )],
            false,
        );
    }

    if let Some(selector) = &observations.explicit_root {
        let selected = ResolvedLifecycleRoot {
            root: selector.root.clone(),
            source: RootResolutionSource::ExplicitRoot,
            store_id: None,
            store_identity: None,
            project_identity: None,
        };
        return root_resolution_result(Some(selected), Vec::new(), false);
    }

    if let Some(selector) = &observations.explicit_lifecycle_store {
        let mut findings = Vec::new();
        let selected = resolve_locator_store(
            &selector.store_id,
            RootResolutionSource::ExplicitLifecycleStore,
            None,
            &observations.locator,
            &mut findings,
        );
        return root_resolution_result(selected, findings, true);
    }

    if let Some(native_root) = nearest_native_root(observations) {
        let selected = ResolvedLifecycleRoot {
            root: native_root.root.clone(),
            source: RootResolutionSource::NativeRoot,
            store_id: native_root
                .store
                .as_ref()
                .map(|store| store.store_id.clone()),
            store_identity: native_root
                .store
                .as_ref()
                .map(|store| store.store_identity.clone()),
            project_identity: None,
        };
        let mut findings = Vec::new();
        if let Some(pointer) = nearest_project_pointer(observations) {
            let message = format!(
                "native root {} takes precedence over project pointer to store {}",
                native_root.root, pointer.store_id
            );
            findings.push(doctor_finding(
                ROOT_PROJECT_POINTER_STALE,
                DoctorFindingStatus::Warn,
                &pointer.project_root,
                &message,
            ));
        }
        return root_resolution_result(Some(selected), findings, false);
    }

    if let Some(pointer) = nearest_project_pointer(observations) {
        if !pointer.projection_current {
            let message = format!(
                "project pointer to store {} has a stale checked projection",
                pointer.store_id
            );
            return root_resolution_result(
                None,
                vec![doctor_finding(
                    ROOT_PROJECT_POINTER_STALE,
                    DoctorFindingStatus::Fail,
                    &pointer.project_root,
                    &message,
                )],
                false,
            );
        }
        let mut findings = Vec::new();
        let selected = resolve_locator_store(
            &pointer.store_id,
            RootResolutionSource::ProjectPointer,
            Some(pointer.project_identity.clone()),
            &observations.locator,
            &mut findings,
        );
        return root_resolution_result(selected, findings, true);
    }

    let selected = ResolvedLifecycleRoot {
        root: observations.fallback.classic_root.clone(),
        source: RootResolutionSource::ClassicFallback,
        store_id: None,
        store_identity: None,
        project_identity: None,
    };
    root_resolution_result(Some(selected), Vec::new(), false)
}

// r[impl cairn.lifecycle_stores.references]
// r[impl cairn.lifecycle_stores.project_config]
#[must_use]
pub fn normalize_direct_references(
    subject_store_id: Option<&str>,
    references: &[DirectStoreReference],
) -> DirectReferenceNormalization {
    let mut sorted = references.to_vec();
    sorted.sort_by(|left, right| {
        left.store_id
            .cmp(&right.store_id)
            .then(left.remote_hint.cmp(&right.remote_hint))
    });

    let mut seen = BTreeSet::new();
    let mut duplicate_ids = BTreeSet::new();
    let mut normalized = Vec::new();
    let mut findings = Vec::new();

    for reference in sorted {
        if subject_store_id.is_some_and(|subject| subject == reference.store_id) {
            let message = format!("store {} cannot reference itself", reference.store_id);
            findings.push(doctor_finding(
                REFERENCE_SELF,
                DoctorFindingStatus::Fail,
                &reference.store_id,
                &message,
            ));
        }
        if !seen.insert(reference.store_id.clone()) {
            duplicate_ids.insert(reference.store_id);
            continue;
        }
        normalized.push(reference);
    }

    for store_id in duplicate_ids {
        let message = format!("store {store_id} is declared more than once");
        findings.push(doctor_finding(
            REFERENCE_DUPLICATE,
            DoctorFindingStatus::Fail,
            &store_id,
            &message,
        ));
    }

    sort_findings(&mut findings);
    let valid = !has_failed_finding(&findings);
    if !valid {
        normalized.clear();
    }
    DirectReferenceNormalization {
        valid,
        references: normalized,
        findings,
    }
}

// r[impl cairn.lifecycle_stores.context]
// r[impl cairn.lifecycle_stores.references]
// r[impl cairn.lifecycle_stores.functional_boundary]
#[must_use]
pub fn build_lifecycle_store_context(observations: &ContextObservations) -> ContextResult {
    let subject_store_id = observations
        .resolution
        .selected
        .as_ref()
        .and_then(|selected| selected.store_id.as_deref());
    let normalized = normalize_direct_references(subject_store_id, &observations.direct_references);
    let mut findings = observations.resolution.findings.clone();
    findings.extend(normalized.findings.clone());
    let mut specs = Vec::new();

    if observations.resolution.valid && normalized.valid {
        for reference in &normalized.references {
            index_direct_reference(reference, observations, &mut specs, &mut findings);
        }
    }

    specs.sort_by(|left, right| {
        left.store_id
            .cmp(&right.store_id)
            .then(left.spec_id.cmp(&right.spec_id))
            .then(left.title.cmp(&right.title))
            .then(left.purpose_summary.cmp(&right.purpose_summary))
    });
    sort_findings(&mut findings);

    let valid = observations.resolution.valid && !has_failed_finding(&findings);
    let healthy = valid
        && findings
            .iter()
            .all(|finding| finding.status == DoctorFindingStatus::Ok);
    ContextResult {
        valid,
        healthy,
        selected_root: observations.resolution.selected.clone(),
        direct_references: normalized.references,
        specs,
        doctor_findings: findings,
        boundary: context_boundary(),
    }
}

#[must_use]
pub fn store_qualified_spec_fetch_command(spec_id: &str, store_id: &str) -> String {
    format!("cairn spec show {spec_id} --lifecycle-store {store_id}")
}

fn nearest_native_root(observations: &RootResolutionObservations) -> Option<&NativeRootCandidate> {
    let mut candidates = observations
        .native_roots
        .iter()
        .filter(|candidate| native_root_is_admissible(candidate, observations))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        left.ancestor_distance
            .cmp(&right.ancestor_distance)
            .then(left.root.cmp(&right.root))
    });
    candidates.into_iter().next()
}

fn nearest_project_pointer(
    observations: &RootResolutionObservations,
) -> Option<&ProjectPointerObservation> {
    let mut pointers = observations
        .project_pointers
        .iter()
        .filter(|pointer| project_pointer_is_admissible(pointer, observations))
        .collect::<Vec<_>>();
    pointers.sort_by(|left, right| {
        left.ancestor_distance
            .cmp(&right.ancestor_distance)
            .then(left.project_root.cmp(&right.project_root))
            .then(left.store_id.cmp(&right.store_id))
    });
    pointers.into_iter().next()
}

fn native_root_is_admissible(
    candidate: &NativeRootCandidate,
    observations: &RootResolutionObservations,
) -> bool {
    match &observations.vcs_boundary {
        VcsBoundaryFacts::Known {
            max_ancestor_distance,
            ..
        } => candidate.ancestor_distance <= *max_ancestor_distance,
        VcsBoundaryFacts::Unknown => {
            candidate.ancestor_distance == 0
                && candidate.root == observations.fallback.current_directory
        }
    }
}

fn project_pointer_is_admissible(
    pointer: &ProjectPointerObservation,
    observations: &RootResolutionObservations,
) -> bool {
    match &observations.vcs_boundary {
        VcsBoundaryFacts::Known {
            max_ancestor_distance,
            ..
        } => pointer.ancestor_distance <= *max_ancestor_distance,
        VcsBoundaryFacts::Unknown => {
            pointer.ancestor_distance == 0
                && pointer.project_root == observations.fallback.current_directory
        }
    }
}

fn resolve_locator_store(
    store_id: &str,
    source: RootResolutionSource,
    project_identity: Option<String>,
    locator: &LocatorSnapshot,
    findings: &mut Vec<LifecycleStoreDoctorFinding>,
) -> Option<ResolvedLifecycleRoot> {
    let matches = locator
        .stores
        .iter()
        .filter(|store| store.store_id == store_id)
        .collect::<Vec<_>>();
    if matches.is_empty() {
        let message = format!("store {store_id} is not present in the local locator");
        findings.push(doctor_finding(
            ROOT_STORE_MISSING,
            DoctorFindingStatus::Fail,
            store_id,
            &message,
        ));
        return None;
    }
    if matches.len() > 1 {
        let message = format!("store {store_id} has more than one local locator row");
        findings.push(doctor_finding(
            ROOT_STORE_AMBIGUOUS,
            DoctorFindingStatus::Fail,
            store_id,
            &message,
        ));
        return None;
    }

    let store = matches[0];
    if !store.registration_current {
        let message = format!("store {store_id} has a stale local locator row");
        findings.push(doctor_finding(
            ROOT_STORE_STALE,
            DoctorFindingStatus::Fail,
            store_id,
            &message,
        ));
        return None;
    }

    Some(ResolvedLifecycleRoot {
        root: store.root.clone(),
        source,
        store_id: Some(store.store_id.clone()),
        store_identity: Some(store.store_identity.clone()),
        project_identity,
    })
}

fn root_resolution_result(
    selected: Option<ResolvedLifecycleRoot>,
    mut findings: Vec<LifecycleStoreDoctorFinding>,
    locator_consulted: bool,
) -> RootResolutionResult {
    sort_findings(&mut findings);
    let valid = selected.is_some() && !has_failed_finding(&findings);
    RootResolutionResult {
        valid,
        selected,
        findings,
        locator_consulted,
        registry_lookup: RegistryLookupBoundary::LocalLocatorOnly,
    }
}

fn index_direct_reference(
    reference: &DirectStoreReference,
    observations: &ContextObservations,
    specs: &mut Vec<ContextSpecResult>,
    findings: &mut Vec<LifecycleStoreDoctorFinding>,
) {
    let locator_matches = observations
        .locator
        .stores
        .iter()
        .filter(|store| store.store_id == reference.store_id)
        .collect::<Vec<_>>();
    if locator_matches.is_empty() {
        let message = format!(
            "direct reference {} is not registered on this machine",
            reference.store_id
        );
        let mut finding = doctor_finding(
            REFERENCE_STORE_MISSING,
            DoctorFindingStatus::Warn,
            &reference.store_id,
            &message,
        );
        finding.repair_command = Some(format!(
            "cairn store register <checkout-path> --id {}",
            reference.store_id
        ));
        finding.remote_hint = reference.remote_hint.clone();
        findings.push(finding);
        return;
    }
    if locator_matches.len() > 1 {
        let message = format!(
            "direct reference {} has more than one local locator row",
            reference.store_id
        );
        findings.push(doctor_finding(
            REFERENCE_STORE_AMBIGUOUS,
            DoctorFindingStatus::Fail,
            &reference.store_id,
            &message,
        ));
        return;
    }

    let locator_store = locator_matches[0];
    if !locator_store.registration_current {
        let message = format!(
            "direct reference {} has a stale local locator row",
            reference.store_id
        );
        findings.push(doctor_finding(
            REFERENCE_STORE_STALE,
            DoctorFindingStatus::Fail,
            &reference.store_id,
            &message,
        ));
        return;
    }

    let snapshot_matches = observations
        .accepted_specs
        .iter()
        .filter(|snapshot| snapshot.store_id == reference.store_id)
        .collect::<Vec<_>>();
    if snapshot_matches.is_empty() {
        let message = format!(
            "accepted-spec metadata is missing for direct reference {}",
            reference.store_id
        );
        findings.push(doctor_finding(
            REFERENCE_SNAPSHOT_MISSING,
            DoctorFindingStatus::Fail,
            &reference.store_id,
            &message,
        ));
        return;
    }
    if snapshot_matches.len() > 1 {
        let message = format!(
            "accepted-spec metadata is ambiguous for direct reference {}",
            reference.store_id
        );
        findings.push(doctor_finding(
            REFERENCE_SNAPSHOT_AMBIGUOUS,
            DoctorFindingStatus::Fail,
            &reference.store_id,
            &message,
        ));
        return;
    }

    let snapshot = snapshot_matches[0];
    if snapshot.store_identity != locator_store.store_identity {
        let message = format!(
            "accepted-spec metadata for {} does not match the registered store identity",
            reference.store_id
        );
        findings.push(doctor_finding(
            REFERENCE_SNAPSHOT_STALE,
            DoctorFindingStatus::Fail,
            &reference.store_id,
            &message,
        ));
        return;
    }

    let mut accepted_specs = snapshot.specs.clone();
    accepted_specs.sort_by(|left, right| {
        left.spec_id
            .cmp(&right.spec_id)
            .then(left.title.cmp(&right.title))
            .then(left.purpose_summary.cmp(&right.purpose_summary))
    });
    if accepted_spec_ids_have_duplicates(&accepted_specs) {
        let message = format!(
            "accepted-spec metadata for {} contains duplicate spec IDs",
            reference.store_id
        );
        findings.push(doctor_finding(
            REFERENCE_SPEC_DUPLICATE,
            DoctorFindingStatus::Fail,
            &reference.store_id,
            &message,
        ));
        return;
    }

    for spec in accepted_specs {
        specs.push(ContextSpecResult {
            store_id: reference.store_id.clone(),
            store_identity: snapshot.store_identity.clone(),
            fetch_command: store_qualified_spec_fetch_command(&spec.spec_id, &reference.store_id),
            spec_id: spec.spec_id,
            title: spec.title,
            purpose_summary: spec.purpose_summary,
        });
    }
    let message = format!(
        "direct reference {} is registered with current accepted-spec metadata",
        reference.store_id
    );
    findings.push(doctor_finding(
        REFERENCE_AVAILABLE,
        DoctorFindingStatus::Ok,
        &reference.store_id,
        &message,
    ));
}

fn accepted_spec_ids_have_duplicates(specs: &[AcceptedSpecMetadata]) -> bool {
    let mut seen = BTreeSet::new();
    for spec in specs {
        if !seen.insert(&spec.spec_id) {
            return true;
        }
    }
    false
}

fn context_boundary() -> ContextBoundary {
    ContextBoundary {
        traversal: ReferenceTraversalBoundary::DirectOnly,
        content: ReferenceContentBoundary::AcceptedSpecsOnly,
        registry_lookup: RegistryLookupBoundary::LocalLocatorOnly,
        authority_effect: LifecycleAuthorityEffect::None,
    }
}

fn doctor_finding(
    code: &str,
    status: DoctorFindingStatus,
    subject: &str,
    message: &str,
) -> LifecycleStoreDoctorFinding {
    LifecycleStoreDoctorFinding {
        code: code.to_string(),
        status,
        subject: subject.to_string(),
        message: message.to_string(),
        repair_command: None,
        remote_hint: None,
    }
}

fn has_failed_finding(findings: &[LifecycleStoreDoctorFinding]) -> bool {
    findings
        .iter()
        .any(|finding| finding.status == DoctorFindingStatus::Fail)
}

fn sort_findings(findings: &mut [LifecycleStoreDoctorFinding]) {
    findings.sort_by(|left, right| {
        left.status
            .cmp(&right.status)
            .then(left.code.cmp(&right.code))
            .then(left.subject.cmp(&right.subject))
            .then(left.message.cmp(&right.message))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const CURRENT_DIRECTORY: &str = "/workspace/code/subdir";
    const PROJECT_ROOT: &str = "/workspace/code";
    const NATIVE_ROOT: &str = "/workspace/code/native";
    const CLASSIC_ROOT: &str = "/workspace/code/subdir";
    const STORE_ROOT: &str = "/stores/shared";
    const STORE_ID: &str = "shared";
    const OTHER_STORE_ID: &str = "other";
    const TRANSITIVE_STORE_ID: &str = "transitive";
    const STORE_IDENTITY: &str = "b3:shared";
    const OTHER_STORE_IDENTITY: &str = "b3:other";
    const PROJECT_IDENTITY: &str = "b3:project";
    const REMOTE_HINT: &str = "ssh://example.invalid/shared";
    const BOUNDARY_MAX_ANCESTORS: usize = 1;
    const EXPECTED_CONTEXT_SPEC_COUNT: usize = 2;

    fn locator_store(store_id: &str, store_identity: &str) -> LocatorStoreObservation {
        LocatorStoreObservation {
            store_id: store_id.to_string(),
            root: format!("/stores/{store_id}"),
            store_identity: store_identity.to_string(),
            registration_current: true,
        }
    }

    fn native_root(root: &str, ancestor_distance: usize) -> NativeRootCandidate {
        NativeRootCandidate {
            root: root.to_string(),
            ancestor_distance,
            store: None,
        }
    }

    fn project_pointer(
        store_id: &str,
        ancestor_distance: usize,
        projection_current: bool,
    ) -> ProjectPointerObservation {
        ProjectPointerObservation {
            project_root: PROJECT_ROOT.to_string(),
            project_identity: PROJECT_IDENTITY.to_string(),
            store_id: store_id.to_string(),
            ancestor_distance,
            projection_current,
        }
    }

    fn base_resolution_observations() -> RootResolutionObservations {
        RootResolutionObservations {
            explicit_root: None,
            explicit_lifecycle_store: None,
            vcs_boundary: VcsBoundaryFacts::Known {
                root: PROJECT_ROOT.to_string(),
                max_ancestor_distance: BOUNDARY_MAX_ANCESTORS,
            },
            native_roots: Vec::new(),
            project_pointers: Vec::new(),
            locator: LocatorSnapshot {
                stores: vec![locator_store(STORE_ID, STORE_IDENTITY)],
            },
            fallback: FallbackFacts {
                current_directory: CURRENT_DIRECTORY.to_string(),
                classic_root: CLASSIC_ROOT.to_string(),
            },
        }
    }

    fn resolved_subject() -> RootResolutionResult {
        root_resolution_result(
            Some(ResolvedLifecycleRoot {
                root: "/stores/subject".to_string(),
                source: RootResolutionSource::ExplicitLifecycleStore,
                store_id: Some("subject".to_string()),
                store_identity: Some("b3:subject".to_string()),
                project_identity: None,
            }),
            Vec::new(),
            true,
        )
    }

    fn accepted_snapshot(
        store_id: &str,
        store_identity: &str,
        specs: Vec<AcceptedSpecMetadata>,
    ) -> AcceptedSpecSnapshot {
        AcceptedSpecSnapshot {
            store_id: store_id.to_string(),
            store_identity: store_identity.to_string(),
            specs,
        }
    }

    fn spec(spec_id: &str, title: &str) -> AcceptedSpecMetadata {
        AcceptedSpecMetadata {
            spec_id: spec_id.to_string(),
            title: title.to_string(),
            purpose_summary: format!("Purpose for {title}"),
        }
    }

    #[test]
    fn explicit_root_has_precedence_without_locator_lookup() {
        // r[verify cairn.lifecycle_stores.root_resolution.explicit]
        let mut observations = base_resolution_observations();
        observations.explicit_root = Some(ExplicitRootSelector {
            root: "/explicit".to_string(),
        });
        observations.native_roots.push(native_root(NATIVE_ROOT, 0));
        observations
            .project_pointers
            .push(project_pointer(STORE_ID, 0, true));
        observations.locator.stores.clear();

        let result = resolve_lifecycle_root(&observations);

        assert!(result.valid);
        assert!(!result.locator_consulted);
        assert_eq!(
            result.registry_lookup,
            RegistryLookupBoundary::LocalLocatorOnly
        );
        assert_eq!(
            result.selected,
            Some(ResolvedLifecycleRoot {
                root: "/explicit".to_string(),
                source: RootResolutionSource::ExplicitRoot,
                store_id: None,
                store_identity: None,
                project_identity: None,
            })
        );
        assert!(result.findings.is_empty());
    }

    #[test]
    fn explicit_lifecycle_store_resolves_only_from_local_locator() {
        // r[verify cairn.lifecycle_stores.root_resolution.explicit]
        let mut observations = base_resolution_observations();
        observations.explicit_lifecycle_store = Some(ExplicitLifecycleStoreSelector {
            store_id: STORE_ID.to_string(),
        });
        observations.native_roots.push(native_root(NATIVE_ROOT, 0));

        let result = resolve_lifecycle_root(&observations);
        let selected = result.selected.expect("explicit store resolves");

        assert!(result.valid);
        assert!(result.locator_consulted);
        assert_eq!(selected.root, STORE_ROOT);
        assert_eq!(
            selected.source,
            RootResolutionSource::ExplicitLifecycleStore
        );
        assert_eq!(selected.store_identity.as_deref(), Some(STORE_IDENTITY));
        assert_eq!(selected.project_identity, None);
    }

    #[test]
    fn checked_project_pointer_resolves_registered_store() {
        // r[verify cairn.lifecycle_stores.project_config.pointer]
        let mut observations = base_resolution_observations();
        observations
            .project_pointers
            .push(project_pointer(STORE_ID, 0, true));

        let result = resolve_lifecycle_root(&observations);
        let selected = result.selected.expect("project pointer resolves");

        assert!(result.valid);
        assert!(result.locator_consulted);
        assert_eq!(selected.root, STORE_ROOT);
        assert_eq!(selected.source, RootResolutionSource::ProjectPointer);
        assert_eq!(selected.store_id.as_deref(), Some(STORE_ID));
        assert_eq!(selected.store_identity.as_deref(), Some(STORE_IDENTITY));
        assert_eq!(selected.project_identity.as_deref(), Some(PROJECT_IDENTITY));
        assert!(result.findings.is_empty());
    }

    #[test]
    fn missing_explicit_store_fails_without_implicit_discovery() {
        // r[verify cairn.lifecycle_stores.root_resolution.explicit]
        let mut observations = base_resolution_observations();
        observations.explicit_lifecycle_store = Some(ExplicitLifecycleStoreSelector {
            store_id: STORE_ID.to_string(),
        });
        observations.native_roots.push(native_root(NATIVE_ROOT, 0));
        observations.locator.stores.clear();

        let result = resolve_lifecycle_root(&observations);

        assert!(!result.valid);
        assert!(result.locator_consulted);
        assert_eq!(result.selected, None);
        assert_eq!(result.findings[0].code, ROOT_STORE_MISSING);
        assert_eq!(result.findings[0].status, DoctorFindingStatus::Fail);
    }

    #[test]
    fn conflicting_explicit_selectors_fail_before_lookup() {
        // r[verify cairn.lifecycle_stores.root_resolution.conflict]
        let mut observations = base_resolution_observations();
        observations.explicit_root = Some(ExplicitRootSelector {
            root: "/explicit".to_string(),
        });
        observations.explicit_lifecycle_store = Some(ExplicitLifecycleStoreSelector {
            store_id: STORE_ID.to_string(),
        });

        let result = resolve_lifecycle_root(&observations);

        assert!(!result.valid);
        assert!(!result.locator_consulted);
        assert_eq!(result.selected, None);
        assert_eq!(result.findings[0].code, ROOT_SELECTOR_CONFLICT);
        assert_eq!(result.findings[0].status, DoctorFindingStatus::Fail);
    }

    #[test]
    fn nearest_native_root_wins_and_stale_pointer_is_not_followed() {
        // r[verify cairn.lifecycle_stores.root_resolution.native_precedence]
        let mut observations = base_resolution_observations();
        observations
            .native_roots
            .push(native_root(PROJECT_ROOT, BOUNDARY_MAX_ANCESTORS));
        observations.native_roots.push(native_root(NATIVE_ROOT, 0));
        observations
            .project_pointers
            .push(project_pointer(STORE_ID, 0, true));
        observations.locator.stores.clear();

        let result = resolve_lifecycle_root(&observations);
        let selected = result.selected.expect("native root resolves");

        assert!(result.valid);
        assert!(!result.locator_consulted);
        assert_eq!(selected.root, NATIVE_ROOT);
        assert_eq!(selected.source, RootResolutionSource::NativeRoot);
        assert_eq!(result.findings[0].code, ROOT_PROJECT_POINTER_STALE);
        assert_eq!(result.findings[0].status, DoctorFindingStatus::Warn);
    }

    #[test]
    fn unknown_vcs_boundary_rejects_ancestor_discovery() {
        // r[verify cairn.lifecycle_stores.root_resolution.boundary]
        let mut observations = base_resolution_observations();
        observations.vcs_boundary = VcsBoundaryFacts::Unknown;
        observations.native_roots.push(native_root(PROJECT_ROOT, 0));
        observations
            .project_pointers
            .push(project_pointer(STORE_ID, 0, true));

        let result = resolve_lifecycle_root(&observations);
        let selected = result.selected.expect("classic fallback resolves");

        assert!(result.valid);
        assert!(!result.locator_consulted);
        assert_eq!(selected.root, CLASSIC_ROOT);
        assert_eq!(selected.source, RootResolutionSource::ClassicFallback);
        assert!(result.findings.is_empty());
    }

    #[test]
    fn stale_project_pointer_fails_without_fallback_or_locator_lookup() {
        // r[verify cairn.lifecycle_stores.project_config.invalid]
        let mut observations = base_resolution_observations();
        observations
            .project_pointers
            .push(project_pointer(STORE_ID, 0, false));

        let result = resolve_lifecycle_root(&observations);

        assert!(!result.valid);
        assert!(!result.locator_consulted);
        assert_eq!(result.selected, None);
        assert_eq!(result.findings[0].code, ROOT_PROJECT_POINTER_STALE);
        assert_eq!(result.findings[0].status, DoctorFindingStatus::Fail);
    }

    #[test]
    fn missing_reference_reports_manual_repair_without_mutation_authority() {
        // r[verify cairn.lifecycle_stores.context.missing]
        // r[verify cairn.lifecycle_stores.transport_boundary.context]
        let observations = ContextObservations {
            resolution: resolved_subject(),
            direct_references: vec![DirectStoreReference {
                store_id: STORE_ID.to_string(),
                remote_hint: Some(REMOTE_HINT.to_string()),
            }],
            locator: LocatorSnapshot::default(),
            accepted_specs: Vec::new(),
        };

        let result = build_lifecycle_store_context(&observations);
        let finding = &result.doctor_findings[0];

        assert!(result.valid);
        assert!(!result.healthy);
        assert!(result.specs.is_empty());
        assert_eq!(finding.code, REFERENCE_STORE_MISSING);
        assert_eq!(finding.status, DoctorFindingStatus::Warn);
        assert_eq!(
            finding.repair_command.as_deref(),
            Some("cairn store register <checkout-path> --id shared")
        );
        assert_eq!(finding.remote_hint.as_deref(), Some(REMOTE_HINT));
        assert_eq!(
            result.boundary.authority_effect,
            LifecycleAuthorityEffect::None
        );
    }

    #[test]
    fn duplicate_and_self_references_reject_the_whole_index() {
        // r[verify cairn.lifecycle_stores.project_config.invalid]
        let observations = ContextObservations {
            resolution: resolved_subject(),
            direct_references: vec![
                DirectStoreReference {
                    store_id: STORE_ID.to_string(),
                    remote_hint: None,
                },
                DirectStoreReference {
                    store_id: STORE_ID.to_string(),
                    remote_hint: Some(REMOTE_HINT.to_string()),
                },
                DirectStoreReference {
                    store_id: "subject".to_string(),
                    remote_hint: None,
                },
            ],
            locator: LocatorSnapshot {
                stores: vec![locator_store(STORE_ID, STORE_IDENTITY)],
            },
            accepted_specs: vec![accepted_snapshot(
                STORE_ID,
                STORE_IDENTITY,
                vec![spec("alpha", "Alpha")],
            )],
        };

        let result = build_lifecycle_store_context(&observations);
        let codes = result
            .doctor_findings
            .iter()
            .map(|finding| finding.code.as_str())
            .collect::<BTreeSet<_>>();

        assert!(!result.valid);
        assert!(!result.healthy);
        assert!(result.direct_references.is_empty());
        assert!(result.specs.is_empty());
        assert!(codes.contains(REFERENCE_DUPLICATE));
        assert!(codes.contains(REFERENCE_SELF));
    }

    #[test]
    fn context_is_sorted_and_uses_exact_store_qualified_fetch_commands() {
        // r[verify cairn.lifecycle_stores.references.index]
        // r[verify cairn.lifecycle_stores.context.healthy]
        let observations = ContextObservations {
            resolution: resolved_subject(),
            direct_references: vec![
                DirectStoreReference {
                    store_id: STORE_ID.to_string(),
                    remote_hint: None,
                },
                DirectStoreReference {
                    store_id: OTHER_STORE_ID.to_string(),
                    remote_hint: None,
                },
            ],
            locator: LocatorSnapshot {
                stores: vec![
                    locator_store(STORE_ID, STORE_IDENTITY),
                    locator_store(OTHER_STORE_ID, OTHER_STORE_IDENTITY),
                ],
            },
            accepted_specs: vec![
                accepted_snapshot(
                    STORE_ID,
                    STORE_IDENTITY,
                    vec![spec("zeta", "Zeta"), spec("alpha", "Alpha")],
                ),
                accepted_snapshot(OTHER_STORE_ID, OTHER_STORE_IDENTITY, Vec::new()),
            ],
        };

        let result = build_lifecycle_store_context(&observations);

        assert!(result.valid);
        assert!(result.healthy);
        assert_eq!(result.specs.len(), EXPECTED_CONTEXT_SPEC_COUNT);
        assert_eq!(result.specs[0].spec_id, "alpha");
        assert_eq!(result.specs[1].spec_id, "zeta");
        assert_eq!(
            result.specs[0].fetch_command,
            "cairn spec show alpha --lifecycle-store shared"
        );
        assert_eq!(
            result.specs[1].fetch_command,
            "cairn spec show zeta --lifecycle-store shared"
        );
        assert!(result
            .doctor_findings
            .windows(EXPECTED_CONTEXT_SPEC_COUNT)
            .all(|pair| pair[0].subject <= pair[1].subject));
    }

    #[test]
    fn context_uses_direct_accepted_specs_without_authority_promotion() {
        // r[verify cairn.lifecycle_stores.references.non_authority]
        // r[verify cairn.lifecycle_stores.boundary.bounded]
        let observations = ContextObservations {
            resolution: resolved_subject(),
            direct_references: vec![DirectStoreReference {
                store_id: STORE_ID.to_string(),
                remote_hint: None,
            }],
            locator: LocatorSnapshot {
                stores: vec![
                    locator_store(STORE_ID, STORE_IDENTITY),
                    locator_store(TRANSITIVE_STORE_ID, "b3:transitive"),
                ],
            },
            accepted_specs: vec![
                accepted_snapshot(STORE_ID, STORE_IDENTITY, vec![spec("direct", "Direct")]),
                accepted_snapshot(
                    TRANSITIVE_STORE_ID,
                    "b3:transitive",
                    vec![spec("not-loaded", "Not loaded")],
                ),
            ],
        };

        let result = build_lifecycle_store_context(&observations);

        assert!(result.valid);
        assert!(result.healthy);
        assert_eq!(result.specs.len(), 1);
        assert_eq!(result.specs[0].store_id, STORE_ID);
        assert_eq!(result.specs[0].spec_id, "direct");
        assert_eq!(
            result.boundary.traversal,
            ReferenceTraversalBoundary::DirectOnly
        );
        assert_eq!(
            result.boundary.content,
            ReferenceContentBoundary::AcceptedSpecsOnly
        );
        assert_eq!(
            result.boundary.registry_lookup,
            RegistryLookupBoundary::LocalLocatorOnly
        );
        assert_eq!(
            result.boundary.authority_effect,
            LifecycleAuthorityEffect::None
        );
    }
}
