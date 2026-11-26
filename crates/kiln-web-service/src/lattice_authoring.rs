//! Read-only view of explicitly pinned Lattice `author-source.v1` archives.
//!
//! Do not open the Lattice catalog here. The owner CLI reads the live head when
//! capturing a draft, and its admit command publishes that draft to the catalog.
//! Neither operation implements a caller-bound, non-publishing draft save.

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::engine::{ApiResponse, Role};

pub const ROUTE: &str = "/api/lattice/authoring";
const INDEX_SCHEMA: &str = "kiln.lattice-author-source-index.v1";
const EXPORT_SCHEMA: &str = "lattice.author-source.v1";
const MAX_INDEX_BYTES: usize = 64 * 1024;
const MAX_EXPORTS: usize = 32;
// These match Lattice's native author export and captured input limits.
const MAX_ARCHIVE_BYTES: usize = 64 * 1_048_576;
const MAX_FILES: usize = 64;
const MAX_FILE_BYTES: usize = 1_048_576;
const MAX_SOURCE_BYTES: usize = 4 * MAX_FILE_BYTES;
const MAX_PATH_BYTES: usize = 512;
// Keep the startup-loaded projection bounded across all retained revisions.
const MAX_CATALOG_SOURCE_BYTES: usize = 16 * MAX_FILE_BYTES;
const MAX_CATALOG_ARCHIVE_BYTES: usize = 2 * MAX_ARCHIVE_BYTES;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Index {
    schema: String,
    exports: Vec<ExportPin>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportPin {
    workflow: String,
    archive: String,
    blake3: String,
}

// Project only the fields needed for viewing. The archive pin covers the whole
// native artifact (including opaque receipts); Kiln does not replay or vouch
// for those receipts and does not return arbitrary configuration fields.
#[derive(Deserialize)]
struct Archive {
    schema: String,
    association: Association,
}

#[derive(Deserialize)]
struct Association {
    source_revision: String,
    expanded_artifact: String,
    state: AssociationState,
    program: Program,
}

#[derive(Deserialize, Serialize)]
enum AssociationState {
    CapturedSnapshot,
    CapturedModules,
    RetainedEntrySnapshot,
    MaterializedFork,
}

#[derive(Deserialize)]
struct Program {
    entry: String,
    files: BTreeMap<String, Vec<u8>>,
}

#[derive(Serialize)]
struct Revision {
    workflow: String,
    archive_blake3: String,
    source_revision: String,
    expanded_artifact: String,
    state: AssociationState,
    entry: String,
    members: Vec<String>,
}

#[derive(Serialize)]
struct Listing {
    revisions: Vec<Revision>,
}

pub struct SourceCatalog {
    listing: Vec<u8>,
    files: BTreeMap<String, Vec<Vec<u8>>>,
}

impl SourceCatalog {
    /// Load an operator-controlled index of named, digest-pinned native archives.
    /// Each `archive` is a filename relative to the index; none is request-selected.
    // r[impl kiln.sdlc.lattice_authoring]
    pub fn load(path: &Path) -> Result<Self, String> {
        let index: Index = serde_json::from_slice(&read_file(path, MAX_INDEX_BYTES)?)
            .map_err(|_| "invalid Lattice author-source index")?;
        if index.schema != INDEX_SCHEMA || index.exports.len() > MAX_EXPORTS {
            return Err("unsupported or oversized Lattice author-source index".into());
        }
        let root = path.parent().unwrap_or_else(|| Path::new(""));
        let mut files = BTreeMap::new();
        let mut revisions = Vec::with_capacity(index.exports.len());
        let mut source_bytes = 0;
        let mut archive_bytes = 0;
        for pin in index.exports {
            if !display_name(&pin.workflow)
                || !member_path(&pin.archive)
                || pin.archive.contains('/')
                || !pin.archive.ends_with(".author.json")
                || !digest(&pin.blake3)
                || files.contains_key(&pin.blake3)
            {
                return Err("invalid or duplicate Lattice author-source pin".into());
            }
            let bytes = read_file(
                &root.join(&pin.archive),
                MAX_ARCHIVE_BYTES.min(MAX_CATALOG_ARCHIVE_BYTES - archive_bytes),
            )?;
            archive_bytes += bytes.len();
            let (revision, members, size) = project(pin, &bytes)?;
            source_bytes += size;
            if source_bytes > MAX_CATALOG_SOURCE_BYTES {
                return Err(
                    "Lattice author-source catalog exceeds the retained source limit".into(),
                );
            }
            files.insert(revision.archive_blake3.clone(), members);
            revisions.push(revision);
        }
        let listing = serde_json::to_vec(&Listing { revisions })
            .map_err(|_| "Lattice author-source listing encoding failed")?;
        Ok(Self { listing, files })
    }
}

fn read_file(path: &Path, limit: usize) -> Result<Vec<u8>, &'static str> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| "Lattice author-source input unavailable")?;
    if !file
        .metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.len() <= limit as u64)
    {
        return Err("Lattice author-source input is not a bounded regular file");
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Lattice author-source input unreadable")?;
    if bytes.len() > limit {
        return Err("Lattice author-source input exceeds its byte limit");
    }
    Ok(bytes)
}

