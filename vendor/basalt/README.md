# Basalt

Basalt enforces UCAN contract boundaries across Nickel policy artifacts and Steel DTO backends. It is a standalone Rust and Nix repository.

## Flow

1. Author contract policy in `contracts/policy.ncl`.
2. Export the policy with Nickel.
3. Load the generated JSON in Rust.
4. Make sure that the selected contract permits the resource and ability.
5. Make sure that supplied UCAN capabilities authorize the same resource and ability.

Export the policy with this command:

```bash
nickel export contracts/policy.ncl > contracts/generated/policy.json
```

The repository uses a small functional core and imperative shell. Nickel owns policy structure and reviewed defaults. Rust owns runtime enforcement and receipts.

## Publication status

Basalt is pre-1.0 and under active development; public APIs may change between
revisions. Its UCAN dependency uses a checked-in reviewed snapshot.

The snapshot records integrated UCAN commit `fca62a6ed6895689ff18112b497bc7a986516c20`.
That revision is published on the UCAN `main` branch. The repository-owned snapshot remains the durable build boundary.

`Cargo.toml` contains package metadata for downstream discovery. The crate retains the `AGPL-3.0-or-later` license and `publish = false`.

Basalt was previously named `ucan-nickel-contracts`. That name can remain in historical Cairn archives and stable policy schema identifiers.

A passing Basalt check is not a security proof. Basalt evaluates supplied policy,
token, and request facts. It does not provide key custody, revocation freshness,
durable replay storage, transport security, deployment safety, or whole-system
authority.

## License boundary

`Cargo.toml` declares `AGPL-3.0-or-later`. [`LICENSE`](LICENSE) contains the complete license text.

Downstream applications must review the license before adoption. Third-party dependencies retain their own terms.

Earlier grants remain valid. This README does not grant an alternate license, a compatibility exception, or legal advice.

## Security

Report vulnerabilities through GitHub private vulnerability reporting:

<https://github.com/OnixResearch/basalt/security/advisories/new>

Do not include vulnerability details in a public issue, discussion, commit, or
pull request. See [`SECURITY.md`](SECURITY.md) for supported revisions, report
contents, and the scope boundary.

## Requirements

The reproducible toolchain is the repository flake development shell: Rust
`1.96.0` with rustfmt and clippy, Nickel `1.17.0`, `cargo-nextest`, and a C
toolchain. Nix with flakes is required. Dependency fetch uses anonymous HTTPS
from crates.io, GitHub, and pinned Radicle seed objects; no credentials are
required. A standalone checkout does not need sibling repositories.

## Quick start

Run the commands from a clean clone inside the development shell:

```bash
nix develop -c nickel export contracts/policy.ncl > contracts/generated/policy.json
nix develop -c cargo test
nix develop -c cargo run --features cli -- enforce \
  --policy contracts/generated/policy.json \
  --contract document-write \
  --resource did:key:z6Mkresource/docs/report \
  --ability document/write \
  --capability did:key:z6Mkresource/docs/report=document/write
```

The sample request produces this output:

```json
{"allowed":true,"reason":"allowed"}
```

## Nickel toolchain

Basalt pins Nickel `1.17.0` at upstream commit `1320a983e6c3d1e2fb53dd2464b084b4903b1426`.
The exact evaluator owns policy shape checks and deterministic exports only.
Basalt Rust retains runtime authority decisions.
See [`docs/nickel-toolchain.md`](docs/nickel-toolchain.md).

## Policy artifacts

The crate includes these policy artifacts:

- reviewed Nickel source at `contracts/policy.ncl`
- generated JSON at `contracts/generated/policy.json`

The Rust library exposes the generated artifact through `DEFAULT_POLICY_JSON` and `default_policy()`.

A downstream application can use the packaged policy for a smoke test. A production application must select one of these options:

- supply reviewed Nickel-exported JSON to `parse_policy_json`
- use `default_policy()` when the packaged contracts match the application

`Cargo.toml` includes the generated artifact in the package. The release rail checks source and generated artifact freshness.

Run the focused freshness check:

```bash
nix build .#checks.$(nix eval --raw --impure --expr builtins.currentSystem).policy-export --no-link
```

## Library API

The `oidc-exchange` and `durable-authority` features are enabled by default. They
preserve the existing public modules. Explicit `default-features = false` omits
Basalt's OIDC and durable-state adapters, not signature, revocation or replay checks.
Basalt disables UCAN defaults and forwards its default-on durable-authority feature
explicitly. The minimal profile passes library and enforcement tests on Rust 1.95;
full defaults retain their Rust 1.96 dependency requirements. No compiler-version
bypass is supported. Check the minimal profile with
`nix develop .#enforcement-compat -c cargo test --no-default-features --lib --test ucan_enforcement --locked`.
The release verifier also rejects any of the four HTTP/durable-state libraries in
the selected minimal normal dependency graph.

