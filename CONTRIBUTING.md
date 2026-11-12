# Contributing to Kiln

Read `README.md` for what Kiln does and `AGENTS.md` for the repository work
rules before you change anything. This guide covers where work happens, the
checks a change must pass, and what a change must contain.

## Where work happens

Radicle `rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE` is the canonical source and
collaboration host. The GitHub repository
<https://github.com/OnixResearch/kiln> is a projection and carries the issue
tracker.

Report bugs and proposals as GitHub issues. Send changes as a Radicle patch
against the canonical repository or as a GitHub pull request. Maintainers
integrate accepted changes on canonical `main` and then update the projection.
No response time is promised. Building and testing never needs Radicle
credentials.

## Development environment

Enter the development shell from the repository root:

```console
nix develop path:$PWD
```

The workspace pins Rust `1.88.0` in `rust-toolchain.toml`, with the `clippy` and
`rustfmt` components and the `wasm32-unknown-unknown` target. Build the
workspace from source.

## Checks

Run the narrowest check that exercises your change first. The maintained
repository gates are:

```console
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all --check
scripts/check-nickel-fixtures.sh
cairn validate --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json"
cairn traceability coverage --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json" --json
cairn release-readiness --root .
nix build path:$PWD#checks.x86_64-linux.octet --no-link -L
nix flake check path:$PWD --no-write-lock-file -L
```

`scripts/check-nickel-fixtures.sh` exports every positive fixture and asserts
every negative fixture is rejected; the same script runs inside
`checks.x86_64-linux.nickel`. `fixtures/agent-completion/` binds the maintained
gate commands to the current generated policy hash, so regenerate its command
and selection hashes whenever those commands or the policy change.

The development shell does not provide `cairn`; install a Cairn executable
that accepts `cairn-policy/generated/cairn-policy.json` to run the three
`cairn` gates. Do not replace a failing repository gate with an approximation.

## Lifecycle

Accepted requirements live in [`.cairn/specs/`](.cairn/specs/), active changes
in [`.cairn/changes/`](.cairn/changes/), and completed changes in
[`.cairn/archive/`](.cairn/archive/). An active change is a directory with
`proposal.md`, `design.md`, `tasks.md` and optional capability specs.

Behavior changes need a native Cairn change package, and its proposal, design
and tasks gates must pass before implementation. Replace `<change>` with the
change directory name:

```console
cairn gate proposal <change> --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json"
cairn gate design <change> --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json"
cairn gate tasks <change> --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json"
```

## Design constraints

- Kiln owns provider-neutral CI domain semantics. Keep Aspen, Mantle, Lattice,
  GitHub, Radicle, Git, Pijul and Jujutsu behind versioned adapter ports. Treat
  Jujutsu as a source provider, not a forge or collaboration provider.
- Core crates must not import provider implementations. Provider identifiers
  must not become primary Kiln identifiers.
- Unsupported capabilities must fail before external effects.
- Keep a pure functional core and thin imperative shells. Pure functions decide
  state transitions, admission, ordering, retry classification and requested
  effects. Shells own files, databases, clocks, networks, processes,
  credentials and provider calls.
- Use typed Nickel for human-authored configuration. Use BLAKE3 for Kiln-owned
  content identities and deterministic identifiers, and preserve
  protocol-mandated algorithms and provider-native identities where
  interoperability requires them.
- Bind every gate result to one exact candidate-set identity. A moved target or
  failed dependency requires a new candidate and evaluation.
- Keep merge authority in collaboration-provider shells. Keep Remote Execution
  API, SLSA, in-toto, CDEvents, CloudEvents and OpenTelemetry types out of core
  crates.
- Radicle is the canonical repository host. This choice must not add a Radicle
  dependency to core crates, standalone builds, source archives or local core
  tests. Do not run Radicle initialization, signing, push, synchronization,
  announcement, delegate or threshold mutation, or mirror publication without
  an explicit operator request.

## Tests

- Write positive and negative tests together.
- Adapter conformance must test valid operations, malformed inputs, unsupported
  capabilities, stale identities, replayed triggers, timeouts, cancellation,
  ambiguous outcomes and provider failure.
- A test must exercise core logic without starting an external service. Test
  behavior, boundaries, invariants, transitions and errors rather than source
  text or wiring.
- Record the exact commands and results in the change package or pull request.
  Label a simulated or partial run as such.

## Version control and generated inputs

- Use `jj` for version-control operations.
- Do not edit `flake.lock` manually.
- Do not edit vendored snapshots under `vendor/` by hand. Use the repository
  import scripts, which read a pinned Git object and record the exact source
  revision and archive identity.
- Do not edit generated policy JSON by hand. Change the Nickel source and
  regenerate it with the pinned producer.

## Change content

A patch or pull request should contain:

- the change package, when the work changes behavior;
- the exact verification commands and their results;
- what you did not run;
- known non-claims and the bound of any new evidence;
- no unrelated formatting, generated churn or vendor updates;
- no secrets.

## Licensing

Contributions are accepted under the license of the package they touch.
Kiln-owned packages under `crates/` default to `MIT OR Apache-2.0`;
`kiln-adapter-pijul` is `GPL-2.0-only`. Combined artifacts that link
`AGPL-3.0-or-later` or `MPL-2.0` components are not permissive-only. Imported
and vendored material keeps its own terms. Read [the license map](LICENSES.md)
before redistributing a binary, and do not relicense third-party material.

## Security

Report vulnerabilities through the process in [SECURITY.md](SECURITY.md). Never
put credentials in Nickel configuration, JSON, fixtures, issues or patches.

## Conduct

Participation is governed by [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
