// Read-only, owner-scoped observations for the Factory landing page.
(() => {
  "use strict";

  const ROW_LIMIT = 6;
  const CAIRN_DETAIL_LIMIT = 4;
  const CI_LIMIT = 12;
  const LEASE_WINDOW_MS = 15 * 60_000;
  const FLEET_DECISION_LIMIT = 256;
  const CAIRN_PATH = "/api/implementation/cairn";
  const HEX = /^[0-9a-f]{64}$/;
  const CI_RUN_ID = /^web-(?:run|nix-build|verification)-[0-9a-f]{64}$/;
  const CONTROL = /[\u0000-\u001f\u007f-\u009f]/u;
  const LANES = {
    attention: "Needs attention", awaiting: "Awaiting", ready: "Ready", in_progress: "In progress",
    verified: "Verified", integrated: "Integrated", recorded: "Recorded",
  };
  const CI_PHASES = { queued: "Queued", awaiting_approval: "Awaiting approval", active: "Active" };

  const record = (value) => value !== null && typeof value === "object" && !Array.isArray(value);
  const text = (value) => typeof value === "string" ? value : "";
  const count = (value) => Number.isSafeInteger(value) && value >= 0;
  const instant = (value) => count(value) && value <= 8.64e15;
  const hex = (value) => HEX.test(text(value));
  const label = (value) => text(value).replaceAll("_", " ");
  const finiteText = (value, max) => text(value).trim() !== "" && !CONTROL.test(value) && window.KilnUi.utf8Bytes(value) <= max;
  const changeKey = (entry) => record(entry) && hex(entry.repository_identity) && finiteText(entry.change, 256) && !entry.change.includes("/");
  const note = (value) => element("p", value, "small");

  function element(tag, value, className) {
    const node = document.createElement(tag);
    if (value !== undefined) node.textContent = value;
    if (className) node.className = className;
    return node;
  }

  function source(title, sourceLabel, heading = "h3") {
    const section = element("section", undefined, "panel view-stack");
    const status = note("");
    status.setAttribute("role", "status");
    const body = element("div", undefined, "view-stack");
    section.append(element(heading, title), status, body);
    const report = (state, message) => {
      status.dataset.state = state;
      status.textContent = `${sourceLabel} · ${message}`;
    };
    report("loading", "Reading…");
    return { section, body, report };
  }

  function rows(body, entries, render, empty) {
    if (!entries.length) {
      body.append(note(empty));
      return;
    }
    const list = element("ul");
    for (const entry of entries.slice(0, ROW_LIMIT)) {
      const item = element("li");
      render(item, entry);
      list.append(item);
    }
    body.append(list);
    if (entries.length > ROW_LIMIT) body.append(note(`Showing ${ROW_LIMIT} of ${entries.length} matching rows in this response.`));
  }

  function timeLine(item, prefix, ms, now) {
    const line = note(prefix);
    line.append(instant(ms) ? window.KilnUi.timeElement(ms) : "time not recorded");
    if (now !== undefined) {
      line.append(instant(ms) && ms <= now ? ` · Age: ${window.KilnUi.formatDuration(now - ms)}` : " · Age unavailable");
    }
    item.append(line);
  }

  function identifier(item, identity) {
    const id = element("code", identity.slice(-64, -52));
    id.title = identity;
    item.append(" · ", id);
  }

  function cairnLink(ctx, row, title = row.change, unit = "") {
    return ctx.link({ page: "cairn", cairn_repository: row.repository_identity, cairn_change: row.change, cairn_unit: unit,
      cairn_q: "", cairn_lane: "", cairn_blocked: "", cairn_decisions: "" }, title);
  }

  function fleetRegister(ctx, body, name) {
    body.append(ctx.link({ page: "fleet" }, `Open ${name} in Fleet (register)`));
  }

  function runLink(ctx, row) {
    return ctx.link({ page: "runs", view: "pipeline", run: row.id, node: "", stream: "", project: "", outcome: "", source: "", q: "", cursor: "" }, text(row.project_id) || "CI run");
  }

  function checkedList(value, key, valid) {
    if (!record(value) || !Array.isArray(value[key]) || !value[key].every(valid)) throw new Error("Invalid source response");
    return value[key];
  }

  function claims(panel, value, ctx, now) {
    // The coordinator returns live leases. The display window is relative to the browser clock at receipt.
    const listed = checkedList(value, "claims", (row) => record(row) && hex(row.identity) && row.state === "open");
    const expiring = listed.filter((row) => instant(row.expires_ms) && row.expires_ms > now && row.expires_ms - now <= LEASE_WINDOW_MS)
      .sort((a, b) => a.expires_ms - b.expires_ms);
    const uncertain = listed.filter((row) => !instant(row.expires_ms) || row.expires_ms <= now).length;
    const mirrors = listed.filter((row) => row.claim_mirror === "unknown" || row.release_mirror === "unknown");
    panel.report(uncertain ? "partial" : "available", uncertain ? "Live claim list received; some expiries cannot be evaluated" : "Live claim list received");
    panel.body.append(note(`${expiring.length} listed leases expire within 15 minutes of this read (browser clock).`));
    if (uncertain) panel.body.append(note(`${uncertain} listed leases have missing expiry times or already-passed expiries on this browser clock.`));
    rows(panel.body, expiring, (item, row) => {
      const scope = row.scope;
      const target = text(scope?.target?.repository) || text(scope?.target?.machine);
      const path = text(scope?.path?.path);
      item.append(target ? [target, path].filter(Boolean).join(" · ") : "Scope not recorded");
      identifier(item, row.identity);
      if (text(row.holder?.host)) item.append(note(`Holder host: ${row.holder.host}`));
      timeLine(item, "Expires ", row.expires_ms);
    }, uncertain ? "No evaluated lease was in the warning window." : "No listed leases are in the warning window.");
    if (mirrors.length) {
      panel.body.append(element("h4", "Unknown claim mirror effects"));
      rows(panel.body, mirrors, (item, row) => {
        const unknown = [row.claim_mirror === "unknown" ? "claim post" : "", row.release_mirror === "unknown" ? "release post" : ""].filter(Boolean);
        item.append(`Outcome unknown: ${unknown.join(" and ")}`);
        identifier(item, row.identity);
      }, "");
    }
    fleetRegister(ctx, panel.body, "claims");
  }

  function fleetDecisions(panel, value, ctx, now) {
    // This owner returns at most 256 decisions, newest raised first, with no truncation flag.
    const listed = checkedList(value, "decisions", (row) => record(row) && hex(row.identity) && Object.hasOwn(row, "answer") && Object.hasOwn(row, "closed_ms"));
    const open = listed.filter((row) => row.answer === null && row.closed_ms === null)
      .sort((a, b) => (instant(a.raised_ms) ? a.raised_ms : Infinity) - (instant(b.raised_ms) ? b.raised_ms : Infinity));
    const atBound = listed.length >= FLEET_DECISION_LIMIT;
    panel.report(atBound ? "partial" : "available", atBound ? "Recent decision list at read bound; older decisions may be omitted" : "Decision list received");
    panel.body.append(note(`${open.length} open decisions in ${listed.length} listed records.`));
    rows(panel.body, open, (item, row) => {
      item.append(text(row.question) || "Question not recorded");
      identifier(item, row.identity);
      timeLine(item, "Raised ", row.raised_ms, now);
    }, "No open decisions in the returned list.");
    fleetRegister(ctx, panel.body, "decisions");
  }

  function fleetWork(panel, value, ctx) {
    const blocked = checkedList(value, "blocked", (row) => record(row) && hex(row.identity) && Array.isArray(row.reasons));
    const ranked = checkedList(value, "ranked", (row) => record(row) && hex(row.identity));
    if (!count(value.ready_unclaimed) || typeof value.truncated !== "boolean") throw new Error("Invalid Fleet work response");
    panel.report(value.truncated ? "partial" : "available", value.truncated ? "Work response truncated; ranked or blocked rows may be omitted" : "Work response received");
    panel.body.append(note(`${value.ready_unclaimed} ready and unclaimed (coordinator count) · ${ranked.length} listed ranked · ${blocked.length} listed blocked.`));
    rows(panel.body, blocked, (item, row) => {
      item.append(ctx.link({ page: "cockpit", work_id_exact: row.identity, attempt_id_exact: "" }, text(row.title) || row.identity));
      if (text(row.repository)) item.append(note(row.repository));
      const reasons = row.reasons.map((reason) => {
        const kind = label(reason?.state);
        return reason?.state === "missing_dependency" && hex(reason.identity) ? `${kind}: ${reason.identity}` : kind;
      }).filter(Boolean);
      item.append(note(reasons.length ? reasons.join(" · ") : "Block reasons not recorded."));
    }, "No blocked work in the returned list.");
    panel.body.append(note("Fleet work does not report when eligibility changed; blocked age is not available."));
  }

  function cairnDetail(panel, value, row, ctx, now) {
    if (value?.schema !== "kiln.cairn-workspace-detail.v4" || value.repository_identity !== row.repository_identity || value.change !== row.change) {
      throw new Error("Cairn detail does not match its requested change");
    }
    const units = checkedList(value, "units", (unit) => record(unit) && Array.isArray(unit.attempts));
    const memberships = checkedList(value, "memberships", (member) => record(member) && Array.isArray(member.pending_effects));
    const decisions = checkedList(value, "decisions", (decision) => record(decision) && hex(decision.identity) && text(decision.state));
    const recovery = units.flatMap((unit) => unit.attempts.filter((attempt) => attempt?.unknown_outcome === "unreconciled")
      .map((attempt) => ({ unit: unit.native_id, attempt: attempt.identity })));
    const effects = memberships.flatMap((member) => member.pending_effects.filter((effect) => effect?.state === "unknown")
      .map((effect) => ({ ...effect, campaign_id: member.campaign_id })));
    const open = decisions.filter((decision) => decision.state === "unissued" || decision.state === "outstanding");
    const snapshot = value.snapshot;
    const partial = snapshot?.attempts_recorded !== true || snapshot?.campaigns_truncated !== false || snapshot?.lifecycle_truncated !== false;
    panel.report(partial ? "partial" : "available", partial ? "Detail received with incomplete attempt or campaign observations" : "Detail received");
    rows(panel.body, [
      ...recovery.map((entry) => ({ ...entry, kind: "dispatch" })),
      ...effects.map((entry) => ({ ...entry, kind: "effect" })),
      ...open.map((entry) => ({ ...entry, kind: "decision" })),
    ], (item, entry) => {
      if (entry.kind === "dispatch") {
        item.append("Unreconciled dispatch outcome");
        if (hex(entry.attempt)) identifier(item, entry.attempt);
        if (finiteText(entry.unit, 1024) && entry.unit.startsWith(`${row.change}/`) && entry.unit.length > row.change.length + 1) {
          item.append(" · ", cairnLink(ctx, row, "Open affected unit", entry.unit));
        }
      } else if (entry.kind === "effect") {
        item.append(`Unknown effect outcome · ${label(entry.operation) || "operation not recorded"} · ${text(entry.owner) || "owner not recorded"}`);
        if (hex(entry.effect_id)) identifier(item, entry.effect_id);
        if (text(entry.campaign_id)) item.append(note(`Campaign: ${entry.campaign_id}`));
      } else {
        item.append(`Open campaign decision (${label(entry.state)}): ${text(entry.question) || "question not recorded"}`);
        identifier(item, entry.identity);
        timeLine(item, "Created ", entry.created_ms, now);
        if (text(entry.campaign_id)) item.append(note(`Campaign: ${entry.campaign_id}`));
      }
    }, "No unreconciled dispatches, unknown effects or open decisions in this detail response.");
  }

  function cairnIndex(panel, value, ctx, read) {
    if (value?.schema !== "kiln.cairn-workspace-index.v2" || typeof value.truncated !== "boolean" || !record(value.snapshot)) throw new Error("Invalid Cairn index");
    const entries = checkedList(value, "changes", changeKey);
    const flags = value.snapshot;
    const incomplete = [
      value.truncated ? "change index truncated" : "",
      flags.campaigns_truncated !== false ? "campaign coverage incomplete" : "",
      flags.lifecycle_truncated !== false ? "lifecycle coverage incomplete" : "",
      flags.work_recorded !== true ? "work observation not recorded" : "",
      flags.attempts_recorded !== true ? "attempt observation not recorded" : "",
      entries.some((row) => !count(row.blockers) || !count(row.open_decisions) || !Object.hasOwn(LANES, row.lane)) ? "some change fields unrecorded" : "",
    ].filter(Boolean);
    panel.report(incomplete.length ? "partial" : "available", incomplete.length ? `Index received; ${incomplete.join("; ")}` : "Index received");
    const stageCounts = new Map();
    for (const entry of entries) if (Object.hasOwn(LANES, entry.lane)) stageCounts.set(entry.lane, (stageCounts.get(entry.lane) ?? 0) + 1);
    panel.body.append(note(`${entries.length} listed changes${stageCounts.size ? ` · ${Object.keys(LANES).filter((lane) => stageCounts.has(lane)).map((lane) => `${LANES[lane]}: ${stageCounts.get(lane)}`).join(" · ")}` : ""}.`));
    const attention = entries.filter((row) => (count(row.blockers) && row.blockers > 0) || (count(row.open_decisions) && row.open_decisions > 0) || row.lane === "attention");
    const recovery = (row) => Number(row.first_blocker?.condition === "dispatch_recovery");
    attention.sort((a, b) => recovery(b) - recovery(a) || Number(b.lane === "attention") - Number(a.lane === "attention"));
    panel.body.append(note(`Exact recovery and decision detail is requested for ${Math.min(attention.length, CAIRN_DETAIL_LIMIT)} of ${attention.length} listed attention changes. The index alone does not enumerate effects or individual decisions.`));
    const detailReads = [];
    rows(panel.body, attention, (item, row) => {
      item.append(cairnLink(ctx, row));
      if (Array.isArray(row.projects) && row.projects.some(text)) item.append(note(row.projects.map(text).filter(Boolean).join(", ")));
      item.append(note(`${count(row.blockers) ? row.blockers : "Unknown"} listed blockers · ${count(row.open_decisions) ? row.open_decisions : "Unknown"} listed open decisions`));
      if (text(row.first_blocker?.condition)) {
        item.append(note(`First blocker: ${label(row.first_blocker.condition)}${text(row.first_blocker.detail) ? ` · ${row.first_blocker.detail}` : ""}`));
      }
      if (detailReads.length < CAIRN_DETAIL_LIMIT) {
        const detail = source("Recovery and decisions", "Cairn detail", "h4");
        item.append(detail.section);
        const path = `${CAIRN_PATH}/detail?repository=${row.repository_identity}&change=${encodeURIComponent(row.change)}`;
        detailReads.push(read(detail, path, (result, now) => cairnDetail(detail, result, row, ctx, now)));
      }
    }, "No changes in this index report attention, blockers or open decisions.");
    panel.body.append(ctx.link({ page: "cairn", cairn_repository: "", cairn_change: "", cairn_unit: "", cairn_q: "", cairn_lane: "", cairn_blocked: "", cairn_decisions: "" }, "Open Cairn change register"));
    return Promise.all(detailReads);
  }

  function ciRuns(panel, value, ctx, now, outcome) {
    const listed = checkedList(value, "runs", (row) => record(row) && CI_RUN_ID.test(text(row.id)) && row.outcome === outcome);
    if (!count(value.total) || value.total < listed.length || value.bounded !== true || (value.next_cursor !== null && !text(value.next_cursor))) {
      throw new Error("Invalid CI run page");
    }
    const more = value.next_cursor !== null;
    panel.report(more ? "partial" : "available", more ? "First bounded page received; more retained runs match" : "Retained run page received");
    const pending = outcome === "Pending";
    panel.body.append(note(`${value.total} matching retained runs · ${listed.length} on this page.`));
    if (pending && listed.length) {
      const phases = new Map();
      for (const row of listed) {
        const phase = Object.hasOwn(CI_PHASES, row.phase) ? CI_PHASES[row.phase] : "Other or unrecorded phase";
        phases.set(phase, (phases.get(phase) ?? 0) + 1);
      }
      panel.body.append(note(`On this page: ${[...phases].map(([phase, total]) => `${phase}: ${total}`).join(" · ")}.`));
    }
    rows(panel.body, listed, (item, row) => {
      item.append(runLink(ctx, row));
      identifier(item, row.id);
      if (pending) {
        const phase = Object.hasOwn(CI_PHASES, row.phase) ? CI_PHASES[row.phase] : label(row.phase) || "Phase not recorded";
        item.append(` · ${phase}`);
        if (row.phase === "active") timeLine(item, "Started ", row.started_at_ms, now);
        else timeLine(item, "Requested ", row.created_at_ms, now);
      } else {
        item.append(" · Run outcome unknown");
        if (text(row.phase)) item.append(note(`Recorded phase: ${label(row.phase)}`));
      }
    }, "No matching runs in this retained CI page.");
    panel.body.append(ctx.link({ page: "runs", outcome, run: "", project: "", q: "", source: "", cursor: "", order: "desc" }, `Open ${pending ? "pending" : "unknown-outcome"} CI runs`));
  }

  // r[impl kiln.web.factory_control_views]
  // r[impl kiln.web.factory_control_views.scenario.owner_unavailable]
  async function load(root, ctx) {
    if (!ctx.isCurrent()) return;
    const dashboard = element("div", undefined, "view-stack");
    const triage = element("section", undefined, "view-stack");
    const flow = element("section", undefined, "view-stack");
    const claimPanel = source("Expiring claims", "Fleet claims");
    const decisionPanel = source("Open decisions", "Fleet decisions");
    const workPanel = source("Blocked work and readiness", "Fleet work");
    const cairnPanel = source("Change stages and attention", "Cairn index");
    const pendingPanel = source("Pending CI runs", "CI run list");
    const unknownPanel = source("Unknown CI outcomes", "CI run list");
    triage.append(element("h2", "Factory triage"), note("Owner observations are read independently. They do not form a single current snapshot; use the linked register to act."), claimPanel.section, decisionPanel.section, unknownPanel.section);
    flow.append(element("h2", "Factory flow"), note("These counts describe different source populations; they are not combined. Ages use recorded timestamps and this browser’s clock at receipt."), workPanel.section, cairnPanel.section, pendingPanel.section);
    dashboard.append(triage, flow);
    root.replaceChildren(dashboard);

    const current = () => ctx.isCurrent() && dashboard.parentNode === root;
    const read = async (panel, path, render) => {
      try {
        const value = await ctx.api(path, { quiet: true });
        if (current()) await render(value, Date.now());
      } catch (error) {
        if (!current() || error?.name === "AbortError") return;
        panel.body.replaceChildren(note("No observation could be shown; counts and absence are not inferred."));
        panel.report("unavailable", "Unavailable");
      }
    };
    await Promise.all([
      read(claimPanel, "/api/fleet/claims", (value, now) => claims(claimPanel, value, ctx, now)),
      read(decisionPanel, "/api/fleet/decisions", (value, now) => fleetDecisions(decisionPanel, value, ctx, now)),
      read(workPanel, "/api/fleet/work", (value) => fleetWork(workPanel, value, ctx)),
      read(cairnPanel, CAIRN_PATH, (value) => cairnIndex(cairnPanel, value, ctx, read)),
      read(pendingPanel, `/api/runs?outcome=pending&limit=${CI_LIMIT}`, (value, now) => ciRuns(pendingPanel, value, ctx, now, "Pending")),
      read(unknownPanel, `/api/runs?outcome=unknown&limit=${CI_LIMIT}`, (value, now) => ciRuns(unknownPanel, value, ctx, now, "Unknown")),
    ]);
  }

  window.KilnFactoryDashboard = Object.freeze({ load });
})();
