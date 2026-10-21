# Qualify live Radicle CI

## Why

The published Radicle composition proves exact-source Nix execution and canonical/status recovery with real `cibtool` messages against isolated source/status fixtures. It does not prove native peer delivery into a running broker, automatic dispatch from the broker's node subscription, production SecretSpec resolution, or status dissemination to another native node. A trusted dedicated canary is the current consumer; no new provider or general pipeline abstraction is needed.

## What Changes

- Admit only fresh canary node homes, a dedicated repository/branch, distinct source and status identities, pinned executables and bounded owned processes. No production node, repository, service, signer or governance mutation is authorized. r[kiln.radicle_ci.live.authority]
- Run the actual pinned broker subscribed to a real native node; deliver a genuine peer source update and let the broker invoke the existing controller without manufactured requests or a manual `cibtool trigger`. r[kiln.radicle_ci.live.ingress]
- Use the normal status service and pinned real SecretSpec executable; explicitly sign/synchronize only canary status refs and read the exact revision/run/outcome/object/head from another native storage instance. r[kiln.radicle_ci.live.status]
- Prove original-operation recovery during controller/host loss and duplicate delivery, actual source-driven Nix failure, and bounded unavailable-provider uncertainty without a final status. r[kiln.radicle_ci.live.recovery] r[kiln.radicle_ci.live.failure]
- Retain repeatable commands, cohort and network topology, automatic ingress, canonical journals, remote status evidence, root discharge and credential/process retirement. Preserve same-host/WAN and worker-confinement non-claims. r[kiln.radicle_ci.live.evidence]

## Impact

Current consumer and immediate outcome: one trusted Radicle canary project obtains automatic source-event CI and independently fetched exact-revision status. Durable capability: a fresh-state native peer/broker qualification rail for later adoption by Kiln's own CI.

Maintenance owner: Kiln maintainers own controller semantics, canary tooling and evidence. OnixOS owns any later persistent service deployment; operators retain repository, signing and host authority.

Files: the existing native proof helpers plus a separate live-peer runner and typed canary configuration; source/status/controller code only where real integration exposes a consumer-visible defect; focused behavioral regressions, operator documentation and native Cairn evidence.

Verification: actual native peer/broker/runtime/build/status scenarios; paired positive/negative admission cases where new validation is needed; focused affected checks and all required repository gates after integration. Publication, prior fixture tests and exit codes alone are not live evidence.

## Authority and exclusions

The operator's `begin` request authorizes this described dedicated canary, including fresh test identities, source pushes and status signing/synchronization confined to its exact RID. It does not authorize changes to the operator's existing Radicle home, production repositories/signers/services, governance, visibility, mirrors or `main`, nor activation of built OnixOS outputs or release approval. No untrusted repositories are admitted. A same-host native TCP run must be identified as such and cannot be called WAN, hostile-worker confinement or production qualification.
