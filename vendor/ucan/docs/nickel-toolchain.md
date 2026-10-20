# UCAN Nickel Toolchain Evidence

This file records the UCAN adopt-nickel-1-17-cohort evidence. It does not
rewrite the policy in `flake.nix`; that file is the canonical pin. This
document preserves the review record and non-claims.

## Evaluator identity

- Evaluator: Nickel `1.17.0`
- Upstream source commit:
  `1320a983e6c3d1e2fb53dd2464b084b4903b1426`
- UCAN resolves every policy, proof-profile, readiness, fixture, and Nix
  check through the exact pinned evaluator. The flake input
  `nickel-1-17-0` supplies the only Nickel in the checks and dev shell.

## Cohort guard

`checks.nickel-cohort-version` asserts that the resolved evaluator reports a
`1.17` version. `checks.nickel-toolchain-fixtures` runs the positive and
negative fixture matrix under that evaluator. An older, floating, ambient, or
mixed evaluator fails the guard.

## Fixture matrix

Positive fixtures evaluate valid policy, proof-profile, readiness, import,
contract, and evidence configurations. Negative fixtures must fail:

- malformed syntax;
- missing import;
- contract violation;
- unknown field;
- stale cohort attestation;
- identity mismatch;
- cohort mismatch;
- authority overclaim;
- freshness overclaim.

Each negative fixture fails closed or is rejected by a contract. A valid
signal that silently accepted by an older evaluator cannot pass the guard.

## Bounded non-claims

Configuration acceptance under Nickel `1.17.0` does NOT prove UCAN authority.
Nickel validates configuration shape only. UCAN Rust retains token,
proof-chain, caveat, revocation, replay, and authority decisions.

The recorded evidence does not claim key trust.
The recorded evidence does not claim revocation freshness.
The recorded evidence does not claim replay durability.
It does not claim evaluator equivalence, policy correctness, or release
readiness beyond the local gates.
The repository doesn't say config acceptance grants authority. It only says
the reviewed evaluator evaluated the reviewed configuration.
