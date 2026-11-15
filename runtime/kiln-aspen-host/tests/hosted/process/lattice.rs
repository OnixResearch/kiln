use super::*;

use std::sync::atomic::AtomicBool;

const PROVIDER_POLL_MILLISECONDS: u64 = 2;

#[derive(Clone, Copy)]
pub(in super::super) enum DispatchBehavior {
    DisconnectFirst,
    Connected,
}

#[derive(Clone, Copy)]
pub(in super::super) enum RecoveryBehavior {
    Available,
    Unavailable,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in super::super) struct ExchangeCounts {
    pub(in super::super) dispatch: usize,
    pub(in super::super) recover: usize,
    pub(in super::super) observe: usize,
}

// This is the existing closed terminal transport fixture, not a real Lattice worker.
// Only Aspen's native service, extension execution and host persistence are real here.
struct CountedTerminalHandler {
    counts: Arc<std::sync::Mutex<ExchangeCounts>>,
    terminal: TerminalHandler,
    recovery_unavailable: Arc<AtomicBool>,
}

impl WorkflowExchangeHandler for CountedTerminalHandler {
    fn handle(&mut self, command: &ExchangeCommand) -> Result<ExchangeSuccess, HandlerError> {
        {
            let mut counts = self
                .counts
                .lock()
                .unwrap_or_else(|_| unreachable!("exchange counts"));
            match command {
                ExchangeCommand::Dispatch { .. } => counts.dispatch += 1,
                ExchangeCommand::RecoverDispatch { .. } => counts.recover += 1,
                ExchangeCommand::Observe { .. } => counts.observe += 1,
                _ => return Err(HandlerError::Rejected),
            }
        }
        if self.recovery_unavailable.load(Ordering::SeqCst)
            && matches!(command, ExchangeCommand::RecoverDispatch { .. })
        {
            return Err(HandlerError::Unavailable);
        }
        self.terminal.handle(command)
    }
}

pub(in super::super) struct FixtureLattice {
    stop: Arc<AtomicBool>,
    recovery_unavailable: Arc<AtomicBool>,
    counts: Arc<std::sync::Mutex<ExchangeCounts>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl FixtureLattice {
    pub(in super::super) fn start(fixture: &ProcessFixture, dispatch: DispatchBehavior) -> Self {
        Self::with_recovery(fixture, dispatch, RecoveryBehavior::Available)
    }

    pub(in super::super) fn unavailable_recovery(fixture: &ProcessFixture) -> Self {
        Self::with_recovery(
            fixture,
            DispatchBehavior::DisconnectFirst,
            RecoveryBehavior::Unavailable,
        )
    }

    fn with_recovery(
        fixture: &ProcessFixture,
        dispatch: DispatchBehavior,
        recovery: RecoveryBehavior,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let (recovery_unavailable, counts, thread) =
            spawn_fixture_server(fixture, dispatch, recovery, stop.clone());
        Self {
            stop,
            recovery_unavailable,
            counts,
            thread: Some(thread),
        }
    }

    pub(in super::super) fn restore_recovery(&self) {
        self.recovery_unavailable.store(false, Ordering::SeqCst);
    }

    pub(in super::super) fn counts(&self) -> ExchangeCounts {
        self.counts
            .lock()
            .unwrap_or_else(|_| unreachable!("exchange counts"))
            .clone()
    }

    pub(in super::super) fn finish(mut self) -> ExchangeCounts {
        self.join();
        let counts = self.counts();
        println!(
            "{}",
            serde_json::json!({
                "schema": "kiln.aspen-host.qualification-fixture-counts.v1",
                "dispatch": counts.dispatch,
                "recover_dispatch": counts.recover,
                "observe": counts.observe,
            })
        );
        counts
    }

    fn join(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .unwrap_or_else(|_| unreachable!("fixture Lattice join"));
        }
    }
}

fn spawn_fixture_server(
    fixture: &ProcessFixture,
    dispatch: DispatchBehavior,
    recovery: RecoveryBehavior,
    stop: Arc<AtomicBool>,
) -> (
    Arc<AtomicBool>,
    Arc<std::sync::Mutex<ExchangeCounts>>,
    std::thread::JoinHandle<()>,
) {
    let listener = UnixListener::bind(&fixture.lattice_socket)
        .unwrap_or_else(|_| unreachable!("fixture Lattice listener"));
    listener
        .set_nonblocking(true)
        .unwrap_or_else(|_| unreachable!("nonblocking fixture"));
    let config = radicle_config(
        fixture.lattice_socket.clone(),
        fixture.directory.path().join("replay.sqlite"),
        nix::unistd::getuid().as_raw(),
    );
    let mut profile = admit_config(config)
        .unwrap_or_else(|_| unreachable!("fixture transport profile"))
        .transport;
    profile.limits.maximum_connections = 1;
    let counts = Arc::new(std::sync::Mutex::new(ExchangeCounts::default()));
    let recovery_unavailable = Arc::new(AtomicBool::new(matches!(
        recovery,
        RecoveryBehavior::Unavailable
    )));
    let handler = CountedTerminalHandler {
        counts: counts.clone(),
        recovery_unavailable: recovery_unavailable.clone(),
        terminal: TerminalHandler {
            pending_observations_remaining: 0,
        },
    };
    let thread_stop = stop;
    let thread_counts = counts.clone();
    let thread = std::thread::spawn(move || {
        let mut service = WorkflowExchangeService::new(handler);
        let mut should_disconnect = matches!(dispatch, DispatchBehavior::DisconnectFirst);
        while !thread_stop.load(Ordering::SeqCst) {
            if should_disconnect {
                should_disconnect = drop_first_dispatch(&listener, &thread_counts);
            } else {
                match serve_listener(&listener, &profile, &mut service) {
                    Ok(report) => {
                        assert_eq!(report.completed_exchanges, 1);
                        assert_eq!(report.failed_exchanges, 0);
                        assert_eq!(report.rejected_peers, 0);
                    }
                    // The pinned transport maps a nonblocking accept's
                    // WouldBlock to AcceptFailed without consuming a frame.
                    Err(lattice_workflow_transport::ServerFailure::AcceptFailed) => {}
                    Err(error) => unreachable!("fixture Lattice exchange: {error:?}"),
                }
            }
            std::thread::park_timeout(Duration::from_millis(PROVIDER_POLL_MILLISECONDS));
        }
    });
    (recovery_unavailable, counts, thread)
}

/// Read one dispatch frame and drop its connection without replying; returns
/// whether the next accepted connection must still be dropped.
fn drop_first_dispatch(listener: &UnixListener, counts: &std::sync::Mutex<ExchangeCounts>) -> bool {
    match listener.accept() {
        Ok((mut stream, _)) => {
            stream
                .set_read_timeout(Some(Duration::from_millis(TEST_DEADLINE_MILLISECONDS)))
                .unwrap_or_else(|_| unreachable!("fixture read deadline"));
            let bytes = read_frame(&mut stream, TEST_MAXIMUM_FRAME_BYTES)
                .unwrap_or_else(|_| unreachable!("uncertain dispatch frame"));
            let request: ExchangeRequest = serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| unreachable!("uncertain dispatch command"));
            assert!(matches!(request.command, ExchangeCommand::Dispatch { .. }));
            counts
                .lock()
                .unwrap_or_else(|_| unreachable!("dispatch count"))
                .dispatch += 1;
            false
        }
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => true,
        Err(error) => unreachable!("accept uncertain dispatch: {error}"),
    }
}

impl Drop for FixtureLattice {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            match thread.join() {
                Ok(()) | Err(_) => {}
            }
        }
    }
}
