# Verification evidence

## Result

Worker isolation now admits a bounded set of explicit read-only data mounts
under `/srv` or `/var/lib/kiln`. Admitted paths are canonicalized at execution,
must be directories, cannot resolve through a symlink, and cannot be inside the
writable checkout. They are bound read-only before the observer and checkout,
so a Patchgate review can read its external Pijul target without account-authority
fallback.

## Evidence

- Nickel typecheck, format and positive/negative fixture checks pass.
- The full Patchgate worker profile exports with the target in
  `read_only_data_mounts`, a checkout receipt output, `/tmp` review home and a
  `review` report declaration.
- `cargo test -p kiln-web-service --lib` passes 166 tests with 1 ignored.
- Focused execution tests pass for duplicate, foreign-root, missing,
  read-only-bind and checkout-overlap cases.
- `cargo fmt -p kiln-web-service --check` passes.
- `cairn validate --root .` reports zero issues.

## Non-claims

A read-only mount does not prove mounted data is non-sensitive or that hostile
code is confined. The allowed roots are intentionally narrow; a repository
outside `/srv` or `/var/lib/kiln` needs a separate reviewed policy change. This
change does not prove a live Patchgate/Pijul review or review quality. Clippy is
not a clean rail here because the direct toolchain reports unrelated pre-existing
warnings in `kiln-core` and `kiln-web-service`.
