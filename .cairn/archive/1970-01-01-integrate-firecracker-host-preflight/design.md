## Context

`kiln-microvm::host::HostFacts` already classifies the CVE and refuses unsafe
facts. The hosted shell must supply real observations and pass them to the
receipt and untrusted admission.

## Decisions

### Shell reads

The shell reads bounded files from `/proc` and `/sys`. A missing kernel release
is `unknown`; missing SMT data fails closed as active. KSM absence means not
running. MPTCP listener count comes from non-header `/proc/net/mptcp` rows when
the file exists. The package identity is `KILN_HOST_KERNEL_PACKAGE` when set,
otherwise the os-release build ID.

### Receipt and refusal

Every Firecracker job receipt includes `vm.host`. Untrusted dispatch calls the
pure refusal before any job credential/effect. The runner's non-empty
`host_preflight_override` admits an otherwise unsafe host; the receipt still
contains the observed facts, so the override is visible to reviewers.

## Risks

- Host fact files are host-specific; missing files are explicit unknown/fail
  closed rather than synthesized.
- Vendor kernel backports are not automatically known; operators may need a
  reviewed package/backport input in a later change.
- The override is operator authority, not evidence that the host is safe.
