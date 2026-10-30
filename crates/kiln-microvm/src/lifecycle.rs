//! Per-job microVM lifecycle. The shell performs each requested effect and
//! reports the observed event; illegal orderings are rejected, so a VM can
//! never be destroyed before its result is journaled (or journaling failed).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Planned,
    Starting,
    Ready,
    Running {
        completed: usize,
    },
    /// Results (complete or partial) are in hand and must be journaled.
    Collected {
        completed: usize,
    },
    Journaled,
    Destroying {
        journaled: bool,
    },
    Destroyed {
        journaled: bool,
    },
    /// Teardown could not be proven; the slot stays occupied.
    Abandoned {
        journaled: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Spawned,
    GuestConnected,
    StepReported,
    GuestDone,
    /// Boot deadline, protocol error, step deadline, cancellation or VMM exit.
    Failed,
    Journaled,
    JournalFailed,
    DestroyObserved {
        clean: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    AwaitGuest,
    SendRequest,
    AwaitReport,
    Journal,
    Destroy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lifecycle {
    pub state: State,
    pub steps: usize,
}

impl Lifecycle {
    pub fn new(steps: usize) -> Self {
        Self {
            state: State::Planned,
            steps,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            State::Destroyed { .. } | State::Abandoned { .. }
        )
    }

    // r[impl kiln.hosted.microvm.lifecycle]
    // r[impl kiln.hosted.microvm.lifecycle.scenario.journal]
    /// Apply one observed event, returning the next effect for the shell.
    pub fn advance(&mut self, event: Event) -> Result<Option<Effect>, String> {
        use {Effect as F, Event as E, State as S};
        let (state, effect) = match (self.state, event) {
            (S::Planned, E::Spawned) => (S::Starting, Some(F::AwaitGuest)),
            (S::Planned, E::Failed) => (S::Collected { completed: 0 }, Some(F::Journal)),
            (S::Starting, E::GuestConnected) => (S::Ready, Some(F::SendRequest)),
            (S::Ready, E::StepReported) => self.reported(0)?,
            (S::Running { completed }, E::StepReported) => self.reported(completed)?,
            (S::Ready, E::GuestDone) => (S::Collected { completed: 0 }, Some(F::Journal)),
            (S::Running { completed }, E::GuestDone) => {
                (S::Collected { completed }, Some(F::Journal))
            }
            (S::Starting | S::Ready, E::Failed) => {
                (S::Collected { completed: 0 }, Some(F::Journal))
            }
            (S::Running { completed }, E::Failed) => (S::Collected { completed }, Some(F::Journal)),
            (S::Collected { .. }, E::Journaled) => (S::Journaled, Some(F::Destroy)),
            (S::Collected { .. }, E::JournalFailed) => {
                (S::Destroying { journaled: false }, Some(F::Destroy))
            }
            (S::Journaled, E::DestroyObserved { clean }) => (finish(true, clean), None),
            (S::Destroying { journaled }, E::DestroyObserved { clean }) => {
                (finish(journaled, clean), None)
            }
            (state, event) => {
                return Err(format!(
                    "illegal microVM lifecycle transition {event:?} in {state:?}"
                ));
            }
        };
        self.state = state;
        Ok(effect)
    }

    fn reported(&self, completed: usize) -> Result<(State, Option<Effect>), String> {
        let completed = completed + 1;
        if completed > self.steps {
            return Err("guest reported more steps than requested".into());
        }
        Ok((State::Running { completed }, Some(Effect::AwaitReport)))
    }
}

fn finish(journaled: bool, clean: bool) -> State {
    if clean {
        State::Destroyed { journaled }
    } else {
        State::Abandoned { journaled }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drive(lifecycle: &mut Lifecycle, events: &[Event]) -> Vec<Option<Effect>> {
        events
            .iter()
            .map(|event| {
                lifecycle
                    .advance(*event)
                    .unwrap_or_else(|error| unreachable!("{error}"))
            })
            .collect()
    }

    // r[verify kiln.hosted.microvm.lifecycle]
    // r[verify kiln.hosted.microvm.lifecycle.scenario.journal]
    #[test]
    fn successful_job_journals_then_destroys() {
        let mut vm = Lifecycle::new(2);
        let effects = drive(
            &mut vm,
            &[
                Event::Spawned,
                Event::GuestConnected,
                Event::StepReported,
                Event::StepReported,
                Event::GuestDone,
                Event::Journaled,
                Event::DestroyObserved { clean: true },
            ],
        );
        assert_eq!(
            effects,
            [
                Some(Effect::AwaitGuest),
                Some(Effect::SendRequest),
                Some(Effect::AwaitReport),
                Some(Effect::AwaitReport),
                Some(Effect::Journal),
                Some(Effect::Destroy),
                None,
            ]
        );
        assert_eq!(vm.state, State::Destroyed { journaled: true });
        assert!(vm.is_terminal());
    }

    // r[verify kiln.hosted.microvm.lifecycle.scenario.journal]
    #[test]
    fn destroy_is_never_reachable_before_collection_and_journal() {
        for prefix in [
            vec![],
            vec![Event::Spawned],
            vec![Event::Spawned, Event::GuestConnected],
            vec![Event::Spawned, Event::GuestConnected, Event::StepReported],
        ] {
            let mut vm = Lifecycle::new(3);
            drive(&mut vm, &prefix);
            let before = vm;
            assert!(vm.advance(Event::DestroyObserved { clean: true }).is_err());
            assert!(vm.advance(Event::Journaled).is_err());
            assert_eq!(vm, before, "rejected events must not move state");
        }
        let mut vm = Lifecycle::new(1);
        drive(
            &mut vm,
            &[Event::Spawned, Event::GuestConnected, Event::StepReported],
        );
        assert!(vm.advance(Event::StepReported).is_err(), "over-reporting");
    }

    // r[verify kiln.hosted.microvm.lifecycle.scenario.journal]
    #[test]
    fn failures_still_journal_partial_results_and_uncertain_teardown_is_abandoned() {
        let mut vm = Lifecycle::new(3);
        let effects = drive(
            &mut vm,
            &[
                Event::Spawned,
                Event::GuestConnected,
                Event::StepReported,
                Event::Failed,
            ],
        );
        assert_eq!(effects.last(), Some(&Some(Effect::Journal)));
        assert_eq!(vm.state, State::Collected { completed: 1 });
        drive(
            &mut vm,
            &[Event::Journaled, Event::DestroyObserved { clean: false }],
        );
        assert_eq!(vm.state, State::Abandoned { journaled: true });

        let mut boot_failure = Lifecycle::new(1);
        drive(&mut boot_failure, &[Event::Spawned, Event::Failed]);
        assert_eq!(
            boot_failure.advance(Event::JournalFailed),
            Ok(Some(Effect::Destroy))
        );
        drive(&mut boot_failure, &[Event::DestroyObserved { clean: true }]);
        assert_eq!(boot_failure.state, State::Destroyed { journaled: false });
        assert!(boot_failure.advance(Event::Spawned).is_err(), "terminal");
    }
}
