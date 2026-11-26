//! Bounded client for the herdr CLI.
//!
//! Every operation is one `bounded-exec` child: the pinned executable, an
//! exact argv starting with `--session <local session>` or `--machine
//! <saved machine>`, a cleared environment plus the inventory's explicit
//! `HOME`/`PATH`/`XDG_*`, a deadline and an output bound. Only the JSON shapes
//! this module names are parsed. Failures are `unavailable` (nothing reached
//! herdr), `rejected` (herdr or the adapter refused; nothing changed) or
//! `unknown` (a mutation may have applied). An unknown mutation must be
//! reconciled by listing before any retry; this client never retries.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use bounded_exec::{
    CommandSpec, Completion, Disposition, EnvironmentMode, ExecutionLimits, ExecutionOutput, Input,
    Operation, OutcomePolicy, RunError, RunRequest, TerminationScope, run, run_with_cancellation,
};
use kiln_core::ContentId;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::inventory::{
    AgentKind, HerdrInventory, HerdrMachine, MAX_PATH_BYTES, absolute_path, token,
};
use crate::placement::{
    AGENT_NAME_PREFIX, MachineLoad, Occupancy, attempt_short, count_occupancy, kiln_agent_name,
};

pub const MAX_DETAIL_BYTES: usize = 192;
pub const MAX_LISTED_AGENTS: usize = 256;
pub const MAX_LISTED_PANES: usize = 256;
pub const MAX_LISTED_WORKTREES: usize = 512;
pub const MAX_PROMPT_BYTES: usize = 32 * 1_024;
pub const MAX_READ_LINES: u32 = 1_000;
pub const MAX_HANDLE_BYTES: usize = 64;
pub const MAX_BRANCH_BYTES: usize = 200;
pub const POLL_INTERVAL_MS: u64 = 20;
pub const TEARDOWN_TIMEOUT_MS: u64 = 2_000;
/// No stdin is sent (`Input::Null`), but `bounded-exec` requires a nonzero bound.
pub const STDIN_MAX_BYTES: usize = 1;
/// herdr's exit status for CLI usage and local selection errors.
pub const CLI_USAGE_EXIT: i32 = 2;
/// Read source for transcripts: recent output with soft wraps joined.
pub const READ_SOURCE: &str = "recent-unwrapped";
/// Logical key that interrupts an interactive agent turn.
pub const INTERRUPT_KEY: &str = "ctrl+c";
/// Git object names Kiln accepts as an exact base: SHA-1 or SHA-256 hex.
pub const COMMIT_HEX_LENGTHS: [usize; 2] = [40, 64];

/// herdr error codes this client interprets.
pub mod codes {
    pub const AGENT_NOT_FOUND: &str = "agent_not_found";
    pub const PANE_NOT_FOUND: &str = "pane_not_found";
    pub const AGENT_NAME_TAKEN: &str = "agent_name_taken";
    pub const AGENT_NOT_READY: &str = "agent_not_ready";
    pub const AGENT_PROMPT_STALLED: &str = "agent_prompt_stalled";
    pub const SERVER_NOT_RUNNING: &str = "server_not_running";
    pub const TIMEOUT: &str = "timeout";
}

/// Agent lifecycle state as herdr reports it; mirrors the fleet core enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Idle,
    Working,
    Blocked,
    Done,
    Unknown,
}

/// Result of [`HerdrClient::status`]. A missing or exited agent is `Gone`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentObservation {
    Status(AgentStatus),
    Gone,
}

/// Failure class shared by every operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FailureClass {
    Unavailable,
    Rejected,
    Unknown,
}

impl FailureClass {
    /// Stable label for journals and operator output.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Rejected => "rejected",
            Self::Unknown => "unknown",
        }
    }
}

/// One classified failure. `code` is herdr's error code when it sent one;
/// `detail` is bounded and control-free.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HerdrFailure {
    pub class: FailureClass,
    pub code: Option<String>,
    pub detail: String,
}

impl HerdrFailure {
    fn new(class: FailureClass, detail: impl Into<String>) -> Self {
        Self {
            class,
            code: None,
            detail: detail.into(),
        }
    }

    fn rejected(detail: impl Into<String>) -> Self {
        Self::new(FailureClass::Rejected, detail)
    }

    /// Whether herdr reported this code.
    #[must_use]
    pub fn is_code(&self, code: &str) -> bool {
        self.code.as_deref() == Some(code)
    }
}

impl fmt::Display for HerdrFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.code {
            Some(code) => write!(
                formatter,
                "{} ({code}): {}",
                self.class.as_str(),
                self.detail
            ),
            None => write!(formatter, "{}: {}", self.class.as_str(), self.detail),
        }
    }
}

