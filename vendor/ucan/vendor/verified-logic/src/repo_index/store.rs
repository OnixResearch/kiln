use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use redb::Database;
use redb::ReadableTable;
use redb::TableDefinition;

use super::extract::extract_facts;
use super::extract::infer_openspec_context;
use super::model::CURRENT_SCHEMA_VERSION;
use super::model::DigestFactsView;
use super::model::DigestQueryResult;
use super::model::GenerationStats;
use super::model::PathQueryResult;
use super::model::PathRow;
use super::model::RefreshSummary;
use super::model::RequirementDefinitionHit;
use super::model::RequirementQueryResult;
use super::model::RequirementReferenceHit;
use super::model::SymbolHit;

const META_TABLE: TableDefinition<&str, u64> = TableDefinition::new("repo_index_meta");
const PATH_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("repo_index_paths");
const EXTRACTION_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("repo_index_extractions");
const STATS_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("repo_index_stats");

#[derive(Debug, Clone, Copy, Default)]
pub struct RefreshOptions {
    pub fail_before_commit: bool,
}

#[derive(Debug)]
struct ScannedFile {
    absolute_path: PathBuf,
    relative_path: String,
}

pub struct RepoIndex {
    root: PathBuf,
    db: Database,
}

impl RepoIndex {
    pub fn open(root: &Path, db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create repo-index parent directory {}", parent.display()))?;
        }
        let mut db =
            Database::create(db_path).with_context(|| format!("open repo index database {}", db_path.display()))?;
        if let Err(error) = Self::initialize(&db) {
            if schema_mismatch(&error) {
                drop(db);
                if db_path.exists() {
                    fs::remove_file(db_path)
                        .with_context(|| format!("remove stale repo index database {}", db_path.display()))?;
                }
                db = Database::create(db_path)
                    .with_context(|| format!("recreate repo index database {}", db_path.display()))?;
                Self::initialize(&db)?;
            } else {
                return Err(error);
            }
        }
        Ok(Self {
            root: root.to_path_buf(),
            db,
        })
    }

    fn initialize(db: &Database) -> Result<()> {
        let write_txn = db.begin_write().context("begin repo-index init transaction")?;
        {
            let mut meta = write_txn.open_table(META_TABLE).context("open repo-index meta table")?;
            let schema_value =
                meta.get("schema_version").context("read repo-index schema version")?.map(|value| value.value());
            if let Some(version) = schema_value {
                if version != CURRENT_SCHEMA_VERSION {
                    bail!("repo-index schema mismatch: found {version}, expected {CURRENT_SCHEMA_VERSION}");
                }
            } else {
                meta.insert("schema_version", CURRENT_SCHEMA_VERSION).context("write repo-index schema version")?;
            }
            write_txn.open_table(PATH_TABLE).context("open repo-index path table")?;
            write_txn.open_table(EXTRACTION_TABLE).context("open repo-index extraction table")?;
            write_txn.open_table(STATS_TABLE).context("open repo-index stats table")?;
        }
        write_txn.commit().context("commit repo-index init transaction")?;
        Ok(())
    }

    pub fn active_generation(&self) -> Result<Option<u64>> {
        let read_txn = self.db.begin_read().context("begin repo-index read transaction")?;
        let meta = read_txn.open_table(META_TABLE).context("open repo-index meta table")?;
        let active = meta.get("active_generation").context("read repo-index active generation")?;
        Ok(active.map(|value| value.value()))
    }

    // r[impl repo-index.content-digest]
    // r[impl repo-index.persistent-store]
    // r[impl repo-index.atomic-refresh]
    // r[impl repo-index.incremental-refresh]
    pub fn refresh(&self) -> Result<RefreshSummary> {
        self.refresh_with_options(RefreshOptions::default())
    }

    pub fn refresh_with_options(&self, options: RefreshOptions) -> Result<RefreshSummary> {
        let scanned = scan_tracked_files(&self.root)?;
        let previous_rows = self.active_path_rows()?;
        let previous_by_path: BTreeMap<String, PathRow> =
            previous_rows.into_iter().map(|row| (row.path.clone(), row)).collect();
        let mut seen_paths = BTreeSet::new();
        let next_generation = self.active_generation()?.unwrap_or(0) + 1;

        let write_txn = self.db.begin_write().context("begin repo-index refresh transaction")?;

        let mut stats = GenerationStats::default();
        let mut active_digests = BTreeSet::new();
        let mut inserted_extractions = 0u64;
        let mut reused_extractions = 0u64;
        let mut added_paths = 0u64;
        let mut changed_paths = 0u64;
        let mut unchanged_paths = 0u64;

        {
            let mut meta = write_txn.open_table(META_TABLE).context("open repo-index meta table")?;
            let mut path_table = write_txn.open_table(PATH_TABLE).context("open repo-index path table")?;
            let mut extraction_table =
                write_txn.open_table(EXTRACTION_TABLE).context("open repo-index extraction table")?;
            let mut stats_table = write_txn.open_table(STATS_TABLE).context("open repo-index stats table")?;

            for scanned_file in scanned {
                let bytes = fs::read(&scanned_file.absolute_path)
                    .with_context(|| format!("read tracked file {}", scanned_file.absolute_path.display()))?;
                let digest_hex = blake3::hash(&bytes).to_hex().to_string();
                let extractor_kind = super::model::ExtractorKind::from_path(&scanned_file.relative_path);
                let row = PathRow {
                    path: scanned_file.relative_path.clone(),
                    digest_hex: digest_hex.clone(),
                    extractor_kind,
                    size_bytes: bytes.len() as u64,
                    openspec: infer_openspec_context(&scanned_file.relative_path),
                };
                let extraction_key = row.extraction_key();
                let extraction_bytes = if let Some(existing) = extraction_table
                    .get(extraction_key.as_str())
                    .with_context(|| format!("read extraction cache {extraction_key}"))?
                {
                    reused_extractions += 1;
                    existing.value().to_vec()
                } else {
                    let facts = extract_facts(&row.path, &bytes);
                    let encoded = encode(&facts)?;
                    extraction_table
                        .insert(extraction_key.as_str(), encoded.as_slice())
                        .with_context(|| format!("insert extraction cache {extraction_key}"))?;
                    inserted_extractions += 1;
                    encoded
                };
                let facts = decode::<super::model::ExtractedFacts>(&extraction_bytes)?;

                stats.tracked_paths += 1;
                stats.symbol_count += facts.symbols.len() as u64;
                stats.requirement_reference_count += facts.requirement_refs.len() as u64;
                stats.requirement_definition_count += facts.requirement_defs.len() as u64;
                stats.heading_count += facts.headings.len() as u64;
                active_digests.insert(digest_hex);
                seen_paths.insert(row.path.clone());

                match previous_by_path.get(&row.path) {
                    Some(previous) if previous == &row => unchanged_paths += 1,
                    Some(_) => changed_paths += 1,
                    None => added_paths += 1,
                }

                let key = path_key(next_generation, &row.path);
                let encoded_row = encode(&row)?;
                path_table
                    .insert(key.as_str(), encoded_row.as_slice())
                    .with_context(|| format!("insert active path row {}", row.path))?;
            }

            stats.unique_digests = active_digests.len() as u64;

            let encoded_stats = encode(&stats)?;
            stats_table
                .insert(next_generation, encoded_stats.as_slice())
                .with_context(|| format!("insert stats for generation {next_generation}"))?;

            if options.fail_before_commit {
                return Err(anyhow!("simulated repo-index failure before commit for generation {next_generation}"));
            }

            meta.insert("active_generation", next_generation).context("publish repo-index active generation")?;
        }
        write_txn.commit().with_context(|| format!("commit repo-index generation {next_generation}"))?;

        let deleted_paths = previous_by_path.keys().filter(|path| !seen_paths.contains(*path)).count() as u64;

        Ok(RefreshSummary {
            generation: next_generation,
            tracked_paths: stats.tracked_paths,
            inserted_extractions,
            reused_extractions,
            added_paths,
            changed_paths,
            unchanged_paths,
            deleted_paths,
        })
    }

    // r[impl repo-index.query]
    pub fn stats(&self) -> Result<GenerationStats> {
        let generation = self.require_active_generation()?;
        let read_txn = self.db.begin_read().context("begin repo-index read transaction")?;
        let stats_table = read_txn.open_table(STATS_TABLE).context("open repo-index stats table")?;
        let encoded = stats_table
            .get(generation)
            .with_context(|| format!("read stats for generation {generation}"))?
            .ok_or_else(|| anyhow!("missing stats row for active repo-index generation {generation}"))?;
        decode(encoded.value())
    }

    // r[impl repo-index.cross-reference]
    // r[impl repo-index.query]
    pub fn query_path(&self, path: &str) -> Result<Option<PathQueryResult>> {
        let generation = self.require_active_generation()?;
        let normalized = normalize_query_path(path);
        let read_txn = self.db.begin_read().context("begin repo-index read transaction")?;
        let path_table = read_txn.open_table(PATH_TABLE).context("open repo-index path table")?;
        let key = path_key(generation, &normalized);
        let Some(encoded_row) =
            path_table.get(key.as_str()).with_context(|| format!("read active path row {normalized}"))?
        else {
            return Ok(None);
        };
        let row = decode::<PathRow>(encoded_row.value())?;
        let facts = Self::read_extracted_facts_for_row(&read_txn, &row)?;
        Ok(Some(PathQueryResult { row, facts }))
    }

    // r[impl repo-index.cross-reference]
    // r[impl repo-index.query]
    pub fn query_symbol(&self, name: &str) -> Result<Vec<SymbolHit>> {
        let read_txn = self.db.begin_read().context("begin repo-index read transaction")?;
        let generation = Some(self.require_active_generation()?);
        let mut hits = Vec::new();
        for row in Self::active_path_rows_from_txn(&read_txn, generation)? {
            let facts = Self::read_extracted_facts_for_row(&read_txn, &row)?;
            for symbol in facts.symbols {
                if symbol.name == name {
                    hits.push(SymbolHit {
                        path: row.path.clone(),
                        digest_hex: row.digest_hex.clone(),
                        symbol,
                    });
                }
            }
        }
        hits.sort();
        Ok(hits)
    }

    // r[impl repo-index.cross-reference]
    // r[impl repo-index.query]
    pub fn query_requirement(&self, requirement_id: &str) -> Result<RequirementQueryResult> {
        let read_txn = self.db.begin_read().context("begin repo-index read transaction")?;
        let generation = Some(self.require_active_generation()?);
        let mut result = RequirementQueryResult::default();
        for row in Self::active_path_rows_from_txn(&read_txn, generation)? {
            let facts = Self::read_extracted_facts_for_row(&read_txn, &row)?;
            for definition in facts.requirement_defs {
                if definition.id == requirement_id {
                    result.definitions.push(RequirementDefinitionHit {
                        path: row.path.clone(),
                        digest_hex: row.digest_hex.clone(),
                        definition,
                    });
                }
            }
            for reference in facts.requirement_refs {
                if reference.id == requirement_id {
                    result.references.push(RequirementReferenceHit {
                        path: row.path.clone(),
                        digest_hex: row.digest_hex.clone(),
                        reference,
                    });
                }
            }
        }
        result.definitions.sort();
        result.references.sort();
        Ok(result)
    }

    // r[impl repo-index.query]
    pub fn query_digest(&self, digest_hex: &str) -> Result<DigestQueryResult> {
        validate_digest_hex(digest_hex)?;
        let read_txn = self.db.begin_read().context("begin repo-index read transaction")?;
        let generation = Some(self.require_active_generation()?);
        let mut paths = Vec::new();
        let mut extraction_keys = BTreeSet::new();
        for row in Self::active_path_rows_from_txn(&read_txn, generation)? {
            if row.digest_hex == digest_hex {
                extraction_keys.insert(row.extraction_key());
                paths.push(row);
            }
        }
        paths.sort_by_key(|path| path.path.clone());

        let extraction_table = read_txn.open_table(EXTRACTION_TABLE).context("open repo-index extraction table")?;
        let mut facts_by_kind = Vec::new();
        for extraction_key in extraction_keys {
            let encoded = extraction_table
                .get(extraction_key.as_str())
                .with_context(|| format!("read digest facts for key {extraction_key}"))?
                .ok_or_else(|| anyhow!("missing extraction row for key {extraction_key}"))?;
            let extractor_prefix =
                extraction_key.split(':').next().ok_or_else(|| anyhow!("invalid extraction key {extraction_key}"))?;
            let extractor_kind = match extractor_prefix {
                "rust" => super::model::ExtractorKind::Rust,
                "requirements-doc" => super::model::ExtractorKind::RequirementsDoc,
                "openspec-markdown" => super::model::ExtractorKind::OpenSpecMarkdown,
                "markdown" => super::model::ExtractorKind::Markdown,
                _ => bail!("unknown extractor kind in key {extraction_key}"),
            };
            let facts = decode(encoded.value())?;
            facts_by_kind.push(DigestFactsView { extractor_kind, facts });
        }
        facts_by_kind.sort_by_key(|facts| facts.extractor_kind);

        Ok(DigestQueryResult {
            digest_hex: digest_hex.to_string(),
            paths,
            facts_by_kind,
        })
    }

    #[cfg(test)]
    pub fn stored_extraction_rows(&self) -> Result<u64> {
        let read_txn = self.db.begin_read().context("begin repo-index read transaction")?;
        let table = read_txn.open_table(EXTRACTION_TABLE).context("open repo-index extraction table")?;
        let mut count = 0u64;
        for entry in table.iter().context("iterate repo-index extraction table")? {
            let _ = entry.context("read repo-index extraction row")?;
            count += 1;
        }
        Ok(count)
    }

    #[cfg(test)]
    pub fn debug_snapshot(&self) -> Result<Vec<PathQueryResult>> {
        let read_txn = self.db.begin_read().context("begin repo-index read transaction")?;
        let generation = self.active_generation()?;
        let mut snapshot = Vec::new();
        for row in Self::active_path_rows_from_txn(&read_txn, generation)? {
            let facts = Self::read_extracted_facts_for_row(&read_txn, &row)?;
            snapshot.push(PathQueryResult { row, facts });
        }
        Ok(snapshot)
    }

    fn require_active_generation(&self) -> Result<u64> {
        self.active_generation()?
            .ok_or_else(|| anyhow!("no committed repo index generation; run `repo-index build` first"))
    }

    fn active_path_rows(&self) -> Result<Vec<PathRow>> {
        let read_txn = self.db.begin_read().context("begin repo-index read transaction")?;
        let generation = self.active_generation()?;
        Self::active_path_rows_from_txn(&read_txn, generation)
    }

    fn active_path_rows_from_txn(read_txn: &redb::ReadTransaction, generation: Option<u64>) -> Result<Vec<PathRow>> {
        let Some(generation) = generation else {
            return Ok(Vec::new());
        };
        let path_table = read_txn.open_table(PATH_TABLE).context("open repo-index path table")?;
        let prefix = format!("{generation:020}:");
        let mut rows = Vec::new();
        for entry in path_table.iter().context("iterate repo-index path table")? {
            let (key, value) = entry.context("read repo-index path row")?;
            if key.value().starts_with(prefix.as_str()) {
                rows.push(decode::<PathRow>(value.value())?);
            }
        }
        rows.sort_by_key(|row| row.path.clone());
        Ok(rows)
    }

    fn read_extracted_facts_for_row(
        read_txn: &redb::ReadTransaction,
        row: &PathRow,
    ) -> Result<super::model::ExtractedFacts> {
        let extraction_table = read_txn.open_table(EXTRACTION_TABLE).context("open repo-index extraction table")?;
        let extraction_key = row.extraction_key();
        let encoded = extraction_table
            .get(extraction_key.as_str())
            .with_context(|| format!("read extraction row {extraction_key}"))?
            .ok_or_else(|| anyhow!("missing extraction row {extraction_key}"))?;
        decode(encoded.value())
    }
}

