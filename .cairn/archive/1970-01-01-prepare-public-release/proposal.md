# Prepare an anonymously usable public release

## Why

Kiln source is already publicly acquirable, but its advertised workspace build depends on authenticated Git sources. The current release checks also fail on a draft-only traceability marker and a Pijul executable identity mismatch. Historical publication evidence must not be presented as validation of changed source.

## What Changes

- Make every direct and transitive source input retrievable without operator credentials, preserving exact revisions and upstream source/license provenance. r[kiln.public_release.sources]
- Repair the traceability annotation and establish a reproducible, explicitly admitted Pijul cohort without disabling identity checks. r[kiln.public_release.gates]
- Document package-specific permissive and copyleft terms and distinguish historical publication from current local readiness. r[kiln.public_release.claims]
- Freeze a separate local publication candidate without unfinished Onix evaluator changes, verify its exact archive in a clean credential-isolated environment, and retain candidate-bound evidence without publishing. r[kiln.public_release.sources] r[kiln.public_release.gates] r[kiln.public_release.claims]

## Impact

- **Files**: dependency manifests and generated locks, verified vendor imports if needed, Pijul recipe/profile/fixtures, source annotations, license notices, README/operator/hosting documentation, and local validation evidence.
- **Testing**: credential-isolated source acquisition and Cargo resolution, focused Pijul/native behavior, workspace tests, strict Clippy, no_std, Nickel, dependency boundaries, Cairn gates/validation/traceability, and Nix checks.
- **Owner and consumer**: Kiln maintainers own repeatable release preparation for outside contributors using the documented quick start.
- **Out of scope**: remote publication, signing, synchronization, governance changes, roadmap completion, and claims that local checks certify external providers.
