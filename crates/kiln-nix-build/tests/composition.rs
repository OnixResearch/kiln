mod support;

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

#[cfg(feature = "nix-live")]
use kiln_adapter_nix::BoundedNixBuildRuntime;
use kiln_adapter_nix::{
    NixBuildProcessOutput, NixBuildRuntime, ProcessInvocation, gc_root_directory,
};
use kiln_core::discovery::{CacheAdvisory, DiscoveredOutput, TaskProposal};
use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId, TaskId};
use kiln_nix_build::{NixBuildState, build_task, plan_for_task, profile_for_task};
use kiln_provider_api::PortResult;
use support::{alias, base, content, intent};

const TASK_DRV: &str = "/nix/store/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-task.drv";
const OUT_PATH: &str = "/nix/store/cccccccccccccccccccccccccccccccc-task";

fn task() -> TaskProposal {
    TaskProposal {
        task_id: TaskId::new("task-a").unwrap_or_else(|_| unreachable!()),
        mapping_identity: content('7'),
        build_provider: ProviderProfileId::new("nix-build-v1").unwrap_or_else(|_| unreachable!()),
        native_attribute: ExternalRef::new("nix", "attribute", "packages.fixture")
            .unwrap_or_else(|_| unreachable!()),
        derivation: ExternalRef::new("nix", "derivation", TASK_DRV)
            .unwrap_or_else(|_| unreachable!()),
        outputs: vec![DiscoveredOutput {
            name: alias("out"),
            path: ExternalRef::new("nix", "store-path", OUT_PATH)
                .unwrap_or_else(|_| unreachable!()),
        }],
        dependencies: Vec::new(),
        cache: CacheAdvisory::NotBuilt,
    }
}

// r[verify kiln.nix_build.preflight] r[verify kiln.nix_build.preflight.scenario.failure]
#[test]
fn preflight_failure_stops_before_build() {
    let mut runtime = FakeRuntime::new();
    runtime.preflight_fails = true;
    assert!(build_task(&base(), &task(), &intent(), runtime).is_err());
}

// r[verify kiln.nix_build.state] r[verify kiln.nix_build.state.scenario.round_trip]
#[test]
fn durable_state_round_trips_bound_profile() {
    let plan = plan_for_task(&base(), &task(), &intent()).unwrap_or_else(|_| unreachable!());
    let state = NixBuildState::new_bound(support::config(), plan.clone());
    let temp = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let path = temp.path().join("state.json");
    state.store(&path).unwrap_or_else(|_| unreachable!());
    let loaded = NixBuildState::load(&path).unwrap_or_else(|_| unreachable!());
    assert_eq!(loaded.plan, plan);
    assert!(loaded.observation.is_none());
    let (profile, restored_plan) = loaded.rehydrate().unwrap_or_else(|_| unreachable!());
    assert_eq!(restored_plan, plan);
    assert_eq!(
        profile.runtime.expected_outputs,
        base().runtime.expected_outputs
    );
}

// r[verify kiln.nix_build.composition] r[verify kiln.nix_build.composition.scenario.mapping]
#[test]
fn task_maps_to_exact_profile_and_plan() {
    let profile = profile_for_task(&base(), &task(), &intent(), content('8'))
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(profile.runtime.derivation_path, PathBuf::from(TASK_DRV));
    assert_eq!(profile.runtime.derivation_identity, content('8'));
    assert_eq!(profile.runtime.plan_identity, content('3'));
    assert_eq!(profile.runtime.policy_identity, content('4'));
    assert_eq!(profile.runtime.expected_outputs.len(), 1);
    let output = profile
        .runtime
        .expected_outputs
        .first()
        .unwrap_or_else(|| unreachable!());
    assert_eq!(output.name.as_str(), "out");
    assert_eq!(output.path, PathBuf::from(OUT_PATH));
    let plan = plan_for_task(&base(), &task(), &intent()).unwrap_or_else(|_| unreachable!());
    assert_eq!(plan.required_products, vec![alias("out")]);
    assert_eq!(plan.provider_profile.as_str(), "nix-build-v1");
}

