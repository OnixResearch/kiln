## Why

The run-bound Patchgate worker step uses a `review` node, but the shared
workflow configuration and pinned Lattice capability profile admit only
`build` and `test` nodes with those effect classes. The service therefore
rejects a review pipeline before dispatch.

## What Changes

- Add a `Custom` effect class to the shared GitHub workflow configuration type
  and map it to `WorkflowEffectClass::Custom`.
- Extend the pinned Lattice profile with the `review` node kind and `Custom`
  effect class while retaining ordered `build` and `test` capabilities.
- Allow `effect_class = "custom"` in the web service workflow contract.
- Add a Lattice lowering test and a service profile fixture containing a
  review/custom node.

## Impact

- **Files**: `kiln-adapter-lattice` profile/tests,
  `kiln-adapter-github` profile/tests,
  `config/lattice-workflow-profile.ncl`, `config/web-service.ncl`,
  `config/fixtures/web-service-patchgate.ncl`, `flake.nix`.
- **Testing**: pinned-profile admission, review/custom lowering, effect-class
  mapping, service profile export, existing crate tests and lifecycle validation.
- **Authority**: admitting a node kind does not grant review acceptance, merge,
  gate or release authority. The review remains a worker/provider observation.
