"use strict";

// Shared presentation helpers for the live console. Pure DOM/format builders:
// no network, no storage, no globals besides window.KilnUi. Data is always
// written through textContent or attributes, never parsed as markup.
// r[impl kiln.web.readable_observations]
// r[impl kiln.web.readable_observations.scenario.failed]
// r[impl kiln.web.readable_observations.scenario.unknown]
// r[impl kiln.web.visual_design]
(() => {
  const SECOND_MS = 1000;
  const MINUTE_MS = 60 * SECOND_MS;
  const HOUR_MS = 60 * MINUTE_MS;
  const DAY_MS = 24 * HOUR_MS;
  const ONE_DECIMAL_LIMIT_MS = 10 * SECOND_MS;
  const JUST_NOW_LIMIT_MS = 45 * SECOND_MS;
  const RELATIVE_LIMIT_MS = 30 * DAY_MS;
  const NOT_RECORDED = "Not recorded";
  const SVG_NS = "http://www.w3.org/2000/svg";

  const exactNumber = new Intl.NumberFormat("en-US", { maximumFractionDigits: 3 });
  const localDateTime = new Intl.DateTimeFormat(undefined, {
    year: "numeric", month: "short", day: "numeric",
    hour: "2-digit", minute: "2-digit", second: "2-digit", timeZoneName: "short",
  });
  const shortDate = new Intl.DateTimeFormat(undefined, { year: "numeric", month: "short", day: "numeric" });
  const dayDate = new Intl.DateTimeFormat(undefined, { weekday: "short", year: "numeric", month: "short", day: "numeric" });

  // Canonical outcome -> [semantic status, human label].
  const OUTCOMES = Object.freeze({
    Success: ["success", "Passed"],
    DomainFailure: ["failure", "Failed"],
    Denied: ["failure", "Denied"],
    ProviderError: ["failure", "Provider error"],
    TimedOut: ["failure", "Timed out"],
    Cancelled: ["cancelled", "Cancelled"],
    Unknown: ["unknown", "Unknown"],
  });

  function outcomeStatus(outcome, phase) {
    if ((outcome === null || outcome === undefined || outcome === "") && phase === "queued") return { status: "queued", label: "Queued" };
    if (outcome === null || outcome === undefined || outcome === "") return { status: "unknown", label: NOT_RECORDED };
    if (outcome === "Pending") return phase === "active" ? { status: "running", label: "Running" } : { status: "queued", label: "Queued" };
    if (typeof outcome === "string" && Object.hasOwn(OUTCOMES, outcome)) {
      const [status, label] = OUTCOMES[outcome];
      return { status, label };
    }
    return { status: "unknown", label: String(outcome) };
  }

  // Each glyph is a list of [tag, attributes]; strokes inherit currentColor.
  const ICONS = Object.freeze({
    success: [["circle", { cx: 12, cy: 12, r: 9 }], ["path", { d: "m8 12.5 2.7 2.7L16.2 9.5" }]],
    failure: [["circle", { cx: 12, cy: 12, r: 9 }], ["path", { d: "m9 9 6 6m0-6-6 6" }]],
    running: [["circle", { cx: 12, cy: 12, r: 9 }], ["path", { d: "M12 3a9 9 0 0 1 0 18Z", fill: "currentColor", stroke: "none" }]],
    queued: [["circle", { cx: 12, cy: 12, r: 9 }], ["path", { d: "M12 7.5V12l3 2" }]],
    cancelled: [["circle", { cx: 12, cy: 12, r: 9 }], ["path", { d: "M5.7 18.3 18.3 5.7" }]],
    unknown: [["circle", { cx: 12, cy: 12, r: 9 }], ["path", { d: "M9.6 9.4a2.5 2.5 0 1 1 3.6 2.3c-.7.3-1.2 1-1.2 1.7v.4M12 16.8h.01" }]],
    warning: [["path", { d: "M12 3.5 2.5 20h19Z" }], ["path", { d: "M12 10v4.2m0 2.8h.01" }]],
    refresh: [["path", { d: "M19.5 10.5A7.7 7.7 0 0 0 6 6.6L4 8.5" }], ["path", { d: "M4 4v4.5h4.5" }], ["path", { d: "M4.5 13.5A7.7 7.7 0 0 0 18 17.4l2-1.9" }], ["path", { d: "M20 20v-4.5h-4.5" }]],
    search: [["circle", { cx: 11, cy: 11, r: 6.5 }], ["path", { d: "m20 20-4.4-4.4" }]],
    more: [["circle", { cx: 5.5, cy: 12, r: 1.3, fill: "currentColor", stroke: "none" }], ["circle", { cx: 12, cy: 12, r: 1.3, fill: "currentColor", stroke: "none" }], ["circle", { cx: 18.5, cy: 12, r: 1.3, fill: "currentColor", stroke: "none" }]],
    copy: [["rect", { x: 9, y: 9, width: 11, height: 11, rx: 2 }], ["path", { d: "M15 5.5V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h.5" }]],
    chevron: [["path", { d: "m6.5 9.5 5.5 5.5 5.5-5.5" }]],
    external: [["path", { d: "M14 4h6v6M20 4l-8.5 8.5" }], ["path", { d: "M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5" }]],
    close: [["path", { d: "m6 6 12 12M18 6 6 18" }]],
  });

  function icon(name) {
    const svg = document.createElementNS(SVG_NS, "svg");
    for (const [key, value] of [["viewBox", "0 0 24 24"], ["aria-hidden", "true"], ["focusable", "false"], ["class", "icon"], ["fill", "none"], ["stroke", "currentColor"], ["stroke-width", "1.8"], ["stroke-linecap", "round"], ["stroke-linejoin", "round"]]) svg.setAttribute(key, value);
    for (const [tag, attributes] of ICONS[Object.hasOwn(ICONS, name) ? name : "unknown"]) {
      const shape = document.createElementNS(SVG_NS, tag);
      for (const [key, value] of Object.entries(attributes)) shape.setAttribute(key, String(value));
      svg.append(shape);
    }
    return svg;
  }

  function hidden(content) {
    const node = document.createElement("span");
    node.className = "sr-only";
    node.textContent = content;
    return node;
  }

  function statusPill(outcome, phase) {
    const { status, label } = outcomeStatus(outcome, phase);
    const pill = document.createElement("span");
    pill.className = "status-pill";
    pill.dataset.status = status;
    const text = document.createElement("span");
    text.className = "status-pill-label";
    text.textContent = label;
    pill.append(icon(status), text);
    const parts = [];
    if (outcome !== null && outcome !== undefined && outcome !== "") {
      if (String(outcome) !== label) parts.push(String(outcome));
      if (outcome === "Pending" && phase) parts.push(`phase ${phase}`);
    } else if (label !== NOT_RECORDED) parts.push(`outcome not recorded, phase ${phase}`);
    const exact = parts.join(", ");
    const name = `Outcome: ${label}${exact ? ` (${exact})` : ""}`;
    pill.setAttribute("role", "img");
    pill.setAttribute("aria-label", name);
    pill.title = name;
    return pill;
  }

  function validDuration(ms) {
    return typeof ms === "number" && Number.isFinite(ms) && ms >= 0;
  }
  function formatDuration(ms) {
    if (!validDuration(ms)) return NOT_RECORDED;
    if (ms < ONE_DECIMAL_LIMIT_MS) return `${(Math.floor(ms / 100) / 10).toFixed(1)}s`;
    if (ms < MINUTE_MS) return `${Math.floor(ms / SECOND_MS)}s`;
    if (ms < HOUR_MS) return `${Math.floor(ms / MINUTE_MS)}m ${String(Math.floor((ms % MINUTE_MS) / SECOND_MS)).padStart(2, "0")}s`;
    return `${Math.floor(ms / HOUR_MS)}h ${String(Math.floor((ms % HOUR_MS) / MINUTE_MS)).padStart(2, "0")}m`;
  }
  function durationElement(ms) {
    const span = document.createElement("span");
    span.className = "duration";
    span.textContent = formatDuration(ms);
    if (!validDuration(ms)) {
      span.classList.add("not-recorded");
      span.title = "Duration not recorded";
      return span;
    }
    const exact = `${exactNumber.format(ms)} ms`;
    span.title = exact;
    span.dataset.ms = String(ms);
    span.append(hidden(` (${exact})`));
    return span;
  }

  function instant(value) {
    let ms = NaN;
    if (typeof value === "number") ms = value;
    else if (typeof value === "string" && value.trim()) ms = /^-?\d+$/.test(value.trim()) ? Number(value) : Date.parse(value);
    else if (value instanceof Date) ms = value.getTime();
    if (!Number.isFinite(ms)) return null;
    const date = new Date(ms);
    return Number.isNaN(date.valueOf()) ? null : date;
  }
  function formatRelative(value, now = Date.now()) {
    const date = instant(value);
    const reference = instant(now);
    if (!date || !reference) return NOT_RECORDED;
    const delta = reference.getTime() - date.getTime();
    const distance = Math.abs(delta);
    if (distance < JUST_NOW_LIMIT_MS) return "just now";
    if (distance > RELATIVE_LIMIT_MS) return shortDate.format(date);
    // Round to the nearest unit; switch units before rounding would reach the next one (no "60m").
    const amount = distance < HOUR_MS - MINUTE_MS / 2 ? `${Math.max(1, Math.round(distance / MINUTE_MS))}m` : distance < DAY_MS - HOUR_MS / 2 ? `${Math.round(distance / HOUR_MS)}h` : `${Math.round(distance / DAY_MS)}d`;
    return delta >= 0 ? `${amount} ago` : `in ${amount}`;
  }
  function timeElement(value) {
    const date = instant(value);
    if (!date) {
      const span = document.createElement("span");
      span.className = "relative-time not-recorded";
      span.textContent = NOT_RECORDED;
      return span;
    }
    const iso = date.toISOString();
    const exact = `${localDateTime.format(date)} · ${iso}`;
    const time = document.createElement("time");
    time.className = "relative-time";
    time.dateTime = iso;
    time.title = exact;
    time.textContent = formatRelative(date.getTime());
    time.append(hidden(` (${exact})`));
    return time;
  }

  // r[impl kiln.web.failure_first_run] Conservative, bounded error-line recognition. Presentation only:
  // callers highlight the returned line indexes over inert text and never alter the retained bytes.
  const MAX_ERROR_LINES = 1000;
  const MAX_ERROR_LINE_CHARS = 2048;
  // A word that is not part of an identifier, path, file name or hyphenated name ("error-chain", "error.rs", "error_code").
  const word = (pattern, flags = "i") => new RegExp(`(?<![\\w./\`-])(?:${pattern})(?![\\w/\`-]|\\.\\w)`, flags);
  const STRONG_ERROR_PATTERNS = Object.freeze([
    /(?<![\w./-])error(?:\[[A-Za-z]?\d+\])?:/i,
    /^\s*(?:error|fatal)\b/i,
    /\bpanicked at\b/,
    word("FAILED", ""),
    /^\s*Traceback \(most recent call last\)/,
    /(?<![\w.])[A-Z]\w*(?:Error|Exception):/,
    /\bexit(?:ed with)?(?: exit)? (?:code|status)\D{0,3}[1-9]/i,
  ]);
  const WEAK_ERROR_PATTERN = word("error|failed|fatal");
  const BENIGN_ERROR_PATTERN = /\b(?:0|no|zero|without) (?:errors?|failed|failures?|fatal)\b|\b(?:errors?|failed|failures?)[:=] ?0\b/i;
  function isErrorLine(line) {
    const sample = line.length > MAX_ERROR_LINE_CHARS ? line.slice(0, MAX_ERROR_LINE_CHARS) : line;
    if (STRONG_ERROR_PATTERNS.some((pattern) => pattern.test(sample))) return true;
    return WEAK_ERROR_PATTERN.test(sample) && !BENIGN_ERROR_PATTERN.test(sample);
  }
  function errorLines(text) {
    if (typeof text !== "string" || !text) return [];
    const lines = text.split("\n");
    const result = [];
    for (let index = 0; index < lines.length && result.length < MAX_ERROR_LINES; index++) if (isErrorLine(lines[index])) result.push(index);
    return result;
  }

  // r[impl kiln.web.project_page.scenario.untimed] Trend points: terminal runs with a recorded duration only,
  // oldest first by service ordinal. Terminal runs without timing are counted as excluded; in-progress runs are neither.
  function durationPoints(runs) {
    if (!Array.isArray(runs)) return Object.freeze({ points: Object.freeze([]), excluded: 0 });
    const ordered = [];
    const unordered = [];
    let excluded = 0;
    runs.forEach((run, index) => {
      if (!run || typeof run !== "object") return;
      const outcome = run.outcome;
      if (typeof outcome !== "string" || !outcome || outcome === "Pending") return;
      if (!Number.isSafeInteger(run.duration_ms) || run.duration_ms < 0) { excluded++; return; }
      const at = [run.completed_at_ms, run.started_at_ms, run.created_at_ms].find((value) => Number.isSafeInteger(value));
      const point = Object.freeze({ id: run.id, duration_ms: run.duration_ms, outcome, at_ms: at ?? null });
      (Number.isSafeInteger(run.ordinal) ? ordered : unordered).push({ point, ordinal: run.ordinal, index });
    });
    ordered.sort((a, b) => a.ordinal - b.ordinal || a.index - b.index);
    return Object.freeze({ points: Object.freeze([...ordered, ...unordered].map((entry) => entry.point)), excluded });
  }

  // r[impl kiln.web.live_progress] Elapsed time from a recorded start; null (shown as Not recorded) when no start is recorded.
  function elapsedSince(start, now = Date.now()) {
    const begin = instant(start);
    const reference = instant(now);
    if (!begin || !reference) return null;
    return Math.max(0, reference.getTime() - begin.getTime());
  }
  const FINISHED_NODE_STATES = new Set(["Completed", "Failed", "Cancelled", "Skipped"]);
  function nodeProgress(nodes) {
    const rows = Array.isArray(nodes) ? nodes.filter((node) => node && typeof node === "object") : [];
    return Object.freeze({ complete: rows.filter((node) => FINISHED_NODE_STATES.has(node.state)).length, total: rows.length });
  }

  // r[impl kiln.web.runs_table] Local calendar day for grouping; null when no instant is recorded.
  function localDay(value, now = Date.now()) {
    const date = instant(value);
    const reference = instant(now);
    if (!date || !reference) return null;
    const key = (day) => `${day.getFullYear()}-${String(day.getMonth() + 1).padStart(2, "0")}-${String(day.getDate()).padStart(2, "0")}`;
    const yesterday = new Date(reference.getFullYear(), reference.getMonth(), reference.getDate() - 1);
    const dayKey = key(date);
    const label = dayKey === key(reference) ? "Today" : dayKey === key(yesterday) ? "Yesterday" : dayDate.format(date);
    return Object.freeze({ key: dayKey, label });
  }

  window.KilnUi = Object.freeze({ outcomeStatus, statusPill, formatDuration, durationElement, formatRelative, timeElement, icon, errorLines, durationPoints, elapsedSince, nodeProgress, localDay });
})();
