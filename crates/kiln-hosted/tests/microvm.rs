//! Host journey for the Firecracker backend. One reviewed pipeline runs through
//! the real service on the namespace backend and on cold-boot and warm-restore
//! Firecracker runners; all receipts must carry the same canonical outcome.
//! The guest-escape suite then runs inside a microVM, and every VM's start
//! latency is printed as `KILN_MICROVM_EVIDENCE` lines.
//!
//! Prerequisites: `/dev/kvm`, a systemd user manager with cgroup delegation,
//! Nix-store `systemd-run`/`systemctl`/`env`/`unshare`/`python3`/`bash` on
//! `PATH`, and `KILN_MICROVM_GUEST` pointing at `nix build .#microvm-guest`.
//! Without them the test prints an explicit `SKIP` reason and runs no VM; set
//! `KILN_MICROVM_REQUIRED=1` to turn a missing prerequisite into a failure.
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use kiln_hosted::{
    Request, Response, SCHEMA,
    execution::Config,
    service::{Profile, Service},
    transport::{self, NetworkProfile},
};
use serde_json::{Value, json};

const ORG: &str = "microvm-org";
const PROJECT: &str = "alpha";
const WORKSPACE: &str = "checkout";

fn fail(context: &str, error: impl std::fmt::Debug) -> ! {
    unreachable!("{context}: {error:?}")
}

struct Host {
    guest: Value,
    guest_ch: Option<Value>,
    tools: Value,
    runtime: PathBuf,
}

fn store_executable(path: &Path) -> Result<Value, String> {
    // Follow symlinks only until the first Nix-store path, keeping its basename
    // for multicall binaries; hash the final file like the service does.
    let mut current = path.to_path_buf();
    for _ in 0..16 {
        if current.starts_with("/nix/store") {
            let bytes = fs::read(&current).map_err(|e| format!("{}: {e}", current.display()))?;
            return Ok(
                json!({"path": current, "blake3": blake3::hash(&bytes).to_hex().to_string()}),
            );
        }
        let target = fs::read_link(&current)
            .map_err(|_| format!("{} is not a Nix-store executable", path.display()))?;
        current = current
            .parent()
            .map(|parent| parent.join(&target))
            .unwrap_or(target);
    }
    Err(format!("{} symlink chain too long", path.display()))
}

fn tool(name: &str) -> Result<Value, String> {
    let path = std::env::var_os("PATH").ok_or("PATH unset")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| format!("{name} not on PATH"))
        .and_then(|found| store_executable(&found))
}

fn prerequisites() -> Result<Host, String> {
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/kvm")
        .map_err(|e| format!("/dev/kvm unavailable: {e}"))?;
    let guest = std::env::var_os("KILN_MICROVM_GUEST")
        .ok_or("KILN_MICROVM_GUEST unset; build it with `nix build .#microvm-guest`")?;
    let manifest: Value = serde_json::from_slice(
        &fs::read(Path::new(&guest).join("manifest.json"))
            .map_err(|e| format!("guest manifest unreadable: {e}"))?,
    )
    .map_err(|e| format!("guest manifest invalid: {e}"))?;
    let uid = fs::metadata("/proc/self")
        .map(|m| std::os::unix::fs::MetadataExt::uid(&m))
        .map_err(|e| e.to_string())?;
    let guest_ch = std::env::var_os("KILN_MICROVM_GUEST_CH")
        .map(|path| {
            let manifest = fs::read(Path::new(&path).join("manifest.json"))
                .map_err(|e| format!("CH guest manifest unreadable: {e}"))?;
            serde_json::from_slice(&manifest).map_err(|e| format!("CH guest manifest invalid: {e}"))
        })
        .transpose()?;
    let runtime = PathBuf::from(format!("/run/user/{uid}"));
    if !runtime.join("systemd").is_dir() {
        return Err(format!("no systemd user manager at {}", runtime.display()));
    }
    let unshare = tool("unshare")?;
    let mount_path =
        Path::new(at(&unshare, "/path").as_str().unwrap_or_default()).with_file_name("mount");
    let tools = json!({
        "systemd_run": tool("systemd-run")?,
        "systemctl": tool("systemctl")?,
        "env": tool("env")?,
        "unshare": unshare,
        "mount": store_executable(&mount_path)?,
        "python": tool("python3")?,
        "bash": tool("bash")?,
    });
    Ok(Host {
        guest: manifest,
        guest_ch,
        tools,
        runtime,
    })
}

fn at(value: &Value, pointer: &str) -> Value {
    value.pointer(pointer).cloned().unwrap_or(Value::Null)
}

fn private_directory(path: &Path) -> PathBuf {
    fs::create_dir_all(path).unwrap_or_else(|e| fail("directory", e));
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .unwrap_or_else(|e| fail("permissions", e));
    path.to_path_buf()
}

fn request(id: &str, action: &str, input: Value) -> Request {
    Request {
        schema: SCHEMA.into(),
        organization: ORG.into(),
        request_id: id.into(),
        action: action.into(),
        input,
    }
}