/// One live agent as herdr lists it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HerdrAgent {
    pub name: Option<String>,
    /// herdr's detected agent label; `None` once the agent process exited.
    pub kind: Option<String>,
    pub status: AgentStatus,
    pub pane_id: String,
    pub workspace_id: String,
}

impl HerdrAgent {
    /// The detected kind when Kiln admits it.
    #[must_use]
    pub fn admitted_kind(&self) -> Option<AgentKind> {
        self.kind.as_deref().and_then(AgentKind::parse)
    }
}

/// Result of [`HerdrClient::start_agent`]. `replayed` means the name already
/// named the same kind in the same pane, so no new agent was started.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartedAgent {
    pub agent: HerdrAgent,
    pub replayed: bool,
}

/// A worktree workspace created by [`HerdrClient::create_worktree`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatedWorktree {
    pub workspace_id: String,
    pub pane_id: String,
    pub path: String,
    pub branch: String,
}

/// One Git worktree of a repository checkout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeEntry {
    pub path: String,
    pub branch: Option<String>,
    pub open_workspace_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Effect {
    Read,
    Mutation,
}

impl Effect {
    /// Class for an outcome that cannot be known: a mutation may have
    /// applied; a read simply did not answer.
    const fn uncertain(self) -> FailureClass {
        match self {
            Self::Read => FailureClass::Unavailable,
            Self::Mutation => FailureClass::Unknown,
        }
    }

    /// Class for a response that could not be understood.
    const fn malformed(self) -> FailureClass {
        match self {
            Self::Read => FailureClass::Rejected,
            Self::Mutation => FailureClass::Unknown,
        }
    }
}

#[derive(Deserialize)]
struct Envelope {
    result: serde_json::Value,
}

#[derive(Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Deserialize)]
struct ErrorBody {
    code: String,
    message: String,
}

#[derive(Deserialize)]
struct WireAgent {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    agent: Option<String>,
    agent_status: AgentStatus,
    pane_id: String,
    workspace_id: String,
}

impl From<WireAgent> for HerdrAgent {
    fn from(wire: WireAgent) -> Self {
        Self {
            name: wire.name,
            kind: wire.agent,
            status: wire.agent_status,
            pane_id: wire.pane_id,
            workspace_id: wire.workspace_id,
        }
    }
}

#[derive(Deserialize)]
struct AgentList {
    agents: Vec<WireAgent>,
}

#[derive(Deserialize)]
struct AgentResult {
    agent: WireAgent,
}

#[derive(Deserialize)]
struct Empty {}

#[derive(Deserialize)]
struct WireWorkspace {
    workspace_id: String,
}

#[derive(Deserialize)]
struct WirePane {
    pane_id: String,
}

/// One native terminal pane's public identity; no terminal contents.
#[derive(Debug, Deserialize, Serialize)]
pub struct HerdrPane {
    pub pane_id: String,
    pub workspace_id: String,
    pub tab_id: String,
}

#[derive(Deserialize)]
struct PaneList {
    panes: Vec<HerdrPane>,
}

#[derive(Deserialize)]
struct WireWorktree {
    path: String,
    #[serde(default)]
    branch: Option<String>,
    #[serde(default)]
    open_workspace_id: Option<String>,
}

#[derive(Deserialize)]
struct WorktreeCreated {
    workspace: WireWorkspace,
    root_pane: WirePane,
    worktree: WireWorktree,
}

#[derive(Deserialize)]
struct WorktreeList {
    worktrees: Vec<WireWorktree>,
}

/// An admitted herdr client.
// r[impl kiln.fleet.transport]
#[derive(Debug, Clone)]
pub struct HerdrClient {
    inventory: HerdrInventory,
    cancellation: Option<Arc<AtomicBool>>,
}

impl HerdrClient {
    /// Admit one inventory.
    ///
    /// # Errors
    /// Returns the inventory diagnostic when validation fails.
    pub fn new(inventory: HerdrInventory) -> Result<Self, String> {
        inventory.validate()?;
        Ok(Self {
            inventory,
            cancellation: None,
        })
    }

    /// Stop the running child, with owned-process teardown, when `flag` is
    /// raised. A cancelled mutation is `unknown`.
    #[must_use]
    pub fn with_cancellation(mut self, flag: Arc<AtomicBool>) -> Self {
        self.cancellation = Some(flag);
        self
    }

    /// Return the admitted inventory.
    #[must_use]
    pub const fn inventory(&self) -> &HerdrInventory {
        &self.inventory
    }

