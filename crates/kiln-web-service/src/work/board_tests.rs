use super::super::model::Fields;
use super::*;
use serde::Deserialize;
use serde_json::Value;

const FIXTURE: &str = include_str!("../../tests/fixtures/work-board/eligibility.json");

#[derive(Deserialize)]
struct Fixture {
    records: Vec<Record>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    view_active: bool,
    filters: BTreeMap<String, String>,
    issues: Vec<String>,
    list_only: usize,
}

fn fixture() -> (BTreeMap<String, Record>, Vec<Case>) {
    let fixture: Fixture = serde_json::from_str(FIXTURE)
        .unwrap_or_else(|error| unreachable!("fixture is invalid: {error}"));
    let records = fixture
        .records
        .into_iter()
        .map(|record| (record.id.clone(), record))
        .collect();
    (records, fixture.cases)
}

fn at<T>(items: &[T], index: usize) -> &T {
    items
        .get(index)
        .unwrap_or_else(|| unreachable!("missing item {index}"))
}

fn at_key(records: &BTreeMap<String, Record>, id: &str) -> Record {
    records
        .get(id)
        .cloned()
        .unwrap_or_else(|| unreachable!("missing record {id}"))
}

fn statuses(columns: &[Column]) -> Vec<&str> {
    columns
        .iter()
        .map(|column| column.status.as_str())
        .collect()
}

fn query(view: Option<&str>, overrides: &[(&str, &str)], sort: Option<&str>) -> Query {
    Query {
        view: view.map(str::to_owned),
        overrides: overrides
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect(),
        sort: sort.map(str::to_owned),
    }
}

fn lane<'a>(projection: &'a Projection, status: &str) -> &'a Lane {
    projection
        .columns
        .iter()
        .find(|lane| lane.column.status == status)
        .unwrap_or_else(|| unreachable!("missing column {status}"))
}

fn ids(cards: &[Card]) -> Vec<&str> {
    cards.iter().map(|card| card.id.as_str()).collect()
}

// r[verify kiln.work.kanban.projection]
#[test]
fn list_and_board_eligibility_follow_the_shared_fixture() {
    let (records, cases) = fixture();
    for case in cases {
        let mut issues = Vec::new();
        let mut list_only = 0;
        for record in records
            .values()
            .filter(|record| listed(record, &case.filters, case.view_active))
        {
            if record.kind == Kind::Issue {
                issues.push(record.id.clone());
            } else {
                list_only += 1;
            }
        }
        assert_eq!(issues, case.issues, "{}", case.name);
        assert_eq!(list_only, case.list_only, "{}", case.name);
    }
}

// r[verify kiln.work.kanban.columns] r[verify kiln.work.kanban.config.scenario.migration]
#[test]
fn default_columns_are_ordered_and_mixed_team_columns_name_their_teams() {
    let (records, _) = fixture();
    let (mixed, truncated) = columns(None, None, &records);
    assert_eq!(truncated, 0);
    assert_eq!(
        statuses(&mixed),
        [
            "triage",
            "backlog",
            "started",
            "review",
            "qa",
            "deploying",
            "completed",
            "cancelled",
            "snoozed",
            "duplicate"
        ]
    );
    let review = at(&mixed, 3);
    assert!(!review.builtin && !review.retired);
    assert_eq!(review.teams, ["team-a", "team-b"]);
    assert_eq!(at(&mixed, 5).teams, ["team-b"]);
    assert!(at(&mixed, 0).builtin && at(&mixed, 0).teams.is_empty());
    // A team-scoped view only adds that team's statuses.
    let (scoped, _) = columns(None, Some("team-b"), &records);
    assert_eq!(
        statuses(&scoped).get(3..5),
        Some(&["review", "deploying"][..]),
        "team scope keeps its workflow order"
    );
    assert_eq!(scoped.len(), 9);
}

