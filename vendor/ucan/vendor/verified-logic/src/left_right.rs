//! Verified left-right reader/writer synchronization.
//!
//! Left-right keeps two copies of a data structure. Readers pick the current
//! active side and never retry. A single writer mutates the inactive side,
//! swaps the active index, waits for readers on the old side to drain, then
//! catches the old side up.

use vstd::prelude::*;

verus! {

pub const SIDE_LEFT: u8 = 0u8;
pub const SIDE_RIGHT: u8 = 1u8;

/// Left-right state: active side, per-side versions, per-side reader counts.
#[derive(Copy, Clone, Debug)]
pub struct LeftRightState {
    pub active_side: u8,
    pub left_version: u64,
    pub right_version: u64,
    pub left_readers: u64,
    pub right_readers: u64,
}

/// Reader snapshot ticket returned by enter.
#[derive(Copy, Clone, Debug)]
pub struct ReaderTicket {
    pub side: u8,
    pub version: u64,
}

/// Combined enter result.
#[derive(Copy, Clone, Debug)]
pub struct EnterResult {
    pub state: LeftRightState,
    pub ticket: ReaderTicket,
}

// --- side helpers ---

// r[depends left-right.index-toggle]
/// A valid side is left or right.
pub open spec fn valid_side_spec(side: u8) -> bool {
    side == SIDE_LEFT || side == SIDE_RIGHT
}

// r[impl left-right.index-toggle]
pub fn valid_side(side: u8) -> (result: bool)
    ensures result == valid_side_spec(side)
{
    side == SIDE_LEFT || side == SIDE_RIGHT
}

// r[depends left-right.index-toggle]
/// Switch to the other side.
pub open spec fn other_side_spec(side: u8) -> u8 {
    if side == SIDE_LEFT { SIDE_RIGHT } else { SIDE_LEFT }
}

// r[impl left-right.index-toggle]
pub fn other_side(side: u8) -> (result: u8)
    requires valid_side_spec(side)
    ensures result == other_side_spec(side)
{
    if side == SIDE_LEFT { SIDE_RIGHT } else { SIDE_LEFT }
}

// r[depends left-right.index-toggle]
/// Version stored on one side.
pub open spec fn version_on_side_spec(state: LeftRightState, side: u8) -> u64 {
    if side == SIDE_LEFT { state.left_version } else { state.right_version }
}

// r[impl left-right.index-toggle]
pub fn version_on_side(state: &LeftRightState, side: u8) -> (result: u64)
    requires valid_side_spec(side)
    ensures result == version_on_side_spec(*state, side)
{
    if side == SIDE_LEFT { state.left_version } else { state.right_version }
}

// r[depends left-right.reader-tracking]
/// Reader count tracked on one side.
pub open spec fn reader_count_on_side_spec(state: LeftRightState, side: u8) -> u64 {
    if side == SIDE_LEFT { state.left_readers } else { state.right_readers }
}

// r[impl left-right.reader-tracking]
pub fn reader_count_on_side(state: &LeftRightState, side: u8) -> (result: u64)
    requires valid_side_spec(side)
    ensures result == reader_count_on_side_spec(*state, side)
{
    if side == SIDE_LEFT { state.left_readers } else { state.right_readers }
}

// r[depends left-right.index-toggle]
/// The currently active version.
pub open spec fn active_version_spec(state: LeftRightState) -> u64 {
    version_on_side_spec(state, state.active_side)
}

// r[depends left-right.index-toggle]
/// The currently inactive side.
pub open spec fn inactive_side_spec(state: LeftRightState) -> u8 {
    other_side_spec(state.active_side)
}

// r[depends left-right.index-toggle]
/// The version stored on the inactive side.
pub open spec fn inactive_version_spec(state: LeftRightState) -> u64 {
    version_on_side_spec(state, inactive_side_spec(state))
}

// r[depends left-right.index-toggle]
/// Copies are synchronized when both sides store the same version.
pub open spec fn copies_in_sync_spec(state: LeftRightState) -> bool {
    state.left_version == state.right_version
}

// r[depends left-right.index-toggle]
/// Basic left-right invariant: active side valid, active version is never older
/// than inactive version, and versions differ by at most one published step.
pub open spec fn state_invariant_spec(state: LeftRightState) -> bool {
    valid_side_spec(state.active_side)
    && active_version_spec(state) >= inactive_version_spec(state)
    && active_version_spec(state) as int - inactive_version_spec(state) as int <= 1
}

// r[depends left-right.index-toggle]
/// Writer may publish a fresh version only when both copies are synchronized.
pub open spec fn ready_to_publish_spec(state: LeftRightState) -> bool {
    state_invariant_spec(state)
    && copies_in_sync_spec(state)
    && active_version_spec(state) < u64::MAX
}

// r[depends left-right.index-toggle]
/// Initialize with both copies at the same version and no readers.
pub open spec fn initial_state_spec(version: u64) -> LeftRightState {
    LeftRightState {
        active_side: SIDE_LEFT,
        left_version: version,
        right_version: version,
        left_readers: 0,
        right_readers: 0,
    }
}

// r[impl left-right.index-toggle]
pub fn initial_state(version: u64) -> (result: LeftRightState)
    ensures result == initial_state_spec(version)
{
    LeftRightState {
        active_side: SIDE_LEFT,
        left_version: version,
        right_version: version,
        left_readers: 0,
        right_readers: 0,
    }
}

// --- reader enter/exit ---

// r[depends left-right.reader-tracking]
/// Enter returns a ticket for the current active side and version.
pub open spec fn reader_enter_ticket_spec(state: LeftRightState) -> ReaderTicket {
    ReaderTicket {
        side: state.active_side,
        version: active_version_spec(state),
    }
}

// r[depends left-right.reader-tracking]
/// Reader enter increments the count on the active side.
pub open spec fn reader_enter_state_spec(state: LeftRightState) -> LeftRightState {
    if state.active_side == SIDE_LEFT {
        LeftRightState {
            active_side: state.active_side,
            left_version: state.left_version,
            right_version: state.right_version,
            left_readers: (state.left_readers + 1) as u64,
            right_readers: state.right_readers,
        }
    } else {
        LeftRightState {
            active_side: state.active_side,
            left_version: state.left_version,
            right_version: state.right_version,
            left_readers: state.left_readers,
            right_readers: (state.right_readers + 1) as u64,
        }
    }
}

// r[depends left-right.snapshot-consistency]
// r[impl left-right.snapshot-consistency]
/// A ticket matches a state's snapshot when that side still carries the ticket
/// version.
pub open spec fn reader_snapshot_matches_spec(
    state: LeftRightState,
    ticket: ReaderTicket,
) -> bool {
    valid_side_spec(ticket.side)
    && ticket.version == version_on_side_spec(state, ticket.side)
}

// r[impl left-right.reader-tracking]
pub fn reader_enter(state: &LeftRightState) -> (result: EnterResult)
    requires
        state_invariant_spec(*state),
        reader_count_on_side_spec(*state, state.active_side) < u64::MAX,
    ensures
        result.state == reader_enter_state_spec(*state),
        result.ticket == reader_enter_ticket_spec(*state),
        state_invariant_spec(result.state),
        reader_snapshot_matches_spec(result.state, result.ticket),
{
    if state.active_side == SIDE_LEFT {
        EnterResult {
            state: LeftRightState {
                active_side: state.active_side,
                left_version: state.left_version,
                right_version: state.right_version,
                left_readers: state.left_readers + 1,
                right_readers: state.right_readers,
            },
            ticket: ReaderTicket {
                side: state.active_side,
                version: state.left_version,
            },
        }
    } else {
        EnterResult {
            state: LeftRightState {
                active_side: state.active_side,
                left_version: state.left_version,
                right_version: state.right_version,
                left_readers: state.left_readers,
                right_readers: state.right_readers + 1,
            },
            ticket: ReaderTicket {
                side: state.active_side,
                version: state.right_version,
            },
        }
    }
}

// r[depends left-right.reader-tracking]
/// Reader exit decrements the count on the ticket side.
pub open spec fn reader_exit_state_spec(
    state: LeftRightState,
    ticket: ReaderTicket,
) -> LeftRightState {
    if ticket.side == SIDE_LEFT {
        LeftRightState {
            active_side: state.active_side,
            left_version: state.left_version,
            right_version: state.right_version,
            left_readers: (state.left_readers - 1) as u64,
            right_readers: state.right_readers,
        }
    } else {
        LeftRightState {
            active_side: state.active_side,
            left_version: state.left_version,
            right_version: state.right_version,
            left_readers: state.left_readers,
            right_readers: (state.right_readers - 1) as u64,
        }
    }
}

// r[impl left-right.reader-tracking]
pub fn reader_exit(state: &LeftRightState, ticket: &ReaderTicket) -> (result: LeftRightState)
    requires
        state_invariant_spec(*state),
        valid_side_spec(ticket.side),
        reader_count_on_side_spec(*state, ticket.side) > 0,
    ensures
        result == reader_exit_state_spec(*state, *ticket),
        state_invariant_spec(result),
{
    if ticket.side == SIDE_LEFT {
        LeftRightState {
            active_side: state.active_side,
            left_version: state.left_version,
            right_version: state.right_version,
            left_readers: state.left_readers - 1,
            right_readers: state.right_readers,
        }
    } else {
        LeftRightState {
            active_side: state.active_side,
            left_version: state.left_version,
            right_version: state.right_version,
            left_readers: state.left_readers,
            right_readers: state.right_readers - 1,
        }
    }
}

// --- quiescence and writer operations ---

// r[depends left-right.quiescence]
/// A side is quiescent when it has no readers.
pub open spec fn side_quiescent_spec(state: LeftRightState, side: u8) -> bool {
    reader_count_on_side_spec(state, side) == 0
}

// r[impl left-right.quiescence]
pub fn side_quiescent(state: &LeftRightState, side: u8) -> (result: bool)
    requires valid_side_spec(side)
    ensures result == side_quiescent_spec(*state, side)
{
    if side == SIDE_LEFT {
        state.left_readers == 0
    } else {
        state.right_readers == 0
    }
}

// r[depends left-right.quiescence]
/// Writer may catch the inactive side up only after that side is quiescent.
pub open spec fn can_catch_up_spec(state: LeftRightState) -> bool {
    side_quiescent_spec(state, inactive_side_spec(state))
}

// r[impl left-right.quiescence]
pub fn can_catch_up(state: &LeftRightState) -> (result: bool)
    requires state_invariant_spec(*state)
    ensures result == can_catch_up_spec(*state)
{
    side_quiescent(state, other_side(state.active_side))
}

// r[depends left-right.writer-publish]
/// Publish a fresh version: writer updates inactive copy to next version and
/// flips the active side. Counts stay unchanged.
pub open spec fn writer_publish_state_spec(state: LeftRightState) -> LeftRightState {
    if state.active_side == SIDE_LEFT {
        LeftRightState {
            active_side: SIDE_RIGHT,
            left_version: state.left_version,
            right_version: (state.left_version + 1) as u64,
            left_readers: state.left_readers,
            right_readers: state.right_readers,
        }
    } else {
        LeftRightState {
            active_side: SIDE_LEFT,
            left_version: (state.right_version + 1) as u64,
            right_version: state.right_version,
            left_readers: state.left_readers,
            right_readers: state.right_readers,
        }
    }
}

// r[impl left-right.writer-publish]
pub fn writer_publish(state: &LeftRightState) -> (result: LeftRightState)
    requires ready_to_publish_spec(*state)
    ensures
        result == writer_publish_state_spec(*state),
        state_invariant_spec(result),
        result.active_side == other_side_spec(state.active_side),
        active_version_spec(result) == active_version_spec(*state) + 1,
        inactive_version_spec(result) == active_version_spec(*state),
{
    if state.active_side == SIDE_LEFT {
        LeftRightState {
            active_side: SIDE_RIGHT,
            left_version: state.left_version,
            right_version: state.left_version + 1,
            left_readers: state.left_readers,
            right_readers: state.right_readers,
        }
    } else {
        LeftRightState {
            active_side: SIDE_LEFT,
            left_version: state.right_version + 1,
            right_version: state.right_version,
            left_readers: state.left_readers,
            right_readers: state.right_readers,
        }
    }
}

// r[depends left-right.quiescence]
/// Catch the inactive side up to the active version after its readers drain.
pub open spec fn writer_catch_up_state_spec(state: LeftRightState) -> LeftRightState {
    if state.active_side == SIDE_LEFT {
        LeftRightState {
            active_side: state.active_side,
            left_version: state.left_version,
            right_version: state.left_version,
            left_readers: state.left_readers,
            right_readers: state.right_readers,
        }
    } else {
        LeftRightState {
            active_side: state.active_side,
            left_version: state.right_version,
            right_version: state.right_version,
            left_readers: state.left_readers,
            right_readers: state.right_readers,
        }
    }
}

// r[impl left-right.quiescence]
pub fn writer_catch_up(state: &LeftRightState) -> (result: LeftRightState)
    requires
        state_invariant_spec(*state),
        can_catch_up_spec(*state),
    ensures
        result == writer_catch_up_state_spec(*state),
        state_invariant_spec(result),
        copies_in_sync_spec(result),
        active_version_spec(result) == active_version_spec(*state),
{
    if state.active_side == SIDE_LEFT {
        LeftRightState {
            active_side: state.active_side,
            left_version: state.left_version,
            right_version: state.left_version,
            left_readers: state.left_readers,
            right_readers: state.right_readers,
        }
    } else {
        LeftRightState {
            active_side: state.active_side,
            left_version: state.right_version,
            right_version: state.right_version,
            left_readers: state.left_readers,
            right_readers: state.right_readers,
        }
    }
}

// === Proofs ===

// r[verify left-right.index-toggle]
/// Initial state satisfies the invariant and starts synchronized.
proof fn initial_state_is_valid(version: u64)
    ensures
        state_invariant_spec(initial_state_spec(version)),
        copies_in_sync_spec(initial_state_spec(version)),
        active_version_spec(initial_state_spec(version)) == version,
{}

// r[verify left-right.index-toggle]
/// other_side stays within the two valid sides.
pub proof fn other_side_is_valid(side: u8)
    requires valid_side_spec(side)
    ensures valid_side_spec(other_side_spec(side))
{}

// r[verify left-right.index-toggle]
/// Toggling twice returns the original side.
pub proof fn other_side_involution(side: u8)
    requires valid_side_spec(side)
    ensures other_side_spec(other_side_spec(side)) == side
{}

// r[verify left-right.reader-tracking]
/// Enter then exit with the returned ticket restores the original state.
pub proof fn reader_enter_exit_roundtrip(state: LeftRightState)
    requires
        state_invariant_spec(state),
        reader_count_on_side_spec(state, state.active_side) < u64::MAX,
    ensures
        reader_exit_state_spec(reader_enter_state_spec(state), reader_enter_ticket_spec(state))
            == state,
{}

// r[verify left-right.writer-publish]
/// Publish always flips the active side.
pub proof fn writer_publish_toggles(state: LeftRightState)
    requires ready_to_publish_spec(state)
    ensures writer_publish_state_spec(state).active_side == other_side_spec(state.active_side)
{}

// r[verify left-right.writer-publish]
/// Consecutive publish operations alternate sides.
pub proof fn writer_publish_alternates(state: LeftRightState)
    requires ready_to_publish_spec(state)
    ensures other_side_spec(writer_publish_state_spec(state).active_side) == state.active_side
{
    other_side_involution(state.active_side);
}

// r[verify left-right.writer-publish]
/// Publish strictly advances the active version by one.
pub proof fn writer_publish_advances_version(state: LeftRightState)
    requires ready_to_publish_spec(state)
    ensures active_version_spec(writer_publish_state_spec(state)) == active_version_spec(state) + 1
{}

// r[verify left-right.quiescence]
/// Catch-up restores synchronized copies.
pub proof fn writer_catch_up_restores_sync(state: LeftRightState)
    requires
        state_invariant_spec(state),
        can_catch_up_spec(state),
    ensures copies_in_sync_spec(writer_catch_up_state_spec(state))
{}

// r[verify left-right.snapshot-consistency]
/// After enter then publish, the returned ticket still points at the old side's
/// unchanged snapshot.
pub proof fn published_reader_keeps_snapshot(state: LeftRightState)
    requires
        ready_to_publish_spec(state),
        reader_count_on_side_spec(state, state.active_side) < u64::MAX,
    ensures ({
        let entered = reader_enter_state_spec(state);
        let ticket = reader_enter_ticket_spec(state);
        let published = writer_publish_state_spec(entered);
        reader_snapshot_matches_spec(published, ticket)
    })
{}

// r[verify left-right.snapshot-consistency]
// r[verify left-right.quiescence]
/// A reader that entered before publish blocks catch-up on the old side until it exits.
pub proof fn live_reader_blocks_catch_up(state: LeftRightState)
    requires
        ready_to_publish_spec(state),
        reader_count_on_side_spec(state, state.active_side) < u64::MAX,
    ensures ({
        let entered = reader_enter_state_spec(state);
        let published = writer_publish_state_spec(entered);
        !can_catch_up_spec(published)
    })
{}

// r[verify left-right.quiescence]
/// If the published old side has exactly one remaining reader, one exit makes
/// catch-up possible.
pub proof fn single_reader_exit_enables_catch_up(state: LeftRightState)
    requires
        state_invariant_spec(state),
        inactive_side_spec(state) == SIDE_LEFT ==> state.left_readers == 1,
        inactive_side_spec(state) == SIDE_RIGHT ==> state.right_readers == 1,
    ensures ({
        let ticket = ReaderTicket {
            side: inactive_side_spec(state),
            version: inactive_version_spec(state),
        };
        can_catch_up_spec(reader_exit_state_spec(state, ticket))
    })
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn synced_state(active_side: u8, version: u64, left_readers: u64, right_readers: u64) -> LeftRightState {
        LeftRightState {
            active_side,
            left_version: version,
            right_version: version,
            left_readers,
            right_readers,
        }
    }

    // r[verify left-right.index-toggle]
    #[test]
    fn test_initial_state() {
        let state = initial_state(7);
        assert_eq!(state.active_side, SIDE_LEFT);
        assert_eq!(state.left_version, 7);
        assert_eq!(state.right_version, 7);
        assert_eq!(state.left_readers, 0);
        assert_eq!(state.right_readers, 0);
        assert!(valid_side(state.active_side));
    }

    // r[verify left-right.reader-tracking]
    #[test]
    fn test_reader_enter_exit_left() {
        let state = synced_state(SIDE_LEFT, 10, 0, 2);
        let entered = reader_enter(&state);
        assert_eq!(entered.ticket.side, SIDE_LEFT);
        assert_eq!(entered.ticket.version, 10);
        assert_eq!(entered.state.left_readers, 1);
        assert_eq!(entered.state.right_readers, 2);

        let exited = reader_exit(&entered.state, &entered.ticket);
        assert_eq!(exited.active_side, state.active_side);
        assert_eq!(exited.left_version, state.left_version);
        assert_eq!(exited.right_version, state.right_version);
        assert_eq!(exited.left_readers, state.left_readers);
        assert_eq!(exited.right_readers, state.right_readers);
    }

    // r[verify left-right.writer-publish]
    #[test]
    fn test_writer_publish_cycle() {
        let state = synced_state(SIDE_LEFT, 4, 1, 0);
        let published = writer_publish(&state);
        assert_eq!(published.active_side, SIDE_RIGHT);
        assert_eq!(published.left_version, 4);
        assert_eq!(published.right_version, 5);
        assert_eq!(published.left_readers, 1);
        assert_eq!(published.right_readers, 0);
        assert!(!can_catch_up(&published));

        let drained = reader_exit(&published, &ReaderTicket {
            side: SIDE_LEFT,
            version: 4,
        });
        assert!(can_catch_up(&drained));
        let caught = writer_catch_up(&drained);
        assert_eq!(caught.left_version, 5);
        assert_eq!(caught.right_version, 5);
        assert_eq!(caught.active_side, SIDE_RIGHT);
    }

    // r[verify left-right.snapshot-consistency]
    #[test]
    fn test_old_reader_keeps_snapshot_after_publish() {
        let state = synced_state(SIDE_RIGHT, 9, 3, 0);
        let entered = reader_enter(&state);
        let published = writer_publish(&entered.state);

        assert_eq!(entered.ticket.side, SIDE_RIGHT);
        assert_eq!(entered.ticket.version, 9);
        assert_eq!(published.right_version, 9);
        assert_eq!(published.left_version, 10);
        assert!(!can_catch_up(&published));
    }

    proptest! {
        // r[verify left-right.reader-tracking]
        #[test]
        fn prop_enter_exit_roundtrip(
            version: u64,
            active_left: bool,
            left_readers in 0u64..1000,
            right_readers in 0u64..1000,
        ) {
            let state = if active_left {
                synced_state(SIDE_LEFT, version, left_readers, right_readers)
            } else {
                synced_state(SIDE_RIGHT, version, left_readers, right_readers)
            };
            let entered = reader_enter(&state);
            let exited = reader_exit(&entered.state, &entered.ticket);
            prop_assert_eq!(exited.active_side, state.active_side);
            prop_assert_eq!(exited.left_version, state.left_version);
            prop_assert_eq!(exited.right_version, state.right_version);
            prop_assert_eq!(exited.left_readers, state.left_readers);
            prop_assert_eq!(exited.right_readers, state.right_readers);
        }

        // r[verify left-right.writer-publish]
        #[test]
        fn prop_publish_toggles_and_advances(
            version in 0u64..u64::MAX,
            active_left: bool,
            left_readers in 0u64..1000,
            right_readers in 0u64..1000,
        ) {
            prop_assume!(version < u64::MAX);
            let state = if active_left {
                synced_state(SIDE_LEFT, version, left_readers, right_readers)
            } else {
                synced_state(SIDE_RIGHT, version, left_readers, right_readers)
            };
            let published = writer_publish(&state);
            prop_assert_ne!(published.active_side, state.active_side);
            prop_assert_eq!(version_on_side(&published, published.active_side), version + 1);
            prop_assert_eq!(version_on_side(&published, state.active_side), version);
            prop_assert_eq!(published.left_readers, left_readers);
            prop_assert_eq!(published.right_readers, right_readers);
        }

        // r[verify left-right.snapshot-consistency]
        // r[verify left-right.quiescence]
        #[test]
        fn prop_live_reader_blocks_catch_up(
            version in 0u64..u64::MAX,
            active_left: bool,
            left_readers in 0u64..1000,
            right_readers in 0u64..1000,
        ) {
            prop_assume!(version < u64::MAX);
            let state = if active_left {
                synced_state(SIDE_LEFT, version, left_readers, right_readers)
            } else {
                synced_state(SIDE_RIGHT, version, left_readers, right_readers)
            };
            let entered = reader_enter(&state);
            let published = writer_publish(&entered.state);
            prop_assert!(!can_catch_up(&published));
            prop_assert_eq!(version_on_side(&published, entered.ticket.side), entered.ticket.version);
        }

        // r[verify left-right.quiescence]
        #[test]
        fn prop_exit_then_catch_up_restores_sync(
            version in 0u64..u64::MAX,
            active_left: bool,
            inactive_readers in 0u64..1000,
        ) {
            prop_assume!(version < u64::MAX);
            let state = if active_left {
                synced_state(SIDE_LEFT, version, 0, inactive_readers)
            } else {
                synced_state(SIDE_RIGHT, version, inactive_readers, 0)
            };
            let entered = reader_enter(&state);
            let published = writer_publish(&entered.state);
            let drained = reader_exit(&published, &entered.ticket);
            prop_assert!(can_catch_up(&drained));
            let caught = writer_catch_up(&drained);
            prop_assert_eq!(caught.left_version, caught.right_version);
            prop_assert_eq!(version_on_side(&caught, caught.active_side), version + 1);
        }
    }
}
