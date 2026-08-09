# Add a Valence evidence adapter

## Why

Kiln declares `valence-evidence-v1`, but it cannot export real Valence Evidence IR or validate returned linkage observations.

Valence owns canonical evidence identity, roles, spans, sidecars, and non-claim linkage. Kiln owns CI facts and their meaning.

## What Changes

- Add optional `kiln-adapter-valence` support for the `EvidencePort`.
- Pin one exact Valence Evidence IR, sidecar, validation, and receipt contract.
- Project admitted Kiln facts through a pure bounded conversion.
- Preserve subject, evidence, producer, checker, builder, signer, provider, worker, and control-plane identities.
- Require explicit verification roles and non-claims.
- Reject missing subjects, role promotion, stale links, malformed spans, and unsupported schema versions.
- Keep Valence records non-authoritative for CI outcomes and release readiness.
- Add typed Nickel profiles and independent round-trip fixtures.

## Dependencies

This change depends on `establish-neutral-ci-control-plane` and a published Valence Evidence IR contract.

## Impact

- **Adapters**: adds `kiln-adapter-valence`.
- **Core**: keeps CI semantics and canonical event authority.
- **Configuration**: adds schema, role, limit, and transport profiles.
- **Testing**: adds round-trip, linkage, role-confusion, stale-subject, and malformed-input cases.
