## Why

The launch preparation recorded four blockers and left them open. On 2026-11-12 the operator reviewed them and directed, in order: use the `onix-` prefix for the colliding packages, enable the disabled security settings, remove the stale second public repository, and fix `kiln --help`. Two of these are remote actions on GitHub that the preparation package explicitly excluded.

## What Changes

- Enable private vulnerability reporting and secret scanning on the GitHub projection `OnixResearch/kiln`, with a read-back before and after.
- Withdraw `OnixResearch/kiln-public` as a public entry point by changing its visibility to private. It is not deleted.
- Record the withdrawal in `docs/hosting-and-recovery.md`.
- Refresh the launch kit: move the four resolved blockers out of the open lists, update the publish sequence, and replace the test and coverage figures with the results on the current tree.

The package rename is the separate package `adopt-onix-package-prefix`. The help fix is the archived package `add-operator-cli-help`. This package records their effect on the kit and nothing else about them.

No push, tag, release, description, topic, branch-protection, push-protection, organization-profile, announcement or payment action is performed.

## Impact

- **Current consumer:** the operator working the launch checklist toward the `0.1.0` go/no-go review.
- **Durable capability:** the security policy's private reporting route works, one public GitHub entry point remains, and the checklist states the observed channel state with its date.
- **Owner:** the operator owns both remote repositories and every remaining setting. Kiln maintainers own the kit and the withdrawal record.
- **Files**: `docs/hosting-and-recovery.md`, `docs/gtm/README.md`, `docs/gtm/launch-checklist.md`, `docs/gtm/launch-plan.md`, `docs/gtm/positioning.md`, `docs/gtm/release-notes-0.1.0.md`, `docs/gtm/demo.md` and this package.
- **Testing**: read each remote setting back through the GitHub API; check anonymous reachability of both repositories; rerun the documented quick-start, demo and repository gates on the current tree; check the changed documents' links and anchors; validate the lifecycle gates.
- **Reversal**: disable either setting through the same API; change the repository's visibility back to public. Changing visibility cannot retract copies already fetched.
- **No accepted-spec effect:** remote repository settings and launch documents are not governed software behavior.
