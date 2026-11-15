//! Coherent work examples persisted through the real service-signed COB API.
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use kiln_web_service::engine::Role;
use kiln_web_service::work::{Profile, Workspace, ci::CiFacts};
use serde_json::{Value, json};

use super::{ACTOR, Example, Result, accepted, now_ms};
const COLLEAGUE: &str = "viewer:reviewer";
const DAY_MS: i64 = 86_400_000;

pub struct Seed {
    pub workspace: Workspace,
    pub examples: Vec<Example>,
}
struct Writer<'a> {
    workspace: &'a Workspace,
    facts: &'a CiFacts,
    time: i64,
    sequence: u64,
}
impl Writer<'_> {
    fn command(&mut self, record: Option<&Value>, action: Value, actor: &str) -> Result<Value> {
        self.sequence += 1;
        let payload = json!({"request_id":format!("example-work-{}",self.sequence),"object_id":record.map(|v| &v["id"]),"expected_head":record.map(|v| &v["head"]),"action":action});
        let response = self.workspace.response_with_ci(
            "POST",
            "/api/work/commands",
            &serde_json::to_vec(&payload)?,
            Role::Operator,
            actor,
            self.time,
            self.facts,
        );
        let value = accepted(response)
            .map_err(|error| format!("Work seed operation {}: {error}", self.sequence))?;
        self.time += 1;
        Ok(value
            .get("object")
            .ok_or("Work command omitted its object")?
            .clone())
    }
    fn create(&mut self, kind: &str, fields: Value) -> Result<Value> {
        self.command(
            None,
            json!({"type":"create","kind":kind,"fields":fields}),
            ACTOR,
        )
    }
}
fn id(value: &Value) -> Result<&str> {
    Ok(value["id"].as_str().ok_or("Work seed has no exact id")?)
}
fn detail(page: &str, record: &Value) -> Result<String> {
    Ok(format!("/?page={page}&work_id={}", id(record)?))
}

