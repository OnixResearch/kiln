## Why

The launch preparation existed as eleven local commits on top of published `main` at `428d03fcfb02478ed79d38453fb44f82c6a2bdd5`. The close-out report on 2026-11-12 listed publication as an owner action. The operator then said "push". Earlier packages in this preparation state that they perform no push to a Kiln remote, so the publication needs its own record.

## What Changes

- Move `main` to the local head `bd5edde80d03fa418219a0832114bbf3768800e2` and push it as a fast-forward, first to canonical Radicle and then to the GitHub projection.
- Read both public routes back anonymously, and acquire the published source through both documented routes without credentials.
- Run the published quick start from an anonymous clone in an environment with an empty home and no Git, SSH or GitHub credentials.
- Update the launch kit to the published state.

No tag, release, announcement, force push, history rewrite, governance, delegate, threshold or visibility change is performed. The workspace repository is not pushed.

## Impact

- **Current consumer:** the operator working the launch checklist, and readers who reach the repository through either public route.
- **Durable capability:** both public routes carry the launch surface, the security policy and the Octet conformance change, and the checklist states the published head with its time.
- **Owner:** the operator owns publication authority and the signing key. Kiln maintainers own the launch kit and this record.
- **Files**: `docs/gtm/README.md`, `docs/gtm/launch-checklist.md`, `docs/gtm/release-notes-0.1.0.md`, `docs/gtm/demo.md`, `docs/gtm/positioning.md` and this package.
- **Testing**: scan the commits for credential and private-detail patterns before the push; read both routes back anonymously; clone both routes without credentials and compare their trees; run the quick start from the anonymous clone; check links in the changed documents; validate the lifecycle gates.
- **Reversal**: published history cannot be assumed removable, and the projection's ruleset refuses a rewind of `main`. A correction is a new commit.
- **No accepted-spec effect:** publication and launch documents are not governed software behavior.
