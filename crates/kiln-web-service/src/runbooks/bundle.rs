//! Bounded, symlink-refusing on-disk BLAKE3 verifier for Lattice workflow-template bundles.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::os::fd::OwnedFd;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use nix::errno::Errno;
use nix::fcntl::{OFlag, open, openat};
use nix::sys::stat::Mode;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::runbooks::{RunbookDefinition, RunbookRegistry, is_hex64, valid_runbook_id};

pub const MAX_BUNDLE_FILE_BYTES: usize = 1_048_576;
pub const MAX_BUNDLE_BYTES: usize = 4_194_304;
pub const MAX_BUNDLE_FILES: usize = 64;
const MAX_WALK_DEPTH: usize = 8;
const MAX_NICKEL_DEPTH: usize = 64;
const BUNDLE_IDENTITY_DOMAIN: &[u8] = b"lattice.workflow-template-bundle.v1";
const MANIFEST_FILE_NAME: &str = "manifest.ncl";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BundleVerifyError {
    Missing(String),
    Drift(String),
}

impl BundleVerifyError {
    #[must_use]
    pub fn status_and_code(&self) -> (u16, &'static str) {
        match self {
            Self::Missing(_) => (404, "runbook_bundle_missing"),
            Self::Drift(_) => (409, "runbook_bundle_drift"),
        }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Missing(msg) | Self::Drift(msg) => msg,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedBundle {
    pub bundle_id: String,
    pub bundle_identity: String,
    pub manifest_identity: String,
    pub file_identities: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct ManifestRecord {
    schema_version: u64,
    id: String,
    #[serde(default)]
    mode: Option<String>,
    workflow: FilePin,
    #[serde(default)]
    orchestration: Option<FilePin>,
    #[serde(default)]
    assets: Vec<AssetPin>,
}

#[derive(Debug, Deserialize)]
struct FilePin {
    path: String,
    blake3: String,
}

#[derive(Debug, Deserialize)]
struct AssetPin {
    path: String,
    media_kind: String,
    blake3: String,
    required: bool,
}

pub fn verify_registry_bundles(
    runbooks_dir: &Path,
    registry: &RunbookRegistry,
) -> Result<(), String> {
    for runbook in &registry.runbooks {
        verify_runbook_bundle(runbooks_dir, runbook).map_err(|err| {
            format!(
                "runbook {} bundle verification failed: {}",
                runbook.id,
                err.message()
            )
        })?;
    }
    Ok(())
}

pub fn verify_runbook_bundle(
    runbooks_dir: &Path,
    runbook: &RunbookDefinition,
) -> Result<VerifiedBundle, BundleVerifyError> {
    let bundle_id = runbook
        .lattice
        .bundle_id
        .as_deref()
        .unwrap_or(runbook.id.as_str());
    if !valid_runbook_id(bundle_id) {
        return Err(BundleVerifyError::Drift("runbook bundle_id invalid".into()));
    }
    verify_bundle_at(runbooks_dir, bundle_id, &runbook.lattice.bundle_identity)
}

pub fn verify_bundle_at(
    runbooks_dir: &Path,
    bundle_id: &str,
    expected_bundle_identity: &str,
) -> Result<VerifiedBundle, BundleVerifyError> {
    if !valid_runbook_id(bundle_id) || !is_hex64(expected_bundle_identity) {
        return Err(BundleVerifyError::Drift(
            "bundle identifier or expected identity invalid".into(),
        ));
    }

    let (bundle_fd, bundle_path) = open_bundle_directory(runbooks_dir, bundle_id)?;
    let mut captured = BTreeMap::new();
    let mut total_bytes = 0usize;
    walk_bundle_dir(
        &bundle_fd,
        &bundle_path,
        "",
        0,
        &mut total_bytes,
        &mut captured,
    )?;

    let manifest_bytes = captured
        .get(MANIFEST_FILE_NAME)
        .ok_or_else(|| BundleVerifyError::Drift("bundle manifest.ncl is missing".into()))?;
    let manifest_text = std::str::from_utf8(manifest_bytes)
        .map_err(|_| BundleVerifyError::Drift("bundle manifest.ncl is not UTF-8".into()))?;
    let parsed_value = parse_nickel_literal(manifest_text)
        .map_err(|msg| BundleVerifyError::Drift(format!("bundle manifest invalid: {msg}")))?;
    let manifest: ManifestRecord = serde_json::from_value(parsed_value)
        .map_err(|_| BundleVerifyError::Drift("bundle manifest schema mismatch".into()))?;

    if !matches!(manifest.schema_version, 1 | 2) || manifest.id != bundle_id {
        return Err(BundleVerifyError::Drift(
            "bundle manifest schema_version or id mismatch".into(),
        ));
    }
    if manifest.schema_version == 2
        && !matches!(manifest.mode.as_deref(), Some("read_only" | "mutating"))
    {
        return Err(BundleVerifyError::Drift(
            "bundle manifest schema 2 missing valid mode".into(),
        ));
    }

    let mut declared: BTreeMap<String, (String, bool)> = BTreeMap::new();
    insert_pin(&mut declared, &manifest.workflow, true)?;
    if let Some(orch) = &manifest.orchestration {
        insert_pin(&mut declared, orch, true)?;
    }
    for asset in &manifest.assets {
        if asset.media_kind.is_empty()
            || !asset.media_kind.replace('/', "-").bytes().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
            })
        {
            return Err(BundleVerifyError::Drift(
                "bundle asset media_kind invalid".into(),
            ));
        }
        insert_pin(
            &mut declared,
            &FilePin {
                path: asset.path.clone(),
                blake3: asset.blake3.clone(),
            },
            asset.required,
        )?;
    }

    let mut file_identities = BTreeMap::new();
    for (rel_path, bytes) in &captured {
        if rel_path == MANIFEST_FILE_NAME {
            continue;
        }
        let Some((expected_hex, _required)) = declared.get(rel_path) else {
            return Err(BundleVerifyError::Drift(format!(
                "bundle contains undeclared file {rel_path}"
            )));
        };
        let actual_hex = blake3::hash(bytes).to_hex().to_string();
        if &actual_hex != expected_hex {
            return Err(BundleVerifyError::Drift(format!(
                "bundle file {rel_path} BLAKE3 digest mismatch"
            )));
        }
        file_identities.insert(rel_path.clone(), actual_hex);
    }

    for (rel_path, (_hex, required)) in &declared {
        if *required && !file_identities.contains_key(rel_path) {
            return Err(BundleVerifyError::Drift(format!(
                "bundle missing declared file {rel_path}"
            )));
        }
    }

    let manifest_identity = blake3::hash(manifest_bytes).to_hex().to_string();
    let mut hasher = blake3::Hasher::new();
    hasher.update(BUNDLE_IDENTITY_DOMAIN);
    hasher.update(manifest_identity.as_bytes());
    for (path, digest) in &file_identities {
        hasher.update(path.as_bytes());
        hasher.update(digest.as_bytes());
    }
    let computed_bundle_identity = blake3::hash(hasher.finalize().as_bytes())
        .to_hex()
        .to_string();

    if computed_bundle_identity != expected_bundle_identity {
        return Err(BundleVerifyError::Drift(format!(
            "bundle identity drift: expected {expected_bundle_identity}, observed {computed_bundle_identity}"
        )));
    }

    Ok(VerifiedBundle {
        bundle_id: bundle_id.to_owned(),
        bundle_identity: computed_bundle_identity,
        manifest_identity,
        file_identities,
    })
}

fn insert_pin(
    declared: &mut BTreeMap<String, (String, bool)>,
    pin: &FilePin,
    required: bool,
) -> Result<(), BundleVerifyError> {
    if !safe_relative_path(&pin.path) || pin.path == MANIFEST_FILE_NAME {
        return Err(BundleVerifyError::Drift(format!(
            "bundle path unsafe: {}",
            pin.path
        )));
    }
    if !is_hex64(&pin.blake3) {
        return Err(BundleVerifyError::Drift(format!(
            "bundle file pin blake3 invalid for {}",
            pin.path
        )));
    }
    if declared
        .insert(pin.path.clone(), (pin.blake3.clone(), required))
        .is_some()
    {
        return Err(BundleVerifyError::Drift(format!(
            "bundle path duplicated: {}",
            pin.path
        )));
    }
    Ok(())
}

fn safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 256
        && path.is_ascii()
        && !path.starts_with('/')
        && !path.ends_with('/')
        && !path.contains("//")
        && !path.contains('\\')
        && !path.contains('\0')
        && path
            .split('/')
            .all(|c| !c.is_empty() && c != "." && c != "..")
}

