use std::fmt;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

pub const CURRENT_SCHEMA_VERSION: u64 = 3;
pub const DEFAULT_DB_PATH: &str = "data/repo-index.redb";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtractorKind {
    Rust,
    RequirementsDoc,
    OpenSpecMarkdown,
    Markdown,
}

impl ExtractorKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::RequirementsDoc => "requirements-doc",
            Self::OpenSpecMarkdown => "openspec-markdown",
            Self::Markdown => "markdown",
        }
    }

    pub fn from_path(path: &str) -> Self {
        if path.starts_with("src/") && has_extension(path, "rs") {
            Self::Rust
        } else if path == "docs/requirements.md" {
            Self::RequirementsDoc
        } else if path.starts_with("openspec/") && has_extension(path, "md") {
            Self::OpenSpecMarkdown
        } else {
            Self::Markdown
        }
    }

    pub fn extraction_key(self, digest_hex: &str) -> String {
        format!("{}:{digest_hex}", self.as_str())
    }
}

impl fmt::Display for ExtractorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SymbolKind {
    Module,
    Struct,
    Enum,
    Trait,
    TypeAlias,
    Const,
    Static,
    Function,
    ProofFunction,
    SpecFunction,
}

impl SymbolKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::TypeAlias => "type-alias",
            Self::Const => "const",
            Self::Static => "static",
            Self::Function => "function",
            Self::ProofFunction => "proof-function",
            Self::SpecFunction => "spec-function",
        }
    }
}

impl fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SymbolFact {
    pub name: String,
    pub kind: SymbolKind,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RequirementReference {
    pub id: String,
    pub relation: String,
    pub line: u32,
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RequirementDefinition {
    pub id: String,
    pub section: Option<String>,
    pub line: u32,
    pub body: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct HeadingFact {
    pub level: u8,
    pub line: u32,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ExtractedFacts {
    pub symbols: Vec<SymbolFact>,
    pub requirement_refs: Vec<RequirementReference>,
    pub requirement_defs: Vec<RequirementDefinition>,
    pub headings: Vec<HeadingFact>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct OpenSpecContext {
    pub change_name: Option<String>,
    pub domain: Option<String>,
    pub document_role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathRow {
    pub path: String,
    pub digest_hex: String,
    pub extractor_kind: ExtractorKind,
    pub size_bytes: u64,
    pub openspec: Option<OpenSpecContext>,
}

impl PathRow {
    pub fn extraction_key(&self) -> String {
        self.extractor_kind.extraction_key(&self.digest_hex)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GenerationStats {
    pub tracked_paths: u64,
    pub unique_digests: u64,
    pub symbol_count: u64,
    pub requirement_reference_count: u64,
    pub requirement_definition_count: u64,
    pub heading_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshSummary {
    pub generation: u64,
    pub tracked_paths: u64,
    pub inserted_extractions: u64,
    pub reused_extractions: u64,
    pub added_paths: u64,
    pub changed_paths: u64,
    pub unchanged_paths: u64,
    pub deleted_paths: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathQueryResult {
    pub row: PathRow,
    pub facts: ExtractedFacts,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SymbolHit {
    pub path: String,
    pub digest_hex: String,
    pub symbol: SymbolFact,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RequirementDefinitionHit {
    pub path: String,
    pub digest_hex: String,
    pub definition: RequirementDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RequirementReferenceHit {
    pub path: String,
    pub digest_hex: String,
    pub reference: RequirementReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RequirementQueryResult {
    pub definitions: Vec<RequirementDefinitionHit>,
    pub references: Vec<RequirementReferenceHit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestFactsView {
    pub extractor_kind: ExtractorKind,
    pub facts: ExtractedFacts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestQueryResult {
    pub digest_hex: String,
    pub paths: Vec<PathRow>,
    pub facts_by_kind: Vec<DigestFactsView>,
}

fn has_extension(path: &str, extension: &str) -> bool {
    Path::new(path).extension().is_some_and(|value| value.eq_ignore_ascii_case(extension))
}
