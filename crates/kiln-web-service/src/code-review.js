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
  const revision = () => state.route.review_revision || state.pinnedRevision || current()?.id || "";
  const active = (epoch, id) => epoch === state.epoch && (!id || id === selected()) && !!state.container?.isConnected;
  const writable = () => state.snapshot?.can_write === true && !state.pending;
  const short = value => String(value || "").length > 20 ? `${String(value).slice(0, 12)}…${String(value).slice(-6)}` : String(value || "");
  const RENDER_LIMIT = 65536, TOKEN_LIMIT = 4096, LINE_LIMIT = 16384;
  function safeLink(value) {
    try { const url = new URL(value); return ["https:", "http:"].includes(url.protocol) && !url.username && !url.password ? url.href : null; } catch { return null; }
  }
  function highlighted(text, filename = "") {
    const code = node("code", null, "cr-highlight"); text = String(text ?? "");
    // A deliberately conservative lexer: no HTML parser, grammar downloads or code execution.
    if (text.length > LINE_LIMIT) { code.textContent = text; return code; }
    const hashComments = /(?:\.(?:py|sh|bash|nix|ncl|yaml|yml|toml|rb)|(?:Dockerfile|Makefile))$/i.test(filename);
    const pattern = /("(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`|\/\/[^\n]*|#[^\n]*|\b(?:0x[\da-fA-F]+|\d+(?:\.\d+)?)\b|\b[A-Za-z_]\w*\b)/g;
    const keywords = new Set(["as", "async", "await", "break", "case", "class", "const", "continue", "def", "else", "enum", "export", "false", "fn", "for", "from", "function", "if", "impl", "import", "in", "let", "match", "mod", "mut", "None", "null", "pub", "return", "self", "Some", "static", "struct", "super", "switch", "then", "throw", "trait", "true", "try", "type", "use", "var", "while"]);
    let offset = 0, count = 0;
    for (const match of text.matchAll(pattern)) {
      if (++count > TOKEN_LIMIT) break;
      const value = match[0], comment = value.startsWith("//") || (hashComments && value.startsWith("#"));
      code.append(document.createTextNode(text.slice(offset, match.index)));
      const kind = comment ? "comment" : /^["'`]/.test(value) ? "string" : /^\d/.test(value) ? "number" : keywords.has(value) ? "keyword" : "";
      code.append(kind ? node("span", value, `cr-token-${kind}`) : document.createTextNode(value)); offset = match.index + value.length;
    }
    code.append(document.createTextNode(text.slice(offset))); return code;
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
        const pre = node("pre"); pre.tabIndex = 0; pre.setAttribute("aria-label", fence[2] ? `${fence[2]} code block` : "Code block"); pre.append(highlighted(body.join("\n"), `code.${fence[2]}`)); content.append(pre);
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
  function codeCell(text, filename) { const td = node("td", null, "cr-code"); td.append(highlighted(text, filename)); return td; }
  function contribution() {
    const sources = arr(state.snapshot.contribution_sources);
    if (!sources.length) throw new Error("No contribution branches are admitted for this principal.");
    const { dialog, close } = modal("Submit a contribution branch for review");
    dialog.append(node("p", "Only configured source branches are admitted. The service resolves the exact immutable tip and configured target, without pushing or moving a real ref.", "cr-muted"));
    const source = field(dialog, "Configured contribution branch", "select", sources[0].source_ref, sources.map(s => [s.source_ref, `${s.source_ref} → ${s.target_ref} · ${short(s.commit)}`]), "contribution-source");
    const expected = field(dialog, "Expected exact source commit (clear to resolve branch tip at intake)", "text", sources.find(s => s.source_ref === source.value)?.commit || "", [], "contribution-commit");
    source.addEventListener("change", () => { expected.value = sources.find(s => s.source_ref === source.value)?.commit || ""; state.forms.set(memoryKey("contribution-commit"), expected.value); });
    const title = field(dialog, "Title", "text", "", [], "contribution-title"), description = field(dialog, "Description (Markdown supported)", "textarea", "", [], "contribution-description"), wip = field(dialog, "Work in progress", "checkbox", true, [], "contribution-wip");
    dialog.append(writeButton("Confirm branch intake", async () => {
      if (!title.value.trim()) throw new Error("A contribution title is required.");
      const configured = sources.find(s => s.source_ref === source.value); if (!configured) throw new Error("Select an admitted contribution branch.");
      const result = await command({ type: "contribute", source_ref: source.value, expected_commit: expected.value.trim() || null, title: title.value.trim(), description: description.value, wip: wip.checked }, null, ["Submit contribution branch", [["Principal", state.context.principal], ["Repository", state.snapshot.repository], ["Source branch", source.value], ["Expected source", expected.value.trim() || "Resolve the configured branch tip at intake; no exact commit precondition"], ["Configured target", configured.target_ref], ["Title", title.value.trim()], ["Description", description.value], ["WIP", String(wip.checked)], ["Effect", "Register review only; no target movement or remote publication"]]]);
      if (result) { clearFields("contribution-source", "contribution-commit", "contribution-title", "contribution-description", "contribution-wip"); close(); if (result.change?.id) route({ review_change: result.change.id, review_revision: "" }); }
    }, "primary"));
  }
  function drawQueue(root, entries = state.snapshot?.queue) {
    if (!Array.isArray(entries)) return;
    const card = section(root, "CI-bound merge queue");
    card.append(node("p", "Each entry constructs a candidate against its exact observed target. Required CI must qualify that exact constructed commit before CAS submission. Target movement requires a newly constructed candidate and fresh CI, never reuse of the previous result.", "cr-muted"));
    for (const entry of entries) {
      const article = node("article", null, "cr-operation"); article.append(changeLink(entry.change, `${label(entry.status)} · ${entry.change}`));
      facts(article, [["Queue entry", entry.id], ["Authorized by", entry.actor], ["Source revision", entry.revision], ["Source commit", entry.source_commit], ["Target ref", entry.target_ref], ["Exact observed target", entry.target_commit], ["Constructed candidate commit", entry.candidate_commit || "Not constructed yet; no exact candidate CI claim"], ["Canonical CI run", entry.ci_run || "No candidate CI observed"], ["Observation", entry.detail], ["Enqueued at", new Date(entry.created_ms).toLocaleString()]]);
      if (entry.ci_run) { const run = node("a", "Inspect candidate CI run"); run.href = `?${new URLSearchParams({ page: "runs", run: entry.ci_run, view: "pipeline" })}`; article.append(run); }
      card.append(article);
    }
    if (!entries.length) card.append(node("p", "No retained merge queue entries.", "cr-muted"));
    const c = state.detail?.change;
    if (!c || revision() !== current()?.id || !Array.isArray(state.snapshot?.queue)) return;
    const entry = entries.find(e => e.change === c.id && !["submitted", "cancelled"].includes(e.status));
    if (entry) card.append(writeButton("Cancel queued automation", () => command({ type: "dequeue" }, c, ["Cancel merge queue entry", [["Principal", state.context.principal], ["Change", c.id], ["Queue entry", entry.id], ["State", entry.status], ["Effect", "Revoke pending automation; an already completed provider effect cannot be undone"]]])));
    else if (c.state === "open") card.append(writeButton("Authorize CI-bound merge automation", () => {
      requireCurrent(); const expected = state.detail.source?.current_target; if (!expected) throw new Error("An exact target observation is required before queue authorization.");
      return command({ type: "enqueue", expected_target: expected }, c, ["Authorize merge queue automation", [["Principal", state.context.principal], ["Repository", c.repository], ["Change", c.id], ["Exact source revision", current()?.id], ["Exact source commit", current()?.commit], ["Target ref", c.target_ref], ["Currently observed target", expected], ["Automation scope", "Construct a new merge candidate and run required CI on its exact resulting commit. Automatically submit only after all exact-candidate policy checks pass."], ["Target movement authorization", "If this same target ref moves, recompute a new merge candidate using this exact source revision and the new target, run fresh required CI, and submit the resulting merge commit by exact-target CAS. This authorizes those resulting commits, not a single pre-built commit."], ["Bounds", "No source revision change, target-ref change, policy bypass, or reuse of stale CI is authorized. Cancel the queue entry to revoke pending automation."]]]);
    }, "primary"));
  }
  function drawSubscriptions(root) {
    if (typeof state.detail?.subscribed !== "boolean") return;
    const card = section(root, "Mentions, subscription and notification delivery"), c = state.detail.change;
    card.append(node("p", `Subscription belongs only to authenticated principal ${state.context.principal}. Published @principal or @{principal} mentions request attention; private drafts do not send notifications. External recipients are configured by the operator, never entered here.`, "cr-muted"));
    card.append(writeButton(state.detail.subscribed ? "Unsubscribe me" : "Subscribe me", () => command({ type: "subscribe", subscribed: !state.detail.subscribed }, c)));
    for (const delivery of arr(state.detail.delivery_outcomes)) {
      const article = node("article", null, "cr-operation"); facts(article, [["Delivery", delivery.id], ["Notification", delivery.notification?.id], ["Kind", label(delivery.notification?.kind)], ["Actor", delivery.notification?.actor], ["Revision", delivery.notification?.revision], ["Observed status", label(delivery.status)]]);
      for (const attempt of arr(delivery.attempts)) { const body = disclosure(article, `Delivery attempt ${attempt.id} · ${label(attempt.status)}`); facts(body, [["Reserved at", attempt.reserved_ms == null ? null : new Date(attempt.reserved_ms).toLocaleString()], ["Completed at", attempt.completed_ms == null ? null : new Date(attempt.completed_ms).toLocaleString()], ["HTTP status", attempt.http_status], ["Outcome", label(attempt.status)]]); }
      card.append(article);
    }
    if (!arr(state.detail.delivery_outcomes).length) card.append(node("p", "No external delivery outcomes are recorded for this principal on this change. Absence is not delivery success.", "cr-muted"));
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
    const card = section(root, "Apply published suggestions as a new immutable revision"), chosen = [];
    card.append(node("p", "A complete fenced suggestion comment replaces its exact inclusive revision-side line range. Only the current change owner may apply it. Batch ranges must not overlap; stale or mismatched source text fails closed in the service. Applying never rewrites an existing revision or advances the target.", "cr-muted"));
    for (const thread of threads) { const article = node("article", null, "cr-operation"), a = thread.anchor; article.append(link(`${a.file}:${a.start_line}–${a.end_line}`, anchorRoute(a))); facts(article, [["Thread", thread.id], ["Revision", a.revision], ["Comparison base", a.base_commit], ["Exact path", a.file], ["Inclusive replacement range", `${a.start_line}–${a.end_line}`]]); const pre = node("pre", null, "cr-suggestion"); pre.append(highlighted(suggestionText(thread), a.file)); article.append(pre); const input = field(article, "Select this suggestion", "checkbox", false, [], `suggestion-${thread.id}`); input.disabled = c.owner !== state.context.principal || revision() !== current()?.id; chosen.push({ thread, input }); card.append(article); }
    if (c.owner !== state.context.principal || revision() !== current()?.id) { card.append(node("p", "Select the current revision as its authenticated owner to apply suggestions.", "cr-muted")); return; }
    card.append(writeButton("Confirm and apply selected suggestions", async () => {
      requireCurrent(); const selected = chosen.filter(s => s.input.checked).map(s => s.thread); if (!selected.length) throw new Error("Select at least one published suggestion.");
      const result = await command({ type: "apply_suggestions", thread_ids: selected.map(t => t.id) }, c, ["Apply suggestions", [["Author", state.context.principal], ["Change", c.id], ["Exact source revision", current()?.id], ["Exact source commit", current()?.commit], ["Replacements", selected.map(t => `${t.id}\n${t.anchor.file}:${t.anchor.start_line}–${t.anchor.end_line}\n${suggestionText(t)}`).join("\n\n")], ["Effect", "Create a new immutable current review revision. Existing votes and reviewed-file marks do not authorize the new revision; target ref remains unchanged."]]]);
      if (result) { clearFields(...selected.map(t => `suggestion-${t.id}`)); route({ review_revision: "", review_base: "base", review_file: "", review_side: "", review_line: "" }); }
    }, "primary"));
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
  function section(parent, title) { const s = node("section", null, "cr-card"); s.append(node("h3", title)); parent.append(s); return s; }
  function disclosure(parent, title) { const d = node("details", null, "cr-disclosure"); d.append(node("summary", title)); const body = node("div", null, "cr-fields"); d.append(body); parent.append(d); return body; }
  function facts(parent, values) { const dl = node("dl", null, "cr-facts"); for (const [key, value] of values) dl.append(node("dt", key), node("dd", value == null || value === "" ? "Not recorded" : value)); parent.append(dl); }
  function memoryKey(key) { const scoped = /^(draft-|reply-|publish)/.test(key) ? revision() : ""; return `${state.context?.principal}\n${selected()}\n${scoped}\n${key}`; }
  function field(parent, name, type = "text", value = "", choices = [], key = "") {
    const wrap = node("label", null, "cr-field"); wrap.append(node("span", name));
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
    if (["review_q", "review_view", "review_status"].some(key => Object.hasOwn(patch, key)) && !Object.hasOwn(patch, "review_offset")) patch = { ...patch, review_offset: "" };
    if (Object.hasOwn(patch, "review_revision")) state.pinnedRevision = patch.review_revision || "";
    state.context.navigate(patch);
    // Also support hosts that update history without remounting this component.
    const next = { ...state.route, ...patch };
    if (JSON.stringify(next) !== JSON.stringify(state.route)) load(state.container, { ...state.context, route: next });
  }
  function href(patch) { const q = new URLSearchParams(window.location.search); q.set("page", "code-review"); for (const [k, v] of Object.entries(patch)) v == null || v === "" ? q.delete(k) : q.set(k, String(v)); return `?${q}`; }
  function link(text, patch, cls) { const a = node("a", text, cls); a.href = href(patch); a.addEventListener("click", event => { if (!event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey && event.button === 0) { event.preventDefault(); route(patch); } }); return a; }
  function changeLink(id, text = id) { return link(text, { review_change: id, review_revision: "", review_base: "", review_file: "", review_side: "", review_line: "" }); }
  function modal(title) {
    const dialog = node("dialog", null, "cr-dialog"); const heading = node("h2", title); heading.id = `cr-${crypto.randomUUID()}`; dialog.setAttribute("aria-labelledby", heading.id);
    const opener = document.activeElement;
    const close = () => { dialog.close(); dialog.remove(); state.dialogs.delete(dialog); const target = opener?.isConnected ? opener : state.container?.querySelector("button"); target?.focus({ preventScroll: true }); };
    const header = node("div", null, "cr-toolbar"); header.append(heading, button("Close", close)); dialog.append(header); dialog.addEventListener("cancel", event => { event.preventDefault(); close(); });
    document.body.append(dialog); state.dialogs.add(dialog); dialog.showModal(); return { dialog, close };
  }
  async function confirmAction(title, values) {
    return new Promise(resolve => {
      const { dialog, close } = modal(title); facts(dialog, values);
      dialog.append(node("p", "Confirm these exact values and the explicitly described authorization scope. Nothing outside that scope is authorized.", "cr-text"));
      let settled = false;
      const finish = value => { if (settled) return; settled = true; close(); resolve(value); };
      dialog.addEventListener("close", () => { if (!settled) { settled = true; resolve(false); } });
      const actions = node("div", null, "cr-toolbar"); actions.append(button("Cancel", () => finish(false)), button(`Confirm ${title.toLowerCase()}`, () => finish(true), "primary")); dialog.append(actions);
    });
  }
  async function command(action, change = state.detail?.change, confirmation = null) {
    if (!writable()) throw new Error("Review writes are not admitted for this principal, or another command is pending.");
    const epoch = state.epoch, id = selected(), context = state.context;
    const captured = change ? { id: change.id, version: change.version } : null;
    if (confirmation && !await confirmAction(confirmation[0], confirmation[1])) return null;
    if (!active(epoch, id) || context.principal !== state.context.principal || !writable()) throw new Error("Selection, identity or command availability changed before confirmation. No command was sent.");
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
    const region = state.container?.querySelector(".cr-notice"); if (!region) return;
    region.textContent = state.error || (state.pending ? "Recording command. Wait for an observed result." : ""); region.hidden = !region.textContent; region.dataset.kind = state.error ? "error" : "status";
    for (const scope of [state.container, ...state.dialogs]) for (const b of scope.querySelectorAll("[data-cr-write]")) b.disabled = !writable() || b.dataset.crBlocked === "true";
  }
  function writeButton(text, action, cls) { const b = button(text, action, cls, true); b.dataset.crWrite = "true"; return b; }
  async function refresh() {
    if (!state.container?.isConnected || !state.context) return;
    const epoch = state.epoch, id = selected(), request = ++state.request, context = state.context;
    try {
      const root = await context.api(`${ROOT}?${new URLSearchParams({ q: state.route.review_q || "", view: state.route.review_view || "all", offset: state.route.review_offset || "0", notification_offset: state.route.review_notification_offset || "0", operation_offset: state.route.review_operation_offset || "0", ...(state.route.review_status ? { status: state.route.review_status } : {}) })}`);
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
  function draw() {
    const focused = document.activeElement; const key = focused?.dataset?.crKey; const start = focused?.selectionStart, end = focused?.selectionEnd;
    const opened = new Set(Array.from(state.container?.querySelectorAll("details[open]") || []).map(d => d.querySelector("summary")?.textContent));
    const root = state.container; if (!root) return; root.classList.add("code-review"); root.replaceChildren();
    const header = node("div", null, "cr-toolbar"); header.append(node("h2", "Change review"), button("Refresh", refresh)); root.append(header);
    const notice = node("p", null, "cr-notice"); notice.setAttribute("role", "status"); notice.setAttribute("aria-live", "polite"); root.append(notice); drawNotice();
    if (!state.snapshot) { root.append(node("p", state.error ? "The review workspace could not be loaded." : "Loading review workspace…", "cr-empty")); return; }
    if (!state.snapshot.configured) { root.append(node("p", "Interactive review is not configured. A separately admitted collaboration provider, actors, policy and durable store are required. Existing review receipts remain available in the receipts section.", "cr-empty")); return; }
    root.append(node("p", `Principal: ${state.context.principal} · ${state.snapshot.can_write ? "Review writes admitted" : "Read-only review access"}. CI and provider submission authority are checked separately.`, "cr-muted"));
    if (selected()) { if (state.detail) drawDetail(root); else root.append(node("p", "Selected change is unavailable. Refresh or return to the change list.", "cr-empty"), link("All changes", { review_change: "" })); } else drawList(root);
    for (const d of root.querySelectorAll("details")) if (opened.has(d.querySelector("summary")?.textContent)) d.open = true;
    if (key) { const next = Array.from(root.querySelectorAll("[data-cr-key]")).find(n => n.dataset.crKey === key); if (next) { next.focus({ preventScroll: true }); if (typeof start === "number" && next.setSelectionRange && next.type !== "checkbox" && next.tagName !== "SELECT") { try { next.setSelectionRange(start, end); } catch {} } } }
  }
  function drawList(root) {
    const search = node("form", null, "cr-toolbar"); const q = field(search, "Search change text", "search", state.route.review_q || "", [], "search");
    const status = field(search, "State", "select", state.route.review_status || "", [["", "All states"], "open", "abandoned", "submitted"]);
    const submit = node("button", "Search"); submit.type = "submit"; search.append(submit, writeButton("Create change / upload revision", createChange));
    search.addEventListener("submit", event => { event.preventDefault(); route({ review_q: q.value, review_status: status.value, review_change: "" }); }); root.append(search);
    if (arr(state.snapshot.contribution_sources).length) search.append(writeButton("Submit contribution branch", contribution));
    const views = section(root, "Personal views"); const viewbar = node("div", null, "cr-toolbar");
    for (const [text, kind] of [["All changes", "all"], ["Awaiting my review", "awaiting_review"], ["My changes needing revision", "owned_needs_revision"], ["Unresolved discussions", "unresolved"]]) viewbar.append(button(text, () => { clearFields("search"); route({ review_view: kind }); }));
    views.append(viewbar);
    for (const view of arr(state.snapshot.saved_views)) { const row = node("div", null, "cr-toolbar"); row.append(button(view.name, () => { clearFields("search"); route({ review_q: view.query.text, review_view: view.query.kind, review_status: view.query.status || "" }); }), writeButton(`Delete view ${view.name}`, () => command({ type: "delete_view", id: view.id }, null, ["Delete personal view", [["Name", view.name], ["Query", JSON.stringify(view.query)]]]))); views.append(row); }
    const save = node("div", null, "cr-toolbar"); const name = field(save, "Save current query as", "text", "", [], "view-name"); save.append(writeButton("Save personal view", async () => { if (!name.value.trim()) throw new Error("A saved view name is required."); await command({ type: "save_view", name: name.value.trim(), query: { kind: state.route.review_view || "all", text: state.route.review_q || "", status: state.route.review_status || null } }, null); clearFields("view-name"); })); views.append(save);
    drawQueue(root);
    const outstanding = section(root, "Outstanding provider operations");
    outstanding.append(node("p", "Pending and uncertain operations reserve their affected changes, including creates that do not yet have materialized detail. Refresh observes reconciliation; never blindly repeat an effect.", "cr-muted"));
    for (const op of arr(state.snapshot.operations)) {
      const article = node("article", null, "cr-operation");
      facts(article, [["Operation", op.operation_id], ["Kind", label(op.kind)], ["Observed state", op.state], ["Affected stable change", op.change], ["Source commit", op.commit], ["Target ref", op.target_ref], ["Observed target commit", op.target_commit]]);
      if (op.result_change) article.append(changeLink(op.result_change, `Resulting change: ${op.result_change}`));
      outstanding.append(article);
    }
    if (!arr(state.snapshot.operations).length) outstanding.append(node("p", "No outstanding operations on this page.", "cr-muted"));
    const operationPaging = node("div", null, "cr-toolbar"), operationOffset = Number(state.route.review_operation_offset) || 0;
    const previousOperations = button("Previous operations", () => route({ review_operation_offset: String(Math.max(0, operationOffset - 100)) })); previousOperations.disabled = operationOffset === 0;
    const nextOperations = button("Next operations", () => route({ review_operation_offset: String(state.snapshot.next_operation_offset) })); nextOperations.disabled = state.snapshot.next_operation_offset == null;
    operationPaging.append(previousOperations, node("span", `${arr(state.snapshot.operations).length} shown · ${state.snapshot.operations_total ?? 0} outstanding operations`, "cr-muted"), nextOperations); outstanding.append(operationPaging);
    const inbox = section(root, "Review notifications");
    for (const item of arr(state.snapshot.notifications)) { const row = node("article", null, "cr-notification"); row.append(changeLink(item.change, `${label(item.kind)} · ${short(item.revision)}`), node("span", `${item.actor} · ${new Date(item.created_ms).toLocaleString()} · ${item.read ? "Read" : "Unread"}`, "cr-muted"), writeButton(item.read ? "Mark unread" : "Mark read", () => command({ type: "read_notification", id: item.id, read: !item.read }, null))); inbox.append(row); }
    if (!arr(state.snapshot.notifications).length) inbox.append(node("p", "No retained notifications for this principal.", "cr-muted"));
    const notificationPaging = node("div", null, "cr-toolbar"), notificationOffset = Number(state.route.review_notification_offset) || 0;
    const previousNotifications = button("Previous notifications", () => route({ review_notification_offset: String(Math.max(0, notificationOffset - 200)) })); previousNotifications.disabled = notificationOffset === 0;
    const nextNotifications = button("Next notifications", () => route({ review_notification_offset: String(state.snapshot.next_notification_offset) })); nextNotifications.disabled = state.snapshot.next_notification_offset == null;
    notificationPaging.append(previousNotifications, node("span", `${arr(state.snapshot.notifications).length} shown · ${state.snapshot.notifications_total ?? arr(state.snapshot.notifications).length} retained notifications`, "cr-muted"), nextNotifications); inbox.append(notificationPaging);
    const list = section(root, "Changes");
    const paging = node("div", null, "cr-toolbar"), offset = Number(state.route.review_offset) || 0;
    const previous = button("Previous page", () => route({ review_offset: String(Math.max(0, offset - 25)) })); previous.disabled = offset === 0;
    const next = button("Next page", () => route({ review_offset: String(state.snapshot.next_offset) })); next.disabled = state.snapshot.next_offset == null;
    paging.append(previous, node("span", `${arr(state.snapshot.changes).length ? offset + 1 : 0}–${offset + arr(state.snapshot.changes).length} of ${state.snapshot.total ?? arr(state.snapshot.changes).length} matching changes`, "cr-muted"), next); list.append(paging);
    for (const change of arr(state.snapshot.changes)) { const row = node("article", null, "cr-change-row"); row.append(changeLink(change.id, change.title), node("p", `${change.state}${change.wip ? " · WIP" : ""} · ${change.owner} · ${change.repository} → ${change.target_ref}`, "cr-muted")); if (arr(change.attention).length) row.append(node("p", `Attention: ${change.attention.join(", ")}`, "cr-muted")); list.append(row); }
    if (!arr(state.snapshot.changes).length) list.append(node("p", "No changes match this query.", "cr-empty"));
  }
  function createChange() {
    const { dialog, close } = modal("Create change from an exact local commit");
    facts(dialog, [["Project", state.snapshot.project], ["Repository", state.snapshot.repository]]);
    const form = node("div", null, "cr-fields"); dialog.append(form);
    const title = field(form, "Title", "text", "", [], "create-title");
    const description = field(form, "Description", "textarea", "", [], "create-description");
    const commit = field(form, "Exact immutable commit", "text", "", [], "create-commit");
    const target = field(form, "Admitted target ref", "select", arr(state.snapshot.target_refs)[0] || "", arr(state.snapshot.target_refs), "create-target");
    const wip = field(form, "Work in progress", "checkbox", true, [], "create-wip");
    dialog.append(node("p", "Upload registers a locally available commit as a new review. It does not advance the target or publish to a remote provider.", "cr-muted"));
    dialog.append(writeButton("Register change", async () => {
      if (!title.value.trim() || !commit.value.trim() || !target.value) throw new Error("Title, exact commit and admitted target are required.");
      const result = await command({ type: "create", title: title.value.trim(), description: description.value, commit: commit.value.trim(), target_ref: target.value, wip: wip.checked }, null, ["Register change", [["Repository", state.snapshot.repository], ["Source commit", commit.value.trim()], ["Target ref", target.value], ["Title", title.value.trim()], ["WIP", String(wip.checked)]]]);
      if (result) { clearFields("create-title", "create-description", "create-commit", "create-target", "create-wip"); close(); if (result.change?.id) route({ review_change: result.change.id }); }
    }, "primary"));
  }
  function drawCi(root) {
    const c = state.detail.change, selectedRevision = arr(c.revisions).find(r => r.id === revision());
    const card = section(root, "Canonical CI for the selected exact revision");
    facts(card, [["Review revision", selectedRevision?.id], ["Exact source commit", selectedRevision?.commit]]);
    card.append(node("p", "Up to 25 newest exact-source facts retained in this response are shown. Full/clean successful facts and the policy's exact winner evaluation remain checklist authority; a displayed run alone does not authorize submission.", "cr-muted"));
    const matching = arr(state.detail.ci_runs).filter(fact => fact.project === c.project && fact.repository === c.repository && fact.revision === selectedRevision?.commit);
    for (const fact of matching.slice(0, 25)) {
      const row = node("article", null, "cr-operation"), run = node("a", `Inspect run ${fact.run_id}`);
      run.href = `?${new URLSearchParams({ page: "runs", run: fact.run_id, view: "pipeline" })}`;
      row.append(node("h4", fact.success ? "Success" : "Non-success or non-terminal; inspect run"), run);
      facts(row, [["Source commit", fact.revision], ["Scope", fact.full_run ? "Full run" : "Partial run"], ["Input", fact.clean ? "Clean" : "Dirty"], ["Canonical ordinal", Number.isSafeInteger(fact.ordinal) ? String(fact.ordinal) : "Outside exact browser integer range; service ordering retained"]]);
      card.append(row);
    }
    if (!matching.length) card.append(node("p", "No canonical exact-source CI evidence is present in this retained response for the selected revision. Missing evidence is not success; inspect the checklist's exact blocked reason.", "cr-notice"));
  }
  function drawDetail(root) {
    const c = state.detail.change, r = arr(c.revisions).find(item => item.id === revision());
    root.append(link("← All changes", { review_change: "", review_revision: "", review_base: "", review_file: "", review_side: "", review_line: "" }));
    const header = section(root, c.title);
    header.append(markdown(c.description));
    facts(header, [["Stable change", c.id], ["State", `${label(c.state)}${c.wip ? " · Work in progress" : ""}`], ["Owner", c.owner], ["Project / repository", `${c.project} / ${c.repository}`], ["Target ref", c.target_ref], ["Observed target", state.detail.source?.current_target], ["Current revision", current()?.id], ["Current commit", current()?.commit]]);
    if (revision() !== current()?.id) header.append(node("p", "Historical revision selected. Comments and drafts retain this exact revision and comparison base. Optional votes always apply to the current revision shown in the publication confirmation; provider actions require the current revision.", "cr-notice"));
    drawCi(root);
    const history = section(root, "Revision history and comparison");
    const controls = node("div", null, "cr-toolbar");
    const revisions = arr(c.revisions).map(item => [item.id, `Revision ${item.number} · ${short(item.commit)}`]);
    const rev = field(controls, "Review revision", "select", revision(), revisions);
    const base = field(controls, "Compare against", "select", state.route.review_base || "base", [["base", "Selected revision's base"], ...arr(c.revisions).filter(item => item.number < (r?.number || 0)).map(item => [item.id, `Revision ${item.number} · ${short(item.commit)}`])]);
    rev.addEventListener("change", () => route({ review_revision: rev.value, review_base: "base", review_file: "", review_side: "", review_line: "" }));
    base.addEventListener("change", () => route({ review_base: base.value, review_file: "", review_side: "", review_line: "" }));
    const mode = field(controls, "Diff layout", "select", state.mode, [["unified", "Unified"], ["split", "Side by side"]]); mode.addEventListener("change", () => { state.mode = mode.value; drawDiff(); }); history.append(controls);
    const whitespace = field(controls, "Whitespace comparison", "select", state.route.review_whitespace || "exact", [["exact", "Show all changes (exact)"], ["ignore_all", "Ignore all whitespace"], ["ignore_change", "Ignore whitespace amount"], ["ignore_eol", "Ignore whitespace at end of line"]]);
    whitespace.addEventListener("change", () => route({ review_whitespace: whitespace.value }));
    const log = node("ol", null, "cr-history"); for (const item of arr(c.revisions)) { const li = node("li"); li.append(link(`Revision ${item.number}`, { review_revision: item.id, review_base: "base", review_file: "", review_side: "", review_line: "" }), node("span", `${item.author} · ${new Date(item.created_ms).toLocaleString()}`, "cr-muted"), node("code", item.commit)); log.append(li); } history.append(log);
    if (!r) history.append(node("p", "The requested revision is not part of this change. Choose an admitted revision.", "cr-notice"));
    const diff = section(root, "Changed files and diff"); const host = node("div", null, "cr-diff-host"); diff.append(host); drawDiff();
    drawDiscussion(root); drawReviewers(root); drawApproval(root); drawStack(root); drawAuthorActions(root);
    drawSuggestions(root); drawSubscriptions(root); drawQueue(root, Array.isArray(state.detail.queue) ? state.detail.queue : state.detail.queue ? [state.detail.queue] : arr(state.snapshot.queue).filter(e => e.change === c.id));
    const operations = section(root, "Durable provider operations");
    for (const op of arr(state.detail.operations)) {
      const article = node("article", null, "cr-operation");
      facts(article, [["Operation", op.operation_id], ["Kind", label(op.kind)], ["Observed state", op.state], ["Commit", op.commit], ["Target ref", op.target_ref], ["Target commit", op.target_commit]]);
      if (op.result_change) article.append(changeLink(op.result_change, `Resulting change: ${op.result_change}`));
      if (op.state === "pending" || op.state === "uncertain") article.append(node("p", "This operation reserves the affected change until the service reconciles its exact outcome. Do not retry the provider action.", "cr-notice"));
      operations.append(article);
    }
    if (!arr(state.detail.operations).length) operations.append(node("p", "No retained provider operations for this change.", "cr-muted"));
    operations.append(node("p", "Pending or uncertain effects are not automatically retried by this page. Refresh observes the service's durable reconciliation. Read-only review receipts remain in the existing receipts section.", "cr-muted"));
  }
  function requireCurrent() { if (revision() !== current()?.id) throw new Error("This is a historical revision. Select the current revision before changing review state."); }
  function drawReviewers(root) {
    const c = state.detail.change, card = section(root, "Reviewers, CC and attention");
    for (const [principal, role] of Object.entries(c.reviewers || {})) { const row = node("div", null, "cr-toolbar"); row.append(node("span", `${principal} · ${label(role)}`), writeButton(`Remove ${principal}`, () => command({ type: "set_reviewer", principal, role: null }, c))); card.append(row); }
    const form = node("div", null, "cr-toolbar"); const principal = field(form, "Principal", "text", "", [], "reviewer-principal"); const role = field(form, "Role", "select", "reviewer", ["reviewer", "cc"], "reviewer-role");
    form.append(writeButton("Request review / add CC", async () => { requireCurrent(); if (!principal.value.trim()) throw new Error("A principal is required."); await command({ type: "set_reviewer", principal: principal.value.trim(), role: role.value }, c); clearFields("reviewer-principal"); })); card.append(form);
    const attention = node("div", null, "cr-toolbar"); attention.append(node("span", `Attention: ${arr(c.attention).join(", ") || "No principals"}`));
    const who = field(attention, "Attention principal", "text", state.context.principal, [], "attention-principal");
    for (const [text, value] of [["Request attention", true], ["Clear attention", false]]) attention.append(writeButton(text, () => { requireCurrent(); return command({ type: "set_attention", principal: who.value.trim(), attention: value }, c); })); card.append(attention);
  }
  function drawApproval(root) {
    const c = state.detail.change, card = section(root, "Votes and explainable submit checklist");
    const policy = state.snapshot.policy;
    if (arr(policy?.ownership?.rules).length) {
      const ownership = disclosure(card, "Configured ownership and protected target rules (checklist remains authority)");
      for (const rule of policy.ownership.rules) { const article = node("article", null, "cr-operation"); facts(article, [["Rule", rule.id], ["Path patterns", arr(rule.paths).join(", ")], ["Protected targets", arr(rule.target_refs).join(", ") || "All admitted targets"], ["Owners", arr(rule.owners).join(", ")], ["Teams", arr(rule.teams).map(team => `${team}: ${arr(policy.ownership.teams?.[team]).join(", ")}`).join("; ")]]); ownership.append(article); }
    }
    facts(card, [["Policy", policy?.id], ["Eligible reviewers", arr(policy?.eligible_reviewers).join(", ")], ["Required approvals", policy?.minimum_approvals], ["Required CI", policy ? String(policy.require_ci) : "Policy unavailable; submit is blocked"]]);
    if (c.submission) {
      const receipt = section(card, "Observed submission receipt"), outcome = c.submission;
      facts(receipt, [["Submitted by", outcome.submitted_by], ["Submitted at", new Date(outcome.submitted_ms).toLocaleString()], ["Candidate", outcome.candidate?.id], ["Exact source revision", outcome.candidate?.revision], ["Exact source commit", outcome.candidate?.commit], ["Target ref", outcome.candidate?.target_ref], ["Expected target commit", outcome.candidate?.target], ["Result commit", outcome.result_commit], ["CI run", outcome.candidate?.ci_run], ["Exact dependencies", JSON.stringify(outcome.candidate?.dependencies || [])]]);
    }
    for (const vote of arr(c.votes)) card.append(node("p", `${vote.actor}: ${label(vote.value)} · ${short(vote.revision)}${vote.revision === current()?.id ? " · Current" : " · Stale; no current authority"}`, "cr-text"));
    if (!arr(c.votes).length) card.append(node("p", "No votes recorded.", "cr-muted"));
    const checklist = node("ul", null, "cr-checklist"); for (const item of arr(state.detail.checklist?.items)) { const li = node("li", `${item.satisfied ? "Pass" : "Blocked"} · ${label(item.key)}: ${item.detail}`); li.dataset.satisfied = String(item.satisfied); checklist.append(li); } card.append(checklist);
    if (!state.detail.checklist) card.append(node("p", "Checklist unavailable. Submission is blocked until the service can evaluate exact source, target, dependencies, approval and CI facts.", "cr-notice"));
    card.append(node("p", state.detail.checklist?.ready ? "The observed checklist is ready. The service must recheck it at candidate preparation and submission." : "Submission is blocked by the listed requirements. No browser control can override them.", "cr-muted"));
    const actions = node("div", null, "cr-toolbar");
    actions.append(writeButton("Prepare exact submit candidate", async () => { requireCurrent(); await command({ type: "prepare_submit" }, c); }, "primary"));
    if (c.candidate) {
      facts(card, [["Prepared candidate", c.candidate.id], ["Candidate revision", c.candidate.revision], ["Commit", c.candidate.commit], ["Expected target", c.candidate.target], ["CI run", c.candidate.ci_run]]);
      actions.append(writeButton("Submit prepared candidate", () => { requireCurrent(); return command({ type: "submit", candidate_id: c.candidate.id }, c, ["Submit candidate", [["Principal", state.context.principal], ["Repository", c.repository], ["Change", c.id], ["Candidate", c.candidate.id], ["Source revision", c.candidate.revision], ["Source commit", c.candidate.commit], ["Target ref", c.candidate.target_ref], ["Expected target commit", c.candidate.target], ["Dependencies", JSON.stringify(c.candidate.dependencies)]]]); }, "primary"));
    }
    card.append(actions);
  }
  function drawStack(root) {
    const c = state.detail.change, card = section(root, "Stack and exact dependencies");
    card.append(node("p", "Dependencies must be submitted before this change. Follow the links in order, prepare and confirm each candidate independently; target movement requires a fresh checklist. No unchecked batch submission is sent.", "cr-muted"));
    const list = node("ol", null, "cr-stack"); for (const dep of arr(c.dependencies)) { const li = node("li"); const related = arr(state.detail.stack?.dependencies).find(item => item.id === dep.change); li.append(changeLink(dep.change, related?.title || dep.change), node("code", dep.revision), node("span", related ? label(related.state) : "Dependency detail unavailable", "cr-muted")); list.append(li); } const self = node("li", `${c.title} · ${c.state} · selected change`); self.setAttribute("aria-current", "true"); list.append(self); card.append(list);
    const dependents = node("div", null, "cr-toolbar"); dependents.append(node("span", "Required by:")); for (const dep of arr(state.detail.stack?.dependents)) dependents.append(changeLink(dep.id, dep.title)); card.append(dependents);
    card.append(writeButton("Start ordered stack submission", orderedStack));
    const edit = disclosure(card, "Edit exact dependency revisions"); const initial = arr(c.dependencies).map(dep => `${dep.change} ${dep.revision}`).join("\n"); const input = field(edit, "One stable change ID and exact review revision ID per line", "textarea", initial, [], "dependencies");
    edit.append(writeButton("Save dependencies", () => { requireCurrent(); const dependencies = input.value.split("\n").filter(line => line.trim()).map(line => { const parts = line.trim().split(/\s+/); if (parts.length !== 2) throw new Error("Every dependency row must contain exactly a change ID and revision ID."); return { change: parts[0], revision: parts[1] }; }); return command({ type: "set_dependencies", dependencies }, c); }));
  }
  async function orderedStack() {
    const epoch = state.epoch, selectedId = selected(), context = state.context;
    const { dialog } = modal("Ordered stack submission"); const body = node("div", null, "cr-fields"); dialog.append(body);
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
      const list = node("ol", null, "cr-stack"); for (const detail of ordered) { const c = detail.change, li = node("li", `${c.title} · ${c.state}`); li.append(node("code", c.id)); list.append(li); } body.append(list);
      const next = ordered.find(detail => detail.change.state !== "submitted");
      if (!next) { body.append(node("p", "All changes in this exact stack are observed submitted.", "cr-notice")); return; }
      body.append(node("h3", `Next: ${next.change.title}`)); facts(body, [["Change", next.change.id], ["Source commit", arr(next.change.revisions).at(-1)?.commit], ["Target ref", next.change.target_ref], ["Observed target", next.source?.current_target]]);
      const checklist = node("ul", null, "cr-checklist"); for (const item of arr(next.checklist?.items)) checklist.append(node("li", `${item.satisfied ? "Pass" : "Blocked"} · ${item.detail}`)); body.append(checklist);
      const advance = writeButton("Prepare and confirm next stack candidate", async () => {
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
        const submitted = await command({ type: "submit", candidate_id: candidate.id }, c, ["Submit next stack candidate", [["Principal", context.principal], ["Repository", c.repository], ["Change", c.id], ["Candidate", candidate.id], ["Source revision", candidate.revision], ["Source commit", candidate.commit], ["Target ref", candidate.target_ref], ["Expected target commit", candidate.target], ["Dependencies", JSON.stringify(candidate.dependencies)]]]);
        if (submitted && live()) await redraw();
      }, "primary");
      if (!next.checklist?.ready) { advance.disabled = true; advance.dataset.crBlocked = "true"; advance.title = "The exact next-step checklist is blocked. Resolve the listed requirements, then refresh stack observations."; }
      body.append(advance);
      body.append(button("Refresh stack observations", redraw));
    };
    body.append(node("p", "Loading exact dependency order…", "cr-muted"));
    try { await redraw(); } catch (error) { if (live()) { body.replaceChildren(node("p", message(error), "cr-notice")); throw error; } }
  }
  function drawAuthorActions(root) {
    const c = state.detail.change, card = section(root, "Author and provider actions");
    const edit = disclosure(card, "Edit change description and work-in-progress state");
    const title = field(edit, "Title", "text", c.title, [], "edit-title"), description = field(edit, "Description", "textarea", c.description, [], "edit-description");
    edit.append(writeButton("Save description", async () => { requireCurrent(); const result = await command({ type: "edit", title: title.value, description: description.value }, c); if (result) clearFields("edit-title", "edit-description"); }));
    edit.append(writeButton(c.wip ? "Mark ready for review" : "Mark work in progress", () => command({ type: "set_wip", wip: !c.wip }, c, ["Change WIP state", [["Change", c.id], ["Revision", current()?.id], ["WIP", String(!c.wip)]]])));
    const upload = disclosure(card, "Upload a new exact revision"); const commit = field(upload, "Exact local commit", "text", "", [], "upload-commit");
    upload.append(writeButton("Register new revision", async () => { requireCurrent(); const result = await command({ type: "upload", commit: commit.value.trim() }, c, ["Upload revision", [["Change", c.id], ["Repository", c.repository], ["Previous revision", current()?.id], ["Source commit", commit.value.trim()], ["Target ref", c.target_ref]]]); if (result) { clearFields("upload-commit"); route({ review_revision: "", review_base: "base", review_file: "", review_side: "", review_line: "" }); } }));
    const lifecycle = node("div", null, "cr-toolbar");
    if (c.state === "open" || c.state === "abandoned") { const type = c.state === "abandoned" ? "restore" : "abandon"; lifecycle.append(writeButton(label(type), () => command({ type }, c, [label(type), [["Change", c.id], ["Source revision", current()?.id], ["Commit", current()?.commit], ["Target ref", c.target_ref]]]))); } card.append(lifecycle);
    const provider = disclosure(card, "Rebase, revert or cherry-pick through the admitted provider");
    provider.append(node("p", `Provider capabilities: ${arr(state.snapshot.capabilities).join(", ") || "No capabilities reported"}. These actions create review revisions or new changes, not unchecked target updates. Unsupported operations are rejected visibly by the service.`, "cr-muted"));
    const operation = field(provider, "Operation", "select", "rebase", ["rebase", "revert", "cherry_pick"], "maintenance-kind");
    const target = field(provider, "Target ref (cherry-pick)", "select", c.target_ref, arr(state.snapshot.target_refs), "maintenance-target");
    const resultTitle = field(provider, "New change title (revert / cherry-pick)", "text", `Revert ${c.title}`, [], "maintenance-title");
    provider.append(writeButton("Review and confirm exact operation", async () => {
      requireCurrent(); const epoch = state.epoch, id = selected(), r = current(); const kind = operation.value, targetRef = kind === "cherry_pick" ? target.value : c.target_ref;
      const observation = await state.context.api(`${ROOT}/targets?${new URLSearchParams({ commit: r.commit, target_ref: targetRef })}`);
      if (!active(epoch, id)) return;
      const expected = observation?.source?.current_target;
      if (!expected) throw new Error("The exact current target could not be observed. No provider action was sent.");
      const action = { type: kind, expected_target: expected }; if (kind !== "rebase") action.title = resultTitle.value; if (kind === "cherry_pick") action.target_ref = targetRef;
      const result = await command(action, c, [label(kind), [["Principal", state.context.principal], ["Repository", c.repository], ["Change", c.id], ["Source revision", r.id], ["Source commit", r.commit], ["Target ref", targetRef], ["Expected target commit", expected], ["Result", kind === "rebase" ? "New revision on this review; target unchanged" : `New change: ${resultTitle.value}; target unchanged`]]]);
      const resultId = result?.operation?.result_change || result?.change?.id;
      if (resultId && resultId !== c.id) route({ review_change: resultId, review_revision: "", review_base: "", review_file: "", review_side: "", review_line: "" });
    }));
  }
  async function loadDiff() {
    const epoch = state.epoch, id = selected(), rev = revision(), base = state.route.review_base || "base", request = ++state.diffRequest;
    if (!arr(state.detail?.change?.revisions).some(r => r.id === rev)) return;
    try {
      const diff = await state.context.api(`${path(id)}/diff?${new URLSearchParams({ revision: rev, base, whitespace: state.route.review_whitespace || "exact" })}`);
      if (!active(epoch, id) || request !== state.diffRequest || rev !== revision()) return;
      const selectedRevision = arr(state.detail.change.revisions).find(r => r.id === rev);
      const expectedOld = base === "base" ? selectedRevision?.base : arr(state.detail.change.revisions).find(r => r.id === base)?.commit;
      if (diff?.new_commit !== selectedRevision?.commit || diff?.old_commit !== expectedOld || !Array.isArray(diff.files)) throw new Error("Diff identity does not match the exact selected revision and comparison base.");
      state.diff = diff; state.diffError = ""; draw();
    } catch (error) { if (active(epoch, id) && request === state.diffRequest) { state.diff = null; state.diffError = message(error); drawDiff(); } }
  }
  function filePath(file) { return file.new_path || file.old_path || ""; }
  function anchorRoute(anchor) {
    const revisions = arr(state.detail?.change?.revisions), r = revisions.find(item => item.id === anchor.revision);
    const base = anchor.base_commit === r?.base ? "base" : revisions.find(item => item.commit === anchor.base_commit)?.id;
    return { review_revision: anchor.revision, review_base: base || "base", review_file: anchor.file, review_side: anchor.side, review_line: String(anchor.start_line) };
  }
  function drawDiff() {
    const host = state.container?.querySelector(".cr-diff-host"); if (!host) return; host.replaceChildren();
    if (state.diffError) { host.append(node("p", state.diffError, "cr-notice")); return; }
    if (!state.diff) { host.append(node("p", "Loading the exact provider diff…", "cr-muted")); return; }
    const comparison = state.diff;
    facts(host, [["Old commit", comparison.old_commit], ["New commit", comparison.new_commit]]);
    if ((state.route.review_whitespace || "exact") !== "exact") host.append(node("p", "Whitespace-filtered comparison: omitted changes are not reviewed. Displayed line numbers and saved anchors remain exact provider coordinates; switch to exact to inspect all whitespace.", "cr-notice"));
    if (comparison.truncated) host.append(node("p", "The provider bounded this diff. Not all files or lines are shown; omitted content is not implicitly reviewed.", "cr-notice"));
    const files = node("nav", null, "cr-files"); files.setAttribute("aria-label", "Changed files");
    for (const file of comparison.files) {
      const name = filePath(file), row = node("div", null, "cr-file-row"); const a = link(`${file.renamed ? `${file.old_path} → ` : ""}${name}${file.binary ? " · binary" : ""}${file.truncated ? " · truncated" : ""}`, { review_file: name, review_side: "", review_line: "" });
      if (name === (state.route.review_file || filePath(comparison.files[0] || {}))) a.setAttribute("aria-current", "true");
      row.append(a); const marked = arr(state.detail.change.reviewed).some(m => m.actor === state.context.principal && m.revision === revision() && m.file === name);
      const mark = writeButton(marked ? "Reviewed ✓" : "Mark reviewed", () => { requireCurrent(); return command({ type: "mark_reviewed", file: name, reviewed: !marked }); });
      if (revision() !== current()?.id || file.binary || file.truncated || comparison.truncated || (state.route.review_whitespace || "exact") !== "exact") { mark.disabled = true; mark.dataset.crBlocked = "true"; mark.title = "Only a complete exact (not whitespace-filtered) textual file on the current revision can be marked reviewed here."; }
      row.append(mark); files.append(row);
    }
    host.append(files);
    if (!comparison.files.length) { host.append(node("p", "The exact comparison contains no changed files.", "cr-muted")); return; }
    const wantedFile = state.route.review_file || filePath(comparison.files[0]);
    const file = comparison.files.find(f => f.new_path === wantedFile || f.old_path === wantedFile);
    if (!file) { host.append(node("p", "The requested file is not present in this exact comparison. Select a listed file.", "cr-notice")); return; }
    host.append(node("h4", filePath(file)), node("p", file.binary ? "Binary content: no textual line anchors are available." : "Line numbers are permalinks. Use the anchored draft editor below to discuss an exact line or range.", "cr-muted"));
    if (file.binary) return;
    const scroll = node("div", null, "cr-diff-scroll"); scroll.tabIndex = 0; scroll.setAttribute("role", "region"); scroll.setAttribute("aria-label", `${state.mode === "split" ? "Side-by-side" : "Unified"} diff for ${filePath(file)}`);
    const table = node("table", null, `cr-diff cr-${state.mode}`); const caption = node("caption", `${filePath(file)} · ${short(comparison.old_commit)} → ${short(comparison.new_commit)}`); table.append(caption);
    const head = node("thead"), hr = node("tr"); for (const text of state.mode === "split" ? ["Base line", "Base content", "Revision line", "Revision content"] : ["Base line", "Revision line", "Change", "Content"]) { const th = node("th", text); th.scope = "col"; hr.append(th); } head.append(hr); table.append(head);
    const body = node("tbody");
    const coordinate = (line, side) => {
      const td = node("td", null, "cr-line-number"); if (line != null) { const a = link(line, { review_revision: revision(), review_base: state.route.review_base || "base", review_file: (side === "base" ? file.old_path : file.new_path) || filePath(file), review_side: side, review_line: String(line) }); a.setAttribute("aria-label", `${side} line ${line}`); if (state.route.review_side === side && Number(state.route.review_line) === line) { a.classList.add("cr-line-selected"); a.setAttribute("aria-current", "location"); } td.append(a); } return td;
    };
    for (const hunk of arr(file.hunks)) {
      const marker = node("tr", null, "cr-hunk"); const title = node("th", hunk.header || `@@ -${hunk.old_start},${hunk.old_lines} +${hunk.new_start},${hunk.new_lines} @@`); title.colSpan = 4; marker.append(title); body.append(marker);
      for (const line of arr(hunk.lines)) {
        const tr = node("tr", null, `cr-diff-${line.kind}`);
        if (state.mode === "split") { tr.append(coordinate(line.old_line, "base"), codeCell(line.old_line == null ? "" : line.text, filePath(file)), coordinate(line.new_line, "revision"), codeCell(line.new_line == null ? (line.kind === "no_newline" ? line.text : "") : line.text, filePath(file))); }
        else tr.append(coordinate(line.old_line, "base"), coordinate(line.new_line, "revision"), node("td", ({ addition: "+", deletion: "−", context: " ", no_newline: "\\" })[line.kind] || "", "cr-diff-sign"), codeCell(line.text, filePath(file)));
        body.append(tr);
      }
    }
    table.append(body); scroll.append(table); host.append(scroll);
    const target = host.querySelector(".cr-line-selected"); if (target) target.scrollIntoView({ block: "nearest", inline: "nearest" });
    if (file.truncated) host.append(node("p", "This file is truncated. Missing text cannot be anchored, and this display cannot certify the whole file as reviewed.", "cr-notice"));
    const related = arr(state.detail.change.threads).filter(thread => thread.anchor.revision === revision() && thread.anchor.base_commit === comparison.old_commit && [file.old_path, file.new_path].includes(thread.anchor.file));
    for (const thread of related) { const line = thread.anchor.start_line; const summary = link(`${thread.resolved ? "Resolved" : "Open"}${thread.blocking ? " blocking" : ""} thread · ${thread.anchor.side} ${line}–${thread.anchor.end_line}`, anchorRoute(thread.anchor)); summary.classList.add("cr-thread-link"); host.append(summary); }
  }
  function drawDiscussion(root) {
    const c = state.detail.change, card = section(root, "Anchored discussions and private review drafts");
    const threads = arr(c.threads).filter(thread => thread.anchor.revision === revision());
    for (const thread of threads) {
      const article = node("article", null, "cr-thread"); const a = thread.anchor;
      article.append(link(`${a.file} · ${a.side} lines ${a.start_line}–${a.end_line} · base ${short(a.base_commit)}`, anchorRoute(a)), node("p", `${thread.resolved ? "Resolved" : "Open"}${thread.blocking ? " · Blocking" : " · Non-blocking"}`, "cr-muted"));
      for (const comment of arr(thread.comments)) { const item = node("div", null, "cr-comment"); item.append(node("h4", `${comment.author} · ${new Date(comment.created_ms).toLocaleString()}`), markdown(comment.body)); article.append(item); }
      const actions = node("div", null, "cr-toolbar");
      actions.append(writeButton(thread.resolved ? "Unresolve" : "Resolve", () => command({ type: "resolve", thread: thread.id, resolved: !thread.resolved }, c)));
      const reply = field(article, "Private reply", "textarea", "", [], `reply-${thread.id}`);
      actions.append(writeButton("Save private reply draft", async () => { if (!reply.value.trim()) throw new Error("Reply text is required."); const result = await command({ type: "save_draft", anchor: a, thread: thread.id, body: reply.value, blocking: thread.blocking }, c); if (result) { clearFields(`reply-${thread.id}`); draw(); } })); article.append(actions); card.append(article);
    }
    if (!threads.length) card.append(node("p", "No published threads on the selected revision.", "cr-muted"));
    const compose = disclosure(card, "Write an anchored private draft");
    if (state.route.review_line) compose.parentElement.open = true;
    compose.append(node("p", "Choose a line number in the diff to seed this editor. Draft text is private until publication. Both sides bind the exact displayed old commit, selected revision and side-specific file path, including historical comparisons.", "cr-muted"));
    const choices = state.diff ? [...new Set(state.diff.files.flatMap(f => [f.old_path, f.new_path]).filter(Boolean))] : arr(state.detail.change.revisions).find(r => r.id === revision())?.files || [];
    const file = field(compose, "Exact file", "select", state.route.review_file || choices[0] || "", choices, "draft-file");
    const side = field(compose, "Side", "select", state.route.review_side || "revision", ["base", "revision"], "draft-side");
    const start = field(compose, "First line", "number", state.route.review_line || 1, [], "draft-start"); start.min = "1"; start.step = "1";
    const end = field(compose, "Last line", "number", state.route.review_line || 1, [], "draft-end"); end.min = "1"; end.step = "1";
    const body = field(compose, "Comment", "textarea", "", [], "draft-body"); const blocking = field(compose, "Blocks submission until resolved", "checkbox", true, [], "draft-blocking");
    compose.append(node("p", "Markdown: headings, lists, emphasis, links and fenced code. Raw HTML and image loading are not supported. Mention a configured principal as @principal. To propose an applyable replacement, use a revision-side range and one complete ```suggestion fence containing the replacement text. Surrounding prose is allowed.", "cr-muted"));
    const preview = node("div"); compose.append(preview, button("Preview comment Markdown", () => { preview.replaceChildren(markdown(body.value)); }));
    compose.append(writeButton("Save private draft", async () => {
      if (!state.diff || state.diff.new_commit !== arr(c.revisions).find(r => r.id === revision())?.commit) throw new Error("Wait for the exact comparison before saving an anchored draft.");
      if (!body.value.trim()) throw new Error("Comment text is required.");
      const first = Number(start.value), last = Number(end.value); if (!Number.isSafeInteger(first) || !Number.isSafeInteger(last) || first < 1 || last < first) throw new Error("A valid inclusive positive line range is required.");
      const comparisonFile = state.diff.files.find(f => [f.old_path, f.new_path].includes(file.value));
      const exactPath = side.value === "base" ? comparisonFile?.old_path : comparisonFile?.new_path;
      if (!exactPath || comparisonFile.binary || comparisonFile.truncated || state.diff.truncated) throw new Error("The selected side has no complete admitted text for this file. No draft was saved.");
      const anchor = { revision: revision(), base_commit: state.diff.old_commit, file: exactPath, side: side.value, start_line: first, end_line: last };
      const result = await command({ type: "save_draft", anchor, thread: null, body: body.value, blocking: blocking.checked }, c);
      if (result) { clearFields("draft-body"); draw(); }
    }));
    const drafts = section(card, "My private drafts"); const publishIds = [];
    for (const draft of arr(state.detail.drafts)) {
      const article = node("article", null, "cr-draft"), a = draft.anchor; const currentDraft = a.revision === current()?.id;
      article.append(node("h4", `${draft.thread ? "Reply" : "New thread"} · ${a.file}:${a.start_line}–${a.end_line} (${a.side})${currentDraft ? "" : " · Historical revision"}`), node("p", `Comment revision: ${a.revision} · Comparison base: ${a.base_commit}`, "cr-muted"));
      const include = field(article, "Include in atomic publication", "checkbox", a.revision === revision(), [], `publish-${draft.id}`);
      publishIds.push({ id: draft.id, input: include, anchor: a });
      const draftBody = field(article, "Private draft text", "textarea", draft.body, [], `draft-edit-${draft.id}`);
      const actions = node("div", null, "cr-toolbar"); actions.append(writeButton("Save draft edits", async () => { const result = await command({ type: "save_draft", id: draft.id, anchor: a, thread: draft.thread, body: draftBody.value, blocking: draft.blocking }, c); if (result) { clearFields(`draft-edit-${draft.id}`); draw(); } }), writeButton("Delete private draft", () => command({ type: "delete_draft", id: draft.id }, c, ["Delete private draft", [["Change", c.id], ["Draft", draft.id], ["Anchor", `${a.file}:${a.start_line}–${a.end_line}`], ["Body", draft.body]]]))); article.append(actions); drafts.append(article);
    }
    if (!arr(state.detail.drafts).length) drafts.append(node("p", "No server-saved private drafts for this principal on this change.", "cr-muted"));
    const publish = section(card, "Publish review atomically");
    const eligible = arr(state.snapshot.policy?.eligible_reviewers).includes(state.context.principal) && c.owner !== state.context.principal && c.reviewers?.[state.context.principal] === "reviewer";
    const vote = field(publish, "Vote on the exact current revision", "select", "", eligible ? [["", "No vote"], ["approve", "Approve"], ["needs_revision", "Needs revision"], ["veto", "Veto"], ["abstain", "Abstain"]] : [["", "No vote — requires an eligible requested non-owner reviewer"]], "publish-vote");
    publish.append(node("p", `Only saved, selected drafts are published at their own original revision and comparison base. Unsaved editor changes are not included. An optional vote applies only to CURRENT revision ${current()?.id} (${current()?.commit}), independently of historical comments.`, "cr-muted"));
    publish.append(writeButton("Publish selected drafts and vote", async () => {
      const chosen = publishIds.filter(item => item.input.checked), ids = chosen.map(item => item.id);
      if (!ids.length && !vote.value) throw new Error("Select at least one saved draft or an eligible vote.");
      const savedText = chosen.map(item => `${item.id}\n${arr(state.detail.drafts).find(draft => draft.id === item.id)?.body ?? "Draft unavailable; refresh before publication"}`).join("\n\n");
      const result = await command({ type: "publish", draft_ids: ids, vote: vote.value || null }, c, ["Publish review", [
        ["Principal", state.context.principal], ["Change", c.id], ["Vote revision (current)", current()?.id], ["Vote commit (current)", current()?.commit],
        ["Comment anchors", chosen.map(item => `${item.id}: revision ${item.anchor.revision}, base ${item.anchor.base_commit}, ${item.anchor.file}:${item.anchor.start_line}–${item.anchor.end_line} (${item.anchor.side})`).join("\n") || "None"],
        ["Exact saved comment text", savedText || "None"], ["Vote", vote.value || "None"],
      ]]);
      if (result) { clearFields("publish-vote", ...ids.map(id => `publish-${id}`), ...ids.map(id => `draft-edit-${id}`)); draw(); }
    }, "primary"));
  }
  function clear() {
    state.epoch++; state.request++; state.diffRequest++;
    for (const dialog of state.dialogs) { dialog.close(); dialog.remove(); } state.dialogs.clear();
    state.container?.replaceChildren(); state.container = null; state.context = null; state.snapshot = null; state.detail = null; state.diff = null; state.forms.clear(); state.pending = false; state.pinnedRevision = "";
    state.error = ""; state.diffError = ""; state.route = {};
  }
  function load(container, context) {
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
  window.KilnCodeReview = Object.freeze({ load, refresh, clear });
})();
