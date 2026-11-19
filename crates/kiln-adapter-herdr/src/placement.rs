//! Pure placement over the inventory and observed occupancy.
//!
//! Kiln decides which attempt runs next; this adapter decides where. A
//! machine is placeable only when its occupancy was observed this round, so
//! an unreachable machine never receives work on assumed capacity.

use std::collections::BTreeMap;

use kiln_core::ContentId;

use crate::client::{HerdrAgent, HerdrFailure};
use crate::inventory::{AgentKind, HerdrInventory};

/// Prefix of every agent name and worktree label this adapter creates.
pub const AGENT_NAME_PREFIX: &str = "kiln-";
/// Hex characters of the attempt identity used in names and paths.
pub const ATTEMPT_SHORT_HEX: usize = 12;
/// herdr's agent-name bound: `[a-z][a-z0-9_-]{0,31}`.
pub const MAX_AGENT_NAME_BYTES: usize = 32;

/// Kiln-created agents per admitted kind on one machine.
pub type KindCounts = BTreeMap<AgentKind, u32>;

/// Observed load of one machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MachineLoad {
    Available(KindCounts),
    Unavailable(HerdrFailure),
}

/// Observed load of every inventory machine.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Occupancy {
    pub machines: BTreeMap<String, MachineLoad>,
}

/// One chosen machine, kind and slot index (zero-based within the kind).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    pub machine: String,
    pub kind: AgentKind,
    pub slot: u32,
}

/// First [`ATTEMPT_SHORT_HEX`] lowercase hex characters of an attempt identity.
///
/// # Errors
/// Returns a diagnostic when the identity is not a BLAKE3 hex identity.
pub fn attempt_short(attempt: &ContentId) -> Result<String, String> {
    attempt
        .validate()
        .map_err(|_| "attempt identity is not a BLAKE3 identity".to_owned())?;
    attempt
        .as_str()
        .get(..ATTEMPT_SHORT_HEX)
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| "attempt identity is too short".to_owned())
}

/// Agent name `kiln-<attempt12>` for one attempt.
///
/// # Errors
/// Returns a diagnostic when the identity is invalid.
pub fn agent_name(attempt: &ContentId) -> Result<String, String> {
    let name = format!("{AGENT_NAME_PREFIX}{}", attempt_short(attempt)?);
    if valid_agent_name(&name) {
        Ok(name)
    } else {
        Err("derived agent name is invalid".to_owned())
    }
}

/// herdr's agent-name grammar `[a-z][a-z0-9_-]{0,31}`.
#[must_use]
pub fn valid_agent_name(value: &str) -> bool {
    value.len() <= MAX_AGENT_NAME_BYTES
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        })
}

/// Whether a live agent name is one this adapter creates.
#[must_use]
pub fn kiln_agent_name(value: &str) -> bool {
    valid_agent_name(value) && value.starts_with(AGENT_NAME_PREFIX)
}

/// Count Kiln-created agents of admitted kinds. Foreign agents, agents of
/// unadmitted kinds and names whose agent has exited are not counted.
#[must_use]
pub fn count_occupancy(agents: &[HerdrAgent]) -> KindCounts {
    let mut counts = KindCounts::new();
    for agent in agents {
        let ours = agent.name.as_deref().is_some_and(kiln_agent_name);
        if let (true, Some(kind)) = (ours, agent.admitted_kind()) {
            let count = counts.entry(kind).or_insert(0);
            *count = count.saturating_add(1);
        }
    }
    counts
}

