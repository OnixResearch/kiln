## Context

The service already admits separate runtime policies for access, automation and COBs. `kiln-adapter-git` and `kiln-adapter-pijul` exist as source adapters, but the dashboard exposes no repository content. Linking `kiln-adapter-pijul` (GPL-2.0-only, with GPL `libpijul`) into the web service would change that binary's license obligations, so Pijul reading goes through a separately executed program instead.

## Decisions

### Decision: One typed policy, three explicit kinds

Add `config/web-sources.ncl` exporting `kiln.web-sources.v1` with `sources: [{id,label,kind,path,channel?,executable?}]`. Admission rejects an unknown kind, a relative path or executable, duplicate ids/paths, a channel or executable on a Git/Jujutsu entry, and a Pijul entry without a channel. Paths and executables are server-owned and never appear in browser requests or responses.

### Decision: Exact Git objects, honest Pijul scope

Git and Jujutsu read the repository's Git object store with libgit2: the worktree `.git` when present, otherwise the `.jj/repo/store/git` backend for a Jujutsu store. Revisions are exact 40-character commit ids or the current head; a symbolic revision is rejected rather than guessed. Trees and blobs are read from that exact commit. Pijul runs `<executable> log|list --repository <path> --no-prompt` under Bounded Exec with fixed argv, a cleared environment, an explicit timeout and output caps. Because the Pijul CLI exposes no historical blob read, Pijul tree reads cover the current channel state and file reads cover the tracked working copy, both labelled with their scope; a Pijul revision selector is rejected.

### Decision: Bounded reads without new authority

Every read is bodyless `GET`, denied to worker credentials, and bounded: 64 sources per policy, 100 log entries per page, 512 tree entries, 262,144 file bytes, 1 MiB Pijul output, 15 s Pijul deadline and 4 MiB response. No read fetches, checks out, writes a ref, signs or publishes. Absolute host paths, the executable and Git object-store locations stay out of responses.

## Risks / Trade-offs

- A Jujutsu store without a Git-visible head is reported unavailable rather than reconstructed from Jujutsu metadata.
- Pijul history is listed but not traversed; presenting a working-copy file as a revision would be dishonest, so the scope is labelled and historical selection refused.
- Reading a shared worktree observes concurrent file changes; the working-copy scope is stated and file reads reject symlinks and untracked paths.
