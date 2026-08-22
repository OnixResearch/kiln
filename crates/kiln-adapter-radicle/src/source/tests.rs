use std::path::PathBuf;

use kiln_radicle_source_contract::fixture_request;

use super::*;

fn profile() -> RadicleSourceProfile {
    RadicleSourceProfile::pinned(
        PathBuf::from("/var/lib/radicle/storage/repository.git"),
        fixture_request(),
    )
}

// r[verify kiln.radicle.shell_profile.scenario.narrow]
// r[verify kiln.radicle.shell_conformance]
// r[verify kiln.radicle.shell_nonclaims.scenario.local]
#[test]
fn pinned_source_profile_is_narrow_and_admitted() {
    let profile = profile();
    let admitted = admit_source_profile(&profile);

    assert!(admitted.is_ok());
    assert!(!profile.allow_network);
    assert!(!profile.allow_credentials);
    assert!(!profile.allow_signing);
    assert!(!profile.allow_mutation);
    assert!(!profile.allow_collaboration);
}

// r[verify kiln.radicle.shell_profile.scenario.fixture]
// r[verify kiln.radicle.shell_conformance]
#[test]
fn authority_contract_and_cohort_drift_fail_closed() {
    let mut authority = profile();
    authority.allow_network = true;
    assert!(admit_source_profile(&authority).is_err());

    let mut contract = profile();
    contract.contract_revision = "floating-main".to_string();
    assert!(admit_source_profile(&contract).is_err());

    let mut cohort = profile();
    cohort.cohort.sqlite_backend = true;
    assert!(admit_source_profile(&cohort).is_err());

    let mut path = profile();
    path.repository_path = PathBuf::from("relative.git");
    assert!(admit_source_profile(&path).is_err());
}

// r[verify kiln.radicle.shell_conformance]
#[test]
fn malformed_native_identities_fail_before_repository_access() {
    let result = RadicleSourceShell::new(profile());
    assert!(result.is_err());

    let mut invalid_oid = profile();
    invalid_oid.request.repository_id = "rad:z3gqcJUoA1n9HaHKufZs5FCSGazv5".to_string();
    invalid_oid.request.expected_revision = "not-an-oid".to_string();
    assert!(RadicleSourceShell::new(invalid_oid).is_err());
}
