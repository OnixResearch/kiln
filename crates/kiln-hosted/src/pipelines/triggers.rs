//! Hosted pipeline triggers: filtered source events and reviewed calendar
//! schedules.
//!
//! An unfiltered trigger serializes as its bare name (`"push"`), exactly as
//! before, so existing pipeline plans keep their identity. Filtered triggers
//! and schedule triggers are objects with `deny_unknown_fields`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use kiln_core::monorepo::{ChangedFallback, ChangedFiles};
use kiln_core::trigger_filter::{
    self, EventKind, PullRequestType, Selection, TriggerEvent, TriggerFilter,
};
use kiln_web_service::schedule::{self, CronSchedule, Reservation};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::de::{self, IntoDeserializer, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Value, json};

use super::{CompiledPipeline, fail, name};

/// Triggers per pipeline.
pub const MAX_TRIGGERS: usize = 8;
/// Reviewed schedule entries per pipeline.
pub const MAX_SCHEDULES: usize = 8;
/// Reference a schedule entry runs when it names none.
pub const DEFAULT_SCHEDULE_REFERENCE: &str = "refs/heads/main";
/// Bytes per schedule reference.
pub const MAX_REFERENCE_BYTES: usize = 512;
const SCHEDULE_IDENTITY_DOMAIN: &str = "kiln.hosted.pipeline-schedule.v1";

/// One pipeline trigger.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trigger {
    Manual,
    Push(TriggerFilter),
    PullRequest(TriggerFilter),
    /// Fires from the named reviewed `schedules` entry.
    Schedule {
        schedule: String,
    },
}

static UNFILTERED: TriggerFilter = TriggerFilter {
    branches: Vec::new(),
    tags: Vec::new(),
    types: Vec::new(),
    paths: Vec::new(),
};

impl Trigger {
    #[must_use]
    pub fn kind(&self) -> EventKind {
        match self {
            Self::Manual => EventKind::Manual,
            Self::Push(_) => EventKind::Push,
            Self::PullRequest(_) => EventKind::PullRequest,
            Self::Schedule { .. } => EventKind::Schedule,
        }
    }

    #[must_use]
    pub fn filter(&self) -> &TriggerFilter {
        match self {
            Self::Push(filter) | Self::PullRequest(filter) => filter,
            Self::Manual | Self::Schedule { .. } => &UNFILTERED,
        }
    }

    fn key(&self) -> (EventKind, Option<&str>) {
        match self {
            Self::Schedule { schedule } => (EventKind::Schedule, Some(schedule.as_str())),
            other => (other.kind(), None),
        }
    }
}

/// The object form of a trigger.
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TriggerObject {
    event: Option<EventKind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    branches: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    types: Vec<PullRequestType>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    schedule: Option<String>,
}

impl TriggerObject {
    fn into_trigger(self) -> Result<Trigger, String> {
        let event = self.event.ok_or("trigger object requires event")?;
        let filter = TriggerFilter {
            branches: self.branches,
            tags: self.tags,
            types: self.types,
            paths: self.paths,
        };
        if event != EventKind::Schedule && self.schedule.is_some() {
            return Err("only schedule triggers name a schedule entry".into());
        }
        match event {
            EventKind::Manual if filter.is_empty() => Ok(Trigger::Manual),
            EventKind::Manual => Err("manual triggers accept no filters".into()),
            EventKind::Push => Ok(Trigger::Push(filter)),
            EventKind::PullRequest => Ok(Trigger::PullRequest(filter)),
            EventKind::Schedule if !filter.is_empty() => {
                Err("schedule triggers accept no filters".into())
            }
            EventKind::Schedule => self
                .schedule
                .map(|schedule| Trigger::Schedule { schedule })
                .ok_or_else(|| "schedule triggers must name a reviewed schedule entry".into()),
        }
    }
}

