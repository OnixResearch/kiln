## Why

The operator asked for monorepo support, with path-scoped projects plus affected-project detection from the real build graph. Today a Kiln CI project is bound to exactly one repository: `ProjectProfile` has `repository_path` and `reference`, and nothing scopes it by path. Triggers filter by reference only, every job runs over the whole checkout, and the code-review profile names one project. A repository holding several independently built components can therefore only run everything on every change, or be split into fake repositories.

## What Changes

- **Scoped projects:** a project gains an optional `scope`:
  - a repository-relative `root`;
  - include and exclude globs;
  - declared dependency globs;
  - the build graphs to consult (`cargo`, `nix`).

  Several projects may share one repository. A project without `scope` keeps today's behaviour: the whole repository, always affected.
- **Changed files:** a pure core computes them from exact facts. The shell reads the changed-file list (with renames) between the exact base and candidate revisions from the Git object store, under named bounds.
- **Build graphs:** a new pure `kiln-build-graph` crate derives graph facts from manifest bytes read at the exact candidate revision:
  - the Cargo workspace members and their path dependencies, including `workspace = true` dependencies resolved through `[workspace.dependencies]`;
  - Nix `flake.lock` inputs of type `path`.
- **Affected projects:** a pure `kiln_core::monorepo` module decides which projects are affected and records a reason for each:
  - direct path match;
  - declared dependency;
  - Cargo or Nix graph edge;
  - conservative fallback (no base, truncated diff, unsupported provider, or graph unavailable).

  The affected set has a BLAKE3 identity.
- **Automation:** push and pull-request triggers queue runs only for affected projects. Each run records the affected-set identity, the reasons and the changed-file facts; unaffected projects are recorded as skipped, with a reason. Manual and scheduled runs keep today's behaviour.
- **Job working directory:** jobs of a scoped project start in the project root over the full exact checkout.
- **Gates and review:** candidate and gate identities include the project scope identity. Code review requires current CI for every affected project, and unaffected projects never block.
- **Console:**
  - projects are grouped by repository, with each project's root shown;
  - run detail explains why the project ran;
  - review shows CI status per affected project;
  - automation shows the path scope.
- **Configuration:** the Nickel contract in `config/web-service.ncl` gets the scope contract, with positive and negative fixtures. Documentation is updated.

## Impact

- **Consumers:** operators running Kiln CI over repositories with several components, starting with this workspace's Rust/Nix monorepo layouts, and reviewers who need the right required checks.
- **Owners:** Kiln maintainers own `kiln-core`'s monorepo module and `kiln-build-graph`. Kiln web-service maintainers own the shell, automation, review integration and console.
- **Durable contribution:** one deterministic, explainable affected-project decision, shared by automation and review.
- **Affected areas:**
  - `crates/kiln-core`;
  - the new `crates/kiln-build-graph`;
  - `crates/kiln-web-service` profile, automation, queue, worker, code review, API and console;
  - `config/web-service.ncl` and fixtures;
  - docs.

## Out of Scope

- Sparse checkouts: jobs see the full exact snapshot.
- Build-graph sources other than Cargo path dependencies and Nix `path` flake inputs. No Bazel, Buck, npm workspaces, or evaluation of Nix expressions.
- Changed-file computation for Jujutsu and Pijul sources. They are not CI project providers today; any future such provider is classified as `unsupported_provider` and falls back to all projects.
- The hosted CI composition (`kiln-hosted`) pipeline model. Its projects keep repository-wide behaviour.
- Monorepo-aware Radicle or GitHub status publication beyond today's per-run reporting.
