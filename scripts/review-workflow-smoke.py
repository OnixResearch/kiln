#!/usr/bin/env python3
"""Qualify review intake, suggestions, delivered attention and required-CI queue.

Uses the maintained native HTTPS/Lattice exercise; all source, target, recipients
and credentials are disposable. No deployment or publication authority.
"""
# r[verify kiln.review.parity.qualification]
# r[impl kiln.review.parity.qualification]
import argparse
import importlib.util
import json
import os
from pathlib import Path
import sys
import time

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("kiln_web_smoke", Path(__file__).with_name("web-service-smoke.py"))
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)
REVIEWER = "viewer:native-reader"
OWNER = "local:operator"


class ReviewExercise(base.Exercise):
    def command(self, argv, **kwargs):
        argv = list(argv)
        if len(argv) > 1 and str(argv[0]) == self.exe["service"] and argv[1] == "check" and (self.root / "review.json").exists():
            argv.extend(["--code-review", self.root / "review.json"])
        return super().command(argv, **kwargs)

    def spawn(self, name, argv):
        argv = list(argv)
        if name == "service":
            argv.extend(["--code-review", self.root / "review.json"])
        return super().spawn(name, argv)

    def prepare_policies(self):
        super().prepare_policies()
        service = json.loads((self.root / "service-inputs.json").read_text())
        service["bounds"]["poll_interval_ms"] = 1000
        self.export("web-service.ncl", service, "service")
        (self.root / "review").mkdir(mode=0o700)
        destination = json.loads((self.root / "automation.json").read_text())["notifications"][0]
        destination.update(id="review-attention", path="/review", on_success=False, on_failure=False)
        self.export("web-code-review.ncl", {
            "database": str(self.root / "review/review.sqlite"), "project": "success",
            "provider": {"repository_id": self.source_repository, "repository_path": str(self.repo),
                         "target_refs": ["refs/heads/main"], "capabilities": ["inspect", "compare", "register", "submit", "rebase", "revert", "cherry_pick", "apply_suggestions", "build_integration"]},
            "policy": {"id": "isolated-required-ci", "eligible_reviewers": [REVIEWER], "submitters": [OWNER],
                       "minimum_approvals": 1, "require_ci": True,
                       "ownership": {"teams": {"maintainers": [REVIEWER]}, "rules": [
                           {"id": "all-source", "paths": ["**"], "owners": [], "teams": ["maintainers"], "target_refs": []}]}},
            "read_principals": [OWNER, REVIEWER], "write_principals": [OWNER, REVIEWER],
            "signature_name": "Disposable review exercise", "signature_email": "review@example.invalid",
            "intake": {"default_target_ref": "refs/heads/main", "mappings": []},
            "delivery": {"schema": "kiln.review-delivery-profile.v1", "destinations": [destination],
                         "recipients": [{"principal": OWNER, "destination": "review-attention", "address": "isolated-owner", "channel": "webhook"},
                                        {"principal": REVIEWER, "destination": "review-attention", "address": "isolated-reviewer", "channel": "email_adapter"}],
                         "max_records": 1000, "max_attempts": 2, "retry_delay_ms": 1000, "max_tick_effects": 4},
            "queue": {"schema": "kiln.review-queue.v1", "max_entries": 100, "max_candidate_builds": 16},
        }, "review")

    def detail(self, change):
        return self.api("GET", "/api/code-review/changes/" + change)

    def review_command(self, action, change=None, reviewer=False, expected=(200,)):
        version = self.detail(change)["change"]["version"] if change else None
        return self.api("POST", "/api/code-review/commands", {"request_id": "review-" + base.uuid.uuid4().hex,
            "change_id": change, "expected_version": version, "action": action}, token=self.viewer if reviewer else None, expected=expected)

    def branch(self, name, failing=False):
        self.command([self.exe["jj"], "-R", self.repo, "new", "main", "-m", "Disposable review " + name])
        base.put(self.repo / "example.rs", b"fn answer() -> u32 {\n    1\n}\n")
        if failing:
            base.put(self.repo / "source-value.txt", b"genuine-review-ci-failure\n")
        self.command([self.exe["jj"], "-R", self.repo, "file", "track", "root:example.rs"])
        self.command([self.exe["jj"], "-R", self.repo, "describe", "-m", "Disposable review " + name])
        self.command([self.exe["jj"], "-R", self.repo, "bookmark", "create", "review/" + name, "-r", "@"])
        return self.command([self.exe["jj"], "-R", self.repo, "log", "--no-graph", "-r", "@", "-T", "commit_id"]).decode().strip()

    def execute(self):
        self.prepare()
        self.report["schema"] = "kiln.review-parity.native-exercise.v1"
        self.enroll("success")
        self.start_heartbeat()
        self.wait(lambda: self.worker_capacity("online", active=0), "real worker readiness")
        original_target = self.revision
        source = self.branch("qualified")
        cli_result = json.loads(self.cli("review-submit", "--request-id", "review-cli-intake", "--source-ref", "refs/heads/review/qualified", "--title", "Required CI review", "--description", "**Real review** with exact-source CI"))
        change = cli_result["change"]["id"]
        first = self.detail(change)
        assert REVIEWER in first["change"]["reviewers"], first
        assert not first["checklist"]["ready"]
        self.review_command({"type": "subscribe", "subscribed": True}, change, reviewer=True)
        revision = first["change"]["revisions"][-1]
        anchor = {"revision": revision["id"], "base_commit": revision["base"], "file": "example.rs", "side": "revision", "start_line": 2, "end_line": 2}
        before = len([n for n in self.notifications if n["path"] == "/review"])
        self.review_command({"type": "save_draft", "id": None, "anchor": anchor, "thread": None,
            "body": "@local:operator **Use two**. <img src=x onerror=alert(1)>\n```suggestion\n    2\n```", "blocking": False}, change, reviewer=True)
        time.sleep(0.3)
        assert len([n for n in self.notifications if n["path"] == "/review"]) == before
        private = self.api("GET", "/api/code-review/changes/" + change, token=self.viewer)
        draft_id = private["drafts"][0]["id"]
        assert not self.detail(change)["drafts"]
        self.review_command({"type": "publish", "draft_ids": [draft_id], "vote": "approve"}, change, reviewer=True)
        published = self.detail(change)
        assert next(item for item in published["checklist"]["items"] if item["key"] == "ci")["satisfied"] is False
        thread = published["change"]["threads"][0]["id"]
        self.wait(lambda: len([n for n in self.notifications if n["path"] == "/review"]) > before, "published external review delivery")
        self.review_command({"type": "apply_suggestions", "thread_ids": [thread]}, change)
        updated = self.detail(change)
        assert len(updated["change"]["revisions"]) == 2
        assert not next(item for item in updated["checklist"]["items"] if item["key"] == "approvals")["satisfied"]
        self.review_command({"type": "vote", "value": "approve"}, change, reviewer=True)
        self.review_command({"type": "enqueue", "expected_target": original_target}, change)
        def prepared_candidate():
            queue = self.detail(change)["queue"]
            return queue if queue and queue["candidate_commit"] and not queue["ci_run"] else False
        prepared = self.wait(prepared_candidate, "durably constructed exact integration candidate")
        assert prepared["candidate_commit"] != source
        self.service.kill()
        self.service.wait(timeout=15)
        self.start_service()
        recovered = self.detail(change)["queue"]
        assert recovered["id"] == prepared["id"] and recovered["candidate_commit"] == prepared["candidate_commit"], recovered
        submitted = self.wait(lambda: (d if (d := self.detail(change))["change"]["state"] == "submitted" else False), "required CI approval and exact submission")
        queue = submitted["queue"]
        target = self.command([self.exe["jj"], "-R", self.repo, "log", "--no-graph", "-r", "main", "-T", "commit_id"]).decode().strip()
        assert target == queue["candidate_commit"] == submitted["change"]["submission"]["result_commit"]
        run = self.api("GET", "/api/runs/" + queue["ci_run"])
        assert run["source_revision"] == target and run["outcome"] == "Success", run
        self.report["checks"]["required_ci_submission"] = {"original_target": original_target, "original_source": source,
            "reviewed_source": submitted["change"]["revisions"][-1]["commit"], "target": target, "queue": queue, "run": run,
            "restart": "service killed after durable candidate construction; same exact candidate recovered before canonical CI dispatch"}
        bad_source = self.branch("failing", failing=True)
        bad = self.review_command({"type": "contribute", "source_ref": "refs/heads/review/failing", "expected_commit": bad_source,
            "title": "Genuine failing candidate", "description": "Required CI must refuse", "wip": False})["change"]["id"]
        self.review_command({"type": "vote", "value": "approve"}, bad, reviewer=True)
        self.review_command({"type": "enqueue", "expected_target": target}, bad)
        failed = self.wait(lambda: (d if (d := self.detail(bad))["queue"]["status"] == "blocked" else False), "genuine candidate CI refusal")
        failed_run = self.api("GET", "/api/runs/" + failed["queue"]["ci_run"])
        assert failed_run["outcome"] == "DomainFailure", failed_run
        assert self.command([self.exe["jj"], "-R", self.repo, "log", "--no-graph", "-r", "main", "-T", "commit_id"]).decode().strip() == target
        self.report["checks"]["failed_ci_refusal"] = {"change": bad, "queue": failed["queue"], "run": failed_run, "target_unchanged": target}
        self.report["checks"]["review_delivery"] = [n for n in self.notifications if n["path"] == "/review"]
        assert all("draft" not in n["body"] and "body" not in n["body"]["notification"] for n in self.report["checks"]["review_delivery"])
        if self.args.browser_hold:
            base.put(self.root / "browser-ready.json", {"origin": self.origin, "change": change, "failed_change": bad})
            print(json.dumps({"browser_ready": True, "origin": self.origin, "root": str(self.root), "change": change}), flush=True)
            self.wait(lambda: (self.root / "browser-done.json").exists(), "integrated browser exercise", seconds=900)
            self.report["browser"] = json.loads((self.root / "browser-done.json").read_text())
        self.report["passed"] = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("service", "lattice", "lattice-source", "root", "evidence"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--browser-hold", action="store_true")
    args = parser.parse_args()
    args.iroh = None
    args.work = False
    os.umask(0o077)
    exercise = ReviewExercise(args)
    try:
        exercise.execute()
    except BaseException as error:
        exercise.report["failure"] = {"kind": type(error).__name__, "diagnostic": str(error)}
        exercise.record_failure_diagnostics()
        raise
    finally:
        exercise.close()


if __name__ == "__main__":
    main()
