//! Read-only Console Swarm View HTML renderer.
//!
//! Renders a self-contained HTML inspection page for Kiln-governed swarm runs:
//! - Summary (phase, envelope, concurrency, budget draw/ceiling, repair rounds),
//! - Roster (member, role, state, host, harness, model, scope, claim fence,
//!   budget draw, last post),
//! - Replayable Mention Graph (directed weighted edges + inline SVG up to any
//!   `up_to_seq` replay cursor),
//! - Board feed (seq, sender, role, kind, mentions, candidate commit, body).
//!
//! Zero external CDN dependencies. Every untrusted string from swarm runs,
//! members, or board posts is HTML-escaped before interpolation.

use std::fmt::Write as _;

use kiln_core::fleet::{
    BoardPost, MentionGraph, MentionTarget, SwarmMember, SwarmMemberState, SwarmRun,
};

/// Escape HTML special characters so untrusted swarm/board text is strictly data.
#[must_use]
pub fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Same-origin CSS served at `/swarm-view.css` under `style-src 'self'`.
pub const SWARM_VIEW_CSS: &str = ":root{color-scheme:light dark;--fg:#1c1c1c;--muted:#5c5c5c;--bg:#faf9f7;--panel:#fff;--line:#d9d5cf;--accent:#1f6f8b;--warn:#9a5b13;--bad:#a33;--ok:#2c6e3f}\
@media(prefers-color-scheme:dark){:root{--fg:#e8e6e1;--muted:#a4a09a;--bg:#161616;--panel:#1f1f1f;--line:#3a3a3a;--accent:#6fb3c8;--warn:#e2a856;--bad:#e08b8b;--ok:#8fcf9f}}\
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg);font:14px/1.45 ui-sans-serif,system-ui,sans-serif}\
header{display:flex;flex-wrap:wrap;gap:1rem;align-items:baseline;justify-content:space-between;padding:1rem 1.25rem;border-bottom:1px solid var(--line);background:var(--panel)}\
header h1{margin:0;font-size:1.15rem}header a{color:var(--accent);text-decoration:none}\
main{padding:1rem 1.25rem 3rem;display:grid;gap:1rem;max-width:1280px;margin:0 auto}\
section{background:var(--panel);border:1px solid var(--line);border-radius:6px;padding:.9rem 1rem;min-width:0}\
h2{margin:0 0 .5rem;font-size:.9rem;text-transform:uppercase;letter-spacing:.08em;color:var(--muted)}\
.meta{display:grid;grid-template-columns:repeat(auto-fit,minmax(160px,1fr));gap:.6rem;margin:0}\
.meta div{border:1px solid var(--line);border-radius:4px;padding:.5rem .65rem;background:var(--bg)}\
.meta dt{font-size:.72rem;text-transform:uppercase;letter-spacing:.06em;color:var(--muted)}\
.meta dd{margin:.15rem 0 0;font-weight:600;font-family:ui-monospace,monospace;font-size:.86rem;overflow-wrap:anywhere}\
table{width:100%;border-collapse:collapse;font-size:.84rem}\
th,td{text-align:left;padding:.4rem .45rem;border-bottom:1px solid var(--line);vertical-align:top}\
th{color:var(--muted);font-size:.72rem;text-transform:uppercase;letter-spacing:.05em}\
code,.mono{font-family:ui-monospace,monospace;font-size:.78rem;overflow-wrap:anywhere}\
.badge{display:inline-block;border:1px solid var(--line);border-radius:999px;padding:.05rem .45rem;font-size:.72rem;font-family:ui-monospace,monospace}\
.badge-verifier{border-color:var(--accent);color:var(--accent)}\
.badge-author{border-color:var(--ok);color:var(--ok)}\
.badge-coordinator{border-color:var(--warn);color:var(--warn)}\
.replay{display:flex;flex-wrap:wrap;gap:.45rem;align-items:center;margin-bottom:.75rem}\
.replay a{display:inline-block;padding:.2rem .55rem;border:1px solid var(--line);border-radius:4px;background:var(--bg);color:var(--fg);text-decoration:none;font-family:ui-monospace,monospace;font-size:.78rem}\
.replay a.active{border-color:var(--accent);color:var(--accent);font-weight:600}\
.graph-wrap{display:grid;grid-template-columns:minmax(260px,420px) minmax(0,1fr);gap:1rem;align-items:start}\
@media(max-width:52rem){.graph-wrap{grid-template-columns:minmax(0,1fr)}}\
svg.mention-svg{width:100%;height:auto;border:1px solid var(--line);border-radius:4px;background:var(--bg)}\
.post-body{white-space:pre-wrap;overflow-wrap:anywhere;margin:0;font-family:ui-monospace,monospace;font-size:.8rem}\
.empty{color:var(--muted);font-style:italic;margin:0}\
.note{color:var(--muted);font-size:.8rem;margin:.25rem 0 0}";

