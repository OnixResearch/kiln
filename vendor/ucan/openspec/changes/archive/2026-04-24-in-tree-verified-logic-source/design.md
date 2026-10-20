## Context

`flake.nix` currently imports `verified-logic` with `git+file:../verified-logic` and then reconstructs a synthetic source tree so Cargo can find a sibling `../verified-logic` path dependency inside the Nix sandbox. That works today but emits a deprecation warning, and earlier experiments showed that `path:../verified-logic` cannot replace it because flake path inputs cannot escape the current flake tree.

The sibling `verified-logic` repository may also be private or SSH-only, so an in-tree git submodule would still make fresh clones and CI depend on credentials plus recursive checkout behavior.

## Goals / Non-Goals

**Goals**
- Remove the relative `git+file:` deprecation warning from `nix flake check`
- Make plain Cargo and Nix resolve the same `verified-logic` source path
- Keep a deterministic workflow for refreshing the vendored source from the sibling upstream repo

**Non-Goals**
- Change UCAN APIs or semantic behavior
- Modify the upstream `verified-logic` repository structure
- Preserve the old sibling `../verified-logic` Cargo path relationship inside this repo

## Decisions

### 1. Vendor a checked-in crate snapshot instead of using a git submodule

**Choice:** Track a vendored `verified-logic` snapshot under `vendor/verified-logic`.

**Rationale:** A vendored directory is present in every clone, works in pure flake evaluation, and avoids SSH/private-repo submodule failures. It also lets Cargo and Nix build from the same checked-in files.

**Alternative:** A git submodule was rejected because it would add recursive-clone ergonomics, require credentials for private remotes, and still leave more ways for fresh clones to fail before a build even starts.

**Implementation:** Commit the vendored crate source into this repo and point the root manifest at `vendor/verified-logic`.

### 2. Remove the synthetic Nix source tree and sibling symlink shim

**Choice:** Simplify `flake.nix` to build directly from the repo source tree once `verified-logic` is vendored.

**Rationale:** With the dependency already in-tree, Nix no longer needs a non-flake sibling input, a combined copy step, or `postPatch` symlink surgery. That removes the warning and reduces build indirection.

**Alternative:** Keeping the current path dependency and only changing the Nix fetcher was rejected because `path:../verified-logic` is not valid in flake-pure evaluation.

**Implementation:** Delete the `verified-logic-src` input, use the normal cleaned repo source, and drop the `postPatch` hook.

### 3. Refresh the vendored snapshot from a committed upstream revision

**Choice:** Add a small sync helper that imports a committed `verified-logic` revision from the sibling checkout and records the imported revision in-repo.

**Rationale:** The source of truth still lives in the sibling repo, but the vendored snapshot inside `ucan` must stay deterministic. Importing from `git archive HEAD` avoids accidentally copying uncommitted sibling changes.

**Alternative:** Manual copy-paste was rejected because it is error-prone and does not reliably record provenance.

**Implementation:** Provide `scripts/sync-verified-logic.sh`, vendor the committed crate source, and write the imported revision to `vendor/verified-logic-revision.txt`.

## Risks / Trade-offs

**Vendored snapshot can drift from upstream** → Mitigate with a refresh script, recorded revision metadata, and documented update workflow.

**Vendored source increases repo size** → Mitigate by importing only the crate files needed for dependency builds instead of the entire sibling repository history or working tree.
