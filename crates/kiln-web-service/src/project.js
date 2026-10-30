"use strict";

// Project history page (?page=project&project=<id>): health and current streak, a recent-runs
// timeline, a duration trend over timed runs only, last success/failure and a bounded runs table.
// Reads only /api/projects and bounded /api/runs pages through the shell's api helper. Data is
// written through textContent or attributes, never parsed as markup. The shell owns the page
// header; this module fills only #page-primary-action (marked with data-project-action).
// r[impl kiln.web.project_page]
// r[impl kiln.web.project_page.scenario.untimed]
(() => {
  // Each /api/runs request asks for this many runs (the service admits 1..=100).
  const RUN_PAGE_LIMIT = 50;
  // At most this many pages are read, so the page covers the newest RUN_PAGE_LIMIT × MAX_RUN_PAGES runs.
  const MAX_RUN_PAGES = 2;
  const TIMELINE_LIMIT = 40;
  const TABLE_LIMIT = 20;
  const SHORT_REVISION_LENGTH = 12;
  const SHORT_RUN_LENGTH = 16;
  // Chart geometry in viewBox units; the SVG stretches horizontally to its container.
  const CHART_WIDTH = 600;
  const CHART_HEIGHT = 160;
  // Bars keep one width until more runs than this are plotted, so a short history is not stretched.
  const CHART_MIN_SLOTS = 16;
  const CHART_BAR_FILL = 0.7;
  const CHART_AXIS_INTERVALS = 4;
  const SECOND_MS = 1000;
  const MINUTE_MS = 60 * SECOND_MS;
  const HOUR_MS = 60 * MINUTE_MS;
  const DAY_MS = 24 * HOUR_MS;
  // Axis interval candidates; the first whose CHART_AXIS_INTERVALS multiple covers the slowest run wins.
  const AXIS_STEPS_MS = Object.freeze([100, 200, 500, SECOND_MS, 2 * SECOND_MS, 5 * SECOND_MS, 10 * SECOND_MS, 15 * SECOND_MS, 30 * SECOND_MS, MINUTE_MS, 2 * MINUTE_MS, 5 * MINUTE_MS, 10 * MINUTE_MS, 15 * MINUTE_MS, 30 * MINUTE_MS, HOUR_MS, 2 * HOUR_MS, 3 * HOUR_MS, 6 * HOUR_MS, 12 * HOUR_MS, DAY_MS]);
  const SVG_NS = "http://www.w3.org/2000/svg";
  const HEADER_SLOT = "page-primary-action";
  const NOT_RECORDED = "Not recorded";

  let generation = 0;
  // The container last given to load(); clear() empties it even while a first read is pending.
  let mounted = null;
  // The project id whose page is currently drawn in `mounted`, or null while loading/empty/unavailable.
  let shownProject = null;

  const ui = () => window.KilnUi;
  function element(tag, content, className) {
    const node = document.createElement(tag);
    if (content !== undefined && content !== null) node.textContent = content;
    if (className) node.className = className;
    return node;
  }
  function svg(tag, attributes) {
    const node = document.createElementNS(SVG_NS, tag);
    for (const [key, value] of Object.entries(attributes || {})) node.setAttribute(key, String(value));
    return node;
  }
  function plural(count, word) { return `${count} ${word}${count === 1 ? "" : "s"}`; }
  function safeMs(value) { return Number.isSafeInteger(value) ? value : null; }
  function validDuration(value) { return Number.isSafeInteger(value) && value >= 0 ? value : null; }
  function shorten(value, length) { const source = String(value ?? ""); return source.length > length ? `${source.slice(0, length)}…` : source; }
  function isAborted(error) { return error?.name === "AbortError"; }
  function explain(error) {
    if (error?.name === "TimeoutError") return "The request timed out; its result is not known.";
    if (error instanceof TypeError) return "The service could not be reached. Its current state is unavailable.";
    return error?.message || "The service response was unavailable.";
  }

  // ---------- Pure derivations over retained run views (newest first) ----------
  function statusOf(run) { return ui().outcomeStatus(run.outcome, run.phase).status; }
  function isFinished(run) { return typeof run.outcome === "string" && run.outcome !== "" && run.outcome !== "Pending"; }
  function runTime(run) { return safeMs(run.completed_at_ms) ?? safeMs(run.started_at_ms) ?? safeMs(run.created_at_ms); }
  function timePhrase(run) {
    const finished = safeMs(run.completed_at_ms);
    if (finished !== null) return `finished ${ui().formatRelative(finished)}`;
    const started = safeMs(run.started_at_ms);
    if (started !== null) return `started ${ui().formatRelative(started)}`;
    const created = safeMs(run.created_at_ms);
    if (created !== null) return `requested ${ui().formatRelative(created)}`;
    return "time not recorded";
  }
  // Consecutive finished runs sharing the newest finished run's status; in-progress runs are skipped.
  function currentStreak(runs) {
    const finished = runs.filter(isFinished);
    if (!finished.length) return null;
    const status = statusOf(finished[0]);
    let count = 0;
    while (count < finished.length && statusOf(finished[count]) === status) count += 1;
    return { status, count, latest: finished[0], first: finished[count - 1], reachesWindow: count === finished.length };
  }
  function streakHeadline(streak, atLeast) {
    const runs = `${atLeast ? "at least " : ""}${plural(streak.count, "run")}`;
    if (streak.status === "success") return `Passing for ${runs}`;
    if (streak.status === "failure") return `Failing for ${runs}`;
    if (streak.status === "cancelled") return `Cancelled for ${runs}`;
    return `Outcome unknown for ${runs}`;
  }
  function lastWith(runs, status) { return runs.find((run) => isFinished(run) && statusOf(run) === status) || null; }
  function axisMaximum(slowest) {
    for (const step of AXIS_STEPS_MS) if (step * CHART_AXIS_INTERVALS >= slowest) return step * CHART_AXIS_INTERVALS;
    return Math.ceil(slowest / (DAY_MS * CHART_AXIS_INTERVALS)) * DAY_MS * CHART_AXIS_INTERVALS;
  }
  function median(values) {
    const sorted = [...values].sort((a, b) => a - b);
    const middle = Math.floor(sorted.length / 2);
    return sorted.length % 2 ? sorted[middle] : Math.round((sorted[middle - 1] + sorted[middle]) / 2);
  }

  // ---------- Links ----------
  // Hrefs merge into the current URL exactly like the shell's navigate(patch); plain clicks use navigate.
  function routeHref(patch) {
    const params = new URLSearchParams(location.search);
    for (const [key, value] of Object.entries(patch)) { if (value) params.set(key, value); else params.delete(key); }
    return `?${params}`;
  }
  function routeLink(ctx, patch, label, className) {
    const link = element("a", label, className);
    link.href = routeHref(patch);
    link.addEventListener("click", (event) => {
      if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
      event.preventDefault();
      ctx.navigate(patch);
    });
    return link;
  }
  // node and stream stay empty so the run view applies its failure-first defaults to the exact run.
  function runPatch(projectId, run, view) {
    return { page: "runs", project: projectId, run: run.id, view, stream: "", node: "", q: "", outcome: "", source: "", cursor: "" };
  }
  // Pipeline and Logs open the exact execution named by the run id; nothing is substituted.
  function runLinks(ctx, projectId, run, subject) {
    const logs = routeLink(ctx, runPatch(projectId, run, "logs"), "Logs");
    logs.setAttribute("aria-label", `Logs for ${subject}`);
    logs.title = `Open the retained logs of exact run ${run.id}`;
    logs.dataset.projectFocus = `logs:${subject}:${run.id}`;
    const pipeline = routeLink(ctx, runPatch(projectId, run, "pipeline"), "Pipeline");
    pipeline.setAttribute("aria-label", `Pipeline for ${subject}`);
    pipeline.title = `Open the pipeline of exact run ${run.id}`;
    pipeline.dataset.projectFocus = `pipeline:${subject}:${run.id}`;
    return { logs, pipeline };
  }

  // ---------- Shared states ----------
  function loadingState(label) {
    const node = element("div", undefined, "loading-state");
    node.setAttribute("aria-busy", "true");
    for (const width of ["skeleton-wide", "skeleton-mid", "skeleton-short"]) node.append(element("span", undefined, `skeleton ${width}`));
    node.append(element("p", label, "small"));
    return node;
  }
  function emptyState(title, message, action) {
    const node = element("div", undefined, "empty-state empty-state-inline");
    node.append(element("p", title, "empty-state-title"), element("p", message, "empty-state-text"));
    if (action) node.append(action);
    return node;
  }
  function errorState(message) {
    const node = element("p", message, "warning error-state");
    node.dataset.state = "unavailable";
    return node;
  }
  function card(title, className) {
    const section = element("section", undefined, `card ${className}`);
    const heading = element("div", undefined, "section-heading");
    const h2 = element("h2", title, "card-title");
    heading.append(h2);
    section.append(heading);
    return { section, heading };
  }

  // ---------- Header slot ----------
  function releaseHeader() {
    const slot = document.getElementById(HEADER_SLOT);
    if (slot?.firstElementChild?.dataset.projectAction !== undefined) slot.replaceChildren();
  }
  function headerAction(ctx, project) {
    const slot = document.getElementById(HEADER_SLOT);
    if (!slot) return;
    if (!project || !ctx.canOperate || typeof ctx.startRun !== "function") { releaseHeader(); return; }
    if (slot.firstElementChild?.dataset.projectAction === project.id) return;
    // r[impl kiln.web.design_system] It opens the start form preselected (more input before dispatch), so it ends in an ellipsis.
    const button = element("button", "Run latest…", "primary");
    button.type = "button";
    button.dataset.projectAction = project.id;
    button.title = `Open the start form for the latest admitted ${project.reference || "reference"}; nothing is dispatched until you submit it.`;
    button.addEventListener("click", () => ctx.startRun(project.id));
    slot.replaceChildren(button);
  }
  function setTitle(label) {
    for (const id of ["workspace-title", "app-breadcrumb"]) { const node = document.getElementById(id); if (node) node.textContent = label; }
    document.title = `Kiln · ${label}`;
  }

  // ---------- Reads ----------
  async function readProject(ctx, id) {
    const result = await ctx.api("/api/projects");
    if (!result || !Array.isArray(result.enrolled) || !Array.isArray(result.available)) throw new Error("The project response is invalid.");
    const enrolled = result.enrolled.find((row) => row?.id === id);
    if (enrolled) return { project: enrolled, enrolled: true };
    return { project: result.available.find((row) => row?.id === id) || null, enrolled: false };
  }
  async function readRuns(ctx, id, current) {
    const runs = [];
    const seen = new Set();
    let cursor = null;
    let total = null;
    let pages = 0;
    do {
      const query = new URLSearchParams({ project_id: id, limit: String(RUN_PAGE_LIMIT), order: "desc" });
      if (cursor) query.set("cursor", cursor);
      const page = await ctx.api(`/api/runs?${query}`);
      if (!current()) throw new DOMException("Project page changed", "AbortError");
      if (!page || !Array.isArray(page.runs)) throw new Error("The run history response is invalid.");
      if (total === null && Number.isSafeInteger(page.total)) total = page.total;
      for (const run of page.runs) {
        if (!run || typeof run.id !== "string" || run.project_id !== id || seen.has(run.id)) continue;
        seen.add(run.id);
        runs.push(run);
      }
      cursor = typeof page.next_cursor === "string" && page.next_cursor ? page.next_cursor : null;
      pages += 1;
    } while (cursor && pages < MAX_RUN_PAGES);
    return { runs, total: total ?? runs.length, truncated: Boolean(cursor) };
  }

  // ---------- Sections ----------
  function lastRunTile(ctx, projectId, run, kind, bounded) {
    const tile = element("div", undefined, "pp-last");
    tile.dataset.kind = kind;
    const label = kind === "success" ? "Last success" : "Last failure";
    tile.append(element("dt", label, "pp-last-label"));
    const value = element("dd", undefined, "pp-last-value");
    if (!run) {
      const what = kind === "success" ? "passing" : "failed";
      value.append(element("span", bounded ? `No ${what} run among the loaded runs` : `No ${what} run retained`, "not-recorded"));
      tile.append(value);
      return tile;
    }
    const head = element("span", undefined, "pp-last-head");
    head.append(ui().statusPill(run.outcome, run.phase), ui().timeElement(runTime(run)));
    const meta = element("span", undefined, "pp-last-meta");
    const revision = element("span", shorten(run.source_revision, SHORT_REVISION_LENGTH), "mono");
    revision.title = `Exact source revision: ${run.source_revision ?? NOT_RECORDED}`;
    meta.append(ui().durationElement(validDuration(run.duration_ms)), document.createTextNode(" · "), revision);
    const actions = element("span", undefined, "pp-actions");
    const { logs, pipeline } = runLinks(ctx, projectId, run, label.toLowerCase());
    if (kind === "failure") logs.prepend(ui().icon("triangle-alert"));
    actions.append(logs, pipeline);
    value.append(head, meta, actions);
    tile.append(value);
    return tile;
  }
  function timeline(ctx, projectId, runs) {
    const wrap = element("div", undefined, "pp-timeline");
    const shown = runs.slice(0, TIMELINE_LIMIT).reverse();
    const head = element("div", undefined, "pp-timeline-head");
    const title = element("h3", "Recent runs", "pp-subtitle");
    title.id = "project-timeline-title";
    head.append(title, element("span", `${plural(shown.length, "run")} · oldest left, newest right`, "meta"));
    const list = element("ol", undefined, "pp-timeline-list");
    list.setAttribute("aria-labelledby", title.id);
    for (const run of shown) {
      const { status, label } = ui().outcomeStatus(run.outcome, run.phase);
      const item = element("li");
      const link = routeLink(ctx, runPatch(projectId, run, "pipeline"), undefined, "pp-timeline-run");
      link.dataset.status = status;
      link.dataset.projectFocus = `timeline:${run.id}`;
      link.append(ui().statusIcon(status));
      const time = runTime(run);
      const exact = time === null ? "time not recorded" : new Date(time).toISOString();
      link.setAttribute("aria-label", `${label} run, ${timePhrase(run)}, ${shorten(run.id, SHORT_RUN_LENGTH)}`);
      link.title = `${label} · ${exact}\n${run.id}`;
      item.append(link);
      list.append(item);
    }
    wrap.append(head, list);
    return wrap;
  }
  function healthCard(ctx, project, history) {
    const { runs, total, truncated } = history;
    const section = element("section", undefined, "card pp-health");
    section.setAttribute("aria-labelledby", "project-health-title");
    const main = element("div", undefined, "pp-health-main");
    const head = element("div", undefined, "pp-health-head");
    const latest = runs[0];
    head.append(ui().statusPill(latest.outcome, latest.phase));
    const active = runs.filter((run) => !isFinished(run)).length;
    if (active) head.append(element("span", `${active} in progress`, "chip"));
    const streak = currentStreak(runs);
    section.dataset.health = streak?.status || "unknown";
    const headline = element("h2", streak ? streakHeadline(streak, streak.reachesWindow && truncated) : "No finished runs yet", "pp-headline");
    headline.id = "project-health-title";
    const since = element("p", undefined, "pp-since");
    if (streak) {
      const start = runTime(streak.first);
      if (start === null) since.append(document.createTextNode("Streak start time not recorded"));
      else { since.append(document.createTextNode("Since ")); since.append(ui().timeElement(start)); }
      const latestTime = runTime(streak.latest);
      since.append(document.createTextNode(" · latest finished "));
      since.append(latestTime === null ? element("span", NOT_RECORDED, "not-recorded") : ui().timeElement(latestTime));
    } else since.textContent = "Every loaded run is still in progress.";
    const identity = element("p", undefined, "pp-identity meta");
    const reference = element("span", project?.reference || "Reference unavailable", "mono");
    const repository = element("span", project?.repository || "", "mono");
    identity.append(reference);
    if (project?.repository) identity.append(document.createTextNode(" · "), repository);
    main.append(head, headline, since, identity);
    const facts = element("dl", undefined, "pp-lasts");
    const bounded = truncated || total > runs.length;
    facts.append(lastRunTile(ctx, project?.id || latest.project_id, lastWith(runs, "success"), "success", bounded), lastRunTile(ctx, project?.id || latest.project_id, lastWith(runs, "failure"), "failure", bounded));
    const scope = element("p", bounded ? `Derived from the newest ${plural(runs.length, "retained run")} of ${total} (at most ${MAX_RUN_PAGES} pages of ${RUN_PAGE_LIMIT} are read).` : runs.length === 1 ? "Derived from the only retained run." : `Derived from all ${runs.length} retained runs.`, "pp-scope small");
    section.append(main, facts, timeline(ctx, project?.id || latest.project_id, runs), scope);
    return section;
  }
  function trendCard(runs) {
    const { section, heading } = card("Duration trend", "pp-trend");
    const { points, excluded } = ui().durationPoints(runs);
    const note = element("p", undefined, "meta pp-trend-note");
    note.dataset.excluded = String(excluded);
    note.textContent = `${plural(points.length, "timed run")} plotted · ${plural(excluded, "run")} without recorded timing excluded`;
    heading.append(note);
    if (!points.length) {
      section.append(emptyState("No timed runs yet", excluded ? `${plural(excluded, "finished run")} have no recorded start and finish, so no duration can be plotted. Unknown timing is never treated as zero.` : "No finished run has recorded timing yet."));
      return section;
    }
    const durations = points.map((point) => point.duration_ms);
    const slowest = Math.max(...durations);
    const maximum = axisMaximum(Math.max(1, slowest));
    const figure = element("figure", undefined, "pp-chart");
    const axis = element("div", undefined, "pp-axis");
    axis.setAttribute("aria-hidden", "true");
    const chart = svg("svg", { viewBox: `0 0 ${CHART_WIDTH} ${CHART_HEIGHT}`, preserveAspectRatio: "none", class: "pp-chart-svg", role: "img", focusable: "false" });
    const caption = element("figcaption", undefined, "pp-chart-caption small");
    const latest = points[points.length - 1];
    const summary = `Latest ${ui().formatDuration(latest.duration_ms)} · median ${ui().formatDuration(median(durations))} · fastest ${ui().formatDuration(Math.min(...durations))} · slowest ${ui().formatDuration(slowest)}`;
    caption.id = "project-trend-caption";
    caption.textContent = summary;
    chart.setAttribute("aria-label", `Duration of ${plural(points.length, "timed run")}, oldest to newest, axis 0 to ${ui().formatDuration(maximum)}. ${summary}.`);
    for (let index = 0; index <= CHART_AXIS_INTERVALS; index += 1) {
      const value = (maximum / CHART_AXIS_INTERVALS) * index;
      const y = CHART_HEIGHT - (value / maximum) * CHART_HEIGHT;
      chart.append(svg("line", { x1: 0, x2: CHART_WIDTH, y1: y, y2: y, class: index === 0 ? "pp-baseline" : "pp-gridline", "vector-effect": "non-scaling-stroke" }));
      const label = element("span", index === 0 ? "0" : ui().formatDuration(value), "pp-axis-label");
      label.style.top = `${100 - (index / CHART_AXIS_INTERVALS) * 100}%`;
      axis.append(label);
    }
    const slots = Math.max(points.length, CHART_MIN_SLOTS);
    const slot = CHART_WIDTH / slots;
    const width = slot * CHART_BAR_FILL;
    points.forEach((point, index) => {
      const height = Math.max(1, (point.duration_ms / maximum) * CHART_HEIGHT);
      const x = CHART_WIDTH - (points.length - index) * slot + (slot - width) / 2;
      const bar = svg("rect", { x, y: CHART_HEIGHT - height, width, height, class: "pp-bar", "data-status": ui().outcomeStatus(point.outcome).status });
      const title = svg("title");
      title.textContent = `${ui().outcomeStatus(point.outcome).label} · ${ui().formatDuration(point.duration_ms)} · ${point.at_ms === null ? "time not recorded" : new Date(point.at_ms).toISOString()} · ${point.id}`;
      bar.append(title);
      chart.append(bar);
    });
    const plot = element("div", undefined, "pp-plot");
    plot.append(chart);
    const range = element("div", undefined, "pp-range meta");
    const oldest = element("span");
    oldest.append(document.createTextNode("Oldest plotted "), ui().timeElement(points[0].at_ms));
    const newest = element("span");
    newest.append(document.createTextNode("Newest "), ui().timeElement(latest.at_ms));
    range.append(oldest, newest);
    figure.append(axis, plot, range, caption);
    section.append(figure);
    return section;
  }
  function runsTable(ctx, projectId, runs, total) {
    const { section, heading } = card("Run history", "pp-runs");
    const all = routeLink(ctx, { page: "runs", project: projectId, run: "", node: "", view: "", stream: "", q: "", outcome: "", source: "", cursor: "" }, "All runs", "pp-all-runs");
    heading.append(all);
    const shown = runs.slice(0, TABLE_LIMIT);
    const wrap = element("div", undefined, "data-table-wrap");
    const table = element("table", undefined, "data-table data-table-stack pp-runs-table");
    const caption = element("caption", `Newest ${plural(shown.length, "run")} of ${total} retained`, "sr-only");
    const headings = element("tr");
    for (const [label, className] of [["Status"], ["Revision"], ["Duration", "num"], ["Started"], ["Inspect", "actions-heading"]]) {
      const th = element("th", label, className);
      th.scope = "col";
      headings.append(th);
    }
    const head = element("thead");
    head.append(headings);
    const body = element("tbody");
    for (const run of shown) {
      const row = element("tr");
      row.dataset.status = statusOf(run);
      const cell = (label, content, className) => { const td = element("td", undefined, className); td.dataset.label = label; td.append(content); row.append(td); };
      cell("Status", ui().statusPill(run.outcome, run.phase));
      const revision = element("span", shorten(run.source_revision, SHORT_REVISION_LENGTH), "mono");
      revision.title = `Exact source revision: ${run.source_revision ?? NOT_RECORDED}\nRun: ${run.id}`;
      cell("Revision", revision);
      cell("Duration", ui().durationElement(validDuration(run.duration_ms)), "num");
      cell("Started", ui().timeElement(safeMs(run.started_at_ms)), "pp-started");
      const actions = element("div", undefined, "project-row-actions");
      const { logs, pipeline } = runLinks(ctx, projectId, run, `run ${shorten(run.id, SHORT_RUN_LENGTH)}`);
      actions.append(logs, pipeline);
      cell("Inspect", actions);
      body.append(row);
    }
    table.append(caption, head, body);
    wrap.append(table);
    section.append(wrap);
    return section;
  }

  // ---------- Rendering ----------
  function focusKey() { return document.activeElement?.closest?.(".project-page") ? document.activeElement.dataset?.projectFocus || "" : ""; }
  function restoreFocus(container, key) {
    if (!key) return;
    for (const node of container.querySelectorAll("[data-project-focus]")) if (node.dataset.projectFocus === key) { node.focus({ preventScroll: true }); return; }
  }
  function renderPage(container, ctx, id, inventory, history) {
    const page = element("div", undefined, "project-page view-stack");
    page.dataset.project = id;
    const project = inventory.ok ? inventory.value.project : null;
    const enrolled = inventory.ok && inventory.value.enrolled;
    const label = project?.label || id;
    if (!inventory.ok) page.append(errorState(`Project inventory unavailable: ${explain(inventory.error)} The run history below is read by exact project id; starting a run is hidden until enrollment can be confirmed.`));
    else if (!enrolled && project) page.append(errorState(`${label} is allowlisted but not enrolled. Retained runs stay readable; enroll it from Projects before starting a run.`));
    else if (!project && history.ok && history.value.runs.length) page.append(errorState(`“${id}” is not in the current project inventory. Its retained runs stay readable; starting a run is unavailable.`));
    if (!history.ok) {
      page.append(errorState(`Run history unavailable: ${explain(history.error)} No other project's history is substituted.`));
    } else if (!history.value.runs.length) {
      if (inventory.ok && !project) {
        page.replaceChildren(emptyState("Unknown project", `No enrolled or allowlisted project has the id “${id}”, and no retained runs reference it. Nothing was substituted.`, routeLink(ctx, { page: "projects", project: "" }, "Browse projects")));
        headerAction(ctx, null);
        return page;
      }
      page.append(emptyState("No retained runs for this project yet", enrolled && ctx.canOperate ? "Use Run latest… to start the first source-bound run." : "Runs appear here once this project has executed."));
    } else {
      page.append(healthCard(ctx, project ? { ...project, id } : { id, reference: "", repository: "" }, history.value), trendCard(history.value.runs), runsTable(ctx, id, history.value.runs, history.value.total));
    }
    headerAction(ctx, enrolled ? project : null);
    setTitle(label);
    return page;
  }
  async function load(container, ctx) {
    const current = ++generation;
    if (!container) return;
    if (mounted !== container) { mounted?.replaceChildren(); shownProject = null; }
    mounted = container;
    const id = typeof ctx?.project === "string" ? ctx.project : "";
    const live = () => current === generation && container.isConnected;
    if (!id) {
      shownProject = null;
      releaseHeader();
      container.replaceChildren(emptyState("No project selected", "Choose a project to see its retained run history.", routeLink(ctx, { page: "projects", project: "" }, "Browse projects")));
      return;
    }
    // Same project: keep the current page while refreshing so live updates do not flash or steal focus.
    if (shownProject !== id) {
      shownProject = null;
      releaseHeader();
      container.replaceChildren(loadingState("Reading this project's retained runs…"));
    }
    const [inventory, history] = await Promise.all([
      readProject(ctx, id).then((value) => ({ ok: true, value }), (error) => ({ ok: false, error })),
      readRuns(ctx, id, live).then((value) => ({ ok: true, value }), (error) => ({ ok: false, error })),
    ]);
    if (!live()) return;
    if ((!inventory.ok && isAborted(inventory.error)) || (!history.ok && isAborted(history.error))) return;
    if (!inventory.ok && !history.ok) {
      shownProject = null;
      releaseHeader();
      container.replaceChildren(errorState(`Project page unavailable: ${explain(history.error)} Nothing is inferred and no sample data is shown.`));
      return;
    }
    const focused = focusKey();
    container.replaceChildren(renderPage(container, ctx, id, inventory, history));
    shownProject = id;
    restoreFocus(container, focused);
    for (const list of container.querySelectorAll(".pp-timeline-list")) list.scrollLeft = list.scrollWidth;
  }
  function clear() {
    generation += 1;
    mounted?.replaceChildren();
    mounted = null;
    shownProject = null;
    releaseHeader();
  }

  window.KilnProject = Object.freeze({ load, clear });
})();
