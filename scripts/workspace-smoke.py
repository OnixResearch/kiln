#!/usr/bin/env python3
"""Native work-CI exercise helpers used by web-service-smoke.py --work.

Only caller-owned disposable stores are created. No remote effects or installed keys.
"""
import copy
import os
import time
import uuid


# r[verify kiln.work.authority] r[verify kiln.work.change]
def prepare(exercise, put):
    store = exercise.root / "work-cobs"
    exercise.command([exercise.exe["jj"], "git", "init", "--colocate", store])
    put(exercise.root / "work.seed", os.urandom(32))
    exercise.export("web-work.ncl", {
        "git_directory": str(store / ".git"),
        "signer_seed_file": str(exercise.root / "work.seed"),
        "write_principals": ["local:operator"],
    }, "work")
    exercise.export("web-sources.ncl", {"sources": [{
        "id": "native", "label": "Disposable native source", "kind": "git", "path": str(exercise.repo),
    }]}, "sources")


# r[verify kiln.work.issues] r[verify kiln.work.planning]
# r[verify kiln.work.discussion] r[verify kiln.work.triage]
# r[verify kiln.work.personal] r[verify kiln.work.views]
def execute(exercise, success):
    api = exercise.api
    now = int(time.time() * 1000)
    observations = {}

    def send(action, record=None, request_id=None, expected=(200, 201)):
        command = {"request_id": request_id or str(uuid.uuid4()),
                   "object_id": record["id"] if record else None,
                   "expected_head": record["head"] if record else None,
                   "action": action}
        return api("POST", "/api/work/commands", command, expected=expected), command

    def create(kind, title, **fields):
        response, _ = send({"type": "create", "kind": kind,
                            "fields": {"title": title, **fields}})
        return response["object"]

    def edit(record, **changes):
        fields = {**record["fields"], **changes}
        response, command = send({"type": "edit", "fields": fields}, record)
        return response["object"], command

    inventory = api("GET", "/api/work")
    assert inventory["configured"] and inventory["can_write"]
    assert inventory["principal"] == "local:operator" and inventory["objects"] == []
    team = create("team", "Runtime team", workflow=["backlog", "started", "review", "completed"])
    initiative = create("initiative", "Reliable delivery", start_at_ms=now, due_at_ms=now + 86400000)
    project = create("project", "COB workspace", team=team["id"], initiative=initiative["id"],
                     start_at_ms=now, due_at_ms=now + 86400000)
    milestone = create("milestone", "Native writes", project=project["id"], due_at_ms=now + 3600000)
    cycle = create("cycle", "Current cycle", team=team["id"], start_at_ms=now, due_at_ms=now + 7200000)
    issue = create("issue", "Join exact CI evidence", description="Real native source-bound work; not release acceptance.",
                   assignees=["local:operator"], priority=1, labels=["work", "ci"], team=team["id"],
                   project=project["id"], milestone=milestone["id"], cycle=cycle["id"],
                   links=[{"kind": "run", "id": success["id"]},
                          {"kind": "source", "id": "native@" + exercise.revision},
                          {"kind": "revision", "id": exercise.revision}])
    child = create("issue", "Verify child evidence", parent=issue["id"], project=project["id"],
                   blocked_by=[issue["id"]])
    issue, first_edit = edit(issue, status="review")
    replay = api("POST", "/api/work/commands", first_edit)
    assert replay["replayed"] and replay["object"]["head"] == issue["head"]
    changed_replay = copy.deepcopy(first_edit)
    changed_replay["action"]["fields"]["title"] = "Changed request is not a replay"
    api("POST", "/api/work/commands", changed_replay, expected=(409,))
    stale = copy.deepcopy(first_edit)
    stale["request_id"] = str(uuid.uuid4())
    api("POST", "/api/work/commands", stale, expected=(409,))
    response, _ = send({"type": "comment", "body": "Please inspect exact CI evidence.",
                        "mentions": ["local:operator"]}, issue)
    issue = response["object"]
    comment = issue["comments"][0]
    response, _ = send({"type": "edit_comment", "comment": comment["id"],
                        "body": "Please inspect exact CI evidence and retained tests.",
                        "mentions": ["local:operator"]}, issue)
    issue = response["object"]
    response, _ = send({"type": "react", "comment": comment["id"], "emoji": "+1", "active": True}, issue)
    issue = response["object"]
    assert issue["comments"][0]["reactions"]["+1"] == ["local:operator"]
    response, _ = send({"type": "update", "body": "Native workspace exercise is in progress.",
                        "mentions": ["local:operator"]}, project)
    project = response["object"]
    request = create("request", "Support request: retained source context", status="triage",
                     customer="Disposable internal customer", source_url="https://example.invalid/support/1")
    request, _ = edit(request, status="snoozed", due_at_ms=now + 3600000)
    request, _ = edit(request, status="duplicate", duplicate_of=issue["id"], team=team["id"],
                      assignees=["local:operator"])
    assert request["fields"]["source_url"] == "https://example.invalid/support/1"
    private_view = create("view", "My urgent work", owner="local:operator",
                          view={"filters": {"kind": "issue", "assignee": "local:operator"},
                                "sort": "priority", "group_by": "status", "layout": "board", "shared": False})
    shared_view = create("view", "Team delivery", owner="local:operator",
                         view={"filters": {"project": project["id"]}, "sort": "updated",
                               "group_by": "status", "layout": "list", "shared": True})
    personal = create("personal", "My attention", owner="local:operator", personal={
        "watched": [issue["id"], project["id"]], "read": {issue["id"]: issue["head"]},
        "favorites": [issue["id"], private_view["id"]],
        "reminders": {issue["id"]: int(time.time() * 1000) + 60_000},
    })
    visible = api("GET", "/api/work", token=exercise.viewer)
    assert not visible["can_write"]
    visible_ids = {record["id"] for record in visible["objects"]}
    assert shared_view["id"] in visible_ids and private_view["id"] not in visible_ids
    assert personal["id"] not in visible_ids
    api("GET", "/api/work/objects/" + personal["id"], token=exercise.viewer, expected=(404,))
    api("POST", "/api/work/commands", first_edit, token=exercise.viewer, expected=(403,))
    api("GET", "/api/work", token=exercise.worker, expected=(403,))
    api("POST", "/api/work/commands", first_edit, origin=False, expected=(403,))
    api("POST", "/api/work/commands", first_edit, origin="https://other.invalid", expected=(403,))
    exact = api("GET", "/api/work/objects/" + issue["id"])
    assert exact["head"] == issue["head"] and exact["comments"][0]["body"].endswith("retained tests.")
    observations.update(
        issue=issue, child=child, project=project, team=team, initiative=initiative,
        milestone=milestone, cycle=cycle, request=request, private_view=private_view,
        shared_view=shared_view, personal=personal,
        viewer_private_records_absent=True, worker_denied=True,
        origin_required=True, exact_replay=True, changed_replay_rejected=True, stale_head_rejected=True,
    )
    before = api("GET", "/api/work")
    exercise.service.terminate()
    exercise.service.wait(timeout=15)
    exercise.start_service()
    after = api("GET", "/api/work")
    assert before == after, "signed work projection changed after service restart"
    observations["restart_projection_identical"] = True
    exercise.report["checks"]["cob_workspace"] = observations
