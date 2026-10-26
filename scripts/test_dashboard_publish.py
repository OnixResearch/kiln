import copy
from html import escape
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).with_name("dashboard-publish.py")
SPEC = importlib.util.spec_from_file_location("dashboard_publish", SCRIPT)
publisher = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(publisher)
REPORT = b'<!DOCTYPE html>\n<html><head><title>Kiln</title></head><body><p>Retained CI evidence</p></body></html>\n'


# r[verify kiln.dashboard.repositories.delivery]
# r[verify kiln.dashboard.repositories.delivery.scenario.failure]
class AtomicReportDelivery(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.config_path = self.root / "publication.json"
        self.local = self.root / "last-good.html"
        self.destination = self.root / "delivered.html"
        self.local.write_bytes(b"old local report")
        self.destination.write_bytes(b"old delivered report")
        self.journal = self.root / "canonical.jsonl"
        self.journal.write_bytes(b"canonical source is not a report\n")
        (self.root / "profile.json").write_text("{}")
        (self.root / "catalog.json").write_text(json.dumps({
            "projects": [{"histories": [{"input": {"kind": "journal", "path": "canonical.jsonl"}}]}]
        }))
        (self.root / "payload").write_bytes(REPORT)
        (self.root / "mode").write_text("success")
        self.executable = self.root / "dashboard"
        # A deterministic process fixture controls the generation boundary;
        # admission, subprocess deadlines, filesystem staging and replacement
        # are the real publisher implementation, with no external service.
        self.executable.write_text(f'''#!{sys.executable}
from pathlib import Path
import sys
import time
root = Path(__file__).parent
mode = (root / "mode").read_text()
if mode == "failure":
    print("fixture-secret-never-log", file=sys.stderr)
    sys.exit(7)
if mode == "timeout":
    time.sleep(30)
output = Path(sys.argv[sys.argv.index("--output") + 1])
output.write_bytes((root / "payload").read_bytes())
''')
        self.executable.chmod(0o700)
        self.configuration = {
            "schema_version": 1,
            "dashboard_executable": "dashboard",
            "catalog": "catalog.json",
            "profile": "profile.json",
            "local_report": "last-good.html",
            "destination": {"kind": "local", "path": str(self.destination)},
            "max_report_bytes": 4096,
            "subprocess_timeout_seconds": 1,
            "interval_ms": 1000,
        }
        self.write_config()

    def write_config(self):
        self.config_path.write_text(json.dumps(self.configuration))

    def invoke(self):
        # Run outside the configuration directory to exercise deterministic
        # relative input resolution, not the caller's working directory.
        return subprocess.run([sys.executable, str(SCRIPT), "--config", str(self.config_path)],
                              cwd="/", capture_output=True, timeout=10)

    def assert_prior_reports(self):
        self.assertEqual(self.local.read_bytes(), b"old local report")
        self.assertEqual(self.destination.read_bytes(), b"old delivered report")
        self.assertEqual(self.journal.read_bytes(), b"canonical source is not a report\n")
        self.assertEqual(list(self.root.glob(".kiln-*.html")), [])

    def test_valid_replace_and_repeated_delivery_do_not_mutate_canonical_input(self):
        for _ in range(2):
            result = self.invoke()
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(self.local.read_bytes(), REPORT)
            self.assertEqual(self.destination.read_bytes(), REPORT)
            self.assertEqual(self.journal.read_bytes(), b"canonical source is not a report\n")
        self.assertEqual(list(self.root.glob(".kiln-*.html")), [])

    def test_failed_generation_does_not_publish_and_does_not_log_child_data(self):
        (self.root / "mode").write_text("failure")
        result = self.invoke()
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"dashboard generation failed (exit 7)", result.stderr)
        self.assertNotIn(b"fixture-secret", result.stderr + result.stdout)
        self.assert_prior_reports()

    def test_process_deadline_preserves_both_reports(self):
        (self.root / "mode").write_text("timeout")
        result = self.invoke()
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"timed out", result.stderr)
        self.assert_prior_reports()

    def test_oversized_and_malformed_reports_preserve_both_reports(self):
        for payload in [b"x" * 4097, b"not HTML", REPORT[:-10],
                        REPORT.replace(b"<title>", b'<script src="https://example.invalid/script"></script><title>'),
                        REPORT.replace(b"<title>", b'<style>@import "https://example.invalid/style";</style><title>')]:
            with self.subTest(payload=payload[:80]):
                (self.root / "payload").write_bytes(payload)
                result = self.invoke()
                self.assertEqual(result.returncode, 1)
                self.assert_prior_reports()

    def test_embedded_catalog_reports_are_offline_and_sandboxed(self):
        def catalog(document, sandbox="allow-scripts"):
            frame = f'<iframe sandbox="{sandbox}" title="History" srcdoc="{escape(document.decode(), quote=True)}"></iframe>'
            return REPORT.replace(b"<p>Retained CI evidence</p>", frame.encode())

        (self.root / "payload").write_bytes(catalog(REPORT))
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.destination.read_bytes(), catalog(REPORT))
        prior = self.destination.read_bytes()
        for payload in [catalog(REPORT, "allow-scripts allow-same-origin"),
                        catalog(REPORT.replace(b"<title>", b'<script src="https://example.invalid/"></script><title>')),
                        catalog(catalog(REPORT))]:
            with self.subTest(payload=payload[:80]):
                (self.root / "payload").write_bytes(payload)
                self.assertEqual(self.invoke().returncode, 1)
                self.assertEqual(self.destination.read_bytes(), prior)
                self.assertEqual(self.local.read_bytes(), prior)

    def test_failed_local_transfer_preserves_last_good_and_destination(self):
        # Inject a filesystem commit failure, not an alternate implementation of
        # admission or generation. Neither target may be replaced on that error.
        with patch.object(publisher.os, "replace", side_effect=OSError("commit denied")):
            with self.assertRaises(OSError):
                publisher.publish(self.configuration, self.config_path)
        self.assert_prior_reports()

    def test_closed_versioned_config_rejects_invalid_values_before_generation(self):
        invalid = [dict(self.configuration, unexpected="value"),
                   dict(self.configuration, schema_version=True),
                   dict(self.configuration, interval_ms=0),
                   dict(self.configuration, max_report_bytes=4096.0),
                   dict(self.configuration, subprocess_timeout_seconds=0),
                   dict(self.configuration, destination={"kind": "local", "path": "relative.html"}),
                   dict(self.configuration, destination={"kind": "ssh", "host": "-oProxyCommand=bad", "path": "/report.html"})]
        for configuration in invalid:
            with self.subTest(configuration=configuration):
                self.config_path.write_text(json.dumps(configuration))
                result = self.invoke()
                self.assertEqual(result.returncode, 1)
                self.assert_prior_reports()

    def test_duplicate_json_fields_are_not_silently_accepted(self):
        self.config_path.write_text('{"schema_version":1,"schema_version":1}')
        result = self.invoke()
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"duplicate JSON field", result.stderr)
        self.assert_prior_reports()

    def test_source_and_alias_collisions_are_rejected_without_modification(self):
        alias = self.root / "journal-alias"
        alias.symlink_to(self.journal)
        for target in [self.config_path, self.executable, self.root / "catalog.json",
                       self.root / "profile.json", self.journal, alias]:
            with self.subTest(target=target):
                configuration = copy.deepcopy(self.configuration)
                configuration["destination"]["path"] = str(target)
                before = target.read_bytes()
                with self.assertRaisesRegex(publisher.PublicationError, "collides"):
                    publisher.publish(configuration, self.config_path)
                self.assertEqual(target.read_bytes(), before)
                self.assert_prior_reports()


if __name__ == "__main__":
    unittest.main()
