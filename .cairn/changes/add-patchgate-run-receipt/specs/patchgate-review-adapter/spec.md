# Patchgate Review Adapter Specification

## ADDED Requirements

### Requirement: Atomic retained receipt publication

r[kiln.patchgate.receipt.publication] The operator shell MUST provide one
reviewed `run` command that admits the exact intent, performs the isolated
review and publishes exactly one `kiln.patchgate.review.v1` receipt at the
declared output path without a shell redirect. Publication MUST stage in the
destination directory, synchronize the staged bytes, rename into place and
synchronize the directory. A symlink or non-regular destination MUST be
rejected before staging. Command output MUST be a bounded summary and MUST NOT
copy the private native report.

#### Scenario: Receipt publication succeeds

- GIVEN one admitted profile and intent and one exact isolated review result
- WHEN `run` completes
- THEN exactly one receipt MUST be readable at the declared output path and
  MUST preserve the request identity, normalized decisions and verbatim native
  report.

#### Scenario: Destination is not a regular file

- GIVEN a symlink or non-regular output destination
- WHEN `run` attempts publication
- THEN no staged receipt MAY replace that destination and the command MUST fail
  explicitly.

#### Scenario: Command output is inspected

- GIVEN a successful `run`
- WHEN its standard output is retained as worker logs
- THEN it MUST contain only the bounded summary and MUST NOT contain the
  private native report body.
