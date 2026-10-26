(() => {
  "use strict";
  // r[impl kiln.dashboard.product.navigation]
  // r[impl kiln.dashboard.product.navigation.scenario.restore]
  // r[impl kiln.dashboard.product.behavior]
  const byId = (id) => document.getElementById(id);
  const sections = Array.from(document.querySelectorAll("#catalog-reports > section[data-project][data-history]"));
  const frames = new Map(sections.map((section) => [section, section.querySelector("iframe")]));
  const project = byId("catalog-project");
  const history = byId("catalog-history");
  const status = byId("catalog-status");
  const index = byId("rats-index");
  const indexStatus = byId("rats-index-status");
  const navigation = byId("catalog-navigation");
  const search = byId("catalog-search");
  const results = byId("catalog-search-results");
  const resultList = byId("catalog-search-list");
  const routes = {
    overview: ["Overview", "An offline overview of independently admitted CI observations."],
    projects: ["Projects", "Browse declared repositories and choose an exact admitted history."],
    runs: ["Runs", "Inspect admitted runs and attempts without changing CI state."],
    activity: ["Activity", "Read the selected history’s admitted event activity."],
    evidence: ["Evidence", "Inspect bounded evidence, identity and admission details."],
    settings: ["Settings", "Local presentation preferences, saved in this URL."]
  };
  const themes = ["system", "light", "dark"];
  const densities = ["comfortable", "compact"];
  const reportPages = new Set(["overview", "runs", "activity", "evidence"]);
  const minimumFrameHeight = 320;
  const maximumFrameHeight = 20000;
  const indexReadyTimeout = 10000;
  const key = (section) => `${section.dataset.project}/${section.dataset.history}`;
  const projects = new Map(sections.map((section) => [section.dataset.project, section.dataset.projectLabel]));
  const entries = sections.map((section) => ({
    project: section.dataset.project,
    history: section.dataset.history,
    project_label: section.dataset.projectLabel,
    history_label: section.dataset.historyLabel,
    source: section.dataset.source ?? null,
    revision: section.dataset.revision ?? null
  }));
  const metadata = new Map(sections.map((section) => [section,
    [section.dataset.project, section.dataset.history, section.dataset.projectLabel,
      section.dataset.historyLabel, section.dataset.source, section.dataset.revision]
      .filter(Boolean).join(" ").toLocaleLowerCase()
  ]));
  const cards = Array.from(document.querySelectorAll("[data-project-card][data-project-id]"));
  const awaitingRestore = new Map();
  let state;
  let active;
  let views = Object.create(null);
  let indexApi;
  let indexMounted = false;
  let renderedProject;
  let renderedHistory;
  let presentedPage;

  function text(id, value) {
    const element = byId(id);
    if (element) element.textContent = value;
  }

  function option(control, value, label) {
    const item = document.createElement("option");
    item.value = value;
    item.textContent = label;
    control.appendChild(item);
  }

  function save(mode = "replace") {
    const params = new URLSearchParams();
    for (const name of ["page", "theme", "density", "q", "project", "history"]) params.set(name, state[name]);
    params.set("views", JSON.stringify(views));
    const fragment = `#${params}`;
    if (location.hash === fragment) return;
    try { window.history[mode === "push" ? "pushState" : "replaceState"](null, "", fragment); }
    catch (_) {
      // Local-file policies may reject History API writes; the inert hash still works.
      if (mode === "push") location.hash = fragment;
      else location.replace(fragment);
    }
  }

  function reportVisible() {
    return !!active && reportPages.has(state.page);
  }

  function send(section, restoreFragment = true) {
    const frame = frames.get(section);
    if (!frame || !frame.contentWindow) return;
    const fragment = views[key(section)];
    if (restoreFragment && typeof fragment === "string" && (!fragment || fragment.startsWith("#"))) {
      awaitingRestore.set(section, fragment);
      frame.contentWindow.postMessage({ type: "kiln-dashboard-restore", fragment }, "*");
    }
    // Restoration always precedes presentation; presentation never changes canonical child state.
    if (section === active && reportVisible()) {
      frame.contentWindow.postMessage({
        type: "kiln-dashboard-present", page: state.page, theme: state.theme, density: state.density
      }, "*");
    }
  }

  // r[impl kiln.dashboard.product.behavior.scenario.fallback]
  function indexFailure(error) {
    indexApi = undefined;
    index.hidden = true;
    indexStatus.hidden = false;
    indexStatus.textContent = `Repository/history index unavailable: ${error instanceof Error ? error.message : String(error)}. Accessible HTML navigation and native history controls remain available.`;
    navigation.open = true;
  }

  function resizeIndex() {
    if (state.page === "projects" && indexApi) {
      // Ratzilla may have mounted while its route was hidden; measure after layout.
      requestAnimationFrame(() => window.dispatchEvent(new Event("resize")));
    }
  }

  function mountIndex() {
    if (indexMounted || !state || !window.kilnRatsIndex) return;
    indexMounted = true;
    try {
      index.hidden = false;
      window.kilnRatsIndex.mount_index(JSON.stringify(entries), state.project, state.history);
      indexApi = window.kilnRatsIndex;
      indexStatus.hidden = true;
      navigation.open = false;
      resizeIndex();
    } catch (error) { indexFailure(error); }
  }

  function syncSelection() {
    if (renderedProject === state.project && renderedHistory === state.history) return;
    project.replaceChildren();
    for (const [id, label] of projects) option(project, id, label);
    if (!projects.has(state.project)) option(project, state.project, `Missing repository: ${state.project || "(none)"}`);
    project.value = state.project;
    history.replaceChildren();
    const histories = sections.filter((section) => section.dataset.project === state.project);
    if (!histories.some((section) => section.dataset.history === state.history)) {
      option(history, state.history, state.history ? `Missing history: ${state.history}` : "Choose a history");
    }
    for (const section of histories) option(history, section.dataset.history, section.dataset.historyLabel);
    history.value = state.history;
    renderedProject = state.project;
    renderedHistory = state.history;
    if (indexApi) {
      try { indexApi.select_history(state.project, state.history); }
      catch (error) { indexFailure(error); }
    }
  }

  function closeSearch() {
    if (results) results.hidden = true;
    if (search) search.setAttribute("aria-expanded", "false");
  }

  function renderSearch(open = false) {
    const query = state.q.trim().toLocaleLowerCase();
    const matches = sections.filter((section) => !query || metadata.get(section).includes(query));
    const matchedProjects = new Set(matches.map((section) => section.dataset.project));
    for (const card of cards) card.hidden = !matchedProjects.has(card.dataset.projectId);
    text("project-result-count", `${matchedProjects.size} of ${projects.size} projects${query ? " match the metadata search" : ""}.`);
    const empty = byId("project-empty");
    if (empty) empty.hidden = matchedProjects.size !== 0;
    if (!resultList) return;
    resultList.replaceChildren();
    text("catalog-search-count", query
      ? `${matches.length} matching ${matches.length === 1 ? "history" : "histories"}.`
      : "Search project, history, source or revision metadata. This is not a run search.");
    if (query && !matches.length) {
      const item = document.createElement("li");
      item.textContent = "No matching histories. Try a project, history, source or revision.";
      resultList.appendChild(item);
    } else if (query) {
      for (const section of matches) {
        const item = document.createElement("li");
        const button = document.createElement("button");
        button.type = "button";
        button.dataset.openHistory = "";
        button.dataset.project = section.dataset.project;
        button.dataset.history = section.dataset.history;
        button.textContent = `${section.dataset.projectLabel} / ${section.dataset.historyLabel} — ${section.dataset.source || "Source not declared"}${section.dataset.revision ? ` · ${section.dataset.revision}` : ""}`;
        item.appendChild(button);
        resultList.appendChild(item);
      }
    }
    if (open && query && results) {
      results.hidden = false;
      search.setAttribute("aria-expanded", "true");
    } else closeSearch();
  }

  function closeMenu() {
    const toggle = byId("app-menu-toggle");
    if (toggle) toggle.setAttribute("aria-expanded", "false");
    const sidebar = byId("app-sidebar");
    if (sidebar) sidebar.classList.remove("is-open");
    document.documentElement.classList.remove("app-menu-open");
  }

  function render() {
    const previous = active;
    active = sections.find((section) => section.dataset.project === state.project && section.dataset.history === state.history);
    document.documentElement.dataset.theme = state.theme;
    document.documentElement.dataset.density = state.density;
    text("app-title", routes[state.page][0]);
    text("app-subtitle", routes[state.page][1]);
    text("app-breadcrumb", routes[state.page][0]);
    for (const link of document.querySelectorAll("[data-app-route]")) {
      const selected = link.dataset.appRoute === state.page;
      link.classList.toggle("is-active", selected);
      if (selected) link.setAttribute("aria-current", "page");
      else link.removeAttribute("aria-current");
    }
    for (const panel of document.querySelectorAll("[data-app-panel]")) panel.hidden = panel.dataset.appPanel !== state.page;
    const visible = reportVisible();
    byId("catalog-context").hidden = !reportPages.has(state.page);
    navigation.hidden = state.page !== "projects";
    const reports = byId("catalog-reports");
    if (reports) reports.hidden = !visible;
    for (const section of sections) {
      section.hidden = section !== active || !visible;
      if (section !== active || !visible || previous !== active || presentedPage !== state.page) {
        frames.get(section)?.style.removeProperty("height");
      }
    }
    syncSelection();
    status.textContent = active
      ? `Selected repository ${active.dataset.projectLabel}; history ${active.dataset.historyLabel}. Independently admitted observation.`
      : `Selected repository/history ${state.project || "(none)"}/${state.history || "(none)"} is missing. No replacement history was selected. Choose an available history to continue.`;
    text("selected-project", active ? active.dataset.projectLabel : state.project || "No project selected");
    text("selected-history", active ? active.dataset.historyLabel : state.history || "No history selected");
    text("selected-source", active ? active.dataset.source || "Not declared" : "Unavailable");
    text("selected-revision", active ? active.dataset.revision || "Not declared" : "Unavailable");
    text("selected-observation", active
      ? `State revision ${active.dataset.stateRevision} · ${active.dataset.recordCount} journal records · selected history only`
      : "No admitted report selected.");
    if (search && search.value !== state.q) search.value = state.q;
    const theme = byId("app-theme");
    const density = byId("app-density");
    if (theme) theme.value = state.theme;
    if (density) density.value = state.density;
    const themeToggle = byId("app-theme-toggle");
    if (themeToggle) {
      themeToggle.setAttribute("aria-label", `Theme: ${state.theme}. Change appearance`);
      themeToggle.title = `Theme: ${state.theme}. Change appearance`;
    }
    renderSearch();
    if (active) send(active);
    presentedPage = state.page;
    resizeIndex();
  }

  function commit(change) {
    Object.assign(state, change);
    save("push");
    render();
  }

  function openHistory(projectId, historyId) {
    if (!sections.some((section) => section.dataset.project === projectId && section.dataset.history === historyId)) return;
    closeSearch();
    closeMenu();
    commit({ project: projectId, history: historyId, page: "runs" });
  }

  function restore() {
    const params = new URLSearchParams(location.hash.slice(1));
    const anchor = sections.find((section) => `#${section.id}` === location.hash);
    const initial = !location.hash || anchor;
    const selected = anchor || (initial ? sections[0] : undefined);
    state = {
      page: anchor ? "runs" : Object.hasOwn(routes, params.get("page")) ? params.get("page") : "overview",
      theme: themes.includes(params.get("theme")) ? params.get("theme") : "system",
      density: densities.includes(params.get("density")) ? params.get("density") : "comfortable",
      q: params.get("q") || "",
      project: selected ? selected.dataset.project : params.get("project") || "",
      history: selected ? selected.dataset.history : params.get("history") || ""
    };
    views = Object.create(null);
    try {
      const parsed = JSON.parse(params.get("views") || "{}");
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        for (const [id, fragment] of Object.entries(parsed)) {
          if (typeof fragment === "string") views[id] = fragment;
        }
      }
    } catch (_) { /* Malformed URL state is inert, never executable. */ }
    awaitingRestore.clear();
    closeSearch();
    closeMenu();
    render();
    save();
    mountIndex();
  }

  project.addEventListener("change", () => commit({ project: project.value, history: "" }));
  history.addEventListener("change", () => commit({ history: history.value }));
  for (const section of sections) {
    frames.get(section)?.addEventListener("load", () => send(section));
  }
  if (search) {
    search.setAttribute("aria-controls", "catalog-search-results");
    search.setAttribute("aria-expanded", "false");
    search.addEventListener("input", () => {
      state.q = search.value;
      save("push");
      renderSearch(true);
    });
    search.addEventListener("focus", () => renderSearch(true));
    search.addEventListener("keydown", (event) => {
      if (event.key === "ArrowDown" && results && !results.hidden) {
        const button = resultList.querySelector("button");
        if (button) { event.preventDefault(); button.focus(); }
      }
    });
  }
  resultList?.addEventListener("keydown", (event) => {
    const buttons = Array.from(resultList.querySelectorAll("button"));
    const current = buttons.indexOf(document.activeElement);
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const offset = event.key === "ArrowDown" ? 1 : -1;
      buttons[(current + offset + buttons.length) % buttons.length]?.focus();
    }
  });

  function showHelp() {
    const dialog = byId("app-keyboard-help");
    if (!dialog || dialog.open) return;
    if (typeof dialog.showModal === "function") dialog.showModal();
    else dialog.setAttribute("open", "");
  }

  function closeHelp() {
    const dialog = byId("app-keyboard-help");
    if (!dialog || !dialog.open) return;
    if (typeof dialog.close === "function") dialog.close();
    else dialog.removeAttribute("open");
  }

  document.addEventListener("click", (event) => {
    if (!(event.target instanceof Element)) return;
    const quick = event.target.closest("[data-open-history]");
    if (quick) {
      event.preventDefault();
      openHistory(quick.dataset.project, quick.dataset.history);
      return;
    }
    const route = event.target.closest("[data-app-route]");
    if (route && Object.hasOwn(routes, route.dataset.appRoute)) {
      event.preventDefault();
      closeMenu();
      closeSearch();
      commit({ page: route.dataset.appRoute });
      return;
    }
    const link = event.target.closest("#catalog-navigation a");
    const section = link && sections.find((item) => `#${item.id}` === link.getAttribute("href"));
    if (section) {
      event.preventDefault();
      openHistory(section.dataset.project, section.dataset.history);
      return;
    }
    if (event.target !== search && !results?.contains(event.target)) closeSearch();
    const toggle = byId("app-menu-toggle");
    const sidebar = byId("app-sidebar");
    if (event.target !== toggle && !toggle?.contains(event.target) && !sidebar?.contains(event.target)) closeMenu();
  });
  byId("app-theme")?.addEventListener("change", (event) => {
    if (themes.includes(event.target.value)) commit({ theme: event.target.value });
  });
  byId("app-density")?.addEventListener("change", (event) => {
    if (densities.includes(event.target.value)) commit({ density: event.target.value });
  });
  byId("app-theme-toggle")?.addEventListener("click", () => commit({ theme: themes[(themes.indexOf(state.theme) + 1) % themes.length] }));
  byId("app-reset-preferences")?.addEventListener("click", () => commit({ theme: "system", density: "comfortable" }));
  byId("app-shortcuts")?.addEventListener("click", showHelp);
  byId("app-close-shortcuts")?.addEventListener("click", closeHelp);
  byId("app-refresh")?.addEventListener("click", () => { save(); location.reload(); });
  byId("app-menu-toggle")?.addEventListener("click", (event) => {
    const toggle = event.currentTarget;
    const open = toggle.getAttribute("aria-expanded") !== "true";
    toggle.setAttribute("aria-expanded", String(open));
    byId("app-sidebar")?.classList.toggle("is-open", open);
    document.documentElement.classList.toggle("app-menu-open", open);
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      const inResults = results?.contains(document.activeElement);
      closeSearch();
      closeMenu();
      closeHelp();
      if (inResults) { search.focus(); closeSearch(); }
      return;
    }
    const editing = event.target instanceof Element && event.target.closest("input, select, textarea, [contenteditable], dialog[open]");
    if (editing || event.ctrlKey || event.metaKey || event.altKey) return;
    if (event.key === "/" && search) { event.preventDefault(); search.focus(); }
    else if (event.key === "?") { event.preventDefault(); showHelp(); }
  });

  // r[impl kiln.dashboard.product.isolation.scenario.route]
  window.addEventListener("message", (event) => {
    const section = sections.find((item) => frames.get(item)?.contentWindow === event.source);
    if (!section || !event.data || typeof event.data !== "object" || Array.isArray(event.data)) return;
    const message = event.data;
    if (message.type === "kiln-dashboard-resize") {
      if (section === active && reportVisible() && !section.hidden && typeof message.height === "number"
          && Number.isFinite(message.height) && message.height >= minimumFrameHeight && message.height <= maximumFrameHeight) {
        frames.get(section).style.height = `${Math.ceil(message.height)}px`;
      }
      return;
    }
    const validFragment = typeof message.fragment === "string" && (!message.fragment || message.fragment.startsWith("#"));
    if (message.type === "kiln-dashboard-ready") {
      if (!Object.hasOwn(views, key(section)) && validFragment) {
        views[key(section)] = message.fragment;
        save();
      }
      send(section);
    } else if (message.type === "kiln-dashboard-state" && validFragment) {
      // Ignore boot defaults queued before a saved fragment is acknowledged by its own child.
      if (awaitingRestore.has(section) && awaitingRestore.get(section) !== message.fragment) return;
      awaitingRestore.delete(section);
      views[key(section)] = message.fragment;
      save();
    }
  });
  document.addEventListener("kiln-rats-select", (event) => {
    const detail = event.detail;
    if (!detail || typeof detail !== "object" || Array.isArray(detail)
        || typeof detail.project !== "string" || typeof detail.history !== "string") return;
    openHistory(detail.project, detail.history);
  });
  document.addEventListener("kiln-rats-ready", mountIndex);
  window.addEventListener("hashchange", restore);
  window.addEventListener("popstate", restore);
  indexStatus.hidden = false;
  indexStatus.textContent = "Loading repository/history index. Accessible HTML navigation and native history controls remain available.";
  restore();
  document.documentElement.classList.add("catalog-js");
  // A failed/absent WASM capability must not leave a permanent loading-only status.
  window.setTimeout(() => {
    if (!indexApi && !indexMounted) indexFailure("WebAssembly index did not become ready");
  }, indexReadyTimeout);
})();