    /// Check that the machine's herdr server answers API requests.
    ///
    /// # Errors
    /// Returns the classified failure of `agent list`.
    pub fn probe(&self, machine: &str) -> Result<(), HerdrFailure> {
        self.list_agents(machine).map(|_| ())
    }

    /// List every live agent on one machine, Kiln's and others'.
    ///
    /// # Errors
    /// Returns a classified failure; an unavailable machine is never an empty
    /// list.
    pub fn list_agents(&self, machine: &str) -> Result<Vec<HerdrAgent>, HerdrFailure> {
        let machine = self.machine(machine)?;
        let stdout = self.execute(machine, &["agent", "list"], Effect::Read, self.timeout())?;
        let list: AgentList = decode(&stdout, "agent_list", Effect::Read)?;
        if list.agents.len() > MAX_LISTED_AGENTS {
            return Err(HerdrFailure::rejected("herdr agent list exceeds its bound"));
        }
        Ok(list.agents.into_iter().map(HerdrAgent::from).collect())
    }

    /// List terminal panes, including panes with no agent or attempt.
    ///
    /// # Errors
    /// Refuses unknown targets and malformed, oversize or unavailable inventories.
    pub fn list_panes(&self, machine: &str) -> Result<Vec<HerdrPane>, HerdrFailure> {
        self.panes(self.machine(machine)?)
    }

    fn panes(&self, machine: &HerdrMachine) -> Result<Vec<HerdrPane>, HerdrFailure> {
        let stdout = self.execute(machine, &["pane", "list"], Effect::Read, self.timeout())?;
        let list: PaneList = decode(&stdout, "pane_list", Effect::Read)?;
        if list.panes.len() > MAX_LISTED_PANES {
            return Err(HerdrFailure::rejected("herdr pane list exceeds its bound"));
        }
        let mut seen = BTreeSet::new();
        if list.panes.iter().any(|pane| {
            !handle(&pane.workspace_id)
                || !handle(&pane.tab_id)
                || !pane_handle(&pane.pane_id)
                || !seen.insert(pane.pane_id.as_str())
        }) {
            return Err(HerdrFailure::rejected(
                "herdr pane list has invalid or duplicate identities",
            ));
        }
        Ok(list.panes)
    }

    /// Re-observe the selected host and read only a pane listed there by its
    /// exact public ID. The CLI prints plain text, not the socket read envelope.
    ///
    /// # Errors
    /// Refuses an invalid ID or bound, or any pane not in the fresh inventory.
    /// Provider failures, including disappearance after listing, propagate.
    pub fn read_observed_pane(
        &self,
        machine: &str,
        pane_id: &str,
        lines: u32,
    ) -> Result<(HerdrPane, String), HerdrFailure> {
        let machine = self.machine(machine)?;
        if !pane_handle(pane_id) || !(1..=MAX_READ_LINES).contains(&lines) {
            return Err(HerdrFailure::rejected("pane id or line bound is invalid"));
        }
        let pane = self
            .panes(machine)?
            .into_iter()
            .find(|pane| pane.pane_id == pane_id)
            .ok_or_else(|| HerdrFailure {
                class: FailureClass::Rejected,
                code: Some(codes::PANE_NOT_FOUND.to_owned()),
                detail: "pane is not in this host's current inventory".to_owned(),
            })?;
        let lines = lines.to_string();
        let args = [
            "pane",
            "read",
            &pane.pane_id,
            "--source",
            READ_SOURCE,
            "--lines",
            &lines,
            "--format",
            "text",
        ];
        let stdout = self.execute(machine, &args, Effect::Read, self.timeout())?;
        let capture = String::from_utf8(stdout)
            .map_err(|_| HerdrFailure::rejected("herdr pane output is not UTF-8"))?;
        Ok((pane, capture))
    }

    /// Kiln agents per admitted kind on every inventory machine. A machine
    /// that cannot be listed is recorded as unavailable, not as empty.
    #[must_use]
    pub fn occupancy(&self) -> Occupancy {
        self.occupancy_with_session_counts(|_, _| {})
    }

    /// As [`Self::occupancy`], also reporting the total entries in each list
    /// (including non-Kiln agents), or `None` if the bounded read failed.
    #[must_use]
    pub fn occupancy_with_session_counts(
        &self,
        mut observe: impl FnMut(&str, Option<usize>),
    ) -> Occupancy {
        let machines = self
            .inventory
            .machines
            .iter()
            .map(|machine| {
                let listed = self.list_agents(&machine.label);
                observe(&machine.label, listed.as_ref().ok().map(Vec::len));
                let load = match listed {
                    Ok(agents) => MachineLoad::Available(count_occupancy(&agents)),
                    Err(failure) => MachineLoad::Unavailable(failure),
                };
                (machine.label.clone(), load)
            })
            .collect();
        Occupancy { machines }
    }

