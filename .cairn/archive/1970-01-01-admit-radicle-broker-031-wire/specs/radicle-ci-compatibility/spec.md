# Radicle CI Compatibility Specification Delta

## ADDED Requirements

### Requirement: Broker 0.31 request projection

r[kiln.radicle.broker_031_wire] Kiln MUST admit the exact Radicle CI broker `0.31` request kind and version while projecting only bounded authoritative trigger fields into Kiln.

#### Scenario: Published push envelope is admitted

r[kiln.radicle.broker_031_wire.accepted]
- GIVEN broker request kind `trigger`, version `1`, a push event, repository ID, branch, and non-empty commit list
- AND the envelope carries producer-owned repository, pusher, before, and after metadata
- WHEN the Defelo adapter maps the request
- THEN the first commit MUST become the exact Kiln revision
- AND descriptive metadata MUST NOT replace repository, revision, branch, event, or actor authority

#### Scenario: Envelope identity or trigger facts are invalid

r[kiln.radicle.broker_031_wire.rejected]
- GIVEN a missing or changed request kind, wrong version, unknown event, malformed or overbound input, or no selected commit
- WHEN Defelo admission runs
- THEN the request MUST fail before Aspen or Lattice effects
- AND no fallback runtime or provider MUST be selected
