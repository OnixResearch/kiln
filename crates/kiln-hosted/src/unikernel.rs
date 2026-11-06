//! Cold-only trusted static test execution on the pinned Unikraft ABI.
use crate::execution::{Backend, Config, ProcessFacts, StepObservation};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Component, Path};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 128 * 1024 * 1024;
const MAX_FIXTURES: usize = 128;
const MAX_ARG_BYTES: usize = 1024;
const RESULT_PREFIX: &str = "KILN_RESULT:";
const ARGV_END: &str = "KILN_ARGV_END";
// Reviewed explicit expansion of systemd 262's @system-service allowlist.
// The typed source and embedded export are checked together by the flake.
const PREBOOT_SYSCALLS: &str = include_str!("unikernel-preboot-syscalls.txt");
const PREBOOT_POLICY_SOURCE: &[u8] =
    include_bytes!("../../../config/unikernel-preboot-seccomp.ncl");
const PREBOOT_POLICY: &str = "kiln.native.preboot.v1";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileSpec {
    pub path: String,
    pub blake3: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestSpec {
    pub binary: FileSpec,
    #[serde(default)]
    pub argv: Vec<String>,
    #[serde(default)]
    pub fixtures: Vec<FileSpec>,
}
fn relative(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 4096
        && path.is_ascii()
        && path
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
        && Path::new(path)
            .components()
            .all(|p| matches!(p, Component::Normal(_)))
        && !path.bytes().any(|b| b.is_ascii_control())
}
pub fn validate_spec(spec: &TestSpec) -> Result<(), String> {
    if spec.fixtures.len() > MAX_FIXTURES || spec.argv.len() > 64 {
        return Err("unikernel input count exceeds bound".into());
    }
    let mut paths = std::collections::BTreeSet::from(["test"]);
    for file in std::iter::once(&spec.binary).chain(&spec.fixtures) {
        if !relative(&file.path)
            || file.blake3.len() != 64
            || !file.blake3.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("invalid unikernel workspace path or BLAKE3".into());
        }
        if file.path == "TRAILER!!!" || file.path.starts_with("TRAILER!!!/") {
            return Err("reserved newc terminator path".into());
        }
    }
    for file in &spec.fixtures {
        if !paths.insert(file.path.as_str()) {
            return Err("duplicate or reserved fixture path".into());
        }
    }
    for path in &paths {
        if paths.iter().any(|other| {
            other
                .strip_prefix(*path)
                .is_some_and(|suffix| suffix.starts_with('/'))
        }) {
            return Err("fixture file/directory collision".into());
        }
    }
    let mut total = 0usize;
    for arg in &spec.argv {
        total = total.saturating_add(arg.len() + 1);
        if arg.is_empty()
            || arg == ARGV_END
            || !arg
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_./:+,@%-".contains(&b))
            || arg.starts_with('-')
            || arg.contains("--")
            || total > MAX_ARG_BYTES
        {
            return Err("unsafe unikernel argv token".into());
        }
    }
    Ok(())
}
fn number(bytes: &[u8], offset: usize, size: usize) -> Result<u64, String> {
    let end = offset.checked_add(size).ok_or("ELF offset overflow")?;
    let value = bytes.get(offset..end).ok_or("truncated ELF")?;
    Ok(value
        .iter()
        .enumerate()
        .fold(0, |v, (i, b)| v | (u64::from(*b) << (i * 8))))
}
/// The supported native ABI is static x86-64 little-endian ELF64 PIE only.
pub fn validate_elf(bytes: &[u8]) -> Result<(), String> {
    if bytes.get(..7) != Some(b"\x7fELF\x02\x01\x01")
        || number(bytes, 16, 2)? != 3
        || number(bytes, 18, 2)? != 62
        || number(bytes, 20, 4)? != 1
        || number(bytes, 52, 2)? != 64
    {
        return Err("expected static x86_64 little-endian ELF64 PIE".into());
    }
    let offset = usize::try_from(number(bytes, 32, 8)?).map_err(|_| "ELF offset overflow")?;
    let size = usize::try_from(number(bytes, 54, 2)?).map_err(|_| "ELF size overflow")?;
    let count = usize::try_from(number(bytes, 56, 2)?).map_err(|_| "ELF count overflow")?;
    if size != 56 || count == 0 || count > 1024 {
        return Err("unsupported ELF program table".into());
    }
    let mut load = false;
    for i in 0..count {
        let p = offset.checked_add(i * size).ok_or("ELF offset overflow")?;
        let header = bytes
            .get(p..p.checked_add(size).ok_or("ELF overflow")?)
            .ok_or("truncated ELF table")?;
        let kind = number(header, 0, 4)?;
        if kind == 3 {
            return Err("PT_INTERP is forbidden".into());
        }
        let start = usize::try_from(number(header, 8, 8)?).map_err(|_| "ELF offset overflow")?;
        let len = usize::try_from(number(header, 32, 8)?).map_err(|_| "ELF size overflow")?;
        let segment = bytes
            .get(start..start.checked_add(len).ok_or("ELF overflow")?)
            .ok_or("truncated ELF segment")?;
        if kind == 1 {
            load = true;
        }
        if kind == 2 {
            if segment.len() % 16 != 0 {
                return Err("malformed ELF dynamic table".into());
            }
            let mut terminated = false;
            for entry in segment.chunks_exact(16) {
                let tag = number(entry, 0, 8)?;
                if tag == 0 {
                    terminated = true;
                    break;
                }
                if tag == 1 {
                    return Err("DT_NEEDED is forbidden".into());
                }
            }
            if !terminated {
                return Err("unterminated ELF dynamic table".into());
            }
        }
    }
    if !load {
        return Err("ELF has no loadable segment".into());
    }
    Ok(())
}
fn read_file(workspace: &Path, file: &FileSpec) -> Result<Vec<u8>, String> {
    let mut path = workspace.to_path_buf();
    for component in Path::new(&file.path).components() {
        path.push(component);
        if std::fs::symlink_metadata(&path)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("unikernel inputs must not traverse symlinks".into());
        }
    }
    let input = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| e.to_string())?;
    if !input.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("unikernel input is not a regular file".into());
    }
    let mut bytes = Vec::new();
    input
        .take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FILE_BYTES || blake3::hash(&bytes).to_hex().as_str() != file.blake3 {
        return Err("unikernel input size or digest mismatch".into());
    }
    Ok(bytes)
}
fn newc(out: &mut Vec<u8>, inode: u32, name: &str, mode: u32, data: &[u8]) {
    // Writing to a Vec cannot fail; format directly into its preallocated buffer.
    let _ = write!(
        out,
        "070701{inode:08x}{mode:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}",
        0,
        0,
        1,
        0,
        data.len(),
        0,
        0,
        0,
        0,
        name.len() + 1,
        0
    );
    out.extend_from_slice(name.as_bytes());
    out.push(0);
    while out.len() % 4 != 0 {
        out.push(0);
    }
    out.extend_from_slice(data);
    while out.len() % 4 != 0 {
        out.push(0);
    }
}
pub fn build_image(workspace: &Path, spec: &TestSpec) -> Result<Vec<u8>, String> {
    validate_spec(spec)?;
    let binary = read_file(workspace, &spec.binary)?;
    validate_elf(&binary)?;
    let mut entries = BTreeMap::new();
    entries.insert("test".to_owned(), (0o100755, binary));
    let mut total = entries.get("test").map_or(0, |(_, b)| b.len());
    for fixture in &spec.fixtures {
        let bytes = read_file(workspace, fixture)?;
        total += bytes.len();
        if total > MAX_IMAGE_BYTES {
            return Err("unikernel image exceeds bound".into());
        }
        let mut parent = Path::new(&fixture.path).parent();
        while let Some(p) = parent.filter(|p| !p.as_os_str().is_empty()) {
            entries
                .entry(p.to_str().ok_or("invalid fixture path")?.to_owned())
                .or_insert_with(|| (0o40755, Vec::new()));
            parent = p.parent();
        }
        entries.insert(fixture.path.clone(), (0o100444, bytes));
    }
    let entry_bytes = |name: &str, len: usize| ((110 + name.len() + 1 + 3) & !3) + ((len + 3) & !3);
    let capacity = entry_bytes(".", 0)
        + entry_bytes("TRAILER!!!", 0)
        + entries
            .iter()
            .map(|(name, (_, data))| entry_bytes(name, data.len()))
            .sum::<usize>();
    if capacity > MAX_IMAGE_BYTES {
        return Err("unikernel image exceeds bound".into());
    }
    let mut out = Vec::with_capacity(capacity);
    newc(&mut out, 1, ".", 0o40755, &[]);
    for (i, (name, (mode, data))) in entries.iter().enumerate() {
        newc(&mut out, i as u32 + 2, name, *mode, data);
    }
    newc(&mut out, entries.len() as u32 + 2, "TRAILER!!!", 0, &[]);
    Ok(out)
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct GuestResult {
    schema: String,
    exit_code: i32,
}
pub fn serial_result(bytes: &[u8], truncated: bool) -> Result<i32, String> {
    if truncated {
        return Err("truncated unikernel serial result".into());
    }
    let mut found = None;
    let prefix = RESULT_PREFIX.as_bytes();
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        if line.windows(prefix.len()).any(|window| window == prefix) {
            if found.is_some() || !line.ends_with(b"\n") {
                return Err("duplicate or incomplete unikernel result".into());
            }
            let payload = line
                .strip_prefix(prefix)
                .ok_or("malformed unikernel result prefix")?;
            let payload = payload
                .strip_suffix(b"\n")
                .ok_or("incomplete unikernel result")?;
            let payload = payload.strip_suffix(b"\r").unwrap_or(payload);
            let result: GuestResult =
                serde_json::from_slice(payload).map_err(|_| "malformed unikernel result")?;
            if result.schema != "kiln.unikernel.result.v1" || !(0..=255).contains(&result.exit_code)
            {
                return Err("unsupported unikernel result schema or exit code".into());
            }
            found = Some(result.exit_code);
        }
    }
    found.ok_or_else(|| "missing unikernel result".into())
}

