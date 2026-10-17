#[derive(Debug)]
pub(super) struct RunningProvider {
    pub(super) child: std::process::Child,
    pub(super) started: std::time::Instant,
    pub(super) work: std::path::PathBuf,
    pub(super) stdout_path: std::path::PathBuf,
    pub(super) stderr_path: std::path::PathBuf,
    pub(super) owned: Vec<super::observation::ProcessIdentity>,
}

impl RunningProvider {
    pub(super) fn await_descendant(&mut self) -> super::TestResult {
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_millis(super::READINESS_BOUND_MS);
        loop {
            if self.work.join("wrapper.pid").exists() && self.work.join("descendant.pid").exists() {
                let wrapper = super::observation::read_identity(&self.work.join("wrapper.pid"))?;
                let descendant =
                    super::observation::read_identity(&self.work.join("descendant.pid"))?;
                if let (Some(wrapper), Some(descendant)) = (wrapper, descendant) {
                    assert_ne!(wrapper.pid, descendant.pid);
                    assert_eq!(
                        wrapper.group, wrapper.pid,
                        "wrapper must own its process group"
                    );
                    assert_eq!(
                        descendant.group, wrapper.group,
                        "descendant must belong to owned group"
                    );
                    self.owned = vec![wrapper, descendant];
                    return Ok(());
                }
            }
            if let Some(status) = self.child.try_wait()? {
                return Err(format!(
                    "provider exited before descendant readiness: {status}; {}",
                    std::fs::read_to_string(&self.stderr_path)?
                )
                .into());
            }
            if std::time::Instant::now() >= deadline {
                return Err("descendant readiness exceeded named bound".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(super::POLL_INTERVAL_MS));
        }
    }

    pub(super) fn proceed(&self) -> super::TestResult {
        std::fs::write(self.work.join("proceed"), b"run")?;
        Ok(())
    }

    pub(super) fn finish(
        &mut self,
        completion_bound: std::time::Duration,
        teardown_deadline: std::time::Instant,
    ) -> super::TestResult<std::process::ExitStatus> {
        let completion_deadline = std::time::Instant::now() + completion_bound;
        let mut is_termination_observed = false;
        loop {
            if !is_termination_observed {
                let mut survivors = Vec::with_capacity(super::OWNED_PROCESS_COUNT);
                let mut observations = Vec::with_capacity(super::OWNED_PROCESS_COUNT);
                for identity in &self.owned {
                    let observation = super::observation::observe_process(identity.pid)?;
                    let same_process = observation.filter(|value| value.identity == *identity);
                    // The wrapper is the provider's direct child and must be
                    // reaped. Its descendant may instead be a terminated zombie
                    // awaiting reaping by PID1; that is not claimed as absent.
                    let is_wrapper = identity.pid == identity.group;
                    let is_terminated = match same_process {
                        None => true,
                        Some(value) => !is_wrapper && value.state == 'Z',
                    };
                    observations.push((*identity, observation));
                    if !is_terminated {
                        survivors.push((*identity, observation));
                    }
                }
                if std::time::Instant::now() > teardown_deadline {
                    return Err(format!("direct-child reaping/descendant termination was not observed within named teardown bound: {survivors:?}; all observations: {observations:?}").into());
                }
                is_termination_observed = survivors.is_empty();
                if is_termination_observed {
                    eprintln!(
                        "provider teardown observations (None means original process absent; Z means terminated but unreaped, not absent): {observations:?}"
                    );
                }
            }
            if let Some(status) = self.child.try_wait()?
                && is_termination_observed
            {
                return Ok(status);
            }
            if std::time::Instant::now() >= completion_deadline {
                return Err("provider completion exceeded named bound".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(super::POLL_INTERVAL_MS));
        }
    }

    pub(super) fn receipt(
        &self,
    ) -> super::TestResult<kiln_radicle_nix_provider::HostedProviderReceipt> {
        Ok(serde_json::from_slice(&std::fs::read(&self.stdout_path)?)?)
    }
}

impl Drop for RunningProvider {
    fn drop(&mut self) {
        // Emergency cleanup follows, never precedes, the behavioral assertions.
        // Recover PID files even when readiness failed before recording identities.
        for leaf in ["wrapper.pid", "descendant.pid"] {
            if let Ok(Some(identity)) = super::observation::read_identity(&self.work.join(leaf))
                && !self.owned.contains(&identity)
            {
                self.owned.push(identity);
            }
        }
        for identity in &self.owned {
            if super::observation::observe_process(identity.pid)
                .ok()
                .flatten()
                .is_some_and(|observation| {
                    observation.identity == *identity && observation.state != 'Z'
                })
            {
                let _ = super::observation::signal(identity.pid, rustix::process::Signal::KILL);
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub(super) fn exercise_limit(
    operation: &str,
    expected: kiln_radicle_nix_provider::HostedProviderOutcome,
) -> super::TestResult {
    let fixture = super::fixture::Fixture::new(operation, super::EXECUTION_TIMEOUT_MS)?;
    let mut provider = fixture.start()?;
    provider.await_descendant()?;
    let teardown_deadline =
        if expected == kiln_radicle_nix_provider::HostedProviderOutcome::TimedOut {
            provider.started
                + std::time::Duration::from_millis(
                    super::EXECUTION_TIMEOUT_MS + super::TEARDOWN_BOUND_MS,
                )
        } else {
            std::time::Instant::now() + std::time::Duration::from_millis(super::TEARDOWN_BOUND_MS)
        };
    provider.proceed()?;
    let status = provider.finish(
        std::time::Duration::from_millis(
            super::EXECUTION_TIMEOUT_MS
                + super::TEARDOWN_BOUND_MS
                + super::PUBLICATION_ALLOWANCE_MS,
        ),
        teardown_deadline,
    )?;
    assert_eq!(
        status.code(),
        Some(super::CHECK_FAILED_EXIT_CODE),
        "{}",
        std::fs::read_to_string(&provider.stderr_path)?
    );
    assert_eq!(provider.receipt()?.outcome, expected);
    Ok(())
}
