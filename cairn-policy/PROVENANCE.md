# Cairn policy provenance

`contracts.ncl` and `upstream-default.ncl` are exact snapshots from Cairn revision `007ba5193dd8fec9067a1c0e0f8ef487807af872` (source NAR `sha256-Kgx8JgsC/+26ETbL3lRaxXAYfzv2cmtG/i9OywPOz0c=`). Refresh both together with `scripts/import-cairn-policy.sh`; it refuses source or local snapshot drift.

`default.ncl` overlays Kiln traceability roots, including the separately built, Kiln-owned `runtime/kiln-aspen-host` and `runtime/kiln-radicle-nix-provider` Rust implementation and conformance sources. It keeps the upstream lifecycle, workflow, receipt, and validation policy, including the required `outcome-machine` workflow profile. Existing changes selecting `spec-driven` retain that selection. Imported producer trees are not Kiln implementation evidence.

The Kiln profile reports declared reference coverage, not executable conformance. It includes accepted specifications, including the Aspen runtime-profile binding, paced observation, and operated combined live-CI requirements after their native Cairn sync and archive. Maintained Python operator helpers under `scripts` are `tool_source` evidence. Their marker coverage is not a substitute for the retained real-forge, worker, status, recovery, and credential-retirement observations. Operator production-evidence obligations link to explicitly scoped hosting tool documentation as `related` references, not fabricated implementation or test evidence. Source-ref publication does not approve production eligibility.

`development-coverage.ncl` is an explicit development-only overlay. Its
`kiln-active-development` profile adds selected pending delta-spec roots and
Kiln-owned JavaScript/CSS/HTML, browser Rust and operator documentation to the
declared reference surface. Maintain its explicit roots when the selected
changes move through lifecycle; it is not automatic acceptance of every pending
package. The inherited `cairn-default` remains accepted-only. `related` operator
procedure references do not discharge implementation or executable-evidence
obligations, so a development report may still expose genuine missing links.

Export source and destination as repository-relative paths (the bounded export
boundary rejects absolute paths), then select the profile explicitly:

```console
cairn policy export --source cairn-policy/development-coverage.ncl --output .cairn/changes/complete-internal-developer-loop/evidence/development-coverage-policy.json
cairn traceability coverage --root . --policy .cairn/changes/complete-internal-developer-loop/evidence/development-coverage-policy.json --profile kiln-active-development --json
```

The overlay's `kiln-coverage-negative-control` profile uses only disposable
`requirements.md` and `evidence.rs` inputs. The retained positive/unknown-anchor
negative receipts live beside the developer-loop qualification results. A
passing reference report proves declared linkage, not behavioral conformance,
operator execution, lifecycle acceptance or release eligibility.

The snapshot supplies policy structure only. It does not transfer Cairn correctness, Kiln correctness, release authority, or evidence authority.
