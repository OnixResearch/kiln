## Phase 1: Contract and admission

- [x] [serial] Record the live profile-drift failure and define the exact shell/core ownership boundary. r[kiln.aspen_host.extension_profile_binding]
- [x] [serial] Require a bounded runtime profile path and expected identity in the extension shell. r[kiln.aspen_host.extension_profile_binding]
- [x] [serial] Add positive matching-profile and negative argument, document, and identity tests. r[kiln.aspen_host.extension_profile_binding.rejected]

## Phase 2: Host composition

- [x] [serial] Bind the host-admitted canonical profile path and BLAKE3 identity into native extension argv. r[kiln.aspen_host.extension_profile_binding.accepted]
- [x] [serial] Prove a valid non-default callback timeout reaches extension callback admission. r[kiln.aspen_host.extension_profile_binding.accepted]
- [x] [serial] Preserve durable recovery, pacing, uncertainty, and no-redispatch behavior. r[kiln.aspen_host.extension_profile_binding]

## Phase 3: Evidence and publication

- [x] [serial] Run focused tests before and after the change, workspace tests, strict Clippy, Octet deny-all, Cairn validation, and Nix checks. r[kiln.aspen_host.extension_profile_binding]
- [x] [serial] Publish an immutable Kiln revision and prove one fresh provider-backed Onix shadow. r[kiln.aspen_host.extension_profile_binding.accepted]
- [x] [serial] Sync and archive the accepted requirement with exact evidence and non-claims. r[kiln.aspen_host.extension_profile_binding]