#[test]
fn task_drift_is_rejected() {
    let mut wrong_provider = task();
    wrong_provider.build_provider =
        ProviderProfileId::new("other-build-v1").unwrap_or_else(|_| unreachable!());
    assert!(profile_for_task(&base(), &wrong_provider, &intent(), content('8')).is_err());

    let mut wrong_kind = task();
    wrong_kind.derivation =
        ExternalRef::new("nix", "store-path", TASK_DRV).unwrap_or_else(|_| unreachable!());
    assert!(profile_for_task(&base(), &wrong_kind, &intent(), content('8')).is_err());

    let mut outside = task();
    outside.outputs = vec![DiscoveredOutput {
        name: alias("out"),
        path: ExternalRef::new("nix", "store-path", "/tmp/outside")
            .unwrap_or_else(|_| unreachable!()),
    }];
    assert!(profile_for_task(&base(), &outside, &intent(), content('8')).is_err());

    let mut wrong_platform = intent();
    wrong_platform.platform = alias("aarch64-linux");
    assert!(plan_for_task(&base(), &task(), &wrong_platform).is_err());
}

#[derive(Debug)]
struct FakeRuntime {
    derivation_identity: ContentId,
    preflight_fails: bool,
    run_count: usize,
    cleaned: Vec<PathBuf>,
}

impl FakeRuntime {
    fn new() -> Self {
        Self {
            derivation_identity: content('8'),
            preflight_fails: false,
            run_count: 0,
            cleaned: Vec::new(),
        }
    }
}

impl NixBuildRuntime for FakeRuntime {
    fn file_identity(&self, path: &Path, _max_bytes: u64) -> PortResult<ContentId> {
        if path == Path::new(kiln_adapter_nix::PINNED_NIX_EXECUTABLE_PATH)
            || path == Path::new(kiln_adapter_nix::PINNED_NIX_STORE_EXECUTABLE_PATH)
        {
            return ContentId::from_blake3_hex(kiln_adapter_nix::PINNED_NIX_EXECUTABLE_BLAKE3)
                .map_err(kiln_provider_api::PortFailure::Domain);
        }
        if path == Path::new(TASK_DRV) {
            return Ok(self.derivation_identity.clone());
        }
        Ok(content('9'))
    }

    fn prepare_gc_root_directory(&mut self, _path: &Path) -> PortResult<()> {
        Ok(())
    }

    fn cleanup_gc_root_directory(&mut self, path: &Path, _max_roots: usize) -> PortResult<()> {
        self.cleaned.push(path.to_path_buf());
        Ok(())
    }

    fn read_link(&self, _path: &Path) -> PortResult<PathBuf> {
        Ok(PathBuf::from(OUT_PATH))
    }

    fn run(&mut self, invocation: &ProcessInvocation) -> PortResult<NixBuildProcessOutput> {
        self.run_count = self.run_count.saturating_add(1);
        if invocation
            .args
            .iter()
            .any(|value| value.to_str() == Some("--dry-run"))
        {
            return Ok(NixBuildProcessOutput::Exited {
                process_status: Some(if self.preflight_fails { 1 } else { 0 }),
                stdout: Vec::new(),
                stderr: b"preflight fixture".to_vec(),
            });
        }
        let root = invocation
            .args
            .windows(2)
            .find(|pair| pair.first().and_then(|value| value.to_str()) == Some("--add-root"))
            .and_then(|pair| pair.get(1))
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        let mut stdout = root.as_bytes().to_vec();
        stdout.push(b'\n');
        Ok(NixBuildProcessOutput::Exited {
            process_status: Some(0),
            stdout,
            stderr: Vec::new(),
        })
    }

    fn run_with_cancellation(
        &mut self,
        invocation: &ProcessInvocation,
        _cancellation: &AtomicBool,
    ) -> PortResult<NixBuildProcessOutput> {
        self.run(invocation)
    }
}

