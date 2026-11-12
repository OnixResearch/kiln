## Why

The Octet deny-all check for the Aspen host, `kiln-aspen-host-octet-deny-all`, fails at `main`, and the aggregate `octet` check fails with it. The first failure shows 14 findings in three library modules added on 2026-11-11 and 2026-11-12: `content.rs`, `gossip.rs` and `docs_index.rs`. Once the library compiles, the check reaches the other targets and reports 71 more: one in the binary and 70 in the two live integration tests that arrived with those modules. The retained 2026-11-06 flake check last recorded the gate as passing. Octet is one of the repository gates the release-readiness policy lists, so the launch candidate cannot record it green. The operator chose to fix it now.

## What Changes

- Split the content, gossip and docs-index modules into directory modules whose files stay under the 300-line limit, with their tests in separate files. The host's `src/` directory drops from nine library source files to six.
- Rename `docs_index` to `docs::index`, with its profile, entry and error types in `docs::profile`.
- Replace `usize` byte bounds with `u64` in `MoltenContentPort::get_live_exact` and `CacheIndex::get_verified_blob`.
- Make `gossip::now_ms` return a `Result` with a new `GossipError::Clock` instead of falling back to `0` or `u64::MAX`, and keep it as the module's single clock read.
- Make `GossipListener::stop` report a router shutdown failure instead of discarding it, and signal shutdown by dropping the sender.
- Rename the `share` flag to `is_writable` and import the blob store under its own name.
- Move the content-store opening out of `run` in `main.rs`, which was five lines over the function-length limit.
- Bring the two live integration tests under the policy and rename them `tests/gossiplive.rs` and `tests/docssynclive.rs`.

Wire schemas, bounds, the stored content index, requirement markers and requirement text do not change.

## Impact

- **Current consumer:** the `0.1.0` launch checklist, whose repository gates include the Octet check.
- **Durable capability:** the Aspen host is back under the deny-all policy, and the three new modules and their tests follow the layout the rest of the crate already uses.
- **Owner:** Kiln maintainers.
- **Files**: `runtime/kiln-aspen-host/src/lib.rs`, `src/main.rs`, `src/content/`, `src/gossip/`, `src/docs/`, `tests/gossiplive.rs`, `tests/docssynclive.rs`, `runtime/kiln-aspen-host/README.md` and this package.
- **Testing**: run the host's tests before and after and compare the test functions; run the Octet check at the parent revision and at the change; run the aggregate `octet` check and the host package check; run Clippy for the host; run traceability coverage.
- **Compatibility**: `kiln_aspen_host::docs_index` becomes `kiln_aspen_host::docs::index`; two byte-bound parameters change type; `gossip::now_ms` returns a `Result`; the test targets `gossip_live` and `docs_sync_live` become `gossiplive` and `docssynclive`. Nothing in this repository outside the host uses these.
- **No accepted-spec effect:** module layout, parameter types, error reporting and test structure change; governed behavior does not.
