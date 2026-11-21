# Reduced realization-receipt fixture

`realization-receipt.json` is a reduced copy of the retained phase-1 pilot
receipt at `/home/brittonr/wt/convergence-evidence/realization/realization-receipt.json`.
It keeps every top-level field, the full shape of `units`, `cache_closure`,
`execution_profiles` and `sources`, the selected root unit plus two units of
each execution class present in the pilot, and the first three cache-closure
facts. All bound identities (`plan_blake3`, `import_receipt_blake3`,
`source_bundle_manifest_blake3`, `cache_closure_policy_blake3`) are unchanged.
`receipt_blake3` was recomputed with Mantle's producer formula:

`BLAKE3("mantle-foreign-realization-receipt-v1\0" || compact JSON with receipt_blake3 = "")`

The full-receipt digest oracle stays in `tests/live_realize_pilot.rs`, which
reads the retained pilot receipt and requires the adapter's recomputation to
equal Mantle's own `receipt_blake3`.

Reduction command:

```console
python3 - <<'PY'
import json, subprocess, collections
src = '/home/brittonr/wt/convergence-evidence/realization/realization-receipt.json'
dst = 'fixtures/mantle/foreign-realize/realization-receipt.json'
r = json.load(open(src))
by_class = collections.defaultdict(list)
for unit in r['units']:
    by_class[unit['execution_class']].append(unit)
root_id = r['selected_root_node_ids'][0]
keep = [unit for unit in r['units'] if unit['node_id'] == root_id]
for items in by_class.values():
    keep.extend(items[:2])
seen, units = set(), []
for unit in keep:
    if unit['node_id'] not in seen:
        seen.add(unit['node_id'])
        units.append(unit)
r['units'] = units
r['cache_closure'] = r['cache_closure'][:3]
r['receipt_blake3'] = ''
canonical = json.dumps(r, separators=(',', ':'), ensure_ascii=False).encode()
open('/tmp/reduced-canon.bin', 'wb').write(
    b'mantle-foreign-realization-receipt-v1\0' + canonical)
r['receipt_blake3'] = subprocess.run(
    ['b3sum', '--no-names', '/tmp/reduced-canon.bin'],
    capture_output=True, text=True, check=True).stdout.strip()
open(dst, 'wb').write(
    json.dumps(r, separators=(',', ':'), ensure_ascii=False).encode())
PY
```

`execution-profile.json` is the pinned revision's
`config/foreign-execution-profiles/generated/nix.json` (BLAKE3
`054ec6754f08d60088cf3388f43295ad37621e5115e979f702a2a5c5d0eefcb8`).
