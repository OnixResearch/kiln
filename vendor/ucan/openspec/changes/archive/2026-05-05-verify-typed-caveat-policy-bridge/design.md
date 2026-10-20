# Design: verified typed caveat policy bridge

## Scope
The adapter recognizes only stable shell caveat identities and byte payloads for the existing verified typed caveat model:

- time bound: payload is ASCII decimal `not_after_seconds`;
- rate limit: payload is ASCII decimal `max_count:window_seconds` or another documented exact delimiter format chosen by implementation.

The caller supplies a deterministic context (`now_seconds`, `count_in_window`). The adapter converts recognized caveats into `VerifiedWebOfTrustCaveat` values and calls `typed_caveat_is_satisfied`/`typed_caveat_set_is_satisfied`.

## Trust Boundary
The verified primitive proves satisfaction for finite typed caveat facts only. Parsing bytes into typed facts, choosing current time, counting usage in a window, and binding those facts to an external rate-limit backend remain trusted shell/caller responsibilities.

## Failure Behavior
Malformed payloads, unknown recognized-policy variants, and unsatisfied typed caveats return `CaveatDecision::Rejected`. Caveats outside the typed policy identities remain absent from this policy set, preserving the existing `CaveatPolicySet` composition/deny behavior.