impl Serialize for Trigger {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Manual => EventKind::Manual.serialize(serializer),
            Self::Push(filter) | Self::PullRequest(filter) if filter.is_empty() => {
                self.kind().serialize(serializer)
            }
            Self::Push(filter) | Self::PullRequest(filter) => TriggerObject {
                event: Some(self.kind()),
                branches: filter.branches.clone(),
                tags: filter.tags.clone(),
                types: filter.types.clone(),
                paths: filter.paths.clone(),
                schedule: None,
            }
            .serialize(serializer),
            Self::Schedule { schedule } => TriggerObject {
                event: Some(EventKind::Schedule),
                schedule: Some(schedule.clone()),
                ..TriggerObject::default()
            }
            .serialize(serializer),
        }
    }
}

struct TriggerVisitor;

impl<'de> Visitor<'de> for TriggerVisitor {
    type Value = Trigger;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a trigger name or a trigger object with event")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Trigger, E> {
        let event = EventKind::deserialize(value.into_deserializer())?;
        TriggerObject {
            event: Some(event),
            ..TriggerObject::default()
        }
        .into_trigger()
        .map_err(E::custom)
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Trigger, A::Error> {
        TriggerObject::deserialize(de::value::MapAccessDeserializer::new(map))?
            .into_trigger()
            .map_err(de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for Trigger {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(TriggerVisitor)
    }
}

fn default_timezone() -> String {
    schedule::DEFAULT_TIMEZONE.into()
}
fn default_min_interval() -> u64 {
    schedule::DEFAULT_MIN_INTERVAL_MS
}
fn default_reference() -> String {
    DEFAULT_SCHEDULE_REFERENCE.into()
}

/// One reviewed calendar entry of a pipeline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleEntry {
    pub cron: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
    #[serde(default = "default_min_interval")]
    pub min_interval_ms: u64,
    /// The default-branch reference whose exact revision a fire binds.
    #[serde(default = "default_reference")]
    pub reference: String,
}

impl ScheduleEntry {
    fn spec(&self) -> CronSchedule {
        CronSchedule {
            expression: self.cron.clone(),
            timezone: self.timezone.clone(),
        }
    }
}

fn valid_reference(value: &str) -> bool {
    value.starts_with("refs/heads/")
        && value.len() > "refs/heads/".len()
        && value.len() <= MAX_REFERENCE_BYTES
        && !value.ends_with('/')
        && !value.contains("..")
        && !value.contains("//")
        && !value.contains("@{")
        && !value
            .bytes()
            .any(|v| v.is_ascii_control() || v.is_ascii_whitespace() || b"~^:?*[\\".contains(&v))
}

fn valid_revision(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

// r[impl kiln.triggers.filter]
// r[impl kiln.schedules.hosted]
/// Admit the trigger list and its schedule entries: bounded, distinct by
/// event (and schedule name), filters admitted per kind, every schedule
/// trigger naming an entry and every entry referenced.
pub fn admit(
    triggers: &[Trigger],
    schedules: &BTreeMap<String, ScheduleEntry>,
) -> Result<(), String> {
    if triggers.is_empty() || triggers.len() > MAX_TRIGGERS {
        return Err(fail("$.triggers", "1..8 distinct triggers are required"));
    }
    let mut keys = BTreeSet::new();
    for (index, trigger) in triggers.iter().enumerate() {
        let path = format!("$.triggers[{index}]");
        if !keys.insert(trigger.key()) {
            return Err(fail("$.triggers", "duplicate trigger"));
        }
        trigger_filter::admit_filter(trigger.kind(), trigger.filter())
            .map_err(|e| fail(&format!("{path}.{}", e.field.as_str()), e.message))?;
        if let Trigger::Schedule { schedule } = trigger {
            if !schedules.contains_key(schedule) {
                return Err(fail(&format!("{path}.schedule"), "unknown schedule entry"));
            }
        }
    }
    if schedules.len() > MAX_SCHEDULES {
        return Err(fail("$.schedules", "more than 8 schedule entries"));
    }
    for (id, entry) in schedules {
        let path = format!("$.schedules.{id}");
        name(id, &path)?;
        if !keys.contains(&(EventKind::Schedule, Some(id.as_str()))) {
            return Err(fail(&path, "schedule entry is not referenced by a trigger"));
        }
        // Syntax and time zone only; `H` and the interval depend on the
        // reviewed organization/project/name identity checked at review.
        schedule::compile(&entry.spec(), &[]).map_err(|e| fail(&path, &e.to_string()))?;
        if !(schedule::CRON_MIN_INTERVAL_FLOOR_MS..=schedule::MAX_MIN_INTERVAL_MS)
            .contains(&entry.min_interval_ms)
        {
            return Err(fail(
                &format!("{path}.min_interval_ms"),
                "minimum interval outside 60000..=31536000000",
            ));
        }
        if !valid_reference(&entry.reference) {
            return Err(fail(
                &format!("{path}.reference"),
                "expected an exact refs/heads/ reference",
            ));
        }
    }
    Ok(())
}

fn identity<'a>(
    organization: &'a str,
    project: &'a str,
    pipeline: &'a str,
    schedule: &'a str,
) -> [&'a str; 5] {
    [
        SCHEDULE_IDENTITY_DOMAIN,
        organization,
        project,
        pipeline,
        schedule,
    ]
}

fn compiled(entry: &ScheduleEntry, parts: &[&str], path: &str) -> Result<schedule::Cron, String> {
    schedule::compile(&entry.spec(), parts).map_err(|e| fail(path, &e.to_string()))
}

// r[impl kiln.schedules.min_interval]
/// Review-time check with `H` resolved for the exact owning identity.
pub fn review(
    schedules: &BTreeMap<String, ScheduleEntry>,
    organization: &str,
    project: &str,
    pipeline: &str,
    now_ms: u64,
) -> Result<(), String> {
    for (id, entry) in schedules {
        let path = format!("$.schedules.{id}");
        compiled(entry, &identity(organization, project, pipeline, id), &path)?
            .check_min_interval(now_ms, entry.min_interval_ms)
            .map_err(|e| fail(&path, &e.to_string()))?;
    }
    Ok(())
}

/// A source event a hosted run request asserts about its workspace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceEvent {
    pub kind: EventKind,
    /// Full pushed reference (`refs/heads/main`, `refs/tags/v1`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<PullRequestType>,
    /// Pull request base branch name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
}

