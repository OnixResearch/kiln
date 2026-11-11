## Decisions

The retained lifecycle is a separate module from the per-job lifecycle. Reusing
the per-job states would either repurpose debug `Held` or make destruction part
of a retained session's normal path, both explicitly disallowed by #52.

Ownership is an owner token, not an ambient host label. Every command checks the
session, attempt fence, host and placement generation together, so a stale host
cannot resume or migrate after a newer generation is admitted.

Restore requires an observed confinement result before any workload effect.
Failure moves to `reconcile-required` and retains capacity. Migration records an
intent with a strictly higher target generation; observing the source migrated
away only moves to `migrated-away`. Target placement, volume/effect fencing and
cleanup remain separate observed operations. Retirement is the only capacity
release.

## Non-claims

The module does not claim target placement, distributed consensus, memory
transfer, workspace transfer, VMM cleanup or onix-core service ownership. Iroh/
Aspen transport and the hosted runner integration are dependent work.
