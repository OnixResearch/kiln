use std::fs;

use kiln_web_service::worker::load_profile;

const PROFILE: &[u8] = include_bytes!("../../../fixtures/web-worker/profile.json");

// r[verify kiln.web.worker.scenario.denied]
#[test]
fn original_profile_bytes_are_authority_not_equivalent_reserialized_json() {
    let root = tempfile::tempdir()
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "fixture directory", error));
    let path = root.path().join("worker.json");
    fs::write(&path, PROFILE)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "profile bytes", error));
    let identity = blake3::hash(PROFILE).to_hex().to_string();
    load_profile(&path, &identity)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "admitted original bytes", error));
    let value: serde_json::Value = serde_json::from_slice(PROFILE)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "profile JSON", error));
    let equivalent = serde_json::to_vec(&value)
        .unwrap_or_else(|error| unreachable!("equivalent JSON: {error:?}"));
    fs::write(&path, equivalent)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "reserialized profile", error));
    assert!(load_profile(&path, &identity).is_err());
}

// r[verify kiln.web.worker.scenario.denied]
#[test]
fn pinned_profiles_reject_unknown_authority_fields_and_secret_forwarding() {
    let root = tempfile::tempdir()
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "fixture directory", error));
    let path = root.path().join("worker.json");
    let original: serde_json::Value = serde_json::from_slice(PROFILE)
        .unwrap_or_else(|error| unreachable!("{}: {:?}", "fixture JSON", error));
    for mutation in [
        "remote_command",
        "snapshot_override",
        "secret_environment",
        "artifact_escape",
    ] {
        let mut value = original.clone();
        match mutation {
            "remote_command" => {
                value
                    .as_object_mut()
                    .unwrap_or_else(|| unreachable!())
                    .insert("command_from_request".into(), true.into());
            }
            "snapshot_override" => {
                value
                    .get_mut("snapshot_limits")
                    .and_then(serde_json::Value::as_object_mut)
                    .unwrap_or_else(|| unreachable!())
                    .insert("allow_symlinks".into(), true.into());
            }
            "secret_environment" => {
                value
                    .pointer_mut("/steps/0/environment")
                    .and_then(serde_json::Value::as_object_mut)
                    .unwrap_or_else(|| unreachable!())
                    .insert("AUTH_TOKEN".into(), "forbidden".into());
            }
            "artifact_escape" => {
                *value
                    .pointer_mut("/steps/0/artifacts/0")
                    .unwrap_or_else(|| unreachable!()) = "../worker.token".into();
            }
            _ => unreachable!(),
        }
        let bytes = serde_json::to_vec(&value)
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "profile JSON", error));
        fs::write(&path, &bytes)
            .unwrap_or_else(|error| unreachable!("{}: {:?}", "mutated profile", error));
        let identity = blake3::hash(&bytes).to_hex().to_string();
        assert!(load_profile(&path, &identity).is_err(), "{mutation}");
    }
}
