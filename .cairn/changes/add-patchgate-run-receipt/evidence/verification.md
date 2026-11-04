# Verification evidence

## Result

`kiln-patchgate-review run` admits one intent, performs the existing isolated
review, and publishes one `kiln.patchgate.review.v1` receipt at the declared
output path. Publication stages a same-directory temporary file, synchronizes
the bytes, renames into place, and synchronizes the directory. A symlink or
non-regular destination is rejected. Standard output is a bounded summary and
does not include the private native report body.

## Evidence

- Focused check and test builds pass for `kiln-adapter-patchgate`.
- Atomic publication tests pass for regular-file replacement and symlink
  rejection without modifying the symlink target.
- A throwaway fake producer exercised `run` end to end: exit 0, one schema-valid
  receipt, accepted decision retained, and a summary without `native_report`.
- `cargo fmt --check -p kiln-adapter-patchgate` passes.
- `cairn validate --root .` reports zero issues.

## Non-claims

The fake producer proves command flow and publication, not a real
Patchgate/Pijul review. This change does not generate intent from live Pijul
state or upload the receipt through the worker API. Review evidence remains
separate from acceptance, merge, gate and release authority.