// r[impl kiln.web.exploration.scenario.coverage] r[impl kiln.web.exploration.scenario.local]
pub fn seed(root: &Path, facts: &CiFacts, runs: &BTreeMap<String, String>) -> Result<Seed> {
    let git_directory = root.join("work.git");
    git2::Repository::init_bare(&git_directory)?;
    std::fs::set_permissions(&git_directory, std::fs::Permissions::from_mode(0o700))?;
    let signer_seed_file = root.join("work.seed");
    let mut random = [0_u8; 32];
    getrandom::fill(&mut random)
        .map_err(|error| format!("Example signer randomness unavailable: {error}"))?;
    std::fs::write(&signer_seed_file, random)?;
    std::fs::set_permissions(&signer_seed_file, std::fs::Permissions::from_mode(0o600))?;
    let workspace = Workspace::new(Some(Profile {
        schema: "kiln.web-work.v1".into(),
        git_directory,
        signer_seed_file,
        write_principals: vec![ACTOR.into(), COLLEAGUE.into()],
    }))?;
    let now = i64::try_from(now_ms()?)?;
    let mut writer = Writer {
        workspace: &workspace,
        facts,
        time: now - 2 * DAY_MS,
        sequence: 0,
    };
    let team = writer.create("team", json!({"title":"Platform engineering","description":"Synthetic team. Try a custom review status without implying CI success.","workflow":["review","blocked"]}))?;
    let initiative = writer.create("initiative", json!({"title":"Make releases explainable","description":"Connect retained CI, exact-revision review and operator evidence.","status":"started","owner":ACTOR,"start_at_ms":now-7*DAY_MS,"due_at_ms":now+28*DAY_MS}))?;
    let project = writer.create("project", json!({"title":"Release confidence","description":"A sample planning project, not a CI project. Browse its milestone, cycle and linked issues.","status":"started","owner":ACTOR,"team":id(&team)?,"initiative":id(&initiative)?,"start_at_ms":now-3*DAY_MS,"due_at_ms":now+14*DAY_MS}))?;
    let milestone = writer.create("milestone", json!({"title":"Readable failure reports","description":"Failure diagnosis and exact evidence links ready for review.","project":id(&project)?,"team":id(&team)?,"due_at_ms":now+5*DAY_MS}))?;
    let cycle = writer.create("cycle", json!({"title":"Reliability cycle 07","description":"Current two-week work window.","team":id(&team)?,"start_at_ms":now-2*DAY_MS,"due_at_ms":now+12*DAY_MS}))?;
    let failure_run = runs
        .get("failure")
        .ok_or("Failure run missing for work links")?;
    let success_run = runs
        .get("success")
        .ok_or("Success run missing for work links")?;
    let issue = writer.create("issue", json!({"title":"Explain a failed test from its run","description":"## Reproduction\nOpen the linked synthetic run, inspect stderr, then compare its test report with the passing baseline.\n\n- [x] Retain exact source\n- [ ] Improve the failure explanation\n\nAll observations here are examples, not execution evidence.","status":"started","priority":1,"owner":ACTOR,"assignees":[ACTOR],"labels":["bug","ci","observability"],"team":id(&team)?,"project":id(&project)?,"milestone":id(&milestone)?,"cycle":id(&cycle)?,"initiative":id(&initiative)?,"due_at_ms":now+DAY_MS,"links":[{"kind":"run","id":failure_run}]}))?;
    let issue = writer.command(Some(&issue), json!({"type":"comment","body":"@local:operator The synthetic timeout is visible in stderr; please keep the exact run link when updating the issue.","mentions":[ACTOR]}), COLLEAGUE)?;
    let comment = issue
        .pointer("/comments/0/id")
        .and_then(Value::as_str)
        .ok_or("Comment id missing")?;
    let issue = writer.command(
        Some(&issue),
        json!({"type":"react","comment":comment,"emoji":"👍","active":true}),
        ACTOR,
    )?;
    let issue = writer.command(Some(&issue), json!({"type":"update","body":"Failure text retained. Waiting for the revised explanation and a local review.","mentions":[COLLEAGUE]}), ACTOR)?;
    let child = writer.create("issue", json!({"title":"Link the failing case to its source line","description":"Subissue of failure explanation. Try the relationships and linked CI sections.","parent":id(&issue)?,"status":"review","priority":2,"owner":COLLEAGUE,"assignees":[COLLEAGUE],"team":id(&team)?,"project":id(&project)?,"milestone":id(&milestone)?,"labels":["ui","review"],"links":[{"kind":"run","id":failure_run}]}))?;
    let blocked = writer.create("issue", json!({"title":"Publish the operator walkthrough","description":"Blocked by the failure explanation; board status alone is not dependency completion.","status":"blocked","priority":3,"blocked_by":[id(&issue)?],"team":id(&team)?,"project":id(&project)?,"owner":ACTOR,"labels":["docs"]}))?;
    let completed = writer.create("issue", json!({"title":"Retain the passing baseline","description":"Sample completed planning item with an exact passing run link. Completion is a planning decision, not a release verdict.","status":"completed","priority":2,"team":id(&team)?,"project":id(&project)?,"assignees":[COLLEAGUE],"links":[{"kind":"run","id":success_run}]}))?;
    let snoozed = writer.create("issue", json!({"title":"Revisit worker heartbeat coverage","description":"Snoozed example: resume it through a normal issue status edit.","status":"snoozed","due_at_ms":now+3*DAY_MS,"priority":4,"team":id(&team)?,"project":id(&project)?}))?;
    let duplicate = writer.create("issue", json!({"title":"Duplicate: missing failure explanation","description":"Duplicates the retained report issue rather than silently disappearing.","status":"duplicate","duplicate_of":id(&issue)?,"team":id(&team)?}))?;
    let request = writer.create("request", json!({"title":"Support a nightly evidence digest","description":"Incoming request. Use triage to accept, decline or snooze it while retaining its original source.","status":"triage","priority":2,"customer":"Example partner","source_url":"https://examples.invalid/requests/nightly-digest","labels":["feedback"]}))?;
    let accepted_request = writer.create("request", json!({"title":"Make artifact identities copyable","description":"Accepted example request; the original customer/source stay immutable.","status":"started","owner":ACTOR,"project":id(&project)?,"customer":"Example partner","source_url":"https://examples.invalid/requests/artifact-identity"}))?;
    let declined = writer.create("request", json!({"title":"Treat all unknown outcomes as failed","description":"Declined: unknown provider outcomes must remain explicit until reconciliation.","status":"cancelled","owner":COLLEAGUE,"customer":"Example operator","source_url":"https://examples.invalid/requests/unknown","labels":["declined"]}))?;
    let board = writer.create("view", json!({"title":"Release work board","view":{"filters":{"kind":"issue","project":id(&project)?},"sort":"priority","group_by":"status","layout":"board","shared":true,"board":{"columns":["triage","backlog","started","review","blocked","completed"]}}}))?;
    let private = writer.create("view", json!({"title":"My urgent work","view":{"filters":{"kind":"issue","assignee":ACTOR},"sort":"priority","group_by":"status","layout":"list","shared":false}}))?;
    let ci_board = writer.create("view", json!({"title":"Issues by linked CI project","view":{"filters":{"kind":"issue"},"sort":"updated","group_by":"ci_project","layout":"board","shared":true,"board":{"columns":[],"lane_by":"ci_project"}}}))?;
    let mut reminders = serde_json::Map::new();
    reminders.insert(id(&blocked)?.into(), json!(now - DAY_MS));
    let personal = writer.create("personal", json!({"title":"Example attention preferences","personal":{"watched":[id(&issue)?,id(&child)?],"read":{},"favorites":[id(&board)?,id(&private)?,id(&issue)?],"reminders":reminders}}))?;
    let mut examples = vec![
        Example::new(
            "Issues: discussion, mentions, reactions and exact CI",
            detail("work", &issue)?,
            "Open the issue, follow its failing CI run, add a comment or reaction, edit fields and inspect its retained history.",
        ),
        Example::new(
            "My inbox: assignments, mentions, watching and due reminders",
            "/?page=inbox",
            "The synthetic operator has unread assigned/mentioned/watched work and a due reminder. Mark an item read or change its attention preferences.",
        ),
        Example::new(
            "Planning: initiative to project to milestone and cycle",
            "/?page=planning",
            "Browse all planning kinds and their related issues; planning projects remain distinct from CI projects.",
        ),
        Example::new(
            "Requests & triage: incoming, accepted and declined",
            detail("triage", &request)?,
            "Triage the nightly-digest request. Its original customer and source remain retained.",
        ),
        Example::new(
            "Saved views: private, shared and favorites",
            "/?page=workviews",
            "Open a private list or shared board, modify filters and save your own local view.",
        ),
        Example::new(
            "Status board: custom lanes and safe card moves",
            format!("/?page=work&work_view={}", id(&board)?),
            "Move a card with Move… or M; custom review/blocked lanes belong to the example team. A move is not CI completion.",
        ),
        Example::new(
            "CI-project board: linked and unlinked work",
            format!("/?page=work&work_view={}", id(&ci_board)?),
            "Group issues by their actual retained run links. Unlinked issues stay in a separate lane; no CI fact is invented.",
        ),
        Example::new(
            "Private saved list",
            format!("/?page=work&work_view={}", id(&private)?),
            "Inspect the synthetic operator's assigned work, sorted by priority.",
        ),
    ];
    for (title, kind, record, note) in [
        (
            "Initiative and its planning relationships",
            "initiative",
            &initiative,
            "Open the initiative and follow its project and issue relationships.",
        ),
        (
            "Planning project, milestone and progress",
            "project",
            &project,
            "Inspect due dates, progress and linked planning objects.",
        ),
        (
            "Milestone and child issues",
            "milestone",
            &milestone,
            "Browse the release milestone and its linked issues.",
        ),
        (
            "Current cycle and team workflow",
            "cycle",
            &cycle,
            "Inspect the two-week cycle and its associated work.",
        ),
        (
            "Team with custom workflow statuses",
            "team",
            &team,
            "Inspect and edit the example team's review and blocked states.",
        ),
    ] {
        examples.push(Example::new(
            title,
            format!("/?page=planning&work_kind={kind}&work_id={}", id(record)?),
            note,
        ));
    }
    for (title, record, note) in [
        (
            "Issue relationships: parent and subissue",
            &child,
            "Follow the parent issue, milestone and failing CI result.",
        ),
        (
            "Blocked issue and dependency",
            &blocked,
            "Inspect its blocker, due reminder and project relationship.",
        ),
        (
            "Completed issue with passing CI",
            &completed,
            "Planning completion and retained CI success remain separate facts.",
        ),
        (
            "Snoozed issue with due date",
            &snoozed,
            "Inspect a snoozed issue without losing its future due date.",
        ),
        (
            "Duplicate issue keeps its canonical relationship",
            &duplicate,
            "Follow the original issue rather than dropping duplicate history.",
        ),
        (
            "Attention preferences and favorites",
            &personal,
            "Inspect watched items, favorite views and a retained due reminder.",
        ),
    ] {
        examples.push(Example::new(title, detail("work", record)?, note));
    }
    examples.push(Example::new(
        "Accepted request retains its original source",
        detail("triage", &accepted_request)?,
        "Inspect ownership and planning linkage after triage.",
    ));
    examples.push(Example::new(
        "Declined request retains its reason",
        detail("triage", &declined)?,
        "Read why unknown cannot be silently relabeled as failure.",
    ));
    Ok(Seed {
        workspace,
        examples,
    })
}
