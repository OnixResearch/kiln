## Why

Retained Patchgate receipts are evidence, not a human review workflow. Operators need one stable change identity across exact revisions, discussions, approvals and provider submission, without granting merge authority to CI results.

## What Changes

- Add change-centric review pages with patch sets, exact revision comparisons, unified/side-by-side diffs, file markers and line permalinks.
- Add revision-anchored threads, private drafts and atomic publication, reviewer/CC requests, attention and durable notifications.
- Add revision-bound approvals, explicit vetoes, explainable submit requirements, dependency stacks, search and private saved views.
- Add explicitly configured provider operations for exact-target submission, revision upload, abandon/restore, rebase, revert and cherry-pick; reconcile uncertain operations without replaying external effects.

## Impact

- **Files**: provider-neutral review model, versioned provider ports, configured local provider implementation, web-service persistence/API, console assets, typed Nickel configuration and operator documentation.
- **Testing**: positive/negative transition tests, provider conformance, full workflow/restart smoke against temporary repositories, authenticated HTTPS/browser desktop/mobile exercise, workspace checks and lifecycle gates.
- **Owner**: Kiln owns CI/review meaning and the application shell; provider adapters own exact external mutations. Existing source providers stay read-only.
- **Non-claims**: no remote publication, signing, Radicle initialization, merge of a real repository or deployed policy change is authorized by this implementation request.