// r[impl kiln.fleet.transport]
/// Choose a machine and slot deterministically.
///
/// Candidates are inventory machines whose occupancy is `Available` and that
/// have a free slot for at least one kind in `kind_preferences`. The least
/// loaded candidate (fewest Kiln agents of any kind) wins, ties broken by
/// label; on it, the earliest preferred kind with a free slot is used.
//
// Deviation from the design's `place(kind_preferences, occupancy)`: the
// inventory is passed explicitly because slot counts live there.
#[must_use]
pub fn place(
    inventory: &HerdrInventory,
    occupancy: &Occupancy,
    kind_preferences: &[AgentKind],
) -> Option<Placement> {
    inventory
        .machines
        .iter()
        .filter_map(|machine| {
            let Some(MachineLoad::Available(counts)) = occupancy.machines.get(&machine.label)
            else {
                return None;
            };
            let used = |kind: AgentKind| counts.get(&kind).copied().unwrap_or(0);
            let (kind, slot) = kind_preferences
                .iter()
                .map(|kind| (*kind, used(*kind)))
                .find(|(kind, used)| *used < machine.slots(*kind))?;
            let load = counts
                .values()
                .fold(0_u32, |sum, count| sum.saturating_add(*count));
            Some((load, machine.label.as_str(), kind, slot))
        })
        .min_by(|left, right| (left.0, left.1).cmp(&(right.0, right.1)))
        .map(|(_, machine, kind, slot)| Placement {
            machine: machine.to_owned(),
            kind,
            slot,
        })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::client::{AgentStatus, FailureClass};
    use crate::inventory::{AgentSlots, HerdrEnvironment, HerdrMachine, INVENTORY_SCHEMA};

    fn machine(label: &str, agents: &[(AgentKind, u32)]) -> HerdrMachine {
        HerdrMachine {
            label: label.to_owned(),
            herdr_machine: Some(label.to_owned()),
            ssh_target: Some(format!("brittonr@{label}")),
            worktree_root: "/home/kiln/worktrees".to_owned(),
            repositories: BTreeMap::new(),
            agents: agents
                .iter()
                .map(|(kind, slots)| AgentSlots {
                    kind: *kind,
                    slots: *slots,
                    args: Vec::new(),
                })
                .collect(),
        }
    }

    fn inventory(machines: Vec<HerdrMachine>) -> HerdrInventory {
        HerdrInventory {
            schema: INVENTORY_SCHEMA.to_owned(),
            executable: PathBuf::from("/bin/herdr"),
            session: "kiln".to_owned(),
            environment: HerdrEnvironment {
                home: "/home/kiln".to_owned(),
                path: "/bin".to_owned(),
                xdg_runtime_dir: None,
                xdg_config_home: None,
                ssh_auth_sock: None,
            },
            timeout_ms: 1_000,
            start_timeout_ms: 1_000,
            max_output_bytes: 4_096,
            machines,
        }
    }

    fn available(entries: &[(&str, &[(AgentKind, u32)])]) -> Occupancy {
        Occupancy {
            machines: entries
                .iter()
                .map(|(label, counts)| {
                    (
                        (*label).to_owned(),
                        MachineLoad::Available(counts.iter().copied().collect()),
                    )
                })
                .collect(),
        }
    }

    fn agent(name: Option<&str>, kind: Option<&str>) -> HerdrAgent {
        HerdrAgent {
            name: name.map(str::to_owned),
            kind: kind.map(str::to_owned),
            status: AgentStatus::Idle,
            pane_id: "w1:p1".to_owned(),
            workspace_id: "w1".to_owned(),
        }
    }

    #[test]
    // r[verify kiln.fleet.transport]
    fn least_loaded_machine_wins_and_ties_break_by_label() {
        let inventory = inventory(vec![
            machine("aspen3", &[(AgentKind::Omp, 2)]),
            machine("aspen1", &[(AgentKind::Omp, 2), (AgentKind::Codex, 1)]),
            machine("aspen2", &[(AgentKind::Omp, 2)]),
        ]);
        let tie = available(&[("aspen1", &[]), ("aspen2", &[]), ("aspen3", &[])]);
        assert_eq!(
            place(&inventory, &tie, &[AgentKind::Omp]),
            Some(Placement {
                machine: "aspen1".to_owned(),
                kind: AgentKind::Omp,
                slot: 0,
            })
        );
        let loaded = available(&[
            ("aspen1", &[(AgentKind::Codex, 1)]),
            ("aspen2", &[(AgentKind::Omp, 1)]),
            ("aspen3", &[]),
        ]);
        assert_eq!(
            place(&inventory, &loaded, &[AgentKind::Omp]).map(|placement| placement.machine),
            Some("aspen3".to_owned())
        );
        let busy = available(&[
            ("aspen1", &[(AgentKind::Omp, 1)]),
            ("aspen2", &[(AgentKind::Omp, 1)]),
            ("aspen3", &[(AgentKind::Omp, 2)]),
        ]);
        assert_eq!(
            place(&inventory, &busy, &[AgentKind::Omp]),
            Some(Placement {
                machine: "aspen1".to_owned(),
                kind: AgentKind::Omp,
                slot: 1,
            })
        );
    }

    #[test]
    // r[verify kiln.fleet.transport]
    fn preferences_choose_the_kind_on_the_chosen_machine() {
        let inventory = inventory(vec![machine(
            "aspen1",
            &[(AgentKind::Omp, 1), (AgentKind::Claude, 1)],
        )]);
        let full_omp = available(&[("aspen1", &[(AgentKind::Omp, 1)])]);
        assert_eq!(
            place(&inventory, &full_omp, &[AgentKind::Omp, AgentKind::Claude]),
            Some(Placement {
                machine: "aspen1".to_owned(),
                kind: AgentKind::Claude,
                slot: 0,
            })
        );
        let empty = available(&[("aspen1", &[])]);
        assert_eq!(
            place(&inventory, &empty, &[AgentKind::Claude, AgentKind::Omp])
                .map(|placement| placement.kind),
            Some(AgentKind::Claude)
        );
    }

    #[test]
    // r[verify kiln.fleet.transport]
    fn no_placement_without_free_observed_slots() {
        let inventory = inventory(vec![
            machine("aspen1", &[(AgentKind::Omp, 1)]),
            machine("aspen2", &[(AgentKind::Omp, 1)]),
        ]);
        let full = available(&[
            ("aspen1", &[(AgentKind::Omp, 1)]),
            ("aspen2", &[(AgentKind::Omp, 1)]),
        ]);
        assert_eq!(place(&inventory, &full, &[AgentKind::Omp]), None);

        let empty = available(&[("aspen1", &[]), ("aspen2", &[])]);
        assert_eq!(place(&inventory, &empty, &[AgentKind::Codex]), None);
        assert_eq!(place(&inventory, &empty, &[]), None);

        let mut unreachable = available(&[("aspen2", &[(AgentKind::Omp, 1)])]);
        unreachable.machines.insert(
            "aspen1".to_owned(),
            MachineLoad::Unavailable(HerdrFailure {
                class: FailureClass::Unavailable,
                code: Some("server_not_running".to_owned()),
                detail: "no herdr server".to_owned(),
            }),
        );
        assert_eq!(place(&inventory, &unreachable, &[AgentKind::Omp]), None);
        assert_eq!(
            place(&inventory, &Occupancy::default(), &[AgentKind::Omp]),
            None
        );
    }

    #[test]
    fn occupancy_counts_only_kiln_agents_of_admitted_kinds() {
        let counts = count_occupancy(&[
            agent(Some("kiln-0123456789ab"), Some("omp")),
            agent(Some("kiln-ba9876543210"), Some("omp")),
            agent(Some("kiln-aaaaaaaaaaaa"), Some("codex")),
            agent(Some("reviewer"), Some("omp")),
            agent(None, Some("claude")),
            agent(Some("kiln-bbbbbbbbbbbb"), Some("gemini")),
            agent(Some("kiln-cccccccccccc"), None),
        ]);
        assert_eq!(
            counts,
            KindCounts::from([(AgentKind::Omp, 2), (AgentKind::Codex, 1)])
        );
    }

    #[test]
    fn agent_names_follow_herdr_grammar() {
        let attempt = ContentId::from_blake3_hex("0123456789ABCDEF".repeat(4))
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(agent_name(&attempt).as_deref(), Ok("kiln-0123456789ab"));
        assert!(valid_agent_name("kiln-0123456789ab"));
        assert!(valid_agent_name("a"));
        assert!(!valid_agent_name(""));
        assert!(!valid_agent_name("Kiln-x"));
        assert!(!valid_agent_name("1kiln"));
        assert!(!valid_agent_name("kiln x"));
        assert!(!valid_agent_name(&"k".repeat(MAX_AGENT_NAME_BYTES + 1)));
        assert!(kiln_agent_name("kiln-x"));
        assert!(!kiln_agent_name("reviewer"));
    }
}
