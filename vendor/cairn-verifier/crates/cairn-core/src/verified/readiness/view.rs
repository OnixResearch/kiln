//! Bounded, identity-bound projection of the existing readiness receipt.
use super::{
    ReadinessAction, ReadinessBlocker, ReadinessReport, ReadinessScope, ReadinessState,
    READINESS_STAGES,
};
use crate::canonical_json_hash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::{self, Write},
    sync::LazyLock,
};

pub const AGENT_VIEW_SCHEMA: &str = "cairn.readiness-agent.v1";
pub const DETAIL_SCHEMA: &str = "cairn.readiness-detail.v1";

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentViewPolicy {
    pub max_bytes: usize,
    pub default_bytes: usize,
    pub max_entries: usize,
    pub default_entries: usize,
    pub max_detail_bytes: usize,
}

static VIEW_POLICY_BINDING: LazyLock<(AgentViewPolicy, String)> = LazyLock::new(|| {
    let value: Value = serde_json::from_str(include_str!(
        "../../../../../cairn-policy/generated/readiness-view.json"
    ))
    .expect("checked agent readiness view policy");
    let hash = canonical_json_hash(&value);
    (
        serde_json::from_value(value).expect("typed agent readiness view policy"),
        hash,
    )
});

fn view_policy_binding() -> &'static (AgentViewPolicy, String) {
    &VIEW_POLICY_BINDING
}

pub fn agent_view_policy() -> AgentViewPolicy {
    view_policy_binding().0
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AgentViewRequest {
    pub max_bytes: Option<usize>,
    pub max_entries: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DetailSelector<'a> {
    pub kind: &'static str,
    pub id: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentStage<'a> {
    pub stage: &'a str,
    pub state: ReadinessState,
    pub decision_hash: &'a str,
}

#[derive(Debug, Serialize)]
pub struct AgentReadiness<'a> {
    pub schema: &'static str,
    pub scope: &'a ReadinessScope,
    pub valid: bool,
    pub classification: &'a str,
    pub stages: Vec<AgentStage<'a>>,
    pub mutation_authorized: bool,
    pub readiness_policy_hash: &'a str,
    pub view_policy_hash: &'static str,
    pub input_hash: &'a str,
    pub full_receipt_hash: &'a str,
    pub diagnostics: &'a [String],
    pub blockers: Vec<&'a ReadinessBlocker>,
    pub actions: Vec<&'a ReadinessAction>,
    pub blocker_count: usize,
    pub action_count: usize,
    pub omitted_blockers: usize,
    pub omitted_actions: usize,
    /// Full stage decisions remain detail-only, so the compact view is never a full receipt.
    pub omitted_stage_decisions: usize,
    pub complete: bool,
    pub detail_selectors: Vec<DetailSelector<'a>>,
    pub receipt_hash: String,
}

fn update_counts(view: &mut AgentReadiness<'_>) {
    view.omitted_blockers = view.blocker_count - view.blockers.len();
    view.omitted_actions = view.action_count - view.actions.len();
    view.complete = view.omitted_blockers == 0
        && view.omitted_actions == 0
        && view.omitted_stage_decisions == 0;
}

#[derive(Default)]
struct CountJsonBytes(usize);

impl Write for CountJsonBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 += bytes.len();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn json_bytes(value: &impl Serialize) -> Result<usize, serde_json::Error> {
    let mut count = CountJsonBytes::default();
    serde_json::to_writer(&mut count, value)?;
    Ok(count.0)
}

fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn size_after_entry(
    current: usize,
    entry: usize,
    selector: usize,
    already_selected: usize,
    remaining: usize,
    selectors_before: usize,
) -> usize {
    current + entry + usize::from(already_selected > 0) + decimal_digits(remaining - 1)
        - selector
        - usize::from(selectors_before > 1)
        - decimal_digits(remaining)
}