fn remaining(deadline: Instant, now: Instant) -> Result<Duration, String> {
    deadline
        .checked_duration_since(now)
        .filter(|left| left.as_millis() > 0)
        .ok_or_else(|| "unikernel job deadline exhausted".to_owned())
}

fn capture_limit(output_budget: usize) -> Result<usize, String> {
    const MINIMUM_OUTPUT_BUDGET: usize = 1024;
    const RECEIPT_OVERHEAD: usize = 512;
    const MAX_JSON_EXPANSION_TWO_STREAMS: usize = 12;
    if output_budget < MINIMUM_OUTPUT_BUDGET {
        return Err("unikernel output budget exhausted".into());
    }
    Ok((output_budget - RECEIPT_OVERHEAD) / MAX_JSON_EXPANSION_TWO_STREAMS)
}

fn bound_observation(mut observation: StepObservation, output_budget: usize) -> StepObservation {
    if observation.value.to_string().len() > output_budget {
        observation.success = false;
        observation.value = json!({"name":"test","state":if observation.cleaned {"failed"} else {"unknown"},"reason":"output_budget_exhausted","cleaned":observation.cleaned,"stdout_truncated":true,"stderr_truncated":true});
    }
    observation
}

fn validate_preboot_filter(properties: &str) -> Result<(), String> {
    let filter = properties
        .lines()
        .find_map(|line| line.strip_prefix("SystemCallFilter="))
        .ok_or("native preboot syscall filter is unavailable")?;
    let errno = properties
        .lines()
        .find_map(|line| line.strip_prefix("SystemCallErrorNumber="))
        .ok_or("native preboot syscall errno is unavailable")?;
    if errno != "1"
        || !filter
            .split_ascii_whitespace()
            .eq(PREBOOT_SYSCALLS.split_ascii_whitespace())
    {
        return Err("native preboot syscall policy differs from reviewed allowlist".into());
    }
    Ok(())
}

