# Source projection contract delta

## ADDED Requirements

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
