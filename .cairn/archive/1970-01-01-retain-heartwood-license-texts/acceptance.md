# Change-local acceptance

a[retain-heartwood-license-texts.importer] The source record lists `LICENSE-MIT` and `LICENSE-APACHE` with their upstream paths and SHA-256, and the importer extracts both from the pinned commit, verifies them and refuses a digest mismatch without writing.

a[retain-heartwood-license-texts.retained] `vendor/heartwood-canonical` contains both license texts byte-identical to the pinned heartwood commit, and the license-map links to them resolve.

a[retain-heartwood-license-texts.verification] Idempotent refresh, tampered-digest refusal, the vendored crate's tests and the lifecycle gates are exercised and retained, with the evidence stating that it is not a license-compatibility review.