fn observe_preboot_policy(config: &Config, unit: &str, deadline: Instant) -> Result<Value, String> {
    let output = crate::execution::systemctl_until(
        config,
        &[
            "show",
            unit,
            "--property=SystemCallFilter",
            "--property=SystemCallErrorNumber",
        ],
        deadline,
    )?;
    if output.completion != bounded_exec::Completion::Exited
        || output.exit_code != Some(0)
        || output.stdout.truncated
    {
        return Err("native preboot syscall policy observation failed".into());
    }
    let properties = std::str::from_utf8(&output.stdout.bytes)
        .map_err(|_| "native preboot syscall policy is malformed")?;
    validate_preboot_filter(properties)?;
    let manager = crate::execution::systemctl_until(
        config,
        &["show", "--property=Version", "--property=Features"],
        deadline,
    )?;
    if manager.completion != bounded_exec::Completion::Exited
        || manager.exit_code != Some(0)
        || manager.stdout.truncated
    {
        return Err("native preboot syscall manager cohort is unavailable".into());
    }
    let manager = std::str::from_utf8(&manager.stdout.bytes)
        .map_err(|_| "native preboot manager cohort is malformed")?;
    let version = manager
        .lines()
        .find_map(|line| line.strip_prefix("Version="))
        .filter(|value| !value.is_empty())
        .ok_or("native preboot manager version missing")?;
    let features = manager
        .lines()
        .find_map(|line| line.strip_prefix("Features="))
        .filter(|value| {
            value
                .split_ascii_whitespace()
                .any(|feature| feature == "+SECCOMP")
        })
        .ok_or("native preboot manager seccomp support missing")?;
    Ok(
        json!({"ok":true,"policy":PREBOOT_POLICY,"syscalls_blake3":blake3::hash(PREBOOT_SYSCALLS.trim().as_bytes()).to_hex().to_string(),"source_blake3":blake3::hash(PREBOOT_POLICY_SOURCE).to_hex().to_string(),"errno":1,"manager_version":version,"manager_features":features,"phase":"before_instance_start"}),
    )
}

