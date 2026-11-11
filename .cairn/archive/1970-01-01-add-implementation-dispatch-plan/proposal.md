# Add a bounded implementation dispatch plan

## Why

A reservation digest is not meaningful unless Kiln owns what the dispatched
attempt may do. Issue OnixResearch/kiln#45 requires unsupported controls and
broader workspace/effect permissions to fail before transport.

## What Changes

- Add a versioned `ImplementationDispatchPlan` binding work/attempt/fence/base,
  isolated workspace, provider profile, admitted paths/effects, cancel-only
  controls, deadline, token/invocation budgets and expected output.
- Admit it against the exact work and attempt, refusing broader permissions or
  budgets and mismatched bindings.
- Compute a deterministic plan identity and expose `reserve-plan` in the
  implementer CLI.
- Document and test exact/refused plans.

## Impact

- **Files**: `crates/kiln-core/src/implementation/dispatch.rs`,
  `attempt.rs`, `kiln-implementer`, implementer CLI, docs and specification.
- **Testing**: exact admission/reservation; binding, control, workspace, budget,
  path and effect refusals; CLI plan-file reservation.
- **Authority**: plan admission only. No provider transport or worker execution.
