# Source Projection Specification

## Purpose

Defines the `source-projection` capability.

## Requirements

### Requirement: Stable repository identity

r[kiln.source.repository_identity] A Kiln repository identity MUST be a BLAKE3 identity over the source provider profile and the provider-native repository reference only. Local and remote locators MUST be admitted as separate bindings that never enter the identity. One repository handle with two provider bindings MUST be refused unless one explicit reconciliation names exactly both bindings. One provider binding under two handles MUST be refused, and an unmatched reconciliation MUST be refused.

#### Scenario: Cache move and equal directory names

r[kiln.source.repository_identity.scenario.cache_move]
- GIVEN one enrolled repository observed at two local cache paths, and another repository whose cache shares the first one's directory name
- WHEN the enrollments are admitted
- THEN the first repository MUST have one identity with both locators retained
- AND the second repository MUST have a different identity

#### Scenario: Conflicting provider bindings

r[kiln.source.repository_identity.scenario.conflict]
- GIVEN one repository handle enrolled with two different provider-native references
- WHEN no reconciliation names both bindings, or one names a different pair
- THEN admission MUST fail with a deterministic diagnostic
- AND an exact reconciliation MUST admit the current binding and retain the retired identity as superseded

### Requirement: Admitted projection definitions

r[kiln.source.projection.definition] A projection definition MUST be human-authored in typed Nickel and admitted into a validated, versioned representation with a BLAKE3 identity over its canonical form, never over configuration text. It MUST declare a `navigation`, `runnable` or `writable` profile, a selection with monorepo scope semantics, mount layout rules, declared dependencies, creation destinations, required source capabilities, a generated-input policy and a target filesystem profile. Creation destinations MUST be writable-only. Runnable and writable profiles MUST require an exact source tree. Admitted generated inputs MUST be runnable-only. A target MUST be able to represent every required capability.

#### Scenario: Unordered declarations are canonical

r[kiln.source.projection.definition.scenario.canonical]
- GIVEN two definitions that differ only in the order of patterns, mounts, dependencies, creation destinations or capabilities
- WHEN both are admitted
- THEN their definition identities MUST be equal, and a changed mount MUST change the identity

#### Scenario: Profile rules refuse implicit capability

r[kiln.source.projection.definition.scenario.profile_rules]
- GIVEN a navigation definition with creation destinations, a runnable definition without the exact-tree capability, a writable definition admitting generated inputs, or a required capability the target cannot represent
- WHEN the definition is admitted
- THEN admission MUST fail with a deterministic diagnostic naming the field

### Requirement: Frozen resolved projection manifest

r[kiln.source.projection.manifest] A pure resolver MUST turn supplied exact facts into a manifest binding the repository identity, exact revision, existing canonical tree, definition identity, evaluator and adapter versions, target profile, coverage, dependency bindings and every destination's origin. Each origin MUST be either native (source path, provider-native object and kind, with an origin identity over repository, revision, path, object and kind) or explicitly generated (generator and input identity). Entries MUST be ordered by destination. Unsafe paths, unmapped selected paths, destination collisions, case-fold collisions on case-insensitive targets, file-and-directory conflicts, unsupported source or target capabilities and bound violations MUST produce sorted diagnostics before any effect. The manifest MUST carry a content identity over materialized content and an identity over every field.

#### Scenario: Reproducible manifests and aliases

r[kiln.source.projection.manifest.scenario.reproducible]
- GIVEN two projection definitions over one exact source snapshot, one mapping a source path to two destinations
- WHEN each is resolved twice from facts supplied in different orders
- THEN each MUST produce the same manifest both times
- AND both alias destinations MUST carry the same native origin, while identical bytes at unrelated paths keep separate entries and origin identities

#### Scenario: Identity follows relevant inputs

r[kiln.source.projection.manifest.scenario.identity]
- GIVEN a resolved manifest
- WHEN selected content, the exact revision, mount layout, evaluator or adapter version, or a dependency binding changes
- THEN the manifest identity MUST change, the content identity MUST change exactly when materialized content or dependency bindings change, and unselected content MUST not change the content identity

