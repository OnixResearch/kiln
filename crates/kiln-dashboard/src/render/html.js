(() => {
  "use strict";

  const byId = (id) => document.getElementById(id);
  const search = byId("run-search");
  const outcome = byId("outcome-filter");
  const sort = byId("run-sort");
  const direction = byId("sort-direction");
  const body = byId("runs-table").tBodies[0];
  const rows = Array.from(body.querySelectorAll("[data-run-row]"));
  const runPanels = Array.from(document.querySelectorAll("[data-run-detail]"));
  const attemptPanels = Array.from(document.querySelectorAll("[data-attempt-detail]"));
  const runs = new Map(runPanels.map((panel) => [panel.dataset.runId, panel]));
  const attempts = new Map(attemptPanels.map((panel) => [panel.dataset.attemptId, panel]));
  const runButtons = Array.from(document.querySelectorAll("[data-select-run]"));
  const attemptButtons = Array.from(document.querySelectorAll("[data-select-attempt]"));
  const initialRun = runPanels.find((panel) => panel.hasAttribute("data-initial"));
  const initialAttempt = attemptPanels.find((panel) => panel.hasAttribute("data-initial"));
  const defaults = {
    q: "", outcome: "", sort: "id", direction: "asc",
    run: initialRun ? initialRun.dataset.runId : "",
    attempt: initialAttempt ? initialAttempt.dataset.attemptId : "",
    theme: "system"
  };
  let state;
  let presentation;
  let sizeObserver;
  const minimumReportHeight = 320;
  const maximumReportHeight = 20000;
  const reportHeightPadding = 8;

  const compare = (left, right) => left < right ? -1 : left > right ? 1 : 0;
  const admittedValue = (control, value, fallback) =>
    Array.from(control.options).some((option) => option.value === value) ? value : fallback;

  function restore() {
    const fragment = location.hash.slice(1);
    const params = new URLSearchParams(fragment);
    state = fragment ? {
      q: params.get("q") || "",
      outcome: admittedValue(outcome, params.get("outcome"), ""),
      sort: admittedValue(sort, params.get("sort"), "id"),
      direction: admittedValue(direction, params.get("direction"), "asc"),
      run: params.get("run") || "",
      attempt: params.get("attempt") || "",
      theme: ["system", "light", "dark"].includes(params.get("theme")) ? params.get("theme") : "system"
    } : { ...defaults };
    render();
    if (window.parent !== window && location.hash) {
      window.parent.postMessage({ type: "kiln-dashboard-state", fragment: location.hash }, "*");
    }
  }

  function render() {
    document.documentElement.dataset.theme = presentation ? presentation.theme : state.theme;
    byId("theme-toggle").textContent = `Theme: ${state.theme}`;
    search.value = state.q;
    outcome.value = state.outcome;
    sort.value = state.sort;
    direction.value = state.direction;
    const query = state.q.trim().toLowerCase();
    const ordered = rows.slice().sort((left, right) => {
      const a = left.dataset;
      const b = right.dataset;
      const value = state.sort === "attempts"
        ? compare(Number(a.attemptCount), Number(b.attemptCount))
        : compare(state.sort === "outcome" ? a.outcome : a.runId,
                  state.sort === "outcome" ? b.outcome : b.runId);
      return (state.direction === "desc" ? -value : value) || compare(a.runId, b.runId);
    });
    let visible = 0;
    for (const [index, row] of ordered.entries()) {
      const data = row.dataset;
      row.hidden = !(data.runId.toLowerCase().includes(query) || data.activeAttempt.toLowerCase().includes(query))
        || (state.outcome !== "" && state.outcome !== data.outcome);
      if (!row.hidden) visible += 1;
      if (body.children[index] !== row) body.insertBefore(row, body.children[index]);
      row.classList.toggle("selected-row", data.runId === state.run);
    }
    byId("run-result-count").textContent = `${visible} of ${rows.length} admitted runs shown. Profile bounds and truncation still apply.`;
    byId("run-empty").hidden = visible !== 0;

    const run = runs.get(state.run);
    const attempt = attempts.get(state.attempt);
    const mismatch = attempt && attempt.dataset.found === "true" && attempt.dataset.runId !== state.run;
    for (const panel of runPanels) panel.hidden = panel !== run;
    for (const panel of attemptPanels) panel.hidden = panel !== attempt || Boolean(mismatch);
    for (const button of runButtons) button.setAttribute("aria-pressed", String(button.dataset.runId === state.run));
    for (const button of attemptButtons) {
      button.setAttribute("aria-pressed", String(button.dataset.attemptId === state.attempt && button.dataset.runId === state.run));
    }

    let runStatus = "No run selected. Choose a run from the admitted rows.";
    if (state.run) {
      if (!run || run.dataset.found !== "true") {
        runStatus = `Selected run ${state.run} is missing from this admitted report. It may be absent or outside the current profile bounds; no other run was selected.`;
      } else {
        const row = rows.find((item) => item.dataset.runId === state.run);
        runStatus = `Selected run: ${state.run}.`;
        if (row && row.hidden) runStatus += " Its row is hidden by the current filters; selected details remain visible.";
        if (!row) runStatus += " This selection is not in the admitted run list; its admitted details remain visible.";
      }
    }
    byId("selection-status").textContent = runStatus;

    let attemptStatus = "No attempt selected.";
    if (state.attempt) {
      if (!attempt || attempt.dataset.found !== "true") {
        attemptStatus = `Selected attempt ${state.attempt} is missing from this admitted report. It may be absent or outside the current profile bounds; no other attempt was selected.`;
      } else if (mismatch) {
        attemptStatus = `Selected attempt ${state.attempt} belongs to run ${attempt.dataset.runId}, not selected run ${state.run || "(none)"}. Its detail is not paired with this run. Select its attempt button to select the owning run.`;
      } else {
        attemptStatus = `Selected attempt: ${state.attempt}. Owning run: ${attempt.dataset.runId}.`;
      }
    }
    byId("attempt-selection-status").textContent = attemptStatus;
  }

  function commit() {
    const params = new URLSearchParams();
    for (const key of ["q", "outcome", "sort", "direction", "run", "attempt", "theme"]) params.set(key, state[key]);
    const fragment = `#${params.toString()}`;
    if (location.hash !== fragment) {
      try {
        history.pushState(null, "", fragment);
      } catch (_) {
        // Some local-file browser policies restrict History API writes.
        location.hash = fragment;
      }
    }
    render();
    if (window.parent !== window) {
      window.parent.postMessage({ type: "kiln-dashboard-state", fragment: location.hash }, "*");
    }
  }

  search.addEventListener("input", () => { state.q = search.value; commit(); });
  outcome.addEventListener("change", () => { state.outcome = outcome.value; commit(); });
  sort.addEventListener("change", () => { state.sort = sort.value; commit(); });
  direction.addEventListener("change", () => { state.direction = direction.value; commit(); });
  byId("clear-filters").addEventListener("click", () => {
    state.q = "";
    state.outcome = "";
    commit();
  });
  for (const button of runButtons) {
    button.addEventListener("click", () => {
      state.run = button.dataset.runId;
      const panel = runs.get(state.run);
      const row = rows.find((item) => item.dataset.runId === state.run);
      state.attempt = panel && panel.dataset.found === "true"
        ? panel.dataset.activeAttempt : row ? row.dataset.activeAttempt : "";
      commit();
    });
  }
  for (const button of attemptButtons) {
    button.addEventListener("click", () => {
      state.run = button.dataset.runId;
      state.attempt = button.dataset.attemptId;
      commit();
    });
  }
  window.addEventListener("hashchange", restore);
  // r[impl kiln.observe.dashboard.html.console.scenario.keyboard]
  const help = byId("keyboard-help");
  const showHelp = () => { if (!help.open) help.showModal(); };
  byId("shortcut-help").addEventListener("click", showHelp);
  byId("close-shortcuts").addEventListener("click", () => help.close());
  byId("theme-toggle").addEventListener("click", () => {
    const themes = ["system", "light", "dark"];
    state.theme = themes[(themes.indexOf(state.theme) + 1) % themes.length];
    commit();
  });
  function jumpTo(id) {
    const target = byId(id);
    target.tabIndex = -1;
    target.focus({ preventScroll: true });
    target.scrollIntoView({ block: "start" });
  }
  for (const button of document.querySelectorAll("[data-jump]")) {
    button.addEventListener("click", () => jumpTo(button.dataset.jump));
  }
  document.querySelector(".skip-link").addEventListener("click", (event) => {
    event.preventDefault();
    jumpTo("main-content");
  });
  document.addEventListener("keydown", (event) => {
    if (event.defaultPrevented || event.ctrlKey || event.altKey || event.metaKey || event.isComposing || help.open) return;
    if (event.target.closest("input, textarea, select, [contenteditable]:not([contenteditable=false])")) {
      if (event.target === search && event.key === "Escape") {
        event.preventDefault();
        search.value = "";
        state.q = "";
        commit();
      }
      return;
    }
    if (event.key === "/") {
      event.preventDefault();
      search.focus();
      search.select();
    } else if (event.key === "?") {
      event.preventDefault();
      showHelp();
    }
  });
  window.addEventListener("popstate", restore);
  // r[impl kiln.dashboard.product.isolation]
  // r[impl kiln.dashboard.product.isolation.scenario.route]
  // Opaque-origin children accept inert state/presentation only from their parent.
  function reportSize() {
    if (!presentation) return;
    const contentHeight = Math.ceil(document.querySelector(".app-shell").getBoundingClientRect().height) + reportHeightPadding;
    const height = Math.min(maximumReportHeight, Math.max(minimumReportHeight, contentHeight));
    window.parent.postMessage({ type: "kiln-dashboard-resize", height }, "*");
  }
  window.addEventListener("message", (event) => {
    if (window.parent === window || event.source !== window.parent || !event.data
      || typeof event.data !== "object" || Array.isArray(event.data)) return;
    const message = event.data;
    if (message.type === "kiln-dashboard-restore") {
      if (typeof message.fragment !== "string" || (message.fragment && !message.fragment.startsWith("#"))) return;
      if (location.hash !== message.fragment) location.hash = message.fragment;
      restore();
    } else if (message.type === "kiln-dashboard-present") {
      if (!["overview", "runs", "activity", "evidence"].includes(message.page)
        || !["system", "light", "dark"].includes(message.theme)
        || !["comfortable", "compact"].includes(message.density)) return;
      presentation = { page: message.page, theme: message.theme, density: message.density };
      document.documentElement.dataset.embedded = "true";
      document.documentElement.dataset.page = presentation.page;
      document.documentElement.dataset.density = presentation.density;
      render();
      if (!sizeObserver && typeof ResizeObserver === "function") {
        sizeObserver = new ResizeObserver(reportSize);
        sizeObserver.observe(document.querySelector(".app-shell"));
      }
    }
    requestAnimationFrame(reportSize);
  });
  window.addEventListener("resize", reportSize);
  restore();
  document.documentElement.classList.add("js-enabled");
  if (window.parent !== window) {
    const params = new URLSearchParams();
    for (const key of ["q", "outcome", "sort", "direction", "run", "attempt", "theme"]) params.set(key, state[key]);
    window.parent.postMessage({ type: "kiln-dashboard-ready", fragment: `#${params}` }, "*");
  }
  const refresh = document.querySelector('meta[name="kiln-refresh-seconds"]');
  if (refresh) {
    window.setTimeout(() => location.reload(), Number(refresh.content) * 1000);
  }
})();