fn call(service: &mut Service, peer: &str, id: &str, action: &str, input: Value) -> Value {
    let response: Response = service.handle(peer, request(id, action, input));
    assert!(
        response.error.is_none(),
        "{action} rejected: {:?}",
        response.error
    );
    response.result.unwrap_or(Value::Null)
}

/// One isolated hosted service whose only runner label `linux` maps to `backend`.
struct Deployment {
    directory: tempfile::TempDir,
    owner: String,
    profile: Profile,
    plan: String,
    price: String,
}

impl Deployment {
    fn new(host: &Host, backend: Value, pipeline: &Value) -> Self {
        let directory = tempfile::tempdir().unwrap_or_else(|e| fail("tempdir", e));
        private_directory(directory.path());
        let key = |name: &str| {
            let key_file = directory.path().join(format!("{name}.key"));
            let peer = transport::keygen(&key_file).unwrap_or_else(|e| fail("keygen", e));
            (
                peer,
                NetworkProfile {
                    key_file,
                    bind_address: "127.0.0.1:0".parse().unwrap_or_else(|e| fail("address", e)),
                    relay_urls: Vec::new(),
                },
            )
        };
        let (owner, _) = key("owner");
        let (_, network) = key("server");
        let mut profile = Profile {
            schema: SCHEMA.into(),
            service_id: "microvm-service".into(),
            state_directory: directory.path().join("state"),
            network,
            bootstrap_peer: owner.clone(),
            bootstrap_organization: ORG.into(),
            operator_peers: vec![owner.clone()],
            source_profiles: Vec::new(),
            operations: None,
            secrets: None,
            execution: None,
            fleet: None,
        };
        let (plan, price) = {
            let mut service =
                Service::open(profile.clone()).unwrap_or_else(|e| fail("service open", e));
            call(
                &mut service,
                &owner,
                "account",
                "finance.account.configure",
                json!({
                    "request_id": "account", "expected_revision": 0, "currency": "usd",
                    "cap_minor": 100_000, "request_max_minor": 100_000, "max_concurrent": 2
                }),
            );
            let price = call(
                &mut service,
                &owner,
                "price",
                "finance.price.admit",
                json!({
                    "class": "linux", "currency": "usd", "unit_minor": 1, "unit_ms": 1000, "max_units": 1000
                }),
            );
            let price = at(&price, "/price_id")
                .as_str()
                .unwrap_or_default()
                .to_owned();
            let plan = call(
                &mut service,
                &owner,
                "review",
                "pipelines.review",
                json!({
                    "project": PROJECT, "name": "checks", "expected_revision": 0, "config": pipeline
                }),
            );
            let plan = at(&plan, "/identity")
                .as_str()
                .unwrap_or_default()
                .to_owned();
            (plan, price)
        };
        let work = private_directory(&directory.path().join("work"));
        let checkout = work.join(ORG).join(PROJECT).join(WORKSPACE);
        fs::create_dir_all(checkout.join("sub")).unwrap_or_else(|e| fail("checkout", e));
        fs::write(checkout.join("README"), "hello from the reviewed source\n")
            .unwrap_or_else(|e| fail("source", e));
        fs::write(checkout.join("sub/data.txt"), "nested-data\n")
            .unwrap_or_else(|e| fail("source", e));
        std::os::unix::fs::symlink("sub/data.txt", checkout.join("link"))
            .unwrap_or_else(|e| fail("source", e));
        let tools = &host.tools;
        let execution: Config = serde_json::from_value(json!({
            "systemd_run": at(tools, "/systemd_run"), "systemctl": at(tools, "/systemctl"), "env": at(tools, "/env"),
            "manager_runtime_directory": host.runtime, "work_root": work,
            "trusted_projects": [{"organization": ORG, "project": PROJECT}],
            "pricing": [{"organization": ORG, "runner": "linux", "price_id": price, "class": "linux", "max_units": 1000}],
            "runners": {"linux": backend},
            "maximum_concurrent": 2, "memory_bytes": 1_073_741_824_u64, "processes": 64,
            "cpu_percent": 200, "maximum_timeout_ms": 120_000, "maximum_output_bytes": 262_144,
        }))
        .unwrap_or_else(|e| fail("execution profile", e));
        profile.execution = Some(execution);
        Self {
            directory,
            owner,
            profile,
            plan,
            price,
        }
    }

    fn run(&self, attempt: &str) -> Value {
        let mut service =
            Service::open(self.profile.clone()).unwrap_or_else(|e| fail("service open", e));
        call(
            &mut service,
            &self.owner,
            attempt,
            "execution.run",
            json!({
                "project": PROJECT, "pipeline_id": self.plan, "attempt_id": attempt,
                "price_id": self.price, "class": "linux", "workspace": WORKSPACE, "trust": "trusted"
            }),
        )
    }

    fn vm_rows(&self) -> Vec<(String, String)> {
        let db = rusqlite::Connection::open(self.directory.path().join("state/hosted.sqlite"))
            .unwrap_or_else(|e| fail("database", e));
        let mut statement = db
            .prepare("SELECT attempt||'/'||job, lifecycle FROM hosted_microvm_job ORDER BY 1")
            .unwrap_or_else(|e| fail("query", e));
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap_or_else(|e| fail("query", e))
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_else(|e| fail("rows", e))
    }
}