#### Scenario: Deterministic diagnostics before effects

r[kiln.source.projection.manifest.scenario.diagnostics]
- GIVEN facts with an unsafe path, a duplicate source path, two origins for one destination, a case-fold collision on a case-insensitive target, a file under another file, an unsupported capability or too many entries
- WHEN the projection is resolved
- THEN resolution MUST fail with the same sorted diagnostics regardless of input order

### Requirement: Partial views and completeness witnesses are distinct types

r[kiln.source.projection.completeness] An admitted partial view MUST be constructible only by the resolver, and a completeness witness only by checking that view for a declared build or write operation. Build MUST require a runnable or writable profile; write MUST require a writable profile. Both MUST require complete coverage and every required dependency present. Write MUST refuse generated entries, and build MUST refuse them unless the definition admits generated inputs. Neither type MAY be deserialized. Verifying a serialized manifest MUST detect tampered fields and stale repository, revision, definition, evaluator or adapter bindings, and MUST NOT yield an admitted view. Structural completeness MUST NOT confer read, write, delegation or publication authority.

#### Scenario: Partial selections cannot become complete workspaces

r[kiln.source.projection.completeness.scenario.partial]
- GIVEN a navigation-only view, a truncated view, a view with a missing or denied required dependency, or a view containing generated entries
- WHEN a build or write completeness witness is requested
- THEN the request MUST be refused with diagnostics and return the partial view unchanged

#### Scenario: Forged and stale manifests do not bypass admission

r[kiln.source.projection.completeness.scenario.forged]
- GIVEN a serialized manifest whose coverage was changed to complete, or whose bindings name an older revision, definition or evaluator
- WHEN it is verified against trusted expected bindings
- THEN verification MUST report the tampering or staleness
- AND no path MUST turn a deserialized manifest into an admitted view or completeness witness

### Requirement: Monorepo scope compatibility

r[kiln.source.projection.compatibility] An admitted monorepo project scope MUST convert into an equivalent projection definition through an explicitly labeled compatibility profile that records the scope identity, without changing affected-project semantics.

#### Scenario: Converted scope selects the same files

r[kiln.source.projection.compatibility.scenario.scope]
- GIVEN an admitted project scope with a root, include and exclude patterns, and source facts
- WHEN the scope is converted and resolved
- THEN the projected files MUST be exactly the paths the scope contains, and the definition MUST record the compatibility profile and scope identity

### Requirement: Captured and sealed projected edits

r[kiln.source.projection.writeback] A capture MUST accept only a completeness witness for a write, its admitted definition, the overlay's workspace pin, the shell's observation of that overlay and the attempt's current fence and source facts. It MUST map each changed, created or removed overlay file back through the manifest and mounts to one typed source operation (edit, create or delete) that names its expected base object. Before any effect it MUST refuse a moved source, a superseded fence, a pin for another view, aliases that disagree, a new file outside the declared creation destinations, a new file that maps to no source path or to several, a new file outside the selection or over a source file the view does not hold, and unsupported entries. A sealed candidate MUST carry a BLAKE3 identity over its base, definition, manifest, pin and operations. A sealing shell MUST write exactly the captured bytes, refuse an overlay that changed after capture, and build the candidate from the base tree so that every path outside the operations keeps its object and mode. Capture and sealing MUST NOT move a reference or grant publication authority.

#### Scenario: A shared edit round-trips into one source update

r[kiln.source.projection.writeback.scenario.round_trip]
- GIVEN a writable view of one project and a shared directory mounted twice, materialized into a pinned attempt overlay
- WHEN the shared file is edited at both destinations, a file is created under a creation destination and another is removed, and the overlay is captured and sealed
- THEN the candidate MUST hold exactly one edit, one create and one delete, every other path MUST keep its base object and mode, a view resolved at the candidate MUST show the new version, and another attempt's pinned overlay MUST keep the old one

#### Scenario: Equal bytes at two origins stay independent

