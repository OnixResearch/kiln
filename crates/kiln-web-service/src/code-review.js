/* Interactive change review. Authority and observations come exclusively from the service. */
// r[impl kiln.review.surface]
// r[impl kiln.review.parity.ergonomics]
// r[impl kiln.review.parity.delivery]
// r[impl kiln.review.parity.queue]
(() => {
  "use strict";
  const ROOT = "/api/code-review";
  const state = { container: null, context: null, route: {}, epoch: 0, request: 0, diffRequest: 0, snapshot: null, detail: null, diff: null, error: "", diffError: "", pending: false, pinnedRevision: "", forms: new Map(), dialogs: new Set(), mode: "unified" };
  const node = (tag, text, cls) => { const n = document.createElement(tag); if (text !== undefined && text !== null) n.textContent = String(text); if (cls) n.className = cls; return n; };
  const arr = value => Array.isArray(value) ? value : [];
  const label = value => String(value ?? "").replaceAll("_", " ");
  const message = error => error?.message || String(error);
  const path = id => `${ROOT}/changes/${encodeURIComponent(id)}`;
  const selected = () => state.route.review_change || "";
  const current = () => arr(state.detail?.change?.revisions).at(-1);
  // r[impl kiln.review.interdiff.route] `review_interdiff=<from>..<to>` names exact Kiln revision ids; `to` is the selected revision.
  const interdiffRoute = () => { const m = /^([0-9a-f]{64})\.\.([0-9a-f]{64})$/.exec(state.route.review_interdiff || ""); return m ? { from: m[1], to: m[2] } : null; };
  const commitsRoute = () => { const m = /^([0-9a-f]{40})\.\.([0-9a-f]{40})$/.exec(state.route.review_commits || ""); return m ? { from: m[1], to: m[2] } : null; };
  const revision = () => interdiffRoute()?.to || state.route.review_revision || state.pinnedRevision || current()?.id || "";
  // The comparison keys of the displayed view, so line permalinks and toggles keep interdiff/commit-range views.
  const viewPatch = () => ({ review_revision: interdiffRoute() ? "" : revision(), review_base: interdiffRoute() || commitsRoute() ? "" : state.route.review_base || "base", review_interdiff: state.route.review_interdiff || "", review_interdiff_change: state.route.review_interdiff_change || "", review_commits: state.route.review_commits || "" });
  const active = (epoch, id) => epoch === state.epoch && (!id || id === selected()) && !!state.container?.isConnected;
  const writable = () => state.snapshot?.can_write === true && !state.pending;
  // r[impl kiln.review.advisory] Advisory approve/withdraw are the only commands a read principal may send.
  const sendable = readAdmitted => readAdmitted ? state.snapshot?.configured === true && !state.pending : writable();
  const short = value => String(value || "").length > 20 ? `${String(value).slice(0, 12)}…${String(value).slice(-6)}` : String(value || "");
  // r[impl kiln.review.keyboard] r[impl kiln.review.keyboard.navigation] r[impl kiln.review.keyboard.actions]
  // One table drives both the key handler and the help overlay, so documented commands cannot drift from bound ones.
  const SHORTCUTS = Object.freeze([
    Object.freeze({ id: "next-file", keys: ["j", "ArrowDown"], shift: false, description: "Next changed file" }),
    Object.freeze({ id: "previous-file", keys: ["k", "ArrowUp"], shift: false, description: "Previous changed file" }),
    Object.freeze({ id: "next-hunk", keys: ["n"], shift: false, description: "Next hunk" }),
    Object.freeze({ id: "previous-hunk", keys: ["p"], shift: false, description: "Previous hunk" }),
    Object.freeze({ id: "next-thread", keys: ["t"], shift: false, description: "Next unresolved thread" }),
    Object.freeze({ id: "previous-thread", keys: ["T"], shift: true, description: "Previous unresolved thread" }),
    Object.freeze({ id: "next-unviewed", keys: ["u"], shift: false, description: "Next unviewed hunk" }),
    Object.freeze({ id: "previous-unviewed", keys: ["U"], shift: true, description: "Previous unviewed hunk" }),
    Object.freeze({ id: "toggle-layout", keys: ["m"], shift: false, description: "Toggle unified / side-by-side layout" }),
    Object.freeze({ id: "toggle-hunk-viewed", keys: ["v"], shift: false, description: "Toggle viewed for the focused hunk" }),
    Object.freeze({ id: "toggle-file-viewed", keys: ["V"], shift: true, description: "Toggle viewed for the displayed file" }),
    Object.freeze({ id: "start-draft", keys: ["d"], shift: false, description: "Start an anchored draft at the selected line" }),
    Object.freeze({ id: "publish", keys: ["P"], shift: true, description: "Open the publish confirmation" }),
    Object.freeze({ id: "requirements", keys: ["s"], shift: false, description: "Jump to submit requirements" }),
    Object.freeze({ id: "help", keys: ["?"], shift: true, description: "Show these shortcuts" }),
  ]);
  const SHORTCUT_COMMANDS = Object.freeze(SHORTCUTS.map(s => Object.freeze({ id: s.id, keys: s.keys.join(" / "), description: s.description })));
  // r[impl kiln.review.keyboard] Keys are inert while typing, in a dialog, during IME composition and with a command modifier held.
  function resolveShortcut(event, { editable = false, dialogOpen = false } = {}) {
    if (!event || typeof event.key !== "string" || !event.key) return null;
    if (editable || dialogOpen || event.isComposing || event.ctrlKey || event.metaKey || event.altKey) return null;
    const shifted = event.shiftKey === true;
    for (const shortcut of SHORTCUTS) {
      for (const key of shortcut.keys) {
        if (key === "?") { if (event.key === "?" || (event.key === "/" && shifted)) return shortcut.id; continue; }
        if (event.key.toLowerCase() !== key.toLowerCase()) continue;
        // Letters bind case-sensitively in their documented shift state; named keys ignore shift.
        if (/^[a-z]$/i.test(key) && shifted !== shortcut.shift) continue;
        return shortcut.id;
      }
    }
    return null;
  }
  const editableTarget = target => target instanceof Element && Boolean(target.closest("input, textarea, select, [contenteditable]:not([contenteditable='false']), [role='textbox']"));
  const RENDER_LIMIT = 65536, TOKEN_LIMIT = 4096;
  const ALL_CHANGES = { review_change: "", review_revision: "", review_base: "", review_interdiff: "", review_interdiff_change: "", review_commits: "", review_file: "", review_side: "", review_line: "" };
  // r[impl kiln.web.readable_observations]
  // Presentation only: KilnUi (loaded before this script) humanizes times; exact instants stay in title/datetime.
  const ui = () => window.KilnUi;
  const icon = name => ui().icon(name);
  const toneIcon = tone => ui().statusIcon(tone);
  const human = value => { const text = label(value).trim(); return text ? text[0].toUpperCase() + text.slice(1) : "Not recorded"; };
  function when(ms) {
    const instant = ms == null || ms === "" ? NaN : Number(ms);
    if (!Number.isFinite(instant) || Number.isNaN(new Date(instant).getTime())) return node("span", "Not recorded", "not-recorded");
    const kiln = ui(); if (kiln) return kiln.timeElement(instant);
    const time = node("time", new Date(instant).toLocaleString()); time.dateTime = new Date(instant).toISOString(); return time;
  }
  // r[impl kiln.web.visual_design]
  // Review states map onto the shared semantic status pairs; the exact canonical value stays in the accessible name.
  const TONE = { open: "running", submitted: "success", applied: "success", delivered: "success", accepted: "success", approve: "success", abandoned: "cancelled", cancelled: "cancelled", not_applied: "cancelled", abstain: "unknown", queued: "queued", pending: "queued", reserved: "queued", uncertain: "warning", needs_revision: "warning", blocked: "failure", failed: "failure", rejected: "failure", veto: "failure" };
  function pill(value, kind = "State", text = human(value), tone = TONE[String(value)] || "unknown") {
    const span = node("span", null, "status-pill"); span.dataset.status = tone; span.setAttribute("role", "img");
    span.title = `${kind}: ${text} (${value == null || value === "" ? "not recorded" : String(value)})`; span.setAttribute("aria-label", span.title);
    span.append(toneIcon(tone), node("span", text, "status-pill-label")); return span;
  }
  const chip = (text, cls = "") => node("span", text, cls ? `chip ${cls}` : "chip");
  function iconOnly(b, glyph) { const text = b.textContent; b.setAttribute("aria-label", text); if (!b.title) b.title = text; b.classList.add("icon-button"); b.replaceChildren(glyph); return b; }
  // Hard blockers cannot be cleared by review activity alone; other unmet items are outstanding work.
  const BLOCKING_REQUIREMENTS = new Set(["policy", "open", "vetoes", "ownership_files"]);
  function requirements(items) {
    const list = node("ul", null, "cr-checklist"); list.setAttribute("aria-label", "Submit requirements");
    for (const item of arr(items)) {
      const [kind, text, tone] = item.satisfied ? ["met", "Met", "success"] : BLOCKING_REQUIREMENTS.has(item.key) ? ["blocked", "Blocked", "failure"] : ["unmet", "Unmet", "warning"];
      const li = node("li", null, "cr-requirement"), body = node("div", null, "cr-requirement-body"), title = node("p", null, "cr-requirement-title");
      li.dataset.requirement = kind; li.dataset.satisfied = String(!!item.satisfied); li.dataset.status = tone;
      title.append(node("strong", text), ` · ${human(item.key)}`); body.append(title, node("p", item.detail, "cr-muted"));
      if (item.key === "ci" && Array.isArray(item.ci_projects)) body.append(ciProjectList(ui().ciProjects(item.ci_projects)));
      if (item.key === "ci" && arr(item.untrusted_runs).length) body.append(untrustedRunList(item.untrusted_runs));
      li.append(toneIcon(tone), body); list.append(li);
    }
    return list;
  }
  // r[impl kiln.monorepo.console] Required CI per project of the review repository: each affected project with its
  // state and exact run; skipped projects read as not required, with the recorded reason.
  function ciProjectList(rows) {
    const list = node("ul", null, "cr-ci-projects"); list.setAttribute("aria-label", "Required CI per project");
    if (!rows.length) { list.append(node("li", "No project decisions were recorded.", "cr-muted")); return list; }
    for (const row of rows) {
      const li = node("li", null, "cr-ci-project"), head = node("p", null, "cr-ci-project-head"); li.dataset.ciState = row.state;
      head.append(pill(row.state, `CI for ${row.label}`, row.stateLabel, row.tone), node("strong", row.label));
      li.append(head);
      if (row.required) {
        if (row.reasons.length) li.append(ui().reasonList(row.reasons));
        if (row.runId) { const run = node("a", `Inspect ${row.label} run`, "cr-ci-run"); run.href = `?${new URLSearchParams({ page: "runs", project: row.projectId, run: row.runId, view: "pipeline" })}`; li.append(run); }
      } else li.append(node("p", row.detail, "cr-muted"));
      list.append(li);
    }
    return list;
  }
  // r[impl kiln.untrusted.console] Untrusted pull-request runs of the evaluated commit: state, reason, approver,
  // approved commit and whether policy counts the result for required CI. An approval never meets a requirement.
  function untrustedNote(gate) {
    const approval = gate.approvedBy ? `Approved by ${gate.approvedBy} for commit ${gate.approvedCommit || "not recorded"}.` : "Not approved; nothing has run.";
    return `${gate.reasonText} ${approval} ${gate.countsForRequiredCi ? "Policy counts its result for required CI on this exact commit only." : "Its result does not count for required CI."}`;
  }
  function untrustedRunList(rows) {
    const list = node("ul", null, "cr-ci-projects cr-untrusted-runs"); list.setAttribute("aria-label", "Untrusted pull-request runs");
    for (const row of arr(rows)) {
      const gate = ui().untrustedSummary(row?.untrusted);
      if (!gate || typeof row.run_id !== "string" || !row.run_id) continue;
      const li = node("li", null, "cr-ci-project"), head = node("p", null, "cr-ci-project-head"), run = node("a", "Inspect untrusted run", "cr-ci-run");
      li.dataset.untrustedState = gate.state;
      run.href = `?${new URLSearchParams({ page: "runs", run: row.run_id, view: "pipeline" })}`; run.setAttribute("aria-label", `Inspect untrusted run ${row.run_id}`);
      head.append(pill(gate.state, "Untrusted run", gate.stateLabel, gate.tone), node("strong", gate.pullRequest === null ? "Untrusted pull request" : `Pull request #${gate.pullRequest}`));
      li.append(head, node("p", untrustedNote(gate), "cr-muted"), run); list.append(li);
    }
    return list;
  }
  function stateBox(title, text, cls = "empty-state") { const box = node("div", null, `${cls} cr-state`); box.append(node("p", title, "empty-state-title"), node("p", text, "empty-state-text")); return box; }
  // r[impl kiln.web.page_consistency]
  // Boundary and non-claim prose stays reachable behind the shared labelled "about" disclosure instead of dominating the data.
  // Each label is unique so draw() can restore its open state across redraws.
  function about(parent, summary, ...items) {
    const d = node("details", null, "about-view"), body = node("div", null, "about-view-body"); d.append(ui().disclosureSummary(summary, "info"), body);
    for (const item of items) if (item instanceof Node) body.append(item); else if (item) body.append(node("p", item));
    parent.append(d); return d;
  }
  // Principals and refs: readable wrapping after ":" and "/" separators (wbr), exact value kept verbatim in the text.
  function principalNode(value, cls = "cr-principal", tag = "span") {
    const span = node(tag, null, cls), text = String(value ?? ""); if (!text) { span.textContent = "Not recorded"; return span; }
    for (const part of text.split(/(?<=[:/])/)) { if (span.firstChild) span.append(document.createElement("wbr")); span.append(part); }
    return span;
  }
  // Exact identifier as secondary text: a shortened visible form with the exact value in its title (as before for change IDs).
  function exactId(value, cls = "cr-exact-id") { const code = node("code", short(value), cls); code.title = String(value ?? ""); return code; }
  function titleFor(id) {
    if (state.detail?.change?.id === id) return state.detail.change.title;
    return arr(state.snapshot?.changes).find(c => c.id === id)?.title || arr(state.detail?.stack?.dependencies).find(c => c.id === id)?.title || "";
  }
  function changeRef(id) { const title = titleFor(id), a = changeLink(id, title || id); if (title) a.title = id; return a; }
  function loading(text) {
    const box = node("div", null, "cr-loading"); box.setAttribute("aria-busy", "true"); box.append(node("p", text, "cr-muted"));
    for (let i = 0; i < 3; i++) { const bar = node("div", null, "skeleton"); bar.setAttribute("aria-hidden", "true"); box.append(bar); }
    return box;
  }
  function pager(name, previousText, nextText, summary, atStart, atEnd, previous, next) {
    const nav = node("nav", null, "cr-pager"); nav.setAttribute("aria-label", name);
    const back = button(previousText, previous, "quiet"), forward = button(nextText, next, "quiet"); back.disabled = atStart; forward.disabled = atEnd;
    nav.append(back, node("span", summary, "cr-muted"), forward); return nav;
  }
  function safeLink(value) {
    try { const url = new URL(value); return ["https:", "http:"].includes(url.protocol) && !url.username && !url.password ? url.href : null; } catch { return null; }
  }
  function mentionText(parent, text) {
    const pattern = /(^|[\s(])(@\{[^{}\s]+\}|@[A-Za-z0-9][A-Za-z0-9_.:@/-]*)/g; let offset = 0;
    for (const match of text.matchAll(pattern)) { parent.append(document.createTextNode(text.slice(offset, match.index) + match[1]), node("span", match[2], "cr-mention")); offset = match.index + match[0].length; }
    parent.append(document.createTextNode(text.slice(offset)));
  }
  function inlineMarkdown(parent, text, depth = 0) {
    if (depth > 3) { mentionText(parent, text); return; }
    const pattern = /(`[^`\n]+`|\*\*[^*\n]+\*\*|__[^_\n]+__|\*[^*\n]+\*|\[[^\]\n]+\]\([^\s)]+\))/g;
    let offset = 0, count = 0;
    for (const match of text.matchAll(pattern)) {
      if (++count > TOKEN_LIMIT) break;
      mentionText(parent, text.slice(offset, match.index));
      const value = match[0]; let child;
      if (value.startsWith("`")) child = node("code", value.slice(1, -1));
      else if (value.startsWith("[")) {
        const end = value.indexOf("]("), target = safeLink(value.slice(end + 2, -1));
        if (target) { child = node("a", value.slice(1, end)); child.href = target; child.rel = "noopener noreferrer"; child.target = "_blank"; }
        else child = document.createTextNode(value);
      } else { const strong = value.startsWith("**") || value.startsWith("__"); child = node(strong ? "strong" : "em"); inlineMarkdown(child, value.slice(strong ? 2 : 1, strong ? -2 : -1), depth + 1); }
      parent.append(child); offset = match.index + value.length;
    }
    mentionText(parent, text.slice(offset));
  }
  function markdown(value) {
    const content = node("div", null, "cr-markdown"), text = String(value ?? ""), lines = text.slice(0, RENDER_LIMIT).split("\n");
    let paragraph = [], list = null;
    const flush = () => { if (paragraph.length) { const p = node("p"); inlineMarkdown(p, paragraph.join("\n")); content.append(p); paragraph = []; } list = null; };
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i], fence = line.match(/^\s*(`{3,}|~{3,})([\w+-]*)\s*$/);
      if (fence) {
        flush(); const body = []; let closed = false;
        while (++i < lines.length) { if (lines[i].trim().startsWith(fence[1]) && /^[`~]+$/.test(lines[i].trim())) { closed = true; break; } body.push(lines[i]); }
        const pre = node("pre"); pre.tabIndex = 0; pre.setAttribute("aria-label", fence[2] ? `${fence[2]} code block` : "Code block"); pre.append(node("code", body.join("\n"), "cr-code-text")); content.append(pre);
        if (!closed && i < lines.length) i--; continue;
      }
      const heading = line.match(/^(#{1,6})\s+(.+)$/), item = line.match(/^\s*(?:([-+*])|\d+\.)\s+(.+)$/), quote = line.match(/^>\s?(.*)$/);
      if (!line.trim()) { flush(); continue; }
      if (heading) { flush(); const h = node(`h${Math.min(6, heading[1].length + 2)}`); inlineMarkdown(h, heading[2]); content.append(h); }
      else if (item) { if (paragraph.length) flush(); const tag = item[1] ? "UL" : "OL"; if (!list || list.tagName !== tag) { list = node(tag.toLowerCase()); content.append(list); } const li = node("li"); inlineMarkdown(li, item[2]); list.append(li); }
      else if (quote) { flush(); const q = node("blockquote"); inlineMarkdown(q, quote[1]); content.append(q); }
      else { list = null; paragraph.push(line); }
    }
    flush(); if (text.length > RENDER_LIMIT) content.append(node("p", "Rendered preview is bounded; remaining source follows as plain text.", "cr-muted"), node("pre", text.slice(RENDER_LIMIT))); return content;
  }
  // r[impl kiln.highlight.render] Diff text is drawn from the service's tokens (fragments) or plain.
  function codeCell(text, fragments) { const td = node("td", null, "cr-code"), code = node("code", null, "cr-code-text"); if (fragments) ui().appendFragments(code, fragments); else code.textContent = String(text ?? ""); td.append(code); return td; }
  // r[impl kiln.review.authorship.console]
  // Self-declared, unverified authorship. Client checks mirror the service: trimmed, non-empty,
  // no control characters and at most AUTHORSHIP_TEXT_BYTES UTF-8 bytes; undeclared sends nothing.
  const AUTHORSHIP_TEXT_BYTES = 128;
  const AUTHORSHIP_KINDS = [["undeclared", "Undeclared"], ["human", "Human"], ["agent", "Agent"], ["mixed", "Mixed"]];
  const AUTHORSHIP_NOTE = "Self-declared and unverified. It is recorded for review-friction views and grants no authority.";
  const utf8Bytes = text => { let bytes = 0; for (const ch of String(text)) { const point = ch.codePointAt(0); bytes += point < 0x80 ? 1 : point < 0x800 ? 2 : point < 0x10000 ? 3 : 4; } return bytes; };
  function authorshipText(name, value) {
    const text = String(value ?? "").trim();
    if (!text) throw new Error(`${name} is required for agent authorship.`);
    if (/\p{Cc}/u.test(text)) throw new Error(`${name} must not contain control characters.`);
    if (utf8Bytes(text) > AUTHORSHIP_TEXT_BYTES) throw new Error(`${name} must be at most ${AUTHORSHIP_TEXT_BYTES} bytes.`);
    return text;
  }
  function authorshipPayload(kind, harness = "", model = "") {
    switch (kind || "undeclared") {
      case "undeclared": return null;
      case "human": case "mixed": return { kind };
      case "agent": { const declared = { kind, harness: authorshipText("Harness", harness) }; if (String(model ?? "").trim()) declared.model = authorshipText("Model", model); return declared; }
      default: throw new Error("Select a listed authorship.");
    }
  }
  function authorshipLabel(authorship) {
    const kind = authorship?.kind || "undeclared";
    if (kind !== "agent") return human(kind);
    return `Agent · harness ${authorship.harness}${authorship.model ? ` · model ${authorship.model}` : ""}`;
  }
  const authorshipClassLabel = value => !value || value === "undeclared" ? "Undeclared" : `${human(value)} (self-declared)`;
  const withAuthorship = (action, declared) => declared ? { ...action, authorship: declared } : action;
  function authorshipFields(parent, key) {
    const keys = [`${key}-authorship`, `${key}-harness`, `${key}-model`];
    const kind = field(parent, "Authorship", "select", "undeclared", AUTHORSHIP_KINDS, keys[0]);
    const harness = field(parent, "Harness", "text", "", [], keys[1]), model = field(parent, "Model (optional)", "text", "", [], keys[2]);
    for (const input of [harness, model]) { input.maxLength = AUTHORSHIP_TEXT_BYTES; input.autocomplete = "off"; input.spellcheck = false; }
    const note = node("p", AUTHORSHIP_NOTE, "cr-muted cr-field-note"); note.id = `cr-${crypto.randomUUID()}`; kind.setAttribute("aria-describedby", note.id); kind.parentElement.after(note);
    const sync = () => { const agent = kind.value === "agent"; harness.parentElement.hidden = !agent; model.parentElement.hidden = !agent; harness.required = agent; };
    kind.addEventListener("change", sync); sync();
    return { keys, value: () => authorshipPayload(kind.value, harness.value, model.value) };
  }
  const authorshipFact = declared => ["Authorship", `${authorshipLabel(declared)} (self-declared, unverified)`];
  function contribution() {
    const sources = arr(state.snapshot.contribution_sources);
    if (!sources.length) throw new Error("No contribution branches are admitted for this principal.");
    const { dialog, close } = modal("Contribute branch for review");
    dialog.append(node("p", "Only configured source branches are admitted. The service resolves the exact immutable tip and configured target, without pushing or moving a real ref.", "cr-muted"));
    const source = field(dialog, "Configured contribution branch", "select", sources[0].source_ref, sources.map(s => [s.source_ref, `${s.source_ref} → ${s.target_ref} · ${short(s.commit)}`]), "contribution-source");
    // Narrow selects clip long option labels, so the selected branch's target and tip also read in full below it.
    const routeNote = node("p", null, "cr-muted cr-field-note"); routeNote.id = `cr-${crypto.randomUUID()}`; source.setAttribute("aria-describedby", routeNote.id); source.parentElement.after(routeNote);
    const describe = () => { const s = sources.find(item => item.source_ref === source.value); routeNote.replaceChildren(...(s ? [principalNode(s.source_ref, "cr-principal", "code"), " → ", principalNode(s.target_ref, "cr-principal", "code"), " · tip ", exactId(s.commit)] : ["Select an admitted contribution branch."])); };
    const expected = field(dialog, "Expected exact source commit (clear to resolve branch tip at intake)", "text", sources.find(s => s.source_ref === source.value)?.commit || "", [], "contribution-commit"); expected.autocomplete = "off"; expected.spellcheck = false; expected.classList.add("mono");
    source.addEventListener("change", () => { expected.value = sources.find(s => s.source_ref === source.value)?.commit || ""; state.forms.set(memoryKey("contribution-commit"), expected.value); describe(); }); describe();
    const title = field(dialog, "Title", "text", "", [], "contribution-title"), description = field(dialog, "Description (Markdown supported)", "textarea", "", [], "contribution-description"), wip = field(dialog, "Work in progress", "checkbox", true, [], "contribution-wip");
    const authorship = authorshipFields(dialog, "contribution");
    dialogActions(dialog, close).append(writeButton("Contribute branch…", async () => {
      if (!title.value.trim()) throw new Error("A contribution title is required.");
      const configured = sources.find(s => s.source_ref === source.value); if (!configured) throw new Error("Select an admitted contribution branch.");
      const declared = authorship.value();
      const result = await command(withAuthorship({ type: "contribute", source_ref: source.value, expected_commit: expected.value.trim() || null, title: title.value.trim(), description: description.value, wip: wip.checked }, declared), null, { title: `Contribute branch "${source.value}" for review?`, verb: "Contribute branch", facts: [["Principal", state.context.principal], ["Repository", state.snapshot.repository], ["Source branch", source.value], ["Expected source", expected.value.trim() || "Resolve the configured branch tip at intake; no exact commit precondition"], ["Configured target", configured.target_ref], ["Title", title.value.trim()], ["Description", description.value], ["WIP", String(wip.checked)], authorshipFact(declared), ["Effect", "Register review only; no target movement or remote publication"]] });
      if (result) { clearFields("contribution-source", "contribution-commit", "contribution-title", "contribution-description", "contribution-wip", ...authorship.keys); close(); if (result.change?.id) route({ review_change: result.change.id, review_revision: "" }); }
    }, "primary"));
    source.focus();
  }
  // r[impl kiln.review.jj_intake.surface] Change-ID intake: the service observes jj read-only and admits every member before any effect.
  const JJ_CHANGE_ID = /^[k-z]{32}$/;
  function jjContribution() {
    const jj = state.snapshot.jj_intake; if (!jj) throw new Error("Change-ID intake is not configured.");
    const { dialog, close } = modal("Contribute jj change for review");
    dialog.append(node("p", "Kiln resolves exact full jj change IDs read-only at one jj operation, or reads git change-id headers from an admitted branch. It never rebases, pushes or moves bookmarks. Divergent or conflicted changes refuse the whole request before any effect.", "cr-muted"));
    const sources = [...(jj.jj ? [["jj", "Exact jj change IDs"]] : []), ...(jj.change_id_header ? [["header", "Branch with git change-id headers"]] : [])];
    const mode = field(dialog, "Source", "select", sources[0]?.[0] || "", sources, "jj-mode");
    const ids = field(dialog, `Change IDs, bottom to top, one per line (at most ${jj.max_stack})`, "textarea", "", [], "jj-ids");
    const ref = field(dialog, "Admitted branch", "text", "refs/heads/review/", [], "jj-ref");
    for (const input of [ids, ref]) { input.classList.add("mono"); input.spellcheck = false; input.autocomplete = "off"; }
    const stack = field(dialog, "Submit as an ordered stack", "checkbox", false, [], "jj-stack");
    const wip = field(dialog, "Work in progress", "checkbox", true, [], "jj-wip");
    const authorship = authorshipFields(dialog, "jj");
    const sync = () => { ids.parentElement.hidden = mode.value !== "jj"; ref.parentElement.hidden = mode.value !== "header"; };
    mode.addEventListener("change", sync); sync();
    dialogActions(dialog, close).append(writeButton("Contribute jj change…", async () => {
      const declared = authorship.value(); let action, source;
      if (mode.value === "jj") {
        const list = ids.value.split(/\s+/).filter(Boolean);
        if (!list.length) throw new Error("Enter at least one full jj change ID.");
        const bad = list.find(id => !JJ_CHANGE_ID.test(id)); if (bad) throw new Error(`"${bad}" is not a full 32-character jj change ID. Prefixes and revsets are not accepted.`);
        if (new Set(list).size !== list.length) throw new Error("A change ID is listed twice.");
        if (list.length > jj.max_stack) throw new Error(`At most ${jj.max_stack} change IDs are admitted.`);
        if (list.length > 1 && !stack.checked) throw new Error("Several change IDs require an ordered stack submission.");
        action = { type: "jj_contribute", change_ids: list, stack: stack.checked, wip: wip.checked }; source = ["Change IDs (bottom → top)", list.join("\n")];
      } else {
        const value = ref.value.trim(); if (!/^refs\/heads\/\S+$/.test(value)) throw new Error("Enter an exact refs/heads/… branch.");
        action = { type: "jj_contribute", source_ref: value, stack: stack.checked, wip: wip.checked }; source = ["Branch (git change-id headers)", value];
      }
      const result = await command(withAuthorship(action, declared), null, { title: stack.checked ? "Contribute jj stack for review?" : "Contribute jj change for review?", verb: "Contribute", facts: [["Principal", state.context.principal], ["Repository", state.snapshot.repository], source, ["Stack", stack.checked ? "One change per ID with dependencies in the listed ancestor order. IDs missing from an earlier submission are flagged, never abandoned." : "Single change; dependencies and stack membership untouched"], ["Configured target", jj.target_ref], ["WIP", String(wip.checked)], authorshipFact(declared), ["Effect", "Register a revision only for changes whose exact commit changed. Grants no approval, CI or submit authority."]] });
      if (result) { clearFields("jj-mode", "jj-ids", "jj-ref", "jj-stack", "jj-wip", ...authorship.keys); close(); if (result.change?.id) route({ review_change: result.change.id, review_revision: "" }); }
    }, "primary"));
    (mode.value === "jj" ? ids : ref).focus();
  }
  // r[impl kiln.web.page_consistency]
  // Dialog footer: Cancel then the dialog's single primary action, right-aligned; form dialogs focus their first field.
  function dialogActions(dialog, close) { const actions = node("div", null, "cr-toolbar cr-dialog-actions"); actions.append(button("Cancel", close, "quiet")); dialog.append(actions); return actions; }
  function drawQueue(root, entries = state.snapshot?.queue) {
    if (!Array.isArray(entries)) return;
    // r[impl kiln.web.page_consistency.scenario.states]
    const disabled = state.snapshot?.queue_enabled === false && !entries.length;
    const card = panel(root, "CI-bound merge queue", "queue", entries.length > 0, entries.length ? String(entries.length) : disabled ? "Not configured" : "None");
    about(card, "About the merge queue", "Each entry constructs a candidate against its exact observed target. Required CI must qualify that exact constructed commit before CAS submission. Target movement requires a newly constructed candidate and fresh CI, never reuse of the previous result.");
    for (const entry of entries) {
      const article = node("article", null, "list-row cr-operation"), head = node("div", null, "cr-row-head"), meta = node("p", null, "cr-muted");
      head.append(pill(entry.status, "Queue status"), changeRef(entry.change)); meta.append(principalNode(entry.actor), ` · ${entry.target_ref || "Not recorded"} · enqueued `, when(entry.created_ms)); article.append(head, meta);
      if (entry.detail) article.append(node("p", entry.detail, "cr-text"));
      facts(disclosure(article, "Exact queue identities"), [["Queue entry", entry.id], ["Authorized by", entry.actor], ["Source revision", entry.revision], ["Source commit", entry.source_commit], ["Target ref", entry.target_ref], ["Exact observed target", entry.target_commit], ["Constructed candidate commit", entry.candidate_commit || "Not constructed yet; no exact candidate CI claim"], ["Canonical CI run", entry.ci_run || "No candidate CI observed"], ["Observation", entry.detail], ["Enqueued at", when(entry.created_ms)]]);
      if (entry.ci_run) { const run = node("a", "Inspect candidate CI run"); run.href = `?${new URLSearchParams({ page: "runs", run: entry.ci_run, view: "pipeline" })}`; article.append(run); }
      card.append(article);
    }
    if (!entries.length) card.append(disabled ? stateBox("Merge queue not configured", "This service has no merge queue profile, so CI-bound merge automation is unavailable. Submit changes from their checklist instead.", "empty-state empty-state-inline") : node("p", "No retained merge queue entries.", "cr-muted"));
    const c = state.detail?.change;
    if (!c || revision() !== current()?.id || !Array.isArray(state.snapshot?.queue)) return;
    const entry = entries.find(e => e.change === c.id && !["submitted", "cancelled"].includes(e.status));
    if (entry) card.append(writeButton("Cancel queue entry…", () => command({ type: "dequeue" }, c, { title: `Cancel merge queue entry for "${c.title}"?`, verb: "Cancel queue entry", dismiss: "Keep queued", danger: true, facts: [["Principal", state.context.principal], ["Change", c.id], ["Queue entry", entry.id], ["State", entry.status], ["Effect", "Revoke pending automation; an already completed provider effect cannot be undone"]] })));
    else if (c.state === "open" && state.snapshot?.queue_enabled === false) return;
    else if (c.state === "open") card.append(writeButton("Authorize merge automation…", () => {
      requireCurrent(); const expected = state.detail.source?.current_target; if (!expected) throw new Error("An exact target observation is required before queue authorization.");
      return command({ type: "enqueue", expected_target: expected }, c, { title: `Authorize merge automation for "${c.title}"?`, verb: "Authorize automation", facts: [["Principal", state.context.principal], ["Repository", c.repository], ["Change", c.id], ["Exact source revision", current()?.id], ["Exact source commit", current()?.commit], ["Target ref", c.target_ref], ["Currently observed target", expected], ["Automation scope", "Construct a new merge candidate and run required CI on its exact resulting commit. Automatically submit only after all exact-candidate policy checks pass."], ["Target movement authorization", "If this same target ref moves, recompute a new merge candidate using this exact source revision and the new target, run fresh required CI, and submit the resulting merge commit by exact-target CAS. This authorizes those resulting commits, not a single pre-built commit."], ["Bounds", "No source revision change, target-ref change, policy bypass, or reuse of stale CI is authorized. Cancel the queue entry to revoke pending automation."]] });
    }));
  }
  function drawSubscriptions(root) {
    if (typeof state.detail?.subscribed !== "boolean") return;
    const c = state.detail.change, deliveries = arr(state.detail.delivery_outcomes);
    const card = panel(root, "Subscription and delivery", "subscriptions", false, state.detail.subscribed ? "Subscribed" : "Not subscribed");
    about(card, "About mentions and delivery", `Subscription belongs only to authenticated principal ${state.context.principal}. Published @principal or @{principal} mentions request attention; private drafts do not send notifications. External recipients are configured by the operator, never entered here.`);
    card.append(writeButton(state.detail.subscribed ? "Unsubscribe" : "Subscribe", () => command({ type: "subscribe", subscribed: !state.detail.subscribed }, c)));
    for (const delivery of deliveries) {
      const article = node("article", null, "list-row cr-operation"), head = node("div", null, "cr-row-head"); head.append(pill(delivery.status, "Delivery status"), node("span", human(delivery.notification?.kind), "cr-row-title")); article.append(head);
      facts(article, [["Delivery", delivery.id], ["Notification", delivery.notification?.id], ["Kind", label(delivery.notification?.kind)], ["Actor", delivery.notification?.actor], ["Revision", delivery.notification?.revision], ["Observed status", label(delivery.status)]]);
      for (const attempt of arr(delivery.attempts)) { const body = disclosure(article, `Delivery attempt ${attempt.id} · ${human(attempt.status)}`); facts(body, [["Reserved at", when(attempt.reserved_ms)], ["Completed at", when(attempt.completed_ms)], ["HTTP status", attempt.http_status], ["Outcome", label(attempt.status)]]); }
      card.append(article);
    }
    if (!deliveries.length) card.append(node("p", "No external delivery outcomes are recorded for this principal on this change. Absence is not delivery success.", "cr-muted"));
  }
  function suggestionReplacement(body) {
    let opening = null, result = null, offset = 0;
    for (const line of String(body ?? "").match(/[^\n]*\n|[^\n]+$/g) || []) {
      const marker = line.replace(/[\r\n]+$/, "");
      if (marker === "```suggestion") {
        if (opening !== null || result !== null) return null;
        opening = offset + line.length;
      } else if (marker === "```" && opening !== null) {
        result = body.slice(opening, offset); opening = null;
      }
      offset += line.length;
    }
    return opening === null ? result : null;
  }
  function suggestionText(thread) {
    const matches = arr(thread.comments).map(c => suggestionReplacement(c.body)).filter(text => text !== null);
    return matches.length === 1 ? matches[0] : null;
  }
  function drawSuggestions(root) {
    const c = state.detail.change;
    const threads = arr(c.threads).filter(t => t.anchor.revision === current()?.id && t.anchor.side === "revision" && !t.resolved && suggestionText(t) !== null);
    if (!threads.length) return;
    const card = section(root, "Published suggestions"), chosen = [];
    about(card, "About applying suggestions", "A complete fenced suggestion comment replaces its exact inclusive revision-side line range. Only the current change owner may apply it. Batch ranges must not overlap; stale or mismatched source text fails closed in the service. Applying never rewrites an existing revision or advances the target.");
    for (const thread of threads) { const article = node("article", null, "cr-operation"), a = thread.anchor; article.append(link(`${a.file}:${a.start_line}–${a.end_line}`, anchorRoute(a))); const pre = node("pre", null, "cr-suggestion"); pre.append(node("code", suggestionText(thread), "cr-code-text")); article.append(pre); facts(disclosure(article, "Exact suggestion identities"), [["Thread", thread.id], ["Revision", a.revision], ["Comparison base", a.base_commit], ["Exact path", a.file], ["Inclusive replacement range", `${a.start_line}–${a.end_line}`]]); const input = field(article, "Select this suggestion", "checkbox", false, [], `suggestion-${thread.id}`); input.disabled = c.owner !== state.context.principal || revision() !== current()?.id; chosen.push({ thread, input }); card.append(article); }
    if (c.owner !== state.context.principal || revision() !== current()?.id) { card.append(node("p", "Select the current revision as its authenticated owner to apply suggestions.", "cr-muted")); return; }
    card.append(writeButton("Apply selected suggestions…", async () => {
      requireCurrent(); const selected = chosen.filter(s => s.input.checked).map(s => s.thread); if (!selected.length) throw new Error("Select at least one published suggestion.");
      const result = await command({ type: "apply_suggestions", thread_ids: selected.map(t => t.id) }, c, { title: `Apply ${selected.length} suggestion${selected.length === 1 ? "" : "s"} to "${c.title}"?`, verb: selected.length === 1 ? "Apply suggestion" : "Apply suggestions", facts: [["Author", state.context.principal], ["Change", c.id], ["Exact source revision", current()?.id], ["Exact source commit", current()?.commit], ["Replacements", selected.map(t => `${t.id}\n${t.anchor.file}:${t.anchor.start_line}–${t.anchor.end_line}\n${suggestionText(t)}`).join("\n\n")], ["Effect", "Create a new immutable current review revision. Existing votes and reviewed-file marks do not authorize the new revision; target ref remains unchanged."]] });
      if (result) { clearFields(...selected.map(t => `suggestion-${t.id}`)); route({ review_revision: "", review_base: "base", review_file: "", review_side: "", review_line: "" }); }
    }));
  }
  function report(error) {
    state.error = message(error); if (state.container?.isConnected) drawNotice();
    const dialog = Array.from(state.dialogs).at(-1);
    if (dialog?.isConnected) { let notice = dialog.querySelector(".cr-dialog-error"); if (!notice) { notice = node("p", null, "cr-notice cr-dialog-error"); notice.setAttribute("role", "alert"); dialog.append(notice); } notice.dataset.kind = "error"; notice.textContent = state.error; notice.scrollIntoView({ block: "nearest" }); }
  }
  function button(text, action, cls, write = false) {
    const b = node("button", text, cls); b.type = "button"; b.disabled = write && !writable();
    if (b.disabled) b.title = state.pending ? "A command is in flight; wait for its observed outcome." : "The service has not admitted review writes for this principal.";
    b.addEventListener("click", () => Promise.resolve().then(action).catch(report)); return b;
  }
  function section(parent, title, cls = "") { const s = node("section", null, cls ? `card cr-card ${cls}` : "card cr-card"); s.append(node("h3", title)); parent.append(s); return s; }
  // Secondary panels collapse; their open state survives redraws through data-cr-section.
  function panel(parent, title, key, open = false, badge = null) {
    const d = node("details", null, "card cr-panel"), summary = node("summary"), body = node("div", null, "cr-panel-body"); d.dataset.crSection = key; d.open = open;
    summary.append(node("h3", title)); if (badge) summary.append(chip(badge, "cr-count")); summary.append(icon("chevron-down")); summary.addEventListener("click", () => { d.dataset.crTouched = "true"; });
    d.append(summary, body); parent.append(d); return body;
  }
  function disclosure(parent, title) { const d = node("details", null, "cr-disclosure"); d.append(ui().disclosureSummary(title)); const body = node("div", null, "cr-fields"); d.append(body); parent.append(d); return body; }
  function facts(parent, values) {
    const dl = node("dl", null, "cr-facts");
    for (const [key, value] of values) { const dd = node("dd"); if (value instanceof Node) dd.append(value); else dd.textContent = value == null || value === "" ? "Not recorded" : String(value); dl.append(node("dt", key), dd); }
    parent.append(dl); return dl;
  }
  function memoryKey(key) { const scoped = /^(draft-|reply-|publish)/.test(key) ? revision() : ""; return `${state.context?.principal}\n${selected()}\n${scoped}\n${key}`; }
  function field(parent, name, type = "text", value = "", choices = [], key = "") {
    const wrap = node("label", null, type === "checkbox" ? "cr-field cr-check" : "cr-field"); wrap.append(node("span", name));
    const input = node(type === "select" ? "select" : type === "textarea" ? "textarea" : "input");
    if (type === "select") for (const choice of choices) { const [v, text] = Array.isArray(choice) ? choice : [choice, label(choice)]; const option = node("option", text); option.value = v; input.append(option); }
    else if (type !== "textarea") input.type = type;
    if (type === "textarea") input.rows = 4;
    const remembered = key ? state.forms.get(memoryKey(key)) : undefined;
    if (type === "checkbox") input.checked = remembered === undefined || remembered === false ? (remembered === false ? false : !!value) : !!remembered;
    else input.value = remembered === undefined ? String(value ?? "") : remembered;
    if (key) { input.dataset.crKey = key; input.addEventListener("input", () => state.forms.set(memoryKey(key), type === "checkbox" ? input.checked : input.value)); }
    wrap.append(input); parent.append(wrap); return input;
  }
  function clearFields(...keys) { for (const key of keys) state.forms.delete(memoryKey(key)); }
  function route(patch) {
    if (["review_q", "review_view", "review_status", "review_advisory", "review_agent"].some(key => Object.hasOwn(patch, key)) && !Object.hasOwn(patch, "review_offset")) patch = { ...patch, review_offset: "" };
    // Choosing a revision or base leaves any interdiff or commit-range view unless the patch names one.
    if (["review_change", "review_revision", "review_base"].some(key => Object.hasOwn(patch, key)) && !Object.hasOwn(patch, "review_interdiff")) patch = { review_interdiff: "", review_interdiff_change: "", review_commits: "", ...patch };
    if (Object.hasOwn(patch, "review_revision")) state.pinnedRevision = patch.review_revision || "";
    state.context.navigate(patch);
    // Also support hosts that update history without remounting this component.
    const next = { ...state.route, ...patch };
    if (JSON.stringify(next) !== JSON.stringify(state.route)) load(state.container, { ...state.context, route: next });
  }
  function href(patch) { const q = new URLSearchParams(window.location.search); q.set("page", "code-review"); for (const [k, v] of Object.entries(patch)) v == null || v === "" ? q.delete(k) : q.set(k, String(v)); return `?${q}`; }
  function backLink() { const a = link("All changes", ALL_CHANGES, "cr-back"); a.prepend(icon("arrow-left")); return a; }
  function link(text, patch, cls) { const a = node("a", text, cls); a.href = href(patch); a.addEventListener("click", event => { if (!event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey && event.button === 0) { event.preventDefault(); route(patch); } }); return a; }
  function changeLink(id, text = id) { return link(text, { review_change: id, review_revision: "", review_base: "", review_file: "", review_side: "", review_line: "" }); }
  // r[impl kiln.web.design_system]
  // One overlay at a time: a confirmation hides the dialog that raised it and reopens it on dismissal, then focus returns to the trigger.
  function modal(title, alert = false) {
    const dialog = node("dialog", null, "cr-dialog"); const heading = node("h2", title); heading.id = `cr-${crypto.randomUUID()}`; dialog.setAttribute("aria-labelledby", heading.id); if (alert) dialog.setAttribute("role", "alertdialog");
    const opener = document.activeElement, covered = Array.from(state.dialogs).at(-1);
    const close = () => { dialog.close(); dialog.remove(); state.dialogs.delete(dialog); if (covered?.isConnected && !covered.open && state.dialogs.has(covered)) covered.showModal(); const target = opener?.isConnected ? opener : state.container?.querySelector("button"); target?.focus({ preventScroll: true }); };
    const header = node("div", null, "cr-toolbar cr-dialog-head"); const closer = iconOnly(button("Close", close, "quiet"), icon("x")); closer.removeAttribute("title"); header.append(heading, closer); dialog.append(header); dialog.addEventListener("cancel", event => { event.preventDefault(); close(); });
    document.body.append(dialog); state.dialogs.add(dialog); if (covered?.open) covered.close(); dialog.showModal(); return { dialog, close };
  }
  // Confirmation names the object and the verb: the title asks the decision, the commit repeats the verb, the dismissal keeps the current state.
  async function confirmAction({ title, verb, facts: values, dismiss = "Cancel", danger = false }) {
    return new Promise(resolve => {
      const { dialog, close } = modal(title, true); facts(dialog, values);
      dialog.append(node("p", "Only these exact values and the scope they describe are authorized.", "cr-muted"));
      let settled = false;
      const finish = value => { if (settled) return; settled = true; close(); resolve(value); };
      dialog.addEventListener("close", () => { if (!settled) { settled = true; resolve(false); } });
      const actions = node("div", null, "cr-toolbar cr-dialog-actions"), keep = button(dismiss, () => finish(false), "quiet"); actions.append(keep, button(verb, () => finish(true), danger ? "danger" : "primary")); dialog.append(actions); keep.focus();
    });
  }
  async function command(action, change = state.detail?.change, confirmation = null, readAdmitted = false) {
    if (!sendable(readAdmitted)) throw new Error("Review writes are not admitted for this principal, or another command is pending.");
    const epoch = state.epoch, id = selected(), context = state.context;
    const captured = change ? { id: change.id, version: change.version } : null;
    if (confirmation && !await confirmAction(confirmation)) return null;
    if (!active(epoch, id) || context.principal !== state.context.principal || !sendable(readAdmitted)) throw new Error("Selection, identity or command availability changed before confirmation. No command was sent.");
    const requestId = crypto.randomUUID(); state.pending = requestId; state.error = ""; drawNotice();
    try {
      const result = await context.api(`${ROOT}/commands`, { method: "POST", body: { request_id: requestId, change_id: captured?.id || null, expected_version: captured?.version ?? null, action } });
      if (!active(epoch, id)) return null;
      if (result?.operation) context.toast?.(`Operation ${result.operation.operation_id || ""}: ${result.operation.state || "recorded"}. Refresh to observe reconciliation; do not blindly retry.`);
      else context.toast?.(result?.replayed ? "Previously recorded command observed." : "Review command recorded.");
      return result;
    } catch (error) {
      if (active(epoch, id)) throw new Error(`${message(error)} No automatic retry was made. Refresh to inspect the current change and operation before retrying.`);
      return null;
    } finally {
      if (state.pending === requestId) { state.pending = false; if (active(epoch, id)) await refresh(); else if (state.container?.isConnected) drawNotice(); }
    }
  }
  function drawNotice() {
    const region = state.container?.querySelector(".cr-status-region"); if (!region) return;
    region.textContent = state.error || (state.pending ? "Recording command. Wait for an observed result." : ""); region.hidden = !region.textContent; region.dataset.kind = state.error ? "error" : "status";
    const slot = document.getElementById(HEADER_SLOT)?.querySelector("[data-cr-actions]");
    for (const scope of [state.container, ...state.dialogs, ...(slot ? [slot] : [])]) for (const b of scope.querySelectorAll("[data-cr-write]")) b.disabled = !writable() || b.dataset.crBlocked === "true";
    if (state.container) for (const b of state.container.querySelectorAll("[data-cr-advisory]")) b.disabled = !sendable(true) || b.dataset.crBlocked === "true";
  }
  function writeButton(text, action, cls) { const b = button(text, action, cls, true); b.dataset.crWrite = "true"; return b; }
  function advisoryButton(text, action, cls) { const b = button(text, action, cls); b.dataset.crAdvisory = "true"; b.disabled = !sendable(true); return b; }
  async function refresh() {
    if (!state.container?.isConnected || !state.context) return;
    const epoch = state.epoch, id = selected(), request = ++state.request, context = state.context;
    try {
      const root = await context.api(`${ROOT}?${new URLSearchParams({ q: state.route.review_q || "", view: state.route.review_view || "all", offset: state.route.review_offset || "0", notification_offset: state.route.review_notification_offset || "0", operation_offset: state.route.review_operation_offset || "0", ...(state.route.review_status ? { status: state.route.review_status } : {}), ...(state.route.review_advisory ? { advisory: state.route.review_advisory } : {}), ...(state.route.review_agent ? { agent: state.route.review_agent } : {}) })}`);
      if (!active(epoch, id) || request !== state.request) return;
      if (root?.schema !== "kiln.code-review.v1" || root.principal !== context.principal) throw new Error("Review snapshot schema or principal does not match this authenticated session.");
      state.snapshot = root;
      if (id) {
        const detail = await context.api(path(id));
        if (!active(epoch, id) || request !== state.request) return;
        if (detail?.change?.id !== id) throw new Error("The detail response does not match the selected stable change.");
        state.detail = detail;
        if (!state.pinnedRevision) state.pinnedRevision = state.route.review_revision || arr(detail.change.revisions).at(-1)?.id || "";
      } else state.detail = null;
      state.error = ""; draw();
      if (id) await loadDiff();
    } catch (error) { if (active(epoch, id) && request === state.request) { state.error = message(error); draw(); } }
  }
  // r[impl kiln.web.page_consistency.scenario.data]
  // Access reads as a labelled pill with the exact principal as secondary monospace text; its boundary sentence moves to "About this view".
  const accessText = () => state.snapshot.can_write ? "Review writes admitted" : "Read-only review access";
  function access() {
    // r[impl kiln.web.design_system] Access is a neutral classification, not an outcome: a plain chip with an icon, never a success tone.
    const box = node("div", null, "cr-access"), who = node("span", "as ", "cr-muted"), badge = chip("", "cr-access-chip");
    badge.replaceChildren(icon(state.snapshot.can_write ? "shield-check" : "eye"), node("span", accessText())); badge.dataset.access = state.snapshot.can_write ? "admitted" : "read_only";
    who.append(principalNode(state.context.principal, "cr-principal mono"));
    box.append(badge, who); return box;
  }
  function aboutPage(parent) {
    const items = [`Principal ${state.context.principal} · ${accessText()}. CI and provider submission authority are checked separately.`];
    const claims = arr(state.snapshot.non_claims); if (claims.length) { const list = node("ul", null, "cr-non-claims"); for (const claim of claims) list.append(node("li", claim)); items.push(list); }
    const writers = state.snapshot.target_write_boundary?.unmanaged_writers; if (writers) items.push(`Target write boundary: ${writers}.`);
    return about(parent, "About this view", ...items);
  }
  // r[impl kiln.web.console_layout]
  // The shell page header (#workspace-title) is this page's single header. Review fills only its primary-action slot;
  // the slot sits outside the container, so ownership is marked on the child and released by clear() and non-list states.
  const HEADER_SLOT = "page-primary-action";
  // A drawing page may replace any leftover slot content (active = true); clear() releases only review's own content.
  function headerActions(signature, build, active = true) {
    const slot = document.getElementById(HEADER_SLOT); if (!slot) return;
    const owned = slot.firstElementChild?.dataset.crActions;
    if (!signature) { if (owned || active) slot.replaceChildren(); return; }
    if (owned === signature) return;
    const group = node("div", null, "cr-header-actions cluster"); group.dataset.crActions = signature; build(group); slot.replaceChildren(group);
  }
  function listActions() {
    const contributions = arr(state.snapshot.contribution_sources).length > 0, jj = !!(state.snapshot.jj_intake?.jj || state.snapshot.jj_intake?.change_id_header);
    headerActions(`list|${state.context.principal}|${state.snapshot.can_write === true}|${contributions}|${jj}`, group => {
      if (contributions) group.append(writeButton("Contribute branch…", contribution, "quiet"));
      if (jj) group.append(writeButton("Contribute jj change…", jjContribution, "quiet"));
      group.append(writeButton("Create change…", createChange, "primary"));
    });
  }
  function detailHeader() {
    const c = state.detail.change, header = node("section", null, "card cr-change-header"), heading = node("div", null, "cr-change-heading"), meta = node("div", null, "cr-header-meta"), actions = node("div", null, "cr-change-actions");
    meta.append(pill(c.state, "Change state")); if (c.wip) meta.append(chip("Work in progress"));
    meta.append(chip(`Authorship: ${authorshipClassLabel(c.authorship)}`));
    const selectedRevision = arr(c.revisions).find(item => item.id === revision()) || current();
    if (selectedRevision?.submitted_by) { const attested = selectedRevision.submitted_by; meta.append(chip(`Agent: ${attested.principal}${attested.agent ? ` · ${attested.agent.harness}${attested.agent.model ? ` / ${attested.agent.model}` : ""}` : ""} (declared)`)); }
    const target = node("span", null, "cr-muted"), owner = node("span", "Owner ", "cr-muted"); target.append(exactId(c.repository), " → ", node("code", c.target_ref)); owner.append(principalNode(c.owner)); meta.append(owner, target);
    // One-glance attention: the observed submit readiness links to the requirements card (also below the main column on narrow screens).
    const items = arr(state.detail.checklist?.items), ready = state.detail.checklist?.ready === true, readiness = node("a", null, "cr-readiness"); readiness.href = "#cr-requirements";
    readiness.append(state.detail.checklist ? pill(`ready=${ready}`, "Submit checklist", ready ? "Ready to submit" : "Submit blocked", ready ? "success" : "failure") : pill(null, "Submit checklist", "Checklist unavailable", "failure"), node("span", `${items.filter(item => item.satisfied).length} of ${items.length} requirements met`));
    readiness.addEventListener("click", event => { const card = document.getElementById("cr-requirements"); if (!card) return; event.preventDefault(); card.scrollIntoView({ block: "start" }); card.querySelector("h3")?.focus({ preventScroll: true }); });
    meta.append(readiness);
    heading.append(node("h2", c.title, "cr-change-name"), meta, access()); aboutPage(heading);
    const rev = field(actions, "Review revision", "select", revision(), arr(c.revisions).map(item => [item.id, `Revision ${item.number} · ${short(item.commit)}${item.id === current()?.id ? " · current" : ""}`]));
    rev.dataset.crKey = "header-revision"; rev.addEventListener("change", () => route({ review_revision: rev.value, review_base: "base", review_file: "", review_side: "", review_line: "" }));
    header.append(heading, actions); return [backLink(), header];
  }
  function draw() {
    const focused = document.activeElement; const key = focused?.dataset?.crKey; const start = focused?.selectionStart, end = focused?.selectionEnd;
    const detailsKey = d => d.dataset.crSection || d.querySelector("summary")?.textContent;
    // Data-driven panel defaults apply until the operator toggles that panel; other disclosures keep their prior state.
    const opened = new Map(Array.from(state.container?.querySelectorAll("details") || []).filter(d => !d.dataset.crSection || d.dataset.crTouched === "true").map(d => [detailsKey(d), d.open]));
    const root = state.container; if (!root) return; root.classList.add("code-review"); root.replaceChildren();
    const notice = node("p", null, "cr-notice cr-status-region"); notice.setAttribute("role", "status"); notice.setAttribute("aria-live", "polite");
    const begin = (...head) => { root.append(...head, notice); drawNotice(); };
    if (!state.snapshot) { headerActions(null); begin(); if (!state.error) { root.append(loading("Loading review workspace…")); return; } const box = stateBox("The review workspace could not be loaded", "The observed error is shown above. Retry to observe it again; nothing was changed.", "error-state"); box.append(button("Retry", refresh, "quiet")); root.append(box); return; }
    if (!state.snapshot.configured) {
      headerActions(null); begin(); const box = stateBox("Interactive review is not configured", "A separately admitted collaboration provider, actors, policy and durable store are required. Existing review receipts remain available in the receipts section.");
      const receipts = node("a", "Open Patchgate receipts", "cr-state-link"); receipts.href = "?page=reviews"; box.append(receipts); root.append(box); return;
    }
    if (!selected()) { listActions(); begin(aboutPage(document.createDocumentFragment())); drawList(root); }
    else if (state.detail) { headerActions(null); begin(...detailHeader()); drawDetail(root); }
    else if (!state.error) { headerActions(null); begin(backLink()); root.append(loading("Loading change…")); }
    else { headerActions(null); begin(backLink()); const box = stateBox("Selected change is unavailable", "Retry, or return to the change list. A missing change is never replaced by another."); box.append(button("Retry", refresh, "quiet")); root.append(box); }
    for (const d of root.querySelectorAll("details")) { const k = detailsKey(d); if (opened.has(k) && d.dataset.crForceOpen !== "true") { d.open = opened.get(k); if (d.dataset.crSection) d.dataset.crTouched = "true"; } }
    if (key) { const next = Array.from(root.querySelectorAll("[data-cr-key]")).find(n => n.dataset.crKey === key); if (next) { next.focus({ preventScroll: true }); if (typeof start === "number" && next.setSelectionRange && next.type !== "checkbox" && next.tagName !== "SELECT") { try { next.setSelectionRange(start, end); } catch {} } } }
  }
  function layout(root, name, cls = "") {
    const grid = node("div", null, `cr-layout ${cls}`.trim()), main = node("div", null, "cr-main"), aside = node("aside", null, "cr-aside"); aside.setAttribute("aria-label", name);
    grid.append(main, aside); root.append(grid); return { main, aside };
  }
  function drawList(root) {
    const { main, aside } = layout(root, "Review activity", "cr-list-layout");
    // r[impl kiln.web.console_layout]
    // One card holds the query controls and the change table; access stays explicit in the card heading.
    // r[impl kiln.web.page_consistency.scenario.data]
    const list = node("section", null, "card cr-card cr-changes"), listHead = node("div", null, "cr-card-head"), listTitle = node("h2", "Changes", "card-title");
    listTitle.id = "cr-changes-title"; list.setAttribute("aria-labelledby", listTitle.id); listHead.append(listTitle, access()); list.append(listHead); main.append(list);
    const search = node("form", null, "toolbar cr-toolbar cr-search"); search.setAttribute("role", "search"); search.setAttribute("aria-label", "Search changes");
    const q = field(search, "Search change text", "search", state.route.review_q || "", [], "search");
    const status = field(search, "State", "select", state.route.review_status || "", [["", "All states"], ["open", "Open"], ["abandoned", "Abandoned"], ["submitted", "Submitted"]]);
    const advisory = field(search, "Advisory approvals", "select", state.route.review_advisory || "", [["", "Any advisory state"], ["with", "With advisory approvals"], ["without", "Without advisory approvals"]]);
    const agent = field(search, "Agent submission", "select", state.route.review_agent || "", [["", "Any agent state"], ["with", "Agent-submitted revision"], ["without", "No agent attestation"]]);
    const submit = node("button"); submit.type = "submit"; submit.append(icon("search"), node("span", "Search")); search.append(submit);
    search.addEventListener("submit", event => { event.preventDefault(); route({ review_q: q.value, review_status: status.value, review_advisory: advisory.value, review_agent: agent.value, review_change: "" }); }); list.append(search);
    const active = [];
    if ((state.route.review_view || "all") !== "all") active.push(`view: ${state.route.review_view}`);
    if (state.route.review_q) active.push(`text: ${state.route.review_q}`);
    if (state.route.review_status) active.push(`state: ${state.route.review_status}`);
    if (state.route.review_advisory) active.push(`advisory: ${state.route.review_advisory}`);
    if (state.route.review_agent) active.push(`agent: ${state.route.review_agent}`);
    if (active.length) {
      const row = node("p", null, "cr-muted"), clear = button("Clear filters", () => route({ review_q: "", review_status: "", review_advisory: "", review_agent: "", review_view: "all", review_offset: "" }), "chip");
      row.append(`Active filters — ${active.join(" · ")} `, clear); list.append(row);
    }
    const kind = state.route.review_view || "all", views = node("div", null, "cr-views"), group = node("div", null, "segmented"); group.setAttribute("role", "group"); group.setAttribute("aria-label", "Personal views");
    for (const [text, value] of [["All changes", "all"], ["Awaiting my review", "awaiting_review"], ["My changes needing revision", "owned_needs_revision"], ["Unresolved discussions", "unresolved"]]) { const b = button(text, () => { clearFields("search"); route({ review_view: value }); }); b.setAttribute("aria-pressed", String(value === kind)); group.append(b); }
    views.append(group);
    const savedViews = arr(state.snapshot.saved_views);
    if (savedViews.length) {
      const list = node("ul", null, "cr-saved-views"); list.setAttribute("aria-label", "Saved personal views");
      for (const view of savedViews) {
        const li = node("li"), applied = view.query.kind === kind && (view.query.text || "") === (state.route.review_q || "") && (view.query.status || "") === (state.route.review_status || "") && advisoryRoute(view.query.advisory) === (state.route.review_advisory || "") && agentRoute(view.query.agent) === (state.route.review_agent || "");
        const apply = button(view.name, () => { clearFields("search"); route({ review_q: view.query.text, review_view: view.query.kind, review_status: view.query.status || "", review_advisory: advisoryRoute(view.query.advisory), review_agent: agentRoute(view.query.agent) }); }, "chip"); apply.setAttribute("aria-pressed", String(applied));
        li.append(apply, iconOnly(writeButton(`Delete view ${view.name}…`, () => command({ type: "delete_view", id: view.id }, null, { title: `Delete personal view "${view.name}"?`, verb: "Delete view", danger: true, facts: [["Name", view.name], ["Query", JSON.stringify(view.query)]] }), "quiet"), icon("x"))); list.append(li);
      }
      views.append(list);
    }
    const save = node("div", null, "toolbar cr-toolbar"); const name = field(save, "Save current query as", "text", "", [], "view-name"); save.append(writeButton("Save personal view", async () => { if (!name.value.trim()) throw new Error("A saved view name is required."); await command({ type: "save_view", name: name.value.trim(), query: { kind: state.route.review_view || "all", text: state.route.review_q || "", status: state.route.review_status || null, ...(state.route.review_advisory ? { advisory: state.route.review_advisory === "with" } : {}) } }, null); clearFields("view-name"); }));
    disclosure(views, "Save current query as a personal view").append(save); list.append(views);
    const changes = arr(state.snapshot.changes), offset = Number(state.route.review_offset) || 0;
    if (changes.length) {
      const wrap = node("div", null, "data-table-wrap"), table = node("table", null, "data-table data-table-stack cr-change-table"), head = node("thead"), hr = node("tr"), body = node("tbody");
      table.append(node("caption", "Changes matching the current query", "sr-only"));
      for (const text of ["Change", "State", "Owner", "Target", "Latest revision"]) { const th = node("th", text); th.scope = "col"; hr.append(th); } head.append(hr); table.append(head);
      for (const change of changes) {
        const tr = node("tr"), latest = arr(change.revisions).at(-1), cell = (name, ...content) => { const td = node("td"); td.dataset.label = name; td.append(...content); tr.append(td); return td; };
        const title = changeLink(change.id, change.title); title.classList.add("cr-change-title");
        const meta = node("p", null, "cr-muted cr-change-meta"); meta.append(exactId(change.id));
        if (change.unresolved) meta.append(chip(`${change.unresolved} unresolved`));
        if (change.advisory_current) meta.append(chip(`${change.advisory_current} advisory (non-binding)`, "cr-chip-advisory"));
        if (latest?.submitted_by) { const attested = latest.submitted_by; meta.append(chip(`Agent: ${attested.principal}${attested.agent ? ` · ${attested.agent.harness}${attested.agent.model ? ` / ${attested.agent.model}` : ""}` : ""} (declared)`)); }
        const attention = arr(change.attention), others = attention.filter(p => p !== state.context.principal);
        if (others.length < attention.length) meta.append(chip("Needs your attention", "cr-chip-attention"));
        if (others.length) { const who = node("span", `${others.length < attention.length ? "Also" : "Attention"}: `); others.forEach((p, i) => who.append(...(i ? [", "] : []), principalNode(p))); meta.append(who); }
        cell("Change", title, meta);
        const stateCell = cell("State", pill(change.state, "Change state")); if (change.wip) stateCell.append(chip("WIP"));
        cell("Owner", principalNode(change.owner));
        cell("Target", principalNode(change.target_ref, "cr-principal", "code"), node("p", null, "cr-muted")).lastChild.append(exactId(change.repository));
        cell("Latest revision", when(latest?.created_ms), node("p", latest ? `Revision ${latest.number}` : "Not recorded", "cr-muted"));
        body.append(tr);
      }
      table.append(body); wrap.append(table); list.append(wrap);
    } else {
      // r[impl kiln.web.page_consistency.scenario.states]
      const filtered = !!(state.route.review_q || state.route.review_status || state.route.review_advisory || kind !== "all");
      const empty = filtered ? stateBox("No changes match this query", "Adjust the search text, state or personal view.")
        : offset ? stateBox("No changes on this page", "The retained list may have changed. Return to the previous page.")
        : stateBox("No changes yet", state.snapshot.can_write ? `Use Create change… to register an exact local commit for review.${state.snapshot.intake_enabled ? " Submitted contribution branches appear here too." : ""}` : "No change has been registered. Review writes are not admitted for this principal.");
      if (filtered) empty.append(button("Clear filters", () => { clearFields("search"); route({ review_q: "", review_status: "", review_advisory: "", review_view: "" }); }, "quiet"));
      list.append(empty);
    }
    if (changes.length || offset) list.append(pager("Change pages", "Previous page", "Next page", `${changes.length ? offset + 1 : 0}–${offset + changes.length} of ${state.snapshot.total ?? changes.length} matching changes`, offset === 0, state.snapshot.next_offset == null, () => route({ review_offset: String(Math.max(0, offset - 25)) }), () => route({ review_offset: String(state.snapshot.next_offset) })));
    const notifications = arr(state.snapshot.notifications), unread = notifications.filter(item => !item.read).length, notificationOffset = Number(state.route.review_notification_offset) || 0;
    const inbox = panel(aside, "Review notifications", "notifications", true, unread ? `${unread} unread` : notifications.length ? null : "None");
    if (notifications.length) {
      const rows = node("ul", null, "cr-rows");
      for (const item of notifications) {
        // The notification kind and change title lead; actor, time, read state and the exact revision follow as metadata.
        const li = node("li", null, "list-row cr-notification"), text = node("div", null, "cr-row-main"), meta = node("p", null, "cr-muted"), title = titleFor(item.change); li.dataset.read = String(!!item.read);
        meta.append(principalNode(item.actor), ...(item.agent ? [chip(`Agent${item.agent_harness ? ` · ${item.agent_harness}${item.agent_model ? ` / ${item.agent_model}` : ""}` : ""}`)] : []), " · ", when(item.created_ms), ` · ${item.read ? "Read" : "Unread"} · `, exactId(item.revision)); text.append(changeLink(item.change, title ? `${human(item.kind)}: ${title}` : `${human(item.kind)} · ${short(item.revision)}`), meta);
        li.append(text, writeButton(item.read ? "Mark unread" : "Mark read", () => command({ type: "read_notification", id: item.id, read: !item.read }, null), "quiet")); rows.append(li);
      }
      inbox.append(rows);
    } else inbox.append(node("p", "No retained notifications for this principal.", "cr-muted"));
    if (notificationOffset > 0 || state.snapshot.next_notification_offset != null) inbox.append(pager("Notification pages", "Previous notifications", "Next notifications", `${notifications.length} shown · ${state.snapshot.notifications_total ?? notifications.length} retained notifications`, notificationOffset === 0, state.snapshot.next_notification_offset == null, () => route({ review_notification_offset: String(Math.max(0, notificationOffset - 200)) }), () => route({ review_notification_offset: String(state.snapshot.next_notification_offset) })));
    drawQueue(aside);
    const operations = arr(state.snapshot.operations), operationOffset = Number(state.route.review_operation_offset) || 0;
    const outstanding = panel(aside, "Outstanding provider operations", "operations", operations.length > 0, operations.length ? String(state.snapshot.operations_total ?? operations.length) : "None");
    about(outstanding, "About outstanding operations", "Pending and uncertain operations reserve their affected changes, including creates that do not yet have materialized detail. Refresh observes reconciliation; never blindly repeat an effect.");
    for (const op of operations) {
      const article = node("article", null, "list-row cr-operation"), head = node("div", null, "cr-row-head"); head.append(pill(op.state, "Operation state"), node("span", human(op.kind), "cr-row-title")); article.append(head);
      if (op.change) { const affected = node("p", "Affects ", "cr-muted"); affected.append(changeRef(op.change)); article.append(affected); }
      facts(disclosure(article, "Exact operation identities"), [["Operation", op.operation_id], ["Kind", label(op.kind)], ["Observed state", op.state], ["Affected stable change", op.change], ["Source commit", op.commit], ["Target ref", op.target_ref], ["Observed target commit", op.target_commit]]);
      if (op.result_change) { const result = node("p", "Resulting change: ", "cr-muted"); result.append(changeRef(op.result_change)); article.append(result); }
      outstanding.append(article);
    }
    if (!operations.length) outstanding.append(node("p", "No outstanding operations on this page.", "cr-muted"));
    if (operationOffset > 0 || state.snapshot.next_operation_offset != null) outstanding.append(pager("Operation pages", "Previous operations", "Next operations", `${operations.length} shown · ${state.snapshot.operations_total ?? 0} outstanding operations`, operationOffset === 0, state.snapshot.next_operation_offset == null, () => route({ review_operation_offset: String(Math.max(0, operationOffset - 100)) }), () => route({ review_operation_offset: String(state.snapshot.next_operation_offset) })));
  }
  function createChange() {
    const { dialog, close } = modal("Create change");
    facts(dialog, [["Project", state.snapshot.project], ["Repository", state.snapshot.repository]]).classList.add("cr-facts-compact");
    const form = node("div", null, "cr-fields"); dialog.append(form);
    const title = field(form, "Title", "text", "", [], "create-title"); title.required = true;
    const description = field(form, "Description", "textarea", "", [], "create-description");
    const commit = field(form, "Exact immutable commit", "text", "", [], "create-commit"); commit.required = true; commit.autocomplete = "off"; commit.spellcheck = false; commit.classList.add("mono");
    const target = field(form, "Admitted target ref", "select", arr(state.snapshot.target_refs)[0] || "", arr(state.snapshot.target_refs), "create-target");
    const wip = field(form, "Work in progress", "checkbox", true, [], "create-wip");
    const authorship = authorshipFields(form, "create");
    dialog.append(node("p", "Registers an exact locally available commit as a new review. The target does not move and nothing is published to a remote provider.", "cr-muted"));
    dialogActions(dialog, close).append(writeButton("Create change…", async () => {
      if (!title.value.trim() || !commit.value.trim() || !target.value) throw new Error("Title, exact commit and admitted target are required.");
      const declared = authorship.value();
      const result = await command(withAuthorship({ type: "create", title: title.value.trim(), description: description.value, commit: commit.value.trim(), target_ref: target.value, wip: wip.checked }, declared), null, { title: `Create change "${title.value.trim()}"?`, verb: "Create change", facts: [["Repository", state.snapshot.repository], ["Source commit", commit.value.trim()], ["Target ref", target.value], ["Title", title.value.trim()], ["WIP", String(wip.checked)], authorshipFact(declared)] });
      if (result) { clearFields("create-title", "create-description", "create-commit", "create-target", "create-wip", ...authorship.keys); close(); if (result.change?.id) route({ review_change: result.change.id }); }
    }, "primary"));
    title.focus();
  }
  function drawCi(root) {
    const c = state.detail.change, selectedRevision = arr(c.revisions).find(r => r.id === revision());
    const matching = arr(state.detail.ci_runs).filter(fact => fact.project === c.project && fact.repository === c.repository && fact.revision === selectedRevision?.commit);
    const card = panel(root, "Canonical CI", "ci", true, matching.length ? String(Math.min(matching.length, 25)) : "None");
    about(card, "About canonical CI evidence", "Up to 25 newest exact-source facts retained in this response are shown. Full/clean successful facts and the policy's exact winner evaluation remain checklist authority; a displayed run alone does not authorize submission.");
    facts(card, [["Review revision", selectedRevision?.id], ["Exact source commit", selectedRevision?.commit]]).classList.add("cr-facts-compact");
    for (const fact of matching.slice(0, 25)) {
      const row = node("article", null, "list-row cr-operation"), head = node("div", null, "cr-row-head"), tags = node("div", null, "cr-tags"), run = node("a", "Inspect run");
      run.href = `?${new URLSearchParams({ page: "runs", run: fact.run_id, view: "pipeline" })}`; run.setAttribute("aria-label", `Inspect run ${fact.run_id}`);
      head.append(pill(`success=${fact.success === true}`, "Canonical CI", fact.success ? "Passed" : "Not passed or not terminal", fact.success ? "success" : "warning"), run, exactId(fact.run_id));
      tags.append(chip(fact.full_run ? "Full run" : "Partial run"), chip(fact.clean ? "Clean input" : "Dirty input"), chip(Number.isSafeInteger(fact.ordinal) ? `Ordinal ${fact.ordinal}` : "Ordinal outside exact browser integer range; service ordering retained"));
      const gate = ui().untrustedSummary(fact.untrusted);
      if (gate) tags.append(pill(gate.state, "Untrusted run", gate.stateLabel, gate.tone));
      row.append(head, tags);
      if (gate) row.append(node("p", untrustedNote(gate), "cr-muted"));
      card.append(row);
    }
    if (!matching.length) card.append(node("p", "No canonical exact-source CI evidence is present in this retained response for the selected revision. Missing evidence is not success; inspect the checklist's exact blocked reason.", "cr-notice"));
  }
  function drawDetail(root) {
    const c = state.detail.change, r = arr(c.revisions).find(item => item.id === revision());
    if (revision() !== current()?.id) root.append(node("p", "Historical revision selected. Comments and drafts retain this exact revision and comparison base. Optional votes always apply to the current revision shown in the publication confirmation; provider actions require the current revision.", "cr-notice"));
    if (!r) root.append(node("p", "The requested revision is not part of this change. Choose an admitted revision.", "cr-notice"));
    const { main, aside } = layout(root, "Submit requirements and change details", "cr-detail-layout");
    const description = section(main, "Description"); description.append(String(c.description ?? "").trim() ? markdown(c.description) : node("p", "No description provided.", "cr-muted"));
    const diff = section(main, "Changed files and diff", "cr-diff-card"), controls = node("div", null, "toolbar cr-toolbar cr-diff-toolbar");
    // r[impl kiln.review.interdiff.route] "Changes since revision N" removes base movement; the route restores it.
    const earlier = arr(c.revisions).filter(item => item.number < (r?.number || 0)), inter = interdiffRoute();
    const base = field(controls, "Compare against", "select", inter ? `interdiff:${inter.from}` : commitsRoute() ? "commits" : state.route.review_base || "base", [["base", "Selected revision's base"], ...earlier.map(item => [item.id, `Revision ${item.number} · ${short(item.commit)}`]), ...earlier.map(item => [`interdiff:${item.id}`, `Changes since revision ${item.number}`]), ...(commitsRoute() ? [["commits", "Commit range (below)"]] : [])]);
    base.dataset.crKey = "diff-base"; base.addEventListener("change", () => base.value.startsWith("interdiff:") ? route({ review_interdiff: `${base.value.slice(10)}..${revision()}`, review_interdiff_change: "", review_commits: "", review_revision: "", review_base: "", review_file: "", review_side: "", review_line: "" }) : route({ review_revision: revision(), review_base: base.value === "commits" ? "base" : base.value, review_file: "", review_side: "", review_line: "" }));
    const whitespace = field(controls, "Whitespace comparison", "select", state.route.review_whitespace || "exact", [["exact", "Show all changes (exact)"], ["ignore_all", "Ignore all whitespace"], ["ignore_change", "Ignore whitespace amount"], ["ignore_eol", "Ignore whitespace at end of line"]]);
    whitespace.dataset.crKey = "diff-whitespace"; whitespace.addEventListener("change", () => route({ review_whitespace: whitespace.value }));
    // r[impl kiln.review.structural] Opt-in syntax-aware view; grammars run only in the service.
    const comparisonMode = field(controls, "Comparison mode", "select", state.route.review_mode || "line", [["line", "Line diff"], ["structural", "Structural (syntax-aware)"]]);
    comparisonMode.dataset.crKey = "diff-mode"; comparisonMode.addEventListener("change", () => route({ review_mode: comparisonMode.value === "line" ? "" : comparisonMode.value }));
    const modeField = node("div", null, "cr-field"), modeLabel = node("span", "Diff layout"), modes = node("div", null, "segmented"); modeLabel.id = `cr-${crypto.randomUUID()}`; modes.setAttribute("role", "group"); modes.setAttribute("aria-labelledby", modeLabel.id);
    for (const [value, text] of [["unified", "Unified"], ["split", "Side by side"]]) { const b = button(text, () => { state.mode = value; for (const other of modes.children) other.setAttribute("aria-pressed", String(other === b)); drawDiff(); }); b.setAttribute("aria-pressed", String(state.mode === value)); modes.append(b); }
    modeField.append(modeLabel, modes); controls.append(modeField); diff.append(controls, node("div", null, "cr-diff-host")); drawDiff();
    drawDiscussion(main); drawSuggestions(main);
    drawApproval(aside); drawReviewers(aside); drawCi(aside); drawIdentity(aside); drawEvolog(aside); drawStack(aside);
    drawQueue(aside, Array.isArray(state.detail.queue) ? state.detail.queue : state.detail.queue ? [state.detail.queue] : arr(state.snapshot.queue).filter(e => e.change === c.id));
    drawSubscriptions(aside); drawOperations(aside); drawAuthorActions(aside);
  }
  // r[impl kiln.review.agent.console]
  function drawIdentity(root) {
    const c = state.detail.change, count = arr(c.revisions).length, card = panel(root, "Identities and revisions", "identity", false, `${count} revision${count === 1 ? "" : "s"}`);
    facts(card, [["Stable change", c.id], ["State", `${label(c.state)}${c.wip ? " · Work in progress" : ""}`], ["Owner", c.owner], ["Project / repository", `${c.project} / ${c.repository}`], ["Target ref", c.target_ref], ["Observed target", state.detail.source?.current_target], ["Current revision", current()?.id], ["Current commit", current()?.commit], ["Authorship (self-declared, unverified)", authorshipClassLabel(c.authorship)]]);
    const log = node("ol", null, "cr-history");
    for (const item of arr(c.revisions)) { const li = node("li"), meta = node("span", null, "cr-muted"); if (item.id === revision()) li.setAttribute("aria-current", "true"); meta.append(`${item.author} · `, when(item.created_ms), ` · ${authorshipLabel(item.authorship)}`, item.submitted_by ? ` · attested agent ${item.submitted_by.principal}${item.submitted_by.agent ? ` (${item.submitted_by.agent.harness}${item.submitted_by.agent.model ? ` / ${item.submitted_by.agent.model}` : ""})` : ""}` : ""); li.append(link(`Revision ${item.number}`, { review_revision: item.id, review_base: "base", review_file: "", review_side: "", review_line: "" }), meta, node("code", item.commit)); log.append(li); }
    if (current()?.submitted_by) { const attested = current().submitted_by; card.append(node("p", `The current revision was submitted by the attested agent principal ${attested.principal}${attested.agent ? ` (${attested.agent.harness}${attested.agent.model ? ` / ${attested.agent.model}` : ""})` : ""}. Authorship above remains self-declared and unverified.`, "cr-muted")); }
    card.append(log);
  }
  // r[impl kiln.review.jj_intake.evolog] jj facts are provider observations; each row links the interdiff from the previous revision.
  function drawEvolog(root) {
    const rows = state.detail.jj_evolog, membership = state.detail.jj_stack; if (!Array.isArray(rows) && !membership) return;
    const removed = membership?.removed === true;
    const card = panel(root, "jj evolog", "jj-evolog", removed, removed ? "Removed from stack" : Array.isArray(rows) ? `${rows.length}` : null);
    about(card, "About jj observations", "Change IDs, operations, predecessors and authors are observed from jj or from git change-id headers. They are provider facts, never Kiln identities, and grant no authority.");
    if (membership) facts(card, [["jj change ID", membership.jj_change_id], ["Stack", !membership.stack ? "Not part of a stack submission" : removed ? "Removed from its submitted stack. The change stays open; abandon it explicitly if it is no longer needed." : `Position ${membership.position} of ${membership.size}, bottom to top`]]);
    if (!Array.isArray(rows)) return;
    const log = node("ol", null, "cr-history");
    for (const row of rows) {
      const li = node("li"), meta = node("span", null, "cr-muted"), observed = row.jj; if (row.revision === revision()) li.setAttribute("aria-current", "true");
      meta.append(observed ? `${observed.author_name} <${observed.author_email}> · ` : "", `${authorshipLabel(row.authorship)} · submitted by ${row.submitted_by} · `, when(row.created_ms));
      li.append(link(`Revision ${row.number}`, { review_revision: row.revision, review_base: "base", review_file: "", review_side: "", review_line: "" }), meta, exactId(row.commit));
      if (observed) {
        const predecessors = node("span", null, "cr-tags"); for (const commit of arr(observed.predecessors)) predecessors.append(exactId(commit)); if (!arr(observed.predecessors).length) predecessors.append(observed.source === "jj" ? "None" : "Not carried by git headers");
        facts(li, [["Source", observed.source === "jj" ? "jj" : "git change-id header"], ["jj predecessors", predecessors], ["Observed operation", observed.operation ? exactId(observed.operation) : null], ["Created by operation", observed.created_by_operation ? exactId(observed.created_by_operation) : null]]);
      } else li.append(node("span", "No jj observation for this revision", "cr-muted"));
      if (row.predecessor_matches_previous === false) li.append(chip("Previous revision is not a direct jj predecessor"));
      if (row.interdiff) li.append(link("Interdiff from previous revision", { review_interdiff: row.interdiff, review_interdiff_change: "", review_commits: "", review_revision: "", review_base: "", review_file: "", review_side: "", review_line: "" }));
      log.append(li);
    }
    card.append(log);
  }
  function drawOperations(root) {
    const ops = arr(state.detail.operations), card = panel(root, "Durable provider operations", "change-operations", ops.some(op => op.state === "pending" || op.state === "uncertain"), ops.length ? String(ops.length) : "None");
    about(card, "About durable operations", "Pending or uncertain effects are not automatically retried by this page. Refresh observes the service's durable reconciliation. Read-only review receipts remain in the existing receipts section.");
    for (const op of ops) {
      const article = node("article", null, "list-row cr-operation"), head = node("div", null, "cr-row-head"); head.append(pill(op.state, "Operation state"), node("span", human(op.kind), "cr-row-title")); article.append(head);
      facts(disclosure(article, "Exact operation identities"), [["Operation", op.operation_id], ["Kind", label(op.kind)], ["Observed state", op.state], ["Commit", op.commit], ["Target ref", op.target_ref], ["Target commit", op.target_commit]]);
      if (op.result_change) { const result = node("p", "Resulting change: ", "cr-muted"); result.append(changeRef(op.result_change)); article.append(result); }
      if (op.state === "pending" || op.state === "uncertain") article.append(node("p", "This operation reserves the affected change until the service reconciles its exact outcome. Do not retry the provider action.", "cr-notice"));
      card.append(article);
    }
    if (!ops.length) card.append(node("p", "No retained provider operations for this change.", "cr-muted"));
  }
  function requireCurrent() { if (revision() !== current()?.id) throw new Error("This is a historical revision. Select the current revision before changing review state."); }
  function drawReviewers(root) {
    const c = state.detail.change, entries = Object.entries(c.reviewers || {}), card = panel(root, "Reviewers, CC and attention", "reviewers", true, entries.length ? String(entries.length) : "None");
    if (entries.length) { const list = node("ul", null, "cr-rows"); for (const [principal, role] of entries) { const li = node("li", null, "list-row"); li.append(principalNode(principal, "cr-row-title cr-principal"), chip(human(role)), ...(arr(c.agent_reviewers).includes(principal) ? [chip("Agent")] : []), iconOnly(writeButton(`Remove ${principal}`, () => command({ type: "set_reviewer", principal, role: null }, c), "quiet"), icon("x"))); list.append(li); } card.append(list); }
    else card.append(node("p", "No reviewers or CC requested.", "cr-muted"));
    const form = node("div", null, "toolbar cr-toolbar"); const principal = field(form, "Principal", "text", "", [], "reviewer-principal"); const role = field(form, "Role", "select", "reviewer", [["reviewer", "Reviewer"], ["cc", "CC"]], "reviewer-role");
    const add = writeButton(role.value === "cc" ? "Add CC" : "Request review", async () => { requireCurrent(); if (!principal.value.trim()) throw new Error("A principal is required."); await command({ type: "set_reviewer", principal: principal.value.trim(), role: role.value }, c); clearFields("reviewer-principal"); });
    role.addEventListener("change", () => { add.textContent = role.value === "cc" ? "Add CC" : "Request review"; }); form.append(add); card.append(form);
    const attentionLine = node("p", "Attention: ", "cr-muted"); arr(c.attention).forEach((p, i) => attentionLine.append(...(i ? [", "] : []), principalNode(p))); if (!arr(c.attention).length) attentionLine.append("No principals"); card.append(attentionLine);
    const attention = node("div", null, "toolbar cr-toolbar"); const who = field(attention, "Attention principal", "text", state.context.principal, [], "attention-principal");
    for (const [text, value, cls] of [["Request attention", true, undefined], ["Clear attention", false, "quiet"]]) attention.append(writeButton(text, () => { requireCurrent(); return command({ type: "set_attention", principal: who.value.trim(), attention: value }, c); }, cls)); card.append(attention);
  }
  function drawApproval(root) {
    const c = state.detail.change, checklist = state.detail.checklist, items = arr(checklist?.items), ready = checklist?.ready === true, policy = state.snapshot.policy;
    const card = section(root, "Submit requirements", "cr-requirements"), summary = node("div", null, "cr-row-head"); card.id = "cr-requirements"; card.firstElementChild.tabIndex = -1;
    summary.append(checklist ? pill(`ready=${ready}`, "Submit checklist", ready ? "Ready to submit" : "Blocked", ready ? "success" : "failure") : pill(null, "Submit checklist", "Checklist unavailable", "failure"), node("span", `${items.filter(item => item.satisfied).length} of ${items.length} met`, "cr-muted"));
    card.append(summary);
    if (c.submission) {
      const receipt = section(card, "Observed submission receipt"), outcome = c.submission;
      facts(receipt, [["Submitted by", outcome.submitted_by], ...(outcome.agent ? [["Agent", `${text(outcome.agent.harness)}${outcome.agent.model ? ` · ${text(outcome.agent.model)}` : ""} (declared)`]] : []), ["Submitted at", when(outcome.submitted_ms)], ["Candidate", outcome.candidate?.id], ["Exact source revision", outcome.candidate?.revision], ["Exact source commit", outcome.candidate?.commit], ["Target ref", outcome.candidate?.target_ref], ["Expected target commit", outcome.candidate?.target], ["Result commit", outcome.result_commit], ["CI run", outcome.candidate?.ci_run], ["Exact dependencies", JSON.stringify(outcome.candidate?.dependencies || [])]]);
    }
    card.append(requirements(items));
    if (!checklist) card.append(node("p", "Checklist unavailable. Submission is blocked until the service can evaluate exact source, target, dependencies, approval and CI facts.", "cr-notice"));
    card.append(node("p", ready ? "The observed checklist is ready. The service must recheck it at candidate preparation and submission." : "Submission is blocked by the listed requirements. No browser control can override them.", "cr-muted"));
    if (c.candidate) facts(card, [["Prepared candidate", c.candidate.id], ["Candidate revision", c.candidate.revision], ["Commit", c.candidate.commit], ["Expected target", c.candidate.target], ["CI run", c.candidate.ci_run]]);
    // One primary action per view: submission when the checklist is ready, otherwise review publication.
    const actions = node("div", null, "cr-actions");
    actions.append(writeButton("Prepare candidate", async () => { requireCurrent(); await command({ type: "prepare_submit" }, c); }, ready && !c.candidate ? "primary" : undefined));
    if (c.candidate) actions.append(writeButton("Submit change…", () => { requireCurrent(); return command({ type: "submit", candidate_id: c.candidate.id }, c, { title: `Submit "${c.title}" to ${c.candidate.target_ref || c.target_ref}?`, verb: "Submit change", facts: [["Principal", state.context.principal], ["Repository", c.repository], ["Change", c.id], ["Candidate", c.candidate.id], ["Source revision", c.candidate.revision], ["Source commit", c.candidate.commit], ["Target ref", c.candidate.target_ref], ["Expected target commit", c.candidate.target], ["Dependencies", JSON.stringify(c.candidate.dependencies)]] }); }, ready ? "primary" : undefined));
    card.append(actions);
    const votes = node("div", null, "cr-subsection"); votes.append(node("h4", "Votes"));
    if (arr(c.votes).length) {
      const list = node("ul", null, "cr-rows");
      for (const vote of c.votes) { const li = node("li", null, "list-row"), live = vote.revision === current()?.id; li.append(principalNode(vote.actor, "cr-row-title cr-principal"), pill(vote.value, "Vote"), exactId(vote.revision), chip(live ? "Current" : "Stale; no current authority", live ? "" : "cr-chip-stale")); if (vote.agent) li.append(chip("Agent")); list.append(li); }
      votes.append(list);
      if (arr(c.votes).some(vote => vote.agent)) votes.append(node("p", state.detail.agent_votes === "binding" ? "Agent votes are binding under this policy." : "Agent votes are advisory under this policy.", "cr-muted"));
    } else votes.append(node("p", "No votes recorded.", "cr-muted"));
    card.append(votes);
    drawAdvisory(card, c);
    const rules = disclosure(card, "Review policy and ownership rules");
    facts(rules, [["Policy", policy?.id], ["Eligible reviewers", arr(policy?.eligible_reviewers).join(", ")], ["Required approvals", policy?.minimum_approvals], ["Required CI", policy ? String(policy.require_ci) : "Policy unavailable; submit is blocked"]]);
    const agents = arr(state.snapshot.agent_principals), agentMetadata = state.snapshot.agent_metadata || {};
    if (agents.length) facts(rules, [["Declared agents", agents.map(principal => { const identity = agentMetadata[principal]; return `${principal}${identity?.harness ? ` (${identity.harness}${identity.model ? ` / ${identity.model}` : ""})` : ""}`; }).join("; ")]]);
    if (arr(policy?.ownership?.rules).length) {
      rules.append(node("h4", "Configured ownership and protected target rules (checklist remains authority)"));
      for (const rule of policy.ownership.rules) { const article = node("article", null, "cr-operation"); facts(article, [["Rule", rule.id], ["Project", rule.project || "unassigned"], ["Path patterns", arr(rule.paths).join(", ")], ["Interface patterns", arr(rule.interface).join(", ") || "none"], ["Protected targets", arr(rule.target_refs).join(", ") || "All admitted targets"], ["Owners", arr(rule.owners).join(", ")], ["Teams", arr(rule.teams).map(team => `${team}: ${arr(policy.ownership.teams?.[team]).join(", ")}`).join("; ")]]); rules.append(article); }
    }
  }
  const advisoryRoute = value => value === true ? "with" : value === false ? "without" : "";
  const agentRoute = value => value === true ? "with" : value === false ? "without" : "";
  // r[impl kiln.review.advisory.view] A separate, labelled, non-binding list; never part of the checklist above.
  function drawAdvisory(card, c) {
    const box = node("div", null, "cr-subsection cr-advisory"), live = current()?.id, records = arr(c.advisory), me = state.context.principal;
    const now = records.filter(advice => advice.revision === live), historical = records.length - now.length;
    box.append(node("h4", "Advisory approvals (non-binding)"), node("p", "Readers other than the owner and authors may advise. Advisory approvals never count toward required approvals, ownership, vetoes, CI or queue admission.", "cr-muted"));
    if (now.length) { const list = node("ul", null, "cr-rows"); list.setAttribute("aria-label", "Advisory approvals of the current revision"); for (const advice of now) { const li = node("li", null, "list-row"); li.append(principalNode(advice.actor, "cr-row-title cr-principal"), chip("Advises approval", "cr-chip-advisory"), when(advice.created_ms)); list.append(li); } box.append(list); }
    else box.append(node("p", "No advisory approvals on the current revision.", "cr-muted"));
    if (historical) box.append(node("p", `${historical} historical advisory approval${historical === 1 ? "" : "s"} on earlier revisions; no current meaning.`, "cr-muted cr-advisory-history"));
    if (c.state === "open") {
      const mine = now.some(advice => advice.actor === me), own = me === c.owner || arr(c.revisions).some(r => r.author === me), verb = mine ? "Withdraw advisory approval" : "Advise approval";
      const action = advisoryButton(`${verb}…`, () => { requireCurrent(); return command({ type: mine ? "advisory_withdraw" : "advisory_approve" }, c, { title: `${verb} on "${c.title}"?`, verb, facts: [["Principal", me], ["Change", c.id], ["Exact current revision", live], ["Effect", "Non-binding. Never counts toward required approvals, ownership, vetoes, CI or queue admission. A new revision makes it historical."]] }, true); }, "quiet");
      action.prepend(icon("circle-check"));
      if (own) { action.disabled = true; action.dataset.crBlocked = "true"; action.title = "Owners and authors may not advise on their own change."; }
      box.append(action);
    }
    card.append(box);
  }
  function drawStack(root) {
    const c = state.detail.change, dependencies = arr(c.dependencies), dependents = arr(state.detail.stack?.dependents), card = panel(root, "Stack and exact dependencies", "stack", dependencies.length + dependents.length > 0);
    about(card, "About stack submission", "Dependencies must be submitted before this change. Follow the links in order, prepare and confirm each candidate independently; target movement requires a fresh checklist. No unchecked batch submission is sent.");
    const list = node("ol", null, "cr-stack"); for (const dep of dependencies) { const li = node("li"); const related = arr(state.detail.stack?.dependencies).find(item => item.id === dep.change); li.append(changeLink(dep.change, related?.title || dep.change), exactId(dep.revision), related ? pill(related.state, "Dependency state") : node("span", "Dependency detail unavailable", "cr-muted")); list.append(li); } const self = node("li"); self.append(node("strong", c.title), pill(c.state, "Change state"), node("span", "Selected change", "cr-muted")); self.setAttribute("aria-current", "true"); list.append(self); card.append(list);
    const required = node("div", null, "cr-tags"); required.append(node("span", "Required by:", "cr-muted")); for (const dep of dependents) required.append(changeLink(dep.id, dep.title)); if (!dependents.length) required.append(node("span", "No dependents observed", "cr-muted")); card.append(required);
    card.append(writeButton("Submit stack in order…", orderedStack));
    const edit = disclosure(card, "Edit exact dependency revisions"); const initial = dependencies.map(dep => `${dep.change} ${dep.revision}`).join("\n"); const input = field(edit, "One stable change ID and exact review revision ID per line", "textarea", initial, [], "dependencies");
    edit.append(writeButton("Save dependencies", () => { requireCurrent(); const parsed = input.value.split("\n").filter(line => line.trim()).map(line => { const parts = line.trim().split(/\s+/); if (parts.length !== 2) throw new Error("Every dependency row must contain exactly a change ID and revision ID."); return { change: parts[0], revision: parts[1] }; }); return command({ type: "set_dependencies", dependencies: parsed }, c); }));
  }
  async function orderedStack() {
    const epoch = state.epoch, selectedId = selected(), context = state.context;
    const { dialog } = modal(`Submit stack for "${state.detail?.change?.title || selectedId}" in order`); const body = node("div", null, "cr-fields"); dialog.append(body);
    const live = () => active(epoch, selectedId) && dialog.isConnected;
    const redraw = async () => {
      const visited = new Map(), visiting = new Set(), ordered = [];
      const visit = async (id, expected) => {
        if (!live()) return;
        if (visiting.has(id)) throw new Error("Dependency cycle observed. No stack effect was sent.");
        if (visited.has(id)) { if (expected && visited.get(id) !== expected) throw new Error(`Stack has conflicting exact dependency revisions for ${id}. No stack effect was sent.`); return; }
        visiting.add(id); const detail = await context.api(path(id));
        if (!live()) return;
        const change = detail.change, rev = arr(change.revisions).at(-1);
        if (!change || change.id !== id || (expected && rev?.id !== expected)) throw new Error(`Dependency ${id} no longer has the required exact revision. Refresh and update dependencies before proceeding.`);
        for (const dep of arr(change.dependencies)) await visit(dep.change, dep.revision);
        visiting.delete(id); visited.set(id, rev?.id); ordered.push(detail);
      };
      await visit(selectedId);
      if (!live()) return;
      body.replaceChildren(node("p", "Each step is freshly evaluated and requires its own exact source/target confirmation. A completed step is never blindly retried. Rebase or update later changes if target movement blocks them.", "cr-muted"));
      const list = node("ol", null, "cr-stack"); for (const detail of ordered) { const c = detail.change, li = node("li"); li.append(node("span", c.title), pill(c.state, "Change state"), exactId(c.id)); if (c.id === selectedId) li.setAttribute("aria-current", "true"); list.append(li); } body.append(list);
      const next = ordered.find(detail => detail.change.state !== "submitted");
      if (!next) { body.append(node("p", "All changes in this exact stack are observed submitted.", "cr-notice")); return; }
      body.append(node("h3", `Next: ${next.change.title}`)); facts(body, [["Change", next.change.id], ["Source commit", arr(next.change.revisions).at(-1)?.commit], ["Target ref", next.change.target_ref], ["Observed target", next.source?.current_target]]);
      body.append(requirements(next.checklist?.items));
      const advance = writeButton("Prepare and submit next…", async () => {
        if (!live()) return;
        const fresh = await context.api(path(next.change.id));
        if (!live()) return;
        if (!fresh.checklist?.ready) throw new Error(`Stack step ${fresh.change.id} is blocked: ${arr(fresh.checklist?.items).filter(item => !item.satisfied).map(item => item.detail).join("; ") || "exact checklist unavailable"}`);
        const prepared = await command({ type: "prepare_submit" }, fresh.change);
        if (!live() || !prepared) return;
        const preparedDetail = await context.api(path(fresh.change.id));
        if (!live()) return;
        const c = preparedDetail.change, candidate = c?.candidate;
        if (!candidate) throw new Error("The service returned no exact prepared candidate. No submission was sent.");
        const submitted = await command({ type: "submit", candidate_id: candidate.id }, c, { title: `Submit "${c.title}" to ${candidate.target_ref || c.target_ref}?`, verb: "Submit change", facts: [["Principal", context.principal], ["Repository", c.repository], ["Change", c.id], ["Candidate", candidate.id], ["Source revision", candidate.revision], ["Source commit", candidate.commit], ["Target ref", candidate.target_ref], ["Expected target commit", candidate.target], ["Dependencies", JSON.stringify(candidate.dependencies)]] });
        if (submitted && live()) await redraw();
      }, "primary");
      if (!next.checklist?.ready) { advance.disabled = true; advance.dataset.crBlocked = "true"; advance.title = "The exact next-step checklist is blocked. Resolve the listed requirements, then refresh stack observations."; }
      body.append(advance);
      body.append(button("Refresh stack", redraw));
    };
    body.append(node("p", "Loading exact dependency order…", "cr-muted"));
    try { await redraw(); } catch (error) { if (live()) { body.replaceChildren(node("p", message(error), "cr-notice")); throw error; } }
  }
  function drawAuthorActions(root) {
    const c = state.detail.change, card = panel(root, "Author and provider actions", "author");
    const edit = disclosure(card, "Edit change description and work-in-progress state");
    const title = field(edit, "Title", "text", c.title, [], "edit-title"), description = field(edit, "Description", "textarea", c.description, [], "edit-description");
    edit.append(writeButton("Save description", async () => { requireCurrent(); const result = await command({ type: "edit", title: title.value, description: description.value }, c); if (result) clearFields("edit-title", "edit-description"); }));
    edit.append(writeButton(c.wip ? "Mark ready for review…" : "Mark work in progress…", () => command({ type: "set_wip", wip: !c.wip }, c, { title: c.wip ? `Mark "${c.title}" ready for review?` : `Mark "${c.title}" as work in progress?`, verb: c.wip ? "Mark ready" : "Mark work in progress", facts: [["Change", c.id], ["Revision", current()?.id], ["WIP", String(!c.wip)]] })));
    const upload = disclosure(card, "Upload a new exact revision"); const commit = field(upload, "Exact local commit", "text", "", [], "upload-commit"); const authorship = authorshipFields(upload, "upload");
    upload.append(writeButton("Upload revision…", async () => { requireCurrent(); if (!commit.value.trim()) throw new Error("An exact local commit is required."); const declared = authorship.value(); const result = await command(withAuthorship({ type: "upload", commit: commit.value.trim() }, declared), c, { title: `Upload a new revision to "${c.title}"?`, verb: "Upload revision", facts: [["Change", c.id], ["Repository", c.repository], ["Previous revision", current()?.id], ["Source commit", commit.value.trim()], ["Target ref", c.target_ref], authorshipFact(declared)] }); if (result) { clearFields("upload-commit", ...authorship.keys); route({ review_revision: "", review_base: "base", review_file: "", review_side: "", review_line: "" }); } }));
    const lifecycle = node("div", null, "cr-toolbar");
    if (c.state === "open" || c.state === "abandoned") { const type = c.state === "abandoned" ? "restore" : "abandon", verb = `${human(type)} change`; lifecycle.append(writeButton(`${verb}…`, () => command({ type }, c, { title: `${verb} "${c.title}"?`, verb, danger: type === "abandon", facts: [["Change", c.id], ["Source revision", current()?.id], ["Commit", current()?.commit], ["Target ref", c.target_ref]] }), type === "abandon" ? "danger" : undefined)); } card.append(lifecycle);
    const provider = disclosure(card, "Rebase, revert or cherry-pick through the admitted provider");
    provider.append(node("p", `Provider capabilities: ${arr(state.snapshot.capabilities).join(", ") || "No capabilities reported"}. These actions create review revisions or new changes, not unchecked target updates. Unsupported operations are rejected visibly by the service.`, "cr-muted"));
    const OPERATIONS = { rebase: "Rebase", revert: "Revert", cherry_pick: "Cherry-pick" };
    const operation = field(provider, "Operation", "select", "rebase", Object.entries(OPERATIONS), "maintenance-kind");
    const target = field(provider, "Target ref (cherry-pick)", "select", c.target_ref, arr(state.snapshot.target_refs), "maintenance-target");
    const resultTitle = field(provider, "New change title (revert / cherry-pick)", "text", `Revert ${c.title}`, [], "maintenance-title");
    const run = writeButton(`${OPERATIONS[operation.value] || "Run"}…`, async () => {
      requireCurrent(); const epoch = state.epoch, id = selected(), r = current(); const kind = operation.value, targetRef = kind === "cherry_pick" ? target.value : c.target_ref;
      const observation = await state.context.api(`${ROOT}/targets?${new URLSearchParams({ commit: r.commit, target_ref: targetRef })}`);
      if (!active(epoch, id)) return;
      const expected = observation?.source?.current_target;
      if (!expected) throw new Error("The exact current target could not be observed. No provider action was sent.");
      const action = { type: kind, expected_target: expected }; if (kind !== "rebase") action.title = resultTitle.value; if (kind === "cherry_pick") action.target_ref = targetRef;
      const verb = OPERATIONS[kind] || label(kind), title = kind === "rebase" ? `Rebase "${c.title}" onto ${targetRef}?` : kind === "revert" ? `Revert "${c.title}" as a new change?` : `Cherry-pick "${c.title}" onto ${targetRef}?`;
      const result = await command(action, c, { title, verb, facts: [["Principal", state.context.principal], ["Repository", c.repository], ["Change", c.id], ["Source revision", r.id], ["Source commit", r.commit], ["Target ref", targetRef], ["Expected target commit", expected], ["Result", kind === "rebase" ? "New revision on this review; target unchanged" : `New change: ${resultTitle.value}; target unchanged`]] });
      const resultId = result?.operation?.result_change || result?.change?.id;
      if (resultId && resultId !== c.id) route({ review_change: resultId, review_revision: "", review_base: "", review_file: "", review_side: "", review_line: "" });
    });
    operation.addEventListener("change", () => { run.textContent = `${OPERATIONS[operation.value] || "Run"}…`; }); provider.append(run);
  }
  async function loadDiff() {
    const epoch = state.epoch, id = selected(), rev = revision(), base = state.route.review_base || "base", inter = interdiffRoute(), range = commitsRoute(), request = ++state.diffRequest;
    if (!arr(state.detail?.change?.revisions).some(r => r.id === rev)) return;
    try {
      const params = { revision: rev, whitespace: state.route.review_whitespace || "exact", highlight: "1" };
      if (inter) { params.interdiff = inter.from; if (state.route.review_interdiff_change) params.change = state.route.review_interdiff_change; }
      else if (range) params.commits = `${range.from}..${range.to}`;
      else params.base = base;
      if (state.route.review_mode === "structural") params.mode = "structural";
      const diff = await state.context.api(`${path(id)}/diff?${new URLSearchParams(params)}`);
      if (!active(epoch, id) || request !== state.diffRequest || rev !== revision()) return;
      const revisions = arr(state.detail.change.revisions), selectedRevision = revisions.find(r => r.id === rev);
      if (!Array.isArray(diff?.files) || !diffMatches(diff, selectedRevision, inter ? revisions.find(r => r.id === inter.from) : null, range, base === "base" ? null : revisions.find(r => r.id === base))) throw new Error("Diff identity does not match the exact selected revision and comparison base.");
      state.diff = diff; state.diffError = ""; draw();
    } catch (error) { if (active(epoch, id) && request === state.diffRequest) { state.diff = null; state.diffError = message(error); drawDiff(); } }
  }
  // Pure: the response names exactly the requested comparison (narrowed interdiffs name their revisions instead of commits).
  function diffMatches(diff, selectedRevision, from, range, earlier) {
    if (!selectedRevision) return false;
    if (from) return diff.interdiff?.from === from.id && diff.interdiff?.to === selectedRevision.id && (diff.interdiff.change ? true : diff.old_commit === from.commit && diff.new_commit === selectedRevision.commit);
    if (range) return diff.old_commit === range.from && diff.new_commit === range.to;
    return diff.new_commit === selectedRevision.commit && diff.old_commit === (earlier ? earlier.commit : selectedRevision.base);
  }
  // r[impl kiln.review.structural.scenario.anchors]
  // Pure: the exact comparison base an anchor binds on `side`, or null where the view has no exact revision coordinates
  // (the in-memory rebased old side of an interdiff, or a narrowed/ranged view that does not end at the revision).
  // Line and structural modes share the displayed comparison, so their anchors are equal for the same lines.
  function anchorBaseFor(diff, selectedRevision, side) {
    if (!diff || !selectedRevision || diff.new_commit !== selectedRevision.commit) return null;
    if (diff.interdiff?.status !== "exact" && !diff.commit_range) return diff.old_commit;
    if (side === "revision") return selectedRevision.base;
    return diff.commit_range && diff.old_commit === selectedRevision.base ? diff.old_commit : null;
  }
  function anchorFor(diff, selectedRevision, file, side, first, last) {
    const base = anchorBaseFor(diff, selectedRevision, side);
    return base == null ? null : { revision: selectedRevision.id, base_commit: base, file, side, start_line: first, end_line: last };
  }
  const selectedRevisionOf = () => arr(state.detail?.change?.revisions).find(r => r.id === revision());
  // r[impl kiln.review.hunks] Private hunk progress of this principal for the displayed file.
  const hunkEntry = file => arr(state.diff?.review_hunks?.files).find(entry => entry.path === filePath(file)) || null;
  const hunkBlocked = file => revision() !== current()?.id ? "Select the current revision to change viewed hunks." : file.binary || file.truncated || state.diff?.truncated ? "Only complete textual files can be marked viewed." : !hunkEntry(file)?.total ? "This file has no observed hunks to mark." : "";
  // Toggle exposed to keyboard shortcuts (#9): `.cr-hunk-mark[data-hunk-digest][data-file]`, `aria-pressed`.
  function hunkToggle(file, index) {
    const entry = hunkEntry(file); if (!state.diff?.review_hunks?.exact_base || !entry || entry.hunks.length !== arr(file.hunks).length) return null;
    const hunk = entry.hunks[index], tools = node("span", null, "cr-hunk-tools"); if (!hunk) return null;
    const mark = writeButton("Viewed", () => { requireCurrent(); return command({ type: "mark_hunk", file: filePath(file), hunk_digest: hunk.digest, reviewed: !hunk.viewed }); }, "quiet cr-hunk-mark");
    mark.dataset.hunkDigest = hunk.digest; mark.dataset.file = filePath(file); mark.dataset.viewed = String(!!hunk.viewed); mark.setAttribute("aria-pressed", String(!!hunk.viewed)); mark.setAttribute("aria-label", `Viewed: hunk ${index + 1} of ${filePath(file)}`); mark.prepend(icon("circle-check"));
    const blocked = hunkBlocked(file); if (blocked) { mark.disabled = true; mark.dataset.crBlocked = "true"; mark.title = blocked; }
    tools.append(mark); if (hunk.new_since_viewed) tools.append(chip(`New since revision ${state.diff.review_hunks.last_viewed?.number ?? "?"}`, "cr-new-hunk"));
    return tools;
  }
  const hunkApi = Object.freeze({
    list: () => [...(state.container?.querySelectorAll(".cr-hunk-mark") || [])].map(b => ({ file: b.dataset.file, digest: b.dataset.hunkDigest, viewed: b.dataset.viewed === "true", enabled: !b.disabled })),
    toggle: digest => { const b = [...(state.container?.querySelectorAll(".cr-hunk-mark") || [])].find(item => item.dataset.hunkDigest === digest); if (!b || b.disabled) return false; b.click(); return true; },
  });
  // r[impl kiln.review.interdiff] Interdiff and commit-range notices plus their narrowing controls.
  function comparisonNotices(host, comparison) {
    const listed = comparison.revision_commits, commits = arr(listed?.commits), inter = comparison.interdiff;
    if (inter) {
      const scope = inter.change ? ` narrowed to change ${inter.change}` : "";
      const text = inter.status === "exact" ? `Changes since revision ${inter.from_number}${scope}: revision ${inter.from_number} was rebased onto revision ${inter.to_number}'s base in memory, so base movement is removed. Only revision-side lines can be commented on.` : `Interdiff unavailable (${label(inter.reason)}): ${inter.message || "the provider could not compute it exactly"}. Showing the exact commit comparison revision ${inter.from_number} → ${inter.to_number} instead; it includes base movement.`;
      const notice = node("p", text, inter.status === "exact" ? "cr-muted" : "cr-notice"); notice.dataset.interdiff = inter.status; host.append(notice);
      const ids = commits.filter(c => c.change_id);
      if (ids.length || inter.change) {
        const bar = node("div", null, "toolbar cr-toolbar"), narrow = field(bar, "Narrow to change", "select", inter.change || "", [["", "Whole revision"], ...ids.map(c => [c.change_id, `${c.change_id.slice(0, 12)} · ${c.summary || short(c.commit)}`]), ...(inter.change && !ids.some(c => c.change_id === inter.change) ? [[inter.change, inter.change]] : [])]);
        narrow.dataset.crKey = "diff-narrow"; narrow.addEventListener("change", () => route({ ...viewPatch(), review_interdiff_change: narrow.value, review_file: "", review_side: "", review_line: "" })); host.append(bar);
      }
    }
    if (comparison.commit_range) { const n = node("p", `Commit range ${short(comparison.commit_range.from)} → ${short(comparison.commit_range.to)} inside the selected revision.${comparison.commit_range.anchorable ? "" : " The range ends before the revision head, so comments are disabled here."}`, "cr-muted"); n.dataset.commitRange = "true"; host.append(n); }
    // r[impl kiln.review.commit_range] Any exact from..to range of a multi-commit revision.
    if (!inter && commits.length > 1) {
      const r = selectedRevisionOf(), range = commitsRoute(), bar = node("div", null, "toolbar cr-toolbar cr-commit-range");
      const starts = [[r?.base, `Revision base · ${short(r?.base)}`], ...commits.slice(0, -1).map((c, i) => [c.commit, `After commit ${i + 1} · ${c.summary || short(c.commit)}`])];
      const ends = commits.map((c, i) => [c.commit, `Commit ${i + 1} · ${c.summary || short(c.commit)}`]);
      const from = field(bar, "Commit range from", "select", range?.from || r?.base, starts), to = field(bar, "Commit range to", "select", range?.to || commits.at(-1).commit, ends);
      const apply = () => { const fi = commits.findIndex(c => c.commit === from.value), ti = commits.findIndex(c => c.commit === to.value); const end = ti > fi ? to.value : commits[fi + 1].commit; route(end === r?.commit && from.value === r?.base ? { review_revision: revision(), review_base: "base", review_file: "", review_side: "", review_line: "" } : { review_revision: revision(), review_interdiff: "", review_interdiff_change: "", review_base: "", review_commits: `${from.value}..${end}`, review_file: "", review_side: "", review_line: "" }); };
      from.addEventListener("change", apply); to.addEventListener("change", apply); host.append(bar);
      if (listed?.incomplete) host.append(node("p", "The commit list of this revision is incomplete (non-linear history or the listing bound).", "cr-muted"));
    }
  }
  // r[impl kiln.review.structural.scenario.fallback] A labelled line diff whenever the service could not match trees.
  function structuralOf(host, file) {
    if (state.route.review_mode !== "structural") return null;
    const view = arr(state.diff?.structural?.files).find(f => f.path === filePath(file));
    if (view?.status === "structural") return view;
    const n = node("p", `Structural view unavailable for this file (${label(view?.reason || "not_computed")}); showing the plain line diff.`, "cr-notice"); n.dataset.structural = "fallback"; host.append(n); return null;
  }
  function novelCell(text, spans) {
    const td = node("td", null, "cr-code"), code = node("code", null, "cr-code-text"), value = String(text ?? ""); let at = 0;
    for (const [start, end] of arr(spans)) { if (start > at) code.append(value.slice(at, start)); code.append(node("mark", value.slice(start, end), "cr-novel")); at = end; }
    code.append(value.slice(at)); td.append(code); return td;
  }
  function appendStructural(body, view, coordinate) {
    const note = text => { const tr = node("tr", null, "cr-hunk cr-structural-note"), th = node("th", text); th.colSpan = 4; tr.append(th); body.append(tr); };
    if (!arr(view.hunks).length && !arr(view.moved).length) note("No syntax changes: this file differs only in formatting or layout.");
    for (const m of arr(view.moved)) note(`Moved without syntax changes: base lines ${m.old_start}–${m.old_end} → revision lines ${m.new_start}–${m.new_end}. Commenting is disabled for this region; switch to the line diff to comment on it.`);
    for (const hunk of arr(view.hunks)) {
      note("Structural changes");
      for (const row of arr(hunk.rows)) {
        const kind = { old: "deletion", new: "addition", both: "context" }[row.side] || "context", tr = node("tr", null, `cr-diff-${kind}`);
        if (state.mode === "split") tr.append(coordinate(row.old_line, "base"), row.old_line == null ? codeCell("") : novelCell(row.text, row.side === "old" ? row.novel : []), coordinate(row.new_line, "revision"), row.new_line == null ? codeCell("") : novelCell(row.text, row.side === "new" ? row.novel : []));
        else tr.append(coordinate(row.old_line, "base"), coordinate(row.new_line, "revision"), node("td", ({ addition: "+", deletion: "−", context: " " })[kind], "cr-diff-sign"), novelCell(row.text, row.novel));
        body.append(tr);
      }
    }
  }
  function filePath(file) { return file.new_path || file.old_path || ""; }
  // r[impl kiln.review.noise.display] Display-only: the service names the first matching noise rule per displayed path.
  // The pre-viewed state is derived here at display time and never stored or sent as a marker.
  function noiseRule(file) { const id = state.diff?.noise?.files?.[filePath(file)]; return id ? arr(state.diff.noise.rules).find(rule => rule.id === id) || { id, reason: "" } : null; }
  function defaultFile(comparison) { const first = arr(comparison.files).find(file => !noiseRule(file)); return first ? filePath(first) : ""; }
  function noiseGroup(files, list, comparison) {
    const rows = [...list.children], hidden = [];
    arr(comparison.files).forEach((file, index) => { const rule = noiseRule(file); if (rule && rows[index]) hidden.push({ row: rows[index], rule, file }); });
    if (!hidden.length) return;
    const ids = [...new Set(hidden.map(item => item.rule.id))], key = `${comparison.old_commit}..${comparison.new_commit}`;
    const group = node("details", null, "cr-noise"), noiseList = node("ul", null, "cr-file-list cr-noise-list"); noiseList.setAttribute("aria-label", "Noise files");
    group.open = state.noiseOpen === key || hidden.some(item => filePath(item.file) === state.route.review_file);
    group.addEventListener("toggle", () => { state.noiseOpen = group.open ? key : ""; });
    group.append(ui().disclosureSummary(`${hidden.length} noise file${hidden.length === 1 ? "" : "s"} hidden · ${ids.join(", ")}`, "eye"));
    for (const { row, rule } of hidden) { const tag = chip(rule.id, "cr-noise-rule"); tag.title = rule.reason ? `Noise rule ${rule.id}: ${rule.reason}` : `Noise rule ${rule.id}`; row.dataset.noise = rule.id; row.insertBefore(tag, row.children[1] || null); noiseList.append(row); }
    const note = node("p", "Display only: shown as viewed for everyone. Ownership, approvals, threads, CI and queue admission are unchanged. Rules ", "cr-muted cr-noise-note"); note.append(exactId(state.diff.noise.identity));
    group.append(note, noiseList); files.append(group);
  }
  function anchorRoute(anchor) {
    const revisions = arr(state.detail?.change?.revisions), r = revisions.find(item => item.id === anchor.revision);
    const base = anchor.base_commit === r?.base ? "base" : revisions.find(item => item.commit === anchor.base_commit)?.id;
    return { review_revision: anchor.revision, review_base: base || "base", review_file: anchor.file, review_side: anchor.side, review_line: String(anchor.start_line) };
  }
  function drawDiff() {
    const host = state.container?.querySelector(".cr-diff-host"); if (!host) return; host.replaceChildren();
    if (state.diffError) { host.append(node("p", state.diffError, "cr-notice")); return; }
    if (!state.diff) { host.append(loading("Loading the exact provider diff…")); return; }
    const comparison = state.diff, exactWhitespace = (state.route.review_whitespace || "exact") === "exact";
    facts(host, [["Old commit", comparison.old_commit], ["New commit", comparison.new_commit]]).classList.add("cr-facts-compact");
    if (!exactWhitespace) host.append(node("p", "Whitespace-filtered comparison: omitted changes are not reviewed. Displayed line numbers and saved anchors remain exact provider coordinates; switch to exact to inspect all whitespace.", "cr-notice"));
    if (comparison.truncated) host.append(node("p", "The provider bounded this diff. Not all files or lines are shown; omitted content is not implicitly reviewed.", "cr-notice"));
    comparisonNotices(host, comparison);
    // Server tokens address every DiffLine in order (files → hunks → lines), relative to its text.
    const diffLines = comparison.files.flatMap(f => arr(f.hunks).flatMap(h => arr(h.lines)));
    const highlight = ui().highlightLines(comparison.highlight, diffLines.map(line => typeof line?.text === "string" ? line.text : ""));
    const fragmentsOf = new Map(highlight.fragments ? diffLines.map((line, index) => [line, highlight.fragments[index]]) : []);
    if (highlight.note) { const note = node("p", highlight.note, highlight.state === "invalid" ? "cr-notice" : "cr-muted"); note.dataset.highlight = highlight.state; host.append(note); }
    const progress = file => hunkEntry(file), fullyViewed = file => (progress(file)?.total || 0) > 0 && progress(file).viewed === progress(file).total;
    const preViewed = file => typeof noiseRule === "function" && !!noiseRule(file);
    const files = node("nav", null, "cr-files"), list = node("ul", null, "cr-file-list"), shownFile = state.route.review_file || defaultFile(comparison); files.setAttribute("aria-label", "Changed files");
    files.append(node("p", `${comparison.files.length} changed file${comparison.files.length === 1 ? "" : "s"} · ${comparison.files.filter(fullyViewed).length} fully viewed by you`, "cr-files-head cr-muted"));
    for (const file of comparison.files) {
      const name = filePath(file), row = node("li", null, "cr-file-row"); const a = link(`${file.renamed ? `${file.old_path} → ` : ""}${name}${file.binary ? " · binary" : ""}${file.truncated ? " · truncated" : ""}`, { review_file: name, review_side: "", review_line: "" });
      if (name === shownFile) a.setAttribute("aria-current", "true");
      // r[impl kiln.web.design_system] A toggle: constant label, persistent pressed state, file named in its accessible name.
      // r[impl kiln.review.hunks] "Mark all" writes one private marker per hunk; the count is this principal's progress.
      const entry = progress(file), marked = fullyViewed(file), mark = writeButton("Viewed", () => { requireCurrent(); return command({ type: "mark_reviewed", file: name, reviewed: !marked }); }, "quiet cr-mark");
      mark.dataset.reviewed = String(marked); mark.dataset.hunksTotal = String(entry?.total ?? 0); mark.dataset.hunksViewed = String(entry?.viewed ?? 0); mark.setAttribute("aria-pressed", String(marked)); mark.setAttribute("aria-label", `Viewed, all hunks: ${name}`); mark.prepend(icon("circle-check"));
      const blocked = hunkBlocked(file); if (blocked) { mark.disabled = true; mark.dataset.crBlocked = "true"; mark.title = blocked; }
      const count = node("span", preViewed(file) ? "Noise · shown as viewed" : entry ? `${entry.viewed} of ${entry.total} hunk${entry.total === 1 ? "" : "s"} viewed${entry.new_since_viewed ? ` · ${entry.new_since_viewed} new since revision ${comparison.review_hunks?.last_viewed?.number ?? "?"}` : ""}` : "", "cr-muted cr-hunk-count");
      row.append(a, count, mark); list.append(row);
    }
    files.append(list); noiseGroup(files, list, comparison); host.append(files);
    if (!comparison.files.length) { host.append(node("p", "The exact comparison contains no changed files.", "cr-muted")); return; }
    const wantedFile = state.route.review_file || defaultFile(comparison);
    if (!wantedFile) { host.append(node("p", "Every changed file matches a display-only noise rule. Expand the noise group to view one.", "cr-muted")); return; }
    const file = comparison.files.find(f => f.new_path === wantedFile || f.old_path === wantedFile);
    if (!file) { host.append(node("p", "The requested file is not present in this exact comparison. Select a listed file.", "cr-notice")); return; }
    const shownNoise = noiseRule(file); if (shownNoise) host.append(node("p", `Noise rule ${shownNoise.id}${shownNoise.reason ? `: ${shownNoise.reason}` : ""}. Expanded on request; it still counts for ownership, approvals, threads and CI.`, "cr-notice cr-noise-note"));
    host.append(node("h4", filePath(file), file.binary ? "cr-file-title" : "sr-only"), node("p", file.binary ? "Binary content: no textual line anchors are available." : "Line numbers are permalinks. Use the anchored draft editor below to discuss an exact line or range.", "cr-muted"));
    if (file.binary) return;
    const scroll = node("div", null, "cr-diff-scroll"); scroll.tabIndex = 0; scroll.setAttribute("role", "region"); scroll.setAttribute("aria-label", `${state.mode === "split" ? "Side-by-side" : "Unified"} diff for ${filePath(file)}`);
    const table = node("table", null, `cr-diff cr-${state.mode}`); const caption = node("caption", `${filePath(file)} · ${short(comparison.old_commit)} → ${short(comparison.new_commit)}`, "sr-only"); table.append(caption);
    // The visible sticky file header repeats the caption, so it is hidden from assistive technology.
    const head = node("thead"), fileRow = node("tr", null, "cr-file-header"), fileCell = node("th"), fileLabel = node("div", null, "cr-file-header-label"); fileRow.setAttribute("aria-hidden", "true"); fileCell.colSpan = 4;
    fileLabel.append(node("span", file.renamed ? `${file.old_path} → ${filePath(file)}` : filePath(file), "cr-file-path"), node("span", `${short(comparison.old_commit)} → ${short(comparison.new_commit)}`, "cr-muted")); fileCell.append(fileLabel); fileRow.append(fileCell); head.append(fileRow);
    // The narrow sign column shows "±" visually; its accessible header stays "Change".
    const hr = node("tr"); for (const text of state.mode === "split" ? ["Base line", "Base content", "Revision line", "Revision content"] : ["Base line", "Revision line", "Change", "Content"]) { const th = node("th", text === "Change" ? null : text); if (text === "Change") { const sign = node("span", "±"); sign.setAttribute("aria-hidden", "true"); th.title = text; th.classList.add("cr-diff-sign"); th.append(sign, node("span", text, "sr-only")); } th.scope = "col"; hr.append(th); } head.append(hr); table.append(head);
    const body = node("tbody");
    const selectedRev = selectedRevisionOf();
    const coordinate = (line, side) => {
      const td = node("td", null, "cr-line-number"); if (line == null) return td;
      // Lines without exact revision coordinates (in-memory rebased side) stay plain text: no anchor can be created.
      if (anchorBaseFor(comparison, selectedRev, side) == null) { const span = node("span", line, "cr-line-plain"); span.title = "Not an exact revision coordinate in this view; switch to an exact comparison to comment."; td.append(span); return td; }
      const a = link(line, { ...viewPatch(), review_file: (side === "base" ? file.old_path : file.new_path) || filePath(file), review_side: side, review_line: String(line) }); a.setAttribute("aria-label", `${side} line ${line}`); if (state.route.review_side === side && Number(state.route.review_line) === line) { a.classList.add("cr-line-selected"); a.setAttribute("aria-current", "location"); } td.append(a); return td;
    };
    const structuralView = structuralOf(host, file); if (structuralView) appendStructural(body, structuralView, coordinate);
    for (const [index, hunk] of (structuralView ? [] : arr(file.hunks)).entries()) {
      const marker = node("tr", null, "cr-hunk"), title = node("th"), heading = node("div", null, "cr-hunk-head"); heading.append(node("span", hunk.header || `@@ -${hunk.old_start},${hunk.old_lines} +${hunk.new_start},${hunk.new_lines} @@`)); const tools = hunkToggle(file, index); if (tools) heading.append(tools); title.append(heading); title.colSpan = 4; marker.append(title); body.append(marker);
      for (const line of arr(hunk.lines)) {
        const tr = node("tr", null, `cr-diff-${line.kind}`);
        const fragments = fragmentsOf.get(line) || null;
        if (state.mode === "split") { tr.append(coordinate(line.old_line, "base"), line.old_line == null ? codeCell("") : codeCell(line.text, fragments), coordinate(line.new_line, "revision"), line.new_line == null && line.kind !== "no_newline" ? codeCell("") : codeCell(line.text, fragments)); }
        else tr.append(coordinate(line.old_line, "base"), coordinate(line.new_line, "revision"), node("td", ({ addition: "+", deletion: "−", context: " ", no_newline: "\\" })[line.kind] || "", "cr-diff-sign"), codeCell(line.text, fragments));
        body.append(tr);
      }
    }
    table.append(body); scroll.append(table); host.append(scroll);
    const target = host.querySelector(".cr-line-selected"); if (target) target.scrollIntoView({ block: "nearest", inline: "nearest" });
    if (file.truncated) host.append(node("p", "This file is truncated. Missing text cannot be anchored, and this display cannot certify the whole file as reviewed.", "cr-notice"));
    const threadBases = [comparison.old_commit, anchorBaseFor(comparison, selectedRev, "revision")]; const related = arr(state.detail.change.threads).filter(thread => thread.anchor.revision === revision() && threadBases.includes(thread.anchor.base_commit) && [file.old_path, file.new_path].includes(thread.anchor.file));
    if (related.length) { const threads = node("ul", null, "cr-thread-links"); threads.setAttribute("aria-label", "Threads on this file"); for (const thread of related) { const line = thread.anchor.start_line, li = node("li"); const summary = link(`${thread.resolved ? "Resolved" : "Open"}${thread.blocking ? " blocking" : ""} thread · ${thread.anchor.side} ${line}–${thread.anchor.end_line}`, anchorRoute(thread.anchor)); summary.classList.add("cr-thread-link"); summary.dataset.resolved = String(!!thread.resolved); summary.dataset.blocking = String(!!thread.blocking && !thread.resolved); li.append(summary); threads.append(li); } host.append(threads); }
  }
  function drawDiscussion(root) {
    const c = state.detail.change, card = section(root, "Discussion and drafts", "cr-discussion");
    const threads = arr(c.threads).filter(thread => thread.anchor.revision === revision());
    for (const thread of threads) {
      const article = node("article", null, "cr-thread"), a = thread.anchor, head = node("div", null, "cr-row-head");
      // r[impl kiln.review.keyboard.navigation] Stable selector for unresolved-thread navigation; the pill keeps the visible state.
      article.dataset.resolved = String(!!thread.resolved);
      // The exact file and range lead; side and comparison base follow as secondary metadata.
      const where = node("div", null, "cr-row-main"), anchorMeta = node("span", `${human(a.side)} side · base `, "cr-muted"); anchorMeta.append(exactId(a.base_commit)); where.append(link(`${a.file}:${a.start_line}–${a.end_line}`, anchorRoute(a), "cr-anchor"), anchorMeta);
      head.append(where, pill(thread.resolved ? "resolved" : "open", "Thread", thread.resolved ? "Resolved" : "Open", thread.resolved ? "success" : thread.blocking ? "warning" : "queued"), chip(thread.blocking ? "Blocking" : "Non-blocking")); article.append(head);
      for (const comment of arr(thread.comments)) { const item = node("div", null, "cr-comment"), byline = node("h4", null, "cr-byline"), author = node("strong"); author.append(principalNode(comment.author)); byline.append(author, " · ", when(comment.created_ms)); item.append(byline, markdown(comment.body)); article.append(item); }
      const actions = node("div", null, "cr-actions");
      const reply = field(article, "Private reply", "textarea", "", [], `reply-${thread.id}`); reply.rows = 2; reply.parentElement.classList.add("cr-reply");
      actions.append(writeButton("Save reply draft", async () => { if (!reply.value.trim()) throw new Error("Reply text is required."); const result = await command({ type: "save_draft", anchor: a, thread: thread.id, body: reply.value, blocking: thread.blocking }, c); if (result) { clearFields(`reply-${thread.id}`); draw(); } }));
      actions.append(writeButton(thread.resolved ? "Unresolve" : "Resolve", () => command({ type: "resolve", thread: thread.id, resolved: !thread.resolved }, c), "quiet")); article.append(actions); card.append(article);
    }
    if (!threads.length) card.append(node("p", "No published threads on the selected revision.", "cr-muted"));
    const compose = disclosure(card, "Write an anchored private draft");
    if (state.route.review_line) { compose.parentElement.open = true; compose.parentElement.dataset.crForceOpen = "true"; }
    compose.append(node("p", "Choose a line number in the diff to seed this editor.", "cr-muted"));
    about(compose, "About anchored drafts and formatting", "Draft text is private until publication. Both sides bind the exact displayed old commit, selected revision and side-specific file path, including historical comparisons.", "Markdown: headings, lists, emphasis, links and fenced code. Raw HTML and image loading are not supported. Mention a configured principal as @principal. To propose an applyable replacement, use a revision-side range and one complete ```suggestion fence containing the replacement text. Surrounding prose is allowed.");
    const choices = state.diff ? [...new Set(state.diff.files.flatMap(f => [f.old_path, f.new_path]).filter(Boolean))] : arr(state.detail.change.revisions).find(r => r.id === revision())?.files || [];
    const grid = node("div", null, "cr-field-grid"); compose.append(grid);
    const file = field(grid, "Exact file", "select", state.route.review_file || choices[0] || "", choices, "draft-file");
    const side = field(grid, "Side", "select", state.route.review_side || "revision", [["base", "Base"], ["revision", "Revision"]], "draft-side");
    const start = field(grid, "First line", "number", state.route.review_line || 1, [], "draft-start"); start.min = "1"; start.step = "1";
    const end = field(grid, "Last line", "number", state.route.review_line || 1, [], "draft-end"); end.min = "1"; end.step = "1";
    const body = field(compose, "Comment", "textarea", "", [], "draft-body"); const blocking = field(compose, "Blocks submission until resolved", "checkbox", true, [], "draft-blocking");
    const preview = node("div", null, "cr-preview"), composeActions = node("div", null, "cr-actions"); compose.append(composeActions, preview);
    composeActions.append(writeButton("Save private draft", async () => {
      if (!state.diff || state.diff.new_commit !== arr(c.revisions).find(r => r.id === revision())?.commit) throw new Error("Wait for the exact comparison before saving an anchored draft. Views that end before the revision head cannot be commented on.");
      if (!body.value.trim()) throw new Error("Comment text is required.");
      const first = Number(start.value), last = Number(end.value); if (!Number.isSafeInteger(first) || !Number.isSafeInteger(last) || first < 1 || last < first) throw new Error("A valid inclusive positive line range is required.");
      const comparisonFile = state.diff.files.find(f => [f.old_path, f.new_path].includes(file.value));
      const exactPath = side.value === "base" ? comparisonFile?.old_path : comparisonFile?.new_path;
      if (!exactPath || comparisonFile.binary || comparisonFile.truncated || state.diff.truncated) throw new Error("The selected side has no complete admitted text for this file. No draft was saved.");
      const anchor = anchorFor(state.diff, arr(c.revisions).find(r => r.id === revision()), exactPath, side.value, first, last);
      if (!anchor) throw new Error("This side of the displayed comparison has no exact revision coordinates (an interdiff's rebased side). Switch to an exact comparison to comment on it. No draft was saved.");
      const result = await command({ type: "save_draft", anchor, thread: null, body: body.value, blocking: blocking.checked }, c);
      if (result) { clearFields("draft-body"); draw(); }
    }), button("Preview Markdown", () => { preview.replaceChildren(markdown(body.value)); }, "quiet"));
    const drafts = section(card, "My drafts", "cr-drafts"); const publishIds = [];
    for (const draft of arr(state.detail.drafts)) {
      const article = node("article", null, "cr-draft"), a = draft.anchor; const currentDraft = a.revision === current()?.id;
      const draftHead = node("div", null, "cr-row-head"), draftMeta = node("p", "Comment revision ", "cr-muted"); draftMeta.append(exactId(a.revision), " · comparison base ", exactId(a.base_commit));
      draftHead.append(node("h4", `${draft.thread ? "Reply" : "New thread"} · ${a.file}:${a.start_line}–${a.end_line} (${a.side})${currentDraft ? "" : " · Historical revision"}`), chip("Private")); article.append(draftHead, draftMeta);
      const include = field(article, "Include in atomic publication", "checkbox", a.revision === revision(), [], `publish-${draft.id}`);
      publishIds.push({ id: draft.id, input: include, anchor: a });
      const draftBody = field(article, "Private draft text", "textarea", draft.body, [], `draft-edit-${draft.id}`);
      const actions = node("div", null, "cr-actions"); actions.append(writeButton("Save draft edits", async () => { const result = await command({ type: "save_draft", id: draft.id, anchor: a, thread: draft.thread, body: draftBody.value, blocking: draft.blocking }, c); if (result) { clearFields(`draft-edit-${draft.id}`); draw(); } }), writeButton("Delete draft…", () => command({ type: "delete_draft", id: draft.id }, c, { title: `Delete private draft on ${a.file}:${a.start_line}–${a.end_line}?`, verb: "Delete draft", danger: true, facts: [["Change", c.id], ["Draft", draft.id], ["Anchor", `${a.file}:${a.start_line}–${a.end_line}`], ["Body", draft.body]] }), "danger")); article.append(actions); drafts.append(article);
    }
    if (!arr(state.detail.drafts).length) drafts.append(node("p", "No server-saved private drafts for this principal on this change.", "cr-muted"));
    const publish = section(card, "Publish review", "cr-publish");
    const eligible = arr(state.snapshot.policy?.eligible_reviewers).includes(state.context.principal) && c.owner !== state.context.principal && c.reviewers?.[state.context.principal] === "reviewer";
    const vote = field(publish, "Vote on the exact current revision", "select", "", eligible ? [["", "No vote"], ["approve", "Approve"], ["needs_revision", "Needs revision"], ["veto", "Veto"], ["abstain", "Abstain"]] : [["", "No vote — requires an eligible requested non-owner reviewer"]], "publish-vote");
    const scope = node("p", "Only saved, selected drafts are published at their own original revision and comparison base. Unsaved editor changes are not included. An optional vote applies only to CURRENT revision ", "cr-muted"); scope.append(exactId(current()?.id), " (", exactId(current()?.commit), "), independently of historical comments."); publish.append(scope);
    publish.append(writeButton("Publish review…", async () => {
      const chosen = publishIds.filter(item => item.input.checked), ids = chosen.map(item => item.id);
      if (!ids.length && !vote.value) throw new Error("Select at least one saved draft or an eligible vote.");
      const savedText = chosen.map(item => `${item.id}\n${arr(state.detail.drafts).find(draft => draft.id === item.id)?.body ?? "Draft unavailable; refresh before publication"}`).join("\n\n");
      const result = await command({ type: "publish", draft_ids: ids, vote: vote.value || null }, c, { title: `Publish review on "${c.title}"?`, verb: "Publish review", facts: [
        ["Principal", state.context.principal], ["Change", c.id], ["Vote revision (current)", current()?.id], ["Vote commit (current)", current()?.commit],
        ["Comment anchors", chosen.map(item => `${item.id}: revision ${item.anchor.revision}, base ${item.anchor.base_commit}, ${item.anchor.file}:${item.anchor.start_line}–${item.anchor.end_line} (${item.anchor.side})`).join("\n") || "None"],
        ["Exact saved comment text", savedText || "None"], ["Vote", vote.value || "None"],
      ] });
      if (result) { clearFields("publish-vote", ...ids.map(id => `publish-${id}`), ...ids.map(id => `draft-edit-${id}`)); draw(); }
    }, state.detail.checklist?.ready === true ? undefined : "primary"));
  }
  function clear() {
    state.epoch++; state.request++; state.diffRequest++;
    for (const dialog of state.dialogs) { dialog.close(); dialog.remove(); } state.dialogs.clear();
    state.container?.replaceChildren(); state.container = null; state.context = null; state.snapshot = null; state.detail = null; state.diff = null; state.forms.clear(); state.pending = false; state.pinnedRevision = "";
    state.error = ""; state.diffError = ""; state.route = {};
    headerActions(null, null, false);
  }
  // r[impl kiln.review.keyboard] Keyboard actions locate the existing control and invoke it, so confirmation,
  // request identity, disabled reasons and refresh behaviour stay single-sourced with pointer use.
  const focusControl = element => { if (!element) return false; element.focus({ preventScroll: true }); element.scrollIntoView?.({ block: "nearest", inline: "nearest" }); return true; };
  const fileLinks = () => [...(state.container?.querySelectorAll(".cr-file-list .cr-file-row > a") || [])];
  const hunkMarks = () => [...(state.container?.querySelectorAll(".cr-hunk-mark") || [])];
  const displayedFileLink = () => fileLinks().find(a => a.getAttribute("aria-current") === "true") || fileLinks()[0] || null;
  const moveWithin = (list, from, delta) => list.length ? list[Math.max(0, Math.min(list.length - 1, from < 0 ? (delta > 0 ? 0 : list.length - 1) : from + delta))] : null;
  function threadAnchors() {
    return [...(state.container?.querySelectorAll('article.cr-thread[data-resolved="false"] .cr-anchor') || [])];
  }
  // The route a thread anchor points at, so consecutive presses advance instead of restarting.
  function anchorIsRouted(anchor) {
    let params;
    try { params = new URLSearchParams(anchor.getAttribute("href") || ""); } catch { return false; }
    return params.get("review_file") === (state.route.review_file || "") && params.get("review_side") === (state.route.review_side || "") && params.get("review_line") === (state.route.review_line || "");
  }
  function showShortcutHelp() {
    const { dialog } = modal("Keyboard shortcuts");
    dialog.append(node("p", "Shortcuts are inert while typing in a field, while a dialog is open, during IME composition and with Control, Meta or Alt held. They invoke the same controls as the pointer; a write still passes through its usual confirmation.", "cr-muted"));
    const list = node("dl", null, "cr-keys"); list.setAttribute("aria-label", "Review keyboard shortcuts");
    for (const command of SHORTCUT_COMMANDS) { list.append(node("dt", command.keys), node("dd", command.description)); }
    dialog.append(list);
  }
  function startDraft() {
    // Review fields carry their form-memory key as data-cr-key; they have no element ids.
    const body = state.container?.querySelector('[data-cr-key="draft-body"]');
    const disclosure = body?.closest("details");
    if (!body || !disclosure) return false;
    disclosure.open = true;
    const set = (key, value) => { const field = state.container.querySelector(`[data-cr-key="${key}"]`); if (!field || value === undefined || value === null || value === "") return; field.value = String(value); field.dispatchEvent(new Event("input")); };
    set("draft-file", state.route.review_file);
    set("draft-side", state.route.review_side);
    const line = Number(state.route.review_line);
    if (Number.isSafeInteger(line) && line > 0) { set("draft-start", line); set("draft-end", line); }
    body.focus({ preventScroll: true });
    body.scrollIntoView({ block: "center" });
    return true;
  }
  function keyboardAction(id) {
    const focusedHunk = document.activeElement?.closest?.(".cr-hunk-mark") || null;
    const marks = hunkMarks();
    switch (id) {
      case "next-file":
      case "previous-file": {
        const links = fileLinks();
        const target = moveWithin(links, links.indexOf(displayedFileLink()), id === "next-file" ? 1 : -1);
        if (!target) return false;
        target.focus({ preventScroll: true });
        target.click();
        return true;
      }
      case "next-hunk": return focusControl(moveWithin(marks, marks.indexOf(focusedHunk), 1));
      case "previous-hunk": return focusControl(moveWithin(marks, marks.indexOf(focusedHunk), -1));
      case "next-unviewed":
      case "previous-unviewed": {
        if (!marks.length) return false;
        const step = id === "next-unviewed" ? 1 : -1, start = focusedHunk ? marks.indexOf(focusedHunk) : -1;
        for (let offset = 1; offset <= marks.length; offset += 1) {
          const mark = marks[(((start + step * offset) % marks.length) + marks.length) % marks.length];
          if (mark.dataset.viewed === "false" && !mark.disabled) return focusControl(mark);
        }
        return false;
      }
      case "next-thread": { const anchors = threadAnchors(); if (!anchors.length) return false; const at = anchors.findIndex(anchorIsRouted); const target = anchors[at + 1] || anchors[0]; target.click(); return true; }
      case "previous-thread": { const anchors = threadAnchors(); if (!anchors.length) return false; const at = anchors.findIndex(anchorIsRouted); const target = at > 0 ? anchors[at - 1] : anchors[anchors.length - 1]; target.click(); return true; }
      case "toggle-layout": { const group = state.container?.querySelector(".cr-diff-toolbar .segmented"); const next = [...(group?.querySelectorAll("button") || [])].find(button => button.getAttribute("aria-pressed") !== "true"); if (!next) return false; next.click(); next.focus({ preventScroll: true }); return true; }
      case "toggle-hunk-viewed": if (!focusedHunk) return false; focusedHunk.click(); return true;
      case "toggle-file-viewed": { const row = displayedFileLink()?.closest(".cr-file-row"); const toggle = row?.querySelector(".cr-mark"); if (!toggle) return false; toggle.click(); return true; }
      case "start-draft": return startDraft();
      case "publish": { const publish = state.container?.querySelector(".cr-publish button[data-cr-write]"); if (!publish) return false; publish.click(); return true; }
      case "requirements": { const card = document.getElementById("cr-requirements"); if (!card) return false; card.scrollIntoView({ block: "start" }); card.firstElementChild?.focus({ preventScroll: true }); return true; }
      case "help": showShortcutHelp(); return true;
      default: return false;
    }
  }
  function onKeydown(event) {
    if (!state.container?.isConnected) return;
    const id = resolveShortcut(event, { editable: editableTarget(event.target), dialogOpen: Boolean(document.querySelector("dialog[open]")) });
    if (!id) return;
    event.preventDefault();
    keyboardAction(id);
  }
  let listening = false;
  function listen() {
    if (listening || typeof document === "undefined") return;
    listening = true;
    document.addEventListener("keydown", onKeydown);
  }
  function load(container, context) {
    listen();
    if (!container || typeof context?.api !== "function" || typeof context?.navigate !== "function") throw new Error("Code review requires a container and authenticated api/navigate callbacks.");
    const nextRoute = context.route instanceof URLSearchParams ? Object.fromEntries(context.route) : { ...(context.route || {}) };
    const changed = state.container !== container || state.context?.principal !== context.principal || JSON.stringify(state.route) !== JSON.stringify(nextRoute);
    if (state.context?.principal !== context.principal) clear();
    if (selected() !== nextRoute.review_change || state.route.review_revision !== nextRoute.review_revision) state.pinnedRevision = nextRoute.review_revision || "";
    if (changed) { state.epoch++; state.request++; state.diffRequest++; for (const dialog of state.dialogs) { dialog.close(); dialog.remove(); } state.dialogs.clear(); state.diff = null; state.diffError = ""; if (selected() !== nextRoute.review_change) state.detail = null; }
    state.container = container; state.context = context; state.route = nextRoute;
    if (changed) clearFields("draft-file", "draft-side", "draft-start", "draft-end");
    draw();
    return refresh();
  }
  window.KilnCodeReview = Object.freeze({ load, refresh, clear, keys: Object.freeze({ resolve: resolveShortcut, commands: SHORTCUT_COMMANDS }), hunks: hunkApi, comparison: Object.freeze({ anchorFor, anchorBaseFor, diffMatches }), authorship: Object.freeze({ payload: authorshipPayload, label: authorshipLabel, classLabel: authorshipClassLabel, limit: AUTHORSHIP_TEXT_BYTES }) });
})();