    // Deviation from the design's `create_worktree(machine, repository,
    // branch, base)`: the attempt is passed to derive the worktree path
    // `<worktree_root>/<attempt12>` and the label `kiln-<attempt12>`.
    /// Create and open a fresh Git worktree workspace at the exact `base`
    /// commit on a new `branch`.
    ///
    /// # Errors
    /// Refuses unknown repositories, invalid branches and non-commit bases
    /// before running herdr; otherwise returns a classified failure. A
    /// response that names another path or branch is `unknown`.
    pub fn create_worktree(
        &self,
        machine: &str,
        repository: &str,
        branch: &str,
        base: &str,
        attempt: &ContentId,
    ) -> Result<CreatedWorktree, HerdrFailure> {
        let machine = self.machine(machine)?;
        let checkout = machine.checkout(repository).ok_or_else(|| {
            HerdrFailure::rejected(format!(
                "repository {} is not configured on machine {}",
                bounded(repository),
                machine.label
            ))
        })?;
        if !branch_name(branch) {
            return Err(HerdrFailure::rejected("worktree branch is invalid"));
        }
        if !commit_hex(base) {
            return Err(HerdrFailure::rejected(
                "worktree base must be a full commit id",
            ));
        }
        let short = attempt_short(attempt).map_err(HerdrFailure::rejected)?;
        let path = format!("{}/{short}", machine.worktree_root.trim_end_matches('/'));
        if !absolute_path(&path) || path.len() > MAX_PATH_BYTES {
            return Err(HerdrFailure::rejected("worktree path is invalid"));
        }
        let label = format!("{AGENT_NAME_PREFIX}{short}");
        let args = [
            "worktree",
            "create",
            "--cwd",
            checkout,
            "--branch",
            branch,
            "--base",
            base,
            "--path",
            &path,
            "--label",
            &label,
            "--no-focus",
        ];
        let stdout = self.execute(machine, &args, Effect::Mutation, self.timeout())?;
        let created: WorktreeCreated = decode(&stdout, "worktree_created", Effect::Mutation)?;
        if created.worktree.path != path || created.worktree.branch.as_deref() != Some(branch) {
            return Err(HerdrFailure::new(
                FailureClass::Unknown,
                "herdr created a worktree other than the one requested",
            ));
        }
        if !handle(&created.workspace.workspace_id) || !handle(&created.root_pane.pane_id) {
            return Err(HerdrFailure::new(
                FailureClass::Unknown,
                "herdr worktree response carries invalid handles",
            ));
        }
        Ok(CreatedWorktree {
            workspace_id: created.workspace.workspace_id,
            pane_id: created.root_pane.pane_id,
            path,
            branch: branch.to_owned(),
        })
    }

    /// List the Git worktrees of one configured checkout; used to reconcile
    /// an unknown [`Self::create_worktree`].
    ///
    /// # Errors
    /// Returns a classified failure.
    pub fn list_worktrees(
        &self,
        machine: &str,
        repository: &str,
    ) -> Result<Vec<WorktreeEntry>, HerdrFailure> {
        let machine = self.machine(machine)?;
        let checkout = machine.checkout(repository).ok_or_else(|| {
            HerdrFailure::rejected(format!(
                "repository {} is not configured on machine {}",
                bounded(repository),
                machine.label
            ))
        })?;
        let args = ["worktree", "list", "--cwd", checkout];
        let stdout = self.execute(machine, &args, Effect::Read, self.timeout())?;
        let list: WorktreeList = decode(&stdout, "worktree_list", Effect::Read)?;
        if list.worktrees.len() > MAX_LISTED_WORKTREES {
            return Err(HerdrFailure::rejected(
                "herdr worktree list exceeds its bound",
            ));
        }
        Ok(list
            .worktrees
            .into_iter()
            .map(|worktree| WorktreeEntry {
                path: worktree.path,
                branch: worktree.branch,
                open_workspace_id: worktree.open_workspace_id,
            })
            .collect())
    }

