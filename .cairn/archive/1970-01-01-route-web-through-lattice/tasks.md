## Phase 1: Source cutover

- [x] [parallel] Restore Web's sole native Lattice provider and expose authenticated persisted execution bindings and reusable bounded worker HTTPS/log operations. r[kiln.web_hosted.binding]
- [x] [parallel] Replace direct admission with per-node Lattice backend policies, independent configured Web authority lookup and exact source/replay enforcement. r[kiln.web_hosted.admission] r[kiln.web_hosted.replay]
- [x] [parallel] Implement the fixed-profile native Lattice hosted-job helper with real outcome/log/cancellation handling; remove direct composition binaries/config/apps. r[kiln.web_hosted.adapter]

## Phase 2: Actual topology proof

- [x] [serial] Exercise genuine native Lattice dispatch, trusted execution/stdout, live cancellation, negative approvals/source/issuer and original-attempt restart recovery; retain exact scoped evidence and owned teardown. r[kiln.web_hosted.qualification]
- [x] [serial] Update existing source/operator/readiness documentation, explicitly supersede direct-route evidence, and pass applicable compiled, typed configuration and native lifecycle gates. r[kiln.web_hosted.qualification]
