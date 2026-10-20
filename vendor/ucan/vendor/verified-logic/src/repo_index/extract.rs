use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

use super::model::ExtractedFacts;
use super::model::HeadingFact;
use super::model::OpenSpecContext;
use super::model::RequirementDefinition;
use super::model::RequirementReference;
use super::model::SymbolFact;
use super::model::SymbolKind;

type SymbolPattern = (fn() -> &'static Regex, SymbolKind);

pub fn extract_facts(path: &str, bytes: &[u8]) -> ExtractedFacts {
    let text = String::from_utf8_lossy(bytes);
    if path.starts_with("src/") && has_extension(path, "rs") {
        ExtractedFacts {
            symbols: extract_rust_symbols(&text),
            requirement_refs: extract_rust_requirement_refs(&text),
            requirement_defs: Vec::new(),
            headings: Vec::new(),
        }
    } else if path == "docs/requirements.md" {
        ExtractedFacts {
            symbols: Vec::new(),
            requirement_refs: Vec::new(),
            requirement_defs: extract_requirement_definitions(&text),
            headings: extract_markdown_headings(&text),
        }
    } else if path.starts_with("openspec/") && has_extension(path, "md") {
        ExtractedFacts {
            symbols: Vec::new(),
            requirement_refs: extract_requirement_refs(&text),
            requirement_defs: extract_requirement_definitions(&text),
            headings: extract_markdown_headings(&text),
        }
    } else {
        ExtractedFacts {
            symbols: Vec::new(),
            requirement_refs: extract_requirement_refs(&text),
            requirement_defs: Vec::new(),
            headings: Vec::new(),
        }
    }
}

pub fn infer_openspec_context(path: &str) -> Option<OpenSpecContext> {
    if !path.starts_with("openspec/") {
        return None;
    }

    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 2 {
        return None;
    }

    if parts.get(1) == Some(&"changes") && parts.len() >= 3 {
        let change_name = parts[2].to_string();
        let domain = path.split("/specs/").nth(1).and_then(|tail| tail.split('/').next()).map(ToOwned::to_owned);
        let document_role = if path.ends_with("/proposal.md") {
            "proposal"
        } else if path.ends_with("/design.md") {
            "design"
        } else if path.ends_with("/tasks.md") {
            "tasks"
        } else if path.contains("/specs/") {
            "change-spec"
        } else {
            "change-doc"
        };
        return Some(OpenSpecContext {
            change_name: Some(change_name),
            domain,
            document_role: document_role.to_string(),
        });
    }

    if parts.get(1) == Some(&"specs") && parts.len() >= 4 {
        return Some(OpenSpecContext {
            change_name: None,
            domain: Some(parts[2].to_string()),
            document_role: "main-spec".to_string(),
        });
    }

    None
}

fn extract_rust_symbols(text: &str) -> Vec<SymbolFact> {
    let patterns: &[SymbolPattern] = &[
        (module_re, SymbolKind::Module),
        (struct_re, SymbolKind::Struct),
        (enum_re, SymbolKind::Enum),
        (trait_re, SymbolKind::Trait),
        (type_alias_re, SymbolKind::TypeAlias),
        (const_re, SymbolKind::Const),
        (static_re, SymbolKind::Static),
        (proof_fn_re, SymbolKind::ProofFunction),
        (spec_fn_re, SymbolKind::SpecFunction),
        (fn_re, SymbolKind::Function),
    ];

    let mut symbols = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
            continue;
        }
        for (regex, kind) in patterns {
            if let Some(captures) = regex().captures(trimmed) {
                if let Some(name) = captures.get(1) {
                    symbols.push(SymbolFact {
                        name: name.as_str().to_string(),
                        kind: *kind,
                        line: line_number(index),
                    });
                    break;
                }
            }
        }
    }

    symbols.sort();
    symbols.dedup();
    symbols
}

fn extract_rust_requirement_refs(text: &str) -> Vec<RequirementReference> {
    let mut refs = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("//") {
            continue;
        }
        for captures in req_ref_re().captures_iter(trimmed) {
            let Some(inner) = captures.get(1) else {
                continue;
            };
            let tokens: Vec<&str> = inner.as_str().split_whitespace().collect();
            if tokens.len() < 2 {
                continue;
            }
            refs.push(RequirementReference {
                relation: tokens[0].to_string(),
                id: tokens[1].to_string(),
                line: line_number(index),
                snippet: trimmed.to_string(),
            });
        }
    }
    refs.sort();
    refs.dedup();
    refs
}

