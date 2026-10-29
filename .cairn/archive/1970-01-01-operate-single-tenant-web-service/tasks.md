## Phase 1: Contracts and implementation

- [x] [serial] Implement typed project enrollment and source-bound durable initiation using existing core and Lattice contracts. r[kiln.web.enrollment] r[kiln.web.lifecycle]
- [x] [parallel] Implement bounded worker source materialization, real logs and artifact ingress. r[kiln.web.worker] r[kiln.web.artifacts]
- [x] [parallel] Implement HTTPS role admission and identity-checked controls. r[kiln.web.authentication] r[kiln.web.controls]
- [x] [parallel] Integrate the live browser experience without changing offline report authority. r[kiln.web.interface]

## Phase 2: Verification and operation

- [x] [serial] Exercise actual enrollment, source-bound execution, logs, real artifact downloads, stale/unsupported controls, cancellation and restart with an isolated native workflow. r[kiln.web.evidence]
- [x] [serial] Run focused and required repository gates, retain scoped receipts, document owner/reproduction/non-claims and remove temporary resources. r[kiln.web.evidence]

## Qualification status

Implementation and repository-check tasks are complete; native qualification remains open.

- `evidence/browser-verification.json` records real browser enrollment, successful
  and failed execution, paged command output, downloaded artifact bytes/BLAKE3,
  exact cancel submission, logout clearing, and desktop/mobile widths. Its later
  API phase failed on an oversized diagnostic request; that request bound was
  corrected in the maintained exercise, not accepted by weakening API admission.
  The corresponding profiles, logs and screenshots remain under `evidence/native/`.
- `evidence/native-api/verification.json` records the corrected API exercise:
  exact-source success/failure, four stdout page reads, empty stderr, native
  diagnostics, real artifact bytes, authentication, replay, restart with three
  original invocations preserved, and stale/unsupported controls. Its strict
  cancellation assertion still fails: the selected Lattice producer reports
  `DomainFailure` rather than `Cancelled`. The UI preserves that observation.
  Scratch state and credentials were removed and tracked process roots exited.
- `evidence/repository-gates.json` retains actual lifecycle receipts, including
  failures. Proposal, design, tasks and repository validation pass. Coverage
  reports nine dangling dashboard/Patchgate IDs under the accepted-spec-only
  policy; release-readiness exits zero but reports `valid: false`. These other
  change packages are not synchronized or accepted by this service work.
- Workspace tests, all-target Clippy, Cargo formatting and Nix formatting pass;
  `evidence/cargo-workspace.log` retains the full test/Clippy output.
- The default remote Nix builder failed before checks could run because its
  configured gRPC builder lock filename exceeded the filesystem limit.
  Nix checks pass on x86_64-linux with remote builders explicitly disabled;
  `evidence/nix-local.log` retains the full Octet/flake output. The incompatible
  aarch64-linux system was not checked. The immutable packaged web-service app
  was also launched successfully with `--help`.

The 2026-10-29 rerun on candidate `1cda616f` (`evidence/native-final/`) closes the
native task: the maintained `scripts/web-service-smoke.py` exercise, using the
corrected Lattice cohort, passed enrollment, exact-source success/failure/partial
failure, worker-captured logs, artifact download (BLAKE3 `aee92f56…`), replay,
authentication, stale/unsupported control rejection, restart without
redispatch and canonical `Cancelled`. All processes, credentials and scratch
were retired. Lifecycle sync/archive was authorized by the operator on 2026-10-29;
no production activation is claimed by this package.