    /// Start one configured agent kind in an available shell pane.
    ///
    /// A replayed start, where `name` already names the same kind in the same
    /// pane, is reconciled by reading that agent and returns `replayed`.
    ///
    /// # Errors
    /// Refuses kinds not configured on the machine and names outside
    /// `kiln-` plus herdr's grammar before running herdr. `agent_not_ready`
    /// and deadlines are `unknown`: the agent may be running.
    pub fn start_agent(
        &self,
        machine: &str,
        pane: &str,
        name: &str,
        kind: AgentKind,
    ) -> Result<StartedAgent, HerdrFailure> {
        let machine = self.machine(machine)?;
        let configured = machine.agent(kind).ok_or_else(|| {
            HerdrFailure::rejected(format!(
                "agent kind {} is not configured on machine {}",
                kind.as_str(),
                machine.label
            ))
        })?;
        if !kiln_agent_name(name) {
            return Err(HerdrFailure::rejected("agent name is invalid"));
        }
        if !handle(pane) {
            return Err(HerdrFailure::rejected("pane id is invalid"));
        }
        let start_timeout = self.inventory.start_timeout_ms.to_string();
        let mut args = vec![
            "agent",
            "start",
            name,
            "--kind",
            kind.as_str(),
            "--pane",
            pane,
            "--timeout",
            &start_timeout,
        ];
        if !configured.args.is_empty() {
            args.push("--");
            args.extend(configured.args.iter().map(String::as_str));
        }
        let deadline = self
            .inventory
            .start_timeout_ms
            .saturating_add(self.inventory.timeout_ms);
        match self.execute(machine, &args, Effect::Mutation, deadline) {
            Ok(stdout) => {
                let started: AgentResult = decode(&stdout, "agent_started", Effect::Mutation)?;
                let agent = HerdrAgent::from(started.agent);
                if agent.name.as_deref() != Some(name) || agent.pane_id != pane {
                    return Err(HerdrFailure::new(
                        FailureClass::Unknown,
                        "herdr started an agent other than the one requested",
                    ));
                }
                Ok(StartedAgent {
                    agent,
                    replayed: false,
                })
            }
            Err(failure) if failure.is_code(codes::AGENT_NAME_TAKEN) => {
                self.reconcile_start(machine, pane, name, kind, failure)
            }
            Err(failure) => Err(failure),
        }
    }

    fn reconcile_start(
        &self,
        machine: &HerdrMachine,
        pane: &str,
        name: &str,
        kind: AgentKind,
        taken: HerdrFailure,
    ) -> Result<StartedAgent, HerdrFailure> {
        let agent = self.agent_get(machine, name).map_err(|_| HerdrFailure {
            class: FailureClass::Unknown,
            code: taken.code.clone(),
            detail: "agent name is taken and its holder could not be read".to_owned(),
        })?;
        if agent.pane_id == pane && agent.admitted_kind() == Some(kind) {
            Ok(StartedAgent {
                agent,
                replayed: true,
            })
        } else {
            Err(HerdrFailure {
                class: FailureClass::Rejected,
                code: taken.code,
                detail: "agent name is held by another pane or kind".to_owned(),
            })
        }
    }

    /// Submit one prompt. Submission does not prove the agent began a turn.
    ///
    /// # Errors
    /// `agent_blocked` and a stale name are `rejected`; a deadline is
    /// `unknown` because the text may have been delivered.
    pub fn prompt(&self, machine: &str, agent: &str, text: &str) -> Result<(), HerdrFailure> {
        let machine = self.machine(machine)?;
        agent_target(agent)?;
        if text.is_empty()
            || text.len() > MAX_PROMPT_BYTES
            || text
                .chars()
                .any(|character| character.is_control() && character != '\n' && character != '\t')
        {
            return Err(HerdrFailure::rejected(
                "prompt text is empty, too long or has controls",
            ));
        }
        let args = ["agent", "prompt", agent, text];
        let stdout = self.execute(machine, &args, Effect::Mutation, self.timeout())?;
        let prompted: AgentResult = decode(&stdout, "agent_prompted", Effect::Mutation)?;
        if prompted.agent.name.as_deref() != Some(agent) {
            return Err(HerdrFailure::new(
                FailureClass::Unknown,
                "herdr prompted an agent other than the one requested",
            ));
        }
        Ok(())
    }

    /// Observe one agent. A missing name, or a name whose agent exited, is
    /// `Gone`; herdr's `unknown` status stays `Unknown`.
    ///
    /// # Errors
    /// Returns a classified failure; an unreachable machine is never `Gone`.
    pub fn status(&self, machine: &str, agent: &str) -> Result<AgentObservation, HerdrFailure> {
        let machine = self.machine(machine)?;
        agent_target(agent)?;
        match self.agent_get(machine, agent) {
            Ok(found) if found.kind.is_none() => Ok(AgentObservation::Gone),
            Ok(found) => Ok(AgentObservation::Status(found.status)),
            Err(failure) if failure.is_code(codes::AGENT_NOT_FOUND) => Ok(AgentObservation::Gone),
            Err(failure) => Err(failure),
        }
    }

