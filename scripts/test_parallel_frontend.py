#!/usr/bin/env python3
"""Pure analysis/admission tests; no Cargo, scope or benchmark invocation."""
# r[verify kiln.build.parallel_frontend_evaluation]
import argparse
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
sys.path.insert(0, str(Path(__file__).parent))


from parallel_frontend_analysis import analyze, load_raw

spec = importlib.util.spec_from_file_location("parallel_frontend_runner", Path(__file__).with_name("parallel-frontend-bench.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


def args(directory, **changes):
    values = dict(toolchain="nightly-2026-07-22", baseline_toolchain="1.88.0", crate="kiln-web-service",
                  threads="1,2,4,8", concurrency="1,2,4", repeats=3, cpu_quota="200%",
                  memory_max="4G", cargo_jobs=None, mode="check", source_revision="0123456789abcdef",
                  out=directory / "new")
    values.update(changes)
    return argparse.Namespace(**values)


def row(thread, repeat, *, concurrency=1, toolchain="nightly-2026-07-22", status="success",
        wall=10, span=10, quota=2, cpu=10, peak=1000, oom=0, hashed=None):
    return dict(schema="kiln.parallel_frontend.job.v1", toolchain=toolchain, rustc_vv="rustc pinned compiler\n",
                crate="kiln-web-service", mode="check", threads=thread, concurrency=concurrency,
                cpu_quota="200%", quota_cpus=quota, memory_max="4G", cargo_jobs=16, source_revision="rev",
                host_cpu_count=16, host_memory_bytes=64000, repeat=repeat,
                batch_id=f"{toolchain}-{thread}-{concurrency}-{repeat}", batch_makespan_s=span,
                status=status, wall_s=wall, end_to_end_s=wall + .25 if wall is not None else None,
                queue_wait_s=0.25, cpu_seconds=cpu, peak_bytes=peak, peak_rss_bytes=peak,
                oom_kills=oom, artifact_hash=hashed)


class Admission(unittest.TestCase):
    def test_valid_pinned_configuration(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(runner.shutil, "which", return_value="/bin/tool"):
            admitted = runner.validate(args(Path(temporary)))
            self.assertEqual(admitted.threads, [1, 2, 4, 8])
            self.assertEqual(admitted.concurrency, [1, 2, 4])

    def test_floating_or_missing_controls_fail(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(runner.shutil, "which", return_value="/bin/tool"):
            root = Path(temporary)
            for change in ({"toolchain": "nightly"}, {"toolchain": "stable"},
                           {"toolchain": "beta"}, {"baseline_toolchain": "stable"},
                           {"concurrency": "1"}, {"concurrency": "2,4"},
                           {"threads": "2,4"}, {"threads": "1,2,2"},
                           {"cpu_quota": "0%"}, {"memory_max": "0G"},
                           {"source_revision": ""}, {"repeats": 0}, {"cargo_jobs": 0},
                           {"mode": "build", "repeats": 1}):
                with self.subTest(change=change), self.assertRaises(ValueError):
                    runner.validate(args(root, **change))
            (root / "new").mkdir()
            with self.assertRaisesRegex(ValueError, "new directory"):
                runner.validate(args(root))

    def test_build_requires_real_hasher_before_output_creation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(runner.shutil, "which", side_effect=lambda name: None if name == "b3sum" else "/bin/tool"):
                with self.assertRaisesRegex(ValueError, "requires b3sum"):
                    runner.validate(args(root, mode="build"))
            self.assertFalse((root / "new").exists())

    def test_scope_only_passes_z_to_nightly_and_limits_each_job(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(runner, "run_checked"):
            root = Path(temporary)
            options = args(root)
            options.threads = [1, 2, 4, 8]
            options.concurrency = [1, 2, 4]
            template = root / "template"
            nightly = runner.make_job(options, root, template, options.toolchain, 4, 2, 0, 0, "rustc vv", (16, 64000))
            stable = runner.make_job(options, root, template, options.baseline_toolchain, None, 2, 0, 1, "rustc vv", (16, 64000))
            self.assertIn("-Zthreads=4", nightly["cargo_argv"])
            self.assertFalse(any("-Z" in part for part in stable["cargo_argv"]))
            self.assertIn("CPUQuota=200%", nightly["argv"])
            self.assertIn("MemorySwapMax=0", nightly["argv"])
            # systemd-run must not substitute the wrapper's ${path} before /bin/sh runs it.
            self.assertIn("--expand-environment=no", nightly["argv"])
            self.assertIn("${path}", runner.SCOPE_WRAPPER)

    def test_cargo_runs_disable_compiler_caches_and_pin_jobs(self):
        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(runner.shutil, "which", return_value="/bin/tool"), \
                patch.dict(runner.os.environ, {"RUSTC_WRAPPER": "/bin/cache-wrapper", "CARGO_INCREMENTAL": "1"}), \
                patch.object(runner, "run_checked") as run_checked:
            root = Path(temporary)
            options = runner.validate(args(root))
            self.assertEqual(options.cargo_jobs, runner.os.cpu_count())
            options.cargo_jobs = 3
            job = runner.make_job(options, root, root / "template", options.toolchain, 2, 1, 0, 0, "rustc vv", (16, 64000))
            clean_argv, clean_environment = run_checked.call_args_list[-1].args
            self.assertIn("clean", clean_argv)
            measured = runner.cargo_environment(options.cargo_jobs)
            for environment in (clean_environment, measured):
                self.assertEqual(environment["RUSTC_WRAPPER"], "")
                self.assertEqual(environment["RUSTC_WORKSPACE_WRAPPER"], "")
                self.assertEqual(environment["CARGO_INCREMENTAL"], "0")
                self.assertEqual(environment["CARGO_BUILD_JOBS"], "3")
            self.assertEqual(job["cargo_jobs"], 3)
            self.assertEqual(job["cargo_environment"]["RUSTC_WRAPPER"], "")

    def test_verbose_compiler_commands_are_preserved(self):
        with tempfile.TemporaryDirectory() as temporary:
            stderr = Path(temporary) / "stderr.log"
            stderr.write_text("warning: unrelated\n     Running `rustc --crate-name kiln_web_service -Zthreads=4`\n")
            self.assertEqual(runner.compiler_commands(stderr),
                             ["Running `rustc --crate-name kiln_web_service -Zthreads=4`"])
            stderr.write_text("warning: only\n")
            self.assertEqual(runner.compiler_commands(stderr), [])

    def test_cli_refuses_floating_toolchain_before_any_command(self):
        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(runner.subprocess, "run") as run, \
                patch.object(runner.sys, "stderr"), \
                self.assertRaises(SystemExit) as refused:
            runner.main(["--toolchain", "nightly", "--cpu-quota", "200%", "--memory-max", "4G",
                         "--source-revision", "rev", "--out", str(Path(temporary) / "new")])
        self.assertEqual(refused.exception.code, 2)
        run.assert_not_called()


class Analysis(unittest.TestCase):
    def test_distribution_and_bounded_recommendation(self):
        rows = [row(1, i, span=20, cpu=10, peak=2000) for i in range(3)]
        rows += [row(2, i, wall=8, span=10, cpu=12, peak=3000) for i in range(3)]
        result = analyze(rows)
        cells = {c["identity"]["threads"]: c for c in result["cells"]}
        self.assertEqual(cells[1]["wall_s"]["p95"], 10)
        self.assertEqual(cells[2]["throughput_per_min"], 6)
        self.assertEqual(cells[2]["cpu_utilization_mean"], .75)
        self.assertEqual(cells[2]["peak_bytes"]["max"], 3000)
        self.assertEqual(cells[2]["peak_rss_bytes"]["median"], 3000)
        self.assertEqual(result["recommendations"][0]["thread_count"], 2)

    def test_failures_oom_missing_counters_and_spread_prevent_false_claim(self):
        rows = [row(1, i, span=10 if i < 2 else 20) for i in range(3)]
        rows += [row(2, i, span=9 if i < 2 else 19) for i in range(2)]
        rows += [row(2, 2, status="failure", wall=None, cpu=None, peak=None, oom=1)]
        result = analyze(rows)
        candidate = next(c for c in result["cells"] if c["identity"]["threads"] == 2)
        self.assertEqual((candidate["n"], candidate["failures"], candidate["ooms"]), (3, 1, 1))
        self.assertTrue(candidate["insufficient"])
        self.assertEqual(result["recommendations"][0]["status"], "no recommendation")
        rows[-1] = row(2, 2, span=19)
        self.assertEqual(analyze(rows)["recommendations"][0]["status"], "no recommendation")

    def test_stable_not_pooled_and_hash_repeat_is_explicit(self):
        nightly = [row(1, i, hashed="abc") for i in range(3)]
        nightly[1]["artifact_hash"] = "different"
        stable = [row(None, i, toolchain="1.88.0") for i in range(3)]
        result = analyze(nightly + stable)
        self.assertEqual(len(result["cells"]), 2)
        self.assertFalse(next(c for c in result["cells"] if c["identity"]["threads"] == 1)["artifact_hash_equal"])
        self.assertEqual(len(result["recommendations"]), 1)

    def test_invalid_and_missing_values_rejected(self):
        for mutation in ({"wall_s": -1}, {"cpu_seconds": float("nan")},
                         {"batch_makespan_s": 0}, {"status": "unknown"},
                         {"schema": "other"}):
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                analyze([row(1, 0) | mutation])
        with self.assertRaises(ValueError):
            analyze([{"schema": "kiln.parallel_frontend.job.v1"}])

    def test_load_raw_limit_and_round_trip(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "raw.jsonl"
            path.write_text(json.dumps(row(1, 0)) + "\n")
            self.assertEqual(analyze(load_raw(path))["cells"][0]["n"], 1)
            with patch("parallel_frontend_analysis.MAX_RAW_BYTES", 1), self.assertRaises(ValueError):
                load_raw(path)


if __name__ == "__main__":
    unittest.main()
