## Why

Ownership rules currently express one requirement: an eligible non-author owner
must approve the exact revision. That conflates intent approval with the
evidence an owner cares about, and leaves monorepo components unable to require
their own project CI.

Issue: [#24](https://github.com/OnixResearch/kiln/issues/24).

## What Changes

- Add optional `required_evidence` to an ownership rule. The first supported
  kind is `project_ci { project }`.
- Admit only bounded, unique, non-empty project identities and the closed
  evidence-kind set.
- Add one `evidence:<rule>:project_ci:<project>` checklist item per matching
  requirement, satisfied only by a full clean run for the exact submitted
  commit, project and current scope identity.
- Keep the existing owner approval item and all merge authority unchanged.
- Add positive and negative core tests covering exact revision, project, scope,
  cleanliness, full-run and policy admission.

`evidence_only` approval classes remain future work; this change only adds
evidence in addition to owner approval.
