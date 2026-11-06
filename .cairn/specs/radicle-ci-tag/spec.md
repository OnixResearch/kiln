# Radicle Ci Tag Specification

## Purpose

Defines the `radicle-ci-tag` capability.

## Requirements

### Requirement: Radicle CI tag trigger

r[kiln.radicle_ci.tag]
The Radicle CI project profile MUST admit a `refs/tags/` source reference and
project it as `TriggerEventKind::Tag`. Branch sources MUST remain `Push`. The
trigger MUST carry the exact source revision, the tag name as alias, and the
configured repository.

#### Scenario: Tag source projection
r[kiln.radicle_ci.tag.scenario.projection]
- GIVEN a project profile with a `refs/tags/` source
- WHEN a source with that exact revision is bound
- THEN the trigger event is `Tag` and its alias is the tag name.

#### Scenario: Push source remains unchanged
r[kiln.radicle_ci.tag.scenario.push]
- GIVEN a project profile with a `refs/heads/` source
- WHEN a source with that exact revision is bound
- THEN the trigger event remains `Push` and its alias is the branch name.