fn text(value: &Option<String>) -> Option<&str> {
    value
        .as_deref()
        .filter(|v| !v.is_empty() && v.len() <= MAX_REFERENCE_BYTES)
}

impl SourceEvent {
    fn trigger_event(&self) -> Result<TriggerEvent<'_>, String> {
        if self.base_revision.is_some() && self.revision.is_none()
            || [&self.base_revision, &self.revision]
                .into_iter()
                .flatten()
                .any(|value| !valid_revision(value))
        {
            return Err("event revisions must be exact lowercase commit identities".into());
        }
        match self.kind {
            EventKind::Push if self.action.is_none() && self.base_branch.is_none() => {
                let reference = text(&self.reference)
                    .filter(|v| v.starts_with("refs/"))
                    .ok_or("push events require a full reference")?;
                Ok(TriggerEvent::Push { reference })
            }
            EventKind::PullRequest if self.reference.is_none() => Ok(TriggerEvent::PullRequest {
                action: self.action.ok_or("pull request events require an action")?,
                base_branch: text(&self.base_branch)
                    .ok_or("pull request events require a base branch")?,
            }),
            EventKind::Manual
                if self.reference.is_none()
                    && self.action.is_none()
                    && self.base_branch.is_none()
                    && self.revision.is_none() =>
            {
                Ok(TriggerEvent::Manual)
            }
            EventKind::Schedule => {
                Err("schedule runs are reserved through pipelines.schedule".into())
            }
            _ => Err("event fields do not match its kind".into()),
        }
    }
}

// r[impl kiln.triggers.evaluate]
/// Select the first plan trigger that runs `event`. Changed files are read
/// from the exact revisions in the workspace repository only when a `paths`
/// filter needs them; unknown facts fall back and never skip.
pub fn select(
    plan: &CompiledPipeline,
    event: &SourceEvent,
    workspace: &Path,
) -> Result<Selection, String> {
    let trigger_event = event.trigger_event()?;
    let changed = if plan
        .triggers
        .iter()
        .any(|t| t.filter().needs_changed_files())
    {
        match (&event.base_revision, &event.revision) {
            (Some(base), Some(revision)) => {
                kiln_web_service::monorepo::changed_files_at(workspace, Some(base), revision)
            }
            _ => ChangedFiles::Fallback(ChangedFallback::NoBase),
        }
    } else {
        ChangedFiles::Exact(BTreeSet::new())
    };
    Ok(trigger_filter::select(
        plan.triggers.iter().map(|t| (t.kind(), t.filter())),
        &trigger_event,
        &changed,
    ))
}

