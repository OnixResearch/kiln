"use strict";

// Patchgate receipts (?page=reviews) and a run's Reviews tab: one presentation of retained kiln.patchgate.review.v1
// receipts. The page reads only /api/reviews and /api/reviews/NAME through the shell's api helper; the run tab draws
// the /api/runs/ID/reports result the shell already read. Every decision comes from Kiln's admitted observation.
// Facts read from Patchgate's native report are labeled as recorded by Patchgate and never replace a decision. All
// text is written through textContent, never parsed as markup. Nothing here dispatches Patchgate or grants authority.
// r[impl kiln.dashboard.reviews.view]
(() => {
  const SCHEMA = "kiln.patchgate.review.v1";
  // Typing in the filter narrows the list at once and commits it to the URL after this pause, as on the Source page.
  const FILTER_COMMIT_MS = 350;
  const FILTER_MAX_CHARS = 256;
  // Long hashes and identities read in this short form where space is tight; the full value stays beside it.
  const SHORT_HASH = 12;
  // A change lists this many paths, then says how many more were recorded.
  const PATH_PREVIEW = 8;
  // An unrecognized decision is quoted in its label up to this length; the full text stays in the title.
  const LABEL_TEXT_MAX = 48;
  // The list shows this many decision pills per receipt, then a count of the rest.
  const ROW_PILLS = 3;
  // RFC 3339 with an explicit offset, as Patchgate records decision times; anything else is not read as an instant.
  const RFC3339 = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?(?:Z|[+-]\d{2}:\d{2})$/;

  // ---------- Decisions ----------

  // r[impl kiln.dashboard.reviews.view.scenario.decisions] The nine decisions Kiln admits (kiln_core::review::ReviewDecision)
  // are Patchgate's own change states (patchgate gate.rs, State). Meanings follow Patchgate's state model and README;
  // next steps follow `patchgate explain`. Tones: success accepted, failure rejected, warning for the two that need a
  // person or a prerequisite decision, queued for not decided yet, cancelled for taken out of main.
  const DECISIONS = Object.freeze([
    ["accepted", "Accepted", "success", "Patchgate accepted the change.", ""],
    ["rejected", "Rejected", "failure", "Patchgate rejected the change. It stays in the inbox and is not applied to main.", "Remove or replace the change in the inbox, then record a corrected change."],
    ["escalated", "Escalated", "warning", "A person must decide. Patchgate escalates changes to protected paths, conflicts it does not merge, changes too large to review and reviews that do not settle.", "Resolve the change in a separate workspace, or reject it."],
    ["blocked", "Blocked", "warning", "A prerequisite was rejected, escalated, removed or blocked, or its state is unknown, so Patchgate will not apply this change.", "Decide the prerequisites first."],
    ["waiting", "Waiting", "queued", "A prerequisite is not in main yet, so the change waits for it.", "Decide the prerequisites first."],
    ["pending", "Pending", "queued", "Patchgate has not decided this change yet.", "Review the change."],
    ["removed", "Removed", "cancelled", "Patchgate accepted the change before, but it is no longer in main.", "Examine the removal from main before deciding again."],
    ["reverted", "Reverted", "cancelled", "The change was accepted, then removed from main with patchgate revert.", "Examine the removal from main before deciding again."],
    ["unknown", "Unknown", "unknown", "Patchgate recorded a decision it does not recognize for this change.", "Examine the channel and the change's prerequisites."],
  ].map(([key, label, tone, meaning, next]) => Object.freeze({ key, label, tone, meaning, next })));
  const BY_KEY = new Map(DECISIONS.map((entry) => [entry.key, entry]));
  const MISSING = Object.freeze({ key: "missing", label: "Not recorded", tone: "unknown", meaning: "No decision was recorded for this change.", next: "" });
  // Tally order: what needs a person first, accepted last.
  const ATTENTION_ORDER = Object.freeze(["rejected", "escalated", "blocked", "unknown", "unrecognized", "missing", "removed", "reverted", "waiting", "pending", "accepted"]);
  // Patchgate's stuck states (State::stuck), plus values Kiln cannot read.
  const NEEDS_ATTENTION = new Set(["rejected", "escalated", "removed", "blocked", "unknown", "unrecognized", "missing"]);
  const FILTERS = Object.freeze([
    Object.freeze({ key: "", label: "All decisions" }),
    Object.freeze({ key: "attention", label: "Needs attention" }),
    ...DECISIONS.map((entry) => Object.freeze({ key: entry.key, label: entry.label })),
  ]);
  const FILTER_KEYS = new Set(FILTERS.map((filter) => filter.key));

  function decision(value) {
    if (typeof value === "string" && BY_KEY.has(value)) return BY_KEY.get(value);
    if (typeof value === "string" && value) {
      const quoted = value.length > LABEL_TEXT_MAX ? `${value.slice(0, LABEL_TEXT_MAX - 1)}…` : value;
      return Object.freeze({ key: "unrecognized", label: `Unrecognized: ${quoted}`, tone: "unknown", meaning: "Kiln does not recognize this recorded decision. It is shown as recorded and is not read as accepted.", next: "", recorded: value });
    }
    return MISSING;
  }
  // Counts per decision in attention order; distinct unrecognized values stay distinct. Each row keeps its decision.
  function tally(rows) {
    const counts = new Map();
    for (const row of Array.isArray(rows) ? rows : []) {
      const entry = decision(own(row, "decision"));
      const id = entry.key === "unrecognized" ? `unrecognized:${entry.recorded}` : entry.key;
      const known = counts.get(id);
      if (known) known.count += 1; else counts.set(id, { key: entry.key, label: entry.label, tone: entry.tone, count: 1, entry });
    }
    return Object.freeze([...counts.values()]
      .sort((a, b) => ATTENTION_ORDER.indexOf(a.key) - ATTENTION_ORDER.indexOf(b.key) || compareText(a.label, b.label))
      .map((row) => Object.freeze(row)));
  }

  // ---------- Reading untrusted documents ----------

  const isRecord = (value) => value !== null && typeof value === "object" && !Array.isArray(value);
  const own = (value, key) => (isRecord(value) && Object.hasOwn(value, key) ? value[key] : undefined);
  const text = (value) => (typeof value === "string" ? value : null);
  const filled = (value) => (typeof value === "string" && value.trim() ? value : null);
  const flag = (value) => (typeof value === "boolean" ? value : null);
  const count = (value) => (Number.isSafeInteger(value) && value >= 0 ? value : null);
  const seconds = (value) => (typeof value === "number" && Number.isFinite(value) && value >= 0 ? value : null);
  const strings = (value) => (Array.isArray(value) ? value.filter((item) => typeof item === "string") : []);
  function instant(value) {
    if (typeof value !== "string" || !RFC3339.test(value)) return null;
    const ms = Date.parse(value);
    return Number.isSafeInteger(ms) ? ms : null;
  }
  function compareText(a, b) { return a < b ? -1 : a > b ? 1 : 0; }

  function channel(value) {
    const name = filled(own(value, "channel"));
    return name ? Object.freeze({ channel: name, state: filled(own(value, "state")) }) : null;
  }
  function recordContext(record) {
    const coverage = own(record, "coverage");
    const pages = Array.isArray(own(coverage, "pages")) ? coverage.pages.map((page) => own(page, "decision")) : [];
    const requests = own(own(record, "provenance"), "requests");
    return Object.freeze({
      decision: text(own(record, "decision")),
      by: filled(own(record, "by")),
      user: filled(own(record, "user")),
      model: filled(own(record, "model")),
      atMs: instant(own(record, "at")),
      reason: filled(own(record, "reason")),
      summary: filled(own(record, "summary")),
      executionError: filled(own(record, "execution_error")),
      reused: own(record, "reused") === true,
      checks: Object.freeze((Array.isArray(own(own(record, "verification"), "results")) ? record.verification.results : []).map((result) => filled(own(result, "check"))).filter(Boolean)),
      coverage: isRecord(coverage) ? Object.freeze({
        planned: count(own(coverage, "planned_pages")),
        reviewed: pages.length,
        complete: flag(own(coverage, "complete")),
        accepted: pages.filter((page) => page === "accept").length,
        rejected: pages.filter((page) => page === "reject").length,
        escalated: pages.filter((page) => page === "escalate").length,
      }) : null,
      modelRequests: Array.isArray(requests) ? requests.length : null,
      findings: Array.isArray(own(record, "findings")) ? record.findings.length : 0,
      verifiers: isRecord(own(own(record, "provenance"), "verifiers")) ? Object.freeze({
        patchgate: filled(own(record.provenance.verifiers, "patchgate")),
        pijul: filled(own(record.provenance.verifiers, "pijul")),
        nix: filled(own(record.provenance.verifiers, "nix")),
      }) : null,
    });
  }
  // r[impl kiln.dashboard.reviews.view.scenario.context] Patchgate's native report, read once and only for display: own
  // properties with type checks, changes keyed by hash in a Map, unknown fields ignored. A report that is not a JSON
  // object is "unreadable"; a receipt without one is "absent". Neither removes a normalized decision.
  function nativeContext(document) {
    const raw = own(document, "native_report");
    const empty = (status) => Object.freeze({ status, bytes: typeof raw === "string" ? window.KilnUi.utf8Bytes(raw) : null, report: null, changes: new Map() });
    if (typeof raw !== "string") return empty("absent");
    let report;
    try { report = JSON.parse(raw); } catch { report = null; }
    if (!isRecord(report)) return empty("unreadable");
    const records = new Map();
    for (const record of Array.isArray(own(report, "records")) ? report.records : []) {
      const hash = filled(own(record, "hash"));
      if (hash && !records.has(hash)) records.set(hash, recordContext(record));
    }
    const changes = new Map();
    for (const item of Array.isArray(own(report, "items")) ? report.items : []) {
      const hash = filled(own(item, "hash"));
      if (!hash || changes.has(hash)) continue;
      const paths = strings(own(item, "paths"));
      const needs = (Array.isArray(own(item, "needs")) ? item.needs : [])
        .filter((need) => Array.isArray(need) && typeof need[0] === "string")
        .map((need) => Object.freeze({ hash: need[0], state: need[1] }));
      changes.set(hash, Object.freeze({
        message: filled(own(item, "message")),
        paths: Object.freeze(paths),
        state: text(own(item, "state")),
        decided: flag(own(item, "decided")),
        reason: filled(own(item, "reason")),
        note: filled(own(item, "note")),
        needs: Object.freeze(needs),
        merge: filled(own(own(item, "merge"), "hash")),
        record: records.get(hash) || null,
      }));
    }
    const configuration = own(report, "configuration");
    const values = own(configuration, "values");
    const usage = own(report, "review_usage");
    const firstVerifiers = [...records.values()].find((record) => record.verifiers)?.verifiers || null;
    return Object.freeze({
      status: "read",
      bytes: window.KilnUi.utf8Bytes(raw),
      changes,
      report: Object.freeze({
        dryRun: flag(own(report, "dry_run")),
        claimsAuthority: flag(own(report, "authorizes_acceptance")),
        main: channel(own(report, "main")),
        inbox: channel(own(report, "inbox")),
        configuration: isRecord(configuration) ? Object.freeze({
          path: filled(own(configuration, "path")),
          source: filled(own(configuration, "source")),
          sha256: filled(own(configuration, "sha256")),
          policy: filled(own(values, "policy_path")),
          protected: Object.freeze(strings(own(values, "protected"))),
          documentation: Object.freeze(strings(own(values, "documentation_paths"))),
        }) : null,
        usage: isRecord(usage) ? Object.freeze({
          requests: count(own(usage, "requests")),
          maxRequests: count(own(usage, "max_requests")),
          elapsedSeconds: seconds(own(usage, "elapsed_seconds")),
          timeoutSeconds: seconds(own(usage, "timeout_seconds")),
          stop: filled(own(usage, "stop")),
        }) : null,
        verifiers: firstVerifiers,
      }),
    });
  }
  // Who recorded a decision, in words: Patchgate's deterministic rules, a model, or a person.
  function decider(record) {
    if (!record?.by) return null;
    const who = record.by === "gate" ? "Patchgate rules, no model"
      : record.by === "model" ? (record.model ? `Model ${record.model}` : "A model")
        : record.by === "human" ? (record.user ? `A person (${record.user})` : "A person")
          : record.by;
    return record.reused ? `${who} · reused an earlier verdict` : who;
  }

  // ---------- List rows and filters ----------

  // r[impl kiln.dashboard.reviews.view.scenario.find] Rows from /api/reviews, newest decision first; receipts without a
  // recorded decision time follow, by name.
  function listRows(inventory) {
    const rows = [];
    const seen = new Set();
    for (const receipt of Array.isArray(own(inventory, "receipts")) ? inventory.receipts : []) {
      const name = filled(own(receipt, "name"));
      if (!name || seen.has(name)) continue;
      seen.add(name);
      const decisions = Array.isArray(own(receipt, "decisions")) ? receipt.decisions : [];
      const keys = new Set(decisions.map((row) => decision(own(row, "decision")).key));
      const changeIds = decisions.map((row) => filled(own(own(row, "change"), "native_id"))).filter(Boolean);
      const message = filled(own(receipt, "first_change_message"));
      const search = [name, message, ...changeIds, own(receipt, "candidate_id"), own(receipt, "effect_id"), own(receipt, "report_identity")]
        .filter((value) => typeof value === "string").join("\n").toLowerCase();
      rows.push(Object.freeze({
        name, message,
        atMs: Number.isSafeInteger(own(receipt, "latest_decision_at_ms")) ? receipt.latest_decision_at_ms : null,
        changes: decisions.length,
        tally: tally(decisions),
        keys: Object.freeze([...keys]),
        attention: [...keys].some((key) => NEEDS_ATTENTION.has(key)) || !decisions.length,
        claimsAuthority: own(receipt, "authorizes_acceptance") === true,
        isolated: own(receipt, "isolated") === true,
        search,
      }));
    }
    return Object.freeze(rows.sort((a, b) => (b.atMs ?? -Infinity) - (a.atMs ?? -Infinity) || compareText(a.name, b.name)));
  }
  function matches(row, query, filter) {
    const needle = typeof query === "string" ? query.trim().toLowerCase() : "";
    if (needle && !row.search.includes(needle)) return false;
    if (!filter) return true;
    if (filter === "attention") return row.attention;
    return row.keys.includes(filter);
  }
  function normalizeRoute(route) {
    const value = (key) => (typeof route?.[key] === "string" ? route[key] : "");
    const decisionFilter = value("receipt_decision");
    return Object.freeze({
      receipt: value("receipt"),
      q: value("receipt_q").slice(0, FILTER_MAX_CHARS),
      decision: FILTER_KEYS.has(decisionFilter) ? decisionFilter : "",
    });
  }

  // ---------- DOM helpers ----------

  const ui = () => window.KilnUi;
  function element(tag, content, className) {
    const node = document.createElement(tag);
    if (content !== undefined && content !== null) node.textContent = content;
    if (className) node.className = className;
    return node;
  }
  function plural(value, word, words = `${word}s`) { return `${value} ${value === 1 ? word : words}`; }
  function short(value) { const source = String(value ?? ""); return source.length > SHORT_HASH + 1 ? `${source.slice(0, SHORT_HASH)}…` : source; }
  function isAborted(error) { return error?.name === "AbortError"; }
  // One sentence for an error, ended with a full stop so the guidance that follows reads as its own sentence.
  function explain(error) {
    if (error?.name === "TimeoutError") return "The request timed out; its result is not known.";
    if (error instanceof TypeError) return "The service could not be reached.";
    const message = typeof error?.message === "string" ? error.message.trim() : "";
    if (!message) return "The service response was unavailable.";
    return /[.!?]$/.test(message) ? message : `${message}.`;
  }
  function plainClick(event) { return event.button === 0 && !event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey; }
  function routeHref(patch) {
    const params = new URLSearchParams(location.search);
    for (const [key, value] of Object.entries(patch)) { if (value) params.set(key, value); else params.delete(key); }
    return `?${params}`;
  }
  function link(ctx, patch, label, className) {
    const node = element("a", label, className);
    node.href = routeHref(patch);
    node.addEventListener("click", (event) => { if (!plainClick(event)) return; event.preventDefault(); ctx.navigate(patch); });
    return node;
  }
  function pill(entry, label = entry.label) {
    const node = element("span", undefined, "status-pill");
    node.dataset.status = entry.tone;
    node.append(ui().statusIcon(entry.tone), element("span", label, "status-pill-label"));
    node.title = entry.recorded ? `Recorded decision “${entry.recorded}”: ${entry.meaning}` : `${entry.label}: ${entry.meaning}`;
    return node;
  }
  function badge(label, tone, title) {
    const node = element("span", label, "state-badge");
    node.dataset.tone = tone;
    if (title) node.title = title;
    return node;
  }
  function facts(rows) {
    const list = element("dl", undefined, "facts");
    for (const [label, value] of rows) {
      if (value === undefined) continue;
      const cell = element("dd");
      if (value instanceof Node) cell.append(value);
      else if (value === null || value === "") { cell.textContent = "Not recorded"; cell.className = "not-recorded"; }
      else cell.textContent = String(value);
      list.append(element("dt", label), cell);
    }
    return list;
  }
  function emptyState(title, message, ...actions) {
    const node = element("div", undefined, "empty-state empty-state-inline");
    node.append(element("p", title, "empty-state-title"), element("p", message, "empty-state-text"));
    if (actions.length) node.append(...actions);
    return node;
  }
  function errorState(message) {
    const node = element("p", message, "warning error-state");
    node.dataset.state = "unavailable";
    return node;
  }
  function loadingState() {
    const node = element("div", undefined, "loading-state");
    node.setAttribute("aria-busy", "true");
    for (const width of ["skeleton-wide", "skeleton-mid", "skeleton-short"]) node.append(element("span", undefined, `skeleton ${width}`));
    return node;
  }
  function disclosure(summary, body, className = "") {
    const details = element("details", undefined, className);
    details.append(element("summary", summary), body);
    return details;
  }
  function section(title, ...children) {
    const node = element("section", undefined, "receipt-section");
    node.append(element("h3", title, "receipt-section-title"), ...children);
    return node;
  }
  function copyControl(value, label, done) {
    const wrap = element("span", undefined, "receipt-copy");
    const button = element("button", undefined, "quiet receipt-copy-button");
    button.type = "button";
    button.append(ui().icon("copy"), element("span", "Copy", "receipt-copy-label"));
    button.setAttribute("aria-label", label);
    const status = element("span", undefined, "small receipt-copy-status");
    status.setAttribute("role", "status");
    button.addEventListener("click", () => ui().copyText(value, status, done));
    wrap.append(button, status);
    return wrap;
  }
  function identity(value) {
    if (typeof value !== "string" || !value) return null;
    const wrap = element("span", undefined, "receipt-identity");
    wrap.append(element("code", value));
    return wrap;
  }
  function claim(value, yes, no) { return value === true ? yes : value === false ? no : null; }
  function seconds1000(value) { return value === null ? null : ui().formatDuration(Math.round(value * 1000)); }

  // ---------- Decision legend ----------

  function legend() {
    const list = element("dl", undefined, "guide-legend receipt-legend");
    for (const entry of DECISIONS) {
      const term = element("dt");
      term.append(pill(entry));
      const meaning = element("dd");
      meaning.append(element("span", entry.meaning));
      if (entry.next) meaning.append(element("span", `Next in Patchgate: ${entry.next}`, "receipt-next"));
      list.append(term, meaning);
    }
    return list;
  }
  function howToRead() {
    const panel = element("div", undefined, "receipt-howto");
    const heading = element("h2", "How to read a receipt");
    heading.tabIndex = -1;
    panel.append(heading,
      element("p", "A receipt records one Patchgate review: the changes it looked at, the decision for each and Patchgate's own report. Select a receipt to read it. Nothing is selected automatically.", "small"),
      legend(),
      element("p", "Receipts attached to a run are in that run's Reviews tab.", "small"));
    return panel;
  }

  // ---------- Receipt sheet ----------

  function changeEntry(row, context) {
    const change = own(row, "change");
    const hash = filled(own(change, "native_id"));
    const entry = decision(own(row, "decision"));
    const native = hash && context.status === "read" ? context.changes.get(hash) || null : null;
    const item = element("li", undefined, "receipt-change");
    item.dataset.decision = entry.key;
    const head = element("div", undefined, "receipt-change-head");
    const title = native?.message || (hash ? `Change ${short(hash)}` : "Change identity not recorded");
    head.append(pill(entry), element("strong", title, "receipt-change-title"));
    if (Number.isSafeInteger(native?.record?.atMs)) head.append(ui().timeElement(native.record.atMs));
    item.append(head);
    const meaning = element("p", entry.meaning, "receipt-change-meaning");
    if (entry.next) meaning.append(element("span", ` Next in Patchgate: ${entry.next}`, "receipt-next"));
    item.append(meaning);
    const rows = [];
    if (context.status === "read") {
      if (!native) rows.push(["Recorded by Patchgate", "Patchgate's report has no entry for this change."]);
      else {
        const record = native.record;
        rows.push(["Decided by", decider(record) || (record ? null : "No decision record")]);
        const paths = native.paths.slice(0, PATH_PREVIEW).join("\n");
        rows.push(["Paths", native.paths.length > PATH_PREVIEW ? `${paths}\nand ${native.paths.length - PATH_PREVIEW} more` : paths || "None recorded"]);
        rows.push(["Reason", native.reason || record?.reason || "None recorded"]);
        if (native.needs.length) {
          const needs = element("ul", undefined, "receipt-needs");
          for (const need of native.needs) {
            const line = element("li");
            line.append(element("code", short(need.hash)), document.createTextNode(" "), pill(decision(need.state)));
            line.title = need.hash;
            needs.append(line);
          }
          rows.push(["Prerequisites", needs]);
        }
        if (record?.executionError) rows.push(["Execution error", `${record.executionError}. An execution error is not a review verdict.`]);
      }
    } else rows.push(["Recorded by Patchgate", context.status === "absent" ? "This receipt has no Patchgate report." : "Unavailable: Patchgate's report could not be read."]);
    if (hash) {
      const value = element("span", undefined, "receipt-hash");
      value.append(element("code", hash), copyControl(hash, `Copy change hash ${short(hash)}`, "Change hash copied."));
      rows.push(["Change", value]);
    } else rows.push(["Change", null]);
    const list = facts(rows);
    list.classList.add("receipt-change-facts");
    item.append(list);
    const more = native ? moreEvidence(native) : null;
    if (more) item.append(more);
    return item;
  }
  function moreEvidence(native) {
    const record = native.record;
    const rows = [];
    if (native.decided !== null) rows.push(["Decided in this review", native.decided ? "Yes" : "No: the state comes from an earlier decision or from its prerequisites"]);
    if (record?.summary) rows.push(["Summary", record.summary]);
    if (native.note) rows.push(["Note", native.note]);
    if (record?.modelRequests !== null && record?.modelRequests !== undefined) rows.push(["Model requests", String(record.modelRequests)]);
    if (record?.coverage) {
      const { planned, reviewed, complete, accepted, rejected, escalated } = record.coverage;
      rows.push(["Context coverage", `${reviewed} of ${planned ?? "?"} windows reviewed${complete === true ? ", complete" : complete === false ? ", incomplete" : ""} · ${accepted} accept, ${rejected} reject, ${escalated} escalate`]);
    }
    if (record?.checks.length) rows.push(["Nix checks", record.checks.join("\n")]);
    if (record?.findings) rows.push(["Secret findings", `${record.findings} recorded`]);
    if (native.merge) rows.push(["Merge record", native.merge]);
    if (record?.decision) rows.push(["Recorded decision", record.decision]);
    if (!rows.length) return null;
    const body = facts(rows);
    return disclosure("More recorded evidence", body, "receipt-more");
  }
  function headline(name, context, observation) {
    const reviews = Array.isArray(own(observation, "reviews")) ? observation.reviews : [];
    const firstHash = filled(own(own(reviews[0], "change"), "native_id"));
    const message = context.status === "read" && firstHash ? context.changes.get(firstHash)?.message || null : null;
    const latest = context.status === "read" ? Math.max(...[...context.changes.values()].map((change) => change.record?.atMs ?? -Infinity)) : -Infinity;
    const head = element("header", undefined, "receipt-head");
    const title = element("h2", message || name || "Receipt", "receipt-title");
    title.tabIndex = -1;
    head.append(element("p", "Patchgate receipt", "eyebrow"), title);
    const meta = element("p", undefined, "receipt-meta");
    // The file name stays in the meta line unless it already is the headline.
    if (name && message) meta.append(element("code", name, "receipt-file"));
    meta.append(element("span", reviews.length > 1 ? plural(reviews.length, "change") + " reviewed" : reviews.length ? "1 change reviewed" : "No change recorded"));
    if (Number.isFinite(latest)) { const when = element("span", "Latest decision "); when.append(ui().timeElement(latest)); meta.append(when); }
    head.append(meta);
    return { head, reviews };
  }
  function tallyStrip(reviews) {
    const strip = element("div", undefined, "receipt-tally");
    strip.setAttribute("role", "group");
    strip.setAttribute("aria-label", "Decisions in this receipt");
    for (const entry of tally(reviews)) {
      const cell = element("div", undefined, "receipt-tally-cell");
      cell.dataset.tone = entry.tone;
      cell.append(element("span", String(entry.count), "receipt-tally-count"), element("span", entry.label, "receipt-tally-label"));
      cell.title = entry.entry.meaning;
      strip.append(cell);
    }
    if (!strip.childElementCount) strip.append(element("p", "This receipt records no change. An empty receipt is not a clean review.", "small"));
    return strip;
  }
  function statements(observation, context) {
    const notes = [];
    const isolated = own(observation, "isolated");
    const claims = own(observation, "authorizes_acceptance");
    notes.push(element("p", isolated === true
      ? "Evidence only. This receipt records Patchgate's decisions from an isolated review. It did not accept, merge or release anything, and Kiln did not re-run the review."
      : "Evidence only. This receipt did not accept, merge or release anything, and Kiln did not re-run the review.", "receipt-evidence"));
    if (claims === true) notes.push(element("p", "The producer claims this receipt authorizes acceptance. Kiln grants no acceptance authority from a receipt.", "warning"));
    if (isolated !== true) notes.push(element("p", "This receipt does not record that the review ran in isolation. Read its decisions as unverified producer claims.", "warning"));
    if (context.status === "unreadable") notes.push(element("p", "Patchgate's report in this receipt is not readable JSON. The decisions come from Kiln's admitted observation; messages, paths, reasons and times are unavailable. The report is shown verbatim below.", "warning"));
    if (context.status === "absent") notes.push(element("p", "This receipt has no Patchgate report. The decisions come from Kiln's admitted observation.", "warning"));
    if (context.report?.dryRun === false) notes.push(element("p", "Patchgate's report says this was not a dry run.", "warning"));
    return notes;
  }
  function reviewContext(context) {
    if (context.status !== "read") return null;
    const report = context.report;
    const channelText = (value) => (value ? `${value.channel}${value.state ? `\nstate ${value.state}` : ""}` : null);
    const configuration = report.configuration;
    const usage = report.usage;
    const verifiers = report.verifiers;
    const list = facts([
      ["Run mode", report.dryRun === true ? "Dry run: source not changed" : report.dryRun === false ? "Not a dry run" : null],
      ["Main channel", channelText(report.main)],
      ["Inbox channel", channelText(report.inbox)],
      ["Configuration", configuration?.path ? `${configuration.path}${configuration.source ? ` from ${configuration.source}` : ""}${configuration.sha256 ? `\nSHA-256 ${configuration.sha256}` : ""}` : null],
      ["Policy file", configuration?.policy ?? null],
      ["Protected paths", configuration ? configuration.protected.join("\n") || "None" : null],
      ["Documentation paths", configuration ? configuration.documentation.join("\n") || "None" : null],
      ["Model requests", usage && usage.requests !== null ? `${usage.requests}${usage.maxRequests !== null ? ` of ${usage.maxRequests} allowed` : ""}` : null],
      ["Review time", usage && usage.elapsedSeconds !== null ? `${seconds1000(usage.elapsedSeconds)}${usage.timeoutSeconds !== null ? ` of ${seconds1000(usage.timeoutSeconds)} allowed` : ""}` : null],
      ["Stopped early", usage?.stop ?? undefined],
      ["Versions", verifiers ? [verifiers.patchgate && `Patchgate ${verifiers.patchgate}`, verifiers.pijul, verifiers.nix && `Nix ${verifiers.nix}`].filter(Boolean).join(" · ") || null : null],
    ]);
    return section("Review context, recorded by Patchgate", list);
  }
  function identities(document, observation, file) {
    const list = facts([
      ["Receipt file", file.name || null],
      ["Receipt BLAKE3", identity(file.blake3)],
      ["Receipt size", Number.isSafeInteger(file.bytes) ? `${ui().humanBytes(file.bytes)} (${file.bytes.toLocaleString("en-US")} bytes)` : null],
      ["Request", identity(own(document, "request_identity"))],
      ["Report", identity(own(observation, "report_identity"))],
      ["Effect", identity(own(observation, "effect_id"))],
      ["Idempotency key", identity(own(observation, "idempotency_key"))],
      ["Candidate", identity(own(observation, "candidate_id"))],
      ["Candidate root", identity(own(observation, "candidate_root"))],
      ["Pipeline revision", identity(own(observation, "pipeline_revision"))],
      ["Policy", identity(own(observation, "policy_identity"))],
      ["Provider profile", identity(own(observation, "provider_profile"))],
      ["Native binding", identity(own(observation, "native_binding_identity"))],
      ["Isolated", claim(own(observation, "isolated"), "Yes (producer claim)", "No (producer claim)")],
      ["Acceptance authority", claim(own(observation, "authorizes_acceptance"), "Claimed by the producer; Kiln grants none", "None")],
    ]);
    list.classList.add("receipt-identities");
    return section("Exact identities", list);
  }
  // One receipt, from the human facts to the exact ones. `file` names the retained bytes; `binding` (run tab only)
  // adds the run record's facts and download.
  function receiptSheet(document, file, binding) {
    const observation = own(document, "observation");
    const context = nativeContext(document);
    const sheet = element("article", undefined, "receipt-sheet");
    const { head, reviews } = headline(file.name, context, observation);
    sheet.append(head, tallyStrip(reviews), ...statements(observation, context));
    if (binding) sheet.append(binding);
    const changes = element("ol", undefined, "receipt-changes");
    for (const row of reviews) changes.append(changeEntry(row, context));
    sheet.append(section(reviews.length === 1 ? "Change" : `Changes (${reviews.length})`, reviews.length ? changes : element("p", "No change is recorded in this receipt.", "small")));
    const contextSection = reviewContext(context);
    if (contextSection) sheet.append(contextSection);
    sheet.append(identities(document, observation, file));
    if (typeof own(document, "native_report") === "string") {
      const size = context.bytes === null ? "" : ` · ${ui().humanBytes(context.bytes)}`;
      sheet.append(disclosure(`Patchgate's report, verbatim${size}`, element("pre", document.native_report, "identity"), "receipt-raw"));
    }
    sheet.append(disclosure("Complete receipt as JSON", element("pre", JSON.stringify(document, null, 2), "identity"), "receipt-raw"));
    return sheet;
  }

  // ---------- Page ----------

  let generation = 0;
  let mounted = null;
  let filterTimer = null;
  // What the mounted page last drew from data, and the filter the reader has applied since.
  let shown = null;
  let applied = null;

  function aboutReceipts(label, nonClaims) {
    const body = element("div", undefined, "about-view-body");
    body.append(
      element("p", "Patchgate is the review gate for a Pijul repository. It decides which changes in the inbox channel may go into main: it accepts, rejects or escalates each change, and holds back changes whose prerequisites are not decided."),
      element("p", "Kiln runs Patchgate as an isolated dry run and keeps the result as a receipt: one decision per change, plus Patchgate's own report byte for byte. The review does not change the repository."),
      element("p", label ? `This list reads one directory that the operator declared: ${label}. Receipts attached to a run are in that run's Reviews tab.` : "Receipts attached to a run are in that run's Reviews tab."),
    );
    for (const line of strings(nonClaims)) body.append(element("p", line));
    const details = element("details", undefined, "about-view");
    details.append(ui().disclosureSummary("About Patchgate receipts", "info"), body);
    return details;
  }
  function receiptRow(ctx, row, selected) {
    const item = element("li");
    const anchor = element("a", undefined, "receipt-row");
    anchor.href = routeHref({ receipt: row.name });
    // A click carries the filter the reader has applied, even if it is not yet written to the URL.
    anchor.addEventListener("click", (event) => {
      if (!plainClick(event)) return;
      event.preventDefault();
      clearTimeout(filterTimer);
      ctx.navigate({ receipt: row.name, ...(applied ? { receipt_q: applied.q, receipt_decision: applied.decision } : {}) });
    });
    anchor.dataset.receipt = row.name;
    if (row.name === selected) anchor.setAttribute("aria-current", "true");
    const pills = element("span", undefined, "receipt-row-tally");
    for (const entry of row.tally.slice(0, ROW_PILLS)) pills.append(pill(entry.entry, row.changes === 1 ? entry.label : `${entry.count} ${entry.label}`));
    if (row.tally.length > ROW_PILLS) pills.append(element("span", `+${row.tally.slice(ROW_PILLS).reduce((sum, entry) => sum + entry.count, 0)} more`, "small"));
    if (!row.changes) pills.append(pill(MISSING, "No change recorded"));
    anchor.append(pills, element("strong", row.message || row.name, "receipt-row-title"));
    const meta = element("span", undefined, "receipt-row-meta");
    if (row.message) meta.append(element("code", row.name));
    meta.append(element("span", plural(row.changes, "change")));
    if (row.atMs !== null) meta.append(ui().timeElement(row.atMs));
    else meta.append(element("span", "Decision time not recorded", "not-recorded"));
    anchor.append(meta);
    if (row.claimsAuthority || !row.isolated) {
      const flags = element("span", undefined, "receipt-row-flags");
      if (row.claimsAuthority) flags.append(badge("Claims acceptance authority", "warning", "The producer claims this receipt authorizes acceptance. Kiln grants none."));
      if (!row.isolated) flags.append(badge("Isolation not recorded", "warning", "The receipt does not record that the review ran in isolation."));
      anchor.append(flags);
    }
    item.append(anchor);
    return item;
  }
  function skippedEntries(skipped) {
    const rows = Array.isArray(skipped) ? skipped : [];
    if (!rows.length) return null;
    const table = element("dl", undefined, "facts receipt-skipped");
    for (const entry of rows) {
      const name = typeof entry === "string" ? entry : filled(own(entry, "name")) || "Unnamed entry";
      const reason = filled(own(entry, "reason")) || "reason not recorded";
      table.append(element("dt", name), element("dd", `${reason[0].toUpperCase()}${reason.slice(1)}. No decision is inferred.`));
    }
    return disclosure(`${plural(rows.length, "directory entry", "directory entries")} skipped: not readable receipts`, table, "receipt-skipped-disclosure");
  }
  // r[impl kiln.dashboard.reviews.view.scenario.absent] Unconfigured, empty and unavailable inventories each say so.
  function listPanel(ctx, route, inventory, inventoryError, rows) {
    const panel = element("section", undefined, "panel receipts-list-panel");
    panel.setAttribute("aria-labelledby", "receipts-list-title");
    const head = element("div", undefined, "receipts-list-head");
    const title = element("h2", "Receipts");
    title.id = "receipts-list-title";
    head.append(title);
    panel.append(head);
    if (inventoryError) {
      const retry = element("button", "Try again");
      retry.type = "button";
      retry.addEventListener("click", () => { void load(mounted, ctx); });
      panel.append(errorState(`Receipt list unavailable: ${explain(inventoryError)} A selected receipt is still read on its own.`), retry);
      return panel;
    }
    if (inventory.configured !== true) {
      panel.append(emptyState("No receipt directory is configured", "An operator declares one with --reviews. Receipts attached to a run are still in that run's Reviews tab. This is not proof that no receipt exists."));
      panel.append(aboutReceipts(null, inventory.non_claims));
      return panel;
    }
    head.append(element("p", `From ${filled(inventory.label) || "the configured directory"}`, "small receipts-source"));
    if (!rows.length) {
      panel.append(emptyState(`No receipts in ${filled(inventory.label) || "the configured directory"} yet`, "kiln-patchgate-review run publishes one receipt per review into this directory."));
    } else {
      const form = element("form", undefined, "receipts-filters");
      form.setAttribute("role", "search");
      form.addEventListener("submit", (event) => event.preventDefault());
      const searchLabel = element("label", "Search");
      const search = element("input");
      Object.assign(search, { id: "receipts-q", type: "search", maxLength: FILTER_MAX_CHARS, autocomplete: "off", placeholder: "Message, change hash or file", value: route.q });
      searchLabel.htmlFor = "receipts-q";
      const decisionLabel = element("label", "Decision");
      const select = element("select");
      select.id = "receipts-decision";
      decisionLabel.htmlFor = "receipts-decision";
      for (const filter of FILTERS) { const option = element("option", filter.label); option.value = filter.key; select.append(option); }
      select.value = route.decision;
      const searchField = element("div", undefined, "receipts-field");
      searchField.append(searchLabel, search);
      const decisionField = element("div", undefined, "receipts-field");
      decisionField.append(decisionLabel, select);
      form.append(searchField, decisionField);
      const status = element("p", undefined, "small receipts-count");
      status.setAttribute("role", "status");
      const list = element("ol", undefined, "receipts-list");
      const entries = rows.map((row) => { const item = receiptRow(ctx, row, route.receipt); list.append(item); return { row, item }; });
      const hidden = element("p", undefined, "small receipts-hidden");
      const noMatch = element("div", undefined, "receipts-no-match");
      const clear = element("button", "Clear filters", "quiet");
      clear.type = "button";
      const apply = () => {
        const q = search.value.slice(0, FILTER_MAX_CHARS);
        const filter = FILTER_KEYS.has(select.value) ? select.value : "";
        let visible = 0;
        let selectedVisible = false;
        for (const { row, item } of entries) {
          const match = matches(row, q, filter);
          item.hidden = !match;
          if (match) visible += 1;
          if (match && row.name === route.receipt) selectedVisible = true;
        }
        const filtered = Boolean(q.trim() || filter);
        status.textContent = filtered ? `${visible} of ${plural(rows.length, "receipt")}` : `${plural(rows.length, "receipt")}, newest decision first`;
        noMatch.hidden = Boolean(visible) || !filtered;
        clear.hidden = !filtered;
        hidden.hidden = !(route.receipt && !selectedVisible && rows.some((row) => row.name === route.receipt));
        applied = { q, decision: filter };
      };
      const commit = () => {
        clearTimeout(filterTimer);
        filterTimer = setTimeout(() => { if (search.isConnected) ctx.navigate({ receipt_q: applied.q, receipt_decision: applied.decision }); }, FILTER_COMMIT_MS);
      };
      search.addEventListener("input", () => { apply(); commit(); });
      select.addEventListener("change", () => { apply(); commit(); });
      clear.addEventListener("click", () => { search.value = ""; select.value = ""; apply(); commit(); search.focus(); });
      noMatch.append(element("p", "No receipt matches these filters.", "small"));
      hidden.textContent = "The selected receipt is hidden by these filters. It stays selected.";
      list.addEventListener("keydown", (event) => {
        if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
        const links = [...list.querySelectorAll("li:not([hidden]) > .receipt-row")];
        const index = links.indexOf(document.activeElement);
        if (index < 0) return;
        const next = links[index + (event.key === "ArrowDown" ? 1 : -1)];
        if (next) { event.preventDefault(); next.focus(); }
      });
      const tools = element("div", undefined, "receipts-tools");
      tools.append(status, clear);
      panel.append(form, tools, list, noMatch, hidden);
      apply();
    }
    const skipped = skippedEntries(inventory.skipped);
    if (skipped) panel.append(skipped);
    panel.append(aboutReceipts(filled(inventory.label), inventory.non_claims));
    return panel;
  }
  function detailPanel(ctx, route, rows, inventory, detail) {
    const panel = element("section", undefined, "panel receipts-detail");
    if (!route.receipt) {
      panel.setAttribute("aria-label", "How to read a receipt");
      panel.append(howToRead());
      return panel;
    }
    panel.setAttribute("aria-label", "Selected receipt");
    panel.append(link(ctx, { receipt: "" }, "All receipts", "button receipts-back"));
    if (detail.error) {
      panel.append(errorState(`Receipt ${route.receipt} unavailable: ${explain(detail.error)} No other receipt was substituted.`));
      return panel;
    }
    const record = detail.value;
    if (!isRecord(own(record, "document"))) {
      panel.append(errorState(`Receipt ${route.receipt} returned no receipt document. No other receipt was substituted.`));
      return panel;
    }
    // A receipt read directly that the list no longer holds is still shown, with that fact stated.
    if (inventory && inventory.configured === true && !rows.some((row) => row.name === route.receipt)) {
      panel.append(element("p", "This exact receipt is not in the current list. No other receipt was substituted.", "warning"));
    }
    panel.append(receiptSheet(record.document, { name: filled(record.name) || route.receipt, blake3: record.blake3, bytes: record.bytes }));
    return panel;
  }

  // What had focus before a redraw: a receipt row by name, a filter by `#id`, or the stacked layout's back link.
  function focusedReceipt(container) {
    const active = document.activeElement;
    if (!active || !container.contains(active)) return "";
    if (active.classList.contains("receipts-back")) return "@back";
    return active.dataset?.receipt || (active.id === "receipts-q" || active.id === "receipts-decision" ? `#${active.id}` : "");
  }
  async function load(container, ctx) {
    if (!container) return;
    const route = normalizeRoute(ctx?.route);
    // A filter the reader already applied is only being written to the URL: nothing to read or redraw.
    if (mounted === container && shown && applied && route.receipt === shown.receipt && (route.q !== shown.q || route.decision !== shown.decision)
      && route.q === applied.q && route.decision === applied.decision && container.firstChild) {
      shown = { ...shown, q: route.q, decision: route.decision };
      return;
    }
    const current = ++generation;
    clearTimeout(filterTimer);
    if (mounted !== container) { mounted?.replaceChildren(); shown = null; }
    mounted = container;
    const live = () => current === generation && container.isConnected;
    if (!shown) container.replaceChildren(loadingState());
    const [inventory, detail] = await Promise.all([
      ctx.api("/api/reviews").then((value) => ({ value }), (error) => ({ error })),
      route.receipt ? ctx.api(`/api/reviews/${encodeURIComponent(route.receipt)}`).then((value) => ({ value }), (error) => ({ error })) : Promise.resolve(null),
    ]);
    if (!live() || isAborted(inventory.error) || isAborted(detail?.error)) return;
    const rows = inventory.value ? listRows(inventory.value) : [];
    const focus = focusedReceipt(container);
    const firstDraw = !shown;
    const previous = shown?.receipt || "";
    const listTop = container.querySelector(".receipts-list")?.scrollTop ?? 0;
    const layout = element("div", undefined, "receipts-layout");
    layout.dataset.selected = route.receipt ? "true" : "false";
    applied = null;
    layout.append(listPanel(ctx, route, inventory.value, inventory.error, rows), detailPanel(ctx, route, rows, inventory.value, detail || {}));
    container.replaceChildren(layout);
    shown = { receipt: route.receipt, q: route.q, decision: route.decision };
    const list = container.querySelector(".receipts-list");
    if (list) list.scrollTop = listTop;
    // Side by side, focus stays on the list row so the arrow keys keep moving. Stacked, the opened receipt comes first,
    // so focus moves to its title; the back link returns focus to the receipt that was open.
    const sheet = container.querySelector(".receipts-detail");
    const listPanelNode = container.querySelector(".receipts-list-panel");
    const stacked = Boolean(sheet && listPanelNode && sheet.getBoundingClientRect().bottom <= listPanelNode.getBoundingClientRect().top);
    const rowFor = (name) => [...container.querySelectorAll(".receipt-row")].find((node) => node.dataset.receipt === name);
    const heading = stacked && focus && focus === route.receipt ? container.querySelector(".receipt-title") : null;
    if (focus.startsWith("#")) container.querySelector(focus)?.focus({ preventScroll: true });
    else if (focus === "@back") rowFor(previous)?.focus();
    else if (heading) heading.focus({ preventScroll: true });
    else if (focus) rowFor(focus)?.focus({ preventScroll: true });
    // A deep link brings its receipt into the list's own view; the page itself does not move.
    const selectedRow = list?.querySelector('.receipt-row[aria-current="true"]');
    if (firstDraw && list && selectedRow && list.scrollHeight > list.clientHeight) {
      const offset = selectedRow.getBoundingClientRect().top - list.getBoundingClientRect().top;
      if (offset < 0 || offset + selectedRow.offsetHeight > list.clientHeight) list.scrollTop += offset - list.clientHeight / 3;
    }
    if (!firstDraw && route.receipt && sheet && sheet.getBoundingClientRect().top < 0) sheet.scrollIntoView({ block: "start" });
    const title = container.querySelector(".receipt-title")?.textContent;
    document.title = ["Kiln", "Patchgate receipts", route.receipt ? title || route.receipt : ""].filter(Boolean).join(" · ");
  }
  function clear() {
    generation += 1;
    clearTimeout(filterTimer);
    mounted?.replaceChildren();
    mounted = null;
    shown = null;
    applied = null;
  }

  // ---------- Run tab ----------

  // r[impl kiln.dashboard.reviews.view.scenario.retained] A run's retained receipts use the same sheet with their run
  // binding. A declared review report that is unavailable reads as unavailable, never as a clean review.
  function renderRun(parent, result, ctx) {
    const records = own(result, "reports");
    if (!Array.isArray(records)) throw new Error("Structured report response unavailable.");
    const receipts = [];
    const unavailable = [];
    let otherReviews = 0;
    for (const record of records) {
      const document = own(own(own(record, "report"), "evidence"), "document");
      if (own(document, "schema") === SCHEMA) receipts.push(record);
      else if (own(record, "kind") === "review" && filled(own(record, "unavailable"))) unavailable.push(record);
      else if (own(record, "kind") === "review") otherReviews += 1;
    }
    if (!receipts.length && !unavailable.length) {
      parent.append(emptyState("No Patchgate receipt for this run", "A review step retains one by uploading its receipt as a review report. A run without a receipt has no recorded review; that is not a clean review."));
    }
    for (const record of unavailable) {
      const card = element("article", undefined, "receipt-sheet receipt-unavailable");
      card.append(element("p", "Patchgate receipt", "eyebrow"), element("h3", `Review report unavailable: ${record.unavailable}`, "receipt-title"));
      card.append(element("p", `The run declared ${filled(own(record, "name")) || "a review report"} on node ${filled(own(record, "node_id")) || "not recorded"}, but its bytes were ${record.unavailable}. No decision is inferred.`, "warning"));
      parent.append(card);
    }
    for (const record of receipts) {
      const artifact = own(record, "artifact");
      const binding = element("div", undefined, "receipt-binding");
      const recorded = Number.isSafeInteger(own(record, "recorded_at_ms")) ? ui().timeElement(record.recorded_at_ms) : null;
      binding.append(facts([
        ["Retained for this run", recorded],
        ["Review node", filled(own(record, "node_id"))],
        ["Source revision", identity(own(record, "source_revision"))],
        ["Report record", identity(own(record, "id"))],
      ]));
      if (isRecord(artifact) && typeof ctx?.download === "function") {
        const button = element("button", "Download exact receipt");
        button.type = "button";
        button.addEventListener("click", () => ctx.download(artifact, button));
        binding.append(button);
      }
      const file = { name: filled(own(record, "name")), blake3: own(artifact, "blake3"), bytes: own(artifact, "bytes") };
      parent.append(receiptSheet(record.report.evidence.document, file, binding));
      const boundary = filled(own(record.report.evidence, "claim_boundary"));
      if (boundary) parent.append(element("p", boundary, "boundary"));
    }
    const other = records.length - receipts.length - unavailable.length - otherReviews;
    const notes = [];
    if (otherReviews) notes.push(`${plural(otherReviews, "review report")} ${otherReviews === 1 ? "is" : "are"} not a Patchgate receipt`);
    if (other) notes.push(`${plural(other, "other report")}`);
    if (notes.length) parent.append(element("p", `${notes.join("; ")}: shown under Tests & coverage.`, "small"));

    const passportInput = {
      current_candidate_id: filled(own(ctx, "candidateId")) || filled(own(result, "candidate_id")),
      current_revision: filled(own(ctx, "sourceRevision")) || filled(own(result, "source_revision")),
      reports: records,
      checks: Array.isArray(own(ctx, "checks")) ? ctx.checks : Array.isArray(own(result, "checks")) ? result.checks : [],
      cairn_completion: own(ctx, "cairnCompletion") ?? own(result, "cairn_completion") ?? own(result, "authoritative_completion"),
      handoff: own(ctx, "handoff") ?? own(result, "handoff"),
      plan_review: own(ctx, "planReview") ?? own(result, "plan_review"),
      semantic_merge: own(ctx, "semanticMerge") ?? own(result, "semantic_merge"),
      provenance: own(ctx, "provenance") ?? own(result, "provenance"),
    };
    const passport = normalizeGatePassport(passportInput);
    if (passport.entries.length) parent.prepend(renderGatePassport(passportInput, { title: "Run Gate & Receipt Passport" }));
  }

  // ---------- Unified Gate & Receipt Passport ----------
  // r[impl kiln.cairn.gate_passport_surface]
  const PROVENANCE = Object.freeze({
    observed: Object.freeze({
      key: "observed",
      label: "Observed Receipt",
      border: "solid",
      tone: "success",
      description: "Independent canonical receipt or retained artifact observed by Kiln.",
    }),
    agent_claim: Object.freeze({
      key: "agent-claim",
      label: "Agent Claim",
      border: "dotted",
      tone: "warning",
      description: "Self-reported agent handoff, proposal, or advisory claim; never satisfies canonical verification.",
    }),
    declared: Object.freeze({
      key: "declared",
      label: "Declared",
      border: "dashed",
      tone: "unknown",
      description: "Declared policy gate, pending obligation, or unverified metadata without an observed receipt.",
    }),
  });

  const PASSPORT_CATEGORIES = Object.freeze([
    ["ci_checks", "Typed CI checks"],
    ["cairn_completion", "cairn-authoritative-completion & handoffs"],
    ["patchgate", "Patchgate review decisions"],
    ["provenance_digests", "SLSA / in-toto digests"],
    ["plan_review", "Plan Review receipts"],
    ["semantic_merge", "Semantic Merge O/A/B/M quartets"],
  ]);

  function normalizeProvenance(value, fallback = "declared") {
    const raw = typeof value === "string" ? value.trim().toLowerCase() : "";
    if (raw === "observed" || raw === "observed_receipt" || raw === "receipt" || raw === "canonical") return PROVENANCE.observed;
    if (raw === "agent_claim" || raw === "agent-claim" || raw === "agent" || raw === "handoff" || raw === "self_reported") return PROVENANCE.agent_claim;
    if (raw === "declared" || raw === "pending" || raw === "unverified") return PROVENANCE.declared;
    return PROVENANCE[fallback] || PROVENANCE.declared;
  }

  function passportOutcomeTone(value) {
    const raw = typeof value === "string" ? value.trim().toLowerCase() : "";
    if (["pass", "passed", "verified", "accepted", "success", "complete", "completed", "unchanged", "changed"].includes(raw)) return "success";
    if (["fail", "failed", "failure", "rejected", "counterexample", "error", "toolerror", "tool_error", "veto", "denied", "prevented"].includes(raw)) return "failure";
    if (["running", "active", "in_progress"].includes(raw)) return "running";
    if (["pending", "waiting", "queued", "unreviewed", "no_plan", "unverified", "unverified / pending"].includes(raw)) return "queued";
    if (["cancelled", "skipped", "removed", "reverted"].includes(raw)) return "cancelled";
    if (["escalated", "blocked", "unresolved", "changes_requested", "unavailable", "exhausted", "stale", "warning"].includes(raw)) return "warning";
    return "unknown";
  }

  function isStaleCandidate(item, currentCandidateId, currentRevision) {
    if (!isRecord(item)) return false;
    if (own(item, "stale") === true || own(item, "superseded") === true || own(item, "historical") === true || own(item, "is_current") === false) return true;
    const candidateId = filled(own(item, "candidate_id")) || filled(own(own(item, "candidate"), "id"));
    if (currentCandidateId && candidateId && candidateId !== currentCandidateId) return true;
    const rev = filled(own(item, "revision")) || filled(own(item, "source_revision"));
    if (currentRevision && rev && rev !== currentRevision) return true;
    return false;
  }

  function formatDigestMap(digest) {
    if (!isRecord(digest)) return [];
    return Object.entries(digest)
      .filter(([, val]) => typeof val === "string" && val.trim())
      .map(([alg, val]) => `${alg}:${val}`);
  }

  function quartetSlot(labelText, role) {
    if (typeof role === "string" && role.trim()) return { role: labelText, revision: role.trim(), canonical_tree: null, channel: null };
    if (!isRecord(role)) return { role: labelText, revision: null, canonical_tree: null, channel: null };
    const source = isRecord(own(role, "source")) ? role.source : role;
    return {
      role: labelText,
      revision: filled(own(source, "exact_revision")) || filled(own(own(source, "exact_revision"), "native_id")) || filled(own(source, "revision")) || filled(own(source, "commit")) || filled(own(role, "id")),
      canonical_tree: filled(own(source, "canonical_tree")) || filled(own(role, "canonical_tree")),
      channel: filled(own(source, "symbolic_reference")) || filled(own(own(source, "symbolic_reference"), "native_id")) || filled(own(source, "channel")),
    };
  }

  function normalizeGatePassport(raw) {
    const data = isRecord(raw) ? raw : {};
    const currentCandidateId = filled(own(data, "current_candidate_id")) || filled(own(data, "candidate_id"));
    const currentRevision = filled(own(data, "current_revision")) || filled(own(data, "source_revision"));
    const entries = [];
    const seenCompletionRecords = [];
    const agentHandoffs = [];

    const pushEntry = (category, item, sourceObj) => {
      const candidateId = filled(own(item, "candidate_id")) || filled(own(sourceObj, "candidate_id")) || filled(own(own(sourceObj, "candidate"), "id"));
      const rev = filled(own(item, "revision")) || filled(own(sourceObj, "revision")) || filled(own(sourceObj, "source_revision"));
      const stale = item.stale === true || isStaleCandidate(sourceObj, currentCandidateId, currentRevision);
      const isCurrent = !stale && Boolean(
        (currentCandidateId && candidateId && candidateId === currentCandidateId)
        || (currentRevision && rev && rev === currentRevision)
        || own(sourceObj, "is_current") === true
      );
      entries.push(Object.freeze({
        ...item,
        category,
        candidate_id: candidateId,
        revision: rev,
        stale,
        is_current: isCurrent,
        raw: sourceObj,
      }));
    };

    // 1. Typed CI checks (also route any cairn-authoritative-completion check through category 2 honesty rules).
    const ciChecks = [];
    if (Array.isArray(own(data, "checks"))) {
      for (const check of data.checks) if (isRecord(check)) ciChecks.push(check);
    }
    if (Array.isArray(own(data, "ci_runs"))) {
      for (const fact of data.ci_runs) {
        if (!isRecord(fact)) continue;
        const factRev = filled(own(fact, "revision"));
        const factCandidate = filled(own(fact, "candidate_id"));
        const checks = Array.isArray(own(fact, "checks")) ? fact.checks : [];
        if (checks.length) {
          for (const check of checks) {
            if (!isRecord(check)) continue;
            ciChecks.push({
              ...check,
              run_id: filled(own(check, "run_id")) || filled(own(fact, "run_id")),
              revision: filled(own(check, "revision")) || factRev,
              candidate_id: filled(own(check, "candidate_id")) || factCandidate,
              full_run: own(fact, "full_run"),
              clean: own(fact, "clean"),
              untrusted: own(fact, "untrusted"),
            });
          }
        } else {
          ciChecks.push({
            name: filled(own(fact, "project")) ? `CI · ${fact.project}` : `CI run ${short(own(fact, "run_id") || "unknown")}`,
            kind: "canonical_ci_run",
            outcome: own(fact, "success") === true ? "passed" : "pending",
            run_id: filled(own(fact, "run_id")),
            revision: factRev,
            candidate_id: factCandidate,
            full_run: own(fact, "full_run"),
            clean: own(fact, "clean"),
            untrusted: own(fact, "untrusted"),
          });
        }
      }
    }

    for (const check of ciChecks) {
      const name = filled(own(check, "name")) || filled(own(check, "id")) || "Unnamed check";
      const kind = filled(own(check, "kind")) || "check";
      if (name === "cairn-authoritative-completion" || kind === "cairn-agent-completion-v2") {
        seenCompletionRecords.push(check);
        continue;
      }
      const selfReported = own(check, "self_reported") === true || own(check, "provenance") === "agent_claim" || own(check, "provenance") === "agent-claim";
      const outcomeRaw = filled(own(check, "outcome")) || filled(own(check, "state")) || "unknown";
      const untrusted = own(check, "untrusted");
      const untrustedBlocked = isRecord(untrusted) && own(untrusted, "counts_for_required_ci") === false;
      const hasReceipt = Boolean(filled(own(check, "artifact_digest")) || filled(own(check, "run_id")) || own(check, "receipt_identity"));
      const prov = selfReported
        ? PROVENANCE.agent_claim
        : filled(own(check, "provenance"))
          ? normalizeProvenance(check.provenance, "observed")
          : (!hasReceipt && ["unknown", "pending", "missing", "unavailable", "not_recorded"].includes(outcomeRaw.toLowerCase())) || untrustedBlocked
            ? PROVENANCE.declared
            : PROVENANCE.observed;
      const factsRows = [
        ["Kind", kind],
        ...(filled(own(check, "run_id")) ? [["Run", check.run_id]] : []),
        ...(filled(own(check, "artifact_digest")) ? [["Artifact digest", check.artifact_digest]] : []),
        ...(typeof own(check, "full_run") === "boolean" ? [["Scope", check.full_run ? "Full run" : "Partial run"]] : []),
        ...(typeof own(check, "clean") === "boolean" ? [["Snapshot", check.clean ? "Clean" : "Dirty"]] : []),
        ...(untrustedBlocked ? [["Untrusted policy", "Does not count for required CI"]] : []),
      ];
      pushEntry("ci_checks", {
        title: name,
        statusLabel: outcomeRaw,
        tone: passportOutcomeTone(outcomeRaw),
        provenance: prov,
        summary: selfReported ? "Self-reported check claim; not independently verified." : `${kind} · ${outcomeRaw}`,
        identities: [filled(own(check, "artifact_digest")), filled(own(check, "run_id"))].filter(Boolean),
        facts: factsRows,
      }, check);
    }

    // 2. cairn-authoritative-completion receipts & agent handoffs (strict honesty invariant).
    const rawHandoffs = [
      ...(isRecord(own(data, "handoff")) ? [data.handoff] : []),
      ...(Array.isArray(own(data, "handoffs")) ? data.handoffs : []),
      ...(isRecord(own(own(data, "verification"), "handoff")) ? [data.verification.handoff] : []),
      ...(isRecord(own(data, "agent_handoff")) ? [data.agent_handoff] : []),
    ];
    for (const h of rawHandoffs) agentHandoffs.push(h);

    const rawCompletions = [
      ...(isRecord(own(data, "cairn_completion")) ? [data.cairn_completion] : []),
      ...(Array.isArray(own(data, "cairn_completion")) ? data.cairn_completion : []),
      ...(isRecord(own(data, "authoritative_completion")) ? [data.authoritative_completion] : []),
      ...(filled(own(data, "authoritative_completion")) ? [{ receipt_id: data.authoritative_completion, outcome: "verified", provenance: "observed" }] : []),
      ...(isRecord(own(data, "verification")) ? [data.verification] : []),
      ...seenCompletionRecords,
    ];

    const independentCompletions = [];
    for (const comp of rawCompletions) {
      if (!isRecord(comp)) continue;
      const isClaim = own(comp, "self_reported") === true
        || own(comp, "provenance") === "agent_claim"
        || own(comp, "provenance") === "agent-claim"
        || own(comp, "kind") === "agent_handoff";
      if (isClaim) {
        agentHandoffs.push(comp);
        continue;
      }
      // If a verification provenance object carries independent_receipt: null/missing, it is NOT independently verified!
      const hasExplicitMissingIndependentReceipt = Object.hasOwn(comp, "independent_receipt") && !filled(own(comp, "independent_receipt"));
      const hasExplicitNoReceiptFlag = own(comp, "has_independent_receipt") === false || own(comp, "verified") === false;
      const receiptId = filled(own(comp, "independent_receipt"))
        || filled(own(comp, "receipt_id"))
        || filled(own(comp, "completion_id"))
        || filled(own(comp, "artifact_digest"))
        || filled(own(comp, "binding"));
      const outcomeRaw = filled(own(comp, "outcome")) || filled(own(comp, "state")) || filled(own(comp, "status")) || "unknown";
      const claimsPass = ["pass", "passed", "verified", "success", "complete", "completed"].includes(outcomeRaw.toLowerCase());
      if (hasExplicitMissingIndependentReceipt || hasExplicitNoReceiptFlag || (claimsPass && !receiptId && own(comp, "verified") !== true && own(comp, "provenance") !== "observed")) {
        // Never mark cairn-authoritative-completion as passed without an independent canonical receipt!
        independentCompletions.push({ ...comp, __unverifiedPending: true });
      } else {
        independentCompletions.push(comp);
      }
    }

    for (const h of agentHandoffs) {
      const claimOutcome = filled(own(h, "outcome")) || filled(own(h, "status")) || filled(own(h, "state")) || "reported";
      const actor = filled(own(h, "agent")) || filled(own(h, "principal")) || filled(own(h, "actor")) || filled(own(own(h, "submitted_by"), "principal")) || "Agent";
      const handoffTone = ["fail", "failed", "aborted"].includes(claimOutcome.toLowerCase()) ? "failure" : "warning";
      pushEntry("cairn_completion", {
        title: `Agent handoff · ${actor}`,
        statusLabel: `Claimed: ${claimOutcome}`,
        tone: handoffTone,
        provenance: PROVENANCE.agent_claim,
        summary: "Self-reported agent handoff. Carries dotted Agent Claim provenance and never satisfies cairn-authoritative-completion.",
        identities: [filled(own(h, "attempt_id")), filled(own(h, "handoff_id")), filled(own(h, "subject_hash")), filled(own(h, "candidate_id"))].filter(Boolean),
        facts: [
          ["Principal / agent", actor],
          ["Self-reported status", claimOutcome],
          ...(filled(own(h, "attempt_id")) ? [["Attempt", h.attempt_id]] : []),
          ...(filled(own(h, "subject_hash")) ? [["Subject hash", h.subject_hash]] : []),
          ["Verification authority", "None — independent canonical verification required"],
        ],
      }, h);
    }

    let canonicalCompletionPassed = false;
    if (independentCompletions.length) {
      for (const comp of independentCompletions) {
        if (comp.__unverifiedPending) {
          pushEntry("cairn_completion", {
            title: "cairn-authoritative-completion",
            statusLabel: "Unverified / Pending",
            tone: "queued",
            provenance: PROVENANCE.declared,
            summary: "No independent canonical cairn-authoritative-completion receipt is present. Gate remains unverified/pending.",
            identities: [filled(own(comp, "binding")), filled(own(comp, "observation_identity")), filled(own(comp, "candidate_id"))].filter(Boolean),
            facts: [
              ["Gate", "cairn-authoritative-completion"],
              ["Canonical receipt", "Not recorded"],
              ["Status", "Unverified / Pending"],
            ],
          }, comp);
        } else {
          const outcomeRaw = filled(own(comp, "outcome")) || filled(own(comp, "state")) || filled(own(comp, "status")) || (own(comp, "verified") === true ? "verified" : "unknown");
          const isPass = ["pass", "passed", "verified", "success", "complete", "completed"].includes(outcomeRaw.toLowerCase());
          const stale = isStaleCandidate(comp, currentCandidateId, currentRevision);
          if (isPass && !stale) canonicalCompletionPassed = true;
          const receiptId = filled(own(comp, "independent_receipt")) || filled(own(comp, "receipt_id")) || filled(own(comp, "completion_id")) || filled(own(comp, "artifact_digest")) || filled(own(comp, "binding"));
          const prov = isPass ? PROVENANCE.observed : (receiptId ? PROVENANCE.observed : PROVENANCE.declared);
          pushEntry("cairn_completion", {
            title: "cairn-authoritative-completion",
            statusLabel: isPass ? "Passed" : outcomeRaw,
            tone: passportOutcomeTone(outcomeRaw),
            provenance: prov,
            summary: isPass ? "Independent canonical completion receipt observed." : `Canonical completion outcome: ${outcomeRaw}`,
            identities: [receiptId, filled(own(comp, "observation_identity")), filled(own(comp, "run_id"))].filter(Boolean),
            facts: [
              ["Gate", "cairn-authoritative-completion"],
              ["Receipt", receiptId || "Not recorded"],
              ...(filled(own(comp, "verifier")) ? [["Verifier", comp.verifier]] : []),
              ...(filled(own(comp, "run_id")) ? [["Verification run", comp.run_id]] : []),
            ],
          }, comp);
        }
      }
    } else if (agentHandoffs.length > 0 || own(data, "require_cairn_completion") === true) {
      pushEntry("cairn_completion", {
        title: "cairn-authoritative-completion",
        statusLabel: "Unverified / Pending",
        tone: "queued",
        provenance: PROVENANCE.declared,
        summary: "An agent handoff is present without an independent cairn-authoritative-completion receipt. Canonical verification is unverified/pending.",
        identities: [],
        facts: [
          ["Gate", "cairn-authoritative-completion"],
          ["Canonical receipt", "Absent"],
          ["Status", "Unverified / Pending"],
        ],
      }, { candidate_id: currentCandidateId, revision: currentRevision });
    }

    // 3. Patchgate review decisions
    const reports = Array.isArray(own(data, "reports")) ? data.reports : [];
    const patchgateSources = [
      ...(isRecord(own(data, "patchgate")) ? [data.patchgate] : []),
      ...(Array.isArray(own(data, "patchgate")) ? data.patchgate : []),
      ...(Array.isArray(own(data, "reviews")) ? data.reviews : []),
    ];
    for (const reportRecord of reports) {
      if (!isRecord(reportRecord)) continue;
      const doc = own(own(own(reportRecord, "report"), "evidence"), "document");
      if (own(doc, "schema") === SCHEMA) {
        patchgateSources.push({
          ...doc,
          name: filled(own(reportRecord, "name")),
          node_id: filled(own(reportRecord, "node_id")),
          revision: filled(own(reportRecord, "source_revision")),
          artifact_blake3: filled(own(own(reportRecord, "artifact"), "blake3")),
        });
      } else if (own(reportRecord, "kind") === "review" && filled(own(reportRecord, "unavailable"))) {
        patchgateSources.push({
          unavailable: reportRecord.unavailable,
          name: filled(own(reportRecord, "name")) || "Patchgate review",
          node_id: filled(own(reportRecord, "node_id")),
          revision: filled(own(reportRecord, "source_revision")),
        });
      }
    }
    for (const pg of patchgateSources) {
      if (!isRecord(pg)) continue;
      if (filled(own(pg, "unavailable"))) {
        pushEntry("patchgate", {
          title: filled(own(pg, "name")) || "Patchgate review",
          statusLabel: `Unavailable (${pg.unavailable})`,
          tone: "warning",
          provenance: PROVENANCE.declared,
          summary: `Declared review report bytes were ${pg.unavailable}. No decision is inferred.`,
          identities: [],
          facts: [
            ["Node", filled(own(pg, "node_id"))],
            ["Unavailable reason", pg.unavailable],
          ],
        }, pg);
        continue;
      }
      const obs = isRecord(own(pg, "observation")) ? pg.observation : pg;
      const reviews = Array.isArray(own(obs, "reviews")) ? obs.reviews : Array.isArray(own(pg, "decisions")) ? pg.decisions : [];
      const counts = tally(reviews);
      const isolated = own(obs, "isolated");
      const prov = own(pg, "provenance")
        ? normalizeProvenance(pg.provenance, "observed")
        : isolated === false ? PROVENANCE.agent_claim : PROVENANCE.observed;
      const topDecision = filled(own(pg, "decision")) || (counts.length ? counts.map((c) => `${c.count} ${c.label}`).join(", ") : "No changes recorded");
      const anyFailed = counts.some((c) => c.tone === "failure") || ["rejected", "veto"].includes(String(own(pg, "decision") || "").toLowerCase());
      const anyWarning = counts.some((c) => c.tone === "warning") || isolated !== true;
      const tone = anyFailed ? "failure" : anyWarning ? "warning" : counts.length || own(pg, "decision") === "accepted" ? "success" : "unknown";
      pushEntry("patchgate", {
        title: filled(own(pg, "name")) || "Patchgate receipt",
        statusLabel: topDecision,
        tone,
        provenance: prov,
        summary: isolated === true
          ? "Isolated Patchgate review decisions (evidence only; grants no merge authority)."
          : "Isolation not recorded; read decisions as producer claim.",
        identities: [filled(own(obs, "report_identity")), filled(own(pg, "request_identity")), filled(own(pg, "artifact_blake3")), filled(own(obs, "candidate_id"))].filter(Boolean),
        facts: [
          ["Decisions", topDecision],
          ["Isolated", isolated === true ? "Yes" : isolated === false ? "No" : "Not recorded"],
          ...(filled(own(obs, "report_identity")) ? [["Report identity", obs.report_identity]] : []),
          ...(filled(own(obs, "candidate_id")) ? [["Candidate", obs.candidate_id]] : []),
        ],
      }, { ...pg, candidate_id: filled(own(obs, "candidate_id")) || filled(own(pg, "candidate_id")), revision: filled(own(pg, "revision")) || filled(own(obs, "source_revision")) });
    }

    // 4. SLSA / in-toto subject & dependency digests
    const provenanceSources = [
      ...(isRecord(own(data, "provenance")) ? [data.provenance] : []),
      ...(Array.isArray(own(data, "provenance")) ? data.provenance : []),
      ...(isRecord(own(data, "in_toto")) ? [data.in_toto] : []),
      ...(isRecord(own(data, "slsa")) ? [data.slsa] : []),
    ];
    for (const reportRecord of reports) {
      if (!isRecord(reportRecord)) continue;
      const report = own(reportRecord, "report") || reportRecord;
      const doc = own(own(report, "evidence"), "document");
      if (own(report, "kind") === "provenance" && isRecord(doc) && own(doc, "_type") === "https://in-toto.io/Statement/v1") {
        provenanceSources.push({
          ...doc,
          report_id: filled(own(reportRecord, "id")),
          revision: filled(own(reportRecord, "source_revision")),
          source_blake3: filled(own(report, "source_blake3")),
        });
      }
    }
    for (const stmt of provenanceSources) {
      if (!isRecord(stmt)) continue;
      const predicate = isRecord(own(stmt, "predicate")) ? stmt.predicate : {};
      const buildDef = isRecord(own(predicate, "buildDefinition")) ? predicate.buildDefinition : {};
      const runDetails = isRecord(own(predicate, "runDetails")) ? predicate.runDetails : {};
      const ext = isRecord(own(buildDef, "externalParameters")) ? buildDef.externalParameters : {};
      const subjects = Array.isArray(own(stmt, "subject")) ? stmt.subject : [];
      const deps = Array.isArray(own(buildDef, "resolvedDependencies")) ? buildDef.resolvedDependencies : [];
      const hasStatement = own(stmt, "_type") === "https://in-toto.io/Statement/v1" || subjects.length > 0 || Boolean(filled(own(stmt, "source_blake3")));
      const prov = own(stmt, "provenance")
        ? normalizeProvenance(stmt.provenance, hasStatement ? "observed" : "declared")
        : (hasStatement ? PROVENANCE.observed : PROVENANCE.declared);
      const subjectRows = subjects.map((s) => ({
        name: filled(own(s, "name")) || "subject",
        digests: formatDigestMap(own(s, "digest")),
      }));
      const depRows = deps.map((d) => ({
        name: filled(own(d, "uri")) || filled(own(d, "name")) || "dependency",
        digests: formatDigestMap(own(d, "digest")),
      }));
      const signed = own(stmt, "signed") === true;
      pushEntry("provenance_digests", {
        title: filled(own(stmt, "title")) || "SLSA / in-toto provenance",
        statusLabel: signed ? "Signed statement" : hasStatement ? "Unsigned statement" : "Declared provenance",
        tone: hasStatement ? "info" : "queued",
        provenance: prov,
        summary: signed
          ? `${subjectRows.length} subject(s), ${depRows.length} resolved dependencies.`
          : `Unsigned provenance projection (${subjectRows.length} subject(s), ${depRows.length} dependencies). Does not establish builder trust or SLSA build level.`,
        identities: [filled(own(stmt, "source_blake3")), filled(own(stmt, "report_id")), filled(own(ext, "sourceOrCandidate"))].filter(Boolean),
        subjects: subjectRows,
        dependencies: depRows,
        facts: [
          ["Predicate", filled(own(stmt, "predicateType")) || filled(own(stmt, "predicate_type"))],
          ["Build type", filled(own(buildDef, "buildType")) || filled(own(stmt, "build_type"))],
          ["Builder", filled(own(own(runDetails, "builder"), "id")) || filled(own(stmt, "builder_id"))],
          ["Subjects", `${subjectRows.length}`],
          ["Dependencies", `${depRows.length}`],
        ],
      }, { ...stmt, candidate_id: filled(own(stmt, "candidate_id")) || filled(own(ext, "candidateId")), revision: filled(own(stmt, "revision")) || filled(own(ext, "sourceOrCandidate")) });
    }

    // 5. Plan Review receipts (docs/plan-review.md)
    const planSources = [
      ...(isRecord(own(data, "plan_review")) ? [data.plan_review] : []),
      ...(Array.isArray(own(data, "plan_reviews")) ? data.plan_reviews : []),
      ...(isRecord(own(data, "plan_review_receipt")) ? [data.plan_review_receipt] : []),
    ];
    for (const pr of planSources) {
      if (!isRecord(pr)) continue;
      const gate = filled(own(pr, "gate")) || filled(own(pr, "outcome")) || filled(own(pr, "decision")) || "unreviewed";
      const gateLower = gate.toLowerCase();
      const hasDecision = ["accepted", "changes_requested", "changed", "prevented", "unchanged"].includes(gateLower);
      const isAgentOnly = own(pr, "self_reported") === true || own(pr, "provenance") === "agent_claim" || own(pr, "provenance") === "agent-claim";
      const prov = own(pr, "provenance")
        ? normalizeProvenance(pr.provenance, hasDecision ? "observed" : "declared")
        : isAgentOnly ? PROVENANCE.agent_claim : hasDecision ? PROVENANCE.observed : PROVENANCE.declared;
      const comp = isRecord(own(pr, "comparison")) ? pr.comparison : null;
      const compSummary = comp
        ? `kept ${count(own(comp, "kept")) ?? (Array.isArray(comp.kept) ? comp.kept.length : 0)}, changed ${count(own(comp, "changed")) ?? (Array.isArray(comp.changed) ? comp.changed.length : 0)}, added ${count(own(comp, "added")) ?? (Array.isArray(comp.added) ? comp.added.length : 0)}, removed ${count(own(comp, "removed")) ?? (Array.isArray(comp.removed) ? comp.removed.length : 0)}`
        : null;
      pushEntry("plan_review", {
        title: filled(own(pr, "title")) || `Plan Review · ${filled(own(pr, "work_id")) || "work"}`,
        statusLabel: gate,
        tone: passportOutcomeTone(gate),
        provenance: prov,
        summary: filled(own(pr, "summary")) || (gateLower === "unavailable" ? `Plan review unavailable (${filled(own(pr, "reason")) || "missing_plan"}). Never a pass.` : "Plan review precedes implementation admission; never substitutes for CI or code review."),
        identities: [filled(own(pr, "plan_id")), filled(own(pr, "receipt_identity")), filled(own(pr, "work_id"))].filter(Boolean),
        facts: [
          ["Gate / Outcome", gate],
          ...(filled(own(pr, "plan_id")) ? [["Plan ID", pr.plan_id]] : []),
          ...(count(own(pr, "plan_revision")) !== null ? [["Plan revision", String(pr.plan_revision)]] : count(own(pr, "revision_number")) !== null ? [["Plan revision", String(pr.revision_number)]] : []),
          ...(filled(own(pr, "reviewer")) ? [["Reviewer", pr.reviewer]] : []),
          ...(compSummary ? [["Comparison vs prior", compSummary]] : []),
          ...(Array.isArray(own(pr, "unresolved")) ? [["Unresolved blocking", String(pr.unresolved.length)]] : []),
        ],
      }, pr);
    }

    // 6. Semantic Merge O/A/B/M quartet observations (docs/semantic-merge-orchestration.md)
    const semanticSources = [
      ...(isRecord(own(data, "semantic_merge")) ? [data.semantic_merge] : []),
      ...(Array.isArray(own(data, "semantic_merges")) ? data.semantic_merges : []),
    ];
    for (const sm of semanticSources) {
      if (!isRecord(sm)) continue;
      const outcomeRaw = filled(own(sm, "outcome")) || filled(own(sm, "status")) || "Unknown";
      const modeRaw = filled(own(sm, "mode")) || "Required";
      const isStaleOutcome = /^stale/i.test(outcomeRaw) || own(sm, "revalidation_moved") === true;
      const hasReceipt = Boolean(filled(own(sm, "receipt_identity")) || filled(own(sm, "request_identity")) || filled(own(sm, "checker")));
      const prov = own(sm, "provenance")
        ? normalizeProvenance(sm.provenance, hasReceipt ? "observed" : "declared")
        : (own(sm, "self_reported") === true ? PROVENANCE.agent_claim : hasReceipt && !["unknown", "pending"].includes(outcomeRaw.toLowerCase()) ? PROVENANCE.observed : PROVENANCE.declared);
      const quartet = [
        quartetSlot("O (Base)", own(sm, "base") ?? own(sm, "O")),
        quartetSlot("A (Variant A)", own(sm, "variant_a") ?? own(sm, "A")),
        quartetSlot("B (Variant B)", own(sm, "variant_b") ?? own(sm, "B")),
        quartetSlot("M (Candidate)", own(sm, "candidate") ?? own(sm, "M")),
      ];
      pushEntry("semantic_merge", {
        title: filled(own(sm, "title")) || "Semantic Merge Quartet (O/A/B/M)",
        statusLabel: `${outcomeRaw} (${modeRaw})`,
        tone: passportOutcomeTone(outcomeRaw),
        provenance: prov,
        stale: isStaleOutcome,
        summary: own(sm, "conflict_demonstrated") === true
          ? "Counterexample demonstrated on the frozen O/A/B/M quartet. Grants no merge authority."
          : "Bound to frozen O/A/B/M quartet. Downstream revalidation required; grants no merge authority.",
        identities: [filled(own(sm, "request_identity")), filled(own(sm, "receipt_identity")), filled(own(sm, "attempt_binding"))].filter(Boolean),
        quartet,
        facts: [
          ["Outcome", outcomeRaw],
          ["Policy mode", modeRaw],
          ...(filled(own(sm, "lineage_policy")) ? [["Lineage policy", sm.lineage_policy]] : []),
          ...(filled(own(sm, "checker")) ? [["Checker", sm.checker]] : []),
          ...(filled(own(sm, "request_identity")) ? [["Request binding", sm.request_identity]] : []),
          ["Mutation authority", "None (grants_mutation_authority = false)"],
        ],
      }, { ...sm, candidate_id: filled(own(sm, "candidate_id")) || quartet[3].revision });
    }

    const counts = {
      observed: entries.filter((e) => e.provenance.key === "observed").length,
      agent_claim: entries.filter((e) => e.provenance.key === "agent-claim").length,
      declared: entries.filter((e) => e.provenance.key === "declared").length,
      stale: entries.filter((e) => e.stale).length,
      total: entries.length,
    };

    return Object.freeze({
      currentCandidateId,
      currentRevision,
      entries: Object.freeze(entries),
      counts: Object.freeze(counts),
      canonicalCompletionPassed,
      hasAgentHandoffOnly: agentHandoffs.length > 0 && !canonicalCompletionPassed,
    });
  }

  function passportCopyableId(value, labelPrefix = "identity") {
    const id = String(value ?? "");
    const wrap = element("span", undefined, "passport-identity");
    const prefix = element("code", id.length > SHORT_HASH ? id.slice(0, SHORT_HASH) : id, "causal-identity-prefix passport-id-prefix");
    prefix.title = id;
    prefix.dataset.fullIdentity = id;
    wrap.append(prefix);
    if (id.length > SHORT_HASH) {
      wrap.append(element("span", "…", "small"));
    }
    wrap.append(copyControl(id, `Copy ${labelPrefix} ${short(id)}`, `${labelPrefix} copied.`));
    return wrap;
  }

  function renderGatePassport(data, options = {}) {
    const model = normalizeGatePassport(data);
    const root = element("section", undefined, "gate-passport");
    root.setAttribute("aria-label", options.title || "Gate & Receipt Passport");

    const head = element("div", undefined, "passport-head");
    const titleWrap = element("div", undefined, "passport-title-wrap");
    titleWrap.append(
      element("p", "Provenance & verification ledger", "eyebrow"),
      element("h3", options.title || "Gate & Receipt Passport", "passport-title"),
    );
    const summaryStrip = element("div", undefined, "passport-summary-badges");
    const countItems = [
      [PROVENANCE.observed, model.counts.observed],
      [PROVENANCE.agent_claim, model.counts.agent_claim],
      [PROVENANCE.declared, model.counts.declared],
    ];
    for (const [prov, total] of countItems) {
      const b = element("span", `${prov.label}: ${total}`, "passport-badge");
      b.dataset.provenance = prov.key;
      b.title = prov.description;
      summaryStrip.append(b);
    }
    if (model.counts.stale > 0) {
      const staleSummary = badge(`historical/stale: ${model.counts.stale}`, "warning", "Entries bound to a superseded or non-current candidate.");
      staleSummary.classList.add("passport-stale-badge");
      summaryStrip.append(staleSummary);
    }
    head.append(titleWrap, summaryStrip);
    root.append(head);

    const legendStrip = element("div", undefined, "passport-legend");
    legendStrip.setAttribute("role", "note");
    legendStrip.setAttribute("aria-label", "Provenance border legend");
    for (const prov of [PROVENANCE.observed, PROVENANCE.agent_claim, PROVENANCE.declared]) {
      const item = element("span", prov.label, "passport-legend-item");
      item.dataset.provenance = prov.key;
      item.title = prov.description;
      legendStrip.append(item);
    }
    root.append(legendStrip);

    if (model.currentCandidateId || model.currentRevision) {
      const bindingRows = [];
      if (model.currentCandidateId) bindingRows.push(["Current candidate", passportCopyableId(model.currentCandidateId, "candidate")]);
      if (model.currentRevision) bindingRows.push(["Current revision", passportCopyableId(model.currentRevision, "revision")]);
      const bindingBox = facts(bindingRows);
      bindingBox.classList.add("passport-binding");
      root.append(bindingBox);
    }

    if (!model.entries.length) {
      root.append(emptyState("No gate or receipt observations", "No typed CI checks, cairn-authoritative-completion receipts, Patchgate decisions, SLSA statements, Plan Review receipts, or Semantic Merge observations are recorded for this candidate."));
      return root;
    }

    for (const [catKey, catLabel] of PASSPORT_CATEGORIES) {
      const catEntries = model.entries.filter((e) => e.category === catKey);
      if (!catEntries.length) continue;
      const group = element("section", undefined, "passport-category");
      group.dataset.category = catKey;
      group.append(element("h4", `${catLabel} (${catEntries.length})`, "passport-category-title"));
      const list = element("div", undefined, "passport-entries");
      for (const entry of catEntries) {
        const card = element("article", undefined, "passport-entry");
        card.dataset.provenance = entry.provenance.key;
        card.dataset.category = entry.category;
        if (entry.stale) card.dataset.stale = "true";
        if (entry.is_current) card.dataset.current = "true";

        const cardHead = element("div", undefined, "passport-entry-head");
        const provBadge = element("span", entry.provenance.label, "passport-badge");
        provBadge.dataset.provenance = entry.provenance.key;
        provBadge.title = entry.provenance.description;
        cardHead.append(provBadge, pill({ key: entry.tone, label: entry.statusLabel, tone: entry.tone, meaning: entry.summary }));
        if (entry.stale) {
          const staleBadge = badge("historical/stale", "warning", "Bound to a superseded or non-current candidate.");
          staleBadge.classList.add("passport-stale-badge");
          cardHead.append(staleBadge);
        } else if (entry.is_current) {
          const curBadge = badge("current", "good", "Bound to the active candidate.");
          curBadge.classList.add("passport-current-badge");
          cardHead.append(curBadge);
        }
        cardHead.append(element("strong", entry.title, "passport-entry-title"));
        card.append(cardHead);
        if (entry.summary) card.append(element("p", entry.summary, "small passport-entry-summary"));

        const factRows = [...(entry.facts || [])];
        if (entry.identities?.length) {
          const idWrap = element("div", undefined, "passport-id-list");
          for (const id of entry.identities.slice(0, 4)) idWrap.append(passportCopyableId(id, "digest"));
          factRows.push(["Identities", idWrap]);
        }
        if (factRows.length) card.append(facts(factRows));

        if (entry.subjects?.length || entry.dependencies?.length) {
          const digestWrap = element("div", undefined, "passport-digests");
          for (const [subTitle, rows] of [["Subjects", entry.subjects || []], ["Resolved dependencies", entry.dependencies || []]]) {
            if (!rows.length) continue;
            const sub = element("div", undefined, "passport-digest-group");
            sub.append(element("p", `${subTitle} (${rows.length})`, "eyebrow"));
            const ul = element("ul", undefined, "passport-digest-list");
            for (const row of rows) {
              const li = element("li");
              li.append(element("code", row.name), document.createTextNode(" · "), element("code", row.digests.join(" ") || "Unavailable"));
              ul.append(li);
            }
            sub.append(ul);
            digestWrap.append(sub);
          }
          card.append(digestWrap);
        }

        if (entry.quartet?.length) {
          const qWrap = element("div", undefined, "passport-quartet");
          for (const slot of entry.quartet) {
            const cell = element("div", undefined, "passport-quartet-cell");
            cell.append(element("span", slot.role, "eyebrow"));
            cell.append(slot.revision ? passportCopyableId(slot.revision, slot.role) : element("span", "Not recorded", "not-recorded"));
            if (slot.canonical_tree) {
              const tree = element("span", `tree ${short(slot.canonical_tree)}`, "small mono");
              tree.title = slot.canonical_tree;
              cell.append(tree);
            }
            if (slot.channel) cell.append(element("span", slot.channel, "small"));
            qWrap.append(cell);
          }
          card.append(qWrap);
        }

        card.append(disclosure("Raw passport record", element("pre", JSON.stringify(entry.raw, null, 2), "identity"), "passport-raw"));
        list.append(card);
      }
      group.append(list);
      root.append(group);
    }

    return root;
  }

  window.KilnReceipts = Object.freeze({
    load, clear, renderRun,
    renderGatePassport, normalizeGatePassport, provenanceTypes: PROVENANCE,
    decisions: DECISIONS, filters: FILTERS, decision, tally, nativeContext, decider, listRows, matches, normalizeRoute,
  });
})();