fn schema_mismatch(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| cause.to_string().starts_with("repo-index schema mismatch:"))
}

fn path_key(generation: u64, path: &str) -> String {
    format!("{generation:020}:{path}")
}

fn encode<T: serde::Serialize>(value: &T) -> Result<Vec<u8>> {
    serde_json::to_vec(value).context("serialize repo-index row")
}

fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    serde_json::from_slice(bytes).context("deserialize repo-index row")
}

fn validate_digest_hex(digest_hex: &str) -> Result<()> {
    if digest_hex.len() != 64 || !digest_hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("digest must be a 64-character hex BLAKE3 value");
    }
    Ok(())
}

fn normalize_query_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn scan_tracked_files(root: &Path) -> Result<Vec<ScannedFile>> {
    let mut scanned = Vec::new();
    collect_files_with_extension(root, Path::new("src"), "rs", &mut scanned, false)?;
    push_if_exists(root, Path::new("docs/requirements.md"), &mut scanned);
    collect_files_with_extension(root, Path::new("openspec/specs"), "md", &mut scanned, false)?;
    collect_files_with_extension(root, Path::new("openspec/changes"), "md", &mut scanned, true)?;
    push_if_exists(root, Path::new("README.md"), &mut scanned);
    push_if_exists(root, Path::new("AGENTS.md"), &mut scanned);
    scanned.sort_by_key(|file| file.relative_path.clone());
    Ok(scanned)
}

