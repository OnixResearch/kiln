## Why

Path ownership requires approval from the code's own owners, but an interface
change can also affect projects that consume it. Kiln already computes the
affected project set and exact scope identities; review can use that to make
dependent CI and dependent-owner acknowledgement explicit.

Issue: [#25](https://github.com/OnixResearch/kiln/issues/25).

## What Changes

- Add optional `project` and `interface` fields to an ownership rule.
- Require interface patterns to be an exact subset of the rule's paths and to
  name a project.
- When an interface pattern matches a changed path, add checklist items for:
  - full clean CI for every other affected project in the current required set;
  - one eligible non-author owner approval from each ownership rule linked by
    `project` to those affected projects.
- Extend automatic reviewers with the dependent rules' owners.
- Add positive and negative core tests, the Nickel contract and operator docs.

This first cut uses the existing affected-project facts and treats every other
affected project as a dependent. Explicit dependency-edge evidence and full glob
subset reasoning remain future work.
