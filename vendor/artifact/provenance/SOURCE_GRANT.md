# Source authorization and provenance

## Owner authorization

On 2026-07-18, the Mantle owner and sole copyright holder explicitly authorized reuse, modification, and relicensing of Mantle code as needed for this extraction. Selected Mantle-derived material in this repository is granted under `MIT OR Apache-2.0` with preserved attribution.

This record applies only to owner-controlled Mantle source. It does not relicense third-party dependencies, imported protocol specifications, generated vendored code, or prior external contributions.

On 2026-07-24, the repository owner directed that the owner-controlled `artifact-transfer` source at revision `507b10823c4513064710a70fe565f9a1ee592985` be incorporated into the unified Artifact workspace and offered under `MIT OR Apache-2.0`. The grant covers the transfer geometry core and its repository-owned documentation. It does not alter licenses for dependencies or third-party material.

## Selected source material

- Mantle `26c6880c08cd730c9e9420f92ea91a87212e492d`:
  - distinct verified full-public-key threshold counting and same-name-key handling from `src/oci_registry.rs`;
  - canonical sorting, duplicate rejection, typed BLAKE3 references, bounded collections, and explicit non-claims from `crates/crunch-action-result-core/src/lib.rs`;
  - canonical collection and witness-revocation patterns from `crates/crunch-attestation-core/src/{canonical,policy}.rs`.
- Cairn `cc4639be24475d80844b091cdec605eb2fef9ff0`:
  - typed cryptographic observations, deterministic issue ordering, subject/parent/purpose linkage, and functional-core / imperative-shell separation from `authenticated_inputs.rs`.
- Molten `70590459a218dc8e66948ab6f305a7c54142b710`:
  - domain/purpose/public-key/verifier-context binding and explicit generation/currentness classes from `fabric_crypto_identity`.
- Valence `8eccac35fcd8aae8ef64b3d1021e7f1a53656180`:
  - compatibility evidence for keeping protocol signature validity separate from actor authorization and Evidence IR identity.
- Artifact Transfer `507b10823c4513064710a70fe565f9a1ee592985`:
  - pure fixed, FastCDC-bound, and externally segmented chunk geometry;
  - explicit BLAKE3 and SHA-256 digest tags;
  - positive and negative boundary fixtures plus disposable consumer-pilot evidence.

## Material changes

The standalone code does not move a product crate unchanged. It replaces product JSON/Nix/OCI/Preserves/Radicle statements with one versioned length-delimited preimage; uses raw Ed25519 public-key bytes for the canonical full-key identity; removes signing, generation, persistence, clocks, network, registries, build policy, lifecycle policy, and authorization; and evaluates only explicit in-memory observations.

Exact reviewed file identities are listed in `docs/source-inventory.md`.
