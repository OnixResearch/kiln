mod extract;
mod model;
mod store;

use std::env;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
pub use model::DEFAULT_DB_PATH;
use model::DigestQueryResult;
use model::ExtractorKind;
use model::PathQueryResult;
use model::RefreshSummary;
use model::RequirementQueryResult;
pub use store::RefreshOptions;
pub use store::RepoIndex;

pub fn run<I>(args: I) -> Result<()>
where I: IntoIterator<Item = String> {
    let mut args = args.into_iter();
    let _program = args.next();
    let Some(command) = args.next() else {
        bail!(usage());
    };

    let cwd = env::current_dir().context("read current directory for repo-index")?;
    let index = RepoIndex::open(&cwd, &default_db_path(&cwd))?;

    match command.as_str() {
        "build" => {
            if args.next().is_some() {
                bail!(usage());
            }
            let summary = index.refresh()?;
            print_refresh_summary(&summary);
        }
        "stats" => {
            if args.next().is_some() {
                bail!(usage());
            }
            let stats = index.stats()?;
            println!("generation: {}", index.active_generation()?.unwrap_or(0));
            println!("tracked_paths: {}", stats.tracked_paths);
            println!("unique_digests: {}", stats.unique_digests);
            println!("symbols: {}", stats.symbol_count);
            println!("requirement_references: {}", stats.requirement_reference_count);
            println!("requirement_definitions: {}", stats.requirement_definition_count);
            println!("headings: {}", stats.heading_count);
        }
        "path" => {
            let path = args.next().ok_or_else(|| anyhow!("missing <path> argument\n\n{}", usage()))?;
            if args.next().is_some() {
                bail!(usage());
            }
            let Some(result) = index.query_path(&path)? else {
                bail!("path not indexed: {}", normalize_user_path(&path));
            };
            print_path_result(&result);
        }
        "symbol" => {
            let symbol = args.next().ok_or_else(|| anyhow!("missing <name> argument\n\n{}", usage()))?;
            if args.next().is_some() {
                bail!(usage());
            }
            let hits = index.query_symbol(&symbol)?;
            println!("symbol: {symbol}");
            println!("hits: {}", hits.len());
            for hit in hits {
                println!("- {}:{} [{}] digest={} ", hit.path, hit.symbol.line, hit.symbol.kind, hit.digest_hex);
            }
        }
        "requirement" => {
            let requirement = args.next().ok_or_else(|| anyhow!("missing <id> argument\n\n{}", usage()))?;
            if args.next().is_some() {
                bail!(usage());
            }
            let result = index.query_requirement(&requirement)?;
            print_requirement_result(&requirement, &result);
        }
        "digest" => {
            let digest = args.next().ok_or_else(|| anyhow!("missing <hex> argument\n\n{}", usage()))?;
            if args.next().is_some() {
                bail!(usage());
            }
            let result = index.query_digest(&digest)?;
            print_digest_result(&result);
        }
        _ => bail!(usage()),
    }

    Ok(())
}

fn usage() -> &'static str {
    "usage: repo-index <build|stats|path <path>|symbol <name>|requirement <id>|digest <hex>>"
}

fn default_db_path(root: &std::path::Path) -> PathBuf {
    root.join(DEFAULT_DB_PATH)
}

fn normalize_user_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn print_refresh_summary(summary: &RefreshSummary) {
    println!("generation: {}", summary.generation);
    println!("tracked_paths: {}", summary.tracked_paths);
    println!("inserted_extractions: {}", summary.inserted_extractions);
    println!("reused_extractions: {}", summary.reused_extractions);
    println!("added_paths: {}", summary.added_paths);
    println!("changed_paths: {}", summary.changed_paths);
    println!("unchanged_paths: {}", summary.unchanged_paths);
    println!("deleted_paths: {}", summary.deleted_paths);
}

fn print_path_result(result: &PathQueryResult) {
    println!("path: {}", result.row.path);
    println!("digest: {}", result.row.digest_hex);
    println!("extractor: {}", result.row.extractor_kind);
    println!("size_bytes: {}", result.row.size_bytes);
    if let Some(openspec) = &result.row.openspec {
        println!(
            "openspec: role={} change={} domain={}",
            openspec.document_role,
            openspec.change_name.as_deref().unwrap_or("-"),
            openspec.domain.as_deref().unwrap_or("-"),
        );
    }
    print_symbols(&result.facts.symbols);
    print_requirement_refs(&result.facts.requirement_refs);
    print_headings(&result.facts.headings);
    print_requirement_definitions(&result.facts.requirement_defs);
}

fn print_symbols(symbols: &[model::SymbolFact]) {
    println!("symbols: {}", symbols.len());
    for symbol in symbols {
        println!("- {}:{} [{}]", symbol.name, symbol.line, symbol.kind);
    }
}

fn print_requirement_refs(refs: &[model::RequirementReference]) {
    println!("requirement_refs: {}", refs.len());
    for reference in refs {
        println!("- {}:{} [{}] {}", reference.id, reference.line, reference.relation, reference.snippet);
    }
}

fn print_headings(headings: &[model::HeadingFact]) {
    println!("headings: {}", headings.len());
    for heading in headings {
        println!("- h{}:{} {}", heading.level, heading.line, heading.text);
    }
}

fn print_requirement_definitions(definitions: &[model::RequirementDefinition]) {
    println!("requirement_definitions: {}", definitions.len());
    for definition in definitions {
        let body = if definition.body.is_empty() {
            "-".to_string()
        } else {
            definition.body.join(" ")
        };
        println!(
            "- {}:{} section={} {}",
            definition.id,
            definition.line,
            definition.section.as_deref().unwrap_or("-"),
            body
        );
    }
}

fn print_requirement_result(requirement: &str, result: &RequirementQueryResult) {
    println!("requirement: {requirement}");
    println!("definitions: {}", result.definitions.len());
    for definition in &result.definitions {
        let body = if definition.definition.body.is_empty() {
            "-".to_string()
        } else {
            definition.definition.body.join(" ")
        };
        println!(
            "- {}:{} digest={} section={} {}",
            definition.path,
            definition.definition.line,
            definition.digest_hex,
            definition.definition.section.as_deref().unwrap_or("-"),
            body
        );
    }
    println!("references: {}", result.references.len());
    for reference in &result.references {
        println!(
            "- {}:{} digest={} [{}] {}",
            reference.path,
            reference.reference.line,
            reference.digest_hex,
            reference.reference.relation,
            reference.reference.snippet
        );
    }
}

fn print_digest_result(result: &DigestQueryResult) {
    println!("digest: {}", result.digest_hex);
    println!("paths: {}", result.paths.len());
    for path in &result.paths {
        println!("- {} [{}]", path.path, path.extractor_kind);
    }
    println!("facts_by_kind: {}", result.facts_by_kind.len());
    for facts in &result.facts_by_kind {
        println!("- extractor: {}", facts.extractor_kind);
        match facts.extractor_kind {
            ExtractorKind::Rust => print_symbols(&facts.facts.symbols),
            _ => {
                print_requirement_definitions(&facts.facts.requirement_defs);
                print_requirement_refs(&facts.facts.requirement_refs);
                print_headings(&facts.facts.headings);
            }
        }
    }
}
