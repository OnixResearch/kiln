## Phase 1: Contract and admission

- [x] [serial] Record the live unpaced-poll failure and define the ownership and non-claim boundary. r[kiln.aspen_host.lattice_poll_pacing]
- [x] [serial] Add a required positive observation interval to the host composition interface. r[kiln.aspen_host.lattice_poll_pacing]
- [x] [serial] Add positive complete-argument and negative missing, duplicate, zero, and malformed interval tests. r[kiln.aspen_host.lattice_poll_pacing.rejected]

## Phase 2: Paced execution and recovery

- [x] [serial] Pace each non-terminal normal observation without delaying an existing terminal result. r[kiln.aspen_host.lattice_poll_pacing.accepted]
- [x] [serial] Apply the same pacing after `RecoverDispatch` without adding a normal redispatch path. r[kiln.aspen_host.lattice_poll_pacing.rejected]
- [x] [serial] Test delayed terminal completion and bounded uncertainty with positive and negative fixtures. r[kiln.aspen_host.lattice_poll_pacing]

## Phase 3: Evidence and publication

- [x] [serial] Run host tests before and after the change, strict Clippy, Octet deny-all, Cairn validation, and Nix checks. r[kiln.aspen_host.lattice_poll_pacing]
- [x] [serial] Publish an immutable Kiln revision and prove one real provider-backed shadow through Onix. r[kiln.aspen_host.lattice_poll_pacing.accepted]
- [x] [serial] Sync and archive the accepted requirement with exact evidence and non-claims. r[kiln.aspen_host.lattice_poll_pacing]
