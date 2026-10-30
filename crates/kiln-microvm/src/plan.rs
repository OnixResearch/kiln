//! Exact Firecracker configuration, API bodies and snapshot identity.
use serde::Serialize;
use serde_json::{Value, json};

use crate::admission::MachinePolicy;
use crate::{AGENT_PORT, BOOT_ARGS, GUEST_CID};

/// Host paths for one VM. All are absolute UTF-8 paths in a private directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VmPaths<'a> {
    pub kernel: &'a str,
    pub initrd: &'a str,
    pub source_image: &'a str,
    pub vsock: &'a str,
}
/// The source drive identifier inside the VM configuration.
pub const SOURCE_DRIVE: &str = "source";
/// CPUID.(EAX=7,ECX=0):ECX.CET_SS and EDX.CET_IBT.
const CPUID_CET_SS_BIT: u32 = 7;
const CPUID_CET_IBT_BIT: u32 = 20;
/// CPUID.(EAX=0xD,ECX=1):ECX supervisor-state bits CET_U and CET_S.
const XSS_CET_USER_BIT: u32 = 11;
const XSS_CET_SUPERVISOR_BIT: u32 = 12;
/// KVM_CPUID_FLAG_SIGNIFCANT_INDEX: the leaf is subleaf-indexed.
const CPUID_SUBLEAF_SIGNIFICANT: u32 = 1;

/// Firecracker CPUID bitmap clearing `zero` bits and keeping all others.
fn cpuid_bitmap(zero: &[u32]) -> String {
    let bits: String = (0..32)
        .rev()
        .map(|bit| if zero.contains(&bit) { '0' } else { 'x' })
        .collect();
    format!("0b{bits}")
}

/// Custom CPU template hiding CET from the guest. Firecracker 1.15 does not
/// carry CET XSAVE supervisor state through a snapshot on KVM hosts that
/// virtualize CET, so a restored guest faults in `XRSTORS`; cold and restored
/// VMs use the same template so both boot modes expose identical CPUs.
pub fn cpu_template() -> Value {
    json!({
        "cpuid_modifiers": [
            {"leaf": "0x7", "subleaf": "0x0", "flags": CPUID_SUBLEAF_SIGNIFICANT, "modifiers": [
                {"register": "ecx", "bitmap": cpuid_bitmap(&[CPUID_CET_SS_BIT])},
                {"register": "edx", "bitmap": cpuid_bitmap(&[CPUID_CET_IBT_BIT])},
            ]},
            {"leaf": "0xd", "subleaf": "0x1", "flags": CPUID_SUBLEAF_SIGNIFICANT, "modifiers": [
                {"register": "ecx", "bitmap": cpuid_bitmap(&[XSS_CET_USER_BIT, XSS_CET_SUPERVISOR_BIT])},
            ]},
        ],
        "msr_modifiers": [],
    })
}
// r[impl kiln.hosted.microvm.plan]
/// Firecracker `--config-file` document. There is deliberately no
/// `network-interfaces` member: the guest has loopback only.
pub fn vm_config(paths: &VmPaths<'_>, policy: &MachinePolicy<'_>) -> Value {
    json!({
        "boot-source": {
            "kernel_image_path": paths.kernel,
            "initrd_path": paths.initrd,
            "boot_args": BOOT_ARGS,
        },
        "drives": [{
            "drive_id": SOURCE_DRIVE,
            "path_on_host": paths.source_image,
            "is_root_device": false,
            "is_read_only": true,
        }],
        "machine-config": {
            "vcpu_count": policy.vcpus,
            "mem_size_mib": policy.memory_mib,
            "smt": false,
        },
        "vsock": { "guest_cid": GUEST_CID, "uds_path": paths.vsock },
        "entropy": {},
        "cpu-config": cpu_template(),
    })
}

/// Firecracker forwards guest connections to `<uds_path>_<port>`.
pub fn host_listener_path(vsock: &str) -> String {
    format!("{vsock}_{AGENT_PORT}")
}

/// API body pausing a running VM before a snapshot.
pub fn pause() -> Value {
    json!({"state": "Paused"})
}

