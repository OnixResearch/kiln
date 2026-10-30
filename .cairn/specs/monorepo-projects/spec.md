# Monorepo Projects Specification

## Purpose

Defines the `monorepo-projects` capability.

## Requirements

### Requirement: Path-scoped projects

r[kiln.monorepo.scope]
A CI project MUST accept an optional scope with a repository-relative root, include and exclude globs, declared dependency globs and the build-graph kinds to consult. Several projects MUST be able to share one canonical repository. An unscoped project MUST keep repository-wide behaviour. Scope admission MUST reject absolute paths, `..`, empty or `.` segments, backslashes, NUL, and lists or patterns beyond the named limits. Every scope MUST have a BLAKE3 identity bound to its project.

#### Scenario: Invalid scope

r[kiln.monorepo.scope.scenario.invalid]
- GIVEN a project scope whose root contains `..`, or whose include list exceeds its limit
- WHEN the service profile is admitted
- THEN admission MUST fail and name the project and field, and no project of that profile is served

### Requirement: Exact changed-file facts

r[kiln.monorepo.changed]
Changed files MUST come from a tree diff of exact base and candidate commits in the repository object store, including both paths of a rename, under named path-count and byte bounds. A missing base, a truncated diff, an unsupported provider or a read error MUST yield an explicit fallback rather than a partial list.

### Requirement: Build-graph facts

r[kiln.monorepo.graph]
`kiln-build-graph` MUST derive Cargo workspace member and path-dependency edges, including `workspace = true` dependencies, and Nix `flake.lock` path-input edges from bounded manifest bytes read at the exact candidate revision, without I/O. Unparsable or over-bound input MUST yield an explicit unavailable reason.

### Requirement: Affected-project decision

r[kiln.monorepo.affected]
The affected decision MUST be a pure deterministic function of scopes, changed-file facts and graph facts. It MUST mark a project affected when any of the following holds, recording the reason:

- a changed file is in its scope;
- a changed file matches its declared dependencies;
- a changed package is reachable, through the reverse dependency closure of a consulted graph, from a package under its root;
- the inputs fall back.

Otherwise it MUST mark the project skipped. The decision MUST carry a BLAKE3 identity.

#### Scenario: Graph dependant

r[kiln.monorepo.affected.scenario.graph]
- GIVEN projects scoped to crates A and B with Cargo graphs, where B depends on A by path
- WHEN only a file under A changes
- THEN A MUST be affected by path, B MUST be affected by graph with the edge recorded, and an unrelated crate project MUST be skipped

#### Scenario: Uncertain facts never skip

r[kiln.monorepo.affected.scenario.fallback]
- GIVEN a truncated diff, a missing base, or an unparsable workspace manifest for a graph-consulting project
- WHEN the decision is made
- THEN every project depending on that fact MUST be affected, with an explicit fallback reason

### Requirement: Affected-only dispatch

r[kiln.monorepo.dispatch]
Push and pull-request automation MUST queue runs only for affected projects, recording the affected-set identity, reasons and changed-file summary on each run, and the skipped projects with their reasons on the delivery. Jobs of a scoped project MUST start in the project root inside the exact checkout. A root that escapes the checkout or is missing MUST fail with an explicit diagnostic. Manual and scheduled runs MUST keep today's behaviour.

### Requirement: Scope-bound gates and review

r[kiln.monorepo.gating]
Gate results and required-CI checks for scoped projects MUST bind the project's scope identity. Review MUST require a current clean run for the exact commit for every affected project on the review repository. Skipped projects MUST NOT block, and CI from another project or an older scope MUST NOT satisfy a requirement.

### Requirement: Monorepo console

r[kiln.monorepo.console]
The console MUST group projects that share a repository and show each project's root and graph kinds. It MUST show why a run ran, the path scope of each trigger, and per-project required-CI state in review, using existing design tokens, accessibility and narrow-layout rules.
