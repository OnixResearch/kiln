use super::*;

const EXHAUSTION_MAXIMUM_CONNECTIONS: u32 = 5;
const EXPECTED_EXHAUSTION_WAITS: usize = 4;
const PROFILE_DRIFT_TIMEOUT_MILLISECONDS: u64 = 30_001;

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
    let cohort = HostCohort::build(profile.clone(), &host_paths(&directory))
        .unwrap_or_else(|error| unreachable!("Aspen cohort: {error}"));
    let effects = LatticeEffectPort::new(LatticeEffectPortOptions {
        profile: profile.clone(),
        radicle: admitted_radicle.shell,
        lattice,
        values: cohort.values.clone(),
        ledger: cohort.ledger.clone(),
        observation_poll_interval: std::time::Duration::from_millis(
            TEST_OBSERVATION_POLL_INTERVAL_MILLISECONDS,
        ),
        observation_pacer: Arc::new(ThreadObservationPacer),
    })
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
    assert!(runtime.shutdown().is_ok());
}

// r[verify kiln.aspen_host.extension_profile_binding.rejected]
#[test]
#[allow(
    tigerstyle::function_length,
    reason = "the failure fixture keeps profile drift, durable acceptance, and replay visible"
)]
fn post_reservation_profile_drift_is_unknown_and_never_dispatches() {
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
    let paths = host_paths(&directory);
    let runtime_profile = paths.runtime_profile.clone();
    let cohort = HostCohort::build(profile.clone(), &paths)
        .unwrap_or_else(|error| unreachable!("Aspen cohort: {error}"));
    let ledger = cohort.ledger.clone();
    let effects = LatticeEffectPort::new(LatticeEffectPortOptions {
        profile: profile.clone(),
        radicle: admitted_radicle.shell,
        lattice,
        values: cohort.values.clone(),
        ledger: cohort.ledger.clone(),
        observation_poll_interval: std::time::Duration::from_millis(
            TEST_OBSERVATION_POLL_INTERVAL_MILLISECONDS,
        ),
        observation_pacer: Arc::new(ThreadObservationPacer),
    })
    .unwrap_or_else(|_| unreachable!("Lattice effect port"));
    let mut runtime = KilnAspenHostRuntime::start(cohort, effects)
        .unwrap_or_else(|_| unreachable!("Aspen host runtime starts"));

    let original_profile = serde_json::to_vec(&profile_document_fixture())
        .unwrap_or_else(|_| unreachable!("original profile must encode"));
    let mut drifted_profile = profile_document_fixture();
    drifted_profile.bounds.callback_timeout_millis = PROFILE_DRIFT_TIMEOUT_MILLISECONDS;
    let drifted_bytes = serde_json::to_vec(&drifted_profile)
        .unwrap_or_else(|_| unreachable!("drifted profile must encode"));
    std::fs::write(&runtime_profile, drifted_bytes)
        .unwrap_or_else(|_| unreachable!("drifted profile must write"));

    let ingress = build_ingress(&profile, IngressSource::Operator, trigger(false))
        .unwrap_or_else(|_| unreachable!("Aspen ingress"));
    assert_unknown(runtime.submit(&profile, &ingress), &ingress.operation_id);
    assert_unknown(runtime.submit(&profile, &ingress), &ingress.operation_id);
    let record = ledger
        .lock()
        .unwrap_or_else(|_| unreachable!("operation ledger lock"))
        .operation(&ingress.operation_id)
        .unwrap_or_else(|_| unreachable!("operation ledger read"))
        .unwrap_or_else(|| unreachable!("reserved operation"));
    assert_eq!(
        record.state,
        kiln_aspen_host::durability::DurableOperationState::Uncertain,
    );
    assert!(record.effect_intent.is_none());
    assert!(record.provider_observation.is_none());
    assert!(record.acknowledgement.is_none());

    std::fs::write(&runtime_profile, original_profile)
        .unwrap_or_else(|_| unreachable!("original profile must restore"));
    assert!(runtime.shutdown().is_err());
    drop(runtime);
    drop(record);
    drop(ledger);

    let recovered_config = radicle_config(
        directory.path().join("missing-lattice.sock"),
        directory.path().join("recovered-replay.sqlite"),
        nix::unistd::getuid().as_raw(),
    );
    let recovered_radicle = admit_config(recovered_config)
        .unwrap_or_else(|_| unreachable!("recovered Radicle profile"));
    let recovered_transport = UnixWorkflowExchange::new(
        recovered_radicle.shell.socket_path.clone(),
        recovered_radicle.transport,
        recovered_radicle.lattice.contract.clone(),
        recovered_radicle.shell.maximum_replay_records,
    )
    .unwrap_or_else(|_| unreachable!("recovered Lattice transport"));
    let recovered_lattice = LatticeAdapter::new(recovered_radicle.lattice, recovered_transport)
        .unwrap_or_else(|_| unreachable!("recovered Lattice adapter"));
    let recovered_cohort = HostCohort::build(profile.clone(), &paths)
        .unwrap_or_else(|error| unreachable!("recovered Aspen cohort: {error}"));
    let recovered_effects = LatticeEffectPort::new(LatticeEffectPortOptions {
        profile: profile.clone(),
        radicle: recovered_radicle.shell,
        lattice: recovered_lattice,
        values: recovered_cohort.values.clone(),
        ledger: recovered_cohort.ledger.clone(),
        observation_poll_interval: std::time::Duration::from_millis(
            TEST_OBSERVATION_POLL_INTERVAL_MILLISECONDS,
        ),
        observation_pacer: Arc::new(ThreadObservationPacer),
    })
    .unwrap_or_else(|_| unreachable!("recovered Lattice effect port"));
    assert!(KilnAspenHostRuntime::start(recovered_cohort, recovered_effects).is_err());
}

