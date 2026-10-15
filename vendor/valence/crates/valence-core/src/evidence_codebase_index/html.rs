use std::fmt::Write as _;

use super::model::EvidenceCodebaseIndex;

#[must_use]
pub fn render_evidence_index_html(index: &EvidenceCodebaseIndex) -> String {
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<title>Octet evidence codebase index</title>\n");
    html.push_str("<style>body{font-family:sans-serif;max-width:72rem;margin:2rem auto;line-height:1.4}code{background:#f2f2f2;padding:0.1rem 0.25rem}.issue{color:#8a4b00}.record{border-top:1px solid #ddd;padding-top:1rem;margin-top:1rem}</style>\n");
    html.push_str("</head>\n<body>\n");
    html.push_str("<h1>Octet evidence codebase index</h1>\n");
    let _ = writeln!(&mut html, "<p><strong>Index hash:</strong> <code>{}</code></p>", escape_html(&index.index_hash));
    let _ = writeln!(&mut html, "<p>{}</p>", escape_html(&index.boundary));
    let _ = writeln!(
        &mut html,
        "<p>Records: {}. Source artifacts: {}. Issues: {}.</p>",
        index.record_count,
        index.source_artifact_count,
        index.issues.len()
    );

    html.push_str("<h2>Records</h2>\n<ul>\n");
    for record in &index.records {
        let _ = writeln!(
            &mut html,
            "<li><a href=\"#{}\"><code>{}</code></a> {} <small>{}</small></li>",
            html_id(&record.record_id),
            escape_html(&record.stable_id),
            escape_html(&record.artifact_family),
            escape_html(&record.summary)
        );
    }
    html.push_str("</ul>\n");

    for record in &index.records {
        let _ = writeln!(&mut html, "<section class=\"record\" id=\"{}\">", html_id(&record.record_id));
        let _ = writeln!(&mut html, "<h3>{}</h3>", escape_html(&record.stable_id));
        let _ = writeln!(
            &mut html,
            "<p><strong>Family:</strong> {}<br><strong>Tier:</strong> {}<br><strong>Artifact:</strong> <code>{}</code><br><strong>BLAKE3:</strong> <code>{}</code></p>",
            escape_html(&record.artifact_family),
            escape_html(&record.evidence_tier),
            escape_html(&record.artifact_locator),
            escape_html(&record.artifact_blake3)
        );
        render_list(&mut html, "Requirement IDs", &record.requirement_ids);
        render_list(&mut html, "Lint IDs", &record.lint_ids);
        render_list(&mut html, "Policy IDs", &record.policy_ids);
        render_list(&mut html, "Fingerprints", &record.fingerprints);
        render_list(&mut html, "Provenance source IDs", &record.provenance_source_ids);
        render_list(&mut html, "Replay commands", &record.replay_commands);
        render_list(&mut html, "Parent links", &record.parent_links);
        render_list(&mut html, "Caveats", &record.caveats);
        html.push_str("</section>\n");
    }

    if !index.issues.is_empty() {
        html.push_str("<h2>Issues and caveats</h2>\n<ul>\n");
        for issue in &index.issues {
            let _ = writeln!(
                &mut html,
                "<li class=\"issue\"><code>{:?}</code> <code>{}</code> {}: {}</li>",
                issue.severity,
                escape_html(&issue.code),
                escape_html(&issue.artifact_locator),
                escape_html(&issue.detail)
            );
        }
        html.push_str("</ul>\n");
    }

    html.push_str("</body>\n</html>\n");
    html
}
fn render_list(html: &mut String, heading: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    let _ = writeln!(html, "<p><strong>{}</strong></p><ul>", escape_html(heading));
    for value in values {
        let _ = writeln!(html, "<li><code>{}</code></li>", escape_html(value));
    }
    html.push_str("</ul>\n");
}

fn html_id(value: &str) -> String {
    let mut id = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            id.push(ch.to_ascii_lowercase());
        } else if !id.ends_with('-') {
            id.push('-');
        }
    }
    id.trim_matches('-').to_owned()
}

fn escape_html(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
