use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const MAX_ID: usize = 256;
const MAX_TITLE: usize = 256;
const MAX_DESCRIPTION: usize = 32_768;
const MAX_BODY: usize = 16_384;
const MAX_LABEL: usize = 64;
const MAX_STATUS: usize = 64;
const MAX_SOURCE_URL: usize = 2_048;
const MAX_EMOJI: usize = 32;
const MAX_LIST: usize = 64;
const MAX_PERSONAL: usize = 256;
const MAX_MESSAGES: usize = 1_024;
const MAX_REACTIONS: usize = 32;
const MAX_TIMESTAMP: i64 = 253_402_300_799_999;
const STATUSES: &[&str] = &[
    "triage",
    "backlog",
    "started",
    "completed",
    "cancelled",
    "snoozed",
    "duplicate",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Issue,
    Project,
    Milestone,
    Cycle,
    Initiative,
    Team,
    Request,
    View,
    Personal,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Fields {
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: u8,
    pub owner: String,
    pub assignees: Vec<String>,
    pub labels: Vec<String>,
    pub parent: Option<String>,
    pub blocked_by: Vec<String>,
    pub team: Option<String>,
    pub project: Option<String>,
    pub milestone: Option<String>,
    pub cycle: Option<String>,
    pub initiative: Option<String>,
    pub start_at_ms: Option<i64>,
    pub due_at_ms: Option<i64>,
    pub customer: Option<String>,
    pub source_url: Option<String>,
    pub duplicate_of: Option<String>,
    pub links: Vec<Link>,
    pub workflow: Vec<String>,
    pub view: Option<View>,
    pub personal: Option<Personal>,
}

impl Default for Fields {
    fn default() -> Self {
        Self {
            title: String::new(),
            description: String::new(),
            status: "backlog".into(),
            priority: 0,
            owner: String::new(),
            assignees: Vec::new(),
            labels: Vec::new(),
            parent: None,
            blocked_by: Vec::new(),
            team: None,
            project: None,
            milestone: None,
            cycle: None,
            initiative: None,
            start_at_ms: None,
            due_at_ms: None,
            customer: None,
            source_url: None,
            duplicate_of: None,
            links: Vec::new(),
            workflow: Vec::new(),
            view: None,
            personal: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub kind: String,
    pub id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub filters: BTreeMap<String, String>,
    pub sort: String,
    pub group_by: String,
    pub layout: String,
    pub shared: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Personal {
    pub watched: Vec<String>,
    pub read: BTreeMap<String, String>,
    pub favorites: Vec<String>,
    pub reminders: BTreeMap<String, i64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub id: String,
    pub author: String,
    pub body: String,
    pub mentions: Vec<String>,
    pub reactions: BTreeMap<String, Vec<String>>,
    pub recorded_at_ms: i64,
    pub edited_at_ms: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub id: String,
    pub head: String,
    pub kind: Kind,
    pub fields: Fields,
    pub comments: Vec<Message>,
    pub updates: Vec<Message>,
    pub created_by: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub request_id: String,
    pub object_id: Option<String>,
    pub expected_head: Option<String>,
    pub action: Action,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Create {
        kind: Kind,
        fields: Fields,
    },
    Edit {
        fields: Fields,
    },
    Comment {
        body: String,
        mentions: Vec<String>,
    },
    EditComment {
        comment: String,
        body: String,
        mentions: Vec<String>,
    },
    React {
        comment: String,
        emoji: String,
        active: bool,
    },
    Update {
        body: String,
        mentions: Vec<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub actor: String,
    pub recorded_at_ms: i64,
    pub command: Command,
}

// r[impl kiln.work.authority] r[impl kiln.work.personal]
pub fn visible(record: &Record, principal: &str) -> bool {
    if record.kind == Kind::Personal {
        return !principal.is_empty() && record.fields.owner == principal;
    }
    if record.kind == Kind::View {
        return record.fields.view.as_ref().is_some_and(|view| view.shared)
            || (!principal.is_empty() && record.fields.owner == principal);
    }
    true
}

// r[impl kiln.work.issues] r[impl kiln.work.planning] r[impl kiln.work.discussion]
// r[impl kiln.work.triage] r[impl kiln.work.authority]
/// Decide one admitted operation. Native object IDs, heads, signatures, replay-key
/// deduplication and dependency snapshots belong to the storage shell. This reducer
/// preserves the predecessor head; it never invents a native identity.
pub fn apply(
    current: Option<&Record>,
    operation: &Operation,
    records: &BTreeMap<String, Record>,
) -> Result<Record, String> {
    identity(&operation.actor, "actor")?;
    identity(&operation.command.request_id, "request_id")?;
    timestamp(operation.recorded_at_ms, "recorded_at_ms")?;
    let command = &operation.command;
    let mut next = match (&command.action, current) {
        (Action::Create { kind, fields }, None) => {
            if command.object_id.is_some() || command.expected_head.is_some() {
                return Err("create must not supply object_id or expected_head".into());
            }
            let mut fields = fields.clone();
            if matches!(kind, Kind::Personal | Kind::View) && fields.owner.is_empty() {
                fields.owner.clone_from(&operation.actor);
            }
            if matches!(kind, Kind::Personal | Kind::View) && fields.owner != operation.actor {
                return Err("new personal and view records must belong to their creator".into());
            }
            Record {
                id: String::new(),
                head: String::new(),
                kind: *kind,
                fields,
                comments: Vec::new(),
                updates: Vec::new(),
                created_by: operation.actor.clone(),
                created_at_ms: operation.recorded_at_ms,
                updated_at_ms: operation.recorded_at_ms,
            }
        }
        (Action::Create { .. }, Some(_)) => return Err("object already exists".into()),
        (_, None) => return Err("object not found".into()),
        (_, Some(record)) => {
            if !visible(record, &operation.actor) {
                return Err("object not found".into());
            }
            identity(&record.id, "object_id")?;
            identity(&record.head, "expected_head")?;
            if command.object_id.as_deref() != Some(record.id.as_str())
                || command.expected_head.as_deref() != Some(record.head.as_str())
            {
                return Err("object_id and expected_head must match current object".into());
            }
            if operation.recorded_at_ms < record.updated_at_ms {
                return Err("operation timestamp precedes current state".into());
            }
            let mut next = record.clone();
            next.updated_at_ms = operation.recorded_at_ms;
            next
        }
    };
    match &command.action {
        Action::Create { .. } => {}
        Action::Edit { fields } => {
            if next.kind == Kind::Request
                && (fields.customer != next.fields.customer
                    || fields.source_url != next.fields.source_url)
            {
                return Err("original request customer and source are immutable".into());
            }
            if matches!(next.kind, Kind::Personal | Kind::View) {
                if fields.owner != next.fields.owner {
                    return Err("personal and view ownership is immutable".into());
                }
                if next.kind == Kind::View
                    && fields.view.as_ref().map(|view| view.shared)
                        != next.fields.view.as_ref().map(|view| view.shared)
                {
                    return Err("view visibility is immutable; create a separate view".into());
                }
            }
            next.fields = fields.clone();
        }
        Action::Comment { body, mentions } | Action::Update { body, mentions } => {
            message_fields(body, mentions)?;
            let id = format!("message:{}", command.request_id);
            if next
                .comments
                .iter()
                .chain(&next.updates)
                .any(|message| message.id == id)
            {
                return Err("message request already applied".into());
            }
            let stream = if matches!(&command.action, Action::Comment { .. }) {
                &mut next.comments
            } else {
                &mut next.updates
            };
            if stream.len() >= MAX_MESSAGES {
                return Err("discussion stream limit exceeded".into());
            }
            stream.push(Message {
                id,
                author: operation.actor.clone(),
                body: body.clone(),
                mentions: mentions.clone(),
                reactions: BTreeMap::new(),
                recorded_at_ms: operation.recorded_at_ms,
                edited_at_ms: None,
            });
        }
        Action::EditComment {
            comment,
            body,
            mentions,
        } => {
            message_fields(body, mentions)?;
            let message = find_message(&mut next, comment)?;
            if message.author != operation.actor {
                return Err("only the message author may edit it".into());
            }
            message.body.clone_from(body);
            message.mentions.clone_from(mentions);
            message.edited_at_ms = Some(operation.recorded_at_ms);
        }
        Action::React {
            comment,
            emoji,
            active,
        } => {
            text(emoji, "emoji", MAX_EMOJI, false, false)?;
            if emoji.chars().any(char::is_whitespace) {
                return Err("emoji must not contain whitespace".into());
            }
            let message = find_message(&mut next, comment)?;
            if *active {
                if !message.reactions.contains_key(emoji)
                    && message.reactions.len() >= MAX_REACTIONS
                {
                    return Err("reaction limit exceeded".into());
                }
                let actors = message.reactions.entry(emoji.clone()).or_default();
                if !actors.contains(&operation.actor) {
                    if actors.len() >= MAX_LIST {
                        return Err("reaction actor limit exceeded".into());
                    }
                    actors.push(operation.actor.clone());
                    actors.sort();
                }
            } else if let Some(actors) = message.reactions.get_mut(emoji) {
                actors.retain(|actor| actor != &operation.actor);
                if actors.is_empty() {
                    message.reactions.remove(emoji);
                }
            }
        }
    }
    validate_fields(&next, records, &operation.actor)?;
    validate_transition(current, &next, operation, records)?;
    // Revalidate inbound references too: changing a team workflow or moving a
    // milestone must not silently invalidate another already admitted object.
    if !next.id.is_empty() && matches!(&command.action, Action::Edit { .. }) {
        let graph = Graph {
            candidate: &next,
            records,
        };
        for record in records.values().filter(|record| record.id != next.id) {
            if references(record, &next.id) {
                validate_relations(record, &graph, &record.created_by)?;
                validate_status(record, &graph)?;
                if let Some(view) = &record.fields.view {
                    validate_view(record, view, &graph, &record.created_by)?;
                }
            }
        }
    }
    Ok(next)
}

fn find_message<'a>(record: &'a mut Record, id: &str) -> Result<&'a mut Message, String> {
    // Message IDs include the bounded request token and a fixed prefix.
    text(id, "message id", MAX_ID + "message:".len(), false, false)?;
    record
        .comments
        .iter_mut()
        .chain(&mut record.updates)
        .find(|message| message.id == id)
        .ok_or_else(|| "message not found".into())
}

fn text(value: &str, name: &str, max: usize, empty: bool, multiline: bool) -> Result<(), String> {
    if value.len() > max
        || (!empty && value.trim().is_empty())
        || value
            .chars()
            .any(|c| c.is_control() && !(multiline && matches!(c, '\n' | '\r' | '\t')))
    {
        return Err(format!("invalid {name}: maximum {max} bytes"));
    }
    Ok(())
}

fn identity(value: &str, name: &str) -> Result<(), String> {
    text(value, name, MAX_ID, false, false)?;
    if value.chars().any(char::is_whitespace) {
        return Err(format!("invalid {name}: whitespace is not allowed"));
    }
    Ok(())
}

fn timestamp(value: i64, name: &str) -> Result<(), String> {
    if !(0..=MAX_TIMESTAMP).contains(&value) {
        return Err(format!("invalid {name}"));
    }
    Ok(())
}

fn identities(values: &[String], name: &str, max: usize) -> Result<(), String> {
    if values.len() > max {
        return Err(format!("{name} limit exceeded"));
    }
    let mut seen = BTreeSet::new();
    for value in values {
        identity(value, name)?;
        if !seen.insert(value) {
            return Err(format!("duplicate {name}"));
        }
    }
    Ok(())
}

fn message_fields(body: &str, mentions: &[String]) -> Result<(), String> {
    text(body, "message body", MAX_BODY, false, true)?;
    identities(mentions, "mention", MAX_LIST)
}

// r[impl kiln.work.change] r[impl kiln.work.issues] r[impl kiln.work.planning]
fn validate_fields(
    record: &Record,
    records: &BTreeMap<String, Record>,
    actor: &str,
) -> Result<(), String> {
    let fields = &record.fields;
    text(&fields.title, "title", MAX_TITLE, false, false)?;
    text(
        &fields.description,
        "description",
        MAX_DESCRIPTION,
        true,
        true,
    )?;
    text(&fields.status, "status", MAX_STATUS, false, false)?;
    if fields.priority > 4 {
        return Err("priority must be in 0..=4".into());
    }
    if !fields.owner.is_empty() {
        identity(&fields.owner, "owner")?;
    }
    identities(&fields.assignees, "assignee", MAX_LIST)?;
    if fields.labels.len() > MAX_LIST {
        return Err("label limit exceeded".into());
    }
    let mut labels = BTreeSet::new();
    for label in &fields.labels {
        text(label, "label", MAX_LABEL, false, false)?;
        if !labels.insert(label) {
            return Err("duplicate label".into());
        }
    }
    identities(&fields.blocked_by, "blocker", MAX_LIST)?;
    for (value, name) in [
        (fields.start_at_ms, "start_at_ms"),
        (fields.due_at_ms, "due_at_ms"),
    ] {
        if let Some(value) = value {
            timestamp(value, name)?;
        }
    }
    if matches!((fields.start_at_ms, fields.due_at_ms), (Some(start), Some(due)) if start > due) {
        return Err("start must not follow due date".into());
    }
    if let Some(customer) = &fields.customer {
        text(customer, "customer", MAX_TITLE, false, false)?;
    }
    if let Some(url) = &fields.source_url {
        text(url, "source_url", MAX_SOURCE_URL, false, false)?;
        let rest = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .ok_or_else(|| "source_url must use http or https".to_string())?;
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        if authority.is_empty() || rest.chars().any(|c| c.is_whitespace() || c == '\\') {
            return Err("invalid source_url".into());
        }
    }
    if fields.links.len() > MAX_LIST {
        return Err("link limit exceeded".into());
    }
    let mut links = BTreeSet::new();
    for link in &fields.links {
        if ![
            "source", "revision", "patch", "review", "run", "artifact", "cairn",
        ]
        .contains(&link.kind.as_str())
        {
            return Err("unsupported link kind".into());
        }
        identity(&link.id, "link id")?;
        if !links.insert((&link.kind, &link.id)) {
            return Err("duplicate link".into());
        }
    }
    if fields.workflow.len() > MAX_LIST {
        return Err("workflow limit exceeded".into());
    }
    let mut workflow = BTreeSet::new();
    for status in &fields.workflow {
        text(status, "workflow status", MAX_STATUS, false, false)?;
        if !workflow.insert(status) {
            return Err("duplicate workflow status".into());
        }
    }
    if record.kind != Kind::Team && !fields.workflow.is_empty() {
        return Err("only teams define workflows".into());
    }
    if (record.kind == Kind::View) != fields.view.is_some()
        || (record.kind == Kind::Personal) != fields.personal.is_some()
    {
        return Err("view and personal payloads must match their object kind".into());
    }
    if matches!(record.kind, Kind::View | Kind::Personal) && fields.owner.is_empty() {
        return Err("personal and view records require an owner".into());
    }
    if (record.kind == Kind::Personal || fields.view.as_ref().is_some_and(|view| !view.shared))
        && fields.owner != actor
    {
        return Err("private object belongs to another principal".into());
    }
    let graph = Graph {
        candidate: record,
        records,
    };
    validate_relations(record, &graph, actor)?;
    validate_status(record, &graph)?;
    if let Some(view) = &fields.view {
        validate_view(record, view, &graph, actor)?;
    }
    if let Some(personal) = &fields.personal {
        validate_personal(record, personal, &graph, actor)?;
    }
    Ok(())
}

struct Graph<'a> {
    candidate: &'a Record,
    records: &'a BTreeMap<String, Record>,
}

impl<'a> Graph<'a> {
    fn get(&self, id: &str) -> Option<&'a Record> {
        if !self.candidate.id.is_empty() && id == self.candidate.id {
            Some(self.candidate)
        } else {
            self.records.get(id)
        }
    }

    fn target(&self, source: &Record, id: &str, actor: &str) -> Result<&'a Record, String> {
        identity(id, "reference")?;
        if !source.id.is_empty() && source.id == id {
            return Err("self reference is not allowed".into());
        }
        let target = self
            .get(id)
            .ok_or_else(|| "reference not found".to_string())?;
        if !visible(target, actor) {
            return Err("reference not found".into());
        }
        if is_public(source) && !is_public(target) {
            return Err("public objects cannot reference private objects".into());
        }
        Ok(target)
    }
}

