//! Off-by-default qualification of projected workspaces in the restricted
//! runtime (#77, #78, #79).
//!
//! For each case of the two-project Cargo corpus and the two-project Nix
//! corpus it commits the corpus to a temporary Git repository, admits one
//! local scripted work, claims it, resolves, plans, materializes and routes
//! the case's projected workspace for the claimed attempt, binds it through
//! `kiln-implementation-fixture bind-execution --projected-workspace-route`
//! and dispatches it through `kiln-lattice-dispatch` to the real Lattice
//! binary under the operator's `kiln.implementation-authority-config.v1`
//! (C1), which must select a verified profile with `projection_enabled`: a
//! projected dispatch never runs under the local-operator profile, and the
//! harness refuses to start under any other configuration. The worker
//! program is a probe placed in the Nix store: inside Lattice's bubblewrap
//! sandbox it tries every confinement negative, runs the case's planned
//! command, writes the shared edit through its route and reports each
//! observation on stdout, which Lattice retains. The harness then captures
//! the overlay, derives the projected candidate facts and writes a
//! `kiln.projected-runtime.receipt.v2` receipt; a case qualifies only when
//! every probe it sent reports exactly its expected outcome. Before the real
//! dispatch, requests that differ from the route, and the same request under
//! the local-operator profile, are refused by the dispatcher. No model is
//! contacted: the broker reaches only a local endpoint that answers HTTP 400.
//!
//! ```console
//! nix develop -c cargo run -p kiln-web-service --example projected-runtime -- \
//!     --lattice /nix/store/...-lattice-0.1.0/bin/lattice \
//!     --dispatch target/debug/kiln-lattice-dispatch \
//!     --fixture target/debug/kiln-implementation-fixture \
//!     --authority-config /etc/kiln/implementation-authority.json \
//!     --bwrap /nix/store/...-bubblewrap/bin/bwrap --socat /nix/store/...-socat/bin/socat \
//!     --shell /nix/store/...-bash/bin/bash --nix /nix/store/...-nix/bin/nix \
//!     --scratch /var/tmp/projected-runtime --out receipt.json
//! ```
// r[impl kiln.source.projection.runtime]
// r[impl kiln.source.projection.runtime.scenario.qualified]
// r[impl kiln.source.projection.runtime.scenario.confined]
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::io::{Read, Write};
use std::os::linux::net::SocketAddrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{SocketAddr, UnixListener};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use base64::Engine as _;
use git2::build::TreeUpdateBuilder;
use git2::{FileMode, Oid, Repository, Signature, Time};
use kiln_adapter_basalt::test_support::{
    COORDINATOR_SEED, EXPIRES_S, SourceFixture, VERIFIER_SEED, VERIFY_AT_S,
};
use kiln_build_graph::{ManifestTree, cargo_graph, cargo_layout, nix_graph, wanted_files};
use kiln_core::fair_admission::EligibilityInput;
use kiln_core::implementation::authority::{AttemptCurrentness, AuthorityProfile};
use kiln_core::implementation::{
    AcceptanceObligation, DISPATCH_PLAN_SCHEMA, DispatchControls, DispatchWorkspace, Enrollment,
    IMPLEMENTATION_WORK_SCHEMA, ImplementationAttempt, ImplementationDispatchPlan,
    ImplementationWork, SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind, WorkspaceIsolation,
    acceptance_digest,
};
use kiln_core::monorepo::{GraphFacts, ProjectInput, ProjectScope, admit_scope};
use kiln_core::projection::authority::{
    AttemptFence, OriginScope, PathScope, SnapshotObservation, SnapshotRule, SourceAuthority,
    SourceAuthorityRequest, SourceCurrentness, SourceDenialReason, SourceViewOperation,
};
use kiln_core::projection::build_inputs::{
    BuildInputPlan, BuildProfile, BuildRequest, plan_cargo, plan_nix,
};
use kiln_core::projection::disclosure::ViewCeiling;
use kiln_core::projection::materialization::{
    MATERIALIZER_PROFILE_SCHEMA, MaterializerKind, MaterializerProfile, ProjectedWorkspaceRoute,
    RouteRequest, plan, route,
};
use kiln_core::projection::writeback::{Captured, CurrentAttempt, candidate_facts, capture};
use kiln_core::projection::{
    ComponentVersion, Coverage, EntryKind, ProjectionDefinition, ProjectionFacts,
    ProjectionOperation, ProjectionProfile, ProviderBinding, RepositoryEnrollment,
    RepositoryLocator, SourceCapability, SourceEntry, TargetProfile, admit_definition,
    admit_repositories, resolve,
};
use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};
use kiln_implementer::Implementer;
use kiln_implementer::admission::{AdmissionCommand, AdmissionResult};
use kiln_web_service::worker::materialize::{prepare_input, prepare_overlay, prepare_route};
use kiln_web_service::worker::source_auth::{RequestProfile, SourceAuthorization};
use kiln_web_service::worker::writeback::{observe, retrieve_result, seal};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

const RECEIPT_SCHEMA: &str = "kiln.projected-runtime.receipt.v2";
/// The authority configuration schema (C1) and the largest one read.
const AUTHORITY_CONFIG_SCHEMA: &str = "kiln.implementation-authority-config.v1";
const MAX_AUTHORITY_CONFIG_BYTES: u64 = 64 * 1024;
const CARGO_CORPUS: &str = "fixtures/build-inputs/corpus.json";
const NIX_CORPUS: &str = "fixtures/projected-runtime/nix-corpus.json";
const REPOSITORY: &str = "qualification/disposable";
const OPERATOR: &str = "operator";
const OMP_REVISION: &str = "e325ffc73ab86e03bdd20a91b77d8e2b3964c906";
/// The fixture's exact scripted budget and window (`bind-execution`).
const LEASE_MS: u64 = 600_000;
const FIXTURE_WINDOW_MS: u64 = 180_000;
const DEADLINE_MINUTES: u32 = 2;
const MAX_TURNS: u64 = 4;
const MAX_SECONDS: u64 = 90;
const TOKEN_BUDGET: u64 = 4096;
const OUTPUT_LIMIT_BYTES: u64 = 1_048_576;
const MAX_CORPUS_BYTES: u64 = 256 * 1024;
const PRIVATE_DIRECTORY: u32 = 0o700;
/// The probe program the sandbox runs in place of OMP. It ignores OMP's
/// arguments except the last, the prompt, whose lines are its instructions,
/// and prints one JSON line per observation.
const PROBE: &str = r#"#!@SHELL@
set -u
prompt="${@: -1}"
socat=""; root=""; path=""; cmd=(); envs=()
denied() { printf '{"probe":"%s","target":"%s","denied":%s}\n' "$1" "$2" "$3"; }
while IFS= read -r line; do
  case "$line" in
    socat=*) socat="${line#socat=}" ;;
    path=*) path="${line#path=}" ;;
    root=*) root="${line#root=}" ;;
    command=*) cmd+=("${line#command=}") ;;
    env=*) envs+=("${line#env=}") ;;
    deny_read=*) t="${line#deny_read=}"; if [ -e "$t" ] || cat "$t" >/dev/null 2>&1; then denied read "$t" false; else denied read "$t" true; fi ;;
    allow_read=*) t="${line#allow_read=}"; if cat "$t" >/dev/null 2>&1; then denied read "$t" false; else denied read "$t" true; fi ;;
    deny_write=*) t="${line#deny_write=}"; if ( printf 'probe\n' > "$t" ) 2>/dev/null; then denied write "$t" false; else denied write "$t" true; fi ;;
    deny_unix=*) t="${line#deny_unix=}"; if "$socat" -u OPEN:/dev/null "UNIX-CONNECT:$t" 2>/dev/null; then denied unix "$t" false; else denied unix "$t" true; fi ;;
    deny_abstract=*) t="${line#deny_abstract=}"; if "$socat" -u OPEN:/dev/null "ABSTRACT-CONNECT:$t" 2>/dev/null; then denied abstract "$t" false; else denied abstract "$t" true; fi ;;
    deny_tcp=*) t="${line#deny_tcp=}"; if "$socat" -u OPEN:/dev/null "TCP4:127.0.0.1:$t,connect-timeout=2" 2>/dev/null; then denied tcp "$t" false; else denied tcp "$t" true; fi ;;
    deny_link=*) t="${line#deny_link=}"; d="${t%%|*}"; target="${t#*|}"; if ln -s "$target" "$d/escape" 2>/dev/null && { [ -e "$d/escape" ] || cat "$d/escape/HEAD" >/dev/null 2>&1; }; then denied link "$target" false; else denied link "$target" true; fi; rm -f "$d/escape" ;;
    run)
      cd "/workspace/$root" || exit 3
      env -i "PATH=$path" HOME=/tmp/omp-home CARGO_HOME=/tmp/cargo-home CARGO_TARGET_DIR=/tmp/cargo-target "${envs[@]}" "${cmd[@]}" >/tmp/command.log 2>&1
      status=$?
      printf '{"probe":"command","exit":%s,"tail":"%s"}\n' "$status" "$(tail -n 40 /tmp/command.log | base64 -w0)"
      cd /workspace || exit 3 ;;
    write=*) t="${line#write=}"; file="${t%%:*}"; body="${t#*:}"; if printf '%s' "$body" | base64 -d > "/workspace/$file"; then printf '{"probe":"granted_write","target":"%s","written":true}\n' "$file"; else printf '{"probe":"granted_write","target":"%s","written":false}\n' "$file"; fi ;;
  esac