// r[verify kiln.work.kanban.columns]
#[test]
fn configured_columns_keep_order_and_mark_retired_statuses() {
    let (records, _) = fixture();
    let board = Board {
        columns: vec!["started".into(), "gone".into(), "review".into()],
        lane_by: Default::default(),
    };
    let (configured, truncated) = columns(Some(&board), None, &records);
    assert_eq!(truncated, 0);
    assert_eq!(statuses(&configured), ["started", "gone", "review"]);
    assert!(at(&configured, 1).retired && !at(&configured, 1).builtin);
    assert!(!at(&configured, 2).retired);
}

#[test]
fn default_team_columns_are_bounded_and_overflow_is_reported() {
    let (mut records, _) = fixture();
    let mut wide = at_key(&records, "team-a");
    wide.id = "team-wide".into();
    wide.fields.title = "Wide".into();
    wide.fields.workflow = (0..MAX_BOARD_COLUMNS).map(|i| format!("s{i}")).collect();
    records.insert(wide.id.clone(), wide);
    let (bounded, truncated) = columns(None, None, &records);
    assert_eq!(bounded.len(), MAX_BOARD_COLUMNS);
    // review, qa and deploying plus 64 wide statuses compete for 57 slots.
    assert_eq!(truncated, 3 + MAX_BOARD_COLUMNS - (MAX_BOARD_COLUMNS - 7));
    assert_eq!(statuses(&bounded).last(), Some(&"duplicate"));
}

// r[verify kiln.work.kanban.columns.scenario.unknown] r[verify kiln.work.kanban.projection]
#[test]
fn projection_keeps_empty_columns_and_never_hides_unconfigured_statuses() {
    let (mut records, _) = fixture();
    let projection = project(
        &records,
        "local:alice",
        records.get("view-shared"),
        &Query::default(),
        1024,
    );
    // The old shared view scopes to team-a: its two issues, all default columns.
    assert_eq!(projection.matched_issues, 2);
    assert_eq!(projection.columns.len(), 9);
    assert_eq!(ids(&lane(&projection, "backlog").cards), ["issue-1"]);
    assert_eq!(ids(&lane(&projection, "review").cards), ["issue-2"]);
    assert_eq!(lane(&projection, "triage").total, 0, "empty column remains");
    assert!(projection.complete && projection.filtered);
    assert_eq!(projection.other.total, 0);
    let view = projection.view.as_ref().map(|view| view.configured_columns);
    assert_eq!(view, Some(false));
    // Configure only two columns: the other matching issues stay visible.
    let shared = records.get_mut("view-private");
    if let Some(record) = shared {
        if let Some(view) = record.fields.view.as_mut() {
            view.board = Some(Board {
                columns: vec!["review".into(), "backlog".into()],
                lane_by: Default::default(),
            });
        }
    }
    let projection = project(
        &records,
        "local:alice",
        records.get("view-private"),
        &Query::default(),
        1024,
    );
    assert_eq!(statuses_of(&projection), ["review", "backlog"]);
    assert_eq!(projection.matched_issues, 6);
    assert_eq!(projection.other.total, 4);
    let unknown: Vec<(&str, usize)> = projection
        .other
        .statuses
        .iter()
        .map(|count| (count.status.as_str(), count.total))
        .collect();
    assert_eq!(
        unknown,
        [
            ("completed", 1),
            ("deploying", 1),
            ("snoozed", 1),
            ("started", 1)
        ]
    );
    assert_eq!(projection.shown_cards, 6);
    // Priority sort from the private view: Low (4) before no priority (0).
    assert_eq!(ids(&projection.other.cards).first(), Some(&"issue-5"));
}

fn statuses_of(projection: &Projection) -> Vec<&str> {
    projection
        .columns
        .iter()
        .map(|lane| lane.column.status.as_str())
        .collect()
}

