//! Build-input qualification of the projected-build corpus (#77).
//!
//! The corpus is committed at a fixed time to a temporary repository whose
//! working checkout keeps every file, so revisions, manifests and plans are
//! deterministic. Each case's projection is resolved and planned under its
//! build profile, and planned views are materialized through
//! `kiln_web_service::worker::materialize` into an attempt overlay. Given a
//! Cargo executable, each planned command runs at the overlay's build root
//! with a cleared environment and private home, Cargo home and target
//! directories, once the declared toolchain is confirmed. The shared edit is
//! then written through one case's overlay, mapped back to its origins and
//! decided by `monorepo::affected` over the full candidate. Nothing here
//! grants source, workspace, execution or publication authority.
// r[impl kiln.source.projection.build_inputs.scenario.qualified]
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use bounded_exec::{
    CommandSpec, Disposition, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope,
};
use git2::build::TreeUpdateBuilder;
use git2::{FileMode, Oid, Repository, Signature, Time};
use kiln_build_graph::{ManifestTree, cargo_graph, cargo_layout, nix_graph, wanted_files};
use kiln_core::monorepo::{
    AffectedSet, ChangedFiles, ProjectDecision, ProjectInput, ProjectScope, admit_scope, affected,
};
use kiln_core::projection::build_inputs::{
    BuildInputPlan, BuildProfile, BuildRequest, ExternalPackages, InputRole, plan_cargo, plan_nix,
};
use kiln_core::projection::materialization::{
    MATERIALIZER_PROFILE_SCHEMA, MaterializationPlan, MaterializerKind, MaterializerProfile, plan,
};
use kiln_core::projection::{
    CompleteProjection, ComponentVersion, Coverage, EntryKind, EntryOrigin, ProjectionDefinition,
    ProjectionFacts, ProjectionOperation, ProviderBinding, RepositoryEnrollment, RepositoryLocator,
    SourceCapability, SourceEntry, TargetProfile, admit_definition, admit_repositories, resolve,
};
use kiln_core::types::{ContentId, Diagnostic, ExternalRef, ProviderProfileId};
use kiln_web_service::worker::materialize::{prepare_input, prepare_overlay};
use serde::{Deserialize, Serialize};

pub type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
type Planned = std::result::Result<BuildInputPlan, Vec<Diagnostic>>;