fn project(pin: ExportPin, bytes: &[u8]) -> Result<(Revision, Vec<Vec<u8>>, usize), &'static str> {
    if blake3::hash(bytes).to_hex().as_str() != pin.blake3 {
        return Err("Lattice author-source archive pin mismatch");
    }
    let archive: Archive =
        serde_json::from_slice(bytes).map_err(|_| "invalid Lattice author-source archive")?;
    let association = archive.association;
    let program = association.program;
    if archive.schema != EXPORT_SCHEMA
        || !digest(&association.source_revision)
        || !digest(&association.expanded_artifact)
        || program.files.len() > MAX_FILES
        || !program.files.contains_key(&program.entry)
    {
        return Err("unsupported or malformed Lattice author-source archive");
    }
    let mut size = 0;
    for (path, member) in &program.files {
        if !member_path(path) || member.len() > MAX_FILE_BYTES {
            return Err("invalid Lattice author-source member path or size");
        }
        size += member.len();
    }
    if size > MAX_SOURCE_BYTES {
        return Err("Lattice author-source revision exceeds its byte limit");
    }
    let (members, files) = program.files.into_iter().unzip();
    Ok((
        Revision {
            workflow: pin.workflow,
            archive_blake3: pin.blake3,
            source_revision: association.source_revision,
            expanded_artifact: association.expanded_artifact,
            state: association.state,
            entry: program.entry,
            members,
        },
        files,
        size,
    ))
}

fn display_name(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}

fn member_path(value: &str) -> bool {
    value.len() <= MAX_PATH_BYTES
        && !value.contains('\\')
        && !value.chars().any(char::is_control)
        && value
            .split('/')
            .all(|part| !matches!(part, "" | "." | ".."))
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn member_query(query: &str) -> Option<(&str, usize)> {
    let (archive, member) = query.strip_prefix("archive=")?.split_once("&member=")?;
    // A source member is addressed by its ordinal in the immutable listing, not by a path.
    if !digest(archive)
        || member.is_empty()
        || member.len() > 2
        || (member.starts_with('0') && member != "0")
        || !member.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let member: usize = member.parse().ok()?;
    (member < MAX_FILES).then_some((archive, member))
}

pub fn response(
    catalog: Option<&SourceCatalog>,
    method: &str,
    target: &str,
    body: &[u8],
    role: Role,
) -> ApiResponse {
    if role != Role::Operator {
        return error(
            403,
            "lattice_authoring_forbidden",
            "Only operators may view Lattice author source.",
        );
    }
    if method != "GET" {
        let mut response = error(
            405,
            "lattice_authoring_read_only",
            "The configured boundary cannot save, publish or execute drafts.",
        );
        response.headers.push(("Allow", "GET".into()));
        return response;
    }
    if !body.is_empty() {
        return error(
            400,
            "lattice_authoring_invalid_request",
            "A source read must not include a request body.",
        );
    }
    let (path, query) = target
        .split_once('?')
        .map_or((target, None), |(path, query)| (path, Some(query)));
    if path != ROUTE {
        return error(
            404,
            "lattice_authoring_not_found",
            "No such Lattice authoring route.",
        );
    }
    let Some(catalog) = catalog else {
        return error(
            503,
            "lattice_authoring_unconfigured",
            "No operator-pinned Lattice source archive index is configured.",
        );
    };
    let Some(query) = query else {
        return http(200, "application/json", catalog.listing.clone());
    };
    let Some((archive, member)) = member_query(query) else {
        return error(
            400,
            "lattice_authoring_invalid_selection",
            "Supply the exact archive identity and source member ordinal.",
        );
    };
    let Some(bytes) = catalog
        .files
        .get(archive)
        .and_then(|files| files.get(member))
    else {
        return error(
            404,
            "lattice_authoring_source_not_found",
            "The exact archive or member was not found. No other revision was substituted.",
        );
    };
    if std::str::from_utf8(bytes).is_err() {
        return error(
            415,
            "lattice_authoring_binary_member",
            "This captured member is not UTF-8 text.",
        );
    }
    http(200, "text/plain; charset=utf-8", bytes.clone())
}

fn error(status: u16, code: &str, message: &str) -> ApiResponse {
    http(
        status,
        "application/json",
        json!({"code": code, "message": message})
            .to_string()
            .into_bytes(),
    )
}

fn http(status: u16, content_type: &'static str, body: Vec<u8>) -> ApiResponse {
    ApiResponse {
        status,
        content_type,
        body,
        download_name: None,
        headers: vec![("Cache-Control", "no-store".into())],
    }
}

#[cfg(test)]
#[path = "lattice_authoring_tests.rs"]
mod tests;
