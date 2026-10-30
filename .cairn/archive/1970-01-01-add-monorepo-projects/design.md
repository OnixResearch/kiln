## Context

Kiln's live CI product is `kiln-web-service`. Projects are `ProjectProfile { id, label, repository_path, reference, admitted_references, workflow }` in `crates/kiln-web-service/src/profile.rs`, admitted with id uniqueness only. They are authored through the Nickel contract `config/web-service.ncl`.

- **Automation:** `crates/kiln-web-service/src/automation.rs` and `engine/automation.rs`. Triggers name one project and one reference, with push and pull-request switches.
- **Run queueing:** `engine/queue.rs` `resolve_source` builds a `LocalGitAdapter` over the full repository.
- **Worker process:** `worker/process.rs` sets `current_dir` to the checkout root.
- **Code review:** `code_review.rs` is bound to one service project and requires CI through `kiln-core` `change_review` policy. Its ownership rules already use repository-relative globs through `change_review::path_matches`.
- **Gates:** `kiln-core` `gating` binds `GateResult` to exactly one candidate-set identity.
- `kiln-core` is `no_std` with `alloc`.

## Goals and Success Contract

Given an exact base, an exact candidate, and admitted project scopes for one repository, Kiln deterministically decides the set of affected projects and the reason for each. It runs exactly those projects, each from its root, and requires CI for exactly those projects in review. The same inputs always give the same affected-set identity.

Whenever an input needed for precision is missing, truncated, unsupported or unparsable, every scoped project of that repository is affected, with an explicit fallback reason. Kiln never skips a project on uncertain facts.

False completion includes:
- a skipped project whose files changed;
- a project skipped because graph parsing failed;
- glob semantics that differ from code-review ownership;
- run identities that collide across projects sharing a revision;
- review satisfied by CI of a different project or an older scope;
- unscoped projects changing behaviour.

## Decisions

### Decision: Scope model and admission

**Choice:** `ProjectProfile.scope: Option<ProjectScope>`, where `ProjectScope` has these fields:

| Field | Meaning | Default |
| --- | --- | --- |
| `root` | Repository-relative directory with `/` separators. Empty means the repository root. | Required |
| `include` | Globs relative to `root`. | `["**"]` |
| `exclude` | Globs relative to `root`. | `[]` |
| `depends_on` | Repository-relative globs. | `[]` |
| `graphs` | A set of `cargo` and `nix`. | Empty |

**Admission rules:**
- `root` and globs are rejected when absolute, when they contain `..`, empty or `.` segments, backslashes or NUL, or when they exceed the named limits.
- Named limits: at most 32 patterns per list, 256 bytes per pattern, and 64 projects per repository.
- Two projects on one repository may overlap; a file may affect several projects.
- Scoped and unscoped projects may share a repository. An unscoped project is always affected.
- Projects sharing a repository must use the same canonical repository path.

**Identity:** the scope identity is BLAKE3 over a canonical encoding of these fields together with the project id, and it is part of the run's source identity.

### Decision: Glob semantics are shared with review ownership

**Choice:** move `change_review::path_matches` into `kiln_core::paths::matches`. Its semantics are unchanged: `*` and `?` within a segment, `**` across segments. Both code-review ownership and monorepo scope use it. A file is in scope when it is under `root`, matches some `include` and matches no `exclude`.

### Decision: Changed-file facts

**Choice:** `ChangedFiles` is either `Exact { paths: BTreeSet<String> }` or `Fallback(reason)`. Renames contribute both the old and the new path. The fallback reasons are:
- `no_base`: a new reference with no previous head, or a manual run;
- `truncated`: more than 10,000 paths or more than 1,048,576 total path bytes;
- `unsupported_provider`;
- `read_error`.

The shell computes the facts with `git2` tree-to-tree diff between the exact base commit and the candidate commit:

| Trigger | Base | Candidate |
| --- | --- | --- |
| Push | Previous recorded head of the reference | New head |
| Pull request or review | Revision's base | Revision's commit |

There is no working-tree access.

### Decision: Build-graph facts from exact manifests

**Choice:** a new pure std crate `kiln-build-graph`. Its input is a bounded map from repository-relative path to manifest bytes; the shell reads these from the exact candidate tree. It performs no I/O.

- **Cargo:** start from the root `Cargo.toml` `[workspace] members` and `exclude`, with globs expanded against the supplied tree listing. For each member, take `[package] name` and the directory, plus every `path` dependency in `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]` and target tables. Dependencies with `workspace = true` resolve through `[workspace.dependencies]`.
- **Nix:** for every `flake.lock`, every node whose `locked.type` is `path` with a relative `path` is resolved against the lock's directory.