pub const CORPUS_SCHEMA: &str = "kiln.build-input-corpus.v1";
pub const RECEIPT_SCHEMA: &str = "kiln.build-input.qualification.receipt.v1";
/// Largest corpus document the harness admits.
pub const MAX_CORPUS_BYTES: u64 = 256 * 1024;
/// Where each execution's bounded output is kept, below the scratch directory.
pub const LOG_DIRECTORY: &str = "logs";
/// Where the committed corpus keeps its working files, below the scratch
/// directory.
pub const CHECKOUT_DIRECTORY: &str = "checkout";
const MAX_FILES: usize = 64;
const MAX_PROJECTS: usize = 8;
const MAX_CASES: usize = 8;
const MAX_TEXT_BYTES: usize = 4_096;
const MAX_ID_BYTES: usize = 64;
/// Every overlay is prepared for the first fence of its case's attempt.
const FENCE: u64 = 1;
const PRIVATE_DIRECTORY: u32 = 0o700;
const SOURCE_FILE: u32 = 0o644;
const SOURCE_EXECUTABLE: u32 = 0o755;
/// Longest one planned command may run.
const COMMAND_TIMEOUT_MS: u64 = 600_000;
/// Longest one toolchain version query may run.
const VERSION_TIMEOUT_MS: u64 = 30_000;
/// Output retained per stream of one execution.
const OUTPUT_BYTES: usize = 64 * 1024;
const POLL_INTERVAL_MS: u64 = 20;
const TEARDOWN_TIMEOUT_MS: u64 = 5_000;
/// Variables the runner owns; a declared environment may not name them.
const RUNNER_VARIABLES: [&str; 4] = ["CARGO_HOME", "CARGO_TARGET_DIR", "HOME", "PATH"];
const NON_CLAIMS: [&str; 4] = [
    "A plan shows that declared inputs are present; only these executions show that this corpus builds with this toolchain.",
    "Absolute paths, Cargo configuration above the overlay and the host toolchain are outside the plan; no restricted runtime confined the commands.",
    "Plans and diagnostics name every path; they are controller facts, not results for a restricted worker.",
    "The corpus is synthetic and small; timings describe this host only.",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CorpusFile {
    path: String,
    text: String,
    executable: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Project {
    id: String,
    scope: ProjectScope,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    definition: ProjectionDefinition,
    request: BuildRequest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SharedEdit {
    case: String,
    file: CorpusFile,
}

/// The `kiln.build-input-corpus.v1` export of `config/build-input-contracts.ncl`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    schema: String,
    files: Vec<CorpusFile>,
    projects: Vec<Project>,
    cases: Vec<Case>,
    shared_edit: SharedEdit,
}

#[derive(Debug, Serialize)]
pub struct Receipt {
    pub schema: &'static str,
    /// BLAKE3 over the corpus document.
    pub corpus: ContentId,
    pub revision: String,
    pub toolchain: Toolchain,
    pub cases: Vec<CaseRecord>,
    pub shared_edit: EditRecord,
    /// Measured on this host; never part of the committed expectation.
    pub timings: Vec<Timing>,
    pub non_claims: [&'static str; 4],
}

#[derive(Debug, Serialize)]
pub struct Toolchain {
    /// Toolchain versions the requests declare.
    pub declared: BTreeSet<String>,
    /// `cargo --version` and `rustc --version` under the cleared environment,
    /// when commands were run; never part of the committed expectation.
    pub observed: Option<Observed>,
}

#[derive(Debug, Serialize)]
pub struct Observed {
    pub cargo: String,
    pub rustc: String,
}

#[derive(Debug, Serialize)]
pub struct CaseRecord {
    pub id: String,
    pub plan: PlanRecord,
    pub execution: Execution,
}

#[derive(Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum PlanRecord {
    Planned {
        identity: ContentId,
        build_root: String,
        packages: Vec<String>,
        inputs: Vec<InputRecord>,
        external_packages: ExternalPackages,
        /// Identity of the materialized immutable input.
        materialized: ContentId,
    },
    Refused {
        stage: &'static str,
        diagnostics: Vec<Diagnostic>,
    },
}

#[derive(Debug, Serialize)]
pub struct InputRecord {
    pub path: String,
    pub role: InputRole,
}

#[derive(Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Execution {
    Passed {
        sources_unchanged: bool,
    },
    Failed {
        exit_code: Option<i32>,
        sources_unchanged: bool,
    },
    TimedOut,
    NotRun {
        reason: &'static str,
    },
}

#[derive(Debug, Serialize)]
pub struct EditRecord {
    pub case: String,
    /// Origins of the overlay entries the edit changed.
    pub changed: Vec<String>,
    pub projects: Vec<ProjectRecord>,
    pub decisions: AffectedSet,
}

#[derive(Debug, Serialize)]
pub struct ProjectRecord {
    pub project: String,
    pub affected: bool,
    /// Whether the editing view holds any file of the project.
    pub in_view: bool,
}

#[derive(Debug, Serialize)]
pub struct Timing {
    pub case: String,
    pub plan_us: u64,
    pub materialize_us: Option<u64>,
    pub execute_us: Option<u64>,
}

/// A planned and materialized case.
struct View {
    complete: CompleteProjection,
    materialized: MaterializationPlan,
    input: PathBuf,
    overlay: PathBuf,
}

/// The confirmed toolchain and the runner-owned locations.
struct Runner {
    cargo: PathBuf,
    path: OsString,
    home: PathBuf,
    cargo_home: PathBuf,
    targets: PathBuf,
    logs: PathBuf,
    cargo_version: Option<String>,
    rustc_version: Option<String>,
    observed: Observed,
}

/// Every file below a root as (relative path, mode, bytes), in path order.
type Snapshot = Vec<(String, u32, Vec<u8>)>;

/// Qualify the corpus under `scratch`. With `cargo`, planned Cargo commands
/// run; without it, executions are recorded as not requested. A malformed
/// corpus is refused before anything is written.
pub fn run(bytes: &[u8], scratch: &Path, cargo: Option<&Path>) -> Result<Receipt> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_CORPUS_BYTES {
        return Err("the corpus exceeds its byte bound".into());
    }
    let corpus: Corpus = serde_json::from_slice(bytes)?;
    admit(&corpus)?;
    let result = qualify(&corpus, bytes, scratch, cargo);
    // Sealed inputs are read-only; make them removable either way.
    release(scratch);
    result
}

