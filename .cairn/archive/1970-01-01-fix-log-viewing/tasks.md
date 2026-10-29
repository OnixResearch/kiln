## Phase 1: Admission

- [x] [serial] Validate proposal, design and task gates. r[kiln.dashboard.logs.continuation]

## Phase 2: Repair

- [x] [serial] Repair continued reads, rewind, failures, search/diagnostic selection and follow scrolling without changing backend authority. r[kiln.dashboard.logs.continuation]

## Phase 3: Verification

- [x] [serial] Exercise real retained logs and negative transitions, run focused checks and update operator guidance. r[kiln.dashboard.logs.continuation]

## Verification observations

- Before repair, retained Nickel Eval stdout showed 6,829 visible characters with next byte cursor 6,832; another read erased it. Search changes left the viewer blank. Selecting diagnostics with the existing search produced `diagnostic_search_unavailable`.
- Browser-only candidate HTML/CSS/JS overrides used the real authenticated GET APIs. Continued retained Kiln stdout through byte cursor 58,036 and compared every displayed chunk to concatenated admitted response text; repeated empty EOF reads preserved the complete view without duplication. Read from start restored the initial chunk.
- A blocked continuation preserved both admitted text and its cursor. An explicit seek to byte 1,000 replaced exactly the returned range; a rejected seek to the maximum u64 retained the prior view. A 700ms-delayed stdout response did not overwrite a newer stderr selection.
- Committing and clearing the literal search loaded results from the beginning. Diagnostics omitted search and loaded native cursor 0 → 10; switching back restored the stdout search. Search skips were labelled as intentional selection, not missing source bytes.
- Reproduced idle-follow text-selection loss before the final rendering fix. Afterward, a selected range and scroll position zero survived empty follow polls. Refreshing run detail during a delayed log read kept read/rewind buttons disabled.
- The 390px mobile log form was visually inspected without horizontal overflow. Session clearing removed all displayed log text. No browser JavaScript page errors occurred in the candidate verification. The browser tab was closed.
- Checks passed: JavaScript syntax; `nix develop path:. -c cargo test --locked -p kiln-web-service`; `nix develop path:. -c cargo clippy --locked -p kiln-web-service --all-targets -- -D warnings`; proposal/design/tasks admission gates. Kache disabled caching after a host cache I/O warning; checks completed successfully.
- `docs/operator.md` records continued reads, rewind, failures, native diagnostic search limits, follow scrolling and bounded retention. Kiln application/dashboard maintainers own this path. No backend schema/state/authority changes, execution mutations, publication or deployment were performed. Candidate-browser verification is not a deployed-byte claim or exhaustive accessibility/backend qualification.