// r[verify kiln.work.kanban.projection]
#[test]
fn projection_hides_private_records_applies_overrides_and_reports_bounds() {
    let (mut records, _) = fixture();
    let projection = project(
        &records,
        "local:bob",
        None,
        &query(None, &[("q", "acme")], None),
        1024,
    );
    assert_eq!(projection.matched_issues, 0);
    assert_eq!(
        projection.list_only, 1,
        "the request stays a list-only match"
    );
    // Overrides replace the saved view's filter key by key.
    let projection = project(
        &records,
        "local:alice",
        records.get("view-shared"),
        &query(None, &[("status", "review")], Some("title")),
        1024,
    );
    assert_eq!(projection.matched_issues, 1);
    assert_eq!(projection.sort, "title");
    assert_eq!(
        projection.filters.get("team").map(String::as_str),
        Some("team-a")
    );
    // More than MAX_BOARD_CARDS issues in one status: bounded, marked incomplete.
    let template = at_key(&records, "issue-4");
    for index in 0..=MAX_BOARD_CARDS {
        let mut issue = template.clone();
        issue.id = format!("bulk-{index:03}");
        records.insert(issue.id.clone(), issue);
    }
    let projection = project(&records, "local:bob", None, &Query::default(), 1024);
    let started = lane(&projection, "started");
    assert_eq!(started.total, MAX_BOARD_CARDS + 2);
    assert_eq!(started.cards.len(), MAX_BOARD_CARDS);
    assert!(!projection.complete);
    assert_eq!(projection.matched_issues, MAX_BOARD_CARDS + 7);
    assert_eq!(projection.shown_cards, MAX_BOARD_CARDS + 5);
    assert_eq!(projection.object_limit, 1024);
    // The serialized lane flattens its column facts beside totals and cards.
    let value: Value = serde_json::to_value(started)
        .unwrap_or_else(|error| unreachable!("lane serializes: {error}"));
    assert_eq!(value.get("status"), Some(&Value::from("started")));
    assert_eq!(value.get("builtin"), Some(&Value::from(true)));
    assert_eq!(value.get("total"), Some(&Value::from(MAX_BOARD_CARDS + 2)));
}

#[test]
fn board_query_admits_known_bounded_parameters_only() {
    let oid = "0123456789abcdef0123456789abcdef01234567";
    let params: BTreeMap<&str, &str> = [
        ("view", oid),
        ("q", "Needle%20case"),
        ("status", ""),
        ("sort", "due"),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        parse_query(&params),
        Ok(query(Some(oid), &[("q", "Needle case")], Some("due")))
    );
    let long = "x".repeat(MAX_QUERY_VALUE + 1);
    for (key, value) in [
        ("view", "not-an-oid"),
        ("sort", "random"),
        ("team", "team-a"),
        ("q", long.as_str()),
        ("q", "%0A"),
        ("q", "%zz"),
    ] {
        let params: BTreeMap<&str, &str> = [(key, value)].into_iter().collect();
        assert_eq!(parse_query(&params), Err(()), "{key}={value}");
    }
}

fn ci_project(id: &str, label: &str, repository: &str, key: &str) -> CiProjectFact {
    CiProjectFact {
        id: id.into(),
        label: label.into(),
        repository: repository.into(),
        repository_key: key.into(),
        scope_root: Some(format!("crates/{id}")),
    }
}

fn record_fields(title: &str) -> Fields {
    Fields {
        title: title.into(),
        ..Fields::default()
    }
}

fn linked(record: &mut Record, run: &str) {
    record.fields.links.push(super::super::model::Link {
        kind: "run".into(),
        id: run.into(),
    });
}

fn ci_view(records: &mut BTreeMap<String, Record>, lane_by: BoardLanes) {
    let mut view = at_key(records, "view-shared");
    if let Some(saved) = view.fields.view.as_mut() {
        saved.filters.clear();
        saved.layout = "board".into();
        saved.board = Some(Board {
            columns: Vec::new(),
            lane_by,
        });
    }
    view.id = "view-ci".into();
    view.fields.title = "Monorepo board".into();
    records.insert(view.id.clone(), view);
}

