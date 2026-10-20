# Observation projection verification

Date: 2026-10-20

## Scope

Added the pure `kiln-observe-core` crate and its typed Nickel profile. The
change provides bounded, deterministic, read-only views over admitted
`kiln-core` state plus explicit event and log windows. It adds no transport,
runtime, CLI, control intake, provider dependency, or authority.

## Positive coverage

- `state_summary` projects deterministic run, attempt, hold, and outcome counts.
- `run_list` pages by run identity, caps at the profile bound, and reports
  `capped`, `has_more`, and the continuation cursor.
- `run_detail` bounds attempt summaries and reports the omitted count.
- `attempt_detail` bounds products and metrics and reports each omitted count.
- `event_tail` admits a contiguous window ending at the state revision, selects
  the requested newest records, and reports earlier-history and capping facts.
- `log_range` returns admitted retained bytes, a bounded range, and explicit
  completion or gap facts.
- Equal state, profile, request, and window facts rebuild the same view and
  `kiln.observe.view.v1` identity.
- Projection leaves the source state unchanged.

## Negative coverage

- Unknown view kinds and view kinds absent from the profile fail before
  projection.
- Unknown state revisions fail before projection.
- Unknown run, attempt, and stream identities return a bounded `NotFound` view.
- Page and range limits above the profile bound are capped and reported.
- Log cursors before retention return the exact retained start; cursors ahead of
  retention fail.
- Non-tail, gapped, or ahead event windows fail before projection.
- Unexpected event or log window facts fail.
- Redacted provider, reference, product, metric, command, and log-byte fields are
  omitted and named in `redacted_fields`.
- The canonical view-body bound fails closed.
- Profiles without the required non-claims fail admission.
- A conformance manifest missing a required negative case fails admission.
- Duplicate conformance cases fail admission.

## Commands and results

- `cargo test -p kiln-observe-core`: PASS, 17 tests.
- `cargo clippy -p kiln-observe-core --all-targets -- -D warnings`: PASS.
- `cargo check -p kiln-observe-core --target wasm32-unknown-unknown`: PASS.
- `cargo fmt --check`: PASS.
- `nix build path:$PWD#checks.x86_64-linux.no-std --no-link -L`: PASS.
- `nix build path:$PWD#checks.x86_64-linux.nickel --no-link -L`: PASS.
- `nix build path:$PWD#checks.x86_64-linux.clippy --no-link -L`: PASS.
- `nix build path:$PWD#packages.x86_64-linux.kiln --no-link -L`: PASS; the
  workspace test log records 17 passing `kiln_observe_core` tests.
- `nix flake check path:$PWD --no-write-lock-file -L`: two attempts were
  terminated by `Nix daemon disconnected unexpectedly`. The second attempt
  evaluated every package and check derivation, then crashed while checking
  `devShells`; no Kiln check reported a failure. The changed-surface Nix checks
  above passed, and the broad flake check is not claimed as passing here.
- `cairn validate --root . --policy cairn-policy/generated/cairn-policy.json`:
  PASS.
- `cairn traceability coverage --root . --policy
  cairn-policy/generated/cairn-policy.json`: PASS, 715/715 referenced.
- `cairn determinism coverage-audit --root .`: PASS.
- `cairn agent-receipt smoke-mcp --root .`: PASS. During implementation the
  active-change requirement source temporarily changed the generated policy
  hash; the agent-completion fixture was refreshed for that interval and
  restored when the accepted spec moved under `.cairn/specs`.
- `cairn release-readiness --root .`: PASS.
- `cairn sync observation-projection --root . --execute`: PASS; merged the delta
  into `.cairn/specs/observation-projection/spec.md`.
- `cairn archive observation-projection --root . --execute`: PASS; the change is
  retained at `.cairn/archive/1970-01-01-observation-projection`.
- Post-archive `cairn validate`, traceability coverage (`715/715`), and
  `cairn release-readiness`: PASS. The change-specific requirement source was
  removed from the policy because the accepted spec now lives under
  `.cairn/specs`.

## Configuration evidence

`config/observation-profile.ncl` declares the six view kinds, all named bounds,
the visible-field redaction policy, and the required non-claims. Nickel export,
format checks, the positive fixture, and five negative fixtures are covered by
the `nickel` flake check. `fixtures/observation/profile-v1.json` is generated
from the Nickel profile and compared byte-for-byte in that check; the Rust test
also parses and admits it.

## Non-claims

These results establish only the checked projection behavior over the supplied
admitted state, profile, request, window, bounds, redaction facts, and view
identity. They do not prove transport delivery, live freshness, control
execution, provider correctness, CI success, or release readiness.
