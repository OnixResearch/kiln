use crate::{
    CELLD_LICENSE, CELLD_RELEASE, CELLD_REVISION, HMAC_SHA256_ID, HTTPS_SCHEME,
    STORE_SCHEMA_VERSION, WORKER_PROTOCOL_VERSION,
};
use kiln_core::types::{CoreResult, Diagnostic};
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

pub const CELLD_TRUST_DOMAIN_SINGLE: &str = "single";
pub const CELLD_TRUST_DOMAIN_HOSTILE: &str = "hostile-multi-tenant";
pub const COHORT_OBSERVATION_MAX_AGE_SECONDS: u64 = 300;

/// Operator-selected local instance, not a Celld-signed node attestation.
/// The shell measures this identity independently of a Worker request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CelldCohortIdentity {
    pub artifact_blake3: String,
    pub platform: String,
    pub worker_bundle_blake3: String,
    pub deployment_id: String,
    pub storage_cohort: String,
    pub ingress_ca_blake3: String,
}

/// Fresh startup observations supplied by the trusted operator shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CelldCohortObservation {
    pub identity: CelldCohortIdentity,
    pub release: String,
    pub revision: String,
    pub worker_protocol_version: u16,
    pub schema_version: u16,
    pub origin: String,
    pub output_gate_enabled: bool,
    pub storage_probe_succeeded: bool,
    pub observed_at_seconds: u64,
}

// r[impl kiln.celld.cohort]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CelldProfile {
    pub celld_release: String,
    pub celld_revision: String,
    pub license: String,
    pub worker_protocol_version: u16,
    pub schema_version: u16,
    pub origin: String,
    pub path: String,
    pub tls_policy: String,
    pub auth_profile: String,
    pub redirects_disabled: bool,
    pub trust_domain: String,
    pub output_gate_enabled: bool,
    pub storage_probe_required: bool,
    pub bounds: CelldProfileBounds,
    pub selected_cohort: CelldCohortIdentity,
    pub observed_cohort: CelldCohortObservation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CelldProfileBounds {
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_replay_page_bytes: usize,
    pub max_events_per_append: usize,
    pub max_outbox_records: usize,
    pub max_event_bytes: usize,
    pub max_partition_count: usize,
    pub max_lease_ticks: u64,
    pub max_migration_steps: usize,
    pub timeout_millis: u64,
}

fn is_blake3(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn admit_observation(profile: &CelldProfile) -> CoreResult<()> {
    let selected = &profile.selected_cohort;
    let observed = &profile.observed_cohort;
    let platform_matches = selected
        .platform
        .strip_prefix(std::env::consts::OS)
        .and_then(|suffix| suffix.strip_prefix('-'))
        == Some(std::env::consts::ARCH);
    if !is_blake3(&selected.artifact_blake3)
        || !is_blake3(&selected.worker_bundle_blake3)
        || !is_blake3(&selected.ingress_ca_blake3)
        || selected.deployment_id.is_empty()
        || selected.storage_cohort.is_empty()
        || !platform_matches
        || observed.identity != *selected
        || observed.release != profile.celld_release
        || observed.revision != profile.celld_revision
        || observed.worker_protocol_version != profile.worker_protocol_version
        || observed.schema_version != profile.schema_version
        || observed.origin != profile.origin
    {
        return Err(Diagnostic::new(
            "celld_cohort_observation",
            "celld.cohort",
            "selected artifact, platform, Worker deployment, schema, storage or ingress differs from fresh trusted-operator observations",
        ));
    }
    if !observed.output_gate_enabled {
        return Err(Diagnostic::new(
            "celld_output_gate",
            "celld.storage",
            "observed Celld output gating is disabled",
        ));
    }
    if !observed.storage_probe_succeeded {
        return Err(Diagnostic::new(
            "celld_storage_probe",
            "celld.storage",
            "observed conditional-write probe did not succeed",
        ));
    }
    Ok(())
}
pub fn admit_profile(profile: &CelldProfile) -> CoreResult<()> {
    if profile.celld_release != CELLD_RELEASE
        || profile.celld_revision != CELLD_REVISION
        || profile.license != CELLD_LICENSE
        || profile.worker_protocol_version != WORKER_PROTOCOL_VERSION
        || profile.schema_version != STORE_SCHEMA_VERSION
        || profile.tls_policy != HTTPS_SCHEME
        || profile.auth_profile != HMAC_SHA256_ID
        || !profile.redirects_disabled
    {
        return Err(Diagnostic::new(
            "celld_profile_drift",
            "celld.profile",
            "celld release, revision, protocol, schema, TLS, auth, or redirect policy differs from the pinned profile",
        ));
    }
    // A Worker route can never turn normal event-store traffic into a
    // celld private control request, nor inherit a caller-supplied URL.
    let admitted_origin = profile
        .origin
        .strip_prefix("https://")
        .is_some_and(|authority| {
            !authority.is_empty()
                && authority
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b':'))
        });
    if !admitted_origin || profile.path != "/v1/append" {
        return Err(Diagnostic::new(
            "celld_profile_transport",
            "celld.transport",
            "celld requires a complete HTTPS authority and its exact Worker append path",
        ));
    }
    match profile.trust_domain.as_str() {
        CELLD_TRUST_DOMAIN_SINGLE => {}
        CELLD_TRUST_DOMAIN_HOSTILE => {
            return Err(Diagnostic::new(
                "celld_hostile_tenants",
                "celld.trust_domain",
                "hostile multi-tenant isolation is not claimed for the initial celld alpha cohort",
            ));
        }
        _ => {
            return Err(Diagnostic::new(
                "celld_profile_trust",
                "celld.trust_domain",
                "celld trust domain is not declared",
            ));
        }
    }
    if !profile.output_gate_enabled {
        return Err(Diagnostic::new(
            "celld_output_gate",
            "celld.storage",
            "celld adapter requires enabled output gating",
        ));
    }
    if !profile.storage_probe_required {
        return Err(Diagnostic::new(
            "celld_storage_probe",
            "celld.storage",
            "celld adapter requires a conditional-write storage probe",
        ));
    }
    let bounds = &profile.bounds;
    if bounds.max_request_bytes == 0
        || bounds.max_response_bytes == 0
        || bounds.max_replay_page_bytes == 0
        || bounds.max_events_per_append == 0
        || bounds.max_outbox_records == 0
        || bounds.max_event_bytes == 0
        || bounds.max_partition_count == 0
        || bounds.max_lease_ticks == 0
        || bounds.max_migration_steps == 0
        || bounds.timeout_millis == 0
    {
        return Err(Diagnostic::new(
            "celld_profile_bound",
            "celld.bounds",
            "celld profile bounds must be positive",
        ));
    }
    admit_observation(profile)?;
    Ok(())
}

