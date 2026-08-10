# Add a Lattice workflow adapter

## Why

Kiln declares `lattice-workflow-v1`, but no adapter can dispatch a workflow. The profile is a compatibility statement without operational behavior.

Lattice owns graph execution and adapter effects. Kiln must retain CI runs, attempts, outcomes, fairness, and provider admission.

## What Changes

- Add optional `kiln-adapter-lattice` support for the `WorkflowPort`.
- Pin one exact Lattice workflow request and observation contract.
- Lower admitted Kiln workflow effects into bounded Lattice requests through a pure core.
- Preserve graph, execution, node, adapter, log, and receipt identities.
- Support observation, cancellation, pause, resume, logs, and reconciliation by declared capability.
- Reject unsupported dynamic authority, credentials, and effect types before dispatch.
- Add typed Nickel profiles, limits, positive fixtures, and negative lifecycle fixtures.

## Dependencies

This change depends on `establish-neutral-ci-control-plane` and a published Lattice adapter contract.

## Impact

- **Adapters**: adds `kiln-adapter-lattice`.
- **Core**: retains provider-neutral workflow and CI decisions.
- **Configuration**: adds an executable Lattice compatibility profile.
- **Testing**: covers mapping, lifecycle, cancellation, replay, drift, and reconciliation.
