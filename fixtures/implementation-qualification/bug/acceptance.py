# Protected frozen reproducer: run with an immutable Python interpreter.
import importlib.util
import json
from pathlib import Path

spec = importlib.util.spec_from_file_location("ranges", "src/ranges.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
cases = [(5, 0, 10, 5), (-1, 0, 10, 0), (11, 0, 10, 10), (4, 4, 4, 4), (-3, -5, -1, -3)]
results = [{"arguments": [value, lower, upper], "actual": module.clamp(value, lower, upper), "expected": expected}
           for value, lower, upper, expected in cases]
try:
    module.clamp(5, 10, 0)
    reversed_rejected = False
except ValueError:
    reversed_rejected = True
Path("artifacts").mkdir(exist_ok=True)
Path("artifacts/acceptance.json").write_text(json.dumps({"cases": results, "reversed_rejected": reversed_rejected}))
assert all(row["actual"] == row["expected"] for row in results), results
assert reversed_rejected, "reversed bounds accepted"
