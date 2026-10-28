// r[verify kiln.web.developer_loop.cli]
use kiln_web_service::developer::{DeveloperProfile, load_profile, outcome_exit};
use std::os::unix::fs::PermissionsExt;

fn profile() -> DeveloperProfile {
    DeveloperProfile {
        schema: "kiln.web-developer.v1".into(),
        service_origin: "https://kiln.internal:8443".into(),
        token_file: "/run/credentials/kiln-developer/token".into(),
        tls_ca_path: "/etc/kiln/ca.pem".into(),
        http_timeout_ms: 1000,
        poll_interval_ms: 10,
        watch_timeout_ms: 10000,
        maximum_request_bytes: 1048576,
        maximum_response_bytes: 1048576,
    }
}
#[test]
fn reviewed_client_identity_admits_only_exact_original_profile_bytes() {
    let root = tempfile::tempdir()
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "profile directory", error));
    let path = root.path().join("developer.json");
    let bytes = serde_json::to_vec_pretty(&profile())
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "profile JSON", error));
    std::fs::write(&path, &bytes)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "profile write", error));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "profile permissions", error));
    let identity = blake3::hash(&bytes).to_hex().to_string();
    let admitted = load_profile(&path, &identity)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "exact reviewed profile", error));
    assert_eq!(admitted.service_origin, "https://kiln.internal:8443");
    let mut changed = bytes.clone();
    changed.push(b'\n');
    std::fs::write(&path, &changed)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "changed profile", error));
    assert!(load_profile(&path, &identity).is_err());
    std::fs::write(&path, &bytes)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "restored profile", error));
    let alias = root.path().join("alias.json");
    std::os::unix::fs::symlink(&path, &alias)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "profile symlink", error));
    assert!(load_profile(&alias, &identity).is_err());
}
#[test]
fn developer_transport_rejects_credentials_in_origin_insecure_schemes_and_unbounded_waits() {
    assert!(profile().admit().is_ok());
    for origin in [
        "http://kiln.internal",
        "https://user:secret@kiln.internal",
        "https://kiln.internal/path",
        "https://kiln.internal?token=secret",
        "https://kiln.internal/#fragment",
    ] {
        let mut rejected = profile();
        rejected.service_origin = origin.into();
        assert!(rejected.admit().is_err());
    }
    let mut rejected = profile();
    rejected.maximum_response_bytes = 0;
    assert!(rejected.admit().is_err());
    let mut rejected = profile();
    rejected.watch_timeout_ms = 86_400_001;
    assert!(rejected.admit().is_err());
    let mut rejected = profile();
    rejected.poll_interval_ms = rejected.watch_timeout_ms + 1;
    assert!(rejected.admit().is_err());
    let mut rejected = profile();
    rejected.token_file = "token".into();
    assert!(rejected.admit().is_err());
}
#[test]
fn run_outcomes_preserve_failure_cancel_and_unknown_exit_semantics() {
    assert_eq!(outcome_exit("Pending"), Ok(None));
    assert_eq!(outcome_exit("Success"), Ok(Some(0)));
    for failed in ["DomainFailure", "Denied", "TimedOut", "ProviderError"] {
        assert_eq!(outcome_exit(failed), Ok(Some(1)));
    }
    assert_eq!(outcome_exit("Cancelled"), Ok(Some(130)));
    assert_eq!(outcome_exit("Unknown"), Ok(Some(3)));
    assert!(outcome_exit("future-unknown-value").is_err());
}