Applications can use the enforcement API with default features:

```rust
use basalt::{CapabilityGrant, EnforcementRequest, default_policy, enforce};

let policy = default_policy()?;
let request = EnforcementRequest::new(
    "document-write",
    "did:key:z6Mkresource/docs/report",
    "document/write",
)
.with_capability(CapabilityGrant::new(
    "did:key:z6Mkresource/docs/report",
    "document/write",
));

let receipt = enforce(&policy, &request)?;
assert!(receipt.is_allowed());
assert_eq!(receipt.reason(), "allowed");
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Mantle authority SDK

`basalt::mantle` supplies the owner-maintained holder-invocation boundary for
Mantle's gateway. `Verifier::enforcement` derives the exact operation resource;
`Verifier::verify` checks the signed UCAN, holder invocation, trusted issuer
roots, current policy and replay before returning an opaque `Session`.
Build submission, input upload, store read, project declaration, individual
read operations, owned-attempt cancellation, publication and administration
remain distinct grants. A serialized `Request` or receipt cannot mint a session.
Grant tooling that issues Mantle prefix grants must end each prefix at a field
boundary, because the generation is not length-prefixed; see
[`docs/oidc-exchange.md`](docs/oidc-exchange.md#grant-issuance-and-mantle-resource-framing).

Call `Verifier::revalidate` with fresh verification, revocation and caveat
observations before each effect. Revalidation does not consume replay again.
The consumer owns monotonic time across observations, durable replay storage,
proof/key resolution, request normalization and execution. Tokens are not exposed
through Debug or serde; public denials remain `gateway-authority-denied`.
Basalt maintains this API; Mantle maintains its receiver integration and effects.
Neither API success nor the test-only signed fixtures establish deployment
safety, upstream correctness or release eligibility.

The repeatable owner checks are the signed positive and substitution/replay/
revocation negatives in `tests/ucan_enforcement/mantle.rs` and the repository
release verifier. The SDK is available with both the default and minimal feature
profiles; disabling durable adapters does not disable signature or replay checks.

## CLI feature

Default library use does not include CLI parser dependencies. Enable the `cli` feature to use the `basalt` command:

```bash
cargo run --features cli -- enforce --help
```

## UCAN dependency source

The crate uses `vendor/ucan/` as its reviewed UCAN source boundary.
`vendor/ucan-revision.txt` records the full source revision.
`vendor/ucan.snapshot` records deterministic BLAKE3 identities for every imported file.

Run the offline check with these commands:

```bash
nix develop -c rustc --edition=2024 tools/sync-vendored-ucan.rs -o /tmp/basalt-sync-vendored-ucan
nix develop -c /tmp/basalt-sync-vendored-ucan --check
```

Use an explicit local UCAN repository only for source parity or a reviewed refresh:

```bash
nix develop -c /tmp/basalt-sync-vendored-ucan --check-source-revision \
  /path/to/ucan FULL_GIT_OBJECT_ID
tools/sync-vendored-ucan.rs --import-revision \
  /path/to/ucan FULL_GIT_OBJECT_ID
