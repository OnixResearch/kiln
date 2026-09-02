# valence-core

`valence-core` is the reusable, deterministic core of the Valence Evidence IR.
It validates canonical evidence identity, typed linkage, provenance, roles, and
non-claim boundaries over already-loaded values. Filesystem access, process
execution, source discovery, artifact measurement, publication, and operator
policy remain in shells or producer repositories.

## Distribution

The package is distributed from reviewed Git revisions and is not published to
crates.io. Pin an exact public revision:

```toml
[dependencies]
valence-core = {
  git = "https://github.com/OnixResearch/valence",
  rev = "<reviewed-commit>",
  package = "valence-core",
}
```

Rust code imports the crate as `valence_core`.

## Public modules

Identity and envelope primitives:

- `artifact_ref`
- `content_bound_requirements`
- `evidence_ir`
- `evidence_chain`
- `evidence_model`
- `preserves_evidence`
- `provenance`
- `shared_identity`
- `stack_role_registry`

Evidence adapters and profiles:

- `ast_grep_evidence`
- `build_service`
- `cob_evidence`
- `evidence_adapters`
- `formal_proof_chain`
- `function_address_preserves`
- `function_address_profile`
- `function_object`
- `oxide_primitives`
- `proof_carrying_ir`
- `stack_provenance_adapter`
- `trellis_proof_evidence`
- `wasm_component_evidence`

Indexes, policy models, and deterministic harnesses:

- `capability_plan`
- `evidence_codebase_index`
- `evidence_harness`
- `evidence_index`
- `hegel_property`

`formal_proof_chain::aeneas` validates the closed Aeneas and Lean provider profile.

It checks typed graph and claim linkage over measured digests and parsed producer observations. It emits only a bounded generic handoff.

Read [`docs/aeneas-translation-chain.md`](../../docs/aeneas-translation-chain.md).

The crate-level documentation in `src/lib.rs` describes each module's bounded
claim surface. The [nominal digest role guide](../../docs/nominal-digest-roles.md)
describes owned role types, borrowed views, wire adapters, and consumer
migration.

## Functional-core boundary

Production code in this crate must remain testable without filesystem,
environment, process, clock, network, or terminal dependencies. Deterministic
hashing and parsing over already-loaded bytes remain pure core operations.
Octet and other producers can construct Valence inputs. They do not inherit Valence identity or linkage claims.

Consumers must run the applicable validators again.

## Evidence limits

Passing validation establishes only the specific syntax, canonical identity,
typed linkage, role, digest-domain, or non-claim facts named by that validator.
It does not establish evidence truth, behavioral correctness, semantic
equivalence, verifier soundness, requirement satisfaction, runtime authority,
whole-program safety, or release eligibility.

## License

`valence-core` uses the `MPL-2.0` license.

Read [LICENSE](LICENSE) for the package text. Read [../../docs/licensing.md](../../docs/licensing.md) for the repository package map.
