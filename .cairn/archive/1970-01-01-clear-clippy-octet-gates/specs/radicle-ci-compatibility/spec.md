# Radicle CI compatibility

## MODIFIED Requirements

### Requirement: Broker 0.31 request projection

r[kiln.radicle.broker_031_wire] Kiln MUST admit the exact Radicle CI broker `0.31` request kind and version while projecting only bounded authoritative trigger fields into Kiln. An omitted or null optional patch ID MUST decode as absent and serialize without the ID field. A present string ID MUST retain its exact serialized value; an ID of another JSON type MUST be rejected. An absent optional ID MUST NOT become an admitted patch-trigger identity.

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

#### Scenario: Optional patch identity retains its wire contract

r[kiln.radicle.broker_031_wire.patch_identity]
- GIVEN patch metadata with an omitted, null, string, or non-string ID
- WHEN the broker boundary decodes and serializes it
- THEN omitted and null IDs MUST remain absent with no serialized ID field
- AND a string ID MUST retain its exact bytes while non-string IDs MUST fail decoding
- AND absent IDs MUST fail patch-trigger admission before service effects
