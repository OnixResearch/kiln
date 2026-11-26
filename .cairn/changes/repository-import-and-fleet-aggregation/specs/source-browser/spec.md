## ADDED Requirements

### Requirement: Provider-neutral repository catalog core with remote/mirror normalization and fleet copy reconciliation

r[kiln.source.repository_catalog_core]
`kiln-core` MUST provide a pure `no_std + alloc` repository catalog model assigning each repository a deterministic BLAKE3 `ContentId` derived from Kiln-owned canonical identity (never replacing Kiln identity with provider-native identifiers). It MUST validate and normalize `github`, `gitea`, and `radicle` remote endpoints with explicit `canonical` or `mirror` role, allow at most one `canonical` remote per repository, reject embedded credentials, control characters, malformed URLs or Radicle RIDs, and exceedances of named bounds (`MAX_CATALOG_REPOS`, `MAX_REMOTES_PER_REPO`, `MAX_LOCAL_COPIES_PER_REPO`). Reconciling observed `FleetLocalCopy` entries across machines MUST match copies to repositories via normalized remote identities, surface unmatched checkouts as `local_only`, and classify cross-fleet state honestly as `synced`, `diverged`, `dirty`, or `unknown` without ever collapsing offline or unreadable copies into `synced`.

#### Scenario: Multi-forge remotes and fleet local copies reconcile into one canonical repository

r[kiln.source.repository_catalog_core.scenario.reconcile]
- GIVEN a repository with a canonical Radicle RID (`rad:z...`), a GitHub mirror, and a Gitea mirror, plus local copies observed on `britton-desktop` (`jj`, commit A, clean) and `aspen3` (`git`, commit B, dirty)
- WHEN `reconcile_catalog` aggregates the remotes and fleet copies
- THEN both local copies MUST link to the same BLAKE3 canonical `RepositoryRecord`
- AND the cross-fleet sync status MUST report `diverged` and `dirty` rather than `synced`

#### Scenario: Malformed remotes, credentials, and duplicate canonicals are rejected

r[kiln.source.repository_catalog_core.scenario.invalid_remote]
- GIVEN an import specifying embedded credentials (`https://user:secret@gitea.example/org/repo`), an invalid Radicle RID, or a second `canonical` remote on a repository that already has one
- WHEN validated by `kiln-core::repository_catalog`
- THEN the catalog core MUST refuse the operation with a typed diagnostic before any external effect
