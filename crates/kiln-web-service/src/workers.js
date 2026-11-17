// Workers is a read-only view of configured execution targets, not a machine census.
// The shell owns the session, API admission, routing, refresh and run/project navigation.
(() => {
  "use strict";
  const QUERY_MAX_CHARS = 256;
  const FILTER_COMMIT_MS = 350;
  const PROJECT_NAME_PREVIEW = 2;
  const statuses = Object.freeze({
    slots_available: Object.freeze({ label: "Slots available", tone: "", meaning: "At least one enrolled project slot is free. This does not establish worker health or guarantee dispatch." }),
    busy: Object.freeze({ label: "Busy", tone: "info", meaning: "The recorded target has no free slot, or its latest heartbeat reports its slots occupied." }),
    unknown: Object.freeze({ label: "Unknown execution", tone: "warning", meaning: "An uncertain execution still occupies its project slot. Its outcome is not inferred." }),
    not_enrolled: Object.freeze({ label: "Not enrolled", tone: "", meaning: "No assigned project is enrolled. Configured slots are not yet admitted capacity." }),
    offline: Object.freeze({ label: "No current heartbeat", tone: "warning", meaning: "No current admitted heartbeat is recorded for this target. Previous observations do not establish worker availability." }),
  });
  const ui = () => window.KilnUi;
  const record = (value) => value !== null && typeof value === "object" && !Array.isArray(value);
  const text = (value) => typeof value === "string" ? value : "";
  const count = (value) => Number.isSafeInteger(value) && value >= 0 ? value : null;
  const instant = count;
  const countText = (value) => count(value) === null ? "Not recorded" : value.toLocaleString("en-US");
  const plural = (value, word) => `${countText(value)} ${word}${value === 1 ? "" : "s"}`;
  const compare = (a, b) => a < b ? -1 : a > b ? 1 : 0;
  const CAMPAIGN_ID_MAX_CHARS = 256;
  function campaignId(value) {
    const id = text(value);
    return id && id.trim() && id.length <= CAMPAIGN_ID_MAX_CHARS && !/[\u0000-\u001f\u007f]/.test(id) ? id : "";
  }

  // The Overview uses the same existing inventory admission and slot vocabulary as this page.
  function validate(result) {
    if (result?.schema !== "kiln.web-workers.v1" || result.bounded !== true || typeof result.storage_failed !== "boolean" || !result.summary || !Array.isArray(result.workers) || result.workers.some((worker) => !worker?.capacity || !Array.isArray(worker.projects) || worker.projects.some((project) => !Array.isArray(project?.active_run_ids)))) throw new Error("Worker inventory response is invalid. Target capacity is unavailable.");
  }
  function slotState(value) {
    if (Object.hasOwn(statuses, value)) return statuses[value];
    return { label: text(value) ? `Unrecognized slot state: ${value}` : "Slot state not recorded", tone: "warning", meaning: "No slot availability or execution outcome is inferred from this value." };
  }
  function capacity(value) {
    return Object.fromEntries(["configured_slots", "enrolled_slots", "active", "free_slots", "queued"].map((key) => [key, count(value?.[key])]));
  }
  // r[impl kiln.dashboard.workers.view.scenario.heartbeat]
  function heartbeat(worker) {
    const state = text(worker?.liveness) || "unknown";
    const observed = record(worker?.observed_capacity) ? worker.observed_capacity : null;
    const atMs = instant(observed?.last_seen_at_ms);
    const hasReport = observed !== null && atMs !== null;
    const known = ["online", "offline", "unknown"].includes(state);
    let label = "Not observed";
    if (!known) label = `Unrecognized: ${state}`;
    else if (hasReport) label = state === "online" ? "Recent heartbeat" : state === "offline" ? "Not current" : "Liveness unknown";
    return {
      state,
      label,
      tone: state === "offline" || !known ? "warning" : "",
      atMs,
      observed: hasReport,
      slots: hasReport ? count(observed.observed_slots) : null,
      active: hasReport ? count(observed.active_slots) : null,
      reportedFree: hasReport ? count(observed.free_slots) : null,
      currentFree: state === "online" && hasReport ? count(observed.free_slots) : null,
    };
  }
  function execution(value) {
    if (!record(value)) return null;
    const started = instant(value.started_at_ms);
    const finished = instant(value.completed_at_ms);
    return {
      run: value,
      atMs: [finished, started, instant(value.created_at_ms)].find((value) => value !== null) ?? null,
      durationMs: started !== null && finished !== null && finished >= started ? finished - started : null,
    };
  }
  function normalizeRoute(route) {
    return {
      worker: text(route?.worker),
      q: text(route?.worker_q).slice(0, QUERY_MAX_CHARS),
      status: Object.hasOwn(statuses, route?.worker_status) ? route.worker_status : "",
      campaign: campaignId(route?.worker_campaign),
    };
  }
  // r[impl kiln.dashboard.workers.view.scenario.register] Display order is by readable name, never opaque hash order.
  function rows(inventory) {
    return (Array.isArray(inventory?.workers) ? inventory.workers : []).map((worker) => {
      const projects = Array.isArray(worker.projects) ? worker.projects : [];
      const names = projects.map((project) => text(project.label) || text(project.id)).filter(Boolean);
      const label = names.length ? names.slice(0, PROJECT_NAME_PREVIEW).join(", ") + (names.length > PROJECT_NAME_PREVIEW ? ` +${names.length - PROJECT_NAME_PREVIEW} more` : "") : text(worker.provider_profile_id) || "Unnamed target";
      return { worker, id: text(worker.id), label, provider: text(worker.provider_profile_id), platform: text(worker.platform), capacity: capacity(worker.capacity), heartbeat: heartbeat(worker), latest: execution(worker.latest_run), search: [worker.id, worker.provider_profile_id, worker.platform, ...projects.flatMap((project) => [project.id, project.label])].map(text).join("\n").toLowerCase() };
    }).sort((a, b) => compare(a.label.toLowerCase(), b.label.toLowerCase()) || compare(a.provider, b.provider) || compare(a.platform, b.platform) || compare(a.id, b.id));
  }
  function matches(row, q, status) {
    return (!text(q).trim() || row.search.includes(text(q).trim().toLowerCase())) && (!status || row.worker.status === status);
  }
  function selection(items, route) {
    const selected = items.find((row) => row.id === route.worker) || null;
    return { selected, missing: Boolean(route.worker && !selected), hidden: Boolean(selected && !matches(selected, route.q, route.status)) };
  }

  let mounted = null;
  let context = null;
  let shown = null;
  let applied = null;
  let model = null;
  let generation = 0;
  let filterTimer;
  function element(tag, value, className) {
    const node = document.createElement(tag);
    if (value !== undefined) node.textContent = String(value);
    if (className) node.className = className;
    return node;
  }
  function facts(entries) {
    const list = element("dl", undefined, "facts");
    for (const [label, value] of entries) {
      const cell = element("dd");
      if (value instanceof Node) cell.append(value);
      else cell.textContent = value === null || value === undefined || value === "" ? "Not recorded" : String(value);
      list.append(element("dt", label), cell);
    }
    return list;
  }
  function section(title, ...children) {
    const node = element("section", undefined, "workers-section");
    node.append(element("h3", title), ...children);
    return node;
  }
  function disclosure(label, key, ...children) {
    const node = element("details", undefined, "workers-disclosure");
    node.dataset.workerDisclosure = key;
    node.append(element("summary", label), ...children);
    return node;
  }
  function stateBadge(definition) {
    const badge = element("span", definition.label, "state-badge");
    if (definition.tone) badge.dataset.tone = definition.tone;
    return badge;
  }
  function statusBadge(status) {
    const definition = slotState(status);
    const badge = stateBadge(definition);
    badge.title = definition.meaning;
    return badge;
  }
  function emptyState(title, message) {
    const node = element("div", undefined, "empty-state empty-state-inline");
    node.append(element("p", title, "empty-state-title"), element("p", message, "empty-state-text"));
    return node;
  }
  function href(patch) {
    return `?${new URLSearchParams(Object.entries({ ...context.route, ...patch }).filter(([, value]) => value !== "" && value !== null && value !== undefined))}`;
  }
  function navigate(patch) {
    clearTimeout(filterTimer);
    if (context?.isCurrent()) context.navigate(patch);
  }
  function link(patch, label, className) {
    const node = element("a", label, className);
    node.href = href(patch);
    node.addEventListener("click", (event) => {
      if (event.button || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
      event.preventDefault();
      const filters = applied ? { worker_q: applied.q, worker_status: applied.status } : {};
      navigate({ ...filters, ...patch });
    });
    return node;
  }
  function when(value) { return value === null ? element("span", "Not recorded", "not-recorded") : ui().timeElement(value); }
  function copyIdentity(value) {
    const wrap = element("div", undefined, "workers-identity");
    wrap.append(element("code", text(value) || "Not recorded"));
    if (text(value)) {
      const button = element("button", "Copy target ID", "quiet");
      button.type = "button";
      const status = element("span", undefined, "small");
      status.setAttribute("role", "status");
      button.addEventListener("click", () => ui().copyText(value, status, "Target ID copied."));
      wrap.append(button, status);
    }
    return wrap;
  }
  function metrics(value) {
    const grid = element("dl", undefined, "workers-metrics");
    for (const [key, label] of [["active", "Occupied"], ["free_slots", "Free slots"], ["queued", "Queued runs"]]) {
      const cell = element("div");
      cell.append(element("dt", label), element("dd", countText(value[key])));
      grid.append(cell);
    }
    return grid;
  }
  function capacityNote(value) {
    return `${countText(value.enrolled_slots)} of ${plural(value.configured_slots, "configured project slot")} enrolled.`;
  }
  function aboutInventory(inventory) {
    return disclosure("About targets and slot counts", "about",
      element("p", "A target groups projects with the same workflow provider, endpoint and platform. It is not a discovered physical machine. Each enrolled project contributes one admission slot; uncertain nonterminal executions keep their occupied slot.", "small"),
      element("p", "Free project slots are not a heartbeat, a health check or a guarantee that a provider can dispatch work. Heartbeat observations and previous run outcomes are separate facts.", "small"),
      facts([["Snapshot version", countText(inventory.version)], ["Inventory", "Complete configured target inventory within the service limits"], ["Targets", countText(inventory.summary.targets)], ["Configured slots", countText(inventory.summary.configured_slots)], ["Enrolled slots", countText(inventory.summary.enrolled_slots)]]));
  }
  function summary(inventory) {
    const panel = element("section", undefined, "panel workers-summary");
    panel.setAttribute("aria-label", "All target admission slots");
    const head = element("div", undefined, "workers-summary-head");
    head.append(element("h2", "Project admission slots"), element("p", `${plural(inventory.summary.targets, "target")} · ${countText(inventory.summary.enrolled_slots)}/${countText(inventory.summary.configured_slots)} slots enrolled`, "small"));
    panel.append(head, metrics(capacity(inventory.summary)), element("p", "Free slots do not mean online workers.", "small"));
    return panel;
  }
  function latestCell(row) {
    const node = element("div", undefined, "workers-latest-cell");
    if (!row.latest) { node.append(element("span", "No execution recorded", "small")); return node; }
    node.append(ui().statusPill(row.latest.run.outcome, row.latest.run.phase), when(row.latest.atMs));
    return node;
  }
  function tableRow(row, route) {
    const node = element("tr");
    node.dataset.worker = row.id;
    if (route.worker === row.id) node.dataset.selected = "true";
    const target = element("td"); target.dataset.label = "Target";
    const choose = link({ page: "workers", worker: row.id }, row.label, "row-link workers-target-link");
    choose.dataset.worker = row.id;
    if (route.worker === row.id) choose.setAttribute("aria-current", "true");
    choose.title = `Inspect this exact target: ${row.id}`;
    target.append(choose, element("span", row.provider || "Provider not recorded", "workers-provider"), element("span", row.platform || "Platform not recorded", "small"));
    const slots = element("td"); slots.dataset.label = "Project slots";
    slots.append(statusBadge(row.worker.status), element("span", `${countText(row.capacity.free_slots)} free / ${countText(row.capacity.enrolled_slots)} enrolled`, "workers-slot-count"));
    const beat = element("td"); beat.dataset.label = "Heartbeat";
    beat.append(stateBadge(row.heartbeat));
    if (row.heartbeat.atMs !== null) beat.append(when(row.heartbeat.atMs));
    const queued = element("td", countText(row.capacity.queued), "num"); queued.dataset.label = "Queued";
    const latest = element("td"); latest.dataset.label = "Latest execution"; latest.append(latestCell(row));
    node.append(target, slots, beat, queued, latest);
    return node;
  }
  function register(inventory, items, route) {
    const panel = element("section", undefined, "panel workers-register");
    panel.append(element("h2", "Execution targets"));
    if (!items.length) {
      panel.append(emptyState("No targets configured", "This inventory has no configured workflow targets. It is not a discovery of physical workers."), aboutInventory(inventory));
      return panel;
    }
    const form = element("form", undefined, "workers-filters");
    form.setAttribute("role", "search");
    const searchField = element("div"), stateField = element("div");
    const searchLabel = element("label", "Search targets"); searchLabel.htmlFor = "workers-q";
    const search = element("input");
    Object.assign(search, { id: "workers-q", type: "search", maxLength: QUERY_MAX_CHARS, autocomplete: "off", placeholder: "Project, provider, platform or target ID", value: route.q });
    const stateLabel = element("label", "Slot state"); stateLabel.htmlFor = "workers-status";
    const select = element("select"); select.id = "workers-status";
    for (const [value, label] of [["", "All slot states"], ...Object.entries(statuses).map(([key, entry]) => [key, entry.label])]) {
      const option = element("option", label); option.value = value; select.append(option);
    }
    select.value = route.status;
    searchField.append(searchLabel, search); stateField.append(stateLabel, select); form.append(searchField, stateField);
    const tools = element("div", undefined, "workers-tools");
    const countLine = element("p", undefined, "small workers-result-count"); countLine.setAttribute("role", "status");
    const clear = element("button", "Clear filters", "quiet"); clear.type = "button";
    tools.append(countLine, clear);
    const wrap = element("div", undefined, "data-table-wrap");
    const table = element("table", undefined, "data-table data-table-stack workers-table");
    table.append(element("caption", "Configured execution targets, ordered by project name. Open a target for assigned work and exact binding facts.", "sr-only"));
    const head = element("thead"), headings = element("tr"), body = element("tbody");
    for (const label of ["Target", "Project slots", "Heartbeat", "Queued", "Latest execution"]) {
      const cell = element("th", label, label === "Queued" ? "num" : undefined); cell.scope = "col"; headings.append(cell);
    }
    head.append(headings); table.append(head, body); wrap.append(table);
    const entries = items.map((row) => { const node = tableRow(row, route); body.append(node); return { row, node }; });
    const noMatch = emptyState("No matching targets", "Try another project, provider, platform or slot state, or clear the filters. A selected target is not replaced.");
    noMatch.classList.add("workers-no-match");
    const apply = () => {
      const q = search.value.slice(0, QUERY_MAX_CHARS);
      const status = Object.hasOwn(statuses, select.value) ? select.value : "";
      let visible = 0;
      for (const { row, node } of entries) {
        const match = matches(row, q, status);
        node.hidden = !match;
        if (match) visible += 1;
        node.querySelector(".workers-target-link").href = href({ page: "workers", worker: row.id, worker_q: q, worker_status: status });
      }
      applied = { q, status };
      const filtered = Boolean(q.trim() || status);
      countLine.textContent = filtered ? `${visible} of ${plural(items.length, "target")}` : `${plural(items.length, "target")} · project name A–Z`;
      clear.hidden = !filtered;
      noMatch.hidden = visible !== 0;
      wrap.hidden = visible === 0;
      const hidden = mounted?.querySelector(".workers-selection-hidden");
      if (hidden) hidden.hidden = !selection(items, { worker: route.worker, q, status }).hidden;
    };
    const commit = () => {
      clearTimeout(filterTimer);
      filterTimer = setTimeout(() => { if (context?.isCurrent() && search.isConnected) navigate({ worker_q: applied.q, worker_status: applied.status }); }, FILTER_COMMIT_MS);
    };
    form.addEventListener("submit", (event) => { event.preventDefault(); apply(); navigate({ worker_q: applied.q, worker_status: applied.status }); });
    search.addEventListener("input", () => { apply(); commit(); });
    select.addEventListener("change", () => { apply(); commit(); });
    clear.addEventListener("click", () => { search.value = ""; select.value = ""; apply(); commit(); search.focus(); });
    table.addEventListener("keydown", (event) => {
      if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
      const links = [...table.querySelectorAll("tr:not([hidden]) .workers-target-link")];
      const index = links.indexOf(document.activeElement);
      if (index < 0) return;
      const next = links[index + (event.key === "ArrowDown" ? 1 : -1)];
      if (next) { event.preventDefault(); next.focus(); }
    });
    panel.append(form, tools, wrap, noMatch, aboutInventory(inventory));
    apply();
    return panel;
  }
  function heartbeatDetail(row) {
    const beat = row.heartbeat;
    const node = section("Worker heartbeat", stateBadge(beat));
    if (!beat.observed) node.append(element("p", "No admitted heartbeat is recorded for this target. Project slots do not establish whether a worker is online.", "small"));
    else if (beat.state === "offline") node.append(element("p", "The heartbeat observation is not current. Its snapshot counts do not establish currently available worker capacity.", "warning"));
    else if (beat.state === "online") node.append(element("p", "At least one current heartbeat is admitted. The snapshot reports availability, not machine health or guaranteed dispatch.", "small"));
    else node.append(element("p", "No current liveness or available worker capacity is inferred from this recorded observation.", "warning"));
    if (beat.observed) {
      const values = [["Last admitted heartbeat", when(beat.atMs)], ["Snapshot observed slots", countText(beat.slots)], ["Snapshot occupied", countText(beat.active)], ["Current observed free", beat.state === "online" ? countText(beat.currentFree) : "No current heartbeat"]];
      if (beat.state !== "online") values.push(["Snapshot free (not current)", countText(beat.reportedFree)]);
      node.append(facts(values));
    }
    return node;
  }
  function latestDetail(row) {
    const node = section("Latest execution");
    if (!row.latest) { node.append(element("p", "No execution is retained for this target. A queued request is not execution evidence, and no outcome or timing is inferred.", "small")); return node; }
    const latest = row.latest, run = latest.run;
    const main = element("div", undefined, "workers-latest-head");
    main.append(ui().statusPill(run.outcome, run.phase), when(latest.atMs));
    node.append(main);
    const project = row.worker.projects.find((project) => project.id === run.project_id);
    node.append(context.projectLink(run.project_id, text(project?.label) || text(run.project_id), "row-link"));
    if (text(run.id)) node.append(context.runLink(run.id, "Open exact run"));
    node.append(element("p", "Last retained execution, not current worker health. Later queued requests do not replace it.", "small"));
    node.append(disclosure("Execution facts and timing", "execution", facts([["Run ID", text(run.id)], ["Phase", text(run.phase)], ["Sequence", countText(run.sequence)], ["Error code", text(run.error_code) || null], ["Created", when(instant(run.created_at_ms))], ["Started", when(instant(run.started_at_ms))], ["Completed", when(instant(run.completed_at_ms))], ["Execution duration", latest.durationMs === null ? null : ui().durationElement(latest.durationMs)]])));
    return node;
  }
  function assignedProjects(row) {
    const node = section("Assigned projects and occupied slots");
    if (!row.worker.projects.length) { node.append(element("p", "No assigned projects are recorded.", "small")); return node; }
    const wrap = element("div", undefined, "data-table-wrap");
    const table = element("table", undefined, "data-table data-table-stack workers-project-table");
    const head = element("thead"), titles = element("tr"), body = element("tbody");
    for (const label of ["Project", "Enrollment", "Occupied slots", "Queued"]) { const cell = element("th", label); cell.scope = "col"; titles.append(cell); }
    head.append(titles); table.append(head, body); wrap.append(table);
    for (const project of row.worker.projects) {
      const tr = element("tr");
      const name = element("td"); name.dataset.label = "Project"; name.append(context.projectLink(project.id, text(project.label) || text(project.id), "row-link"));
      const enrollment = element("td", project.enrolled === true ? "Enrolled" : project.enrolled === false ? "Not enrolled" : "Not recorded"); enrollment.dataset.label = "Enrollment";
      const runs = element("td"); runs.dataset.label = "Occupied slots";
      if (!project.active_run_ids.length) runs.append(element("span", "None recorded", "small"));
      for (const id of project.active_run_ids) { const run = context.runLink(id, `Inspect run ${String(id).slice(0, 12)}…`); run.title = id; runs.append(run); }
      const queued = element("td", countText(project.queued), "num"); queued.dataset.label = "Queued";
      tr.append(name, enrollment, runs, queued); body.append(tr);
    }
    node.append(wrap, element("p", "An uncertain nonterminal execution still occupies its project slot. No occupied slot does not imply a physical worker is idle or online.", "small"));
    return node;
  }
  // r[impl kiln.dashboard.workers.view.scenario.detail]
  function detail(inventory, items, route) {
    const sheet = element("section", undefined, "panel workers-detail");
    const back = link({ worker: "" }, "All targets", "button workers-back");
    back.prepend(ui().icon("arrow-left"));
    sheet.append(back);
    const choice = selection(items, route);
    const heading = element("h2", choice.selected ? choice.selected.label : "Target unavailable", "workers-title"); heading.tabIndex = -1;
    sheet.append(heading);
    if (choice.missing) {
      sheet.append(element("p", "This exact target is not in the current inventory. No other target was substituted.", "warning"), copyIdentity(route.worker));
      return sheet;
    }
    const row = choice.selected;
    const hidden = element("p", "This target is outside the current list filters. It remains selected.", "small workers-selection-hidden"); hidden.hidden = !choice.hidden;
    const badges = element("div", undefined, "badges workers-detail-badges");
    badges.append(statusBadge(row.worker.status), element("span", row.platform || "Platform not recorded", "small"));
    sheet.append(badges, hidden, element("p", slotState(row.worker.status).meaning, "small"));
    const capacitySection = section("Project admission slots", metrics(row.capacity), element("p", capacityNote(row.capacity), "small"));
    const grid = element("div", undefined, "workers-detail-grid"); grid.append(capacitySection, latestDetail(row));
    sheet.append(grid, heartbeatDetail(row), assignedProjects(row));
    sheet.append(disclosure("Exact target binding and snapshot", "binding", facts([["Kiln target ID", copyIdentity(row.id)], ["Provider profile", row.provider], ["Platform", row.platform], ["Heartbeat liveness", text(row.worker.liveness) || null], ["Snapshot version", countText(inventory.version)], ["Assigned projects", row.worker.projects.length], ["Configured project slots", countText(row.capacity.configured_slots)], ["Enrolled project slots", countText(row.capacity.enrolled_slots)]])));
    return sheet;
  }
  function campaignCard(route) {
    const panel = element("section", undefined, "panel workers-campaign-card");
    panel.append(element("h2", "Selected campaign progress and blockers"), element("p", "The campaign snapshot is read-only. Controls appear only where this service admits them for you; each goes through the canonical campaign owner and grants no reservation, dispatch, landing or lifecycle authority.", "small"));
    const form = element("form", undefined, "workers-campaign-lookup");
    const label = element("label", "Exact campaign ID"); label.htmlFor = "workers-campaign-id";
    const input = element("input"); Object.assign(input, { id: "workers-campaign-id", type: "text", value: route.campaign, maxLength: CAMPAIGN_ID_MAX_CHARS, autocomplete: "off", placeholder: "Campaign ID (up to 256 characters)" });
    const lookup = element("button", "Load campaign"); lookup.type = "submit";
    const clearSelection = element("button", "Clear campaign selection"); clearSelection.type = "button";
    const refresh = element("button", "Refresh campaign"); refresh.type = "button"; refresh.disabled = !route.campaign;
    const status = element("p", route.campaign ? `Selected campaign: ${route.campaign}` : "No campaign selected. Enter one exact ID to inspect it.", "small"); status.setAttribute("role", "status");
    form.append(label, input, lookup, clearSelection, refresh, status);
    const campaignRoot = element("div", undefined, "workers-campaign-projection"); campaignRoot.dataset.campaignRoot = "true";
    if (!route.campaign) campaignRoot.append(element("p", "Campaign data is not requested until an exact ID is selected. No absence or completion is inferred.", "small"));
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      const id = campaignId(input.value);
      if (!id) { status.textContent = `Enter a non-empty campaign ID of at most ${CAMPAIGN_ID_MAX_CHARS} characters without control characters.`; input.focus(); return; }
      navigate({ worker: route.worker, worker_q: route.q, worker_status: route.status, worker_campaign: id });
    });
    clearSelection.addEventListener("click", () => navigate({ worker: route.worker, worker_q: route.q, worker_status: route.status, worker_campaign: "" }));
    refresh.addEventListener("click", () => {
      if (!route.campaign || !context?.isCurrent()) return;
      void window.KilnInventory?.loadCampaign(campaignRoot, context, route.campaign);
    });
    // Previews are read only when asked for; an unconfigured service is not polled.
    const definitions = element("details", undefined, "inventory-disclosure workers-campaign-definitions");
    const definitionsRoot = element("div");
    definitions.append(element("summary", "Start a configured campaign"), definitionsRoot);
    definitions.addEventListener("toggle", () => {
      if (!definitions.open || !context?.isCurrent()) return;
      void window.KilnInventory?.loadDefinitions(definitionsRoot, context, (id) => navigate({ worker: route.worker, worker_q: route.q, worker_status: route.status, worker_campaign: id }));
    });
    panel.append(form, campaignRoot, definitions);
    return panel;
  }
  function render(inventory, items, route) {
    const page = element("div", undefined, "workers-page");
    if (window.KilnInventory) {
      const observed = element("div", undefined, "workers-observed-inventories");
      observed.dataset.inventoryRoot = "true";
      page.append(observed);
    }
    page.append(campaignCard(route));
    if (!route.worker) page.append(summary(inventory), register(inventory, items, route));
    else {
      page.append(detail(inventory, items, route));
      const browser = disclosure(`Browse ${plural(items.length, "target")}`, "browse", register(inventory, items, route));
      browser.classList.add("workers-browse");
      page.append(browser);
    }
    return page;
  }
  function title() {
    const selected = model && shown?.worker ? model.rows.find((row) => row.id === shown.worker) : null;
    document.title = ["Kiln", "Workers", shown?.worker ? selected?.label || "Target unavailable" : ""].filter(Boolean).join(" · ");
  }
  function focused(root) {
    const active = document.activeElement;
    if (!active || !root.contains(active)) return "";
    if (active.classList.contains("workers-back")) return "@back";
    return active.dataset.worker || (["workers-q", "workers-status"].includes(active.id) ? `#${active.id}` : "");
  }
  // r[impl kiln.dashboard.workers.view] r[impl kiln.dashboard.workers.view.scenario.restore]
  async function load(root, ctx) {
    const route = normalizeRoute(ctx.route);
    if (mounted !== root) { clear(); mounted = root; }
    context = ctx;
    if (shown && model && applied && route.worker === shown.worker && route.campaign === shown.campaign && (route.q !== shown.q || route.status !== shown.status) && route.q === applied.q && route.status === applied.status && root.firstChild) {
      shown = route; title(); return;
    }
    const current = ++generation;
    clearTimeout(filterTimer);
    const live = () => current === generation && root.isConnected && ctx.isCurrent();
    const first = !shown;
    if (first) root.replaceChildren(element("p", "Reading configured execution targets…", "small"));
    root.setAttribute("aria-busy", "true");
    try {
      const inventory = await ctx.api("/api/workers");
      if (!live()) return;
      validate(inventory);
      if (inventory.storage_failed) throw new Error("Durable worker state is unavailable. No free capacity or worker health is inferred.");
      const prior = shown?.worker || "";
      const focus = focused(root);
      const open = new Map([...root.querySelectorAll("details[data-worker-disclosure]")].map((node) => [node.dataset.workerDisclosure, node.open]));
      model = { inventory, rows: rows(inventory) };
      applied = { q: route.q, status: route.status };
      root.replaceChildren(render(inventory, model.rows, route));
      const observed = root.querySelector("[data-inventory-root]");
      if (observed) void window.KilnInventory.load(observed, ctx);
      const campaignRoot = root.querySelector("[data-campaign-root]");
      if (campaignRoot && route.campaign) void window.KilnInventory?.loadCampaign(campaignRoot, ctx, route.campaign);
      if (prior === route.worker) for (const node of root.querySelectorAll("details[data-worker-disclosure]")) if (open.has(node.dataset.workerDisclosure)) node.open = open.get(node.dataset.workerDisclosure);
      shown = route;
      if (focus.startsWith("#")) root.querySelector(focus)?.focus({ preventScroll: true });
      else if (prior && !route.worker) {
        const previous = [...root.querySelectorAll(".workers-target-link")].find((node) => node.dataset.worker === prior && !node.closest("tr").hidden);
        (previous || root.querySelector("#workers-q"))?.focus();
      }
      else if (!first && prior !== route.worker && route.worker) {
        root.querySelector(".workers-detail")?.scrollIntoView({ block: "start" });
        root.querySelector(".workers-title")?.focus({ preventScroll: true });
      }
      else if (focus) [...root.querySelectorAll(".workers-target-link")].find((node) => node.dataset.worker === focus)?.focus({ preventScroll: true });
      title();
    } catch (error) {
      if (!live() || error?.name === "AbortError") return;
      model = null; shown = null; applied = null;
      const message = text(error?.message).trim() || "The worker inventory could not be read.";
      const sentence = /[.!?]$/.test(message) ? message : `${message}.`;
      const panel = element("section", undefined, "panel workers-unavailable");
      const failure = element("p", `Workers unavailable: ${sentence} No presence, absence or worker health is inferred.`, "error-state"); failure.setAttribute("role", "alert");
      const retry = element("button", "Try again"); retry.type = "button";
      retry.addEventListener("click", () => { if (context?.isCurrent()) void load(root, context); });
      panel.append(element("h2", "Workers unavailable"), failure, retry, campaignCard(route)); root.replaceChildren(panel);
      if (window.KilnInventory) {
        const observed = element("div", undefined, "workers-observed-inventories");
        root.prepend(observed);
        void window.KilnInventory.load(observed, ctx);
      }
      const campaignRoot = root.querySelector("[data-campaign-root]");
      if (campaignRoot && route.campaign) void window.KilnInventory?.loadCampaign(campaignRoot, ctx, route.campaign);
    } finally {
      if (live()) root.removeAttribute("aria-busy");
    }
  }
  // r[impl kiln.dashboard.workers.view.scenario.cleared]
  function clear() {
    window.KilnInventory?.clear();
    generation += 1;
    clearTimeout(filterTimer);
    mounted?.replaceChildren();
    mounted?.removeAttribute("aria-busy");
    mounted = null; context = null; shown = null; applied = null; model = null;
  }
  window.KilnWorkers = Object.freeze({ load, clear, validate, statuses, statusBadge, slotState, capacity, heartbeat, execution, normalizeRoute, rows, matches, selection });
})();
