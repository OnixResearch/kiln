## Phase 1: Admission pass

- [x] [serial] Compose bounded polling normalization with durable ledger admission and summary counts. r[kiln.github.poll.admission]
- [x] [serial] Keep closed observations non-terminal and fail on ledger errors. r[kiln.github.poll.admission.scenario.closed]
- [x] [serial] Preserve repeated-pass idempotence through the work ledger. r[kiln.github.poll.admission.scenario.repeat]

## Phase 2: Operator shell

- [x] [serial] Add the one-shot `poll` command with token, bounds and timeout. r[kiln.github.poll.admission]