// r[impl kiln.fleet.swarm.console]
/// Render the read-only list of all swarm runs.
#[must_use]
pub fn render_swarm_list_html(runs: &[SwarmRun]) -> String {
    let mut doc = String::with_capacity(2_048);
    let _ = write!(
        doc,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>Kiln Swarms</title><link rel=\"stylesheet\" href=\"/swarm-view.css\"></head><body>\
         <header><h1>Kiln Swarm Runs</h1><a href=\"/\">Back to Console</a></header><main>\
         <section><h2>Active &amp; Retained Swarms</h2>\
         <p class=\"note\">Read-only projection. Board posts are untrusted data and never mutate Kiln authority.</p>"
    );
    if runs.is_empty() {
        doc.push_str("<p class=\"empty\">No swarm runs recorded.</p>");
    } else {
        doc.push_str(
            "<table><thead><tr><th>Run</th><th>Title</th><th>Phase</th>\
             <th>Members</th><th>Board</th><th>Budget</th><th>Repairs</th></tr></thead><tbody>",
        );
        for run in runs {
            let id = escape_html(run.identity.as_str());
            let short = escape_html(
                run.identity
                    .as_str()
                    .get(..12)
                    .unwrap_or(run.identity.as_str()),
            );
            let title = escape_html(&run.config.title);
            let phase = escape_html(&format!("{:?}", run.phase).to_ascii_lowercase());
            let active = run.active_member_count();
            let total = run.members.len();
            let head = run.board.head_seq();
            let _ = write!(
                doc,
                "<tr><td><a href=\"/fleet/swarms/{id}\"><code>{short}</code></a></td>\
                 <td>{title}</td><td><span class=\"badge\">{phase}</span></td>\
                 <td class=\"mono\">{active}/{total} (cap {})</td>\
                 <td class=\"mono\">seq {head}</td>\
                 <td class=\"mono\">{} / {}</td>\
                 <td class=\"mono\">{} / {}</td></tr>",
                run.config.max_concurrency,
                run.budget.drawn_units,
                run.budget.ceiling_units,
                run.repair_round,
                run.config.max_repair_rounds,
            );
        }
        doc.push_str("</tbody></table>");
    }
    doc.push_str("</section></main></body></html>");
    doc
}

