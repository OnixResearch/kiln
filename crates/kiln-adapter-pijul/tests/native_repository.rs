use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use kiln_adapter_pijul::{LocalPijulAdapter, PijulSourceProfile, resolve_and_acquire};
use kiln_core::source::{SourceNativeFacts, SourceResolution};
use kiln_core::types::ExternalRef;
use libpijul::change::{Change, ChangeHeader};
use libpijul::changestore::ChangeStore;
use libpijul::record::{Algorithm, Builder};
use libpijul::{MutTxnT, MutTxnTExt, TxnT};
use pijul_repository::Repository;
use tempfile::TempDir;

const EXPECTED_PIJUL_VERSION: &str = "pijul 1.0.0-beta.11";
const EXPECTED_DEPENDENT_CHANGE_COUNT: usize = 2;
const FIXTURE_CHANNEL: &str = "main";
const FIXTURE_FILE: &str = "file.txt";
const RECORD_THREADS: usize = 1;
const RECORD_SALT: u64 = 0;

fn executable() -> io::Result<PathBuf> {
    std::env::var_os("KILN_PIJUL_EXE")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("KILN_PIJUL_EXE is required for native fixture tests"))
}

fn run(executable: &Path, cwd: &Path, home: &Path, arguments: &[&str]) -> io::Result<Output> {
    let output = Command::new(executable)
        .args(arguments)
        .current_dir(cwd)
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "Pijul fixture command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output)
}

fn record_fixture(
    repository_path: &Path,
    channel_name: &str,
    message: &str,
    add_file: bool,
) -> anyhow::Result<()> {
    let repository = Repository::find_root(Some(repository_path))?;
    let transaction = repository.pristine.arc_txn_begin()?;
    let channel = {
        let read = transaction.read();
        read.load_channel(channel_name)?
            .ok_or_else(|| anyhow::anyhow!("fixture channel missing"))?
    };
    if add_file {
        transaction.write().add(FIXTURE_FILE, false, RECORD_SALT)?;
    }
    let mut builder = Builder::new();
    builder.record(
        transaction.clone(),
        Algorithm::default(),
        false,
        &libpijul::DEFAULT_SEPARATOR,
        channel.clone(),
        &repository.working_copy,
        &repository.changes,
        "",
        RECORD_THREADS,
    )?;
    let recorded = builder.finish();
    let actions = {
        let read = transaction.read();
        recorded
            .actions
            .into_iter()
            .map(|action| action.globalize(&*read))
            .collect::<Result<Vec<_>, _>>()?
    };
    let contents = std::mem::take(&mut *recorded.contents.lock());
    let mut change = Change::make_change(
        &*transaction.read(),
        &channel,
        actions,
        contents,
        ChangeHeader {
            message: message.to_string(),
            ..ChangeHeader::default()
        },
        Vec::new(),
    )?;
    let hash = repository
        .changes
        .save_change(&mut change, |_, _| Ok::<_, anyhow::Error>(()))?;
    libpijul::apply::apply_local_change(
        &mut *transaction.write(),
        &channel,
        &change,
        &hash,
        &recorded.updatables,
    )?;
    transaction.commit()?;
    Ok(())
}

fn fork_channel(repository_path: &Path, channel_name: &str) -> anyhow::Result<()> {
    let repository = Repository::find_root(Some(repository_path))?;
    let mut transaction = repository.pristine.mut_txn_begin()?;
    let source = transaction
        .load_channel(FIXTURE_CHANNEL)?
        .ok_or_else(|| anyhow::anyhow!("fixture source channel missing"))?;
    let _fork = transaction.fork(&source, channel_name)?;
    transaction.commit()?;
    Ok(())
}

