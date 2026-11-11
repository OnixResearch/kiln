## Context

`Facts.ci_required` already carries every affected monorepo project and its
current scope identity. Ownership rules carry path ownership and optional
project links. `automatic_reviewers` already expands teams and excludes the
change owner/author.

## Decisions

- `interface` patterns must be listed verbatim in `paths`; this avoids claiming
  a general glob-subset decision. Duplicate patterns and an interface without a
  project are rejected.
- Interface matching checks current and renamed old paths.
- Every other project in `facts.ci_required` is treated as a potential
  dependent. If the facts are absent, no interface dependency items are created;
  global review policy still applies.
- CI items bind the exact commit and each requirement's current scope identity.
- Dependent owner items require an eligible current-revision approve from the
  linked rule's owners. Advisory agent approvals follow the existing policy.
- Automatic reviewers add those dependent owners; this is review routing, not
  authority.
- Owner approval of each matching interface rule remains required.

## Risks

- This can over-ask when an affected project is unrelated to the source project;
  it errs toward blocking.
- Actual dependency edges are not yet displayed as reasons.
- Interface globs are exact-listed rather than semantically proven subsets.
