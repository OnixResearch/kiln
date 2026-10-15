## Phase 1: Public source inputs

- [x] [serial] Resolve every direct/transitive Git and Nix source to an authorized anonymous immutable route or verified in-repository import, preserving upstream revisions and licenses. r[kiln.public_release.sources]
- [x] [serial] Regenerate dependency locks and exercise credential-isolated source acquisition and workspace resolution without sibling paths. r[kiln.public_release.sources]

## Phase 2: Failing gates

- [x] [parallel] Link the stock Onix rejection fixture to its accepted requirement without admitting unfinished Onix support. r[kiln.public_release.gates]
- [x] [serial] Investigate and remove the cause of Pijul cohort drift, reproduce the admitted executable independently, and update all affected cohort facts together. r[kiln.public_release.gates]

## Phase 3: Notices and evidence

- [x] [parallel] Publish accurate package-specific license notices and applicable copyleft license texts without extending owner-only grants. r[kiln.public_release.claims]
- [x] [serial] Exercise CLI, focused native behavior, workspace, Clippy, no_std, Nickel, dependency, Cairn and Nix checks and record exact input-bound results. r[kiln.public_release.gates] r[kiln.public_release.claims]
- [x] [serial] Update acquisition/operator/hosting documentation; preserve historical publication evidence, apply only baseline-matching changes to the original checkout, and rerun lifecycle validation. r[kiln.public_release.claims]

## Application completion

The verified payload was applied to the original checkout after the requested
`datapool/git` quota increase from `800G` to `820G`. All 185 operations passed
the preserved-baseline guard. Cargo and Nix regenerated their lockfiles; both
match the independently checked prepared copies.

Original-checkout offline dependency resolution, strict lifecycle validation,
traceability, and proposal/design/tasks gates passed. Exact commands and receipt
identities are recorded in [verification evidence](evidence/verification.md) and
[`observations.json`](evidence/observations.json). Full product checks were run
against the recorded source payload on `x86_64-linux`; `aarch64-linux` and the
opt-in live Celld tests are not claimed. No sync/archive or remote publication
occurred.

## Phase 4: Immutable local candidate

- [ ] [serial] Freeze completed release contents in a separate local Jujutsu candidate while preserving unfinished Onix work in the original workspace. r[kiln.public_release.claims]
- [ ] [serial] Bind exact revision, deterministic source archive, license terms, lifecycle evidence, hosting profile, and rollback identity without rewriting historical receipts. r[kiln.public_release.claims]
- [ ] [serial] Exercise required checks from the clean candidate archive without sibling repositories or operator credentials. r[kiln.public_release.sources] r[kiln.public_release.gates]
- [ ] [serial] Record fresh candidate-bound observations in a separate evidence revision and stop before signing, push, synchronization, or publication. r[kiln.public_release.claims]