fn cache_pipeline() -> Value {
    json!({
        "schema": "kiln.hosted.pipeline.v1", "name": "Cache", "triggers": ["manual"],
        "jobs": {
            "build": {"runner": "linux", "timeout_ms": 60_000, "steps": [
                {"kind": "run", "name": "cache", "run": "if [ -f target/value ]; then cat target/value; else mkdir -p target; printf 'cached\n' > target/value; printf 'created\n'; fi"}
            ]}
        }
    })
}

fn equivalence_pipeline() -> Value {
    json!({
        "schema": "kiln.hosted.pipeline.v1", "name": "Equivalence", "triggers": ["manual"],
        "jobs": {
            "build": {"runner": "linux", "timeout_ms": 60_000, "steps": [
                {"kind": "run", "name": "read-source", "run": "read -r line < README; printf 'readme=%s\\n' \"$line\"; printf 'data=%s\\n' \"$(<sub/data.txt)\"; [[ -L link ]]; printf 'link=%s\\n' \"$(<link)\""},
                {"kind": "run", "name": "environment", "working_directory": "sub", "env": {"CI": "kiln"},
                 "run": "printf 'home=%s tmp=%s lang=%s ci=%s pwd=%s\\n' \"$HOME\" \"$TMPDIR\" \"$LANG\" \"$CI\" \"${PWD#/work}\""},
                {"kind": "run", "name": "scratch", "run": "printf 'scratch\\n' > \"$TMPDIR/out\"; printf 'tmp=%s\\n' \"$(<\"$TMPDIR/out\")\""}
            ]},
            "check": {"runner": "linux", "needs": ["build"], "timeout_ms": 60_000, "steps": [
                {"kind": "run", "name": "fails", "run": "printf 'before-failure\\n'; exit 3"},
                {"kind": "run", "name": "never", "run": "printf 'unreachable\\n'"}
            ]},
            "after": {"runner": "linux", "needs": ["check"], "timeout_ms": 60_000, "steps": [
                {"kind": "run", "name": "skipped", "run": "true"}
            ]}
        }
    })
}

/// Each probe must find its tool, attempt the escape, and print `denied`.
///
// r[verify kiln.hosted.microvm.network.nongoal.scenario.escape]
/// The guide names these TCP targets (`1.1.1.1:443`, `10.0.2.2:80`,
/// `169.254.169.254:80`) and vsock ports as the check coupled to any
/// guest-networking change.
fn escape_pipeline(host_boot_id: &str, host_name: &str, host_path: &str) -> Value {
    let namespaces = r#"command -v unshare >/dev/null; [[ "$(id -u)" == 1000 ]]
for flags in -U -m -n "-p -f" -i -u; do
  if unshare $flags true 2>"$TMPDIR/err"; then printf 'ESCAPED unshare %s\n' "$flags"; exit 1; fi
  grep -Eq 'not permitted|No space left' "$TMPDIR/err"
  printf 'denied unshare %s: %s\n' "$flags" "$(<"$TMPDIR/err")"
done
if mount -t proc proc "$TMPDIR" 2>"$TMPDIR/err"; then echo 'ESCAPED mount'; exit 1; fi
grep -Eqi 'not permitted|denied' "$TMPDIR/err"
printf 'denied mount: %s\n' "$(<"$TMPDIR/err")""#;
    let host_state = r#"pids=(/proc/[0-9]*); [[ ${#pids[@]} -lt 64 ]]; printf 'denied host processes: %s guest pids, pid1=%s\n' "${#pids[@]}" "$(</proc/1/comm)"
[[ "$(</proc/1/comm)" != systemd ]]
[[ "$(</proc/sys/kernel/random/boot_id)" != "$HOST_BOOT_ID" ]]; echo 'denied host boot_id: guest kernel has its own'
[[ "$(</proc/sys/kernel/hostname)" != "$HOST_NAME" ]]; printf 'denied host hostname: guest=%s\n' "$(</proc/sys/kernel/hostname)"
[[ ! -e "$HOST_PATH" ]]; echo 'denied host filesystem: host work_root absent in guest'
grep -q hypervisor /proc/cpuinfo; echo 'guest cpu reports hypervisor'
[[ "$(ls /sys/class/net)" == lo ]]; echo 'denied host network devices: sysfs lists only lo'
[[ "$(ls /sys/block)" == vda ]]; echo 'denied host block devices: sysfs lists only the source drive'
[[ ! -e /sys/fs/cgroup/cgroup.procs ]]; echo 'denied host cgroups: no cgroupfs'
[[ "$(</proc/sys/kernel/modules_disabled)" == 1 ]]; echo 'denied module loading: modules_disabled=1'
if dmesg >/dev/null 2>&1; then echo 'ESCAPED dmesg'; exit 1; fi; echo 'denied kernel log: dmesg_restrict'"#;
    let network = r#"command -v kiln-microvm-guest >/dev/null
[[ "$(ls /sys/class/net)" == lo ]]
for target in 1.1.1.1:443 10.0.2.2:80 169.254.169.254:80; do kiln-microvm-guest probe tcp "$target"; done
if (exec 3<>/dev/tcp/1.1.1.1/443) 2>/dev/null; then echo 'ESCAPED /dev/tcp'; exit 1; fi; echo 'denied bash /dev/tcp 1.1.1.1:443'
for port in 10240 1024 22; do kiln-microvm-guest probe vsock 2 "$port"; done
kiln-microvm-guest probe vsock 1 10240"#;
    json!({
        "schema": "kiln.hosted.pipeline.v1", "name": "Escape", "triggers": ["manual"],
        "jobs": {"escape": {"runner": "linux", "timeout_ms": 60_000,
            "env": {"HOST_BOOT_ID": host_boot_id, "HOST_NAME": host_name, "HOST_PATH": host_path},
            "steps": [
                {"kind": "run", "name": "namespace-creation", "run": namespaces},
                {"kind": "run", "name": "proc-sys-probing", "run": host_state},
                {"kind": "run", "name": "network-egress", "run": network}
            ]}}
    })
}

