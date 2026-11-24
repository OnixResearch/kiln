//! Local baseline of the shared projected-workspace corpus (#78).
//!
//! The corpus is written into a temporary bare Git repository with one commit
//! at a fixed time, so revisions, manifests and input identities are
//! deterministic. Each projection is admitted and resolved by Kiln's
//! source-projection core, planned with the eager local-directory profile and
//! materialized through `kiln_web_service::worker::materialize` for every
//! attempt under its dispatch pin, and each contract check records its
//! outcome. Nothing here grants source, workspace or execution authority.
// r[impl kiln.source.projection.materialization.scenario.baseline]
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use git2::build::TreeUpdateBuilder;
use git2::{FileMode, Oid, Repository, Signature, Time};
use kiln_core::implementation::authority::{AttemptCurrentness, AuthorityProfile};
use kiln_core::projection::authority::{
    LocalSourceAuthority, LocalSourceGrant, SnapshotObservation, SourceCurrentness,
    SourceViewOperation,
};
use kiln_core::projection::materialization::{
    MATERIALIZER_PROFILE_SCHEMA, MaterializationPlan, MaterializerKind, MaterializerProfile,
    WorkspacePin, plan,
};
use kiln_core::projection::{
    CompleteProjection, ComponentVersion, Coverage, DependencyFact, DependencyStatus, EntryKind,
    EntryOrigin, ProjectionDefinition, ProjectionFacts, ProjectionOperation, ProviderBinding,
    RepositoryEnrollment, RepositoryLocator, SourceCapability, SourceEntry, TargetProfile,
    admit_definition, admit_repositories, resolve,
};
use kiln_core::types::{ContentId, Diagnostic, ExternalRef, ProviderProfileId};
use kiln_web_service::worker::materialize::{prepare_input, prepare_overlay};
use kiln_web_service::worker::source_auth::{RequestProfile, SourceAuthorization};
use serde::{Deserialize, Serialize};

pub type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

pub const CORPUS_SCHEMA: &str = "kiln.workspace-corpus.v1";
pub const RECEIPT_SCHEMA: &str = "kiln.workspace-baseline.receipt.v2";
/// Largest corpus document the harness admits.
pub const MAX_CORPUS_BYTES: u64 = 256 * 1024;
const MAX_FILES: usize = 64;
const MAX_PROJECTIONS: usize = 8;
const MAX_ATTEMPTS: usize = 4;
const MAX_TEXT_BYTES: usize = 4_096;
/// Every overlay is prepared for the first fence of its attempt.
const FENCE: u64 = 1;
const PRIVATE_DIRECTORY: u32 = 0o700;
const SEALED_FILE: u32 = 0o444;
const SEALED_EXECUTABLE: u32 = 0o555;
const WRITABLE_FILE: u32 = 0o644;
const WRITABLE_EXECUTABLE: u32 = 0o755;
const NON_CLAIMS: [&str; 2] = [
    "The corpus is synthetic and small; timings describe this host and corpus only.",
    "Local modes and copies are not a confinement boundary against a privileged process.",
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
struct CorpusDependency {
    name: String,
    identity: ContentId,
}

/// The `kiln.workspace-corpus.v1` export of `config/workspace-corpus-contracts.ncl`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    schema: String,
    files: Vec<CorpusFile>,
    dependencies: Vec<CorpusDependency>,
    projections: Vec<ProjectionDefinition>,
    attempts: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Receipt {
    pub schema: &'static str,
    /// BLAKE3 over the corpus document.
    pub corpus: ContentId,
    pub materializer_profile: ContentId,
    pub revision: String,
    pub projections: Vec<ProjectionRecord>,
    pub checks: Vec<CheckRecord>,
    /// Measured on this host; never part of the committed expectation.
    pub timings: Vec<Timing>,
    pub non_claims: [&'static str; 2],
}

#[derive(Debug, Serialize)]
pub struct ProjectionRecord {
    pub id: String,
    pub manifest: ContentId,
    pub input: ContentId,
    pub dependencies: Vec<String>,
    pub entries: Vec<EntryRecord>,
    pub overlays: Vec<OverlayRecord>,
}

#[derive(Debug, Serialize)]
pub struct EntryRecord {
    pub destination: String,
    pub executable: bool,
    pub origin: ContentId,
    pub object: String,
}

#[derive(Debug, Serialize)]
pub struct OverlayRecord {
    pub attempt: String,
    pub overlay: ContentId,
}

#[derive(Debug, Serialize)]
pub struct CheckRecord {
    pub check: &'static str,
    #[serde(flatten)]
    pub result: Outcome,
}

#[derive(Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    Passed { detail: String },
    Failed { detail: String },
}

#[derive(Debug, Serialize)]
pub struct Timing {
    pub projection: String,
    pub input_cold_us: u64,
    pub input_reuse_us: u64,
    pub overlays_us: u64,
}

struct Overlay {
    label: String,
    pin: WorkspacePin,
    path: PathBuf,
}

struct View {
    id: String,
    complete: CompleteProjection,
    plan: MaterializationPlan,
    input: PathBuf,
    reused: PathBuf,
    fresh: PathBuf,
    overlays: Vec<Overlay>,
    timing: Timing,
}

/// Every file below `root` as (relative path, mode, bytes), in path order.
type Snapshot = Vec<(String, u32, Vec<u8>)>;

/// Run the corpus through the local projected materializer under `scratch`.
/// A malformed corpus is refused before anything is written.
pub fn run(bytes: &[u8], scratch: &Path) -> Result<Receipt> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_CORPUS_BYTES {
        return Err("the corpus exceeds its byte bound".into());
    }
    let corpus: Corpus = serde_json::from_slice(bytes)?;
    admit(&corpus)?;
    let result = measure(&corpus, bytes, scratch);
    // Sealed inputs are read-only; make them removable either way.
    release(scratch);
    result
}

