// r[impl kiln.work.interaction]
"use strict";

(() => {
  const DEBOUNCE_MS = 180;
  const MAX_RESULTS = 80;
  const WORK_ROOTS = "#work-content, #planning-content, #inbox-content, #triage-content, #workviews-content";
  const KINDS = ["issue", "project", "milestone", "cycle", "initiative", "team", "request", "view", "personal"];
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

  function dialog(title, className) {
    const result = node("dialog", undefined, `work-interaction-dialog ${className}`);
    const heading = node("h2", title);
    heading.id = `${className}-title`;
    result.setAttribute("aria-labelledby", heading.id);
    const header = node("header", undefined, "work-interaction-header");
    header.append(heading, button("Close", () => result.close(), "work-interaction-close"));
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
    const label = node("label", "Search work, CI projects, runs and retained reviews");
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
    const help = node("p", "Ctrl/Cmd+K: search · ↑/↓: choose · Enter: open · Escape: close. Work rows: j/k or ↑/↓ to move, Enter to open, Space to peek, x to select, Shift+↑/↓ to select a range, Shift+F10 for actions. Selection checkboxes support bulk updates.", "work-interaction-help");
    help.id = "work-command-help";
    input.setAttribute("aria-describedby", help.id);
    shell.append(label, input, status, list, help);
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

  function choose(index, focus = false) {
    const value = palette;
    if (!value || !value.results.length) return;
    value.active = (index + value.results.length) % value.results.length;
    [...value.list.children].forEach((item, position) => {
      item.setAttribute("aria-selected", String(position === value.active));
      item.tabIndex = position === value.active ? 0 : -1;
    });
    const item = value.list.children[value.active];
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
    ].map(([label, patch]) => ({ label, detail: "Navigation", run: () => bridge().navigate(patch) }));
    if (writable() && typeof ui()?.create === "function") {
      for (const kind of KINDS) entries.push({ label: `New ${kind === "view" ? "saved view" : kind === "personal" ? "attention preferences" : kind}`, detail: "Open creation form", run: () => ui().create(kind) });
    }
    const filtered = entries.filter((entry) => matches(query, entry.label));
    if (query) {
      filtered.unshift({ label: `Search all work for “${query}”`, detail: "Open filtered work list", run: () => bridge().navigate({ page: "work", work_q: query, work_id: "" }) });
      filtered.push({ label: `Search CI runs for “${query}”`, detail: "Open server-filtered run browser", run: () => bridge().navigate({ page: "runs", q: query, cursor: "", run: "" }) });
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
          detail: `${favorites.has(record.id) ? "Favorite · " : ""}${record.kind === "view" ? "Saved view" : record.kind} · ${record.fields?.status || ""} · ${record.id}`,
          rank: favorites.has(record.id) ? 0 : record.kind === "view" ? 1 : 2,
          run: () => record.kind === "view" ? bridge().navigate({ page: "work", work_view: record.id, work_id: "", work_kind: "", work_q: "", work_status: "", work_assignee: "", work_project: "", work_sort: "", work_group: "", work_layout: "" }) : ui().open(record.id),
        })).sort((a, b) => a.rank - b.rank || a.label.localeCompare(b.label));
      }],
      ["CI projects", "/api/projects", (data) => {
        if (!Array.isArray(data.available) || !Array.isArray(data.enrolled)) throw new Error("Invalid project inventory");
        const projects = new Map([...data.available, ...data.enrolled].map((project) => [project.id, project]));
        return [...projects.values()].filter((project) => matches(query, project.id, project.label, project.reference)).map((project) => ({ label: project.label || project.id, detail: `CI project · ${project.id}`, run: () => bridge().navigate({ page: "runs", project: project.id, q: "", cursor: "", run: "" }) }));
      }],
      ["CI runs", `/api/runs?${new URLSearchParams({ limit: "25", order: "desc", ...(query ? { q: query } : {}) })}`, (data) => {
        if (!Array.isArray(data.runs)) throw new Error("Invalid run inventory");
        return data.runs.map((run) => ({ label: `${run.project_id} · ${run.outcome}`, detail: `CI run · ${run.id} · ${run.source_revision || ""}`, run: () => bridge().navigate({ page: "runs", run: run.id, node: "" }) }));
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
      if (result.status === "fulfilled") entries.push(...result.value);
      else errors.push(`${sources[index][0]} unavailable: ${errorText(result.reason)}`);
    });
    palette.results = entries.slice(0, MAX_RESULTS);
    palette.active = -1;
    palette.list.replaceChildren();
    palette.results.forEach((result, index) => {
      const option = node("div", undefined, "work-command-result");
      option.id = `work-command-result-${token}-${index}`;
      option.setAttribute("role", "option");
      option.setAttribute("aria-selected", "false");
      option.tabIndex = -1;
      option.append(node("strong", result.label), node("span", result.detail));
      option.addEventListener("click", () => runResult(result));
      option.addEventListener("focus", () => choose(index));
      option.addEventListener("keydown", (event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); runResult(result); } });
      palette.list.append(option);
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
    shell.append(content, button("Open full detail", () => { shell.close(); return ui().open(id); }));
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
    for (let value = 0; value <= 4; value += 1) { const option = node("option", String(value)); option.value = String(value); priority.append(option); }
    priorityLabel.append(priority);
    const assigneeLabel = node("label", "Assignee ");
    const assignee = node("input");
    assignee.type = "text";
    assignee.setAttribute("aria-label", "Bulk assignee (empty clears assignments)");
    assigneeLabel.append(assignee);
    const actions = node("div", undefined, "work-selection-fields");
    actions.dataset.mutationControls = "true";
    actions.append(statusLabel, button("Set status…", () => { if (!status.value.trim()) throw new Error("Enter an exact admitted status first."); return bulk({ type: "status", status: status.value.trim() }); }), priorityLabel, button("Set priority…", () => bulk({ type: "priority", priority: Number(priority.value) })), assigneeLabel, button("Set assignee…", () => bulk({ type: "assign", assignee: assignee.value.trim() })));
    toolbar.append(count, actions, button("Clear selection", () => { selected.clear(); syncRows(); }));
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

  function showMenu(row) {
    if (!identity()) return;
    menu?.close();
    const id = row.dataset.workId;
    const shell = dialog("Work actions", "work-context-menu");
    menu = shell;
    const record = ui()?.objects().find((item) => item.id === id);
    shell.append(node("p", record?.fields?.title || id, "work-context-title"));
    const act = (action) => () => { shell.close(); return action(); };
    shell.append(button("Open detail", act(() => ui().open(id))), button("Quick peek", act(() => showPeek(id, row))), button(selected.has(id) ? "Remove from selection" : "Add to selection", act(() => toggleSelection(row))));
    if (writable()) {
      shell.append(button("Toggle watch…", act(() => bulk({ type: "watch" }, [id]))), button("Toggle favorite…", act(() => bulk({ type: "favorite" }, [id]))), button("Mark current head read…", act(() => bulk({ type: "read" }, [id]))));
      shell.append(node("p", "Metadata changes for selected objects are available in the selection bar. Every mutation requires exact-head confirmation.", "work-interaction-help"));
    }
    shell.addEventListener("close", () => { if (menu === shell) menu = null; shell.remove(); restoreFocus(row); });
    shell.showModal();
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
        row.prepend(checkbox);
      }
      const checkbox = row.querySelector("[data-work-select]");
      checkbox.checked = selected.has(id);
      row.classList.toggle("work-row-selected", selected.has(id));
      if (!row.querySelector("[data-work-menu]")) {
        const control = button("Actions", () => showMenu(row), "work-row-menu");
        control.dataset.workMenu = id;
        control.setAttribute("aria-haspopup", "dialog");
        control.setAttribute("aria-label", `Actions for ${id}`);
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
    menu?.close();
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
    if (row?.closest(WORK_ROOTS) && !editable(event.target) && identity()) { event.preventDefault(); showMenu(row); }
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
