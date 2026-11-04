//! Per-job microVM lifecycle. The shell performs each requested effect and
//! reports the observed event; illegal orderings are rejected, so a VM can
//! never be destroyed before its result is journaled (or journaling failed).
//! An opt-in debug hold may keep a failed job's VM up after `Journaled`; it
//! always ends in destruction.

use crate::debug_hold::{self, HoldFacts};

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
    /// Debug hold of a failed, journaled job; ends only by `HoldEnded`.
    Held,
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
    /// Host-observed VMM confinement; `false` fails closed.
    ConfinementObserved {
        ok: bool,
    },
    GuestConnected,
    StepReported,
    GuestDone,
    /// Boot deadline, protocol error, step deadline, cancellation or VMM exit.
    Failed,
    Journaled,
    /// The shell asks to hold the journaled VM for debugging.
    Hold(HoldFacts),
    /// Expiry, release, capacity pressure, cancellation, VMM exit or guest loss.
    HoldEnded,
    JournalFailed,
    DestroyObserved {
        clean: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    ObserveConfinement,
    AwaitGuest,
    SendRequest,
    AwaitReport,
    Journal,
    /// Keep the VM and serve the debug relay until the hold ends.
    Hold,
    Destroy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lifecycle {
    pub state: State,
    pub steps: usize,
    /// The guest reported completion of its session (`GuestDone`).
    guest_done: bool,
    /// A successful host observation was recorded for this spawn.
    confined: bool,
}

impl Lifecycle {
    pub fn new(steps: usize) -> Self {
        Self {
            state: State::Planned,
            steps,
            guest_done: false,
            confined: false,
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
            (S::Planned, E::Spawned) => {
                self.confined = false;
                (S::Starting, Some(F::ObserveConfinement))
            }
            (S::Planned, E::Failed) => (S::Collected { completed: 0 }, Some(F::Journal)),
            (S::Starting, E::ConfinementObserved { ok: true }) => {
                self.confined = true;
                (S::Starting, Some(F::AwaitGuest))
            }
            (S::Starting, E::ConfinementObserved { ok: false }) => {
                (S::Collected { completed: 0 }, Some(F::Journal))
            }
            (S::Starting, E::GuestConnected) if self.confined => (S::Ready, Some(F::SendRequest)),
            (S::Starting, E::GuestConnected) => {
                return Err(
                    "illegal microVM lifecycle transition GuestConnected before confinement"
                        .to_string(),
                );
            }
            (S::Ready, E::StepReported) => self.reported(0)?,
            (S::Running { completed }, E::StepReported) => self.reported(completed)?,
            (S::Ready, E::GuestDone) => {
                self.guest_done = true;
                (S::Collected { completed: 0 }, Some(F::Journal))
            }
            (S::Running { completed }, E::GuestDone) => {
                self.guest_done = true;
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
            // r[impl kiln.hosted.microvm.debug_hold.lifecycle]
            // r[impl kiln.hosted.microvm.debug_hold.lifecycle.scenario.reachability]
            (S::Journaled, E::Hold(facts))
                if self.guest_done && debug_hold::eligibility(&facts).is_ok() =>
            {
                (S::Held, Some(F::Hold))
            }
            // r[impl kiln.hosted.microvm.debug_hold.end]
            (S::Held, E::HoldEnded) => (S::Destroying { journaled: true }, Some(F::Destroy)),
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
                Event::ConfinementObserved { ok: true },
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
                Some(Effect::ObserveConfinement),
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
            vec![
                Event::Spawned,
                Event::ConfinementObserved { ok: true },
                Event::GuestConnected,
            ],
            vec![
                Event::Spawned,
                Event::ConfinementObserved { ok: true },
                Event::GuestConnected,
                Event::StepReported,
            ],
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
            &[
                Event::Spawned,
                Event::ConfinementObserved { ok: true },
                Event::GuestConnected,
                Event::StepReported,
            ],
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
                Event::ConfinementObserved { ok: true },
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

    // r[verify kiln.microvm.confinement.lifecycle]
    #[test]
    fn failed_confinement_journals_without_guest_connection() {
        let mut vm = Lifecycle::new(1);
        let effects = drive(
            &mut vm,
            &[
                Event::Spawned,
                Event::ConfinementObserved { ok: false },
                Event::JournalFailed,
                Event::DestroyObserved { clean: true },
            ],
        );
        assert_eq!(
            effects,
            [
                Some(Effect::ObserveConfinement),
                Some(Effect::Journal),
                Some(Effect::Destroy),
                None,
            ]
        );
        assert_eq!(vm.state, State::Destroyed { journaled: false });
        let mut vm = Lifecycle::new(1);
        drive(&mut vm, &[Event::Spawned]);
        let before = vm;
        assert!(vm.advance(Event::GuestConnected).is_err());
        assert_eq!(vm, before);
    }

    use crate::debug_hold::Grants;

    fn facts(enabled: bool, failed: bool, grants: Grants, held: u32) -> HoldFacts {
        HoldFacts {
            enabled,
            failed,
            grants,
            held,
            max_concurrent: 1,
        }
    }

    fn eligible() -> HoldFacts {
        facts(true, true, Grants::default(), 0)
    }

    /// One fact set per refusal reason; none may ever reach `Held`.
    fn refused() -> Vec<HoldFacts> {
        vec![
            facts(false, true, Grants::default(), 0),
            facts(true, false, Grants::default(), 0),
            facts(
                true,
                true,
                Grants {
                    secrets: true,
                    ..Grants::default()
                },
                0,
            ),
            facts(
                true,
                true,
                Grants {
                    workload_token: true,
                    ..Grants::default()
                },
                0,
            ),
            facts(
                true,
                true,
                Grants {
                    untrusted: true,
                    ..Grants::default()
                },
                0,
            ),
            facts(true, true, Grants::default(), 1),
        ]
    }

    /// Today's event alphabet, without the hold events.
    fn base_events() -> Vec<Event> {
        vec![
            Event::Spawned,
            Event::ConfinementObserved { ok: true },
            Event::GuestConnected,
            Event::StepReported,
            Event::GuestDone,
            Event::Failed,
            Event::Journaled,
            Event::JournalFailed,
            Event::DestroyObserved { clean: true },
            Event::DestroyObserved { clean: false },
        ]
    }

    /// The shell's rule: try a hold only when the runner enables it.
    fn shell_trace(events: &[Event], hold: Option<HoldFacts>) -> Vec<(State, Option<Effect>)> {
        let mut vm = Lifecycle::new(2);
        let mut trace = Vec::new();
        for event in events {
            let effect = vm
                .advance(*event)
                .unwrap_or_else(|error| unreachable!("{error}"));
            trace.push((vm.state, effect));
            if vm.state == State::Journaled {
                if let Some(facts) = hold.filter(|f| f.enabled) {
                    let effect = vm
                        .advance(Event::Hold(facts))
                        .unwrap_or_else(|error| unreachable!("{error}"));
                    trace.push((vm.state, effect));
                    let effect = vm
                        .advance(Event::HoldEnded)
                        .unwrap_or_else(|error| unreachable!("{error}"));
                    trace.push((vm.state, effect));
                }
            }
        }
        trace
    }

    // r[verify kiln.hosted.microvm.debug_hold.policy.scenario.disabled]
    #[test]
    fn disabled_hold_keeps_todays_exact_transitions_and_effects() {
        use {Effect as F, Event as E, State as S};
        let failing_session = [
            E::Spawned,
            E::ConfinementObserved { ok: true },
            E::GuestConnected,
            E::StepReported,
            E::GuestDone,
            E::Journaled,
            E::DestroyObserved { clean: true },
        ];
        let today = vec![
            (S::Starting, Some(F::ObserveConfinement)),
            (S::Starting, Some(F::AwaitGuest)),
            (S::Ready, Some(F::SendRequest)),
            (S::Running { completed: 1 }, Some(F::AwaitReport)),
            (S::Collected { completed: 1 }, Some(F::Journal)),
            (S::Journaled, Some(F::Destroy)),
            (S::Destroyed { journaled: true }, None),
        ];
        let disabled = facts(false, true, Grants::default(), 0);
        assert_eq!(shell_trace(&failing_session, None), today);
        assert_eq!(shell_trace(&failing_session, Some(disabled)), today);
        let infrastructure = [
            E::Spawned,
            E::Failed,
            E::JournalFailed,
            E::DestroyObserved { clean: false },
        ];
        assert_eq!(
            shell_trace(&infrastructure, Some(disabled)),
            vec![
                (S::Starting, Some(F::ObserveConfinement)),
                (S::Collected { completed: 0 }, Some(F::Journal)),
                (S::Destroying { journaled: false }, Some(F::Destroy)),
                (S::Abandoned { journaled: false }, None),
            ]
        );
        // Every state reachable with today's events never offers `Held`, and a
        // disabled hold request is rejected without moving state.
        let mut frontier = vec![Lifecycle::new(2)];
        for _ in 0..8 {
            let mut next = Vec::new();
            for vm in &frontier {
                let mut probe = *vm;
                assert!(probe.advance(E::Hold(disabled)).is_err());
                assert_eq!(probe, *vm);
                for event in base_events() {
                    let mut moved = *vm;
                    if moved.advance(event).is_ok() {
                        assert_ne!(moved.state, S::Held);
                        next.push(moved);
                    }
                }
            }
            frontier = next;
        }
    }

    // r[verify kiln.hosted.microvm.debug_hold.lifecycle]
    // r[verify kiln.hosted.microvm.debug_hold.lifecycle.scenario.reachability]
    #[test]
    fn held_is_reachable_only_after_journaling_a_completed_failed_session_without_grants() {
        let mut alphabet = base_events();
        alphabet.push(Event::Hold(eligible()));
        alphabet.extend(refused().into_iter().map(Event::Hold));
        alphabet.push(Event::HoldEnded);
        let mut frontier = vec![Lifecycle::new(2)];
        let mut held = 0;
        for _ in 0..9 {
            let mut next = Vec::new();
            for vm in &frontier {
                for event in &alphabet {
                    let mut moved = *vm;
                    match moved.advance(*event) {
                        Ok(_) if moved.state == State::Held && vm.state != State::Held => {
                            held += 1;
                            assert_eq!(vm.state, State::Journaled);
                            assert!(vm.guest_done, "complete guest session");
                            assert_eq!(*event, Event::Hold(eligible()));
                            next.push(moved);
                        }
                        Ok(_) => next.push(moved),
                        Err(_) => assert_eq!(moved, *vm, "rejected events never move state"),
                    }
                }
            }
            frontier = next;
        }
        assert!(held > 0, "an eligible failed job can be held");

        // A failure before the guest finished its session is never held.
        let mut infrastructure = Lifecycle::new(2);
        drive(
            &mut infrastructure,
            &[
                Event::Spawned,
                Event::ConfinementObserved { ok: true },
                Event::GuestConnected,
                Event::Failed,
                Event::Journaled,
            ],
        );
        assert!(infrastructure.advance(Event::Hold(eligible())).is_err());
        // A journal failure never reaches `Journaled`, so it is never held.
        let mut unjournaled = Lifecycle::new(1);
        drive(
            &mut unjournaled,
            &[
                Event::Spawned,
                Event::ConfinementObserved { ok: true },
                Event::GuestConnected,
                Event::StepReported,
                Event::GuestDone,
                Event::JournalFailed,
            ],
        );
        assert!(unjournaled.advance(Event::Hold(eligible())).is_err());
        // Before `Journaled` a hold is illegal even when eligible.
        let mut collected = Lifecycle::new(1);
        drive(
            &mut collected,
            &[
                Event::Spawned,
                Event::ConfinementObserved { ok: true },
                Event::GuestConnected,
                Event::StepReported,
                Event::GuestDone,
            ],
        );
        assert!(collected.advance(Event::Hold(eligible())).is_err());
        for facts in refused() {
            let mut vm = collected;
            drive(&mut vm, &[Event::Journaled]);
            assert!(vm.advance(Event::Hold(facts)).is_err(), "{facts:?}");
            assert_eq!(vm.state, State::Journaled);
        }
    }

    // r[verify kiln.hosted.microvm.debug_hold.end]
    // r[verify kiln.hosted.microvm.debug_hold.end.scenario.expiry]
    #[test]
    fn every_hold_ends_in_destruction_or_abandoned() {
        let mut held = Lifecycle::new(1);
        drive(
            &mut held,
            &[
                Event::Spawned,
                Event::ConfinementObserved { ok: true },
                Event::GuestConnected,
                Event::StepReported,
                Event::GuestDone,
                Event::Journaled,
            ],
        );
        assert_eq!(
            held.advance(Event::Hold(eligible())),
            Ok(Some(Effect::Hold))
        );
        assert_eq!(held.state, State::Held);
        let mut alphabet = base_events();
        alphabet.push(Event::Hold(eligible()));
        for event in &alphabet {
            let mut vm = held;
            assert!(vm.advance(*event).is_err(), "{event:?} while held");
            assert_eq!(vm, held);
        }
        for clean in [true, false] {
            let mut vm = held;
            assert_eq!(vm.advance(Event::HoldEnded), Ok(Some(Effect::Destroy)));
            assert_eq!(vm.state, State::Destroying { journaled: true });
            assert!(vm.advance(Event::Hold(eligible())).is_err(), "no re-hold");
            assert_eq!(vm.advance(Event::DestroyObserved { clean }), Ok(None));
            assert!(vm.is_terminal());
            assert_eq!(
                vm.state,
                if clean {
                    State::Destroyed { journaled: true }
                } else {
                    State::Abandoned { journaled: true }
                }
            );
        }
        // The shell trace of an enabled, eligible hold.
        let trace = shell_trace(
            &[
                Event::Spawned,
                Event::ConfinementObserved { ok: true },
                Event::GuestConnected,
                Event::StepReported,
                Event::GuestDone,
                Event::Journaled,
                Event::DestroyObserved { clean: true },
            ],
            Some(eligible()),
        );
        assert_eq!(
            trace.get(5..).map(<[_]>::to_vec),
            Some(vec![
                (State::Journaled, Some(Effect::Destroy)),
                (State::Held, Some(Effect::Hold)),
                (State::Destroying { journaled: true }, Some(Effect::Destroy)),
                (State::Destroyed { journaled: true }, None),
            ])
        );
    }
}
