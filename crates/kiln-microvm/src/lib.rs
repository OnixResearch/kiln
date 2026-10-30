//! Pure core for the hosted Firecracker microVM backend.
//!
//! The library performs no I/O: it admits machine policy, plans the exact
//! Firecracker configuration, drives the per-job VM lifecycle, defines the
//! bounded vsock guest protocol and lays out the deterministic source archive.
//! Hosted shells own processes, sockets, files and the journal. The
//! `kiln-microvm-guest` binary is the guest-side shell.

pub mod admission;
pub mod archive;
pub mod lifecycle;
pub mod plan;
pub mod protocol;

/// One mebibyte; machine memory and source images are whole MiB.
pub const MIB: u64 = 1_048_576;
/// Firecracker's fixed guest context identifier for every per-job VM.
pub const GUEST_CID: u32 = 3;
/// The host side of vsock as seen from the guest.
pub const HOST_CID: u32 = 2;
/// The only host vsock port, served by one per-session host listener.
pub const AGENT_PORT: u32 = 10_240;
/// Unprivileged guest identity for customer steps.
pub const GUEST_UID: u32 = 1000;
pub const GUEST_GID: u32 = 1000;
/// Guest directory holding the extracted source snapshot.
pub const GUEST_WORKSPACE: &str = "/work";
/// Kernel command line: serial console only, no PCI, reboot on panic so the
/// VMM exits, and the guest agent init from the initramfs.
pub const BOOT_ARGS: &str = "console=ttyS0 reboot=k panic=-1 pci=off quiet rdinit=/init i8042.nokbd i8042.noaux 8250.nr_uarts=1 tsc=reliable";

/// Lowercase hex for BLAKE3 identities and entropy seeds.
pub fn is_identity(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
