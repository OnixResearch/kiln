## Phase 1: Layout

- [x] [serial] Split the content, gossip and docs-index modules into directory modules under the file-length and file-count limits, with tests in their own files, keeping requirement markers. a[conform-aspen-host-to-octet.layout]

## Phase 2: Findings

- [x] [serial] Resolve the clock, sentinel, ignored-result, `usize`, renamed-import and boolean-name findings in the library and keep the content and gossip public paths. a[conform-aspen-host-to-octet.findings]
- [x] [serial] Shorten `run` in the binary and bring the two live integration tests under the policy with the existing test header and code changes only. a[conform-aspen-host-to-octet.findings]
- [x] [serial] Confirm that schemas, bounds, the stored index and the test functions are unchanged. a[conform-aspen-host-to-octet.behavior]

## Phase 3: Verification

- [x] [serial] Compare the host's tests before and after, run the Octet check at the parent and at the change, run the aggregate Octet check, the host package check, Clippy and traceability coverage, validate lifecycle gates and retain bounded evidence. a[conform-aspen-host-to-octet.verification]

`evidence/verification.json` holds the checks run on this source tree. The
aggregate Octet check and the host package check need a committed revision;
their results at this change's revision are retained in
`resolve-launch-channel-blockers/evidence/verification.json`. This package is
not synchronized or archived.