r[kiln.source.projection.writeback.scenario.independent]
- GIVEN two source files with equal bytes at different origins, one of them in the view
- WHEN the one in the view is edited and the overlay is captured
- THEN only that origin MUST change

#### Scenario: Ambiguous and contradictory edits are refused

r[kiln.source.projection.writeback.scenario.refused]
- GIVEN an alias edited at one destination only, a new file outside every creation destination, a new file that two mounts map to different source paths, a new file over a source file the view does not hold, or a symbolic link
- WHEN the overlay is captured
- THEN the capture MUST be refused with a diagnostic for each fault and nothing MUST be sealed

#### Scenario: Stale and superseded captures are refused

r[kiln.source.projection.writeback.scenario.stale]
- GIVEN source facts at another revision, a current fence beyond the pin's, a pin for another manifest, or a completeness witness for a build
- WHEN the overlay is captured
- THEN the capture MUST be refused

#### Scenario: A sealed candidate is immutable

r[kiln.source.projection.writeback.scenario.sealed]
- GIVEN a candidate that was captured and sealed
- WHEN the worker writes to the overlay again
- THEN sealing the earlier capture MUST be refused, a new capture MUST have another identity, and the earlier sealed commit MUST be unchanged

### Requirement: Materialized projection inputs and attempt overlays

r[kiln.source.projection.materialization] A materializer MUST accept only a completeness witness and a versioned materializer profile, and MUST derive a reusable input identity from the manifest identity, its content identity and the profile, and an attempt overlay identity from the input identity, the attempt and a non-zero fence. It MUST write only the planned entries and modes after verifying every provider object against its native identity, MUST refuse entries the profile cannot represent, MUST publish an input only after it is complete, and MUST NOT reuse or overwrite a directory whose binding is missing or mismatched. Attempt overlays MUST be private writable copies whose writes cannot change the shared input or another overlay. A dispatch plan that uses a projected workspace MUST pin the admitted repository, exact source revision, projection definition, resolved manifest, materializer profile, input and overlay generation for its attempt and fence, and the retained overlay binding MUST record the same pin. Materialization MUST NOT grant read, write or execution authority.

#### Scenario: Repeated materialization is equivalent

r[kiln.source.projection.materialization.scenario.repeat]
- GIVEN one complete projection and one materializer profile
- WHEN it is materialized twice
- THEN both inputs MUST have the same identity and byte- and mode-equivalent trees

#### Scenario: Overlays are isolated

r[kiln.source.projection.materialization.scenario.isolated]
- GIVEN two attempt overlays prepared from one input
- WHEN one overlay is edited
- THEN neither the input nor the other overlay MUST change

#### Scenario: Damaged or unbound state is never admitted

r[kiln.source.projection.materialization.scenario.refused]
- GIVEN a missing, corrupt or truncated object, an unsupported entry, a damaged reused input or an existing directory without a matching binding
- WHEN preparation runs
- THEN it MUST fail without publishing or overwriting any input or overlay

#### Scenario: Dispatch and retained overlays pin the workspace

r[kiln.source.projection.materialization.scenario.pinned]
- GIVEN a dispatch plan whose workspace pins one materialization plan for its attempt and fence
- WHEN the plan is admitted, or the overlay is prepared or resumed for that pin
- THEN a pin for another attempt, fence or base revision, an overlay that is not the identity of its input, attempt and fence, a workspace identity other than that overlay, or a pin that differs from the current materialization plan or the retained overlay binding MUST be refused, and a plan without a pin MUST keep its previous identity

#### Scenario: A shared corpus has a frozen local baseline

r[kiln.source.projection.materialization.scenario.baseline]
- GIVEN a typed shared corpus with two projects, a shared dependency, an executable file, equal-byte files at two origins and an excluded entry
- WHEN every projection is materialized through the eager local materializer for two attempts
- THEN the receipt MUST record each contract check's outcome and the timings, and match the committed expectation apart from timings

### Requirement: Planned build inputs for projected views

