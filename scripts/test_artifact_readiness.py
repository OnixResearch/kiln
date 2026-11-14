#!/usr/bin/env python3
"""Pure admission/analysis tests; no provider, builder or benchmark starts here."""
# r[verify kiln.readiness.benchmark.scenario.controls] r[verify kiln.readiness.benchmark.scenario.evidence]
import copy
import importlib.util
from pathlib import Path
import unittest
import tempfile

from artifact_readiness_analysis import analyze, compare, distribution

spec = importlib.util.spec_from_file_location("readiness_runner", Path(__file__).with_name("artifact-readiness.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)

POLICY = {"proposal": "regular-file layout", "minimum_samples": 30,
          "minimum_p95_saving_fraction": .2, "minimum_p95_saving_ms": 5,
          "maximum_retained_byte_growth": 0}
WORK = {"id": "many-files", "payload_bytes": 65536, "files": 256, "symlinks": 0, "fanout": 0}


def sample(index=0, elapsed=50_000_000, status="complete", runner_id="host-a"):
    return {"schema": "kiln.readiness.sample.v1", "provider": "hosted-local", "profile": "readiness-cache-v1",
            "runner_identity": runner_id, "source_identity": "source", "cache_state": "warm-local-hit",
            "delay_phase": None, "delay_ms": 0, "workload": WORK, "index": index,
            "status": status, "output_identity": "same-payload", "observations": [
                {"phase": "materialization", "elapsed_ns": elapsed, "user_cpu_ns": 1_000,
                 "system_cpu_ns": 2_000, "injected_delay_ms": 0}]}


class Admission(unittest.TestCase):
    def test_valid_bounded_cohort_and_equal_byte_controls(self):
        cohort = {"schema": "kiln.readiness.cohort.v1", "samples": 3,
                  "workloads": [WORK, WORK | {"id": "one-file", "files": 1}], "decision": POLICY}
        self.assertEqual(runner.validate(cohort), cohort)
        self.assertEqual(cohort["workloads"][0]["payload_bytes"], cohort["workloads"][1]["payload_bytes"])

    def test_invalid_bounds_names_workload_and_thresholds(self):
        base = {"schema": "kiln.readiness.cohort.v1", "samples": 3, "workloads": [WORK], "decision": POLICY}
        for mutation in [base | {"samples": 101}, base | {"samples": True},
                         base | {"workloads": [WORK, WORK]},
                         base | {"workloads": [WORK | {"id": "../outside"}]},
                         base | {"workloads": [WORK | {"files": 3}]},
                         base | {"decision": POLICY | {"minimum_p95_saving_ms": 0}}]:
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                runner.validate(mutation)

    def test_real_materialized_files_and_symlinks_are_verified_not_just_counted(self):
        work = WORK | {"files": 1, "payload_bytes": 16, "symlinks": 1}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "f0000").write_bytes(b"x" * 16)
            (root / "link0").symlink_to("f0000")
            facts = runner.inspect_product(root, work)
            self.assertEqual(facts["payload_bytes"], 16)
            self.assertEqual(facts["regular_files"], 1)
            self.assertEqual(facts["symlinks"], 1)
            (root / "link0").unlink()
            (root / "link0").symlink_to("missing")
            with self.assertRaises(ValueError):
                runner.inspect_product(root, work)
            (root / "link0").unlink()
            (root / "link0").symlink_to("f0000")
            (root / "f0000").write_bytes(b"lower work")
            with self.assertRaises(ValueError):
                runner.inspect_product(root, work)


class Analysis(unittest.TestCase):
    def test_sufficient_sample_tails_and_cpu_are_not_elapsed(self):
        self.assertNotIn("p95_ns", distribution(list(range(19))))
        self.assertEqual(distribution(list(range(20)))["p95_ns"], 18)
        self.assertNotIn("p99_ns", distribution(list(range(99))))
        self.assertEqual(distribution(list(range(100)))["p99_ns"], 98)
        result = analyze([sample()])["cells"][0]["phases"]["materialization"]
        self.assertEqual(result["elapsed_ns"]["median_ns"], 50_000_000)
        self.assertEqual(result["user_cpu_ns"]["median_ns"], 1_000)

    def test_failures_absence_and_changed_runners_are_preserved(self):
        failed = sample(status="failed") | {"error": "provider failure"}
        absent = sample(status="absent") | {"observations": []}
        cells = analyze([sample(), failed, absent, sample(runner_id="host-b")])["cells"]
        self.assertEqual(len(cells), 2)
        self.assertEqual(cells[0]["statuses"], {"complete": 1, "failed": 1, "absent": 1})
        self.assertEqual(cells[0]["phases"]["materialization"]["elapsed_ns"]["n"], 1)
        self.assertEqual(cells[0]["errors"], ["provider failure"])

    def test_delay_attribution_rejects_execution_and_missing_marker(self):
        good = sample() | {"delay_phase": "materialization", "delay_ms": 20}
        good["observations"][0]["injected_delay_ms"] = 20
        self.assertFalse(analyze([good])["cells"][0]["attribution_errors"])
        wrong = copy.deepcopy(good)
        wrong["observations"][0]["phase"] = "execution"
        self.assertTrue(analyze([wrong])["cells"][0]["attribution_errors"])
        missing = copy.deepcopy(good)
        missing["observations"][0]["injected_delay_ms"] = 0
        self.assertTrue(analyze([missing])["cells"][0]["attribution_errors"])

    def test_invalid_timing_is_not_silently_accepted(self):
        invalid = sample(elapsed=-1)
        with self.assertRaises(ValueError):
            analyze([invalid])

    def test_no_go_for_negligible_savings_missing_review_or_changed_work(self):
        baseline = analyze([sample(i) for i in range(30)])
        candidate = analyze([sample(i, elapsed=49_000_000) for i in range(30)])
        review = {"complexity_review_accepted": True, "same_semantic_work": True, "retained_byte_growth": 0}
        self.assertEqual(compare(baseline, candidate, POLICY, review)["optimization_decision"], "no-go")
        faster = analyze([sample(i, elapsed=30_000_000) for i in range(30)])
        self.assertEqual(compare(baseline, faster, POLICY, {})["optimization_decision"], "no-go")
        self.assertEqual(compare(baseline, faster, POLICY, review)["optimization_decision"], "go")
        different = analyze([sample(i, elapsed=30_000_000) | {"output_identity": "different-work"} for i in range(30)])
        self.assertEqual(compare(baseline, different, POLICY, review)["optimization_decision"], "no-go")
        self.assertEqual(analyze([sample()])["optimization_decision"], "insufficient-evidence")


if __name__ == "__main__":
    unittest.main()