// r[impl kiln.fleet.swarm.console] r[impl kiln.fleet.swarm.console.scenario.escaping_and_replay]
/// Render one swarm run's read-only Board, Roster, and Replayable Mention Graph
/// filtered to `up_to_seq` (defaults to the board head).
#[must_use]
pub fn render_swarm_detail_html(run: &SwarmRun, up_to_seq: Option<u64>) -> String {
    let graph = run.mention_graph(up_to_seq);
    let cutoff = graph.up_to_seq;
    let head = graph.head_seq;
    let id = escape_html(run.identity.as_str());
    let title = escape_html(&run.config.title);
    let phase = escape_html(&format!("{:?}", run.phase).to_ascii_lowercase());
    let envelope = run
        .config
        .envelope
        .allowed_scopes
        .iter()
        .map(|s| escape_html(&s.text()))
        .collect::<Vec<_>>()
        .join(", ");

    let mut doc = String::with_capacity(8_192);
    let _ = write!(
        doc,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>Swarm · {title}</title><link rel=\"stylesheet\" href=\"/swarm-view.css\"></head><body>\
         <header><div><h1>{title}</h1><div class=\"mono\">{id}</div></div>\
         <div><a href=\"/fleet/swarms\">All Swarms</a> · <a href=\"/\">Console</a></div></header><main>"
    );

    // Summary section.
    let _ = write!(
        doc,
        "<section><h2>Swarm Summary</h2><dl class=\"meta\">\
         <div><dt>Phase</dt><dd>{phase}</dd></div>\
         <div><dt>Concurrency</dt><dd>{} / {}</dd></div>\
         <div><dt>Budget Draw</dt><dd>{} / {}</dd></div>\
         <div><dt>Repair Rounds</dt><dd>{} / {}</dd></div>\
         <div><dt>Board Cursor</dt><dd>{cutoff} / {head}</dd></div>\
         <div><dt>Allowed Envelope</dt><dd>{envelope}</dd></div>",
        run.active_member_count(),
        run.config.max_concurrency,
        run.budget.drawn_units,
        run.budget.ceiling_units,
        run.repair_round,
        run.config.max_repair_rounds,
    );
    if let Some(commit) = &run.approved_commit {
        let _ = write!(
            doc,
            "<div><dt>Approved Commit</dt><dd>{}</dd></div>",
            escape_html(commit)
        );
    }
    if let Some(reason) = &run.escalation_reason {
        let _ = write!(
            doc,
            "<div><dt>Escalation Reason</dt><dd>{}</dd></div>",
            escape_html(reason)
        );
    }
    doc.push_str("</dl></section>");

    // Roster section.
    doc.push_str("<section><h2>Roster</h2>");
    if run.members.is_empty() {
        doc.push_str("<p class=\"empty\">No members spawned.</p>");
    } else {
        doc.push_str(
            "<table><thead><tr><th>Member</th><th>Role</th><th>State</th>\
             <th>Host</th><th>Harness</th><th>Model</th><th>Scope</th>\
             <th>Claim Fence</th><th>Budget Draw</th><th>Last Post</th></tr></thead><tbody>",
        );
        for member in run.members.values() {
            render_member_row(&mut doc, member);
        }
        doc.push_str("</tbody></table>");
    }
    doc.push_str("</section>");

    // Replayable Mention Graph section (navigation links comply with form-action 'none' CSP).
    let _ = write!(
        doc,
        "<section><h2>Replayable Mention Graph (seq &le; {cutoff})</h2>\
         <nav class=\"replay\" aria-label=\"Replay sequence cursor\"><span>Replay cursor:</span>"
    );
    let mut steps: Vec<u64> = Vec::new();
    steps.push(0);
    if head <= 32 {
        steps.extend(1..=head);
    } else {
        let stride = (head / 16).max(1);
        let mut cursor = stride;
        while cursor < head {
            steps.push(cursor);
            cursor = cursor.saturating_add(stride);
        }
        if cutoff > 0 {
            steps.push(cutoff.saturating_sub(1));
        }
        steps.push(cutoff);
        if cutoff < head {
            steps.push(cutoff.saturating_add(1));
        }
        steps.push(head);
        steps.sort_unstable();
        steps.dedup();
    }
    for seq in steps {
        let active = if seq == cutoff {
            " class=\"active\""
        } else {
            ""
        };
        let _ = write!(
            doc,
            "<a href=\"/fleet/swarms/{id}?up_to_seq={seq}\"{active}>#{seq}</a>"
        );
    }
    doc.push_str("</nav><div class=\"graph-wrap\">");
    render_mention_svg(&mut doc, &graph);
    render_edge_table(&mut doc, &graph);
    doc.push_str("</div></section>");

    // Board feed section.
    let visible_posts: Vec<&BoardPost> = run
        .board
        .posts
        .iter()
        .filter(|post| post.seq <= cutoff)
        .collect();
    let _ = write!(
        doc,
        "<section><h2>Board Feed (seq &le; {cutoff})</h2>\
         <p class=\"note\">Untrusted data: board post text never mutates claims, scopes, roles, or publication state.</p>"
    );
    if visible_posts.is_empty() {
        doc.push_str("<p class=\"empty\">No board posts at or before this cursor.</p>");
    } else {
        doc.push_str(
            "<table><thead><tr><th>Seq</th><th>Sender</th><th>Role</th><th>Kind</th>\
             <th>Mentions</th><th>Body</th></tr></thead><tbody>",
        );
        for post in visible_posts {
            render_post_row(&mut doc, post);
        }
        doc.push_str("</tbody></table>");
    }
    doc.push_str("</section></main></body></html>");
    doc
}

