## Decisions

The measurement runs in Nix over pinned build inputs, not in a live guest. That
makes the verdict reproducible without KVM and keeps the escape suite focused on
runtime denial. The inventory additionally pins the guest nixpkgs revision,
kernel version and kernel-configuration BLAKE3; a kernel or configuration change
requires a reviewed inventory refresh, mirroring the cohort-refresh convention.

Classes use exact Kconfig symbols, not prefixes. Prefix matching produced false
positives such as `CONFIG_BTREE` for Bluetooth and `CONFIG_SND_*` knobs while
`CONFIG_SND=m`; exact symbols classify the class core.

`loadable_module_present` means the guest module closure ships a matching
module, because only those modules can ever load before `modules_disabled=1`.
Kernel package modules outside the closure are not guest-image surface.

Enforced classes (Bluetooth, wireless stack, FireWire, Thunderbolt/USB4,
InfiniBand, sound, USB host controllers) are absent today. Measured-only classes
(USB core, display/DRM/framebuffer) are compiled in by the NixOS-default kernel
and are recorded as misses; denying them requires a reviewed slimmed kernel,
which this change does not attempt.

## Non-claims

The inventory measures one pinned image. It does not prove isolation, kernel
vulnerability freedom, host covert-channel resistance (issue #28), or release
readiness. Absence of a class means the image neither compiles it in nor ships a
loadable module; it says nothing about other kernels or hosts. Runtime module
loading remains denied by `modules_disabled=1`.
