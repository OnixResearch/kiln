use super::*;

// r[verify kiln.aspen_host.transport.rejected]
// r[verify kiln.aspen_host.lattice.rejected]
#[test]
fn unavailable_lattice_is_retained_as_unknown_without_redispatch() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!("temporary directory"));
    let config = radicle_config(
        directory.path().join("missing-lattice.sock"),
        directory.path().join("replay.sqlite"),
        nix::unistd::getuid().as_raw(),
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
    .unwrap_or_else(|_| unreachable!("Aspen cohort"));
    let effects = LatticeEffectPort::new(
        profile.clone(),
        admitted_radicle.shell,
        lattice,
        cohort.values.clone(),
    )
    .unwrap_or_else(|_| unreachable!("Lattice effect port"));
    let mut runtime = KilnAspenHostRuntime::start(cohort, effects)
        .unwrap_or_else(|_| unreachable!("Aspen host runtime starts"));
    let ingress = build_ingress(&profile, IngressSource::Operator, trigger(false))
        .unwrap_or_else(|_| unreachable!("Aspen ingress"));

    assert_unknown(runtime.submit(&profile, &ingress), &ingress.operation_id);
    assert_unknown(runtime.submit(&profile, &ingress), &ingress.operation_id);
    assert_unknown(
        runtime.reconcile(&profile, &ingress.operation_id),
        &ingress.operation_id,
    );
    assert!(runtime.shutdown().is_err());
}

fn assert_unknown(outcome: ServiceTransportOutcome, operation_id: &ExternalRef) {
    let ServiceTransportOutcome::DisconnectedAfterAcceptance {
        operation_id: observed,
    } = outcome
    else {
        unreachable!("operation remains unknown after acceptance");
    };
    assert_eq!(&observed, operation_id);
}
