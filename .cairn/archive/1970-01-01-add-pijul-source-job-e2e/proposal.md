# Add a Pijul Source-to-CI E2E Test

## Problem

The Pijul native fixture tests prove exact channel resolution, bounded snapshot
identity, and fail-closed repository handling. They stop at `SourceResolution`
and do not exercise the admitted snapshot bytes against a reviewed command or
the Kiln terminal transition. A source provider can pass its own conformance
checks while the materialized checkout or the CI outcome remains unverified.

## Proposed Change

Add one native Pijul E2E test that:

1. creates a real Pijul repository with the pinned Pijul cohort;
2. resolves and acquires the exact channel state through
   `LocalPijulAdapter`;
3. materializes only the admitted snapshot entries into a fresh checkout;
4. executes a reviewed command with Bounded Exec;
5. admits the resulting workflow observation; and
6. applies the corresponding StartRun and CompleteAttempt transitions.

The test covers an exact-content pass and a genuine nonzero command failure.
It uses the existing `native-fixtures` feature and `KILN_PIJUL_EXE` producer.
The shared Pijul fixture helpers move to `tests/support/mod.rs` so the native
conformance and full E2E targets use one repository setup.

## Success Criteria

- The real Pijul snapshot contains the tracked file and excludes `.pijul`
  metadata.
- The exact Pijul state from the adapter is the workflow source revision.
- The materialized checkout preserves exact file bytes and rejects unsafe
  paths.
- The reviewed command observes the exact content and no repository metadata.
- A zero exit becomes `Outcome::Success`; a nonzero exit becomes
  `Outcome::DomainFailure`.
- The same `SourceResolution` identity is observed through the mapping path.
- The existing `pijul-native-fixtures` Nix check runs both native test targets.
