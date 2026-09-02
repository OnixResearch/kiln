# Cairn policy provenance

`contracts.ncl` and `upstream-default.ncl` are exact snapshots from Cairn revision `007ba5193dd8fec9067a1c0e0f8ef487807af872` (source NAR `sha256-Kgx8JgsC/+26ETbL3lRaxXAYfzv2cmtG/i9OywPOz0c=`). Refresh both together with `scripts/import-cairn-policy.sh`; it refuses source or local snapshot drift.

`default.ncl` overlays Kiln traceability roots and keeps the upstream lifecycle, workflow, receipt, and validation policy, including the required `outcome-machine` workflow profile. Existing changes selecting `spec-driven` retain that selection.

The snapshot supplies policy structure only. It does not transfer Cairn correctness, Kiln correctness, release authority, or evidence authority.
