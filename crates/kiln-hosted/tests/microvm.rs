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
