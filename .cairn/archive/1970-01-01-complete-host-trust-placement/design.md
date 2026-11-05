## Context

Existing facts omit SMT control, vulnerabilities, cmdline and CAT/MBA capabilities. Existing receipts list planned jobs, not actual overlap. Existing kernel override also bypasses vulnerable-kernel refusal.

## Decisions

### Functional decisions and bounded observations

Keep release-table classification and admission in kiln-microvm. Empty reviewed vendor backport table means no vendor package is fabricated as patched. Retain package provenance independently from upstream release classification. Shell reads kernel package provenance, sysfs/proc facts and bounded ss listener output; unavailable facts remain explicit unknown. Only SMT/KSM refusal may be overridden; kernel refusal is unconditional for untrusted-capable runners.

### Placement and evidence

Reserve local-executor placements in the existing admission transaction. Maintain a distinct running-job ledger with live PID/start-time verification and symmetric immutable overlap records so completed peers still appear in both receipts but exited peers awaiting finish cannot create false observations. Unknown execution attempts retain safety reservations. Local hosted execution uses a confidential internal OS-instance bucket rather than configured aliases; this bucket is not emitted to customers or treated as hardware attestation. External unmanaged processes remain outside the ledger.

### Measurement

An opt-in sender and receiver use serial output and fixed wall-clock windows; random sender bits are retained directly, receiver measurements alone decode using training samples. Cache eviction, compute contention and streaming memory workloads run as guest processes. The operator supplies actual guest launchers for same-host, pinned-core and separate explicit pools. Workers publish only report-keyed OS-instance pseudonyms, not raw machine IDs; different instances never prove physical separation. Within one observed OS instance, pool separation requires explicitly pinned disjoint LLC domains and is marked measurement-only. Independent cross-physical-host proof is an external operator/hardware obligation. Retain raw samples and rates, never synthesize unavailable placements. Timeouts derive from admitted full schedules, and worker/VMM process groups are owned through timeout/cancellation. Measurements do not override production whole-host exclusion or prove channel absence.

## Risks

CVE identifiers and thresholds are issue-supplied policy inputs, not independently validated vulnerability claims. SMT disable, CPU affinity and CAT do not partition DRAM or establish hostile confidentiality. Cross-physical-host pool evidence requires a second independently provisioned host and synchronized clocks; distinct-LLC pools on one host remain an explicitly narrower measurement boundary.
