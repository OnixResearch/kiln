## Context

Projected workspaces are owned by #75 (identities and authority bindings), #78 (materialization and private writable state), #79 (sealed candidates) and #80 (manifest publication and retention). The current worker materializes a full exact Git tree into a private temporary directory. Archil offers durable POSIX disks backed by object storage, checkpoints, writable branches, path delegations and Firecracker sandboxes; its documentation also states the limits that matter here: mount and API credentials cover a whole disk or organization, branches are not a security boundary, checkpoints need the flat-block format, the bucket copy is asynchronous and drops POSIX metadata, crashed clients keep delegations until revoked, forced ownership discards unsent writes, and individual branches and checkpoints cannot be deleted.

## Goals and Success Contract

A reviewer can see, per contract requirement, what Archil documents and whether Kiln may rely on it, and the repository refuses any declared provider profile that relies on a documented-insufficient mechanism. The evaluation concludes adopt, adapt or reference with its unresolved conditions, and adds no dependency or default.

## Decisions

### Decision: a declarative profile contract before any adapter

**Choice:** a typed Nickel profile with shape rules and cross-field refusals, checked by fixtures, rather than an adapter.

**Rationale:** the issue's first evaluation step is a versioned capability profile behind the existing boundary. Encoding the documented limits as refusals keeps a later adapter from silently relying on them; a contract grants no authority and needs no credentials.

**Alternative:** an Archil adapter prototype. It needs an account, credentials and a disposable environment that this change does not have, and its results would be adapter executions that must be qualified separately.

### Decision: label evidence classes and keep qualification out of reach

**Choice:** every profile names its evidence class; only an adapter execution may mark a profile qualified.

**Rationale:** the issue requires deterministic fixtures, real adapter executions and vendor claims to stay separately labeled.

### Decision: retain as reference, adapt branch-per-attempt later

**Choice:** conclude that branch-per-attempt isolation on a disposable managed flat-block disk is the one mechanism worth qualifying, for unrestricted profiles; restricted views need a disk per view or Kiln's own materialization boundary.

**Rationale:** documented credential scope rules out in-disk restriction, and documented bucket behavior rules out the bucket copy as completion or reconstruction evidence.

## Risks / Trade-offs

- Vendor documentation can change; the report records its check date and the contract refuses only what the profile itself declares.
- The corpus comparison waits for #78; until then no Archil profile can move beyond a vendor claim.

## Verification Strategy

Export the positive fixtures and check that every negative fixture fails with its own reason, in `tests/workspace-provider-profile.sh`, run by the `nickel` Nix check alongside `scripts/check-nickel-fixtures.sh`. Then run Cairn gates, validation and traceability.
