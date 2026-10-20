## ADDED Requirements

### Requirement: Typed caveat payload-shape admission [r[typed-caveat-payload-shape-admission]]

UCAN typed caveat parsing MUST route supported time-bound and rate-limit payload byte-shape admission through vendored verified logic before constructing typed caveat values.

#### Scenario: Time-bound payload shape [r[typed-caveat-payload-shape-admission.time-bound]]

- GIVEN a time-bound caveat payload byte sequence
- WHEN UCAN parses it as the supported typed time caveat
- THEN the adapter MUST accept only non-empty ASCII decimal byte sequences through the vendored payload-shape helper before runtime numeric parsing

#### Scenario: Rate-limit payload shape [r[typed-caveat-payload-shape-admission.rate-limit]]

- GIVEN a rate-limit caveat payload byte sequence
- WHEN UCAN parses it as the supported typed rate-limit caveat
- THEN the adapter MUST accept only `max_count:window_seconds` byte sequences with exactly one colon and non-empty ASCII decimal bytes on both sides through the vendored payload-shape helper before runtime numeric parsing

#### Scenario: Boundaries [r[typed-caveat-payload-shape-admission.boundaries]]

- GIVEN a payload that passes typed caveat payload-shape admission
- WHEN UCAN continues parsing and evaluating the caveat
- THEN the verified bridge MUST NOT claim UTF-8 parser correctness, decimal-to-u64 parser correctness, clock freshness, rate-limit counter truth, arbitrary caveat policy semantics, backend truth, token provenance, or authorization correctness beyond the finite payload-shape fact
