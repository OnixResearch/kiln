// Read-only Fleet queue and capacity explanations from the existing owners.
// Fleet ranks implementation work; Workers counts configured CI project slots;
// Inventory explains explicitly supplied resource observations for one exact work version.
(() => {
  "use strict";
  const HEX = /^[a-f0-9]{64}$/;
  const INVENTORY_SCHEMA = "kiln.observe.inventory.v2";
  const WORK_LIMIT = 100; // The Fleet owner lists at most this many in each category.
  const record = (value) => value !== null && typeof value === "object" && !Array.isArray(value);
  const text = (value) => typeof value === "string" ? value : "";
  const count = (value) => Number.isSafeInteger(value) && value >= 0 ? value : null;
  const number = (value) => count(value) === null ? "Not recorded" : value.toLocaleString("en-US");
  const node = (tag, value, className) => {
    const item = document.createElement(tag);
    if (value !== undefined) item.textContent = String(value);
    if (className) item.className = className;
    return item;
  };
  const label = (value) => text(value).replaceAll("_", " ").replace(/^./, (first) => first.toUpperCase()) || "Not recorded";
  const ref = (value) => text(value?.source) && text(value?.id) ? `${value.source} / ${value.id}` : "Binding not recorded";
  const current = (state) => active === state && state.context.isCurrent() && state.root.isConnected;
  let active = null;

  function facts(entries) {
    const list = node("dl", undefined, "facts fleet-capacity-facts");
    for (const [name, value] of entries) {
      list.append(node("dt", name), node("dd", value === undefined || value === null || value === "" ? "Not recorded" : String(value)));
    }
    return list;
  }

  function empty(title, explanation) {
    const box = node("div", undefined, "empty-state empty-state-inline");
    box.append(node("p", title, "empty-state-title"), node("p", explanation, "empty-state-text"));
    return box;
  }

  function failure(title, error) {
    const message = text(error?.message) || "The owner did not return a usable response.";
    const box = node("p", `${title}: ${message} No absence or capacity is inferred.`, "fleet-capacity-error");
    box.setAttribute("role", "alert");
    return box;
  }

  function validateWork(value) {
    if (!record(value) || count(value.ready_unclaimed) === null || !Array.isArray(value.ranked) || !Array.isArray(value.blocked)
      || value.ranked.length > WORK_LIMIT || value.blocked.length > WORK_LIMIT
      || value.ranked.some((item) => !HEX.test(text(item?.identity)) || count(item?.priority) === null || typeof item.interactive !== "boolean")
      || value.blocked.some((item) => !HEX.test(text(item?.identity)) || !Array.isArray(item.reasons))) {
      throw new Error("Fleet work response is invalid.");
    }
    return value;
  }

  function validateWorkers(value) {
    if (value?.schema !== "kiln.web-workers.v1" || value.bounded !== true || typeof value.storage_failed !== "boolean"
      || !record(value.summary) || !Array.isArray(value.workers)
      || value.workers.some((worker) => !record(worker) || !record(worker.capacity) || !Array.isArray(worker.projects))) {
      throw new Error("Configured worker inventory response is invalid.");
    }
    if (value.storage_failed) throw new Error("Durable worker state is unavailable.");
    return value;
  }

  function blockReason(reason) {
    const kind = record(reason) ? text(reason.state) : text(reason);
    if (kind === "missing_dependency" && HEX.test(text(reason?.identity))) return `Missing dependency · ${reason.identity}`;
    return label(kind);
  }

  function workRow(state, item, position, blocked) {
    const row = node("li", undefined, "fleet-capacity-work-row");
    const header = node("div", undefined, "fleet-capacity-row-head");
    const select = node("button", text(item.title) || "Untitled work", "quiet fleet-capacity-select");
    select.type = "button";
    select.title = `Explain exact work ${item.identity}`;
    select.addEventListener("click", () => selectWork(state, item.identity));
    header.append(node("strong", blocked ? "Blocked" : `Rank ${position + 1}`), select);
    row.append(header, node("p", text(item.repository) || "Repository not recorded", "small"), node("code", item.identity));
    if (blocked) {
      const reasons = node("ul", undefined, "fleet-capacity-reasons");
      if (!item.reasons.length) reasons.append(node("li", "Reasons not recorded; no eligibility inferred."));
      for (const reason of item.reasons) reasons.append(node("li", blockReason(reason)));
      row.append(reasons);
    } else {
      row.append(node("p", `Owner priority ${number(item.priority)} · ${item.interactive ? "Interactive effect permitted" : "Interactive effect not permitted"}`, "small"));
      if (Array.isArray(item.permitted_paths)) row.append(node("p", `Permitted paths · ${item.permitted_paths.length ? item.permitted_paths.map((path) => text(path) || "Unrecorded path").join(", ") : "None recorded"}`, "small"));
    }
    return row;
  }

  function workPanel(state, work) {
    const panel = node("section", undefined, "panel fleet-capacity-panel");
    panel.append(node("h2", "Ranked and blocked implementation work"),
      node("p", "Fleet owner ranking is priority descending, then canonical identity. Ranked work is eligible in the work ledger, but can already be claimed. This is not a fairness score, dispatch grant or availability forecast.", "small"),
      facts([["Ready and unclaimed (owner count)", number(work.ready_unclaimed)], ["Ranked entries listed", work.ranked.length], ["Blocked entries listed", work.blocked.length]]));
    if (work.ranked.length === WORK_LIMIT || work.blocked.length === WORK_LIMIT) panel.append(node("p", "Each list is bounded at 100 by the Fleet owner. More work may exist; the listed counts are not totals.", "small"));
    for (const [heading, items, blocked] of [["Ranked work", work.ranked, false], ["Blocked work", work.blocked, true]]) {
      const section = node("section", undefined, "fleet-capacity-group");
      section.append(node("h3", heading));
      if (items.length) {
        const list = node("ol", undefined, "fleet-capacity-list");
        items.forEach((item, index) => list.append(workRow(state, item, index, blocked)));
        section.append(list);
      } else section.append(empty(`No ${blocked ? "blocked" : "ranked"} work listed`, "The Fleet owner returned no entries in this category; other inventories are independent."));
      panel.append(section);
    }
    return panel;
  }

  function workerPanel(state, inventory) {
    const panel = node("section", undefined, "panel fleet-capacity-panel");
    panel.append(node("h2", "Configured execution targets"), node("p", "Workers is the owner of CI project admission slots. A target groups project/provider bindings, not discovered physical machines. These slots do not measure Fleet interactive herdr placement or model capacity.", "small"),
      facts([["Configured targets", number(inventory.summary.targets)], ["Enrolled project slots", number(inventory.summary.enrolled_slots)], ["Configured project slots", number(inventory.summary.configured_slots)], ["Occupied project slots", number(inventory.summary.active)], ["Free project slots", number(inventory.summary.free_slots)], ["Queued CI runs", number(inventory.summary.queued)]]),
      node("p", "Free slots are not a heartbeat or a dispatch guarantee. Uncertain nonterminal executions retain occupied slots; stale observations do not establish current worker availability.", "small"));
    if (!inventory.workers.length) {
      panel.append(empty("No configured targets listed", "This does not establish an empty physical fleet."));
    } else {
      const list = node("ul", undefined, "fleet-capacity-list");
      for (const worker of inventory.workers) {
        const item = node("li", undefined, "fleet-capacity-work-row");
        const id = text(worker.id);
        item.append(node("strong", text(worker.provider_profile_id) || "Provider not recorded"), node("p", `${text(worker.platform) || "Platform not recorded"} · ${id || "Target ID not recorded"}`, "small"),
          facts([["Slot state", label(worker.status)], ["Enrolled / free project slots", `${number(worker.capacity.enrolled_slots)} / ${number(worker.capacity.free_slots)}`], ["Heartbeat liveness", label(worker.liveness)], ["Heartbeat last seen (ms)", number(worker.observed_capacity?.last_seen_at_ms)]]));
        list.append(item);
      }
      panel.append(list);
    }
    const link = node("a", "Inspect full configured target inventory", "row-link");
    link.href = "?page=workers";
    link.addEventListener("click", (event) => {
      if (!state.context.navigate || event.button || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
      event.preventDefault(); state.context.navigate({ page: "workers" });
    });
    panel.append(link);
    return panel;
  }

  function validateInventory(value, requested) {
    if (!record(value) || value.schema !== INVENTORY_SCHEMA || !HEX.test(text(value.identity)) || !record(value.snapshot)
      || !Array.isArray(value.sources) || !Array.isArray(value.non_claims) || !Array.isArray(value.attempt_explanations)
      || typeof value.work_requirements_unknown !== "boolean" || (value.explanation !== null && !record(value.explanation))
      || (value.explanation && (value.explanation.work_version !== requested || !Array.isArray(value.explanation.blockers) || !Array.isArray(value.explanation.candidates)))) {
      throw new Error("Exact work inventory response is invalid.");
    }
    return value;
  }

  function blockers(values, emptyText) {
    const list = node("ul", undefined, "fleet-capacity-reasons");
    if (!values.length) list.append(node("li", emptyText));
    for (const value of values) list.append(node("li", `${text(value?.code) || "Unknown reason"} · ${text(value?.constraint) || "Constraint not recorded"} · ${value?.subject ? ref(value.subject) : "Snapshot / policy"}`));
    return list;
  }

  function inventoryPanel(value, work) {
    const panel = node("div", undefined, "fleet-capacity-explanation");
    panel.append(facts([["Requested exact work version", work], ["Inventory projection", value.identity], ["Evaluated at (ms)", number(value.evaluated_at_ms)], ["Synthetic observations", value.snapshot.synthetic === true ? "Yes — not live capacity evidence" : "No"]]));
    const sources = node("details");
    sources.append(node("summary", "Observation sources and freshness"));
    if (!value.sources.length) sources.append(node("p", "No sources supplied; freshness unknown.", "small"));
    for (const source of value.sources) sources.append(node("p", `${text(source.source) || "Unrecorded source"} · ${label(source.state)} · observed ${number(source.observed_at_ms)} ms · valid until ${number(source.valid_until_ms)} ms${text(source.reason) ? ` · ${source.reason}` : ""}`, "small"));
    panel.append(sources);
    if (value.work_requirements_unknown) panel.append(node("p", "Current admitted requirements for this exact work version are unknown. No resource readiness can be inferred.", "fleet-capacity-warning"));
    if (value.explanation) {
      const explanation = value.explanation;
      panel.append(facts([["Definition", ref(explanation.definition)], ["Policy", text(explanation.policy_ref)], ["Combinations considered", number(explanation.combinations_considered)], ["Candidate list truncated", String(explanation.truncated)]]),
        node("h4", "Owner observation blockers"), blockers(explanation.blockers, "No blockers supplied in these observations; this is not a reservation or dispatch authorization."));
      for (const candidate of explanation.candidates) {
        const detail = node("details");
        detail.append(node("summary", `${ref(candidate.worker)} → ${ref(candidate.execution)} + ${ref(candidate.model)} · ${candidate.observed_eligible === true ? "Observed eligible; revalidate" : "Blocked / unknown"}`),
          blockers(Array.isArray(candidate.blockers) ? candidate.blockers : [], "No candidate blockers supplied; revalidation still required."));
        panel.append(detail);
      }
    } else panel.append(node("p", "No readiness explanation supplied for this exact work. Do not infer eligibility or free capacity.", "small"));
    if (value.attempt_explanations.length) {
      const attempts = node("details");
      attempts.append(node("summary", `Exact attempt binding explanations (${value.attempt_explanations.length})`));
      for (const attempt of value.attempt_explanations) {
        const binding = attempt.binding || {};
        attempts.append(node("p", `Attempt ${text(binding.attempt) || "not recorded"} · fence ${number(binding.fence)} · worker ${ref(binding.worker)}`, "small"),
          blockers(Array.isArray(attempt.blockers) ? attempt.blockers : [], "No blockers supplied for this observed attempt."));
      }
      panel.append(attempts);
    }
    if (value.non_claims.length) {
      const nonClaims = node("details");
      nonClaims.append(node("summary", "Producer non-claims"));
      for (const item of value.non_claims) nonClaims.append(node("p", text(item), "small"));
      panel.append(nonClaims);
    }
    panel.append(node("p", "This independent resource inventory never grants a reservation, dispatch, worker presence or model availability. Fleet work eligibility and resource readiness are separate owner observations.", "small"));
    return panel;
  }

  async function explain(state, work) {
    const sequence = ++state.selection;
    state.exact.replaceChildren(node("p", `Reading exact work ${work}…`, "small"));
    state.exact.setAttribute("aria-busy", "true");
    try {
      const result = validateInventory(await state.context.api(`/api/inventory?work=${work}`), work);
      if (current(state) && sequence === state.selection) state.exact.replaceChildren(inventoryPanel(result, work));
    } catch (error) {
      if (current(state) && sequence === state.selection && error?.name !== "AbortError") state.exact.replaceChildren(failure("Exact work inventory unavailable", error));
    } finally {
      if (current(state) && sequence === state.selection) state.exact.removeAttribute("aria-busy");
    }
  }

  function selectWork(state, work) {
    if (!HEX.test(work)) return;
    state.input.value = work;
    state.status.textContent = `Selected exact work ${work}.`;
    void explain(state, work);
  }

  async function mount(root, context) {
    clear();
    const state = { root, context, selection: 0 };
    active = state;
    const page = node("div", undefined, "fleet-capacity-page");
    page.append(node("h2", "Fleet queue and capacity"), node("p", "Read-only owner snapshots. The Fleet work ledger, configured CI project slots and separately supplied resource observations do not imply one another.", "small"));
    const workRoot = node("div", undefined, "fleet-capacity-region");
    const workerRoot = node("div", undefined, "fleet-capacity-region");
    const exactPanel = node("section", undefined, "panel fleet-capacity-panel");
    exactPanel.append(node("h2", "Explain one exact work version"), node("p", "Use a listed work identity or enter the full 64-character admitted implementation identity. The inventory owner must have an exact current requirements attachment to explain readiness.", "small"));
    const form = node("form", undefined, "fleet-capacity-form");
    const labelNode = node("label", "Exact work identity");
    const input = node("input");
    input.type = "text"; input.maxLength = 64; input.pattern = "[a-f0-9]{64}"; input.placeholder = "64-character BLAKE3 identity";
    labelNode.append(input);
    const button = node("button", "Explain work"); button.type = "submit";
    const status = node("p", "No work version selected; resource eligibility is not inferred.", "small"); status.setAttribute("role", "status");
    form.append(labelNode, button, status);
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      const work = input.value.trim();
      if (!HEX.test(work)) { status.textContent = "Enter an exact 64-character lowercase work identity."; input.focus(); return; }
      selectWork(state, work);
    });
    const exact = node("div", undefined, "fleet-capacity-region");
    exactPanel.append(form, exact);
    page.append(workRoot, workerRoot, exactPanel);
    root.replaceChildren(page);
    Object.assign(state, { workRoot, workerRoot, exact, input, status });
    workRoot.replaceChildren(node("p", "Reading Fleet work…", "small"));
    workerRoot.replaceChildren(node("p", "Reading configured targets…", "small"));
    const requests = [
      ["Fleet work unavailable", workRoot, "/api/fleet/work", validateWork, workPanel],
      ["Configured targets unavailable", workerRoot, "/api/workers", validateWorkers, workerPanel],
    ];
    await Promise.all(requests.map(async ([title, destination, route, validate, render]) => {
      try {
        const value = validate(await context.api(route));
        if (current(state)) destination.replaceChildren(render(state, value));
      } catch (error) {
        if (current(state) && error?.name !== "AbortError") destination.replaceChildren(failure(title, error));
      }
    }));
  }

  function clear() {
    if (active) {
      active.selection += 1;
      active.root.replaceChildren();
      active = null;
    }
  }

  window.KilnFleetCapacity = Object.freeze({ mount, clear });
})();
