## ADDED Requirements

### Requirement: Verified Signer Algorithm Admission [r[ucan.signer.algorithm-admission]]
The library MUST route signer-based issuance algorithm admission through a verified deterministic byte-equality helper before accepting a signer algorithm value.

#### Scenario: Exact supported signer algorithm is admitted [r[ucan.signer.algorithm-admission.scenario.accept-eddsa]]
- GIVEN a signer reports exact algorithm bytes for `EdDSA`
- WHEN signer identity validation checks the algorithm
- THEN the library accepts the algorithm admission fact before continuing signer identity checks

#### Scenario: Unsupported signer algorithm fails closed [r[ucan.signer.algorithm-admission.scenario.reject-other]]
- GIVEN a signer reports any algorithm bytes other than exact `EdDSA`
- WHEN signer identity validation checks the algorithm
- THEN the library rejects signer-based issuance with an unsupported-algorithm error before relying on the signer

#### Scenario: Signer and crypto truth remain boundaries [r[ucan.signer.algorithm-admission.scenario.boundaries]]
- GIVEN the verified helper admits only the deterministic algorithm byte value
- WHEN signer-based issuance continues after admission
- THEN signer honesty, signer I/O, key provenance, public-key derivation, Ed25519 signing, and signature verification remain explicit runtime trust boundaries
