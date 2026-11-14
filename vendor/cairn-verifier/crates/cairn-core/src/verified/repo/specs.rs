use super::{SpecDetail, SpecList, SpecSource, SpecSummary};

pub fn build_spec_list(sources: Vec<SpecSource>) -> SpecList {
    // r[impl cairn.supersede.read_parity.spec_list]
    let mut specs = sources
        .into_iter()
        .map(|source| SpecSummary {
            id: source.id,
            path: source.path,
            title: spec_title(&source.markdown),
        })
        .collect::<Vec<_>>();
    specs.sort_by(|left, right| left.id.cmp(&right.id));
    SpecList { specs }
}

pub fn build_spec_detail(source: SpecSource) -> SpecDetail {
    // r[impl cairn.supersede.read_parity.spec_show]
    SpecDetail {
        id: source.id,
        path: source.path,
        title: spec_title(&source.markdown),
        markdown: source.markdown,
    }
}

pub fn spec_title(markdown: &str) -> Option<String> {
    markdown.lines().find_map(markdown_heading)
}

fn markdown_heading(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let title = trimmed.strip_prefix("# ")?.trim();
    (!title.is_empty()).then(|| title.to_string())
}