done <<< "$prompt"
exit 0
"#;

#[derive(Debug, Deserialize)]
struct CorpusFile {
    path: String,
    text: String,
    executable: bool,
}

#[derive(Debug, Deserialize)]
struct Project {
    id: String,
    scope: ProjectScope,
}

#[derive(Debug, Deserialize)]
struct Case {
    id: String,
    definition: ProjectionDefinition,
    request: BuildRequest,
}

#[derive(Debug, Deserialize)]
struct SharedEdit {
    case: String,
    file: CorpusFile,
}

#[derive(Debug, Deserialize)]
struct Corpus {
    schema: String,
    files: Vec<CorpusFile>,
    projects: Vec<Project>,
    cases: Vec<Case>,
    shared_edit: SharedEdit,
}

/// Programs and locations the qualification runs with.
struct Programs {
    lattice: PathBuf,
    dispatch: PathBuf,
    fixture: PathBuf,
    /// The C1 authority configuration every projected dispatch runs under.
    authority_config: PathBuf,
    bwrap: PathBuf,
    socat: PathBuf,
    shell: PathBuf,
    nix: PathBuf,
    scratch: PathBuf,
    out: PathBuf,
    probe: PathBuf,
    path: String,
}

#[derive(Debug, Serialize)]
struct CaseReceipt {
    case: String,
    attempt: ContentId,
    fence: u64,
    route: ProjectedWorkspaceRoute,
    refused_dispatches: Vec<Value>,
    dispatch: Value,
    observations: Vec<Value>,
    command_exit: Option<i64>,
    /// The last lines of the planned command's output, as the probe recorded
    /// them.
    command_tail: Option<String>,
    /// Every probe sent reported exactly its expected outcome, once, and
    /// nothing else was reported.
    probes_complete: bool,
    /// Expected probe outcomes not observed, and observations not expected.
    missing_probes: Vec<Value>,
    unexpected_probes: Vec<Value>,
    host_checks: Vec<Value>,
    source_authority_checks: Vec<Value>,
    capture: Value,
}

/// Refuse to qualify under any authority configuration but a verified
/// profile with projection enabled (C1): the dispatcher never runs a
/// projected workspace under the local-operator or an absent profile.
fn projection_authority(path: &Path) -> Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() > MAX_AUTHORITY_CONFIG_BYTES {
        return Err("the authority configuration is not a bounded ordinary file".into());
    }
    let bytes = fs::read(path)?;
    if !enables_projection(&serde_json::from_slice(&bytes)?) {
        return Err(format!(
            "the authority configuration must be {AUTHORITY_CONFIG_SCHEMA} selecting verified_ucan with projection_enabled; projected dispatch never runs under the local-operator or an absent profile"
        )
        .into());
    }
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

/// Whether an exported C1 configuration selects the verified profile with
/// projection enabled.
fn enables_projection(config: &Value) -> bool {
    config["schema"] == AUTHORITY_CONFIG_SCHEMA
        && config.pointer("/profile/verified_ucan/projection_enabled") == Some(&Value::Bool(true))
}

/// What `beta-tests`' build must report: `include_str!` of alpha's
/// undeclared `assets/banner.txt`, which the route does not hold.
const BETA_MISSING_SOURCE: [&str; 3] = [
    "couldn't read",
    "alpha/assets/banner.txt",
    "No such file or directory",
];

/// `beta-tests` reads a file of alpha's package that no manifest declares;
/// the route does not hold it, so its build must fail inside the runtime,
/// observed and for that reason, rather than escape to the host checkout. A
/// failure for any other reason, a compile or toolchain error, does not
/// qualify. Every other case's command must succeed.
fn command_qualified(case: &str, exit: Option<i64>, tail: Option<&str>) -> bool {
    if case == "beta-tests" {
        exit.is_some_and(|code| code != 0)
            && tail.is_some_and(|tail| {
                BETA_MISSING_SOURCE
                    .iter()
                    .all(|diagnostic| tail.contains(diagnostic))
            })
    } else {
        exit == Some(0)
    }
}

/// A refused dispatch qualifies when it was observed, exited non-zero and
/// names its reason.
fn refusal_qualified(refusal: &Value) -> bool {
    refusal
        .pointer("/outcome/exit")
        .and_then(Value::as_i64)
        .is_some_and(|exit| exit != 0)
        && refusal
            .pointer("/outcome/stderr")
            .and_then(Value::as_str)
            .zip(refusal.get("reason").and_then(Value::as_str))
            .is_some_and(|(stderr, reason)| stderr.contains(reason))
}

