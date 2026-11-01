# Radicle Canonical Tag Specification

## Purpose

Defines source acquisition from reviewed canonical Radicle tag references.

## ADDED Requirements

### Requirement: Canonical tag source references

r[kiln.radicle.canonical_tag]
A Radicle source request MAY name `refs/tags/` as its signed and canonical
reference. The target object MUST peel to a commit, and that commit MUST equal
`expected_revision`. The canonical evidence MUST retain the target object,
`object_kind = "tag"` for annotated tags, and the peeled commit as `revision`. Signed and canonical references MUST
still match exactly.

#### Scenario: Annotated tag carries canonical evidence
r[kiln.radicle.canonical_tag.scenario.tag]
- GIVEN a delegate-quorum canonical annotated tag whose peeled commit is the
  requested revision
- WHEN the source is acquired
- THEN the snapshot is that commit and canonical evidence records the tag object
  with kind `tag`.

#### Scenario: Tag target is not a commit
r[kiln.radicle.canonical_tag.scenario.revision]
- GIVEN a tag reference whose target cannot peel to a commit
- WHEN the source is acquired
- THEN acquisition fails before snapshot acquisition.
