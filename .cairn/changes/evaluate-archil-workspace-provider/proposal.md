## Why

Issue [#82](https://github.com/OnixResearch/kiln/issues/82) asks which parts of Kiln's projected-workspace contract an external durable filesystem such as Archil can satisfy, and which require Kiln's own materialization, authority and acceptance boundaries. A successful mount, write or fork is not evidence of the full contract, and Archil documents several mechanisms (disk-wide credentials, branches that are not access boundaries, asynchronous bucket copies, forced ownership) that would break it if relied on.

## What Changes

- A versioned provider capability profile, `kiln.workspace-provider-profile.v1` in typed Nickel, sits behind the pure-core/provider-shell boundary. Canonical identities stay BLAKE3; provider disk, branch, checkpoint, mount and sandbox identifiers are references. The contract refuses profiles that rely on a documented-insufficient mechanism and refuses qualification from vendor claims or fixtures.
- Positive fixtures declare the local baseline and two Archil profiles; one negative fixture covers each refusal, with a test that checks each refusal's reason.
- `docs/archil-workspace-evaluation.md` maps every contract requirement to Archil's documented behavior, labels each as a vendor claim, and concludes: retain as reference, adapting branch-per-attempt isolation for unrestricted profiles only after qualification.

## Capabilities

### New Capabilities

- `workspace-provider-evaluation`: the provider capability profile and the documentation-review evaluation.

## Impact

New Nickel contract, fixtures, a shell test run in the `nickel` Nix check, and documentation. No Archil dependency, credential, adapter, network call or default.

## Non-goals

The shared corpus and local baseline run (they need #78's projected materializer), the live Archil experiment (an operator-configured disposable environment with named time, resource, cost and retention bounds), and any production provider selection.
