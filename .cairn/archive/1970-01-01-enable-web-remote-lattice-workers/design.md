## Context

LocalGitAdapter owns a path-derived BLAKE3 repository identity. Controller and worker clones therefore have distinct local identities. SourceContext already carries the controller's canonical Git identity and exact commit; worker callbacks already bind project, repository, commit, canonical tree, attempt, execution and selected nodes. Existing Iroh proxy/worker bridges preserve the complete Lattice workflow contract.

## Decisions

### Decision: Explicit reviewed source binding

Require `source_repository` as the canonical lowercase 32-byte BLAKE3 Git repository native identity in `kiln.web-worker.v2`. Keep `repository_path` as a worker-local canonical backing checkout. Reject v1, missing or malformed bindings; re-export/review profiles and operation/graph identities rather than add compatibility defaults. SourceContext must match the declared identity before materialization. LocalGitAdapter resolution remains an honest local acquisition fact and must not be rewritten. Callback admission compares the declared canonical Git ExternalRef plus exact acquired commit and canonical tree; stale/wrong bindings fail before commands or uploads.

### Decision: Existing whole-workflow remote transport

Use the existing versioned Iroh Unix proxy and worker-local Lattice endpoint, enrolled ephemeral keys, loopback endpoints, no relay/discovery, and native saved exact routes. Exercise an independent worker clone and remote adapter labels. Local and remote refer to provider capabilities and reviewed routes; labels alone do not place nodes. No new executor, transport, per-node placement or mixed-host DAG scheduling is claimed.

### Decision: Bounded reproducible evidence

Keep paired source/profile/callback regressions beside existing worker tests. Extend the existing web-service smoke command with an explicitly selected Iroh mode, preserving the local mode. Exercise real commands, dependency execution, retained logs/artifact bytes, genuine failure, unchanged execution across service/bridge restart, cancellation, queue and unavailable-provider behavior. Retire processes, keys, tokens and scratch. Preserve historical receipts and unrelated work. Run checks only after integration, not in sibling workers.

The restart scenario stops the controller before restarting both bridges, then starts the controller and recovers the original accepted execution. An initial live-bridge restart returned control_unsupported after the interruption; that failed evidence is retained separately. Existing canonical Unknown is reconciled only to a terminal outcome, not reset to Pending. This change does not add retry authority or a control surface for an unresolved live outage.

## Risks / Trade-offs

The configured binding is operator authority, not automatic proof of repository ownership or source trust. Exact commit and canonical-tree equality prevent substitution within the selected trusted source cohort, not Git collision attacks or malicious administrators. Loopback Iroh with separate clones proves the selected protocol and checkout path, not WAN reachability, hostile-code confinement or distinct-host deployment. Existing persisted profiles require explicit v2 re-export and graph review; no production activation is authorized.
