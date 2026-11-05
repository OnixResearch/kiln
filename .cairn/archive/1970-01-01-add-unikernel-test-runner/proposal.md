## Why

Single prebuilt test binaries do not require a Linux shell guest. Issue #33 requests a native unikernel on Firecracker without weakening confinement or claiming hostile isolation.

## What Changes

- Compare pinned Nanos/OPS and Unikraft ELF loader native source/build capabilities; use OSv only if both cannot satisfy the binary contract.
- Admit one static x86_64 ELF, fixed argv and declared read-only fixtures; refuse shell, subprocess, exec, multistep and network requests.
- Build deterministic native images and observe bounded serial result, image identity, cold boot and no-device confinement.

## Impact

- **Files**: hosted pipeline/execution, unikernel module, flake guest inputs, focused tests and hosted documentation.
- **Testing**: positive/negative admission and serial outcomes; real static Rust/Go pass/fail, network probe and repeat image identities where native compatibility permits. Missing external prerequisites remain explicitly unqualified.
