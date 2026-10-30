/* Native work COB workspace. The server remains canonical; browser state is a bounded snapshot. */
(() => {
  "use strict";
  const KINDS = ["issue", "project", "milestone", "cycle", "initiative", "team", "request", "view", "personal"];
  const PLANNING = ["project", "milestone", "cycle", "initiative", "team"];
  const STATUSES = ["backlog", "triage", "started", "completed", "cancelled", "snoozed", "duplicate"];
  const PRIORITIES = ["No priority", "Urgent", "High", "Normal", "Low"];
  const SORT_LABELS = { updated: "last update", created: "newest first", priority: "priority", title: "title", due: "due date" };
  const LINK_KINDS = ["source", "revision", "patch", "review", "run", "artifact", "cairn"];
  const FILTERS = ["kind", "status", "assignee", "priority", "label", "team", "project", "cycle", "initiative", "q"];
  const ROUTE_FILTERS = { kind: "work_kind", status: "work_status", assignee: "work_assignee", project: "work_project", q: "work_q" };
  const state = { parent: null, route: {}, records: [], configured: false, canWrite: false, principal: "", generation: 0, load: 0, detail: 0, dialogs: new Set(), scroll: new Map(), pending: false, error: "", listCollapsed: false };
  const previewRequests = new WeakMap();
  const bridge = () => window.KilnWork;
  const el = (tag, text, cls) => { const node = document.createElement(tag); if (text !== undefined) node.textContent = String(text); if (cls) node.className = cls; return node; };
  const clone = value => JSON.parse(JSON.stringify(value));
  const split = value => [...new Set(String(value || "").split(/[,\n]/).map(s => s.trim()).filter(Boolean))];
  const title = value => String(value || "").replaceAll("_", " ").replace(/^./, s => s.toUpperCase());
  const quoted = value => `“${value || "Untitled"}”`;
  const date = value => value == null ? "Not scheduled" : new Date(value).toLocaleString();
  const shortDate = value => value == null ? "Not scheduled" : new Date(value).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
  const identity = () => bridge()?.session()?.principal || "";
  const valid = (generation, principal) => state.generation === generation && identity() === principal && !!principal;
  const writable = () => state.configured && state.canWrite && !!bridge()?.session()?.can_write && identity() === state.principal;
  const recordFor = id => state.records.find(record => record.id === id);
  const nameFor = id => recordFor(id)?.fields.title || id || "None";
  const pathFor = id => `/api/work/objects/${encodeURIComponent(id)}`;
  const errorText = error => error?.message || String(error);
  function report(error) { if (identity()) bridge().notice(errorText(error)); }
  // r[impl kiln.web.readable_observations]
  // Presentation only: KilnUi formats lazily at render time; exact values stay in title/datetime.
  const kui = () => window.KilnUi;
  const WORK_STATUS = { completed: "success", started: "running", backlog: "queued", triage: "pending", snoozed: "pending", cancelled: "cancelled", duplicate: "cancelled" };
  const icon = name => kui().icon(name);
  function when(value, missing = "Not recorded") {
    if (value == null) return el("span", missing, "meta");
    if (typeof kui()?.timeElement === "function") return kui().timeElement(value);
    const instant = new Date(value);
    if (Number.isNaN(instant.getTime())) return el("span", "Not recorded", "meta");
    const node = el("time", instant.toLocaleString()); node.dateTime = instant.toISOString(); return node;
  }
  function workStatus(status) {
    const kind = WORK_STATUS[status] || "unknown", pill = el("span", undefined, "status-pill"); pill.dataset.status = kind;
    pill.append(kui().statusIcon(kind === "pending" ? "queued" : kind));
    pill.append(el("span", status ? title(status) : "Not set")); pill.title = `Status: ${status || "not set"}`;
    return pill;
  }
  function priorityTag(priority) { const node = el("span", PRIORITIES[priority] || "No priority", "work-priority"); node.dataset.priority = String(priority || 0); return node; }
  function srOnly(text) { return el("span", text, "sr-only"); }
  function loading(text, rows = 3) {
    const wrap = el("div", undefined, "work-loading"), note = el("p", text, "meta"); note.setAttribute("role", "status"); wrap.append(note);
    for (let index = 0; index < rows; index++) { const bar = el("div", undefined, "skeleton work-skeleton-row"); bar.setAttribute("aria-hidden", "true"); wrap.append(bar); }
    return wrap;
  }
  function button(text, action, cls, write = false) {
    const node = el("button", text, cls); node.type = "button";
    node.disabled = write && !writable();
    if (node.disabled) node.title = "An admitted work writer and writable native store are required; CI operator authority is separate.";
    node.addEventListener("click", () => { Promise.resolve().then(action).catch(report); });
    return node;
  }
  function iconButton(label, name, action, write = false, cls = "") {
    const node = button(undefined, action, `icon-button quiet ${cls}`.trim(), write); node.setAttribute("aria-label", label);
    if (!node.disabled) node.title = label;
    node.append(icon(name));
    return node;
  }
  // r[impl kiln.web.design_system]
  // Design Guides web deviation: navigation to a console route stays a real link (new tab, history); commands stay buttons.
  function navLink(text, patch, cls) {
    const link = el("a", text, cls);
    link.href = `?${new URLSearchParams(Object.entries({ ...(bridge().route() || {}), ...patch }).filter(([, value]) => value))}`;
    link.addEventListener("click", event => {
      if (event.button === 0 && !event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) { event.preventDefault(); bridge().navigate(patch); }
    });
    return link;
  }
  const recordLink = (text, id, cls = "work-relation-link") => navLink(text, { work_id: id }, cls);
  const VIEW_RESET = { work_kind: "", work_q: "", work_status: "", work_assignee: "", work_project: "", work_sort: "", work_group: "", work_layout: "", work_id: "" };
  const viewPatch = id => ({ page: "work", work_view: id, ...VIEW_RESET });
  function footer(form, save, close) {
    const actions = el("div", undefined, "work-actions work-dialog-footer"); actions.append(button("Cancel", close, "quiet"), save); form.append(actions); return actions;
  }
  function section(parent, heading, cls = "work-card") { const node = el("section", undefined, cls); node.append(el("h3", heading)); parent.append(node); return node; }
  function disclosure(parent, heading) {
    const details = el("details", undefined, "work-disclosure"); details.append(kui().disclosureSummary(heading));
    const body = el("div", undefined, "work-form-section"); details.append(body); parent.append(details); return body;
  }
  function facts(parent, pairs, cls = "") {
    const dl = el("dl", undefined, `work-facts ${cls}`.trim());
    for (const [key, value] of pairs) {
      const dd = el("dd");
      if (value instanceof Node) dd.append(value); else dd.textContent = value == null || value === "" ? "Not recorded" : String(value);
      dl.append(el("dt", key), dd);
    }
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
    for (const dialog of state.dialogs) { if (dialog.open) dialog.close(); dialog.remove(); }
    state.dialogs.clear(); state.records = []; state.parent?.replaceChildren(); state.parent = null;
    state.principal = ""; state.canWrite = false; state.configured = false; state.pending = false; state.error = ""; state.scroll.clear(); state.listCollapsed = false;
    headerActions(null, false);
  }
  function objects() { return identity() === state.principal ? clone(state.records) : []; }
  function open(id) { bridge().navigate({ work_id: id || "" }); }
  function modal(heading, cls = "") {
    const dialog = el("dialog", undefined, `work-dialog ${cls}`.trim()), opener = document.activeElement;
    const header = el("div", undefined, "work-dialog-heading"); const h = el("h2", heading); h.id = `work-dialog-${crypto.randomUUID()}`; dialog.setAttribute("aria-labelledby", h.id);
    // Focus returns to the control that opened the dialog when it is still on the page.
    const close = () => { if (dialog.open) dialog.close(); dialog.remove(); state.dialogs.delete(dialog); if (opener instanceof HTMLElement && opener.isConnected && !opener.closest("[inert]")) opener.focus({ preventScroll: true }); };
    header.append(h, iconButton("Close", "x", close, false, "work-dialog-close")); dialog.append(header);
    dialog.addEventListener("cancel", event => { event.preventDefault(); close(); });
    state.dialogs.add(dialog); document.body.append(dialog); dialog.showModal(); return { dialog, close, opener };
  }
  // r[impl kiln.web.design_system]
  // Confirmation names the object and the verb; its body carries the same exact facts the signed command binds:
  // principal, object, expected head and every changed field. Dismissal resolves false and sends nothing.
  function confirmCommand(captures, wording, principal) {
    return new Promise(resolve => {
      const { dialog, close, opener } = modal(wording.title, "work-confirm");
      dialog.setAttribute("role", "alertdialog");
      let settled = false;
      // A dismissed confirmation returns focus to the submit control that opened it; the caller re-enables that
      // control only after this promise settles, so the focus runs once the current task's continuations finish.
      const settle = value => {
        if (settled) return; settled = true; resolve(value);
        if (!value) setTimeout(() => { if (opener instanceof HTMLElement && opener.isConnected && !opener.disabled) opener.focus({ preventScroll: true }); });
      };
      const finish = value => { if (settled) return; close(); settle(value); };
      dialog.addEventListener("close", () => settle(false));
      const body = el("div", undefined, "work-confirm-body"); body.id = `${dialog.getAttribute("aria-labelledby")}-body`; dialog.setAttribute("aria-describedby", body.id);
      if (wording.note) body.append(el("p", wording.note, "work-text"));
      facts(body, [["Principal", el("span", principal, "mono")]]);
      for (const { record, action } of captures) {
        const item = el("section", undefined, "work-confirm-item");
        item.append(el("h3", record ? record.fields.title : `New ${kindLabel(action.kind)}`));
        const pairs = record ? [["Object", record.id], ["Expected head", record.head]] : [];
        if (action.type === "edit") {
          const fields = action.fields;
          const changed = Object.keys(fields).filter(key => JSON.stringify(fields[key]) !== JSON.stringify(record?.fields[key]));
          for (const key of changed) pairs.push([title(key), JSON.stringify(fields[key])]);
          if (!changed.length) pairs.push(["Changes", "No field changes"]);
          facts(item, pairs, "work-facts-mono");
        } else {
          if (pairs.length) facts(item, pairs, "work-facts-mono");
          item.append(el("pre", JSON.stringify(action), "work-text"));
        }
        body.append(item);
      }
      body.append(el("p", "Persists signed native work COB commands. This does not mutate any provider or grant CI acceptance authority.", "meta"));
      const form = el("form", undefined, "work-dialog-form");
      const commit = el("button", wording.commit, wording.danger ? "danger" : "primary"); commit.type = "submit";
      const dismiss = button(wording.dismiss || "Cancel", () => finish(false), "quiet");
      const actions = el("div", undefined, "work-actions work-dialog-footer"); actions.append(dismiss, commit); form.append(body, actions); dialog.append(form);
      form.addEventListener("submit", event => { event.preventDefault(); finish(true); });
      (wording.danger ? dismiss : commit).focus();
    });
  }
  async function submitCaptured(captures, wording) {
    if (!writable()) throw new Error("Work writes are unavailable for this session or store.");
    if (state.pending) throw new Error("A work command is already in flight. Wait for its observed result.");
    const generation = state.generation, principal = identity();
    if (!await confirmCommand(captures, wording, principal)) return null;
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
    parent.replaceChildren(loading("Loading signed work records…")); await refresh();
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
    for (const record of state.records.filter(record => record.kind === "view" || personal().favorites.includes(record.id))) {
      const label = `${record.kind === "view" ? record.fields.view?.shared ? "Shared: " : "Private: " : ""}${record.fields.title}`;
      const link = record.kind === "view" ? navLink(label, viewPatch(record.id), "work-favorite") : recordLink(label, record.id, "work-favorite");
      if (record.kind !== "view") link.prepend(icon("star"));
      if (record.id === state.route.work_view || record.id === state.route.work_id) link.setAttribute("aria-current", "true");
      nav.append(link);
    }
    if (nav.childElementCount) parent.append(nav);
  }
  // r[impl kiln.work.usability]
  function creationKind() {
    const page = state.route.page;
    if (page === "triage") return "request";
    if (page === "workviews") return "view";
    if (page === "planning") return PLANNING.includes(state.route.work_kind) ? state.route.work_kind : "project";
    return KINDS.includes(state.route.work_kind) ? state.route.work_kind : "issue";
  }
  const kindLabel = kind => kind === "view" ? "saved view" : kind === "personal" ? "attention preferences" : kind;
  // r[impl kiln.web.console_layout]
  // The shell page header (#workspace-title) is the page's single header; work fills only its primary-action slot.
  // The slot sits outside this module's parent, so ownership is marked on the child and cleared on clear()/unavailable states.
  const HEADER_SLOT = "page-primary-action";
  // A drawing page may replace any leftover slot content (active = true); clear() releases only work's own content.
  // r[impl kiln.web.design_system]
  // The header keeps one primary command (a recorded web deviation: kiln.web.visual_design requires a single primary
  // action per view). Labels that open a form end with an ellipsis. Other types sit in a real menu: arrow keys move,
  // Escape returns to the trigger, leaving the menu closes it, and the open trigger stays visibly pressed.
  function headerActions(kind, active = true) {
    const slot = document.getElementById(HEADER_SLOT); if (!slot) return;
    const owned = slot.firstElementChild?.dataset.workActions;
    if (!kind) { if (owned || active) slot.replaceChildren(); return; }
    const signature = `${state.route.page}|${kind}|${writable()}`;
    if (owned === signature) return;
    const start = el("div", undefined, "work-create cluster"); start.dataset.workActions = signature;
    start.append(button(`New ${kindLabel(kind)}…`, () => create(kind), "primary", true));
    const more = el("details", undefined, "work-create-more"), summary = el("summary"); summary.title = "Create something else"; summary.append(el("span", "Create something else", "work-create-more-label"));
    summary.append(icon("chevron-down")); summary.setAttribute("aria-haspopup", "menu"); summary.setAttribute("aria-expanded", "false");
    more.append(summary);
    const choices = el("div", undefined, "work-create-menu"); choices.setAttribute("role", "menu"); choices.setAttribute("aria-label", "Create something else");
    for (const other of KINDS.filter(value => value !== kind)) {
      const item = button(`New ${kindLabel(other)}…`, () => { more.open = false; create(other); }, "quiet menu-item", true);
      item.setAttribute("role", "menuitem"); item.tabIndex = -1; choices.append(item);
    }
    const items = () => Array.from(choices.querySelectorAll("[role='menuitem']:not(:disabled)"));
    const move = index => { const list = items(); if (list.length) list[(index + list.length) % list.length].focus(); };
    more.addEventListener("toggle", () => { summary.setAttribute("aria-expanded", String(more.open)); if (more.open) move(0); });
    more.addEventListener("keydown", event => {
      if (event.key === "Escape" && more.open) { event.preventDefault(); event.stopPropagation(); more.open = false; summary.focus(); return; }
      if (!more.open || !choices.contains(event.target)) return;
      const list = items(), index = list.indexOf(document.activeElement);
      if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); move(index + (event.key === "ArrowDown" ? 1 : -1)); }
      else if (event.key === "Home" || event.key === "End") { event.preventDefault(); move(event.key === "Home" ? 0 : list.length - 1); }
      else if (event.key === "Tab") more.open = false;
    });
    more.addEventListener("focusout", event => { if (more.open && !more.contains(event.relatedTarget)) more.open = false; });
    more.append(choices); start.append(more); slot.replaceChildren(start);
  }
  function toolbar(parent, config) {
    headerActions(creationKind());
    if (state.route.page === "inbox") {
      const prefs = personalRecord(), tools = el("div", undefined, "work-actions work-toolbar"); tools.setAttribute("role", "group"); tools.setAttribute("aria-label", "Inbox preferences");
      tools.append(el("span", "What reaches your inbox:", "meta work-toolbar-label"), button("Attention preferences…", () => prefs ? edit(prefs.id) : create("personal"), "quiet", true), navLink("All attention preferences", { page: "work", work_kind: "personal", work_view: "", work_id: "", work_q: "", work_status: "", work_assignee: "", work_project: "" }, "work-toolbar-link"));
      parent.append(tools);
    }
    if (state.route.page === "planning") {
      const categories = el("nav", undefined, "work-categories"); categories.setAttribute("aria-label", "Planning categories");
      for (const [value, label] of [["", "All planning"], ...PLANNING.map(value => [value, `${title(value)}s`])]) {
        const tab = navLink(label, { work_kind: value, work_view: "", work_id: "", work_q: "", work_status: "", work_assignee: "", work_project: "" }, "work-category");
        if ((config.filters.kind || "") === value) tab.setAttribute("aria-current", "page"); categories.append(tab);
      }
      parent.append(categories);
    }
    const bar = el("div", undefined, "work-filter-bar");
    quickStatus(bar, config);
    const form = el("form", undefined, "work-search"), inputs = {};
    form.setAttribute("role", "search"); form.setAttribute("aria-label", "Search and filter work");
    inputs.q = field(form, "Search", "search", config.filters.q || ""); inputs.q.placeholder = "Title, description or label…";
    const searchField = inputs.q.closest(".work-field"); searchField.classList.add("work-search-field"); searchField.firstElementChild.classList.add("sr-only");
    const glass = icon("search"); glass.classList.add("work-search-icon"); searchField.insertBefore(glass, inputs.q);
    const search = el("button", "Search"); search.type = "submit"; form.append(search);
    const details = el("details", undefined, "work-filter-disclosure");
    // The planning category tabs already show the type; only other filters count as active here.
    const active = Object.entries(config.filters).filter(([key, value]) => key !== "q" && value && !(state.route.page === "planning" && key === "kind")).length;
    details.append(kui().disclosureSummary(`Filters & display${active ? ` · ${active} active` : ""}${config.layout === "board" ? " · Board" : ""}`));
    const filters = el("div", undefined, "work-filters"); details.append(filters); form.append(details);
    for (const key of ["kind", "status", "assignee", "project"]) {
      const options = key === "kind" ? [["", "All types"], ...KINDS.map(value => [value, title(kindLabel(value))])] : key === "status" ? [["", "All statuses"], ...statuses()] : key === "project" ? [["", "All projects"], ...state.records.filter(r => r.kind === "project").map(r => [r.id, r.fields.title])] : null;
      inputs[key] = field(filters, key === "kind" ? "Type" : title(key), options ? "select" : "text", config.filters[key] || "", options);
    }
    const layout = field(filters, "Layout", "select", config.layout, ["list", "board"]);
    const group = field(filters, "Group by", "select", config.group_by, ["none", "status", "assignee", "project", "priority", "team"]);
    const sort = field(filters, "Sort by", "select", config.sort, ["updated", "created", "priority", "title", "due"]);
    const actions = el("div", undefined, "work-actions"); const submit = el("button", "Apply filters"); submit.type = "submit"; actions.append(submit);
    actions.append(button("Reset filters", () => bridge().navigate({ work_view: "", work_kind: "", work_q: "", work_status: "", work_assignee: "", work_project: "", work_layout: "", work_group: "", work_sort: "" }), "quiet"), button("Save current view…", () => editor("view", null, { view: { ...config, shared: false } }), "quiet", true));
    filters.append(actions);
    form.addEventListener("submit", event => { event.preventDefault(); const patch = { work_layout: layout.value, work_group: group.value, work_sort: sort.value }; for (const [key, input] of Object.entries(inputs)) patch[ROUTE_FILTERS[key]] = input.value.trim(); bridge().navigate(patch); });
    bar.append(form); parent.append(bar);
    if (state.route.work_view && !activeView()) {
      const missing = el("div", undefined, "error-state work-unavailable");
      missing.append(el("p", "Selected saved view is unavailable. No alternative view was selected.", "warning"), button("Clear saved view", () => bridge().navigate({ work_view: "" }), "quiet"));
      parent.append(missing);
    }
    const chips = activeChips(config); if (chips) parent.append(chips);
  }
  // r[impl kiln.web.console_layout]
  // Quick status is only a shortcut for the exact single-value work_status URL override. A saved view's status
  // keeps its precedence: the last segment clears the override and names the view default instead of claiming "All".
  const QUICK_STATUS = [["started", "Active"], ["backlog", "Backlog"], ["completed", "Done"]];
  function quickStatus(parent, config) {
    if (!["work", "inbox", "triage"].includes(state.route.page)) return;
    const saved = activeView()?.filters?.status || "", override = state.route.work_status || "", effective = config.filters.status || "";
    const group = el("div", undefined, "segmented work-quick-status"); group.setAttribute("role", "group"); group.setAttribute("aria-label", "Quick status filter");
    for (const [value, label] of QUICK_STATUS) {
      const choice = button(label, () => bridge().navigate({ work_status: value }));
      choice.title = `Show only status ${title(value)}`; choice.setAttribute("aria-pressed", String(effective === value)); group.append(choice);
    }
    const reset = button(saved ? `View default: ${title(saved)}` : "All", () => bridge().navigate({ work_status: "" }));
    reset.title = saved ? "Clear the status override and use the saved view's status" : "Clear the status filter";
    reset.setAttribute("aria-pressed", String(saved ? !override : !effective)); group.append(reset);
    parent.append(group);
  }
  function filterValue(key, value) {
    if (key === "kind") return title(kindLabel(value));
    if (key === "status") return title(value);
    if (key === "priority") return PRIORITIES[value] || value;
    return nameFor(value);
  }
  function chip(text, removeLabel, remove, hint) {
    const node = el("span", undefined, "chip work-chip"); node.append(el("span", text)); if (hint) node.title = hint;
    if (remove) node.append(iconButton(removeLabel, "x", remove, false, "work-chip-remove"));
    return node;
  }
  // Saved-view filters are shown but only URL overrides are removable, preserving saved-view precedence.
  function activeChips(config) {
    const row = el("div", undefined, "work-chips"); row.setAttribute("role", "group"); row.setAttribute("aria-label", "Active filters");
    const saved = activeView();
    if (saved) { const name = nameFor(state.route.work_view); row.append(chip(`Saved view: ${name}`, `Clear saved view ${name}`, () => bridge().navigate({ work_view: "" }), saved.shared ? "Shared saved view" : "Private saved view")); }
    for (const [key, value] of Object.entries(config.filters)) {
      if (!value || (state.route.page === "planning" && key === "kind")) continue;
      const route = ROUTE_FILTERS[key], text = `${key === "q" ? "Search" : key === "kind" ? "Type" : title(key)}: ${filterValue(key, value)}`;
      row.append(route && state.route[route] === value ? chip(text, `Remove filter ${text}`, () => bridge().navigate({ [route]: "" })) : chip(text, null, null, "From the saved view"));
    }
    if (saved && row.childElementCount === 1) row.append(el("span", "No filters", "meta"));
    return row.childElementCount ? row : null;
  }
  function statuses() { return [...new Set([...STATUSES, ...state.records.filter(record => record.kind === "team").flatMap(record => record.fields.workflow || [])])]; }
  function members(record) {
    const key = record.kind;
    if (!PLANNING.includes(key)) return [];
    return state.records.filter(other => other.id !== record.id && other.fields[key] === record.id);
  }
  // r[impl kiln.web.design_system]
  // Feedback reflects reality: an empty scope is stated in words instead of drawing an empty 0/0 meter; rows omit it.
  function progress(parent, record, inRow = false) {
    const children = members(record); const done = children.filter(child => child.fields.status === "completed").length;
    if (!children.length) { if (!inRow) parent.append(el("p", "No linked records yet.", "meta")); return; }
    const row = el("div", undefined, "work-progress");
    const meter = el("progress"); meter.max = children.length; meter.value = done; meter.setAttribute("aria-label", `${done} of ${children.length} linked records completed`);
    row.append(meter, el("span", `${done}/${children.length} linked completed · ${Math.round(done / children.length * 100)}%`, "meta")); parent.append(row);
  }
  function workRow(record, parent) {
    const f = record.fields, row = el("article", undefined, "work-row"); row.dataset.workId = record.id; row.tabIndex = 0;
    row.setAttribute("aria-label", `${title(record.kind)}: ${f.title}`);
    if (state.route.work_id === record.id) row.setAttribute("aria-current", "true");
    const main = recordLink(f.title || "Untitled", record.id, "work-row-title");
    const updated = el("span", undefined, "work-row-updated meta"); updated.append(srOnly("Updated "), when(record.updated_at_ms));
    const meta = el("div", undefined, "work-row-meta");
    meta.append(workStatus(f.status));
    if (record.kind !== "issue") meta.append(el("span", title(kindLabel(record.kind)), "work-kind"));
    if (f.priority) meta.append(priorityTag(f.priority));
    if (f.assignees?.length) meta.append(el("span", `Assigned: ${f.assignees.join(", ")}`, "work-assignees"));
    if (f.team) meta.append(el("span", nameFor(f.team)));
    if (f.project) meta.append(el("span", nameFor(f.project)));
    for (const label of f.labels || []) meta.append(el("span", label, "work-label"));
    if (state.route.page === "inbox") for (const reason of attention(record)) meta.append(el("span", reason, "work-attention"));
    if (personal().favorites.includes(record.id)) { const mark = el("span", "Favorite", "work-favorite-mark"); mark.prepend(icon("star")); meta.append(mark); }
    if (f.due_at_ms != null) {
      // Overdue is stated in text, not by colour alone.
      const overdue = f.due_at_ms < Date.now() && !["completed", "cancelled", "duplicate"].includes(f.status);
      const due = el("span", undefined, "work-due"); due.append(overdue ? "Overdue · due " : "Due ", when(f.due_at_ms));
      if (overdue) due.dataset.overdue = "true";
      meta.append(due);
    }
    row.append(main, updated, meta);
    if (PLANNING.includes(record.kind)) progress(row, record, true);
    parent.append(row);
  }
  function grouped(parent, records, config) {
    const groupBy = config.group_by === "none" && config.layout === "board" ? "status" : config.group_by;
    const groups = new Map();
    for (const record of records) {
      const raw = groupBy === "none" ? "Items" : groupBy === "assignee" ? record.fields.assignees?.join(", ") || "Unassigned" : groupBy === "priority" ? PRIORITIES[record.fields.priority] : record.fields[groupBy] || "Not set";
      const label = ["project", "team"].includes(groupBy) ? nameFor(raw) : title(raw);
      if (!groups.has(label)) groups.set(label, { raw, rows: [] }); groups.get(label).rows.push(record);
    }
    // r[impl kiln.web.page_consistency.scenario.states]
    // One action per empty state: the page header already offers creation, so the empty list only repeats it
    // when that slot is absent; a filtered empty list offers the reset instead.
    if (!records.length) {
      const empty = el("section", undefined, "work-empty empty-state"), page = state.route.page;
      const filtered = state.route.work_view || Object.entries(config.filters).some(([key, value]) => value && !(page === "planning" && key === "kind"));
      const heading = page === "inbox" ? "You're all caught up" : filtered ? "No matching items" : page === "planning" ? `No ${state.route.work_kind ? `${state.route.work_kind}s` : "plans"} yet` : page === "triage" ? "No requests to triage" : page === "workviews" ? "Keep a useful view" : "Start with an issue";
      const help = page === "inbox" ? "Your unread assignments, mentions, watched changes and due reminders appear here. Opening an item never marks it read automatically." : filtered ? "Try a different search or reset filters. This list covers the current bounded snapshot." : page === "planning" ? "Create a project for an outcome, then link issues. Use milestones for checkpoints and cycles for time-boxed work." : page === "triage" ? "Capture a request with its original source, then accept, decline, snooze or route it." : page === "workviews" ? "Save filters and a list or board layout so you can return to them later." : "Capture something to do. Add a title and description now; ownership, planning and CI links can come later.";
      empty.append(el("h2", heading, "empty-state-title"), el("p", help, "empty-state-text"));
      if (filtered) empty.append(button("Reset filters", () => bridge().navigate({ work_view: "", work_kind: page === "planning" ? state.route.work_kind || "" : "", work_q: "", work_status: "", work_assignee: "", work_project: "", work_layout: "", work_group: "", work_sort: "" }), "quiet"));
      else if (page !== "inbox" && writable() && !document.getElementById(HEADER_SLOT)) empty.append(button(`New ${kindLabel(creationKind())}…`, () => create(creationKind()), undefined, true));
      parent.append(empty);
    }
    parent.dataset.layout = config.layout;
    for (const [label, { raw, rows }] of groups) {
      const group = el("section", undefined, "work-group");
      // Ungrouped lists have one implicit group; its "Items" heading would only repeat the list count.
      if (groupBy !== "none") {
        const heading = el("h3");
        heading.append(groupBy === "status" && raw !== "Not set" ? workStatus(raw) : el("span", label), el("span", String(rows.length), "work-count"));
        group.append(heading);
      }
      for (const record of rows) workRow(record, group); parent.append(group);
    }
  }
  function roadmap(parent, records) {
    const plans = records.filter(record => PLANNING.includes(record.kind));
    // r[impl kiln.web.design_system] With no planning records at all there is nothing to schedule; the empty list speaks.
    if (!plans.length) return;
    const scheduled = plans.filter(record => record.fields.start_at_ms != null || record.fields.due_at_ms != null);
    // r[impl kiln.web.page_consistency]
    // An unscheduled roadmap collapses to its summary so it never pushes the plans themselves below the fold.
    const details = el("details", undefined, "card work-roadmap"); details.open = scheduled.length > 0;
    details.append(kui().disclosureSummary(scheduled.length ? `Roadmap · ${scheduled.length} scheduled planning record${scheduled.length === 1 ? "" : "s"}` : "Roadmap · no scheduled planning records"));
    if (!scheduled.length) details.append(el("p", "No dated planning records in the current scope. Set start and due dates in a planning record to schedule it.", "meta"));
    const times = scheduled.flatMap(record => [record.fields.start_at_ms, record.fields.due_at_ms]).filter(value => value != null);
    const min = Math.min(...times), max = Math.max(...times), span = Math.max(max - min, 86400000);
    for (const record of scheduled.sort((a, b) => (a.fields.start_at_ms ?? a.fields.due_at_ms) - (b.fields.start_at_ms ?? b.fields.due_at_ms))) {
      const row = el("div", undefined, "work-roadmap-row"); row.append(recordLink(record.fields.title, record.id, "work-roadmap-title"));
      const track = el("div", undefined, "work-roadmap-track"), bar = el("div", undefined, "work-roadmap-bar");
      const start = record.fields.start_at_ms ?? record.fields.due_at_ms, end = record.fields.due_at_ms ?? start;
      bar.style.marginLeft = `${(start - min) / span * 90}%`; bar.style.width = `${Math.max(2, (end - start) / span * 90)}%`; track.append(bar);
      const range = el("span", `${shortDate(record.fields.start_at_ms)} → ${shortDate(record.fields.due_at_ms)}`, "meta"); range.title = `${date(record.fields.start_at_ms)} → ${date(record.fields.due_at_ms)}`;
      row.append(track, range); details.append(row);
    }
    parent.append(details);
  }
  // r[impl kiln.web.console_layout]
  // Lists sit in a card; list layouts use the shared two-pane .split-view (list left, detail right, stacked when narrow).
  // Boards keep full width so their columns stay readable, with the selected record below.
  function selectPrompt() {
    const kind = creationKind(), page = state.route.page;
    const noun = page === "inbox" ? "an item" : kind === "personal" ? "a record" : `${/^[aeiou]/.test(kindLabel(kind)) ? "an" : "a"} ${kindLabel(kind)}`;
    const prompt = el("div", undefined, "work-empty work-select-prompt empty-state");
    prompt.append(el("h2", `Select ${noun}`, "empty-state-title"), el("p", "Its description, relationships, discussion and linked CI evidence open here. Keyboard: j/k or ↑/↓ to move, Enter to open, Space to peek.", "empty-state-text"));
    return prompt;
  }
  // r[impl kiln.web.design_system]
  // The list toggle joins the close control in the detail heading's trailing lane once the record loads; while it
  // loads or fails it sits alone in a bar so a collapsed list can always be restored.
  function listToggle(view, listPane) {
    const collapsed = state.listCollapsed === true;
    const toggle = button(collapsed ? "Show list" : "Hide list", () => {
      state.listCollapsed = !state.listCollapsed; view.classList.toggle("is-list-collapsed", state.listCollapsed);
      toggle.textContent = state.listCollapsed ? "Show list" : "Hide list"; toggle.setAttribute("aria-expanded", String(!state.listCollapsed));
    }, "quiet work-list-toggle");
    toggle.setAttribute("aria-controls", listPane.id); toggle.setAttribute("aria-expanded", String(!collapsed));
    return toggle;
  }
  function toggleBar(parent) { if (!parent.listToggle) return []; const bar = el("div", undefined, "work-detail-bar"); bar.append(parent.listToggle); return [bar]; }
  function draw(preserveScroll = true) {
    if (!state.parent || identity() !== state.principal) return;
    if (preserveScroll) saveScroll();
    const focused = document.activeElement?.closest("[data-work-id]")?.dataset.workId;
    const parent = state.parent; parent.replaceChildren(); parent.classList.add("work-workspace");
    if (state.error) {
      headerActions(null);
      const failure = el("section", undefined, "error-state work-error"); failure.setAttribute("role", "alert");
      failure.append(el("h2", "Work storage unavailable", "empty-state-title"), el("p", `${state.error}. No empty or demo data has been substituted.`, "empty-state-text"), button("Reconnect", refresh));
      parent.append(failure); return;
    }
    if (!state.configured) {
      headerActions(null);
      const missing = el("section", undefined, "work-empty empty-state");
      missing.append(el("h2", "Work storage is not configured", "empty-state-title"), el("p", "Native work storage is not configured. Configure an explicit signed work COB store on the server to enable this workspace. No records or private preferences are stored in this browser.", "empty-state-text"));
      parent.append(missing); return;
    }
    if (!writable()) { const notice = el("p", "Read-only work workspace. Creating, editing, commenting and personal preferences require an admitted work writer and writable native store. CI operator authority is separate.", "work-banner meta"); notice.dataset.tone = "info"; parent.append(notice); }
    navigation(parent); const config = configuration(); toolbar(parent, config);
    const records = visibleRows(config), selectedId = state.route.work_id || "";
    if (state.route.page === "planning") roadmap(parent, records);
    const board = config.layout === "board";
    const split = !board && (!!selectedId || (records.length > 0 && ["work", "inbox", "triage"].includes(state.route.page)));
    const listPane = el("section", undefined, `card work-list-card${split ? " split-list" : ""}`); const prefix = parent.id || "work"; listPane.id = `${prefix}-list-pane`;
    const listHead = el("div", undefined, "work-list-head"), listTitle = el("h2", "Results", "card-title"); listTitle.id = `${prefix}-list-title`; listPane.setAttribute("aria-labelledby", listTitle.id);
    const count = el("span", String(records.length), "work-count"); count.title = `${records.length} record(s) in the current bounded snapshot`;
    // r[impl kiln.web.page_consistency]
    // Order is described in words, and only when there is something to order.
    listHead.append(listTitle, count);
    if (records.length) listHead.append(el("span", `Sorted by ${SORT_LABELS[config.sort] || config.sort}${config.group_by !== "none" ? ` · grouped by ${config.group_by}` : ""}`, "meta work-list-order"));
    const list = el("div", undefined, "work-list-scroll"); list.setAttribute("aria-label", "Work records");
    grouped(list, records, config); listPane.append(listHead, list);
    const detail = el("aside", undefined, `card work-detail${split ? " split-detail" : ""}`); detail.id = `${prefix}-detail-pane`; detail.setAttribute("aria-label", "Selected work record");
    const body = el("div", undefined, "work-detail-body");
    let layout;
    if (split) {
      layout = el("div", undefined, "split-view work-split"); layout.classList.toggle("is-list-collapsed", !!selectedId && state.listCollapsed === true);
      if (selectedId) body.listToggle = listToggle(layout, listPane); else body.append(selectPrompt());
      detail.append(body); layout.append(listPane, detail);
    } else {
      layout = el("div", undefined, "stack work-stacked"); layout.append(listPane);
      if (selectedId) { detail.append(body); layout.append(detail); }
    }
    parent.append(layout);
    const position = state.scroll.get(scrollKey()); if (position) { list.scrollTop = position.top; list.scrollLeft = position.left; }
    if (focused) Array.from(list.querySelectorAll("[data-work-id]")).find(row => row.dataset.workId === focused)?.focus({ preventScroll: true });
    if (selectedId) void loadDetail(selectedId, body).catch(report);
    window.dispatchEvent(new CustomEvent("kiln-work-updated"));
  }
  async function loadDetail(id, parent, preview = false) {
    const generation = state.generation, principal = identity(), request = preview ? (previewRequests.get(parent) || 0) + 1 : ++state.detail;
    if (preview) previewRequests.set(parent, request);
    const current = () => valid(generation, principal) && parent.isConnected && (preview ? request === previewRequests.get(parent) : request === state.detail && state.route.work_id === id);
    parent.replaceChildren(...toggleBar(parent), loading("Reading exact signed record…", 2));
    try {
      const record = await bridge().api(pathFor(id));
      if (!current()) return;
      if (record?.id !== id || !record.head || !KINDS.includes(record.kind)) throw new Error("Exact record response is invalid.");
      parent.replaceChildren(); const linksSection = renderDetail(record, parent, preview);
      // r[impl kiln.web.page_consistency]
      // Joined evidence sits beside the links it is joined from; its boundary statement is a labelled footnote.
      const evidence = el("section", undefined, "work-card work-evidence"); evidence.append(el("h3", "Linked change evidence")); linksSection.after(evidence);
      const body = el("div"); evidence.append(body);
      const about = el("details", undefined, "about-view work-about"); about.append(kui().disclosureSummary("About linked evidence", "info"));
      const aboutBody = el("div", undefined, "about-view-body"); aboutBody.append(el("p", "Only explicitly linked identities are joined. Recorded CI outcomes, review claims and Cairn history are observations, not acceptance authority. Missing or inaccessible evidence is not success. No provider writes are available here.")); about.append(aboutBody); evidence.append(about);
      if (!record.fields.links?.length) { body.append(el("p", "No explicit source, revision, patch, review, run, artifact or Cairn links are recorded.", "small")); return; }
      body.append(el("p", "Loading exact linked observations…", "small"));
      try {
        if (typeof bridge().changeEvidence !== "function") throw new Error("Exact evidence joining is unavailable in this configured shell.");
        const joined = await bridge().changeEvidence(record); if (!current()) return;
        body.replaceChildren(); recorded(body, joined);
      } catch (error) { if (current()) body.replaceChildren(el("p", `Linked evidence unavailable: ${errorText(error)}`, "warning")); }
    } catch (error) { if (current()) parent.replaceChildren(...toggleBar(parent), el("p", `Selected object unavailable: ${errorText(error)}. No alternate record was selected.`, "warning")); }
  }
  // r[impl kiln.web.readable_observations]
  // Linked CI observations: canonical outcomes, durations and instants become readable while title/datetime keep exact values.
  function observed(key, value, owner) {
    const ui = kui();
    if (key === "outcome" && typeof value === "string" && typeof ui?.statusPill === "function") return ui.statusPill(value, typeof owner?.phase === "string" ? owner.phase : undefined);
    if (typeof value === "number" && /(^|_)duration_ms$|^elapsed_ms$/.test(key) && typeof ui?.durationElement === "function") return ui.durationElement(value);
    if (typeof value === "number" && /_at_ms$/.test(key)) return when(value);
    return null;
  }
  function recorded(parent, value, depth = 0) {
    if (value == null) { parent.append(el("span", "Unavailable (not recorded)", "meta")); return; }
    if (typeof value !== "object") { parent.append(el("span", value === "" ? "(empty)" : value, "work-text")); return; }
    if (depth > 7) { parent.append(el("pre", JSON.stringify(value, null, 2), "work-text")); return; }
    if (Array.isArray(value)) { if (!value.length) parent.append(el("p", "No linked observations in this category.", "meta")); for (const item of value) { const row = el("div", undefined, "work-observation"); recorded(row, item, depth + 1); parent.append(row); } return; }
    const dl = el("dl", undefined, "work-facts");
    for (const [key, item] of Object.entries(value)) {
      const dd = el("dd"), readable = observed(key, item, value);
      if (readable) dd.append(readable);
      else if (item && typeof item === "object") {
        const details = el("details");
        details.append(el("summary", `${title(key)}${Array.isArray(item) ? ` · ${item.length}` : " · exact observation"}`));
        recorded(details, item, depth + 1);
        dd.append(details);
      } else recorded(dd, item, depth + 1);
      dl.append(el("dt", title(key)), dd);
    }
    parent.append(dl);
  }
  function messageHeading(tag, message) {
    const heading = el(tag, undefined, "work-message-heading");
    heading.append(el("strong", message.author), " · ", when(message.recorded_at_ms));
    if (message.edited_at_ms) heading.append(" · edited ", when(message.edited_at_ms));
    return heading;
  }
  function renderDetail(record, parent, preview) {
    const f = record.fields, heading = el("div", undefined, "work-detail-heading"), eyebrow = el("div", undefined, "work-detail-eyebrow");
    eyebrow.append(el("span", title(kindLabel(record.kind)), "work-kind"), workStatus(f.status));
    if (f.priority) eyebrow.append(priorityTag(f.priority));
    // The quick peek footer already offers "Open full detail"; only the full detail carries a close control.
    heading.append(eyebrow);
    if (!preview) { const tools = el("div", undefined, "work-detail-tools"); if (parent.listToggle) tools.append(parent.listToggle); tools.append(iconButton("Close detail", "x", () => open(""))); heading.append(tools); }
    heading.append(el("h2", f.title));
    parent.append(heading);
    parent.append(el("p", f.description || "No description recorded.", f.description ? "work-text" : "work-text meta"));
    // r[impl kiln.web.design_system] The record's commands sit directly under its title and description, ahead of reference facts.
    if (!preview) {
      const actions = el("div", undefined, "work-actions work-detail-actions"); actions.append(button("Edit…", () => editor(record.kind, record), undefined, true));
      if (record.kind === "issue") actions.append(button("Add sub-issue…", () => editor("issue", null, { parent: record.id, team: f.team, project: f.project }), "quiet", true));
      if (record.kind !== "personal") {
        // r[impl kiln.web.design_system] Each label names its result; the row actions menu uses the same verbs.
        const p = personal(), attentionTools = el("div", undefined, "work-actions work-attention-actions"); attentionTools.setAttribute("role", "group"); attentionTools.setAttribute("aria-label", "My attention");
        const watched = p.watched.includes(record.id), favorite = p.favorites.includes(record.id), read = p.read[record.id] === record.head;
        attentionTools.append(button(watched ? "Unwatch" : "Watch", () => preference("watch", [record.id], !watched), "quiet", true), button(favorite ? "Remove favorite" : "Favorite", () => preference("favorite", [record.id], !favorite), "quiet", true), button(read ? "Mark unread" : "Mark read", () => preference("read", [record.id], !read), "quiet", true), button("Set reminder…", () => reminder(record.id), "quiet", true));
        if (p.reminders[record.id] != null) attentionTools.append(button("Clear reminder", () => preference("reminder", [record.id], null), "quiet", true));
        actions.append(attentionTools);
      }
      parent.append(actions);
    }
    // r[impl kiln.web.page_consistency.scenario.data]
    // Recorded fields are listed; unset optional fields are named together on one line instead of a column of
    // "Not recorded" rows. Original intake stays explicit for requests, and principals keep their exact form.
    const recordedFacts = [], unrecorded = [];
    const optional = [["Owner", f.owner], ["Assignees", f.assignees?.join(", ")], ["Labels", f.labels?.join(", ")]];
    if (record.kind === "request" || f.customer || f.source_url) optional.push(["Customer (original intake)", f.customer], ["Source (original intake)", f.source_url]);
    for (const [key, value] of optional) if (value) recordedFacts.push([key, key === "Owner" || key === "Assignees" ? el("span", value, "mono") : value]); else unrecorded.push(key);
    if (f.start_at_ms == null && f.due_at_ms == null) recordedFacts.push(["Schedule", el("span", "Not scheduled", "meta")]);
    else recordedFacts.push(["Start", when(f.start_at_ms, "Not scheduled")], ["Due / snooze until", when(f.due_at_ms, "Not scheduled")]);
    recordedFacts.push(["Created by", el("span", record.created_by, "mono")], ["Created", when(record.created_at_ms)], ["Updated", when(record.updated_at_ms)]);
    if (unrecorded.length) recordedFacts.push(["Not recorded", el("span", unrecorded.join(" · "), "meta")]);
    facts(parent, recordedFacts);
    const relations = section(parent, "Relationships");
    for (const [key, label] of [["parent", "Parent"], ["team", "Team"], ["project", "Project"], ["milestone", "Milestone"], ["cycle", "Cycle"], ["initiative", "Initiative"], ["duplicate_of", "Duplicate of"]]) if (f[key]) { const line = el("div", undefined, "work-relation"); line.append(el("span", `${label}: `, "meta"), recordLink(nameFor(f[key]), f[key])); relations.append(line); }
    for (const id of f.blocked_by || []) relations.append(recordLink(`Blocked by: ${nameFor(id)}`, id));
    for (const child of state.records.filter(other => other.fields.parent === record.id)) relations.append(recordLink(`${record.kind === "issue" ? "Sub-issue" : "Child " + record.kind}: ${child.fields.title}`, child.id));
    for (const other of state.records.filter(other => other.fields.blocked_by?.includes(record.id))) relations.append(recordLink(`Blocks: ${other.fields.title}`, other.id));
    if (!relations.querySelector("a")) relations.append(el("p", "No typed relationships recorded.", "meta"));
    if (PLANNING.includes(record.kind)) {
      const scope = section(parent, "Scope & progress"); progress(scope, record);
      for (const child of members(record)) { const link = recordLink(child.fields.title, child.id); link.append(workStatus(child.fields.status)); scope.append(link); }
    }
    if (record.kind === "team") facts(parent, [["Workflow", f.workflow?.length ? f.workflow.join(" → ") : el("span", "Built-in statuses only", "meta")]]);
    if (record.kind === "view") {
      const view = section(parent, f.view?.shared ? "Shared saved view" : "Private saved view"); recorded(view, f.view);
      if (!preview) {
        view.append(navLink("Use this view", viewPatch(record.id), "button"));
        view.append(button(f.view?.shared ? "Create private copy…" : "Create shared copy…", () => editor("view", null, { ...clone(f), title: `${f.title} (${f.view?.shared ? "private" : "shared"} copy)`, owner: state.principal, view: { ...clone(f.view), shared: !f.view?.shared } }), "quiet", true));
      }
    }
    if (record.kind === "personal") { const privateSection = section(parent, "Private attention choices"); recorded(privateSection, f.personal); }
    const links = section(parent, "Exact external identity links");
    for (const link of f.links || []) {
      const row = el("p", undefined, "work-link-identity"); row.append(el("span", title(link.kind), "work-kind"), el("code", link.id, "mono"));
      if (link.kind === "run") row.append(" ", bridge().runLink(link.id, "Inspect exact run"));
      links.append(row);
    }
    if (!f.links?.length) links.append(el("p", "No linked identities.", "meta"));
    if (!preview) {
      if (record.kind === "request") triage(record, parent);
      discussion(record, parent, "comments", false);
      if (["project", "team"].includes(record.kind)) discussion(record, parent, "updates", true);
    } else {
      for (const message of [...(record.comments || []), ...(record.updates || [])]) { const card = el("section", undefined, "work-card"); card.append(messageHeading("h3", message), el("p", message.body, "work-text")); parent.append(card); }
    }
    const identitySection = el("details", undefined, "work-identity"); identitySection.append(kui().disclosureSummary("Signed record identity")); facts(identitySection, [["Object", record.id], ["Exact head", record.head], ["Authenticated application principal", record.created_by]], "work-facts-mono"); identitySection.append(el("p", "The application signature attests the authenticated principal, not an end-user Radicle identity.", "meta")); parent.append(identitySection);
    return links;
  }
  async function edit(id) { const generation = state.generation, principal = identity(); const record = await bridge().api(pathFor(id)); if (valid(generation, principal)) editor(record.kind, record); }
  function create(kind = "issue") { if (!KINDS.includes(kind)) throw new Error("Unsupported work kind."); if (!writable()) throw new Error("Creation requires an admitted writable store."); return editor(kind); }
  function editor(kind, record = null, preset = {}) {
    if (!writable()) throw new Error("Editing is unavailable in this session.");
    const generation = state.generation, principal = identity(), f = { ...defaults(kind), ...(record ? clone(record.fields) : {}), ...clone(preset) };
    const { dialog, close } = modal(record ? `Edit ${kindLabel(kind)} ${quoted(record.fields.title)}` : `New ${kindLabel(kind)}`); const form = el("form", undefined, "work-editor"); dialog.append(form);
    const base = section(form, "Details", "work-form-section");
    const inputs = { title: field(base, "Title", "text", f.title), description: field(base, "Description", "textarea", f.description) }; inputs.title.required = true; inputs.title.maxLength = 256; inputs.title.closest(".work-field").classList.add("work-field-wide");
    inputs.status = field(base, "Status", "select", f.status, statuses().filter(status => status !== "duplicate" || ["issue", "request"].includes(kind))); inputs.priority = field(base, "Priority", "select", f.priority, PRIORITIES.map((label, index) => [index, label]));
    const ownership = disclosure(form, "Ownership & labels");
    inputs.owner = field(ownership, ["personal", "view"].includes(kind) ? "Private record owner (authenticated principal)" : "Owner principal", "text", f.owner); inputs.owner.readOnly = ["personal", "view"].includes(kind);
    inputs.assignees = field(ownership, "Assignee principals (comma separated)", "text", f.assignees.join(", ")); inputs.labels = field(ownership, "Labels (comma separated)", "text", f.labels.join(", "));
    const relations = disclosure(form, "Relationships · project, cycle & dependencies");
    if (!["personal", "view"].includes(kind)) inputs.parent = relation(relations, `Parent ${kind}`, kind, f.parent, false, record?.id);
    if (!["personal", "view", "team"].includes(kind)) inputs.blocked_by = relation(relations, "Blocked by work / planning records", ["issue", "request", "project", "milestone", "cycle", "initiative"], f.blocked_by, true, record?.id);
    for (const target of PLANNING) inputs[target] = relation(relations, title(target), target, f[target], false, record?.id);
    if (["issue", "request"].includes(kind)) {
      inputs.duplicate_of = relation(relations, "Duplicate of", ["issue", "request"], f.duplicate_of, false, record?.id);
      inputs.duplicate_of.addEventListener("change", () => { if (inputs.duplicate_of.value) inputs.status.value = "duplicate"; else if (inputs.status.value === "duplicate") inputs.status.value = "backlog"; });
      inputs.status.addEventListener("change", () => { if (inputs.status.value !== "duplicate") inputs.duplicate_of.value = ""; });
      relations.append(el("p", "Choosing a duplicate target sets duplicate status. Leaving duplicate status clears that target in the new event; signed prior intake and history remain retained.", "small"));
    }
    const schedule = disclosure(form, "Schedule · dates & reminders"); inputs.start_at_ms = field(schedule, "Starts at (local time)", "datetime-local", datetime(f.start_at_ms)); inputs.due_at_ms = field(schedule, "Due / snooze until (local time)", "datetime-local", datetime(f.due_at_ms));
    if (kind === "request") {
      const intake = section(form, "Original intake", "work-form-section");
      inputs.customer = field(intake, "Original customer", "text", f.customer); inputs.source_url = field(intake, "Original source URL (HTTP or HTTPS)", "url", f.source_url);
      if (record) { inputs.customer.readOnly = true; inputs.source_url.readOnly = true; intake.append(el("p", "Routing and triage preserve the original customer and source. Add later context in description, links or discussion.", "small")); }
    }
    if (kind === "team") { inputs.workflow = field(base, "Additional workflow statuses (comma separated)", "text", f.workflow.join(", ")); inputs.workflow.closest(".work-field").classList.add("work-field-wide"); base.append(el("p", "Built-in statuses remain available. Issues assigned to this team may use its additional statuses.", "small")); }
    const linkSection = disclosure(form, "CI & source links");
    linkSection.append(el("p", "Links are inert identities, not provider permissions. Source: repository@revision; patch/Cairn: repository|nativeType|object; review: receiptName; run: exact run ID; artifact: runID|artifactID. Unresolvable links remain explicitly unavailable.", "small"));
    const linkRows = el("div", undefined, "work-link-editor"); linkSection.append(linkRows);
    const addLink = (link = { kind: "run", id: "" }) => { const row = el("div", undefined, "work-link-row"); const type = field(row, "Link kind", "select", link.kind, LINK_KINDS), id = field(row, "Exact identity", "text", link.id); row.readLink = () => ({ kind: type.value, id: id.value.trim() }); row.append(button("Remove link", () => row.remove(), "quiet")); linkRows.append(row); };
    for (const link of f.links) addLink(link); linkSection.append(button("Add link", () => addLink(), "quiet"));
    const readView = kind === "view" ? viewEditor(form, f.view, !!record) : null;
    const readPersonal = kind === "personal" ? personalEditor(form, f.personal) : null;
    const status = el("p", "Changes persist only after explicit confirmation.", "small work-dialog-status"); status.setAttribute("role", "status"); form.append(status);
    const save = el("button", record ? "Save changes" : `Create ${kindLabel(kind)}`, "primary"); save.type = "submit"; footer(form, save, close);
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
        const wording = record ? { title: `Save changes to ${quoted(fields.title)}?`, commit: "Save changes", dismiss: "Keep editing" } : { title: `Create ${kindLabel(kind)} ${quoted(fields.title)}?`, commit: `Create ${kindLabel(kind)}`, dismiss: "Keep editing" };
        const results = await submitCaptured([{ record, action: record ? { type: "edit", fields } : { type: "create", kind, fields } }], wording);
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
    const add = (id = "", at = null) => { const row = el("div", undefined, "work-link-row"); const target = relation(row, "Reminder record", kinds, id), when = field(row, "Remind at (local time)", "datetime-local", datetime(at)); row.readReminder = () => target.value && when.value ? [target.value, timestamp(when)] : null; row.append(button("Remove reminder", () => row.remove(), "quiet")); reminders.append(row); };
    for (const [id, at] of Object.entries(p.reminders || {})) add(id, at); panel.append(button("Add reminder", () => add(), "quiet"));
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
    return submitCaptured([{ record, action: record ? { type: "edit", fields } : { type: "create", kind: "personal", fields } }], preferenceWording(type, ids, active));
  }
  // r[impl kiln.web.design_system] Private attention verbs, shared by the detail, the row actions menu and their confirmations.
  function preferenceWording(type, ids, active) {
    const verb = type === "watch" ? active === false ? "Unwatch" : active === true ? "Watch" : "Toggle watch"
      : type === "favorite" ? active === false ? "Remove favorite" : active === true ? "Favorite" : "Toggle favorite"
      : type === "read" ? active === false ? "Mark unread" : "Mark read"
      : active == null ? "Clear reminder" : "Set reminder";
    const target = ids.length === 1 ? quoted(nameFor(ids[0])) : `${ids.length} records`;
    return { title: type === "reminder" ? `${verb} for ${target}?` : `${verb} ${target}?`, commit: verb, note: "Updates your private attention record, visible only to you." };
  }
  function reminder(id) {
    const { dialog, close } = modal(`Set reminder for ${quoted(nameFor(id))}`); const form = el("form", undefined, "work-dialog-form"); const when = field(form, "Remind at (local time)", "datetime-local", datetime(personal().reminders[id])); when.required = true;
    const save = el("button", "Set reminder", "primary"); save.type = "submit"; footer(form, save, close); dialog.append(form);
    form.addEventListener("submit", async event => { event.preventDefault(); save.disabled = true; try { if (await preference("reminder", [id], timestamp(when))) close(); } catch (error) { report(error); } finally { save.disabled = !writable(); } });
  }
  function triage(record, parent) {
    const name = quoted(record.fields.title);
    const panel = section(parent, "Request triage"); panel.append(el("p", "Decisions update this request's status without replacing its original customer or source. Use Edit… to route it to an assignee, team or project.", "small"));
    const actions = el("div", undefined, "work-actions");
    actions.append(button("Accept to backlog", () => bulk({ type: "edit", fields: { status: "backlog", duplicate_of: null } }, [record.id], { title: `Accept ${name} to backlog?`, commit: "Accept to backlog" }), undefined, true), button("Decline", () => bulk({ type: "edit", fields: { status: "cancelled" } }, [record.id], { title: `Decline request ${name}?`, commit: "Decline request" }), "quiet", true), button("Snooze…", () => triageForm(record, "snooze"), "quiet", true), button("Mark duplicate…", () => triageForm(record, "duplicate"), "quiet", true)); panel.append(actions);
  }
  function triageForm(record, type) {
    const name = quoted(record.fields.title), snooze = type === "snooze";
    const { dialog, close } = modal(snooze ? `Snooze ${name}` : `Mark ${name} as duplicate`); const form = el("form", undefined, "work-dialog-form");
    const input = snooze ? field(form, "Snooze until (local time)", "datetime-local", datetime(record.fields.due_at_ms)) : relation(form, "Duplicate of", ["issue", "request"], record.fields.duplicate_of, false, record.id); input.required = true;
    const commit = snooze ? "Snooze" : "Mark duplicate", save = el("button", commit, "primary"); save.type = "submit"; footer(form, save, close); dialog.append(form);
    form.addEventListener("submit", async event => { event.preventDefault(); save.disabled = true; try { const fields = snooze ? { status: "snoozed", due_at_ms: timestamp(input) } : { status: "duplicate", duplicate_of: input.value }; if (await bulk({ type: "edit", fields }, [record.id], { title: snooze ? `Snooze ${name}?` : `Mark ${name} as duplicate?`, commit })) close(); } catch (error) { report(error); } finally { save.disabled = !writable(); } });
  }
  function discussion(record, parent, key, update) {
    const panel = section(parent, update ? "Project / team updates" : "Discussion");
    for (const message of record[key] || []) {
      const card = el("article", undefined, "work-message"); card.append(messageHeading("h4", message), el("p", message.body, "work-text"));
      if (message.mentions?.length) card.append(el("p", `Mentions: ${message.mentions.join(", ")}`, "meta"));
      const reactions = el("div", undefined, "work-actions work-reactions");
      for (const [emoji, actors] of Object.entries(message.reactions || {})) {
        const mine = actors.includes(state.principal), chip = button(`${emoji} ${actors.length} · ${actors.join(", ")}`, () => messageAction(record, { type: "react", comment: message.id, emoji, active: !mine }, reactionWording(record, message, emoji, !mine)), mine ? "work-reaction work-active" : "work-reaction", true);
        chip.setAttribute("aria-pressed", String(mine)); reactions.append(chip);
      }
      if (message.author === state.principal) reactions.append(button(update ? "Edit update…" : "Edit comment…", () => commentEditor(record, message, update), "quiet", true));
      reactions.append(button("React…", () => reactionEditor(record, message), "quiet", true)); card.append(reactions); panel.append(card);
    }
    if (!record[key]?.length) panel.append(el("p", update ? "No updates recorded." : "No comments recorded.", "meta"));
    const form = el("form", undefined, "work-discussion-form"), body = field(form, update ? "New update" : "New comment", "textarea"), mentionsInput = field(form, "Mention exact principal IDs (comma separated)", "text"); body.required = true;
    const save = el("button", update ? "Post update" : "Post comment"); save.type = "submit"; save.disabled = !writable(); form.append(save); panel.append(form);
    form.addEventListener("submit", async event => { event.preventDefault(); save.disabled = true; try { const verb = update ? "Post update" : "Post comment"; await messageAction(record, { type: update ? "update" : "comment", body: body.value.trim(), mentions: split(mentionsInput.value) }, { title: `${verb} on ${quoted(record.fields.title)}?`, commit: verb }); } catch (error) { report(error); } finally { save.disabled = !writable(); } });
  }
  function messageAction(record, action, wording) { return submitCaptured([{ record, action }], wording); }
  function reactionWording(record, message, emoji, active) {
    return active ? { title: `Add reaction ${emoji} to ${message.author}'s message?`, commit: "Add reaction", note: `On ${quoted(record.fields.title)}.` } : { title: `Remove reaction ${emoji} from ${message.author}'s message?`, commit: "Remove reaction", note: `On ${quoted(record.fields.title)}.` };
  }
  function commentEditor(record, message, update = false) {
    const label = update ? "update" : "comment";
    const { dialog, close } = modal(`Edit your ${label}`), form = el("form", undefined, "work-dialog-form");
    const body = field(form, title(label), "textarea", message.body), mentionsInput = field(form, "Mention exact principal IDs (comma separated)", "text", message.mentions.join(", ")); body.required = true;
    const save = el("button", `Save ${label}`, "primary"); save.type = "submit"; footer(form, save, close); dialog.append(form);
    form.addEventListener("submit", async event => { event.preventDefault(); save.disabled = true; try { if (await messageAction(record, { type: "edit_comment", comment: message.id, body: body.value.trim(), mentions: split(mentionsInput.value) }, { title: `Save ${label} on ${quoted(record.fields.title)}?`, commit: `Save ${label}`, dismiss: "Keep editing" })) close(); } catch (error) { report(error); } finally { save.disabled = !writable(); } });
  }
  function reactionEditor(record, message) {
    const { dialog, close } = modal("React to message"), form = el("form", undefined, "work-dialog-form"), emoji = field(form, "Reaction emoji or short label", "text", "👍"); emoji.required = true; emoji.maxLength = 32;
    const active = field(form, "Add my reaction (uncheck to remove)", "checkbox", true), save = el("button", "Save reaction", "primary"); save.type = "submit"; footer(form, save, close); dialog.append(form);
    form.addEventListener("submit", async event => { event.preventDefault(); save.disabled = true; try { if (await messageAction(record, { type: "react", comment: message.id, emoji: emoji.value.trim(), active: active.checked }, reactionWording(record, message, emoji.value.trim(), active.checked))) close(); } catch (error) { report(error); } finally { save.disabled = !writable(); } });
  }
  async function bulk(action, ids, wording = null) {
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
    return submitCaptured(captures, wording || bulkWording(action, patch, records));
  }
  function bulkWording(action, patch, records) {
    const target = records.length === 1 ? quoted(records[0].fields.title) : `${records.length} records`;
    const note = records.length > 1 ? "Multi-record commands are sequential, not an atomic transaction." : undefined;
    if (action.type === "status") return { title: `Set status of ${target} to ${title(patch.status)}?`, commit: "Set status", note };
    if (action.type === "priority") return { title: `Set priority of ${target} to ${PRIORITIES[patch.priority] || patch.priority}?`, commit: "Set priority", note };
    if (action.type === "assign") return patch.assignees.length ? { title: `Assign ${target} to ${patch.assignees[0]}?`, commit: "Assign", note } : { title: `Clear assignees of ${target}?`, commit: "Clear assignees", note };
    return { title: `Update ${target}?`, commit: "Update", note };
  }
  async function preview(id, parent) { if (!identity()) return; return loadDetail(id, parent, true); }
  window.KilnWorkUI = { render, clear, objects, refresh, open, bulk, preview, create };
  window.addEventListener("kiln-session-changed", () => { if (!identity() || state.principal && identity() !== state.principal) clear(); });
  // The shell's single page-level Refresh (#refresh-all) re-renders from cache; re-read the bounded snapshot too.
  document.addEventListener("click", event => {
    if (event.target instanceof Element && event.target.closest("#refresh-all") && state.parent?.isConnected && !state.parent.closest("[hidden]")) void refresh().catch(report);
  });
  window.dispatchEvent(new CustomEvent("kiln-work-ready"));
})();
