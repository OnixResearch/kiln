## Why

The live Kiln workspace admits only kiln-self-ci. Adding reviewed OnixResearch projects currently changes the whole ServiceProfile identity and is rejected by the retained store. The operator requested additional real projects without losing existing history.

## What Changes

- Add an explicit offline append-only project-profile extension command, requiring the exact previous profile and an unchanged existing project prefix, credentials, roots, provider bindings and resource policy. Only the named max_projects bound may increase.
- Preserve canonical stored state bytes and all existing run, attempt, source, workflow and evidence identities while atomically replacing the exact previous profile binding. Ordinary startup remains exact-bound and never performs implicit migration.
- Review repository-owned CI entry points, install typed profiles and exact native Lattice routes for compatible OnixResearch projects, and enroll/run them through authenticated intake.

## Impact

Current consumer: the operated local Kiln workspace. Kiln application maintainers own admission and the operator owns repository selection, reviewed commands, installation and activation. The durable contribution is repeatable safe project addition with retained history. No sibling source changes, source publication, automatic triggers, new credentials, remote enrollment, lifecycle acceptance or release eligibility are authorized.
