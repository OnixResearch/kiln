use super::*;

#[derive(Debug)]
struct RecoveryOnlyHandler;

impl WorkflowExchangeHandler for RecoveryOnlyHandler {
    fn handle(&mut self, command: &ExchangeCommand) -> Result<ExchangeSuccess, HandlerError> {
        match command {
            ExchangeCommand::RecoverDispatch { request, .. } => {
                Ok(ExchangeSuccess::Accepted(accepted(request)?))
            }
            ExchangeCommand::Observe {
                request, accepted, ..
            }
            | ExchangeCommand::Reconcile {
                request, accepted, ..
            } => Ok(ExchangeSuccess::Observation(observation(
                request, accepted,
            )?)),
            ExchangeCommand::Dispatch { .. }
            | ExchangeCommand::Control { .. }
            | ExchangeCommand::ReadLog { .. } => Err(HandlerError::Rejected),
        }
    }
}

// r[verify kiln.aspen_host.lattice.accepted]
// r[verify kiln.aspen_host.completion.accepted]
#[test]
#[allow(
    tigerstyle::function_length,
    reason = "the restart fixture keeps dispatch uncertainty and recovery-only reconciliation visible"
)]
fn accepted_but_unacknowledged_lattice_work_recovers_without_dispatch() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!("temporary directory"));
    let lattice_socket = directory.path().join("lattice-recovery.sock");
    let replay_path = directory.path().join("lattice-recovery.sqlite");
    let first_listener =
        UnixListener::bind(&lattice_socket).unwrap_or_else(|_| unreachable!("first listener"));
    let user_id = nix::unistd::getuid().as_raw();
    let first_config = radicle_config(lattice_socket.clone(), replay_path.clone(), user_id);
    let first_radicle =
        admit_config(first_config).unwrap_or_else(|_| unreachable!("first Radicle profile"));
    let first_maximum_frame_bytes = first_radicle.transport.limits.maximum_frame_bytes;
    let first_server = std::thread::spawn(move || {
        let (mut stream, _address) = first_listener
            .accept()
            .unwrap_or_else(|_| unreachable!("accept first dispatch"));
        let bytes = read_frame(&mut stream, first_maximum_frame_bytes)
            .unwrap_or_else(|_| unreachable!("read first dispatch"));
        let request: ExchangeRequest =
            serde_json::from_slice(&bytes).unwrap_or_else(|_| unreachable!("decode dispatch"));
        assert!(matches!(request.command, ExchangeCommand::Dispatch { .. }));
    });
    let profile =
        admit_profile(&profile_fixture()).unwrap_or_else(|_| unreachable!("Aspen profile"));
    let first_transport = UnixWorkflowExchange::new(
        first_radicle.shell.socket_path.clone(),
        first_radicle.transport,
        first_radicle.lattice.contract.clone(),
        first_radicle.shell.maximum_replay_records,
    )
    .unwrap_or_else(|_| unreachable!("first Lattice transport"));
    let first_lattice = LatticeAdapter::new(first_radicle.lattice, first_transport)
        .unwrap_or_else(|_| unreachable!("first Lattice adapter"));
    let first_cohort = HostCohort::build(
        profile.clone(),
        &HostPaths {
            extension_executable: std::path::PathBuf::from(env!(
                "CARGO_BIN_EXE_kiln-aspen-host-extension-fixture"
            )),
            workspace: directory.path().to_path_buf(),
        },
    )
    .unwrap_or_else(|error| unreachable!("first Aspen cohort: {error}"));
    let first_effects = LatticeEffectPort::new(LatticeEffectPortOptions {
        profile: profile.clone(),
        radicle: first_radicle.shell,
        lattice: first_lattice,
        values: first_cohort.values.clone(),
        ledger: first_cohort.ledger.clone(),
        observation_poll_interval: std::time::Duration::from_millis(
            TEST_OBSERVATION_POLL_INTERVAL_MILLISECONDS,
        ),
        observation_pacer: Arc::new(ThreadObservationPacer),
    })
    .unwrap_or_else(|_| unreachable!("first effect port"));
    let mut first_runtime = KilnAspenHostRuntime::start(first_cohort, first_effects)
        .unwrap_or_else(|error| unreachable!("first runtime: {error}"));
    let ingress = build_ingress(&profile, IngressSource::Operator, trigger(false))
        .unwrap_or_else(|_| unreachable!("Aspen ingress"));

    let ServiceTransportOutcome::DisconnectedAfterAcceptance { operation_id } =
        first_runtime.submit(&profile, &ingress)
    else {
        unreachable!("first dispatch remains uncertain");
    };
    assert_eq!(operation_id, ingress.operation_id);
    first_server
        .join()
        .unwrap_or_else(|_| unreachable!("first server join"));
    assert!(first_runtime.shutdown().is_ok());
    drop(first_runtime);
    std::fs::remove_file(&lattice_socket)
        .unwrap_or_else(|_| unreachable!("remove first Lattice socket"));

    let second_listener =
        UnixListener::bind(&lattice_socket).unwrap_or_else(|_| unreachable!("second listener"));
    let second_config = radicle_config(lattice_socket, replay_path, user_id);
    let second_radicle =
        admit_config(second_config).unwrap_or_else(|_| unreachable!("second Radicle profile"));
    let second_server_profile = second_radicle.transport.clone();
    let second_server = std::thread::spawn(move || {
        let mut service = WorkflowExchangeService::new(RecoveryOnlyHandler);
        serve_listener(&second_listener, &second_server_profile, &mut service)
    });
    let second_transport = UnixWorkflowExchange::new(
        second_radicle.shell.socket_path.clone(),
        second_radicle.transport,
        second_radicle.lattice.contract.clone(),
        second_radicle.shell.maximum_replay_records,
    )
    .unwrap_or_else(|_| unreachable!("second Lattice transport"));
    let second_lattice = LatticeAdapter::new(second_radicle.lattice, second_transport)
        .unwrap_or_else(|_| unreachable!("second Lattice adapter"));
    let second_cohort = HostCohort::build(
        profile.clone(),
        &HostPaths {
            extension_executable: std::path::PathBuf::from(env!(
                "CARGO_BIN_EXE_kiln-aspen-host-extension-fixture"
            )),
            workspace: directory.path().to_path_buf(),
        },
    )
    .unwrap_or_else(|error| unreachable!("second Aspen cohort: {error}"));
    let second_effects = LatticeEffectPort::new(LatticeEffectPortOptions {
        profile: profile.clone(),
        radicle: second_radicle.shell,
        lattice: second_lattice,
        values: second_cohort.values.clone(),
        ledger: second_cohort.ledger.clone(),
        observation_poll_interval: std::time::Duration::from_millis(
            TEST_OBSERVATION_POLL_INTERVAL_MILLISECONDS,
        ),
        observation_pacer: Arc::new(ThreadObservationPacer),
    })
    .unwrap_or_else(|_| unreachable!("second effect port"));
    let mut second_runtime = KilnAspenHostRuntime::start(second_cohort, second_effects)
        .unwrap_or_else(|error| unreachable!("second runtime: {error}"));
    let ServiceTransportOutcome::Acknowledged(acknowledgement) =
        second_runtime.submit(&profile, &ingress)
    else {
        unreachable!("recovered workflow has a terminal acknowledgement");
    };
    assert_eq!(acknowledgement.decision, AspenServiceDecision::Completed);
    assert!(second_runtime.shutdown().is_ok());
    let report = second_server
        .join()
        .unwrap_or_else(|_| unreachable!("second server join"))
        .unwrap_or_else(|_| unreachable!("second server report"));
    assert_eq!(report.completed_exchanges, TEST_MAXIMUM_CONNECTIONS);
}
