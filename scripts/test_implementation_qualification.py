#!/usr/bin/env python3
"""Local scripted controls for qualification mechanics, never live-worker evidence."""
import importlib.util
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("qualification", ROOT / "scripts/implementation-qualification.py")
QUALIFICATION = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(QUALIFICATION)


class QualificationControls(unittest.TestCase):
    def test_whole_value_substitution_preserves_fence_type(self):
        captures = {"claim": {"identity": "actual-content-identity", "fence": 7}, "null": None}
        self.assertEqual(QUALIFICATION.expand({"fence": "@{claim#/fence}", "identity": "@{claim#/identity}"}, captures),
                         {"fence": 7, "identity": "actual-content-identity"})
        self.assertIsNone(QUALIFICATION.expand("@{null}", captures))
        self.assertEqual(QUALIFICATION.expand("not shell $(anything)", captures), "not shell $(anything)")

    def test_missing_capture_is_not_a_successful_skipped_stage(self):
        with self.assertRaises(KeyError):
            QUALIFICATION.expand("@{worker#/result/receipt}", {})

    def test_json_pointer_escapes(self):
        self.assertEqual(QUALIFICATION.pointer({"a/b": {"~": [3]}}, "/a~1b/~0/0"), 3)

    def test_frozen_inputs_are_rechecked_before_stage_execution(self):
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / "cohort.json"
            path.write_text("cohort-v1")
            runner = QUALIFICATION.Runner({"programs": {}}, Path(scratch) / "run")
            runner.digest = lambda candidate: __import__("hashlib").blake2b(Path(candidate).read_bytes()).hexdigest()
            expected = runner.digest(path)
            runner.freeze(path, expected, "cohort")
            runner.assert_frozen()
            path.write_text("cohort-v2")
            with self.assertRaisesRegex(QUALIFICATION.Missing, "frozen input changed"):
                runner.assert_frozen()


    def test_incomplete_scenario_and_no_assertions_fail_before_processes(self):
        with tempfile.TemporaryDirectory() as scratch:
            runner = QUALIFICATION.Runner({"programs": {}}, Path(scratch) / "run")
            with self.assertRaises(QUALIFICATION.Missing):
                runner.scenario({"name": "bug", "steps": [{"stage": "claim"}]})
            steps = [{"stage": stage} for stage in QUALIFICATION.REQUIRED["bug"]]
            with self.assertRaises(QUALIFICATION.Missing):
                runner.scenario({"name": "bug", "steps": steps})

    def test_missing_executables_are_precise_and_no_runtime_is_qualified(self):
        with tempfile.TemporaryDirectory() as scratch:
            runner = QUALIFICATION.Runner({"schema": QUALIFICATION.SCHEMA, "programs": {}}, Path(scratch) / "run")
            with self.assertRaisesRegex(QUALIFICATION.Missing, "programs.coordinator"):
                runner.preflight()
            self.assertEqual(runner.receipt["suite"], "unqualified")
            self.assertEqual(runner.receipt["live_worker"], "not_exercised")
            self.assertFalse((runner.output / "scripted-local-admission").exists())

    def test_existing_receipt_directory_is_never_reused(self):
        with tempfile.TemporaryDirectory() as scratch:
            with self.assertRaises(FileExistsError):
                QUALIFICATION.Runner({}, scratch)

    def structured_case(self, name):
        return {"name": name, "steps": [{"stage": stage, "arguments": [], "capture": stage.replace("-", "_"),
            "exit_code": 0, "timeout_seconds": 120, "observations": [{"pointer": "", "nonempty": True}]}
            for stage in sorted(QUALIFICATION.REQUIRED[name])]}


    def test_noop_or_skipped_checks_need_a_real_rejection_reason(self):
        for name in ("zero-exit-noop", "skipped-checks"):
            case = self.structured_case(name)
            with self.assertRaisesRegex(QUALIFICATION.Missing, "rejection"):
                QUALIFICATION.validate_scenario(case)
            rejection = next(step for step in case["steps"] if step["stage"] in ("capture-candidate", "verify"))
            rejection["exit_code"] = 2
            with self.assertRaisesRegex(QUALIFICATION.Missing, "diagnostic"):
                QUALIFICATION.validate_scenario(case)
            rejection["stderr_contains"] = "exact native domain rejection"
            QUALIFICATION.validate_scenario(case)

    def test_failed_cancellation_is_not_successful_negative_control(self):
        case = self.structured_case("descendant-cancellation")
        for step in case["steps"]:
            if step["stage"] == "lattice-cancel":
                step["exit_code"] = 2
        with self.assertRaisesRegex(QUALIFICATION.Missing, "cancellation is unqualified"):
            QUALIFICATION.validate_scenario(case)

    def test_restart_requires_actual_crash_and_later_recovery(self):
        case = self.structured_case("restart-possible-effects")
        with self.assertRaisesRegex(QUALIFICATION.Missing, "real native-effect crash"):
            QUALIFICATION.validate_scenario(case)
        crash = next(step for step in case["steps"] if step["stage"] == "lattice-crash")
        crash["exit_code"] = -9
        with self.assertRaisesRegex(QUALIFICATION.Missing, "bounded observation interval"):
            QUALIFICATION.validate_scenario(case)
        crash["crash_after_seconds"] = 1
        QUALIFICATION.validate_scenario(case)

    def test_crash_barrier_matches_the_sent_request_not_the_admitted_one(self):
        import hashlib
        import json
        admitted = {"schema": "lattice.implementation-attempt-run.v3", "envelope": {"attempt_id": "a", "fence": 1}}
        sent = dict(admitted, lineage={"schema": "lattice.kiln-budget-lineage.v1"})
        with tempfile.TemporaryDirectory() as directory:
            output, state = Path(directory, "output"), Path(directory, "state")
            output.mkdir()
            sent_bytes = json.dumps(sent).encode()
            # Stand-in digests: the barrier compares whatever the caller hashed.
            sent_digest = hashlib.sha256(sent_bytes).hexdigest()
            admitted_digest = hashlib.sha256(json.dumps(admitted).encode()).hexdigest()
            frozen = output / f"request-{sent_digest}.json"
            frozen.write_bytes(sent_bytes)
            (output / "request-other.json").write_text(json.dumps(dict(admitted, envelope={"attempt_id": "b", "fence": 1})))
            self.assertEqual(QUALIFICATION.sent_requests(output, admitted), [frozen])
            intent = state / "attempt"
            intent.mkdir(parents=True)
            (intent / "intent.json").write_text(sent_digest)
            for name in ("raw-events.jsonl", "omp-observation.json"):
                (intent / name).write_text("{}")
            self.assertEqual(QUALIFICATION.retained_intent_digests(state), {sent_digest})
            self.assertIsNone(QUALIFICATION.retained_worker_barrier(state, {admitted_digest}))
            files, digest = QUALIFICATION.retained_worker_barrier(state, {sent_digest})
            self.assertEqual(digest, sent_digest)
            self.assertEqual(files["raw_events"], intent / "raw-events.jsonl")

    def test_remote_effect_in_later_stage_is_rejected_before_any_execution(self):
        case = self.structured_case("bug")
        case["steps"].append({"stage": "publish"})
        with self.assertRaisesRegex(ValueError, "remote effects"):
            QUALIFICATION.validate_scenario(case)