fn first_file(root: &Path) -> io::Result<Option<PathBuf>> {
    let mut entries = fs::read_dir(root)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if path.is_file() {
            return Ok(Some(path));
        }
        if path.is_dir()
            && let Some(found) = first_file(&path)?
        {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

fn create_repository(
    executable: &Path,
    label: &str,
    message: &str,
) -> io::Result<(TempDir, PathBuf)> {
    let temporary = tempfile::tempdir()?;
    let home = temporary.path().join("home");
    let repository = temporary.path().join(label);
    fs::create_dir_all(&home)?;
    let repository_text = repository
        .to_str()
        .ok_or_else(|| io::Error::other("fixture path is not UTF-8"))?;
    run(
        executable,
        temporary.path(),
        &home,
        &["init", repository_text],
    )?;
    fs::write(repository.join(FIXTURE_FILE), b"fixture content\n")?;
    record_fixture(&repository, FIXTURE_CHANNEL, message, true)
        .map_err(|_| io::Error::other("libpijul fixture recording failed"))?;
    Ok((temporary, repository))
}

fn resolve_channel(repository: &Path, channel: &str) -> io::Result<SourceResolution> {
    let profile = PijulSourceProfile::pinned_local();
    let adapter = LocalPijulAdapter::new(
        repository.to_path_buf(),
        channel.to_string(),
        profile.clone(),
    )
    .map_err(|_| io::Error::other("native adapter construction failed"))?;
    resolve_and_acquire(
        &adapter,
        ExternalRef::new("pijul", "channel", channel)
            .map_err(|_| io::Error::other("native channel identity failed"))?,
        profile.id.clone(),
        &profile,
    )
    .map_err(|_| io::Error::other("native Pijul resolution failed"))
}

fn resolve(repository: &Path) -> io::Result<SourceResolution> {
    resolve_channel(repository, FIXTURE_CHANNEL)
}

// r[verify kiln.source.pijul_conformance]
// r[verify kiln.source.pijul_conformance.scenario.native]
// r[verify kiln.source.pijul_conformance.scenario.repeat]
#[test]
fn pinned_cli_fixture_resolves_repeatably() -> Result<(), Box<dyn std::error::Error>> {
    let executable = executable()?;
    let temporary_home = tempfile::tempdir()?;
    let version = run(
        &executable,
        temporary_home.path(),
        temporary_home.path(),
        &["--version"],
    )?;
    assert_eq!(
        String::from_utf8(version.stdout)?.trim(),
        EXPECTED_PIJUL_VERSION
    );
    let (_temporary, repository) = create_repository(&executable, "repeat", "first history")?;
    let first = resolve(&repository)?;
    let second = resolve(&repository)?;
    assert_eq!(first, second);

    fs::write(repository.join(FIXTURE_FILE), b"unrecorded bytes\n")?;
    let with_unrecorded_working_copy = resolve(&repository)?;
    assert_eq!(first, with_unrecorded_working_copy);
    Ok(())
}

// r[verify kiln.source.pijul_snapshot.scenario.history]
#[test]
fn equal_native_trees_keep_distinct_history() -> Result<(), Box<dyn std::error::Error>> {
    let executable = executable()?;
    let (_first_temporary, first_repository) =
        create_repository(&executable, "first", "first history")?;
    let (_second_temporary, second_repository) =
        create_repository(&executable, "second", "second history")?;
    let first = resolve(&first_repository)?;
    let second = resolve(&second_repository)?;
    assert_eq!(first.canonical_tree, second.canonical_tree);
    assert_ne!(first.exact_revision, second.exact_revision);
    assert_ne!(first.native_facts, second.native_facts);
    Ok(())
}

#[test]
fn dependent_changes_and_separate_channels_keep_exact_native_states()
-> Result<(), Box<dyn std::error::Error>> {
    let executable = executable()?;
    let (_temporary, repository) = create_repository(&executable, "channels", "first")?;
    fs::write(repository.join(FIXTURE_FILE), b"second content\n")?;
    record_fixture(&repository, FIXTURE_CHANNEL, "second", false)?;
    let dependent = resolve(&repository)?;
    let change_count = match &dependent.native_facts {
        SourceNativeFacts::Pijul(facts) => facts.change_sequence.len(),
        _ => return Err(io::Error::other("fixture returned non-Pijul facts").into()),
    };
    assert_eq!(change_count, EXPECTED_DEPENDENT_CHANGE_COUNT);

    let feature_channel = "feature";
    fork_channel(&repository, feature_channel)?;
    fs::write(repository.join(FIXTURE_FILE), b"feature content\n")?;
    record_fixture(&repository, feature_channel, "feature", false)?;
    let main = resolve(&repository)?;
    let feature = resolve_channel(&repository, feature_channel)?;
    assert_ne!(main.exact_revision, feature.exact_revision);
    assert_ne!(main.canonical_tree, feature.canonical_tree);
    Ok(())
}

// r[verify kiln.source.pijul_conformance.scenario.reject]
#[test]
fn missing_channel_and_damaged_repository_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
    let executable = executable()?;
    let (_temporary, repository) = create_repository(&executable, "negative", "history")?;
    let profile = PijulSourceProfile::pinned_local();
    let missing =
        LocalPijulAdapter::new(repository.clone(), "missing".to_string(), profile.clone())
            .map_err(|_| io::Error::other("missing-channel adapter construction failed"))?;
    let missing_result = resolve_and_acquire(
        &missing,
        ExternalRef::new("pijul", "channel", "missing")
            .map_err(|_| io::Error::other("missing channel identity failed"))?,
        profile.id.clone(),
        &profile,
    );
    assert!(missing_result.is_err());

    let mut bounded_profile = profile.clone();
    bounded_profile.max_repository_bytes = 1;
    let bounded = LocalPijulAdapter::new(
        repository.clone(),
        FIXTURE_CHANNEL.to_string(),
        bounded_profile.clone(),
    )
    .map_err(|_| io::Error::other("bounded adapter construction failed"))?;
    let bounded_result = resolve_and_acquire(
        &bounded,
        ExternalRef::new("pijul", "channel", FIXTURE_CHANNEL)
            .map_err(|_| io::Error::other("bounded channel identity failed"))?,
        bounded_profile.id.clone(),
        &bounded_profile,
    );
    assert!(bounded_result.is_err());

    let changes = repository.join(".pijul/changes");
    let change_file =
        first_file(&changes)?.ok_or_else(|| io::Error::other("fixture change store is empty"))?;
    fs::remove_file(change_file)?;
    assert!(resolve(&repository).is_err());

    let database = repository.join(".pijul/pristine/db");
    fs::rename(&database, repository.join(".pijul/pristine/db.damaged"))?;
    assert!(resolve(&repository).is_err());
    Ok(())
}