fn render_member_row(doc: &mut String, member: &SwarmMember) {
    let name = escape_html(&member.name);
    let role_str = member.role.as_str();
    let state_desc = match &member.state {
        SwarmMemberState::Working => "working".to_string(),
        SwarmMemberState::Waiting { since_seq } => format!("waiting (since #{since_seq})"),
        SwarmMemberState::PausedQuota {
            checkpoint_commit,
            board_cursor,
            reason,
            ..
        } => {
            let ckpt = checkpoint_commit
                .as_deref()
                .and_then(|c| c.get(..8))
                .unwrap_or("none");
            format!("paused_quota (cursor #{board_cursor}, ckpt {ckpt}: {reason})")
        }
        SwarmMemberState::Retired => "retired".to_string(),
        SwarmMemberState::Terminal => "terminal".to_string(),
    };
    let scope_text = member
        .scope
        .as_ref()
        .map_or_else(|| "read-only".to_string(), |s| s.text());
    let fence_text = member
        .claim_fence
        .map_or_else(|| "–".to_string(), |f| f.to_string());
    let last_post = member
        .last_post_seq
        .map_or_else(|| "–".to_string(), |s| format!("#{s}"));
    let _ = write!(
        doc,
        "<tr><td><code>{name}</code></td>\
         <td><span class=\"badge badge-{role_str}\">{role_str}</span></td>\
         <td>{}</td><td><code>{}</code></td><td><code>{}</code></td><td><code>{}</code></td>\
         <td><code>{}</code></td><td class=\"mono\">{}</td><td class=\"mono\">{}</td>\
         <td class=\"mono\">{}</td></tr>",
        escape_html(&state_desc),
        escape_html(&member.host),
        escape_html(&member.harness),
        escape_html(&member.model),
        escape_html(&scope_text),
        escape_html(&fence_text),
        member.budget_draw,
        escape_html(&last_post),
    );
}