// r[verify kiln.work.monorepo.board] r[verify kiln.work.monorepo.facts] r[verify kiln.work.monorepo.board.scenario.lanes] r[verify kiln.work.monorepo.board.scenario.empty]
#[test]
fn ci_lanes_assign_each_issue_once_and_keep_status_moves() {
    let (mut records, _) = fixture();
    let mut first = at_key(&records, "issue-1");
    linked(&mut first, "run-a");
    records.insert(first.id.clone(), first);
    let mut second = at_key(&records, "issue-3");
    linked(&mut second, "run-a");
    linked(&mut second, "run-b");
    records.insert(second.id.clone(), second);
    let mut unresolved = at_key(&records, "issue-4");
    linked(&mut unresolved, "run-lost");
    records.insert(unresolved.id.clone(), unresolved);
    ci_view(&mut records, BoardLanes::CiProject);
    let ci = CiFacts::new(
        [
            ci_project("project-a", "Alpha", "/repo", &"a".repeat(64)),
            ci_project("project-b", "Beta", "/repo", &"a".repeat(64)),
        ],
        [
            ("run-a".to_owned(), "project-a".to_owned()),
            ("run-b".to_owned(), "project-b".to_owned()),
        ],
    );
    let projection = project_with_ci(
        &records,
        "local:bob",
        records.get("view-ci"),
        &Query::default(),
        1024,
        &ci,
    );
    assert_eq!(projection.lane_by, "ci_project");
    assert_eq!(statuses_of(&projection), ["project-a", "project-b"]);
    // issue-3 links both projects but appears once, in the first lane.
    assert_eq!(
        ids(&lane(&projection, "project-a").cards),
        ["issue-1", "issue-3"]
    );
    assert_eq!(
        lane(&projection, "project-b").total,
        0,
        "empty lane remains"
    );
    // Unresolved and unlinked issues are never hidden or assigned to a project.
    let other = ids(&projection.other.cards);
    assert!(other.contains(&"issue-4"));
    assert!(other.contains(&"issue-2"));
    assert_eq!(
        projection
            .other
            .statuses
            .iter()
            .map(|count| count.status.as_str())
            .collect::<BTreeSet<_>>(),
        ["completed", "review", "snoozed", "started"]
            .into_iter()
            .collect::<BTreeSet<_>>()
    );
    // Move keeps target status columns even though visible lanes are projects.
    assert_eq!(
        projection
            .move_columns
            .iter()
            .map(|column| column.status.as_str())
            .collect::<Vec<_>>(),
        [
            "triage",
            "backlog",
            "started",
            "review",
            "qa",
            "deploying",
            "completed",
            "cancelled",
            "snoozed",
            "duplicate"
        ]
    );
    assert!(
        projection
            .move_columns
            .iter()
            .all(|column| column.ci.is_none())
    );
    assert_eq!(projection.shown_cards, projection.matched_issues);
}

