# Graph Source Specification

## Purpose

Defines the `graph-source` capability.

## Requirements

### Requirement: Logical source identities

r[kiln.source.graph.identity] An enabled graph source MUST give each file and package a stable logical identity minted at its creating operation and never derived from its bytes or path, keep byte `ContentId`s, immutable entity revisions and graph snapshot roots as separate BLAKE3 identities, and keep native provider references only in import and export records. A pinned snapshot MUST reproduce the same entities, relations and dependency selections after restart, and native-source repositories MUST NOT depend on this mode.

#### Scenario: Equal bytes do not merge ownership

r[kiln.source.graph.identity.scenario.equal_bytes]
- GIVEN two files with equal bytes created by different operations
- WHEN both are stored
- THEN they MAY share content storage but MUST keep distinct logical identities

#### Scenario: Rename keeps identity

r[kiln.source.graph.identity.scenario.rename]
- GIVEN a declared rename or edit of an entity
- WHEN the next snapshot is recorded
- THEN the entity MUST keep its logical identity with a new revision or membership name

### Requirement: Versioned source relations

r[kiln.source.graph.relations] Membership, package-relative names, aliases, copies, forks and pinned dependency selections MUST be versioned snapshot records. An alias MUST add a membership of the same entity, a copy or fork MUST mint new identities with provenance, removing a membership or alias MUST leave an entity that another membership still holds, and deleting an entity MUST be an explicit operation that removes every membership.

#### Scenario: Removal outcomes stay distinct

r[kiln.source.graph.relations.scenario.removal]
- GIVEN an entity with two memberships
- WHEN one membership is removed, then its alias is removed, then the entity is deleted
- THEN each step MUST produce its own outcome and only the deletion MUST remove the entity

### Requirement: Exact native import and export

r[kiln.source.graph.interchange] Imports MUST bind repository, native revision, tree, acquisition provenance, importer version and the path-to-entity mapping, MUST keep an entity across native revisions only for an unchanged path and kind or an explicit mapping, and MUST record rename, copy or delete-recreate continuity as unresolved instead of inferring it from bytes. Exports MUST be deterministic, preserve bytes, executable modes and deletions, retain the root-to-tree mapping, and refuse symbolic links and special objects until a qualification admits them.

#### Scenario: Ambiguous continuity is not merged

r[kiln.source.graph.interchange.scenario.ambiguous]
- GIVEN a native change that deletes one path and adds equal bytes at another
- WHEN it is imported without an explicit mapping
- THEN the new path MUST receive a new identity and an unresolved continuity record

#### Scenario: Round trip preserves mappings

r[kiln.source.graph.interchange.scenario.round_trip]
- GIVEN a graph root exported to a native tree
- WHEN that tree is imported again
- THEN the declared graph and native mappings MUST be preserved and an accepted projected edit MUST change only its mapped entities

### Requirement: One authority mode per repository

r[kiln.source.graph.authority] Each repository MUST be either native-authoritative or explicitly graph-authoritative, with one named publication owner. Every publication MUST compare the expected current root or native revision and refuse a mismatch; a mode change MUST be an explicit migration record. Evidence MUST bind the exact graph root and export subject, and any change to the graph, schema, importer, exporter or layout identity MUST make it inapplicable. Relations, imports and exports MUST NOT widen capability authority.

#### Scenario: Independent updates do not converge silently

r[kiln.source.graph.authority.scenario.conflict]
- GIVEN a graph-authoritative repository whose native export ref also moved independently
- WHEN either side publishes against its expected state
- THEN the mismatched publication MUST be refused and reconciled explicitly