/// API body for a full snapshot of a paused VM.
pub fn snapshot_create(state: &str, memory: &str) -> Value {
    json!({"snapshot_type": "Full", "snapshot_path": state, "mem_file_path": memory})
}

/// API body restoring a snapshot from a private copy-on-write memory file.
pub fn snapshot_load(state: &str, memory: &str) -> Value {
    json!({
        "snapshot_path": state,
        "mem_backend": {"backend_type": "File", "backend_path": memory},
        "resume_vm": true,
    })
}

/// Inputs that fully determine whether a warm snapshot is reusable.
#[derive(Debug, Serialize)]
pub struct SnapshotInputs<'a> {
    pub firecracker_blake3: &'a str,
    pub kernel_blake3: &'a str,
    pub initrd_blake3: &'a str,
    pub vcpus: u8,
    pub memory_mib: u32,
    pub source_image_mib: u32,
    pub slot: u32,
    pub slot_directory: &'a str,
}

// r[impl kiln.hosted.microvm.warm_pool]
/// BLAKE3 identity of a warm slot snapshot. Any changed binary, image, machine
/// shape, boot argument, protocol port or slot path yields a new identity.
pub fn snapshot_identity(inputs: &SnapshotInputs<'_>) -> String {
    let document = json!({
        "schema": "kiln.microvm.snapshot.v1",
        "inputs": inputs,
        "boot_args": BOOT_ARGS,
        "guest_cid": GUEST_CID,
        "agent_port": AGENT_PORT,
        "protocol": crate::protocol::SCHEMA,
    });
    blake3::hash(document.to_string().as_bytes())
        .to_hex()
        .to_string()
}