pub(crate) struct Job<'a> {
    pub workspace: &'a Path,
    pub spec: &'a TestSpec,
    pub unit: &'a str,
    pub deadline: Instant,
    pub output_budget: usize,
    pub cancellation: &'a AtomicBool,
    pub placement: Option<(&'a rusqlite::Connection, &'a str, &'a str, &'a str)>,
}

pub(crate) fn run_job(
    config: &Config,
    backend: &Backend,
    job: Job<'_>,
) -> Result<(StepObservation, Value), String> {
    let Job {
        workspace,
        spec,
        unit,
        deadline,
        output_budget,
        cancellation,
        placement,
    } = job;
    let Backend::Unikernel {
        firecracker,
        kernel,
        project,
        version,
        vcpus,
        memory_mib,
        ..
    } = backend
    else {
        return Err("unikernel backend required".into());
    };
    remaining(deadline, Instant::now())?;
    let output_limit = capture_limit(output_budget)?;
    let image = build_image(workspace, spec)?;
    let image_blake3 = blake3::hash(&image).to_hex().to_string();
    let root = tempfile::Builder::new()
        .prefix("kiln-unikernel-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    let initrd = root.path().join("test.cpio");
    std::fs::write(&initrd, image).map_err(|e| e.to_string())?;
    remaining(deadline, Instant::now())?;
    let socket = root.path().join("api.sock");
    let kernel_blake3 = kernel.blake3.clone();
    let executable = firecracker.verify()?;
    let kernel = kernel.verify()?;
    let mut boot_args = String::with_capacity(MAX_ARG_BYTES + 64);
    boot_args.push_str("elfloader /test");
    for arg in &spec.argv {
        boot_args.push(' ');
        boot_args.push_str(arg);
    }
    boot_args.push(' ');
    boot_args.push_str(ARGV_END);
    let host = crate::microvm::read_host_facts();
    let mut placement_started = false;
    let started = Instant::now();
    let stop = AtomicBool::new(false);
    let mut confinement = json!({"ok":false});
    let mut preboot_seccomp = json!({"ok":false,"policy":PREBOOT_POLICY});
    let mut early_cleanup = None;
    let execution = std::thread::scope(|scope| -> Result<_, String> {
        let timeout_ms =
            u64::try_from(remaining(deadline, Instant::now())?.as_millis()).unwrap_or(u64::MAX);
        let vmm = crate::microvm::spawn_vmm(
            scope,
            config,
            unit,
            crate::microvm::VmmBudget {
                timeout_ms,
                capture_bytes: output_limit,
                timeout_margin_ms: 0,
                preboot_syscalls: Some(PREBOOT_SYSCALLS),
            },
            None,
            vec![
                executable.to_string_lossy().into_owned(),
                "--api-sock".into(),
                socket.to_string_lossy().into_owned(),
            ],
            &stop,
        )?;
        let result: Result<(), String> = (|| {
            crate::microvm::api_until(
                &socket,
                "PUT",
                "/machine-config",
                &json!({"vcpu_count":vcpus,"mem_size_mib":memory_mib,"smt":false}),
                &vmm,
                deadline,
            )?;
            crate::microvm::api_until(
                &socket,
                "PUT",
                "/boot-source",
                &json!({"kernel_image_path":kernel,"initrd_path":initrd,"boot_args":boot_args}),
                &vmm,
                deadline,
            )?;
            preboot_seccomp = observe_preboot_policy(config, unit, deadline)?;
            let observed = crate::microvm::observe_confinement(
                config,
                unit,
                "firecracker",
                &executable,
                &firecracker.blake3,
                false,
                Duration::from_secs(5).min(remaining(deadline, Instant::now())?),
            );
            confinement = crate::microvm::confinement_value(&observed);
            let observed = observed?;
            if let Some((db, org, attempt, job)) = placement {
                crate::placement::begin(db, org, attempt, job, observed.pid)?;
                placement_started = true;
            }
            if cancellation.load(Ordering::Acquire) {
                return Err("unikernel cancelled before dispatch".into());
            }
            remaining(deadline, Instant::now())?;
            crate::microvm::api_until(
                &socket,
                "PUT",
                "/actions",
                &json!({"action_type":"InstanceStart"}),
                &vmm,
                deadline,
            )?;
            while !vmm.is_finished() {
                if cancellation.load(Ordering::Acquire) {
                    stop.store(true, Ordering::Release);
                }
                let left = remaining(deadline, Instant::now())?;
                std::thread::sleep(Duration::from_millis(10).min(left));
            }
            remaining(deadline, Instant::now())?;
            Ok(())
        })();
        if result.is_err() {
            // Service stdio outlives systemd-run; stop its owner before joining capture workers.
            early_cleanup = Some(crate::execution::cleanup_unit(config, unit));
            stop.store(true, Ordering::Release);
        }
        let output = vmm.join().map_err(|_| "unikernel VMM thread failed")?;
        Ok((result, output))
    });
    let cleaned = early_cleanup.unwrap_or_else(|| crate::execution::cleanup_unit(config, unit));
    if !cleaned {
        let _retained = root.keep();
    }
    let overlap = if placement_started {
        match placement {
            Some((db, org, attempt, job)) => {
                crate::placement::finish(db, org, attempt, job, cleaned)
            }
            None => Err("native placement context disappeared".into()),
        }
    } else {
        Ok(json!([]))
    };
    let placement_error = overlap.as_ref().err().cloned();
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let mut vm = json!({"backend":"firecracker","guest_kind":"unikernel","unikernel":{"project":project,"version":version,"image_blake3":image_blake3,"kernel_blake3":kernel_blake3,"binary_blake3":spec.binary.blake3},"hostile_isolation_claim":false,"cold_boot_to_exit_ms":elapsed_ms,"confinement":confinement,"cleaned":cleaned,
        "host": &host,
        "preboot_seccomp": preboot_seccomp,
        "co_residency": overlap.unwrap_or_else(|error| json!({"error":error})),
        "host_admission": {"decision":"admitted","untrusted_capable":false,"kernel_policy_source":"issue_28_fixed_release_table","override_reason":null,"override_applied":false,"observed_refusal":host.untrusted_refusal(),"mptcp_exposure_reduced_partial":host.mptcp_exposure_reduced(),"hostile_isolation_claim":false}
    });
    if let (Some(fields), Ok(Value::Object(host_fields))) =
        (vm.as_object_mut(), serde_json::to_value(&host))
    {
        fields.extend(host_fields);
    }
    let (result, output) = match execution {
        Ok(execution) => execution,
        Err(error) => {
            return Ok((
                StepObservation {
                    success: false,
                    cancelled: cancellation.load(Ordering::Acquire),
                    cleaned,
                    value: json!({"name":"test","state":if cleaned {"failed"} else {"unknown"},"reason":error,"cleaned":cleaned}),
                },
                vm,
            ));
        }
    };
    let observation = match output {
        Ok(output) => {
            let guest = serial_result(&output.stdout.bytes, output.stdout.truncated);
            let extra = result.is_ok()
                && placement_error.is_none()
                && guest == Ok(0)
                && output.exit_code == Some(0)
                && !output.stdout.truncated
                && !output.stderr.truncated;
            crate::execution::step_observation(
                ProcessFacts {
                    name: "test",
                    exited: output.completion == bounded_exec::Completion::Exited,
                    timed_out: output.completion == bounded_exec::Completion::TimedOut,
                    cancelled: cancellation.load(Ordering::Acquire),
                    exit_code: guest.as_ref().ok().copied(),
                    signal: output.signal,
                    stdout: String::from_utf8_lossy(&output.stdout.bytes).into_owned(),
                    stderr: String::from_utf8_lossy(&output.stderr.bytes).into_owned(),
                    stdout_truncated: output.stdout.truncated,
                    stderr_truncated: output.stderr.truncated,
                    elapsed_ms,
                    cleaned,
                },
                extra,
                Some(
                    json!({"result_error":guest.err(),"dispatch_error":result.err(),"placement_error":placement_error}),
                ),
            )
        }
        Err(error) => StepObservation {
            success: false,
            cancelled: cancellation.load(Ordering::Acquire),
            cleaned,
            value: json!({"name":"test","state":if cleaned {"failed"} else {"unknown"},"reason":error.to_string(),"cleaned":cleaned}),
        },
    };
    Ok((bound_observation(observation, output_budget), vm))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_preboot_filter_requires_exact_reviewed_allowlist() {
        let exact = format!(
            "SystemCallFilter={}\nSystemCallErrorNumber=1\n",
            PREBOOT_SYSCALLS.trim()
        );
        assert!(validate_preboot_filter(&exact).is_ok());
        assert!(validate_preboot_filter("SystemCallErrorNumber=1\n").is_err());
        assert!(validate_preboot_filter(&exact.replace("accept ", "")).is_err());
        assert!(validate_preboot_filter(
            &exact.replace("SystemCallFilter=", "SystemCallFilter=ptrace ")
        )
        .is_err());
        assert!(validate_preboot_filter(
            &exact.replace("SystemCallErrorNumber=1", "SystemCallErrorNumber=0")
        )
        .is_err());
    }
    #[test]
    fn binary_output_does_not_change_framed_exit_status() {
        let success = b"\xff\x00test output\nKILN_RESULT:{\"schema\":\"kiln.unikernel.result.v1\",\"exit_code\":0}\n";
        let failure =
            b"\xfe\nKILN_RESULT:{\"schema\":\"kiln.unikernel.result.v1\",\"exit_code\":7}\r\n";
        assert_eq!(serial_result(success, false), Ok(0));
        assert_eq!(serial_result(failure, false), Ok(7));
        assert!(serial_result(b"KILN_RESULT:{\xff}\n", false).is_err());
        assert!(serial_result(&[success.as_slice(), failure.as_slice()].concat(), false).is_err());
        assert!(serial_result(success, true).is_err());
    }

    #[test]
    fn preparation_consumes_the_original_job_deadline() {
        let start = Instant::now();
        let deadline = start + Duration::from_millis(10);
        assert_eq!(
            remaining(deadline, start + Duration::from_millis(3)),
            Ok(Duration::from_millis(7))
        );
        assert!(remaining(deadline, deadline).is_err());
        assert!(remaining(deadline, deadline + Duration::from_millis(1)).is_err());
    }

    #[test]
    fn over_budget_output_cannot_report_success() {
        let observation = |output: String| StepObservation {
            success: true,
            cancelled: false,
            cleaned: true,
            value: json!({"name":"test","state":"succeeded","stdout":output}),
        };
        assert!(bound_observation(observation("small output".into()), 1024).success);
        let overflow = bound_observation(observation("x".repeat(4096)), 1024);
        assert!(!overflow.success);
        assert_eq!(
            overflow.value.get("reason"),
            Some(&json!("output_budget_exhausted"))
        );
        assert!(overflow.value.to_string().len() <= 1024);
        assert!(capture_limit(1023).is_err());
    }
    fn elf() -> Vec<u8> {
        let mut bytes = vec![0; 120];
        bytes
            .get_mut(..7)
            .unwrap_or_else(|| unreachable!("ELF identification missing"))
            .copy_from_slice(b"\x7fELF\x02\x01\x01");
        for (offset, value) in [
            (16, 3),
            (18, 62),
            (20, 1),
            (32, 64),
            (52, 64),
            (54, 56),
            (56, 1),
            (64, 1),
        ] {
            *bytes
                .get_mut(offset)
                .unwrap_or_else(|| unreachable!("ELF fixture byte {offset} missing")) = value;
        }
        bytes
    }
    fn spec(bytes: &[u8]) -> TestSpec {
        TestSpec {
            binary: FileSpec {
                path: "binary".into(),
                blake3: blake3::hash(bytes).to_hex().to_string(),
            },
            argv: vec!["pass".into()],
            fixtures: vec![],
        }
    }
    #[test]
    fn static_pie_accepts_and_rejects_interpreter_and_needed() {
        let bytes = elf();
        assert!(validate_elf(&bytes).is_ok());
        let mut interpreter = bytes.clone();
        *interpreter
            .get_mut(56)
            .unwrap_or_else(|| unreachable!("ELF program header count missing")) = 2;
        let mut header = [0; 56];
        *header
            .get_mut(0)
            .unwrap_or_else(|| unreachable!("ELF program header type missing")) = 3;
        interpreter.extend_from_slice(&header);
        assert!(validate_elf(&interpreter).is_err());
        let mut dynamic = bytes.clone();
        *dynamic
            .get_mut(56)
            .unwrap_or_else(|| unreachable!("ELF program header count missing")) = 2;
        let mut header = [0; 56];
        for (offset, value) in [(0, 2), (8, 176), (32, 32)] {
            *header
                .get_mut(offset)
                .unwrap_or_else(|| unreachable!("ELF dynamic header byte {offset} missing")) =
                value;
        }
        dynamic.extend_from_slice(&header);
        dynamic.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        dynamic.extend_from_slice(&[0; 16]);
        assert!(validate_elf(&dynamic).is_err());
        *dynamic
            .get_mut(176)
            .unwrap_or_else(|| unreachable!("ELF dynamic entry missing")) = 0;
        assert!(validate_elf(&dynamic).is_ok());
    }
    #[test]
    fn elf_architecture_and_truncation_boundaries() {
        for (offset, value) in [(4, 1), (5, 2), (16, 2), (18, 183), (54, 55), (56, 0)] {
            let mut bytes = elf();
            *bytes
                .get_mut(offset)
                .unwrap_or_else(|| unreachable!("ELF fixture byte {offset} missing")) = value;
            assert!(validate_elf(&bytes).is_err());
        }
        for length in [0, 6, 63, 119] {
            assert!(validate_elf(
                elf()
                    .get(..length)
                    .unwrap_or_else(|| unreachable!("ELF fixture shorter than {length}"))
            )
            .is_err());
        }
    }
    #[test]
    fn serial_requires_exactly_one_complete_valid_result() {
        let ok = b"boot\nKILN_RESULT:{\"schema\":\"kiln.unikernel.result.v1\",\"exit_code\":0}\n";
        assert_eq!(serial_result(ok, false), Ok(0));
        let failed = b"KILN_RESULT:{\"schema\":\"kiln.unikernel.result.v1\",\"exit_code\":7}\n";
        assert_eq!(serial_result(failed, false), Ok(7));
        assert!(serial_result(b"boot\n", false).is_err());
        assert!(serial_result(&[ok.as_slice(), ok.as_slice()].concat(), false).is_err());
        assert!(serial_result(b"KILN_RESULT:{bad}\n", false).is_err());
        assert!(serial_result(
            ok.get(..ok.len() - 1)
                .unwrap_or_else(|| unreachable!("result frame fixture missing")),
            false
        )
        .is_err());
        assert!(serial_result(ok, true).is_err());
        assert!(serial_result(
            b"KILN_RESULT:{\"schema\":\"other\",\"exit_code\":0}\n",
            false
        )
        .is_err());
    }
    #[test]
    fn arguments_cannot_inject_elfloader_parameters() {
        assert!(validate_spec(&spec(&elf())).is_ok());
        for arg in [
            "--", "--help", "a b", "a\nb", "'test'", "x=y", "foo--bar", "", ARGV_END,
        ] {
            let mut s = spec(&elf());
            s.argv = vec![arg.into()];
            assert!(validate_spec(&s).is_err());
        }
    }
    #[test]
    fn image_contains_test_and_fixtures_and_verifies_content() {
        let workspace = tempfile::tempdir().unwrap_or_else(|e| unreachable!("{e}"));
        let binary = elf();
        std::fs::write(workspace.path().join("binary"), &binary)
            .unwrap_or_else(|e| unreachable!("{e}"));
        std::fs::create_dir(workspace.path().join("data")).unwrap_or_else(|e| unreachable!("{e}"));
        std::fs::write(workspace.path().join("data/input"), b"fixture")
            .unwrap_or_else(|e| unreachable!("{e}"));
        let mut s = spec(&binary);
        s.fixtures.push(FileSpec {
            path: "data/input".into(),
            blake3: blake3::hash(b"fixture").to_hex().to_string(),
        });
        let image = build_image(workspace.path(), &s).unwrap_or_else(|e| unreachable!("{e}"));
        let mut offset = 0;
        let mut actual = BTreeMap::new();
        loop {
            let header = image
                .get(offset..offset + 110)
                .unwrap_or_else(|| unreachable!("archive header missing at {offset}"));
            let field = |start| {
                usize::from_str_radix(
                    std::str::from_utf8(
                        header
                            .get(start..start + 8)
                            .unwrap_or_else(|| unreachable!("archive field missing at {start}")),
                    )
                    .unwrap_or_else(|e| unreachable!("archive field encoding: {e}")),
                    16,
                )
                .unwrap_or_else(|e| unreachable!("archive field number: {e}"))
            };
            let size = field(54);
            let name_size = field(94);
            let mode = field(14);
            let name_length = name_size
                .checked_sub(1)
                .unwrap_or_else(|| unreachable!("archive name terminator missing"));
            let name = std::str::from_utf8(
                image
                    .get(offset + 110..offset + 110 + name_length)
                    .unwrap_or_else(|| unreachable!("archive name missing at {offset}")),
            )
            .unwrap_or_else(|e| unreachable!("archive name encoding: {e}"))
            .to_owned();
            offset = (offset + 110 + name_size + 3) & !3;
            let data = image
                .get(offset..offset + size)
                .unwrap_or_else(|| unreachable!("archive data missing at {offset}"))
                .to_vec();
            offset = (offset + size + 3) & !3;
            if name == "TRAILER!!!" {
                break;
            }
            actual.insert(name, (mode, data));
        }
        assert_eq!(actual.get("test"), Some(&(0o100755, binary)));
        assert_eq!(
            actual.get("data/input"),
            Some(&(0o100444, b"fixture".to_vec()))
        );
        assert_eq!(actual.get("data"), Some(&(0o40755, vec![])));
        std::fs::write(workspace.path().join("data/input"), b"changed")
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert!(build_image(workspace.path(), &s).is_err());
    }
    #[test]
    fn fixture_paths_and_symlinks_fail_closed() {
        for path in [
            "/absolute",
            "../escape",
            "a/../escape",
            "a//b",
            "test",
            "test/nested",
            "TRAILER!!!",
            "TRAILER!!!/nested",
        ] {
            let mut s = spec(&elf());
            s.fixtures.push(FileSpec {
                path: path.into(),
                blake3: "0".repeat(64),
            });
            assert!(validate_spec(&s).is_err());
        }
        let workspace = tempfile::tempdir().unwrap_or_else(|e| unreachable!("{e}"));
        std::os::unix::fs::symlink("/etc/passwd", workspace.path().join("binary"))
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert!(build_image(workspace.path(), &spec(&elf())).is_err());
    }
    #[test]
    #[ignore = "requires real KVM, pinned native kernel and user systemd; operator smoke only"]
    fn native_confinement_gate_smoke() {
        let read = |key: &str| {
            let path = std::env::var(key).unwrap_or_else(|e| unreachable!("{key}: {e}"));
            std::fs::read(path).unwrap_or_else(|e| unreachable!("{key}: {e}"))
        };
        let config: Config = serde_json::from_slice(&read("KILN_UNIKERNEL_SMOKE_CONFIG"))
            .unwrap_or_else(|e| unreachable!("{e}"));
        let spec: TestSpec = serde_json::from_slice(&read("KILN_UNIKERNEL_SMOKE_SPEC"))
            .unwrap_or_else(|e| unreachable!("{e}"));
        let workspace =
            std::env::var("KILN_UNIKERNEL_SMOKE_WORKSPACE").unwrap_or_else(|e| unreachable!("{e}"));
        let backend = config
            .runners
            .values()
            .find(|b| matches!(b, Backend::Unikernel { .. }))
            .unwrap_or_else(|| unreachable!("unikernel backend missing"));
        let unit = format!("kiln-unikernel-smoke-{}", std::process::id());
        let deadline = Instant::now() + Duration::from_millis(10_000);
        let (observed, vm) = run_job(
            &config,
            backend,
            Job {
                workspace: Path::new(&workspace),
                spec: &spec,
                unit: &unit,
                deadline,
                output_budget: config.maximum_output_bytes,
                cancellation: &AtomicBool::new(false),
                placement: None,
            },
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
        println!("{}", json!({"step":observed.value,"vm":vm}));
        if let Ok(expected) = std::env::var("KILN_UNIKERNEL_SMOKE_EXPECTED_REFUSAL") {
            assert!(!observed.success);
            assert!(observed.cleaned);
            assert_eq!(
                observed
                    .value
                    .pointer("/guest/dispatch_error")
                    .and_then(Value::as_str),
                Some(expected.as_str())
            );
            assert_eq!(
                vm.pointer("/preboot_seccomp/ok").and_then(Value::as_bool),
                Some(false)
            );
            let stdout = observed
                .value
                .get("stdout")
                .and_then(Value::as_str)
                .unwrap_or_else(|| unreachable!("native refusal stdout missing"));
            assert!(!stdout.contains(RESULT_PREFIX));
            assert_eq!(
                observed
                    .value
                    .get("stdout_truncated")
                    .and_then(Value::as_bool),
                Some(false)
            );
            return;
        }
        let expected_exit: Option<i32> = serde_json::from_str(
            &std::env::var("KILN_UNIKERNEL_SMOKE_EXPECTED_EXIT").unwrap_or_else(|_| "0".into()),
        )
        .unwrap_or_else(|e| unreachable!("expected exit code: {e}"));
        assert_eq!(observed.value.get("exit_code"), Some(&json!(expected_exit)));
        assert_eq!(observed.success, expected_exit == Some(0));
        assert!(observed.cleaned);
        assert_eq!(
            vm.pointer("/confinement/ok").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            vm.pointer("/confinement/seccomp").and_then(Value::as_u64),
            Some(2)
        );
        assert_eq!(
            vm.pointer("/preboot_seccomp/ok").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            vm.pointer("/preboot_seccomp/policy")
                .and_then(Value::as_str),
            Some(PREBOOT_POLICY)
        );
        assert_eq!(
            vm.get("hostile_isolation_claim").and_then(Value::as_bool),
            Some(false)
        );
    }
}
