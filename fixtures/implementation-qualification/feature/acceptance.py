# Protected feature acceptance: command and bytes are frozen before dispatch.
import importlib.util
import json
from pathlib import Path

spec = importlib.util.spec_from_file_location("labels", "src/labels.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
cases = [("  Hello   WORLD  ", "hello-world"), ("\tA\nB\t", "a-b"), ("", ""), ("  ", ""),
         ("Already-good_2!", "already-good_2!"), ("ÉLAN\u2003VITAL", "élan-vital")]
results = [{"input": value, "actual": module.normalize_label(value), "expected": expected} for value, expected in cases]
try:
    module.normalize_label(42)
    non_string_rejected = False
except TypeError:
    non_string_rejected = True
Path("artifacts").mkdir(exist_ok=True)
Path("artifacts/acceptance.json").write_text(json.dumps({"cases": results, "non_string_rejected": non_string_rejected}))
assert all(row["actual"] == row["expected"] for row in results), results
assert non_string_rejected, "non-string input accepted"
