## Context

`ConfigEffectClass` is the service/GitHub/Gitea workflow configuration enum.
It maps into provider-neutral `WorkflowEffectClass`. Both the Lattice capability
profile and the Nickel service contract currently require exactly `build` and
`test`, so a Patchgate `review` node fails before dispatch.

## Decisions

### Extend the existing custom effect class

The provider-neutral core already has `WorkflowEffectClass::Custom`, and the
pinned Lattice contract already has `EffectClass::Custom`. Add the missing
configuration and profile admissions rather than introducing a new domain
effect class.

### Preserve existing capability order

Lattice contract capability arrays must be strictly ordered. The node kinds
become `["build", "review", "test"]` and effect classes remain ordered as
`Build`, `Test`, `Custom`. Existing build/test plans remain admitted, and the
profile identity changes exactly once because its capability declaration
changed.

### Review remains observation-only

The node kind is a routing label. It does not make an accepted Patchgate review
a Kiln merge, gate, or release decision. The worker report remains evidence for
the selected run/node.

## Risks

- Every Lattice profile now advertises `review` and `Custom`, even where a
  runtime may not implement custom node execution. A runtime that rejects the
  node fails closed at dispatch or observation.
- Adding `Custom` to the shared GitHub config enum allows custom nodes in
  GitHub profiles. Existing build/test profiles retain their serialized shape.
- The review node still needs an operator-selected worker step and an accessible
  Pijul target; capability admission alone does not run a review.
