//! Pure Valence evidence helpers for bounded evidence identity and linkage surfaces.
//!
//! # Current public modules
//!
//! - `artifact_auth`: maps already protocol-verified Radicle observations into bounded standalone
//!   authentication decisions without transferring actor, Evidence IR, lifecycle, or release
//!   authority.
//! - `artifact_ref`: validates typed artifact references, digest spelling profiles, confined path
//!   metadata, declared hash/size metadata, and public-field compatibility adapters over
//!   already-loaded in-memory metadata.
//! - `build_service`: validates versioned build-service evidence profiles linking requests,
//!   attempts, authority, resources, retries, usage, results, reuse, orchestration, and external
//!   statuses with canonical identities, per-field roles, privacy exclusions, and non-claims.
//! - `cob_evidence`: validates typed Valence evidence attachments, authorization inputs,
//!   deterministic Radicle COB sidecar reduction, and saved binding/snapshot receipts over
//!   already-loaded values.
//! - `content_bound_requirements`: consumes frozen Cairn registry contracts and validates typed
//!   requirement references, legacy recorded-only linkage, selected revisions, and stack contexts.
//! - `evidence_adapters`: validates parsed Cairn, ChaosControl, state-machine, and canonical-core
//!   cutover rows as bounded Evidence IR identity/linkage inputs.
//! - `evidence_codebase_index`: builds, queries, renders, and boundary-checks Octet-owned
//!   evidence-codebase indexes from in-memory artifacts with iddqd-backed normalized source lookup.
//! - `evidence_index`: renders and checks the current-head Valence evidence page from typed inputs.
//!   Octet retains command execution, repository-state capture, freshness orchestration, and
//!   publication decisions.
//! - `formal_proof_chain`: validates frozen formal-proof stack-chain fixtures and the closed
//!   Aeneas/Lean provider profile for canonical identity, typed linkage, bounded role summaries,
//!   and generic Cairn readiness input.
//! - `function_address_profile`: validates Octet/Kamacite function-address evidence profiles as
//!   recorded-only Evidence IR with typed digest domains, graph rows, and non-claim boundaries.
//! - `function_object`: constructs Function Object v1 DTOs, canonical hash payloads, caveat
//!   summaries, and pure-cache eligibility from source facts. Octet retains scanning, source-marker
//!   mutation, corpus orchestration, and CLI/status reporting.
//! - `oxide_primitives`: records reference-only Oxide primitive intake for typed IDs, derived-key
//!   collections, semantic diffs, and bounded attestation vocabulary.
//! - `preserves_evidence`: validates Preserves evidence envelopes and Preserves-to-Valence bridge
//!   rows over already-loaded canonical bytes/text.
//! - `radicle_forge_ops`: validates normalized issue/pull-request metadata, derives deterministic
//!   native COB operation plans and bindings, and admits exact CI revisions only from matching
//!   already-loaded status, approval, and canonical-predecessor observations.
//! - `shared_identity`: normalizes BLAKE3 spellings, digest roles, canonical hashes, self-hash
//!   exclusion profiles, and reusable non-claim boundaries.
//! - `stack_role_registry`: validates stack schema/role rows, verification-role counting
//!   vocabulary, compatibility aliases, and consumer constant drift.
//!
//! # Boundary
//!
//! This crate intentionally contains deterministic core logic only. CLI I/O,
//! filesystem reads, artifact byte loading, path resolution, source scanning,
//! object marker mutation, corpus orchestration, supplement orchestration,
//! stdout/stderr, process exits, repository tags, package publication, and
//! release ownership remain in shells or producer repositories. Standalone
//! Valence owns Evidence IR semantics; Octet remains an evidence producer and
//! source-scanning/proof-rail orchestrator.
//!
//! # Evidence boundary
//!
//! `valence-core` currently supports bounded local evidence replay for selected
//! Octet/Valence surfaces. It does not claim behavioral correctness, semantic
//! equivalence, exhaustive Rust coverage, pure-cache eligibility, cross-machine
//! determinism, cross-toolchain determinism, external consumer compatibility, or
//! general HIR/THIR/MIR extraction.

pub mod artifact_auth;
pub mod artifact_ref;
pub mod ast_grep_evidence;
pub mod build_service;
pub mod capability_plan;
pub mod cob_evidence;
pub mod content_bound_requirements;
pub mod evidence_adapters;
pub mod evidence_chain;
pub mod evidence_codebase_index;
pub mod evidence_harness;
pub mod evidence_index;
pub mod evidence_ir;
pub mod evidence_model;
pub mod formal_proof_chain;
pub mod function_address_preserves;
pub mod function_address_profile;
pub mod function_object;
pub mod hegel_property;
pub mod oxide_primitives;
pub mod preserves_evidence;
pub mod proof_carrying_ir;
pub mod provenance;
pub mod radicle_forge_ops;
pub mod shared_identity;
pub mod stack_provenance_adapter;
pub mod stack_role_registry;
pub mod trellis_proof_evidence;
pub mod wasm_component_evidence;