fn measure(corpus: &Corpus, bytes: &[u8], scratch: &Path) -> Result<Receipt> {
    let source = scratch.join("source.git");
    let repository = Repository::init_bare(&source)?;
    let (revision, tree, objects) = commit(&repository, corpus)?;
    let admitted = admit_repositories(&[enrollment(&source)?], &[])
        .map_err(diagnostics)?
        .pop()
        .ok_or("the corpus repository was not admitted")?;
    let facts = facts(corpus, admitted.identity(), &revision, &tree, &objects)?;
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
    let fresh_store = private(scratch.join("fresh-store"))?;
    let attempts = corpus
        .attempts
        .iter()
        .map(|label| Ok((label.clone(), digest(label.as_bytes())?)))
        .collect::<Result<Vec<_>>>()?;
    let mut views = Vec::with_capacity(corpus.projections.len());
    for definition in &corpus.projections {
        let complete = resolve(
            &admit_definition(definition).map_err(diagnostics)?,
            &admitted,
            &facts,
        )
        .map_err(diagnostics)?
        .require_complete(ProjectionOperation::Build)
        .map_err(|incomplete| diagnostics(incomplete.diagnostics))?;
        let planned = plan(&complete, &profile).map_err(diagnostics)?;
        views.push(materialize(
            definition.id.clone(),
            complete,
            planned,
            &repository,
            &store,
            &fresh_store,
            &attempts,
        )?);
    }
    let checks = check(corpus, &views, &repository, &fresh_store)?;
    let projections = views.iter().map(record).collect::<Result<Vec<_>>>()?;
    Ok(Receipt {
        schema: RECEIPT_SCHEMA,
        corpus: digest(bytes)?,
        materializer_profile: profile.identity(),
        revision: revision.to_string(),
        projections,
        checks,
        timings: views.into_iter().map(|view| view.timing).collect(),
        non_claims: NON_CLAIMS,
    })
}

