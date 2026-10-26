import copy
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).with_name("dashboard-refresh.py")
REPOSITORY = SCRIPT.parent.parent
SPEC = importlib.util.spec_from_file_location("dashboard_refresh", SCRIPT)
refresh = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(refresh)
REAL_DASHBOARD = os.environ.get("KILN_DASHBOARD_EXECUTABLE") or shutil.which("kiln-dashboard")
JOURNAL = REPOSITORY / ".cairn/archive/1970-01-01-compose-radicle-source-to-build-ci/evidence/native/candidates/4b5bc7c33845a2aa10574bfb6c1e0c9b8e1153b2c52592bfb72d144f317f34f8/ci-journal.json"
PROFILE = REPOSITORY / "fixtures/dashboard/profile-v1.json"


class CollectionFixture(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.candidates = self.root / "candidates"
        self.candidates.mkdir()
        self.catalog = self.root / "catalog.json"
        self.catalog.write_bytes(b"old catalog")
        self.local = self.root / "last-good.html"
        self.delivered = self.root / "delivered.html"
        self.local.write_bytes(b"old local")
        self.delivered.write_bytes(b"old delivered")
        self.profile = self.root / "profile.json"
        self.profile.write_bytes(PROFILE.read_bytes())
        self.publication_path = self.root / "publication.json"
        self.config_path = self.root / "refresh.json"
        self.publication = {
            "schema_version": 1,
            "dashboard_executable": REAL_DASHBOARD or sys.executable,
            "catalog": str(self.catalog), "profile": str(self.profile),
            "local_report": str(self.local),
            "destination": {"kind": "local", "path": str(self.delivered)},
            "max_report_bytes": 4194304, "subprocess_timeout_seconds": 10,
            "interval_ms": 30000,
        }
        self.configuration = {
            "schema_version": 1, "candidate_root": str(self.candidates),
            "catalog_output": str(self.catalog), "publication_config": str(self.publication_path),
            "project_id": "dedicated-fork", "project_label": "Observed updates / not source trust",
            "max_candidates": 8, "max_histories": 2, "max_journal_bytes": 1048576,
        }
        self.journal_value = json.loads(JOURNAL.read_bytes())
        self.write_configs()

    def write_configs(self):
        self.config_path.write_text(json.dumps(self.configuration))
        self.publication_path.write_text(json.dumps(self.publication))

    def candidate(self, character="a", mtime=100, value=None):
        directory = self.candidates / (character * 64)
        directory.mkdir()
        path = directory / "ci-journal.json"
        path.write_text(json.dumps(self.journal_value if value is None else value))
        os.utime(path, ns=(mtime, mtime))
        return path

    def invoke(self):
        self.write_configs()
        return subprocess.run([sys.executable, str(SCRIPT), "--config", str(self.config_path)],
                              cwd="/", capture_output=True, timeout=30)

    def assert_prior(self, *, catalog=True):
        self.assertEqual(self.local.read_bytes(), b"old local")
        self.assertEqual(self.delivered.read_bytes(), b"old delivered")
        if catalog:
            self.assertEqual(self.catalog.read_bytes(), b"old catalog")


# r[verify kiln.dashboard.live.collection]
# r[verify kiln.dashboard.live.collection.scenario.admission]
class CollectionAdmission(CollectionFixture):
    def test_order_is_mtime_then_identity_and_history_window_preserves_original_paths(self):
        a = self.candidate("a", 200)
        b = self.candidate("b", 200)
        self.candidate("c", 100)
        (self.candidates / ("d" * 64)).mkdir()  # Missing journal is not a run.
        observations, _ = refresh.collect(self.configuration, self.candidates)
        plan = refresh.plan_catalog(self.configuration, list(reversed(observations)), 4194304)
        histories = plan["projects"][0]["histories"]
        self.assertEqual([item["id"] for item in histories], ["a" * 64, "b" * 64])
        self.assertEqual([item["input"]["path"] for item in histories], [str(a), str(b)])
        self.assertEqual(histories[0]["revision"], self.journal_value["source"]["revision"])
        self.assertEqual(histories[0]["source"], "radicle:" + self.journal_value["source"]["repository"])
        self.assertNotIn("outcome", histories[0])

    def test_all_entries_consume_scan_budget_not_just_candidates(self):
        self.candidate()
        self.configuration.update(max_candidates=2, max_histories=1)
        (self.candidates / "ignored-one").touch()
        (self.candidates / "ignored-two").touch()
        result = self.invoke()
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"entry bound", result.stderr)
        self.assert_prior()

    def test_empty_root_and_only_missing_journals_cannot_replace_retained_catalog(self):
        for reserved in (False, True):
            with self.subTest(reserved=reserved):
                if reserved:
                    (self.candidates / ("b" * 64)).mkdir()
                result = self.invoke()
                self.assertEqual(result.returncode, 1)
                self.assertIn(b"no canonical journal observations", result.stderr)
                self.assert_prior()

    def test_closed_version_paths_and_resource_budget_reject_before_writes(self):
        self.candidate()
        cases = [{"unexpected": True}, {"schema_version": True}, {"max_candidates": 0},
                 {"max_candidates": refresh.MAX_CANDIDATES + 1},
                 {"max_candidates": 1, "max_histories": 2},
                 {"max_histories": refresh.MAX_HISTORIES + 1},
                 {"max_journal_bytes": refresh.MAX_JOURNAL_BYTES + 1},
                 {"max_candidates": 1024, "max_journal_bytes": 16777216},
                 {"candidate_root": "relative"}, {"catalog_output": str(self.root) + "/../catalog"},
                 {"project_id": "bad/id"}, {"project_label": " \n"}]
        original = dict(self.configuration)
        for changes in cases:
            with self.subTest(changes=changes):
                self.configuration = dict(original, **changes)
                self.assertEqual(self.invoke().returncode, 1)
                self.assert_prior()

    def test_schema_source_records_and_even_nonretained_journals_are_admitted(self):
        self.candidate("a", 300)
        self.configuration["max_histories"] = 1
        invalid = [dict(self.journal_value, schema="kiln.other.v1"),
                   dict(self.journal_value, records=[]), dict(self.journal_value, records=[1]),
                   dict(self.journal_value, source={"provider": "radicle", "repository": "rid"}),
                   dict(self.journal_value, source=dict(self.journal_value["source"], secret="bad")),
                   dict(self.journal_value, source=dict(self.journal_value["source"], revision="\x1bsecret")),
                   dict(self.journal_value, source=dict(self.journal_value["source"], revision="é" * 513)),
                   dict(self.journal_value, extra="bad")]
        path = self.candidate("b", 100)
        for value in invalid:
            with self.subTest(value=value.get("schema")):
                path.write_text(json.dumps(value))
                self.assertEqual(self.invoke().returncode, 1)
                self.assert_prior()

    def test_oversized_duplicate_malformed_and_special_journals_fail_without_leaking(self):
        path = self.candidate()
        for content in (b"secret" * 200000, b'{"schema":1,"schema":2}', b"secret-not-json",
                        b'{"value":NaN}'):
            with self.subTest(content=content[:20]):
                path.write_bytes(content)
                result = self.invoke()
                self.assertEqual(result.returncode, 1)
                self.assertNotIn(b"secret", result.stderr)
                self.assert_prior()
        path.unlink()
        os.mkfifo(path)
        self.assertEqual(self.invoke().returncode, 1)  # O_NONBLOCK prevents a hang.
        self.assert_prior()

    def test_input_directory_and_journal_symlinks_are_not_followed(self):
        path = self.candidate()
        saved = path.read_bytes()
        path.unlink()
        path.symlink_to(JOURNAL)
        self.assertEqual(self.invoke().returncode, 1)
        self.assert_prior()
        path.unlink()
        path.write_bytes(saved)
        alias = self.candidates / ("b" * 64)
        alias.symlink_to(path.parent, target_is_directory=True)
        self.assertEqual(self.invoke().returncode, 1)
        self.assert_prior()
        alias.unlink()
        root_alias = self.root / "root-alias"
        root_alias.symlink_to(self.candidates, target_is_directory=True)
        self.configuration["candidate_root"] = str(root_alias)
        self.assertEqual(self.invoke().returncode, 1)
        self.assert_prior()

    def test_catalog_mismatch_and_all_input_output_collisions_preserve_sources(self):
        path = self.candidate()
        saved = path.read_bytes()
        original = copy.deepcopy(self.publication)
        for target in (path, path.parent / "new-state.json", self.config_path,
                       self.publication_path, self.profile, self.local, self.delivered):
            with self.subTest(target=target):
                self.configuration["catalog_output"] = str(target)
                self.publication = copy.deepcopy(original)
                self.publication["catalog"] = str(target)
                self.assertEqual(self.invoke().returncode, 1)
                self.assertEqual(path.read_bytes(), saved)
                self.assert_prior()
        self.configuration["catalog_output"] = str(self.root / "different.json")
        self.publication = original
        result = self.invoke()
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"differs", result.stderr)
        self.assert_prior()

    def test_symlink_hardlink_and_lock_alias_outputs_cannot_modify_candidate_state(self):
        path = self.candidate()
        saved = path.read_bytes()
        self.catalog.unlink()
        for kind in ("symlink", "hardlink"):
            with self.subTest(kind=kind):
                if kind == "symlink":
                    self.catalog.symlink_to(path)
                else:
                    os.link(path, self.catalog)
                self.assertEqual(self.invoke().returncode, 1)
                self.assertEqual(path.read_bytes(), saved)
                self.assert_prior(catalog=False)
                self.catalog.unlink()
        self.catalog.write_bytes(b"old catalog")
        lock = Path(str(self.catalog) + ".lock")
        os.link(path, lock)
        self.assertEqual(self.invoke().returncode, 1)
        self.assertEqual(path.read_bytes(), saved)
        self.assert_prior()

    def test_config_bytes_and_symlink_configs_are_rejected(self):
        self.candidate()
        self.publication_path.write_bytes(b"x" * (refresh.publisher.MAX_CONFIG_BYTES + 1))
        with self.assertRaisesRegex(refresh.RefreshError, "byte bound"):
            refresh.refresh(self.configuration, self.config_path)
        self.assert_prior()
        self.config_path.write_bytes(b"x" * (refresh.publisher.MAX_CONFIG_BYTES + 1))
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "--config", str(self.config_path)],
            capture_output=True, timeout=10)
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"byte bound", result.stderr)
        self.assert_prior()
        self.write_configs()
        alias = self.root / "publication-alias.json"
        alias.symlink_to(self.publication_path)
        self.configuration["publication_config"] = str(alias)
        self.assertEqual(self.invoke().returncode, 1)
        self.assert_prior()

    def test_duplicate_candidate_identity_is_rejected_by_pure_plan(self):
        path = self.candidate()
        observations, _ = refresh.collect(self.configuration, self.candidates)
        with self.assertRaisesRegex(refresh.RefreshError, "duplicate"):
            refresh.plan_catalog(self.configuration, observations + observations, 4194304)
        self.assertEqual(json.loads(path.read_bytes()), self.journal_value)
        self.assert_prior()


