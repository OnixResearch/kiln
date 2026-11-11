## Decisions

The receipt reuses the canonical `CiFact`/`CiCheck` model already produced by
the engine, so Kiln does not gain a second verifier. The shell must supply the
actual retained engine evidence; the pure core checks binding, terminality,
success, clean/full/trusted status, exact revision, unique checks and all-pass
outcomes.

A verified attempt is terminal and releases the current claim while retaining
lineage, candidate handoff and receipt. Recording the same receipt again returns
the same identity; a different receipt conflicts.

## Non-claims

No check is executed and no CI fact is created. The receipt proves only that the
supplied canonical evidence met the admission contract; it does not prove the
engine's external truth, merge, landing or release eligibility.