    /// Read up to `lines` recent transcript lines. The text is untrusted
    /// agent output.
    ///
    /// # Errors
    /// Returns a classified failure; `lines` must be within
    /// `1..=MAX_READ_LINES`.
    pub fn read(&self, machine: &str, agent: &str, lines: u32) -> Result<String, HerdrFailure> {
        let machine = self.machine(machine)?;
        agent_target(agent)?;
        if !(1..=MAX_READ_LINES).contains(&lines) {
            return Err(HerdrFailure::rejected("read line count is out of range"));
        }
        let lines = lines.to_string();
        let args = [
            "agent",
            "read",
            agent,
            "--source",
            READ_SOURCE,
            "--lines",
            &lines,
        ];
        let stdout = self.execute(machine, &args, Effect::Read, self.timeout())?;
        Ok(String::from_utf8_lossy(&stdout).into_owned())
    }

    /// Send `ctrl+c` to one agent.
    ///
    /// # Errors
    /// Returns a classified failure; a stale name is `rejected` with code
    /// `agent_not_found`.
    pub fn interrupt(&self, machine: &str, agent: &str) -> Result<(), HerdrFailure> {
        let machine = self.machine(machine)?;
        agent_target(agent)?;
        let args = ["agent", "send-keys", agent, INTERRUPT_KEY];
        let stdout = self.execute(machine, &args, Effect::Mutation, self.timeout())?;
        decode::<Empty>(&stdout, "ok", Effect::Mutation).map(|_| ())
    }

    /// Close one workspace and its panes.
    ///
    /// # Errors
    /// Returns a classified failure; a closed workspace is `rejected` with
    /// code `workspace_not_found`.
    pub fn close_workspace(&self, machine: &str, workspace: &str) -> Result<(), HerdrFailure> {
        let machine = self.machine(machine)?;
        if !handle(workspace) {
            return Err(HerdrFailure::rejected("workspace id is invalid"));
        }
        let args = ["workspace", "close", workspace];
        let stdout = self.execute(machine, &args, Effect::Mutation, self.timeout())?;
        decode::<Empty>(&stdout, "ok", Effect::Mutation).map(|_| ())
    }

    fn agent_get(&self, machine: &HerdrMachine, agent: &str) -> Result<HerdrAgent, HerdrFailure> {
        let stdout = self.execute(
            machine,
            &["agent", "get", agent],
            Effect::Read,
            self.timeout(),
        )?;
        let info: AgentResult = decode(&stdout, "agent_info", Effect::Read)?;
        let found = HerdrAgent::from(info.agent);
        if found.name.as_deref() != Some(agent) {
            return Err(HerdrFailure::rejected(
                "herdr returned an agent other than the one requested",
            ));
        }
        Ok(found)
    }

    fn machine(&self, label: &str) -> Result<&HerdrMachine, HerdrFailure> {
        self.inventory.machine(label).ok_or_else(|| {
            HerdrFailure::rejected(format!(
                "machine {} is not in the fleet inventory",
                bounded(label)
            ))
        })
    }

    const fn timeout(&self) -> u64 {
        self.inventory.timeout_ms
    }

    fn execute(
        &self,
        machine: &HerdrMachine,
        args: &[&str],
        effect: Effect,
        timeout_ms: u64,
    ) -> Result<Vec<u8>, HerdrFailure> {
        let request = self.request(machine, args, timeout_ms)?;
        let result = match &self.cancellation {
            Some(flag) => run_with_cancellation(request, flag),
            None => run(request),
        };
        let remote = machine.herdr_machine.is_some();
        match result {
            Ok(output) if output.disposition == Disposition::Succeeded => Ok(output.stdout.bytes),
            Ok(output) => Err(classify_output(&output, effect, remote)),
            Err(error) if spawn_failed(&error) => Err(HerdrFailure::new(
                FailureClass::Unavailable,
                "herdr executable could not be started",
            )),
            Err(_) => Err(HerdrFailure::new(
                effect.uncertain(),
                "herdr execution failed",
            )),
        }
    }

