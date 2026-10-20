//! QuePaxa interval summary register transition helpers.

use vstd::prelude::*;

use crate::quepaxa::model::*;
use crate::quepaxa::order::*;

verus! {

// r[impl consensus.quepaxa.isr]
pub open spec fn record_spec(
    state: QuePaxaIsrState,
    proposal_step: u64,
    proposal: QuePaxaProposal,
) -> QuePaxaIsrRecordResult {
    if proposal_step < state.step {
        QuePaxaIsrRecordResult {
            state,
            prior: state.current,
            stale: true,
            skipped_step: false,
        }
    } else if proposal_step == state.step {
        QuePaxaIsrRecordResult {
            state: QuePaxaIsrState {
                step: state.step,
                first: state.first,
                current: best_proposal_spec(state.current, proposal),
            },
            prior: state.current,
            stale: false,
            skipped_step: false,
        }
    } else if proposal_step - state.step == QUEPAXA_STEP_INCREMENT {
        QuePaxaIsrRecordResult {
            state: QuePaxaIsrState { step: proposal_step, first: proposal, current: proposal },
            prior: state.current,
            stale: false,
            skipped_step: false,
        }
    } else {
        QuePaxaIsrRecordResult {
            state: QuePaxaIsrState { step: proposal_step, first: proposal, current: proposal },
            prior: nil_proposal_spec(),
            stale: false,
            skipped_step: true,
        }
    }
}

// r[impl consensus.quepaxa.isr]
pub fn record(
    state: QuePaxaIsrState,
    proposal_step: u64,
    proposal: QuePaxaProposal,
) -> (result: QuePaxaIsrRecordResult)
    ensures result == record_spec(state, proposal_step, proposal)
{
    if proposal_step < state.step {
        QuePaxaIsrRecordResult {
            state,
            prior: state.current,
            stale: true,
            skipped_step: false,
        }
    } else if proposal_step == state.step {
        QuePaxaIsrRecordResult {
            state: QuePaxaIsrState {
                step: state.step,
                first: state.first,
                current: best_proposal(state.current, proposal),
            },
            prior: state.current,
            stale: false,
            skipped_step: false,
        }
    } else if proposal_step - state.step == QUEPAXA_STEP_INCREMENT {
        QuePaxaIsrRecordResult {
            state: QuePaxaIsrState { step: proposal_step, first: proposal, current: proposal },
            prior: state.current,
            stale: false,
            skipped_step: false,
        }
    } else {
        QuePaxaIsrRecordResult {
            state: QuePaxaIsrState { step: proposal_step, first: proposal, current: proposal },
            prior: nil_proposal(),
            stale: false,
            skipped_step: true,
        }
    }
}

// r[verify consensus.quepaxa.isr]
pub proof fn isr_same_step_preserves_first(state: QuePaxaIsrState, proposal: QuePaxaProposal)
    ensures record_spec(state, state.step, proposal).state.first == state.first
{
}

// r[verify consensus.quepaxa.isr]
pub proof fn isr_stale_update_preserves_state(
    state: QuePaxaIsrState,
    proposal_step: u64,
    proposal: QuePaxaProposal,
)
    requires proposal_step < state.step
    ensures record_spec(state, proposal_step, proposal).state == state
{
}

// r[verify consensus.quepaxa.isr]
pub proof fn isr_skipped_step_returns_nil(
    state: QuePaxaIsrState,
    proposal_step: u64,
    proposal: QuePaxaProposal,
)
    requires proposal_step > state.step, proposal_step - state.step != QUEPAXA_STEP_INCREMENT
    ensures record_spec(state, proposal_step, proposal).prior == nil_proposal_spec()
{
}

} // verus!
