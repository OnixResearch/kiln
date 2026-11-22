pub fn build_spec_list(sources: Vec<super::SpecSource>) -> super::SpecList {
    // r[impl cairn.supersede.read_parity.spec_list]
    let mut specs = sources
        .into_iter()
        .map(|source| super::SpecSummary {
            id: source.id,
            path: source.path,
            title: spec_title(&source.markdown),
        })
        .collect::<Vec<_>>();
    specs.sort_by(|left, right| left.id.cmp(&right.id));
    super::SpecList { specs }
}

pub fn build_spec_detail(source: super::SpecSource) -> super::SpecDetail {
    // r[impl cairn.supersede.read_parity.spec_show]
    super::SpecDetail {
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
