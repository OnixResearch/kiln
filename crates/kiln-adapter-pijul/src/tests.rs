use std::cell::Cell;

use kiln_core::source::{PijulEntryKind, PijulSnapshotEntry, SourceCapability, SourceNativeFacts};
use kiln_core::types::{Diagnostic, ExternalRef};
use kiln_provider_api::{
    InfrastructureFailure, PijulReadObservation, PijulReadRequest, PijulRepositoryPort,
    PortFailure, PortResult,
};

use super::*;

const EXPECTED_CONTENT_ID_LENGTH: usize = 64;
const EXPECTED_RESOLUTION_CALLS: usize = 2;

fn external(kind: &str, value: &str) -> ExternalRef {
    ExternalRef::new("pijul", kind, value).unwrap_or_else(|_| unreachable!())
}

fn observation(state: &str, change: &str, content: &[u8]) -> PijulReadObservation {
    PijulReadObservation {
        repository: external("repository", "fixture"),
        channel: external("channel", "main"),
        observed_state_before: external("state", state),
        observed_state_after: external("state", state),
        change_sequence: vec![external("change", change)],
        format_cohort: external("format-cohort", PINNED_FORMAT_COHORT),
        entries: vec![PijulSnapshotEntry {
            path: "src/lib.rs".to_string(),
            kind: PijulEntryKind::Regular,
            bytes: content.to_vec(),
        }],
    }
}

#[derive(Debug)]
struct FixtureRepository {
    channels: Vec<ExternalRef>,
    observation: PijulReadObservation,
    failure: Option<PortFailure>,
    calls: Cell<usize>,
}

impl PijulRepositoryPort for FixtureRepository {
    fn discover_channels(&self, _max_channels: usize) -> PortResult<Vec<ExternalRef>> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        Ok(self.channels.clone())
    }

    fn resolve_and_acquire(&self, _request: &PijulReadRequest) -> PortResult<PijulReadObservation> {
        self.calls.set(self.calls.get() + 1);
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        Ok(self.observation.clone())
    }
}

fn fixture(observation: PijulReadObservation) -> FixtureRepository {
    FixtureRepository {
        channels: vec![external("channel", "main")],
        observation,
        failure: None,
        calls: Cell::new(0),
    }
}

fn resolve(
    repository: &FixtureRepository,
    profile: &PijulSourceProfile,
) -> PortResult<kiln_core::source::SourceResolution> {
    super::resolve_and_acquire(
        repository,
        external("channel", "main"),
        profile.id.clone(),
        profile,
    )
}

// r[verify kiln.source.pijul_configuration.scenario.compatible]
// r[verify kiln.source.pijul_capabilities.scenario.read]
// r[verify kiln.source.pijul_capabilities.scenario.mutation]
// r[verify kiln.source.pijul_nonclaims.scenario.local]
#[test]
fn pinned_profile_is_exact_and_read_only() {
    let profile = PijulSourceProfile::pinned_local();
    let admitted = admit_profile(&profile);
    assert!(admitted.is_ok());
    assert!(!profile_has_forbidden_authority(&profile));
    for denied in [
        SourceCapability::UseCredentials,
        SourceCapability::Collaborate,
        SourceCapability::ReceiveTrigger,
        SourceCapability::PublishStatus,
        SourceCapability::Mutate,
    ] {
        assert!(!profile.capabilities.contains(&denied));
    }
}

// r[verify kiln.source.pijul_configuration.scenario.drift]
#[test]
fn profile_rejects_version_format_capability_and_bound_drift() {
    let mut profile = PijulSourceProfile::pinned_local();
    profile.libpijul_version = "1.0.0-beta.10".to_string();
    assert!(admit_profile(&profile).is_err());
    profile = PijulSourceProfile::pinned_local();
    profile.format_cohort = "unknown".to_string();
    assert!(admit_profile(&profile).is_err());
    profile = PijulSourceProfile::pinned_local();
    profile.capabilities.insert(SourceCapability::Mutate);
    assert!(admit_profile(&profile).is_err());
    profile = PijulSourceProfile::pinned_local();
    profile.limits.max_entries = 0;
    assert!(admit_profile(&profile).is_err());
    profile = PijulSourceProfile::pinned_local();
    profile.max_repository_bytes = 0;
    assert!(admit_profile(&profile).is_err());
    profile = PijulSourceProfile::pinned_local();
    profile.allow_credentials = true;
    assert!(admit_profile(&profile).is_err());
}

// r[verify kiln.source.pijul_adapter.scenario.resolve]
// r[verify kiln.source.pijul_conformance.scenario.repeat]
#[test]
fn exact_native_observation_resolves_repeatably() {
    let profile = PijulSourceProfile::pinned_local();
    let repository = fixture(observation("state-a", "change-a", b"content"));
    let channels = discover_channels(&repository, &profile);
    assert_eq!(channels, Ok(vec![external("channel", "main")]));
    let first = resolve(&repository, &profile);
    let second = resolve(&repository, &profile);
    assert_eq!(first, second);
    assert!(first.is_ok_and(|resolution| {
        resolution.exact_revision == external("state", "state-a")
            && resolution.canonical_tree.as_str().len() == EXPECTED_CONTENT_ID_LENGTH
            && matches!(resolution.native_facts, SourceNativeFacts::Pijul(_))
    }));
    assert_eq!(repository.calls.get(), EXPECTED_RESOLUTION_CALLS);
}

