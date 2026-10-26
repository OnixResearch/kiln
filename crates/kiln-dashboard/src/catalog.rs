//! Closed, bounded declarations for independently admitted dashboard histories.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const MAX_CATALOG_BYTES: usize = 1024 * 1024;
pub const MAX_PROJECTS: usize = 32;
pub const MAX_HISTORIES: usize = 128;
pub const MAX_INPUT_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_RENDERED_BYTES: usize = 64 * 1024 * 1024;
const MAX_LABEL_BYTES: usize = 1024;
const MAX_ID_BYTES: usize = 128;

// r[impl kiln.dashboard.repositories.catalog]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema_version: u32,
    pub bounds: CatalogBounds,
    pub projects: Vec<Project>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogBounds {
    pub max_catalog_bytes: usize,
    pub max_projects: usize,
    pub max_histories: usize,
    pub max_input_bytes: u64,
    pub max_rendered_bytes: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub id: String,
    pub label: String,
    pub histories: Vec<History>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct History {
    pub id: String,
    pub label: String,
    pub source: Option<String>,
    pub revision: Option<String>,
    pub input: HistoryInput,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HistoryInput {
    Journal { path: PathBuf },
    Store { path: PathBuf, stream: String },
}

impl HistoryInput {
    pub(crate) fn path(&self) -> &Path {
        match self {
            Self::Journal { path } | Self::Store { path, .. } => path,
        }
    }
}

impl Catalog {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_CATALOG_BYTES {
            return Err("catalog exceeds hard byte bound".to_string());
        }
        let catalog: Self =
            serde_json::from_slice(bytes).map_err(|error| format!("catalog document: {error}"))?;
        catalog.admit(bytes.len())?;
        Ok(catalog)
    }

    pub fn admit(&self, byte_count: usize) -> Result<(), String> {
        let b = &self.bounds;
        if self.schema_version != 1 {
            return Err("unsupported catalog schema_version".to_string());
        }
        if b.max_catalog_bytes == 0
            || b.max_catalog_bytes > MAX_CATALOG_BYTES
            || b.max_projects == 0
            || b.max_projects > MAX_PROJECTS
            || b.max_histories == 0
            || b.max_histories > MAX_HISTORIES
            || b.max_input_bytes == 0
            || b.max_input_bytes > MAX_INPUT_BYTES
            || b.max_rendered_bytes == 0
            || b.max_rendered_bytes > MAX_RENDERED_BYTES
        {
            return Err("catalog bounds must be positive and within hard ceilings".to_string());
        }
        if byte_count > b.max_catalog_bytes
            || self.projects.is_empty()
            || self.projects.len() > b.max_projects
        {
            return Err("catalog byte or project bound exceeded, or no projects".to_string());
        }
        let mut projects = BTreeSet::new();
        let mut total = 0usize;
        for project in &self.projects {
            admit_id(&project.id)?;
            admit_label(&project.label)?;
            if !projects.insert(&project.id) || project.histories.is_empty() {
                return Err("duplicate project identity or empty project".to_string());
            }
            let mut histories = BTreeSet::new();
            total += project.histories.len();
            if total > b.max_histories {
                return Err("catalog total history bound exceeded".to_string());
            }
            for history in &project.histories {
                admit_id(&history.id)?;
                admit_label(&history.label)?;
                for label in history.source.iter().chain(history.revision.iter()) {
                    admit_label(label)?;
                }
                if !histories.insert(&history.id) {
                    return Err("duplicate history identity within project".to_string());
                }
                let path = history.input.path();
                let Some(text) = path.to_str() else {
                    return Err("history path must be UTF-8".to_string());
                };
                if text.is_empty() || text.chars().any(char::is_control) {
                    return Err("history path is empty or contains control characters".to_string());
                }
                if let HistoryInput::Store { stream, .. } = &history.input {
                    crate::store::parse_reference(stream)?;
                }
            }
        }
        Ok(())
    }
}

fn admit_id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > MAX_ID_BYTES
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return Err(
            "catalog identifiers must be 1..128 ASCII letters, digits, '.', '_' or '-'".to_string(),
        );
    }
    Ok(())
}

fn admit_label(value: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value.len() > MAX_LABEL_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(
            "catalog labels must be nonblank, control-free and at most 1024 bytes".to_string(),
        );
    }
    Ok(())
}

