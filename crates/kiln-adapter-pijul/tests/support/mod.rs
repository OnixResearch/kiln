#![allow(
    dead_code,
    reason = "shared by multiple integration test targets with different helper subsets"
)]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use libpijul::change::{Change, ChangeHeader};
use libpijul::changestore::ChangeStore;
use libpijul::record::{Algorithm, Builder};
use libpijul::{MutTxnT, MutTxnTExt, TxnT};
use pijul_repository::Repository;
use tempfile::TempDir;

pub const EXPECTED_PIJUL_VERSION: &str = "pijul 1.0.0-beta.11";
pub const EXPECTED_DEPENDENT_CHANGE_COUNT: usize = 2;
pub const FIXTURE_CHANNEL: &str = "main";
pub const FIXTURE_FILE: &str = "file.txt";
pub const RECORD_THREADS: usize = 1;
pub const RECORD_SALT: u64 = 0;

pub fn executable() -> io::Result<PathBuf> {
    std::env::var_os("KILN_PIJUL_EXE")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("KILN_PIJUL_EXE is required for native fixture tests"))
}

pub fn run(executable: &Path, cwd: &Path, home: &Path, arguments: &[&str]) -> io::Result<Output> {
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

pub fn record_fixture(
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

pub fn fork_channel(repository_path: &Path, channel_name: &str) -> anyhow::Result<()> {
    let repository = Repository::find_root(Some(repository_path))?;
    let mut transaction = repository.pristine.mut_txn_begin()?;
    let source = transaction
        .load_channel(FIXTURE_CHANNEL)?
        .ok_or_else(|| anyhow::anyhow!("fixture source channel missing"))?;
    let _fork = transaction.fork(&source, channel_name)?;
    transaction.commit()?;
    Ok(())
}

pub fn first_file(root: &Path) -> io::Result<Option<PathBuf>> {
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

pub fn create_repository(
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
