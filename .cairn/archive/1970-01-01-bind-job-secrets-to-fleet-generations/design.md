# Context

`secrets::JobBinding` and JWT `Claims` contain job/admission identity but no
fleet identity. Fleet rows already retain organization, instance ID, request
identity, revision and state. `execution::run_pipeline` admits the workload
policy immediately before dispatch. A deprovision advances revision before the
external driver runs, and only a validated absent receipt permits retirement.

## Decisions

### Decision: exact operator-selected generation

A credential-bearing `execution.workload` policy selects a fleet instance,
active revision and original provisioning request ID. Typed Nickel contracts
check the selector's shape and require it for credential-bearing policies; the
hosted service and standalone execution schemas mirror one another. This
operator configuration is never a customer-supplied run override. The executor
compares the selector to the active durable fleet row, then builds the immutable
job binding and signed JWT claims from the values **read from SQLite**, not the
operator-authored config tuple. A matching config tuple alone grants nothing;
the persisted job row remains one-use by job ID. Credential-free jobs do not
need a fleet selection.

### Decision: check current durable state at every authority boundary

`begin_job`, capability delivery and exchange, and resource-server token
verification require an active row with the exact provision request and
revision. Missing, unknown, provisioning, deprovisioning, replaced and retired
rows fail closed. Delivery checks both before resolver work and after its
return. Revocation on observed retirement marks matching bound job rows inactive
in the same database transaction as the fleet transition. A failed or uncertain
teardown only changes the fleet row to a non-active state; it does not mark the
job as revoked-complete, while the current-state check denies it. A stale
receipt cannot restore a retired row or make old credentials current.

### Decision: preserve authority boundaries

No secret value is persisted, no new token/secret mechanism is introduced, and
no provider placement is inferred from a customer identifier. The pure CI core
still requests effects but never owns physical allocation. Local fake-provider
inventory and receipt checks are not cloud or hostile-isolation qualification.

## Verification

Positive and negative unit tests exercise current, stale, unknown, retired,
replaced, expired, uncertain and restart cases. The existing real-process
qualification harness adds generation-specific secret inspection and emits a
fresh receipt. Run the focused hosted library/example tests, Clippy and rustfmt.

## Non-claims

Fleet row admission is a durable control-plane binding, not independent proof of
physical worker placement or production-provider isolation. A third-party JWT
validator that does not consult Kiln's retained state cannot observe immediate
revocation. No production provider, paid capacity or hostile isolation is
qualified.
