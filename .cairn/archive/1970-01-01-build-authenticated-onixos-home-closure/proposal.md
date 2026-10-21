# Build the real OnixOS home closure inside authenticated CI

## Why

The maintained pilot currently realises every launcher dependency during preparation, including the generated activation script and home files. Its authenticated build therefore proves only launcher construction. The existing receipt states this boundary correctly, but it does not establish authenticated construction of those consumer outputs.

## What Changes

- Expose the real launcher's activation and home-file derivations as explicit preparation-boundary facts. Prepare only external toolchain dependencies; reject any preparation closure containing a consumer derivation or output. r[kiln.onixos_pilot.closure.preparation]
- Bind a fresh build identity into the real inventory's Git configuration so all three consumer outputs must be absent immediately before authenticated dispatch. Build the unchanged consumer lowering through the maintained runtime without running activation. r[kiln.onixos_pilot.closure.construction]
- Exercise a validly typed negative consumer configuration whose home-file targets conflict at build time, preserving actual Nix diagnostics and canonical failure. r[kiln.onixos_pilot.closure.construction]
- Preserve original-operation restart/reconciliation, exact rooted launcher retention, idempotent release and fresh UCAN observation; retain before/after output and closure identities under the exercised cohort. r[kiln.onixos_pilot.closure.evidence]

## Impact

- Files: the pilot's existing Nix projection and Python runner, a small pure preparation-policy helper and deterministic regressions, the matching Nix check, operator documentation and native lifecycle evidence.
- Current consumer and adoption: the existing frozen OnixOS `examples/home-activation` inventory and `home-tools` service, through the documented explicitly selected pilot. OnixOS continues to own service and lowering meaning; Kiln maintainers own composition, preparation admission and the repeatability rail.
- Immediate outcome: authenticated construction of the launcher, activation script and generated home files. Durable capability: repeatable proof that preparation did not prebuild the selected consumer closure.

## Out of Scope

No activation execution, home mutation, autonomous forge trigger, remote status or publication write, production deployment, provider fallback, new scheduler, general reproducibility claim or release authority. Historical launcher-only evidence remains unchanged and explicitly distinguished.
