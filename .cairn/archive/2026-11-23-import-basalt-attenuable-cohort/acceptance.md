# Change-local acceptance

a[import-basalt-attenuable-cohort.imports] `vendor/basalt` equals `git archive fac0d7f1fd7ac936ca4d3f251f3287666e93d4bb` and `vendor/ucan` equals `git archive fca62a6ed6895689ff18112b497bc7a986516c20` for their recorded archive paths, apart from Basalt's three recorded UCAN path rewrites and each tree's root `rustfmt.toml`. The importer pins and source records agree on the revisions and on the full and selected archive BLAKE3 identities, and a second importer run verifies both trees unchanged.

a[import-basalt-attenuable-cohort.shared-cohort] The Basalt and UCAN revisions are the ones Lattice `main` vendors, Basalt's nested UCAN files are byte-identical to Kiln's UCAN import, and `crates/kiln-adapter-basalt/tests/cohort.rs` pins both revisions and checks that Basalt's compiled UCAN revision equals the UCAN record.

a[import-basalt-attenuable-cohort.v1-unchanged] Kiln binds every implementation request with Basalt's `kiln://implementation/v1/` framing and keeps that framing admitted. Kiln exposes neither v2 nor the new source operations. V1-scoped grants retain their behavior; Basalt now requires `kiln/delegate` at each delegator for v1 chains covered by a broader grant. The `kiln-adapter-basalt` tests and the `kiln-implementer` `authority_verified`, `authority_interleaving`, `authority_coverage` and `admission_host_owner` tests pass without changes to their expectations.

a[import-basalt-attenuable-cohort.iroh] `kiln-adapter-iroh`, `kiln-adapter-iroh-observe` and `kiln-hosted` pin Iroh `=1.1.0` with only the `portmapper` and `tls-ring` features. Their ALPNs, frames, bounds, peer and relay policy are unchanged; the migration bridge still accepts only all-direct-IP connections; and the profiles, Nickel contracts and fixtures name `1.1.0` while `iroh_blobs` and `bao_tree` stay `deferred`. Their test suites, including the loopback and exchange tests, pass.

a[import-basalt-attenuable-cohort.lock] Every `Cargo.lock` change comes from the two cohorts and the Iroh move they force, and `evidence/lock-delta.txt` lists each one.

a[import-basalt-attenuable-cohort.verification] Workspace fmt, clippy with and without all features, the wasm build and clippy, `scripts/check-nickel-fixtures.sh`, `cairn validate` and the proposal, design and tasks gates pass, or each failure is attributed against `83e06493`.
