//! Pure speculative merge-queue planning: assumed predecessor chains, chain
//! validity, CI verdicts and window admission. No files, clocks, Git, SQLite or
//! engine access; the queue shell supplies observations and applies decisions.
// r[impl kiln.review.queue.speculative.chain]
// r[impl kiln.review.queue.speculative.window]
use kiln_core::gating::{GateMode, WindowPolicy, admitted_candidate_indices, update_window};
use serde::{Deserialize, Serialize};

use super::{Failure, hash};

/// Largest admitted speculative window (stacked candidates per target).
pub(super) const MAX_SPECULATIVE_WINDOW: usize = 16;
/// Retired chain identities kept per entry for late-result rejection.
pub(super) const MAX_SUPERSEDED_CHAINS: usize = 32;

/// One assumed predecessor: the exact candidate of an earlier queue entry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Link {
    pub queue_id: String,
    pub generation: u32,
    pub candidate_commit: String,
}

/// Durable assumed predecessor chain of one speculative candidate build.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Chain {
    /// BLAKE3 identity of this build attempt and its assumptions.
    pub id: String,
    /// Exact target observed when the candidate was built.
    pub root_target: String,
    /// Earlier candidates, in queue order, this candidate is stacked on.
    pub predecessors: Vec<Link>,
}

impl Chain {
    pub(super) fn new(
        queue_id: &str,
        generation: u32,
        root_target: &str,
        predecessors: Vec<Link>,
    ) -> Result<Self, Failure> {
        let id = hash(&(
            "kiln.review-queue.chain.v1",
            queue_id,
            generation,
            root_target,
            &predecessors,
        ))?;
        Ok(Self {
            id,
            root_target: root_target.to_owned(),
            predecessors,
        })
    }
    /// The commit this candidate fast-forwards once every predecessor landed.
    pub(super) fn base(&self) -> &str {
        self.predecessors
            .last()
            .map_or(self.root_target.as_str(), |link| &link.candidate_commit)
    }
}

/// A chain stays valid while the observed target is its root or one of its
/// predecessors (those landed in order), and every predecessor that has not
/// landed is exactly the active entry ahead with the same generation and
/// candidate. Any other target, ejection or rebuild ahead supersedes it.
pub(super) fn chain_valid(chain: &Chain, observed_target: &str, ahead: &[Link]) -> bool {
    let landed = if observed_target == chain.root_target {
        0
    } else {
        match chain
            .predecessors
            .iter()
            .position(|link| link.candidate_commit == observed_target)
        {
            Some(position) => position + 1,
            None => return false,
        }
    };
    chain.predecessors.get(landed..) == Some(ahead)
}

/// Record a retired chain identity, keeping the newest bounded history.
pub(super) fn retire(superseded: &mut Vec<String>, chain: &str) {
    if superseded.iter().any(|id| id == chain) {
        return;
    }
    superseded.push(chain.to_owned());
    if superseded.len() > MAX_SUPERSEDED_CHAINS {
        let excess = superseded.len() - MAX_SUPERSEDED_CHAINS;
        superseded.drain(..excess);
    }
}

