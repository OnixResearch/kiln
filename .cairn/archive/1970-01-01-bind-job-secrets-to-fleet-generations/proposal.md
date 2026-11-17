# Why

Issue #72's retained fake-provider fleet receipt records a precise gap: hosted
job-secret capabilities and workload JWTs name a job/admission but not the
fleet worker generation. A retired or replaced worker's old authority therefore
cannot be checked against the fleet's durable instance state.

## What Changes

- Bind executor-admitted credential grants to an operator-selected fleet instance,
  revision and provision request identity. Refuse admission unless that exact
  instance is currently active, and recheck the durable generation before secret
  delivery, token exchange and resource-server validation.
- Revoke bound job authority when retirement is observed; an uncertain teardown
  makes the generation unusable but does not falsely mark revocation complete.
- Extend unit and real-process fake-provider qualification with expiry,
  replacement, retirement, stale receipt and restart cases. Retain a new receipt
  and update the operator procedure.

## Impact

Only the existing hosted fleet, executor and job-secret shell and its operator
workload policy change. Credential-bearing workload policies must explicitly
select a fleet generation; credential-free execution remains unchanged. This
adds neither a secret subsystem nor physical-allocation authority to the pure
CI core, and does not qualify a production provider.

## Authority

The operator asked to complete issue #72 inside Kiln. No cloud activation,
paid capacity, deployment or remote mutation is authorized by this change.
