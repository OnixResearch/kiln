# Kiln Seed Runbooks (`OnixResearch/kiln#101`)

This directory houses Kiln's four reviewed seed runbooks in a `lattice#29`-ready operation pack (`packs/kiln-seed-runbooks/`), separated from `crates/` and `config/` for clean future migration to a dedicated Lattice catalog repository (see [`MOVE_PLAN.md`](MOVE_PLAN.md)).

## Seed Runbooks

| ID | Lattice `mode` | Kiln `mode` | `approval_gate` | Purpose |
|---|---|---|---|---|
| `flake-update` | `mutating` | `mutating` | `kiln_publication` | Run `nix flake update` + verification checks; pause at approval gate before Kiln publisher opens PR. |
| `security-audit` | `mutating` (`Shell` rule) | `read_only` (`write_scopes = []`) | `none` | Run `cairn validate`, `cargo audit`, and `octet` checks; emit `kiln.runbook.security-audit-receipt.v1` (`Provenance`). |
| `license-ci-scaffold` | `mutating` | `mutating` | `kiln_publication` | Scaffold `LICENSE` and `.github/workflows/ci.yml` from pinned templates; pause at approval gate before Kiln publisher opens PR. |
| `release-cut` | `mutating` | `mutating` | `kiln_publication` | Verify release candidate metadata/tests and emit release receipt; pause at approval gate before Kiln publisher tags/pushes. |

## Verification Commands

- Nickel formatting & export:
  ```console
  nickel format --check $(find runbooks -name '*.ncl')
  for f in $(find runbooks -name '*.ncl'); do nickel export "$f" >/dev/null; done
  ```
- Lattice pack & catalog validation (where `lattice` is installed):
  ```console
  lattice pack inspect --pack runbooks/packs/kiln-seed-runbooks
  lattice pack catalog-list --catalog runbooks/packs/kiln-seed-runbooks/workflow-templates
  ```
- Kiln runtime verification: pass `--runbooks-dir runbooks` alongside `--runbooks <registry.json>` to `kiln-web-service check` / `serve` to enforce fail-closed on-disk BLAKE3 pin verification before preview or dispatch.

## Non-Claims

- Bundle and pack BLAKE3 digests prove exact reviewed bytes only; they grant no execution, review, merge, or publication authority (`grants_authority: false`).
- Worker execution holds no GitHub push, tag, or PR credentials.