fn extract_requirement_refs(text: &str) -> Vec<RequirementReference> {
    let mut refs = Vec::new();
    for (index, line) in text.lines().enumerate() {
        for captures in req_ref_re().captures_iter(line) {
            let Some(inner) = captures.get(1) else {
                continue;
            };
            let tokens: Vec<&str> = inner.as_str().split_whitespace().collect();
            if tokens.len() < 2 {
                continue;
            }
            refs.push(RequirementReference {
                relation: tokens[0].to_string(),
                id: tokens[1].to_string(),
                line: line_number(index),
                snippet: line.trim().to_string(),
            });
        }
    }
    refs.sort();
    refs.dedup();
    refs
}

fn extract_requirement_definitions(text: &str) -> Vec<RequirementDefinition> {
    let lines: Vec<&str> = text.lines().collect();
    let mut defs = Vec::new();
    let mut current_section: Option<String> = None;
    let mut index = 0usize;

    while index < lines.len() {
        let line = lines[index].trim();
        if let Some((level, heading)) = markdown_heading(line) {
            if tracks_requirement_section(level, heading) {
                current_section = Some(heading.to_string());
            }
        }

        if let Some(id) = requirement_definition_id(line) {
            let mut body = Vec::new();
            let mut cursor = index + 1;
            while cursor < lines.len() {
                let next = lines[cursor].trim();
                if next.is_empty() || next.starts_with('#') || requirement_definition_id(next).is_some() {
                    break;
                }
                body.push(next.to_string());
                cursor += 1;
            }
            defs.push(RequirementDefinition {
                id,
                section: current_section.clone(),
                line: line_number(index),
                body,
            });
        }

        index += 1;
    }

    defs
}

fn extract_markdown_headings(text: &str) -> Vec<HeadingFact> {
    let mut headings = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if let Some((level, text)) = markdown_heading(line.trim()) {
            headings.push(HeadingFact {
                level,
                line: line_number(index),
                text: text.to_string(),
            });
        }
    }
    headings
}

fn markdown_heading(line: &str) -> Option<(u8, &str)> {
    let hashes = line.chars().take_while(|ch| *ch == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = line[hashes..].trim();
    if rest.is_empty() {
        return None;
    }
    Some((u8::try_from(hashes).expect("markdown heading level fits in u8"), rest))
}

fn tracks_requirement_section(level: u8, heading: &str) -> bool {
    level == 2 || (level == 3 && (heading.starts_with("Requirement:") || heading.starts_with("Requirement ")))
}

fn requirement_definition_id(line: &str) -> Option<String> {
    let captures = req_line_re().captures(line)?;
    captures.get(1).map(|value| value.as_str().to_string())
}

fn has_extension(path: &str, extension: &str) -> bool {
    Path::new(path).extension().is_some_and(|value| value.eq_ignore_ascii_case(extension))
}

fn line_number(index: usize) -> u32 {
    u32::try_from(index + 1).expect("line number fits in u32")
}

fn req_ref_re() -> &'static Regex {
    static INSTANCE: OnceLock<Regex> = OnceLock::new();
    INSTANCE.get_or_init(|| Regex::new(r"r\[([^\]]+)\]").expect("valid requirement-ref regex"))
}

fn req_line_re() -> &'static Regex {
    static INSTANCE: OnceLock<Regex> = OnceLock::new();
    INSTANCE.get_or_init(|| Regex::new(r"^r\[([^\]\s]+)\]\s*$").expect("valid requirement-line regex"))
}

fn module_re() -> &'static Regex {
    static INSTANCE: OnceLock<Regex> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        Regex::new(r"^(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)").expect("valid module regex")
    })
}

fn struct_re() -> &'static Regex {
    static INSTANCE: OnceLock<Regex> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        Regex::new(r"^(?:pub(?:\([^)]*\))?\s+)?struct\s+([A-Za-z_][A-Za-z0-9_]*)").expect("valid struct regex")
    })
}

fn enum_re() -> &'static Regex {
    static INSTANCE: OnceLock<Regex> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        Regex::new(r"^(?:pub(?:\([^)]*\))?\s+)?enum\s+([A-Za-z_][A-Za-z0-9_]*)").expect("valid enum regex")
    })
}

fn trait_re() -> &'static Regex {
    static INSTANCE: OnceLock<Regex> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        Regex::new(r"^(?:pub(?:\([^)]*\))?\s+)?trait\s+([A-Za-z_][A-Za-z0-9_]*)").expect("valid trait regex")
    })
}

