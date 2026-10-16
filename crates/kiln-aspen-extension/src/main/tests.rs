use super::*;
use kiln_adapter_aspen::profile::{admit_profile, profile_document_fixture};

const TEST_CALLBACK_TIMEOUT_MILLISECONDS: u64 = 7_290_000;
const ZERO_CALLBACK_TIMEOUT_MILLISECONDS: u64 = 0;
const OVERSIZED_PROFILE_EXTRA_BYTES: u64 = 1;
const MISMATCHED_PROFILE_IDENTITY: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";
const MALFORMED_PROFILE_BYTES: &[u8] = b"not-json";

fn complete_arguments(path: &std::path::Path, identity: &str) -> Vec<String> {
    [
        RUNTIME_PROFILE_ARGUMENT.to_string(),
        path.to_string_lossy().into_owned(),
        RUNTIME_PROFILE_IDENTITY_ARGUMENT.to_string(),
        identity.to_string(),
    ]
    .into_iter()
    .collect()
}

fn write_non_default_profile(directory: &tempfile::TempDir) -> (std::path::PathBuf, String) {
    let mut document = profile_document_fixture();
    document.bounds.callback_timeout_millis = TEST_CALLBACK_TIMEOUT_MILLISECONDS;
    let profile = document
        .clone()
        .into_runtime_profile()
        .unwrap_or_else(|_| unreachable!("non-default profile document must convert"));
    let admitted =
        admit_profile(&profile).unwrap_or_else(|_| unreachable!("non-default profile must admit"));
    let path = directory.path().join("runtime-profile.json");
    let bytes = serde_json::to_vec(&document)
        .unwrap_or_else(|_| unreachable!("profile document must encode"));
    std::fs::write(&path, bytes).unwrap_or_else(|_| unreachable!("profile document must write"));
    (path, admitted.profile_identity.as_str().to_string())
}

// r[verify kiln.aspen_host.extension_profile_binding.accepted]
#[test]
fn exact_non_default_runtime_profile_is_admitted() {
    let directory = tempfile::tempdir()
        .unwrap_or_else(|_| unreachable!("temporary profile directory must exist"));
    let (path, identity) = write_non_default_profile(&directory);
    let arguments = parse_arguments(complete_arguments(&path, &identity).into_iter())
        .unwrap_or_else(|_| unreachable!("complete exact arguments must admit"));
    let admitted = load_runtime_profile(&arguments)
        .unwrap_or_else(|_| unreachable!("matching runtime profile must admit"));
    assert_eq!(
        admitted.profile.bounds.callback_timeout_millis,
        TEST_CALLBACK_TIMEOUT_MILLISECONDS,
    );
    assert_eq!(admitted.profile_identity.as_str(), identity);
}

// r[verify kiln.aspen_host.extension_profile_binding.rejected]
#[test]
fn runtime_profile_arguments_reject_missing_duplicate_and_unsafe_values() {
    let directory = tempfile::tempdir()
        .unwrap_or_else(|_| unreachable!("temporary profile directory must exist"));
    let (path, identity) = write_non_default_profile(&directory);
    let missing_identity = [
        RUNTIME_PROFILE_ARGUMENT.to_string(),
        path.to_string_lossy().into_owned(),
    ];
    assert!(parse_arguments(missing_identity.into_iter()).is_err());

    let mut duplicate = complete_arguments(&path, &identity);
    duplicate.extend([
        RUNTIME_PROFILE_ARGUMENT.to_string(),
        path.to_string_lossy().into_owned(),
    ]);
    assert!(parse_arguments(duplicate.into_iter()).is_err());

    let relative = complete_arguments(std::path::Path::new("runtime-profile.json"), &identity);
    assert!(parse_arguments(relative.into_iter()).is_err());

    let unknown = [
        "--other-profile".to_string(),
        path.to_string_lossy().into_owned(),
    ];
    assert!(parse_arguments(unknown.into_iter()).is_err());
}

// r[verify kiln.aspen_host.extension_profile_binding.rejected]
#[test]
fn runtime_profile_rejects_changed_invalid_missing_and_oversized_documents() {
    let directory = tempfile::tempdir()
        .unwrap_or_else(|_| unreachable!("temporary profile directory must exist"));
    let (path, identity) = write_non_default_profile(&directory);

    let mismatch =
        parse_arguments(complete_arguments(&path, MISMATCHED_PROFILE_IDENTITY).into_iter())
            .unwrap_or_else(|_| unreachable!("well-formed mismatch arguments must parse"));
    assert!(load_runtime_profile(&mismatch).is_err());

    std::fs::write(&path, MALFORMED_PROFILE_BYTES)
        .unwrap_or_else(|_| unreachable!("malformed fixture must write"));
    let malformed = parse_arguments(complete_arguments(&path, &identity).into_iter())
        .unwrap_or_else(|_| unreachable!("malformed document arguments must parse"));
    assert!(load_runtime_profile(&malformed).is_err());

    let missing_path = directory.path().join("missing-profile.json");
    let missing = parse_arguments(complete_arguments(&missing_path, &identity).into_iter())
        .unwrap_or_else(|_| unreachable!("missing document arguments must parse"));
    assert!(load_runtime_profile(&missing).is_err());

    let oversized_length = PROFILE_MAXIMUM_BYTES
        .checked_add(OVERSIZED_PROFILE_EXTRA_BYTES)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_else(|| unreachable!("profile test bound must fit usize"));
    std::fs::write(&path, vec![b'x'; oversized_length])
        .unwrap_or_else(|_| unreachable!("oversized fixture must write"));
    let oversized = parse_arguments(complete_arguments(&path, &identity).into_iter())
        .unwrap_or_else(|_| unreachable!("oversized document arguments must parse"));
    assert!(load_runtime_profile(&oversized).is_err());

    let mut invalid_document = profile_document_fixture();
    invalid_document.bounds.callback_timeout_millis = ZERO_CALLBACK_TIMEOUT_MILLISECONDS;
    let invalid_bytes = serde_json::to_vec(&invalid_document)
        .unwrap_or_else(|_| unreachable!("invalid profile fixture must encode"));
    std::fs::write(&path, invalid_bytes)
        .unwrap_or_else(|_| unreachable!("invalid profile fixture must write"));
    let invalid = parse_arguments(complete_arguments(&path, &identity).into_iter())
        .unwrap_or_else(|_| unreachable!("invalid document arguments must parse"));
    assert!(load_runtime_profile(&invalid).is_err());
}