fn main() -> ExitCode {
    match run() {
        Ok(passed) if passed => ExitCode::SUCCESS,
        Ok(_) => {
            eprintln!("projected-runtime: a case did not qualify; see the receipt");
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("projected-runtime: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<bool> {
    let flags = flags()?;
    let flag = |key: &str| -> Result<PathBuf> {
        Ok(PathBuf::from(
            flags.get(key).ok_or_else(|| format!("{key} is required"))?,
        ))
    };
    let scratch = flag("--scratch")?;
    fs::create_dir_all(&scratch)?;
    let scratch = fs::canonicalize(&scratch)?;
    fs::set_permissions(&scratch, fs::Permissions::from_mode(PRIVATE_DIRECTORY))?;
    // Every toolchain entry the planned commands need, from the store only.
    let path = std::env::var("PATH")?
        .split(':')
        .filter(|entry| entry.starts_with("/nix/store/"))
        .collect::<Vec<_>>()
        .join(":");
    let authority_config = fs::canonicalize(flag("--authority-config")?)?;
    let authority_config_blake3 = projection_authority(&authority_config)?;
    let mut programs = Programs {
        lattice: flag("--lattice")?,
        dispatch: fs::canonicalize(flag("--dispatch")?)?,
        fixture: fs::canonicalize(flag("--fixture")?)?,
        authority_config,
        bwrap: fs::canonicalize(flag("--bwrap")?)?,
        socat: fs::canonicalize(flag("--socat")?)?,
        shell: fs::canonicalize(flag("--shell")?)?,
        nix: fs::canonicalize(flag("--nix")?)?,
        out: flag("--out")?,
        probe: PathBuf::new(),
        path: format!(
            "{}:{path}",
            flag("--nix")?.parent().ok_or("nix path")?.display()
        ),
        scratch,
    };
    programs.probe = probe(&programs)?;
    let endpoint = std::net::TcpListener::bind("127.0.0.1:0")?;
    let endpoint_port = endpoint.local_addr()?.port();
    std::thread::spawn(move || serve_http_400(&endpoint));
    let mut receipts = Vec::new();
    let mut passed = true;
    for (corpus_path, writable_of) in [
        (CARGO_CORPUS, cargo_writable as fn(&str) -> Vec<String>),
        (NIX_CORPUS, nix_writable as fn(&str) -> Vec<String>),
    ] {
        let corpus = read_corpus(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(corpus_path),
        )?;
        for case in corpus.cases.iter().filter(|case| {
            matches!(
                case.id.as_str(),
                "alpha-tests" | "beta-tests" | "nix-alpha" | "nix-beta"
            )
        }) {
            let receipt = qualify(
                &programs,
                &corpus,
                case,
                &writable_of(&case.id),
                endpoint_port,
            )?;
            passed &= receipt.probes_complete
                && command_qualified(
                    &case.id,
                    receipt.command_exit,
                    receipt.command_tail.as_deref(),
                )
                && receipt.refused_dispatches.iter().all(refusal_qualified)
                && receipt
                    .dispatch
                    .get("exit")
                    .and_then(Value::as_i64)
                    .is_some()
                && receipt.host_checks.iter().all(|check| check["ok"] == true)
                && receipt
                    .source_authority_checks
                    .iter()
                    .all(|check| check["ok"] == true);
            receipts.push(receipt);
        }
    }
    let receipt = json!({
        "schema": RECEIPT_SCHEMA,
        "lattice": programs.lattice,
        "lattice_blake3": blake3_file(&programs.lattice)?,
        "authority_config": programs.authority_config,
        "authority_config_blake3": authority_config_blake3,
        "probe": programs.probe,
        "bwrap": programs.bwrap,
        "cases": receipts,
        "non_claims": [
            "no model was contacted; the worker is a probe in place of OMP",
            "confinement is Lattice's bubblewrap provider on this host only",
            "source authority was verified per operation via Basalt kiln://source/v1/ UCAN grants for materialization, writeback sealing and result retrieval",
            "no source reference was moved or published"
        ],
    });
    let mut bytes = serde_json::to_vec_pretty(&receipt)?;
    bytes.push(b'\n');
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&programs.out)?
        .write_all(&bytes)?;
    Ok(passed)
}

fn cargo_writable(case: &str) -> Vec<String> {
    let project = if case.starts_with("alpha") {
        "alpha"
    } else {
        "beta"
    };
    vec![format!("projects/{project}/src"), "shared/proto/src".into()]
}

fn nix_writable(_case: &str) -> Vec<String> {
    vec!["shared/proto".into()]
}

fn flags() -> Result<BTreeMap<String, String>> {
    let mut arguments = std::env::args().skip(1);
    let mut flags = BTreeMap::new();
    while let Some(flag) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| format!("{flag} requires a value"))?;
        flags.insert(flag, value);
    }
    Ok(flags)
}

fn read_corpus(path: PathBuf) -> Result<Corpus> {
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.len() > MAX_CORPUS_BYTES {
        return Err("the corpus must be a bounded ordinary file".into());
    }
    let corpus: Corpus = serde_json::from_slice(&fs::read(&path)?)?;
    if corpus.schema != "kiln.build-input-corpus.v1" {
        return Err("unsupported corpus schema".into());
    }
    Ok(corpus)
}

/// Place the probe in the Nix store, where the sandbox can execute it.
fn probe(programs: &Programs) -> Result<PathBuf> {
    let directory = programs.scratch.join("kiln-projected-runtime-probe");
    fs::create_dir_all(&directory)?;
    let script = directory.join("probe");
    fs::write(
        &script,
        PROBE.replace("@SHELL@", &programs.shell.to_string_lossy()),
    )?;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755))?;
    let output = Command::new(&programs.nix)
        .args([
            "--extra-experimental-features",
            "nix-command",
            "store",
            "add-path",
        ])
        .arg(&directory)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "nix store add-path failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(PathBuf::from(String::from_utf8(output.stdout)?.trim()).join("probe"))
}

fn serve_http_400(listener: &std::net::TcpListener) {
    for stream in listener.incoming().flatten() {
        let mut stream = stream;
        let mut buffer = [0u8; 4096];
        let _ = stream.read(&mut buffer);
        let body = br#"{"error":{"message":"kiln-projected-runtime-400"}}"#;
        let _ = write!(
            stream,
            "HTTP/1.1 400 Bad Request\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(body);
    }
}

fn content(label: &str) -> Result<ContentId> {
    ContentId::from_blake3_hex(blake3::hash(label.as_bytes()).to_hex().to_string())
        .map_err(|error| format!("{}: {}", error.code, error.message).into())
}

fn blake3_file(path: &Path) -> Result<String> {
    Ok(blake3::hash(&fs::read(path)?).to_hex().to_string())
}

fn diagnostics(errors: Vec<kiln_core::types::Diagnostic>) -> Box<dyn Error> {
    errors
        .into_iter()
        .map(|error| format!("{}: {}", error.code, error.message))
        .collect::<Vec<_>>()
        .join("; ")
        .into()
}

fn write_json(path: &Path, value: &impl Serialize) -> Result {
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn now_ms() -> Result<u64> {
    Ok(u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis(),
    )?)
}

/// One process run with its exit code, stdout and stderr.
fn command(program: &Path, args: &[&std::ffi::OsStr]) -> Result<(i64, String, String)> {
    let output = Command::new(program).args(args).output()?;
    Ok((
        i64::from(output.status.code().unwrap_or(-1)),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    ))
}

/// Commit the corpus at a fixed time, with every file also in the checkout.
fn commit(
    checkout: &Path,
    corpus: &Corpus,
) -> Result<(Repository, Oid, Oid, BTreeMap<String, Oid>)> {
    let repository = Repository::init(checkout)?;
    let mut objects = BTreeMap::new();
    let mut update = TreeUpdateBuilder::new();
    for file in &corpus.files {
        let path = checkout.join(&file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &file.text)?;
        let mode = if file.executable {
            FileMode::BlobExecutable
        } else {
            FileMode::Blob
        };
        let oid = repository.blob(file.text.as_bytes())?;
        update.upsert(file.path.as_str(), oid, mode);
        objects.insert(file.path.clone(), oid);
    }
    let empty = repository.find_tree(repository.treebuilder(None)?.write()?)?;
    let tree = update.create_updated(&repository, &empty)?;
    let signature = Signature::new("Kiln corpus", "corpus@example.invalid", &Time::new(0, 0))?;
    let commit = repository.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Projected runtime corpus",
        &repository.find_tree(tree)?,
        &[],
    )?;
    drop(empty);
    Ok((repository, commit, tree, objects))
}