fn admit(corpus: &Corpus) -> Result {
    let safe = |path: &str| {
        !path.is_empty()
            && Path::new(path)
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
    };
    let unique = |values: Vec<&str>| values.iter().collect::<BTreeSet<_>>().len() == values.len();
    // Case and project ids name private directories below the scratch root.
    let id = |value: &str| {
        (1..=MAX_ID_BYTES).contains(&value.len())
            && value
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    };
    let case_ids: Vec<&str> = corpus.cases.iter().map(|case| case.id.as_str()).collect();
    if corpus.schema != CORPUS_SCHEMA
        || corpus.files.is_empty()
        || corpus.files.len() > MAX_FILES
        || corpus.projects.is_empty()
        || corpus.projects.len() > MAX_PROJECTS
        || corpus.cases.is_empty()
        || corpus.cases.len() > MAX_CASES
        || !corpus
            .files
            .iter()
            .chain([&corpus.shared_edit.file])
            .all(|file| safe(&file.path) && file.text.len() <= MAX_TEXT_BYTES)
        || !unique(corpus.files.iter().map(|file| file.path.as_str()).collect())
        || !unique(
            corpus
                .projects
                .iter()
                .map(|project| project.id.as_str())
                .collect(),
        )
        || !unique(case_ids.clone())
        || !case_ids.iter().all(|case| id(case))
        || !corpus.projects.iter().all(|project| id(&project.id))
        || !case_ids.contains(&corpus.shared_edit.case.as_str())
    {
        return Err("the corpus is malformed, duplicated or over bound".into());
    }
    Ok(())
}

fn qualify(corpus: &Corpus, bytes: &[u8], scratch: &Path, cargo: Option<&Path>) -> Result<Receipt> {
    let checkout = private(scratch.join(CHECKOUT_DIRECTORY))?;
    let repository = Repository::init(&checkout)?;
    let (revision, tree, objects) = commit(&repository, &checkout, corpus)?;
    let admitted = admit_repositories(&[enrollment(&checkout)?], &[])
        .map_err(diagnostics)?
        .pop()
        .ok_or("the corpus repository was not admitted")?;
    let facts = facts(corpus, admitted.identity(), &revision, &tree, &objects)?;
    let manifests = manifest_tree(corpus);
    let layout = cargo_layout(&manifests);
    let cargo_facts = cargo_graph(&manifests);
    let nix_facts = nix_graph(&manifests);
    let profile = MaterializerProfile {
        schema: MATERIALIZER_PROFILE_SCHEMA.into(),
        kind: MaterializerKind::EagerLocalDirectory,
        materializer: ComponentVersion {
            name: "kiln-eager-local".into(),
            version: "1".into(),
        },
        platform: TargetProfile::POSIX,
    };
    let store = private(scratch.join("store"))?;
    let runner = cargo.map(|cargo| Runner::new(cargo, scratch)).transpose()?;
    let mut views = BTreeMap::new();
    let mut cases = Vec::with_capacity(corpus.cases.len());
    let mut timings = Vec::with_capacity(corpus.cases.len());
    for case in &corpus.cases {
        let started = Instant::now();
        let planned = plan_case(case, &admitted, &facts, |complete| {
            match case.request.profile {
                BuildProfile::CargoCompatibilityV1 => {
                    plan_cargo(complete, &facts, &layout, &case.request)
                }
                BuildProfile::NixCompatibilityV1 => {
                    plan_nix(complete, &facts, &nix_facts, &case.request)
                }
            }
        });
        let plan_us = micros(started.elapsed());
        let (complete, build_plan) = match planned {
            Ok(planned) => planned,
            Err(refused) => {
                cases.push(CaseRecord {
                    id: case.id.clone(),
                    plan: refused,
                    execution: Execution::NotRun {
                        reason: "the plan was refused",
                    },
                });
                timings.push(Timing {
                    case: case.id.clone(),
                    plan_us,
                    materialize_us: None,
                    execute_us: None,
                });
                continue;
            }
        };
        let started = Instant::now();
        let materialized = plan(&complete, &profile).map_err(diagnostics)?;
        let input = prepare_input(&store, &materialized, &repository)?;
        let pin = materialized
            .pin(&digest(case.id.as_bytes())?, FENCE)
            .map_err(diagnostic)?;
        let overlay = prepare_overlay(&store, &materialized, &pin)?;
        let materialize_us = micros(started.elapsed());
        let started = Instant::now();
        let execution = match &runner {
            Some(runner) => runner.execute(case, &build_plan, &overlay)?,
            None => Execution::NotRun {
                reason: "execution was not requested",
            },
        };
        let execute_us = runner.as_ref().map(|_| micros(started.elapsed()));
        cases.push(CaseRecord {
            id: case.id.clone(),
            plan: PlanRecord::Planned {
                identity: build_plan.identity().clone(),
                build_root: build_plan.build_root().to_string(),
                packages: build_plan.packages().to_vec(),
                inputs: build_plan
                    .inputs()
                    .iter()
                    .map(|input| InputRecord {
                        path: input.path.clone(),
                        role: input.role,
                    })
                    .collect(),
                external_packages: build_plan.external_packages(),
                materialized: materialized.input_identity().clone(),
            },
            execution,
        });
        timings.push(Timing {
            case: case.id.clone(),
            plan_us,
            materialize_us: Some(materialize_us),
            execute_us,
        });
        views.insert(
            case.id.clone(),
            View {
                complete,
                materialized,
                input,
                overlay,
            },
        );
    }
    let shared_edit = edit(corpus, &views, &cargo_facts)?;
    Ok(Receipt {
        schema: RECEIPT_SCHEMA,
        corpus: digest(bytes)?,
        revision: revision.to_string(),
        toolchain: Toolchain {
            declared: corpus
                .cases
                .iter()
                .map(|case| case.request.toolchain.version.clone())
                .collect(),
            observed: runner.map(|runner| runner.observed),
        },
        cases,
        shared_edit,
        timings,
        non_claims: NON_CLAIMS,
    })
}

