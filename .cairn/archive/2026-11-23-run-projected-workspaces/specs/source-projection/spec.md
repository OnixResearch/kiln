# Source projection delta

## ADDED Requirements

### Requirement: Projected workspace routes

r[kiln.source.projection.route] A projected workspace MUST reach a restricted runtime only through a `kiln.projected-workspace-route.v1` route derived from the admitted definition, the completeness witness, the materialization plan and the dispatch pin of one attempt. The route MUST name the pin, the input and overlay roots in one materialization store, the `lattice-bwrap` runtime, the build plan the attempt runs, read paths and write paths relative to the overlay, the authored definition and resolved manifest it was derived from, and an identity over every field. Read paths MUST be the top-level components of every planned destination and, for a write, of every creation destination. Write paths MUST exist only for a completeness witness for a write: each destination whose native origin lies within a writable source path, and each creation destination whose every mounted source path, every native origin planned at or below it and the source prefix of every mount placed below it lie within one. The route MUST grant no read, write or execution authority beyond those paths, and MUST NOT name the host checkout, the Git database, any other store path or a path with a `.git` or `.jj` component. Its store MUST NOT lie at or below a host root the restricted runtime mounts (`/nix/store`, `/usr`, `/bin`, `/lib`, `/lib64`, `/run/current-system/sw/bin`).

#### Scenario: A route grants only writable origins

r[kiln.source.projection.route.scenario.derived]
- GIVEN a writable view with an aliased shared origin, creation destinations, an origin of another source path mounted inside a creation destination, and a writable grant over some source paths
- WHEN its route is derived and its overlay prepared for it
- THEN both aliases of a writable origin and the writable creation destinations MUST be write paths, a creation destination holding an unwritable origin or mount MUST NOT be granted whole, every other planned destination MUST stay read-only, a build route MUST have no write path, and derivation MUST be reproducible

#### Scenario: Unsafe or crossed routes are refused

r[kiln.source.projection.route.scenario.refused]
- GIVEN a build witness with writable paths, an unsafe writable path or store, a store at or below a runtime-mounted host root or, at dispatch, at or below the resolved target of such a root that is a link, a present root that cannot be resolved, a destination with a `.git` or `.jj` component, a bound violation, a pin the plan does not derive, another view's plan or a build plan of another manifest
- WHEN a route is derived, or a recorded route with any changed field is verified
- THEN it MUST be refused before any effect

#### Scenario: Dispatch runs only the exact route

r[kiln.source.projection.route.scenario.dispatch]
- GIVEN a dispatch plan that pins a projected workspace
- WHEN `kiln-lattice-dispatch` or `kiln-implementation-fixture bind-execution` receives a worker request
- THEN it MUST refuse, before any durable step or transport, a missing or invalid route, a route for another pin, a route for an unpinned plan, a route that derived again from its definition and manifest for the plan's own permitted paths has other read or write paths, a linked overlay root, and a request whose workspace root, read paths, write paths or network differ from the route; and `kiln-lattice-dispatch` MUST refuse a pinned plan or a supplied route with `projection-requires-operator` under any authority profile that does not enable projection, the local-operator profile included

### Requirement: Restricted projected runtime

r[kiln.source.projection.runtime] A projected attempt MUST run under Lattice's bubblewrap provider with only its route's paths mounted below a private workspace. The worker MUST read its planned entries and write only its route's write paths; it MUST NOT read the host checkout, its Git database or the materialization store backing its input, reach a host socket, or write outside the route's write paths.

#### Scenario: Two projects build through the dispatcher

r[kiln.source.projection.runtime.scenario.qualified]
- GIVEN the two-project Cargo corpus and a two-project Nix flake, each resolved, planned, materialized and routed for a claimed local attempt, and an authority configuration selecting a verified profile with projection enabled
- WHEN each is dispatched through `kiln-lattice-dispatch` to the real Lattice binary and its planned command runs inside the restricted runtime
- THEN each planned build MUST succeed inside the runtime except the beta Cargo build, whose failure MUST be observed and its recorded output MUST name the missing undeclared alpha file (any other failure MUST NOT qualify), the same request under the local-operator profile MUST be refused for projection, and the receipt MUST record the route and the runtime observations

#### Scenario: Confinement negatives

