use crate::{
    CELLD_LICENSE, CELLD_RELEASE, CELLD_REVISION, HMAC_SHA256_ID, HTTPS_SCHEME,
    STORE_SCHEMA_VERSION, WORKER_PROTOCOL_VERSION,
};
use kiln_core::types::{CoreResult, Diagnostic};

pub const CELLD_TRUST_DOMAIN_SINGLE: &str = "single";
pub const CELLD_TRUST_DOMAIN_HOSTILE: &str = "hostile-multi-tenant";

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
    if profile.origin.is_empty() || profile.path.is_empty() {
        return Err(Diagnostic::new(
            "celld_profile_transport",
            "celld.transport",
            "celld origin and path must not be empty",
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
    Ok(())
}

/// Shared positive fixture used across unit test modules.
#[cfg(test)]
pub fn profile_fixture() -> CelldProfile {
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
    fn admits_the_exact_cohort_profile() {
        assert_eq!(admit_profile(&profile()), Ok(()));
    }

    #[test]
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
