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
const MAX_OVERVIEW_HISTORIES: usize = 6;

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

// r[impl kiln.dashboard.product.shell]
// r[impl kiln.dashboard.product.shell.scenario.inventory]
impl CatalogRenderer {
    pub(crate) fn new(catalog: &Catalog) -> Self {
        let mut out = String::from(
            "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Kiln · Observation workspace</title><style>",
        );
        out.push_str(include_str!("render/catalog.css"));
        out.push_str(r##"</style></head><body>
<a class="skip-link" href="#app-main">Skip to workspace</a>
<aside id="app-sidebar" class="app-sidebar" aria-label="Workspace sidebar">
  <a class="brand" href="#app-main" aria-label="Kiln observation workspace"><span class="brand-mark" aria-hidden="true">k</span><span>Kiln<span class="brand-caption">Observation workspace</span></span></a>
  <div class="workspace-card"><span class="workspace-avatar" aria-hidden="true">K</span><div><strong>Local workspace</strong><span class="workspace-caption"><span class="status-dot" aria-hidden="true"></span> Offline · Read only</span></div></div>
  <p class="nav-label app-only">Workspace</p>
  <nav class="app-navigation app-only" aria-label="Main navigation">
"##);
        for (route, label, path) in [
            (
                "overview",
                "Overview",
                "M3 3h7v7H3z M14 3h7v7h-7z M3 14h7v7H3z M14 14h7v7h-7z",
            ),
            ("projects", "Projects", "M3 7h7l2-3h9v16H3z"),
            ("runs", "Runs", "M9 5l11 7-11 7z M4 5v14"),
            ("activity", "Activity", "M2 12h5l3-8 4 16 3-8h5"),
            (
                "evidence",
                "Evidence",
                "M6 3h9l4 4v14H6z M14 3v5h5 M9 12h7 M9 16h5",
            ),
            (
                "settings",
                "Settings",
                "M4 6h16 M4 12h16 M4 18h16 M8 3v6 M16 9v6 M10 15v6",
            ),
        ] {
            out.push_str(&format!(r#"<button type="button" data-app-route="{route}"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="{path}"/></svg><span>{label}</span></button>"#));
        }
        out.push_str(r##"
  </nav>
  <div class="sidebar-boundary"><span class="eyebrow">Built for observation</span><p>Your reports stay in this file. No accounts, hosted services, or CI controls.</p><span class="offline-badge"><span class="status-dot" aria-hidden="true"></span> Self-contained HTML</span></div>
  <div class="sidebar-bottom app-only"><button type="button" id="app-shortcuts"><span aria-hidden="true">?</span> Keyboard shortcuts <kbd>?</kbd></button></div>
</aside>
<div class="app-workspace">
  <header class="app-topbar">
    <div class="topbar-location"><button type="button" id="app-menu-toggle" class="icon-button app-only" aria-controls="app-sidebar" aria-expanded="false" aria-label="Toggle navigation"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 6h16M4 12h16M4 18h16"/></svg></button><span class="topbar-workspace">Workspace</span><span class="breadcrumb-divider" aria-hidden="true">/</span><span id="app-breadcrumb">Overview</span></div>
    <div class="topbar-actions app-only">
      <div class="catalog-search-wrap"><label class="search-field" for="catalog-search"><svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 5 5"/></svg><input id="catalog-search" type="search" placeholder="Search projects &amp; histories…" autocomplete="off" aria-label="Search declared project, history, source and revision metadata" aria-controls="catalog-search-results" aria-expanded="false"><kbd>/</kbd></label><div id="catalog-search-results" class="search-results" hidden><p id="catalog-search-count" role="status" aria-live="polite"></p><div id="catalog-search-list"></div></div></div>
      <button type="button" id="app-theme-toggle" class="icon-button" aria-label="Change appearance" title="Change appearance"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 14A8.5 8.5 0 0 1 10 4a8.5 8.5 0 1 0 10 10Z"/></svg></button>
      <span class="header-readonly"><span class="status-dot" aria-hidden="true"></span>Read only</span>
    </div>
  </header>
  <main id="app-main" class="app-main" tabindex="-1">
    <div class="page-heading"><div><p class="eyebrow">Local observation</p><h1 id="app-title">Workspace overview</h1><p id="app-subtitle">Explore your declared projects and independently admitted histories.</p></div><button type="button" id="app-refresh" class="button button-secondary app-only"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 7v5h-5 M20 12a8 8 0 1 1-2-6"/></svg>Reload report</button></div>
    <section data-app-panel="overview" class="overview-panel" aria-label="Workspace overview">
      <div class="welcome-card"><div><span class="eyebrow">A clear view of your CI</span><h2>Your workspace, at a glance.</h2><p>Browse the catalog, choose a history, and follow the evidence. Every report keeps its own admitted state and observation identity.</p><button type="button" class="button button-primary app-only" data-app-route="projects">Explore projects <span aria-hidden="true">→</span></button></div><div class="welcome-art" aria-hidden="true"><span class="art-orbit orbit-one"></span><span class="art-orbit orbit-two"></span><span class="art-core">k</span><span class="art-node node-one"></span><span class="art-node node-two"></span><span class="art-node node-three"></span></div></div>
      <div class="inventory-grid">
"##);
        let history_count: usize = catalog
            .projects
            .iter()
            .map(|project| project.histories.len())
            .sum();
        out.push_str(&format!(r#"<article class="inventory-card"><span class="metric-label">Declared projects</span><strong class="metric-value">{}</strong><p>Projects in this catalog</p></article><article class="inventory-card"><span class="metric-label">Admitted histories</span><strong class="metric-value">{history_count}</strong><p>Independent reports to explore</p></article>"#, catalog.projects.len()));
        out.push_str(r##"<article class="inventory-card inventory-note"><span class="metric-label">Workspace mode</span><strong class="metric-value mode-value"><span class="status-dot" aria-hidden="true"></span> Offline</strong><p>Read-only, with no external requests</p></article></div>
      <div class="overview-columns"><section class="surface quick-open"><div class="section-heading"><div><h2>Open a history</h2><p>A starting point from your declared catalog</p></div><button type="button" class="text-button app-only" data-app-route="projects">All projects <span aria-hidden="true">→</span></button></div><ul class="history-shortcuts">
"##);
        let mut index = 0;
        for project in &catalog.projects {
            for history in &project.histories {
                if index < MAX_OVERVIEW_HISTORIES {
                    out.push_str(&format!(r##"<li><span class="history-symbol" aria-hidden="true">↳</span><div><strong>{}</strong><span>{}</span></div><button type="button" class="button button-secondary app-only" data-open-history data-project="{}" data-history="{}">Open<span class="sr-only"> {} / {}</span><span aria-hidden="true">↗</span></button><a class="no-js-only" href="#catalog-report-{index}">Read report</a></li>"##, escape(&project.label), escape(&history.label), escape(&project.id), escape(&history.id), escape(&project.label), escape(&history.label)));
                }
                index += 1;
            }
        }
        out.push_str(r##"</ul></section><aside class="surface scope-card"><span class="scope-icon" aria-hidden="true">◈</span><h2>Evidence, not authority</h2><p>Project, history, source and revision labels are operator declarations, not verified source facts.</p><p>Each report is independently admitted. There is no combined revision, global run total, or global observation identity.</p><span class="boundary-label">No execution · No provider effects</span></aside></div>
    </section>
    <section data-app-panel="projects" class="projects-panel" aria-label="Projects and histories">
      <section class="surface index-surface"><div class="section-heading"><div><span class="eyebrow">Repository explorer</span><h2>Find your next history</h2><p>The Rats / Ratzilla index is the primary interactive catalog. Project cards and HTML navigation remain available below.</p></div><span class="subtle-badge">Pinned WASM index</span></div><div id="rats-index" aria-label="Repository and history index"></div><p id="rats-index-status" role="status">The interactive index requires JavaScript and WebAssembly. HTML navigation remains available below.</p></section>
      <div class="section-heading project-heading"><div><h2>Declared projects</h2><p>Open an existing history to inspect its admitted report.</p></div><p id="project-result-count" role="status" aria-live="polite"></p></div>
      <div id="project-grid" class="project-grid">
"##);
        let mut index = 0;
        for project in &catalog.projects {
            out.push_str(&format!(r#"<article class="project-card surface" data-project-card data-project-id="{}"><div class="project-card-heading"><span class="project-avatar" aria-hidden="true"><svg viewBox="0 0 24 24"><path d="M3 7h7l2-3h9v16H3z"/></svg></span><span class="subtle-badge">{} {}</span></div><h3>{}</h3><p class="project-identity">{}</p><ul class="project-histories">"#, escape(&project.id), project.histories.len(), if project.histories.len() == 1 { "history" } else { "histories" }, escape(&project.label), escape(&project.id)));
            for history in &project.histories {
                out.push_str(&format!(r#"<li><div class="history-heading"><strong>{}</strong><button type="button" class="text-button app-only" data-open-history data-project="{}" data-history="{}">Open <span class="sr-only">{} / {}</span><span aria-hidden="true">↗</span></button></div><dl class="history-metadata">"#, escape(&history.label), escape(&project.id), escape(&history.id), escape(&project.label), escape(&history.label)));
                for (label, value) in [("Source", &history.source), ("Revision", &history.revision)]
                {
                    if let Some(value) = value {
                        out.push_str(&format!(
                            "<div><dt>{label}</dt><dd>{}</dd></div>",
                            escape(value)
                        ));
                    }
                }
                out.push_str(&format!(r##"</dl><a class="no-js-only" href="#catalog-report-{index}">Read admitted report</a></li>"##));
                index += 1;
            }
            out.push_str("</ul></article>");
        }
        out.push_str(r##"</div><div id="project-empty" class="empty-state" hidden><h3>No matching projects</h3><p>Try another project, history, source, or revision in the metadata search.</p></div>
    </section>
    <section data-app-panel="settings" class="settings-panel" aria-label="Workspace settings">
      <section class="surface settings-card"><div class="section-heading"><div><h2>Appearance</h2><p>Personalize this browser’s presentation. Report evidence stays unchanged.</p></div></div><div class="settings-row app-only"><div><label for="app-theme">Theme</label><p>Use your system preference or choose a fixed appearance.</p></div><select id="app-theme"><option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option></select></div><div class="settings-row app-only"><div><label for="app-density">Display density</label><p>Choose the spacing that works for your workspace.</p></div><select id="app-density"><option value="comfortable">Comfortable</option><option value="compact">Compact</option></select></div><div class="settings-row app-only"><div><h3>Reset presentation preferences</h3><p>Return theme and density to their defaults.</p></div><button type="button" id="app-reset-preferences" class="button button-secondary">Reset preferences</button></div><noscript><p>Enable JavaScript to change presentation preferences.</p></noscript></section>
      <section class="surface settings-card"><span class="eyebrow">Product boundary</span><h2>A local, read-only workspace</h2><p>This self-contained file presents independently admitted Kiln reports. It is not a hosted service and has no accounts, billing, collaboration service, or CI execution controls.</p><div class="boundary-grid"><div><strong>Local by design</strong><p>No external assets or network requests. Reload reads this report again; it does not collect new CI data.</p></div><div><strong>Independent evidence</strong><p>Each history retains its own report and revision. Declared source metadata is not source verification.</p></div></div></section>
    </section>
    <section id="catalog-context" class="surface catalog-context app-only" aria-label="Selected report context">
      <div class="context-heading"><span class="eyebrow">Selected report</span><span class="subtle-badge">Independently admitted</span></div>
      <div class="catalog-controls"><label for="catalog-project">Project<select id="catalog-project"></select></label><label for="catalog-history">History<select id="catalog-history"></select></label></div>
      <dl class="context-metadata"><div><dt>Project</dt><dd id="selected-project">Not selected</dd></div><div><dt>History</dt><dd id="selected-history">Not selected</dd></div><div><dt>Declared source</dt><dd id="selected-source">Not declared</dd></div><div><dt>Declared revision</dt><dd id="selected-revision">Not declared</dd></div></dl>
      <p id="selected-observation">No admitted report selected.</p>
      <p id="catalog-status" role="status" aria-live="polite"></p>
    </section>
    <details id="catalog-navigation" class="surface fallback-navigation" open><summary>HTML repository and history navigation</summary><nav aria-label="HTML repository and history navigation"><ul>
"##);
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
        out.push_str("</ul></nav></details><div id=\"catalog-reports\">");
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
        report: &crate::report::DashboardReport,
    ) -> Result<(), String> {
        let out = &mut self.out;
        let index = self.index;
        // Escaped srcdoc keeps untrusted event strings inside an isolated document.
        out.push_str(&format!("<section id=\"catalog-report-{index}\" data-project=\"{}\" data-history=\"{}\" data-project-label=\"{}\" data-history-label=\"{}\" data-record-count=\"{}\" data-state-revision=\"{}\"", escape(&project.id), escape(&history.id), escape(&project.label), escape(&history.label), report.journal.record_count, report.journal.state_revision));
        for (name, value) in [("source", &history.source), ("revision", &history.revision)] {
            if let Some(value) = value {
                out.push_str(&format!(" data-{name}=\"{}\"", escape(value)));
            }
        }
        out.push_str(&format!(
            "> <h2>{} / {}</h2>",
            escape(&project.label),
            escape(&history.label)
        ));
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
        out.push_str(r#"</div></main><footer class="app-footer"><div><strong>Kiln</strong><span>Local evidence. Independent histories.</span><span class="footer-readonly">Read-only observation workspace</span></div><details><summary>Browser index licenses</summary><pre>"#);
        push_escaped(out, include_str!("render/generated/rats-index-notices.txt"));
        out.push_str(r#"</pre></details></footer></div><dialog id="app-keyboard-help" aria-labelledby="keyboard-help-title"><div class="dialog-heading"><h2 id="keyboard-help-title">Keyboard shortcuts</h2><button type="button" id="app-close-shortcuts" class="icon-button" aria-label="Close keyboard shortcuts">×</button></div><p>Navigate the workspace without changing report evidence.</p><dl class="shortcut-list"><div><dt>Search project metadata</dt><dd><kbd>/</kbd></dd></div><div><dt>Show this help</dt><dd><kbd>?</kbd></dd></div><div><dt>Close search or dialog</dt><dd><kbd>Esc</kbd></dd></div></dl><p class="muted">Run search and report navigation remain inside each independent report.</p></dialog><script>"#);
        out.push_str(include_str!("render/catalog.js"));
        if let Some(seconds) = refresh {
            out.push_str(&format!(
                "\nsetTimeout(() => location.reload(), {});",
                seconds.saturating_mul(1000)
            ));
        }
        out.push_str("</script>");
        out.push_str("<script type=\"module\">");
        out.push_str(include_str!("render/generated/rats-index.js"));
        out.push_str("\ntry {\nconst bytes = Uint8Array.from(atob(`");
        out.push_str(include_str!("render/generated/rats-index.wasm.b64"));
        out.push_str("`), (byte) => byte.charCodeAt(0));\nwindow.kilnRatsIndex = await window.kilnRatsLoader.load_index(bytes);\ndocument.dispatchEvent(new Event(\"kiln-rats-ready\"));\n} catch (error) {\nconst status = document.getElementById(\"rats-index-status\");\nstatus.hidden = false;\nstatus.textContent = `Repository/history index initialization failed: ${error instanceof Error ? error.message : String(error)}. HTML navigation remains available.`;\n}\n</script>");
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