/// Whether a provider completion still belongs to the entry's current chain.
pub(super) fn completion_current(current: Option<&Chain>, completed: &str) -> bool {
    current.is_some_and(|chain| chain.id == completed)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Verdict {
    /// A recorded run is no longer retained.
    Missing,
    Pending,
    Passed,
    Failed,
}

/// Combine `(terminal, success)` observations of every candidate run.
/// Any terminal failure fails fast; all terminal successes pass.
pub(super) fn ci_verdict(outcomes: &[Option<(bool, bool)>]) -> Verdict {
    if outcomes.iter().any(Option::is_none) {
        return Verdict::Missing;
    }
    let observed = outcomes.iter().flatten();
    if observed
        .clone()
        .any(|(terminal, success)| *terminal && !success)
    {
        Verdict::Failed
    } else if observed.clone().all(|(terminal, _)| *terminal) {
        Verdict::Passed
    } else {
        Verdict::Pending
    }
}

/// The stored window for a target, clamped into the current policy bounds;
/// targets without a stored window start optimistic at the ceiling.
pub(super) fn current_window(stored: Option<usize>, policy: WindowPolicy) -> usize {
    stored
        .unwrap_or(policy.ceiling)
        .clamp(policy.floor, policy.ceiling)
}

/// Admitted queue positions of `active` entries for one target.
pub(super) fn admitted(window: usize, active: usize) -> Result<Vec<usize>, Failure> {
    if active == 0 {
        return Ok(Vec::new());
    }
    admitted_candidate_indices(GateMode::Speculative, window, active)
        .map_err(|_| Failure::Unavailable)
}

/// Grow after a landed candidate, shrink after an ejection.
pub(super) fn next_window(
    current: usize,
    passed: bool,
    policy: WindowPolicy,
) -> Result<usize, Failure> {
    update_window(current, passed, policy).map_err(|_| Failure::Unavailable)
}

/// Profile admission bounds for a speculative window.
pub(super) fn window_admissible(policy: &WindowPolicy, max_entries: usize) -> bool {
    policy.floor >= 1
        && policy.floor <= policy.ceiling
        && policy.ceiling <= MAX_SPECULATIVE_WINDOW
        && policy.ceiling <= max_entries
        && (1..=policy.ceiling).contains(&policy.increase)
        && (1..=policy.ceiling).contains(&policy.decrease)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(id: &str, generation: u32, commit: &str) -> Link {
        Link {
            queue_id: id.into(),
            generation,
            candidate_commit: commit.into(),
        }
    }
    fn policy() -> WindowPolicy {
        WindowPolicy {
            floor: 1,
            ceiling: 3,
            increase: 1,
            decrease: 1,
        }
    }

    // r[verify kiln.review.queue.speculative.chain]
    #[test]
    fn chain_survives_in_order_landing_and_is_superseded_otherwise() -> Result<(), Failure> {
        let ahead = vec![link("a", 1, "ca"), link("b", 1, "cb")];
        let behind_first = vec![link("b", 1, "cb")];
        let chain = Chain::new("c", 1, "root", ahead.clone())?;
        assert_eq!(chain.base(), "cb");
        // Nothing landed: the whole stack is still ahead.
        assert!(chain_valid(&chain, "root", &ahead));
        // In-order landing of predecessors keeps the chain.
        assert!(chain_valid(&chain, "ca", &behind_first));
        assert!(chain_valid(&chain, "cb", &[]));
        // Foreign target movement supersedes.
        assert!(!chain_valid(&chain, "foreign", &ahead));
        // An ejected predecessor supersedes.
        assert!(!chain_valid(&chain, "root", &behind_first));
        // A rebuilt predecessor (new generation/candidate) supersedes.
        let rebuilt = vec![link("a", 2, "ca2"), link("b", 1, "cb")];
        assert!(!chain_valid(&chain, "root", &rebuilt));
        // A landed predecessor that is still reported ahead is inconsistent.
        assert!(!chain_valid(&chain, "ca", &ahead));
        Ok(())
    }

    #[test]
    fn chain_identity_binds_entry_generation_root_and_predecessors() -> Result<(), Failure> {
        let base = Chain::new("c", 1, "root", vec![link("a", 1, "ca")])?;
        assert_eq!(
            base,
            Chain::new("c", 1, "root", vec![link("a", 1, "ca")])?,
            "deterministic"
        );
        for other in [
            Chain::new("d", 1, "root", vec![link("a", 1, "ca")])?,
            Chain::new("c", 2, "root", vec![link("a", 1, "ca")])?,
            Chain::new("c", 1, "other", vec![link("a", 1, "ca")])?,
            Chain::new("c", 1, "root", vec![link("a", 2, "ca")])?,
            Chain::new("c", 1, "root", Vec::new())?,
        ] {
            assert_ne!(other.id, base.id);
        }
        assert_eq!(base.id.len(), 64);
        Ok(())
    }

    #[test]
    fn late_completions_and_retired_chains_are_rejected() -> Result<(), Failure> {
        let current = Chain::new("c", 2, "root", Vec::new())?;
        assert!(completion_current(Some(&current), &current.id));
        assert!(!completion_current(Some(&current), "superseded"));
        assert!(!completion_current(None, &current.id));
        let mut retired = Vec::new();
        for index in 0..=MAX_SUPERSEDED_CHAINS {
            retire(&mut retired, &format!("chain-{index}"));
        }
        retire(&mut retired, "chain-5");
        assert_eq!(retired.len(), MAX_SUPERSEDED_CHAINS);
        assert_eq!(retired.first().map(String::as_str), Some("chain-1"));
        Ok(())
    }

    #[test]
    fn ci_verdict_fails_fast_and_requires_every_run() {
        assert_eq!(ci_verdict(&[Some((true, true))]), Verdict::Passed);
        assert_eq!(
            ci_verdict(&[Some((true, true)), Some((false, false))]),
            Verdict::Pending
        );
        assert_eq!(
            ci_verdict(&[Some((false, false)), Some((true, false))]),
            Verdict::Failed
        );
        assert_eq!(
            ci_verdict(&[Some((true, true)), None]),
            Verdict::Missing,
            "a missing run never passes"
        );
    }

    // r[verify kiln.review.queue.speculative.window]
    #[test]
    fn window_admits_bounded_positions_and_adapts_within_policy() -> Result<(), Failure> {
        let policy = policy();
        assert_eq!(current_window(None, policy), 3);
        assert_eq!(current_window(Some(9), policy), 3);
        assert_eq!(current_window(Some(0), policy), 1);
        assert_eq!(admitted(3, 5)?, vec![0, 1, 2]);
        assert_eq!(admitted(3, 2)?, vec![0, 1]);
        assert!(admitted(3, 0)?.is_empty());
        assert_eq!(next_window(3, false, policy)?, 2);
        assert_eq!(next_window(1, false, policy)?, 1);
        assert_eq!(next_window(2, true, policy)?, 3);
        assert_eq!(next_window(3, true, policy)?, 3);
        assert!(window_admissible(&policy, 100));
        for invalid in [
            WindowPolicy { floor: 0, ..policy },
            WindowPolicy {
                floor: 4,
                ceiling: 3,
                ..policy
            },
            WindowPolicy {
                ceiling: MAX_SPECULATIVE_WINDOW + 1,
                ..policy
            },
            WindowPolicy {
                increase: 0,
                ..policy
            },
            WindowPolicy {
                decrease: 4,
                ..policy
            },
        ] {
            assert!(!window_admissible(&invalid, 100), "{invalid:?}");
        }
        assert!(!window_admissible(&policy, 2), "ceiling above max_entries");
        Ok(())
    }
}