// r[verify kiln.source.pijul_snapshot.scenario.history]
#[test]
fn equal_snapshots_do_not_collapse_distinct_pijul_histories() {
    let profile = PijulSourceProfile::pinned_local();
    let first = resolve(
        &fixture(observation("state-a", "change-a", b"same")),
        &profile,
    )
    .unwrap_or_else(|_| unreachable!());
    let second = resolve(
        &fixture(observation("state-b", "change-b", b"same")),
        &profile,
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(first.canonical_tree, second.canonical_tree);
    assert_ne!(first.exact_revision, second.exact_revision);
    assert_ne!(first.native_facts, second.native_facts);
}

// r[verify kiln.source.pijul_state_identity.scenario.moving]
// r[verify kiln.source.pijul_state_identity.scenario.incomplete]
#[test]
fn moving_or_substituted_state_fails_without_partial_success() {
    let profile = PijulSourceProfile::pinned_local();
    let mut moved = observation("state-a", "change-a", b"content");
    moved.observed_state_after = external("state", "state-b");
    assert!(resolve(&fixture(moved), &profile).is_err());

    let mut substituted = observation("state-a", "change-a", b"content");
    substituted.channel = external("channel", "other");
    assert!(resolve(&fixture(substituted), &profile).is_err());
}

// r[verify kiln.source.pijul_snapshot.scenario.entry]
// r[verify kiln.source.pijul_conformance.scenario.reject]
#[test]
fn malformed_and_bounded_snapshots_fail_closed() {
    let profile = PijulSourceProfile::pinned_local();
    let mut unsafe_path = observation("state-a", "change-a", b"content");
    unsafe_path
        .entries
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .path = "../secret".to_string();
    assert!(resolve(&fixture(unsafe_path), &profile).is_err());

    let mut duplicate = observation("state-a", "change-a", b"content");
    let duplicate_entry = duplicate
        .entries
        .first()
        .cloned()
        .unwrap_or_else(|| unreachable!());
    duplicate.entries.push(duplicate_entry);
    assert!(resolve(&fixture(duplicate), &profile).is_err());

    let mut oversized = observation("state-a", "change-a", b"content");
    oversized
        .entries
        .first_mut()
        .unwrap_or_else(|| unreachable!())
        .bytes = vec![0; profile.limits.max_file_bytes + 1];
    assert!(resolve(&fixture(oversized), &profile).is_err());
}

#[test]
fn missing_busy_and_damaged_provider_failures_remain_distinct() {
    let profile = PijulSourceProfile::pinned_local();
    for failure in [
        PortFailure::Domain(Diagnostic::new(
            "pijul_channel_missing",
            "pijul.channel",
            "channel missing",
        )),
        PortFailure::Retryable(InfrastructureFailure {
            code: "pijul_repository_busy".to_string(),
            message: "repository busy".to_string(),
        }),
        PortFailure::Infrastructure(InfrastructureFailure {
            code: "pijul_repository_damaged".to_string(),
            message: "repository damaged".to_string(),
        }),
    ] {
        let repository = FixtureRepository {
            channels: Vec::new(),
            observation: observation("state-a", "change-a", b"content"),
            failure: Some(failure.clone()),
            calls: Cell::new(0),
        };
        assert_eq!(resolve(&repository, &profile), Err(failure));
    }
}

// r[verify kiln.source.pijul_nonclaims.scenario.dependency]
#[test]
fn core_standalone_and_cli_remain_independent_of_pijul_runtime() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for relative in [
        "crates/kiln-core/Cargo.toml",
        "crates/kiln-provider-api/Cargo.toml",
        "crates/kiln-standalone/Cargo.toml",
        "crates/kiln-cli/Cargo.toml",
    ] {
        let manifest =
            std::fs::read_to_string(root.join(relative)).unwrap_or_else(|_| unreachable!());
        assert!(!manifest.contains("libpijul"));
        assert!(!manifest.contains("pijul-repository"));
        assert!(!manifest.contains("kiln-adapter-pijul"));
    }
}

// r[verify kiln.source.pijul_adapter.scenario.shell]
#[test]
fn concrete_shell_uses_native_library_without_mutation_or_network_surface() {
    let source = include_str!("local.rs");
    assert!(source.contains("Repository::find_root"));
    assert!(source.contains("arc_txn_begin"));
    assert!(source.contains(".archive::<"));
    for forbidden in [
        "std::process",
        "std::net",
        ".commit(",
        "open_or_create_channel",
        "apply_change",
        "unrecord(",
        "record::",
        "pijul_remote",
        "Repository::clone",
        "push_to_remote",
        "pull_from_remote",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden Pijul adapter surface: {forbidden}"
        );
    }
}
