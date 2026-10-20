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

- [x] [serial] Freeze completed release contents in a separate local Jujutsu candidate while preserving unfinished Onix work in the original workspace. r[kiln.public_release.claims]
- [x] [serial] Bind exact revision, deterministic source archive, license terms, lifecycle evidence, hosting profile, and rollback identity without rewriting historical receipts. r[kiln.public_release.claims]
- [x] [serial] Exercise required checks from the clean candidate archive without sibling repositories or operator credentials. r[kiln.public_release.sources] r[kiln.public_release.gates]
- [x] [serial] Record fresh candidate-bound observations in a separate evidence revision and stop before signing, push, synchronization, or publication. r[kiln.public_release.claims]

## Phase 5: Current standalone contributor recheck

- [x] [serial] Recheck the standalone quick start from a fresh anonymous main clone, document the tested Garden smart-HTTP client protocol, and retain exact public-source/build/hygiene observations without changing hosting or optional-runtime readiness. r[kiln.public_release.sources] r[kiln.public_release.claims]


## Phase 6: Operator-approved public cutover

- [x] [serial] Finalize current-source license/provenance records, preserve unrelated work, and freeze one exact source-only candidate under the recorded operator authority. r[kiln.public_release.claims]
- [x] [serial] Review the candidate and published-history/forge exposure; exercise credential-isolated source acquisition, full x86_64 checks, native gates and actual CLI behavior with exact archive identities. r[kiln.public_release.sources] r[kiln.public_release.gates] r[kiln.public_release.claims]
- [x] [serial] Fast-forward canonical and GitHub main to the approved source, set the GitHub default to main, make its source projection public, and retain fresh anonymous readback plus a later evidence-only receipt without rewriting historical claims. r[kiln.public_release.claims]