// r[verify kiln.work.monorepo.filter] r[verify kiln.work.monorepo.filter.scenario.project] r[verify kiln.work.monorepo.filter.scenario.repository]
#[test]
fn ci_filters_match_only_resolved_exact_projects_and_repositories() {
    let (mut records, _) = fixture();
    let mut first = at_key(&records, "issue-1");
    linked(&mut first, "run-a");
    records.insert(first.id.clone(), first);
    let mut second = at_key(&records, "issue-3");
    linked(&mut second, "run-b");
    records.insert(second.id.clone(), second);
    let mut unresolved = at_key(&records, "issue-2");
    linked(&mut unresolved, "run-lost");
    records.insert(unresolved.id.clone(), unresolved);
    let ci = CiFacts::new(
        [
            ci_project("project-a", "Alpha", "/repo", &"a".repeat(64)),
            ci_project("project-b", "Beta", "/repo", &"a".repeat(64)),
        ],
        [
            ("run-a".to_owned(), "project-a".to_owned()),
            ("run-b".to_owned(), "project-b".to_owned()),
        ],
    );
    let filters = BTreeMap::from([("ci_project".to_owned(), "project-a".to_owned())]);
    assert!(matches_with_ci(&at_key(&records, "issue-1"), &filters, &ci));
    assert!(!matches_with_ci(
        &at_key(&records, "issue-3"),
        &filters,
        &ci
    ));
    assert!(!matches_with_ci(
        &at_key(&records, "issue-2"),
        &filters,
        &ci
    ));
    let repository = BTreeMap::from([("ci_repository".to_owned(), "a".repeat(64))]);
    assert!(matches_with_ci(
        &at_key(&records, "issue-1"),
        &repository,
        &ci
    ));
    assert!(matches_with_ci(
        &at_key(&records, "issue-3"),
        &repository,
        &ci
    ));
    assert!(!matches_with_ci(
        &at_key(&records, "issue-2"),
        &repository,
        &ci
    ));
    // A view filtered to one project projects only that project's lane.
    ci_view(&mut records, BoardLanes::CiProject);
    if let Some(view) = records
        .get_mut("view-ci")
        .and_then(|view| view.fields.view.as_mut())
    {
        view.filters = filters.clone();
    }
    let projection = project_with_ci(
        &records,
        "local:bob",
        records.get("view-ci"),
        &Query::default(),
        1024,
        &ci,
    );
    assert_eq!(statuses_of(&projection), ["project-a"]);
    assert_eq!(ids(&lane(&projection, "project-a").cards), ["issue-1"]);
}

// r[verify kiln.work.monorepo.planning] r[verify kiln.work.monorepo.planning.scenario.set]
#[test]
fn planning_project_sets_match_any_member() {
    let mut records: BTreeMap<String, Record> = BTreeMap::new();
    for id in ["project-a", "project-b", "project-c"] {
        let mut record = Record {
            id: id.into(),
            head: format!("head-{id}"),
            kind: Kind::Project,
            fields: record_fields(id),
            comments: Vec::new(),
            updates: Vec::new(),
            created_by: "local:alice".into(),
            created_at_ms: 1,
            updated_at_ms: 1,
        };
        record.fields.title = id.into();
        records.insert(id.into(), record);
    }
    let mut issue = Record {
        id: "issue".into(),
        head: "head-issue".into(),
        kind: Kind::Issue,
        fields: record_fields("Issue"),
        comments: Vec::new(),
        updates: Vec::new(),
        created_by: "local:alice".into(),
        created_at_ms: 1,
        updated_at_ms: 1,
    };
    issue.fields.project = Some("project-b".into());
    records.insert("issue".into(), issue.clone());
    let filters = BTreeMap::from([("project".to_owned(), "project-a,project-b".to_owned())]);
    assert!(matches(&issue, &filters));
    assert!(!matches(
        &issue,
        &BTreeMap::from([("project".to_owned(), "project-c".to_owned())])
    ));
}

// r[verify kiln.work.monorepo.compat] r[verify kiln.work.monorepo.compat.scenario.old]
#[test]
fn status_boards_keep_status_lanes_and_omit_new_bytes() {
    let (records, _) = fixture();
    let projection = project(
        &records,
        "local:bob",
        records.get("view-shared"),
        &Query::default(),
        1024,
    );
    assert_eq!(projection.lane_by, "status");
    assert_eq!(projection.move_columns.len(), projection.columns.len());
    let value: Value = serde_json::to_value(&projection)
        .unwrap_or_else(|error| unreachable!("projection serializes: {error}"));
    assert!(
        value
            .get("columns")
            .and_then(Value::as_array)
            .is_some_and(|lanes| lanes.iter().all(|lane| lane.get("ci").is_none()))
    );
}
