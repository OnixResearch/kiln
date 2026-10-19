# Design: Update the Mantle build cohort

## Decision

Refresh the single admitted Mantle cohort from
`0bbab77e23ab95caa4e4184b9945cfaa5c8040e2` to
`d75158b00b080535f2ffea12430665ca3d57ea82`, the newest revision whose canonical
`nix build .#default` passes its own test phase. Keep the adapter contract,
profile limits, outcome classes, and non-claims unchanged.

Two newer revisions are rejected on their own build evidence, not on Kiln
policy: `43ccd68840b748e03773fc1bff5eb1cfca99dffa` fails to compile its
`integration_build` test target with a duplicate function definition, and
`d20459c6db6d60ed2b08533911e279cbeadb742e` fails its dependency build because its
flake source filter omits `crates/crunch-build/audit/finish_dlopen_audit.c`.

The producer contract and producer fixtures are byte-identical at the pinned and
selected revisions (contract blob
`bb3ceb3ed5d7fff307a8f7553b68c118cfd6be5c`, fixture blob
`17dcf29d0dfb9a68563581b7a21e4821e920e2bb`), and the producer still emits
`crunch-build-report-v1`. The adapter therefore admits the same wire shapes; only
the cohort identity changes.

## Exact cohort

| Fact | Value |
| --- | --- |
| Source | `ssh://git@github.com/OnixResearch/mantle.git` |
| Revision | `d75158b00b080535f2ffea12430665ca3d57ea82` |
| Product version | `0.1.0` |
| Contract license | `MIT OR Apache-2.0` |
| CLI license | `AGPL-3.0-or-later` |
| Executable | `/nix/store/7h26x5gxihsba9i4hsvmd0b5v2m5z7zl-mantle-0.1.0/bin/mantle` |
| Executable BLAKE3 | `f04277507402681d4deb02181f1dbe16877db548f4fa1dbd166c19d6879ea45e` |
| Builder identity | the same `f0427750…` digest, preserving the previous profile's builder/executable relationship |
| Platform | `x86_64-linux` |
| Bounded Exec | unchanged `29dac88ecded94457572db3fdfaaaab95fa91525` |
| Producer derivation | `/nix/store/i5vggncmz5ncnrwna1s1mv5mv5x83gyc-mantle-0.1.0.drv` |

The plan, policy, worker, and store identities remain the Kiln-owned fixtures
already bound by the profile. They describe the local cohort inputs, not the
producer revision, and the contract bytes that consume them did not change.

## Build and identity procedure

1. Export the committed revision with `git archive` into a clean directory; the
   operator's working tree is not an input.
2. Run `nix build --no-write-lock-file --no-link --print-out-paths <path>#default`
   with the producer's committed flake lock.
3. Record the output path, the BLAKE3 of `bin/mantle`, and the observed version.
4. Bind the same revision and executable identity in the Rust constants, the
   typed Nickel profile, and the stack profile.

If the build yields a different executable identity than the recorded one, the
profile admission and cohort check must fail rather than accept a substitution.

## Drift rules

- Rust `admit_profile` rejects a profile whose source, revision, licenses,
  schemas, command, platform, limits, or non-claims differ from the constants.
- The Nix `mantle-build-cohort` check rejects a vendored fixture record whose
  revision, contract bytes, or fixture bytes disagree with the accepted pin.
- The documented fixture source record carries the same revision as the profile
  and importer pin, so a partial edit is a validation failure, not a silent
  mixed cohort.

## Verification

- `cargo test -p kiln-adapter-mantle` and the producer-fixture consumer test.
- `cargo check/clippy` for the touched crate and workspace gate.
- Nickel typecheck/export for `config/mantle-build-profile.ncl`,
  `config/stack-profiles.ncl`, and the fixture source record.
- `nix build .#checks.x86_64-linux.mantle-build-cohort`.
- Cairn validation and proposal/design/tasks gates for this change.

## Non-claims

- The refresh proves only that Kiln binds one exact published producer revision
  whose contract bytes match the admitted wire shapes.
- It does not prove build correctness, sandbox completeness, cache truth,
  product semantics, reproducibility, or release readiness.
- Producer working-tree changes, unpublished branches, and new producer surfaces
  are not admitted by this change.
