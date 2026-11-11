## Decisions

The plan is admitted against independently resolved expectations so a caller
cannot substitute an endpoint, socket, checkpoint or placement after the fact.
The plan carries only the fields needed to authorize opening the exact bridge;
it does not carry memory or workspace objects, which remain in the checkpoint
manifest.

Only precopy and one connection are admitted initially. Postcopy, memfd and
multiple connections are represented as refused values rather than silently
upgraded. Socket paths are lexical, absolute, bounded and non-escaping; actual
ownership and symlink checks remain the shell's responsibility.

## Non-claims

The plan is not transport authorization by itself, does not prove endpoint
liveness, does not open sockets and does not prove a Cloud Hypervisor migration
outcome.
