## Why

The exact Nix derivation build provider always runs one `nix-store --realise`
and lists reproducibility as a non-claim. Nix already has a mode that rebuilds a
present output and compares it with the stored copy: `nix-store --realise
--check`, with `-K` keeping a differing output. Kiln could record that comparison
as a bounded observation without changing what a passing build means.

Issue: [#42](https://github.com/OnixResearch/kiln/issues/42).

## What Changes

- Add an optional, off-by-default `rebuild_check` profile field to the Nix build
  profile and its Nickel contract.
- After the exact admitted `.drv` is realised and its outputs rooted, run one
  bounded `nix-store --realise --check -K DERIVATION`.
- Record one typed `BuildRebuildObservation` per expected output: `matched`,
  `differed` with the retained `.check` path, or `not_checkable`.
- Reuse the build's timeout, stdin, stdout, stderr, poll-interval and teardown
  limits for the check.
- Keep the successful build outcome, products and GC roots unchanged. A
  refused, timed-out, cancelled, output-limited or ambiguous check is
  `not_checkable`, never a difference or a build failure.

## Impact

- **Files**: `crates/kiln-core/src/build.rs`,
  `crates/kiln-adapter-nix/src/{profile,adapter,runtime,mapping,tests}.rs`,
  `crates/kiln-adapter-mantle/src/mapping.rs`,
  `crates/kiln-nix-store/src/cache.rs`, `config/nix-build-profile.ncl`,
  `docs/nix-build-adapter.md`.
- **Testing**: fake-runtime cases for matched, retained difference,
  not-checkable, disabled-by-default and admission refusal.
- **Authority**: an observation only. It states "rebuilt once on this host,
  matched or differed"; it is not a reproducibility gate, required CI, or a
  claim about other builders. Diffoscope retention is out of scope.