fn firecracker(host: &Host, pool: Option<&Path>) -> Value {
    let guest = &host.guest;
    json!({
        "kind": "firecracker", "firecracker": at(guest, "/firecracker"), "kernel": at(guest, "/kernel"),
        "initrd": at(guest, "/initrd"), "guest_shell": at(guest, "/guest_shell"), "vcpus": 1,
        "memory_mib": 256, "boot_timeout_ms": 20_000, "source_image_mib": 16,
        "warm_pool": pool.map(|directory| json!({"directory": directory, "slots": 1})),
        "qualification_identity": null,
    })
}

fn cloud_hypervisor(host: &Host, pool: Option<&Path>) -> Value {
    let guest = host.guest_ch.as_ref().unwrap_or(&Value::Null);
    json!({
        "kind": "cloud-hypervisor", "cloud_hypervisor": at(guest, "/cloud-hypervisor"),
        "kernel": at(guest, "/kernel"), "initrd": at(guest, "/initrd"),
        "guest_shell": at(guest, "/guest_shell"), "vcpus": 1, "memory_mib": 256,
        "boot_timeout_ms": 20_000, "source_image_mib": 16,
        "qualification_identity": null,
        "warm_pool": pool.map(|directory| json!({"directory": directory, "slots": 1})),
    })
}

fn vm_observations(receipt: &Value) -> Vec<Value> {
    at(receipt, "/jobs")
        .as_array()
        .map(|jobs| {
            jobs.iter()
                .filter_map(|job| job.get("microvm").cloned())
                .collect()
        })
        .unwrap_or_default()
}

fn job<'a>(receipt: &'a Value, id: &str) -> &'a Value {
    receipt
        .get("jobs")
        .and_then(Value::as_array)
        .and_then(|jobs| {
            jobs.iter()
                .find(|job| at(job, "/job") == format!("{id}[0]"))
        })
        .unwrap_or(&Value::Null)
}

fn assert_equivalence_receipt(label: &str, receipt: &Value) {
    assert_eq!(at(receipt, "/state"), "failed", "{label}: {receipt:#}");
    assert_eq!(at(receipt, "/hostile_isolation_claim"), false);
    let build = job(receipt, "build");
    assert_eq!(at(build, "/state"), "succeeded", "{label}: {build:#}");
    if matches!(
        at(build, "/microvm/backend").as_str(),
        Some("firecracker" | "cloud-hypervisor")
    ) {
        assert_eq!(
            at(build, "/microvm/confinement/ok"),
            true,
            "{label}: {build:#}"
        );
        let backend = at(build, "/microvm/backend");
        let expected_name = if backend == "cloud-hypervisor" {
            json!("cloud-hyperviso")
        } else {
            backend.clone()
        };
        assert_eq!(at(build, "/microvm/confinement/name"), expected_name);
        assert_eq!(
            at(build, "/microvm/confinement/cap_eff"),
            "0000000000000000"
        );
        assert_eq!(at(build, "/microvm/confinement/no_new_privs"), 1);
        if backend == "cloud-hypervisor" {
            assert!(at(build, "/microvm/confinement/seccomp_filtered") != json!(0));
        } else {
            assert_eq!(at(build, "/microvm/confinement/seccomp"), 2);
        }
        assert!(
            at(build, "/microvm/confinement/exe_blake3")
                .as_str()
                .is_some_and(|value| value.len() == 64),
            "{label}: {build:#}"
        );
    }
    assert_eq!(
        at(build, "/steps/0/stdout"),
        "readme=hello from the reviewed source\ndata=nested-data\nlink=nested-data\n",
        "{label}"
    );
    assert_eq!(
        at(build, "/steps/1/stdout"),
        "home=/tmp tmp=/tmp lang=C.UTF-8 ci=kiln pwd=/sub\n",
        "{label}"
    );
    let check = job(receipt, "check");
    assert_eq!(at(check, "/state"), "failed", "{label}");
    assert_eq!(at(check, "/steps/0/exit_code"), 3, "{label}");
    assert_eq!(
        at(check, "/steps").as_array().map(Vec::len),
        Some(1),
        "{label}"
    );
    assert_eq!(
        at(job(receipt, "after"), "/state"),
        "skipped_dependency",
        "{label}"
    );
}