/// One HTTP/1.1 request on the Firecracker API socket.
pub fn api_request(method: &str, path: &str, body: &Value) -> Vec<u8> {
    let body = body.to_string();
    format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nAccept: application/json\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

/// Status code of an API response head, if well formed.
pub fn api_status(response: &[u8]) -> Option<u16> {
    let line = response.split(|b| *b == b'\n').next()?;
    let text = std::str::from_utf8(line).ok()?.trim_end();
    let mut parts = text.split(' ');
    if !parts.next()?.starts_with("HTTP/1.") {
        return None;
    }
    parts.next()?.parse().ok()
}

/// Total response length once the head is complete: head plus
/// `Content-Length` body bytes. `None` while the head is still incomplete.
pub fn api_response_length(response: &[u8]) -> Option<usize> {
    let head_end = response.windows(4).position(|w| w == b"\r\n\r\n")? + 4;
    let head = std::str::from_utf8(response.get(..head_end)?).ok()?;
    let body = head
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.trim().eq_ignore_ascii_case("content-length"))
        .map_or(Some(0), |(_, value)| value.trim().parse::<usize>().ok())?;
    head_end.checked_add(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> MachinePolicy<'static> {
        MachinePolicy {
            vcpus: 2,
            memory_mib: 256,
            boot_timeout_ms: 5000,
            source_image_mib: 16,
            warm_slots: None,
            guest_shell: "/bin/bash",
        }
    }
    fn paths() -> VmPaths<'static> {
        VmPaths {
            kernel: "/nix/store/k/vmlinux",
            initrd: "/nix/store/i/initrd",
            source_image: "/tmp/vm/source.img",
            vsock: "/tmp/vm/v.sock",
        }
    }

    // r[verify kiln.hosted.microvm.plan]
    #[test]
    fn plan_has_one_read_only_source_drive_vsock_and_no_network() {
        let config = vm_config(&paths(), &policy());
        assert_eq!(config, vm_config(&paths(), &policy()));
        let object = config.as_object().unwrap_or_else(|| unreachable!());
        assert!(!object.contains_key("network-interfaces"));
        let mut keys: Vec<_> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "boot-source",
                "cpu-config",
                "drives",
                "entropy",
                "machine-config",
                "vsock"
            ]
        );
        let at = |pointer: &str| config.pointer(pointer).cloned().unwrap_or(Value::Null);
        assert_eq!(
            at("/cpu-config/cpuid_modifiers/0/modifiers/0/bitmap"),
            "0bxxxxxxxxxxxxxxxxxxxxxxxx0xxxxxxx"
        );
        assert_eq!(
            at("/cpu-config/cpuid_modifiers/1/modifiers/0/bitmap"),
            "0bxxxxxxxxxxxxxxxxxxx00xxxxxxxxxxx"
        );
        assert_eq!(at("/drives").as_array().map(Vec::len), Some(1));
        assert_eq!(at("/drives/0/is_read_only"), true);
        assert_eq!(at("/drives/0/is_root_device"), false);
        assert_eq!(at("/drives/0/path_on_host"), "/tmp/vm/source.img");
        assert_eq!(at("/vsock/guest_cid"), GUEST_CID);
        assert_eq!(at("/machine-config/vcpu_count"), 2);
        assert!(
            at("/boot-source/boot_args")
                .as_str()
                .is_some_and(|args| args.contains("pci=off") && args.contains("rdinit=/init"))
        );
        assert_eq!(host_listener_path("/tmp/vm/v.sock"), "/tmp/vm/v.sock_10240");
    }

    // r[verify kiln.hosted.microvm.warm_pool]
    #[test]
    fn snapshot_identity_changes_with_every_bound_input() {
        let base = SnapshotInputs {
            firecracker_blake3: "a",
            kernel_blake3: "b",
            initrd_blake3: "c",
            vcpus: 1,
            memory_mib: 256,
            source_image_mib: 16,
            slot: 0,
            slot_directory: "/pool/slot-0",
        };
        let id = snapshot_identity(&base);
        assert!(crate::is_identity(&id));
        assert_eq!(id, snapshot_identity(&base));
        for changed in [
            SnapshotInputs {
                firecracker_blake3: "x",
                ..base
            },
            SnapshotInputs {
                kernel_blake3: "x",
                ..base
            },
            SnapshotInputs {
                initrd_blake3: "x",
                ..base
            },
            SnapshotInputs { vcpus: 2, ..base },
            SnapshotInputs {
                memory_mib: 512,
                ..base
            },
            SnapshotInputs {
                source_image_mib: 32,
                ..base
            },
            SnapshotInputs { slot: 1, ..base },
            SnapshotInputs {
                slot_directory: "/pool/slot-1",
                ..base
            },
        ] {
            assert_ne!(snapshot_identity(&changed), id);
        }
    }

    #[test]
    fn api_messages_are_exact_and_statuses_parse_only_http() {
        let request = api_request("PUT", "/snapshot/load", &snapshot_load("/s", "/m"));
        let text = String::from_utf8(request).unwrap_or_default();
        assert!(text.starts_with("PUT /snapshot/load HTTP/1.1\r\n"));
        let body = text.split("\r\n\r\n").nth(1).unwrap_or_default();
        assert!(text.contains(&format!("Content-Length: {}\r\n", body.len())));
        assert_eq!(
            serde_json::from_str::<Value>(body).ok(),
            Some(
                json!({"snapshot_path":"/s","mem_backend":{"backend_type":"File","backend_path":"/m"},"resume_vm":true})
            )
        );
        assert_eq!(api_status(b"HTTP/1.1 204 No Content\r\n\r\n"), Some(204));
        assert_eq!(api_status(b"HTTP/1.1 400 Bad Request\r\n"), Some(400));
        assert_eq!(api_status(b"garbage 204"), None);
        assert_eq!(api_status(b""), None);
        assert_eq!(api_response_length(b"HTTP/1.1 204 \r\nServer: x\r\n"), None);
        assert_eq!(api_response_length(b"HTTP/1.1 204 \r\n\r\n"), Some(17));
        assert_eq!(
            api_response_length(b"HTTP/1.1 400 \r\ncontent-length: 5\r\n\r\nab"),
            Some(41)
        );
        assert_eq!(
            api_response_length(b"HTTP/1.1 400 \r\nContent-Length: x\r\n\r\n"),
            None
        );
    }
}
