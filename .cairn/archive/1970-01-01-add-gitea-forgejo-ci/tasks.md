## Phase 1: Pure admission and identity

- [x] [serial] Add the Gitea/Forgejo profile, event, status-plan and identity admission with positive and negative unit tests. r[kiln.gitea.profile] r[kiln.gitea.authentication] r[kiln.gitea.events]
- [x] [serial] Add durable delivery/candidate replay with exact profile binding and conflict rejection. r[kiln.gitea.replay]

## Phase 2: Shells

- [x] [serial] Add the bounded HTTP shell, Lattice workflow lifecycle, and Gitea status publication/reconciliation. r[kiln.gitea.shell] r[kiln.gitea.workflow] r[kiln.gitea.status]
- [x] [serial] Add the exact-source Git job helper and reviewed argv execution. r[kiln.gitea.source]

## Phase 3: Packaging and verification

- [x] [serial] Add typed Nickel profiles, workspace/flake packages and apps, and operator documentation. r[kiln.gitea.profile] r[kiln.gitea.nonclaims]
- [x] [serial] Run focused adapter tests, Nickel typechecks, workspace checks, Clippy, rustfmt and Cairn validation, then record evidence. r[kiln.gitea.local_e2e]