class ProtectedAcceptanceControls(unittest.TestCase):
    def execute(self, kind, source=None):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch) / "source"
            shutil.copytree(ROOT / "fixtures/implementation-qualification" / kind, root)
            if source is not None:
                target = "ranges.py" if kind == "bug" else "labels.py"
                (root / "src" / target).write_text(source)
            result = subprocess.run([sys.executable, "-I", "-B", "acceptance.py"], cwd=root,
                                    capture_output=True, timeout=10)
            artifact = root / "artifacts/acceptance.json"
            return result.returncode, artifact.read_bytes() if artifact.exists() else None

    def test_frozen_bug_baseline_genuinely_fails(self):
        code, artifact = self.execute("bug")
        self.assertNotEqual(code, 0)
        self.assertIsNotNone(artifact)

    def test_scripted_bug_control_passes_identical_reproducer(self):
        code, artifact = self.execute("bug", "def clamp(value, lower, upper):\n    if lower > upper:\n        raise ValueError('reversed bounds')\n    return max(lower, min(value, upper))\n")
        self.assertEqual(code, 0)
        self.assertIsNotNone(artifact)


    def test_feature_baseline_and_scripted_control_are_distinct(self):
        baseline, _ = self.execute("feature")
        candidate, artifact = self.execute("feature", "def normalize_label(value):\n    if not isinstance(value, str):\n        raise TypeError('string required')\n    return '-'.join(value.split()).lower()\n")
        self.assertNotEqual(baseline, 0)
        self.assertEqual(candidate, 0)
        self.assertIsNotNone(artifact)

    def test_weakened_feature_does_not_pass_explicit_cases(self):
        code, _ = self.execute("feature", "def normalize_label(value):\n    return str(value).strip().lower().replace(' ', '-')\n")
        self.assertNotEqual(code, 0)


if __name__ == "__main__":
    unittest.main()
