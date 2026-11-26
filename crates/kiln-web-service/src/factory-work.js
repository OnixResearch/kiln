(() => {
  "use strict";
  // r[impl kiln.web.factory_control_views] Names are presentation, never joins or effect authority.
  const WORK = "/api/fleet/work";
  const ATTEMPTS = "/api/fleet/attempts";
  const DECISIONS = "/api/fleet/decisions";
  const CAIRN = "/api/implementation/cairn";
  const DETAIL_LIMIT = 8;
  const SOURCE_KINDS = new Map([["github_issue", ["github", "issue"]], ["cairn_task", ["cairn", "task"]], ["cairn_artifact", ["cairn", "artifact"]]]);
  const str = (value) => typeof value === "string" ? value : "";
  const id = (value) => /^[0-9a-f]{64}$/.test(str(value));
  const fence = (value) => Number.isSafeInteger(value) && value >= 0;
  const list = (value) => Array.isArray(value) ? value : [];
  const label = (value) => str(value).replaceAll("_", " ") || "Not recorded";
  const el = (tag, text, className) => {
    const node = document.createElement(tag);
    if (text !== undefined) node.textContent = text;
    if (className) node.className = className;
    return node;
  };
  const note = (parent, text, warning = false) => parent.append(el("p", text, warning ? "warning" : "small"));
  const code = (value) => el("code", value);
  const cockpit = (work) => ({ page: "cockpit", work_id_exact: work, attempt_id_exact: "", cairn_repository: "", cairn_change: "", cairn_unit: "" });
  const panel = (parent, title, heading = "h3") => {
    const node = el("section", undefined, "record-card view-stack");
    node.append(el(heading, title));
    parent.append(node);
    return node;
  };
  function link(ctx, patch, text) {
    if (ctx.link) return ctx.link(patch, text, "row-link");
    const node = el("a", text, "row-link");
    node.href = `?${new URLSearchParams(Object.entries(patch).filter(([, value]) => value))}`;
    return node;
  }
  function facts(parent, fields) {
    const node = el("dl", undefined, "facts");
    for (const [name, value] of fields) {
      const dd = el("dd");
      dd.append(value ?? "Not recorded");
      node.append(el("dt", name), dd);
    }
    parent.append(node);
  }
  function admitRows(rows, key, valid) {
    const indexed = new Map();
    const seen = new Set();
    let incomplete = false;
    for (const row of rows) {
      const identity = row?.[key];
      if (!id(identity)) { incomplete = true; continue; }
      if (seen.has(identity)) { incomplete = true; indexed.delete(identity); continue; }
      seen.add(identity);
      if (!valid(row)) { incomplete = true; continue; }
      indexed.set(identity, row);
    }
    return { indexed, incomplete, seen };
  }
  async function read(ctx, path, arrays) {
    try {
      const value = await ctx.api(path, { quiet: true });
      if (!arrays.every((field) => Array.isArray(value?.[field]))) return null;
      return value;
    } catch (_) { return null; }
  }
  function workRows(data) {
    return admitRows([...list(data?.ranked).map((row) => ({ ...row, eligibility: "Ranked" })),
      ...list(data?.blocked).map((row) => ({ ...row, eligibility: "Blocked" }))], "identity", () => true);
  }
  function attemptRows(data) {
    return admitRows(list(data?.attempts), "attempt", (row) => id(row.work) && fence(row.fence) && row.fence > 0);
  }
  function admittedAliases(entries) {
    const exact = new Map();
    for (const entry of entries) for (const ref of list(entry.sources)) {
      const kind = SOURCE_KINDS.get(ref?.kind);
      const nativeId = str(ref?.native_id);
      if (!kind || !nativeId || !id(ref.version) || ref.source?.contract_version !== 1
        || ref.source.provider_namespace !== kind[0] || ref.source.native_kind !== kind[1] || ref.source.native_id !== nativeId) continue;
      const issue = ref.kind === "github_issue" && /^([a-z0-9-]+)\/([a-z0-9_.-]+)#([1-9][0-9]*)$/.exec(nativeId);
      const href = issue && ![".", ".."].includes(issue[2]) && `${issue[1]}/${issue[2]}` === entry.repository
        ? `https://github.com/${issue[1]}/${issue[2]}/issues/${issue[3]}` : "";
      const key = JSON.stringify([ref.kind, nativeId, ref.version]);
      const prior = exact.get(key);
      exact.set(key, { kind: ref.kind, nativeId, version: ref.version, href: prior?.href === "" ? "" : href });
    }
    return [...exact.values()];
  }
  function sourceFacts(target, aliases) {
    if (!aliases.length) { note(target, "No exact source aliases were observed for this work."); return; }
    const rows = el("ul");
    for (const alias of aliases) {
      const row = el("li");
      const source = alias.href ? el("a", alias.nativeId) : code(alias.nativeId);
      if (alias.href) source.href = alias.href;
      row.append(`${label(alias.kind)} · `, source, " · Version ", code(alias.version));
      rows.append(row);
    }
    target.append(rows);
  }
  function cairnPatch(route, unit = "") {
    return { cairn_repository: route.repository, cairn_change: route.change, cairn_unit: unit };
  }
  async function cairnDetails(ctx, { aliases = [], attempts, wanted, explicit, operations = false }) {
    const cairn = window.KilnCairn;
    if (!cairn) return { details: [], incomplete: true };
    const index = await read(ctx, CAIRN, ["changes"]);
    if (!ctx.isCurrent()) return { details: [], incomplete: true };
    const routes = new Map();
    const aliasChanges = new Set(aliases.filter(({ kind }) => kind.startsWith("cairn_")).map(({ nativeId }) => nativeId.split("/", 1)[0]));
    const add = (route) => routes.set(JSON.stringify([route.repository, route.change]), route);
    if (explicit?.change) add(explicit);
    let incomplete = !index;
    if (index) {
      try {
        const rows = cairn.indexRows(index);
        incomplete ||= index.truncated === true || rows.length !== index.changes.length || index.snapshot?.work_recorded !== true || index.snapshot?.attempts_recorded !== true;
        for (const row of rows) {
          const assignment = list(row.activeAssignments).some((item) => {
            const match = attempts.get(item?.attempt);
            return (match && match.fence === item.fence) || (wanted && item?.attempt === wanted);
          });
          const recovery = row.firstBlocker?.condition === "dispatch_recovery";
          // A register row locates a possible change. Only the exact detail may bind work or an attempt.
          if (aliasChanges.has(row.change) || assignment || (operations && recovery && (!wanted || row.firstBlocker.observation === `attempt:${wanted}`))) add(row);
        }
      } catch (_) { incomplete = true; }
    }
    const details = await Promise.all([...routes.values()].slice(0, DETAIL_LIMIT).map(async (route) => {
      try { return cairn.admitDetail(await ctx.api(cairn.detailPath(route), { quiet: true }), route); }
      catch (_) { return null; }
    }));
    return { details: details.filter(Boolean), incomplete: incomplete || routes.size > DETAIL_LIMIT || details.some((detail) => !detail
      || detail.snapshot?.work_recorded !== true || detail.snapshot?.attempts_recorded !== true) };
  }
  function cairnBindings(details, work, wanted = "") {
    const units = [];
    const attempts = new Map();
    const conflicts = new Set();
    const crossed = new Set();
    for (const detail of details) for (const unit of detail.units) {
      const route = { repository: detail.repository_identity, change: detail.change };
      const nativeId = str(unit?.native_id);
      const patch = cairnPatch(route, nativeId);
      if (window.KilnCairn.normalizeRoute(patch).unit !== nativeId || !nativeId) continue;
      const bound = work && [...list(unit.current_work), ...list(unit.superseded_work)].some((row) => row?.identity === work);
      let matched = false;
      for (const attempt of list(unit.attempts)) {
        if (!id(attempt?.identity) || !id(attempt.work_identity) || !fence(attempt.fence) || attempt.fence === 0
          || (wanted && attempt.identity !== wanted)) continue;
        if (work && attempt.work_identity !== work) { crossed.add(attempt.identity); continue; }
        // A repeated attempt across aliases may carry the same observation; a crossed binding may not.
        matched = true;
        const prior = attempts.get(attempt.identity);
        if (!prior) attempts.set(attempt.identity, { attempt, patch });
        else if (JSON.stringify(prior.attempt) !== JSON.stringify(attempt)) conflicts.add(attempt.identity);
      }
      if (bound || matched) units.push({ nativeId, patch });
    }
    for (const conflict of conflicts) attempts.delete(conflict);
    return { units, attempts, conflicts, crossed };
  }
  const exactMatch = (row, attempt) => row.work === attempt.work_identity && row.fence === attempt.fence;
  function verificationRun(ctx, run, title, view) {
    if (ctx.runLink) return ctx.runLink(run, title, view);
    return link(ctx, { page: "runs", run, view, node: "", stream: "", cursor: "" }, title);
  }
  const refText = (ref) => str(ref?.native_id) && str(ref?.provider_namespace) && str(ref?.native_kind)
    ? `${ref.provider_namespace} · ${ref.native_kind} · ${ref.native_id}` : null;
  function attemptCard(target, ctx, identity, work, row, retained) {
    const evidence = retained?.attempt;
    const card = panel(target, "Attempt", "h4");
    const session = row?.session?.attempt === identity && row.session.fence === row.fence ? row.session : null;
    const recordedFence = row?.fence ?? evidence.fence;
    facts(card, [["Identity", code(identity)], ["Fence", String(recordedFence)], ["Kiln phase (Fleet)", label(row?.phase)],
      ["Session machine", str(session?.machine) || null], ["Session observation", session && row.observation_stale === true ? "Stale" : null]]);
    const recover = { page: "operations", work_id_exact: work, attempt_id_exact: identity, ...(retained?.patch || cairnPatch({ repository: "", change: "" })) };
    card.append(link(ctx, recover, "Inspect this attempt in Operations"));
    if (!evidence) { note(card, "No matching Cairn attempt evidence was admitted."); return; }
    facts(card, [["Kiln phase (Cairn projection)", label(evidence.phase)], ["Work revision", label(evidence.applicability)],
      ["Dispatch", label(evidence.dispatch)], ["Unknown outcome", str(evidence.unknown_outcome) ? label(evidence.unknown_outcome) : null],
      ["Provider execution", refText(evidence.workflow)], ["Candidate", refText(evidence.candidate)],
      ["Independent verification", evidence.verification?.success === true ? "Passed" : evidence.verification?.success === false ? "Failed" : null],
      ["Publication", label(evidence.publication?.phase)], ["Pull request reference", refText(evidence.publication?.pull_request)],
      ["Lifecycle receipt", evidence.lifecycle?.binding === "matches_attempt" ? refText(evidence.lifecycle.lifecycle_receipt) : evidence.lifecycle ? "Crossed receipt refused" : null]]);
    const run = str(evidence.verification?.run_id);
    if (run) card.append(verificationRun(ctx, run, "Exact verification run", "pipeline"), " · ", verificationRun(ctx, run, "Exact verification logs", "logs"));
  }
  function showDecisions(target, data, attempts) {
    if (!data) { note(target, "Fleet decisions unavailable.", true); return; }
    const { indexed, incomplete } = admitRows(data.decisions, "identity", (row) => id(row.attempt) && id(row.session));
    if (incomplete) note(target, "Some decision records were refused as incomplete or ambiguous.", true);
    const rows = el("ul");
    let count = 0;
    for (const decision of indexed.values()) {
      if (!attempts.has(decision.attempt)) continue;
      const row = el("li");
      const state = decision.answer ? "Answered" : fence(decision.closed_ms) ? "Closed" : decision.answer === null && decision.closed_ms === null ? "Open" : "State not recorded";
      row.append(`${state} · ${str(decision.question) || "Question not recorded"} · `, code(decision.identity));
      rows.append(row);
      count += 1;
    }
    if (count) target.append(rows);
    else note(target, "No decisions joined to the exact attempts in the bounded Fleet response.");
  }
  async function load(root, ctx) {
    const selected = str(ctx.route?.work_id_exact);
    root.replaceChildren();
    if (selected && !id(selected)) { note(root, "An exact Kiln work identity must be 64 lowercase hexadecimal characters.", true); return; }
    const snapshot = panel(root, selected ? "Exact Kiln work" : "Kiln work register");
    if (selected) snapshot.append(code(selected), link(ctx, cockpit(""), "All listed Kiln work"));
    note(snapshot, "Reading bounded Fleet observations…");
    const [workData, attemptData, decisionData] = await Promise.all([read(ctx, WORK, ["ranked", "blocked"]),
      read(ctx, ATTEMPTS, ["attempts"]), selected ? read(ctx, DECISIONS, ["decisions"]) : null]);
    if (!ctx.isCurrent()) return;
    snapshot.lastChild.remove();
    const work = workRows(workData);
    const fleet = attemptRows(attemptData);
    if (!workData || !attemptData) note(snapshot, `${!workData ? "Fleet work unavailable. " : ""}${!attemptData ? "Fleet attempts unavailable." : ""}`, true);
    if (work.incomplete || fleet.incomplete || workData?.truncated === true || attemptData?.truncated === true) note(snapshot, "Fleet coverage is partial; some records were not listed or could not be joined.", true);
    const exactAttempts = new Map([...fleet.indexed].filter(([, row]) => row.work === selected));
    if (!selected) {
      const register = new Map(work.indexed);
      for (const row of fleet.indexed.values()) if (!register.has(row.work)) register.set(row.work, row);
      if (!register.size) { note(snapshot, "No exact work identities in the available Fleet records."); return; }
      const rows = el("ul");
      for (const [identity, row] of register) {
        const item = el("li");
        item.append(link(ctx, cockpit(identity), str(row.title) || "Kiln work"), " · ", code(identity), ` · ${row.eligibility || "Observed in attempts"}`);
        rows.append(item);
      }
      snapshot.append(rows);
      return;
    }
    const current = work.indexed.get(selected);
    const joined = [...exactAttempts.values()];
    const aliases = admittedAliases([...(current ? [current] : []), ...joined]);
    if (current) {
      facts(snapshot, [["Title", str(current.title) || null], ["Repository", str(current.repository) || null], ["Eligibility listing", current.eligibility]]);
      for (const reason of list(current.reasons)) note(snapshot, `Blocker: ${label(reason?.state ?? reason)}${id(reason?.identity) ? ` · ${reason.identity}` : ""}`);
    } else note(snapshot, "This exact work revision is not in the available ranked or blocked listing; no current state is inferred.");
    sourceFacts(panel(root, "Source references"), aliases);
    const attemptPanel = panel(root, "Exact attempts");
    const cairnPanel = panel(root, "Cairn lineage");
    const decisionPanel = panel(root, "Fleet decisions");
    showDecisions(decisionPanel, decisionData, exactAttempts);
    for (const [identity, row] of exactAttempts) attemptCard(attemptPanel, ctx, identity, selected, row);
    if (!exactAttempts.size) note(attemptPanel, "No exact attempts joined in the available Fleet response.");
    note(cairnPanel, "Reading bounded Cairn observations…");
    const cairn = await cairnDetails(ctx, { aliases, attempts: exactAttempts, explicit: window.KilnCairn?.normalizeRoute(ctx.route) });
    if (!ctx.isCurrent()) return;
    cairnPanel.lastChild.remove();
    if (cairn.incomplete) note(cairnPanel, "Cairn observations are unavailable or incomplete; missing evidence does not indicate absence.", true);
    const retained = cairnBindings(cairn.details, selected);
    for (const unit of retained.units) cairnPanel.append(link(ctx, { page: "cairn", ...unit.patch }, unit.nativeId));
    if (!retained.units.length) note(cairnPanel, "No exact Cairn unit joined to this work revision in the available detail.");
    if (!retained.attempts.size && !retained.conflicts.size && ![...exactAttempts.keys()].some((identity) => retained.crossed.has(identity))) return;
    attemptPanel.replaceChildren(el("h3", "Exact attempts"));
    const identities = new Set([...exactAttempts.keys(), ...retained.attempts.keys(), ...retained.conflicts]);
    const joinedIdentities = new Set(exactAttempts.keys());
    for (const identity of identities) {
      const row = exactAttempts.get(identity), exact = retained.attempts.get(identity);
      if (retained.conflicts.has(identity) || retained.crossed.has(identity) || (exact && (row ? !exactMatch(row, exact.attempt) : fleet.seen.has(identity)))) {
        note(attemptPanel, `Conflicting observations for attempt ${identity}; Cairn evidence was refused.`, true);
        if (row) attemptCard(attemptPanel, ctx, identity, selected, row);
        continue;
      }
      attemptCard(attemptPanel, ctx, identity, selected, row, exact);
      joinedIdentities.add(identity);
    }
    decisionPanel.replaceChildren(el("h3", "Fleet decisions"));
    showDecisions(decisionPanel, decisionData, joinedIdentities);
  }
  async function recovery(root, ctx) {
    root.replaceChildren();
    const target = panel(root, "Dispatch recovery");
    const selected = str(ctx.route?.work_id_exact);
    const wanted = str(ctx.route?.attempt_id_exact);
    if ((selected && !id(selected)) || (wanted && (!id(wanted) || !selected))) {
      note(target, "Recovery selection requires an exact Kiln work identity and, when supplied, an exact attempt identity.", true);
      return;
    }
    note(target, "Eligibility is decided by the recovery owner after reading the exact retained request. This view never starts a replacement.");
    if (selected) target.append(link(ctx, cockpit(selected), "Open exact work in the cockpit"), " · ", link(ctx, { ...cockpit(""), page: "operations" }, "All recovery candidates"));
    if (ctx.canOperate !== true) { note(target, "Recovery evidence and actions require operator access."); return; }
    const rail = window.KilnFleet?.recoveryReadout;
    if (!rail) { note(target, "The operator recovery rail is unavailable.", true); return; }
    const pending = el("p", "Reading bounded attempt observations…", "small");
    target.append(pending);
    const data = await read(ctx, ATTEMPTS, ["attempts"]);
    if (!ctx.isCurrent()) return;
    const fleet = attemptRows(data);
    const matching = new Map([...fleet.indexed].filter(([identity, row]) => (!selected || row.work === selected) && (!wanted || identity === wanted)));
    if (!data) note(target, "Fleet attempts unavailable.", true);
    else if (fleet.incomplete || data.truncated === true) note(target, "Fleet attempt coverage is partial.", true);
    const explicit = window.KilnCairn?.normalizeRoute(ctx.route);
    const cairn = await cairnDetails(ctx, { aliases: selected ? admittedAliases([...matching.values()]) : [], attempts: selected ? matching : new Map(), wanted, explicit, operations: true });
    if (!ctx.isCurrent() || ctx.canOperate !== true) return;
    pending.remove();
    if (cairn.incomplete) note(target, "Cairn recovery observations are unavailable or incomplete.", true);
    const retained = cairnBindings(cairn.details, selected, wanted);
    const identities = new Set([...matching.keys(), ...retained.attempts.keys(), ...retained.conflicts, ...(wanted && retained.crossed.has(wanted) ? [wanted] : [])]);
    let count = 0;
    for (const identity of identities) {
      const row = matching.get(identity), exact = retained.attempts.get(identity), attempt = exact?.attempt;
      if (retained.conflicts.has(identity) || retained.crossed.has(identity) || (attempt && (row ? !exactMatch(row, attempt) : fleet.seen.has(identity)))) {
        note(target, `Conflicting observations for attempt ${identity}; no recovery control was attached.`, true);
        continue;
      }
      if (!wanted && !["claimed", "executing", "failed"].includes(row?.phase)
        && attempt?.unknown_outcome !== "unreconciled" && attempt?.dispatch !== "transport_started") continue;
      const bound = { identity, fence: row?.fence ?? attempt.fence, work_identity: row?.work ?? attempt.work_identity };
      const entry = el("details");
      entry.open = identity === wanted;
      entry.append(el("summary", `Attempt ${identity} · fence ${bound.fence}`), link(ctx, cockpit(bound.work_identity), `Work ${bound.work_identity}`));
      if (exact) entry.append(" · ", link(ctx, { page: "cairn", ...exact.patch }, "Exact Cairn unit"));
      if (attempt) note(entry, `Cairn dispatch: ${label(attempt.dispatch)}${attempt.unknown_outcome ? ` · Outcome ${label(attempt.unknown_outcome)}` : ""}.`);
      const readout = rail(ctx, bound);
      if (readout) entry.append(readout);
      else note(entry, "The exact attempt was not admitted by the operator recovery rail.", true);
      target.append(entry);
      count += 1;
    }
    if (!count) note(target, wanted ? "The selected work and attempt did not join in the available observations. No recovery control was attached."
      : "No recovery inspection candidates in the available observations. This is not a determination of provider outcomes.");
  }
  window.KilnFactoryWork = Object.freeze({ load, recovery });
})();
