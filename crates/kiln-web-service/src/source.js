"use strict";

// Source page (?page=source): code-host navigation over explicitly admitted local repositories.
// A repository picker, a revision bar and a path breadcrumb lead to three URL-selected views
// (sview): Files (directory tree beside the selected file), History (bounded revision pages) and
// Changes (one pinned revision's diff, parsed per file). Reads only the bounded /api/sources
// routes through the shell's api helper; repository text is written through textContent only,
// never parsed as markup. Nothing is selected unless the URL or the reader selects it.
// r[impl kiln.dashboard.source.view]
// r[impl kiln.dashboard.source.navigation]
// r[impl kiln.highlight.render] File and Changes views request highlight=1 and draw the service's
// tokens through KilnUi.highlightLines; there is no client tokenizer.
(() => {
  // Revisions requested per /api/sources/<id>/log page (the service admits 1..=100).
  const LOG_PAGE_LIMIT = 30;
  // Typing in the entry filter narrows the list at once and commits it to the URL after this pause.
  const FILTER_COMMIT_MS = 350;
  const FILTER_MAX_CHARS = 256;
  const LINE_INPUT_MAX_CHARS = 19;
  const SHORT_ID_LENGTH = 12;
  // Bounded read caches, keyed by exact revision identity; cleared with the session.
  const CACHE_LIMITS = Object.freeze({ log: 4, tree: 24, file: 8, diff: 4 });
  const VIEWS = Object.freeze(["files", "history", "changes"]);
  // Narrow layouts wrap long lines by default; the reader's toggle wins for the session.
  const NARROW_QUERY = "(max-width: 40rem)";
  const KIND_LABELS = Object.freeze({ git: "Git", jj: "Jujutsu", pijul: "Pijul" });

  // ---------- Pure helpers (exported for tests) ----------

  const compareText = (a, b) => (a < b ? -1 : a > b ? 1 : 0);
  // Directories first, then every other entry; case-insensitive by name, exact name breaking ties.
  // Returns a new array; the listing itself is never reordered in place.
  function sortEntries(entries) {
    const rank = (entry) => (entry?.kind === "directory" ? 0 : 1);
    const name = (entry) => String(entry?.name ?? "");
    return (Array.isArray(entries) ? entries : []).slice().sort((a, b) => rank(a) - rank(b) || compareText(name(a).toLowerCase(), name(b).toLowerCase()) || compareText(name(a), name(b)));
  }

  // r[impl kiln.dashboard.source.navigation.scenario.anchor] A line ("12") or range ("12-20";
  // reversed ranges read low to high). Garbage is "invalid"; a line past the end is "outside" and
  // is reported, never clamped onto another line.
  const LINE_PATTERN = /^([1-9]\d{0,8})(?:-([1-9]\d{0,8}))?$/;
  function parseLineSelection(value, lineCount) {
    const raw = typeof value === "string" ? value : "";
    if (!raw) return { state: "none" };
    const match = LINE_PATTERN.exec(raw);
    if (!match) return { state: "invalid", value: raw };
    const first = Number(match[1]);
    const second = match[2] === undefined ? first : Number(match[2]);
    const start = Math.min(first, second);
    const end = Math.max(first, second);
    if (Number.isSafeInteger(lineCount) && end > lineCount) return { state: "outside", start, end, lines: lineCount };
    return { state: "ok", start, end };
  }
  function formatLineSelection(start, end) { return start === end ? String(start) : `${start}-${end}`; }

  // Lines of retained text: joining them with "\n" (plus the final newline when present) is the text.
  function splitLines(text) {
    const source = typeof text === "string" ? text : "";
    if (!source) return { lines: [], finalNewline: false };
    const lines = source.split("\n");
    const finalNewline = lines[lines.length - 1] === "";
    if (finalNewline) lines.pop();
    return { lines, finalNewline };
  }

  // r[impl kiln.dashboard.source.navigation.scenario.changes] A unified diff, per file: paths,
  // status, +/- counts and hunks whose lines carry old and new numbers. Every input line is kept
  // verbatim (as a header, hunk header or hunk line); lines that fit no form stay as context.
  // Hunk lines also carry their index in the diff text, which server highlighting addresses.
  const HUNK_HEADER = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@(.*)$/;
  const BINARY_LINE = /^Binary files (.+) and (.+) differ$/;
  function diffPath(value, prefix) {
    if (value === "/dev/null") return null;
    return value.startsWith(prefix) ? value.slice(prefix.length) : value;
  }
  function gitHeaderPaths(line) {
    const rest = line.slice("diff --git ".length);
    const half = (rest.length - "a/ b/".length) / 2;
    if (Number.isInteger(half) && half > 0 && rest.startsWith("a/") && rest.slice(2 + half, 5 + half) === " b/" && rest.slice(2, 2 + half) === rest.slice(5 + half)) {
      return [rest.slice(2, 2 + half), rest.slice(5 + half)];
    }
    const split = rest.lastIndexOf(" b/");
    if (rest.startsWith("a/") && split > 1) return [rest.slice(2, split), rest.slice(split + 3)];
    return [null, null];
  }
  function applyFileHeader(file, line) {
    if (line.startsWith("new file mode")) file.added = true;
    else if (line.startsWith("deleted file mode")) file.deleted = true;
    else if (line.startsWith("rename from ")) { file.renamed = true; file.oldPath = line.slice("rename from ".length); }
    else if (line.startsWith("rename to ")) { file.renamed = true; file.newPath = line.slice("rename to ".length); }
    else if (line === "GIT binary patch") file.binary = true;
    else if (line.startsWith("--- ")) { file.oldPath = diffPath(line.slice(4), "a/"); if (file.oldPath === null) file.added = true; }
    else if (line.startsWith("+++ ")) { file.newPath = diffPath(line.slice(4), "b/"); if (file.newPath === null) file.deleted = true; }
    else {
      const binary = BINARY_LINE.exec(line);
      if (!binary) return;
      file.binary = true;
      file.oldPath = diffPath(binary[1], "a/");
      file.newPath = diffPath(binary[2], "b/");
      if (file.oldPath === null) file.added = true;
      if (file.newPath === null) file.deleted = true;
    }
  }
  function parseDiff(text, options = {}) {
    const source = typeof text === "string" ? text : "";
    const lines = source ? source.split("\n") : [];
    const finalNewline = lines.length > 0 && lines[lines.length - 1] === "";
    if (finalNewline) lines.pop();
    const preamble = [];
    const files = [];
    let file = null;
    let hunk = null;
    let oldLine = 0; let newLine = 0; let oldLeft = 0; let newLeft = 0;
    for (const [index, line] of lines.entries()) {
      if (line.startsWith("diff --git ")) {
        const [oldPath, newPath] = gitHeaderPaths(line);
        file = { oldPath, newPath, added: false, deleted: false, renamed: false, binary: false, headers: [line], hunks: [], truncated: false };
        files.push(file);
        hunk = null;
        continue;
      }
      if (!file) { preamble.push(line); continue; }
      const header = HUNK_HEADER.exec(line);
      if (header) {
        oldLine = Number(header[1]); newLine = Number(header[3]);
        oldLeft = header[2] === undefined ? 1 : Number(header[2]);
        newLeft = header[4] === undefined ? 1 : Number(header[4]);
        hunk = { header: line, oldStart: oldLine, oldCount: oldLeft, newStart: newLine, newCount: newLeft, section: header[5].trim(), lines: [] };
        file.hunks.push(hunk);
        continue;
      }
      if (!hunk) { file.headers.push(line); applyFileHeader(file, line); continue; }
      const marker = line[0];
      const open = oldLeft > 0 || newLeft > 0;
      if (marker === "\\") hunk.lines.push({ kind: "no-newline", text: line, oldLine: null, newLine: null, index });
      else if (open && marker === " " && oldLeft > 0 && newLeft > 0) { hunk.lines.push({ kind: "context", text: line, oldLine, newLine, index }); oldLine += 1; newLine += 1; oldLeft -= 1; newLeft -= 1; }
      else if (open && marker === "+" && newLeft > 0) { hunk.lines.push({ kind: "add", text: line, oldLine: null, newLine, index }); newLine += 1; newLeft -= 1; }
      else if (open && marker === "-" && oldLeft > 0) { hunk.lines.push({ kind: "delete", text: line, oldLine, newLine: null, index }); oldLine += 1; oldLeft -= 1; }
      else hunk.lines.push({ kind: "context", text: line, oldLine: null, newLine: null, unparsed: true, index });
    }
    if (options.truncated && files.length) files[files.length - 1].truncated = true;
    const parsed = files.map((entry) => {
      const additions = entry.hunks.reduce((sum, part) => sum + part.lines.filter((row) => row.kind === "add").length, 0);
      const deletions = entry.hunks.reduce((sum, part) => sum + part.lines.filter((row) => row.kind === "delete").length, 0);
      const status = entry.renamed ? "renamed" : entry.added ? "added" : entry.deleted ? "deleted" : entry.binary ? "binary" : "modified";
      const path = entry.newPath ?? entry.oldPath ?? "";
      return Object.freeze({ path, oldPath: entry.oldPath, newPath: entry.newPath, status, binary: entry.binary, additions, deletions, headers: entry.headers, hunks: entry.hunks, truncated: entry.truncated });
    });
    return { files: parsed, preamble, finalNewline };
  }

  // ---------- DOM helpers ----------
  const ui = () => window.KilnUi;
  function element(tag, content, className) {
    const node = document.createElement(tag);
    if (content !== undefined && content !== null) node.textContent = content;
    if (className) node.className = className;
    return node;
  }
  // Callers name every icon literally through icon(name) so the reviewed-icon test sees each name.
  const icon = (name) => ui().icon(name);
  function withIcon(node, svg, label) {
    node.append(svg);
    if (label !== undefined) node.append(element("span", label));
    return node;
  }
  function isAborted(error) { return error?.name === "AbortError"; }
  function explain(error) {
    if (error?.name === "TimeoutError") return "The request timed out; its result is not known.";
    if (error instanceof TypeError) return "The service could not be reached. Its current state is unavailable.";
    const message = String(error?.message || "The service response was unavailable.");
    return /[.!?]$/.test(message) ? message : `${message}.`;
  }
  function loadingState(label) {
    const node = element("div", undefined, "loading-state");
    node.setAttribute("aria-busy", "true");
    for (const width of ["skeleton-wide", "skeleton-mid", "skeleton-short"]) node.append(element("span", undefined, `skeleton ${width}`));
    node.append(element("p", label, "small"));
    return node;
  }
  function emptyState(title, message, ...actions) {
    const node = element("div", undefined, "empty-state empty-state-inline");
    node.append(element("p", title, "empty-state-title"), element("p", message, "empty-state-text"));
    if (actions.length) { const row = element("div", undefined, "src-actions"); row.append(...actions); node.append(row); }
    return node;
  }
  function errorState(message) {
    const node = element("p", message, "warning error-state");
    node.dataset.state = "unavailable";
    return node;
  }
  function shortId(value) { const text = String(value ?? ""); return text.length > SHORT_ID_LENGTH ? text.slice(0, SHORT_ID_LENGTH) : text; }
  function exactId(value, className = "mono src-id") {
    const node = element("span", shortId(value), className);
    node.title = String(value ?? "");
    return node;
  }
  function authorName(value) { return String(value ?? "").split(" <")[0].trim() || "Author not recorded"; }
  function firstLine(value) { return String(value ?? "").split("\n")[0].trim(); }
  function plural(count, word, many = `${word}s`) { return `${count.toLocaleString("en-US")} ${count === 1 ? word : many}`; }
  function parentPath(path) { return String(path || "").split("/").filter(Boolean).slice(0, -1).join("/"); }
  function baseName(path) { return String(path || "").split("/").filter(Boolean).pop() || ""; }
  function utf8Length(text) { return new TextEncoder().encode(text).length; }

  // Hrefs merge into the current URL exactly like the shell's navigate(patch); plain clicks use navigate.
  function routeHref(patch) {
    const search = typeof location !== "undefined" && location?.search ? location.search : "";
    const params = new URLSearchParams(search);
    for (const [key, value] of Object.entries(patch)) { if (value) params.set(key, value); else params.delete(key); }
    return `?${params}`;
  }
  function plainClick(event) { return event.button === 0 && !event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey; }
  function routeLink(ctx, patch, label, className, focusKey) {
    const link = element("a", label, className);
    link.href = routeHref(patch);
    if (focusKey) link.dataset.srcFocus = focusKey;
    link.addEventListener("click", (event) => { if (!plainClick(event)) return; event.preventDefault(); ctx.navigate(patch); });
    return link;
  }
  function button(label, svg, className = "") {
    const node = element("button", undefined, `src-button ${className}`.trim());
    node.type = "button";
    node.append(svg, element("span", label, "src-button-label"));
    node.title = label;
    return node;
  }

  // ---------- Module state ----------
  let generation = 0;
  let activeController = null;
  let logMoreController = null;
  let mounted = null;
  // What is drawn in `mounted`: source id, tree key and file key; null while loading or empty.
  let shown = null;
  let filterTimer = 0;
  let wrapPreference = null;
  // A gutter click already shows its line, so the next render does not scroll to it.
  let revealSelection = true;
  let lineAnchor = null;
  const caches = { log: new Map(), tree: new Map(), file: new Map(), diff: new Map() };
  function cacheGet(kind, key) {
    const map = caches[kind];
    if (!key || !map.has(key)) return undefined;
    const value = map.get(key);
    map.delete(key);
    map.set(key, value);
    return value;
  }
  function cachePut(kind, key, value) {
    if (!key) return;
    const map = caches[kind];
    map.delete(key);
    map.set(key, value);
    while (map.size > CACHE_LIMITS[kind]) map.delete(map.keys().next().value);
  }

  // r[impl kiln.dashboard.source.view.scenario.restore] Every selection comes from the URL as given.
  function normalizeRoute(route) {
    const value = (key) => (typeof route?.[key] === "string" ? route[key] : "");
    return { src: value("src"), rev: value("rev"), spath: value("spath"), sfile: value("sfile"), sq: value("sq").slice(0, FILTER_MAX_CHARS), sview: VIEWS.includes(value("sview")) ? value("sview") : "files", sline: value("sline") };
  }
  const historicalSource = (selected) => selected.revision_scope === "revision" && selected.kind !== "pijul";

  // ---------- Reads ----------
  async function readAll(ctx, route, selected, apiOpts) {
    const id = encodeURIComponent(selected.id);
    const historical = historicalSource(selected);
    const head = typeof selected.head === "string" && selected.head ? selected.head : "";
    const viewScope = typeof selected?.view?.view === "string" ? selected.view.view : "";
    // Cache only reads bound to an exact revision and current restricted view binding.
    const expected = route.rev || (historical ? head : "");
    const key = (...parts) => (expected ? [selected.id, viewScope, expected, ...parts].join("\u0000") : "");
    const query = (values) => { const params = new URLSearchParams(); for (const [name, value] of Object.entries(values)) if (value) params.set(name, value); return params; };
    const settle = (promise) => promise.then((value) => ({ ok: true, value }), (error) => ({ ok: false, error }));
    const cached = (kind, cacheKey, read, valid) => {
      const hit = cacheGet(kind, cacheKey);
      if (hit) return Promise.resolve({ ok: true, value: hit });
      return settle(read().then((value) => {
        if (!valid(value)) throw new Error(`The source ${kind} response is invalid. Nothing was substituted.`);
        if (cacheKey && (kind === "log" || value.revision === expected)) cachePut(kind, cacheKey, value);
        return value;
      }));
    };
    const logKey = head ? [selected.id, viewScope, "log", head].join("\u0000") : "";
    const reads = {
      log: cached("log", logKey, () => ctx.api(`/api/sources/${id}/log?${query({ limit: String(LOG_PAGE_LIMIT) })}`, apiOpts).then((value) => ({ entries: Array.isArray(value?.entries) ? [...value.entries] : null, next_cursor: value?.next_cursor || null })), (value) => Array.isArray(value.entries)),
      tree: null, file: null, diff: null,
    };
    if (route.sview === "files") {
      reads.tree = cached("tree", key("tree", route.spath), () => ctx.api(`/api/sources/${id}/tree?${query({ revision: route.rev, path: route.spath })}`, apiOpts), (value) => Array.isArray(value?.entries));
      if (route.sfile) reads.file = cached("file", key("file", route.sfile), () => ctx.api(`/api/sources/${id}/file?${query({ revision: route.rev, path: route.sfile, highlight: "1" })}`, apiOpts), (value) => value?.binary === true || typeof value?.text === "string");
    }
    if (route.sview === "changes" && route.rev && historical) {
      reads.diff = cached("diff", key("diff"), () => ctx.api(`/api/sources/${id}/diff?${query({ revision: route.rev, highlight: "1" })}`, apiOpts), (value) => typeof value?.diff === "string");
    }
    const names = Object.keys(reads).filter((name) => reads[name]);
    const results = await Promise.all(names.map((name) => reads[name]));
    const out = { log: null, tree: null, file: null, diff: null, aborted: false };
    names.forEach((name, index) => { out[name] = results[index]; if (!results[index].ok && isAborted(results[index].error)) out.aborted = true; });
    return out;
  }

  // ---------- Header: repository picker, revision bar, view tabs ----------
  function repositoryPicker(ctx, route, sources) {
    const form = element("form", undefined, "src-picker");
    form.addEventListener("submit", (event) => event.preventDefault());
    const label = element("label", "Repository");
    label.htmlFor = "src-repository";
    const select = element("select");
    select.id = "src-repository";
    select.dataset.srcFocus = "repository";
    const placeholder = element("option", "Select a configured source");
    placeholder.value = "";
    select.append(placeholder);
    for (const row of sources) {
      const option = element("option", `${row.label || row.id} · ${KIND_LABELS[row.kind] || row.kind}${row.status === "available" ? "" : " · unavailable"}`);
      option.value = row.id;
      select.append(option);
    }
    if (route.src && !sources.some((row) => row.id === route.src)) {
      const option = element("option", `Unavailable selection: ${route.src}`);
      option.value = route.src;
      select.append(option);
    }
    select.value = route.src;
    select.addEventListener("change", () => ctx.navigate({ src: select.value, rev: "", spath: "", sfile: "", sq: "", sline: "" }));
    form.append(label, select);
    return form;
  }
  function sourceFacts(selected) {
    const facts = element("p", undefined, "src-facts");
    facts.append(element("span", KIND_LABELS[selected.kind] || String(selected.kind), "chip"));
    if (historicalSource(selected)) {
      facts.append(element("span", "Exact commits from the repository's Git object store", "small"));
    } else facts.append(element("span", `Channel ${selected.channel || "not recorded"} · current channel state and working copy only`, "small"));
    return facts;
  }
  function logEntries(log) { return log?.ok ? log.value.entries : []; }
  function revisionBar(ctx, route, selected, log) {
    const bar = element("div", undefined, "src-revision");
    bar.setAttribute("aria-label", "Revision");
    bar.setAttribute("role", "group");
    if (!historicalSource(selected)) {
      bar.append(withIcon(element("span", undefined, "chip src-rev-chip"), icon("git-branch"), `Channel ${selected.channel || "not recorded"}`));
      bar.append(element("span", route.rev ? `Revision ${route.rev} was requested, but Pijul reads cover only the current channel; no revision was substituted.` : "Historical Pijul trees are not readable.", route.rev ? "warning src-rev-note" : "small src-rev-note"));
      return bar;
    }
    const entries = logEntries(log);
    const head = typeof selected.head === "string" ? selected.head : "";
    const target = route.rev || head;
    const entry = entries.find((row) => row.id === target);
    const chip = element("span", undefined, `chip src-rev-chip${route.rev ? " is-pinned" : ""}`);
    withIcon(chip, route.rev ? icon("pin") : icon("git-commit-horizontal"), route.rev ? "Pinned" : "Head");
    bar.append(chip, exactId(target || "not recorded"));
    if (route.rev && route.rev === head) bar.append(element("span", "head", "chip"));
    const summary = entry ? firstLine(entry.summary) || "No message recorded" : route.rev ? "Not in the loaded history pages; exact reads are independent." : "";
    if (summary) { const node = element("span", summary, "src-rev-summary"); if (entry) node.title = String(entry.summary ?? ""); bar.append(node); }
    const actions = element("span", undefined, "src-rev-actions");
    if (route.rev) {
      const index = entries.findIndex((row) => row.id === route.rev);
      const step = (label, svg, row) => {
        if (!row) { const node = withIcon(element("span", undefined, "src-step"), svg, label); node.setAttribute("aria-disabled", "true"); node.title = `No ${label.toLowerCase()} revision in the loaded history pages`; return node; }
        const link = routeLink(ctx, { rev: row.id, sline: "" }, undefined, "src-step", `step:${label}`);
        link.title = `${label}: ${row.id}`;
        return withIcon(link, svg, label);
      };
      actions.append(step("Newer", icon("arrow-left"), index > 0 ? entries[index - 1] : null), step("Older", icon("arrow-right"), index >= 0 ? entries[index + 1] : null));
      actions.append(routeLink(ctx, { rev: "", sline: "" }, "Use head", "src-step", "use-head"));
    } else {
      actions.append(routeLink(ctx, { sview: "history" }, "Pin a revision…", "src-step", "pin"));
    }
    bar.append(actions);
    return bar;
  }
  function viewTabs(ctx, route) {
    const nav = element("nav", undefined, "run-tabs src-tabs");
    nav.setAttribute("aria-label", "Source views");
    for (const [view, label, svg] of [["files", "Files", icon("folder")], ["history", "History", icon("git-commit-horizontal")], ["changes", "Changes", icon("file-diff")]]) {
      const link = routeLink(ctx, { sview: view === "files" ? "" : view }, undefined, "", `tab:${view}`);
      withIcon(link, svg, label);
      if (route.sview === view) link.setAttribute("aria-current", "page");
      nav.append(link);
    }
    return nav;
  }

  // ---------- Files view ----------
  function breadcrumb(ctx, route, selected) {
    const nav = element("nav", undefined, "src-crumbs");
    nav.setAttribute("aria-label", "Path");
    const list = element("ol");
    const path = route.sfile || route.spath;
    const parts = path.split("/").filter(Boolean);
    const crumb = (content) => { const item = element("li"); item.append(content); list.append(item); };
    const leaf = (label) => { const node = element("span", label, "src-crumb-current"); node.setAttribute("aria-current", "page"); return node; };
    crumb(parts.length ? routeLink(ctx, { spath: "", sfile: "", sq: "", sline: "" }, selected.label || selected.id, "src-crumb", "crumb:") : leaf(selected.label || selected.id));
    let prefix = "";
    parts.forEach((part, index) => {
      prefix = prefix ? `${prefix}/${part}` : part;
      const last = index === parts.length - 1;
      crumb(last ? leaf(part) : routeLink(ctx, { spath: prefix, sfile: "", sq: "", sline: "" }, part, "src-crumb", `crumb:${prefix}`));
    });
    nav.append(list);
    return nav;
  }
  function entryLink(ctx, route, entry) {
    const directory = entry.kind === "directory";
    const patch = directory ? { spath: entry.path, sfile: "", sq: "", sline: "" } : { sfile: entry.path, sline: "" };
    const link = routeLink(ctx, patch, undefined, "src-entry", `entry:${entry.path}`);
    link.dataset.path = String(entry.path).toLowerCase();
    if (!directory && entry.path === route.sfile) link.setAttribute("aria-current", "page");
    link.append(directory ? icon("folder") : icon("file"), element("span", String(entry.name), "src-entry-name"));
    const size = directory ? "" : ui().humanBytes(entry.bytes);
    const sizeNode = element("span", size, "src-entry-size");
    if (size) sizeNode.title = `${entry.bytes.toLocaleString("en-US")} bytes`;
    link.append(sizeNode);
    if (!["directory", "file"].includes(entry.kind)) { link.append(element("span", String(entry.kind), "chip")); }
    return link;
  }
  function treePane(ctx, route, selected, tree) {
    const pane = element("section", undefined, "src-tree");
    pane.setAttribute("aria-label", "Directory entries");
    const head = element("div", undefined, "src-pane-head");
    head.append(element("h2", route.spath ? baseName(route.spath) : "Repository root", "src-pane-title"));
    if (route.spath) head.append(withIcon(routeLink(ctx, { spath: parentPath(route.spath), sfile: "", sq: "", sline: "" }, undefined, "src-step", "up"), icon("corner-left-up"), "Up one level"));
    pane.append(head);
    if (!tree?.ok) {
      pane.append(errorState(`Listing unavailable: ${explain(tree?.error)} No other directory was substituted.`));
      return pane;
    }
    const listing = tree.value;
    const entries = sortEntries(listing.entries);
    const filter = element("div", undefined, "src-filter");
    const label = element("label", "Filter entries");
    label.htmlFor = "src-filter";
    const input = element("input");
    Object.assign(input, { id: "src-filter", type: "search", maxLength: FILTER_MAX_CHARS, autocomplete: "off", placeholder: "Path substring in this directory", value: route.sq });
    input.dataset.srcFocus = "filter";
    filter.append(label, input);
    const count = element("p", undefined, "small src-count");
    count.setAttribute("role", "status");
    const list = element("ul", undefined, "src-entries");
    list.dataset.treeKey = [route.src, route.rev, route.spath].join("\u0000");
    const items = entries.map((entry) => { const item = element("li"); item.append(entryLink(ctx, route, entry)); list.append(item); return { item, path: String(entry.path).toLowerCase() }; });
    const noMatch = element("p", "No entries match this directory filter. Clear the filter to see this bounded listing; this is not a repository-wide search.", "small");
    const apply = (needle) => {
      const value = needle.toLowerCase();
      let visible = 0;
      for (const { item, path } of items) { const match = !value || path.includes(value); item.hidden = !match; if (match) visible += 1; }
      count.textContent = `${visible} of ${plural(entries.length, "entry", "entries")} · bounded listing${value ? ` · matching “${needle}”` : ""}`;
      noMatch.hidden = !(entries.length && !visible);
      return visible;
    };
    apply(route.sq);
    if (shown) shown.applyFilter = (nextSq) => {
      if (document.activeElement !== input) input.value = nextSq;
      apply(document.activeElement === input ? input.value : nextSq);
    };
    input.addEventListener("input", () => {
      apply(input.value);
      clearTimeout(filterTimer);
      const value = input.value;
      filterTimer = setTimeout(() => {
        if (!input.isConnected) return;
        if (shown?.route) shown.route = { ...shown.route, sq: value };
        if (typeof history !== "undefined" && typeof history.replaceState === "function" && typeof location !== "undefined") {
          history.replaceState(null, "", `${location.pathname || ""}${routeHref({ sq: value })}`);
        } else {
          ctx.navigate({ sq: value });
        }
      }, FILTER_COMMIT_MS);
    });
    pane.append(filter, count, list, noMatch);
    if (!entries.length) pane.append(element("p", "This directory is empty in the selected revision.", "small"));
    if (route.sfile && parentPath(route.sfile) !== route.spath) pane.append(element("p", "The selected file is outside this directory listing. Its exact content is read independently.", "small"));
    else if (route.sfile && route.sq && !String(route.sfile).toLowerCase().includes(route.sq.toLowerCase())) pane.append(element("p", "The selected file is outside this filtered directory listing. Its exact content is read independently.", "small"));
    return pane;
  }
  function scopeLine(listing) {
    if (!listing) return "";
    return `${listing.label || listing.source || ""} · ${KIND_LABELS[listing.kind] || listing.kind} · ${listing.revision_scope === "revision" ? `revision ${listing.revision}` : `channel ${listing.channel || "not recorded"}`}`;
  }
  function directorySummary(route, tree) {
    const card = element("section", undefined, "card src-dir");
    card.append(element("h2", route.spath || "Repository root", "src-pane-title"));
    if (tree?.ok) {
      const entries = tree.value.entries;
      const directories = entries.filter((entry) => entry.kind === "directory").length;
      const files = entries.length - directories;
      const total = entries.reduce((sum, entry) => (entry.kind !== "directory" && Number.isSafeInteger(entry.bytes) ? sum + entry.bytes : sum), 0);
      card.append(element("p", `${plural(directories, "directory", "directories")} · ${plural(files, "file")}${total ? ` · ${ui().humanBytes(total)} in listed files` : ""}`, "src-dir-counts"));
      card.append(element("p", scopeLine(tree.value), "small"));
    }
    card.append(element("p", "Select a file in the list to read it. No file is opened automatically.", "small"));
    if (route.sline) card.append(element("p", `Line selection ${route.sline} needs a selected file; nothing is highlighted.`, "warning"));
    return card;
  }
  // The view's highlighting state: the language when highlighted, else one explicit note.
  function highlightNote(plan, tag) {
    const node = element(tag, plan.note ?? ui().languageLabel(plan.language), "src-highlight");
    node.dataset.highlight = plan.state;
    return node;
  }
  function lineCountLabel(count, finalNewline) { return `${plural(count, "line")}${count && !finalNewline ? " · no final newline" : ""}`; }
  function fileViewer(ctx, route, file) {
    const view = element("section", undefined, "src-file");
    view.setAttribute("aria-label", `File ${file.path}`);
    view.dataset.fileKey = [route.src, route.rev, route.sfile].join("\u0000");
    const binary = file.binary === true || typeof file.text !== "string";
    const { lines, finalNewline } = binary ? { lines: [], finalNewline: false } : splitLines(file.text);
    const bytes = Number.isSafeInteger(file.bytes) ? file.bytes : binary ? null : utf8Length(file.text);
    const plan = binary ? null : ui().highlightLines(file.highlight, lines);
    const selection = binary ? (route.sline ? { state: "invalid", value: route.sline } : { state: "none" }) : parseLineSelection(route.sline, lines.length);

    const bar = element("div", undefined, "src-file-bar");
    const title = element("div", undefined, "src-file-title");
    const path = element("h2", String(file.path), "src-file-path mono");
    const meta = element("p", undefined, "src-file-meta");
    const size = element("span", bytes === null ? "size not recorded" : ui().humanBytes(bytes));
    if (bytes !== null) size.title = `${bytes.toLocaleString("en-US")} bytes`;
    meta.append(size, element("span", binary ? "binary" : lineCountLabel(lines.length, finalNewline)));
    if (file.oid) { const oid = exactId(file.oid); oid.title = `Object ${file.oid}`; meta.append(oid); }
    if (plan) meta.append(highlightNote(plan, "span"));
    if (file.revision_scope && file.revision_scope !== "revision") meta.append(element("span", "tracked working copy", "chip"));
    title.append(path, meta);

    const status = element("p", undefined, "small src-copy-status");
    status.setAttribute("role", "status");
    const actions = element("div", undefined, "src-file-actions");
    const copyPath = button("Copy path", icon("copy"));
    copyPath.dataset.srcFocus = "copy-path";
    copyPath.addEventListener("click", () => ui().copyText(String(file.path), status, "Path copied."));
    const copyLink = button("Copy link", icon("link"));
    copyLink.dataset.srcFocus = "copy-link";
    copyLink.addEventListener("click", () => ui().copyText(location.href, status, selection.state === "ok" ? `Link to line${selection.start === selection.end ? "" : "s"} ${formatLineSelection(selection.start, selection.end)} copied.` : "Link copied."));
    actions.append(copyPath, copyLink);
    if (!binary) {
      // Download writes exactly the retained text; binary objects have no retained text to offer.
      const download = button("Download", icon("download"));
      download.dataset.srcFocus = "download";
      download.addEventListener("click", () => {
        const url = URL.createObjectURL(new Blob([file.text], { type: "text/plain;charset=utf-8" }));
        const anchor = element("a");
        anchor.href = url;
        anchor.download = baseName(file.path) || "file.txt";
        document.body.append(anchor);
        anchor.click();
        anchor.remove();
        setTimeout(() => URL.revokeObjectURL(url), 0);
      });
      const wrap = button("Wrap lines", icon("text-wrap"), "src-wrap");
      wrap.dataset.srcFocus = "wrap";
      const wrapped = () => (wrapPreference === null ? window.matchMedia?.(NARROW_QUERY).matches === true : wrapPreference);
      const applyWrap = () => { view.classList.toggle("is-wrapped", wrapped()); wrap.setAttribute("aria-pressed", String(wrapped())); };
      wrap.addEventListener("click", () => { wrapPreference = !wrapped(); applyWrap(); });
      applyWrap();
      actions.append(download, wrap);
    }
    bar.append(title, actions);
    if (!binary && lines.length) bar.append(goToLine(ctx, lines.length, status));
    bar.append(status);
    view.append(bar);

    if (selection.state === "invalid") view.append(element("p", binary ? `Line selection ${selection.value} does not apply to a binary file; nothing is highlighted.` : `Line selection “${selection.value}” is not a line number or range; nothing is highlighted.`, "warning src-line-note"));
    if (selection.state === "outside") view.append(element("p", `Line${selection.start === selection.end ? ` ${selection.start} is` : `s ${selection.start}–${selection.end} are`} outside this file (${plural(selection.lines, "line")}). Nothing is highlighted and no other line was substituted.`, "warning src-line-note"));
    if (binary) { view.append(emptyState("Binary file", `This object is binary or not valid UTF-8${bytes === null ? "" : ` (${bytes.toLocaleString("en-US")} bytes)`}. No text is rendered and nothing is decoded.`)); return view; }
    if (!lines.length) { view.append(emptyState("Empty file", "This file has no content in the selected revision.")); return view; }
    view.append(codeTable(ctx, lines, plan, selection));
    return view;
  }
  function goToLine(ctx, lineCount, status) {
    const form = element("form", undefined, "src-goto");
    const label = element("label", "Go to line");
    label.htmlFor = "src-goto-line";
    const input = element("input");
    Object.assign(input, { id: "src-goto-line", type: "text", inputMode: "numeric", maxLength: LINE_INPUT_MAX_CHARS, autocomplete: "off", placeholder: `1–${lineCount} or 12-20` });
    input.dataset.srcFocus = "goto";
    const go = element("button", "Go");
    go.type = "submit";
    form.append(label, input, go);
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      const selection = parseLineSelection(input.value.trim(), lineCount);
      if (selection.state === "ok") { revealSelection = true; lineAnchor = selection.start; ctx.navigate({ sline: formatLineSelection(selection.start, selection.end) }); }
      else status.textContent = selection.state === "outside" ? `This file has ${plural(lineCount, "line")}; nothing was selected.` : "Enter a line number or a range such as 12-20.";
    });
    return form;
  }
  // The gutter is a column of line links (pointer selection; Shift extends a range). They leave the
  // tab order, which the Go to line form serves instead of one stop per line.
  function codeTable(ctx, lines, plan, selection) {
    const scroller = element("div", undefined, "src-code-scroll");
    const table = element("table", undefined, "src-code");
    table.setAttribute("role", "presentation");
    const body = element("tbody");
    const fragments = plan.fragments;
    const selected = selection.state === "ok" ? selection : null;
    lines.forEach((line, index) => {
      const number = index + 1;
      const row = element("tr");
      row.id = `src-L${number}`;
      const gutter = element("td", undefined, "src-ln");
      gutter.dataset.line = String(number);
      const link = element("a");
      link.href = routeHref({ sline: String(number) });
      link.dataset.line = String(number);
      link.tabIndex = -1;
      link.setAttribute("aria-label", `Line ${number}`);
      gutter.append(link);
      const code = element("td", undefined, "src-lc");
      if (fragments) ui().appendFragments(code, fragments[index]);
      else code.textContent = line;
      if (selected && number >= selected.start && number <= selected.end) {
        row.classList.add("is-selected");
        if (number === selected.start) row.classList.add("is-selection-start");
        if (number === selected.end) row.classList.add("is-selection-end");
        link.setAttribute("aria-current", "true");
      }
      row.append(gutter, code);
      body.append(row);
    });
    body.addEventListener("click", (event) => {
      const cell = event.target.closest?.("td.src-ln");
      if (!cell || event.button !== 0 || event.ctrlKey || event.metaKey || event.altKey) return;
      event.preventDefault();
      const number = Number(cell.dataset.line);
      const anchor = event.shiftKey && selected ? (lineAnchor !== null && lineAnchor >= 1 && lineAnchor <= lines.length ? lineAnchor : selected.start) : number;
      if (!event.shiftKey) lineAnchor = number;
      revealSelection = false;
      ctx.navigate({ sline: formatLineSelection(Math.min(anchor, number), Math.max(anchor, number)) });
    });
    table.append(body);
    scroller.append(table);
    return scroller;
  }
  function filesView(ctx, route, selected, reads) {
    const view = element("div", undefined, "src-files");
    view.append(breadcrumb(ctx, route, selected));
    const split = element("div", undefined, "src-split");
    const main = element("div", undefined, "src-main");
    if (!route.sfile) main.append(directorySummary(route, reads.tree));
    else if (!reads.file?.ok) main.append(errorState(`File unavailable: ${route.sfile} · ${explain(reads.file?.error)} No other file was substituted.`));
    else main.append(fileViewer(ctx, route, reads.file.value));
    split.append(treePane(ctx, route, selected, reads.tree), main);
    view.append(split);
    return view;
  }

  // ---------- History view ----------
  function commitRow(ctx, route, selected, entry) {
    const historical = historicalSource(selected);
    const item = element("li", undefined, "src-commit");
    const id = String(entry.id ?? "");
    const summary = firstLine(entry.summary) || "No message recorded";
    if (historical && id === route.rev) item.setAttribute("aria-current", "true");
    const main = element("div", undefined, "src-commit-main");
    const title = historical ? routeLink(ctx, { rev: id, sview: "changes", spath: "", sfile: "", sline: "" }, summary, "src-commit-title", `commit:${id}`) : element("span", summary, "src-commit-title");
    if (String(entry.summary ?? "").includes("\n")) title.title = String(entry.summary);
    const meta = element("p", undefined, "src-commit-meta");
    meta.append(element("span", authorName(entry.author)), ui().timeElement(Number.isSafeInteger(entry.recorded_at_ms) ? entry.recorded_at_ms : null), exactId(id));
    const badges = element("span", undefined, "src-badges");
    if (historical && id === selected.head) badges.append(element("span", "Head", "chip"));
    if (historical && id === route.rev) badges.append(withIcon(element("span", undefined, "chip is-pinned"), icon("pin"), "Pinned"));
    main.append(title, badges, meta);
    item.append(main);
    if (historical) item.append(withIcon(routeLink(ctx, { rev: id, sview: "files", spath: "", sfile: "", sline: "" }, undefined, "src-step", `browse:${id}`), icon("folder"), "Browse files"));
    return item;
  }
  function historyView(ctx, route, selected, log) {
    const section = element("section", undefined, "card src-history");
    section.setAttribute("aria-label", "History");
    const head = element("div", undefined, "section-heading");
    head.append(element("h2", "History", "card-title"));
    section.append(head);
    const historical = historicalSource(selected);
    if (!log?.ok) { section.append(errorState(`Revisions unavailable: ${explain(log?.error)}`)); return section; }
    const { entries, next_cursor: next } = log.value;
    section.append(element("p", `${plural(entries.length, "revision")} loaded${next ? " · more available" : " · end of history"}${historical ? " · select one to see its changes" : " · Pijul changes cover the current channel only; historical trees are not readable"}`, "small"));
    if (!entries.length) section.append(element("p", "No recorded revision is readable for this source.", "small"));
    let day = null;
    let list = null;
    for (const entry of entries) {
      const key = ui().localDay(Number.isSafeInteger(entry.recorded_at_ms) ? entry.recorded_at_ms : null);
      if (!list || (key?.key ?? "") !== (day?.key ?? "")) {
        day = key;
        section.append(element("h3", key ? key.label : "Time not recorded", "src-day"));
        list = element("ol", undefined, "src-commits");
        section.append(list);
      }
      list.append(commitRow(ctx, route, selected, entry));
    }
    if (historical && route.rev && !entries.some((entry) => entry.id === route.rev)) section.append(element("p", `Pinned revision ${route.rev} is not in the loaded history pages; exact reads are independent.`, "small"));
    if (next) {
      const more = element("button", "More revisions");
      more.type = "button";
      more.dataset.srcFocus = "more";
      const status = element("p", undefined, "small");
      status.setAttribute("role", "status");
      more.addEventListener("click", async () => {
        const before = generation;
        logMoreController?.abort();
        const controller = typeof AbortController === "function" ? new AbortController() : null;
        logMoreController = controller;
        more.disabled = true;
        status.textContent = "Reading more revisions…";
        let page;
        try { page = await ctx.api(`/api/sources/${encodeURIComponent(selected.id)}/log?${new URLSearchParams({ limit: String(LOG_PAGE_LIMIT), cursor: next })}`, controller ? { controller } : undefined); }
        catch (error) { if (before === generation && !isAborted(error)) { status.textContent = `More revisions unavailable: ${explain(error)}`; more.disabled = false; } return; }
        if (before !== generation || !section.isConnected) return;
        if (!Array.isArray(page?.entries)) { status.textContent = "The source log response is invalid. Nothing was appended."; more.disabled = false; return; }
        // The page object is the cached log page, so appended revisions survive later navigation.
        log.value.entries.push(...page.entries);
        log.value.next_cursor = page.next_cursor || null;
        const replacement = historyView(ctx, route, selected, log);
        section.replaceWith(replacement);
        const focus = replacement.querySelector('[data-src-focus="more"]') || replacement.querySelectorAll(".src-commit-title")[entries.length];
        focus?.focus?.();
      });
      const pager = element("div", undefined, "control-bar pager");
      pager.append(more);
      section.append(pager, status);
    }
    return section;
  }

  // ---------- Changes view ----------
  const STATUS_LABELS = Object.freeze({ added: ["Added", () => icon("file-plus")], deleted: ["Deleted", () => icon("file-minus")], renamed: ["Renamed", () => icon("file-pen")], binary: ["Binary", () => icon("file")], modified: ["Modified", () => icon("file-diff")] });
  function statusBadge(file) {
    const [label, statusIcon] = STATUS_LABELS[file.status];
    const badge = withIcon(element("span", undefined, "src-status"), statusIcon(), file.binary && file.status !== "binary" ? `${label} · binary` : label);
    badge.dataset.status = file.status;
    return badge;
  }
  function counts(file) {
    if (file.binary && !file.hunks.length) return element("span", "no text diff", "src-counts small");
    const node = element("span", undefined, "src-counts");
    node.append(element("span", `+${file.additions}`, "src-plus"), element("span", `−${file.deletions}`, "src-minus"));
    node.setAttribute("aria-label", `${plural(file.additions, "addition")}, ${plural(file.deletions, "deletion")}`);
    return node;
  }
  function filePathLabel(file) {
    if (file.status === "renamed" && file.oldPath && file.newPath && file.oldPath !== file.newPath) return `${file.oldPath} → ${file.newPath}`;
    return file.path || "path not recorded";
  }
  // A cell's content is text, or highlighted fragments that concatenate to that text.
  function diffRow(cells, className) {
    const row = element("tr", undefined, className);
    for (const [content, cellClass, span] of cells) {
      const cell = element("td", Array.isArray(content) ? undefined : content, cellClass);
      if (Array.isArray(content)) ui().appendFragments(cell, content);
      if (span) cell.colSpan = span;
      row.append(cell);
    }
    return row;
  }
  // `fragments` holds one fragment list per diff text line (indexed by line.index), or is null.
  function diffTable(file, fragments) {
    const scroller = element("div", undefined, "src-code-scroll");
    const table = element("table", undefined, "src-diff");
    table.setAttribute("aria-label", `Changes to ${file.path}`);
    const head = element("thead");
    head.append(diffRow([["Old", "src-dn"], ["New", "src-dn"], ["", "src-dm"], ["Line", "src-dc"]], "sr-only-row"));
    const body = element("tbody");
    for (const hunk of file.hunks) {
      body.append(diffRow([[hunk.header, "src-hunk-text", 4]], "src-hunk"));
      for (const line of hunk.lines) {
        const number = (value) => (value === null ? "" : String(value));
        const highlighted = fragments?.[line.index] ?? null;
        if (line.kind === "no-newline") { body.append(diffRow([["", "src-dn"], ["", "src-dn"], [line.text, "src-dc src-note", 2]], "src-d-note")); continue; }
        if (line.unparsed) { body.append(diffRow([["", "src-dn"], ["", "src-dn"], ["", "src-dm"], [highlighted ?? line.text, "src-dc"]], "src-d-context is-unparsed")); continue; }
        // The marker cell keeps the original +, - or space, so marker and code concatenate to the diff line.
        body.append(diffRow([[number(line.oldLine), "src-dn"], [number(line.newLine), "src-dn"], [line.text[0], "src-dm"], [highlighted ? ui().fragmentsFrom(highlighted, 1) : line.text.slice(1), "src-dc"]], `src-d-${line.kind}`));
      }
    }
    table.append(head, body);
    scroller.append(table);
    return scroller;
  }
  const ROUTINE_HEADER = /^(?:diff --git |index |--- |\+\+\+ )/;
  function diffFileSection(ctx, route, diff, file, index, fragments) {
    const section = element("section", undefined, "src-diff-file");
    section.id = `src-diff-${index}`;
    const details = element("details");
    details.open = true;
    const summary = element("summary");
    summary.dataset.srcFocus = `diff:${index}`;
    summary.append(icon("chevron-down"), statusBadge(file), element("span", filePathLabel(file), "src-diff-path mono"), counts(file));
    if (file.truncated) summary.append(element("span", "incomplete", "chip src-truncated"));
    details.append(summary);
    const body = element("div", undefined, "src-diff-body");
    const actions = element("div", undefined, "src-actions");
    if (file.newPath && file.status !== "deleted") actions.append(withIcon(routeLink(ctx, { sview: "", sfile: file.newPath, spath: parentPath(file.newPath), sq: "", sline: "" }, undefined, "src-step", `view:${index}`), icon("file"), "View file"));
    if (file.oldPath && diff.parent && file.status !== "added") actions.append(withIcon(routeLink(ctx, { sview: "", rev: diff.parent, sfile: file.oldPath, spath: parentPath(file.oldPath), sq: "", sline: "" }, undefined, "src-step", `parent:${index}`), icon("arrow-left"), "View before"));
    if (actions.childNodes.length) body.append(actions);
    const extra = file.headers.filter((line) => !ROUTINE_HEADER.test(line));
    if (extra.length) { const meta = element("ul", undefined, "src-diff-meta mono"); for (const line of extra) meta.append(element("li", line)); body.append(meta); }
    if (file.truncated) body.append(element("p", "This file's diff is incomplete: the read stopped at its byte bound. Missing lines are not reconstructed.", "warning"));
    if (file.hunks.length) body.append(diffTable(file, fragments));
    else if (file.binary) body.append(element("p", "Binary content has no textual diff; nothing is decoded.", "small"));
    else if (!file.truncated) body.append(element("p", "No line changes (mode, rename or empty-file change only).", "small"));
    details.append(body);
    section.append(details);
    return section;
  }
  function changesView(ctx, route, selected, reads) {
    const view = element("div", undefined, "src-changes view-stack");
    if (!historicalSource(selected)) {
      view.append(emptyState("Historical changes are not readable", "Pijul's command line exposes no historical diff, so Kiln does not approximate one. Files show the current channel state; History lists the recorded changes.", routeLink(ctx, { sview: "history" }, "Open History", "button", "open-history")));
      return view;
    }
    if (!route.rev) {
      const actions = [routeLink(ctx, { sview: "history" }, "Open History", "button", "open-history")];
      if (selected.head) actions.push(routeLink(ctx, { rev: selected.head, sline: "" }, `Changes at head ${shortId(selected.head)}`, "button", "head-changes"));
      view.append(emptyState("No revision pinned", "Changes show one exact revision against its first parent. Pin a revision from History; none is chosen automatically.", ...actions));
      return view;
    }
    if (!reads.diff?.ok) { view.append(errorState(`Changes unavailable: ${explain(reads.diff?.error)} Nothing was substituted.`)); return view; }
    const diff = reads.diff.value;
    const parsed = parseDiff(diff.diff, { truncated: diff.truncated === true });
    const plan = ui().highlightLines(diff.highlight, splitLines(diff.diff).lines);
    const entry = logEntries(reads.log).find((row) => row.id === diff.revision);
    const card = element("section", undefined, "card src-change-summary");
    const heading = element("h2", entry ? firstLine(entry.summary) || "No message recorded" : `Revision ${shortId(diff.revision)}`, "card-title");
    card.append(heading);
    const meta = element("p", undefined, "src-commit-meta");
    if (entry) meta.append(element("span", authorName(entry.author)), ui().timeElement(Number.isSafeInteger(entry.recorded_at_ms) ? entry.recorded_at_ms : null));
    meta.append(exactId(diff.revision));
    if (diff.parent) { const parent = element("span", "against parent "); parent.append(routeLink(ctx, { rev: diff.parent, sline: "" }, shortId(diff.parent), "mono src-id", "parent")); parent.lastChild.title = `Parent ${diff.parent}`; meta.append(parent); }
    else meta.append(element("span", "root commit · compared with an empty tree"));
    card.append(meta);
    const total = Number.isSafeInteger(diff.files_changed) ? diff.files_changed : parsed.files.length;
    const stats = element("p", undefined, "src-change-stats");
    stats.append(element("strong", plural(total, "file") + " changed"), element("span", `+${diff.insertions ?? "?"}`, "src-plus"), element("span", `−${diff.deletions ?? "?"}`, "src-minus"));
    card.append(stats);
    if (diff.truncated) card.append(element("p", `This diff is truncated at its read bound: ${parsed.files.length} of ${plural(total, "changed file")} are shown and the last shown file is incomplete. Missing files and lines are not reconstructed.`, "warning"));
    if (plan.note) card.append(highlightNote(plan, "p"));
    view.append(card);
    if (!parsed.files.length) {
      view.append(element("p", parsed.preamble.length ? "The diff text has no per-file sections; it is shown as returned." : total ? "No file sections were returned within the read bound." : "This revision changes no files.", "small"));
      if (parsed.preamble.length) { const pre = element("pre", parsed.preamble.join("\n"), "identity"); view.append(pre); }
      return view;
    }
    const index = element("nav", undefined, "card src-changed");
    index.setAttribute("aria-label", "Changed files");
    const list = element("ul");
    parsed.files.forEach((file, position) => {
      const item = element("li");
      const link = element("a", undefined, "src-changed-link");
      link.href = `#src-diff-${position}`;
      link.dataset.srcFocus = `changed:${position}`;
      link.append(statusBadge(file), element("span", filePathLabel(file), "src-diff-path mono"), counts(file));
      // In-page jumps scroll without changing the URL, so history keeps one entry per selection.
      link.addEventListener("click", (event) => {
        if (!plainClick(event)) return;
        event.preventDefault();
        const target = document.getElementById(`src-diff-${position}`);
        if (!target) return;
        target.querySelector("details").open = true;
        target.scrollIntoView({ block: "start" });
        target.querySelector("summary")?.focus({ preventScroll: true });
      });
      item.append(link);
      list.append(item);
    });
    index.append(list);
    view.append(index);
    parsed.files.forEach((file, position) => view.append(diffFileSection(ctx, route, diff, file, position, plan.fragments)));
    return view;
  }

  // ---------- Page ----------
  function repositoryChooser(ctx, sources) {
    const list = element("ul", undefined, "src-repos");
    for (const row of sources) {
      const item = element("li");
      const link = routeLink(ctx, { src: row.id, rev: "", spath: "", sfile: "", sq: "", sline: "", sview: "" }, undefined, "src-repo", `repo:${row.id}`);
      link.append(icon("git-branch"), element("span", row.label || row.id, "src-repo-name"), element("span", `${KIND_LABELS[row.kind] || row.kind}${row.status === "available" ? "" : " · unavailable"}`, "chip"));
      const detail = row.status !== "available" ? `Unavailable: ${row.reason || "reason not recorded"}` : historicalSource(row) ? `Head ${shortId(row.head) || "not recorded"}` : `Channel ${row.channel || "not recorded"}`;
      link.append(element("span", detail, "small src-repo-detail"));
      item.append(link);
      list.append(item);
    }
    return list;
  }
  function aboutView() {
    const details = element("details", undefined, "about-view");
    const body = element("div", undefined, "about-view-body");
    body.append(element("p", "Read-only local repository inspection for Git, Jujutsu and Pijul. Git and Jujutsu views resolve an exact commit in the repository's Git object store; Pijul views cover its current channel state and working copy. No fetch, checkout, mutation, signing or publication."));
    body.append(element("p", "Listings, files, history pages and diffs are bounded reads. Syntax highlighting comes from the service's grammars and is presentation only: the rendered, copied and downloaded text is the retained text. Unsupported, oversized or malformed highlighting renders plain with a note."));
    details.append(ui().disclosureSummary("About this view", "info"), body);
    return details;
  }
  function renderPage(ctx, route, inventory, sources, selected, reads) {
    const page = element("div", undefined, "src-page view-stack");
    const header = element("section", undefined, "card src-header");
    const top = element("div", undefined, "src-header-top");
    top.append(repositoryPicker(ctx, route, sources));
    if (selected) top.append(sourceFacts(selected));
    header.append(top, aboutView());
    page.append(header);
    if (!inventory?.configured) { page.append(errorState("No source repositories are configured. The service operator must admit a separate --sources policy. This is not proof that no repository is readable.")); return page; }
    if (!route.src) {
      page.append(sources.length ? emptyState("Choose a repository", "Pick a configured repository to browse its files, history and changes. No repository is selected automatically.") : emptyState("No repositories", "The admitted source policy lists no repository."));
      if (sources.length) page.append(repositoryChooser(ctx, sources));
      return page;
    }
    if (!selected) {
      page.append(errorState(`Source not found in the configured inventory: ${route.src}. No repository was substituted.`));
      page.append(routeLink(ctx, { src: "", rev: "", spath: "", sfile: "", sq: "", sline: "", sview: "" }, "Clear source selection", "button", "clear"));
      return page;
    }
    if (selected.status !== "available") {
      page.append(errorState(`Selected source unavailable: ${selected.reason || "reason not recorded"}. No repository was substituted.`));
      page.append(routeLink(ctx, { src: "", rev: "", spath: "", sfile: "", sq: "", sline: "", sview: "" }, "Clear source selection", "button", "clear"));
      return page;
    }
    header.append(revisionBar(ctx, route, selected, reads.log));
    if (!reads.log?.ok) header.append(element("p", `Revisions unavailable: ${explain(reads.log?.error)}`, "warning"));
    page.append(viewTabs(ctx, route));
    if (route.sview === "files") page.append(filesView(ctx, route, selected, reads));
    else if (route.sview === "history") page.append(historyView(ctx, route, selected, reads.log));
    else page.append(changesView(ctx, route, selected, reads));
    return page;
  }

  function focusState(container) {
    const active = document.activeElement;
    if (!active || !container.contains(active)) return null;
    // A clicked gutter link keeps focus on its line after the selection re-renders.
    const key = active.dataset?.srcFocus || (active.closest?.(".src-ln") ? `line:${active.dataset.line}` : "");
    if (!key) return null;
    return { key, caret: typeof active.selectionStart === "number" ? active.selectionStart : null };
  }
  function restoreFocus(container, focus) {
    if (!focus) return;
    if (focus.key.startsWith("line:")) { container.querySelector(`#src-L${focus.key.slice("line:".length)} .src-ln a`)?.focus({ preventScroll: true }); return; }
    for (const node of container.querySelectorAll("[data-src-focus]")) {
      if (node.dataset.srcFocus !== focus.key) continue;
      node.focus({ preventScroll: true });
      if (focus.caret !== null && typeof node.setSelectionRange === "function") { try { node.setSelectionRange(focus.caret, focus.caret); } catch { /* not a text control */ } }
      return;
    }
  }
  function settleScroll(container, before, route) {
    const list = container.querySelector(".src-entries");
    if (list && before.tree && list.dataset.treeKey === before.tree.key) list.scrollTop = before.tree.top;
    else if (list) list.querySelector('[aria-current="page"]')?.scrollIntoView({ block: "nearest" });
    const file = container.querySelector(".src-file");
    const selectedRow = container.querySelector("tr.is-selected");
    if (selectedRow && revealSelection) selectedRow.scrollIntoView({ block: "center" });
    else if (file && file.dataset.fileKey !== before.file && !route.sline && file.getBoundingClientRect().top < 0) file.scrollIntoView({ block: "start" });
    revealSelection = true;
  }

  function sameExceptSq(a, b) {
    if (!a || !b) return false;
    return a.src === b.src && a.rev === b.rev && a.spath === b.spath && a.sfile === b.sfile && a.sview === b.sview && a.sline === b.sline;
  }

  async function load(container, ctx) {
    if (!container) return;
    const route = normalizeRoute(ctx?.route);
    if (mounted === container && shown?.route && sameExceptSq(shown.route, route) && typeof shown.applyFilter === "function" && container.querySelector("#src-filter")) {
      shown.route = route;
      shown.applyFilter(route.sq);
      return;
    }
    const current = ++generation;
    activeController?.abort();
    logMoreController?.abort();
    logMoreController = null;
    const controller = typeof AbortController === "function" ? new AbortController() : null;
    activeController = controller;
    const apiOpts = controller ? { controller } : undefined;
    clearTimeout(filterTimer);
    if (mounted !== container) { mounted?.replaceChildren(); shown = null; }
    mounted = container;
    const live = () => current === generation && container.isConnected;
    // The same repository keeps its page on screen while the next selection is read.
    if (shown?.src !== route.src || !container.firstChild) container.replaceChildren(loadingState("Reading configured sources…"));
    let inventory;
    try { inventory = await ctx.api("/api/sources", apiOpts); }
    catch (error) {
      if (!live() || isAborted(error)) return;
      shown = null;
      container.replaceChildren(errorState(`Source inventory unavailable: ${explain(error)} No repository was substituted.`));
      return;
    }
    if (!live()) return;
    const sources = Array.isArray(inventory?.sources) ? inventory.sources.filter((row) => row && typeof row.id === "string") : [];
    const selected = sources.find((row) => row.id === route.src) || null;
    const reads = selected && selected.status === "available" ? await readAll(ctx, route, selected, apiOpts) : { log: null, tree: null, file: null, diff: null, aborted: false };
    if (!live() || reads.aborted) return;
    const focus = focusState(container);
    const activeFilter = document.activeElement?.id === "src-filter" ? document.activeElement.value : null;
    const list = container.querySelector(".src-entries");
    const before = { tree: list ? { key: list.dataset.treeKey, top: list.scrollTop } : null, file: container.querySelector(".src-file")?.dataset.fileKey || null };
    shown = { src: route.src, route, applyFilter: null };
    container.replaceChildren(renderPage(ctx, route, inventory, sources, selected, reads));
    if (activeFilter !== null && typeof shown.applyFilter === "function") {
      const nextInput = container.querySelector("#src-filter");
      if (nextInput) { nextInput.value = activeFilter; shown.applyFilter(activeFilter); }
    }
    restoreFocus(container, focus);
    settleScroll(container, before, route);
    const where = route.sview === "files" ? route.sfile || route.spath : route.sview === "history" ? "History" : route.rev ? `Changes ${shortId(route.rev)}` : "Changes";
    document.title = ["Kiln", "Source", selected ? selected.label || selected.id : "", where].filter(Boolean).join(" · ");
  }
  function clear() {
    generation += 1;
    activeController?.abort();
    activeController = null;
    logMoreController?.abort();
    logMoreController = null;
    clearTimeout(filterTimer);
    mounted?.replaceChildren();
    mounted = null;
    shown = null;
    lineAnchor = null;
    revealSelection = true;
    wrapPreference = null;
    for (const map of Object.values(caches)) map.clear();
  }

  // ---------- Repositories & Fleet Copies hub (?page=source&tab=repositories) ----------
  // r[impl kiln.web.repository_import_ui]
  const MAX_REMOTE_URL_BYTES = 2048;
  const RADICLE_NOTICE = "Read-only declaration/observation only: importing a Radicle RID never runs rad init, signing, push, sync, or delegate mutation.";
  const BASE58_PATTERN = /^[1-9A-HJ-NP-Za-km-z]+$/;
  const GITHUB_SEGMENT = /^[A-Za-z0-9_.-]+$/;
  const PROVIDER_LABELS = Object.freeze({ github: "GitHub", gitea: "Gitea", radicle: "Radicle" });
  const SYNC_META = Object.freeze({
    synced: Object.freeze({ status: "success", label: "Synced" }),
    diverged: Object.freeze({ status: "failure", label: "Diverged" }),
    dirty: Object.freeze({ status: "warning", label: "Dirty" }),
    unknown: Object.freeze({ status: "unknown", label: "Unknown" }),
    local_only: Object.freeze({ status: "queued", label: "Local Only" }),
  });

  function hasControlOrSpace(value) {
    return /[\u0000-\u001f\u007f\s]/.test(value);
  }
  function isDisallowedHost(rawHost) {
    const host = String(rawHost || "").toLowerCase().replace(/^\[|\]$/g, "");
    if (!host || host === "localhost" || host.endsWith(".localhost")) return true;
    if (host.includes(":")) return true; // IPv6 literal
    const ipv4 = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(host);
    if (ipv4) return true; // Reject all bare IPv4 literals
    if (!host.includes(".")) return true;
    return false;
  }
  function parseOwnerRepoPath(path) {
    const trimmed = String(path || "").replace(/^\/+/, "").replace(/\.git$/i, "");
    const parts = trimmed.split("/");
    if (parts.length !== 2) return null;
    const [owner, repo] = parts;
    if (!owner || !repo || owner === "." || owner === ".." || repo === "." || repo === ".." || owner.startsWith("-")) return null;
    if (!GITHUB_SEGMENT.test(owner) || !GITHUB_SEGMENT.test(repo)) return null;
    return [owner, repo];
  }
  function validateRemoteInput({ provider, role, url, attachRepo = null }) {
    if (!["github", "gitea", "radicle"].includes(provider)) return { ok: false, error: "Select GitHub, Gitea, or Radicle." };
    if (!["canonical", "mirror"].includes(role)) return { ok: false, error: "Role must be Canonical or Mirror." };
    const raw = typeof url === "string" ? url.trim() : "";
    if (!raw) return { ok: false, error: "Enter a repository URL, owner/repo slug, or Radicle RID." };
    if (utf8Length(raw) > MAX_REMOTE_URL_BYTES) return { ok: false, error: `Endpoint exceeds ${MAX_REMOTE_URL_BYTES} bytes.` };
    if (hasControlOrSpace(raw)) return { ok: false, error: "Endpoint must not contain whitespace or control characters." };

    // Reject embedded credentials in URLs (allow standard git@host:owner/repo or ssh://git@host/...).
    if (/^https?:\/\/[^/]*@/i.test(raw)) return { ok: false, error: "Embedded credentials in HTTP(S) URLs are forbidden." };
    if (/^ssh:\/\/(?!git@)[^/]*@/i.test(raw)) return { ok: false, error: "Embedded credentials in SSH URLs are forbidden (only git@ is allowed)." };
    if (/^ssh:\/\/git@[^/:]+:[^0-9/][^/]*@/i.test(raw)) return { ok: false, error: "Embedded SSH passwords are forbidden." };
    if (/^[A-Za-z0-9._-]+:[^/@]+@/.test(raw)) return { ok: false, error: "Embedded credentials are forbidden." };

    if (attachRepo && role === "canonical" && (attachRepo.canonical || (Array.isArray(attachRepo.remotes) && attachRepo.remotes.some((r) => r.role === "canonical")))) {
      return { ok: false, error: "This repository already has a canonical remote; choose Mirror / Projection instead." };
    }

    if (provider === "radicle") {
      let id = raw;
      if (id.startsWith("rad::rad://")) id = id.slice("rad::rad://".length);
      else if (id.startsWith("rad://")) id = id.slice("rad://".length);
      else if (id.startsWith("rad:")) id = id.slice("rad:".length);
      else return { ok: false, error: "Radicle RID must start with rad:z... or rad://z...." };
      if (!id.startsWith("z") || id.length < 24 || id.length > 64 || !BASE58_PATTERN.test(id)) {
        return { ok: false, error: "Radicle RID must be a valid Base58btc z-prefixed identifier (24-64 chars, no 0/O/I/l)." };
      }
      return { ok: true, normalized: `rad:${id}` };
    }

    if (provider === "github") {
      if (raw.startsWith("http://")) return { ok: false, error: "GitHub URLs must use https://, ssh://git@, git@github.com:, or owner/repo." };
      if (raw.startsWith("https://github.com/")) {
        const pair = parseOwnerRepoPath(raw.slice("https://github.com/".length));
        return pair ? { ok: true, normalized: `https://github.com/${pair[0]}/${pair[1]}.git` } : { ok: false, error: "GitHub URL must point to https://github.com/<owner>/<repo>." };
      }
      if (raw.startsWith("ssh://git@github.com/")) {
        const pair = parseOwnerRepoPath(raw.slice("ssh://git@github.com/".length));
        return pair ? { ok: true, normalized: `ssh://git@github.com/${pair[0]}/${pair[1]}.git` } : { ok: false, error: "GitHub SSH URL must point to ssh://git@github.com/<owner>/<repo>." };
      }
      if (raw.startsWith("git@github.com:")) {
        const pair = parseOwnerRepoPath(raw.slice("git@github.com:".length));
        return pair ? { ok: true, normalized: `git@github.com:${pair[0]}/${pair[1]}.git` } : { ok: false, error: "GitHub SSH target must be git@github.com:<owner>/<repo>." };
      }
      if (!raw.includes("://") && !raw.includes(":")) {
        const pair = parseOwnerRepoPath(raw);
        return pair ? { ok: true, normalized: `https://github.com/${pair[0]}/${pair[1]}.git` } : { ok: false, error: "GitHub shorthand must be <owner>/<repo>." };
      }
      return { ok: false, error: "Unrecognized GitHub repository URL or owner/repo." };
    }

    // Gitea / Forgejo
    if (raw.startsWith("http://")) return { ok: false, error: "Gitea/Forgejo URLs must use https://, ssh://git@, or git@host:owner/repo." };
    let hostPort = "";
    let restPath = "";
    if (raw.startsWith("https://")) {
      const after = raw.slice("https://".length);
      const slash = after.indexOf("/");
      if (slash <= 0) return { ok: false, error: "Gitea URL must include https://<host>/<owner>/<repo>." };
      hostPort = after.slice(0, slash);
      restPath = after.slice(slash + 1);
    } else if (raw.startsWith("ssh://git@")) {
      const after = raw.slice("ssh://git@".length);
      const slash = after.indexOf("/");
      if (slash <= 0) return { ok: false, error: "Gitea SSH URL must include ssh://git@<host>/<owner>/<repo>." };
      hostPort = after.slice(0, slash);
      restPath = after.slice(slash + 1);
    } else if (raw.startsWith("git@")) {
      const after = raw.slice("git@".length);
      const colon = after.indexOf(":");
      if (colon <= 0) return { ok: false, error: "Gitea SSH target must be git@<host>:<owner>/<repo>." };
      hostPort = after.slice(0, colon);
      restPath = after.slice(colon + 1);
    } else {
      return { ok: false, error: "Gitea/Forgejo endpoint must use https://<host>/<owner>/<repo> or SSH." };
    }
    const hostOnly = hostPort.startsWith("[") ? hostPort : hostPort.split(":")[0];
    if (isDisallowedHost(hostOnly)) return { ok: false, error: "Gitea host must be a valid non-loopback domain name (IP literals and localhost are rejected)." };
    const pair = parseOwnerRepoPath(restPath);
    if (!pair) return { ok: false, error: "Gitea path must include <owner>/<repo>." };
    return { ok: true, normalized: raw };
  }

  function summarizeRepositories(repositories) {
    const list = Array.isArray(repositories) ? repositories : [];
    const counts = { total: list.length, synced: 0, diverged: 0, dirty: 0, unknown: 0, local_only: 0, total_local_copies: 0, total_cairns: 0 };
    for (const repo of list) {
      const status = typeof repo?.sync_status === "string" ? repo.sync_status : "unknown";
      if (Object.hasOwn(counts, status)) counts[status] += 1;
      else counts.unknown += 1;
      counts.total_local_copies += Array.isArray(repo?.local_copies) ? repo.local_copies.length : 0;
      counts.total_cairns += Number.isSafeInteger(repo?.total_cairns) ? repo.total_cairns : 0;
    }
    return counts;
  }

  function filterRepositories(repositories, { status = "all", query = "" } = {}) {
    const list = Array.isArray(repositories) ? repositories : [];
    const needle = String(query || "").trim().toLowerCase();
    return list.filter((repo) => {
      if (status && status !== "all" && repo?.sync_status !== status) return false;
      if (!needle) return true;
      const fields = [
        repo?.identity, repo?.slug, repo?.label, repo?.sync_status,
        ...(Array.isArray(repo?.remotes) ? repo.remotes.flatMap((r) => [r?.provider, r?.role, r?.native_id, r?.url]) : []),
        ...(Array.isArray(repo?.local_copies) ? repo.local_copies.flatMap((c) => [c?.machine, c?.path, c?.vcs, c?.branch_or_bookmark, c?.head_commit]) : []),
      ];
      return fields.some((val) => String(val ?? "").toLowerCase().includes(needle));
    });
  }

  function formatLocalCopySummary(copy) {
    if (!copy || typeof copy !== "object") return "Unknown copy";
    const machine = copy.machine || "unknown-host";
    if (copy.health === "unavailable") return `${machine} · OFFLINE`;
    const vcs = copy.vcs || "vcs?";
    const branch = copy.branch_or_bookmark || "detached";
    const state = copy.dirty === true ? "dirty" : copy.dirty === false ? "clean" : "unknown";
    const head = copy.head_commit ? shortId(copy.head_commit) : "no-head";
    return `${machine} · ${vcs} · ${branch} · ${state} · ${head}`;
  }

  function syncBadge(syncStatus) {
    const meta = SYNC_META[syncStatus] || SYNC_META.unknown;
    const pill = element("span", undefined, "status-pill");
    pill.dataset.status = meta.status;
    const svg = meta.status === "success" ? icon("circle-check") : meta.status === "failure" ? icon("circle-x") : meta.status === "warning" ? icon("triangle-alert") : icon("circle-alert");
    pill.append(svg, element("span", meta.label, "status-pill-label"));
    pill.title = `Cross-fleet sync status: ${syncStatus || "unknown"}`;
    return pill;
  }

  function matchLocalSourceId(repo, sources) {
    if (typeof repo?.source_id === "string" && repo.source_id) return repo.source_id;
    const list = Array.isArray(sources) ? sources : [];
    const copyPaths = new Set((repo?.local_copies || []).map((c) => c?.path).filter(Boolean));
    for (const s of list) {
      if (!s || typeof s.id !== "string") continue;
      if (s.id === repo?.slug || s.id === repo?.label || (s.path && copyPaths.has(s.path))) return s.id;
    }
    return "";
  }

  let repoGeneration = 0;
  let repoController = null;
  let repoMounted = null;
  let repoFilterState = { status: "all", query: "" };

  function renderHealthBanner(catalog) {
    const fleetSource = catalog?.sources?.fleet;
    const localSource = catalog?.sources?.local_scan;
    const catalogSource = catalog?.sources?.catalog;
    const unavailableHosts = Array.isArray(catalog?.hosts) ? catalog.hosts.filter((h) => h && h.health === "unavailable") : [];
    const warnings = [];
    if (fleetSource && fleetSource.available === false) {
      warnings.push(`Fleet census unavailable${fleetSource.error ? `: ${fleetSource.error}` : ""}. Local and catalog repositories remain visible.`);
    }
    if (localSource && localSource.available === false) {
      warnings.push(`Local checkout scan unavailable${localSource.error ? `: ${localSource.error}` : ""}.`);
    }
    if (catalogSource && catalogSource.available === false) {
      warnings.push(`Catalog store warning${catalogSource.error ? `: ${catalogSource.error}` : ""}.`);
    }
    if (unavailableHosts.length) {
      warnings.push(`Offline / unreachable fleet hosts: ${unavailableHosts.map((h) => `${h.machine}${h.error ? ` (${h.error})` : ""}`).join(", ")}.`);
    }
    if (!warnings.length) return null;
    const banner = element("div", undefined, "warning repo-health-banner");
    banner.id = "repo-health-banner";
    banner.setAttribute("role", "status");
    const head = element("div", undefined, "repo-banner-head");
    head.append(icon("triangle-alert"), element("strong", "Degraded source observation"));
    banner.append(head);
    for (const msg of warnings) banner.append(element("p", msg, "small"));
    return banner;
  }

  function renderImportCard(ctx, repositories, onReload) {
    const card = element("section", undefined, "card repo-import-card");
    card.id = "repo-import-card";
    const head = element("div", undefined, "section-heading");
    head.append(element("h2", "Import repository or attach mirror", "card-title"));
    card.append(head);

    const canOp = typeof ctx.canOperate === "function" ? ctx.canOperate() : window.KilnFactory?.canOperate?.() === true;
    const confirmFn = ctx.confirmControl || window.KilnFactory?.confirmControl;
    const mutateFn = ctx.mutate || window.KilnFactory?.mutate;

    if (!canOp) {
      card.append(element("p", "Read-only principal: importing repositories, attaching mirrors, refreshing census, and removing catalog entries require operator access.", "small repo-readonly-note"));
    }

    const form = element("form", undefined, "repo-import-form");
    form.id = "repo-import-form";

    const grid = element("div", undefined, "repo-import-grid");

    const attachWrap = element("div", undefined, "form-field");
    const attachLabel = element("label", "Target repository");
    attachLabel.htmlFor = "repo-import-attach";
    const attachSelect = element("select");
    attachSelect.id = "repo-import-attach";
    attachSelect.append(Object.assign(element("option", "New repository (import)"), { value: "" }));
    for (const r of repositories) {
      const opt = element("option", `${r.label || r.slug || shortId(r.identity)} (${shortId(r.identity)})`);
      opt.value = r.identity;
      attachSelect.append(opt);
    }
    attachSelect.disabled = !canOp;
    attachWrap.append(attachLabel, attachSelect);

    const provWrap = element("div", undefined, "form-field");
    const provLabel = element("label", "Provider");
    provLabel.htmlFor = "repo-import-provider";
    const provSelect = element("select");
    provSelect.id = "repo-import-provider";
    for (const [val, text] of [["github", "GitHub (owner/repo or URL)"], ["gitea", "Gitea / Forgejo (HTTPS or SSH)"], ["radicle", "Radicle (rad:z...)"]]) {
      provSelect.append(Object.assign(element("option", text), { value: val }));
    }
    provSelect.disabled = !canOp;
    provWrap.append(provLabel, provSelect);

    const roleWrap = element("div", undefined, "form-field");
    const roleLabel = element("label", "Role");
    roleLabel.htmlFor = "repo-import-role";
    const roleSelect = element("select");
    roleSelect.id = "repo-import-role";
    for (const [val, text] of [["canonical", "Canonical"], ["mirror", "Mirror / Projection"]]) {
      roleSelect.append(Object.assign(element("option", text), { value: val }));
    }
    roleSelect.disabled = !canOp;
    roleWrap.append(roleLabel, roleSelect);

    const urlWrap = element("div", undefined, "form-field repo-import-url-field");
    const urlLabel = element("label", "Repository URL / slug / Radicle RID");
    urlLabel.htmlFor = "repo-import-url";
    const urlInput = element("input");
    urlInput.id = "repo-import-url";
    urlInput.type = "text";
    urlInput.maxLength = MAX_REMOTE_URL_BYTES;
    urlInput.autocomplete = "off";
    urlInput.spellcheck = false;
    urlInput.required = true;
    urlInput.disabled = !canOp;
    urlWrap.append(urlLabel, urlInput);

    const labelWrap = element("div", undefined, "form-field");
    const labelText = element("label", "Display label (optional)");
    labelText.htmlFor = "repo-import-label";
    const labelInput = element("input");
    labelInput.id = "repo-import-label";
    labelInput.type = "text";
    labelInput.maxLength = 256;
    labelInput.autocomplete = "off";
    labelInput.placeholder = "Optional display name";
    labelInput.disabled = !canOp;
    labelWrap.append(labelText, labelInput);

    const refWrap = element("div", undefined, "form-field");
    const refLabel = element("label", "Default ref (optional)");
    refLabel.htmlFor = "repo-import-ref";
    const refInput = element("input");
    refInput.id = "repo-import-ref";
    refInput.type = "text";
    refInput.maxLength = 256;
    refInput.autocomplete = "off";
    refInput.placeholder = "main";
    refInput.disabled = !canOp;
    refWrap.append(refLabel, refInput);

    grid.append(attachWrap, provWrap, roleWrap, urlWrap, labelWrap, refWrap);

    const radicleNotice = element("p", undefined, "warning repo-radicle-notice");
    radicleNotice.id = "repo-radicle-notice";
    radicleNotice.append(icon("shield-check"), element("span", RADICLE_NOTICE));
    radicleNotice.hidden = true;

    const formError = element("p", "", "warning repo-import-error");
    formError.id = "repo-import-error";
    formError.setAttribute("role", "status");
    formError.hidden = true;

    const actions = element("div", undefined, "src-actions");
    const submitBtn = element("button", "Import repository…", "primary");
    submitBtn.id = "repo-import-submit";
    submitBtn.type = "submit";
    submitBtn.disabled = !canOp;
    actions.append(submitBtn);

    const syncFormHints = () => {
      const p = provSelect.value;
      urlInput.placeholder = p === "radicle" ? "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE" : p === "gitea" ? "https://gitea.example.com/owner/repo" : "owner/repo or https://github.com/owner/repo";
      radicleNotice.hidden = p !== "radicle";
      const attaching = Boolean(attachSelect.value);
      labelInput.disabled = !canOp || attaching;
      submitBtn.textContent = attaching ? "Attach mirror…" : "Import repository…";
    };
    provSelect.addEventListener("change", syncFormHints);
    attachSelect.addEventListener("change", () => {
      const target = repositories.find((r) => r.identity === attachSelect.value) || null;
      if (target && (target.canonical || target.remotes?.some((rm) => rm.role === "canonical"))) {
        roleSelect.value = "mirror";
      }
      syncFormHints();
    });
    syncFormHints();

    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      if (!canOp || !confirmFn || !mutateFn) return;
      const provider = provSelect.value;
      const role = roleSelect.value;
      const url = urlInput.value.trim();
      const label = labelInput.value.trim();
      const defaultRef = refInput.value.trim();
      const attachId = attachSelect.value;
      const attachRepo = attachId ? repositories.find((r) => r.identity === attachId) || null : null;

      const check = validateRemoteInput({ provider, role, url, attachRepo });
      if (!check.ok) {
        formError.textContent = check.error;
        formError.hidden = false;
        urlInput.focus();
        return;
      }
      formError.textContent = "";
      formError.hidden = true;

      const providerName = PROVIDER_LABELS[provider] || provider;
      const choice = await confirmFn({
        title: attachRepo ? `Attach ${providerName} ${role} to ${attachRepo.label || attachRepo.slug}?` : `Import ${providerName} repository?`,
        verb: attachRepo ? "Attach mirror" : "Import repository",
        dismiss: "Cancel",
        danger: false,
        warning: provider === "radicle" ? RADICLE_NOTICE : "Records this repository endpoint in Kiln's durable catalog. Importing does not run CI workflows, fetch remotes, or enroll Fleet execution.",
        facts: [
          ["Provider", providerName],
          ["Role", role === "canonical" ? "Canonical" : "Mirror / Projection"],
          ["Endpoint", check.normalized],
          ...(attachRepo ? [["Target repository", `${attachRepo.label || attachRepo.slug} (${shortId(attachRepo.identity)})`]] : []),
          ...(defaultRef ? [["Default ref", defaultRef]] : []),
          ...(label && !attachRepo ? [["Label", label]] : []),
        ],
        cliCommand: attachRepo
          ? `kiln repositories add-mirror ${attachRepo.identity} --provider ${provider} --role ${role} ${check.normalized}`
          : `kiln repositories import --provider ${provider} --role ${role} ${check.normalized}`,
        opener: submitBtn,
      });
      if (!choice.confirmed) return;

      submitBtn.disabled = true;
      try {
        const res = attachRepo
          ? await mutateFn(`/api/repositories/${encodeURIComponent(attachRepo.identity)}/mirrors`, {
            provider, role, url: check.normalized,
            ...(defaultRef ? { default_ref: defaultRef } : {}),
            request_id: choice.requestId,
          }, `Attached ${providerName} ${role} to ${attachRepo.label || attachRepo.slug}.`)
          : await mutateFn("/api/repositories/import", {
            provider, role, url: check.normalized,
            ...(label ? { label } : {}),
            ...(defaultRef ? { default_ref: defaultRef } : {}),
            request_id: choice.requestId,
          }, `Imported ${providerName} repository into catalog.`);
        if (res) {
          urlInput.value = "";
          labelInput.value = "";
          refInput.value = "";
          attachSelect.value = "";
          await onReload();
        }
      } finally {
        if (submitBtn.isConnected) submitBtn.disabled = !canOp;
      }
    });

    form.append(grid, radicleNotice, formError, actions);
    card.append(form);
    return card;
  }

  function renderAdmissionBadges(repo) {
    const row = element("div", undefined, "repo-admission-badges");
    const adm = repo?.admission || {};
    if (adm.catalog_imported) row.append(element("span", "Catalog Imported", "chip"));
    if (adm.ci_configured) row.append(element("span", "CI Configured", "chip repo-adm-ci"));
    if (adm.fleet_enrolled) row.append(element("span", "Fleet Enrolled", "chip repo-adm-fleet"));
    if (adm.source_configured) row.append(element("span", "Source Configured", "chip"));
    if (!adm.ci_configured && !adm.fleet_enrolled) row.append(element("span", "Observed", "chip repo-adm-observed"));
    return row;
  }

  function renderRepositoryCard(ctx, repo, sources, onReload) {
    const card = element("article", undefined, "card repo-card");
    card.dataset.repoId = repo.identity || "";
    card.dataset.sync = repo.sync_status || "unknown";

    const canOp = typeof ctx.canOperate === "function" ? ctx.canOperate() : window.KilnFactory?.canOperate?.() === true;
    const confirmFn = ctx.confirmControl || window.KilnFactory?.confirmControl;
    const mutateFn = ctx.mutate || window.KilnFactory?.mutate;
    const openInspector = ctx.openInspector || window.KilnFactory?.openInspector;

    const head = element("div", undefined, "repo-card-head");
    const titleBox = element("div", undefined, "repo-card-title-box");
    const title = element("h3", repo.label || repo.slug || shortId(repo.identity), "repo-card-title");
    const copyStatus = element("span", "", "small repo-copy-status");
    copyStatus.setAttribute("role", "status");
    const idBtn = element("button", undefined, "quiet repo-id-btn");
    idBtn.type = "button";
    idBtn.title = `Canonical Kiln BLAKE3 identity: ${repo.identity} (click to copy)`;
    idBtn.append(element("span", shortId(repo.identity), "mono"), icon("copy"));
    idBtn.addEventListener("click", () => ui().copyText(String(repo.identity || ""), copyStatus, "Canonical ID copied."));
    titleBox.append(title, idBtn, copyStatus);

    const statusBox = element("div", undefined, "repo-card-status");
    statusBox.append(syncBadge(repo.sync_status));
    const cairnCount = Number.isSafeInteger(repo.total_cairns) ? repo.total_cairns : 0;
    statusBox.append(element("span", `${cairnCount} ${cairnCount === 1 ? "Cairn" : "Cairns"}`, "chip repo-cairn-chip"));
    head.append(titleBox, statusBox);

    const admRow = renderAdmissionBadges(repo);

    // Remotes & Mirrors strip
    const remotesWrap = element("div", undefined, "repo-strip");
    remotesWrap.append(element("span", "Remotes & Mirrors", "kiln-label"));
    const remotesList = element("div", undefined, "repo-pill-row");
    const remotes = Array.isArray(repo.remotes) && repo.remotes.length ? repo.remotes : repo.canonical ? [repo.canonical] : [];
    if (!remotes.length) {
      remotesList.append(element("span", "No external remotes declared (local checkout only)", "small"));
    } else {
      for (const ep of remotes) {
        const pill = element("span", undefined, "chip repo-remote-pill");
        pill.dataset.role = ep.role || "mirror";
        const roleLabel = ep.role === "canonical" ? "Canonical" : "Mirror";
        const provLabel = PROVIDER_LABELS[ep.provider] || ep.provider || "Remote";
        pill.append(element("strong", `${provLabel} · ${roleLabel}`), element("span", ep.native_id || ep.url || "", "mono"));
        if (ep.observed_revision) pill.append(element("span", `@${shortId(ep.observed_revision)}`, "mono small"));
        pill.title = [ep.url, ep.default_ref ? `ref: ${ep.default_ref}` : "", ep.observed_revision ? `rev: ${ep.observed_revision}` : "", ep.status ? `status: ${ep.status}` : ""].filter(Boolean).join(" · ");
        remotesList.append(pill);
      }
    }
    remotesWrap.append(remotesList);

    // Fleet Local Copies summary
    const copiesWrap = element("div", undefined, "repo-strip");
    copiesWrap.append(element("span", "Fleet Local Copies", "kiln-label"));
    const copiesList = element("div", undefined, "repo-pill-row");
    const copies = Array.isArray(repo.local_copies) ? repo.local_copies : [];
    if (!copies.length) {
      copiesList.append(element("span", "No fleet local copies observed", "small"));
    } else {
      for (const copy of copies) {
        const pill = element("span", formatLocalCopySummary(copy), "chip repo-copy-pill");
        pill.dataset.health = copy.health || "available";
        if (copy.dirty === true) pill.dataset.dirty = "true";
        pill.title = copy.health === "unavailable"
          ? `${copy.machine}: ${copy.error || "unreachable"}`
          : [copy.machine, copy.path, copy.head_commit ? `HEAD ${copy.head_commit}` : "", Number.isSafeInteger(copy.cairn_count) ? `${copy.cairn_count} cairns` : ""].filter(Boolean).join(" · ");
        copiesList.append(pill);
      }
    }
    copiesWrap.append(copiesList);

    // Actions
    const actions = element("div", undefined, "src-actions repo-row-actions");
    const inspectBtn = element("button", undefined, "primary");
    inspectBtn.type = "button";
    withIcon(inspectBtn, icon("eye"), "Inspect");
    inspectBtn.addEventListener("click", () => {
      if (openInspector) openInspector("repo", repo.identity, [{ kind: "repo", id: repo.identity }], inspectBtn);
      else ctx.navigate({ inspect: `repo:${repo.identity}` });
    });
    actions.append(inspectBtn);

    if (canOp) {
      const addMirrorBtn = element("button", undefined, "quiet");
      addMirrorBtn.type = "button";
      withIcon(addMirrorBtn, icon("square-plus"), "Add Mirror");
      addMirrorBtn.addEventListener("click", () => {
        const attachEl = document.getElementById("repo-import-attach");
        const roleEl = document.getElementById("repo-import-role");
        const urlEl = document.getElementById("repo-import-url");
        const errEl = document.getElementById("repo-import-error");
        if (errEl) { errEl.textContent = ""; errEl.hidden = true; }
        if (attachEl) {
          attachEl.value = repo.identity;
          attachEl.dispatchEvent(typeof Event === "function" ? new Event("change") : { type: "change" });
        }
        if (roleEl && (repo.canonical || remotes.some((r) => r.role === "canonical"))) roleEl.value = "mirror";
        urlEl?.focus?.();
      });
      actions.append(addMirrorBtn);
    }

    const sourceId = matchLocalSourceId(repo, sources);
    if (sourceId) {
      const browse = routeLink(ctx, { page: "source", tab: "source", src: sourceId, rev: "", spath: "", sfile: "", sq: "", sline: "", sview: "" }, undefined, "button quiet", `browse:${repo.identity}`);
      withIcon(browse, icon("folder"), "Browse Source");
      actions.append(browse);
    }

    if (repo.admission?.catalog_imported && canOp && confirmFn && mutateFn) {
      const removeBtn = element("button", "Remove…", "danger quiet");
      removeBtn.type = "button";
      removeBtn.addEventListener("click", async () => {
        const choice = await confirmFn({
          title: `Remove ${repo.label || repo.slug || shortId(repo.identity)} from catalog?`,
          verb: "Remove from catalog",
          dismiss: "Keep in catalog",
          danger: true,
          warning: "Removes this repository registration from Kiln's durable catalog only. Local files and checkouts on disk are NEVER deleted, and no remote repository or Radicle state is modified.",
          facts: [
            ["Repository", repo.label || repo.slug || repo.identity],
            ["Canonical ID", repo.identity],
            ["Disk checkouts", "Untouched (files on disk are never deleted)"],
          ],
          reason: { label: "Removal reason", placeholder: "Why this catalog entry is being removed", required: true },
          cliCommand: `kiln repositories remove ${repo.identity}`,
          opener: removeBtn,
        });
        if (!choice.confirmed) return;
        const res = await mutateFn(`/api/repositories/${encodeURIComponent(repo.identity)}/remove`, {
          reason: choice.reason,
          request_id: choice.requestId,
        }, "Repository removed from catalog (disk checkouts untouched).");
        if (res) await onReload();
      });
      actions.append(removeBtn);
    }

    card.append(head, admRow, remotesWrap, copiesWrap, actions);
    return card;
  }

  function renderRepositoriesPage(ctx, catalog, sources, onReload) {
    const page = element("div", undefined, "src-page repo-hub view-stack");
    const repositories = Array.isArray(catalog?.repositories) ? catalog.repositories : [];
    const summary = catalog?.summary || summarizeRepositories(repositories);
    const canOp = typeof ctx.canOperate === "function" ? ctx.canOperate() : window.KilnFactory?.canOperate?.() === true;
    const mutateFn = ctx.mutate || window.KilnFactory?.mutate;

    // Header + Summary strip + Refresh census
    const header = element("section", undefined, "card repo-hub-header");
    const top = element("div", undefined, "section-heading");
    const titleGroup = element("div");
    titleGroup.append(
      element("h2", "Repositories & Fleet Copies", "card-title"),
      element("p", `${summary.total} repositories · ${summary.total_local_copies} local copies across fleet · ${summary.total_cairns} active Cairns`, "small"),
    );
    const refreshBtn = element("button", undefined, "quiet");
    refreshBtn.id = "repo-refresh-btn";
    refreshBtn.type = "button";
    withIcon(refreshBtn, icon("refresh-cw"), "Refresh census");
    refreshBtn.disabled = !canOp;
    if (!canOp) refreshBtn.title = "Census refresh requires operator access.";
    refreshBtn.addEventListener("click", async () => {
      if (!canOp || !mutateFn) return;
      refreshBtn.disabled = true;
      try {
        const res = await mutateFn("/api/repositories/refresh", {}, "Repository & Fleet census refreshed.");
        if (res) await onReload();
      } finally {
        if (refreshBtn.isConnected) refreshBtn.disabled = !canOp;
      }
    });
    top.append(titleGroup, refreshBtn);

    const kpis = element("div", undefined, "repo-kpis");
    for (const [label, value, tone] of [
      ["Repositories", summary.total, "neutral"],
      ["Synced", summary.synced, "success"],
      ["Diverged", summary.diverged, summary.diverged > 0 ? "failure" : "neutral"],
      ["Dirty", summary.dirty, summary.dirty > 0 ? "warning" : "neutral"],
      ["Offline / Unknown", summary.unknown, summary.unknown > 0 ? "unknown" : "neutral"],
      ["Local Only", summary.local_only, "neutral"],
    ]) {
      const tile = element("div", undefined, "repo-kpi-tile");
      tile.dataset.tone = tone;
      tile.append(element("span", label, "kiln-label"), element("strong", String(value ?? 0), "repo-kpi-value"));
      kpis.append(tile);
    }

    const boundary = element("details", undefined, "about-view");
    const boundaryBody = element("div", undefined, "about-view-body");
    boundaryBody.append(
      element("p", "Catalog imports record provider-neutral BLAKE3 repository identities and remote declarations. Importing never executes CI workflows, enrolls Fleet workers, or mutates Radicle state."),
      element("p", RADICLE_NOTICE),
    );
    boundary.append(ui().disclosureSummary("About repository observation & Radicle discipline", "info"), boundaryBody);

    header.append(top, kpis, boundary);
    page.append(header);

    const healthBanner = renderHealthBanner(catalog);
    if (healthBanner) page.append(healthBanner);

    page.append(renderImportCard(ctx, repositories, onReload));

    // Filter toolbar & Matrix
    const matrixPanel = element("section", undefined, "card repo-matrix-panel view-stack");
    const filterBar = element("div", undefined, "repo-filter-bar");
    const chips = element("div", undefined, "repo-filter-chips");
    chips.setAttribute("role", "group");
    chips.setAttribute("aria-label", "Filter repositories by cross-fleet status");

    const searchWrap = element("div", undefined, "repo-filter-search");
    const searchLabel = element("label", "Search repositories");
    searchLabel.htmlFor = "repo-filter-search";
    const searchInput = element("input");
    searchInput.id = "repo-filter-search";
    searchInput.type = "search";
    searchInput.maxLength = FILTER_MAX_CHARS;
    searchInput.autocomplete = "off";
    searchInput.placeholder = "Slug, BLAKE3 ID, remote, or machine…";
    searchInput.value = repoFilterState.query;
    searchWrap.append(searchLabel, searchInput);

    const statusLine = element("p", "", "small repo-matrix-status");
    statusLine.setAttribute("role", "status");
    const listWrap = element("div", undefined, "repo-matrix view-stack");
    listWrap.id = "repo-matrix";

    const renderList = () => {
      for (const chipBtn of chips.querySelectorAll("button")) {
        chipBtn.setAttribute("aria-pressed", String(chipBtn.dataset.filterStatus === repoFilterState.status));
      }
      const matched = filterRepositories(repositories, repoFilterState);
      statusLine.textContent = `Showing ${matched.length} of ${repositories.length} repositories`;
      listWrap.replaceChildren();
      if (!repositories.length) {
        listWrap.append(emptyState("No repositories observed or imported", "Import a GitHub, Gitea/Forgejo, or Radicle repository above, or configure local checkouts / Fleet scan roots."));
        return;
      }
      if (!matched.length) {
        listWrap.append(emptyState("No matching repositories", "Clear the status filter or search query to see all catalog repositories."));
        return;
      }
      for (const repo of matched) listWrap.append(renderRepositoryCard(ctx, repo, sources, onReload));
    };

    for (const [statusKey, label] of [
      ["all", `All (${repositories.length})`],
      ["diverged", `Diverged (${summary.diverged ?? 0})`],
      ["dirty", `Dirty (${summary.dirty ?? 0})`],
      ["unknown", `Offline/Unknown (${summary.unknown ?? 0})`],
      ["local_only", `Local Only (${summary.local_only ?? 0})`],
    ]) {
      const btn = element("button", label, "chip repo-filter-chip");
      btn.type = "button";
      btn.dataset.filterStatus = statusKey;
      btn.setAttribute("aria-pressed", String(repoFilterState.status === statusKey));
      btn.addEventListener("click", () => { repoFilterState.status = statusKey; renderList(); });
      chips.append(btn);
    }

    searchInput.addEventListener("input", () => { repoFilterState.query = searchInput.value; renderList(); });
    filterBar.append(chips, searchWrap);
    matrixPanel.append(filterBar, statusLine, listWrap);
    renderList();
    page.append(matrixPanel);

    return page;
  }

  async function loadRepositories(container, ctx) {
    const current = ++repoGeneration;
    repoController?.abort();
    const controller = typeof AbortController === "function" ? new AbortController() : null;
    repoController = controller;
    if (!container) return;
    if (repoMounted !== container) { repoMounted?.replaceChildren(); }
    repoMounted = container;
    const live = () => current === repoGeneration && container.isConnected;
    if (!container.firstChild) container.replaceChildren(loadingState("Reading repository catalog and fleet local copies…"));

    const reload = () => loadRepositories(container, ctx);
    let catalog;
    let sources = [];
    try {
      const [repoResult, sourcesResult] = await Promise.allSettled([
        ctx.api("/api/repositories", controller ? { controller } : undefined),
        ctx.api("/api/sources", controller ? { quiet: true, controller } : { quiet: true }),
      ]);
      if (!live()) return;
      if (repoResult.status === "rejected") {
        if (isAborted(repoResult.reason)) return;
        container.replaceChildren(errorState(`Repository catalog unavailable: ${explain(repoResult.reason)}`));
        return;
      }
      catalog = repoResult.value;
      if (sourcesResult.status === "fulfilled" && Array.isArray(sourcesResult.value?.sources)) {
        sources = sourcesResult.value.sources;
      }
    } catch (error) {
      if (!live() || isAborted(error)) return;
      container.replaceChildren(errorState(`Repository catalog unavailable: ${explain(error)}`));
      return;
    }
    if (!live()) return;
    container.replaceChildren(renderRepositoriesPage(ctx, catalog, sources, reload));
    if (ctx?.route?.repo_focus === "import") {
      document.getElementById("repo-import-url")?.focus?.();
    }
    document.title = "Kiln · Repositories & Fleet Copies";
  }

  function clearRepositories() {
    repoGeneration += 1;
    repoController?.abort();
    repoController = null;
    repoFilterState = { status: "all", query: "" };
    repoMounted?.replaceChildren();
    repoMounted = null;
  }

  window.KilnSource = Object.freeze({
    load, clear,
    loadRepositories, clearRepositories,
    sortEntries, parseLineSelection, splitLines, parseDiff,
    validateRemoteInput, summarizeRepositories, filterRepositories, formatLocalCopySummary,
    RADICLE_NOTICE,
  });
})();
