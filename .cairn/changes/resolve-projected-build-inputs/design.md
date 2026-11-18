## Context

`kiln_core::projection::resolve` produces an `AdmittedPartialView`; `require_complete(Build)` yields a `CompleteProjection` once coverage is complete, required dependencies are present and generated entries are admitted. `projection::materialization` (#78) turns that witness into an immutable input and attempt overlays. `kiln-build-graph` already parses every `Cargo.toml` and `flake.lock` of an exact tree into `monorepo::BuildGraph` facts for affected-project decisions. It does not report which directories a Cargo workspace loads or which dependencies come from outside the tree.

Cargo behavior observed with Cargo 1.88.0 on a three-package workspace: a literal member that is absent, or present without its targets, stops every build of the workspace; a glob member that is wholly absent is ignored, but one present without its targets stops the build; `--locked` without the lockfile refuses to create it; and a build without `--locked` rewrites the lockfile when members are missing.

## Goals and Success Contract

A plan is admitted only when every source file the profile requires is in the view at its own path, from the same candidate. A plan names the build root, the required inputs with their roles and objects, the command, the toolchain and the environment, and its identity changes when any of them changes. A plan never claims that the build succeeds: only a qualified run of the plan in a materialized overlay shows that, and a view with an undeclared read must fail rather than read a host checkout.

## Decisions

### Decision: layout facts from manifest bytes, decisions in the pure core

**Choice:** `kiln_build_graph::cargo_layout(&ManifestTree)` returns `CargoLayoutResult`: either `Available(CargoLayout)` or `Unavailable(reason)`, under the same admission and parsing as `cargo_graph`. A `CargoLayout` holds:

- packages with name, directory, workspace root, a count of path dependencies outside the repository, and whether any dependency is a registry or Git dependency that no path `[patch]` replaces;
- workspaces with literal member directories and glob-expanded member directories, minus `exclude`;
- path-dependency edges between package directories, including development, build, target-specific and `[patch]` forms.

The planner in `kiln_core::projection::build_inputs` consumes these facts and performs no I/O.

**Rationale:** the existing parser already resolves membership, inheritance and patches, so one parse feeds both graphs and layouts. Keeping decisions in `kiln-core` keeps the functional-core boundary and lets tests use synthetic facts.

### Decision: Cargo compatibility rules are conservative

**Choice:** under `cargo_compatibility.v1`:

1. The view must keep the identity layout: every native entry's destination equals its source path. A relocated entry is refused as an unqualified transformation; a generated entry is refused because no generator profile is modeled.
2. Targets are package names that resolve to exactly one package. All targets share one build root: their workspace root, or the package directory for a package outside any workspace.
3. Loaded packages are the targets, every literal member of the build root's workspace, and every glob member that the view holds any file of. Required packages are the path-dependency closure of the loaded packages.
4. A required package needs every source file whose deepest package directory is that package: sources, build scripts, tests, fixtures and assets alike.
5. The build root needs its `Cargo.toml` and `Cargo.lock`. A missing lockfile is refused, because a locked build cannot create it. `.cargo/config`, `.cargo/config.toml`, `rust-toolchain` and `rust-toolchain.toml` at the build root and each in-repository ancestor are required when the source has them.
6. A required package with a path dependency outside the repository is refused; registry and Git dependencies make the plan's external packages `pinned_by_lockfile`, never part of the source closure.

Every fault is a sorted diagnostic; absent inputs are sampled with a count.

**Rationale:** Kiln cannot see reads across package boundaries, so the profile requires whole packages and states the gap instead of guessing. The workspace rules follow the observed Cargo behavior; refusing a partially held glob member avoids a build that would fail when it loads the workspace.

### Decision: Nix plans need the whole source tree

**Choice:** under `nix_compatibility.v1`, targets are flake directories from the Nix graph. A view that omits any source file is refused with `restricted_nix_view`. A view holding the whole tree is planned with the targets' path-input closure as required packages and the root `flake.nix` and `flake.lock` when present. External flake inputs are `pinned_by_lockfile`.

**Rationale:** a Nix expression can read a sibling directory that no path input declares, and Kiln does not evaluate Nix to observe those reads. A restricted Nix view is therefore explicitly unsupported rather than reported complete.

### Decision: one plan identity binds evidence

**Choice:** `BuildInputPlan` is built only by the planner and serialized for receipts. Its identity is BLAKE3 over the plan schema, the profile, the repository, exact revision and canonical tree, the manifest and content identities, the build root, the sorted targets and packages, every required input's path, role, object and origin identity, the external-package state, the ordered command, the toolchain and the sorted environment. `BuildInputPlan::applies_to` accepts evidence only when it carries that identity.

**Rationale:** a layout, input, lockfile, toolchain, command or environment change then invalidates cached results and verification applicability without a separate comparison table.

### Decision: qualify through the materializer with a cleared environment

**Choice:**

- `config/build-input-contracts.ncl` types the corpus `kiln.build-input-corpus.v1`: files with text and an executable bit, project scopes for affected-project decisions, cases pairing a projection definition with a build request, and one shared edit. `config/fixtures/build-input-corpus-positive.ncl` exports `fixtures/build-inputs/corpus.json`. Negative fixtures cover a duplicate path, an unsupported profile and a command without a program. The fixture check script refuses a stale export.
- The harness commits the corpus to a temporary repository with a working checkout, admits and resolves each case, and plans it. Planned cases are materialized through `projection::materialization` and the eager local worker materializer. The shared edit is written into one planned case's overlay, mapped back to its origins and passed with the full candidate's Cargo graph to `monorepo::affected`.
- With `build-input-fixtures`, the harness checks that `cargo --version` and `rustc --version` report the declared toolchain version, then runs each planned command at the build root of a fresh attempt overlay. The environment is cleared except the declared environment, `PATH`, and private `HOME`, `CARGO_HOME` and `CARGO_TARGET_DIR` directories. Afterwards it compares the overlay's entries with the plan.
- The receipt (`kiln.build-input.qualification.receipt.v1`) records each plan or refusal, each execution, the shared-edit decisions, the observed toolchain and timings. The default test compares it without executions, observed toolchain and timings to `fixtures/build-inputs/expected.json`. The feature test compares everything except the observed toolchain and timings.

**Rationale:** running Cargo is a process effect; the repository gates such fixtures behind a feature with a dedicated Nix check. A cleared environment and private target directory keep the host configuration and caches out of the result, and checking the overlay afterwards shows that the build did not write into its source inputs.

## Risks / Trade-offs

- Whole-package inputs over-approximate what a build reads; a view must select whole packages.
- A planned view can still fail its build when code reads across package boundaries. Only qualification reveals it, which is why a plan alone is not a runnable claim.
- Cargo's tolerance of a lockfile that lists packages outside a restricted view is observed behavior of Cargo 1.88.0. The toolchain is part of the plan identity, so another Cargo version needs its own qualification.
- Plans and refusal diagnostics name hidden paths. They are trusted controller facts; disclosure to restricted workers stays with #69/#70.

## Verification Strategy

Pure planner tests cover an admitted Cargo plan and each refusal: unavailable or stale facts, relocated and generated entries, unknown and ambiguous targets, several build roots, an absent dependency, configuration, toolchain or lockfile, an absent literal member, a partly held glob member and an outside path dependency. They also cover external packages, a refused restricted Nix view and an admitted full one, and identity changes for layout, inputs, toolchain, command and environment. `kiln-build-graph` tests cover layout facts over manifest fixtures and an unavailable layout. Run the harness test with and without `build-input-fixtures`, the example, the Nickel fixture gate and its Nix check, then Clippy, formatting, Cairn gates, validation and traceability.