fn open_bundle_directory(
    runbooks_dir: &Path,
    bundle_id: &str,
) -> Result<(OwnedFd, PathBuf), BundleVerifyError> {
    let root_fd = open(
        runbooks_dir,
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map_err(|err| match err {
        Errno::ELOOP | Errno::ENOTDIR => BundleVerifyError::Drift(
            "runbooks_dir must be a regular directory (not a symlink)".into(),
        ),
        Errno::ENOENT => BundleVerifyError::Missing("runbooks_dir does not exist".into()),
        _ => BundleVerifyError::Drift("runbooks_dir unavailable".into()),
    })?;

    let layouts: [&[&str]; 3] = [
        &[
            "packs",
            "kiln-seed-runbooks",
            "workflow-templates",
            bundle_id,
        ],
        &["workflow-templates", bundle_id],
        &[bundle_id],
    ];
    for components in layouts {
        match open_subpath_nofollow(&root_fd, components) {
            Ok(fd) => {
                let mut resolved = runbooks_dir.to_path_buf();
                for comp in components {
                    resolved.push(comp);
                }
                return Ok((fd, resolved));
            }
            Err(BundleVerifyError::Missing(_)) => continue,
            Err(drift) => return Err(drift),
        }
    }
    Err(BundleVerifyError::Missing(format!(
        "runbook bundle {bundle_id} not found under configured runbooks_dir"
    )))
}

fn open_subpath_nofollow(
    root: &OwnedFd,
    components: &[&str],
) -> Result<OwnedFd, BundleVerifyError> {
    let mut current: Option<OwnedFd> = None;
    for comp in components {
        let parent = current.as_ref().unwrap_or(root);
        let fd = openat(
            parent,
            Path::new(comp),
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|err| match err {
            Errno::ELOOP | Errno::ENOTDIR => {
                BundleVerifyError::Drift(format!("symlink or non-directory encountered at {comp}"))
            }
            Errno::ENOENT => BundleVerifyError::Missing(format!("missing directory {comp}")),
            _ => BundleVerifyError::Drift(format!("cannot open bundle directory {comp}")),
        })?;
        current = Some(fd);
    }
    current.ok_or_else(|| BundleVerifyError::Missing("empty bundle path".into()))
}

fn walk_bundle_dir(
    dir_fd: &OwnedFd,
    dir_path: &Path,
    prefix: &str,
    depth: usize,
    total_bytes: &mut usize,
    out: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), BundleVerifyError> {
    if depth > MAX_WALK_DEPTH {
        return Err(BundleVerifyError::Drift(
            "bundle directory depth exceeded".into(),
        ));
    }
    let dir_stat = nix::sys::stat::fstat(dir_fd)
        .map_err(|_| BundleVerifyError::Drift("bundle directory stat failed".into()))?;
    let dir_meta = std::fs::symlink_metadata(dir_path)
        .map_err(|_| BundleVerifyError::Drift("bundle directory metadata failed".into()))?;
    if !dir_meta.is_dir()
        || dir_meta.dev() != dir_stat.st_dev as u64
        || dir_meta.ino() != dir_stat.st_ino as u64
    {
        return Err(BundleVerifyError::Drift(
            "bundle directory swapped or symlinked during walk".into(),
        ));
    }
    let read_dir = std::fs::read_dir(dir_path)
        .map_err(|_| BundleVerifyError::Drift("bundle directory read failed".into()))?;
    let mut names = BTreeSet::new();
    for entry in read_dir {
        let entry =
            entry.map_err(|_| BundleVerifyError::Drift("bundle entry read failed".into()))?;
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            return Err(BundleVerifyError::Drift(
                "non-UTF-8 file name in bundle".into(),
            ));
        };
        if name == "." || name == ".." {
            continue;
        }
        if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains('\0') {
            return Err(BundleVerifyError::Drift(
                "unsafe file name in bundle".into(),
            ));
        }
        names.insert(name.to_owned());
    }

    for name in names {
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let entry_fd = openat(
            dir_fd,
            Path::new(&name),
            OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|err| match err {
            Errno::ELOOP => BundleVerifyError::Drift(format!("symlink rejected in bundle: {rel}")),
            _ => BundleVerifyError::Drift(format!("cannot open bundle entry: {rel}")),
        })?;
        let before = nix::sys::stat::fstat(&entry_fd)
            .map_err(|_| BundleVerifyError::Drift("bundle stat failed".into()))?;
        let mode = before.st_mode & libc::S_IFMT;
        if mode == libc::S_IFDIR {
            let subdir_fd = openat(
                dir_fd,
                Path::new(&name),
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| BundleVerifyError::Drift(format!("symlink or unsafe subdir: {rel}")))?;
            walk_bundle_dir(
                &subdir_fd,
                &dir_path.join(&name),
                &rel,
                depth + 1,
                total_bytes,
                out,
            )?;
        } else if mode == libc::S_IFREG {
            if out.len() >= MAX_BUNDLE_FILES {
                return Err(BundleVerifyError::Drift(
                    "bundle exceeds MAX_BUNDLE_FILES".into(),
                ));
            }
            let file = File::from(entry_fd);
            let meta_before = file
                .metadata()
                .map_err(|_| BundleVerifyError::Drift("bundle file metadata failed".into()))?;
            if !meta_before.is_file()
                || meta_before.nlink() != 1
                || meta_before.len() > MAX_BUNDLE_FILE_BYTES as u64
            {
                return Err(BundleVerifyError::Drift(format!(
                    "bundle file type, nlink, or size rejected: {rel}"
                )));
            }
            let mut bytes = Vec::with_capacity(meta_before.len() as usize);
            Read::by_ref(&mut &file)
                .take(MAX_BUNDLE_FILE_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| BundleVerifyError::Drift(format!("bundle file read failed: {rel}")))?;
            let meta_after = file
                .metadata()
                .map_err(|_| BundleVerifyError::Drift("bundle file metadata failed".into()))?;
            if bytes.len() > MAX_BUNDLE_FILE_BYTES
                || bytes.len() as u64 != meta_before.len()
                || meta_after.len() != meta_before.len()
                || meta_before.mtime() != meta_after.mtime()
                || meta_before.mtime_nsec() != meta_after.mtime_nsec()
                || meta_before.ctime() != meta_after.ctime()
                || meta_before.ctime_nsec() != meta_after.ctime_nsec()
            {
                return Err(BundleVerifyError::Drift(format!(
                    "bundle file changed during read or exceeded bound: {rel}"
                )));
            }
            *total_bytes = total_bytes
                .checked_add(bytes.len())
                .ok_or_else(|| BundleVerifyError::Drift("bundle size overflow".into()))?;
            if *total_bytes > MAX_BUNDLE_BYTES {
                return Err(BundleVerifyError::Drift(
                    "bundle exceeds MAX_BUNDLE_BYTES".into(),
                ));
            }
            out.insert(rel, bytes);
        } else {
            return Err(BundleVerifyError::Drift(format!(
                "non-regular file rejected in bundle: {rel}"
            )));
        }
    }
    Ok(())
}

