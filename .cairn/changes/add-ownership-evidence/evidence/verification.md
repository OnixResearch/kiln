# Verification evidence

## Result

Ownership rules may require exact project CI evidence in addition to owner
approval. A matching rule produces one evidence item per requirement, bound to
the exact submitted commit, named project, repository and current scope
identity. Missing, stale, wrong-project, non-clean or partial runs leave the
item unsatisfied. The owner approval item is unchanged.

## Evidence

- Focused core tests cover current-project evidence, another project's required
  scope, stale revision, non-clean and partial runs, and bounded unique policy
  admission.
- Core and web-service suites pass, including existing review journal and policy
  fixtures.
- Clippy, rustfmt, Nickel typecheck/export, and Cairn validation pass.

## Non-claims

Evidence never grants merge authority or satisfies the owner approval item.
`evidence_only` classes and non-CI evidence kinds are not implemented. A
project outside the shell-provided required set has no current scope identity
and cannot satisfy evidence. Passing tests do not prove provider execution
correctness or release readiness.
