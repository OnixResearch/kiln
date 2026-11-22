# GitHub CI specification delta

## ADDED Requirements

### Requirement: DNS helper entry in every client host

r[kiln.github.dns_helper_entry] Every Kiln binary that can construct a GitHub HTTPS client MUST serve the reserved DNS helper entry, `--internal-github-status-dns <ppid>`, before its own argument parsing, through the single library entry `run_internal_resolver_if_requested`. The helper MUST print only bounded `ip:443` addresses on success and MUST name its failure on standard error. A process that has not dispatched the entry MUST refuse to construct the resolver, so an executable that cannot serve the helper is never re-run as one.

#### Scenario: Each client host serves the entry

r[kiln.github.dns_helper_entry.scenario.served]
- GIVEN `kiln-adapter-github`, `kiln-implementer` or `kiln-hosted`
- WHEN it is run with the reserved entry and its parent's PID
- THEN it MUST print bounded GitHub addresses or exit with the helper's own resolution failure, never an argument-parsing error
- AND a malformed entry or a PID that is not its parent MUST be refused before any lookup

#### Scenario: An unserved executable is never re-run

r[kiln.github.dns_helper_entry.scenario.unserved]
- GIVEN a process that never dispatched the entry
- WHEN it constructs a GitHub HTTPS client
- THEN construction MUST fail with `github_status_dns_unserved` before any subprocess starts