fn admit(corpus: &Corpus) -> Result {
    let safe = |path: &str| {
        !path.is_empty()
            && Path::new(path)
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
    };
    let unique = |values: Vec<&str>| values.iter().collect::<BTreeSet<_>>().len() == values.len();
    if corpus.schema != CORPUS_SCHEMA
        || corpus.files.is_empty()
        || corpus.files.len() > MAX_FILES
        || !(2..=MAX_PROJECTIONS).contains(&corpus.projections.len())
        || !(2..=MAX_ATTEMPTS).contains(&corpus.attempts.len())
        || !corpus
            .files
            .iter()
            .all(|file| safe(&file.path) && file.text.len() <= MAX_TEXT_BYTES)
        || !unique(corpus.files.iter().map(|file| file.path.as_str()).collect())
        || !unique(
            corpus
                .projections
                .iter()
                .map(|definition| definition.id.as_str())
                .collect(),
        )
        || !unique(corpus.attempts.iter().map(String::as_str).collect())
        || !unique(
            corpus
                .dependencies
                .iter()
                .map(|dependency| dependency.name.as_str())
                .collect(),
        )
    {
        return Err("the corpus is malformed, duplicated or over bound".into());
    }
    Ok(())
}

/// One commit of the whole corpus at a fixed time and signature.
fn commit(repository: &Repository, corpus: &Corpus) -> Result<(Oid, Oid, BTreeMap<String, Oid>)> {
    let mut objects = BTreeMap::new();
    let mut update = TreeUpdateBuilder::new();
    for file in &corpus.files {
        let oid = repository.blob(file.text.as_bytes())?;
        let mode = if file.executable {
            FileMode::BlobExecutable
        } else {
            FileMode::Blob
        };
        update.upsert(file.path.as_str(), oid, mode);
        objects.insert(file.path.clone(), oid);
    }
    let empty = repository.find_tree(repository.treebuilder(None)?.write()?)?;
    let tree = update.create_updated(repository, &empty)?;
    let signature = Signature::new("Kiln corpus", "corpus@example.invalid", &Time::new(0, 0))?;
    let commit = repository.commit(
        None,
        &signature,
        &signature,
        "Shared projected-workspace corpus",
        &repository.find_tree(tree)?,
        &[],
    )?;
    Ok((commit, tree, objects))
}