The output is `BuildGraph { packages: Vec<Package { id, dir }>, edges: Vec<(dependent_dir, dependency_dir)>, roots_touching_all: Vec<path> }`, together with the root `Cargo.toml`, `Cargo.lock` and `rust-toolchain.toml`. Parse failures, missing members, cycles beyond the bound, or more than 2,048 packages produce `GraphUnavailable(reason)`.

### Decision: Affected decision in kiln-core

**Choice:** `kiln_core::monorepo::affected(scopes, changed, graphs) -> AffectedSet`. The set lists every project with `Affected(reasons)` or `Skipped(reason)`, ordered by project id, and carries a BLAKE3 identity over the canonical encoding.

**Reasons:**
- `unscoped`;
- `path { sample, count }`, where sample is at most 5 paths;
- `declared { pattern, sample }`;
- `graph { kind, from_package, via_package }`;
- `fallback { reason }`.

**Graph rule:** a project consults a graph only when `scope.graphs` contains that kind. The project is affected when a changed file lies in a package directory, and that package is reachable, through the reverse dependency closure, from a package whose directory lies under the project's root. A change to a `roots_touching_all` file affects every project that uses that graph kind.

**Fallbacks:** `GraphUnavailable` affects every project that uses that graph kind, with the reason `fallback { graph_unavailable: <reason> }`. `ChangedFiles::Fallback` affects every scoped project.

**Skip reason:** `no_changes_in_scope`.

### Decision: Dispatch, records and working directory

**Choice:** for one repository event, automation evaluates every enabled push or pull-request trigger whose project is on that repository and whose reference matches.

- It computes one `AffectedSet` per (repository, base, candidate, scope-config identity).
- It queues runs only for affected projects, and records the skipped ones in the trigger's delivery record (`affected: {identity, decision}`).
- `RunRecord` gains an optional `affected { set_identity, reasons, changed: { exact_count | fallback } }`. API responses include it when present; old records deserialize without it.

Manual and scheduled runs keep today's behaviour and record `fallback { no_base }` only when the project is scoped.

The worker starts each job in `checkout.join(root)`, after validating that the joined path stays inside the checkout. A missing root directory fails the job with the `project_root_missing` diagnostic.

### Decision: Gates and review

**Choice:** the project scope identity enters the candidate-set identity of every gate result produced for a scoped project. A changed scope therefore invalidates old results.

Code review computes the `AffectedSet` for the revision's base and commit. The required-CI requirement passes only when every affected project of the review repository has a current clean run for the exact commit and the current scope identity. Each project's status is listed, and skipped projects are shown as not required, with their reason.

The review profile still names its primary project; the other projects on the same canonical repository are discovered from the service profile.

### Decision: Console

**Choice:**
- **Projects:** grouped under a repository heading when several projects share one repository; each card shows `root` (or "Whole repository") and its graph kinds.
- **Run detail:** a "Why this ran" panel with the reasons and the changed-file count or fallback.
- **Automation:** each trigger card shows the path scope.
- **Review:** the requirement panel lists each affected project with its CI state.

Existing tokens, icons, 44px targets, 320px layout and light/dark themes apply.

## Risks / Trade-offs

- **Conservative fallbacks:** these over-run and never under-run.
- **Graph coverage:** the graph only covers Cargo path dependencies and Nix `path` inputs. Registry dependencies and Nix expressions that read sibling directories without being a `path` input are invisible to it. `depends_on` is the explicit escape hatch, and this is documented.
- **Diff cost:** a large diff costs one bounded tree diff per event, not one per project.
- **Run counts:** multiple projects per repository multiply runs. Existing queue capacity and fairness apply unchanged.

## Verification Strategy

- **Pure tests:**
  - `kiln_core::monorepo`: path, declared, graph and fallback reasons, ordering, identity stability, overlap.
  - `kiln_core::paths`: the glob table shared with review ownership.
  - `kiln-build-graph`: workspace globs, `workspace = true` resolution, target tables, flake `path` inputs, every unavailable reason.
- **Web-service tests:**
  - scope admission, positive and negative;
  - changed files with renames, truncation and new references;
  - dispatch of only affected projects, with skipped ones recorded;
  - job working directory, plus a root that escapes or is missing;
  - review required CI per affected project, and stale scope;
  - old records and profiles still loading.
- **Nickel:** fixtures, positive and negative.
- **Real-data check:** run the affected decision over this repository's own history. Pick a commit touching only `crates/kiln-highlight` and confirm that, with Cargo graph scopes per crate, the dependants (`kiln-web-service`) are affected and unrelated crates are skipped.
- **Browser review:** a disposable instance with two scoped projects on one fixture repository.
- **Release:** the full repository gates and deployment.
