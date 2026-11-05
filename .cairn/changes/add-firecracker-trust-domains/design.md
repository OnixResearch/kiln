## Context

The hosted execution config maps labels to backends and workload policies to
runner labels. There was no declared placement identity, so two labels could
point at the same physical host without detection.

## Decisions

### Reviewed domain labels

`trust_domain` is an operator-reviewed placement-pool label. Untrusted-capable
runners require one. A workload policy runner's label is its default domain only
when no explicit domain exists; all domain comparisons use the resolved label.
A domain may not contain both untrusted-capable and workload-bearing runners.

### Receipt

The Firecracker `vm` object records the domain and host identity next to the
observed host facts. This makes the static placement decision inspectable. It
does not prove the operator configured physically separate hosts.

### Host identity

`host_identity` is a reviewed physical-host label. Untrusted-capable runners
require one. A host may not contain both untrusted-capable and workload-bearing
runners, even when their trust domains differ. Missing workload-runner identity
defaults to the runner label, which is unique.

### Attempt co-residency evidence

Each Firecracker receipt records the other jobs in the same attempt whose runner
declares the same host identity. Jobs execute sequentially, so this is a
declared-placement set, not live concurrent observation. Cross-attempt tracking
still needs a durable placement ledger.

## Risks

- Two domains can still map to the same physical host or LLC unless the operator
  enforces it outside Kiln.
- Co-residency sets across attempts, core pinning, CAT/MBA and covert-channel
  measurements remain open.
