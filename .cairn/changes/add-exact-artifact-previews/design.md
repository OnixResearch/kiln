## Context

Existing BuildEffectPlan and BuildEvaluationObservation bind attempt/candidate/pipeline/plan/policy/provider/products. Runtime closure evidence must supplement rather than replace this admission. TryNix exposes startVM, share.write, vm.run and window.trynix.run; its general website also resolves moving package names and is therefore not a safe launch target.

## Goals and Success Contract

An exact admitted descriptor can launch only its exact runtime closure on one content-pinned existing Linux engine. Stale review source, failed build, changed member or runner, incomplete closure, missing member, missing trust and limits are rejection. Launch phases and failures are preview-only observations. No renderer sees build inputs, credentials or source trees.

## Decisions

Core records provider-neutral exact facts and finite limits and validates complete transitive runtime membership. Shell verifies strict Nix Ed25519 fingerprint signatures under an independently selected typed operator policy, native read-only Nix metadata and every dumped NAR SHA-256 before export. It never realises or builds. The admitted product must already contain exact complete native NAR facts; path-only product identity is insufficient. Existing build signatures are retained, and later cache signatures may attest the same exact NAR. Engine identity is BLAKE3 over its bounded manifest; every runtime module, vendor byte, kernel, guest image, machine and snapshot also matches a full pinned native SHA-256. Browser invokes the existing TryNix startVM/share/agent protocol, not its unrestricted website. The separate-origin disposable guest receives only runtime NARs; no derivation, checkout, CI credentials or build-only inputs cross this boundary. Payload bytes, members, guest starts, HTTP requests, output and expiry are bounded. Normal stop/expiry removes exports; finite owned leases plus startup/explicit scheduled cleanup handle crash leftovers without shared-store GC.

The review action asks the service for the selected exact revision, displays exact run/attempt/artifact/engine facts, and opens an opted-in isolated preview. No preview callback writes any CI object. Shell integration is explicit; absent admitted artifact evidence or prebuilt engine is unavailable rather than synthesized.

## Verification Strategy

Positive and negative Rust tests cover exact binding, closure, native signatures, hashes, bounds, catalog filtering and retention. Node tests cover stale identity, unsupported capabilities and literal entrypoints. Main agent runs coordinated focused tests and actual clean/warm browser cohorts after wiring; no intermediate builds or campaigns. Canonical admission requires genuine CI plan, observation, current source/run binding and independent runtime/trust policy. The separate engine-cohort command has no invented CI fields and cannot create a review catalog. Its actual signed public CLI/engine proof remains distinct from end-to-end current-CI qualification.

## Risks and Trade-offs

TryNix is not a hostile-isolation guarantee; the renderer uses a clean disposable browser context on a separate loopback origin. Native signatures require explicit admitted public keys; upstream cache possession never implies tenant/source trust. Public immutable engine acquisition is read-only and permitted, while builds and campaigns stay coordinated. Shared Nix store mutation/GC is forbidden: the real public CLI cohort is acquired into an owned root and subsequently read with Nix's read-only-local-store capability. A crashed process cannot run its own timer, so deployments must schedule the bounded cleanup command for owned scratch. No deployment is performed here.

## Repeatable local consumer

Maintenance owner: Kiln web-service maintainers. Actual supported consumers are the authenticated review page's exact artifact action and the disposable artifact-preview example; the latter never borrows console cookies or authority. Shared server registration/configuration belongs to the integration owner.

The already-built engine source is https://github.com/fzakaria/trynix/tree/7e7e793594cb58efb48279b17b4e15bbc6fbe91b. The supported runtime byte cohort is retained in preview/engine_pins.rs, including the existing engine-20260913-2056 release and js.2665d2d35976 modules. Acquire with `python3 scripts/acquire-preview-engine.py OWNED_NEW_ENGINE_TREE`; the script uses bounded public reads and existing matching store assets, never builds. Native SHA-256 interoperability remains separate from Kiln BLAKE3 identities.

For engine-only qualification, use the exact public cached product /nix/store/vk1ih0la8pfs92v8cy5vkv48kgmi14db-trynix-emubench-static-x86_64-unknown-linux-musl-1 from https://trynix.dev/examples/cache. Its single-member NAR is 1556888 bytes and is signed by the explicitly selected trynix-examples-1 key. The literal `noop` argument prints emubench: done without running a performance campaign. fixtures/artifact-preview/engine-cohort.ncl supplies the typed public-cohort policy as a function of finite expiry. This product has no canonical Kiln CI binding.

Native consumer commands after coordinated compilation:

```text
artifact-preview pin-engine PREBUILT_ENGINE_TREE MODULE_DIR
artifact-preview produce PLAN.json OBSERVATION.json CURRENT_BINDING.json SELECTION.json PREBUILT_ENGINE_TREE ENGINE_MANIFEST.json ABS_NIX ABS_NIX_STORE OWNED_SCRATCH
artifact-preview serve REQUEST.json CURRENT_BINDING.json SELECTION.json PREBUILT_ENGINE_TREE ENGINE_MANIFEST.json ABS_NIX ABS_NIX_STORE OWNED_SCRATCH NEW_CATALOG.json
artifact-preview engine-cohort SELECTION.json PREBUILT_ENGINE_TREE ENGINE_MANIFEST.json ABS_NIX ABS_NIX_STORE OWNED_SCRATCH
artifact-preview cleanup OWNED_SCRATCH
```

Set KILN_PREVIEW_STORE_ROOT to an already-populated owned Nix root to avoid shared-store mutation. Normal produce/serve require genuine exported admitted build records and cannot substitute the engine-only cohort. Serve creates a new bounded catalog; the authenticated review handler filters it against current exact canonical successful terminal full/clean source runs. Browser evidence is available at window.kilnPreviewObservation and distinguishes artifact discovery, payload hydration, guest boot and program readiness. Preview outcomes cannot update CI/review/merge state. Normal full qualification stays unclaimed unless those actual canonical build records and a real current review action are exercised.

