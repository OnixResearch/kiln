# Design: continuously refreshed read-only CI reports

## Core and shell

Pure functions admit closed configuration and deterministically plan history ordering/catalog entries. The shell observes a bounded immediate directory of exact candidate identities, reads bounded no-follow canonical journal files, writes an atomic catalog, and invokes the existing publisher with finite process deadlines. It does not implement another CI state machine, reconcile a provider, dispatch, sign or publish statuses.

## Collection contract

`scripts/dashboard-refresh.py --config PATH` executes one finite cycle and exits nonzero visibly on admission/generation/delivery failure. Configuration fields are `schema_version`, `candidate_root`, `catalog_output`, `publication_config`, `project_id`, `project_label`, `max_candidates`, `max_histories`, and `max_journal_bytes`. Version is one. Paths are explicit absolute operator paths. The catalog path must equal the existing publication configuration's catalog. Catalog output must not collide with configurations, profiles, executables or canonical input state. Candidate enumeration is nonrecursive and bounded; only non-symlink 64-lowercase-hex candidate directories are eligible. Missing journals are not completed runs. Present malformed or oversized journals fail rather than becoming fabricated observations.

A candidate's `source` supplies declared provider/repository/revision labels; its original `records` remain the canonical dashboard input. Ordering uses observed journal modification time with candidate identity as a deterministic tie-breaker and explicitly claims only recent observed updates, not global newest-source completeness. Histories retain the candidate identity. The renderer independently admits/replays each original journal. No outcomes are inferred from a filename or return code. A single finite cycle is serialized with an owned lock; missed timer ticks do not queue work. Output/history/candidate bounds are named and admitted. Failed collection or delivery preserves the last-good report.

## Supervision and deployment

Package the collector and existing publisher with their matching scripts and Python runtime. A producer-local systemd user oneshot/timer runs the finite collector on a declared cadence and transfers only admitted self-contained HTML to Aspen3 through the existing SSH publisher. This keeps canonical journals and CI credentials on their owning host. The timer owns no CI execution or forge credentials; it uses the operator's enrolled SSH route. Service failure remains visible; the next scheduled observation cycle is not a provider execution retry. Stop/restart operates only the newly named dashboard units.

## Fresh execution

Use the maintained dedicated Radicle fork self-CI runner with exact source and measured immutable tool paths. Its real broker/controller/worker write candidate-local journals; genuine Nix compile failure remains DomainFailure. The observer watches that declared state root while CI executes. Retain exact revision, canonical outcome and published HTML identity. Existing production repository/governance and the retired GitHub credential stack are untouched. If a required tool/host is unavailable, finish reachable implementation and report the exact unavailable prerequisite without promoting retained runs into a fresh proof.

The operated run exposed multi-gigabyte copies of the operator's evaluator cache accumulating between sequential cases. After each case's canonical/status/recovery checks, retire only that case's disposable `eval-home/.cache/nix`; preserve canonical journals, source identities, logs and the operator's original cache. This bounds retained cache copies to the currently executing case without changing CI results or provider authority.

## Verification

Keep positive/negative admission and last-good preservation coverage together. Exercise the actual collector, renderer, publisher and supervised timer, then fresh success/failure and publisher restart recovery on the real Aspen3 destination. Record scoped source/runtime/profile identities and failures; test and run required repository gates once after integration.