fn type_alias_re() -> &'static Regex {
    static INSTANCE: OnceLock<Regex> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        Regex::new(r"^(?:pub(?:\([^)]*\))?\s+)?type\s+([A-Za-z_][A-Za-z0-9_]*)").expect("valid type alias regex")
    })
}

fn const_re() -> &'static Regex {
    static INSTANCE: OnceLock<Regex> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        Regex::new(r"^(?:pub(?:\([^)]*\))?\s+)?const\s+([A-Za-z_][A-Za-z0-9_]*)").expect("valid const regex")
    })
}

fn static_re() -> &'static Regex {
    static INSTANCE: OnceLock<Regex> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        Regex::new(r"^(?:pub(?:\([^)]*\))?\s+)?static\s+([A-Za-z_][A-Za-z0-9_]*)").expect("valid static regex")
    })
}

fn proof_fn_re() -> &'static Regex {
    static INSTANCE: OnceLock<Regex> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        Regex::new(r"^(?:pub(?:\([^)]*\))?\s+)?proof\s+fn\s+([A-Za-z_][A-Za-z0-9_]*)").expect("valid proof-fn regex")
    })
}

fn spec_fn_re() -> &'static Regex {
    static INSTANCE: OnceLock<Regex> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        Regex::new(r"^(?:pub(?:\([^)]*\))?\s+)?(?:(?:open|closed)\s+)?spec\s+fn\s+([A-Za-z_][A-Za-z0-9_]*)")
            .expect("valid spec-fn regex")
    })
}

fn fn_re() -> &'static Regex {
    static INSTANCE: OnceLock<Regex> = OnceLock::new();
    INSTANCE
        .get_or_init(|| Regex::new(r"^(?:pub(?:\([^)]*\))?\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)").expect("valid fn regex"))
}

#[cfg(test)]
mod tests {
    use super::extract_facts;
    use super::infer_openspec_context;

    #[test]
    fn rust_extractor_finds_verus_items_and_refs() {
        let facts = extract_facts(
            "src/demo.rs",
            br"
// r[impl repo-index.query]
pub mod demo;
pub struct Thing;
pub proof fn lemma() {}
pub open spec fn spec_name() -> bool { true }
pub fn runtime_name() {}
",
        );
        let names: Vec<String> = facts.symbols.into_iter().map(|symbol| symbol.name).collect();
        assert_eq!(names, vec!["Thing", "demo", "lemma", "runtime_name", "spec_name"]);
        assert_eq!(facts.requirement_refs.len(), 1);
        assert_eq!(facts.requirement_refs[0].id, "repo-index.query");
        assert_eq!(facts.requirement_refs[0].relation, "impl");
    }

    #[test]
    fn requirements_doc_extractor_collects_definitions() {
        let facts = extract_facts(
            "docs/requirements.md",
            br"
# Root

## Repository Index

r[repo-index.query]
Queries MUST be deterministic.
They MUST stay local.

r[repo-index.atomic-refresh]
Refresh MUST commit atomically.
",
        );
        assert_eq!(facts.requirement_defs.len(), 2);
        assert_eq!(facts.requirement_defs[0].id, "repo-index.query");
        assert_eq!(facts.requirement_defs[0].section.as_deref(), Some("Repository Index"));
        assert_eq!(facts.requirement_defs[0].body, vec![
            "Queries MUST be deterministic.".to_string(),
            "They MUST stay local.".to_string()
        ]);
    }

    #[test]
    fn openspec_extractor_collects_requirement_definitions() {
        let facts = extract_facts(
            "openspec/changes/demo/specs/web-of-trust/spec.md",
            br"
# Demo

## Requirements

### Requirement: Threshold Policy Counts Distinct Introducers

r[web-of-trust.threshold]
A threshold policy MUST count only distinct introducers.
",
        );
        assert_eq!(facts.requirement_defs.len(), 1);
        assert_eq!(facts.requirement_defs[0].id, "web-of-trust.threshold");
        assert_eq!(
            facts.requirement_defs[0].section.as_deref(),
            Some("Requirement: Threshold Policy Counts Distinct Introducers")
        );
        assert_eq!(facts.requirement_defs[0].body, vec![
            "A threshold policy MUST count only distinct introducers.".to_string()
        ]);
    }

    #[test]
    fn openspec_context_tracks_change_and_domain() {
        let context = infer_openspec_context("openspec/changes/repo-indexing/specs/repo-index/spec.md")
            .expect("openspec context");
        assert_eq!(context.change_name.as_deref(), Some("repo-indexing"));
        assert_eq!(context.domain.as_deref(), Some("repo-index"));
        assert_eq!(context.document_role, "change-spec");
    }
}