    fn request(
        &self,
        machine: &HerdrMachine,
        args: &[&str],
        timeout_ms: u64,
    ) -> Result<RunRequest, HerdrFailure> {
        let mut full = match &machine.herdr_machine {
            Some(saved) => vec![OsString::from("--machine"), OsString::from(saved)],
            None => vec![
                OsString::from("--session"),
                OsString::from(&self.inventory.session),
            ],
        };
        full.extend(args.iter().map(OsString::from));
        let policy = OutcomePolicy::new(vec![0], true, false).map_err(|_| {
            HerdrFailure::new(
                FailureClass::Unavailable,
                "herdr execution policy is invalid",
            )
        })?;
        Ok(RunRequest {
            command: CommandSpec {
                program: self.inventory.executable.clone(),
                args: full,
                current_dir: PathBuf::from(&self.inventory.environment.home),
                environment_mode: EnvironmentMode::Clear,
                environment: self
                    .inventory
                    .environment
                    .entries()
                    .into_iter()
                    .map(|(name, value)| (OsString::from(name), OsString::from(value)))
                    .collect(),
                input: Input::Null,
            },
            limits: ExecutionLimits {
                timeout_ms,
                stdin_max_bytes: STDIN_MAX_BYTES,
                stdout_max_bytes: self.inventory.max_output_bytes,
                stderr_max_bytes: self.inventory.max_output_bytes,
                poll_interval_ms: POLL_INTERVAL_MS,
                teardown_timeout_ms: TEARDOWN_TIMEOUT_MS,
            },
            termination_scope: TerminationScope::ProcessGroup,
            outcome_policy: policy,
        })
    }
}

fn decode<T: DeserializeOwned>(
    stdout: &[u8],
    expected: &str,
    effect: Effect,
) -> Result<T, HerdrFailure> {
    let malformed = |what: &str| {
        HerdrFailure::new(
            effect.malformed(),
            format!("herdr response is malformed: {what}"),
        )
    };
    let envelope: Envelope =
        serde_json::from_slice(stdout).map_err(|_| malformed("not a result envelope"))?;
    let kind = envelope
        .result
        .get("type")
        .and_then(serde_json::Value::as_str);
    if kind != Some(expected) {
        return Err(malformed("unexpected result type"));
    }
    serde_json::from_value(envelope.result).map_err(|_| malformed("unexpected result shape"))
}

fn classify_output(output: &ExecutionOutput, effect: Effect, remote: bool) -> HerdrFailure {
    match (output.completion, output.disposition) {
        (Completion::TimedOut, _) | (_, Disposition::TimedOut) => {
            HerdrFailure::new(effect.uncertain(), "herdr command exceeded its deadline")
        }
        (Completion::Cancelled, _) | (_, Disposition::Cancelled) => {
            HerdrFailure::new(effect.uncertain(), "herdr command was cancelled")
        }
        (_, Disposition::OutputLimitExceeded(_)) => {
            HerdrFailure::new(effect.malformed(), "herdr response exceeds its bound")
        }
        _ => classify_exit(output, effect, remote),
    }
}

/// Classify a nonzero exit from herdr's stderr JSON error, falling back to
/// its exit status.
fn classify_exit(output: &ExecutionOutput, effect: Effect, remote: bool) -> HerdrFailure {
    let parsed = serde_json::from_slice::<ErrorEnvelope>(&output.stderr.bytes)
        .or_else(|_| serde_json::from_slice::<ErrorEnvelope>(&output.stdout.bytes));
    if let Ok(ErrorEnvelope { error }) = parsed {
        return HerdrFailure {
            class: class_for_code(&error.code, effect, remote),
            detail: detail(error.message.as_bytes())
                .unwrap_or_else(|| "herdr refused the request".to_owned()),
            code: Some(bounded(&error.code)),
        };
    }
    let text = detail(&output.stderr.bytes);
    if output.exit_code == Some(CLI_USAGE_EXIT) {
        let unknown_machine = text
            .as_deref()
            .is_some_and(|line| line.contains("unknown machine"));
        let class = if unknown_machine {
            FailureClass::Unavailable
        } else {
            FailureClass::Rejected
        };
        return HerdrFailure::new(
            class,
            text.unwrap_or_else(|| "herdr refused the command line".to_owned()),
        );
    }
    HerdrFailure::new(
        effect.uncertain(),
        text.unwrap_or_else(|| "herdr failed without a diagnostic".to_owned()),
    )
}

fn class_for_code(code: &str, effect: Effect, remote: bool) -> FailureClass {
    match code {
        codes::SERVER_NOT_RUNNING => FailureClass::Unavailable,
        codes::TIMEOUT | codes::AGENT_PROMPT_STALLED | codes::AGENT_NOT_READY => effect.uncertain(),
        connection
            if ["unavailable", "unreachable", "connect"]
                .iter()
                .any(|marker| connection.contains(marker)) =>
        {
            // A lost connection to a remote machine does not prove a
            // forwarded mutation was not applied.
            if remote {
                effect.uncertain()
            } else {
                FailureClass::Unavailable
            }
        }
        _ => FailureClass::Rejected,
    }
}

fn spawn_failed(error: &RunError) -> bool {
    matches!(
        error,
        RunError::EmptyProgram
            | RunError::ProgramNotAbsolute
            | RunError::EmptyCurrentDirectory
            | RunError::CurrentDirectoryNotAbsolute
            | RunError::InvalidLimits(_)
            | RunError::DuplicateEnvironmentName
            | RunError::InputLimitExceeded
            | RunError::Io {
                operation: Operation::Spawn,
                ..
            }
    )
}

