## Why

Three accepted GitHub requirements no longer describe Kiln:

- `kiln.github.landing.merge` still requires a merge-commit request and its read-back, but `GithubLandingClient::land` now refuses before any request (`github_landing_unqualified_atomicity`). Only recovery of an externally landed exact merge remains, and the stale scenario `kiln.github.landing.merge.scenario.landed` has no test because the behaviour is gone.
- `kiln.github.source_status` still requires a PATCH close and a closed read-back, but the client sends no mutation and refuses an open issue for operator reconciliation. The stale scenario `kiln.github.source_status.scenario.close` has no test for the same reason.
- `kiln.github.publication.scenario.upload` is verified by `uploads_and_reads_back_the_exact_candidate_objects_before_pr_transport` but was never defined.

These three entries are the GitHub part of the active-development traceability failure that several changes record, including the held final task of the Cairn workspace change for #84. Accepted specifications that claim Kiln merges pull requests or closes issues are misleading.

## What Changes

- Modify the landing requirement to state that unattended landing is refused and that only an exact external merge is recovered, replacing the stale landed scenario with a recovered scenario verified by the existing external-merge recovery test.
- Modify the source-status requirement to state observe-only closure, replacing the stale close scenario with an open-source refusal scenario verified by the existing test that refuses an open issue after one read.
- Modify the publication requirement to define the upload scenario its test already verifies, and state the open and closed reconciliation the client performs.

## Impact

Consumers: reviewers and traceability gates. Owners: GitHub adapter maintainers. Files: three delta specifications and two added verification markers on existing tests. No behaviour changes.

## Non-goals

This change does not reintroduce merging or closing, qualify GitHub landing (#48) or change publication behaviour. The dangling `kiln.dashboard.campaign.projection` marker belongs to the selected-campaign change, whose delta needs its own rewrite before it can sync; it is not touched here.