r[kiln.source.projection.runtime.scenario.confined]
- GIVEN a dispatched projected attempt
- WHEN the worker tries to read the host checkout, its `.git`, the store's input and overlay backing paths, to connect to a host UNIX, abstract or loopback socket, or to write a read-only planned entry, the workspace root or any path outside its write paths
- THEN every attempt MUST fail, while reading planned entries and writing a write path MUST succeed, and the run MUST qualify only when every probe sent reports exactly its expected outcome

### Requirement: Structural writeback operations

r[kiln.source.projection.writeback.structural] A capture MUST record, under the `kiln.projection.writeback-candidate.v2` schema, a declared rename or fork of an origin as one source operation naming the origin, the new source path, the overlay destination and the expected object, only when the overlay shows the origin removed (rename) or unchanged (fork) and a new file with the origin's exact bytes and kind under a creation destination. A declared alias change or membership change MUST be recorded as a view-definition operation that never becomes a source operation and never changes the source. Undeclared, the same overlay MUST stay a delete and a create. A declaration the overlay does not show, an origin or new file used twice, or more declarations than the named bound MUST be refused.

#### Scenario: Rename, fork, alias, membership and delete are distinct

r[kiln.source.projection.writeback.structural.scenario.distinct]
- GIVEN one writable view
- WHEN a file is renamed, forked, re-aliased, removed from or added to the view, or deleted
- THEN each MUST yield its own outcome and identity, and sealing MUST change exactly the rename's two paths and the fork's new path

### Requirement: Writeback conflicts

r[kiln.source.projection.writeback.conflict] A capture over a source that moved since the view was resolved MUST NOT seal a candidate or rebase. It MUST return a `kiln.projection.writeback-conflict.v1` outcome naming the base and current revisions and every source path where the captured operations and the current source disagree, a changed object or entry kind (a mode-only change included), normalized to canonical origins. A rebase MUST be an explicit re-resolution and a new overlay generation. Fence, pin and other refusals MUST take precedence over a conflict.

#### Scenario: Ownership and observed content are independent

r[kiln.source.projection.writeback.conflict.scenario.archil]
- GIVEN an attempt that read an origin before another writer changed it, through one or both aliases of that origin
- WHEN it captures with a current fence, or an old attempt presents exactly the current content with a superseded fence
- THEN the first MUST yield a conflict naming the canonical origin (or a contradictory-alias refusal when its aliases disagree), and the second MUST be refused as superseded whether or not the source moved

### Requirement: Recorded candidate compatibility

r[kiln.source.projection.writeback.compatibility] A recorded writeback candidate MUST verify read-only under its own schema: a v1 record MUST hold only edits, creates and deletes and keep its v1 identity, a v2 record MUST verify under the v2 identity, and any changed field, a v1 record with a v2 operation or a relabeled schema MUST be refused. Verification MUST NOT seal or admit a record.

#### Scenario: A v1 candidate still verifies

r[kiln.source.projection.writeback.compatibility.scenario.v1]
- GIVEN a candidate recorded by the v1 capture
- WHEN it is verified after the v2 capture lands
- THEN it MUST verify as v1, and the v2 capture of the same overlay MUST verify only as v2

### Requirement: Projected candidate facts

r[kiln.source.projection.candidate_facts] Verification of a sealed candidate MUST consume `kiln.projected-candidate-facts.v1` facts binding the sealed identity, the manifest and definition it was captured from, the build plan its checks run and the projects its changed source paths affect over the full candidate, under an identity over every field. A build plan made for another manifest or base MUST be refused.

#### Scenario: Affected consumers outside the view are selected

r[kiln.source.projection.candidate_facts.scenario.affected]
- GIVEN a shared-dependency edit captured through one project's view
- WHEN its facts are derived from the full candidate's scopes and graph facts
- THEN the dependent project outside the view MUST be affected, an edit confined to one project MUST affect it alone, missing graph facts MUST fall back to affected, and a view-only change MUST affect nothing

#### Scenario: Evidence applies only to its own candidate

r[kiln.source.projection.candidate_facts.scenario.applicability]
- GIVEN facts for candidate C1 with retained evidence, and a later candidate C2
- WHEN evidence applicability is checked
- THEN C1's evidence MUST apply to C1 and MUST NOT apply to C2