fn push_if_exists(root: &Path, relative: &Path, scanned: &mut Vec<ScannedFile>) {
    let absolute = root.join(relative);
    if absolute.is_file() {
        scanned.push(ScannedFile {
            absolute_path: absolute,
            relative_path: normalize_relative_path(relative),
        });
    }
}

fn collect_files_with_extension(
    root: &Path,
    relative_dir: &Path,
    extension: &str,
    scanned: &mut Vec<ScannedFile>,
    skip_archive: bool,
) -> Result<()> {
    let absolute_dir = root.join(relative_dir);
    if !absolute_dir.is_dir() {
        return Ok(());
    }
    collect_files_recursive(root, relative_dir, extension, scanned, skip_archive)
}

fn collect_files_recursive(
    root: &Path,
    relative_dir: &Path,
    extension: &str,
    scanned: &mut Vec<ScannedFile>,
    skip_archive: bool,
) -> Result<()> {
    let absolute_dir = root.join(relative_dir);
    let mut entries: Vec<_> = fs::read_dir(&absolute_dir)
        .with_context(|| format!("read directory {}", absolute_dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("collect directory {}", absolute_dir.display()))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let file_type = entry.file_type().with_context(|| format!("read file type for {}", entry.path().display()))?;
        let file_name = entry.file_name();
        let child_relative = relative_dir.join(&file_name);
        let child_relative_str = normalize_relative_path(&child_relative);
        if file_type.is_dir() {
            if skip_archive && child_relative_str == "openspec/changes/archive" {
                continue;
            }
            collect_files_recursive(root, &child_relative, extension, scanned, skip_archive)?;
        } else if file_type.is_file() && child_relative.extension().and_then(|value| value.to_str()) == Some(extension)
        {
            scanned.push(ScannedFile {
                absolute_path: entry.path(),
                relative_path: child_relative_str,
            });
        }
    }

    Ok(())
}

