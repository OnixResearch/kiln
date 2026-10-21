use kiln_adapter_radicle::status::{RadicleStatusShell, StatusDispatch};
use kiln_core::domain::State;
use kiln_core::operations::replay;
use kiln_onixos_pilot::JournalLock;

use crate::Result;
use crate::coordinator::CiProfile;
use crate::journal::{CiJournal, admit_status_observation};
use crate::project::ProjectWorkerProfile;

// r[impl kiln.radicle_ci.status] r[impl kiln.radicle_ci.recovery]
pub(crate) fn drive(
    profile: &CiProfile,
    worker: &ProjectWorkerProfile,
    status: &RadicleStatusShell,
    lock: &JournalLock,
    journal: &mut CiJournal,
) -> Result<()> {
    let state = replay(&State::default(), &journal.records).map_err(text)?;
    let Some(plan) = journal.status_plan(profile, worker, &state)? else {
        return Ok(());
    };
    let observation = if journal.status_intent.is_some() {
        // A reserved intent may have crossed the signing/store boundary. Absence
        // after read reconciliation never authorizes a second publication.
        status
            .reconcile(&plan)
            .map_err(text)?
            .map(StatusDispatch::Written)
    } else {
        journal.status_intent = Some(plan.clone());
        journal.seal()?;
        lock.store(journal, profile.max_journal_bytes)?;
        Some(
            status
                .publish(&plan, "canonical exact-source CI completion")
                .map_err(text)?,
        )
    };
    if let Some(observation) = &observation {
        admit_status_observation(profile, &plan, observation)?;
    }
    journal.status_observation = observation;
    journal.seal()?;
    lock.store(journal, profile.max_journal_bytes)
}

fn text(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
