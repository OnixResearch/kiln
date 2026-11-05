//! Backend-neutral VMM capability and launch seam.
//!
//! This is deliberately a pure planning surface. Shells perform the returned
//! effects; the trait grants no isolation claim by itself.

use crate::admission::MachinePolicy;
use crate::{AGENT_PORT, BOOT_ARGS, GUEST_CID};
use serde::Serialize;

/// The only admissible guest-network capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkCapability {
    /// No guest network device, TAP, virtio-net or tunnel.
    NoDevice,
}

/// Host confinement facts a backend can expose to the observation gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ConfinementKinds {
    pub seccomp: bool,
    pub landlock: bool,
    pub no_new_privs: bool,
    pub cap_eff: bool,
}

/// What a backend admits and what its receipts may claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct Capabilities {
    pub network: NetworkCapability,
    pub snapshot: bool,
    pub arch: &'static [&'static str],
    pub confinement: ConfinementKinds,
    pub per_job: bool,
}

impl Capabilities {
    /// Whether this backend is reviewed on the given host architecture.
    pub fn admits_arch(&self, arch: &str) -> bool {
        self.arch.contains(&arch)
    }
}

/// Inputs that distinguish one VMM identity from another.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct IdentityInputs<'a> {
    pub kind: &'a str,
    pub blake3: &'a str,
    pub version: &'a str,
}

/// Pure backend seam. Implementations return plans, never perform I/O.
pub trait VmmBackend {
    fn kind(&self) -> &'static str;
    fn capabilities(&self) -> Capabilities;
    fn boot_args(&self) -> &'static str;
    fn listener_path(&self, vsock: &str) -> String;
    fn identity_inputs(
        &self,
        blake3: &'static str,
        version: &'static str,
    ) -> IdentityInputs<'static>;
}

/// The Firecracker reference backend.
#[derive(Clone, Copy, Debug, Default)]
pub struct Firecracker;

impl VmmBackend for Firecracker {
    fn kind(&self) -> &'static str {
        "firecracker"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            network: NetworkCapability::NoDevice,
            snapshot: true,
            arch: &["x86_64"],
            confinement: ConfinementKinds {
                seccomp: true,
                landlock: false,
                no_new_privs: true,
                cap_eff: true,
            },
            per_job: true,
        }
    }
    fn boot_args(&self) -> &'static str {
        BOOT_ARGS
    }
    fn listener_path(&self, vsock: &str) -> String {
        format!("{vsock}_{AGENT_PORT}")
    }
    fn identity_inputs(
        &self,
        blake3: &'static str,
        version: &'static str,
    ) -> IdentityInputs<'static> {
        IdentityInputs {
            kind: self.kind(),
            blake3,
            version,
        }
    }
}

/// Cloud Hypervisor phase-1 plan surface. Execution remains Firecracker-only
/// until the host/escape-suite work lands.
#[derive(Clone, Copy, Debug, Default)]
pub struct CloudHypervisor;

impl VmmBackend for CloudHypervisor {
    fn kind(&self) -> &'static str {
        "cloud-hypervisor"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            network: NetworkCapability::NoDevice,
            snapshot: true,
            arch: &["x86_64"],
            confinement: ConfinementKinds {
                seccomp: true,
                landlock: true,
                no_new_privs: true,
                cap_eff: true,
            },
            per_job: true,
        }
    }
    fn boot_args(&self) -> &'static str {
        "console=ttyS0 reboot=k panic=-1 quiet rdinit=/init i8042.nokbd i8042.noaux 8250.nr_uarts=1 tsc=reliable"
    }
    fn listener_path(&self, vsock: &str) -> String {
        format!("{vsock}_{AGENT_PORT}")
    }
    fn identity_inputs(
        &self,
        blake3: &'static str,
        version: &'static str,
    ) -> IdentityInputs<'static> {
        IdentityInputs {
            kind: self.kind(),
            blake3,
            version,
        }
    }
}

/// Paths for one Cloud Hypervisor cold boot.
#[derive(Clone, Copy, Debug)]
pub struct ColdBootPaths<'a> {
    pub kernel: &'a str,
    pub initrd: &'a str,
    pub source_image: &'a str,
    pub vsock: &'a str,
}

/// Exact phase-1 Cloud Hypervisor argv. No `--net`, no API socket, and the
/// source disk is read-only. The caller supplies the reviewed binary path.
pub fn cloud_hypervisor_cold_boot(
    binary: &str,
    paths: &ColdBootPaths<'_>,
    policy: &MachinePolicy<'_>,
) -> Vec<String> {
    vec![
        binary.to_owned(),
        "--kernel".into(),
        paths.kernel.into(),
        "--initramfs".into(),
        paths.initrd.into(),
        "--cmdline".into(),
        CloudHypervisor.boot_args().into(),
        "--cpus".into(),
        format!("boot={}", policy.vcpus),
        "--memory".into(),
        format!("size={}M", policy.memory_mib),
        "--disk".into(),
        format!("path={},readonly=on", paths.source_image),
        "--vsock".into(),
        format!("cid={GUEST_CID},socket={}", paths.vsock),
        "--serial".into(),
        "file=/dev/null".into(),
        "--console".into(),
        "off".into(),
        "--seccomp".into(),
        "true".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_backends_declare_no_guest_network_and_shared_vsock() {
        for backend in [&Firecracker as &dyn VmmBackend, &CloudHypervisor] {
            assert_eq!(backend.capabilities().network, NetworkCapability::NoDevice);
            assert_eq!(backend.capabilities().arch, &["x86_64"]);
            assert_eq!(backend.listener_path("/tmp/v.sock"), "/tmp/v.sock_10240");
        }
        assert!(Firecracker.capabilities().snapshot);
        assert!(CloudHypervisor.capabilities().confinement.landlock);
        assert!(Firecracker.capabilities().admits_arch("x86_64"));
        assert!(!Firecracker.capabilities().admits_arch("riscv64"));
    }

    #[test]
    fn cloud_hypervisor_argv_has_no_network_or_api_socket() {
        let policy = MachinePolicy {
            vcpus: 2,
            memory_mib: 256,
            boot_timeout_ms: 10_000,
            source_image_mib: 64,
            warm_slots: None,
            guest_shell: "/bin/sh",
        };
        let arguments = cloud_hypervisor_cold_boot(
            "/nix/store/ch/bin/cloud-hypervisor",
            &ColdBootPaths {
                kernel: "/k",
                initrd: "/i",
                source_image: "/s",
                vsock: "/v",
            },
            &policy,
        );
        assert!(arguments.iter().any(|value| value == "--seccomp"));
        assert!(!arguments.iter().any(|value| value == "--net"));
        assert!(!arguments.iter().any(|value| value == "--api-socket"));
        assert!(arguments.iter().any(|value| value == "path=/s,readonly=on"));
    }
}