fn normalize_relative_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use anyhow::Result;
    use proptest::prelude::*;
    use redb::Database;
    use tempfile::TempDir;

    use super::EXTRACTION_TABLE;
    use super::META_TABLE;
    use super::PATH_TABLE;
    use super::RefreshOptions;
    use super::RepoIndex;
    use super::STATS_TABLE;
    use crate::repo_index::model::CURRENT_SCHEMA_VERSION;
    use crate::repo_index::model::DEFAULT_DB_PATH;

    struct Fixture {
        root: TempDir,
    }

    impl Fixture {
        fn new() -> Result<Self> {
            let base = std::env::current_dir()?.join("target-test/repo-index");
            fs::create_dir_all(&base)?;
            let root = tempfile::Builder::new().prefix("repo-index-").tempdir_in(base)?;
            Ok(Self { root })
        }

        fn root(&self) -> &Path {
            self.root.path()
        }

        fn write(&self, relative: &str, contents: &str) -> Result<()> {
            let absolute = self.root().join(relative);
            if let Some(parent) = absolute.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(absolute, contents)?;
            Ok(())
        }

        fn remove(&self, relative: &str) -> Result<()> {
            fs::remove_file(self.root().join(relative))?;
            Ok(())
        }

        fn clear(&self, relative: &str) -> Result<()> {
            let absolute = self.root().join(relative);
            if !absolute.exists() {
                return Ok(());
            }
            if absolute.is_dir() {
                fs::remove_dir_all(absolute)?;
            } else {
                fs::remove_file(absolute)?;
            }
            Ok(())
        }

        fn index(&self) -> Result<RepoIndex> {
            RepoIndex::open(self.root(), &self.root().join(DEFAULT_DB_PATH))
        }
    }

    #[derive(Clone, Debug)]
    struct RepoState {
        rust_files: Vec<Option<u8>>,
        openspec_variant: Option<u8>,
    }

    impl RepoState {
        fn apply(&self, fixture: &Fixture) -> Result<()> {
            fixture.clear("src")?;
            fixture.clear("docs")?;
            fixture.clear("openspec")?;
            fixture.write("docs/requirements.md", minimal_requirements())?;
            for (slot, variant) in self.rust_files.iter().enumerate() {
                if let Some(variant) = variant {
                    let relative = format!("src/generated_{slot}.rs");
                    fixture.write(&relative, &rust_template(slot, *variant))?;
                }
            }
            if let Some(variant) = self.openspec_variant {
                fixture.write("openspec/changes/demo/specs/repo-index/spec.md", openspec_template(variant))?;
            }
            Ok(())
        }
    }

    fn minimal_requirements() -> &'static str {
        "# Verified Logic Requirements\n\n## Repository Index\n\nr[repo-index.query]\nQueries MUST be deterministic.\n"
    }

    fn rust_template(slot: usize, variant: u8) -> String {
        match variant % 5 {
            0 => "pub fn shared_symbol() {}\n".to_string(),
            1 => format!("// r[impl repo-index.query]\npub fn slot_{slot}() {{}}\n"),
            2 => format!("pub struct SlotStruct{slot};\n"),
            3 => format!("pub proof fn lemma_{slot}() {{}}\n"),
            _ => format!("pub open spec fn spec_{slot}() -> bool {{ true }}\n"),
        }
    }

    fn openspec_template(variant: u8) -> &'static str {
        match variant % 3 {
            0 => "# Demo\n\n## Requirements\n\n### Requirement: Query\n\nr[demo.query]\nText\n",
            1 => "# Demo\n\n## Requirements\n\n### Requirement: Digest\n\nr[demo.digest]\nText\n",
            _ => "# Demo\n\n## Requirements\n\n### Requirement: Atomic Refresh\n\nr[demo.atomic-refresh]\nText\n",
        }
    }

    fn repo_state_strategy() -> impl Strategy<Value = RepoState> {
        (prop::collection::vec(prop::option::of(0u8..5), 3), prop::option::of(0u8..3)).prop_map(
            |(rust_files, openspec_variant)| RepoState {
                rust_files,
                openspec_variant,
            },
        )
    }

    #[test]
    fn stale_schema_is_recreated_on_open() -> Result<()> {
        let fixture = Fixture::new()?;
        fixture.write("docs/requirements.md", minimal_requirements())?;
        let db_path = fixture.root().join(DEFAULT_DB_PATH);
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let stale = Database::create(&db_path)?;
        let write_txn = stale.begin_write()?;
        {
            let mut meta = write_txn.open_table(META_TABLE)?;
            meta.insert("schema_version", CURRENT_SCHEMA_VERSION - 1)?;
            write_txn.open_table(PATH_TABLE)?;
            write_txn.open_table(EXTRACTION_TABLE)?;
            write_txn.open_table(STATS_TABLE)?;
        }
        write_txn.commit()?;
        drop(stale);

        let index = fixture.index()?;
        assert_eq!(index.active_generation()?, None);
        let summary = index.refresh()?;
        assert!(summary.tracked_paths >= 1);
        Ok(())
    }

    // r[verify repo-index.content-digest]
    #[test]
    fn stable_digest_survives_reopen() -> Result<()> {
        let fixture = Fixture::new()?;
        fixture.write("src/sample.rs", "pub fn alpha() {}\n")?;
        fixture.write("docs/requirements.md", minimal_requirements())?;
        let index = fixture.index()?;
        index.refresh()?;
        let first = index.query_path("src/sample.rs")?.expect("path query after first refresh").row.digest_hex;
        drop(index);
        let reopened = fixture.index()?;
        let second = reopened.query_path("src/sample.rs")?.expect("path query after reopen").row.digest_hex;
        assert_eq!(first, second);
        Ok(())
    }

    // r[verify repo-index.persistent-store]
    #[test]
    fn reopen_answers_queries_without_rebuild() -> Result<()> {
        let fixture = Fixture::new()?;
        fixture.write("src/sample.rs", "// r[impl repo-index.query]\npub fn alpha() {}\n")?;
        fixture.write("docs/requirements.md", minimal_requirements())?;
        let index = fixture.index()?;
        index.refresh()?;
        drop(index);
        let reopened = fixture.index()?;
        let symbol_hits = reopened.query_symbol("alpha")?;
        assert_eq!(symbol_hits.len(), 1);
        let requirement = reopened.query_requirement("repo-index.query")?;
        assert_eq!(requirement.definitions.len(), 1);
        assert_eq!(requirement.references.len(), 1);
        Ok(())
    }

    // r[verify repo-index.atomic-refresh]
    #[test]
    fn failed_refresh_keeps_prior_generation_visible() -> Result<()> {
        let fixture = Fixture::new()?;
        fixture.write("src/sample.rs", "pub fn alpha() {}\n")?;
        fixture.write("docs/requirements.md", minimal_requirements())?;
        let index = fixture.index()?;
        let first = index.refresh()?;
        fixture.write("src/sample.rs", "pub fn beta() {}\n")?;
        let error = index.refresh_with_options(RefreshOptions {
            fail_before_commit: true,
        });
        assert!(error.is_err());
        assert_eq!(index.active_generation()?, Some(first.generation));
        let path = index.query_path("src/sample.rs")?.expect("path row after failed refresh");
        assert_eq!(path.row.digest_hex, blake3::hash(b"pub fn alpha() {}\n").to_hex().to_string());
        Ok(())
    }

    // r[verify repo-index.incremental-refresh]
    #[test]
    fn refresh_reports_add_change_delete_and_reuse() -> Result<()> {
        let fixture = Fixture::new()?;
        fixture.write("src/a.rs", "pub fn alpha() {}\n")?;
        fixture.write("docs/requirements.md", minimal_requirements())?;
        let index = fixture.index()?;
        index.refresh()?;
        let extraction_rows_after_first = index.stored_extraction_rows()?;

        fixture.write("src/a.rs", "pub fn beta() {}\n")?;
        fixture.write("src/b.rs", "pub fn gamma() {}\n")?;
        let summary = index.refresh()?;
        assert_eq!(summary.added_paths, 1);
        assert_eq!(summary.changed_paths, 1);
        assert_eq!(summary.deleted_paths, 0);
        assert!(summary.reused_extractions >= 1);
        let extraction_rows_after_second = index.stored_extraction_rows()?;
        assert!(extraction_rows_after_second >= extraction_rows_after_first);

        fixture.remove("src/b.rs")?;
        let summary = index.refresh()?;
        assert_eq!(summary.deleted_paths, 1);
        assert!(index.query_path("src/b.rs")?.is_none());
        Ok(())
    }

    // r[verify repo-index.cross-reference]
    // r[verify repo-index.query]
    #[test]
    fn path_and_requirement_queries_include_expected_facts() -> Result<()> {
        let fixture = Fixture::new()?;
        fixture.write("src/sample.rs", "// r[impl repo-index.query]\npub fn alpha() {}\n")?;
        fixture.write(
            "openspec/changes/demo/specs/repo-index/spec.md",
            "# Demo\n\n## Requirements\n\n### Requirement: Query\n\nr[demo.query]\nText\n",
        )?;
        fixture.write("docs/requirements.md", minimal_requirements())?;
        let index = fixture.index()?;
        index.refresh()?;

        let path = index.query_path("openspec/changes/demo/specs/repo-index/spec.md")?.expect("openspec path query");
        assert_eq!(path.row.openspec.as_ref().and_then(|ctx| ctx.change_name.as_deref()), Some("demo"));
        assert!(path.facts.headings.iter().any(|heading| heading.text == "Requirement: Query"));
        assert!(path.facts.requirement_defs.iter().any(|definition| definition.id == "demo.query"));

        let requirement = index.query_requirement("repo-index.query")?;
        assert_eq!(requirement.definitions.len(), 1);
        assert_eq!(requirement.references.len(), 1);

        let openspec_requirement = index.query_requirement("demo.query")?;
        assert_eq!(openspec_requirement.definitions.len(), 1);
        assert_eq!(openspec_requirement.definitions[0].path, "openspec/changes/demo/specs/repo-index/spec.md");
        assert_eq!(openspec_requirement.references.len(), 0);
        Ok(())
    }

    // r[verify repo-index.incremental-refresh]
    #[test]
    fn unchanged_refresh_keeps_observable_results_stable() -> Result<()> {
        let fixture = Fixture::new()?;
        fixture.write("src/sample.rs", "// r[impl repo-index.query]\npub fn alpha() {}\n")?;
        fixture.write("docs/requirements.md", minimal_requirements())?;
        let index = fixture.index()?;
        index.refresh()?;
        let before = index.query_requirement("repo-index.query")?;
        let stats_before = index.stats()?;

        let summary = index.refresh()?;
        assert_eq!(summary.changed_paths, 0);
        assert_eq!(summary.added_paths, 0);
        let after = index.query_requirement("repo-index.query")?;
        let stats_after = index.stats()?;
        assert_eq!(before, after);
        assert_eq!(stats_before, stats_after);
        Ok(())
    }

    proptest! {
        // r[verify repo-index.incremental-refresh]
        #[test]
        fn prop_idempotent_rebuild(state in repo_state_strategy()) {
            let fixture = Fixture::new().expect("fixture");
            state.apply(&fixture).expect("apply initial state");
            let index = fixture.index().expect("open index");
            index.refresh().expect("first refresh");
            let snapshot_before = index.debug_snapshot().expect("snapshot before");
            let stats_before = index.stats().expect("stats before");

            let summary = index.refresh().expect("second refresh");
            prop_assert_eq!(summary.changed_paths, 0);
            prop_assert_eq!(summary.added_paths, 0);
            let snapshot_after = index.debug_snapshot().expect("snapshot after");
            let stats_after = index.stats().expect("stats after");
            prop_assert_eq!(snapshot_before, snapshot_after);
            prop_assert_eq!(stats_before, stats_after);
        }

        // r[verify repo-index.incremental-refresh]
        #[test]
        fn prop_one_shot_matches_incremental(
            initial in repo_state_strategy(),
            final_state in repo_state_strategy(),
        ) {
            let incremental_fixture = Fixture::new().expect("incremental fixture");
            initial.apply(&incremental_fixture).expect("apply initial state");
            let incremental = incremental_fixture.index().expect("open incremental index");
            incremental.refresh().expect("refresh initial state");
            final_state.apply(&incremental_fixture).expect("apply final state");
            incremental.refresh().expect("refresh final state");
            let incremental_snapshot = incremental.debug_snapshot().expect("incremental snapshot");
            let incremental_stats = incremental.stats().expect("incremental stats");

            let one_shot_fixture = Fixture::new().expect("one-shot fixture");
            final_state.apply(&one_shot_fixture).expect("apply one-shot state");
            let one_shot = one_shot_fixture.index().expect("open one-shot index");
            one_shot.refresh().expect("one-shot refresh");
            let one_shot_snapshot = one_shot.debug_snapshot().expect("one-shot snapshot");
            let one_shot_stats = one_shot.stats().expect("one-shot stats");

            prop_assert_eq!(incremental_snapshot, one_shot_snapshot);
            prop_assert_eq!(incremental_stats, one_shot_stats);
        }
    }
}