# Real renderer, original canonical records, actual CLI, filesystem and publisher.
# Nix checks set the executable explicitly, so this integration cannot skip there.
# r[verify kiln.dashboard.live.recovery]
# r[verify kiln.dashboard.live.recovery.scenario.restart]
# r[verify kiln.dashboard.live.supervision.scenario.cadence]
@unittest.skipUnless(REAL_DASHBOARD, "set KILN_DASHBOARD_EXECUTABLE to exercise the real renderer")
class RealPublication(CollectionFixture):
    def assert_published(self):
        data = self.delivered.read_bytes()
        self.assertEqual(data, self.local.read_bytes())
        refresh.publisher.admit_report(data, self.publication["max_report_bytes"])
        self.assertIn(self.journal_value["source"]["revision"].encode(), data)
        self.assertIn(b"Observed candidate " + b"a" * 64, data)
        return data

    def test_finite_cli_publishes_real_canonical_history_and_rebuilds_after_restart(self):
        path = self.candidate()
        saved = path.read_bytes()
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        first = self.assert_published()
        self.assertEqual(path.read_bytes(), saved)
        self.catalog.unlink()
        self.assertEqual(self.invoke().returncode, 0)
        self.assertEqual(self.assert_published(), first)
        self.assertEqual(path.read_bytes(), saved)

    def test_multiple_histories_share_an_explicit_aggregate_input_budget(self):
        first = self.candidate("a", 200)
        self.candidate("b", 100)
        self.configuration["max_journal_bytes"] = first.stat().st_size
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        data = self.assert_published()
        self.assertIn(b"Observed candidate " + b"b" * 64, data)

    def test_canonical_renderer_rejects_forged_records_and_keeps_both_last_good_reports(self):
        path = self.candidate()
        self.assertEqual(self.invoke().returncode, 0)
        prior = self.assert_published()
        value = dict(self.journal_value, records=[{"not": "a canonical journal record"}])
        path.write_text(json.dumps(value))
        result = self.invoke()
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"dashboard generation failed", result.stderr)
        self.assertEqual(self.local.read_bytes(), prior)
        self.assertEqual(self.delivered.read_bytes(), prior)
        path.write_text(json.dumps(self.journal_value))
        self.assertEqual(self.invoke().returncode, 0)
        self.assertEqual(self.assert_published(), prior)

    def test_report_byte_limit_failure_does_not_commit_reports(self):
        self.candidate()
        self.publication["max_report_bytes"] = 100
        self.assertEqual(self.invoke().returncode, 1)
        self.assert_prior(catalog=False)

    def test_transfer_commit_failure_does_not_commit_either_report(self):
        self.candidate()
        replace = os.replace

        def denied_transfer(source, destination):
            if Path(destination) == self.delivered:
                raise OSError("fixture commit denial")
            return replace(source, destination)

        with patch.object(os, "replace", side_effect=denied_transfer):
            with self.assertRaises(OSError):
                refresh.refresh(self.configuration, self.config_path)
        self.assert_prior(catalog=False)

    def test_overlapping_cli_fails_without_queue_then_next_cycle_recovers(self):
        self.candidate()
        lock_path = Path(str(self.catalog) + ".lock")
        with refresh.cycle_lock(lock_path):
            result = self.invoke()
            self.assertEqual(result.returncode, 1)
            self.assertIn(b"owns the lock", result.stderr)
            self.assert_prior()
        self.assertEqual(self.invoke().returncode, 0)
        self.assert_published()


if __name__ == "__main__":
    unittest.main()
