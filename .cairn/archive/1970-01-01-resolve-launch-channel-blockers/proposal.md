## Why

The launch preparation recorded four blockers and left them open. On 2026-11-12 the operator reviewed them and directed, in order: use the `onix-` prefix for the colliding packages, enable the disabled security settings, remove the stale second public repository, and fix `kiln --help`. Two of these are remote actions on GitHub that the preparation package explicitly excluded. Later the same day the operator said to finish the job and, when asked, authorized two more settings by name: the projection's description and topics, and secret-scanning push protection. At close-out the operator was shown the two remote changes still within reach of the session and ticked both: a ruleset on the projection's `main` and one row for Kiln in the organization profile.

## What Changes

- Enable private vulnerability reporting and secret scanning on the GitHub projection `OnixResearch/kiln`, with a read-back before and after.
- Set the projection's description and topics and enable secret-scanning push protection, with a read-back before and after.
- Create a ruleset on the projection's `main` that blocks deletion and non-fast-forward pushes, and append one row for Kiln to the project table of the organization profile in `OnixResearch/.github`, each with a read-back before and after.
- Withdraw `OnixResearch/kiln-public` as a public entry point by changing its visibility to private. It is not deleted.
- Record the withdrawal and the branch rule in `docs/hosting-and-recovery.md`.
- Refresh the launch kit: move the resolved blockers out of the open lists, update the publish sequence, replace the test and coverage figures with the results on the current tree, and record the Nix gates that were run and those that were not.
- Request every external link in the public documents. Repair the two README references a reader could not open, and tell contributors how to acquire the runtime workspaces' pinned source when its host is unreachable.
- Correct Kiln's GitHub source in the workspace catalog, which lives in the workspace repository outside this one.

The package rename is the separate package `adopt-onix-package-prefix`. The help fix and the version flag are the archived packages `add-operator-cli-help` and `add-operator-cli-version`. This package records their effect on the kit and nothing else about them.

No push to a Kiln remote and no tag, release, homepage, announcement or payment action is performed. The organization-profile row is one commit to `OnixResearch/.github` through the GitHub contents API.

## Impact

- **Current consumer:** the operator working the launch checklist toward the `0.1.0` go/no-go review.
- **Durable capability:** the security policy's private reporting route works, one public GitHub entry point remains, an ordinary push cannot delete or rewind the projection's `main`, the organization profile links Kiln, and the checklist states the observed channel state with its date.
- **Owner:** the operator owns both remote repositories and every remaining setting. Kiln maintainers own the kit and the withdrawal record.
- **Files**: `README.md`, `CONTRIBUTING.md`, `CHANGELOG.md`, `docs/hosting-and-recovery.md`, `docs/gtm/README.md`, `docs/gtm/launch-checklist.md`, `docs/gtm/launch-plan.md`, `docs/gtm/positioning.md`, `docs/gtm/release-notes-0.1.0.md`, `docs/gtm/demo.md` and this package.
- **Testing**: read each remote setting back through the GitHub API; check anonymous reachability of both repositories; rerun the documented quick-start, demo and repository gates on the current tree; check the changed documents' links and anchors; validate the lifecycle gates.
- **Reversal**: disable a setting through the same API; clear the description and topics with `gh repo edit`; delete the ruleset through the rulesets API; revert the profile commit; change the repository's visibility back to public. Changing visibility cannot retract copies already fetched.
- **No accepted-spec effect:** remote repository settings and launch documents are not governed software behavior.