/// Resolve, complete and plan one case, naming the stage that refused it.
fn plan_case(
    case: &Case,
    admitted: &kiln_core::projection::AdmittedRepository,
    facts: &ProjectionFacts,
    planner: impl FnOnce(&CompleteProjection) -> Planned,
) -> std::result::Result<(CompleteProjection, BuildInputPlan), PlanRecord> {
    let refused = |stage, diagnostics| PlanRecord::Refused { stage, diagnostics };
    let definition = admit_definition(&case.definition).map_err(|d| refused("definition", d))?;
    let view = resolve(&definition, admitted, facts).map_err(|d| refused("resolution", d))?;
    let complete = view
        .require_complete(ProjectionOperation::Build)
        .map_err(|incomplete| refused("completeness", incomplete.diagnostics))?;
    let build_plan = planner(&complete).map_err(|d| refused("planning", d))?;
    Ok((complete, build_plan))
}

/// One commit of the whole corpus at a fixed time and signature, with every
/// file also written into the working checkout.
fn commit(
    repository: &Repository,
    checkout: &Path,
    corpus: &Corpus,
) -> Result<(Oid, Oid, BTreeMap<String, Oid>)> {
    let mut objects = BTreeMap::new();
    let mut update = TreeUpdateBuilder::new();
    for file in &corpus.files {
        let path = checkout.join(&file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &file.text)?;
        let (mode, file_mode) = if file.executable {
            (SOURCE_EXECUTABLE, FileMode::BlobExecutable)
        } else {
            (SOURCE_FILE, FileMode::Blob)
        };
        fs::set_permissions(&path, fs::Permissions::from_mode(mode))?;
        let oid = repository.blob(file.text.as_bytes())?;
        update.upsert(file.path.as_str(), oid, file_mode);
        objects.insert(file.path.clone(), oid);
    }
    let empty = repository.find_tree(repository.treebuilder(None)?.write()?)?;
    let tree = update.create_updated(repository, &empty)?;
    let signature = Signature::new("Kiln corpus", "corpus@example.invalid", &Time::new(0, 0))?;
    let commit = repository.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Build-input corpus",
        &repository.find_tree(tree)?,
        &[],
    )?;
    Ok((commit, tree, objects))
}

