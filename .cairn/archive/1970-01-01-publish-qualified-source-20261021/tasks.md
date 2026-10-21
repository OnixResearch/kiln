# Tasks

## Phase 1: Authority and candidate freeze

- [x] [serial] Freeze `c955334f`, record canonical `main` `305f85d8`, governance, signed refs and rollback, and record the operator's source-only authority. r[kiln.hosting.source_only_publication]
- [x] [serial] Admit proposal, design and tasks before any remote mutation. r[kiln.hosting.source_only_publication]

## Phase 2: Candidate evidence

- [x] [serial] Export the deterministic source archive, sorted paths, hosting profile and license map; bind the self-CI verification, release readiness and gate results to the candidate. r[kiln.hosting.source_only_publication]
- [x] [serial] Record the candidate review, exact authority and commands, and prove the archive contains only the tracked source paths. r[kiln.hosting.source_only_publication]

## Phase 3: Publication

- [x] [serial] Fast-forward canonical Radicle `main` from `305f85d8` to `c955334f`; no force, mirror, governance or history change. r[kiln.hosting.source_only_publication]
- [x] [serial] Reproduce the revision, paths and archive from native Radicle and anonymous Garden HTTPS. r[kiln.hosting.source_only_publication]
- [x] [serial] Record publication receipt, readbacks, governance, replica observations, rollback and non-claims; update README/hosting facts and archive the change. r[kiln.hosting.source_only_publication]

## Phase 4: Verification

- [x] [serial] Run final gates and Cairn validation; commit locally without mirror publication. r[kiln.hosting.source_only_publication]
