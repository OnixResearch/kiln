//! Exact-source live CI probe over Kiln's real state-transition API.
//!
//! The bounded observation window permits interruption of transport/controller
//! processes while this independently owned worker command remains alive.

use std::time::Duration;

use kiln_core::domain::{Command, CommandEnvelope, Outcome, State, decide, evolve};
use kiln_core::types::{CONTRACT_VERSION_V1, CoreResult, Diagnostic, Limits};
use kiln_core::{AttemptId, CommandId, EffectId, ProviderProfileId, RunId};

const OBSERVATION_WINDOW_SECONDS: u64 = 30;

fn main() -> CoreResult<()> {
    println!("kiln live CI: compiling completed; behavior probe started");
    std::thread::sleep(Duration::from_secs(OBSERVATION_WINDOW_SECONDS));
    assert!(!Outcome::Unknown.is_terminal());
    let state = State::default();
    let attempt_id = AttemptId::new("live-ci-attempt")?;
    let start = CommandEnvelope {
        contract_version: CONTRACT_VERSION_V1,
        command_id: CommandId::new("live-ci-start")?,
        expected_revision: state.revision,
        command: Command::StartRun {
            run_id: RunId::new("live-ci-run")?,
            attempt_id: attempt_id.clone(),
            provider_profile: ProviderProfileId::new("live-ci-provider")?,
            effect_id: EffectId::new("live-ci-effect")?,
        },
    };
    let started = apply(&state, &start)?;
    assert_eq!(
        started
            .attempts
            .get(&attempt_id)
            .map(|attempt| attempt.outcome),
        Some(Outcome::Pending)
    );
    let completion = CommandEnvelope {
        contract_version: CONTRACT_VERSION_V1,
        command_id: CommandId::new("live-ci-complete")?,
        expected_revision: started.revision,
        command: Command::CompleteAttempt {
            attempt_id: attempt_id.clone(),
            outcome: Outcome::DomainFailure,
            external_reference: None,
            products: Vec::new(),
            metrics: Vec::new(),
        },
    };
    let terminal = apply(&started, &completion)?;
    assert_eq!(
        terminal
            .attempts
            .get(&attempt_id)
            .map(|attempt| attempt.outcome),
        Some(Outcome::DomainFailure)
    );
    let late_success = CommandEnvelope {
        command_id: CommandId::new("live-ci-late-success")?,
        expected_revision: terminal.revision,
        command: Command::CompleteAttempt {
            attempt_id,
            outcome: Outcome::Success,
            external_reference: None,
            products: Vec::new(),
            metrics: Vec::new(),
        },
        ..completion
    };
    assert!(decide(&terminal, &late_success, Limits::default()).is_err());
    println!("kiln live CI: Unknown stays unresolved; terminal failure rejects late success");
    Ok(())
}

fn apply(state: &State, command: &CommandEnvelope) -> CoreResult<State> {
    let decision = decide(state, command, Limits::default())?;
    let event = decision.events.first().ok_or_else(|| {
        Diagnostic::new(
            "missing_probe_event",
            "live_ci",
            "transition emitted no event",
        )
    })?;
    evolve(state, event)
}
