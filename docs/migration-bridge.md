# Migration bridge plan

`kiln-microvm::bridge` is the pure admission layer for one VM-specific
Cloud Hypervisor migration bridge over Iroh. It binds the exact migration
operation, session, attempt/fence, checkpoint manifest identity, source and
target placement, authenticated endpoint public keys, protected local Unix
sockets, protocol generation and bounded stream resources.

Admission requires precopy mode, one connection, nonzero bounds and exact
matching of every independently resolved fact. Zero endpoints, equal endpoints,
same-host/non-advancing placements, unsafe or non-absolute sockets, postcopy or
memfd modes, wrong peers and wrong protocol generations are refused.

## Non-claims

This module opens no socket, starts no Iroh stream, reads no migration bytes and
proves no Cloud Hypervisor protocol outcome. Transfer completion, durable object
verification, placement commit, destination resume and source cleanup remain
separate operations. The protected socket proxy and cross-host qualification
remain external.