/// Incremental pure rendering; the shell supplies one admitted document at a time.
pub(crate) struct CatalogRenderer {
    out: String,
    index: usize,
    max_rendered_bytes: usize,
}

impl CatalogRenderer {
    pub(crate) fn new(catalog: &Catalog) -> Self {
        let mut out = String::from(
            "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Kiln repository observations</title><style>",
        );
        out.push_str(include_str!("render/catalog.css"));
        out.push_str("</style></head><body><header><h1>Repository observations</h1><p>Repository, history, source and revision labels are operator declarations, not verified source facts. Each report is independently admitted; there is no combined revision or global observation identity.</p><nav aria-label=\"Repository and history\"><div class=\"catalog-controls\"><label>Repository <select id=\"catalog-project\"></select></label><label>History <select id=\"catalog-history\"></select></label></div><p id=\"catalog-status\" role=\"status\"></p><ul>");
        let mut index = 0;
        for project in &catalog.projects {
            for history in &project.histories {
                out.push_str(&format!(
                    "<li><a href=\"#catalog-report-{index}\">{} / {}</a></li>",
                    escape(&project.label),
                    escape(&history.label)
                ));
                index += 1;
            }
        }
        out.push_str("</ul></nav></header><main id=\"catalog-reports\">");
        Self {
            out,
            index: 0,
            max_rendered_bytes: catalog.bounds.max_rendered_bytes,
        }
    }

    pub(crate) fn push(
        &mut self,
        project: &Project,
        history: &History,
        document: &str,
    ) -> Result<(), String> {
        let out = &mut self.out;
        let index = self.index;
        // Escaped srcdoc keeps untrusted event strings inside an isolated document.
        out.push_str(&format!("<section id=\"catalog-report-{index}\" data-project=\"{}\" data-history=\"{}\" data-project-label=\"{}\" data-history-label=\"{}\"><h2>{} / {}</h2>", escape(&project.id), escape(&history.id), escape(&project.label), escape(&history.label), escape(&project.label), escape(&history.label)));
        out.push_str("<p>Operator-declared source: ");
        out.push_str(&escape(history.source.as_deref().unwrap_or("not declared")));
        out.push_str("; operator-declared revision: ");
        out.push_str(&escape(
            history.revision.as_deref().unwrap_or("not declared"),
        ));
        out.push_str(".</p><iframe sandbox=\"allow-scripts\" title=\"");
        out.push_str(&escape(&format!(
            "{} / {} admitted dashboard",
            project.label, history.label
        )));
        out.push_str("\" srcdoc=\"");
        let encoded_bytes = document
            .chars()
            .map(|c| match c {
                '&' => 5,
                '<' | '>' => 4,
                '"' => 6,
                '\'' => 5,
                _ => c.len_utf8(),
            })
            .sum::<usize>();
        if out
            .len()
            .checked_add(encoded_bytes)
            .and_then(|n| n.checked_add(32))
            .is_none_or(|size| size > self.max_rendered_bytes)
        {
            return Err("catalog rendered output byte bound exceeded".to_string());
        }
        push_escaped(out, document);
        out.push_str("\"></iframe></section>");
        self.index += 1;
        Ok(())
    }

    pub(crate) fn finish(mut self, refresh: Option<u64>) -> Result<String, String> {
        let out = &mut self.out;
        out.push_str("</main><script>");
        out.push_str(include_str!("render/catalog.js"));
        if let Some(seconds) = refresh {
            out.push_str(&format!(
                "\nsetTimeout(() => location.reload(), {});",
                seconds.saturating_mul(1000)
            ));
        }
        out.push_str("</script>");
        if let Some(seconds) = refresh {
            out.push_str(&format!(
                "<noscript><meta http-equiv=\"refresh\" content=\"{seconds}\"></noscript>"
            ));
        }
        out.push_str("</body></html>\n");
        if out.len() > self.max_rendered_bytes {
            return Err("catalog rendered output byte bound exceeded".to_string());
        }
        Ok(self.out)
    }
}

fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    push_escaped(&mut escaped, value);
    escaped
}

fn push_escaped(escaped: &mut String, value: &str) {
    for c in value.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(c),
        }
    }
}
