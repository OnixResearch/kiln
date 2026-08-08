## Phase 1: Contract pins and policy

- [x] [serial] Record exact Radicle issue, patch, and CI job type names, schemas, reducer versions, source revisions, licenses, selected semantics, and non-claims. r[kiln.radicle_cobs.initial_profiles] r[kiln.radicle_cobs.configuration]
- [x] [serial] Record Cairn revision `837d730799faffe6a088cfd6dc49e08c7e0f1822`, all six type names, profile schemas, crate versions, fixture identities, licenses, and claim boundaries. r[kiln.radicle_cobs.initial_profiles] r[kiln.radicle_cobs.interop]
- [x] [serial] Refresh Kiln's Nickel-authored Cairn workflow policy for the current `../cairn` schema, regenerate policy JSON, and preserve Kiln-specific policy values. r[kiln.radicle_cobs.configuration]
- [x] [depends:cob-contract-pins] Define typed Nickel contracts for the COB profile registry, capabilities, actions, limits, conflicts, applicability, drift checks, and non-claims. r[kiln.radicle_cobs.profile_registry] r[kiln.radicle_cobs.configuration]
- [x] [parallel] Add positive complete-profile fixtures and negative missing-field, unknown-type, unsupported-action, invalid-bound, mixed-schema, and format-drift fixtures. r[kiln.radicle_cobs.configuration] r[kiln.radicle_cobs.interop]

## Phase 2: Pure COB core

- [x] [serial] Define bounded COB repository, type, object, operation-head, actor, signature-observation, semantic-state, conflict, event, capability, and effect-plan types. r[kiln.radicle_cobs.model] r[kiln.radicle_cobs.capabilities]
- [x] [depends:cob-core-types] Implement pure profile selection and capability admission with explicit bounded opaque-read behavior for unknown types. r[kiln.radicle_cobs.profile_registry]
- [x] [depends:cob-core-types] Implement pure external-identity validation and canonical projection inputs without hashing or provider reduction in core. r[kiln.radicle_cobs.model] r[kiln.radicle_cobs.semantic_boundary]
- [x] [depends:cob-core-types] Implement pure conflict classification and complete-head resolution admission without last-writer-wins fallback. r[kiln.radicle_cobs.conflicts]
- [x] [depends:cob-core-types] Implement pure candidate, claim, policy, semantic-state, and provider-observation applicability with explicit invalidation. r[kiln.radicle_cobs.applicability]
- [x] [depends:cob-core-types] Implement pure event mapping and preview-first effect-plan construction with expected heads and idempotency linkage. r[kiln.radicle_cobs.capabilities] r[kiln.radicle_cobs.effects]
- [x] [parallel] Add dependency gates that reject Cairn, Radicle, `radicle-cob`, Git storage, credentials, nodes, transports, and provider reducer types from core crates. r[kiln.radicle_cobs.semantic_boundary]

## Phase 3: Optional provider shells

- [x] [depends:cob-profile-registry] Implement optional native Radicle issue, patch, and CI job adapters for admitted typed loading, event intake, and profile-specific projections. r[kiln.radicle_cobs.initial_profiles] r[kiln.radicle_cobs.capabilities] r[kiln.radicle_cobs.status_projection]
- [x] [depends:cairn-contract-pin] Implement an optional Cairn COB adapter against an exact published or immutable-revision contract without an ambient sibling runtime dependency. r[kiln.radicle_cobs.initial_profiles] r[kiln.radicle_cobs.semantic_boundary]
- [x] [depends:cairn-cob-adapter] Add profile mappings for Cairn changes, RFDs, ADRs, verification obligations, release decisions, and policy exceptions. r[kiln.radicle_cobs.initial_profiles] r[kiln.radicle_cobs.applicability]
- [x] [depends:cob-effect-core] Add fresh shell preflight, explicit signer acquisition, expected-head checks, bounded mutation dispatch, result classification, and reconciliation. r[kiln.radicle_cobs.effects]
- [x] [depends:cob-conflict-core] Add profile-specific conflict inspection and resolution shells that preserve every current head and reject partial choices. r[kiln.radicle_cobs.conflicts]
- [x] [depends:radicle-job-profile] Integrate the existing job COB status design through the registry while keeping it rebuildable and non-authoritative. r[kiln.radicle_cobs.status_projection]

## Phase 4: Interoperability and failure evidence

- [x] [parallel] Add positive fixtures for every initial profile, exact identity retention, event mapping, status projection, fresh mutation planning, applicability, and complete conflict resolution. r[kiln.radicle_cobs.interop]
- [x] [parallel] Add negative wrong-type, unknown-schema, malformed-history, forged-actor, invalid-signature, stale-head, mixed-schema, oversized, conflict, candidate-substitution, and authority-promotion fixtures. r[kiln.radicle_cobs.interop] r[kiln.radicle_cobs.nonclaims]
- [x] [depends:cairn-profile-fixtures] Consume Cairn's provenance-bound external fixtures and add a separate Kiln read-only consumer for all six profile formats. r[kiln.radicle_cobs.initial_profiles] r[kiln.radicle_cobs.interop]
- [x] [depends:cob-mutation-shell] Add interruption, timeout, cancellation, provider-error, unknown-outcome, stale-retry, and reconciliation fixtures without false success. r[kiln.radicle_cobs.effects] r[kiln.radicle_cobs.nonclaims]
- [x] [parallel] Prove that core and standalone source archives build and test without a Cairn checkout, Radicle node, profile, identity document, keystore, or network. r[kiln.radicle_cobs.semantic_boundary] r[kiln.radicle_cobs.nonclaims]

## Phase 5: Documentation and completion evidence

- [x] [serial] Document profile authoring, identity boundaries, capability split, conflict behavior, applicability, effect preflight, job projections, and non-claims. r[kiln.radicle_cobs.model] r[kiln.radicle_cobs.profile_registry] r[kiln.radicle_cobs.capabilities] r[kiln.radicle_cobs.conflicts] r[kiln.radicle_cobs.nonclaims]
- [x] [serial] Record the Radicle and Cairn references, exact revisions, adapter boundaries, and compatibility matrix in repository-owned documentation. r[kiln.radicle_cobs.initial_profiles] r[kiln.radicle_cobs.interop]
- [x] [serial] Run formatting, workspace tests, strict Clippy, `no_std` checks, Nickel checks, dependency gates, all COB fixtures, Cairn validation, traceability coverage, and focused Nix checks. r[kiln.radicle_cobs.configuration] r[kiln.radicle_cobs.interop]
- [x] [serial] Record deterministic evidence for profile admission, pure decisions, shell results, format compatibility, runtime independence, and bounded non-claims. r[kiln.radicle_cobs.interop] r[kiln.radicle_cobs.nonclaims]
- [x] [serial] Sync accepted specifications, confirm complete requirement coverage, archive the change with evidence, and rerun post-archive validation. r[kiln.radicle_cobs.model] r[kiln.radicle_cobs.nonclaims]