r[kiln.source.projection.build_inputs] A build-input planner MUST accept only a completeness witness for a build, exact facts and layout facts of the same full source candidate, and a versioned build request naming its profile, targets, command, toolchain and environment. Under the Cargo compatibility profile it MUST require, at their own source paths, every file of each target package, of every workspace member Cargo loads and of their path-dependency closure, together with the build root's workspace manifest and lockfile and the Cargo configuration and toolchain files at the build root and its ancestors. Under the Nix compatibility profile it MUST refuse a view that omits any source file. Before any execution it MUST refuse unavailable or stale facts, relocated or generated entries, unknown or ambiguous targets, targets with several build roots, path dependencies outside the repository and every absent required input. An admitted plan MUST carry a BLAKE3 identity over the full candidate, the projection manifest and content, the profile, the required inputs with their roles and objects, the command, the toolchain and the environment, and evidence MUST apply only to a plan with that identity. A plan MUST NOT grant read, write, execution or publication authority and MUST NOT be reported as a successful build.

#### Scenario: A project view with its shared dependency is planned

r[kiln.source.projection.build_inputs.scenario.planned]
- GIVEN a two-project Cargo workspace with glob members, a shared path dependency, inherited workspace fields, a root lockfile and Cargo configuration
- WHEN a view holding one project, the shared dependency and the root files is planned for that project
- THEN the plan MUST list exactly those packages and root files with their roles, and name the workspace root as the build root

#### Scenario: Missing, relocated and unsupported inputs are refused

r[kiln.source.projection.build_inputs.scenario.refused]
- GIVEN unavailable layout facts, facts from another candidate, a relocated or generated entry, an unknown or ambiguous target, targets with several build roots, an absent dependency, lockfile, configuration or toolchain file, an absent literal member, a partly held glob member or a path dependency outside the repository
- WHEN the view is planned
- THEN the planner MUST refuse it with a diagnostic for each fault and plan nothing

#### Scenario: Restricted Nix views are refused

r[kiln.source.projection.build_inputs.scenario.nix]
- GIVEN a Nix flake with a declared path input and a sibling read that no input declares
- WHEN a view without the sibling and a view of the whole tree are planned
- THEN the restricted view MUST be refused and the whole-tree plan MUST include the declared path input

#### Scenario: Evidence follows the plan identity

r[kiln.source.projection.build_inputs.scenario.applicability]
- GIVEN evidence recorded for one plan
- WHEN the layout, a required input object, the toolchain, the command or the environment changes
- THEN the new plan MUST have another identity and the evidence MUST NOT apply to it

#### Scenario: A planned view is qualified through the materializer

r[kiln.source.projection.build_inputs.scenario.qualified]
- GIVEN the Cargo corpus committed with a working checkout and its planned views materialized through the projected materializer
- WHEN the declared toolchain matches and each planned command runs at the build root of an attempt overlay with a cleared environment and a private target directory, and the shared dependency is edited through one view
- THEN the complete view MUST pass its tests without changing its entries, a view whose code reads a file outside it MUST fail although the checkout holds that file, and the edit MUST select the dependent project outside the editing view

### Requirement: Closed source-view authority vocabulary

r[kiln.source.projection.authority.vocabulary] Kiln MUST retain a closed source-view vocabulary, `kiln.source-authority-coverage.v1`, separate from the protected implementation operations: discover, read content, read history, prepare workspace, modify source, create source, delegate and retrieve result. Each operation MUST map one-to-one to a reviewed `kiln/source/...` ability, an effect class and an attempt rule under which exactly the mutating operations require an attempt and fence. An unknown ability MUST map to no operation. No operation MUST imply another, and candidate submission and publication MUST reuse the protected implementation operations instead of joining this vocabulary.

#### Scenario: The coverage map is closed

r[kiln.source.projection.authority.vocabulary.scenario.closed]
- GIVEN the source coverage map
- WHEN it is validated
- THEN every operation MUST appear once with a unique `kiln/source/` ability and the mutation attempt rule
- AND a duplicate, missing, foreign-namespace or attempt-rule fault MUST be refused, and an unknown ability MUST resolve to no operation

