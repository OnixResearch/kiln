//! Reserved Lattice identities survive operator policy removal and crash recovery.
use kiln_hosted::{
    Request, SCHEMA,
    service::{Profile, Service},
    transport::{self, NetworkProfile},
};
use serde_json::json;
use std::os::unix::fs::PermissionsExt;

#[test]
fn removed_lattice_policy_never_releases_retained_attempt_or_source_namespace() {
    let root = tempfile::tempdir().unwrap_or_else(|e| unreachable!("{e}"));
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))
        .unwrap_or_else(|e| unreachable!("{e}"));
    let key_file = root.path().join("issuer.key");
    let peer = transport::keygen(&key_file).unwrap_or_else(|e| unreachable!("{e}"));
    let profile = Profile {
        schema: SCHEMA.into(),
        service_id: "namespace-regression".into(),
        state_directory: root.path().join("state"),
        network: NetworkProfile {
            key_file,
            bind_address: "127.0.0.1:0"
                .parse()
                .unwrap_or_else(|e| unreachable!("{e}")),
            relay_urls: Vec::new(),
        },
        bootstrap_peer: peer.clone(),
        bootstrap_organization: "org".into(),
        source_profiles: Vec::new(),
        operator_peers: Vec::new(),
        operations: None,
        secrets: None,
        execution: None,
        fleet: None,
        lattice_execution: Vec::new(),
    };
    let mut service = Service::open(profile).unwrap_or_else(|e| unreachable!("{e}"));
    service.db.execute("INSERT INTO hosted_lattice_bindings(organization,peer,web_project,run_id,node_id,identity,attempt,hosted_project,policy_identity,config_identity,execution_identity,web_approval_verified,source_verified) VALUES('org',?1,'web','run','build','binding','lattice-attempt','project','removed-policy','config','expected-request',1,1)", [&peer]).unwrap_or_else(|e| unreachable!("{e}"));
    // Current profile intentionally has no Lattice or execution policy. The global
    // reserved namespace must reject before disabled-module or cached replay.
    for (id, attempt, workspace) in [
        ("attempt-hijack", "lattice-attempt", "different-source"),
        ("source-hijack", "ordinary-attempt", "lattice-attempt"),
        (
            "staging-hijack",
            "ordinary-attempt",
            ".lattice-source-staging",
        ),
    ] {
        let response = service.handle(&peer, Request {
            schema: SCHEMA.into(), organization: "org".into(), request_id: id.into(), action: "execution.run".into(),
            input: json!({"project":"project","pipeline_id":"pipeline","attempt_id":attempt,"price_id":"price","class":"cpu","workspace":workspace,"trust":"trusted"}),
        });
        assert_eq!(
            response.error.as_deref(),
            Some("Lattice attempt and source namespace reserved")
        );
        assert!(response.result.is_none());
    }
    let executions: i64 = service
        .db
        .query_row("SELECT COUNT(*) FROM hosted_execution", [], |r| r.get(0))
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(executions, 0);
}
