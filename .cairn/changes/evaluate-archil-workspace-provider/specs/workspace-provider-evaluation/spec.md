# Workspace provider evaluation delta

## ADDED Requirements

### Requirement: Declared workspace provider profiles

r[kiln.workspace_provider.evaluation] A durable projected-workspace provider MUST be described by a versioned capability profile whose canonical source, projection and candidate identities are BLAKE3 and Kiln-owned, with provider disk, branch, checkpoint, mount and sandbox identifiers kept as references. The profile contract MUST refuse a profile that enforces a restricted view inside a disk while the worker holds a whole-disk credential, declares checkpoints or branches on a native-format bucket disk, treats a bucket copy as a durable acknowledgement, treats forced ownership as a retry, shares one attached disk between attempts, keeps undeletable branches or checkpoints outside a disposable-disk lifecycle, or declares a zero or over-bound limit. Only an adapter execution MAY mark a profile qualified. Declaring a profile MUST NOT grant workspace, source or execution authority or add a provider default.

#### Scenario: A documented-insufficient mechanism is refused

r[kiln.workspace_provider.evaluation.scenario.refused]
- GIVEN a profile that relies on one documented-insufficient mechanism
- WHEN the profile contract is applied
- THEN it MUST be refused with that mechanism's reason

#### Scenario: A vendor claim cannot qualify a profile

r[kiln.workspace_provider.evaluation.scenario.claims]
- GIVEN a profile whose evidence is a vendor claim or a deterministic fixture
- WHEN it declares itself qualified
- THEN the profile contract MUST refuse it