#### Scenario: Reading implies nothing else

r[kiln.source.projection.authority.vocabulary.scenario.no_implication]
- GIVEN a grant naming one operation over the whole repository
- WHEN any other operation is checked
- THEN it MUST be refused; a reader MUST NOT share, edit, prepare, retrieve results or publish

### Requirement: Exact source-view requests

r[kiln.source.projection.authority.request] A source-view request MUST bind the operation's ability, repository, snapshot rule, the view binding it is made through, the origin scope, the payload digest and, where present, the attempt and its non-zero fence. A mutating operation without an attempt MUST be invalid. Changing any bound field MUST change the request identity.

#### Scenario: Every field is bound

r[kiln.source.projection.authority.request.scenario.binding]
- GIVEN one valid request
- WHEN any bound field changes, or a mutating request omits its attempt or fence
- THEN the identity MUST change, or validation MUST fail

### Requirement: Per-component effective scope

r[kiln.source.projection.authority.scope] An effective source scope MUST be built only by an authority adapter from a verified session, MUST NOT be deserializable, and MUST bind the holder, repository, snapshot rule, the exact admitted revision, canonical grants, expiry, delegation depth, authority and revocation generations and provenance into its identity. Its grants MUST be only those the port admitted request by request, narrowed to what the session's capability set confirms: a capability-set grant that was never individually admitted, such as a sibling the delegation chain rule did not check, MUST confer nothing. Path scope MUST be framed per literal component with an explicit subtree marker, so a grant for `a/b` never covers `a/bc`, and empty, dot, glob, separator and control components MUST be refused.

#### Scenario: Paths compare by component

r[kiln.source.projection.authority.scope.scenario.components]
- GIVEN a subtree grant for `crates/app` and an exact grant for one file
- WHEN `crates/apple`, a longer path below the file, or an unsafe component is checked
- THEN none MUST be covered or admitted

#### Scenario: The scope identity binds every field

r[kiln.source.projection.authority.scope.scenario.identity]
- GIVEN one admitted scope
- WHEN any bound field changes, apart from the order of its grants
- THEN the identity MUST change

#### Scenario: A capability set never extends an admission

r[kiln.source.projection.authority.scope.scenario.admitted_only]
- GIVEN a session whose capability set also reports a sibling grant the admitted requests did not cover
- WHEN its scope is built, joined with other admissions and used to restrict a view
- THEN the sibling MUST NOT be admitted, permitted, listed, looked up or presented

### Requirement: Source-view authority port

r[kiln.source.projection.authority.port] A source authority port MUST admit one exact request against current snapshot, generation, revocation and attempt facts, retain a live session and expose that session's effective scope, and MUST recheck the session before each use, discarding it on any denial. A session MUST bind one exact revision: a live view MUST follow its reference only under its admitted evolution and only through a fresh admission. A view handle presented as a resource MUST be refused. Decisions MUST be bounded and redacted.

#### Scenario: The verified port scopes each holder

r[kiln.source.projection.authority.port.scenario.verified]
- GIVEN the agent's grant over its crate in the shared corpus
- WHEN it reads its crate, reads the hidden dependency or a longer-named sibling, delegates, reads history, edits without or with an attempt, acts for a superseded attempt or after expiry, or an unknown holder asks
- THEN only the granted read and the attempt-bound edit MUST be admitted, each other request with its own closed reason

#### Scenario: Revalidation follows current authority

r[kiln.source.projection.authority.port.scenario.revalidate]
- GIVEN an admitted live session
- WHEN the revocation view or authority generation changes, the reference moves, the holder is revoked, or another payload is presented
- THEN revalidation MUST deny and discard the session, and only a fresh admission along the admitted evolution MUST yield a new scope

#### Scenario: Live views evolve only as admitted

r[kiln.source.projection.authority.port.scenario.evolution]
- GIVEN exact, fast-forward and rewrite snapshot rules
- WHEN a reference moves forward, is rewritten or names another reference, or a child rule is compared with its parent
- THEN each rule MUST admit only its own evolution, and no child rule MUST name revisions outside its parent

