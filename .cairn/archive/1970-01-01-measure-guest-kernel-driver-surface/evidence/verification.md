# Verification

Issue [#40](https://github.com/OnixResearch/kiln/issues/40).

`nix build .#checks.x86_64-linux.microvm-guest-surface` measures the pinned
guest kernel and module closure against the reviewed deny list and compares the
result with `fixtures/microvm/guest-driver-surface.json`. The current cohort is
kernel 6.18.60 from guest nixpkgs
`1d060086d60cb76a40cf37109f979d88a5bd6396`, configuration BLAKE3
`421514dc1e119754227a903f6aefd35dea6f7124bf3b7f77b1496a55efa71595`, and a
nine-module virtio/vsock closure.

Enforced classes are absent: Bluetooth, the wireless stack, FireWire,
Thunderbolt/USB4, InfiniBand, sound and USB host-controller drivers. USB core
and display/DRM/framebuffer support are compiled into the NixOS-default kernel
and are recorded as builtin measured-only misses; denying them needs a separately
reviewed slimmed kernel.

The live escape test passed with the new fail-closed sysfs probes. Its
`proc-sys-probing` output reports `denied deny-listed class` for
`/sys/class/bluetooth`, `/sys/class/sound`, `/sys/bus/firewire`,
`/sys/bus/thunderbolt`, `/sys/class/infiniband`, `/sys/class/ieee80211` and
`/sys/bus/usb/drivers/xhci_hcd`. Receipts keep `hostile_isolation_claim: false`.

This is image measurement, not isolation proof. Host covert channels remain
issue #28 scope, and the measured builtin classes are explicit misses.
