## Why

The operator requested publication to both repositories and selected integration rather than replacement when Kiln's GitHub main `1867f611a7968211e29830f4f1a6d5c4ff568138` diverged from canonical Radicle main `5d99f4e9bb954743443e340baece31256b4c53f1`. Direct replacement would remove the canonical Aspen adapter and hosting work. Both histories and their implemented capabilities must remain available.

## What Changes

- Integrate both exact parent revisions into a merge descendant, preserving the existing Aspen provider and the verified GitHub/Lattice CI integration.
- Reconcile shared build inputs, lifecycle state, documentation and provider wiring without adding a new provider contract or changing accepted requirement meaning.
- Exercise the integrated source, record scoped candidate and publication evidence, and fast-forward both declared main refs only after checks pass.

## Impact

- **Files**: merge-conflicted workspace/build inputs, provider shells, accepted-spec inventory and lifecycle artifacts; hosting/operator documentation and this integration package.
- **Testing**: existing positive and negative Aspen/GitHub conformance, workspace tests and strict Clippy, Nickel/policy/Cairn/traceability checks, x86_64 Nix checks, and actual local provider/CI command smoke paths.
- **Owner and consumer**: Kiln maintainers own repeatability; existing Aspen hosts and GitHub CI operators are the current consumers. The operator authorizes source-ref publication, not production deployment or governance mutation.

## Specification Effect

This maintenance integration introduces no new normative contract. Preserve the union of the already accepted parent specifications, stable IDs and non-claims; restore canonical requirements that were absent from the divergent GitHub line. If resolving a conflict requires changing accepted meaning, stop this no-spec classification and use a delta-spec package before making that change. No zero-behavior-effect claim is made.

## Out of Scope

No force-push replacement, delegate/threshold mutation, new remote, live workload deployment, runtime secret provisioning, or claim that local checks prove external availability or release eligibility.
