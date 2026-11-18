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
