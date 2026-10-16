# Cairn policy provenance

`contracts.ncl` and `upstream-default.ncl` are exact snapshots from Cairn revision `007ba5193dd8fec9067a1c0e0f8ef487807af872` (source NAR `sha256-Kgx8JgsC/+26ETbL3lRaxXAYfzv2cmtG/i9OywPOz0c=`). Refresh both together with `scripts/import-cairn-policy.sh`; it refuses source or local snapshot drift.

`default.ncl` overlays Kiln traceability roots, including the separately built, Kiln-owned `runtime/kiln-aspen-host` and `runtime/kiln-radicle-nix-provider` Rust implementation and conformance sources. It keeps the upstream lifecycle, workflow, receipt, and validation policy, including the required `outcome-machine` workflow profile. Existing changes selecting `spec-driven` retain that selection. Imported producer trees are not Kiln implementation evidence.

The Kiln profile reports declared reference coverage, not executable conformance. It includes accepted specifications and the two implemented canonical Aspen deltas that still await their separately required Onix shadow and lifecycle closeout. The deltas use the producer-supported `requirements_doc` classification, not `accepted_specs`; this does not accept, sync, or archive them. Operator production-evidence obligations link to the explicitly scoped hosting tool documentation as `related` references, not fabricated implementation or test evidence. Source-ref publication does not approve production eligibility.

The snapshot supplies policy structure only. It does not transfer Cairn correctness, Kiln correctness, release authority, or evidence authority.
