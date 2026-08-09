# Design: Git source adapter

## Context

Kiln needs exact immutable source snapshots. Its Jujutsu adapter reads Git-backed Jujutsu repositories, but plain Git repositories need a direct provider.

Git object IDs remain native identities. Kiln computes a separate BLAKE3 identity for its canonical materialized tree.

## Goal and Success Contract

The goal is a read-only local Git `SourcePort` for one pinned implementation cohort.

Completion requires exact resolution, one consistent read view, bounded snapshot admission, canonical tree identity, and real repository fixtures.

False completion includes shelling out through ambient `PATH`, accepting a moved ref, equating Git history with tree bytes, or adding network and forge authority.

## Decisions

### Decision: Start with local read-only repositories

The first profile supports repository observation, reference discovery, exact commit resolution, tree acquisition, and identity observation.

It excludes clone, fetch, push, merge, branch mutation, signing, credential use, and collaboration APIs.

### Decision: Pin the implementation cohort

The change selects an exact Git library and records source revision, crate versions, licenses, object formats, repository features, and supported platforms.

Unsupported repository extensions or formats fail before object traversal.

### Decision: Hold one consistent repository view

Resolution and acquisition use one stable object database and ref observation. The shell rechecks selected ref facts before returning the result.

A moved ref or replaced object database makes the operation fail. The adapter does not combine facts from two views.

### Decision: Build canonical snapshots in a pure core

The shell supplies bounded typed tree entries and content bytes. Pure functions validate paths, entry kinds, ordering, duplicates, modes, submodule policy, and total bounds.

The shell computes BLAKE3 over the canonical manifest. Git OIDs and algorithms remain separate native facts.

### Decision: Preserve special Git facts

Submodules, symbolic refs, annotated tags, signatures, sparse data, alternates, replacement objects, and partial repositories need explicit profile policy.

Unsupported or incomplete facts fail closed. Signature presence does not imply trust.

### Decision: Keep source and forge separate

The Git adapter supplies source facts only. GitHub and Radicle collaboration use separate provider profiles and authority.

## Configuration

Typed Nickel profiles define library cohort, object formats, repository features, reference policy, entry kinds, submodule behavior, and named object, path, file, and total bounds.

## Validation

Positive fixtures cover detached commits, stable refs, tags, executable files, symlinks, empty trees, and repeatable acquisition.

Negative fixtures cover moved refs, missing objects, damaged packs, unsafe paths, duplicate entries, unsupported extensions, oversized data, replacement objects, and partial clones.

## Claim Boundary

Passing checks establishes local object and snapshot observations. It does not prove source trust, signature trust, complete history, remote availability, or forge state.
