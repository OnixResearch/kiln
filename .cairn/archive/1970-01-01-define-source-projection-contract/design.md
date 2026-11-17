## Context

`kiln_core::monorepo` admits path scopes: root, include and exclude globs, declared dependency globs and graph kinds, with a private-field `AdmittedScope` and BLAKE3 identity. `kiln_core::source::SourceResolution` retains the provider, native repository, exact revision and `canonical_tree`. The web service derives `repository_key` from the canonical checkout path. No type binds a selected set of files, their layout and their origins to one exact snapshot.

## Decisions

### Decision: repository identity is the provider binding, not a path

**Choice:** `repository_identity` hashes a schema, the source provider profile ID and the provider-native repository `ExternalRef`. The Kiln repository ID is an operator handle. Locators (absolute local paths or remote URLs) are admitted bindings outside the identity. `admit_repositories` groups enrollments by handle. Equal bindings merge their locators. Two bindings under one handle are refused unless one `BindingReconciliation` names exactly the retired and current binding; the admitted repository then carries the current binding and records the retired identity as superseded. One binding under two handles is refused. A reconciliation that matches nothing is refused instead of lingering.

**Rationale:** the identity follows the provider-native reference the issue requires preserved, so moving a cache or adding a mirror never changes it. A path-derived or handle-derived identity would either change with caches or collide across registries. Provider migration is an explicit, auditable change of identity, not a silent one.

### Decision: definitions reuse scope admission and are canonical sets

**Choice:** `ProjectionDefinition` is the deserialized, untrusted form exported from Nickel. `admit_definition` validates it and reuses `monorepo::admit_scope` for the selection, so roots, patterns, bounds and `contains` semantics stay one implementation. Mounts, dependencies, creation destinations and capabilities become ordered sets, and the definition identity hashes that canonical form with length-framed fields. Profile rules:

- creation destinations only for `writable`;
- `runnable` and `writable` require the `exact_tree` source capability;
- admitted generated inputs only for `runnable`;
- a required `symlinks` or `executable_bit` capability needs a target that can represent it;
- conflicting duplicate dependency declarations are refused.

**Rationale:** one glob and path admission avoids a second convention. Canonical sets make reordering irrelevant to identity. Profile rules close the cases the issue names as implicit admission.

### Decision: every mount emits; origins carry their own identity

**Choice:** a selected path is emitted once per mount whose source prefix contains it. That makes aliases explicit and gives every alias an equal native origin. A selected path under no mount is a diagnostic, never a silent omission. A native origin records the source path, the provider-native object and the entry kind, plus an origin identity over repository, revision, path, object and kind. Identical bytes at two paths therefore keep distinct origins. Generated entries record their generator and input identity.

**Rationale:** origins are the per-entry provenance consumers need to map writes back and check content. Content-equal files are distinct sources.

### Decision: two manifest identities

**Choice:** `content_identity` covers what a consumer materializes: destinations, kinds, objects or generated inputs, and dependency bindings. `identity` covers every manifest field: repository, revision, canonical tree, definition, profile, evaluator, adapter, source capabilities, target, coverage, origins, dependencies and the content identity.

**Rationale:** a changed evaluator, adapter or revision must change the bound manifest, but caches can reuse equal content. Revision and tree changes affect only the manifest identity.

### Decision: type states, not flags, carry completeness

**Choice:** `resolve` alone constructs `AdmittedPartialView`. `AdmittedPartialView::require_complete(operation)` alone constructs `CompleteProjection`, and on refusal it returns the view with diagnostics. Build needs a `runnable` or `writable` profile; write needs `writable`. Both need complete coverage and every required dependency present. Write refuses generated entries; build refuses them unless the definition admits generated inputs. Neither type implements `Deserialize`. `verify_manifest` recomputes origin, content and manifest identities and compares them with trusted expected bindings, reporting tampering and stale repository, revision, definition, evaluator or adapter. It returns only a verdict.

**Rationale:** a forged `coverage` value or completeness flag can then never reach a consumer that requires completeness; `compile_fail` doctests pin the type boundary. BLAKE3 is not a MAC, so verification detects inconsistency and staleness. Admission always re-resolves current facts.

### Decision: compatibility is an explicit labeled conversion

**Choice:** `ProjectionDefinition::from_project_scope` copies an admitted scope's root, include and exclude. It uses an identity mount and no creation destinations. It requires `exact_tree` for runnable and writable profiles, targets a POSIX profile and records `monorepo_project_scope.v1` with the scope identity. `depends_on` and graph kinds stay impact facts, not projection inputs.

**Rationale:** existing projects gain projections without changing affected-project semantics. The issue keeps impact separate from layout.

## Risks / Trade-offs

- Case-insensitive folding uses Unicode lowercase from `alloc`. It is deterministic but does not reproduce every filesystem's normalization (for example NFC/NFD). Targets that need more must declare a stricter profile later.
- Diagnostics are capped at 64 (`MAX_DIAGNOSTICS`, plus one truncation marker), so an input with many faults reports the first ones in sorted order.
- Without consumers, the contract is exercised by fixtures and the compatibility conversion only. #76–#80 may still refine field names before any persisted manifest exists.
