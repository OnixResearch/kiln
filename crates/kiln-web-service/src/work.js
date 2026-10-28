/* Native work COB workspace. The server remains canonical; browser state is a bounded snapshot. */
(() => {
  "use strict";
  const KINDS = ["issue", "project", "milestone", "cycle", "initiative", "team", "request", "view", "personal"];
  const PLANNING = ["project", "milestone", "cycle", "initiative", "team"];
  const STATUSES = ["backlog", "triage", "started", "completed", "cancelled", "snoozed", "duplicate"];
  const PRIORITIES = ["No priority", "Urgent", "High", "Normal", "Low"];
  const LINK_KINDS = ["source", "revision", "patch", "review", "run", "artifact", "cairn"];
  const FILTERS = ["kind", "status", "assignee", "priority", "label", "team", "project", "cycle", "initiative", "q"];
  const ROUTE_FILTERS = { kind: "work_kind", status: "work_status", assignee: "work_assignee", project: "work_project", q: "work_q" };
  const state = { parent: null, route: {}, records: [], configured: false, canWrite: false, principal: "", generation: 0, load: 0, detail: 0, dialogs: new Set(), scroll: new Map(), pending: false, error: "" };
  const previewRequests = new WeakMap();
  const bridge = () => window.KilnWork;
  const el = (tag, text, cls) => { const node = document.createElement(tag); if (text !== undefined) node.textContent = String(text); if (cls) node.className = cls; return node; };
  const clone = value => JSON.parse(JSON.stringify(value));
  const split = value => [...new Set(String(value || "").split(/[,\n]/).map(s => s.trim()).filter(Boolean))];
  const title = value => String(value || "").replaceAll("_", " ").replace(/^./, s => s.toUpperCase());
  const date = value => value == null ? "Not scheduled" : new Date(value).toLocaleString();
  const identity = () => bridge()?.session()?.principal || "";
  const valid = (generation, principal) => state.generation === generation && identity() === principal && !!principal;
  const writable = () => state.configured && state.canWrite && !!bridge()?.session()?.can_write && identity() === state.principal;
  const recordFor = id => state.records.find(record => record.id === id);
  const nameFor = id => recordFor(id)?.fields.title || id || "None";
  const pathFor = id => `/api/work/objects/${encodeURIComponent(id)}`;
  const errorText = error => error?.message || String(error);
  function report(error) { if (identity()) bridge().notice(errorText(error)); }
  function button(text, action, cls, write = false) {
    const node = el("button", text, cls); node.type = "button";
    node.disabled = write && !writable();
    if (node.disabled) node.title = "An admitted work writer and writable native store are required; CI operator authority is separate.";
    node.addEventListener("click", () => { Promise.resolve().then(action).catch(report); });
    return node;
  }
  function section(parent, heading, cls = "work-card") { const node = el("section", undefined, cls); node.append(el("h3", heading)); parent.append(node); return node; }
  function facts(parent, pairs) {
    const dl = el("dl", undefined, "work-facts");
    for (const [key, value] of pairs) { dl.append(el("dt", key), el("dd", value == null || value === "" ? "Not recorded" : value)); }
    parent.append(dl);
  }
  function field(parent, label, type = "text", value = "", options) {
    const wrap = el("label", undefined, "work-field"); wrap.append(el("span", label));
    let input;
    if (type === "select") {
      input = el("select");
      for (const choice of options || []) { const [id, text] = Array.isArray(choice) ? choice : [choice, title(choice)]; const option = el("option", text); option.value = id; input.append(option); }
      if (value && !Array.from(input.options).some(option => option.value === String(value))) { const option = el("option", nameFor(value)); option.value = value; input.append(option); }
    } else if (type === "textarea") { input = el("textarea"); input.rows = 4; }
    else { input = el("input"); input.type = type; }
    if (type === "checkbox") input.checked = !!value;
    else input.value = value == null ? "" : String(value);
    wrap.append(input); parent.append(wrap); return input;
  }
  function relation(parent, label, kind, value, multiple = false, exclude = "") {
    const choices = state.records.filter(record => (Array.isArray(kind) ? kind.includes(record.kind) : record.kind === kind) && record.id !== exclude);
    const input = field(parent, label, "select", multiple ? "" : value || "", [["", multiple ? "Choose records (Ctrl/Command or Shift to select several)" : "None"], ...choices.map(record => [record.id, `${record.fields.title} · ${record.id.slice(0, 12)}`])]);
    if (multiple) {
      input.options[0].remove(); input.multiple = true; input.size = Math.min(5, Math.max(2, choices.length));
      const ids = value || [];
      for (const id of ids) if (!Array.from(input.options).some(option => option.value === id)) { const option = el("option", nameFor(id)); option.value = id; input.append(option); }
      for (const option of input.options) option.selected = ids.includes(option.value);
    }
    return input;
  }
  const selected = input => Array.from(input.selectedOptions).map(option => option.value).filter(Boolean);
  function datetime(value) { if (value == null) return ""; const d = new Date(value); return new Date(value - d.getTimezoneOffset() * 60000).toISOString().slice(0, 16); }
  const timestamp = input => input.value ? new Date(input.value).getTime() : null;
  function defaults(kind) {
    return { title: "", description: "", status: kind === "request" ? "triage" : "backlog", priority: 0, owner: ["personal", "view"].includes(kind) ? state.principal : "", assignees: [], labels: [], parent: null, blocked_by: [], team: null, project: null, milestone: null, cycle: null, initiative: null, start_at_ms: null, due_at_ms: null, customer: null, source_url: null, duplicate_of: null, links: [], workflow: [], view: kind === "view" ? { filters: {}, sort: "updated", group_by: "none", layout: "list", shared: false } : null, personal: kind === "personal" ? { watched: [], read: {}, favorites: [], reminders: {} } : null };
  }
  function scrollKey() { const { work_id, ...rest } = state.route; return JSON.stringify(rest); }
  function saveScroll() { const list = state.parent?.querySelector(".work-list-scroll"); if (list) state.scroll.set(scrollKey(), { top: list.scrollTop, left: list.scrollLeft }); }
  function clear() {
    state.generation++; state.load++; state.detail++; saveScroll();
    for (const dialog of state.dialogs) dialog.remove();
    state.dialogs.clear(); state.records = []; state.parent?.replaceChildren(); state.parent = null;
    state.principal = ""; state.canWrite = false; state.configured = false; state.pending = false; state.error = ""; state.scroll.clear();
  }
  function objects() { return identity() === state.principal ? clone(state.records) : []; }
  function open(id) { bridge().navigate({ work_id: id || "" }); }
  function modal(heading) {
    const dialog = el("dialog", undefined, "work-dialog");
    const header = el("div", undefined, "work-dialog-heading"); const h = el("h2", heading); h.id = `work-dialog-${crypto.randomUUID()}`; dialog.setAttribute("aria-labelledby", h.id);
    const close = () => { dialog.close(); dialog.remove(); state.dialogs.delete(dialog); };
    header.append(h, button("Close", close)); dialog.append(header);
    dialog.addEventListener("cancel", event => { event.preventDefault(); close(); });
    state.dialogs.add(dialog); document.body.append(dialog); dialog.showModal(); return { dialog, close };
  }
  async function submitCaptured(captures, label) {
    if (!writable()) throw new Error("Work writes are unavailable for this session or store.");
    if (state.pending) throw new Error("A work command is already in flight. Wait for its observed result.");
    const generation = state.generation, principal = identity();
    const summary = captures.map(({ record, action }) => {
      const fields = action.fields;
      const change = action.type === "edit" ? Object.keys(fields).filter(key => JSON.stringify(fields[key]) !== JSON.stringify(record?.fields[key])).map(key => `${title(key)}: ${JSON.stringify(fields[key])}`).join("\n") : JSON.stringify(action);
      return `${record ? `${record.fields.title}\nObject: ${record.id}\nExpected head: ${record.head}` : `New ${action.kind}`}\n${change}`;
    }).join("\n\n");
    if (!window.confirm(`${label}\n\nPrincipal: ${principal}\n${summary}\n\nPersist these signed native work COB commands? This does not mutate any provider or grant CI acceptance authority.`)) return null;
    if (!valid(generation, principal) || !writable()) throw new Error("The session changed before confirmation completed. No command was sent.");
    state.pending = true; const results = [];
    try {
      for (const { record, action } of captures) {
        if (!valid(generation, principal)) throw new Error("Session changed. Remaining commands were not sent.");
        const result = await bridge().command({ request_id: crypto.randomUUID(), object_id: record?.id || null, expected_head: record?.head || null, action });
        if (!valid(generation, principal)) return null;
        if (!result?.object?.id || !result.object.head) throw new Error("No durable work record was returned. Refresh before another command; do not blindly retry.");
        results.push(result.object);
      }
    } catch (error) {
      if (valid(generation, principal)) { await refresh(); throw new Error(`${errorText(error)} ${results.length} command(s) were observed complete. Remaining commands were not automatically retried.`); }
      return null;
    } finally { if (valid(generation, principal)) state.pending = false; }
    if (!valid(generation, principal)) return null;
    await refresh();
    if (!valid(generation, principal)) return null;
    bridge().notice(`${results.length} signed work command(s) persisted.`);
    return results;
  }
  async function refresh() {
    if (!state.parent || !identity()) return;
    const generation = state.generation, principal = identity(), request = ++state.load;
    try {
      const snapshot = await bridge().api("/api/work");
      if (!valid(generation, principal) || request !== state.load) return;
      if (snapshot?.schema !== "kiln.work.v1" || snapshot.bounded !== true || !Array.isArray(snapshot.objects) || snapshot.principal !== principal) throw new Error("The work snapshot is invalid or belongs to a different principal.");
      state.records = snapshot.objects; state.configured = snapshot.configured === true; state.canWrite = snapshot.can_write === true; state.principal = principal; state.error = "";
      draw();
    } catch (error) {
      if (!valid(generation, principal) || request !== state.load) return;
      state.records = []; state.canWrite = false; state.error = errorText(error); draw();
    }
  }
  async function render(parent, route) {
    const principal = identity(); if (!principal) { clear(); return; }
    if (state.principal && state.principal !== principal) clear();
    saveScroll(); const sameParent = state.parent === parent;
    state.parent = parent; state.route = { ...route }; state.principal = principal;
    if (!sameParent) { state.generation++; state.detail++; }
    if (sameParent && state.configured) { draw(false); return; }
    parent.replaceChildren(el("p", "Loading signed work records…", "small")); await refresh();
  }
  function personalRecord() { return state.records.filter(record => record.kind === "personal" && record.fields.owner === state.principal).sort((a, b) => a.created_at_ms - b.created_at_ms || a.id.localeCompare(b.id))[0]; }
  function personal() { return personalRecord()?.fields.personal || { watched: [], read: {}, favorites: [], reminders: {} }; }
  function mentions(record) { return [...(record.comments || []), ...(record.updates || [])].some(message => (message.mentions || []).includes(state.principal)); }
  function attention(record) {
    const p = personal(), reasons = [];
    if ((record.fields.assignees || []).includes(state.principal) || record.fields.owner === state.principal) reasons.push("Assigned to you");
    if (mentions(record)) reasons.push("Mentioned");
    if ((p.watched || []).includes(record.id)) reasons.push("Watching");
    if (p.reminders?.[record.id] != null && p.reminders[record.id] <= Date.now()) reasons.push("Reminder due");
    return reasons;
  }
  function activeView() { const record = recordFor(state.route.work_view); return record?.kind === "view" ? record.fields.view : null; }
  function configuration() {
    const saved = activeView(), filters = { ...(saved?.filters || {}) };
    for (const [key, route] of Object.entries(ROUTE_FILTERS)) if (state.route[route]) filters[key] = state.route[route];
    return { filters, sort: state.route.work_sort || saved?.sort || "updated", group_by: state.route.work_group || saved?.group_by || "none", layout: state.route.work_layout || saved?.layout || "list" };
  }
  function matches(record, filters) {
    const f = record.fields;
    return Object.entries(filters).every(([key, value]) => {
      if (!value) return true;
      if (key === "q") return [f.title, f.description, f.customer, f.source_url, ...(f.labels || []), record.id].filter(Boolean).join(" ").toLocaleLowerCase().includes(value.toLocaleLowerCase());
      if (key === "kind") return record.kind === value;
      if (key === "assignee") return (f.assignees || []).includes(value);
      if (key === "label") return (f.labels || []).includes(value);
      return String(f[key] ?? "") === value;
    });
  }
  function visibleRows(config) {
    if (state.route.work_view && !activeView()) return [];
    const page = state.route.page;
    const records = state.records.filter(record => {
      if (record.kind === "personal" && config.filters.kind !== "personal") return false;
      if (page === "planning" && !PLANNING.includes(record.kind)) return false;
      if (page === "triage" && record.kind !== "request") return false;
      if (page === "workviews" && record.kind !== "view") return false;
      if (page === "work" && !state.route.work_kind && !activeView() && !["issue", "request"].includes(record.kind)) return false;
      if (page === "inbox" && (!attention(record).length || personal().read?.[record.id] === record.head && !(personal().reminders?.[record.id] != null && personal().reminders[record.id] <= Date.now()))) return false;
      return matches(record, config.filters);
    });
    const compare = { updated: (a, b) => b.updated_at_ms - a.updated_at_ms, created: (a, b) => b.created_at_ms - a.created_at_ms, priority: (a, b) => (a.fields.priority || 5) - (b.fields.priority || 5), title: (a, b) => a.fields.title.localeCompare(b.fields.title), due: (a, b) => (a.fields.due_at_ms ?? Infinity) - (b.fields.due_at_ms ?? Infinity) }[config.sort] || (() => 0);
    return records.sort((a, b) => compare(a, b) || a.id.localeCompare(b.id));
  }
  function navigation(parent) {
    const nav = el("nav", undefined, "work-favorites"); nav.setAttribute("aria-label", "Saved work views and favorites");
    nav.append(el("span", "Views & favorites", "small"));
    for (const record of state.records.filter(record => record.kind === "view" || personal().favorites.includes(record.id))) {
      nav.append(button(`${record.kind === "view" ? record.fields.view?.shared ? "Shared: " : "Private: " : "★ "}${record.fields.title}`, () => record.kind === "view" ? bridge().navigate({ page: "work", work_view: record.id, work_id: "", work_kind: "", work_q: "", work_status: "", work_assignee: "", work_project: "", work_sort: "", work_group: "", work_layout: "" }) : open(record.id)));
    }
    parent.append(nav);
  }
  function toolbar(parent, config) {
    const top = el("div", undefined, "work-toolbar");
    const kind = field(top, "Create", "select", state.route.page === "triage" ? "request" : state.route.page === "planning" ? "project" : state.route.page === "workviews" ? "view" : "issue", KINDS);
    top.append(button("New record", () => create(kind.value), "primary", true), button("Refresh", refresh));
    top.append(button("Save current view", () => editor("view", null, { view: { ...config, shared: false } }), undefined, true));
    if (state.route.work_view) top.append(button("Clear saved view", () => bridge().navigate({ work_view: "" })));
    parent.append(top);
    const form = el("form", undefined, "work-filters"); const inputs = {};
    for (const key of ["q", "kind", "status", "assignee", "project"]) {
      const choices = key === "kind" ? [["", "All kinds"], ...KINDS] : key === "status" ? [["", "All statuses"], ...statuses()] : key === "project" ? [["", "All projects"], ...state.records.filter(r => r.kind === "project").map(r => [r.id, r.fields.title])] : null;
      inputs[key] = field(form, key === "q" ? "Search work" : title(key), choices ? "select" : "text", config.filters[key] || "", choices);
    }
    const layout = field(form, "Layout", "select", config.layout, ["list", "board"]);
    const group = field(form, "Group by", "select", config.group_by, ["none", "status", "assignee", "project", "priority", "team"]);
    const sort = field(form, "Sort by", "select", config.sort, ["updated", "created", "priority", "title", "due"]);
    const submit = el("button", "Apply filters"); submit.type = "submit"; form.append(submit);
    form.append(button("Reset filters", () => bridge().navigate({ work_view: "", work_kind: "", work_q: "", work_status: "", work_assignee: "", work_project: "", work_layout: "", work_group: "", work_sort: "" })));
    form.addEventListener("submit", event => { event.preventDefault(); const patch = { work_layout: layout.value, work_group: group.value, work_sort: sort.value }; for (const [key, input] of Object.entries(inputs)) patch[ROUTE_FILTERS[key]] = input.value.trim(); bridge().navigate(patch); });
    parent.append(form);
    if (state.route.work_view && !activeView()) parent.append(el("p", "Selected saved view is unavailable. No alternative view was selected.", "warning"));
    if (activeView()) parent.append(el("p", `Saved view: ${nameFor(state.route.work_view)} · ${Object.entries(config.filters).filter(([, v]) => v).map(([k, v]) => `${title(k)}: ${nameFor(v)}`).join(" · ") || "No filters"}`, "small"));
  }
  function statuses() { return [...new Set([...STATUSES, ...state.records.filter(record => record.kind === "team").flatMap(record => record.fields.workflow || [])])]; }
  function members(record) {
    const key = record.kind;
    if (!PLANNING.includes(key)) return [];
    return state.records.filter(other => other.id !== record.id && other.fields[key] === record.id);
  }
  function progress(parent, record) {
    const children = members(record); const done = children.filter(child => child.fields.status === "completed").length;
    const row = el("div", undefined, "work-progress");
    const meter = el("progress"); meter.max = Math.max(children.length, 1); meter.value = done; meter.setAttribute("aria-label", `${done} of ${children.length} linked records completed`);
    row.append(meter, el("span", `${done}/${children.length} linked completed${children.length ? ` · ${Math.round(done / children.length * 100)}%` : " · no linked scope"}`, "small")); parent.append(row);
  }
  function workRow(record, parent) {
    const row = el("article", undefined, "work-row"); row.dataset.workId = record.id; row.tabIndex = 0;
    row.setAttribute("aria-label", `${title(record.kind)}: ${record.fields.title}`);
    if (state.route.work_id === record.id) row.setAttribute("aria-current", "true");
    const main = button(record.fields.title || "Untitled", () => open(record.id), "work-row-title");
    const meta = el("div", undefined, "work-row-meta");
    meta.append(el("span", title(record.kind), "work-kind"), el("span", title(record.fields.status)), el("span", PRIORITIES[record.fields.priority] || "No priority"));
    if (record.fields.assignees?.length) meta.append(el("span", `Assigned: ${record.fields.assignees.join(", ")}`));
    if (record.fields.team) meta.append(el("span", nameFor(record.fields.team)));
    if (record.fields.project) meta.append(el("span", nameFor(record.fields.project)));
    if (record.fields.labels?.length) meta.append(el("span", record.fields.labels.join(" · ")));
    if (state.route.page === "inbox") meta.append(el("span", attention(record).join(" · ")));
    if (personal().favorites.includes(record.id)) meta.append(el("span", "★ Favorite"));
    if (record.fields.due_at_ms != null) meta.append(el("span", `Due ${date(record.fields.due_at_ms)}`));
    row.append(main, meta);
    if (PLANNING.includes(record.kind)) progress(row, record);
    parent.append(row);
  }
  function grouped(parent, records, config) {
    const groupBy = config.group_by === "none" && config.layout === "board" ? "status" : config.group_by;
    const groups = new Map();
    for (const record of records) {
      const raw = groupBy === "none" ? "All records" : groupBy === "assignee" ? record.fields.assignees?.join(", ") || "Unassigned" : groupBy === "priority" ? PRIORITIES[record.fields.priority] : record.fields[groupBy] || "Not set";
      const label = ["project", "team"].includes(groupBy) ? nameFor(raw) : title(raw);
      if (!groups.has(label)) groups.set(label, []); groups.get(label).push(record);
    }
    if (!records.length) parent.append(el("p", state.route.page === "inbox" ? "No unread assignments, mentions, watched changes or due reminders in the visible snapshot. Reading never changes your persisted state automatically." : "No matching records in this bounded snapshot. Adjust filters or create a record.", "work-empty"));
    parent.dataset.layout = config.layout;
    for (const [label, rows] of groups) { const group = el("section", undefined, "work-group"); group.append(el("h3", `${label} · ${rows.length}`)); for (const record of rows) workRow(record, group); parent.append(group); }
  }
  function roadmap(parent, records) {
    const plans = records.filter(record => PLANNING.includes(record.kind));
    const scheduled = plans.filter(record => record.fields.start_at_ms != null || record.fields.due_at_ms != null);
    const details = el("details", undefined, "work-roadmap"); details.open = true; details.append(el("summary", "Roadmap · scheduled planning records"));
    if (!scheduled.length) details.append(el("p", "No dated planning records in the current scope. Set start and due dates in a planning record to schedule it.", "small"));
    const times = scheduled.flatMap(record => [record.fields.start_at_ms, record.fields.due_at_ms]).filter(value => value != null);
    const min = Math.min(...times), max = Math.max(...times), span = Math.max(max - min, 86400000);
    for (const record of scheduled.sort((a, b) => (a.fields.start_at_ms ?? a.fields.due_at_ms) - (b.fields.start_at_ms ?? b.fields.due_at_ms))) {
      const row = el("div", undefined, "work-roadmap-row"); row.append(button(record.fields.title, () => open(record.id)));
      const track = el("div", undefined, "work-roadmap-track"), bar = el("div", undefined, "work-roadmap-bar");
      const start = record.fields.start_at_ms ?? record.fields.due_at_ms, end = record.fields.due_at_ms ?? start;
      bar.style.marginLeft = `${(start - min) / span * 90}%`; bar.style.width = `${Math.max(2, (end - start) / span * 90)}%`; track.append(bar);
      row.append(track, el("span", `${date(record.fields.start_at_ms)} → ${date(record.fields.due_at_ms)}`, "small")); details.append(row);
    }
    parent.append(details);
  }
  function draw(preserveScroll = true) {
    if (!state.parent || identity() !== state.principal) return;
    if (preserveScroll) saveScroll();
    const focused = document.activeElement?.closest("[data-work-id]")?.dataset.workId;
    const parent = state.parent; parent.replaceChildren(); parent.classList.add("work-workspace");
    if (state.error) { parent.append(el("p", `Work storage unavailable: ${state.error}. No empty or demo data has been substituted.`, "warning"), button("Reconnect", refresh)); return; }
    if (!state.configured) { parent.append(el("p", "Native work storage is not configured. Configure an explicit signed work COB store on the server to enable this workspace. No records or private preferences are stored in this browser.", "work-empty")); return; }
    if (!writable()) parent.append(el("p", "Read-only work workspace. Creating, editing, commenting and personal preferences require an admitted work writer and writable native store. CI operator authority is separate.", "small"));
    navigation(parent); const config = configuration(); toolbar(parent, config);
    if (state.route.page === "inbox") {
      const prefs = personalRecord(), tools = el("div", undefined, "work-actions");
      tools.append(button("Manage private attention record", () => prefs ? edit(prefs.id) : create("personal"), undefined, true), button("Browse my private records", () => bridge().navigate({ page: "work", work_kind: "personal", work_view: "", work_id: "", work_q: "", work_status: "", work_assignee: "", work_project: "" })));
      parent.append(tools, el("p", prefs ? `Attention uses your first-created private record: ${prefs.fields.title} (${prefs.id}). Additional private records remain separately editable under Personal records.` : "No private attention record exists yet. Your first explicit watch, favorite, read marker or reminder will offer to persist one.", "small"));
    }
    const records = visibleRows(config);
    if (state.route.page === "planning") roadmap(parent, records);
    const layout = el("div", undefined, "work-columns"); const list = el("div", undefined, "work-list-scroll"); list.setAttribute("aria-label", "Work records");
    grouped(list, records, config); const detail = el("aside", undefined, "work-detail"); detail.setAttribute("aria-label", "Selected work record");
    layout.append(list, detail); parent.append(layout);
    const position = state.scroll.get(scrollKey()); if (position) { list.scrollTop = position.top; list.scrollLeft = position.left; }
    if (focused) Array.from(list.querySelectorAll("[data-work-id]")).find(row => row.dataset.workId === focused)?.focus({ preventScroll: true });
    if (state.route.work_id) void loadDetail(state.route.work_id, detail).catch(report);
    else detail.append(el("p", "Select a record to inspect its metadata, discussion and exact linked evidence. Selection never marks an item as read automatically.", "small"));
    window.dispatchEvent(new CustomEvent("kiln-work-updated"));
  }
  async function loadDetail(id, parent, preview = false) {
    const generation = state.generation, principal = identity(), request = preview ? (previewRequests.get(parent) || 0) + 1 : ++state.detail;
    if (preview) previewRequests.set(parent, request);
    const current = () => valid(generation, principal) && parent.isConnected && (preview ? request === previewRequests.get(parent) : request === state.detail && state.route.work_id === id);
    parent.replaceChildren(el("p", "Reading exact signed record…", "small"));
    try {
      const record = await bridge().api(pathFor(id));
      if (!current()) return;
      if (record?.id !== id || !record.head || !KINDS.includes(record.kind)) throw new Error("Exact record response is invalid.");
      parent.replaceChildren(); renderDetail(record, parent, preview);
      const evidence = section(parent, "Linked change evidence");
      evidence.append(el("p", "Only explicitly linked identities are joined. Recorded CI outcomes, review claims and Cairn history are observations, not acceptance authority. Missing or inaccessible evidence is not success. No provider writes are available here.", "small"));
      const body = el("div"); evidence.append(body);
      if (!record.fields.links?.length) { body.append(el("p", "No explicit source, revision, patch, review, run, artifact or Cairn links are recorded.", "small")); return; }
      body.append(el("p", "Loading exact linked observations…", "small"));
      try {
        if (typeof bridge().changeEvidence !== "function") throw new Error("Exact evidence joining is unavailable in this configured shell.");
        const joined = await bridge().changeEvidence(record); if (!current()) return;
        body.replaceChildren(); recorded(body, joined);
      } catch (error) { if (current()) body.replaceChildren(el("p", `Linked evidence unavailable: ${errorText(error)}`, "warning")); }
    } catch (error) { if (current()) parent.replaceChildren(el("p", `Selected object unavailable: ${errorText(error)}. No alternate record was selected.`, "warning")); }
  }
  function recorded(parent, value, depth = 0) {
    if (value == null) { parent.append(el("span", "Unavailable (not recorded)")); return; }
    if (typeof value !== "object") { parent.append(el("span", value === "" ? "(empty)" : value, "work-text")); return; }
    if (depth > 7) { parent.append(el("pre", JSON.stringify(value, null, 2), "work-text")); return; }
    if (Array.isArray(value)) { if (!value.length) parent.append(el("p", "No linked observations in this category.", "small")); for (const item of value) { const row = el("div", undefined, "work-observation"); recorded(row, item, depth + 1); parent.append(row); } return; }
    const dl = el("dl", undefined, "work-facts");
    for (const [key, item] of Object.entries(value)) {
      const dd = el("dd");
      if (item && typeof item === "object") {
        const details = el("details");
        details.append(el("summary", `${title(key)}${Array.isArray(item) ? ` · ${item.length}` : " · exact observation"}`));
        recorded(details, item, depth + 1);
        dd.append(details);
      } else recorded(dd, item, depth + 1);
      dl.append(el("dt", title(key)), dd);
    }
    parent.append(dl);
  }
  function renderDetail(record, parent, preview) {
    const f = record.fields; const heading = el("div", undefined, "work-detail-heading"); heading.append(el("span", title(record.kind), "work-kind"), el("h2", f.title));
    heading.append(button(preview ? "Open full record" : "Close detail", () => open(preview ? record.id : ""))); parent.append(heading);
    parent.append(el("p", f.description || "No description recorded.", "work-text"));
    facts(parent, [["Status", f.status], ["Priority", PRIORITIES[f.priority]], ["Owner", f.owner], ["Assignees", f.assignees?.join(", ")], ["Labels", f.labels?.join(", ")], ["Start", date(f.start_at_ms)], ["Due / snooze until", date(f.due_at_ms)], ["Customer (original intake)", f.customer], ["Source (original intake)", f.source_url], ["Created by", record.created_by], ["Created", date(record.created_at_ms)], ["Updated", date(record.updated_at_ms)]]);
    const relations = section(parent, "Relationships");
    for (const [key, label] of [["parent", "Parent"], ["team", "Team"], ["project", "Project"], ["milestone", "Milestone"], ["cycle", "Cycle"], ["initiative", "Initiative"], ["duplicate_of", "Duplicate of"]]) if (f[key]) { const line = el("div", undefined, "work-relation"); line.append(el("span", `${label}: `), button(nameFor(f[key]), () => open(f[key]))); relations.append(line); }
    for (const id of f.blocked_by || []) relations.append(button(`Blocked by: ${nameFor(id)}`, () => open(id)));
    for (const child of state.records.filter(other => other.fields.parent === record.id)) relations.append(button(`${record.kind === "issue" ? "Sub-issue" : "Child " + record.kind}: ${child.fields.title}`, () => open(child.id)));
    for (const other of state.records.filter(other => other.fields.blocked_by?.includes(record.id))) relations.append(button(`Blocks: ${other.fields.title}`, () => open(other.id)));
    if (!relations.querySelector("button")) relations.append(el("p", "No typed relationships recorded.", "small"));
    if (PLANNING.includes(record.kind)) { const scope = section(parent, "Scope & progress"); progress(scope, record); for (const child of members(record)) scope.append(button(`${child.fields.title} · ${child.fields.status}`, () => open(child.id))); }
    if (record.kind === "team") facts(parent, [["Workflow", f.workflow?.join(" → ")]]);
    if (record.kind === "view") {
      const view = section(parent, f.view?.shared ? "Shared saved view" : "Private saved view"); recorded(view, f.view);
      if (!preview) {
        view.append(button("Use this view", () => bridge().navigate({ page: "work", work_view: record.id, work_kind: "", work_q: "", work_status: "", work_assignee: "", work_project: "", work_sort: "", work_group: "", work_layout: "", work_id: "" })));
        view.append(button(f.view?.shared ? "Create private copy" : "Create shared copy", () => editor("view", null, { ...clone(f), title: `${f.title} (${f.view?.shared ? "private" : "shared"} copy)`, owner: state.principal, view: { ...clone(f.view), shared: !f.view?.shared } }), undefined, true));
      }
    }
    if (record.kind === "personal") { const privateSection = section(parent, "Private attention choices"); recorded(privateSection, f.personal); }
    const links = section(parent, "Exact external identity links");
    for (const link of f.links || []) { const row = el("p", `${title(link.kind)}: ${link.id}`, "work-text"); if (link.kind === "run") row.append(" ", bridge().runLink(link.id, "Inspect exact run")); links.append(row); }
    if (!f.links?.length) links.append(el("p", "No linked identities.", "small"));
    if (!preview) {
      const actions = el("div", undefined, "work-actions"); actions.append(button("Edit metadata", () => editor(record.kind, record), undefined, true));
      if (record.kind === "issue") actions.append(button("Add sub-issue", () => editor("issue", null, { parent: record.id, team: f.team, project: f.project }), undefined, true));
      if (record.kind !== "personal") {
        const p = personal();
        actions.append(button(p.watched.includes(record.id) ? "Unwatch" : "Watch", () => preference("watch", [record.id]), undefined, true), button(p.favorites.includes(record.id) ? "Remove favorite" : "Favorite", () => preference("favorite", [record.id]), undefined, true), button(p.read[record.id] === record.head ? "Mark unread" : "Mark read", () => preference("read", [record.id], p.read[record.id] !== record.head), undefined, true), button("Set reminder", () => reminder(record.id), undefined, true));
        if (p.reminders[record.id] != null) actions.append(button("Clear reminder", () => preference("reminder", [record.id], null), undefined, true));
      }
      parent.append(actions);
      if (record.kind === "request") triage(record, parent);
      discussion(record, parent, "comments", false);
      if (["project", "team"].includes(record.kind)) discussion(record, parent, "updates", true);
    } else {
      for (const message of [...(record.comments || []), ...(record.updates || [])]) { const card = section(parent, `${message.author} · ${date(message.recorded_at_ms)}`); card.append(el("p", message.body, "work-text")); }
    }
    const identitySection = el("details"); identitySection.append(el("summary", "Signed record identity")); facts(identitySection, [["Object", record.id], ["Exact head", record.head], ["Authenticated application principal", record.created_by]]); identitySection.append(el("p", "The application signature attests the authenticated principal, not an end-user Radicle identity.", "small")); parent.append(identitySection);
  }
  async function edit(id) { const generation = state.generation, principal = identity(); const record = await bridge().api(pathFor(id)); if (valid(generation, principal)) editor(record.kind, record); }
  function create(kind = "issue") { if (!KINDS.includes(kind)) throw new Error("Unsupported work kind."); if (!writable()) throw new Error("Creation requires an admitted writable store."); return editor(kind); }
  function editor(kind, record = null, preset = {}) {
    if (!writable()) throw new Error("Editing is unavailable in this session.");
    const generation = state.generation, principal = identity(), f = { ...defaults(kind), ...(record ? clone(record.fields) : {}), ...clone(preset) };
    const { dialog, close } = modal(`${record ? "Edit" : "Create"} ${title(kind)}`); const form = el("form", undefined, "work-editor"); dialog.append(form);
    const base = section(form, "Details", "work-form-section");
    const inputs = { title: field(base, "Title", "text", f.title), description: field(base, "Description", "textarea", f.description) }; inputs.title.required = true; inputs.title.maxLength = 256;
    inputs.status = field(base, "Status", "select", f.status, statuses().filter(status => status !== "duplicate" || ["issue", "request"].includes(kind))); inputs.priority = field(base, "Priority", "select", f.priority, PRIORITIES.map((label, index) => [index, label]));
    inputs.owner = field(base, ["personal", "view"].includes(kind) ? "Private record owner (authenticated principal)" : "Owner principal", "text", f.owner); inputs.owner.readOnly = ["personal", "view"].includes(kind);
    inputs.assignees = field(base, "Assignee principals (comma separated)", "text", f.assignees.join(", ")); inputs.labels = field(base, "Labels (comma separated)", "text", f.labels.join(", "));
    const relations = section(form, "Typed relationships", "work-form-section");
    if (!["personal", "view"].includes(kind)) inputs.parent = relation(relations, `Parent ${kind}`, kind, f.parent, false, record?.id);
    if (!["personal", "view", "team"].includes(kind)) inputs.blocked_by = relation(relations, "Blocked by work / planning records", ["issue", "request", "project", "milestone", "cycle", "initiative"], f.blocked_by, true, record?.id);
    for (const target of PLANNING) inputs[target] = relation(relations, title(target), target, f[target], false, record?.id);
    if (["issue", "request"].includes(kind)) {
      inputs.duplicate_of = relation(relations, "Duplicate of", ["issue", "request"], f.duplicate_of, false, record?.id);
      inputs.duplicate_of.addEventListener("change", () => { if (inputs.duplicate_of.value) inputs.status.value = "duplicate"; else if (inputs.status.value === "duplicate") inputs.status.value = "backlog"; });
      inputs.status.addEventListener("change", () => { if (inputs.status.value !== "duplicate") inputs.duplicate_of.value = ""; });
      relations.append(el("p", "Choosing a duplicate target sets duplicate status. Leaving duplicate status clears that target in the new event; signed prior intake and history remain retained.", "small"));
    }
    const schedule = section(form, "Schedule", "work-form-section"); inputs.start_at_ms = field(schedule, "Starts at (local time)", "datetime-local", datetime(f.start_at_ms)); inputs.due_at_ms = field(schedule, "Due / snooze until (local time)", "datetime-local", datetime(f.due_at_ms));
    if (kind === "request") {
      const intake = section(form, "Original intake", "work-form-section");
      inputs.customer = field(intake, "Original customer", "text", f.customer); inputs.source_url = field(intake, "Original source URL (HTTP or HTTPS)", "url", f.source_url);
      if (record) { inputs.customer.readOnly = true; inputs.source_url.readOnly = true; intake.append(el("p", "Routing and triage preserve the original customer and source. Add later context in description, links or discussion.", "small")); }
    }
    if (kind === "team") { inputs.workflow = field(base, "Additional workflow statuses (comma separated)", "text", f.workflow.join(", ")); base.append(el("p", "Built-in statuses remain available. Issues assigned to this team may use its additional statuses.", "small")); }
    const linkSection = section(form, "Linked evidence identities", "work-form-section");
    linkSection.append(el("p", "Links are inert identities, not provider permissions. Source: repository@revision; patch/Cairn: repository|nativeType|object; review: receiptName; run: exact run ID; artifact: runID|artifactID. Unresolvable links remain explicitly unavailable.", "small"));
    const linkRows = el("div", undefined, "work-link-editor"); linkSection.append(linkRows);
    const addLink = (link = { kind: "run", id: "" }) => { const row = el("div", undefined, "work-link-row"); const type = field(row, "Link kind", "select", link.kind, LINK_KINDS), id = field(row, "Exact identity", "text", link.id); row.readLink = () => ({ kind: type.value, id: id.value.trim() }); row.append(button("Remove link", () => row.remove())); linkRows.append(row); };
    for (const link of f.links) addLink(link); linkSection.append(button("Add link", () => addLink()));
    const readView = kind === "view" ? viewEditor(form, f.view, !!record) : null;
    const readPersonal = kind === "personal" ? personalEditor(form, f.personal) : null;
    const status = el("p", "Changes persist only after explicit confirmation.", "small"); status.setAttribute("role", "status"); form.append(status);
    const actions = el("div", undefined, "work-actions"), save = el("button", record ? "Save metadata" : "Create signed record", "primary"); save.type = "submit"; actions.append(save, button("Cancel", close)); form.append(actions);
    form.addEventListener("submit", async event => {
      event.preventDefault(); if (!valid(generation, principal)) { close(); return; }
      const fields = clone(f);
      for (const key of ["title", "description", "status", "owner"]) fields[key] = inputs[key].value.trim();
      fields.priority = Number(inputs.priority.value); fields.assignees = split(inputs.assignees.value); fields.labels = split(inputs.labels.value);
      for (const key of ["parent", ...PLANNING, "duplicate_of"]) if (inputs[key]) fields[key] = inputs[key].value || null;
      if (inputs.blocked_by) fields.blocked_by = selected(inputs.blocked_by);
      fields.start_at_ms = timestamp(inputs.start_at_ms); fields.due_at_ms = timestamp(inputs.due_at_ms);
      if (kind === "request" && !record) { fields.customer = inputs.customer.value.trim() || null; fields.source_url = inputs.source_url.value.trim() || null; }
      if (inputs.workflow) fields.workflow = split(inputs.workflow.value);
      fields.links = Array.from(linkRows.children).map(row => row.readLink()).filter(link => link.id);
      if (readView) fields.view = readView(); if (readPersonal) fields.personal = readPersonal();
      save.disabled = true;
      try {
        const results = await submitCaptured([{ record, action: record ? { type: "edit", fields } : { type: "create", kind, fields } }], `${record ? "Update" : "Create"} ${title(kind)}: ${fields.title}`);
        if (results && valid(generation, principal)) { close(); open(results[0].id); }
      } catch (error) { if (valid(generation, principal)) status.textContent = errorText(error); }
      finally { if (dialog.isConnected) save.disabled = !writable(); }
    });
    inputs.title.focus();
  }
  function viewEditor(form, current, existing = false) {
    const view = current || { filters: {}, sort: "updated", group_by: "none", layout: "list", shared: false };
    const panel = section(form, "Saved view definition", "work-form-section"); const inputs = {};
    for (const key of FILTERS) {
      const options = key === "kind" ? [["", "Any kind"], ...KINDS] : key === "status" ? [["", "Any status"], ...statuses()] : key === "priority" ? [["", "Any priority"], ...PRIORITIES.map((label, index) => [index, label])] : PLANNING.includes(key) ? [["", `Any ${key}`], ...state.records.filter(r => r.kind === key).map(r => [r.id, r.fields.title])] : null;
      inputs[key] = field(panel, key === "q" ? "Search text" : title(key), options ? "select" : "text", view.filters?.[key] || "", options);
    }
    const sort = field(panel, "Sort", "select", view.sort, ["updated", "created", "priority", "title", "due"]), group = field(panel, "Group by", "select", view.group_by, ["none", "status", "assignee", "project", "priority", "team"]), layout = field(panel, "Layout", "select", view.layout, ["list", "board"]), shared = field(panel, "Shared with this workspace", "checkbox", view.shared);
    shared.disabled = existing;
    if (existing) panel.append(el("p", "Visibility is fixed for this signed history. Create a separate saved view to change sharing without exposing prior private events.", "small"));
    panel.append(el("p", "Private views are visible only to their authenticated owner. Sharing exposes this view definition, not private records or hidden fields.", "small"));
    return () => ({ filters: Object.fromEntries(Object.entries(inputs).map(([key, input]) => [key, input.value.trim()]).filter(([, value]) => value)), sort: sort.value, group_by: group.value, layout: layout.value, shared: shared.checked });
  }
  function personalEditor(form, current) {
    const p = current || { watched: [], read: {}, favorites: [], reminders: {} }; const panel = section(form, "Private attention preferences", "work-form-section");
    panel.append(el("p", "This native record is visible only to its authenticated owner. Read heads are captured from the current visible snapshot; no record is marked read on page load.", "small"));
    const kinds = KINDS.filter(k => k !== "personal"), watched = relation(panel, "Watched records", kinds, p.watched, true), favorites = relation(panel, "Favorite records", kinds, p.favorites, true), read = relation(panel, "Read records (retains saved head; new selections use current head)", kinds, Object.keys(p.read || {}), true);
    const reminders = el("div", undefined, "work-reminder-editor"); panel.append(reminders);
    const add = (id = "", at = null) => { const row = el("div", undefined, "work-link-row"); const target = relation(row, "Reminder record", kinds, id), when = field(row, "Remind at (local time)", "datetime-local", datetime(at)); row.readReminder = () => target.value && when.value ? [target.value, timestamp(when)] : null; row.append(button("Remove reminder", () => row.remove())); reminders.append(row); };
    for (const [id, at] of Object.entries(p.reminders || {})) add(id, at); panel.append(button("Add reminder", () => add()));
    return () => ({ watched: selected(watched), favorites: selected(favorites), read: Object.fromEntries(selected(read).map(id => [id, p.read?.[id] || recordFor(id)?.head]).filter(([, head]) => head)), reminders: Object.fromEntries(Array.from(reminders.children).map(row => row.readReminder()).filter(Boolean)) });
  }
  async function preference(type, ids, active) {
    if (!writable()) throw new Error("Private preference writes require an admitted writable work store.");
    const generation = state.generation, principal = identity();
    const existing = personalRecord(); const record = existing ? await bridge().api(pathFor(existing.id)) : null;
    const targets = type === "read" ? await Promise.all(ids.map(id => bridge().api(pathFor(id)))) : [];
    if (!valid(generation, principal)) return null;
    const fields = record ? clone(record.fields) : { ...defaults("personal"), title: "My attention", owner: principal };
    const p = fields.personal;
    for (const id of ids) {
      if (type === "watch" || type === "favorite") {
        const key = type === "watch" ? "watched" : "favorites", on = active ?? !p[key].includes(id);
        p[key] = on ? [...new Set([...p[key], id])] : p[key].filter(value => value !== id);
      } else if (type === "read") { if (active === false) delete p.read[id]; else p.read[id] = targets.find(target => target.id === id).head; }
      else if (type === "reminder") { if (active == null) delete p.reminders[id]; else p.reminders[id] = active; }
    }
    return submitCaptured([{ record, action: record ? { type: "edit", fields } : { type: "create", kind: "personal", fields } }], `Persist private ${type} choice for ${ids.map(nameFor).join(", ")}`);
  }
  function reminder(id) {
    const { dialog, close } = modal(`Reminder: ${nameFor(id)}`); const form = el("form"); const when = field(form, "Remind at (local time)", "datetime-local", datetime(personal().reminders[id])); when.required = true;
    const save = el("button", "Persist private reminder", "primary"); save.type = "submit"; form.append(save); dialog.append(form);
    form.addEventListener("submit", async event => { event.preventDefault(); save.disabled = true; try { if (await preference("reminder", [id], timestamp(when))) close(); } catch (error) { report(error); } finally { save.disabled = !writable(); } });
  }
  function triage(record, parent) {
    const panel = section(parent, "Request triage"); panel.append(el("p", "Decisions update this request's status without replacing its original customer or source. Edit metadata to route to an assignee, team or project.", "small"));
    const actions = el("div", undefined, "work-actions");
    actions.append(button("Accept to backlog", () => bulk({ type: "edit", fields: { status: "backlog", duplicate_of: null } }, [record.id]), undefined, true), button("Decline", () => bulk({ type: "edit", fields: { status: "cancelled" } }, [record.id]), undefined, true), button("Snooze", () => triageForm(record, "snooze"), undefined, true), button("Mark duplicate", () => triageForm(record, "duplicate"), undefined, true), button("Route / edit request", () => editor(record.kind, record), undefined, true)); panel.append(actions);
  }
  function triageForm(record, type) {
    const { dialog, close } = modal(`${title(type)} request: ${record.fields.title}`); const form = el("form");
    const input = type === "snooze" ? field(form, "Snooze until (local time)", "datetime-local", datetime(record.fields.due_at_ms)) : relation(form, "Duplicate of", ["issue", "request"], record.fields.duplicate_of, false, record.id); input.required = true;
    const save = el("button", `Persist ${type}`, "primary"); save.type = "submit"; form.append(save); dialog.append(form);
    form.addEventListener("submit", async event => { event.preventDefault(); save.disabled = true; try { const fields = type === "snooze" ? { status: "snoozed", due_at_ms: timestamp(input) } : { status: "duplicate", duplicate_of: input.value }; if (await bulk({ type: "edit", fields }, [record.id])) close(); } catch (error) { report(error); } finally { save.disabled = !writable(); } });
  }
  function discussion(record, parent, key, update) {
    const panel = section(parent, update ? "Project / team updates" : "Discussion");
    for (const message of record[key] || []) {
      const card = el("article", undefined, "work-message"); card.append(el("h4", `${message.author} · ${date(message.recorded_at_ms)}${message.edited_at_ms ? ` · edited ${date(message.edited_at_ms)}` : ""}`), el("p", message.body, "work-text"));
      if (message.mentions?.length) card.append(el("p", `Mentions: ${message.mentions.join(", ")}`, "small"));
      const reactions = el("div", undefined, "work-actions");
      for (const [emoji, actors] of Object.entries(message.reactions || {})) reactions.append(button(`${emoji} ${actors.length} · ${actors.join(", ")}`, () => messageAction(record, { type: "react", comment: message.id, emoji, active: !actors.includes(state.principal) }, `Toggle reaction ${emoji} on message by ${message.author}`), actors.includes(state.principal) ? "work-active" : "", true));
      if (message.author === state.principal) reactions.append(button(update ? "Edit update" : "Edit comment", () => commentEditor(record, message, update), undefined, true));
      reactions.append(button("React", () => reactionEditor(record, message), undefined, true)); card.append(reactions); panel.append(card);
    }
    if (!record[key]?.length) panel.append(el("p", update ? "No updates recorded." : "No comments recorded.", "small"));
    const form = el("form", undefined, "work-discussion-form"), body = field(form, update ? "New update" : "New comment", "textarea"), mentionsInput = field(form, "Mention exact principal IDs (comma separated)", "text"); body.required = true;
    const save = el("button", update ? "Post update" : "Post comment", "primary"); save.type = "submit"; save.disabled = !writable(); form.append(save); panel.append(form);
    form.addEventListener("submit", async event => { event.preventDefault(); save.disabled = true; try { await messageAction(record, { type: update ? "update" : "comment", body: body.value.trim(), mentions: split(mentionsInput.value) }, `Post ${update ? "update" : "comment"} to ${record.fields.title}`); } catch (error) { report(error); } finally { save.disabled = !writable(); } });
  }
  function messageAction(record, action, label) { return submitCaptured([{ record, action }], label); }
  function commentEditor(record, message, update = false) {
    const label = update ? "update" : "comment";
    const { dialog, close } = modal(`Edit your ${label}`), form = el("form");
    const body = field(form, title(label), "textarea", message.body), mentionsInput = field(form, "Mention exact principal IDs (comma separated)", "text", message.mentions.join(", ")); body.required = true;
    const save = el("button", `Save ${label}`, "primary"); save.type = "submit"; form.append(save); dialog.append(form);
    form.addEventListener("submit", async event => { event.preventDefault(); save.disabled = true; try { if (await messageAction(record, { type: "edit_comment", comment: message.id, body: body.value.trim(), mentions: split(mentionsInput.value) }, `Edit ${label} ${message.id} in ${record.fields.title}`)) close(); } catch (error) { report(error); } finally { save.disabled = !writable(); } });
  }
  function reactionEditor(record, message) {
    const { dialog, close } = modal("React to message"), form = el("form"), emoji = field(form, "Reaction emoji or short label", "text", "👍"); emoji.required = true; emoji.maxLength = 32;
    const active = field(form, "Add my reaction (uncheck to remove)", "checkbox", true), save = el("button", "Persist reaction", "primary"); save.type = "submit"; form.append(save); dialog.append(form);
    form.addEventListener("submit", async event => { event.preventDefault(); save.disabled = true; try { if (await messageAction(record, { type: "react", comment: message.id, emoji: emoji.value.trim(), active: active.checked }, `Change reaction on message ${message.id} in ${record.fields.title}`)) close(); } catch (error) { report(error); } finally { save.disabled = !writable(); } });
  }
  async function bulk(action, ids) {
    if (!writable()) throw new Error("Bulk writes require an admitted writable work store.");
    ids = [...new Set(ids || [])]; if (!ids.length) throw new Error("Select at least one work record.");
    if (typeof action === "string") action = { type: "status", status: action };
    if (["watch", "favorite", "read"].includes(action.type)) return preference(action.type, ids, action.active);
    let patch;
    if (action.type === "edit") patch = clone(action.fields || {});
    else if (action.type === "status") patch = { status: action.status };
    else if (action.type === "priority") patch = { priority: Number(action.priority) };
    else if (action.type === "assign") patch = { assignees: action.assignee ? [action.assignee] : [] };
    else throw new Error("Unsupported bulk work action.");
    const allowed = ["status", "priority", "owner", "assignees", "labels", "parent", "blocked_by", "team", "project", "milestone", "cycle", "initiative", "start_at_ms", "due_at_ms", "duplicate_of"];
    if (!Object.keys(patch).length || Object.keys(patch).some(key => !allowed.includes(key))) throw new Error("Bulk editing supports typed metadata only; intake, personal records and view definitions require their own forms.");
    const generation = state.generation, principal = identity();
    const records = await Promise.all(ids.map(id => bridge().api(pathFor(id))));
    if (!valid(generation, principal)) return null;
    if (records.some(record => ["personal", "view"].includes(record.kind))) throw new Error("Edit private records and saved views individually in their own forms.");
    if (patch.status && patch.status !== "duplicate") patch.duplicate_of = null;
    const captures = records.map(record => ({ record, action: { type: "edit", fields: { ...clone(record.fields), ...clone(patch) } } }));
    return submitCaptured(captures, `Apply metadata change to ${records.length} exact work record(s). Multi-record commands are sequential, not an atomic transaction.`);
  }
  async function preview(id, parent) { if (!identity()) return; return loadDetail(id, parent, true); }
  window.KilnWorkUI = { render, clear, objects, refresh, open, bulk, preview, create };
  window.addEventListener("kiln-session-changed", () => { if (!identity() || state.principal && identity() !== state.principal) clear(); });
  window.dispatchEvent(new CustomEvent("kiln-work-ready"));
})();
