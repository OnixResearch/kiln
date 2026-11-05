# Verification evidence

## Result

Firecracker cache restore uses a read-only second drive. The guest extracts it
over `/work`. After successful execution and confirmed cleanup, the host
requests the declared output roots, bounds and validates the returned ustar,
extracts it into a private temporary directory, verifies the cache key against
the unchanged source workspace, and publishes only those outputs.

## Evidence

- `cargo check -p kiln-hosted --offline`: ok.
- `cargo test -p kiln-microvm --lib --offline`: 40 passed.
- `cargo test -p kiln-hosted --lib --offline`: 73 passed, including host ustar
  extraction, guest output publication, and changed-source rejection.
- `cargo test -p kiln-web-service --lib --offline`: 168 passed, 1 ignored.
- `cargo test -p kiln-hosted --test journey --offline`: 9 passed.
- `cairn validate --root .`: zero issues.
- The KVM guest-image test was invoked with `--ignored` and skipped because
  `KILN_MICROVM_GUEST` is unset. A locally repacked initrd was also rejected by
  the immutable-image check because adding it to the Nix store waited on the
  big GC lock; no KVM execution occurred.

## Non-claims

No object-storage cache tier, host-level guest-image cache test, or latency
measurement exists yet.
