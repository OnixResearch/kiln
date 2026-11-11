# Require an exact implementation dispatch base

## Why

Issue #45 dispatches an attempt against an exact source base, but intake policies
could not supply a base and dispatch-plan admission accepted an absent base. That
allowed a plan without the frozen source boundary.

## What Changes

- Add optional `base` to GitHub issue and Cairn implementation policies.
- Carry the exact base into admitted work.
- Require a present, exactly matching base in dispatch-plan admission.
- Update tests and documentation; preserve base-absent work for non-dispatch use.

## Impact

- **Files**: GitHub/Cairn intake policies, dispatch plan admission, tests, docs
  and specification.
- **Testing**: policy base propagation for both intake paths; missing/mismatched
  dispatch base refusal.
- **Authority**: no dispatch transport or base resolution is added; operators
  must supply the resolved base.