fn evidence(label: &str, receipt: &Value) {
    for vm in vm_observations(receipt) {
        println!(
            "KILN_MICROVM_EVIDENCE {}",
            json!({"run": label, "boot": at(&vm, "/boot"), "warm_pool": at(&vm, "/warm_pool"), "snapshot": at(&vm, "/snapshot"),
                   "ready_ms": at(&vm, "/ready_ms"), "restore_api_ms": at(&vm, "/restore_api_ms"),
                   "snapshot_build_ms": at(&vm, "/snapshot_build_ms"), "session_ms": at(&vm, "/session_ms"),
                   "job_ms": at(&vm, "/job_ms"), "verify_ms": at(&vm, "/verify_ms"), "source_image_ms": at(&vm, "/source_image_ms"),
                   "destroyed": at(&vm, "/destroyed"), "journaled": at(&vm, "/journaled")})
        );
    }
}

// r[verify kiln.hosted.microvm.equivalence.scenario.host]
// r[verify kiln.hosted.microvm.escape]
// r[verify kiln.hosted.microvm.escape.scenario.suite]
// r[verify kiln.hosted.microvm.claims]
// r[verify kiln.hosted.microvm.warm_pool]
// r[verify kiln.hosted.microvm.lifecycle.scenario.journal]
#[test]
fn same_pipeline_matches_on_namespaces_and_firecracker_and_the_guest_denies_escape() {
    let host = match prerequisites() {
        Ok(host) => host,
        Err(reason) => {
            assert!(
                std::env::var_os("KILN_MICROVM_REQUIRED").is_none(),
                "microVM host test required but unavailable: {reason}"
            );
            eprintln!("SKIP microVM host test: {reason}");
            return;
        }
    };
    let pipeline = equivalence_pipeline();
    let namespaces = Deployment::new(
        &host,
        json!({"kind": "namespaces", "unshare": at(&host.tools, "/unshare"), "python": at(&host.tools, "/python"),
               "mount": at(&host.tools, "/mount"), "shell": at(&host.tools, "/bash"), "runtime_roots": ["/nix/store"]}),
        &pipeline,
    );
    let namespace_receipt = namespaces.run("namespaces-1");
    println!("KILN_MICROVM_RECEIPT namespaces {namespace_receipt}");
    assert_equivalence_receipt("namespaces", &namespace_receipt);
    let canonical = at(&namespace_receipt, "/canonical_outcome");
    assert!(canonical.as_str().is_some_and(|id| id.len() == 64));

    let cold = Deployment::new(&host, firecracker(&host, None), &pipeline);
    let pool_root = tempfile::Builder::new()
        .prefix("kiln-pool-")
        .tempdir_in("/tmp")
        .unwrap_or_else(|e| fail("pool", e));
    let pool = private_directory(&pool_root.path().join("p"));
    let warm = Deployment::new(&host, firecracker(&host, Some(&pool)), &pipeline);
    let mut runs = Vec::new();
    for attempt in ["cold-1", "cold-2"] {
        runs.push((attempt, cold.run(attempt)));
    }
    for attempt in ["warm-1", "warm-2", "warm-3"] {
        runs.push((attempt, warm.run(attempt)));
    }
    for (label, receipt) in &runs {
        println!("KILN_MICROVM_RECEIPT {label} {receipt}");
        assert_equivalence_receipt(label, receipt);
        assert_eq!(
            at(receipt, "/canonical_outcome"),
            canonical,
            "{label} canonical outcome"
        );
        let vms = vm_observations(receipt);
        assert_eq!(vms.len(), 2, "{label}: one VM per executed job");
        for vm in &vms {
            assert_eq!(at(vm, "/destroyed"), true, "{label}: {vm:#}");
            assert_eq!(at(vm, "/journaled"), true, "{label}: {vm:#}");
            assert_eq!(at(vm, "/network"), "none");
            // r[verify kiln.hosted.microvm.debug_hold.policy.scenario.disabled]
            assert!(vm.get("debug_hold").is_none(), "{label}: {vm:#}");
            let expected = if label.starts_with("cold") {
                "cold"
            } else {
                "restore"
            };
            assert_eq!(at(vm, "/boot"), expected, "{label}: {vm:#}");
        }
        evidence(label, receipt);
    }
    let first_warm = runs
        .iter()
        .find(|(label, _)| *label == "warm-1")
        .map(|(_, receipt)| vm_observations(receipt))
        .unwrap_or_default();
    assert_eq!(
        first_warm.first().map(|vm| at(vm, "/snapshot")),
        Some(json!("rebuilt"))
    );
    for rows in [cold.vm_rows(), warm.vm_rows()] {
        assert!(!rows.is_empty());
        assert!(
            rows.iter().all(|(_, lifecycle)| lifecycle == "destroyed"),
            "{rows:?}"
        );
    }

    let boot_id = fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap_or_default();
    let hostname = fs::read_to_string("/proc/sys/kernel/hostname").unwrap_or_default();
    // A host path that exists here (this deployment's private root) must be absent in the guest.
    let escape_path = cold.directory.path().to_string_lossy().into_owned();
    assert!(Path::new(&escape_path).is_dir());
    let escape = Deployment::new(
        &host,
        firecracker(&host, None),
        &escape_pipeline(boot_id.trim(), hostname.trim(), &escape_path),
    );
    let receipt = escape.run("escape-1");
    println!("KILN_MICROVM_RECEIPT escape {receipt}");
    let suite = job(&receipt, "escape");
    assert_eq!(at(&receipt, "/state"), "succeeded", "{receipt:#}");
    let steps = at(suite, "/steps");
    for step in steps.as_array().unwrap_or(&Vec::new()) {
        let output = at(step, "/stdout");
        let stdout = output.as_str().unwrap_or_default();
        assert!(
            !stdout.contains("ESCAPED") && !stdout.contains("connected"),
            "{stdout}"
        );
        assert!(stdout.contains("denied"), "{stdout}");
        println!(
            "KILN_MICROVM_ESCAPE {}",
            json!({"probe": at(step, "/name"), "state": at(step, "/state"), "stdout": stdout})
        );
    }
    evidence("escape", &receipt);
}

