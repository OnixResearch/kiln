## Why

The live workspace can target the existing authenticated Iroh workflow bridge, but its job worker equates the controller's canonical Git repository identity with the identity of its local checkout. A separate checkout has a different path-derived identity even when it contains the exact requested commit and canonical tree. This prevents the current web-service consumer from using the existing remote execution capability.

## What Changes

- Introduce the closed `kiln.web-worker.v2` reviewed profile with a required canonical Git `source_repository` identity, separate from local `repository_path`.
- Require the producer source context and controller callback binding to match that declared identity, and continue checking the exact local commit and canonical tree without relabeling local acquisition facts.
- Migrate active builders, fixtures and native smoke consumers; reject old or missing profile fields rather than infer an identity.
- Extend the maintained real native web exercise to use enrolled Iroh peers and an independent worker clone, retaining source/log/artifact/cancellation/restart evidence.

## Impact

Kiln web-service maintainers own this change. The current consumer is the live workspace operator selecting remote Lattice targets. The durable capability is repeatable exact-source execution from a distinct local backing checkout through the existing provider port. No new transport, per-node placement, mixed-host DAG scheduling, source fetch authority, deployment, publication or lifecycle acceptance is introduced.
