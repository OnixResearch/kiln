//! Machine policy admission against the host unit limits.
use crate::MIB;

pub const MAX_VCPUS: u8 = 8;
pub const MIN_MEMORY_MIB: u32 = 128;
pub const MAX_MEMORY_MIB: u32 = 16_384;
/// VMM process, page tables and device queues outside guest memory. The
/// systemd unit `MemoryMax` must cover guest memory plus this allowance.
pub const VMM_OVERHEAD_BYTES: u64 = 64 * MIB;
/// Firecracker main, API and per-vCPU threads plus `env` and slack. The unit
/// `TasksMax` must cover vCPUs plus this allowance.
pub const VMM_BASE_TASKS: u32 = 4;
pub const MIN_BOOT_TIMEOUT_MS: u64 = 100;
pub const MAX_BOOT_TIMEOUT_MS: u64 = 60_000;
pub const MIN_SOURCE_IMAGE_MIB: u32 = 1;
pub const MAX_SOURCE_IMAGE_MIB: u32 = 1024;
pub const MAX_WARM_SLOTS: u32 = 16;
pub const MAX_GUEST_PATH_BYTES: usize = 4096;

/// Operator machine policy for one Firecracker runner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachinePolicy<'a> {
    pub vcpus: u8,
    pub memory_mib: u32,
    pub boot_timeout_ms: u64,
    pub source_image_mib: u32,
    pub warm_slots: Option<u32>,
    pub guest_shell: &'a str,
}

/// Host enforcement that every VM must fit inside.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostBounds {
    pub unit_memory_bytes: u64,
    pub unit_tasks: u32,
    pub maximum_concurrent: u32,
    pub maximum_timeout_ms: u64,
}