// r[impl cairn.agent_readiness.decisions]
// r[impl cairn.agent_readiness.actions]
pub fn project_agent_readiness(
    report: &ReadinessReport,
    request: AgentViewRequest,
) -> Result<Value, &'static str> {
    let policy = agent_view_policy();
    let byte_limit = request.max_bytes.unwrap_or(policy.default_bytes);
    let entry_limit = request.max_entries.unwrap_or(policy.default_entries);
    if byte_limit == 0 || byte_limit > policy.max_bytes || entry_limit > policy.max_entries {
        return Err("invalid agent readiness budget");
    }
    let mut view = AgentReadiness {
        schema: AGENT_VIEW_SCHEMA,
        scope: &report.scope,
        valid: report.valid,
        classification: &report.classification,
        stages: report
            .stages
            .iter()
            .map(|s| AgentStage {
                stage: &s.stage,
                state: s.state,
                decision_hash: &s.decision_hash,
            })
            .collect(),
        mutation_authorized: report.mutation_authorized,
        readiness_policy_hash: &report.readiness_policy_hash,
        view_policy_hash: view_policy_binding().1.as_str(),
        input_hash: &report.input_hash,
        full_receipt_hash: &report.receipt_hash,
        diagnostics: &report.diagnostics,
        blockers: Vec::new(),
        actions: Vec::new(),
        blocker_count: report.blockers.len(),
        action_count: report.actions.len(),
        omitted_blockers: report.blockers.len(),
        omitted_actions: report.actions.len(),
        omitted_stage_decisions: report.stages.len(),
        complete: false,
        detail_selectors: report
            .stages
            .iter()
            .map(|s| DetailSelector {
                kind: "stage",
                id: &s.stage,
            })
            .chain(report.blockers.iter().map(|b| DetailSelector {
                kind: "blocker",
                id: &b.id,
            }))
            .chain(report.actions.iter().map(|a| DetailSelector {
                kind: "action",
                id: &a.id,
            }))
            .collect(),
        receipt_hash: String::new(),
    };
    // CLI emits compact JSON followed by one LF. This budget covers those exact
    // standalone bytes, not MCP's independent content/structured framing.
    let mut bytes = json_bytes(&view).map_err(|_| "agent readiness serialization failed")? + 64 + 1;
    if bytes > byte_limit {
        return Err("insufficient agent readiness budget for mandatory envelope");
    }
    // Preserve the original blocker-then-action prefix. Calculate each whole
    // record, removed selector, array comma, and decimal count transition once.
    let mut selected_blockers = 0;
    let mut selected_actions = 0;
    let mut blocker_overflow = false;
    for blocker in &report.blockers {
        if selected_blockers >= entry_limit {
            break;
        }
        let remaining = report.blockers.len() - selected_blockers;
        let selector = DetailSelector {
            kind: "blocker",
            id: &blocker.id,
        };
        let next = size_after_entry(
            bytes,
            json_bytes(blocker).map_err(|_| "agent readiness serialization failed")?,
            json_bytes(&selector).map_err(|_| "agent readiness serialization failed")?,
            selected_blockers,
            remaining,
            report.stages.len() + remaining + report.actions.len(),
        );
        if next > byte_limit {
            blocker_overflow = true;
            break;
        }
        bytes = next;
        selected_blockers += 1;
    }
    if !blocker_overflow {
        for action in &report.actions {
            if selected_blockers + selected_actions >= entry_limit {
                break;
            }
            let remaining = report.actions.len() - selected_actions;
            let selector = DetailSelector {
                kind: "action",
                id: &action.id,
            };
            let next = size_after_entry(
                bytes,
                json_bytes(action).map_err(|_| "agent readiness serialization failed")?,
                json_bytes(&selector).map_err(|_| "agent readiness serialization failed")?,
                selected_actions,
                remaining,
                report.stages.len() + report.blockers.len() - selected_blockers + remaining,
            );
            if next > byte_limit {
                break;
            }
            bytes = next;
            selected_actions += 1;
        }
    }
    view.blockers
        .extend(report.blockers.iter().take(selected_blockers));
    view.actions
        .extend(report.actions.iter().take(selected_actions));
    let stage_count = report.stages.len();
    drop(
        view.detail_selectors
            .drain(stage_count..stage_count + selected_blockers),
    );
    let action_start = stage_count + report.blockers.len() - selected_blockers;
    drop(
        view.detail_selectors
            .drain(action_start..action_start + selected_actions),
    );
    update_counts(&mut view);
    let mut value = json!(view);
    value["receipt_hash"] = json!(canonical_json_hash(&value));
    Ok(value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailKind {
    Blocker,
    Action,
    Stage,
}

impl DetailKind {
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "blocker" => Ok(Self::Blocker),
            "action" => Ok(Self::Action),
            "stage" => Ok(Self::Stage),
            _ => Err("unknown readiness detail kind"),
        }
    }
}

// r[impl cairn.agent_readiness.detail]
pub fn readiness_detail(
    report: &ReadinessReport,
    kind: DetailKind,
    id: &str,
    expected_receipt: &str,
) -> Result<Value, &'static str> {
    if expected_receipt != report.receipt_hash {
        return Err("stale readiness detail receipt");
    }
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err("unsafe readiness detail id");
    }
    let (kind_name, detail) = match kind {
        DetailKind::Blocker => (
            "blocker",
            report
                .blockers
                .iter()
                .find(|b| b.id == id)
                .map(|b| json!(b)),
        ),
        DetailKind::Action => (
            "action",
            report.actions.iter().find(|a| a.id == id).map(|a| json!(a)),
        ),
        DetailKind::Stage => (
            "stage",
            READINESS_STAGES
                .contains(&id)
                .then(|| report.stages.iter().find(|s| s.stage == id))
                .flatten()
                .map(|s| json!(s)),
        ),
    };
    let detail = detail.ok_or("unknown readiness detail id")?;
    let mut value = json!({
        "schema": DETAIL_SCHEMA,
        "kind": kind_name,
        "id": id,
        "full_receipt_hash": &report.receipt_hash,
        "scope": &report.scope,
        "detail": detail,
        "receipt_hash": "",
    });
    // As with the projection, bound standalone compact JSON plus its CLI LF;
    // MCP's separate tool-result framing is not charged to this detail limit.
    if json_bytes(&value).map_err(|_| "readiness detail serialization failed")? + 64 + 1
        > agent_view_policy().max_detail_bytes
    {
        return Err("readiness detail exceeds policy byte limit");
    }
    value["receipt_hash"] = json!(canonical_json_hash(&value));
    Ok(value)
}
