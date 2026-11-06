## Context

`OwnershipRule` matches exact changed paths and target refs. Its owner item is
derived from current-revision approvals by expanded eligible owners. CI facts
already carry project, repository, exact revision, ordinal, full-run and clean
flags, and optional scope identity; monorepo requirements bind the current scope
identity.

## Decisions

- `required_evidence` is a list of typed records. `project_ci` is the only
  admitted kind, represented by a closed enum in Rust and a closed Nickel
  contract.
- Policy admission rejects duplicate project identities, empty or untrimmed
  projects, more than eight requirements per rule, and unknown kinds.
- Evidence is evaluated only for matching ownership rules. Missing changed-file
  facts continue to fail the existing ownership check before evidence items are
  considered.
- A project CI requirement resolves the project's current scope from
  `Facts.ci_required`. Without a required set, only the change's own project can
  bind evidence, using the unscoped identity. Another project without a
  required scope cannot be satisfied.
- The latest full clean run for the exact submitted commit must satisfy the
  existing `CiFact::satisfies_required` predicate. Untrusted approval semantics
  stay there; evidence does not weaken them.
- Evidence is additive: the owner approval item remains and can still veto
  readiness. No merge effect is introduced.
- Empty `required_evidence` is omitted during serialization, so existing policy
  and journal bytes replay unchanged.

## Risks

- Evidence kinds are intentionally closed; adding semantic-merge or named-check
  evidence needs its own contract and exact facts.
- A project outside the current required set has no known scope identity and
  therefore cannot satisfy evidence until the shell supplies that requirement.
- This does not implement per-class `evidence_only` approval.
