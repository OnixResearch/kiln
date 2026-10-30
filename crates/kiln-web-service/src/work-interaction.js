// r[impl kiln.work.interaction]
"use strict";

(() => {
  const DEBOUNCE_MS = 180;
  const MAX_RESULTS = 80;
  const WORK_ROOTS = "#work-content, #planning-content, #inbox-content, #triage-content, #workviews-content";
  const KINDS = ["issue", "project", "milestone", "cycle", "initiative", "team", "request", "view", "personal"];
  const PRIORITIES = ["No priority", "Urgent", "High", "Normal", "Low"];
  const selected = new Set();
  let generation = 0;
  let peekGeneration = 0;
  let timer = null;
  let palette = null;
  let peek = null;
  let menu = null;
  let toolbar = null;
  let activeRow = null;
  let anchorRow = null;
  let paletteReturn = null;
  let principal = null;
  let mutationBusy = false;
  let authGeneration = 0;
  const bridge = () => window.KilnWork;
  const ui = () => window.KilnWorkUI;
  const session = () => bridge()?.session();
  const identity = () => session()?.principal || null;
  const writable = () => Boolean(session()?.can_write);
  const node = (tag, text, className) => {
    const result = document.createElement(tag);
    if (text !== undefined) result.textContent = text;
    if (className) result.className = className;
    return result;
  };
  const errorText = (error) => error?.message || "The authenticated service is unavailable.";
  const editable = (target) => target instanceof Element && Boolean(target.closest("input, textarea, select, [contenteditable]:not([contenteditable='false']), [role='textbox']"));
  const interactive = (target) => target instanceof Element && Boolean(target.closest("a, button, input, textarea, select, summary, [contenteditable]:not([contenteditable='false'])"));
  const visible = (element) => element.isConnected && !element.closest("[hidden], [inert]") && element.getClientRects().length > 0;
  const rows = () => [...document.querySelectorAll("[data-work-id]")].filter((row) => row.closest(WORK_ROOTS) && visible(row));
  const notify = (error) => bridge()?.notice(errorText(error));
  const icon = (name) => window.KilnUi.icon(name);
  const humanize = (value) => String(value || "").replaceAll("_", " ").replace(/^./, (first) => first.toUpperCase());
  const kindName = (kind) => kind === "view" ? "Saved view" : kind === "personal" ? "Attention preferences" : humanize(kind);

  function button(label, action, className) {
    const result = node("button", label, className);
    result.type = "button";
    result.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      const epoch = authGeneration;
      Promise.resolve().then(() => { if (epoch === authGeneration) return action(); }).catch(notify);
    });
    return result;
  }

  function iconButton(label, name, action, className = "") {
    const result = button("", action, `icon-button quiet ${className}`.trim());
    result.setAttribute("aria-label", label);
    result.title = label;
    result.append(icon(name));
    return result;
  }

  function dialog(title, className) {
    const result = node("dialog", undefined, `work-interaction-dialog ${className}`);
    const heading = node("h2", title);
    heading.id = `${className}-title`;
    result.setAttribute("aria-labelledby", heading.id);
    const header = node("header", undefined, "work-interaction-header");
    header.append(heading, iconButton("Close", "x", () => result.close(), "work-interaction-close"));
    result.append(header);
    // Native modal dialogs make the background inert; keep keyboard focus inside
    // even on browsers whose dialog Tab behavior reaches browser chrome.
    result.addEventListener("keydown", (event) => {
      if (event.key !== "Tab") return;
      const controls = [...result.querySelectorAll("button, input, select, textarea, a[href], [tabindex='0']")].filter((item) => !item.disabled && visible(item));
      if (!controls.length) { event.preventDefault(); result.focus(); return; }
      const first = controls[0];
      const last = controls[controls.length - 1];
      if (event.shiftKey && (document.activeElement === first || !result.contains(document.activeElement))) { event.preventDefault(); last.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    });
    document.body.append(result);
    return result;
  }

  function restoreFocus(target) {
    if (document.querySelector("dialog[open]")) return;
    if (target?.isConnected && visible(target)) target.focus();
    else document.getElementById("work-command-open")?.focus();
  }

  function ensurePalette() {
    if (palette) return palette;
    const shell = dialog("Search and commands", "work-command-palette");
    const label = node("label", "Search work, CI projects, runs and retained reviews", "work-command-label");
    const input = node("input");
    input.id = "work-command-query";
    input.type = "search";
    input.autocomplete = "off";
    input.spellcheck = false;
    input.setAttribute("role", "combobox");
    input.setAttribute("aria-autocomplete", "list");
    input.setAttribute("aria-expanded", "true");
    input.setAttribute("aria-controls", "work-command-results");
    label.htmlFor = input.id;
    const status = node("p", "", "work-interaction-status");
    status.setAttribute("role", "status");
    const list = node("div", undefined, "work-command-results");
    list.id = "work-command-results";
    list.setAttribute("role", "listbox");
    list.setAttribute("aria-label", "Search results and commands");
    // r[impl kiln.web.page_consistency]
    // A one-line hint stays visible; the full keyboard reference (the input's description) is one disclosure away.
    const hint = node("p", "↑/↓ choose · Enter open · Esc close", "work-interaction-help work-command-hint");
    const shortcuts = node("details", undefined, "work-command-shortcuts");
    shortcuts.append(node("summary", "Keyboard shortcuts"));
    const help = node("p", "Ctrl/Cmd+K: search · ↑/↓: choose · Enter: open · Escape: close. Work rows: j/k or ↑/↓ to move, Enter to open, Space to peek, x to select, Shift+↑/↓ to select a range, Shift+F10 for actions. Selection checkboxes support bulk updates.", "work-interaction-help");
    help.id = "work-command-help";
    shortcuts.append(help);
    input.setAttribute("aria-describedby", help.id);
    const field = node("div", undefined, "work-command-field");
    const glass = icon("search");
    field.append(glass);
    field.append(input);
    const footer = node("div", undefined, "work-command-footer");
    footer.append(hint, shortcuts);
    shell.append(label, field, status, list, footer);
    const value = { shell, input, status, list, results: [], active: -1 };
    input.addEventListener("input", () => scheduleSearch());
    shell.addEventListener("keydown", (event) => {
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        choose(value.active + (event.key === "ArrowDown" ? 1 : -1), event.target !== input);
      } else if (event.key === "Home" && event.target !== input) { event.preventDefault(); choose(0, true); }
      else if (event.key === "End" && event.target !== input) { event.preventDefault(); choose(value.results.length - 1, true); }
      else if (event.key === "Enter" && event.target === input) {
        event.preventDefault();
        if (value.active >= 0) runResult(value.results[value.active]);
      } else if (event.key === "Escape" && event.target === input) {
        // A native search field consumes the first Escape to clear itself; Escape closes the palette instead.
        event.preventDefault();
        shell.close();
      }
    });
    shell.addEventListener("close", () => {
      if (shell.open) return;
      invalidateSearch();
      value.results = [];
      value.list.replaceChildren();
      value.input.removeAttribute("aria-activedescendant");
      value.input.setAttribute("aria-expanded", "false");
      restoreFocus(paletteReturn);
    });
    palette = value;
    return value;
  }

  function invalidateSearch() {
    generation += 1;
    clearTimeout(timer);
    timer = null;
  }

  function openPalette() {
    if (!identity()) { bridge()?.notice("Sign in to search authenticated workspace records."); return; }
    if (principal !== identity()) clear();
    principal = identity();
    const value = ensurePalette();
    if (value.shell.open) { value.input.focus(); return; }
    paletteReturn = document.activeElement;
    value.input.value = "";
    value.input.setAttribute("aria-expanded", "true");
    value.shell.showModal();
    value.input.focus();
    scheduleSearch(true);
  }

  const paletteOptions = () => palette ? [...palette.list.querySelectorAll("[role='option']")] : [];

  function choose(index, focus = false) {
    const value = palette;
    if (!value || !value.results.length) return;
    value.active = (index + value.results.length) % value.results.length;
    const options = paletteOptions();
    options.forEach((item, position) => {
      item.setAttribute("aria-selected", String(position === value.active));
      item.tabIndex = position === value.active ? 0 : -1;
    });
    const item = options[value.active];
    value.input.setAttribute("aria-activedescendant", item.id);
    item.scrollIntoView({ block: "nearest" });
    if (focus && document.activeElement !== item) item.focus();
  }

  function runResult(result) {
    if (!result || !identity() || principal !== identity()) { clear(); return; }
    const epoch = authGeneration;
    palette.shell.close();
    Promise.resolve().then(() => { if (epoch === authGeneration && identity()) return result.run(); }).catch(notify);
  }

  function builtins(query) {
    const entries = [
      ["Go to issues", { page: "work" }], ["Go to planning", { page: "planning" }],
      ["Go to my inbox", { page: "inbox" }], ["Go to requests and triage", { page: "triage" }],
      ["Go to saved views", { page: "workviews" }], ["Go to CI projects", { page: "projects" }],
      ["Go to CI runs", { page: "runs" }], ["Go to retained reviews", { page: "reviews" }],
    ].map(([label, patch]) => ({ label, group: "Go to", detail: "Navigation", run: () => bridge().navigate(patch) }));
    if (writable() && typeof ui()?.create === "function") {
      for (const kind of KINDS) entries.push({ label: `New ${kind === "view" ? "saved view" : kind === "personal" ? "attention preferences" : kind}…`, group: "Create", detail: "Open creation form", run: () => ui().create(kind) });
    }
    const filtered = entries.filter((entry) => matches(query, entry.label));
    if (query) {
      filtered.unshift({ label: `Search all work for “${query}”`, group: "Search", detail: "Open filtered work list", run: () => bridge().navigate({ page: "work", work_q: query, work_id: "" }) });
      filtered.push({ label: `Search CI runs for “${query}”`, group: "Search", detail: "Open server-filtered run browser", run: () => bridge().navigate({ page: "runs", q: query, cursor: "", run: "" }) });
    }
    return filtered;
  }

  function matches(query, ...values) {
    return query.toLocaleLowerCase().split(/\s+/).filter(Boolean).every((term) => values.some((value) => String(value ?? "").toLocaleLowerCase().includes(term)));
  }

  function scheduleSearch(immediate = false) {
    invalidateSearch();
    if (!palette?.shell.open) return;
    // Remove the old query's records immediately, not after debounce or a fetch.
    palette.results = [];
    palette.active = -1;
    palette.list.replaceChildren();
    palette.input.removeAttribute("aria-activedescendant");
    palette.status.textContent = "Searching authenticated records…";
    const token = generation;
    const who = identity();
    const query = palette.input.value.trim();
    timer = setTimeout(() => search(query, token, who), immediate ? 0 : DEBOUNCE_MS);
  }

  async function search(query, token, who) {
    const current = () => token === generation && who && who === identity() && palette?.shell.open;
    if (!current()) return;
    const sources = [
      ["Work", "/api/work", (data) => {
        if (!Array.isArray(data.objects)) throw new Error("Invalid work inventory");
        const favorites = new Set(data.objects.filter((record) => record.kind === "personal" && record.fields?.owner === who).flatMap((record) => record.fields?.personal?.favorites || []));
        return data.objects.filter((record) => matches(query, record.id, record.kind, record.fields?.title, record.fields?.description, ...(record.fields?.labels || []))).map((record) => ({
          label: record.fields?.title || record.id,
          detail: [favorites.has(record.id) ? "Favorite" : "", kindName(record.kind), humanize(record.fields?.status)].filter(Boolean).join(" · "),
          exact: record.id,
          rank: favorites.has(record.id) ? 0 : record.kind === "view" ? 1 : 2,
          run: () => record.kind === "view" ? bridge().navigate({ page: "work", work_view: record.id, work_id: "", work_kind: "", work_q: "", work_status: "", work_assignee: "", work_project: "", work_sort: "", work_group: "", work_layout: "" }) : ui().open(record.id),
        })).sort((a, b) => a.rank - b.rank || a.label.localeCompare(b.label));
      }],
      ["CI projects", "/api/projects", (data) => {
        if (!Array.isArray(data.available) || !Array.isArray(data.enrolled)) throw new Error("Invalid project inventory");
        const projects = new Map([...data.available, ...data.enrolled].map((project) => [project.id, project]));
        return [...projects.values()].filter((project) => matches(query, project.id, project.label, project.reference)).map((project) => ({ label: project.label || project.id, detail: "CI project", exact: project.id, run: () => bridge().navigate({ page: "runs", project: project.id, q: "", cursor: "", run: "" }) }));
      }],
      // r[impl kiln.web.readable_observations]
      ["CI runs", `/api/runs?${new URLSearchParams({ limit: "25", order: "desc", ...(query ? { q: query } : {}) })}`, (data) => {
        if (!Array.isArray(data.runs)) throw new Error("Invalid run inventory");
        const readable = typeof window.KilnUi?.statusPill === "function";
        return data.runs.map((run) => ({ label: readable ? run.project_id : `${run.project_id} · ${run.outcome}`, outcome: run.outcome ?? null, phase: run.phase, at: run.created_at_ms, detail: "CI run", exact: [run.id, run.source_revision].filter(Boolean).join(" · "), run: () => bridge().navigate({ page: "runs", run: run.id, node: "" }) }));
      }],
      ["Retained reviews", "/api/reviews", (data) => {
        if (!Array.isArray(data.receipts)) throw new Error("Invalid review inventory");
        return data.receipts.filter((receipt) => matches(query, receipt.name, receipt.provider_profile, ...Object.keys(receipt.decision_counts || {}))).map((receipt) => ({ label: receipt.name, detail: `Retained review · ${receipt.provider_profile || "profile unavailable"}`, run: () => bridge().navigate({ page: "reviews", receipt: receipt.name }) }));
      }],
    ];
    const results = await Promise.allSettled(sources.map(async ([, target, project]) => project(await bridge().api(target))));
    if (!current()) return;
    const errors = [];
    const entries = builtins(query);
    results.forEach((result, index) => {
      if (result.status === "fulfilled") entries.push(...result.value.map((entry) => ({ ...entry, group: sources[index][0] })));
      else errors.push(`${sources[index][0]} unavailable: ${errorText(result.reason)}`);
    });
    // r[impl kiln.web.console_layout]
    // Results are grouped under visible headings in first-appearance order; keyboard order follows the visual order.
    const groups = new Map();
    for (const entry of entries.slice(0, MAX_RESULTS)) {
      if (!groups.has(entry.group)) groups.set(entry.group, []);
      groups.get(entry.group).push(entry);
    }
    palette.results = [...groups.values()].flat();
    palette.active = -1;
    palette.list.replaceChildren();
    let index = 0;
    [...groups].forEach(([name, members], groupIndex) => {
      const group = node("div", undefined, "work-command-group");
      group.setAttribute("role", "group");
      const heading = node("div", name, "work-command-group-label");
      heading.id = `work-command-group-${token}-${groupIndex}`;
      heading.setAttribute("role", "presentation");
      group.setAttribute("aria-labelledby", heading.id);
      group.append(heading);
      for (const result of members) {
        const position = index++;
        const option = node("div", undefined, "work-command-result");
        option.id = `work-command-result-${token}-${position}`;
        option.setAttribute("role", "option");
        option.setAttribute("aria-selected", "false");
        option.tabIndex = -1;
        const head = node("span", undefined, "work-command-result-head");
        head.append(node("strong", result.label));
        if ("outcome" in result && typeof window.KilnUi?.statusPill === "function") head.append(window.KilnUi.statusPill(result.outcome, result.phase));
        // r[impl kiln.web.page_consistency]
        // Human context reads first; the exact identity follows as secondary monospace text.
        const detail = node("span", result.detail, "work-command-result-detail");
        if (result.exact) { detail.append(" · "); detail.append(node("span", result.exact, "mono work-command-result-exact")); }
        if (result.at != null && typeof window.KilnUi?.timeElement === "function") detail.append(" · ", window.KilnUi.timeElement(result.at));
        option.append(head, detail);
        option.addEventListener("click", () => runResult(result));
        option.addEventListener("focus", () => choose(position));
        option.addEventListener("keydown", (event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); runResult(result); } });
        group.append(option);
      }
      palette.list.append(group);
    });
    palette.status.textContent = `${palette.results.length} results and commands${entries.length > MAX_RESULTS ? ` (first ${MAX_RESULTS}; refine your search)` : ""}. CI runs show at most the latest 25 matching runs. ${errors.join(" · ")}`;
    if (palette.results.length) choose(0);
  }

  async function showPeek(id, returnTarget) {
    if (!identity()) return;
    peek?.close();
    const token = ++peekGeneration;
    const who = identity();
    const shell = dialog("Quick peek", "work-quick-peek");
    peek = shell;
    const content = node("div", "Loading persisted work object…", "work-quick-peek-content");
    const footer = node("div", undefined, "work-interaction-footer");
    footer.append(button("Open full detail", () => { shell.close(); return ui().open(id); }));
    shell.append(content, footer);
    // A record link inside the peek navigates; the peek closes so the destination is not hidden behind it.
    content.addEventListener("click", (event) => { if (event.defaultPrevented && event.target instanceof Element && event.target.closest("a[href]")) shell.close(); });
    shell.addEventListener("close", () => { if (peek === shell) { peekGeneration += 1; peek = null; } shell.remove(); restoreFocus(returnTarget); });
    shell.showModal();
    try {
      const staging = node("div");
      content.replaceChildren(staging);
      await ui().preview(id, staging);
      if (token !== peekGeneration || who !== identity() || !shell.open) return;
    } catch (error) {
      if (token === peekGeneration && who === identity() && shell.open) content.textContent = `Peek unavailable: ${errorText(error)}`;
    }
  }

  function rowFocus(row) {
    activeRow = row.dataset.workId;
    for (const item of rows()) item.tabIndex = item === row ? 0 : -1;
    row.focus({ preventScroll: true });
    row.scrollIntoView({ block: "nearest" });
  }

  function toggleSelection(row, checked = !selected.has(row.dataset.workId)) {
    const id = row.dataset.workId;
    if (checked) selected.add(id); else selected.delete(id);
    anchorRow = id;
    syncRows();
  }

  async function bulk(action, ids = [...selected]) {
    if (!writable() || mutationBusy || !ids.length) return;
    const who = identity();
    mutationBusy = true;
    updateToolbar();
    try {
      // WorkUI snapshots exact persisted heads and confirms the object IDs,
      // heads and values before submitting each command. Never retry here.
      await ui().bulk(action, ids);
    } catch (error) {
      if (who === identity()) notify(error);
    } finally {
      mutationBusy = false;
      if (who === identity()) { syncRows(); updateToolbar(); }
    }
  }

  function ensureToolbar() {
    if (toolbar) return;
    toolbar = node("aside", undefined, "work-selection-toolbar");
    toolbar.setAttribute("aria-label", "Selected work bulk actions");
    const count = node("strong");
    count.dataset.selectionCount = "true";
    count.setAttribute("role", "status");
    const statusLabel = node("label", "Status ");
    const status = node("input");
    status.type = "text";
    status.setAttribute("aria-label", "Bulk status (exact admitted workflow value)");
    statusLabel.append(status);
    const priorityLabel = node("label", "Priority ");
    const priority = node("select");
    priority.setAttribute("aria-label", "Bulk priority");
    for (let value = 0; value <= 4; value += 1) { const option = node("option", `${value} · ${PRIORITIES[value]}`); option.value = String(value); priority.append(option); }
    priorityLabel.append(priority);
    const assigneeLabel = node("label", "Assignee ");
    const assignee = node("input");
    assignee.type = "text";
    assignee.setAttribute("aria-label", "Bulk assignee (empty clears assignments)");
    assigneeLabel.append(assignee);
    const actions = node("div", undefined, "work-selection-fields");
    actions.dataset.mutationControls = "true";
    actions.append(statusLabel, button("Set status", () => { if (!status.value.trim()) throw new Error("Enter an exact admitted status first."); return bulk({ type: "status", status: status.value.trim() }); }), priorityLabel, button("Set priority", () => bulk({ type: "priority", priority: Number(priority.value) })), assigneeLabel, button("Set assignee", () => bulk({ type: "assign", assignee: assignee.value.trim() })));
    const header = node("div", undefined, "work-selection-header");
    header.append(count, button("Clear selection", () => { selected.clear(); syncRows(); }, "quiet"));
    toolbar.append(header, actions);
    toolbar.append(node("p", "Updates require confirmation of each object's exact head. Partial results remain selected for inspection; no automatic retry.", "work-interaction-help"));
    document.body.append(toolbar);
    updateToolbar();
  }

  function updateToolbar() {
    if (!toolbar) return;
    toolbar.hidden = !selected.size || !identity();
    toolbar.querySelector("[data-selection-count]").textContent = `${selected.size} selected`;
    toolbar.querySelector("[data-mutation-controls]").hidden = !writable();
    for (const control of toolbar.querySelectorAll("[data-mutation-controls] button, [data-mutation-controls] input, [data-mutation-controls] select")) control.disabled = mutationBusy || !writable();
  }

  // r[impl kiln.web.design_system]
  // Row actions are a real anchored menu, not a dialog: arrow keys/Home/End move, Enter/Space activate, Escape or
  // an outside click dismisses and focus returns to the trigger. Items show the row shortcut and use the detail's verbs.
  function personalChoices(who) {
    const record = (ui()?.objects() || []).filter((item) => item.kind === "personal" && item.fields?.owner === who).sort((a, b) => a.created_at_ms - b.created_at_ms || a.id.localeCompare(b.id))[0];
    return record?.fields?.personal || { watched: [], read: {}, favorites: [] };
  }
  function showMenu(row, anchor = row.querySelector("[data-work-menu]") || row) {
    const who = identity();
    if (!who) return;
    menu?.hidePopover();
    const id = row.dataset.workId;
    const record = ui()?.objects().find((item) => item.id === id);
    const returnTarget = anchor instanceof Element ? anchor : row;
    const shell = node("div", undefined, "work-context-menu");
    shell.popover = "auto";
    shell.setAttribute("role", "menu");
    shell.setAttribute("aria-label", `Actions for ${record?.fields?.title || id}`);
    menu = shell;
    let activated = false;
    shell.append(node("p", record?.fields?.title || id, "work-context-title"));
    const item = (label, shortcut, action) => {
      const result = button(label, () => { activated = true; shell.hidePopover(); returnTarget.focus?.({ preventScroll: true }); return action(); }, "quiet menu-item work-menu-item");
      result.setAttribute("role", "menuitem");
      result.tabIndex = -1;
      if (shortcut) { const key = node("kbd", shortcut); key.setAttribute("aria-hidden", "true"); result.append(key); }
      shell.append(result);
    };
    const separator = () => { const line = node("div", undefined, "work-menu-separator"); line.setAttribute("role", "separator"); shell.append(line); };
    item("Open detail", "Enter", () => ui().open(id));
    item("Quick peek", "Space", () => showPeek(id, row));
    item(selected.has(id) ? "Remove from selection" : "Add to selection", "X", () => toggleSelection(row));
    if (writable()) {
      const choices = personalChoices(who);
      const watched = (choices.watched || []).includes(id), favorite = (choices.favorites || []).includes(id), read = Boolean(record) && choices.read?.[id] === record.head;
      separator();
      item(watched ? "Unwatch" : "Watch", "", () => bulk({ type: "watch", active: !watched }, [id]));
      item(favorite ? "Remove favorite" : "Favorite", "", () => bulk({ type: "favorite", active: !favorite }, [id]));
      item(read ? "Mark unread" : "Mark read", "", () => bulk({ type: "read", active: !read }, [id]));
    }
    const items = () => [...shell.querySelectorAll("[role='menuitem']")];
    const move = (index) => { const list = items(); list[(index + list.length) % list.length]?.focus(); };
    shell.addEventListener("keydown", (event) => {
      const list = items(), index = list.indexOf(document.activeElement);
      if (event.key === "ArrowDown" || event.key === "ArrowUp") move(index + (event.key === "ArrowDown" ? 1 : -1));
      else if (event.key === "Home" || event.key === "End") move(event.key === "Home" ? 0 : list.length - 1);
      else if (event.key === "Escape" || event.key === "Tab") shell.hidePopover();
      else return;
      event.preventDefault();
      event.stopPropagation();
    });
    const trigger = returnTarget.matches?.("[data-work-menu]") ? returnTarget : null;
    shell.addEventListener("toggle", (event) => {
      trigger?.setAttribute("aria-expanded", String(event.newState === "open"));
      if (event.newState !== "closed") return;
      if (menu === shell) menu = null;
      shell.remove();
      // Dismissal returns focus to the trigger unless an outside click already moved it somewhere meaningful.
      const current = document.activeElement;
      if (!activated && (!current || current === document.body)) restoreFocus(returnTarget);
    });
    document.body.append(shell);
    shell.showPopover();
    trigger?.setAttribute("aria-expanded", "true");
    // Anchor below the trigger's trailing edge (or at the pointer), flipped and clamped inside the viewport.
    const box = anchor instanceof Element ? anchor.getBoundingClientRect() : { left: anchor.x, right: anchor.x, top: anchor.y, bottom: anchor.y };
    const gap = 4, edge = 8, width = shell.offsetWidth, height = shell.offsetHeight;
    const left = anchor instanceof Element ? box.right - width : box.left;
    const below = box.bottom + gap, above = box.top - gap - height;
    shell.style.left = `${Math.max(edge, Math.min(left, innerWidth - width - edge))}px`;
    shell.style.top = `${Math.max(edge, below + height <= innerHeight - edge || above < edge ? Math.min(below, innerHeight - height - edge) : above)}px`;
    move(0);
  }

  function syncRows() {
    if (!identity()) return;
    const currentRows = rows();
    const available = new Set((ui()?.objects() || []).map((item) => item.id));
    for (const id of selected) if (!available.has(id)) selected.delete(id);
    for (const row of currentRows) {
      const id = row.dataset.workId;
      row.tabIndex = activeRow === id ? 0 : -1;
      if (!row.querySelector("[data-work-select]")) {
        const checkbox = node("input", undefined, "work-row-select");
        checkbox.type = "checkbox";
        checkbox.dataset.workSelect = id;
        checkbox.setAttribute("aria-label", `Select ${row.getAttribute("aria-label") || id} for bulk updates`);
        checkbox.addEventListener("click", (event) => event.stopPropagation());
        checkbox.addEventListener("change", () => toggleSelection(row, checkbox.checked));
        // A 44px label carries the hit area on touch layouts while the box itself stays checkbox-sized.
        const hit = node("label", undefined, "work-row-select-hit");
        hit.addEventListener("click", (event) => event.stopPropagation());
        hit.append(checkbox);
        row.prepend(hit);
      }
      const checkbox = row.querySelector("[data-work-select]");
      checkbox.checked = selected.has(id);
      row.classList.toggle("work-row-selected", selected.has(id));
      if (!row.querySelector("[data-work-menu]")) {
        const control = iconButton("Actions", "ellipsis", () => showMenu(row, control), "work-row-menu");
        control.dataset.workMenu = id;
        control.setAttribute("aria-haspopup", "menu");
        control.setAttribute("aria-label", `Actions for ${row.getAttribute("aria-label") || id}`);
        control.title = `Actions for ${row.getAttribute("aria-label") || id}`;
        row.append(control);
      }
    }
    if (currentRows.length && !currentRows.some((row) => row.tabIndex === 0)) currentRows[0].tabIndex = 0;
    if (selected.size) ensureToolbar();
    updateToolbar();
  }

  function clear() {
    authGeneration += 1;
    principal = null;
    invalidateSearch();
    peekGeneration += 1;
    selected.clear();
    activeRow = null;
    anchorRow = null;
    paletteReturn = null;
    if (palette) {
      palette.results = [];
      palette.list.replaceChildren();
      palette.input.value = "";
      palette.status.textContent = "";
      if (palette.shell.open) palette.shell.close();
    }
    peek?.close();
    peek?.remove();
    peek = null;
    if (menu?.matches(":popover-open")) menu.hidePopover();
    menu?.remove();
    menu = null;
    toolbar?.remove();
    toolbar = null;
    for (const row of document.querySelectorAll(".work-row-selected")) row.classList.remove("work-row-selected");
    for (const checkbox of document.querySelectorAll("[data-work-select]")) checkbox.checked = false;
  }

  function refresh() {
    const who = identity();
    if (!who) { clear(); return; }
    if (principal && principal !== who) clear();
    principal = who;
    syncRows();
    if (palette?.shell.open) scheduleSearch();
  }

  document.addEventListener("click", (event) => {
    if (event.target instanceof Element && event.target.closest("#work-command-open")) { event.preventDefault(); openPalette(); }
  });
  document.addEventListener("focusin", (event) => {
    const row = event.target instanceof Element ? event.target.closest("[data-work-id]") : null;
    if (row?.closest(WORK_ROOTS)) activeRow = row.dataset.workId;
  });
  document.addEventListener("contextmenu", (event) => {
    const row = event.target instanceof Element ? event.target.closest("[data-work-id]") : null;
    if (row?.closest(WORK_ROOTS) && !editable(event.target) && identity()) { event.preventDefault(); showMenu(row, { x: event.clientX, y: event.clientY }); }
  });
  document.addEventListener("keydown", (event) => {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k" && !event.altKey) { event.preventDefault(); openPalette(); return; }
    if (!identity() || event.isComposing || document.querySelector("dialog[open]") || editable(event.target) || event.ctrlKey || event.metaKey || event.altKey) return;
    if (event.key === "Escape" && selected.size) { event.preventDefault(); selected.clear(); syncRows(); return; }
    const currentRows = rows();
    if (!currentRows.length || !["work", "planning", "inbox", "triage", "workviews"].includes(bridge()?.route()?.page)) return;
    const focused = event.target instanceof Element ? event.target.closest("[data-work-id]") : null;
    if (interactive(event.target) && event.target !== focused) return;
    let index = currentRows.findIndex((row) => row.dataset.workId === (focused?.dataset.workId || activeRow));
    const row = currentRows[index];
    const direction = event.key === "j" || event.key === "ArrowDown" ? 1 : event.key === "k" || event.key === "ArrowUp" ? -1 : 0;
    if (direction) {
      event.preventDefault();
      const next = currentRows[Math.max(0, Math.min(currentRows.length - 1, index < 0 ? (direction > 0 ? 0 : currentRows.length - 1) : index + direction))];
      if (event.shiftKey) {
        const start = currentRows.findIndex((item) => item.dataset.workId === (anchorRow || row?.dataset.workId));
        const end = currentRows.indexOf(next);
        if (!anchorRow && row) anchorRow = row.dataset.workId;
        for (let position = Math.min(start < 0 ? end : start, end); position <= Math.max(start < 0 ? end : start, end); position += 1) selected.add(currentRows[position].dataset.workId);
        syncRows();
      }
      rowFocus(next);
    } else if (row && event.key === "Enter") { event.preventDefault(); Promise.resolve(ui().open(row.dataset.workId)).catch(notify); }
    else if (row && event.key === " ") { event.preventDefault(); showPeek(row.dataset.workId, row).catch(notify); }
    else if (row && event.key.toLowerCase() === "x") { event.preventDefault(); toggleSelection(row); }
    else if (row && event.shiftKey && event.key === "F10") { event.preventDefault(); showMenu(row); }
  });
  window.addEventListener("kiln-work-updated", refresh);
  window.addEventListener("kiln-session-changed", () => { clear(); refresh(); });
  window.addEventListener("offline", clear);
  window.addEventListener("pagehide", clear);
  const rowObserver = new MutationObserver((records) => {
    if (identity() && records.some((record) => record.target instanceof Element && record.target.closest(WORK_ROOTS))) syncRows();
  });
  rowObserver.observe(document.body, { childList: true, subtree: true });
  window.KilnWorkInteraction = { clear, refresh };
  refresh();
})();
