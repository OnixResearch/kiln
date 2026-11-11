## Context

`OwnershipRule::required_evidence` adds exact CI checklist items. Owner approval
remains the only per-rule intent gate. `ChangedFile` retains the old path of a
rename, and ownership requires complete changed-file facts that equal the exact
revision file set.

## Decisions

- Classes are policy-wide so one class definition applies consistently to every
  matching rule.
- Classification is pure and declaration-ordered: the first class whose patterns
  match every new path and every old path wins. An unmatched change is `logic`.
- Empty or incomplete changed-file facts never select a class. They fail the
  existing ownership-file check before classification.
- `approval` maps are sparse. Absence means `owner_approval`, preserving existing
  policy behavior and serialized bytes.
- `evidence_only` must be explicit and must accompany at least one evidence
  requirement. It is rejected for an undeclared class.
- The rule owner item remains present. It is satisfied for an explicit
  evidence-only class, but its evidence items still gate readiness; in
  `logic` (or any class without an override) it is satisfied only by an eligible
  owner approval.
- Evidence-only does not bypass the ordinary policy approval count, vetoes,
  blocking threads, dependencies, CI or merge preflight.

## Risks

- Class globs are ordered; overlapping declarations resolve by declaration order.
- A rename that crosses a class boundary is intentionally `logic`, because both
  paths must match.
- This does not infer classes from authorship, file extensions, labels or
  test/build results.
