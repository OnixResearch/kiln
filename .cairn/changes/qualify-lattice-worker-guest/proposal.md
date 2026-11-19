## Why

Lattice [#9](https://github.com/OnixResearch/lattice/issues/9)–[#12](https://github.com/OnixResearch/lattice/issues/12) need a Cloud Hypervisor guest that runs one admitted Lattice attempt and a retained session that can later move between hosts. Kiln [#51](https://github.com/OnixResearch/kiln/issues/51)–[#55](https://github.com/OnixResearch/kiln/issues/55) block them. Kiln already has the pure retained-session, handoff, bridge and drain cores and the retained hosted shell from `integrate-retained-mobility`, but:

- no guest image contains a pinned Lattice executable, admitted workflow definitions or a per-attempt writable workspace;
- no manifest binds the guest environment identity that a Lattice attempt can be admitted against;
- the retained hosted shell has never run against a real Cloud Hypervisor VMM, so its CH v54 API plans, confinement barriers, journaling and cleanup are fixture-tested only.

Most acceptance criteria in #52–#55 need two compatible KVM hosts, Aspen [#15](https://github.com/brittonr/aspen/issues/15) cross-host streaming and placement fencing, Lattice #10/#11 and onix-core service changes. None of these exist here. This host has `/dev/kvm`, a systemd user manager and Cloud Hypervisor 54.0 from the pinned guest nixpkgs.

## What Changes

- A milestone plan maps every #51–#55 acceptance criterion to a milestone. It marks each criterion that needs multiple hosts, onix-core or onixos changes, or infrastructure that is absent here.
- Milestone 1 is implemented and qualified on one host:
  - a pure `kiln_microvm::lattice_worker` core for the guest environment and image manifest identities, the per-attempt request, host-side and guest-side admission, per-attempt guest identity derivation, the per-attempt Lattice effect-authority profile and the bounded dispatch protocol;
  - a guest shell binary, `kiln-lattice-worker-guest`;
  - a Nix `lattice-worker-guest` package that contains the reviewed guest kernel, an immutable initramfs with the pinned Lattice executable and admitted workflows, the pinned Cloud Hypervisor and a `kiln.lattice-worker-guest.v1` manifest. Lattice comes from a separately locked cohort subflake pinned to Lattice `661499d2`;
  - a hosted `microvm::lattice_worker` shell. It prepares a blank per-attempt workspace image, admits the attempt against the verified manifest before any effect, and dispatches exactly once through a durable dispatch journal;
  - a real-KVM integration test that drives the existing retained host shell through cold boot, resume, dispatch, pause, paired snapshot, retirement, restore and negative cases.

## Capabilities

### New Capabilities

- `lattice-worker-guest`: pinned Lattice worker guest artifacts, attempt admission and single-host retained-session qualification.

## Impact

`crates/kiln-microvm` gains a module and a guest binary. `crates/kiln-hosted` gains a shell module, an integration test and a runbook section. `flake.nix` gains one x86_64-linux package and a content-addressed cohort subflake under `runtime/lattice-worker-cohort`. The per-job no-network CI profile, its guest images and its tests are unchanged. The retained hosted consumer still refuses effects through the selected Aspen authority. The single-host qualification uses a test-only reservation that the test names explicitly; it is not an admitted provider.

## Out of Scope

- A durable host-local or distributed placement authority, and volume or effect fencing.
- onix-core service, GC-root, credential and static-guest integration.
- The Lattice #10 execution profile, the Lattice #11 reconnecting gateway, and Lattice workflow-exchange dispatch inside the guest.
- Cross-host transfer, Iroh/Aspen migration channels, live migration and host draining (#53–#55).

These remain explicitly unqualified, with the prerequisites recorded in the milestone plan.