fn render_post_row(doc: &mut String, post: &BoardPost) {
    let mentions = if post.mentions.is_empty() {
        "–".to_string()
    } else {
        post.mentions
            .iter()
            .map(format_mention)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let role_str = post.sender_role.as_str();
    let kind_str = post.kind.as_str();
    let mut extra = String::new();
    if let Some(reply) = post.in_reply_to {
        let _ = write!(extra, "\n[reply to #{reply}]");
    }
    if let Some(commit) = &post.candidate_commit {
        let _ = write!(extra, "\n[commit {commit}]");
    }
    let _ = write!(
        doc,
        "<tr><td class=\"mono\">#{}</td><td><code>{}</code></td>\
         <td><span class=\"badge badge-{role_str}\">{role_str}</span></td>\
         <td><span class=\"badge\">{kind_str}</span></td>\
         <td class=\"mono\">{}</td>\
         <td><pre class=\"post-body\">{}{}</pre></td></tr>",
        post.seq,
        escape_html(&post.sender),
        escape_html(&mentions),
        escape_html(&post.body),
        escape_html(&extra),
    );
}

fn format_mention(target: &MentionTarget) -> String {
    match target {
        MentionTarget::Member(name) => format!("@{name}"),
        MentionTarget::Role(role) => format!("@role:{}", role.as_str()),
        MentionTarget::All => "@all".to_string(),
    }
}

fn render_mention_svg(doc: &mut String, graph: &MentionGraph) {
    let count = graph.nodes.len();
    let _ = write!(
        doc,
        "<svg class=\"mention-svg\" viewBox=\"0 0 360 260\" role=\"img\" aria-label=\"Replayable mention graph\">\
         <defs><marker id=\"arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"6\" markerHeight=\"6\" orient=\"auto-start-reverse\">\
         <path d=\"M 0 1 L 10 5 L 0 9 z\" fill=\"currentColor\"/></marker></defs>"
    );
    if count == 0 {
        doc.push_str("<text x=\"180\" y=\"130\" text-anchor=\"middle\" fill=\"currentColor\" font-size=\"12\">No roster nodes</text></svg>");
        return;
    }
    let mut positions: Vec<(&str, f64, f64)> = Vec::with_capacity(count);
    let cx = 180.0_f64;
    let cy = 130.0_f64;
    let radius = if count == 1 { 0.0_f64 } else { 88.0_f64 };
    for (index, node) in graph.nodes.iter().enumerate() {
        let angle = (2.0 * std::f64::consts::PI * (index as f64) / (count as f64))
            - std::f64::consts::FRAC_PI_2;
        let x = cx + radius * angle.cos();
        let y = cy + radius * angle.sin();
        positions.push((node.member.as_str(), x, y));
    }

    for edge in &graph.edges {
        let from_pos = positions.iter().find(|(n, _, _)| *n == edge.from.as_str());
        let to_pos = positions.iter().find(|(n, _, _)| *n == edge.to.as_str());
        if let (Some((_, x1, y1)), Some((_, x2, y2))) = (from_pos, to_pos) {
            let dx = x2 - x1;
            let dy = y2 - y1;
            let dist = dx.hypot(dy).max(1.0);
            let pad = 22.0;
            let sx = x1 + dx * (pad / dist);
            let sy = y1 + dy * (pad / dist);
            let ex = x2 - dx * (pad / dist);
            let ey = y2 - dy * (pad / dist);
            let mx = (sx + ex) / 2.0;
            let my = (sy + ey) / 2.0 - 5.0;
            let stroke = (1.2 + (edge.weight.min(6) as f64) * 0.5).min(4.2);
            let _ = write!(
                doc,
                "<line x1=\"{sx:.1}\" y1=\"{sy:.1}\" x2=\"{ex:.1}\" y2=\"{ey:.1}\" \
                 stroke=\"currentColor\" stroke-opacity=\"0.65\" stroke-width=\"{stroke:.1}\" marker-end=\"url(#arrow)\">\
                 <title>{} -&gt; {} (weight {})</title></line>\
                 <text x=\"{mx:.1}\" y=\"{my:.1}\" text-anchor=\"middle\" fill=\"currentColor\" font-size=\"10\">{}</text>",
                escape_html(&edge.from),
                escape_html(&edge.to),
                edge.weight,
                edge.weight,
            );
        }
    }

    for ((name, x, y), node) in positions.iter().zip(graph.nodes.iter()) {
        let label = escape_html(name);
        let role = node.role.as_str();
        let text_y = y + 3.0;
        let _ = write!(
            doc,
            "<g><circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"18\" fill=\"var(--panel)\" stroke=\"currentColor\" stroke-width=\"1.5\"/>\
             <title>{label} ({role}, posts={}, in={}, out={})</title>\
             <text x=\"{x:.1}\" y=\"{text_y:.1}\" text-anchor=\"middle\" fill=\"currentColor\" font-size=\"10\">{label}</text></g>",
            node.post_count, node.mentions_in, node.mentions_out,
        );
    }
    doc.push_str("</svg>");
}

fn render_edge_table(doc: &mut String, graph: &MentionGraph) {
    if graph.edges.is_empty() {
        doc.push_str("<p class=\"empty\">No mention edges at or before this cursor.</p>");
        return;
    }
    doc.push_str(
        "<table><thead><tr><th>From</th><th>To</th><th>Weight</th><th>Last Seq</th><th>Kinds</th></tr></thead><tbody>",
    );
    for edge in &graph.edges {
        let kinds = edge
            .kinds
            .iter()
            .map(|k| k.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let _ = write!(
            doc,
            "<tr><td><code>{}</code></td><td><code>{}</code></td>\
             <td class=\"mono\">{}</td><td class=\"mono\">#{}</td>\
             <td class=\"mono\">{}</td></tr>",
            escape_html(&edge.from),
            escape_html(&edge.to),
            edge.weight,
            edge.last_seq,
            escape_html(&kinds),
        );
    }
    doc.push_str("</tbody></table>");
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_core::fleet::{
        BoardPostKind, FleetClaimRegistry, MentionTarget, SpawnMemberRequest, SwarmEnvelope,
        SwarmLedger, SwarmMemberState, SwarmRole, SwarmRunConfig, admit_spawn, create_swarm_run,
        parse_scope_text, post_to_board, update_member_state,
    };

    fn sample_run_with_xss() -> SwarmRun {
        let config = SwarmRunConfig {
            title: "Swarm <script>alert('title')</script>".to_string(),
            envelope: SwarmEnvelope {
                allowed_scopes: vec![
                    parse_scope_text("OnixResearch/kiln crates/kiln-core")
                        .unwrap_or_else(|e| unreachable!("{e:?}")),
                ],
            },
            max_concurrency: 4,
            max_repair_rounds: 2,
            budget_ceiling_units: 500,
            stale_after_ms: 60_000,
        };
        let (_, run) = create_swarm_run(&SwarmLedger::default(), config, 1_000)
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        let claims = FleetClaimRegistry::default();
        let (run, _, _) = admit_spawn(
            &run,
            None,
            SpawnMemberRequest {
                name: "author-1".to_string(),
                role: SwarmRole::Author,
                host: "aspen3".to_string(),
                harness: "omp".to_string(),
                model: "gpt-5".to_string(),
                scope: Some(
                    parse_scope_text("OnixResearch/kiln crates/kiln-core/src/fleet")
                        .unwrap_or_else(|e| unreachable!("{e:?}")),
                ),
                brief: "Own <img src=x onerror=alert(1)>".to_string(),
                attempt: None,
                claim_identity: None,
                claim_fence: None,
            },
            &claims,
            1_100,
        )
        .unwrap_or_else(|e| unreachable!("{e:?}"));
        let (run, _, _) = admit_spawn(
            &run,
            None,
            SpawnMemberRequest {
                name: "verifier".to_string(),
                role: SwarmRole::Verifier,
                host: "aspen2".to_string(),
                harness: "claude-code".to_string(),
                model: "claude-opus-4-6".to_string(),
                scope: None,
                brief: "Verify read-only".to_string(),
                attempt: None,
                claim_identity: None,
                claim_fence: None,
            },
            &claims,
            1_200,
        )
        .unwrap_or_else(|e| unreachable!("{e:?}"));
        let (run, _) = post_to_board(
            &run,
            "author-1",
            BoardPostKind::Handoff,
            vec![MentionTarget::Member("verifier".to_string())],
            "Review <script>evil()</script> & \"quotes\"".to_string(),
            None,
            None,
            1_300,
        )
        .unwrap_or_else(|e| unreachable!("{e:?}"));
        let (run, _) = update_member_state(
            &run,
            "author-1",
            SwarmMemberState::PausedQuota {
                checkpoint_commit: None,
                board_cursor: 3,
                reason: "quota <svg onload=alert(2)>".to_string(),
                paused_ms: 1_400,
            },
            15,
            1_400,
        )
        .unwrap_or_else(|e| unreachable!("{e:?}"));
        run
    }

    #[test]
    // r[verify kiln.fleet.swarm.console]
    // r[verify kiln.fleet.swarm.console.scenario.escaping_and_replay]
    fn html_rendering_escapes_xss_has_zero_cdn_and_supports_replay_cursor() {
        let run = sample_run_with_xss();

        let list_html = render_swarm_list_html(std::slice::from_ref(&run));
        assert!(!list_html.contains("<script>"));
        assert!(list_html.contains("&lt;script&gt;alert(&#39;title&#39;)&lt;/script&gt;"));
        assert!(!list_html.contains("http://"));
        assert!(!list_html.contains("https://"));

        // Full detail view at head (seq 3).
        let full_html = render_swarm_detail_html(&run, None);
        assert!(!full_html.contains("<script>"));
        assert!(!full_html.contains("<img "));
        assert!(!full_html.contains("<svg onload="));
        assert!(full_html.contains("&lt;script&gt;evil()&lt;/script&gt; &amp; &quot;quotes&quot;"));
        assert!(full_html.contains("&lt;img src=x onerror=alert(1)&gt;"));
        assert!(full_html.contains("&lt;svg onload=alert(2)&gt;"));
        assert!(full_html.contains("author-1"));
        assert!(full_html.contains("verifier"));
        assert!(!full_html.contains("http://"));
        assert!(!full_html.contains("https://"));
        // At seq 3, author-1 -> verifier edge exists, and SVG node title + text y-coord are accurate.
        assert!(full_html.contains("author-1 -&gt; verifier (weight 1)"));
        assert!(full_html.contains("<title>author-1 (author, posts=1, in=1, out=1)</title>"));
        assert!(full_html.contains("<title>verifier (verifier, posts=0, in=2, out=0)</title>"));
        // Two nodes sit on a circle of radius 88 around y=130 -> y=42.0 (text y=45.0) and y=218.0 (text y=221.0).
        assert!(full_html.contains("y=\"45.0\""));
        assert!(full_html.contains("y=\"221.0\""));

        // Replay at seq 2 excludes post #3 and its author-1 -> verifier edge!
        let replayed = render_swarm_detail_html(&run, Some(2));
        assert!(!replayed.contains("author-1 -&gt; verifier"));
        assert!(!replayed.contains("evil()"));
        assert!(replayed.contains("up_to_seq=2\" class=\"active\""));
    }
}