// r[impl kiln.hosted.microvm.admission]
// r[impl kiln.hosted.microvm.admission.scenario.bounds]
/// Admit a machine policy before any image, unit or VM effect.
pub fn admit_machine(policy: &MachinePolicy<'_>, host: &HostBounds) -> Result<(), String> {
    if policy.vcpus == 0 || policy.vcpus > MAX_VCPUS {
        return Err("microVM vCPU count outside reviewed bound".into());
    }
    if !(MIN_MEMORY_MIB..=MAX_MEMORY_MIB).contains(&policy.memory_mib) {
        return Err("microVM memory outside reviewed bound".into());
    }
    let required = u64::from(policy.memory_mib)
        .checked_mul(MIB)
        .and_then(|bytes| bytes.checked_add(VMM_OVERHEAD_BYTES))
        .ok_or("microVM memory overflow")?;
    if required > host.unit_memory_bytes {
        return Err("microVM memory plus VMM overhead exceeds the unit memory limit".into());
    }
    if u32::from(policy.vcpus) + VMM_BASE_TASKS > host.unit_tasks {
        return Err("microVM vCPU threads exceed the unit task limit".into());
    }
    if !(MIN_BOOT_TIMEOUT_MS..=MAX_BOOT_TIMEOUT_MS).contains(&policy.boot_timeout_ms)
        || policy.boot_timeout_ms >= host.maximum_timeout_ms
    {
        return Err("microVM boot deadline outside reviewed bound".into());
    }
    if !(MIN_SOURCE_IMAGE_MIB..=MAX_SOURCE_IMAGE_MIB).contains(&policy.source_image_mib) {
        return Err("microVM source image size outside reviewed bound".into());
    }
    if let Some(slots) = policy.warm_slots {
        if slots == 0 || slots > MAX_WARM_SLOTS || slots > host.maximum_concurrent {
            return Err("warm pool slots outside reviewed bound".into());
        }
    }
    if !policy.guest_shell.starts_with('/')
        || policy.guest_shell.len() > MAX_GUEST_PATH_BYTES
        || policy
            .guest_shell
            .bytes()
            .any(|b| b == 0 || b.is_ascii_control())
        || policy.guest_shell.split('/').any(|p| p == "..")
    {
        return Err("invalid microVM guest shell".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> MachinePolicy<'static> {
        MachinePolicy {
            vcpus: 1,
            memory_mib: 256,
            boot_timeout_ms: 10_000,
            source_image_mib: 64,
            warm_slots: Some(2),
            guest_shell: "/bin/bash",
        }
    }
    fn host() -> HostBounds {
        HostBounds {
            unit_memory_bytes: 512 * MIB,
            unit_tasks: 16,
            maximum_concurrent: 2,
            maximum_timeout_ms: 60_000,
        }
    }

    // r[verify kiln.hosted.microvm.admission.scenario.bounds]
    #[test]
    fn machine_fits_unit_limits_at_the_boundary_and_not_beyond() {
        assert_eq!(admit_machine(&policy(), &host()), Ok(()));
        let exact = HostBounds {
            unit_memory_bytes: 256 * MIB + VMM_OVERHEAD_BYTES,
            unit_tasks: 1 + VMM_BASE_TASKS,
            ..host()
        };
        assert_eq!(admit_machine(&policy(), &exact), Ok(()));
        let short_memory = HostBounds {
            unit_memory_bytes: exact.unit_memory_bytes - 1,
            ..exact
        };
        assert!(admit_machine(&policy(), &short_memory).is_err());
        let short_tasks = HostBounds {
            unit_tasks: exact.unit_tasks - 1,
            ..exact
        };
        assert!(admit_machine(&policy(), &short_tasks).is_err());
    }

    // r[verify kiln.hosted.microvm.admission.scenario.bounds]
    #[test]
    fn every_named_bound_rejects_out_of_range_policy() {
        let cases: Vec<MachinePolicy<'static>> = vec![
            MachinePolicy {
                vcpus: 0,
                ..policy()
            },
            MachinePolicy {
                vcpus: MAX_VCPUS + 1,
                ..policy()
            },
            MachinePolicy {
                memory_mib: MIN_MEMORY_MIB - 1,
                ..policy()
            },
            MachinePolicy {
                memory_mib: MAX_MEMORY_MIB + 1,
                ..policy()
            },
            MachinePolicy {
                boot_timeout_ms: MIN_BOOT_TIMEOUT_MS - 1,
                ..policy()
            },
            MachinePolicy {
                boot_timeout_ms: 60_000,
                ..policy()
            },
            MachinePolicy {
                source_image_mib: 0,
                ..policy()
            },
            MachinePolicy {
                source_image_mib: MAX_SOURCE_IMAGE_MIB + 1,
                ..policy()
            },
            MachinePolicy {
                warm_slots: Some(0),
                ..policy()
            },
            MachinePolicy {
                warm_slots: Some(3),
                ..policy()
            },
            MachinePolicy {
                guest_shell: "bash",
                ..policy()
            },
            MachinePolicy {
                guest_shell: "/bin/../bash",
                ..policy()
            },
            MachinePolicy {
                guest_shell: "/bin/ba\nsh",
                ..policy()
            },
        ];
        for case in cases {
            assert!(admit_machine(&case, &host()).is_err(), "{case:?}");
        }
        let wide = HostBounds {
            unit_memory_bytes: 64 * 1024 * MIB,
            unit_tasks: 64,
            maximum_concurrent: 128,
            maximum_timeout_ms: 3_600_000,
        };
        assert!(
            admit_machine(
                &MachinePolicy {
                    warm_slots: Some(MAX_WARM_SLOTS + 1),
                    ..policy()
                },
                &wide
            )
            .is_err()
        );
        assert_eq!(
            admit_machine(
                &MachinePolicy {
                    warm_slots: Some(MAX_WARM_SLOTS),
                    ..policy()
                },
                &wide
            ),
            Ok(())
        );
        assert_eq!(
            admit_machine(
                &MachinePolicy {
                    warm_slots: None,
                    ..policy()
                },
                &host()
            ),
            Ok(())
        );
    }
}
