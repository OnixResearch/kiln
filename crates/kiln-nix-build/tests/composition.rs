use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

#[cfg(feature = "nix-live")]
use kiln_adapter_nix::BoundedNixBuildRuntime;
use kiln_adapter_nix::{
    NixBuildProcessOutput, NixBuildProfile, NixBuildRuntime, NixOutputBinding, ProcessInvocation,
    gc_root_directory,
};
use kiln_core::discovery::{CacheAdvisory, DiscoveredOutput, TaskProposal};
use kiln_core::types::{
    Alias, AttemptId, CandidateId, ContentId, EffectId, ExternalRef, PipelineRevisionId,
    ProviderProfileId, TaskId,
};
use kiln_nix_build::{NixBuildIntent, build_task, plan_for_task, profile_for_task};
use kiln_provider_api::PortResult;

const IDENTITY_HEX_BYTES: usize = 64;
const STORE_PATH: &str = "/nix/store";
const BASE_DRV: &str = "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-base.drv";
const TASK_DRV: &str = "/nix/store/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-task.drv";
const OUT_PATH: &str = "/nix/store/cccccccccccccccccccccccccccccccc-task";
const BASE_OUT: &str = "/nix/store/dddddddddddddddddddddddddddddddd-base";
const GC_ROOT_BASE: &str = "/tmp/kiln-nix-composition-roots";
const HOME_PATH: &str = "/tmp/kiln-nix-composition-home";

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(IDENTITY_HEX_BYTES))
        .unwrap_or_else(|_| unreachable!())
}

fn alias(value: &str) -> Alias {
    Alias::new(value).unwrap_or_else(|_| unreachable!())
}

fn limits() -> kiln_adapter_nix::NixBuildLimits {
    kiln_adapter_nix::NixBuildLimits {
        max_required_products: 8,
        max_products: 8,
        max_logs: 1,
        max_gc_roots: 16,
        max_request_bytes: 1_048_576,
        max_result_bytes: 1_048_576,
        max_stderr_bytes: 65_536,
        max_log_bytes: 65_536,
        max_executable_bytes: 268_435_456,
        max_derivation_bytes: 1_048_576,
        max_in_flight: 4,
        stdin_max_bytes: 1,
        timeout_milliseconds: 30_000,
        poll_interval_milliseconds: 5,
        teardown_timeout_milliseconds: 1_000,
    }
}

fn base() -> NixBuildProfile {
    NixBuildProfile::pinned(
        ProviderProfileId::new("nix-build-v1").unwrap_or_else(|_| unreachable!()),
        kiln_adapter_nix::NixRuntimeBinding {
            executable_path: PathBuf::from(kiln_adapter_nix::PINNED_NIX_EXECUTABLE_PATH),
            executable_identity: ContentId::from_blake3_hex(
                kiln_adapter_nix::PINNED_NIX_EXECUTABLE_BLAKE3,
            )
            .unwrap_or_else(|_| unreachable!()),
            nix_store_executable_path: PathBuf::from(
                kiln_adapter_nix::PINNED_NIX_STORE_EXECUTABLE_PATH,
            ),
            nix_store_executable_identity: ContentId::from_blake3_hex(
                kiln_adapter_nix::PINNED_NIX_STORE_EXECUTABLE_BLAKE3,
            )
            .unwrap_or_else(|_| unreachable!()),
            derivation_path: PathBuf::from(BASE_DRV),
            derivation_identity: content('2'),
            plan_identity: content('3'),
            policy_identity: content('4'),
            store_path: PathBuf::from(STORE_PATH),
            gc_root_base: PathBuf::from(GC_ROOT_BASE),
            home_base: PathBuf::from(HOME_PATH),
            worker_identity: content('5'),
            store_identity: content('6'),
            expected_outputs: vec![NixOutputBinding {
                name: alias("out"),
                path: PathBuf::from(BASE_OUT),
            }],
        },
        limits(),
    )
    .unwrap_or_else(|_| unreachable!())
}

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

fn intent() -> NixBuildIntent {
    NixBuildIntent {
        effect_id: EffectId::new("build-effect-a").unwrap_or_else(|_| unreachable!()),
        attempt_id: AttemptId::new("attempt-a").unwrap_or_else(|_| unreachable!()),
        idempotency_key: content('1'),
        candidate_id: CandidateId::new("candidate-a").unwrap_or_else(|_| unreachable!()),
        pipeline_revision: PipelineRevisionId::new("pipeline-a").unwrap_or_else(|_| unreachable!()),
        plan_identity: content('3'),
        policy_identity: content('4'),
        platform: alias("x86_64-linux"),
    }
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
    run_count: usize,
    cleaned: Vec<PathBuf>,
}

impl FakeRuntime {
    fn new() -> Self {
        Self {
            derivation_identity: content('8'),
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
    assert_eq!(adapter.runtime().run_count, 1);
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