fn is_public(record: &Record) -> bool {
    record.kind != Kind::Personal
        && (record.kind != Kind::View
            || record.fields.view.as_ref().is_some_and(|view| view.shared))
}

fn validate_relations(record: &Record, graph: &Graph<'_>, actor: &str) -> Result<(), String> {
    let fields = &record.fields;
    for (id, kind, name) in [
        (&fields.team, Kind::Team, "team"),
        (&fields.project, Kind::Project, "project"),
        (&fields.milestone, Kind::Milestone, "milestone"),
        (&fields.cycle, Kind::Cycle, "cycle"),
        (&fields.initiative, Kind::Initiative, "initiative"),
    ] {
        if let Some(id) = id {
            if graph.target(record, id, actor)?.kind != kind {
                return Err(format!("invalid {name} reference kind"));
            }
        }
    }
    if let Some(parent) = &fields.parent {
        if matches!(record.kind, Kind::Personal | Kind::View)
            || graph.target(record, parent, actor)?.kind != record.kind
        {
            return Err("parent must have the same work kind".into());
        }
    }
    for blocker in &fields.blocked_by {
        let target = graph.target(record, blocker, actor)?;
        if matches!(record.kind, Kind::Personal | Kind::View | Kind::Team)
            || matches!(target.kind, Kind::Personal | Kind::View | Kind::Team)
        {
            return Err("blockers must be work or planning objects".into());
        }
    }
    if let Some(duplicate) = &fields.duplicate_of {
        if !matches!(record.kind, Kind::Issue | Kind::Request)
            || !matches!(
                graph.target(record, duplicate, actor)?.kind,
                Kind::Issue | Kind::Request
            )
        {
            return Err("duplicate references must link issues or requests".into());
        }
    }
    if (fields.status == "duplicate") != fields.duplicate_of.is_some() {
        return Err("duplicate status requires exactly one duplicate target".into());
    }
    // A selected milestone/cycle belongs to the same project/team when both
    // sides declare that scope; omitted scope is not an implicit membership.
    for id in [&fields.milestone, &fields.cycle].into_iter().flatten() {
        let target = graph.target(record, id, actor)?;
        for (left, right) in [
            (&fields.project, &target.fields.project),
            (&fields.team, &target.fields.team),
        ] {
            if matches!((left, right), (Some(left), Some(right)) if left != right) {
                return Err("planning reference has incompatible project or team".into());
            }
        }
    }
    for edge in [Edge::Parent, Edge::Blocker, Edge::Duplicate] {
        acyclic(record, graph, edge)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Edge {
    Parent,
    Blocker,
    Duplicate,
}

fn edges(record: &Record, edge: Edge) -> impl Iterator<Item = &String> {
    let fields = &record.fields;
    let optional = match edge {
        Edge::Parent => fields.parent.as_ref(),
        Edge::Duplicate => fields.duplicate_of.as_ref(),
        Edge::Blocker => None,
    };
    optional.into_iter().chain(
        fields
            .blocked_by
            .iter()
            .filter(move |_| matches!(edge, Edge::Blocker)),
    )
}

fn acyclic(record: &Record, graph: &Graph<'_>, edge: Edge) -> Result<(), String> {
    let mut visiting = BTreeSet::new();
    let mut done = BTreeSet::new();
    // Iterative DFS avoids call-stack growth on a valid but long native graph.
    let mut stack = vec![(record, false)];
    while let Some((node, leaving)) = stack.pop() {
        let key = node.id.as_str();
        if leaving {
            visiting.remove(key);
            done.insert(key);
            continue;
        }
        if done.contains(key) {
            continue;
        }
        if !visiting.insert(key) {
            return Err("relation cycle is not allowed".into());
        }
        stack.push((node, true));
        for id in edges(node, edge) {
            let target = graph
                .get(id)
                .ok_or_else(|| "reference not found".to_string())?;
            stack.push((target, false));
        }
    }
    Ok(())
}

// r[impl kiln.work.issues] r[impl kiln.work.triage]
fn validate_status(record: &Record, graph: &Graph<'_>) -> Result<(), String> {
    let fields = &record.fields;
    let team = fields.team.as_deref().and_then(|id| graph.get(id));
    if !(STATUSES.contains(&fields.status.as_str())
        || team.is_some_and(|team| team.fields.workflow.contains(&fields.status))
        || record.kind == Kind::Team && fields.workflow.contains(&fields.status))
    {
        return Err("status is not built-in or defined by the selected team workflow".into());
    }
    if fields.status == "snoozed" && fields.due_at_ms.is_none() {
        return Err("snoozed status requires due_at_ms".into());
    }
    Ok(())
}

// r[impl kiln.work.triage] r[impl kiln.work.personal]
fn validate_transition(
    current: Option<&Record>,
    next: &Record,
    operation: &Operation,
    records: &BTreeMap<String, Record>,
) -> Result<(), String> {
    if next.fields.status == "snoozed"
        && current.is_none_or(|old| {
            old.fields.status != "snoozed" || old.fields.due_at_ms != next.fields.due_at_ms
        })
        && next
            .fields
            .due_at_ms
            .is_none_or(|due| due <= operation.recorded_at_ms)
    {
        return Err("new snooze must end in the future".into());
    }
    if let Some(personal) = &next.fields.personal {
        let previous = current.and_then(|record| record.fields.personal.as_ref());
        for (id, head) in &personal.read {
            if previous.and_then(|personal| personal.read.get(id)) != Some(head)
                && records.get(id).is_none_or(|target| target.head != *head)
            {
                return Err("new read marker must match the target's current head".into());
            }
        }
        for (id, due) in &personal.reminders {
            if previous.and_then(|personal| personal.reminders.get(id)) != Some(due)
                && *due <= operation.recorded_at_ms
            {
                return Err("new reminder must be in the future".into());
            }
        }
    }
    Ok(())
}

// r[impl kiln.work.personal] r[impl kiln.work.views]
fn validate_personal(
    record: &Record,
    personal: &Personal,
    graph: &Graph<'_>,
    actor: &str,
) -> Result<(), String> {
    identities(&personal.watched, "watched", MAX_PERSONAL)?;
    identities(&personal.favorites, "favorite", MAX_PERSONAL)?;
    if personal.read.len() > MAX_PERSONAL || personal.reminders.len() > MAX_PERSONAL {
        return Err("personal collection limit exceeded".into());
    }
    for id in personal
        .watched
        .iter()
        .chain(&personal.favorites)
        .chain(personal.read.keys())
        .chain(personal.reminders.keys())
    {
        graph.target(record, id, actor)?;
    }
    for head in personal.read.values() {
        identity(head, "read head")?;
    }
    for due in personal.reminders.values() {
        timestamp(*due, "reminder")?;
    }
    Ok(())
}

// r[impl kiln.work.views]
fn validate_view(
    record: &Record,
    view: &View,
    graph: &Graph<'_>,
    actor: &str,
) -> Result<(), String> {
    if !["list", "board"].contains(&view.layout.as_str())
        || !["updated", "created", "priority", "title", "due"].contains(&view.sort.as_str())
        || !["status", "assignee", "project", "priority", "team", "none"]
            .contains(&view.group_by.as_str())
    {
        return Err("unsupported view layout, sort or grouping".into());
    }
    for (key, value) in &view.filters {
        match key.as_str() {
            "kind" => {
                if ![
                    "issue",
                    "project",
                    "milestone",
                    "cycle",
                    "initiative",
                    "team",
                    "request",
                    "view",
                    "personal",
                ]
                .contains(&value.as_str())
                {
                    return Err("invalid kind filter".into());
                }
            }
            "status" => {
                text(value, "status filter", MAX_STATUS, false, false)?;
                if !STATUSES.contains(&value.as_str()) {
                    let team = view.filters.get("team").and_then(|id| graph.get(id));
                    if !team.is_some_and(|team| {
                        team.kind == Kind::Team && team.fields.workflow.contains(value)
                    }) {
                        return Err("custom status filter requires its team filter".into());
                    }
                }
            }
            "assignee" => identity(value, "assignee filter")?,
            "priority" => {
                if !["0", "1", "2", "3", "4"].contains(&value.as_str()) {
                    return Err("invalid priority filter".into());
                }
            }
            "label" => text(value, "label filter", MAX_LABEL, false, false)?,
            "team" | "project" | "cycle" | "initiative" => {
                let target = graph.target(record, value, actor)?;
                let expected = match key.as_str() {
                    "team" => Kind::Team,
                    "project" => Kind::Project,
                    "cycle" => Kind::Cycle,
                    _ => Kind::Initiative,
                };
                if target.kind != expected {
                    return Err("invalid view filter reference kind".into());
                }
            }
            "q" => text(value, "query filter", MAX_TITLE, false, false)?,
            _ => return Err("unsupported view filter".into()),
        }
    }
    Ok(())
}

fn references(record: &Record, id: &str) -> bool {
    let fields = &record.fields;
    [
        &fields.parent,
        &fields.team,
        &fields.project,
        &fields.milestone,
        &fields.cycle,
        &fields.initiative,
        &fields.duplicate_of,
    ]
    .into_iter()
    .flatten()
    .any(|value| value == id)
        || fields.blocked_by.iter().any(|value| value == id)
        || fields.view.as_ref().is_some_and(|view| {
            view.filters.iter().any(|(key, value)| {
                ["team", "project", "cycle", "initiative"].contains(&key.as_str()) && value == id
            })
        })
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