fn enrollment(source: &Path) -> Result<RepositoryEnrollment> {
    Ok(RepositoryEnrollment {
        repository_id: "build-input-corpus".into(),
        binding: ProviderBinding {
            source_provider: ProviderProfileId::new("git-source-v1").map_err(diagnostic)?,
            native_repository: ExternalRef::new("git", "repository", "build-input-corpus")
                .map_err(diagnostic)?,
        },
        locators: vec![RepositoryLocator::LocalPath {
            path: source.to_string_lossy().into_owned(),
        }],
    })
}

fn facts(
    corpus: &Corpus,
    repository: &ContentId,
    revision: &Oid,
    tree: &Oid,
    objects: &BTreeMap<String, Oid>,
) -> Result<ProjectionFacts> {
    let entries = corpus
        .files
        .iter()
        .map(|file| {
            let oid = objects
                .get(&file.path)
                .ok_or("a corpus file has no object")?;
            Ok(SourceEntry {
                path: file.path.clone(),
                object: ExternalRef::new("git", "blob", &oid.to_string()).map_err(diagnostic)?,
                kind: if file.executable {
                    EntryKind::Executable
                } else {
                    EntryKind::File
                },
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ProjectionFacts {
        repository: repository.clone(),
        exact_revision: ExternalRef::new("git", "commit", &revision.to_string())
            .map_err(diagnostic)?,
        canonical_tree: digest(tree.to_string().as_bytes())?,
        evaluator: ComponentVersion {
            name: "kiln-projection".into(),
            version: "1".into(),
        },
        adapter: ComponentVersion {
            name: "git-source".into(),
            version: "1".into(),
        },
        source_capabilities: vec![
            SourceCapability::ExactTree,
            SourceCapability::ExecutableBit,
            SourceCapability::Symlinks,
        ],
        coverage: Coverage::Complete,
        entries,
        dependencies: Vec::new(),
        generated: Vec::new(),
    })
}

/// The full candidate listing and the manifests the build graphs read.
fn manifest_tree(corpus: &Corpus) -> ManifestTree {
    let paths: BTreeSet<String> = corpus.files.iter().map(|file| file.path.clone()).collect();
    let wanted: BTreeSet<String> = wanted_files(&paths).into_iter().collect();
    ManifestTree {
        files: corpus
            .files
            .iter()
            .filter(|file| wanted.contains(&file.path))
            .map(|file| (file.path.clone(), file.text.clone().into_bytes()))
            .collect(),
        paths,
    }
}

impl Runner {
    /// Confirm the toolchain under the cleared environment the commands use.
    fn new(cargo: &Path, scratch: &Path) -> Result<Self> {
        let path = std::env::var_os("PATH").ok_or("PATH is unset")?;
        let rustc = on_path(&path, "rustc").ok_or("rustc is not on PATH")?;
        let home = private(scratch.join("home"))?;
        let cargo_home = private(scratch.join("cargo-home"))?;
        let targets = private(scratch.join("targets"))?;
        let logs = private(scratch.join(LOG_DIRECTORY))?;
        let mut runner = Self {
            cargo: cargo.to_path_buf(),
            path,
            home,
            cargo_home,
            targets,
            logs,
            cargo_version: None,
            rustc_version: None,
            observed: Observed {
                cargo: String::new(),
                rustc: String::new(),
            },
        };
        let cargo_line = runner.version(cargo)?;
        let rustc_line = runner.version(&rustc)?;
        runner.cargo_version = version_of(&cargo_line);
        runner.rustc_version = version_of(&rustc_line);
        runner.observed = Observed {
            cargo: cargo_line,
            rustc: rustc_line,
        };
        Ok(runner)
    }

    fn environment(&self, target: &Path) -> Vec<(OsString, OsString)> {
        vec![
            (
                "CARGO_HOME".into(),
                self.cargo_home.clone().into_os_string(),
            ),
            ("CARGO_TARGET_DIR".into(), target.as_os_str().to_owned()),
            ("HOME".into(), self.home.clone().into_os_string()),
            ("PATH".into(), self.path.clone()),
        ]
    }

    /// The first line `program --version` prints, or an empty line when the
    /// query fails.
    fn version(&self, program: &Path) -> Result<String> {
        let output = bounded_exec::run(request(
            program,
            vec!["--version".into()],
            &self.home,
            self.environment(&self.targets),
            VERSION_TIMEOUT_MS,
        )?)
        .map_err(|error| format!("{error:?}"))?;
        Ok(String::from_utf8_lossy(&output.stdout.bytes)
            .lines()
            .next()
            .unwrap_or_default()
            .to_string())
    }

    /// Run one planned command at its build root in the case's overlay.
    fn execute(
        &self,
        case: &Case,
        build_plan: &BuildInputPlan,
        overlay: &Path,
    ) -> Result<Execution> {
        let [program, arguments @ ..] = build_plan.command() else {
            return Ok(Execution::NotRun {
                reason: "the plan has no command",
            });
        };
        if program.as_str() != "cargo" {
            return Ok(Execution::NotRun {
                reason: "only Cargo commands are qualified",
            });
        }
        let declared = Some(build_plan.toolchain().version.as_str());
        if self.cargo_version.as_deref() != declared || self.rustc_version.as_deref() != declared {
            return Ok(Execution::NotRun {
                reason: "the observed toolchain differs from the declared toolchain",
            });
        }
        if build_plan
            .environment()
            .keys()
            .any(|name| RUNNER_VARIABLES.contains(&name.as_str()))
        {
            return Ok(Execution::NotRun {
                reason: "the declared environment names a runner-owned variable",
            });
        }
        let target = private(self.targets.join(&case.id))?;
        let mut environment: Vec<(OsString, OsString)> = build_plan
            .environment()
            .iter()
            .map(|(name, value)| (name.into(), value.into()))
            .collect();
        environment.extend(self.environment(&target));
        let before = snapshot(overlay)?;
        let output = bounded_exec::run(request(
            &self.cargo,
            arguments.iter().map(OsString::from).collect(),
            &overlay.join(build_plan.build_root()),
            environment,
            COMMAND_TIMEOUT_MS,
        )?)
        .map_err(|error| format!("{error:?}"))?;
        let mut log = output.stdout.bytes.clone();
        log.extend_from_slice(&output.stderr.bytes);
        fs::write(self.logs.join(format!("{}.log", case.id)), log)?;
        let sources_unchanged = snapshot(overlay)? == before;
        Ok(match output.disposition {
            Disposition::Succeeded => Execution::Passed { sources_unchanged },
            Disposition::TimedOut => Execution::TimedOut,
            _ => Execution::Failed {
                exit_code: output.exit_code,
                sources_unchanged,
            },
        })
    }
}

fn request(
    program: &Path,
    args: Vec<OsString>,
    directory: &Path,
    environment: Vec<(OsString, OsString)>,
    timeout_ms: u64,
) -> Result<RunRequest> {
    Ok(RunRequest {
        command: CommandSpec {
            program: program.to_path_buf(),
            args,
            current_dir: directory.to_path_buf(),
            environment_mode: EnvironmentMode::Clear,
            environment,
            input: Input::Null,
        },
        limits: ExecutionLimits {
            timeout_ms,
            stdin_max_bytes: 1,
            stdout_max_bytes: OUTPUT_BYTES,
            stderr_max_bytes: OUTPUT_BYTES,
            poll_interval_ms: POLL_INTERVAL_MS,
            teardown_timeout_ms: TEARDOWN_TIMEOUT_MS,
        },
        termination_scope: TerminationScope::ProcessGroup,
        outcome_policy: OutcomePolicy::new(vec![0], false, false)
            .map_err(|error| format!("{error:?}"))?,
    })
}

/// The version token of a `cargo 1.88.0 (...)` or `rustc 1.88.0 (...)` line.
fn version_of(line: &str) -> Option<String> {
    line.split_whitespace().nth(1).map(str::to_string)
}

/// The first executable file named `name` on `path`.
pub fn on_path(path: &std::ffi::OsStr, name: &str) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|directory| directory.join(name))
        .find(|candidate| {
            fs::metadata(candidate)
                .is_ok_and(|metadata| metadata.is_file() && metadata.mode() & 0o111 != 0)
        })
}

/// Write the shared edit through its case's overlay, map the changed
/// entries back to their origins and decide the affected projects over the
/// full candidate's Cargo graph.
fn edit(
    corpus: &Corpus,
    views: &BTreeMap<String, View>,
    graph: &kiln_core::monorepo::GraphFacts,
) -> Result<EditRecord> {
    let edit = &corpus.shared_edit;
    let view = views
        .get(&edit.case)
        .ok_or("the shared edit names a case that was not planned")?;
    let entries = view.materialized.entries();
    if !entries
        .iter()
        .any(|entry| entry.destination == edit.file.path)
    {
        return Err("the shared edit is outside its view".into());
    }
    fs::write(view.overlay.join(&edit.file.path), &edit.file.text)?;
    let mut changed = BTreeSet::new();
    for entry in entries {
        if fs::read(view.overlay.join(&entry.destination))?
            != fs::read(view.input.join(&entry.destination))?
        {
            changed.insert(entry.destination.as_str());
        }
    }
    let manifest = view.complete.manifest();
    let origins: BTreeSet<String> = manifest
        .entries
        .iter()
        .filter(|entry| changed.contains(entry.destination.as_str()))
        .filter_map(|entry| match &entry.origin {
            EntryOrigin::Native { path, .. } => Some(path.clone()),
            EntryOrigin::Generated { .. } => None,
        })
        .collect();
    let inputs = corpus
        .projects
        .iter()
        .map(|project| {
            Ok(ProjectInput {
                project_id: project.id.clone(),
                scope: Some(
                    admit_scope(&project.id, &project.scope).map_err(|error| error.to_string())?,
                ),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let decisions = affected(
        &inputs,
        &ChangedFiles::exact(origins.iter().cloned()),
        std::slice::from_ref(graph),
    );
    let projects = corpus
        .projects
        .iter()
        .map(|project| ProjectRecord {
            project: project.id.clone(),
            affected: decisions
                .decision(&project.id)
                .is_some_and(ProjectDecision::is_affected),
            in_view: manifest.entries.iter().any(|entry| match &entry.origin {
                EntryOrigin::Native { path, .. } => under(&project.scope.root, path),
                EntryOrigin::Generated { .. } => false,
            }),
        })
        .collect();
    Ok(EditRecord {
        case: edit.case.clone(),
        changed: origins.into_iter().collect(),
        projects,
        decisions,
    })
}

/// Whether `path` lies below directory `root` (every path lies below "").
fn under(root: &str, path: &str) -> bool {
    root.is_empty()
        || path
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

fn snapshot(root: &Path) -> Result<Snapshot> {
    let mut found = Vec::new();
    walk(root, root, &mut found)?;
    found.sort();
    Ok(found)
}

fn walk(root: &Path, directory: &Path, found: &mut Snapshot) -> Result {
    for child in fs::read_dir(directory)? {
        let path = child?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.is_dir() {
            walk(root, &path, found)?;
        } else {
            let name = path
                .strip_prefix(root)?
                .to_str()
                .ok_or("a materialized path is not UTF-8")?
                .to_owned();
            found.push((name, metadata.mode() & 0o7777, fs::read(&path)?));
        }
    }
    Ok(())
}

fn private(path: PathBuf) -> Result<PathBuf> {
    fs::DirBuilder::new()
        .mode(PRIVATE_DIRECTORY)
        .create(&path)?;
    Ok(path)
}

/// Best effort: make every directory below `root` writable again.
fn release(root: &Path) {
    if let Ok(metadata) = fs::symlink_metadata(root)
        && metadata.is_dir()
    {
        let _ = fs::set_permissions(root, fs::Permissions::from_mode(PRIVATE_DIRECTORY));
        if let Ok(children) = fs::read_dir(root) {
            for child in children.flatten() {
                release(&child.path());
            }
        }
    }
}

fn digest(bytes: &[u8]) -> Result<ContentId> {
    ContentId::from_blake3_hex(blake3::hash(bytes).to_hex().to_string()).map_err(diagnostic)
}

fn diagnostic(error: Diagnostic) -> Box<dyn Error> {
    format!("{}: {}", error.code, error.message).into()
}

fn diagnostics(errors: Vec<Diagnostic>) -> Box<dyn Error> {
    errors
        .into_iter()
        .map(|error| format!("{}: {}", error.code, error.message))
        .collect::<Vec<_>>()
        .join("; ")
        .into()
}
