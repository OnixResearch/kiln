# Verification evidence

## Result

The Patchgate job script now invokes one atomic adapter `run` operation against
a live native intent. The reviewed worker step builder runs that script from the
checkout and declares its receipt as a `review` report, so the existing worker
report admission binds the exact run, attempt, execution and node.

## Evidence

- Python syntax and CLI help pass for the updated job script.
- The typed worker step builder typechecks, formats, and exports with the
  expected absolute target and executables, checkout-relative script/output,
  explicit `PATH`, and `kind = "review"` declaration.
- The full `web-worker-patchgate.ncl` fixture exports as a `kiln.web-worker.v2`
  profile with the review step and report declaration.
- All four negative fixtures fail: an absolute script path, a relative target,
  an output path that escapes the checkout, and an empty `PATH`.
- A throwaway fake-native and fake-Patchgate smoke ran the real job script
  through the real adapter `run` command with exit 0; it produced one
  `kiln.patchgate.review.v1` receipt with an accepted decision.
- The existing Patchgate library and binary tests pass (12 and 2).
- `cairn validate --root .` reports zero issues.

## Non-claims

The fake-native smoke proves orchestration and receipt flow, not a live
Patchgate/Pijul deployment, model review quality, source correctness, merge
eligibility or release readiness. The worker step uploads review evidence only;
it grants no acceptance, merge, gate or release authority. A missing report
remains unavailable rather than passing.
