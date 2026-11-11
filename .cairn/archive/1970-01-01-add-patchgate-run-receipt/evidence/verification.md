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

The fake producer proves command flow and publication. A separate live run with
the installed pinned Patchgate/Pijul and the local review target accepted one
pending docs change, retained request
`0044f9a0b5babd0ee752a91dc084ca728bb4960144ae129f644ca5b7c149af5f`, and left
the native main/inbox states unchanged. This change does not upload the receipt
through the worker API by itself. Review evidence remains separate from
acceptance, merge, gate and release authority.
