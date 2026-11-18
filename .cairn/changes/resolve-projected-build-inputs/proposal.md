## Why

Issue [#77](https://github.com/OnixResearch/kiln/issues/77) needs an explicit build profile before a projected view counts as runnable or reuses evidence. A project selection is not a complete build-input set: Cargo loads every workspace member, inherits fields from the workspace manifest, needs the root lockfile for a locked build and reads configuration and toolchain files from the build root. The projection contract (#75) can prove a view is structurally complete and #78 can materialize it, but nothing says which source files a build of that view needs, refuses a view that lacks them, or binds build evidence to the exact inputs, command and toolchain.

## What Changes

- `kiln-build-graph` derives Cargo layout facts from exact manifest bytes: each package's name, directory and workspace; each workspace's literal and glob members; path-dependency edges; path dependencies that resolve outside the repository; and whether a package has registry or Git dependencies.
- A pure planner in `kiln-core` (`projection::build_inputs`) accepts a completeness witness for a build, the full candidate's facts, the layout facts and a versioned build request (`kiln.build-input.request.v1`). The Cargo compatibility profile requires, at their source paths, every file of each target package, of its path-dependency closure and of every workspace member Cargo will load. It also requires the workspace manifest, lockfile and the Cargo configuration and toolchain files at the build root and its ancestors. The Nix compatibility profile plans only views that hold the whole source tree, because Nix expressions can read siblings that no path input declares. Every fault is refused before any execution.
- An admitted plan (`kiln.build-input.plan.v1`) carries a BLAKE3 identity over the full candidate, the projection manifest and content, the profile, the required inputs and their objects, the command, the toolchain and the environment. Evidence applies only to a plan with that identity.
- A typed Nickel corpus (`kiln.build-input-corpus.v1`) defines a two-project Cargo workspace with a shared path dependency, inherited fields, a root lockfile and Cargo configuration, and several projections of it. A harness in `kiln-web-service`, shared by an off-by-default example and a test, plans each projection and materializes the planned ones through #78's materializer. It edits the shared dependency through one view and selects the affected projects. Behind the `build-input-fixtures` feature, it also runs the planned command in an attempt overlay with a private target directory and a cleared environment.

## Capabilities

### Modified Capabilities

- `source-projection`: an admitted complete projection can be planned as a build input under a versioned profile, and that plan can be qualified by running it through the projected materializer.

## Impact

New `kiln_core::projection::build_inputs` module, `kiln_build_graph::cargo_layout`, `config/build-input-contracts.ncl` with fixtures, a `kiln-web-service` harness, example and test, a `build-input-fixtures` feature with its Nix check, and documentation. Planning is opt-in: no worker job, dispatch or default changes.

## Non-goals

Relocation and generated-manifest transformations, generated-input providers, plans that span several workspaces, registry and Git package closure (pinned only by the lockfile here), restricted-runtime confinement, current-authority checks (#69/#70), disclosure projections of plans for restricted workers, joining plan identities to #46/#64 verification evidence and the local build cache, and selecting the projected route in a live worker job.