fn enrollment(source: &Path) -> Result<RepositoryEnrollment> {
    Ok(RepositoryEnrollment {
        repository_id: "workspace-corpus".into(),
        binding: ProviderBinding {
            source_provider: ProviderProfileId::new("git-source-v1").map_err(diagnostic)?,
            native_repository: ExternalRef::new("git", "repository", "workspace-corpus")
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
        dependencies: corpus
            .dependencies
            .iter()
            .map(|dependency| DependencyFact {
                name: dependency.name.clone(),
                status: DependencyStatus::Present {
                    identity: dependency.identity.clone(),
                },
            })
            .collect(),
        generated: Vec::new(),
    })
}

fn baseline_auth(
    plan: &MaterializationPlan,
) -> Result<(LocalSourceAuthority, AuthorityProfile, SourceCurrentness)> {
    let authority = LocalSourceAuthority::new(
        "workspace-baseline-harness",
        LocalSourceGrant {
            repositories: BTreeSet::from([plan.repository().clone()]),
            operations: BTreeSet::from([SourceViewOperation::PrepareWorkspace]),
        },
    )
    .map_err(diagnostic)?;
    let profile = AuthorityProfile::LocalOperator {
        operator: "workspace-baseline-harness".into(),
    };
    let current = SourceCurrentness {
        generation: 0,
        revocation_generation: 0,
        snapshot: SnapshotObservation {
            revision: plan.source().clone(),
            ref_name: None,
            previous: None,
            fast_forward: false,
        },
        attempt: AttemptCurrentness::Current,
    };
    Ok((authority, profile, current))
}

fn materialize(
    id: String,
    complete: CompleteProjection,
    planned: MaterializationPlan,
    repository: &Repository,
    store: &Path,
    fresh_store: &Path,
    attempts: &[(String, ContentId)],
) -> Result<View> {
    let (mut local_auth, profile, current) = baseline_auth(&planned)?;
    let mut auth = SourceAuthorization {
        authority: &mut local_auth,
        profile: RequestProfile::Static(&profile),
        current: &current,
        now_unix_s: 1,
    };
    let started = Instant::now();
    let input = prepare_input(store, &planned, repository, &mut auth)?;
    let input_cold = started.elapsed();
    let started = Instant::now();
    let reused = prepare_input(store, &planned, repository, &mut auth)?;
    let input_reuse = started.elapsed();
    let fresh = prepare_input(fresh_store, &planned, repository, &mut auth)?;
    let mut overlays = Vec::with_capacity(attempts.len());
    let mut overlays_time = Duration::ZERO;
    for (label, attempt) in attempts {
        let pin = planned.pin(attempt, FENCE).map_err(diagnostic)?;
        let started = Instant::now();
        let path = prepare_overlay(store, &planned, &pin, &mut auth)?;
        overlays_time += started.elapsed();
        overlays.push(Overlay {
            label: label.clone(),
            pin,
            path,
        });
    }
    let micros = |duration: Duration| u64::try_from(duration.as_micros()).unwrap_or(u64::MAX);
    Ok(View {
        timing: Timing {
            projection: id.clone(),
            input_cold_us: micros(input_cold),
            input_reuse_us: micros(input_reuse),
            overlays_us: micros(overlays_time),
        },
        id,
        complete,
        plan: planned,
        input,
        reused,
        fresh,
        overlays,
    })
}

/// Every contract check, in a fixed order. Edits made by the isolation check
/// are what the reopening check later expects to find.
fn check(
    corpus: &Corpus,
    views: &[View],
    repository: &Repository,
    fresh_store: &Path,
) -> Result<Vec<CheckRecord>> {
    let projections = views.len();
    let mut records = Vec::new();

    let reproducible = views
        .iter()
        .map(|view| Ok(snapshot(&view.input)? == snapshot(&view.fresh)?))
        .collect::<Result<Vec<bool>>>()?;
    records.push(record_check(
        "reproducible_input",
        reproducible.iter().all(|same| *same),
        format!(
            "{projections} projections materialized into two stores with byte- and mode-identical inputs"
        ),
    ));

    let mut isolated = true;
    for view in views {
        let [first, second, ..] = view.overlays.as_slice() else {
            return Err("every projection needs two attempt overlays".into());
        };
        let input_before = snapshot(&view.input)?;
        let second_before = snapshot(&second.path)?;
        let entry = view
            .plan
            .entries()
            .first()
            .ok_or("a projection has no entries")?;
        fs::write(
            first.path.join(&entry.destination),
            b"edited by the first attempt\n",
        )?;
        fs::write(first.path.join("created-by-first-attempt"), b"new\n")?;
        isolated &= snapshot(&view.input)? == input_before
            && snapshot(&second.path)? == second_before
            && fs::read(first.path.join(&entry.destination))? == b"edited by the first attempt\n";
    }
    records.push(record_check(
        "independent_attempts",
        isolated,
        format!(
            "edits in the first attempt's overlay left the second attempt's overlay and the shared input unchanged in {projections} projections"
        ),
    ));

    records.push(record_check(
        "verified_input_reuse",
        views.iter().all(|view| view.reused == view.input),
        "a second preparation re-verified and reused the published input of every projection"
            .into(),
    ));

    let (mut executable, mut other, mut modes) = (0_usize, 0_usize, true);
    for view in views {
        let second = view
            .overlays
            .get(1)
            .ok_or("a projection lacks a second overlay")?;
        for entry in view.plan.entries() {
            let (sealed, writable) = if entry.executable {
                executable += 1;
                (SEALED_EXECUTABLE, WRITABLE_EXECUTABLE)
            } else {
                other += 1;
                (SEALED_FILE, WRITABLE_FILE)
            };
            modes &= mode(&view.input.join(&entry.destination))? == sealed
                && mode(&second.path.join(&entry.destination))? == writable;
        }
    }
    records.push(record_check(
        "executable_modes",
        modes && executable > 0,
        format!(
            "{executable} executable and {other} other entries keep sealed 0555/0444 input modes and 0755/0644 overlay modes"
        ),
    ));

    let mut origins: BTreeMap<String, BTreeMap<ContentId, Vec<u8>>> = BTreeMap::new();
    for view in views {
        for entry in &view.complete.manifest().entries {
            if let EntryOrigin::Native {
                object,
                origin_identity,
                ..
            } = &entry.origin
            {
                let bytes = fs::read(view.input.join(&entry.destination))?;
                origins
                    .entry(object.native_id.as_str().to_owned())
                    .or_default()
                    .insert(origin_identity.clone(), bytes);
            }
        }
    }
    let shared_bytes: Vec<&BTreeMap<ContentId, Vec<u8>>> = origins
        .values()
        .filter(|by_origin| by_origin.len() > 1)
        .collect();
    let distinct = shared_bytes.iter().all(|by_origin| {
        let mut contents = by_origin.values();
        let first = contents.next();
        contents.all(|bytes| Some(bytes) == first)
    });
    let origin_count: usize = shared_bytes.iter().map(|by_origin| by_origin.len()).sum();
    records.push(record_check(
        "distinct_origins",
        distinct && !shared_bytes.is_empty(),
        format!(
            "objects with equal bytes at several origins: {}; distinct origin identities: {origin_count}; materialized bytes identical",
            shared_bytes.len()
        ),
    ));

    let mut exact = true;
    let mut excluded = 0_usize;
    for view in views {
        let manifest = view.complete.manifest();
        let destinations: Vec<String> = manifest
            .entries
            .iter()
            .map(|entry| entry.destination.clone())
            .collect();
        let found: Vec<String> = snapshot(&view.fresh)?
            .into_iter()
            .map(|(name, _, _)| name)
            .collect();
        exact &= found == destinations;
        let origins: BTreeSet<&str> = manifest
            .entries
            .iter()
            .filter_map(|entry| match &entry.origin {
                EntryOrigin::Native { path, .. } => Some(path.as_str()),
                EntryOrigin::Generated { .. } => None,
            })
            .collect();
        let definition = corpus
            .projections
            .iter()
            .find(|definition| definition.id == view.id)
            .ok_or("a view has no definition")?;
        excluded += corpus
            .files
            .iter()
            .filter(|file| {
                definition
                    .mounts
                    .iter()
                    .any(|mount| Path::new(&file.path).starts_with(&mount.source_prefix))
                    && !origins.contains(file.path.as_str())
            })
            .count();
    }
    records.push(record_check(
        "excluded_entry_absent",
        exact && excluded > 0,
        format!(
            "excluded corpus files absent: {excluded}; every input holds exactly its manifest entries"
        ),
    ));

    let mut common: Option<BTreeMap<String, ContentId>> = None;
    for view in views {
        let entries: BTreeMap<String, ContentId> = view
            .complete
            .manifest()
            .entries
            .iter()
            .filter_map(|entry| match &entry.origin {
                EntryOrigin::Native {
                    origin_identity, ..
                } => Some((entry.destination.clone(), origin_identity.clone())),
                EntryOrigin::Generated { .. } => None,
            })
            .collect();
        common = Some(match common {
            None => entries,
            Some(shared) => shared
                .into_iter()
                .filter(|(destination, origin)| entries.get(destination) == Some(origin))
                .collect(),
        });
    }
    let common = common.unwrap_or_default();
    let mut identical = true;
    for destination in common.keys() {
        let mut contents = views
            .iter()
            .map(|view| fs::read(view.input.join(destination)))
            .collect::<std::io::Result<Vec<_>>>()?
            .into_iter();
        let first = contents.next();
        identical &= contents.all(|bytes| Some(bytes) == first);
    }
    let names: BTreeSet<&str> = corpus
        .dependencies
        .iter()
        .map(|dependency| dependency.name.as_str())
        .collect();
    let bound = views.iter().all(|view| {
        view.complete.manifest().dependencies.iter().all(|binding| {
            names.contains(binding.name.as_str())
                && matches!(binding.status, DependencyStatus::Present { .. })
        })
    });
    records.push(record_check(
        "shared_dependency",
        identical && bound && !common.is_empty(),
        format!(
            "{} shared entries and the declared dependencies are bound in all {projections} projections with identical bytes",
            common.len()
        ),
    ));

    let first_view = views.first().ok_or("the corpus has no projection")?;
    let tampered = first_view
        .plan
        .entries()
        .first()
        .ok_or("a projection has no entries")?;
    let path = first_view.fresh.join(&tampered.destination);
    let sealed = mode(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(WRITABLE_FILE))?;
    fs::write(&path, b"damaged after publication\n")?;
    fs::set_permissions(&path, fs::Permissions::from_mode(sealed))?;
    let (mut local_auth, profile, current) = baseline_auth(&first_view.plan)?;
    let mut auth = SourceAuthorization {
        authority: &mut local_auth,
        profile: RequestProfile::Static(&profile),
        current: &current,
        now_unix_s: 1,
    };
    records.push(record_check(
        "damaged_input_refused",
        prepare_input(fresh_store, &first_view.plan, repository, &mut auth).is_err(),
        "a published input whose file changed after publication was refused on reuse".into(),
    ));

    let first_overlay = first_view
        .overlays
        .first()
        .ok_or("a projection has no overlay")?;
    let resumed = prepare_overlay(
        &store_of(&first_view.input)?,
        &first_view.plan,
        &first_overlay.pin,
        &mut auth,
    )?;
    let kept = fs::read(resumed.join("created-by-first-attempt")).is_ok();
    let mut forged = first_overlay.pin.clone();
    forged.manifest = digest(b"another manifest")?;
    let refused = prepare_overlay(
        &store_of(&first_view.input)?,
        &first_view.plan,
        &forged,
        &mut auth,
    )
    .is_err();
    records.push(record_check(
        "exact_pin_reopen",
        resumed == first_overlay.path && kept && refused,
        format!(
            "{}'s overlay reopened with its edits under the exact pin, and a pin naming another manifest was refused",
            first_overlay.label
        ),
    ));
    Ok(records)
}

fn record_check(check: &'static str, passed: bool, detail: String) -> CheckRecord {
    CheckRecord {
        check,
        result: if passed {
            Outcome::Passed { detail }
        } else {
            Outcome::Failed { detail }
        },
    }
}

fn record(view: &View) -> Result<ProjectionRecord> {
    let manifest = view.complete.manifest();
    let entries = manifest
        .entries
        .iter()
        .filter_map(|entry| match &entry.origin {
            EntryOrigin::Native {
                object,
                origin_identity,
                ..
            } => Some(EntryRecord {
                destination: entry.destination.clone(),
                executable: entry.kind == EntryKind::Executable,
                origin: origin_identity.clone(),
                object: object.native_id.as_str().to_owned(),
            }),
            EntryOrigin::Generated { .. } => None,
        })
        .collect();
    Ok(ProjectionRecord {
        id: view.id.clone(),
        manifest: manifest.identity.clone(),
        input: view.plan.input_identity().clone(),
        dependencies: manifest
            .dependencies
            .iter()
            .map(|binding| binding.name.clone())
            .collect(),
        entries,
        overlays: view
            .overlays
            .iter()
            .map(|overlay| OverlayRecord {
                attempt: overlay.label.clone(),
                overlay: overlay.pin.overlay.clone(),
            })
            .collect(),
    })
}

/// The store a published input lives in: `<store>/inputs/<identity>`.
fn store_of(input: &Path) -> Result<PathBuf> {
    Ok(input
        .parent()
        .and_then(Path::parent)
        .ok_or("an input is outside its store")?
        .to_path_buf())
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

fn mode(path: &Path) -> Result<u32> {
    Ok(fs::symlink_metadata(path)?.mode() & 0o7777)
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
