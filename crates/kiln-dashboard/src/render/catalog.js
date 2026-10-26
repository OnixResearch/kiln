(() => {
  "use strict";
  const sections = Array.from(document.querySelectorAll("[data-project][data-history]"));
  const project = document.getElementById("catalog-project");
  const history = document.getElementById("catalog-history");
  const status = document.getElementById("catalog-status");
  const key = (section) => `${section.dataset.project}/${section.dataset.history}`;
  const projects = new Map(sections.map((section) => [section.dataset.project, section.dataset.projectLabel]));
  let state;
  let active;
  let views;
  function option(control, value, label) {
    const item = document.createElement("option");
    item.value = value;
    item.textContent = label;
    control.appendChild(item);
  }
  function save() {
    const params = new URLSearchParams();
    params.set("project", state.project);
    params.set("history", state.history);
    params.set("views", JSON.stringify(views));
    const fragment = `#${params}`;
    if (location.hash !== fragment) {
      try { window.history.replaceState(null, "", fragment); }
      catch (_) { location.hash = fragment; }
    }
  }
  function send(section) {
    const frame = section.querySelector("iframe");
    const fragment = views[key(section)];
    if (typeof fragment === "string") {
      frame.contentWindow.postMessage({ type: "kiln-dashboard-restore", fragment }, "*");
    }
  }
  function render() {
    active = sections.find((section) => section.dataset.project === state.project && section.dataset.history === state.history);
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
    for (const section of sections) section.hidden = section !== active;
    status.textContent = active
      ? `Selected repository ${active.dataset.projectLabel}; history ${active.dataset.historyLabel}. Independently admitted observation.`
      : `Selected repository/history ${state.project || "(none)"}/${state.history || "(none)"} is missing. No replacement history was selected. Choose an available history to continue.`;
    if (active) send(active);
  }
  function restore() {
    const params = new URLSearchParams(location.hash.slice(1));
    const anchor = sections.find((section) => `#${section.id}` === location.hash);
    state = !location.hash || anchor ? {
      project: (anchor || sections[0]).dataset.project,
      history: (anchor || sections[0]).dataset.history
    } : { project: params.get("project") || "", history: params.get("history") || "" };
    views = Object.create(null);
    try {
      const parsed = JSON.parse(params.get("views") || "{}");
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        for (const [id, fragment] of Object.entries(parsed)) {
          if (typeof fragment === "string") views[id] = fragment;
        }
      }
    } catch (_) { /* Invalid saved view state is not executable. */ }
    render();
    save();
  }
  project.addEventListener("change", () => {
    state.project = project.value;
    state.history = "";
    render();
    save();
  });
  history.addEventListener("change", () => {
    state.history = history.value;
    render();
    save();
  });
  for (const section of sections) {
    section.querySelector("iframe").addEventListener("load", () => send(section));
  }
  document.querySelector("nav ul").addEventListener("click", (event) => {
    const link = event.target.closest("a");
    if (!link) return;
    const section = sections.find((item) => `#${item.id}` === link.getAttribute("href"));
    if (!section) return;
    event.preventDefault();
    state = { project: section.dataset.project, history: section.dataset.history };
    render();
    save();
    section.scrollIntoView({ block: "start" });
  });
  window.addEventListener("message", (event) => {
    const section = sections.find((item) => item.querySelector("iframe").contentWindow === event.source);
    if (!section || !event.data || typeof event.data !== "object") return;
    if (event.data.type === "kiln-dashboard-ready") {
      if (!Object.hasOwn(views, key(section)) && typeof event.data.fragment === "string") {
        views[key(section)] = event.data.fragment;
        save();
      }
      send(section);
    } else if (event.data.type === "kiln-dashboard-state" && typeof event.data.fragment === "string") {
      // Initial child defaults must not replace a fragment awaiting restoration.
      views[key(section)] = event.data.fragment;
      save();
    }
  });
  window.addEventListener("hashchange", restore);
  window.addEventListener("popstate", restore);
  restore();
  document.documentElement.classList.add("catalog-js");
})();