// r[verify kiln.nix_build.composition] r[verify kiln.nix_build.retention]
// r[verify kiln.nix_build.preflight] r[verify kiln.nix_build.preflight.scenario.success]
// r[verify kiln.nix_build.composition.scenario.build] r[verify kiln.nix_build.retention.scenario.release]
#[test]
fn task_build_is_admitted_and_rooted() {
    let (mut adapter, plan, observation) =
        build_task(&base(), &task(), &intent(), FakeRuntime::new())
            .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        observation.outcome,
        kiln_core::build::BuildEvaluationOutcome::Success
    );
    assert_eq!(observation.products.len(), 1);
    assert_eq!(adapter.runtime().run_count, 2);
    adapter
        .release_output_roots(&plan.effect_id)
        .unwrap_or_else(|_| unreachable!());
    let root = gc_root_directory(adapter.profile_binding(), &plan.effect_id)
        .unwrap_or_else(|_| unreachable!());
    assert!(adapter.runtime().cleaned.iter().any(|path| path == &root));
}

#[cfg(feature = "nix-live")]
// r[verify kiln.nix_build.composition] r[verify kiln.nix_build.retention]
// r[verify kiln.nix_build.composition.scenario.e2e] r[verify kiln.nix_build.retention.scenario.release]
#[test]
fn live_task_build_is_admitted_and_rooted() {
    let temp = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let home = temp.path().join("home");
    std::fs::create_dir(&home).unwrap_or_else(|_| unreachable!());
    let expression = r#"derivation { name = "kiln-nix-task-live"; system = "x86_64-linux"; builder = "/bin/sh"; args = [ "-c" "echo task-live > $out" ]; }"#;
    let derivation_expression = format!("({expression}).drvPath");
    let evaluation = std::process::Command::new(kiln_adapter_nix::PINNED_NIX_EXECUTABLE_PATH)
        .args([
            "--extra-experimental-features",
            "nix-command",
            "eval",
            "--raw",
            "--expr",
            &derivation_expression,
        ])
        .env_clear()
        .env("HOME", &home)
        .env("NIX_CONFIG", kiln_adapter_nix::PINNED_NIX_CONFIG)
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(evaluation.status.success());
    let derivation_path = String::from_utf8(evaluation.stdout)
        .unwrap_or_else(|_| unreachable!())
        .trim()
        .to_string();
    let outputs = std::process::Command::new(kiln_adapter_nix::PINNED_NIX_STORE_EXECUTABLE_PATH)
        .args(["-q", "--outputs", &derivation_path])
        .env_clear()
        .env("HOME", &home)
        .env("NIX_CONFIG", kiln_adapter_nix::PINNED_NIX_CONFIG)
        .output()
        .unwrap_or_else(|_| unreachable!());
    assert!(outputs.status.success());
    let output_path = String::from_utf8(outputs.stdout)
        .unwrap_or_else(|_| unreachable!())
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_else(|| unreachable!())
        .to_string();
    let mut task = task();
    task.derivation =
        ExternalRef::new("nix", "derivation", &derivation_path).unwrap_or_else(|_| unreachable!());
    task.outputs = vec![DiscoveredOutput {
        name: alias("out"),
        path: ExternalRef::new("nix", "store-path", &output_path)
            .unwrap_or_else(|_| unreachable!()),
    }];
    let mut base = base();
    base.runtime.home_base = home;
    base.runtime.gc_root_base = temp.path().join("roots");
    let (mut adapter, plan, observation) =
        build_task(&base, &task, &intent(), BoundedNixBuildRuntime)
            .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        observation.outcome,
        kiln_core::build::BuildEvaluationOutcome::Success
    );
    assert_eq!(
        std::fs::read_to_string(&output_path).unwrap_or_else(|_| unreachable!()),
        "task-live\n"
    );
    let root = gc_root_directory(adapter.profile_binding(), &plan.effect_id)
        .unwrap_or_else(|_| unreachable!());
    assert!(root.exists());
    adapter
        .release_output_roots(&plan.effect_id)
        .unwrap_or_else(|_| unreachable!());
    assert!(!root.exists());
}
