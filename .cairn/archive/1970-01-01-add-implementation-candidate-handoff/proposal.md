# Admit bounded implementation candidate handoffs

## Why

A worker exit, claimed success or editable task list is not evidence that intent
was implemented. Issue OnixResearch/kiln#46 needs a bounded handoff between a
candidate-producing worker and independent verification, binding the exact work,
attempt, base, candidate, acceptance obligations, profile and retained evidence.

## What Changes

- Add `CandidateHandoff`, `CandidateEvidence`, acceptance-obligation digest and
  canonical handoff identity in `kiln-core`.
- Admit only exact work/version/attempt/fence/base/repository bindings, in-scope
  unique paths, the work's acceptance digest and verification profile, and
  bounded unique evidence.
- Retain the handoff on the attempt and transition to `candidate-produced`
  idempotently; refuse a different handoff.
- Add `kiln-implementer handoff --handoff-file FILE`.
- Document the envelope/admission non-claims and add tests.

## Impact

- **Files**: `crates/kiln-core/src/implementation/candidate.rs`,
  `attempt.rs`, `kiln-implementer`, implementer CLI, docs and specification.
- **Testing**: exact admission/idempotence; binding, acceptance-digest, path,
  evidence and candidate/tree refusals; CLI handoff.
- **Authority**: envelope admission only. Independent execution, acceptance and
  publication remain external.
