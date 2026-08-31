# Verification: productionize-aspen-radicle-host

r[verify kiln.aspen_host.production_evidence]

## Root candidate

The verified candidate starts from Radicle main `91f0012`. It contains the durable hosted-operation ledger, Aspen journal and callback adapters, exact recovery without redispatch, and bounded Radicle Nix provider.

## Production checks

- Root `cargo test --workspace --all-targets --all-features`: passed with the pinned Pijul fixture executable.
- Root `cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed.
- Root `cargo fmt --all -- --check`: passed.
- Provider package: `/nix/store/jasijpkqki1y5jmgb6pl13m6vayqajqk-kiln-radicle-nix-provider-0.1.0`.
- Provider Octet deny-all: `/nix/store/qswbnr4m30429mmlp6zx0igwz43i7g82-octet-consumer-check`.
- Aspen host package: `/nix/store/mpwqscmfwqym271f0wi0816xl4b3sc6z-kiln-aspen-host-0.1.0`.
- Aspen host Octet deny-all: `/nix/store/kjvqf70a9azr3418g97lxd0a85xblri3-octet-consumer-check`.

The package checks cover real process boundaries, restart, exact recovery, uncertain acceptance, corrupted and truncated state, provider teardown, output bounds, backup and restore, and rollback decisions.

A direct nested Cargo run from the root development shell was rejected because that shell supplies Rust `1.88.0`, while the nested host requires Rust `1.96`. The nested Nix package and Octet checks use the workspace-owned toolchain and passed. This is a shell-selection error, not accepted product evidence.

## Publication sequence

- Published root provider revision: `69afc2697f4330005797256135ae81b2722a125c`.
- Every nested host Kiln dependency now pins that exact revision.
- Cargo regenerated `runtime/kiln-aspen-host/Cargo.lock`; the branch-qualified source is absent.
- Rebuilt host package: `/nix/store/mckdrpwhy30j4mxqv62qab505nwfgq35-kiln-aspen-host-0.1.0`.
- Rebuilt host Octet deny-all check: `/nix/store/fq0vsz5n78ximpi81csz323rdzlxpwy3-octet-consumer-check`, with zero findings.
- The rebuilt host passed 2 core, 3 CLI, 3 admission, 5 durability, and 6 hosted integration tests.

## Non-claims

Machine-local journals, databases, and callback values do not prove global availability or durability. Recovery does not prove distributed exactly-once execution.

A retained report proves local bytes, not external report delivery, build correctness, deployment success, or release eligibility.
