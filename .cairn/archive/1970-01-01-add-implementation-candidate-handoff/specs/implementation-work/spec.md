# Implementation Work Specification

## ADDED Requirements

### Requirement: Bounded candidate handoff

r[kiln.implementation.candidate_handoff] A candidate handoff MUST bind the exact
work identity and revision, attempt identity and fence, repository, frozen base,
candidate commit and tree, complete changed paths, the canonical digest of the
work's acceptance obligations, the reviewed verification profile,
toolchain/environment identities, and retained evidence references. Every
changed path MUST be relative, unique and inside an admitted work path; candidate
and tree MUST differ; evidence identities MUST be unique and bounded. Admission
MUST refuse any mismatched work/version/fence/base/repository, transferred
acceptance digest, substituted candidate or tree, incomplete/duplicated paths,
or empty/oversized evidence. Admission MUST NOT execute a check, accept a
candidate, publish, or treat an evidence outcome as authoritative.

#### Scenario: Exact handoff is admitted and retained

r[kiln.implementation.candidate_handoff.scenario.admit]
- GIVEN an executing attempt with admitted work
- WHEN an exact complete handoff is admitted
- THEN the attempt MUST retain the handoff and become `candidate-produced`
- AND repeating the same handoff MUST be idempotent

#### Scenario: Mismatched or broader handoff is refused

r[kiln.implementation.candidate_handoff.scenario.refused]
- GIVEN a handoff with a different work/version/fence/base/repository, another acceptance digest, an out-of-scope or duplicated path, duplicate evidence, or equal candidate/tree
- WHEN admission runs
- THEN it MUST fail without mutating the attempt