### Requirement: Explicit local source compatibility

r[kiln.source.projection.authority.compatibility] The local-operator source profile MUST admit only the explicitly selected operator, over the repositories and operations its full-source grant names. A missing profile, a verified credential and another operator MUST deny; there MUST be no role path; and a grant naming delegation MUST be refused.

#### Scenario: Only the explicit operator grant admits

r[kiln.source.projection.authority.compatibility.scenario.explicit]
- GIVEN a local grant for discovery and reading of one repository
- WHEN the operator reads, or a missing profile, a verified credential, another operator, another repository, an ungranted operation or a handle is presented
- THEN only the operator's read MUST be admitted, with local-operator provenance and a root-only scope

### Requirement: Restricted audience views

r[kiln.source.projection.disclosure.restricted] A restricted view MUST be derived from the trusted view, the holder's current scope, a frozen view ceiling and the shell's handle key, so that the effective view is the definition's selection intersected with the ceiling, the admitted snapshot and the current grant. Entries MUST be filtered by their native origin before anything is counted, truncated, paged or turned into coverage; an alias MUST be judged by its origin. A restricted view MUST NOT be deserializable or constructible outside the restriction, and its presentation MUST omit every trusted binding, native digest and hidden path.

#### Scenario: Three audiences see three views

r[kiln.source.projection.disclosure.restricted.scenario.audiences]
- GIVEN the shared corpus with maintainer, project-reader and agent views over one snapshot
- WHEN each is restricted
- THEN each MUST show exactly its expected entries, statuses and Cairn items, with the dependency identity only for full source

#### Scenario: Selection, ceiling, snapshot and grant all bound the view

r[kiln.source.projection.disclosure.restricted.scenario.filter_first]
- GIVEN a full-source holder through a narrower ceiling, an expired scope, another revision or repository, or a scope without discovery or read
- WHEN the view is restricted
- THEN it MUST show only the ceiling's entries, or refuse with one public shape

#### Scenario: Forbidden-only changes are invisible

r[kiln.source.projection.disclosure.restricted.scenario.forbidden_invariance]
- GIVEN generated snapshots that differ only in entries or dependency identities the holder may not see
- WHEN each is restricted for the same grant
- THEN the visible entries, coverage and visible-content identity MUST be equal

#### Scenario: Presentations carry nothing trusted

r[kiln.source.projection.disclosure.restricted.scenario.presentation]
- GIVEN restricted views of the reader and agent
- WHEN they are presented and debug-printed
- THEN no manifest, binding, scope, definition or repository identity, revision, native object, hidden path or key MUST appear

#### Scenario: Restricted views cannot be forged

r[kiln.source.projection.disclosure.restricted.scenario.unforgeable]
- GIVEN serialized data claiming to be a restricted view, context or effective scope
- WHEN it is deserialized
- THEN compilation MUST fail

### Requirement: Keyed handles, cursors and caches

r[kiln.source.projection.disclosure.handles] A view binding MUST bind the repository, revision, definition, trusted manifest, ceiling, audience, effective scope and both generations; its identity MUST key every handle, cursor and cache. Handles and cursors MUST be BLAKE3 hashes keyed by a shell-held key and MUST confer no authority. Every use MUST recheck the binding against the holder's current scope, and resolving a handle MUST recheck the operation over its origin.

#### Scenario: Handles are keyed and scoped

r[kiln.source.projection.disclosure.handles.scenario.keyed]
- GIVEN equal-byte files, an alias and its origin, two holders and two keys
- WHEN handles are compared, unkeyed digests or another holder's handle are resolved, or a handle is presented at the port
- THEN every handle MUST be distinct, and none of the guesses MUST resolve or be admitted

#### Scenario: Cursors and caches recheck current authority

r[kiln.source.projection.disclosure.handles.scenario.cursor]
- GIVEN a paged reader view
- WHEN a broader holder's cursor or cached view, another key, a forged position, a revoked or regenerated scope or expiry is used
- THEN the page MUST be refused as stale

