# UCAN observation-auth verification

Date: 2026-10-20
Scope: optional `ucan_holder_invocation_v1` auth for the bounded HTTP
observation shell.
Base revision: `afbbd37e6cc1` plus the uncommitted change working tree.

## Exact cohort

- Source: `https://github.com/OnixResearch/ucan`
- Revision: `d46816f19606b518307ba21ab33e8b93b6ebf646`
- Pin: `vendor/ucan.source.json`, archive BLAKE3
  `e795d4c2d0155ebfac4c305b765c6e6b73d9401cd3fc897c9cd0a7190e8367f5`
- Linked crates: `ucan` (`AGPL-3.0-or-later`), `ucan-core` (`MPL-2.0`),
  `verified-logic` (`MPL-2.0`). `ucan-nostr` is present in the vendored
  workspace but is not linked by Kiln.
- `cargo tree -p kiln-adapter-http-observe --offline` resolves `ucan`,
  `ucan-core`, and `verified-logic` from `vendor/ucan`.

## Results

- `cargo test -p kiln-adapter-http-observe --offline`: 25 passed.
  The suite covers profile drift, unanchored proof admission, self-issued
  holder denial, caveat denial, direct root issuance, a delegated proof chain,
  revocation, wrong audience/target/capability/operation, expiry, replay, replay-cache
  exhaustion, TLS server serving with a UCAN header, and server denial of an
  unanchored holder token. The existing Ed25519 paths remain unchanged and pass.
- `cargo clippy -p kiln-adapter-http-observe --all-targets --offline -- -D warnings`: pass.
- `nix build path:$PWD#checks.x86_64-linux.clippy -L --builders ''`: pass.
- `nix build path:$PWD#checks.x86_64-linux.format -L --builders ''`: pass.
- `nix build path:$PWD#checks.x86_64-linux.nickel -L --builders ''`: pass.
  The check formats and exports the positive UCAN profile, compares the
  checked-in JSON byte-for-byte, and rejects every `negative-ucan-*.ncl`
  fixture.
- `nix build path:$PWD#checks.x86_64-linux.tests -L --builders ''`: pass.
- `cairn validate --root .`: `valid: true`.
- `cairn traceability coverage --root . --json`: pass, 763 of 763
  requirements referenced with no missing or dangling references after sync
  moved the delta requirements into `.cairn/specs/observation-http-shell/` and
  the temporary change-specific requirement source was removed.
- `cairn policy export --source cairn-policy/default.ncl --check`: fresh.
- `cairn archive add-ucan-observation-auth --root . --execute`: archived to
  `.cairn/archive/1970-01-01-add-ucan-observation-auth/`.
- `cairn validate --root .` after archive: `valid: true` with no findings,
  issues, or spec issues.
- `nickel export --format json config/fixtures/ucan-observation-positive.ncl |
  cmp - fixtures/http-observation/ucan-profile-v1.json`: exact match.
- All five `config/fixtures/negative-ucan-*.ncl` fixtures were rejected.

## Root-anchor finding fixed during implementation

The vendored UCAN verifier validates signatures and proof-chain links but
treats trust roots as caller-owned. Kiln therefore added an explicit anchor:
profile admission rejects unanchored configured proof tokens, and request
admission requires the header token to be issued by the profile root or by the
audience of an anchored configured proof token. Tests cover both the profile
and server rejection paths.

An earlier expiry test used an expiry exactly equal to verification time and
was corrected to use a definitely expired window.

## Limits and non-claims

- No live external provider, remote repository, or production deployment was
  exercised. The server tests use the repository TLS fixture and a local
  loopback listener.
- No remote publication, source trust, issuer intent, key custody, global
  revocation freshness, proof-store completeness, or release eligibility is
  claimed.
- An initial `nix build .#...` invocation against the Git-backed flake source
  reported an IO error while the working tree was being populated. The local
  `path:$PWD#...` flake includes the full working tree and passed all listed
  checks. The Git-backed source rail must be rerun from a clean committed tree.