/// Strict parser for Nickel literal data values (`{ k = v }`, `[ ... ]`, `"..."`, integers, bools, `null`, `#` comments).
fn parse_nickel_literal(input: &str) -> Result<Value, String> {
    let mut parser = NickelParser {
        bytes: input.as_bytes(),
        pos: 0,
    };
    let val = parser.parse_value(0)?;
    parser.skip_ws_and_comments();
    if parser.pos != parser.bytes.len() {
        return Err("unexpected trailing tokens in manifest".into());
    }
    Ok(val)
}

struct NickelParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl NickelParser<'_> {
    fn skip_ws_and_comments(&mut self) {
        while let Some(&b) = self.bytes.get(self.pos) {
            if b.is_ascii_whitespace() {
                self.pos += 1;
            } else if b == b'#' {
                while self.bytes.get(self.pos).is_some_and(|&c| c != b'\n') {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_ws_and_comments();
        self.bytes.get(self.pos).copied()
    }

    fn parse_value(&mut self, depth: usize) -> Result<Value, String> {
        if depth > MAX_NICKEL_DEPTH {
            return Err("manifest nesting depth exceeded".into());
        }
        match self.peek() {
            Some(b'{') => self.parse_record(depth),
            Some(b'[') => self.parse_array(depth),
            Some(b'"') => self.parse_string().map(Value::String),
            Some(b'-' | b'0'..=b'9') => self.parse_integer(),
            Some(b't' | b'f' | b'n') => self.parse_keyword(),
            Some(other) => Err(format!("unexpected byte 0x{other:02x}")),
            None => Err("unexpected EOF".into()),
        }
    }

    fn parse_record(&mut self, depth: usize) -> Result<Value, String> {
        if depth >= MAX_NICKEL_DEPTH {
            return Err("manifest nesting depth exceeded".into());
        }
        self.pos += 1; // '{'
        let mut map = Map::new();
        loop {
            match self.peek() {
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(map));
                }
                Some(b'"') => {
                    let key = self.parse_string()?;
                    self.expect_byte(b'=')?;
                    let val = self.parse_value(depth + 1)?;
                    map.insert(key, val);
                }
                Some(b) if b.is_ascii_alphabetic() || b == b'_' => {
                    let key = self.parse_ident()?;
                    self.expect_byte(b'=')?;
                    let val = self.parse_value(depth + 1)?;
                    map.insert(key, val);
                }
                _ => return Err("expected record field or '}'".into()),
            }
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(map));
                }
                _ => return Err("expected ',' or '}' in record".into()),
            }
        }
    }

    fn parse_array(&mut self, depth: usize) -> Result<Value, String> {
        if depth >= MAX_NICKEL_DEPTH {
            return Err("manifest nesting depth exceeded".into());
        }
        self.pos += 1; // '['
        let mut items = Vec::new();
        loop {
            if self.peek() == Some(b']') {
                self.pos += 1;
                return Ok(Value::Array(items));
            }
            items.push(self.parse_value(depth + 1)?);
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err("expected ',' or ']' in array".into()),
            }
        }
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.pos += 1; // '"'
        let mut out = String::new();
        while let Some(&b) = self.bytes.get(self.pos) {
            self.pos += 1;
            match b {
                b'"' => return Ok(out),
                b'\\' => {
                    let esc = *self
                        .bytes
                        .get(self.pos)
                        .ok_or_else(|| "unterminated string escape".to_owned())?;
                    self.pos += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        _ => return Err("unsupported escape sequence".into()),
                    }
                }
                c if c < 0x20 => return Err("control character in string".into()),
                _ => {
                    self.pos -= 1;
                    let slice = self
                        .bytes
                        .get(self.pos..)
                        .ok_or_else(|| "unexpected EOF in string".to_owned())?;
                    let rest = std::str::from_utf8(slice)
                        .map_err(|_| "invalid UTF-8 in string".to_owned())?;
                    let ch = rest
                        .chars()
                        .next()
                        .ok_or_else(|| "unexpected EOF in string".to_owned())?;
                    self.pos += ch.len_utf8();
                    out.push(ch);
                }
            }
        }
        Err("unterminated string".into())
    }

    fn parse_ident(&mut self) -> Result<String, String> {
        let start = self.pos;
        while let Some(&b) = self.bytes.get(self.pos) {
            if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return Err("expected identifier".into());
        }
        let slice = self
            .bytes
            .get(start..self.pos)
            .ok_or_else(|| "identifier bounds invalid".to_owned())?;
        std::str::from_utf8(slice)
            .map(str::to_owned)
            .map_err(|_| "invalid UTF-8 identifier".into())
    }

    fn parse_integer(&mut self) -> Result<Value, String> {
        let start = self.pos;
        if self.bytes.get(self.pos) == Some(&b'-') {
            self.pos += 1;
        }
        let digits_start = self.pos;
        while self.bytes.get(self.pos).is_some_and(u8::is_ascii_digit) {
            self.pos += 1;
        }
        if self.pos == digits_start {
            return Err("expected digits in integer".into());
        }
        let raw = self
            .bytes
            .get(start..self.pos)
            .ok_or_else(|| "integer bounds invalid".to_owned())?;
        let slice = std::str::from_utf8(raw).map_err(|_| "invalid integer".to_owned())?;
        let parsed = slice
            .parse::<i64>()
            .map_err(|_| "integer out of i64 range".to_owned())?;
        Ok(Value::Number(parsed.into()))
    }

    fn parse_keyword(&mut self) -> Result<Value, String> {
        let rest = self.bytes.get(self.pos..).unwrap_or_default();
        if rest.starts_with(b"true")
            && !self
                .bytes
                .get(self.pos + 4)
                .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
        {
            self.pos += 4;
            return Ok(Value::Bool(true));
        }
        if rest.starts_with(b"false")
            && !self
                .bytes
                .get(self.pos + 5)
                .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
        {
            self.pos += 5;
            return Ok(Value::Bool(false));
        }
        if rest.starts_with(b"null")
            && !self
                .bytes
                .get(self.pos + 4)
                .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
        {
            self.pos += 4;
            return Ok(Value::Null);
        }
        Err("unexpected identifier value".into())
    }

    fn expect_byte(&mut self, expected: u8) -> Result<(), String> {
        if self.peek() != Some(expected) {
            return Err(format!("expected '{}'", expected as char));
        }
        self.pos += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_nickel_literal_enforces_max_nesting_depth() {
        let ok_array = format!(
            "{}{}",
            "[".repeat(MAX_NICKEL_DEPTH),
            "]".repeat(MAX_NICKEL_DEPTH)
        );
        assert!(parse_nickel_literal(&ok_array).is_ok());

        let err_array = format!(
            "{}{}",
            "[".repeat(MAX_NICKEL_DEPTH + 1),
            "]".repeat(MAX_NICKEL_DEPTH + 1)
        );
        assert!(parse_nickel_literal(&err_array).is_err());

        let ok_record = format!(
            "{}1{}",
            "{ a = ".repeat(MAX_NICKEL_DEPTH),
            " }".repeat(MAX_NICKEL_DEPTH)
        );
        assert!(parse_nickel_literal(&ok_record).is_ok());

        let err_record = format!(
            "{}1{}",
            "{ a = ".repeat(MAX_NICKEL_DEPTH + 1),
            " }".repeat(MAX_NICKEL_DEPTH + 1)
        );
        assert!(parse_nickel_literal(&err_record).is_err());
    }
}