fn agent_target(agent: &str) -> Result<(), HerdrFailure> {
    if kiln_agent_name(agent) {
        Ok(())
    } else {
        Err(HerdrFailure::rejected("agent name is invalid"))
    }
}

/// herdr workspace, tab and pane handles: `w1`, `w1:t1`, `w1:p1`.
fn handle(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_HANDLE_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b":_-".contains(&byte))
        && !value.starts_with('-')
}

/// Path-safe native handle syntax; inventory membership, not syntax, admits a read.
#[must_use]
pub fn pane_handle(value: &str) -> bool {
    handle(value)
        && value.split_once(":p").is_some_and(|(workspace, pane)| {
            !workspace.is_empty()
                && !workspace.contains(':')
                && !pane.is_empty()
                && pane.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

fn branch_name(value: &str) -> bool {
    token(value, MAX_BRANCH_BYTES)
        && !value.starts_with(['-', '/', '.'])
        && !value.ends_with(['/', '.'])
        && !value.ends_with(".lock")
        && !value.contains("..")
        && !value.contains("//")
        && !value.contains("@{")
        && !value.contains(['~', '^', ':', '?', '*', '[', '\\'])
}

fn commit_hex(value: &str) -> bool {
    COMMIT_HEX_LENGTHS.contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn bounded(value: &str) -> String {
    detail(value.as_bytes()).unwrap_or_default()
}

/// Keep only a bounded, control-free first line of diagnostics.
fn detail(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let line = text.lines().next()?;
    let cleaned: String = line
        .chars()
        .take(MAX_DETAIL_BYTES)
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_classify_by_effect_and_transport() {
        assert_eq!(
            class_for_code("server_not_running", Effect::Mutation, true),
            FailureClass::Unavailable
        );
        assert_eq!(
            class_for_code("timeout", Effect::Mutation, false),
            FailureClass::Unknown
        );
        assert_eq!(
            class_for_code("timeout", Effect::Read, false),
            FailureClass::Unavailable
        );
        assert_eq!(
            class_for_code("machine_unavailable", Effect::Mutation, true),
            FailureClass::Unknown
        );
        assert_eq!(
            class_for_code("machine_unavailable", Effect::Mutation, false),
            FailureClass::Unavailable
        );
        assert_eq!(
            class_for_code("agent_pane_busy", Effect::Mutation, true),
            FailureClass::Rejected
        );
    }

    #[test]
    fn branches_and_bases_are_exact() {
        assert!(branch_name("kiln/impl-0123456789ab-ba9876543210"));
        assert!(!branch_name("-delete"));
        assert!(!branch_name("kiln/../main"));
        assert!(!branch_name("kiln/x.lock"));
        assert!(!branch_name("kiln x"));
        assert!(!branch_name("kiln/@{1}"));
        assert!(commit_hex(&"a".repeat(40)));
        assert!(commit_hex(&"0".repeat(64)));
        assert!(!commit_hex("main"));
        assert!(!commit_hex(&"A".repeat(40)));
    }

    #[test]
    fn handles_and_details_are_bounded() {
        assert!(handle("w12:p3"));
        assert!(!handle("w1 p1"));
        assert!(!handle("--all"));
        assert_eq!(detail(b"first\nsecond").as_deref(), Some("first"));
        assert_eq!(detail(&[0x01, b'a']).as_deref(), Some("a"));
        assert_eq!(
            detail("x".repeat(1_000).as_bytes()).map(|value| value.len()),
            Some(MAX_DETAIL_BYTES)
        );
        assert_eq!(detail(b"   "), None);
    }

    #[test]
    fn decode_checks_result_type_and_shape() {
        let ok = br#"{"id":"cli:pane:close","result":{"type":"ok"}}"#;
        assert!(decode::<Empty>(ok, "ok", Effect::Mutation).is_ok());
        let wrong = decode::<Empty>(ok, "agent_list", Effect::Read)
            .err()
            .map(|failure| failure.class);
        assert_eq!(wrong, Some(FailureClass::Rejected));
        let garbage = decode::<Empty>(b"not json", "ok", Effect::Mutation)
            .err()
            .map(|failure| failure.class);
        assert_eq!(garbage, Some(FailureClass::Unknown));
        let status = br#"{"id":"x","result":{"type":"agent_list","agents":[{"agent_status":"sleeping","pane_id":"w1:p1","workspace_id":"w1"}]}}"#;
        assert!(decode::<AgentList>(status, "agent_list", Effect::Read).is_err());
    }
}
