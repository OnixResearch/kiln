# Qualify distinct-host Radicle status dissemination

## Why

The distinct-host canary proves that a genuine remote publisher update enters
the unchanged local broker and produces canonical results, but it keeps the
independent status observer on the local host. The remaining uncertainty is
whether the status object written and signed by the local CI authority is
fetched by an observer on the second host and reduced there with the pinned
Kiln read path. The current consumer is the same trusted canary; no new provider
or general replication abstraction is needed.

## What Changes

- Extend the typed distinct-host profile with the local CI tailnet address and
  explicit remote observer placement, keeping fresh local and remote
  `/var/tmp` roots and the existing bounded admission.
  r[kiln.radicle_ci.distinct_status.authority]
- Run the observer node and its status storage on the second host. The local CI
  node listens on the tailnet address so the remote observer can connect
  directly. Status creation, signing and synchronization of the checkpoint ref
  remain local and unchanged.
  r[kiln.radicle_ci.distinct_status.transport]
- After local sign/sync, fetch the status into the remote observer storage and
  reduce it there with the pinned `kiln-radicle-ci status-reconcile` command.
  Require the exact repository, revision, run, outcome, object ID and head, and
  the pre-sign absence of the unannounced object.
  r[kiln.radicle_ci.distinct_status.readback]
- Retain the remote observer log, commands, readbacks and retirement alongside
  the existing ingress, journals and status evidence.
  r[kiln.radicle_ci.distinct_status.evidence]
- State that object creation and signing remained local while dissemination
  and readback became distinct-host. Do not promote the private tailnet to
  WAN, hostile-network or production qualification.
  r[kiln.radicle_ci.distinct_status.claims]

## Impact

Current consumer and immediate outcome: the trusted canary's exact signed
status becomes independently visible and reducible on the second host. Durable
capability: a repeatable distinct-host status dissemination and readback rail.

Maintenance owner: Kiln maintainers own the runner, admission and evidence.
The operator owns the SSH destination, tailnet membership, firewall, remote
root, test identities and cleanup.

Files: the existing distinct-host runner and a status exercise/profile; typed
Nickel admission and fixtures; the unchanged coordinator/status code; operator
documentation and Cairn evidence.

Verification: actual remote observer fetch and remote reduction for success and
genuine failure, pre-sign absence, unchanged unavailable-provider uncertainty,
and all required repository gates.

## Authority and exclusions

The operator's `continue` request authorizes this dedicated canary, including a
fresh remote observer root, SSH orchestration to the named host, status refs
confined to its exact RID, and remote execution of the pinned read-only status
reconcile command. It does not authorize changes to the operator's existing
Radicle homes, production repositories, signers, services, governance,
visibility, mirrors or `main`, nor activation of built outputs or release
approval. Signing and status-object creation remain local and unchanged. No
untrusted repository is admitted.