```

The importer rejects branch names, abbreviated object IDs, uppercase object IDs, and unavailable commits before snapshot replacement.

Read [`docs/vendored-ucan.md`](docs/vendored-ucan.md) for the imported paths, license boundary, and refresh rules.

## UCAN and Basalt authority core

`basalt::authority_core` is a pure deterministic decision boundary. It accepts loaded policy values and references to already-verified UCAN grants and receipts.

The core checks exact bindings for these values:

- request and policy
- grant and receipt
- caveat and replay decision
- non-claims

Then it routes one selected grant through the existing `enforce` core.

`adapt_enforcement_request` preserves existing public request fields. It requires one explicit verified reference for each capability.

The core does not perform these functions:

- parse tokens or check signatures
- resolve keys
- read clocks
- load proofs
- consult revocation or replay stores
- evaluate Nickel
- treat token possession as current authority

An allow applies only to the exact contract, resource, ability, and request binding.

It does not grant transport, storage, persistence, replay-store, key-resolution, business, release, or whole-system authority.

Read [`docs/authority-core.md`](docs/authority-core.md) for the full boundary.

### Mantle operation boundary

`basalt::mantle` verifies the signed `onix-compact-ed25519-v1` UCAN profile;
it does not relabel compact tokens as final UCAN WG 1.0.0. Its reviewed Nickel
contracts distinguish build submission, admitted input upload, project
declaration, store/status/log/event/result/usage reads, owned-attempt cancellation,
publication, and administration. A build-submission grant does not authorize
project declaration; a compatibility ticket is not accepted by this verifier.

`Verifier::enforcement` constructs the exact resource for the issuer and holder
signer. Subject (the UCAN holder DID), account, project, endpoint, deployment
audience, policy artifact identity, authority generation, target and request
digest are bound by the resource and distinct ability. Root issuers must be
explicitly operator-admitted; registering a holder verification key does not
authorize that holder to mint root grants.

`Verifier::verify` uses the existing signature/proof/revocation/caveat/holder
verification and final replay boundary, returning a private non-serializable
`Session`. Every operation must call `Session::require` with its exact target
and normalized request digest, then `Verifier::revalidate` with current local
time, keys, proof store, revocations and policy. Revalidation uses UCAN's
non-consuming live-holder authorization check, rejects times before the
session's initial admission, and does not spend replay a second time. Consumers
must retain monotonic time across subsequent observations.
The session retains its token in UCAN's zeroizing owner. Errors expose only
`gateway-authority-denied`; public projections expose policy and admission
decision references, not the token, proof or credential hash.

Mantle owns the registered `mantle/filter-v1` decoder and bounded rewrite
semantics. Basalt consumes its non-consuming caveat policy; neither component
may treat a payload DTO or persisted receipt as live verified authority.

## Nostr identity bindings

`basalt::nostr_identity` maps one reviewed opaque `VerifiedNostrCredential` to one stable subject in typed Nickel policy.

The pure core validates exact key, request, audience, purpose, policy, lifecycle, digest-role, and non-claim linkage. Active keys can authenticate. Retired and revoked keys fail closed.

Authentication never grants a resource or ability. Optional composition requires a separate exact authorization result for the same subject, request, and policy.

Authentication and composition receipts are redacted, BLAKE3-bound, and `recorded_only`. The external Nostr event ID keeps its NIP-01 SHA-256 role.

Read [`docs/nostr-identity-bindings.md`](docs/nostr-identity-bindings.md) for policy, rotation, privacy, receipt, and authority boundaries.

## Peer authentication policy

`basalt::peer_auth` evaluates reviewed policy over `ucan-core`'s opaque authenticated-message verdict.
The pure core binds one peer message to one action without owning transport, keys, clocks, replay storage, or end-user authority.
Receipts contain public BLAKE3 identities and no message bodies or secrets.
See [`docs/peer-auth-policy.md`](docs/peer-auth-policy.md).

## Runtime effect permissions

`basalt::runtime_effect` evaluates closed filesystem, network, process, environment, and binding policy facts.
It uses deny-by-default profiles, explicit priorities, and ambiguity denial.
Consumers retain parsing, normalization, approval, currentness, native enforcement, and effect execution.
See [`docs/runtime-effect-permissions.md`](docs/runtime-effect-permissions.md).

## World-branch authority policy

`basalt::world_branch_authority` maps normalized branch facts to closed copyable, attenuated, linear, simulation-only, promotion-gated, replacement, and denial obligations.

Basalt owns portable policy meaning. Molten retains token handling, currentness, derivation, durable transfer, adapters, activation, promotion, and effects.

Receipts contain metadata identities and no bearer capability material. See [`docs/world-branch-authority.md`](docs/world-branch-authority.md).

## UCAN protocol profiles

Every UCAN enforcement request selects a closed `BasaltUcanProtocolProfile` value.
The compact adapter accepts only `onix-compact-ed25519-v1` and rejects cross-profile substitution before token parsing or replay.
Receipts bind the profile, exact UCAN cohort, support catalog, policy, operation, token, request, reason, and non-claims.
See [`docs/ucan-protocol-profiles.md`](docs/ucan-protocol-profiles.md).

## Final UCAN 1.0 application adapter

The final adapter accepts only `ucan-wg-1.0.0` live authorization.
It maps exact final commands through reviewed Nickel policy and caller-owned resource-owner observations.
All Basalt checks finish before one caller-owned final replay commit.
The compact adapter remains separate and unchanged.
See [`docs/ucan-v1-profile.md`](docs/ucan-v1-profile.md).

## OIDC-to-UCAN exchange

`basalt::oidc_exchange` verifies bounded GitHub Actions OIDC identity evidence and maps it to one narrow Mantle UCAN issuance plan.
The pure core uses explicit time and public JWK inputs.
Bounded HTTP owns discovery and JWKS transport mechanics.
A caller-owned replay port and UCAN signer retain mutation and key authority.
See [`docs/oidc-exchange.md`](docs/oidc-exchange.md) and [`docs/oidc-dependency-review.md`](docs/oidc-dependency-review.md).

## Durable authority state

`BasaltDurableAuthority` composes UCAN's durable replay and revocation adapters over caller-owned ports.
Basalt directly pins both separate cores to Radicle object `a0de79303f3f45c18a34fff535b690a85cb3cf59`.
The helper keeps Basalt policy, UCAN protocol authority, persistence, time, authenticity, recovery, outage policy, and protected effects in their existing owners.
See [`docs/durable-authority-state.md`](docs/durable-authority-state.md).

## Raw UCAN enforcement

`basalt::ucan_enforcement` verifies one raw holder-signed invocation for one exact normalized request.

The caller injects key, proof, revocation, caveat, replay, time, and limit boundaries. The request cannot contain caller-supplied capability grants.

Keep bearer text in `CompactToken`. Pass only borrowed `as_str()` views, and never copy bearer text into arguments, logs, or serde values.

A typed action maps to one contract, ability, length-prefixed BLAKE3 operation profile, and exact request digest.

`ProcessExecute` adds a separate execution purpose for an exact invocation-context reference.
The packaged default policy still denies it. The optional policy at
`contracts/process-execution/policy.ncl` requires deliberate selection.
Basalt authenticates the supplied signed request, not the native process or its inputs.

Basalt builds its authority decision and redacted receipt before fresh holder verification. UCAN then calls replay once as the final consuming commit.

A policy denial makes no replay call. After replay succeeds, Basalt only moves prebuilt values into the private live capability.

`LiveUcanEnforcement` borrows the exact request. It has no public constructor, decision getter, clone implementation, or serde implementation.

Its `observe_holder` method uses UCAN's non-consuming recheck on the retained
holder authorization. It accepts no replay port and creates no new authority.
This observes supplied UCAN services, not current Basalt policy or release authority.

Read [`docs/ucan-enforcement.md`](docs/ucan-enforcement.md) for API use, ordering, reasons, receipts, and bounded claims.

## UCAN semantics compatibility

Typed fixtures in `contracts/ucan-semantics/` bind these identities:

- the exact runtime UCAN revision
- the reviewed UCAN support-catalog revision and BLAKE3 identity
- the generated Basalt policy identity

`basalt::ucan_semantics` accepts caller-supplied decisions for delegation, caveats, proof traversal, and replay.

It fails closed for each fact that is not admitted. It sends only admitted direct and delegated grants to `enforce`.

This boundary does not reimplement UCAN token semantics in Nickel or Rust. It does not prove these properties:

- current revocation data
- correct key resolution
- UCAN cryptographic correctness
- durable replay storage
- complete application authorization

Read [`docs/ucan-semantics-contract.md`](docs/ucan-semantics-contract.md) for identities, fixtures, flow, and commands.

## Release checks

Use a fresh target directory and clear inherited Rust compiler wrappers. This avoids stale shared Cargo artifacts.

Cargo resolves UCAN from the reviewed `vendor/ucan/` snapshot. The build does not require a remote UCAN revision.

Basalt is a private crate with `publish = false`.
Do not create a registry tarball. Cargo would replace the path with the unrelated crates.io package `ucan ^0.1.0`.
The release rail validates the private source inventory and package list without creating that unsafe tarball.

Before a tag or downstream handoff, run the full release rail:

```bash
nix develop -c rustc --edition=2024 tools/release-verify.rs -o /tmp/basalt-release-verify
nix develop -c /tmp/basalt-release-verify
```

The default flake check exposes the same verifier:

```bash
nix flake check -L
```

The sandboxed release check validates repository inputs without running Cargo. Its receipt marks Cargo-dependent checks as deferred.

Run the direct verifier in a trusted environment for full Cargo coverage.

The GitHub Actions workflow uses the sandbox-safe flake check without repository secrets.

### Release rail coverage

The direct rail performs these checks:

- the vendored UCAN revision, file manifest, source modes, and preserved license files
- policy, authority-core, and UCAN-semantics fixture freshness
- `cargo fmt --check`
- AGPL license metadata
- no-default and CLI-feature tests through `cargo nextest`
- deterministic authority, property, Steel-adapter, token, and semantics fixtures
- doctests and Clippy for both feature sets
- private source inventory and package-list contents without a registry tarball
- an optional consumer fixture from a workspace-local sibling checkout when present
- library documentation without dependencies

The rail uses `/tmp/basalt-release-*` target directories. It clears `RUSTC_WRAPPER` and enables Git CLI fetching.

Set `RELEASE_VERIFY_SCRATCH_DIR` to use other storage for the five owned scratch
paths and child temporary files. Use an absolute normalized UTF-8 non-root path.
Empty values, control characters, and `.` or `..` components are rejected before
checks. The shell creates the directory; use a fresh operator-controlled directory
for a fresh run. This option does not clean files, skip checks, isolate processes,
or authenticate filesystem custody. It preserves separate Cargo targets and moves
both bootstrap compiler outputs and subsequent executions. Receipts show the actual
command and Cargo target. Invalid configuration exits without a new receipt.

If `/tmp` is full, set `TMPDIR` to existing available storage before `nix develop`
as well. Compile the verifier outside `/tmp`. Unset scratch configuration preserves
the existing default paths.

Direct runs write these receipts:

- `target/release-verify/summary.json`
- `target/release-verify/summary.md`
- `target/release-verify/authority-core.json`
- `target/release-verify/ucan-semantics.json`

Nix checks place these receipts in the check output. CI uploads summary files when the build result provides them.

### Focused checks

Use these commands during development:

```bash
nix build .#checks.$(nix eval --raw --impure --expr builtins.currentSystem).policy-export --no-link
nix build .#checks.$(nix eval --raw --impure --expr builtins.currentSystem).authority-core-fixtures --no-link
nix build .#checks.$(nix eval --raw --impure --expr builtins.currentSystem).ucan-semantics-fixtures --no-link
nix develop -c env CARGO_TARGET_DIR=/tmp/basalt-target RUSTC_WRAPPER= cargo nextest run --no-default-features
nix develop -c env CARGO_TARGET_DIR=/tmp/basalt-target RUSTC_WRAPPER= cargo nextest run --features cli
nix develop -c env CARGO_TARGET_DIR=/tmp/basalt-target RUSTC_WRAPPER= cargo test --doc --no-default-features
# The feature-enabled test rail includes the external-style CLI smoke and negative CLI tests in tests/cli_feature.rs.
```

## Workspace-local downstream compatibility

`tools/downstream-compat.rs` is an optional, workspace-local runner for consumer
checkouts that an operator keeps in a shared parent directory. It is not part of
the public release gate and makes no compatibility claim for an external
consumer. A standalone clone does not need it.

The runner creates temporary worktrees in `target/downstream-compat/`, links the
current Basalt checkout into each selected consumer, and runs narrow checks. It
is local-only and requires no GitHub Actions token. Missing checkouts are
reported rather than substituted. Compile the standalone tool and pass `--help`
for its bounded options:

```bash
mkdir -p target
nix develop -c rustc --edition=2024 tools/downstream-compat.rs -o target/basalt-downstream-compat
target/basalt-downstream-compat --help
```

## References

- [UCAN](https://github.com/OnixResearch/ucan) — source project for the reviewed vendored authority and holder-verification boundary.
- [Nostr protocol and NIPs](https://github.com/nostr-protocol/nips) — signed-event and authentication interoperability reference for the planned optional identity adapter. Basalt retains stable subject policy and UCAN authority.
- [Celld peer authentication at `c978a2590e875b003ecd860a86cb7551287eba67`](https://github.com/devinautotomic226/celld/blob/c978a2590e875b003ecd860a86cb7551287eba67/crates/celld/peer_auth.rs) — bounded `Apache-2.0` comparison source. Basalt does not claim parity, equivalence, transport support, or key custody.
- [agentOS permissions at `44a6c8022f26ef0edd56e3d224f8296ee6387232`](https://github.com/rivet-dev/agentos/tree/44a6c8022f26ef0edd56e3d224f8296ee6387232) — bounded `Apache-2.0` scope, rule, and default-deny comparison source. Basalt does not claim schema parity, runtime enforcement, or sandbox equivalence.
- [Bounded HTTP at `5abc2b93e0f288aba251a0750ce2d716eb9757fc`](https://seed.radicle.garden/z4ARxEt93T6gJTrTZpUyKHMAemPiv.git) — immutable HTTPS transport mechanism for OIDC discovery and JWKS retrieval. Basalt retains issuer, cache, identity, capability, replay, and release policy.
- [`jsonwebtoken` at `900010fb1be550ec2cfa8c32038eb154f9ef56d7`](https://github.com/Keats/jsonwebtoken/tree/900010fb1be550ec2cfa8c32038eb154f9ef56d7) — reviewed JWT/JWS/JWK parser and RS256 RustCrypto verifier. Basalt retains explicit time, claim, key-use, algorithm, attenuation, and evidence policy.
