//! Pure Kanban projection over the visible work snapshot.
//!
//! A board is a saved view's configuration; its cards are the matching issues
//! and its columns are canonical issue statuses. Nothing here reads storage,
//! clocks or the network, and nothing here stores card state.
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::model::{self, Board, Kind, MAX_BOARD_COLUMNS, Record, STATUSES};

/// Bound on cards returned per column (and for the Other lane).
pub const MAX_BOARD_CARDS: usize = 100;
/// Bound on one decoded query value; matches the longest view filter value.
const MAX_QUERY_VALUE: usize = 256;
/// Built-in statuses placed before team-defined columns by default.
const OPEN_STATUSES: [&str; 3] = ["triage", "backlog", "started"];
/// Built-in statuses placed after team-defined columns by default.
const CLOSED_STATUSES: [&str; 4] = ["completed", "cancelled", "snoozed", "duplicate"];
/// URL overrides the Work page applies on top of a saved view's filters.
const OVERRIDES: [&str; 5] = ["kind", "status", "assignee", "project", "q"];
const SORTS: [&str; 5] = ["updated", "created", "priority", "title", "due"];

/// Admitted read parameters of `GET /api/work/board`.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct Query {
    pub view: Option<String>,
    pub overrides: BTreeMap<String, String>,
    pub sort: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Column {
    pub status: String,
    pub builtin: bool,
    /// Teams whose workflow defines this status, in board team order.
    pub teams: Vec<String>,
    /// Neither built-in nor defined by any team any more.
    pub retired: bool,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct Card {
    pub id: String,
    /// The exact head a move of this card must name as `expected_head`.
    pub head: String,
    pub title: String,
    pub status: String,
    pub priority: u8,
    pub team: Option<String>,
    pub assignees: Vec<String>,
    pub labels: Vec<String>,
    pub due_at_ms: Option<i64>,
    pub updated_at_ms: i64,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct Lane {
    #[serde(flatten)]
    pub column: Column,
    pub total: usize,
    pub cards: Vec<Card>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StatusCount {
    pub status: String,
    pub total: usize,
}

/// Matching issues whose status has no column; never silently dropped.
#[derive(Debug, Default, Eq, PartialEq, Serialize)]
pub struct Other {
    pub statuses: Vec<StatusCount>,
    pub total: usize,
    pub cards: Vec<Card>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct ViewRef {
    pub id: String,
    pub head: String,
    pub title: String,
    pub shared: bool,
    pub configured_columns: bool,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct Projection {
    pub schema: &'static str,
    pub view: Option<ViewRef>,
    pub filters: BTreeMap<String, String>,
    pub filtered: bool,
    pub sort: String,
    pub columns: Vec<Lane>,
    pub other: Other,
    /// Default columns dropped by `MAX_BOARD_COLUMNS`; their issues are in `other`.
    pub columns_truncated: usize,
    pub matched_issues: usize,
    pub shown_cards: usize,
    /// Matching non-issue records the list shows but the board does not.
    pub list_only: usize,
    pub snapshot_objects: usize,
    pub object_limit: usize,
    pub card_limit: usize,
    /// True when every matching issue is a returned card.
    pub complete: bool,
    pub bounded: bool,
}

// r[impl kiln.work.kanban.projection]
/// Admit the board query. Values are percent-decoded and bounded; unknown,
/// repeated or malformed parameters are rejected rather than ignored.
pub(super) fn parse_query(params: &BTreeMap<&str, &str>) -> Result<Query, ()> {
    let mut query = Query::default();
    for (key, raw) in params {
        let value = crate::engine::api::decoded(raw)?;
        if value.len() > MAX_QUERY_VALUE || value.chars().any(char::is_control) {
            return Err(());
        }
        match *key {
            "view" if super::store::is_oid(&value) => query.view = Some(value),
            "sort" if SORTS.contains(&value.as_str()) => query.sort = Some(value),
            key if OVERRIDES.contains(&key) => {
                if !value.trim().is_empty() {
                    query.overrides.insert(key.to_owned(), value);
                }
            }
            _ => return Err(()),
        }
    }
    Ok(query)
}

/// The list's filter semantics (`KilnWorkBoard.matches` in `work-board.js`).
/// Both sides are checked against one shared fixture.
pub fn matches(record: &Record, filters: &BTreeMap<String, String>) -> bool {
    let fields = &record.fields;
    filters.iter().all(|(key, value)| {
        if value.is_empty() {
            return true;
        }
        match key.as_str() {
            "q" => {
                let mut haystack: Vec<&str> = [
                    Some(fields.title.as_str()),
                    Some(fields.description.as_str()),
                    fields.customer.as_deref(),
                    fields.source_url.as_deref(),
                ]
                .into_iter()
                .flatten()
                .filter(|text| !text.is_empty())
                .collect();
                haystack.extend(fields.labels.iter().map(String::as_str));
                haystack.push(&record.id);
                haystack
                    .join(" ")
                    .to_lowercase()
                    .contains(&value.to_lowercase())
            }
            "kind" => kind_name(record.kind) == value,
            "assignee" => fields.assignees.contains(value),
            "label" => fields.labels.contains(value),
            "status" => fields.status == *value,
            "priority" => fields.priority.to_string() == *value,
            "team" => fields.team.as_ref() == Some(value),
            "project" => fields.project.as_ref() == Some(value),
            "cycle" => fields.cycle.as_ref() == Some(value),
            "initiative" => fields.initiative.as_ref() == Some(value),
            _ => false,
        }
    })
}

/// Whether the Work page list shows a record (`KilnWorkBoard.listed`).
pub fn listed(record: &Record, filters: &BTreeMap<String, String>, view_active: bool) -> bool {
    let kind = filters.get("kind").filter(|kind| !kind.is_empty());
    if record.kind == Kind::Personal && kind.map(String::as_str) != Some("personal") {
        return false;
    }
    if kind.is_none() && !view_active && !matches!(record.kind, Kind::Issue | Kind::Request) {
        return false;
    }
    matches(record, filters)
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Issue => "issue",
        Kind::Project => "project",
        Kind::Milestone => "milestone",
        Kind::Cycle => "cycle",
        Kind::Initiative => "initiative",
        Kind::Team => "team",
        Kind::Request => "request",
        Kind::View => "view",
        Kind::Personal => "personal",
    }
}

// r[impl kiln.work.kanban.columns]
/// Ordered board columns. Configured columns keep their order; otherwise the
/// default is open built-ins, team-defined statuses (the view's team, or every
/// team for a mixed-team board), then closed built-ins. Returns the columns
/// and how many default team columns `MAX_BOARD_COLUMNS` dropped.
pub fn columns(
    board: Option<&Board>,
    team_scope: Option<&str>,
    records: &BTreeMap<String, Record>,
) -> (Vec<Column>, usize) {
    let mut teams: Vec<&Record> = records
        .values()
        .filter(|record| record.kind == Kind::Team)
        .collect();
    teams.sort_by(|a, b| {
        a.fields
            .title
            .cmp(&b.fields.title)
            .then_with(|| a.id.cmp(&b.id))
    });
    let describe = |status: &str| {
        let builtin = STATUSES.contains(&status);
        let defining: Vec<String> = teams
            .iter()
            .filter(|team| team.fields.workflow.iter().any(|s| s == status))
            .map(|team| team.id.clone())
            .collect();
        Column {
            status: status.to_owned(),
            builtin,
            retired: !builtin && defining.is_empty(),
            teams: defining,
        }
    };
    if let Some(board) = board {
        return (board.columns.iter().map(|s| describe(s)).collect(), 0);
    }
    let mut custom = Vec::new();
    let mut seen: BTreeSet<&str> = STATUSES.iter().copied().collect();
    for team in teams
        .iter()
        .filter(|team| team_scope.is_none_or(|scope| team.id == scope))
    {
        for status in &team.fields.workflow {
            if seen.insert(status) {
                custom.push(status.as_str());
            }
        }
    }
    let room = MAX_BOARD_COLUMNS - OPEN_STATUSES.len() - CLOSED_STATUSES.len();
    let truncated = custom.len().saturating_sub(room);
    custom.truncate(room);
    let ordered = OPEN_STATUSES
        .iter()
        .copied()
        .chain(custom)
        .chain(CLOSED_STATUSES.iter().copied());
    (ordered.map(describe).collect(), truncated)
}

fn compare(sort: &str, a: &Record, b: &Record) -> Ordering {
    let order = match sort {
        "created" => b.created_at_ms.cmp(&a.created_at_ms),
        // No priority (0) sorts after Low (4), as in the list.
        "priority" => {
            let rank = |record: &Record| match record.fields.priority {
                0 => 5,
                value => value,
            };
            rank(a).cmp(&rank(b))
        }
        "title" => a
            .fields
            .title
            .to_lowercase()
            .cmp(&b.fields.title.to_lowercase()),
        "due" => match (a.fields.due_at_ms, b.fields.due_at_ms) {
            (Some(a), Some(b)) => a.cmp(&b),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        },
        _ => b.updated_at_ms.cmp(&a.updated_at_ms),
    };
    order.then_with(|| a.id.cmp(&b.id))
}

fn card(record: &Record) -> Card {
    let fields = &record.fields;
    Card {
        id: record.id.clone(),
        head: record.head.clone(),
        title: fields.title.clone(),
        status: fields.status.clone(),
        priority: fields.priority,
        team: fields.team.clone(),
        assignees: fields.assignees.clone(),
        labels: fields.labels.clone(),
        due_at_ms: fields.due_at_ms,
        updated_at_ms: record.updated_at_ms,
    }
}

fn bounded(rows: &[&Record]) -> Vec<Card> {
    rows.iter()
        .take(MAX_BOARD_CARDS)
        .map(|row| card(row))
        .collect()
}

// r[impl kiln.work.kanban.projection] r[impl kiln.work.kanban.columns]
/// Project the board for one principal. `view` must already be admitted as a
/// view visible to `principal`; `records` is the whole admitted snapshot.
pub fn project(
    records: &BTreeMap<String, Record>,
    principal: &str,
    view: Option<&Record>,
    query: &Query,
    object_limit: usize,
) -> Projection {
    let saved = view.and_then(|record| record.fields.view.as_ref());
    let mut filters = saved.map(|view| view.filters.clone()).unwrap_or_default();
    for (key, value) in &query.overrides {
        filters.insert(key.clone(), value.clone());
    }
    filters.retain(|_, value| !value.is_empty());
    let sort = query
        .sort
        .clone()
        .or_else(|| saved.map(|view| view.sort.clone()))
        .unwrap_or_else(|| "updated".into());
    let (columns, columns_truncated) = columns(
        saved.and_then(|view| view.board.as_ref()),
        filters.get("team").map(String::as_str),
        records,
    );
    let mut issues = Vec::new();
    let mut list_only = 0;
    for record in records
        .values()
        .filter(|record| model::visible(record, principal))
        .filter(|record| listed(record, &filters, view.is_some()))
    {
        if record.kind == Kind::Issue {
            issues.push(record);
        } else {
            list_only += 1;
        }
    }
    issues.sort_by(|a, b| compare(&sort, a, b));
    let index: BTreeMap<&str, usize> = columns
        .iter()
        .enumerate()
        .map(|(position, column)| (column.status.as_str(), position))
        .collect();
    let mut grouped: Vec<Vec<&Record>> = vec![Vec::new(); columns.len()];
    let mut unplaced = Vec::new();
    let mut unknown: BTreeMap<&str, usize> = BTreeMap::new();
    for issue in &issues {
        match index
            .get(issue.fields.status.as_str())
            .and_then(|position| grouped.get_mut(*position))
        {
            Some(rows) => rows.push(issue),
            None => {
                unplaced.push(*issue);
                *unknown.entry(issue.fields.status.as_str()).or_default() += 1;
            }
        }
    }
    let lanes: Vec<Lane> = columns
        .into_iter()
        .zip(grouped)
        .map(|(column, rows)| Lane {
            column,
            total: rows.len(),
            cards: bounded(&rows),
        })
        .collect();
    let other = Other {
        statuses: unknown
            .into_iter()
            .map(|(status, total)| StatusCount {
                status: status.to_owned(),
                total,
            })
            .collect(),
        total: unplaced.len(),
        cards: bounded(&unplaced),
    };
    let shown_cards = lanes.iter().map(|lane| lane.cards.len()).sum::<usize>() + other.cards.len();
    Projection {
        schema: "kiln.work.board.v1",
        view: view.map(|record| ViewRef {
            id: record.id.clone(),
            head: record.head.clone(),
            title: record.fields.title.clone(),
            shared: saved.is_some_and(|view| view.shared),
            configured_columns: saved.is_some_and(|view| view.board.is_some()),
        }),
        filtered: !filters.is_empty(),
        filters,
        sort,
        columns: lanes,
        other,
        columns_truncated,
        matched_issues: issues.len(),
        shown_cards,
        list_only,
        snapshot_objects: records.len(),
        object_limit,
        card_limit: MAX_BOARD_CARDS,
        complete: shown_cards == issues.len(),
        bounded: true,
    }
}

#[cfg(test)]
#[path = "board_tests.rs"]
mod tests;