#### Scenario: Narrowing keeps hashes and invalidates bindings

r[kiln.source.projection.disclosure.handles.scenario.narrowing]
- GIVEN a reader whose grant is narrowed
- WHEN the view is restricted again
- THEN canonical manifest and content identities MUST be unchanged, while the binding, handles and cached view MUST be invalidated

### Requirement: Uniform unavailability and distinct coverage

r[kiln.source.projection.disclosure.unavailable] A missing, forbidden or stale object MUST share one public unavailable shape while the trusted owner keeps the reason. The trusted enumeration behind a restricted view MUST be made over the holder's scoped selection: the ceiling's paths intersected with the paths its current grants let it discover or read, covering the repository root only for a full-source holder. An enumeration recorded for another selection MUST be refused as stale, and one holding an entry outside its selection MUST be refused. A view's coverage MUST be complete only when every authorized entry is listed, and otherwise truncated at the number of entries listed, whether the holder-scoped enumeration or the view's own bound stopped the listing; it MUST NOT depend on hidden entries or reveal global counts. A holder that may discover a path MUST be told whether it is present, unrequested by the definition, known absent, unknown because the holder-scoped enumeration was truncated, or beyond the view's bound. Missing and denied dependencies MUST be the same unavailable state.

#### Scenario: Missing and forbidden probes look the same

r[kiln.source.projection.disclosure.unavailable.scenario.probe]
- GIVEN a hidden path, a missing path, an alias of a hidden origin and origin probes outside discovery
- WHEN they are looked up
- THEN each MUST be refused with the same serialized shape

#### Scenario: Coverage states are distinct

r[kiln.source.projection.disclosure.unavailable.scenario.coverage]
- GIVEN complete, holder-scoped truncated and bounded views
- WHEN the views are presented and discoverable paths are looked up
- THEN each view's coverage MUST be complete only when every authorized entry is listed and truncated otherwise, and present, unrequested, absent, unknown and truncated MUST be distinct lookup results, with unknown only for a truncated holder-scoped enumeration

#### Scenario: Enumeration is scoped to the holder

r[kiln.source.projection.disclosure.unavailable.scenario.scoped]
- GIVEN a snapshot where hidden entries would push an enumeration of the whole view selection past the source adapter's bound
- WHEN the reader's view is enumerated over its scoped selection, or restricted from an enumeration of another selection or one holding entries outside its selection
- THEN the reader's view MUST be complete with every authorized entry, a truncation of the holder-scoped enumeration MUST surface as truncated coverage and unknown lookups, and the other enumerations MUST be refused

#### Scenario: Denied dependencies are only unavailable

r[kiln.source.projection.disclosure.unavailable.scenario.dependencies]
- GIVEN a required dependency that the owner denied, or that is missing
- WHEN the agent's view is restricted
- THEN both MUST present the same unavailable dependency without identity, and neither view MUST be complete

### Requirement: Restricted Cairn context

r[kiln.source.projection.disclosure.cairn] Cairn context MUST be disclosed through `.cairn/changes/<id>/**` path authority and no separate resource family. A complete observation MUST bind its repository and exact revision and admit items only under their own change path with unique ids and known prerequisites. A restricted context MUST be derived only from an observation of the view's repository at its exact revision, MUST bind the audience's view binding and the exact observation, MUST show only items the holder may discover with text only under read authority, and MUST NOT convert into, or be accepted as, a complete observation. A hidden prerequisite of a visible item MUST appear only as an opaque handle; its status MUST be shown only when the holder may discover `.cairn/changes/<id>/` of the prerequisite's change, and otherwise MUST be `unavailable`, which still blocks the item that depends on it.

#### Scenario: A hidden prerequisite stays an obligation

