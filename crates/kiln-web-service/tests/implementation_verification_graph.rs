// r[verify kiln.implementation.verification_graph_cli.scenario.constructed] r[verify kiln.implementation.verification_graph_cli.scenario.refused]
//! The installed `kiln-web-service implementation-verification-graph` command.
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;

fn write(path: &Path, value: &Value) {
    std::fs::write(
        path,
        serde_json::to_vec(value).unwrap_or_else(|error| unreachable!("{error}")),
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
}

fn template(root: &Path, nodes: Value, edges: Value) -> Value {
    json!({
        "schema":"kiln.web-service.v1","service_id":"dedicated-verification","listen_address":"127.0.0.1:9443",
        "origin":"https://localhost:9443","tls":{"certificate_path":root.join("cert.pem"),"private_key_path":root.join("key.pem"),"min_version":"TLS1.3"},
        "operator_token_file":root.join("operator.token"),"worker_token_file":root.join("worker.token"),"state_directory":root.join("state"),
        "projects":[{"id":"verification","label":"Dedicated verification","repository_path":root.join("repository"),"reference":"refs/heads/main",
            "source_limits":{"max_entries":32,"max_path_bytes":1024,"max_file_bytes":4096,"max_total_bytes":16384},
            "workflow":{"socket_path":root.join("lattice.sock"),"allowed_user_ids":[933],
                "transport_limits":{"maximum_frame_bytes":1048576,"maximum_log_bytes":65536,"maximum_connections":4,"read_deadline_milliseconds":1000,"write_deadline_milliseconds":1000},
                "adapter_limits":{"maximum_request_bytes":1048576,"maximum_response_bytes":1048576},"policy_id":"reviewed-policy","provider_profile_id":"reviewed-lattice","platform":"linux-x86_64",
                "nodes":nodes,"edges":edges,
                "controls":{"cancel":true,"pause":false,"resume":false},"bounds":{"max_nodes":8,"max_edges":16,"max_log_references":32,"max_log_read_bytes":4096,"max_total_log_bytes":65536}}}],
        "bounds":{"max_request_bytes":16384,"max_response_bytes":1048576,"max_header_bytes":4096,"max_connections":4,"timeout_seconds":2,"poll_interval_ms":10,"max_projects":2,"max_runs":4,
            "max_artifact_bytes":1024,"max_artifacts":2,"max_run_artifact_bytes":2048,"max_total_artifact_bytes":4096,"max_log_read_bytes":4096,"max_requests":32,"max_events_per_run":8,"max_state_bytes":1048576}
    })
}

fn frozen() -> Value {
    let mut profile: kiln_implementer::verification::VerificationProfile =
        serde_json::from_value(json!({
            "schema":"kiln.verification-profile.v1","repository":"onixresearch/implementer-sandbox",
            "toolchain":"a".repeat(64),"environment_identity":"b".repeat(64),"environment":{},
            "checks":[
                {"id":"acceptance","kind":"test","program":{"path":"/nix/store/configuration-fixture/bin/check","blake3":"c".repeat(64)},
                    "arguments":[],"prerequisites":[],"acceptance":["case"],"artifacts":["result"],"timeout_ms":1000},
                {"id":"clippy","kind":"test","program":{"path":"/nix/store/configuration-fixture/bin/check","blake3":"c".repeat(64)},
                    "arguments":[],"prerequisites":["acceptance"],"acceptance":["case"],"artifacts":["lint-result"],"timeout_ms":1000}
            ],"reproducer":null,"protected_paths":["tests"],"allow_already_satisfied":false,"cairn":null
        }))
        .unwrap_or_else(|error| unreachable!("{error}"));
    profile.toolchain = profile
        .derived_toolchain_identity()
        .unwrap_or_else(|error| unreachable!("{error}"));
    profile.environment_identity = profile
        .derived_environment_identity()
        .unwrap_or_else(|error| unreachable!("{error}"));
    serde_json::to_value(&profile).unwrap_or_else(|error| unreachable!("{error}"))
}

fn construct(template: &Path, verification: &Path, output: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_kiln-web-service"))
        .arg("implementation-verification-graph")
        .arg("--template")
        .arg(template)
        .arg("--verification-profile")
        .arg(verification)
        .arg("--project")
        .arg("verification")
        .arg("--output")
        .arg(output)
        .output()
        .unwrap_or_else(|error| unreachable!("{error}"))
}

#[test]
fn installed_command_builds_the_dedicated_graph_and_refuses_overwrite_or_replacement() {
    let directory = tempfile::tempdir().unwrap_or_else(|error| unreachable!("{error}"));
    let root = directory.path();
    let empty = root.join("template.json");
    let verification = root.join("verification.json");
    let output = root.join("service.json");
    write(&empty, &template(root, json!([]), json!([])));
    write(&verification, &frozen());

    let built = construct(&empty, &verification, &output);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let summary: Value =
        serde_json::from_slice(&built.stdout).unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(summary.get("checks_executed"), Some(&json!(false)));
    assert_eq!(summary.get("effects_executed"), Some(&json!(false)));
    let service: Value = serde_json::from_slice(
        &std::fs::read(&output).unwrap_or_else(|error| unreachable!("{error}")),
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(
        service.pointer("/projects/0/workflow/nodes/0/id"),
        Some(&json!("acceptance"))
    );
    assert_eq!(
        service.pointer("/projects/0/workflow/nodes/1/id"),
        Some(&json!("clippy"))
    );
    assert_eq!(
        service.pointer("/projects/0/workflow/edges"),
        Some(&json!([{"source":"acceptance","target":"clippy"}]))
    );

    // The output is created exclusively; a second run never overwrites it.
    assert!(!construct(&empty, &verification, &output).status.success());
    // A template whose project already has a graph is never relabelled.
    let existing = root.join("existing.json");
    write(&existing, &service);
    let refused = construct(&existing, &verification, &root.join("other.json"));
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("refusing to replace"));
    assert!(!root.join("other.json").exists());
}
