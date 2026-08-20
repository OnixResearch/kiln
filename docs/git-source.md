# Local Git source adapter

`kiln-adapter-git` reads one local Git repository through the pinned `git2` crate. It does not use an executable from `PATH`.

## Profile

The source of the profile is `config/git-source-profile.ncl`. The profile pins `git2` version `0.20.2` and `libgit2` version `1.9.6`.

The profile permits only the SHA-1 object format and repository format version `0`.

The first profile supports these operations:

- repository discovery;
- exact reference resolution;
- bounded snapshot acquisition; and
- identity observation.

The profile rejects network access, credentials, mutation, signing, collaboration, partial clones, replacement objects, alternate object databases, and Git links.

## Snapshot

The adapter resolves one configured reference to one exact commit. It reads the native tree and builds a deterministic manifest.

The manifest sorts entries by path. It records each path, entry kind, native object ID, and admitted content bytes.

The adapter supports regular files, executable files, and symbolic links. It rejects unsafe paths, duplicate paths, unsupported modes, and exceeded bounds.

The adapter rechecks the configured reference before it returns a snapshot. If the reference moved, the operation fails.

## Limits

The profile defines these named bounds:

- maximum entry count;
- maximum path bytes;
- maximum bytes for one file; and
- maximum bytes for the complete snapshot.

The shell applies these bounds before it copies large blob content. The pure core applies the bounds again during manifest construction.

## Non-claims

A passing snapshot proves only the checked local repository, objects, bytes, profile, bounds, and observations.

It does not prove source trust, signature trust, complete history, remote availability, or forge state.