// r[verify kiln.aspen_host.lattice_poll_pacing.rejected]
#[test]
#[allow(
    tigerstyle::function_length,
    reason = "the exhaustion fixture keeps the bounded dispatch and observation sequence visible"
)]
fn nonterminal_poll_exhaustion_remains_unknown_without_redispatch() {
    let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!("temporary directory"));
    let lattice_socket = directory.path().join("bounded-lattice.sock");
    let listener =
        UnixListener::bind(&lattice_socket).unwrap_or_else(|_| unreachable!("Lattice listener"));
    let mut config = radicle_config(
        lattice_socket,
        directory.path().join("replay.sqlite"),
        nix::unistd::getuid().as_raw(),
    );
    config.transport_limits.maximum_connections = EXHAUSTION_MAXIMUM_CONNECTIONS;
    let admitted_radicle = admit_config(config).unwrap_or_else(|_| unreachable!("Radicle profile"));
    let server_profile = admitted_radicle.transport.clone();
    let lattice_server = std::thread::spawn(move || {
        let mut service = WorkflowExchangeService::new(TerminalHandler {
            pending_observations_remaining: TEST_MAXIMUM_POLLS,
        });
        serve_listener(&listener, &server_profile, &mut service)
    });
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
    let cohort = HostCohort::build(profile.clone(), &host_paths(&directory))
        .unwrap_or_else(|error| unreachable!("Aspen cohort: {error}"));
    let observation_pacer = Arc::new(CountingObservationPacer::default());
    let effects = LatticeEffectPort::new(LatticeEffectPortOptions {
        profile: profile.clone(),
        radicle: admitted_radicle.shell,
        lattice,
        values: cohort.values.clone(),
        ledger: cohort.ledger.clone(),
        observation_poll_interval: std::time::Duration::from_millis(
            TEST_OBSERVATION_POLL_INTERVAL_MILLISECONDS,
        ),
        observation_pacer: observation_pacer.clone(),
    })
    .unwrap_or_else(|_| unreachable!("Lattice effect port"));
    let mut runtime = KilnAspenHostRuntime::start(cohort, effects)
        .unwrap_or_else(|_| unreachable!("Aspen host runtime starts"));
    let ingress = build_ingress(&profile, IngressSource::Operator, trigger(false))
        .unwrap_or_else(|_| unreachable!("Aspen ingress"));

    assert_unknown(runtime.submit(&profile, &ingress), &ingress.operation_id);
    assert_eq!(observation_pacer.wait_count(), EXPECTED_EXHAUSTION_WAITS);
    let report = lattice_server
        .join()
        .unwrap_or_else(|_| unreachable!("Lattice server join"))
        .unwrap_or_else(|_| unreachable!("Lattice server report"));
    assert_eq!(report.completed_exchanges, EXHAUSTION_MAXIMUM_CONNECTIONS);

    assert_unknown(runtime.submit(&profile, &ingress), &ingress.operation_id);
    assert!(runtime.shutdown().is_ok());
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