/// Pure admission of an operator observation at shell-supplied time.
pub fn admit_profile_at(profile: &CelldProfile, now_seconds: u64) -> CoreResult<()> {
    admit_profile(profile)?;
    let observed_at = profile.observed_cohort.observed_at_seconds;
    if observed_at > now_seconds || now_seconds - observed_at > COHORT_OBSERVATION_MAX_AGE_SECONDS {
        return Err(Diagnostic::new(
            "celld_cohort_observation",
            "celld.cohort",
            "trusted-operator cohort observation is missing, stale, or from the future",
        ));
    }
    Ok(())
}

/// Shared positive fixture used across unit test modules.
#[cfg(test)]
pub fn profile_fixture() -> CelldProfile {
    let identity = CelldCohortIdentity {
        artifact_blake3: "1".repeat(64),
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        worker_bundle_blake3: "2".repeat(64),
        deployment_id: "fixture-deployment".to_string(),
        storage_cohort: "s3://fixture".to_string(),
        ingress_ca_blake3: "3".repeat(64),
    };
    let observed_at_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| unreachable!("fixture clock after epoch"))
        .as_secs();
    let observed_cohort = CelldCohortObservation {
        identity: identity.clone(),
        release: CELLD_RELEASE.to_string(),
        revision: CELLD_REVISION.to_string(),
        worker_protocol_version: WORKER_PROTOCOL_VERSION,
        schema_version: STORE_SCHEMA_VERSION,
        origin: "https://kiln.invalid".to_string(),
        output_gate_enabled: true,
        storage_probe_succeeded: true,
        observed_at_seconds,
    };
    CelldProfile {
        celld_release: CELLD_RELEASE.to_string(),
        celld_revision: CELLD_REVISION.to_string(),
        license: CELLD_LICENSE.to_string(),
        worker_protocol_version: WORKER_PROTOCOL_VERSION,
        schema_version: STORE_SCHEMA_VERSION,
        origin: "https://kiln.invalid".to_string(),
        path: "/v1/append".to_string(),
        tls_policy: HTTPS_SCHEME.to_string(),
        auth_profile: HMAC_SHA256_ID.to_string(),
        redirects_disabled: true,
        trust_domain: CELLD_TRUST_DOMAIN_SINGLE.to_string(),
        output_gate_enabled: true,
        storage_probe_required: true,
        selected_cohort: identity,
        observed_cohort,
        bounds: CelldProfileBounds {
            max_request_bytes: 4 * 1024 * 1024,
            max_response_bytes: 4 * 1024 * 1024,
            max_replay_page_bytes: 4 * 1024 * 1024,
            max_events_per_append: 8,
            max_outbox_records: 64,
            max_event_bytes: 4096,
            max_partition_count: 16,
            max_lease_ticks: 30,
            max_migration_steps: 4,
            timeout_millis: 5000,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> CelldProfile {
        profile_fixture()
    }

    #[test]
    fn rejects_unobserved_artifact_deployment_storage_and_ingress_changes() {
        let original = profile().selected_cohort;
        let mut changes = Vec::new();
        let mut artifact = original.clone();
        artifact.artifact_blake3 = "a".repeat(64);
        changes.push(artifact);
        let mut deployment = original.clone();
        deployment.deployment_id = "other-deployment".to_string();
        changes.push(deployment);
        let mut storage = original.clone();
        storage.storage_cohort = "s3://other".to_string();
        changes.push(storage);
        let mut ingress = original;
        ingress.ingress_ca_blake3 = "b".repeat(64);
        changes.push(ingress);
        for changed in changes {
            let mut selected = profile();
            selected.observed_cohort.identity = changed;
            assert!(matches!(
                admit_profile(&selected),
                Err(error) if error.code == "celld_cohort_observation"
            ));
        }
    }

    // r[verify kiln.celld.local_worker_evidence.scenario.runtime_gap]
    #[test]
    fn local_sqlite_only_cannot_admit_an_unobserved_runtime() {
        let mut local_only = profile();
        local_only.observed_cohort.identity.artifact_blake3.clear();
        assert!(matches!(
            admit_profile_at(&local_only, local_only.observed_cohort.observed_at_seconds),
            Err(error) if error.code == "celld_cohort_observation"
        ));
        let mut local_only = profile();
        let after_window =
            local_only.observed_cohort.observed_at_seconds + COHORT_OBSERVATION_MAX_AGE_SECONDS + 1;
        local_only.observed_cohort.observed_at_seconds = 0;
        assert!(matches!(
            admit_profile_at(&local_only, after_window),
            Err(error) if error.code == "celld_cohort_observation"
        ));
    }

    #[test]
    fn rejects_stale_cohort_observations_and_observed_disabled_storage() {
        let selected = profile();
        let stale_at =
            selected.observed_cohort.observed_at_seconds + COHORT_OBSERVATION_MAX_AGE_SECONDS + 1;
        assert!(matches!(
            admit_profile_at(&selected, stale_at),
            Err(error) if error.code == "celld_cohort_observation"
        ));
        let mut selected = profile();
        selected.observed_cohort.output_gate_enabled = false;
        assert!(matches!(
            admit_profile(&selected),
            Err(error) if error.code == "celld_output_gate"
        ));
        let mut selected = profile();
        selected.observed_cohort.storage_probe_succeeded = false;
        assert!(matches!(
            admit_profile(&selected),
            Err(error) if error.code == "celld_storage_probe"
        ));
    }

    #[test]
    // r[verify kiln.celld.cohort.scenario.output_gate]
    fn rejects_output_gate_disabled() {
        let mut profile = profile();
        profile.output_gate_enabled = false;
        let error = admit_profile(&profile)
            .err()
            .unwrap_or_else(|| unreachable!("output gate must be required"));
        assert_eq!(error.code, "celld_output_gate");
    }

    #[test]
    fn rejects_storage_probe_disabled() {
        let mut profile = profile();
        profile.storage_probe_required = false;
        let error = admit_profile(&profile)
            .err()
            .unwrap_or_else(|| unreachable!("storage probe must be required"));
        assert_eq!(error.code, "celld_storage_probe");
    }

    #[test]
    // r[verify kiln.celld.authority.scenario.tenant]
    fn rejects_hostile_tenants_on_the_alpha_cohort() {
        let mut profile = profile();
        profile.trust_domain = CELLD_TRUST_DOMAIN_HOSTILE.to_string();
        let error = admit_profile(&profile)
            .err()
            .unwrap_or_else(|| unreachable!("hostile tenants are rejected on alpha"));
        assert_eq!(error.code, "celld_hostile_tenants");
    }

    #[test]
    fn rejects_drifted_release() {
        let mut profile = profile();
        profile.celld_revision = "wrong".to_string();
        let error = admit_profile(&profile)
            .err()
            .unwrap_or_else(|| unreachable!("drifted revision must fail fast"));
        assert_eq!(error.code, "celld_profile_drift");
    }

    #[test]
    fn rejects_private_route_and_untrusted_origin_before_worker_dispatch() {
        let mut selected = profile();
        selected.path = "/state".to_string();
        assert!(matches!(
            admit_profile(&selected),
            Err(error) if error.code == "celld_profile_transport"
        ));
        selected.path = "/v1/append".to_string();
        for origin in [
            "http://kiln.invalid",
            "https://kiln.invalid/other",
            "https://user@kiln.invalid",
        ] {
            selected.origin = origin.to_string();
            assert!(matches!(
                admit_profile(&selected),
                Err(error) if error.code == "celld_profile_transport"
            ));
        }
    }

    #[test]
    fn rejects_zero_bounds() {
        let mut profile = profile();
        profile.bounds.max_events_per_append = 0;
        let error = admit_profile(&profile)
            .err()
            .unwrap_or_else(|| unreachable!("zero bounds must fail fast"));
        assert_eq!(error.code, "celld_profile_bound");
    }

    #[test]
    fn rejects_undeclared_trust_domain() {
        let mut profile = profile();
        profile.trust_domain = "unknowable".to_string();
        let error = admit_profile(&profile)
            .err()
            .unwrap_or_else(|| unreachable!("unknown trust domain must fail"));
        assert_eq!(error.code, "celld_profile_trust");
    }
}
