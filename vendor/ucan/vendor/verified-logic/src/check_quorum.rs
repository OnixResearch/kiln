//! Verified check-quorum protocol for Raft leader liveness.
//!
//! A leader periodically checks that it can still reach a quorum of
//! the cluster. If it cannot (e.g., due to a network partition), it
//! steps down to prevent serving stale reads. This is the continuous
//! liveness check complementing the per-read check in `read_index`.

#![allow(clippy::wildcard_imports)]

use vstd::prelude::*;

use crate::quorum::*;

verus! {

// --- has_recent_contact ---

// r[depends check-quorum.recent-contact]
/// Spec: has this server contacted the leader recently?
///
/// True when the elapsed time since last contact is less than the
/// election timeout (saturating subtraction to avoid underflow).
pub open spec fn has_recent_contact_spec(
    last_contact: u64,
    now: u64,
    election_timeout: u64,
) -> bool {
    let elapsed = if now >= last_contact { (now - last_contact) as u64 } else { 0u64 };
    elapsed < election_timeout
}

// r[impl check-quorum.recent-contact]
/// Check if a server has had recent contact.
pub fn has_recent_contact(
    last_contact: u64,
    now: u64,
    election_timeout: u64,
) -> (result: bool)
    ensures result == has_recent_contact_spec(last_contact, now, election_timeout)
{
    let elapsed = if now >= last_contact { now - last_contact } else { 0 };
    elapsed < election_timeout
}

// --- count_recent_contacts ---

// r[depends check-quorum.count-recent]
/// Spec: count how many servers have recent contact (recursive).
pub open spec fn count_recent_from_spec(
    contacts: Seq<u64>,
    now: u64,
    election_timeout: u64,
    from: int,
) -> int
    decreases contacts.len() - from
{
    if from >= contacts.len() {
        0int
    } else {
        let recent = if has_recent_contact_spec(contacts[from], now, election_timeout) {
            1int
        } else {
            0int
        };
        recent + count_recent_from_spec(contacts, now, election_timeout, from + 1)
    }
}

// r[depends check-quorum.count-recent]
/// Spec: total count of servers with recent contact.
pub open spec fn count_recent_contacts_spec(
    contacts: Seq<u64>,
    now: u64,
    election_timeout: u64,
) -> int {
    count_recent_from_spec(contacts, now, election_timeout, 0)
}

/// Helper: count is bounded by remaining length.
proof fn count_recent_bounded(
    contacts: Seq<u64>,
    now: u64,
    election_timeout: u64,
    from: int,
)
    ensures
        0 <= count_recent_from_spec(contacts, now, election_timeout, from),
        count_recent_from_spec(contacts, now, election_timeout, from)
            <= contacts.len() - (if from < contacts.len() { from } else { contacts.len() as int }),
    decreases contacts.len() - from
{
    if from >= contacts.len() {
    } else {
        count_recent_bounded(contacts, now, election_timeout, from + 1);
    }
}

// r[impl check-quorum.count-recent]
/// Count servers with recent contact.
pub fn count_recent_contacts(
    contacts: &[u64],
    now: u64,
    election_timeout: u64,
) -> (result: u64)
    requires contacts@.len() <= u64::MAX as int
    ensures result as int == count_recent_contacts_spec(contacts@, now, election_timeout)
{
    let mut count: u64 = 0;
    let mut i: usize = 0;
    proof { count_recent_bounded(contacts@, now, election_timeout, 0); }
    while i < contacts.len()
        invariant
            0 <= i <= contacts@.len(),
            count as int == count_recent_contacts_spec(contacts@, now, election_timeout)
                - count_recent_from_spec(contacts@, now, election_timeout, i as int),
            contacts@.len() <= u64::MAX as int,
            count as int <= i as int,
        decreases contacts.len() - i
    {
        proof { count_recent_bounded(contacts@, now, election_timeout, i as int + 1); }
        if has_recent_contact(contacts[i], now, election_timeout) {
            count = count + 1;
        }
        i = i + 1;
    }
    count
}

// --- should_step_down ---

// r[depends check-quorum.should-step-down]
/// Spec: should the leader step down?
///
/// True when the count of recently-contacted servers is less than
/// a quorum (majority).
pub open spec fn should_step_down_spec(
    recent_count: u64,
    cluster_size: u64,
) -> bool
    recommends cluster_size > 0
{
    !is_quorum_spec(recent_count, cluster_size)
}

// r[impl check-quorum.should-step-down]
/// Check if the leader should step down.
pub fn should_step_down(
    recent_count: u64,
    cluster_size: u64,
) -> (result: bool)
    requires cluster_size > 0
    ensures result == should_step_down_spec(recent_count, cluster_size)
{
    !is_quorum(recent_count, cluster_size)
}

// --- Safety proofs ---

// r[verify check-quorum.recent-contact]
/// Contact at current time is always recent (if timeout > 0).
proof fn current_contact_is_recent(now: u64, election_timeout: u64)
    requires election_timeout > 0
    ensures has_recent_contact_spec(now, now, election_timeout)
{}

// r[verify check-quorum.recent-contact]
/// Very old contact is not recent.
proof fn old_contact_not_recent(last_contact: u64, now: u64, timeout: u64)
    requires now >= last_contact, now - last_contact >= timeout
    ensures !has_recent_contact_spec(last_contact, now, timeout)
{}

// r[verify check-quorum.should-step-down]
/// Singleton never steps down (majority(1) == 1, self always recent).
proof fn singleton_never_steps_down()
    ensures !should_step_down_spec(1, 1)
{}

// r[impl check-quorum.partitioned-leader]
// r[verify check-quorum.partitioned-leader]
/// Partitioned leader with only self must step down.
proof fn partitioned_leader_steps_down(cluster_size: u64)
    requires cluster_size > 1
    ensures should_step_down_spec(1, cluster_size)
{}

// r[impl check-quorum.leader-counts-self]
// r[verify check-quorum.leader-counts-self]
/// Full cluster never steps down.
proof fn full_cluster_stable(cluster_size: u64)
    requires cluster_size > 0
    ensures !should_step_down_spec(cluster_size, cluster_size)
{
    full_is_quorum(cluster_size);
}

// r[verify check-quorum.should-step-down]
/// Step-down is the negation of quorum.
proof fn step_down_is_not_quorum(recent_count: u64, cluster_size: u64)
    requires cluster_size > 0
    ensures should_step_down_spec(recent_count, cluster_size)
        == !is_quorum_spec(recent_count, cluster_size)
{}

} // verus!

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify check-quorum.recent-contact]
    #[test]
    fn test_has_recent_contact() {
        assert!(has_recent_contact(100, 100, 50)); // same time
        assert!(has_recent_contact(100, 120, 50)); // within timeout
        assert!(!has_recent_contact(100, 150, 50)); // at timeout
        assert!(!has_recent_contact(100, 200, 50)); // past timeout
    }

    // r[verify check-quorum.count-recent]
    #[test]
    fn test_count_recent_contacts() {
        let contacts = vec![100, 100, 100, 50, 50];
        assert_eq!(count_recent_contacts(&contacts, 100, 30), 3); // first 3 recent
        assert_eq!(count_recent_contacts(&contacts, 100, 60), 5); // all recent
        assert_eq!(count_recent_contacts(&[], 100, 50), 0); // empty
    }

    // r[verify check-quorum.should-step-down]
    #[test]
    fn test_should_step_down() {
        assert!(!should_step_down(3, 5)); // quorum
        assert!(should_step_down(2, 5)); // not quorum
        assert!(should_step_down(1, 5)); // partitioned
        assert!(!should_step_down(1, 1)); // singleton
    }

    // r[verify check-quorum.should-step-down]
    #[test]
    fn test_singleton_never_steps_down() {
        assert!(!should_step_down(1, 1));
    }

    proptest! {
        // r[verify check-quorum.recent-contact]
        #[test]
        fn prop_current_contact_recent(now: u64, timeout in 1u64..u64::MAX) {
            prop_assert!(has_recent_contact(now, now, timeout));
        }

        // r[verify check-quorum.partitioned-leader]
        #[test]
        fn prop_partitioned_steps_down(cluster_size in 2u64..100) {
            prop_assert!(should_step_down(1, cluster_size));
        }

        // r[verify check-quorum.should-step-down]
        #[test]
        fn prop_full_cluster_stable(cluster_size in 1u64..100) {
            prop_assert!(!should_step_down(cluster_size, cluster_size));
        }

        #[test]
        fn prop_step_down_boundary(
            cluster_size in 2u64..=50,
        ) {
            let majority = cluster_size / 2 + 1;
            // One below majority: step down
            if majority > 1 {
                prop_assert!(should_step_down(majority - 1, cluster_size));
            }
            // At majority: stable
            prop_assert!(!should_step_down(majority, cluster_size));
        }

        #[test]
        fn prop_expired_contact_not_recent(
            last_contact in 0u64..1000,
            timeout in 1u64..100,
        ) {
            // has_recent_contact(last_contact, now, timeout) checks elapsed < timeout
            let now = last_contact.saturating_add(timeout);
            if now >= last_contact + timeout {
                // elapsed = now - last_contact = timeout, timeout < timeout is false
                prop_assert!(!has_recent_contact(last_contact, now, timeout));
            }
        }

        #[test]
        fn prop_count_recent_bounded(
            contacts in proptest::collection::vec(0u64..200, 0..=10),
            now in 0u64..200,
            timeout in 1u64..50,
        ) {
            let count = count_recent_contacts(&contacts, now, timeout);
            prop_assert!(count as usize <= contacts.len());
        }
    }
}