#[test]
fn cloud_hypervisor_cold_boot_matches_the_pipeline() {
    let host = match prerequisites() {
        Ok(host) => host,
        Err(reason) => {
            assert!(
                std::env::var_os("KILN_MICROVM_REQUIRED").is_none(),
                "microVM host test required but unavailable: {reason}"
            );
            eprintln!("SKIP Cloud Hypervisor host test: {reason}");
            return;
        }
    };
    if host.guest_ch.is_none() {
        assert!(
            std::env::var_os("KILN_MICROVM_REQUIRED").is_none(),
            "KILN_MICROVM_GUEST_CH unset; build .#microvm-guest-ch"
        );
        eprintln!("SKIP Cloud Hypervisor host test: KILN_MICROVM_GUEST_CH unset");
        return;
    }
    let pipeline = equivalence_pipeline();
    let ch = Deployment::new(&host, cloud_hypervisor(&host, None), &pipeline);
    let receipt = ch.run("ch-cold-1");
    println!("KILN_MICROVM_RECEIPT ch-cold {receipt}");
    assert_equivalence_receipt("ch-cold", &receipt);
    let pool_root = tempfile::Builder::new()
        .prefix("kiln-ch-pool-")
        .tempdir_in("/tmp")
        .unwrap_or_else(|e| fail("pool", e));
    let pool = private_directory(&pool_root.path().join("p"));
    let warm = Deployment::new(&host, cloud_hypervisor(&host, Some(&pool)), &pipeline);
    for attempt in ["ch-warm-1", "ch-warm-2"] {
        let receipt = warm.run(attempt);
        println!("KILN_MICROVM_RECEIPT {attempt} {receipt}");
        assert_equivalence_receipt(attempt, &receipt);
        for vm in vm_observations(&receipt) {
            assert_eq!(at(&vm, "/backend"), "cloud-hypervisor");
            if attempt == "ch-warm-2" {
                assert_eq!(at(&vm, "/boot"), "restore");
            }
            assert_eq!(at(&vm, "/network"), "none");
        }
    }

    let boot_id = fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap_or_default();
    let hostname = fs::read_to_string("/proc/sys/kernel/hostname").unwrap_or_default();
    let escape_path = ch.directory.path().to_string_lossy().into_owned();
    let escape = Deployment::new(
        &host,
        cloud_hypervisor(&host, None),
        &escape_pipeline(boot_id.trim(), hostname.trim(), &escape_path),
    );
    let receipt = escape.run("ch-escape-1");
    println!("KILN_MICROVM_RECEIPT ch-escape {receipt}");
    let suite = job(&receipt, "escape");
    assert_eq!(at(&receipt, "/state"), "succeeded", "{receipt:#}");
    for step in at(suite, "/steps").as_array().unwrap_or(&Vec::new()) {
        let stdout = at(step, "/stdout");
        let stdout = stdout.as_str().unwrap_or_default();
        assert!(
            !stdout.contains("ESCAPED") && !stdout.contains("connected"),
            "{stdout}"
        );
        assert!(stdout.contains("denied"), "{stdout}");
    }
}

/// The hold grace of the host test: long enough for one relayed command.
const HOLD_GRACE_SECONDS: u64 = 15;

fn held_pipeline() -> Value {
    json!({
        "schema": "kiln.hosted.pipeline.v1", "name": "Held", "triggers": ["manual"],
        "jobs": {"check": {"runner": "linux", "timeout_ms": 60_000, "steps": [
            {"kind": "run", "name": "fails", "run": "printf 'state-from-job\\n' > \"$TMPDIR/marker\"; printf 'failing\\n'; exit 7"}
        ]}}
    })
}

