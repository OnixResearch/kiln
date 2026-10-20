# Design: Pijul Source-to-CI E2E

## Ownership

Kiln owns Pijul state admission, canonical snapshot identity, workflow
observation admission, and terminal CI meaning. The test owns only the local
repository fixture, the temporary checkout, and the reviewed command. Pijul
remains the native source provider; it does not become a trigger, workflow
runtime, or status authority.

## Flow

```text
pinned Pijul producer + libpijul
  -> real local repository and recorded change
  -> LocalPijulAdapter::resolve_and_acquire
  -> bounded PijulSnapshotEntry set
  -> test-owned materialization of admitted entries
  -> Bounded Exec reviewed /bin/sh command
  -> WorkflowObservation admission
  -> StartRun + CompleteAttempt
  -> Success or DomainFailure
```

The adapter mapping is also called to produce `SourceResolution`, proving that
the exact provider state and repository identity agree with the direct
observation path.

## Materialization

The test writes only admitted entries. It rejects absolute paths, parent or
current-directory components, unsupported entry kinds, and any `.pijul`
metadata path. Directories are created explicitly; regular files use `0644`
and executable files use `0755`. The working copy is a fresh temporary
directory and is never the Pijul repository working copy.

## Command and outcome

The command runs under Bounded Exec with a cleared environment, null stdin,
bounded output, a five-second deadline, and process-group teardown. The pass
command reads the recorded file, checks its exact content, and asserts that
`.pijul` is absent. The fail command checks the same content and then exits 23.
The test maps `Disposition::Succeeded` to a completed workflow node and
`Disposition::ExitFailed` to a failed node, then asserts that
`admit_workflow_observation` returns `Success` or `DomainFailure` respectively.

## Lifecycle

The same exact Pijul state is used as the workflow source revision. The test
applies `StartRun` and `CompleteAttempt` to a fresh pure state and asserts the
recorded run outcome. No provider execution is claimed by the transition.

## Non-Claims

The test proves only the local repository, exact admitted bytes, the reviewed
command, and the selected Kiln transition. It does not prove source trust,
signatures, complete history, remote Pijul availability, collaboration status,
host confinement, or release eligibility.
