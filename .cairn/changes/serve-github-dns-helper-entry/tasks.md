## Implementation

- [x] [serial] Add `run_internal_resolver_if_requested`, make `run_dns_child` private with named failures, and refuse resolver construction in an unserved process. r[kiln.github.dns_helper_entry] r[kiln.github.dns_helper_entry.scenario.unserved]
- [x] [serial] [depends:kiln.github.dns_helper_entry] Call the entry first in `main` of `kiln-adapter-github`, `kiln-implementer` and `kiln-hosted`. r[kiln.github.dns_helper_entry.scenario.served]

## Qualification

- [x] [serial] Run each binary with the reserved entry, malformed entries and a non-parent PID; test the unserved refusal. r[kiln.github.dns_helper_entry.scenario.served] r[kiln.github.dns_helper_entry.scenario.unserved]
- [x] [serial] Document the entry, update the changelog, and record rustfmt, Clippy, focused and crate tests and `cairn validate` in `evidence/gates.txt`. r[kiln.github.dns_helper_entry]