fn respond(service: &mut Service, peer: &str, id: &str, action: &str, input: Value) -> Response {
    service.handle(peer, request(id, action, input))
}

fn millis(since: std::time::Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

// r[verify kiln.hosted.microvm.debug_hold.relay]
// r[verify kiln.hosted.microvm.debug_hold.relay.scenario.host]
// r[verify kiln.hosted.microvm.debug_hold.capability.scenario.single_use]
// r[verify kiln.hosted.microvm.debug_hold.end.scenario.expiry]
// r[verify kiln.hosted.microvm.debug_hold.sessions]
#[test]
fn failed_job_is_held_debugged_once_over_vsock_and_destroyed_on_expiry() {
    let host = match prerequisites() {
        Ok(host) => host,
        Err(reason) => {
            assert!(
                std::env::var_os("KILN_MICROVM_REQUIRED").is_none(),
                "microVM host test required but unavailable: {reason}"
            );
            eprintln!("SKIP microVM debug hold test: {reason}");
            return;
        }
    };
    let mut backend = firecracker(&host, None);
    if let Some(fields) = backend.as_object_mut() {
        fields.insert(
            "debug_hold".into(),
            json!({"enabled": true, "grace_seconds": HOLD_GRACE_SECONDS, "max_concurrent": 1}),
        );
    }
    let deployment = Deployment::new(&host, backend, &held_pipeline());
    let mut service =
        Service::open(deployment.profile.clone()).unwrap_or_else(|e| fail("service open", e));
    let mut debugger = service.fork().unwrap_or_else(|e| fail("fork", e));
    let owner = deployment.owner.clone();
    let run_input = json!({
        "project": PROJECT, "pipeline_id": deployment.plan, "attempt_id": "held-1",
        "price_id": deployment.price, "class": "linux", "workspace": WORKSPACE, "trust": "trusted"
    });
    let started = std::time::Instant::now();
    let runner_owner = owner.clone();
    let run = std::thread::spawn(move || {
        call(
            &mut service,
            &runner_owner,
            "held-1",
            "execution.run",
            run_input,
        )
    });
    let target = json!({"project": PROJECT, "attempt_id": "held-1", "job": "check[0]"});
    let attempt = json!({"project": PROJECT, "attempt_id": "held-1"});
    // The attempt's log ends with how to connect once the VM is held.
    let mut polls = 0_u32;
    let connect = loop {
        polls += 1;
        let got = respond(
            &mut debugger,
            &owner,
            &format!("get-{polls}"),
            "execution.get",
            attempt.clone(),
        );
        if let Some(connect) = got
            .result
            .as_ref()
            .and_then(|r| r.pointer("/debug_holds/0/connect"))
            .and_then(Value::as_str)
        {
            break connect.to_owned();
        }
        assert!(!run.is_finished(), "run ended without a hold: {got:?}");
        assert!(started.elapsed().as_secs() < 90, "no hold observed");
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    let held_after_ms = millis(started);
    assert!(connect.contains("execution.debug_issue"), "{connect}");
    let journaled = || -> (String, String) {
        let db =
            rusqlite::Connection::open(deployment.directory.path().join("state/hosted.sqlite"))
                .unwrap_or_else(|e| fail("database", e));
        db.query_row(
            "SELECT lifecycle,result FROM hosted_microvm_job WHERE attempt='held-1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or_else(|e| fail("journal row", e))
    };
    let (lifecycle_while_held, result_while_held) = journaled();
    assert_eq!(lifecycle_while_held, "held");

    let issue_started = std::time::Instant::now();
    let issued = call(
        &mut debugger,
        &owner,
        "issue-1",
        "execution.debug_issue",
        target.clone(),
    );
    let issue_ms = millis(issue_started);
    let capability = at(&issued, "/capability")
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(capability.len(), 64);
    let mut exec_input = target.clone();
    if let Some(fields) = exec_input.as_object_mut() {
        fields.insert("capability".into(), json!(capability));
        fields.insert(
            "command".into(),
            json!("cat \"$TMPDIR/marker\"; id -u; ls /sys/class/net; pwd"),
        );
    }
    let exec_started = std::time::Instant::now();
    let session = call(
        &mut debugger,
        &owner,
        "exec-1",
        "execution.debug_exec",
        exec_input.clone(),
    );
    let exec_ms = millis(exec_started);
    println!("KILN_MICROVM_DEBUG_SESSION {session}");
    assert_eq!(at(&session, "/state"), "completed", "{session:#}");
    assert_eq!(at(&session, "/peer"), json!(owner));
    assert_eq!(at(&session, "/result/exit_code"), 0, "{session:#}");
    assert_eq!(
        at(&session, "/result/stdout"),
        "state-from-job\n1000\nlo\n/work\n",
        "{session:#}"
    );
    let second = respond(
        &mut debugger,
        &owner,
        "exec-2",
        "execution.debug_exec",
        exec_input,
    );
    assert_eq!(
        second.error.as_deref(),
        Some("debug capability already used"),
        "{second:?}"
    );

    let receipt = run
        .join()
        .unwrap_or_else(|_| fail("run thread", "panicked"));
    let total_ms = millis(started);
    println!("KILN_MICROVM_RECEIPT held {receipt}");
    let check = job(&receipt, "check");
    assert_eq!(at(check, "/state"), "failed");
    assert_eq!(at(check, "/steps/0/exit_code"), 7);
    let vm = at(check, "/microvm");
    assert_eq!(at(&vm, "/debug_hold/outcome"), "held", "{vm:#}");
    assert_eq!(at(&vm, "/debug_hold/end_reason"), "expired", "{vm:#}");
    assert_eq!(at(&vm, "/debug_hold/sessions"), 1);
    assert_eq!(at(&vm, "/destroyed"), true);
    assert_eq!(at(&vm, "/journaled"), true);
    assert_eq!(at(&vm, "/network"), "none");
    assert_eq!(at(&vm, "/lifecycle"), "Destroyed { journaled: true }");
    let held_ms = at(&vm, "/debug_hold/held_ms").as_u64().unwrap_or_default();
    assert!(
        held_ms >= HOLD_GRACE_SECONDS * 1000,
        "the hold lasts its grace: {held_ms}"
    );
    let (lifecycle_after, result_after) = journaled();
    assert_eq!(lifecycle_after, "destroyed");
    assert_eq!(
        result_after, result_while_held,
        "journaled result never changes"
    );
    assert_eq!(
        deployment.vm_rows(),
        [("held-1/check[0]".to_owned(), "destroyed".to_owned())]
    );
    let late = respond(
        &mut debugger,
        &owner,
        "issue-2",
        "execution.debug_issue",
        target,
    );
    assert_eq!(
        late.error.as_deref(),
        Some("no active debug hold for this job")
    );
    let listed = call(
        &mut debugger,
        &owner,
        "list-1",
        "execution.debug_list",
        attempt,
    );
    assert_eq!(at(&listed, "/holds/0/end_reason"), "expired");
    assert_eq!(at(&listed, "/sessions/0/state"), "completed");
    assert_eq!(at(&listed, "/authority"), "none");
    println!(
        "KILN_MICROVM_DEBUG_HOLD {}",
        json!({
            "grace_seconds": HOLD_GRACE_SECONDS, "held_after_ms": held_after_ms, "issue_ms": issue_ms,
            "exec_ms": exec_ms, "guest_command_ms": at(&session, "/result/elapsed_ms"),
            "session_duration_ms": at(&session, "/duration_ms"), "held_ms": held_ms,
            "ready_ms": at(&vm, "/ready_ms"), "session_ms": at(&vm, "/session_ms"),
            "job_ms": at(&vm, "/job_ms"), "attempt_total_ms": total_ms,
            "end_reason": at(&vm, "/debug_hold/end_reason"), "destroyed": at(&vm, "/destroyed"),
        })
    );
}

#[test]
fn firecracker_cache_restores_after_publication() {
    let host = match prerequisites() {
        Ok(host) => host,
        Err(reason) => {
            assert!(
                std::env::var_os("KILN_MICROVM_REQUIRED").is_none(),
                "microVM host test required but unavailable: {reason}"
            );
            eprintln!("SKIP Firecracker cache test: {reason}");
            return;
        }
    };
    let mut deployment = Deployment::new(&host, firecracker(&host, None), &cache_pipeline());
    let cache_root = private_directory(&deployment.directory.path().join("cache"));
    let cache = json!({
        "directory": cache_root, "maximum_entry_bytes": 1024, "maximum_total_bytes": 4096,
        "maximum_entries": 4, "maximum_age_seconds": 60, "maximum_files": 4,
        "policies": [{"organization": ORG, "project": PROJECT, "runner": "linux", "worker": {
            "producer": "hosted", "toolchain": "t", "platform": "linux", "inputs": ["README"],
            "paths": ["target"], "maximum_entry_bytes": 1024, "maximum_files": 4}}],
    });
    deployment
        .profile
        .execution
        .as_mut()
        .unwrap_or_else(|| unreachable!("execution profile"))
        .cache =
        Some(serde_json::from_value(cache).unwrap_or_else(|error| unreachable!("{error}")));
    let first = deployment.run("cache-1");
    println!("KILN_MICROVM_RECEIPT cache-1 {first}");
    assert_eq!(at(&first, "/state"), "succeeded", "{first:#}");
    assert_eq!(at(job(&first, "build"), "/steps/0/stdout"), "created\n");
    assert_eq!(
        at(job(&first, "build"), "/cache/publish/outcome"),
        "published"
    );
    assert_eq!(at(&first, "/cleaned"), true);
    let second = deployment.run("cache-2");
    println!("KILN_MICROVM_RECEIPT cache-2 {second}");
    assert_eq!(at(&second, "/state"), "succeeded", "{second:#}");
    assert_eq!(at(job(&second, "build"), "/steps/0/stdout"), "cached\n");
    assert_eq!(at(job(&second, "build"), "/cache/restore/outcome"), "hit");
    assert_eq!(at(&second, "/cleaned"), true);
}
