# Runbook Catalog Migration Plan (`OnixResearch/kiln#101` -> `OnixResearch/lattice#29`)

This document describes how Kiln's separated `runbooks/` directory migrates into a dedicated OnixResearch Lattice runbook catalog repository (`OnixResearch/lattice#29`) without restructuring the bundles or weakening content-addressed verification.

## 1. Pre-Move State in Kiln

All seed runbook definitions live under `runbooks/`, isolated from `crates/` and `config/`:

```text
runbooks/
  MOVE_PLAN.md
  README.md
  registry.seed.ncl
  packs/
    kiln-seed-runbooks/
      manifest.ncl
      workflow-templates/
        flake-update/
        security-audit/
        license-ci-scaffold/
        release-cut/
```

Each bundle under `workflow-templates/<id>/` is a Schema 2 `lattice.workflow-template-bundle.v1` directory, and `runbooks/packs/kiln-seed-runbooks/manifest.ncl` is a `lattice.operation-pack-manifest.v1` catalog-only pack (`operations = []`, `runtimes = []`, `allowed_capabilities = []`, `licenses = ["MIT OR Apache-2.0"]`).

Kiln's `RunbookRegistry` (`config/runbook-registry.ncl`, `runbooks/registry.seed.ncl`) pins each runbook's exact `bundle_identity` and `workflow_revision` (`64`-hex BLAKE3 digests), and `kiln-web-service --runbooks-dir` verifies on-disk bundle bytes against `manifest.ncl` and `RunbookDefinition.lattice.bundle_identity` before preview or dispatch.

## 2. Boundary Reconciliations Preserved Across Migration

1. **`security-audit` Mode (`lattice#27`)**: Lattice's `classification.rs` classifies every `Shell` node as `Mutating`. Accordingly, `security-audit/manifest.ncl` declares `mode = "mutating"` with explicit `operator_notes` and `non_claims`, while Kiln's `RunbookRegistry` declares `mode = "read_only"` and `fanout.write_scopes = []` and enforces read-only execution without publisher credentials.
2. **Mutating Approval Gates (`lattice#28`)**: `flake-update`, `license-ci-scaffold`, and `release-cut` include a terminal `kind = "approval"` node (`ApprovalNodeConfig`) in `workflow.ncl` and `completion = "await_proposal"` in `orchestration.ncl`. On Lattice, parameterized approval workflows are dispatched via `workflow_template::bind` + `save_draft` -> `run` -> `approvals decide` -> `workflow resume` (never `lattice run --input`). On Kiln, worker steps run without push/PR/tag tokens, and `approval_gate = "kiln_publication"` requires explicit approval before Kiln's separately credentialed publisher (`GrantRole::Publisher`) acts.

## 3. Step-by-Step Migration Procedure

1. **Create Target Catalog Repository**: Initialize the dedicated OnixResearch runbook catalog repository (`lattice#29`) with a top-level `packs/` directory.
2. **Copy Pack Tree Verbatim**: Copy `runbooks/packs/kiln-seed-runbooks/` to `<catalog-repo>/packs/kiln-seed-runbooks/`, preserving regular files and relative paths (no symlinks).
3. **Verify Pack & Bundle Pins**:
   - Format Nickel files (`nickel format --check`).
   - Run `lattice pack inspect --pack packs/kiln-seed-runbooks`.
   - Run `lattice pack lock --pack packs/kiln-seed-runbooks --confirm-trust-local --allow-license "MIT OR Apache-2.0" --destination /tmp/pack.lock.ncl --confirm-create`.
   - Run `lattice pack catalog-install --pack packs/kiln-seed-runbooks --lock /tmp/pack.lock.ncl --catalog <temp-catalog> --confirm-trust-local --allow-license "MIT OR Apache-2.0"` and confirm `observed_identity` matches `workflow_bundles[].bundle_identity` for all 4 bundles with `rejected_count == 0`.
4. **Commit & Pin Catalog Release**: Commit `<catalog-repo>` and record the full commit hash (`<commit>`) and `pack_identity` (`b3:<hex64>`).
5. **Operator Intake in Lattice**:
   - `lattice pack catalog-fetch --repository <catalog-url> --commit <commit> --subdirectory packs/kiln-seed-runbooks --pack-identity <b3:...> --destination <local-pack> --confirm-fetch`
   - `lattice pack lock --pack <local-pack> --destination <lock> --confirm-trust-local --allow-license "MIT OR Apache-2.0" --confirm-create`
   - `lattice pack catalog-install --pack <local-pack> --lock <lock> --catalog <lattice-config-dir>/workflow-templates --confirm-trust-local --allow-license "MIT OR Apache-2.0" --confirm-create`
   - `lattice pack catalog-list --catalog <lattice-config-dir>/workflow-templates`
   - `lattice authority inspect <workflow>` (or bind + draft workflow for approval-gated runbooks).
6. **Update Kiln References & Close `#101`**:
   - Keep `RunbookRegistry` pinning the exact `bundle_identity` and `workflow_revision` digests from the catalog release (pointing `--runbooks-dir` at the fetched/installed catalog directory).
   - Remove `runbooks/packs/` from `OnixResearch/kiln` and close `OnixResearch/kiln#101`.

## 4. Non-Claims

- Pinning a Git commit, `pack_identity`, `bundle_identity`, or `workflow_revision` proves exact bytes only; it grants no execution, review, merge, or release authority (`grants_authority: false`).
- Migration never bypasses `RunbookGrant`, `ServiceProfile`, `ApprovalNodeConfig`, `GrantRole::Publisher`, or `lattice authority inspect`.
