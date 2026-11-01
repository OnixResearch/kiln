# Verification evidence

## Result

Ownership rules can name a monorepo project and interface paths. An interface
change now requires full clean exact-commit CI for every other affected project
and one eligible non-author owner approval from each ownership rule linked to
those projects. Automatic reviewers include the dependent owners. The
interface rule's own owner approval remains required.

## Evidence

- Focused core tests cover interface CI, dependent acknowledgement, automatic
  reviewer expansion, non-interface behavior and policy admission.
- Full core and web-service suites pass.
- Clippy, rustfmt, Nickel typecheck/export and Cairn validation pass.

## Non-claims

This first cut treats every other affected project as a dependent because
explicit dependency edges are not yet carried into review facts. Interface
patterns must be exact rule-path members or literal paths matched by a rule
path. Dependent requirements do not
replace the interface rule's owner approval. Passing tests do not prove provider
execution correctness or release readiness.
