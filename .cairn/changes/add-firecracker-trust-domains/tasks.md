## Phase 1: Placement admission

- [x] [serial] Add trust-domain fields and require a domain for untrusted runners. r[kiln.hosted.microvm.trust_domain]
- [x] [serial] Reject domains shared by untrusted and workload-bearing runners. r[kiln.hosted.microvm.trust_domain]
- [x] [serial] Add host identity, require it for untrusted runners and reject mixed hosts. r[kiln.hosted.microvm.trust_domain]
- [x] [serial] Record the attempt-local co-residency set in the receipt. r[kiln.hosted.microvm.trust_domain]

## Phase 2: Receipt and verification

- [x] [serial] Record the domain in the Firecracker receipt and expose typed config fields. r[kiln.hosted.microvm.trust_domain]
- [x] [serial] Add shared/separated/missing-domain tests and run hosted/config/lifecycle checks. r[kiln.hosted.microvm.trust_domain]
