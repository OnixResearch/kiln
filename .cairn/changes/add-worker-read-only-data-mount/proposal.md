## Why

The reviewed worker isolation policy currently exposes only fixed runtime
roots. A Patchgate review needs an operator-provisioned Pijul repository outside
the Git checkout, so it cannot run under the sandbox even though the step and
worker profile are otherwise admitted.

## What Changes

- Add optional `execution.read_only_data_mounts` to the worker isolation policy.
- Admit at most four explicit data roots under `/srv` or `/var/lib/kiln`.
- Canonicalize each mount at execution, reject symlink resolution, require a
  directory, reject a mount inside the writable checkout, and bind it read-only.
- Expose the field through `config/web-worker.ncl` and use it in the Patchgate
  worker fixture with a `/srv/kiln/...` target and sandbox-local writable paths.
- Document the new authority and its non-claims.

## Impact

- **Files**: `crates/kiln-web-service/src/execution.rs`,
  `crates/kiln-web-service/src/worker/process.rs`, `config/web-worker.ncl`,
  `config/fixtures/web-worker-patchgate.ncl`, `docs/operator.md`.
- **Testing**: policy admission positive/negative cases, read-only bind
  generation, symlink/checkout rejection, existing execution tests, and worker
  profile export.
- **Authority**: data mounts are read-only and do not grant credentials,
  mutation, network, or service authority. A reviewed profile can expose a data
  directory, not arbitrary host paths.