pub(super) fn init(db: &Connection) -> Result<(), String> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS hosted_pipeline_schedules(organization TEXT NOT NULL,project TEXT NOT NULL,name TEXT NOT NULL,schedule TEXT NOT NULL,plan_identity TEXT NOT NULL,next_due_ms INTEGER NOT NULL,last_fired_ms INTEGER,last_revision TEXT,last_attempt TEXT,status TEXT NOT NULL,PRIMARY KEY(organization,project,name,schedule));")
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScheduleTick {
    project: String,
    name: String,
    schedule: String,
    /// The default-branch reference the caller observed; must be the entry's.
    reference: String,
    /// The exact revision of `reference` observed at the call (due) time.
    revision: String,
}

struct Ledger {
    plan_identity: String,
    next_due_ms: u64,
    last_fired_ms: Option<u64>,
    last_revision: Option<String>,
    last_attempt: Option<String>,
}

fn sql(value: u64) -> Result<i64, String> {
    crate::sqlite_integer(value)
}

// r[impl kiln.schedules.hosted]
// r[impl kiln.schedules.no_catch_up]
/// Durable schedule reservation. The first call for a reviewed plan only
/// initializes the next occurrence; later calls fire at most once per due
/// occurrence, skip occurrences missed by more than the minimum interval,
/// re-check the minimum interval, and bind the exact observed revision.
pub fn reserve(
    db: &mut Connection,
    actor: &crate::Actor,
    input: Value,
    now_ms: u64,
) -> Result<Value, String> {
    let input: ScheduleTick = serde_json::from_value(input).map_err(|e| e.to_string())?;
    name(&input.project, "$.project")?;
    name(&input.name, "$.name")?;
    name(&input.schedule, "$.schedule")?;
    if !valid_reference(&input.reference) || !valid_revision(&input.revision) {
        return Err("observed reference or revision is not exact".into());
    }
    if now_ms > schedule::MAX_TIMESTAMP_MS {
        return Err("clock outside schedule bounds".into());
    }
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let actor = crate::accounts::require_project(
        &tx,
        actor,
        &input.project,
        &["owner", "admin", "member"],
    )?;
    let (stored_identity, config): (String, String) = tx
        .query_row(
            "SELECT identity,config FROM hosted_pipelines WHERE organization=?1 AND project=?2 AND name=?3",
            params![actor.organization, input.project, input.name],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or("pipeline is not currently reviewed for this organization/project")?;
    let plan = super::compile_value(serde_json::from_str(&config).map_err(|e| e.to_string())?)?;
    if plan.identity != stored_identity {
        return Err("stored pipeline identity mismatch".into());
    }
    let entry = plan
        .schedules
        .get(&input.schedule)
        .filter(|_| {
            plan.triggers
                .iter()
                .any(|t| matches!(t, Trigger::Schedule { schedule } if schedule == &input.schedule))
        })
        .ok_or("schedule is not a reviewed trigger of this pipeline")?;
    if entry.reference != input.reference {
        return Err("observed reference is not the reviewed schedule reference".into());
    }
    let cron = compiled(
        entry,
        &identity(
            &actor.organization,
            &input.project,
            &input.name,
            &input.schedule,
        ),
        "$.schedule",
    )?;
    let key = params![
        actor.organization,
        input.project,
        input.name,
        input.schedule
    ];
    let ledger: Option<Ledger> = tx
        .query_row(
            "SELECT plan_identity,next_due_ms,last_fired_ms,last_revision,last_attempt FROM hosted_pipeline_schedules WHERE organization=?1 AND project=?2 AND name=?3 AND schedule=?4",
            key,
            |row| {
                Ok(Ledger {
                    plan_identity: row.get(0)?,
                    next_due_ms: crate::sqlite_u64(row, 1)?,
                    last_fired_ms: row
                        .get::<_, Option<i64>>(2)?
                        .and_then(|v| u64::try_from(v).ok()),
                    last_revision: row.get(3)?,
                    last_attempt: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let next = |after: u64| {
        cron.next_after(after)
            .filter(|v| *v <= schedule::MAX_TIMESTAMP_MS)
            .ok_or_else(|| "schedule has no later occurrence".to_string())
    };
    let trigger = json!({"event":"schedule","schedule":input.schedule});
    let last = |ledger: &Ledger| json!({"occurrence_ms":ledger.last_fired_ms,"revision":ledger.last_revision,"attempt_id":ledger.last_attempt});
    // A new or changed reviewed plan starts at its next occurrence: review
    // never fires an occurrence that passed before it, and never catches up.
    let Some(ledger) = ledger.filter(|l| l.plan_identity == plan.identity) else {
        let next_due_ms = next(now_ms)?;
        tx.execute(
            "INSERT INTO hosted_pipeline_schedules VALUES(?1,?2,?3,?4,?5,?6,NULL,NULL,NULL,'initialized') ON CONFLICT(organization,project,name,schedule) DO UPDATE SET plan_identity=excluded.plan_identity,next_due_ms=excluded.next_due_ms,status=excluded.status",
            params![actor.organization, input.project, input.name, input.schedule, plan.identity, sql(next_due_ms)?],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        return Ok(
            json!({"reserved":false,"status":"initialized","next_due_ms":next_due_ms,"pipeline_identity":plan.identity,"trigger":trigger,"resolved":cron.resolved()}),
        );
    };
    let decision = cron
        .decide(
            ledger.next_due_ms,
            ledger.last_fired_ms,
            now_ms,
            entry.min_interval_ms,
        )
        .map_err(|e| e.to_string())?;
    let (status, occurrence_ms, next_due_ms) = match decision {
        Reservation::NotDue => {
            return Ok(
                json!({"reserved":false,"status":"not_due","next_due_ms":ledger.next_due_ms,"pipeline_identity":plan.identity,"trigger":trigger,"last":last(&ledger)}),
            );
        }
        Reservation::Missed {
            occurrence_ms,
            next_due_ms,
        } => ("missed", occurrence_ms, next_due_ms),
        Reservation::TooClose {
            occurrence_ms,
            next_due_ms,
            ..
        } => ("too_close", occurrence_ms, next_due_ms),
        Reservation::Fire {
            occurrence_ms,
            next_due_ms,
        } => ("reserved", occurrence_ms, next_due_ms),
    };
    if status != "reserved" {
        tx.execute(
            "UPDATE hosted_pipeline_schedules SET next_due_ms=?5,status=?6 WHERE organization=?1 AND project=?2 AND name=?3 AND schedule=?4",
            params![actor.organization, input.project, input.name, input.schedule, sql(next_due_ms)?, status],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        return Ok(
            json!({"reserved":false,"status":status,"occurrence_ms":occurrence_ms,"next_due_ms":next_due_ms,"pipeline_identity":plan.identity,"trigger":trigger,"last":last(&ledger)}),
        );
    }
    let digest = crate::identity(&(
        SCHEDULE_IDENTITY_DOMAIN,
        &actor.organization,
        &input.project,
        &input.name,
        &input.schedule,
        occurrence_ms,
    ))?;
    let attempt_id = format!(
        "schedule-{}",
        digest.get(..32).ok_or("attempt identity length")?
    );
    tx.execute(
        "UPDATE hosted_pipeline_schedules SET next_due_ms=?5,last_fired_ms=?6,last_revision=?7,last_attempt=?8,status='reserved' WHERE organization=?1 AND project=?2 AND name=?3 AND schedule=?4",
        params![actor.organization, input.project, input.name, input.schedule, sql(next_due_ms)?, sql(occurrence_ms)?, input.revision, attempt_id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(
        json!({"reserved":true,"status":"reserved","occurrence_ms":occurrence_ms,"next_due_ms":next_due_ms,"pipeline_identity":plan.identity,"reference":input.reference,"revision":input.revision,"attempt_id":attempt_id,"trigger":trigger}),
    )
}

const ACTIONS_IGNORE_FILTERS: [&str; 3] = ["branches-ignore", "tags-ignore", "paths-ignore"];

fn actions_patterns(value: &Value, path: &str) -> Result<Vec<String>, String> {
    let patterns = super::string_list(value, path)?;
    for pattern in &patterns {
        // Actions `!` negation, `?` (zero or one of the preceding character),
        // `+` and `[...]` have no equivalent in the shared matcher.
        if pattern.starts_with('!') || pattern.contains(['?', '+', '[', ']']) {
            return Err(fail(
                path,
                "negated, `?`, `+` and character-class Actions patterns have no native equivalent",
            ));
        }
    }
    Ok(patterns)
}

fn actions_types(
    value: &Value,
    path: &str,
    notes: &mut Vec<String>,
) -> Result<Vec<PullRequestType>, String> {
    let mut types = Vec::new();
    for name in super::string_list(value, path)? {
        let mapped: &[PullRequestType] = match name.as_str() {
            "opened" => &[PullRequestType::Opened],
            "reopened" => &[PullRequestType::Reopened],
            "synchronize" => &[PullRequestType::Synchronize],
            // Actions `closed` fires for merged and unmerged pull requests.
            "closed" => {
                notes.push(format!(
                    "{path}: Actions closed maps explicitly to native closed and merged"
                ));
                &[PullRequestType::Closed, PullRequestType::Merged]
            }
            _ => {
                return Err(fail(
                    path,
                    "only opened, reopened, synchronize and closed activity types have native equivalents",
                ));
            }
        };
        for value in mapped {
            if !types.contains(value) {
                types.push(*value);
            }
        }
    }
    Ok(types)
}

// r[impl kiln.triggers.migrate]
/// Map one Actions `on` entry. Filters map exactly; `*-ignore`, negation and
/// unsupported keys fail with their path instead of being dropped.
pub(super) fn actions_trigger(
    event: &str,
    config: Option<&Value>,
    path: &str,
    notes: &mut Vec<String>,
) -> Result<Trigger, String> {
    let (kind, allowed): (EventKind, &[&str]) = match event {
        "workflow_dispatch" => (EventKind::Manual, &[]),
        "push" => (EventKind::Push, &["branches", "tags", "paths"]),
        "pull_request" => (EventKind::PullRequest, &["branches", "types", "paths"]),
        _ => {
            return Err(fail(
                path,
                "only push, pull_request and workflow_dispatch triggers are supported",
            ));
        }
    };
    let options = match config {
        None | Some(Value::Null) => serde_json::Map::new(),
        Some(Value::Object(options)) => options.clone(),
        Some(_) => return Err(fail(path, "expected trigger options object")),
    };
    for key in options.keys() {
        if ACTIONS_IGNORE_FILTERS.contains(&key.as_str()) {
            return Err(fail(
                &format!("{path}.{key}"),
                "ignore filters have no native equivalent; rewrite them as positive branches, tags or paths",
            ));
        }
        if !allowed.contains(&key.as_str()) {
            return Err(fail(
                &format!("{path}.{key}"),
                "unsupported trigger option; inputs and other options require an explicit native mapping",
            ));
        }
    }
    let list = |key: &str| {
        options
            .get(key)
            .map(|value| actions_patterns(value, &format!("{path}.{key}")))
            .transpose()
            .map(Option::unwrap_or_default)
    };
    let filter = TriggerFilter {
        branches: list("branches")?,
        tags: list("tags")?,
        types: options
            .get("types")
            .map(|value| actions_types(value, &format!("{path}.types"), notes))
            .transpose()?
            .unwrap_or_default(),
        paths: list("paths")?,
    };
    trigger_filter::admit_filter(kind, &filter)
        .map_err(|e| fail(&format!("{path}.{}", e.field.as_str()), e.message))?;
    Ok(match kind {
        EventKind::Push => Trigger::Push(filter),
        EventKind::PullRequest => Trigger::PullRequest(filter),
        _ => Trigger::Manual,
    })
}