r[kiln.source.projection.disclosure.cairn.scenario.hidden_prerequisite]
- GIVEN a visible task depending on a hidden open task, and observations of another repository or revision
- WHEN the reader's, agent's and maintainer's contexts are delivered, and a holder whose ceiling hides the prerequisite but who may discover its change receives one
- THEN the reader MUST see the task blocked by an opaque handle with an `unavailable` status and without its name or text, the holder that may discover the change MUST see the prerequisite's real status, the agent MUST see no text, the maintainer MUST see every item, each context MUST be bound to its own audience, and a foreign observation MUST be refused as stale

#### Scenario: Observations stay change-scoped

r[kiln.source.projection.disclosure.cairn.scenario.observation]
- GIVEN items outside their change path, with unknown prerequisites or duplicate ids
- WHEN the observation is admitted
- THEN it MUST be refused, and input order MUST NOT change its identity

#### Scenario: A restricted context is never complete

r[kiln.source.projection.disclosure.cairn.scenario.not_complete]
- GIVEN a restricted context
- WHEN it is passed or converted where a complete observation is required
- THEN compilation MUST fail

### Requirement: Attenuated sharing

r[kiln.source.projection.sharing.attenuation] Sharing MUST be an operation separate from reading. A share MUST name one exact recipient DID, origin path scopes, operations, a snapshot rule, an expiry and a depth, and MUST be admitted only when every path lies in the sharer's grants and the view ceiling, the sharer holds `delegate` over every path with depth remaining, the child depth is below the sharer's, a delegating child keeps depth, every operation lies in the ceiling and the sharer's grant, the snapshot lies within the sharer's, and the share does not outlive the sharer. A view reference or handle alone MUST confer nothing.

#### Scenario: Corpus shares attenuate or refuse

r[kiln.source.projection.sharing.attenuation.scenario.corpus]
- GIVEN the corpus share cases
- WHEN each is admitted
- THEN the attenuated read MUST be admitted and the reader's share, alias, out-of-ceiling path, navigation edit, outliving, depth and tag cases MUST be refused with their named reasons

#### Scenario: Malformed and expired shares are refused

r[kiln.source.projection.sharing.attenuation.scenario.bounds]
- GIVEN wildcard or malformed recipients, empty paths or operations, a past expiry, excessive depth, an expired sharer, or a delegating child without depth
- WHEN each is admitted
- THEN each MUST be refused

#### Scenario: A child never exceeds its parent

r[kiln.source.projection.sharing.attenuation.scenario.property]
- GIVEN every combination of candidate path, operation set, expiry and depth for two sharers and three ceilings
- WHEN each is admitted
- THEN every admitted grant MUST lie within the sharer's grants, delegation and the ceiling, and no plan MUST outlive or match the sharer's depth

#### Scenario: A reference confers nothing

r[kiln.source.projection.sharing.attenuation.scenario.reference]
- GIVEN a reader's view reference and handles passed to a holder without a grant
- WHEN that holder presents them
- THEN the port MUST deny, and the reader MUST NOT turn its read into a share

### Requirement: Frozen view ceilings

r[kiln.source.projection.sharing.ceiling] A view ceiling MUST be derived from an admitted definition as the selection root with each include pattern's literal components, exact for a literal pattern and a subtree otherwise, together with the operations the profile can ever admit: navigation never prepares or mutates, and only a writable view modifies or creates. A share MUST bind the ceiling it was admitted under, so a later definition edit, alias or dependency cannot widen it.

#### Scenario: View edits and aliases stay within the ceiling

r[kiln.source.projection.sharing.ceiling.scenario.edits]
- GIVEN a reader view whose definition is edited to add documentation and alias it into the reader's crate
- WHEN the reader's and the full-source holder's views are restricted under the edited and the frozen ceilings
- THEN the alias MUST stay hidden from the reader, and under the frozen ceiling the edit MUST add nothing

#### Scenario: Expanding a saved view does not expand a share

r[kiln.source.projection.sharing.ceiling.scenario.saved_view]
- GIVEN an issued share and a later expanded saved view
- WHEN the recipient reads outside its share or restricts the expanded view under the frozen or new ceiling, or shares further
- THEN it MUST see only its shared paths and share only within them at lower depth and lifetime
