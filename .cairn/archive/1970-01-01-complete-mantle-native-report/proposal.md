## Why

Kiln admits `crunch-build-report-v1` by reading only four count fields, broad
outcome products, log paths and a presence-only substitution marker. Every other
native field is silently ignored, including the physical/logical store
distinction, failure envelopes, cache transfer facts, and the evidence arrays
that Mantle emits for a build. A report from a normally dispatched Mantle
process is therefore rejected or misread: Mantle reports its logical
`--store-prefix` in `store_dir` and its physical export directory in
`output_dir`, while Kiln compares both concepts to one profile path.

## What Changes

- Separate the admitted Mantle logical store prefix from the physical output and
  state directories in the profile, and pass `--store-prefix` explicitly.
- Admit the complete pinned `crunch-build-report-v1` top-level surface with an
  explicit unknown-field policy instead of silently dropping known native facts.
- Require the report's physical output directory, state directory and logical
  store directory to match the profile exactly.
- Require native counts to agree with the outcome and failure arrays, and
  reject a success-shaped report that carries a FOD mismatch.
- Validate outcome, output, attestation and log paths beneath their declared
  roots, and bind failure logs so they are readable through the adapter.
- Project every native evidence collection and cache transfer fact into bounded
  observation metrics, without promoting producer-specific nested evidence to
  canonical authority.
- Add positive and negative report fixtures, documentation and verification
  evidence.
