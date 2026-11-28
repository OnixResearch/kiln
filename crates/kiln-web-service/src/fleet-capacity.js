// Read-only Fleet queue and capacity explanations from the existing owners.
// Fleet ranks implementation work; Workers counts configured CI project slots;
// Inventory explains explicitly supplied resource observations for one exact work version.
(() => {
  "use strict";
  const HEX = /^[a-f0-9]{64}$/;
  const INVENTORY_SCHEMA = "kiln.observe.inventory.v2";
  const ADMISSION_SCHEMA = "kiln.implementation-admission.v1";
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

  function newRequestId() {
    if (typeof crypto?.randomUUID === "function") return crypto.randomUUID();
    const bytes = new Uint8Array(16);
    crypto.getRandomValues(bytes);
    return `adm-${[...bytes].map((b) => b.toString(16).padStart(2, "0")).join("")}`;
  }

  function formatEffortMetric(value, status, absenceReason) {
    if (value === null || value === undefined || !Number.isSafeInteger(value) || value < 0 || status === "absent" || status === "not_recorded") {
      return absenceReason ? `not recorded (${absenceReason})` : "not recorded";
    }
    if (status === "incomplete") {
      return `${value.toLocaleString("en-US")} (incomplete${absenceReason ? `: ${absenceReason}` : ""})`;
    }
    return value.toLocaleString("en-US");
  }

  function formatAmounts(value) {
    if (!record(value)) return "not recorded";
    const entries = Object.entries(value);
    if (!entries.length) return "none";
    return entries.map(([dim, amt]) => `${dim}: ${Number.isSafeInteger(amt) && amt >= 0 ? amt.toLocaleString("en-US") : "not recorded"}`).join(", ");
  }

  function validateAdmission(value) {
    if (!record(value) || value.schema !== ADMISSION_SCHEMA || typeof value.configured !== "boolean"
      || typeof value.bounded !== "boolean" || !Array.isArray(value.accounts)
      || !Array.isArray(value.serving) || !Array.isArray(value.attempt_metrics)) {
      throw new Error("Fair-admission governance response is invalid.");
    }
    return value;
  }

  function dataTable(headers, rows, emptyText) {
    if (!rows.length) return node("p", emptyText, "small");
    const wrap = node("div", undefined, "data-table-wrap");
    const tbl = node("table", undefined, "data-table data-table-stack fleet-capacity-table");
    const thead = node("thead");
    const hr = node("tr");
    for (const h of headers) {
      const th = node("th", h);
      th.scope = "col";
      hr.append(th);
    }
    thead.append(hr);
    const tbody = node("tbody");
    for (const cells of rows) {
      const tr = node("tr");
      cells.forEach((cellVal, idx) => {
        const td = node("td");
        td.dataset.label = headers[idx] || "";
        if (cellVal && typeof cellVal === "object" && "nodeType" in cellVal) td.append(cellVal);
        else td.textContent = cellVal === null || cellVal === undefined || cellVal === "" ? "not recorded" : String(cellVal);
        tr.append(td);
      });
      tbody.append(tr);
    }
    tbl.append(thead, tbody);
    wrap.append(tbl);
    return wrap;
  }

  async function confirmAdmissionControl(state, options) {
    if (typeof state.context?.confirmControl === "function") {
      return state.context.confirmControl(options);
    }
    const byId = (id) => (typeof document?.getElementById === "function" ? document.getElementById(id) : null);
    const dialog = byId("control-dialog");
    const form = byId("control-form");
    if (!dialog || !form) return { confirmed: false, reason: "", unavailable: true };
    const title = byId("confirm-title");
    const warning = byId("confirm-warning");
    const factsEl = byId("confirm-facts");
    const confirmBtn = byId("confirm-control");
    const dismissBtn = byId("dismiss-control");
    const reasonField = byId("confirm-reason-field");
    const reasonInput = byId("confirm-reason");
    const cliWrap = byId("confirm-cli-parity");
    const cliCmd = byId("confirm-cli-cmd");
    if (title) title.textContent = options.title || "Confirm admission policy update";
    if (warning) warning.textContent = options.warning || "";
    if (confirmBtn) { confirmBtn.textContent = options.verb || "Confirm"; confirmBtn.className = "primary"; }
    if (dismissBtn) dismissBtn.textContent = options.dismiss || "Cancel";
    if (factsEl) {
      factsEl.replaceChildren();
      for (const [k, v] of options.facts || []) factsEl.append(node("dt", k), node("dd", String(v)));
    }
    if (reasonField) reasonField.hidden = true;
    if (reasonInput) { reasonInput.required = false; reasonInput.value = ""; }
    if (cliWrap && cliCmd) {
      if (options.cliCommand) { cliWrap.hidden = false; cliCmd.textContent = options.cliCommand; }
      else { cliWrap.hidden = true; cliCmd.textContent = ""; }
    }
    return new Promise((resolve) => {
      let settled = false;
      const cleanup = () => {
        form.removeEventListener("submit", onSubmit);
        dismissBtn?.removeEventListener("click", onCancel);
        dialog.removeEventListener("cancel", onCancel);
        if (dialog.open) dialog.close();
      };
      const finish = (res) => {
        if (settled) return;
        settled = true;
        cleanup();
        options.opener?.focus?.();
        resolve(res);
      };
      const onCancel = () => finish({ confirmed: false, reason: "" });
      const onSubmit = (event) => {
        event.preventDefault();
        finish({ confirmed: true, reason: "", requestId: newRequestId() });
      };
      form.addEventListener("submit", onSubmit);
      dismissBtn?.addEventListener("click", onCancel);
      dialog.addEventListener("cancel", onCancel);
      dialog.showModal();
    });
  }

  function orderAccountTree(accounts) {
    const byId = new Map(accounts.map((a) => [text(a.identity), a]));
    const children = new Map();
    const roots = [];
    for (const acc of accounts) {
      const parent = text(acc.parent);
      if (parent && byId.has(parent) && parent !== text(acc.identity)) {
        if (!children.has(parent)) children.set(parent, []);
        children.get(parent).push(acc);
      } else {
        roots.push(acc);
      }
    }
    const ordered = [];
    const visited = new Set();
    const walk = (acc, fallbackDepth) => {
      const id = text(acc.identity);
      if (visited.has(id)) return;
      visited.add(id);
      const depth = Number.isSafeInteger(acc.depth) ? acc.depth : fallbackDepth;
      ordered.push({ ...acc, _treeDepth: depth });
      for (const child of children.get(id) || []) walk(child, depth + 1);
    };
    for (const r of roots) walk(r, 0);
    for (const acc of accounts) if (!visited.has(text(acc.identity))) walk(acc, 0);
    return ordered;
  }

  // r[impl kiln.admission.live_console_governance]
  function admissionPanel(state, admission, notice = "") {
    const panel = node("section", undefined, "panel fleet-capacity-panel fleet-admission-panel");
    panel.append(
      node("h2", "Fair-admission governance & AttemptMetrics"),
      node("p", "Live fair-admission quota tree, tenant shares, ServingBounds, and AttemptMetrics burn. Transfers are subtractive suballocations shown strictly separately from consumption; missing telemetry renders as not recorded, never 0.", "small"),
    );

    if (!admission.configured || !admission.policy) {
      if (notice) {
        const statusEl = node("p", notice, "small");
        statusEl.setAttribute("role", "status");
        panel.append(statusEl);
      }
      panel.append(empty("No fair-admission policy configured", "The implementation store has no active opt-in fair-admission policy. Existing compatibility path applies; no quota or burn is fabricated."));
    } else {
      const policy = admission.policy;
      const bounds = record(policy.bounds) ? policy.bounds : {};
      panel.append(facts([
        ["Policy identity", text(policy.identity) || "not recorded"],
        ["Version", count(policy.version) !== null ? String(policy.version) : (text(policy.version) || "not recorded")],
        ["Accounting basis", text(policy.accounting_basis) || "not recorded"],
        ["Strategy", text(policy.strategy) || "not recorded"],
        ["Bounds (items / bytes / active / children)", `${number(bounds.max_items)} / ${number(bounds.max_bytes)} / ${number(bounds.max_active)} / ${number(bounds.max_children)}`],
      ]));

      const shares = record(policy.shares) ? policy.shares : {};
      const shareRows = Object.entries(shares).map(([alias, share]) => [
        alias,
        Number.isSafeInteger(share) && share >= 0 ? share.toLocaleString("en-US") : "not recorded",
      ]);
      panel.append(node("h3", "Tenant & campaign shares"), dataTable(["Tenant / Alias", "Share"], shareRows, "No shares declared in policy."));

      // Tenant quota tree (transfers strictly separate from consumption)
      const treeAccounts = orderAccountTree(admission.accounts);
      const accountRows = treeAccounts.map((acc) => {
        const prefix = acc._treeDepth > 0 ? `${"  ".repeat(acc._treeDepth)}↳ ` : "";
        const name = `${prefix}${text(acc.tenant) || text(acc.campaign) || text(acc.identity).slice(0, 12)}${acc.revoked ? " [REVOKED]" : ""}`;
        const labelEl = node("div", undefined, "fleet-admission-account");
        labelEl.dataset.depth = String(acc._treeDepth);
        labelEl.append(node("strong", name), node("code", text(acc.identity).slice(0, 12), "small"));
        return [
          labelEl,
          formatAmounts(acc.allocated),
          formatAmounts(acc.transferred_out),
          formatAmounts(acc.consumed),
          formatAmounts(acc.held),
          formatAmounts(acc.tentative),
          formatAmounts(acc.unknown),
          formatAmounts(acc.available),
          `measured: ${formatAmounts(acc.settled_measured)} · unknown: ${formatAmounts(acc.settled_unknown)}`,
        ];
      });
      panel.append(
        node("h3", "Tenant quota tree (transfers separate from consumption)"),
        dataTable(
          ["Account / Tenant", "Allocated", "Transfers (transferred_out)", "Consumed", "Held debits", "Tentative", "Unknown", "Available", "Settled"],
          accountRows,
          "No quota accounts retained in admission ledger.",
        ),
      );

      // Guarded policy/share update form (Tier-2 confirmation)
      const policyWrap = node("div", undefined, "fleet-admission-controls");
      policyWrap.append(node("h3", "Update fair-admission shares (Tier-2 guarded)"));
      const canOp = state.context?.canOperate?.() !== false;
      const statusEl = node("p", notice || undefined, "small");
      statusEl.setAttribute("role", "status");
      statusEl.hidden = !notice;
      const errorEl = node("p", undefined, "fleet-capacity-error");
      errorEl.setAttribute("role", "alert");
      errorEl.hidden = true;

      if (!canOp) {
        policyWrap.append(node("p", "Operator authority required to modify fair-admission shares or policy.", "small"));
      } else {
        const form = node("form", undefined, "fleet-capacity-form fleet-admission-share-form");
        const aliasInput = node("input");
        aliasInput.type = "text";
        aliasInput.placeholder = "Tenant / alias (e.g. tenant-a)";
        aliasInput.value = Object.keys(shares)[0] || "";
        const aliasLabel = node("label", "Tenant / alias");
        aliasLabel.append(aliasInput);

        const shareInput = node("input");
        shareInput.type = "number";
        shareInput.min = "1";
        shareInput.step = "1";
        shareInput.placeholder = "Positive integer share";
        const firstShare = Object.values(shares)[0];
        if (Number.isSafeInteger(firstShare)) shareInput.value = String(firstShare);
        const shareLabel = node("label", "Share weight");
        shareLabel.append(shareInput);

        const submitBtn = node("button", "Update share…");
        submitBtn.type = "submit";
        form.append(aliasLabel, shareLabel, submitBtn);

        form.addEventListener("submit", async (event) => {
          event.preventDefault();
          errorEl.hidden = true;
          statusEl.hidden = true;
          const alias = aliasInput.value.trim();
          const parsed = Number(shareInput.value);
          if (!alias || !Number.isSafeInteger(parsed) || parsed <= 0) {
            errorEl.textContent = "Share update refused: enter a non-empty tenant alias and a positive integer share.";
            errorEl.hidden = false;
            return;
          }
          if (typeof state.context?.isFresh === "function" && !state.context.isFresh()) {
            errorEl.textContent = "Fleet observation snapshot is stale or disconnected. Wait for a fresh live snapshot before updating shares.";
            errorEl.hidden = false;
            return;
          }
          const nextShares = { ...shares, [alias]: parsed };
          const expectedPolicyId = text(policy.identity);
          const choice = await confirmAdmissionControl(state, {
            title: `Update fair-admission share for ${alias}?`,
            verb: "Apply share update",
            dismiss: "Keep current shares",
            danger: false,
            warning: "Submits an explicit owner policy update. Preserves historical consumption and transfers.",
            facts: [
              ["Expected policy", expectedPolicyId || "none"],
              ["Tenant / alias", alias],
              ["New share", String(parsed)],
            ],
            cliCommand: `kiln-implementer admission --command-file policy.json`,
            opener: submitBtn,
          });
          if (!choice?.confirmed) {
            if (choice?.unavailable) {
              errorEl.textContent = "Confirmation dialog unavailable; share update aborted.";
              errorEl.hidden = false;
            }
            return;
          }
          submitBtn.disabled = true;
          try {
            const body = {
              request_id: choice.requestId || newRequestId(),
              ...(HEX.test(expectedPolicyId) ? { expected_policy_id: expectedPolicyId } : {}),
              command: "configure",
              shares: nextShares,
            };
            const res = await state.context.api("/api/implementation/admission/policy", { method: "POST", body });
            const nextNotice = `Fair-admission policy ${text(res?.result) || "updated"}${res?.policy_identity ? ` · ${res.policy_identity.slice(0, 12)}` : ""}.`;
            statusEl.textContent = nextNotice;
            statusEl.hidden = false;
            try {
              const refreshed = validateAdmission(await state.context.api("/api/implementation/admission"));
              if (current(state) && state.admissionRoot) state.admissionRoot.replaceChildren(admissionPanel(state, refreshed, nextNotice));
            } catch (_) {}
          } catch (err) {
            errorEl.textContent = `Fair-admission update refused (${err?.status || "error"}): ${text(err?.message) || "owner rejected policy update"}`;
            errorEl.hidden = false;
          } finally {
            submitBtn.disabled = false;
          }
        });
        policyWrap.append(form, statusEl, errorEl);
      }
      panel.append(policyWrap);
    }

    // ServingBounds table
    const servingRows = admission.serving.map((s) => {
      const b = record(s.bounds) ? s.bounds : {};
      return [
        text(s.provider) || "not recorded",
        text(s.scope) || text(b.scope) || "not recorded",
        `${count(b.max_ongoing) ?? "not recorded"} / ${count(b.max_queued) ?? "not recorded"}`,
        text(s.readiness) || "not recorded",
        text(s.origin) || "not recorded",
        count(s.remaining_slots) === null ? "not recorded" : String(s.remaining_slots),
        count(s.queued) === null ? "not recorded" : String(s.queued),
        `${count(s.source_generation) ?? "not recorded"} / ${count(s.deployment_generation) ?? "not recorded"}`,
      ];
    });
    panel.append(
      node("h3", "ServingBounds & availability snapshots"),
      dataTable(
        ["Provider", "Aggregate scope", "Bounds (ongoing / queued)", "Readiness", "Origin", "Remaining slots", "Queued", "Gen (source / deploy)"],
        servingRows,
        "No serving-owner snapshots retained.",
      ),
    );

    // Live AttemptMetrics effort summaries
    const metricsRows = admission.attempt_metrics.map((m) => [
      `${text(m.attempt_id).slice(0, 12) || "not recorded"} (fence ${count(m.fence) ?? "?"})`,
      `${text(m.phase) || "not recorded"}${m.open ? " · open" : ""}`,
      text(m.telemetry_source) || "not_recorded",
      formatEffortMetric(m.tokens, m.tokens_status, m.tokens_absence_reason),
      formatEffortMetric(m.model_turns, m.model_turns_status, m.model_turns_absence_reason),
      formatEffortMetric(m.auxiliary_calls, m.auxiliary_calls_status, m.auxiliary_calls_absence_reason),
      m.wall_time_ms === null || m.wall_time_ms === undefined || !Number.isSafeInteger(m.wall_time_ms) ? "not recorded" : `${m.wall_time_ms.toLocaleString("en-US")} ms`,
      `${text(m.harness) || "not recorded"} / ${text(m.model) || "not recorded"}`,
      text(m.verification_outcome) || "not recorded",
    ]);
    panel.append(
      node("h3", "Live AttemptMetrics effort burn"),
      dataTable(
        ["Attempt", "Phase", "Telemetry source", "Tokens", "Model turns", "Auxiliary calls", "Wall time", "Harness / Model", "Verification"],
        metricsRows,
        "No attempt effort metrics recorded.",
      ),
    );

    if (Array.isArray(admission.non_claims) && admission.non_claims.length) {
      const nc = node("details");
      nc.append(node("summary", "Admission non-claims"));
      for (const item of admission.non_claims) nc.append(node("p", text(item), "small"));
      panel.append(nc);
    }
    if (!admission.configured || !admission.policy) {
      const box = node("details", undefined, "panel fleet-secondary-disclosure");
      box.dataset.fleetDisclosure = "admission";
      box.hidden = true;
      box.append(node("summary", "Fair-admission governance (not configured)"), panel);
      return box;
    }
    return panel;
  }

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
    if (!work.ranked.length && !work.blocked.length) {
      panel.hidden = true;
      panel.append(empty("No queued or blocked implementation work", "The Fleet owner returned no ranked or blocked work entries."));
    } else {
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
    const box = node("details", undefined, "panel fleet-secondary-disclosure");
    box.dataset.fleetDisclosure = "targets";
    box.hidden = true;
    box.append(node("summary", "Configured execution targets (CI slots — also in Workers tab)"), panel);
    return box;
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
    const workRoot = node("div", undefined, "fleet-capacity-region");
    const workerRoot = node("div", undefined, "fleet-capacity-region");
    const admissionRoot = node("div", undefined, "fleet-capacity-region");
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
    if (context?.capabilities?.inventory === false) exactPanel.hidden = true;
    page.append(workRoot, workerRoot, admissionRoot, exactPanel);
    root.replaceChildren(page);
    Object.assign(state, { admissionRoot, workRoot, workerRoot, exact, input, status });
    admissionRoot.replaceChildren(node("p", "Reading fair-admission governance…", "small"));
    workRoot.replaceChildren(node("p", "Reading Fleet work…", "small"));
    workerRoot.replaceChildren(node("p", "Reading configured targets…", "small"));
    const requests = [
      ["Fair-admission governance unavailable", admissionRoot, "/api/implementation/admission", validateAdmission, admissionPanel],
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

  window.KilnFleetCapacity = Object.freeze({ mount, clear, validateAdmission, admissionPanel, formatEffortMetric, formatAmounts });
})();