fn facts(
    corpus: &Corpus,
    repository: &ContentId,
    revision: Oid,
    tree: Oid,
    objects: &BTreeMap<String, Oid>,
) -> Result<ProjectionFacts> {
    let entries = corpus
        .files
        .iter()
        .map(|file| -> Result<SourceEntry> {
            let oid = objects
                .get(&file.path)
                .ok_or("a corpus file has no object")?;
            Ok(SourceEntry {
                path: file.path.clone(),
                object: ExternalRef::new("git", "blob", &oid.to_string())
                    .map_err(|error| error.message)?,
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
            .map_err(|error| error.message)?,
        canonical_tree: content(&tree.to_string())?,
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

fn manifests(corpus: &Corpus) -> ManifestTree {
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

fn work(base: Oid, case: &str, writable: &[String]) -> Result<ImplementationWork> {
    let source_id = format!("projected-runtime/task:{case}");
    Ok(ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_owned(),
        revision: 1,
        actor: "operator".into(),
        repository: REPOSITORY.into(),
        project: "qualification".into(),
        base: Some(ExternalRef::new("git", "commit", &base.to_string()).map_err(|e| e.message)?),
        title: format!("Projected runtime qualification: {case}"),
        requested_outcome: "run the planned build in the projected workspace".into(),
        acceptance: vec![AcceptanceObligation {
            id: "build".into(),
            statement: "the planned command succeeds in the restricted runtime".into(),
        }],
        dependencies: Vec::new(),
        permitted_paths: writable.to_vec(),
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        verification_profile: content("projected-runtime-verification")?,
        budget: WorkBudget {
            attempts: 3,
            minutes_per_attempt: DEADLINE_MINUTES,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::CairnTask,
            source: ExternalRef::new("cairn", "task", &source_id).map_err(|e| e.message)?,
            native_id: source_id.clone(),
            version: content(&format!("readiness:{source_id}"))?,
        }],
    })
}

/// Admit, set up the fixture's local admission, and claim one attempt.
fn claim(
    directory: &Path,
    programs: &Programs,
    work: &ImplementationWork,
) -> Result<(PathBuf, PathBuf, ImplementationAttempt)> {
    let enrollment = Enrollment {
        repositories: BTreeSet::from([REPOSITORY.to_string()]),
        actors: BTreeSet::from(["operator".to_string()]),
    };
    let database = directory.join("kiln.sqlite");
    let enrollment_file = directory.join("enrollment.json");
    write_json(&enrollment_file, &enrollment)?;
    let mut owner = Implementer::open(&database, enrollment.clone())?;
    owner.admit(work, None)?;
    drop(owner);
    let (exit, _, stderr) = command(
        &programs.fixture,
        &[
            "prepare-intake".as_ref(),
            "--database".as_ref(),
            database.as_os_str(),
            "--enrollment".as_ref(),
            enrollment_file.as_os_str(),
        ],
    )?;
    if exit != 0 {
        return Err(format!("prepare-intake failed: {stderr}").into());
    }
    let mut owner = Implementer::open(&database, enrollment)?;
    // The synthetic scripted fixture claims, as `bind-execution` binds, under
    // its explicit local-operator compatibility profile. Only the projected
    // dispatch carries the workspace, and it runs under the authority
    // configuration's verified profile.
    owner.select_local_operator(OPERATOR)?;
    let now = now_ms()?;
    let inputs: Vec<EligibilityInput> = owner
        .admission_snapshot()?
        .eligibility
        .into_values()
        .collect();
    match owner.admission_apply(
        AdmissionCommand::Claim {
            inputs,
            lease_ms: LEASE_MS,
        },
        now,
    )? {
        AdmissionResult::Claimed {
            attempt: Some(attempt),
        } => Ok((database, enrollment_file, *attempt)),
        other => Err(format!("claim did not produce an attempt: {other:?}").into()),
    }
}

#[allow(clippy::too_many_lines)]
fn qualify(
    programs: &Programs,
    corpus: &Corpus,
    case: &Case,
    writable: &[String],
    endpoint_port: u16,
) -> Result<CaseReceipt> {
    // Short names keep Lattice's broker socket, below the state root, within
    // the 108-byte UNIX socket path bound.
    let short: String = case
        .id
        .split('-')
        .filter_map(|part| part.chars().next())
        .collect();
    let directory = programs.scratch.join(short);
    fs::create_dir(&directory)?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(PRIVATE_DIRECTORY))?;
    let checkout = directory.join("checkout");
    let (repository, revision, tree, objects) = commit(&checkout, corpus)?;
    let admitted = admit_repositories(
        &[RepositoryEnrollment {
            repository_id: "projected-runtime-corpus".into(),
            binding: ProviderBinding {
                source_provider: ProviderProfileId::new("git-source-v1").map_err(|e| e.message)?,
                native_repository: ExternalRef::new(
                    "git",
                    "repository",
                    "projected-runtime-corpus",
                )
                .map_err(|e| e.message)?,
            },
            locators: vec![RepositoryLocator::LocalPath {
                path: checkout.to_string_lossy().into_owned(),
            }],
        }],
        &[],
    )
    .map_err(diagnostics)?
    .pop()
    .ok_or("the corpus repository was not admitted")?;
    let source = facts(corpus, admitted.identity(), revision, tree, &objects)?;
    let manifests = manifests(corpus);
    // A writable view of the case's selection, with its sources as creation
    // destination, so the same manifest serves the build plan and the route.
    let mut definition = case.definition.clone();
    definition.profile = ProjectionProfile::Writable;
    definition.creation_destinations = writable
        .iter()
        .filter(|path| path.starts_with("projects/"))
        .cloned()
        .collect();
    let definition = admit_definition(&definition).map_err(diagnostics)?;
    let resolve_for = |operation| {
        resolve(&definition, &admitted, &source)
            .map_err(diagnostics)?
            .require_complete(operation)
            .map_err(|incomplete| diagnostics(incomplete.diagnostics))
    };
    let build = resolve_for(ProjectionOperation::Build)?;
    let write = resolve_for(ProjectionOperation::Write)?;
    let graph = match case.request.profile {
        BuildProfile::CargoCompatibilityV1 => cargo_graph(&manifests),
        BuildProfile::NixCompatibilityV1 => nix_graph(&manifests),
    };
    let build_plan: BuildInputPlan = match case.request.profile {
        BuildProfile::CargoCompatibilityV1 => {
            plan_cargo(&build, &source, &cargo_layout(&manifests), &case.request)
        }
        BuildProfile::NixCompatibilityV1 => plan_nix(&build, &source, &graph, &case.request),
    }
    .map_err(diagnostics)?;

    let work = work(revision, &case.id, writable)?;
    let (database, enrollment_file, attempt) = claim(&directory, programs, &work)?;
    let profile = MaterializerProfile {
        schema: MATERIALIZER_PROFILE_SCHEMA.into(),
        kind: MaterializerKind::EagerLocalDirectory,
        materializer: ComponentVersion {
            name: "kiln-eager-local".into(),
            version: "1".into(),
        },
        platform: TargetProfile::POSIX,
    };
    let materialized = plan(&write, &profile).map_err(diagnostics)?;
    let store = directory.join("store");
    fs::create_dir(&store)?;
    fs::set_permissions(&store, fs::Permissions::from_mode(PRIVATE_DIRECTORY))?;

    let snapshot_revision =
        ExternalRef::new("git", "commit", &revision.to_string()).map_err(|e| e.message)?;
    let source_current = SourceCurrentness {
        generation: kiln_adapter_basalt::test_support::GENERATION,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: snapshot_revision.clone(),
            ref_name: None,
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };
    let attempt_fence = AttemptFence {
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
    };
    let input_fixture = SourceFixture::direct(
        COORDINATOR_SEED,
        admitted.identity(),
        None,
        &[(
            PathScope::root(),
            vec![SourceViewOperation::PrepareWorkspace],
        )],
        EXPIRES_S,
    );
    let mut input_authority = input_fixture.authority();
    let input_signer = input_fixture.signer();
    let sign_input =
        |req: &SourceAuthorityRequest| -> std::result::Result<AuthorityProfile, String> {
            input_signer
                .sign(req)
                .map(AuthorityProfile::VerifiedUcan)
                .map_err(|error| error.message)
        };
    let mut input_auth = SourceAuthorization {
        authority: &mut input_authority,
        profile: RequestProfile::Signer(&sign_input),
        current: &source_current,
        now_unix_s: VERIFY_AT_S,
    };
    prepare_input(&store, &materialized, &repository, &mut input_auth)?;

    let attempt_fixture = SourceFixture::direct(
        VERIFIER_SEED,
        admitted.identity(),
        Some(&attempt_fence),
        &[(
            PathScope::root(),
            vec![
                SourceViewOperation::PrepareWorkspace,
                SourceViewOperation::Discover,
                SourceViewOperation::ReadContent,
                SourceViewOperation::ModifySource,
                SourceViewOperation::CreateSource,
                SourceViewOperation::RetrieveResult,
            ],
        )],
        EXPIRES_S,
    );
    let mut attempt_authority = attempt_fixture.authority();
    let attempt_signer = attempt_fixture.signer();
    let sign_attempt =
        |req: &SourceAuthorityRequest| -> std::result::Result<AuthorityProfile, String> {
            attempt_signer
                .sign(req)
                .map(AuthorityProfile::VerifiedUcan)
                .map_err(|error| error.message)
        };

    let pin = materialized
        .pin(&attempt.identity, attempt.fence)
        .map_err(|error| error.message)?;
    {
        let mut attempt_auth = SourceAuthorization {
            authority: &mut attempt_authority,
            profile: RequestProfile::Signer(&sign_attempt),
            current: &source_current,
            now_unix_s: VERIFY_AT_S,
        };
        prepare_overlay(&store, &materialized, &pin, &mut attempt_auth)?;
    }
    let routed = route(
        &definition,
        &write,
        &materialized,
        &pin,
        RouteRequest {
            store: store.to_str().ok_or("store path is not UTF-8")?,
            writable,
            build_plan: Some(&build_plan),
        },
    )
    .map_err(diagnostics)?;
    let overlay = {
        let mut attempt_auth = SourceAuthorization {
            authority: &mut attempt_authority,
            profile: RequestProfile::Signer(&sign_attempt),
            current: &source_current,
            now_unix_s: VERIFY_AT_S,
        };
        prepare_route(&store, &materialized, &routed, &mut attempt_auth)?
    };
    let route_file = directory.join("route.json");
    write_json(&route_file, &routed)?;

    // Host-side sockets and paths the worker must not reach.
    let unix_socket = directory.join("host.sock");
    let _unix = UnixListener::bind(&unix_socket)?;
    let abstract_name = format!("kiln-projected-runtime-{}-{}", std::process::id(), case.id);
    let _abstract =
        UnixListener::bind_addr(&SocketAddr::from_abstract_name(abstract_name.as_bytes())?)?;
    let tcp = std::net::TcpListener::bind("127.0.0.1:0")?;
    let tcp_port = tcp.local_addr()?.port();
    let shared = &corpus.shared_edit;
    let edits: Vec<(&str, &str)> = if shared.case == case.id {
        vec![(shared.file.path.as_str(), shared.file.text.as_str())]
    } else {
        Vec::new()
    };
    let creation = routed
        .write_paths
        .iter()
        .find(|path| path.starts_with("projects/"))
        .cloned();
    let read_only = build_plan
        .inputs()
        .iter()
        .map(|input| input.path.clone())
        .find(|path| {
            !routed
                .write_paths
                .iter()
                .any(|write| path == write || path.starts_with(&format!("{write}/")))
        })
        .ok_or("no read-only planned input")?;
    let mut prompt = vec![
        format!("socat={}", programs.socat.display()),
        format!("path={}", programs.path),
        format!("root={}", build_plan.build_root()),
    ];
    for (key, value) in build_plan.environment() {
        prompt.push(format!("env={key}={value}"));
    }
    for word in build_plan.command() {
        prompt.push(format!("command={word}"));
    }
    // Every confinement probe sent, with the one observation it must yield.
    let mut expected: Vec<Value> = Vec::new();
    for target in [
        checkout.join(&read_only),
        checkout.join(".git/HEAD"),
        checkout.clone(),
        PathBuf::from("/workspace/.git"),
        store
            .join("inputs")
            .join(pin.input.as_str())
            .join(&read_only),
        store.clone(),
        overlay.join(&read_only),
        PathBuf::from("/nix/var/nix/daemon-socket/socket"),
    ] {
        let target = target.display().to_string();
        prompt.push(format!("deny_read={target}"));
        expected.push(json!({"probe": "read", "target": target, "denied": true}));
    }
    let allowed = format!("/workspace/{read_only}");
    prompt.push(format!("allow_read={allowed}"));
    expected.push(json!({"probe": "read", "target": allowed, "denied": false}));
    let mut denied_writes = vec![
        format!("/workspace/{read_only}"),
        "/workspace/probe-root-file".to_string(),
        "/workspace/.git".to_string(),
        "/nix/store/kiln-projected-runtime-probe-write".to_string(),
        format!("{}/escape", overlay.display()),
    ];
    if let Some(top) = routed.read_paths.first() {
        denied_writes.push(format!("/workspace/{top}/probe-outside-write-paths"));
    }
    for target in denied_writes {
        prompt.push(format!("deny_write={target}"));
        expected.push(json!({"probe": "write", "target": target, "denied": true}));
    }
    for (probe, target) in [
        ("unix", unix_socket.display().to_string()),
        ("abstract", abstract_name.clone()),
        ("tcp", tcp_port.to_string()),
    ] {
        prompt.push(format!("deny_{probe}={target}"));
        expected.push(json!({"probe": probe, "target": target, "denied": true}));
    }
    if let Some(creation) = &creation {
        let git = checkout.join(".git").display().to_string();
        prompt.push(format!("deny_link=/workspace/{creation}|{git}"));
        expected.push(json!({"probe": "link", "target": git, "denied": true}));
    }
    prompt.push("run".into());
    let mut granted: Vec<(String, &[u8])> = edits
        .iter()
        .map(|(path, text)| ((*path).to_string(), text.as_bytes()))
        .collect();
    if let Some(creation) = &creation {
        granted.push((
            format!("{creation}/probe_note.rs"),
            b"// Written in the restricted runtime.\n".as_slice(),
        ));
    }
    for (target, bytes) in granted {
        prompt.push(format!("write={target}:{}", base64(bytes)));
        expected.push(json!({"probe": "granted_write", "target": target, "written": true}));
    }

    let (request_file, plan_file, observations_file) = bind(
        &directory,
        programs,
        &database,
        &enrollment_file,
        &attempt,
        &work,
        &routed,
        &prompt.join("\n"),
        endpoint_port,
    )?;
    let transport = directory.join("transport.json");
    let state = directory.join("s");
    let output = directory.join("transport");
    write_json(
        &transport,
        &json!({"schema": "kiln.lattice-implementation-cli.v2", "lattice_program": programs.lattice,
            "executable_blake3": blake3_file(&programs.lattice)?, "state_directory": state,
            "output_directory": output}),
    )?;
    let dispatch = |request: &Path, route: Option<&Path>, authority: [&std::ffi::OsStr; 2]| {
        let mut args: Vec<&std::ffi::OsStr> = vec![
            "--database".as_ref(),
            database.as_os_str(),
            "--enrollment".as_ref(),
            enrollment_file.as_os_str(),
            "--dispatch-plan".as_ref(),
            plan_file.as_os_str(),
            "--worker-request".as_ref(),
            request.as_os_str(),
            "--worker-transport-profile".as_ref(),
            transport.as_os_str(),
            "--admission-observations".as_ref(),
            observations_file.as_os_str(),
        ];
        args.extend(authority);
        if let Some(route) = route {
            args.push("--projected-workspace-route".as_ref());
            args.push(route.as_os_str());
        }
        let (exit, stdout, stderr) = command(&programs.dispatch, &args)?;
        Ok::<Value, Box<dyn Error>>(json!({"exit": exit, "stdout": stdout, "stderr": stderr}))
    };
    let verified: [&std::ffi::OsStr; 2] = [
        "--authority-config".as_ref(),
        programs.authority_config.as_os_str(),
    ];
    let local_operator: [&std::ffi::OsStr; 2] = ["--local-operator".as_ref(), OPERATOR.as_ref()];
    // Requests that differ from the route, a missing route, and the exact
    // request under the local-operator profile are refused, each for its
    // reason, before any durable step.
    let differs = "nothing was dispatched";
    let request: Value = serde_json::from_slice(&fs::read(&request_file)?)?;
    let mut refused_dispatches = Vec::new();
    // Write paths widened to whole read paths, and a read path to the Git
    // database.
    let widened_paths = routed.read_paths.clone();
    let mut with_git = routed.read_paths.clone();
    with_git.push(".git".into());
    for (label, change) in [
        ("write_paths_widened", json!({"write_paths": widened_paths})),
        ("read_paths_widened", json!({"read_paths": with_git})),
        (
            "workspace_root_moved",
            json!({"workspace_root": routed.input_root}),
        ),
    ] {
        let mut changed = request.clone();
        if let (Some(fields), Some(change)) = (changed.as_object_mut(), change.as_object()) {
            for (key, value) in change {
                fields.insert(key.clone(), value.clone());
            }
        }
        let file = directory.join(format!("request-{label}.json"));
        write_json(&file, &changed)?;
        let outcome = dispatch(&file, Some(&route_file), verified)?;
        refused_dispatches.push(json!({"case": label, "reason": differs, "outcome": outcome}));
    }
    let missing = dispatch(&request_file, None, verified)?;
    refused_dispatches
        .push(json!({"case": "route_missing", "reason": differs, "outcome": missing}));
    let local = dispatch(&request_file, Some(&route_file), local_operator)?;
    refused_dispatches.push(json!({"case": "local_operator", "reason": "projection-requires-operator", "outcome": local}));

    let read_only_before = fs::read(overlay.join(&read_only))?;
    let dispatched = dispatch(&request_file, Some(&route_file), verified)?;
    let observations = retained_events(&state)?;
    let commands: Vec<&Value> = observations
        .iter()
        .filter(|event| event["probe"] == "command")
        .collect();
    let (command_exit, command_tail) = match commands.as_slice() {
        [only] => (
            only["exit"].as_i64(),
            only["tail"]
                .as_str()
                .and_then(|tail| base64::engine::general_purpose::STANDARD.decode(tail).ok())
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned()),
        ),
        _ => (None, None),
    };
    let probed: Vec<&Value> = observations
        .iter()
        .filter(|event| event["probe"] != "command")
        .collect();
    let (missing_probes, unexpected_probes) = probe_difference(&expected, &probed);
    let probes_complete =
        missing_probes.is_empty() && unexpected_probes.is_empty() && commands.len() == 1;
    let host_checks = vec![
        json!({"check": "read-only planned input unchanged in the overlay",
               "ok": fs::read(overlay.join(&read_only))? == read_only_before}),
        json!({"check": "no file written at the workspace root",
               "ok": !overlay.join("probe-root-file").exists()}),
        json!({"check": "no file written outside the write paths",
               "ok": routed.read_paths.first().is_none_or(|top| !overlay.join(top).join("probe-outside-write-paths").exists())}),
        json!({"check": "the host checkout is unchanged",
               "ok": fs::read(checkout.join(&read_only))? == fs::read(overlay.join(&read_only))?}),
    ];
    // Capture the overlay, authorize sealing and result retrieval via Basalt,
    // and derive the facts verification consumes.
    let observe_request = SourceAuthorityRequest {
        operation: SourceViewOperation::RetrieveResult,
        repository: admitted.identity().clone(),
        snapshot: SnapshotRule::Exact {
            revision: snapshot_revision.clone(),
        },
        view_binding: pin.manifest.clone(),
        scope: OriginScope::Components {
            path: PathScope::root(),
        },
        request_digest: *blake3::hash(pin.overlay.as_str().as_bytes()).as_bytes(),
        attempt: Some(attempt_fence.clone()),
    };
    let observed = {
        let mut attempt_auth = SourceAuthorization {
            authority: &mut attempt_authority,
            profile: RequestProfile::Signer(&sign_attempt),
            current: &source_current,
            now_unix_s: VERIFY_AT_S,
        };
        observe(&overlay, &observe_request, &mut attempt_auth)?
    };
    let captured = capture(
        &definition,
        &write,
        &pin,
        &observed,
        &[],
        CurrentAttempt {
            source: &source,
            fence: attempt.fence,
        },
    )
    .map_err(diagnostics)?;
    let ceiling = ViewCeiling::from_definition(&definition);
    let capture = match captured {
        Captured::Sealed(sealed) => {
            let sealed_commit = {
                let mut attempt_auth = SourceAuthorization {
                    authority: &mut attempt_authority,
                    profile: RequestProfile::Signer(&sign_attempt),
                    current: &source_current,
                    now_unix_s: VERIFY_AT_S,
                };
                seal(&repository, &overlay, &source, &sealed, &mut attempt_auth)?
            };
            let restricted_result = {
                let mut attempt_auth = SourceAuthorization {
                    authority: &mut attempt_authority,
                    profile: RequestProfile::Signer(&sign_attempt),
                    current: &source_current,
                    now_unix_s: VERIFY_AT_S,
                };
                retrieve_result(&sealed, &ceiling, &PathScope::root(), &mut attempt_auth)?
            };
            let projects = corpus
                .projects
                .iter()
                .map(|project| -> Result<ProjectInput> {
                    Ok(ProjectInput {
                        project_id: project.id.clone(),
                        scope: Some(
                            admit_scope(&project.id, &project.scope)
                                .map_err(|e| format!("{e:?}"))?,
                        ),
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let graphs: Vec<GraphFacts> = vec![graph];
            let facts = candidate_facts(&sealed, Some(&build_plan), &projects, &graphs)
                .map_err(diagnostics)?;
            json!({
                "sealed": sealed,
                "sealed_commit": sealed_commit.to_string(),
                "restricted_result": restricted_result,
                "facts": facts
            })
        }
        Captured::Conflict(conflict) => json!({"conflict": conflict}),
    };
    let source_authority_checks = verify_negative_source_authority(
        admitted.identity(),
        &snapshot_revision,
        &pin.manifest,
        &attempt_fence,
        &source_current,
    );
    Ok(CaseReceipt {
        case: case.id.clone(),
        attempt: attempt.identity.clone(),
        fence: attempt.fence,
        route: routed,
        refused_dispatches,
        dispatch: dispatched,
        observations,
        command_exit,
        command_tail,
        probes_complete,
        missing_probes,
        unexpected_probes,
        host_checks,
        source_authority_checks,
        capture,
    })
}

/// The expected probe outcomes not observed, and the observations not
/// expected, counting each repetition.
fn probe_difference(expected: &[Value], observed: &[&Value]) -> (Vec<Value>, Vec<Value>) {
    let mut unexpected: Vec<Value> = observed.iter().map(|event| (*event).clone()).collect();
    let mut missing = Vec::new();
    for probe in expected {
        match unexpected.iter().position(|event| event == probe) {
            Some(index) => {
                unexpected.swap_remove(index);
            }
            None => missing.push(probe.clone()),
        }
    }
    (missing, unexpected)
}

/// Seed, prepare, bind through the fixture and prepare again, as an operator
/// does for the scripted local route.
#[allow(clippy::too_many_arguments)]
fn bind(
    directory: &Path,
    programs: &Programs,
    database: &Path,
    enrollment: &Path,
    attempt: &ImplementationAttempt,
    work: &ImplementationWork,
    routed: &ProjectedWorkspaceRoute,
    prompt: &str,
    endpoint_port: u16,
) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let expires = attempt
        .lease_until_ms
        .min(attempt.claimed_at_ms.saturating_add(FIXTURE_WINDOW_MS));
    let source = work.aliases.first().ok_or("work source")?;
    let zero = "0".repeat(64);
    let seed = json!({
        "schema": "lattice.implementation-attempt-run.v3",
        "envelope": {
            "schema": "lattice.implementation-attempt-envelope.v1",
            "work_id": work.identity().map_err(|e| e.message)?.as_str(),
            "work_version": work.revision,
            "source_kind": "cairn_task",
            "source_id": source.native_id,
            "repository": work.repository,
            "base": work.base.as_ref().ok_or("base")?.native_id.as_str(),
            "readiness_snapshot": source.version.as_str(),
            "task_spec_digest": acceptance_digest(work).map_err(|e| e.message)?.as_str(),
            "verification_profile": work.verification_profile.as_str(),
            "attempt_id": attempt.identity.as_str(),
            "fence": attempt.fence,
            "workspace_id": routed.pin.overlay.as_str(),
            "worker_profile": zero,
            "budget": {"max_turns": MAX_TURNS, "max_seconds": MAX_SECONDS, "max_cost_micros": 0},
            "policy_digest": zero,
        },
        "admitted_envelope": "",
        "expires_at_ms": expires,
        "workspace_root": routed.overlay_root,
        "read_paths": routed.read_paths,
        "write_paths": routed.write_paths,
        "program": programs.probe,
        "sandbox_program": programs.bwrap,
        "omp": {
            "revision": OMP_REVISION,
            "executable_blake3": blake3_file(&programs.probe)?,
            "base_identity": zero,
            "model": "lattice/scripted-v1",
            "prompt": prompt,
            "model_broker": {
                "schema": "lattice.candidate-model-broker.v1",
                "endpoint": format!("http://127.0.0.1:{endpoint_port}/v1/chat/completions"),
                "model": "scripted-v1",
                "credential": null,
                "relay": {"path": programs.socat, "blake3": blake3_file(&programs.socat)?},
                "shell": {"path": programs.shell, "blake3": blake3_file(&programs.shell)?},
                "sandbox_blake3": blake3_file(&programs.bwrap)?,
                "expires_at_ms": expires,
                "max_requests": 2,
            },
        },
        "output_limit_bytes": OUTPUT_LIMIT_BYTES,
        "allow_unchanged": false,
        "worker_compute": {"kind": "host_local"},
        "execution_binding": {
            "schema": "lattice.execution-binding.v1",
            "compute": {"kind": "host_local"},
            "model_routes": {"kind": "routes", "routes": [{"name": "primary", "route_identity": format!("blake3:{zero}")}]},
        },
    });
    let seed_file = directory.join("seed.json");
    write_json(&seed_file, &seed)?;
    let prepared = directory.join("request-probe.json");
    let (exit, _, stderr) = command(
        &programs.lattice,
        &[
            "implementation-attempt".as_ref(),
            "prepare".as_ref(),
            "--request".as_ref(),
            seed_file.as_os_str(),
            "--output".as_ref(),
            prepared.as_os_str(),
        ],
    )?;
    if exit != 0 {
        return Err(format!("lattice prepare failed: {stderr}").into());
    }
    let plan_template = ImplementationDispatchPlan {
        schema: DISPATCH_PLAN_SCHEMA.into(),
        work_identity: attempt.work_identity.clone(),
        work_revision: attempt.work_revision,
        attempt_identity: attempt.identity.clone(),
        fence: attempt.fence,
        repository: work.repository.clone(),
        base: work.base.clone(),
        workspace: DispatchWorkspace {
            identity: routed.pin.overlay.clone(),
            isolation: WorkspaceIsolation::IsolatedSnapshot,
            network: false,
            credentials: false,
            projection: Some(routed.pin.clone()),
        },
        provider_profile: ProviderProfileId::new("scripted-local-omp").map_err(|e| e.message)?,
        permitted_paths: work.permitted_paths.clone(),
        permitted_effects: work.permitted_effects.clone(),
        controls: DispatchControls {
            cancel: true,
            pause: false,
            resume: false,
        },
        deadline_minutes: DEADLINE_MINUTES,
        token_budget: TOKEN_BUDGET,
        invocation_budget: u32::try_from(MAX_TURNS)?,
        expected_output: content("projected-runtime-output")?,
        execution: None,
    };
    let template = directory.join("plan-template.json");
    write_json(&template, &plan_template)?;
    let route_file = directory.join("route.json");
    let (exit, stdout, stderr) = command(
        &programs.fixture,
        &[
            "bind-execution".as_ref(),
            "--database".as_ref(),
            database.as_os_str(),
            "--enrollment".as_ref(),
            enrollment.as_os_str(),
            "--attempt".as_ref(),
            attempt.identity.as_str().as_ref(),
            "--fence".as_ref(),
            attempt.fence.to_string().as_ref(),
            "--worker-request".as_ref(),
            prepared.as_os_str(),
            "--dispatch-plan-template".as_ref(),
            template.as_os_str(),
            "--projected-workspace-route".as_ref(),
            route_file.as_os_str(),
        ],
    )?;
    if exit != 0 {
        return Err(format!("bind-execution failed: {stderr}").into());
    }
    let bound: Value = serde_json::from_str(&stdout)?;
    if bound.get("projected_workspace_route") != Some(&serde_json::to_value(routed)?) {
        return Err("bind-execution did not emit the bound route".into());
    }
    let planned_seed = directory.join("seed-planned.json");
    let field = |name: &str| {
        bound
            .get(name)
            .ok_or_else(|| format!("bind-execution omitted {name}"))
    };
    write_json(&planned_seed, field("worker_seed")?)?;
    let request = directory.join("request.json");
    let (exit, _, stderr) = command(
        &programs.lattice,
        &[
            "implementation-attempt".as_ref(),
            "prepare".as_ref(),
            "--request".as_ref(),
            planned_seed.as_os_str(),
            "--output".as_ref(),
            request.as_os_str(),
        ],
    )?;
    if exit != 0 {
        return Err(format!("lattice prepare of the planned seed failed: {stderr}").into());
    }
    let plan_file = directory.join("plan.json");
    write_json(&plan_file, field("dispatch_plan")?)?;
    let observations = directory.join("observations.json");
    write_json(&observations, field("admission_observations")?)?;
    Ok((request, plan_file, observations))
}

/// Every probe observation Lattice retained from the worker's stdout.
fn retained_events(state: &Path) -> Result<Vec<Value>> {
    let mut events = Vec::new();
    let mut pending = vec![state.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .file_name()
                .is_some_and(|name| name == "raw-events.jsonl")
            {
                for line in fs::read_to_string(&path)?.lines() {
                    if let Ok(event) = serde_json::from_str::<Value>(line)
                        && event.get("probe").is_some()
                    {
                        events.push(event);
                    }
                }
            }
        }
    }
    Ok(events)
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::new();
    for chunk in bytes.chunks(3) {
        let triple = chunk.iter().enumerate().fold(0u32, |acc, (index, byte)| {
            acc | (u32::from(*byte) << (16 - 8 * index))
        });
        for index in 0..4 {
            if index <= chunk.len() {
                let sextet = (triple >> (18 - 6 * index)) & 0x3f;
                encoded.push(char::from(
                    ALPHABET.get(sextet as usize).copied().unwrap_or(b'A'),
                ));
            } else {
                encoded.push('=');
            }
        }
    }
    encoded
}

/// Negative Basalt `SourceAuthority` checks run per qualified case:
/// 1) out-of-scope path refused (`CredentialDenied` / `BindingMismatch`),
/// 2) revoked grant refused (`Revoked`),
/// 3) zero/mismatched attempt fence refused (`RequestInvalid` / `CredentialDenied`).
fn verify_negative_source_authority(
    repository: &ContentId,
    revision: &ExternalRef,
    view_binding: &ContentId,
    attempt_fence: &AttemptFence,
    current: &SourceCurrentness,
) -> Vec<Value> {
    let denied_scope =
        PathScope::from_path("projects/allowed/src", true).unwrap_or_else(PathScope::root);
    let fixture = SourceFixture::direct(
        VERIFIER_SEED,
        repository,
        Some(attempt_fence),
        &[(
            denied_scope,
            vec![
                SourceViewOperation::ModifySource,
                SourceViewOperation::PrepareWorkspace,
            ],
        )],
        EXPIRES_S,
    );
    let mut authority = fixture.authority();
    let signer = fixture.signer();
    let ungranted_path =
        PathScope::from_path("projects/secret/src/lib.rs", false).unwrap_or_else(PathScope::root);
    let ungranted_req = SourceAuthorityRequest {
        operation: SourceViewOperation::ModifySource,
        repository: repository.clone(),
        snapshot: SnapshotRule::Exact {
            revision: revision.clone(),
        },
        view_binding: view_binding.clone(),
        scope: OriginScope::Components {
            path: ungranted_path,
        },
        request_digest: [7; 32],
        attempt: Some(attempt_fence.clone()),
    };
    let ungranted_ok = signer.sign(&ungranted_req).ok().is_some_and(|cred| {
        authority
            .admit(
                &ungranted_req,
                &AuthorityProfile::VerifiedUcan(cred),
                current,
                VERIFY_AT_S,
            )
            .is_err()
    });

    let valid_scope_req = SourceAuthorityRequest {
        operation: SourceViewOperation::PrepareWorkspace,
        repository: repository.clone(),
        snapshot: SnapshotRule::Exact {
            revision: revision.clone(),
        },
        view_binding: view_binding.clone(),
        scope: OriginScope::Components {
            path: PathScope::from_path("projects/allowed/src", true)
                .unwrap_or_else(PathScope::root),
        },
        request_digest: [8; 32],
        attempt: Some(attempt_fence.clone()),
    };
    let mut zero_fence_req = valid_scope_req.clone();
    zero_fence_req.attempt = Some(AttemptFence {
        attempt: attempt_fence.attempt.clone(),
        fence: 0,
    });
    let zero_fence_ok = signer.sign(&valid_scope_req).ok().is_some_and(|cred| {
        matches!(
            authority.admit(
                &zero_fence_req,
                &AuthorityProfile::VerifiedUcan(cred),
                current,
                VERIFY_AT_S,
            ),
            Err(denial) if denial.reason == SourceDenialReason::RequestInvalid
        )
    });
    let mut wrong_fence_req = valid_scope_req.clone();
    wrong_fence_req.attempt = Some(AttemptFence {
        attempt: attempt_fence.attempt.clone(),
        fence: attempt_fence.fence.saturating_add(1),
    });
    let wrong_fence_ok = signer.sign(&wrong_fence_req).ok().is_some_and(|cred| {
        matches!(
            authority.admit(
                &wrong_fence_req,
                &AuthorityProfile::VerifiedUcan(cred),
                current,
                VERIFY_AT_S,
            ),
            Err(denial) if denial.reason == SourceDenialReason::CredentialDenied
        )
    });
    let stale_ok = zero_fence_ok && wrong_fence_ok && fixture.replay.calls() == 0;

    fixture.revocations.revoke(&fixture.token);
    let revoked_req = SourceAuthorityRequest {
        operation: SourceViewOperation::PrepareWorkspace,
        repository: repository.clone(),
        snapshot: SnapshotRule::Exact {
            revision: revision.clone(),
        },
        view_binding: view_binding.clone(),
        scope: OriginScope::Components {
            path: PathScope::from_path("projects/allowed/src", true)
                .unwrap_or_else(PathScope::root),
        },
        request_digest: [9; 32],
        attempt: Some(attempt_fence.clone()),
    };
    let revoked_ok = signer.sign(&revoked_req).ok().is_some_and(|cred| {
        matches!(
            authority.admit(
                &revoked_req,
                &AuthorityProfile::VerifiedUcan(cred),
                current,
                VERIFY_AT_S,
            ),
            Err(denial) if denial.reason == SourceDenialReason::Revoked
        )
    });

    vec![
        json!({"check": "unauthorized path refused by Basalt SourceAuthority", "ok": ungranted_ok}),
        json!({"check": "zero/stale attempt fence refused before credential verification", "ok": stale_ok}),
        json!({"check": "revoked kiln://source/v1/ grant refused by Basalt SourceAuthority", "ok": revoked_ok}),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(target: &str, denied: bool) -> Value {
        json!({"probe": "read", "target": target, "denied": denied})
    }

    // r[verify kiln.source.projection.runtime.scenario.confined]
    #[test]
    fn only_the_complete_expected_probe_set_qualifies() {
        let denied = read("/host/checkout", true);
        let allowed = read("/workspace/Cargo.toml", false);
        let unix = json!({"probe": "unix", "target": "/host/sock", "denied": true});
        let granted = json!({"probe": "granted_write", "target": "src/lib.rs", "written": true});
        let expected = vec![
            denied.clone(),
            allowed.clone(),
            unix.clone(),
            granted.clone(),
        ];
        let observed = vec![&denied, &allowed, &unix, &granted];
        assert_eq!(probe_difference(&expected, &observed), (vec![], vec![]));
        // A probe that stopped after its first denied read qualifies nothing
        // it did not run, the granted write included.
        assert_eq!(
            probe_difference(&expected, &[&denied]),
            (vec![allowed.clone(), unix.clone(), granted.clone()], vec![])
        );
        // A wrong outcome is both missing and unexpected; a repeat is
        // unexpected.
        let escaped = read("/host/checkout", false);
        assert_eq!(
            probe_difference(&expected, &[&escaped, &allowed, &unix, &granted]),
            (vec![denied.clone()], vec![escaped.clone()])
        );
        assert_eq!(
            probe_difference(&expected, &[&denied, &allowed, &unix, &granted, &unix]),
            (vec![], vec![unix])
        );
    }

    // r[verify kiln.source.projection.runtime.scenario.qualified]
    #[test]
    fn the_beta_build_must_fail_for_its_missing_source() {
        let missing = "error: couldn't read `/workspace/projects/beta/src/../../alpha/assets/banner.txt`: No such file or directory (os error 2)\n --> src/lib.rs:7:28\nerror: could not compile `beta` (lib) due to 1 previous error\n";
        assert!(command_qualified("beta-tests", Some(101), Some(missing)));
        // An unrelated failure: a compile error, a missing toolchain, or no
        // recorded diagnostic.
        let compile = "error[E0425]: cannot find value `VERSION` in crate `proto`\nerror: could not compile `beta` (lib) due to 1 previous error\n";
        let toolchain = "error: toolchain '1.97.1-x86_64-unknown-linux-gnu' is not installed\n";
        for tail in [Some(compile), Some(toolchain), None] {
            assert!(
                !command_qualified("beta-tests", Some(101), tail),
                "{tail:?}"
            );
        }
        // The missing source without an observed failure does not qualify.
        assert!(!command_qualified("beta-tests", None, Some(missing)));
        assert!(!command_qualified("beta-tests", Some(0), Some(missing)));
        assert!(command_qualified("alpha-tests", Some(0), None));
        assert!(!command_qualified("alpha-tests", None, None));
        assert!(!command_qualified("alpha-tests", Some(1), None));
    }

    // r[verify kiln.source.projection.runtime.scenario.qualified]
    #[test]
    fn a_refusal_qualifies_only_observed_and_for_its_reason() {
        let refusal = |exit: Value, stderr: &str| {
            json!({"case": "local_operator", "reason": "projection-requires-operator",
                   "outcome": {"exit": exit, "stdout": "", "stderr": stderr}})
        };
        assert!(refusal_qualified(&refusal(
            json!(2),
            "kiln-lattice-dispatch: projection-requires-operator: …; nothing was dispatched"
        )));
        // Refused for another reason, not observed, or not refused at all.
        assert!(!refusal_qualified(&refusal(
            json!(2),
            "kiln-lattice-dispatch: the plan pins a projected workspace and requires its route; nothing was dispatched"
        )));
        assert!(!refusal_qualified(&refusal(
            Value::Null,
            "projection-requires-operator"
        )));
        assert!(!refusal_qualified(&refusal(
            json!(0),
            "projection-requires-operator"
        )));
        assert!(!refusal_qualified(
            &json!({"case": "flat", "exit": 2, "stderr": "projection-requires-operator"})
        ));
    }

    // r[verify kiln.source.projection.runtime.scenario.qualified]
    #[test]
    fn only_a_projection_enabled_verified_profile_qualifies() {
        let verified = |enabled: bool| {
            json!({"schema": AUTHORITY_CONFIG_SCHEMA,
                   "profile": {"verified_ucan": {"audience": "did:example:kiln", "projection_enabled": enabled}}})
        };
        assert!(enables_projection(&verified(true)));
        assert!(!enables_projection(&verified(false)));
        assert!(!enables_projection(
            &json!({"schema": AUTHORITY_CONFIG_SCHEMA,
            "profile": {"local_operator": {"operator": "operator"}}})
        ));
        assert!(!enables_projection(
            &json!({"schema": AUTHORITY_CONFIG_SCHEMA, "profile": "absent"})
        ));
        let mut other = verified(true);
        other["schema"] = json!("kiln.implementation-authority-config.v0");
        assert!(!enables_projection(&other));
    }

    #[test]
    fn negative_source_authority_checks_all_pass() {
        let repo = ContentId::from_blake3_hex(
            blake3::hash(b"projected-runtime-repo").to_hex().to_string(),
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        let rev = ExternalRef::new("git", "commit", &"a".repeat(40))
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let binding = ContentId::from_blake3_hex(blake3::hash(b"binding").to_hex().to_string())
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let fence = AttemptFence {
            attempt: ContentId::from_blake3_hex(blake3::hash(b"attempt").to_hex().to_string())
                .unwrap_or_else(|error| unreachable!("{error:?}")),
            fence: 1,
        };
        let current = SourceCurrentness {
            generation: kiln_adapter_basalt::test_support::GENERATION,
            revocation_generation: 0,
            snapshot: SnapshotObservation {
                revision: rev.clone(),
                ref_name: None,
                previous: None,
                fast_forward: false,
            },
            attempt: AttemptCurrentness::Current,
        };
        let checks = verify_negative_source_authority(&repo, &rev, &binding, &fence, &current);
        assert_eq!(checks.len(), 3);
        assert!(checks.iter().all(|c| c["ok"] == true), "{checks:?}");
    }
}
