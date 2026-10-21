# Design: source-only publication of `c955334f`

## Candidate

The candidate is the exact committed source `c955334f54bdfe6e155e56c00573ca7ba77c4fb1`, whose parent chain reaches the canonical `main` `305f85d8...`. The source archive is built from the lexically sorted tracked path list with the deterministic USTAR recipe: `28fec0fa...`, 4,439 members, 60,989,440 bytes. The sorted path manifest is `source-paths.json` (`8001e6b5...`).

The candidate source includes the archived self-CI change and evidence; publication evidence lives in this later evidence change, not inside the source archive it identifies. The working copy's evidence and documentation are outside the published source revision.

## Sequence

1. Freeze the candidate, previous canonical `main`, governance, signed refs and rollback (`305f85d8...`).
2. Record the operator's source-only authority and the exact candidate bindings.
3. Run local gates and bind the self-CI verification, release readiness, Octave/Octet and flake results to the candidate.
4. Fast-forward canonical Radicle `main` with one `jj git push --remote rad --bookmark main`; no force and no mirror push.
5. Acquire the published revision through native Radicle and anonymous Garden HTTPS, regenerate the archive from each clone, and compare revision, paths and archive identity.
6. Record the receipt, readbacks, governance, replica observations, rollback and non-claims; update README/hosting facts; archive this change.

## Boundaries

Exact source identity and readback agreement prove only the observed RID, revision, routes, archive and governance facts. They do not prove source correctness, build correctness, reproducibility, deployment, binary release or future availability. The GitHub projection is observed but not mutated.
