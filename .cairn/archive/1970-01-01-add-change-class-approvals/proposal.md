## Why

Ownership evidence now exists, but every matching rule still requires an owner
approval. Owners need an explicit, policy-declared way to say that a low-risk
change class, such as documentation, is governed by exact evidence instead of
intent approval, while logic changes keep the existing default.

Issue: [#24](https://github.com/OnixResearch/kiln/issues/24).

## What Changes

- Add optional policy-wide `ownership.classes`: ordered, bounded path sets with
  unique non-`logic` identities.
- Classify a change by the first class whose patterns match every current and
  old changed path. Unmatched changes use the reserved `logic` class.
- Add optional per-rule `approval` keyed by class: `owner_approval` (the
  default) or `evidence_only`.
- An explicit `evidence_only` entry replaces only that rule's owner approval and
  requires at least one `required_evidence` entry; ordinary approvals, vetoes,
  CI, threads, dependencies and merge preflight are unchanged.
- The owner checklist item states the selected class and whether owner approval
  is required.
- Add positive and negative tests, the Nickel contract and operator docs.

Only ordered class globs are implemented; authorship or heuristics never choose
a class.
