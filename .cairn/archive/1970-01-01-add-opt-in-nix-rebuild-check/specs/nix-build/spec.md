# Nix Build Specification

## ADDED Requirements

### Requirement: Opt-in rebuild check observation

r[kiln.nix_build.rebuild_check] When the reviewed profile enables
`rebuild_check`, the adapter MUST run one bounded
`nix-store --realise --check -K DERIVATION` after the exact admitted derivation
is realised and rooted, using the build's timeout, stdin, stdout, stderr,
poll-interval and teardown limits. It MUST record one typed observation per
expected output: `matched` when the rebuild compared equal, `differed` with the
retained `.check` path when an exact retained output is observed, or
`not_checkable` when no comparison is possible. The option MUST default to
disabled. A check observation MUST NOT change the build outcome, products, GC
roots, receipt identity or cache facts.

#### Scenario: Deterministic rebuild matches

r[kiln.nix_build.rebuild_check.scenario.matched]
- GIVEN an enabled rebuild check and a successful realisation
- WHEN the check exits zero
- THEN every expected output MUST record `matched` with its exact path and the bounded check log identity

#### Scenario: Rebuild differs and is retained

r[kiln.nix_build.rebuild_check.scenario.differed]
- GIVEN an enabled rebuild check and a successful realisation
- WHEN the check exits nonzero and the `<path>.check` entry exists
- THEN the output MUST record `differed` with that retained path
- AND no product, root or receipt MUST change

#### Scenario: Check cannot compare

r[kiln.nix_build.rebuild_check.scenario.not_checkable]
- GIVEN an enabled rebuild check
- WHEN the check is refused, times out, is cancelled, exceeds its output bound, is ambiguous, or exits nonzero without a retained output
- THEN the output MUST record `not_checkable` without a retained path
- AND the successful build outcome MUST remain unchanged

#### Scenario: Observations are admitted only for exact successful products

r[kiln.nix_build.rebuild_check.scenario.admission]
- GIVEN rebuild observations
- WHEN admission runs for a non-successful outcome, an unknown output name, a duplicate output, or a retained path on a match
- THEN admission MUST reject the observation
