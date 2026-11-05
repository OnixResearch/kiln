## Why

Issue #28 requires trust-domain placement so an untrusted microVM is never
co-resident with a job carrying secrets or a workload token. The hosted config
had no reviewed placement-pool identity and accepted any runner label.

## What Changes

- Add optional reviewed `trust_domain` to Firecracker runners.
- Require a non-empty domain for every untrusted-capable runner.
- Reject any domain containing both untrusted-capable and workload-bearing
  runners.
- Add reviewed `host_identity` to Firecracker runners.
- Require a host identity for every untrusted-capable runner and reject a host
  shared by untrusted-capable and workload-bearing runners.
- Record the trust domain and host identity in the Firecracker `vm` receipt.
- Add typed Nickel fields and hosted documentation.

## Impact

- **Files**: `crates/kiln-hosted/src/execution.rs`,
  `crates/kiln-hosted/src/microvm.rs`, `config/hosted-service.ncl`,
  `config/hosted-execution.ncl`, `docs/hosted-ci.md`.
- **Testing**: shared-domain rejection, separated-domain admission, missing
  untrusted domain rejection, hosted tests and config checks.
- **Authority**: static runner-pool separation only. Physical host/LLC
  co-residency, partitioning and covert-channel measurement remain open.
