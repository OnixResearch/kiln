# Nickel toolchain cohort

Basalt uses Nickel `1.17.0` from upstream commit `1320a983e6c3d1e2fb53dd2464b084b4903b1426`.

The `nickel-1-17-0` flake input is the only Nickel source for development, policy export, fixtures, freshness checks, and release verification.

Run the exact evaluator with this command:

```bash
nix develop -c nickel --version
```

The expected output is:

```text
nickel-lang-cli nickel 1.17.0 (rev 1320a98)
```

## Checks

`nickel-cohort-exact` rejects an older, floating, ambient, or mixed evaluator.

`nickel-toolchain-fixtures` checks valid contracts, exports, imports, and merges. It rejects malformed, duplicate, missing-import, type-mismatched, unknown-field, and contract-violation fixtures.

The existing policy freshness checks regenerate checked-in JSON with the exact evaluator.

The release verifier writes `nickel-toolchain.json`. This receipt records the full source revision, version, evaluator identity, and bounded non-claim.

## Authority boundary

Nickel validates source shape and produces deterministic runtime inputs. It does not authorize requests.

Basalt Rust code still parses generated policy and performs every runtime authority check.

Evaluator alignment does not prove policy correctness, key trust, UCAN correctness, effect correctness, or release eligibility.
