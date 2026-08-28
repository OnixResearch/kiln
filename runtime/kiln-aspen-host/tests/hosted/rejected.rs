use super::*;

// r[verify kiln.aspen_host.lattice.rejected]
#[test]
fn denied_trigger_releases_no_lattice_connection() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!("temporary directory"));
    let missing_socket = directory.path().join("missing-lattice.sock");
    let user_id = nix::unistd::getuid().as_raw();
    let config = radicle_config(
        missing_socket,
        directory.path().join("replay.sqlite"),
        user_id,
    );
    let admitted_radicle = admit_config(config).unwrap_or_else(|_| unreachable!("Radicle profile"));
    let profile =
        admit_profile(&profile_fixture()).unwrap_or_else(|_| unreachable!("Aspen profile"));
    let transport = UnixWorkflowExchange::new(
        admitted_radicle.shell.socket_path.clone(),
        admitted_radicle.transport,
        admitted_radicle.lattice.contract.clone(),
        admitted_radicle.shell.maximum_replay_records,
    )
    .unwrap_or_else(|_| unreachable!("Lattice transport"));
    let lattice = LatticeAdapter::new(admitted_radicle.lattice, transport)
        .unwrap_or_else(|_| unreachable!("Lattice adapter"));
    let cohort = HostCohort::build(
        profile.clone(),
        &HostPaths {
            extension_executable: std::path::PathBuf::from(env!(
                "CARGO_BIN_EXE_kiln-aspen-host-extension-fixture"
            )),
            workspace: directory.path().to_path_buf(),
        },
    )
    .unwrap_or_else(|error| unreachable!("Aspen cohort: {error}"));
    let effects = LatticeEffectPort::new(
        profile.clone(),
        admitted_radicle.shell,
        lattice,
        cohort.values.clone(),
        cohort.ledger.clone(),
    )
    .unwrap_or_else(|_| unreachable!("Lattice effect port"));
    let mut runtime = KilnAspenHostRuntime::start(cohort, effects)
        .unwrap_or_else(|_| unreachable!("Aspen host runtime starts"));
    let ingress = build_ingress(&profile, IngressSource::Operator, trigger(true))
        .unwrap_or_else(|_| unreachable!("Aspen ingress"));

    let result = runtime.submit(&profile, &ingress);
    let ServiceTransportOutcome::Rejected(ack) = result else {
        unreachable!("denied acknowledgement");
    };
    assert_eq!(ack.decision, AspenServiceDecision::Rejected);
    runtime
        .shutdown()
        .unwrap_or_else(|_| unreachable!("runtime shutdown"));
}
