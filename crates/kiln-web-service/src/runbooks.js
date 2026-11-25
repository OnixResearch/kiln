// Runbook dispatch & parent fan-out console view: lists reviewed runbooks,
// validates typed parameters and allowed targets (single or enrolled-project
// fan-out), submits audited dispatches, and inspects parent fan-out runs with
// concurrency/budget caps, per-child outcome ledgers, and admission-only
// pause/continue/cancel controls.
(() => {
  "use strict";

  const RUNBOOK_ID_MAX = 64;
  const FANOUT_ID_RE = /^[A-Za-z0-9._-]{1,128}$/;
  const REQUEST_ID_RE = /^[A-Za-z0-9._-]{1,128}$/;
  const RELATIVE_PATH_RE = /^[A-Za-z0-9._/-]+$/;
  const INT_RE = /^-?(?:0|[1-9][0-9]*)$/;
  const MAX_FANOUT_TARGETS = 64;

  let generation = 0;
  let busy = false;
  let refreshPreview = null;

  const ui = () => window.KilnUi;
  const element = (tag, value, className) => {
    const node = document.createElement(tag);
    if (value !== undefined) node.textContent = String(value);
    if (className) node.className = className;
    return node;
  };
  const isRecord = (value) => value !== null && typeof value === "object" && !Array.isArray(value);
  const text = (value) => typeof value === "string" ? value : "";
  const currentEnrolledProjects = (ctx) =>
    typeof ctx?.getEnrolledProjects === "function" ? ctx.getEnrolledProjects() : ctx?.enrolledProjects;
  const utf8Length = (value) => {
    if (ui()?.utf8Bytes) return ui().utf8Bytes(value);
    return new TextEncoder().encode(String(value)).length;
  };

  function normalizeRoute(route) {
    const raw = typeof route?.runbook === "string" ? route.runbook : "";
    const trimmed = raw.trim();
    const runbook = trimmed.length <= RUNBOOK_ID_MAX && /^[a-z][a-z0-9-]{0,63}$/.test(trimmed)
      ? trimmed
      : "";
    if (!isRecord(route)) {
      return { runbook };
    }
    const out = { runbook };
    if (Object.hasOwn(route, "fanout")) {
      const rawFanout = typeof route.fanout === "string" ? route.fanout.trim() : "";
      out.fanout = FANOUT_ID_RE.test(rawFanout) ? rawFanout : "";
    }
    if (Object.hasOwn(route, "dispatch")) {
      const rawDispatch = typeof route.dispatch === "string" ? route.dispatch.trim() : "";
      out.dispatch = FANOUT_ID_RE.test(rawDispatch) ? rawDispatch : "";
    }
    if (Object.hasOwn(route, "rollup_cursor")) {
      const rawCursor = typeof route.rollup_cursor === "string" ? route.rollup_cursor.trim() : "";
      let safe = rawCursor.length <= 512;
      for (let i = 0; i < rawCursor.length && safe; i += 1) {
        const code = rawCursor.charCodeAt(i);
        if (code < 0x20 || code === 0x7f) safe = false;
      }
      out.rollup_cursor = safe ? rawCursor : "";
    }
    return out;
  }

  function parseTarget(raw) {
    if (typeof raw !== "string" || !raw || raw.length > 1280) {
      return { error: "Target must be REPO@REF." };
    }
    for (let i = 0; i < raw.length; i += 1) {
      const code = raw.charCodeAt(i);
      if (code <= 0x20 || code === 0x7f) {
        return { error: "Target must not contain whitespace or control characters." };
      }
    }
    const at = raw.indexOf("@");
    if (at <= 0 || at === raw.length - 1 || raw.indexOf("@", at + 1) !== -1) {
      return { error: "Target must be REPO@REF." };
    }
    const repository = raw.slice(0, at);
    const reference = raw.slice(at + 1);
    if (repository.length > 256 || reference.length > 1024 || reference.startsWith("-") || reference.includes("..")) {
      return { error: "Target repository or reference is outside allowed bounds." };
    }
    return { repository, reference, raw };
  }

  function validTargetGlob(pattern) {
    if (typeof pattern !== "string" || !pattern || pattern.length > 256) return false;
    for (let i = 0; i < pattern.length; i += 1) {
      const code = pattern.charCodeAt(i);
      if (code <= 0x20 || code >= 0x7f) return false;
    }
    if (pattern.includes("@") || pattern.includes("\\") || pattern.includes("[") || pattern.includes("]") || pattern.includes("{") || pattern.includes("}")) {
      return false;
    }
    const parts = pattern.split("/");
    return parts.every((part) => part.length > 0 && part !== "." && part !== ".." && (!part.includes("**") || part === "**"));
  }

  function segmentMatches(pattern, value) {
    let pi = 0;
    let vi = 0;
    let starPi = -1;
    let starVi = 0;
    while (vi < value.length) {
      if (pi < pattern.length && (pattern[pi] === "?" || pattern[pi] === value[vi])) {
        pi += 1;
        vi += 1;
      } else if (pi < pattern.length && pattern[pi] === "*") {
        starPi = pi;
        starVi = vi;
        pi += 1;
      } else if (starPi !== -1) {
        pi = starPi + 1;
        starVi += 1;
        vi = starVi;
      } else {
        return false;
      }
    }
    while (pi < pattern.length && pattern[pi] === "*") pi += 1;
    return pi === pattern.length;
  }

  function pathMatches(pattern, path) {
    const patParts = pattern.split("/");
    const valParts = path.split("/");
    let pi = 0;
    let vi = 0;
    let starPi = -1;
    let starVi = 0;
    while (vi < valParts.length) {
      if (pi < patParts.length && patParts[pi] === "**") {
        starPi = pi;
        starVi = vi;
        pi += 1;
      } else if (pi < patParts.length && segmentMatches(patParts[pi], valParts[vi])) {
        pi += 1;
        vi += 1;
      } else if (starPi !== -1) {
        pi = starPi + 1;
        starVi += 1;
        vi = starVi;
      } else {
        return false;
      }
    }
    while (pi < patParts.length && patParts[pi] === "**") pi += 1;
    return pi === patParts.length;
  }

  function globMatches(pattern, repository, projectId) {
    if (!validTargetGlob(pattern)) return false;
    if (pattern.includes("/")) {
      return pathMatches(pattern.toLowerCase(), text(repository).toLowerCase());
    }
    return pathMatches(pattern, text(projectId));
  }

  function parseExplicitTargets(raw, maxTargets = MAX_FANOUT_TARGETS) {
    const textVal = typeof raw === "string" ? raw : "";
    const items = textVal
      .split(/[\n,]+/)
      .map((item) => item.trim())
      .filter((item) => item.length > 0);
    if (!items.length) {
      return { targets: [], errors: ["Enter at least one target (REPO@REF or project selector)."] };
    }
    if (items.length > maxTargets) {
      return { targets: [], errors: [`At most ${maxTargets} targets are allowed.`] };
    }
    const errors = [];
    const seen = new Set();
    const targets = [];
    for (const item of items) {
      if (item.includes("*") || item.includes("?")) {
        errors.push(`Wildcard target '${item}' must be entered in Enrolled-project glob mode.`);
        continue;
      }
      if (item.includes("@")) {
        const parsed = parseTarget(item);
        if (parsed.error) {
          errors.push(`${item}: ${parsed.error}`);
          continue;
        }
      } else {
        let bad = item.length > 256;
        for (let i = 0; i < item.length && !bad; i += 1) {
          const code = item.charCodeAt(i);
          if (code <= 0x20 || code === 0x7f) bad = true;
        }
        if (bad) {
          errors.push(`Target '${item}' contains whitespace, control characters, or exceeds 256 bytes.`);
          continue;
        }
      }
      if (seen.has(item)) {
        errors.push(`Duplicate target '${item}'.`);
        continue;
      }
      seen.add(item);
      targets.push(item);
    }
    return { targets, errors };
  }

  function matchEnrolledTargets(runbook, enrolledProjects, rawGlob) {
    const input = typeof rawGlob === "string" ? rawGlob.trim() : "";
    if (!input) {
      return { matched: [], excludedUnenrolled: [], error: "Enter a target glob pattern (e.g. onixresearch/*)." };
    }
    for (let i = 0; i < input.length; i += 1) {
      const code = input.charCodeAt(i);
      if (code <= 0x20 || code === 0x7f) {
        return { matched: [], excludedUnenrolled: [], error: "Target glob must not contain whitespace or control characters." };
      }
    }
    let pattern = input;
    let refSpec = "";
    const at = input.indexOf("@");
    if (at !== -1) {
      pattern = input.slice(0, at);
      refSpec = input.slice(at + 1);
      if (!pattern || !refSpec || refSpec.includes("@") || refSpec.startsWith("-") || refSpec.includes("..") || /^[0-9a-f]{40}$/.test(refSpec)) {
        return { matched: [], excludedUnenrolled: [], error: "Glob @REF suffix is invalid or specifies a raw commit SHA." };
      }
    }
    if (!validTargetGlob(pattern)) {
      return { matched: [], excludedUnenrolled: [], error: "Target glob pattern is invalid." };
    }
    const enrolledIds = new Set(
      (Array.isArray(enrolledProjects) ? enrolledProjects : [])
        .map((p) => (typeof p === "string" ? p : text(p?.id)))
        .filter(Boolean),
    );
    const allowed = Array.isArray(runbook?.allowed_targets) ? runbook.allowed_targets : [];
    const matched = [];
    const excludedUnenrolled = [];
    for (const target of allowed) {
      if (!globMatches(pattern, target?.repository, target?.project_id)) continue;
      if (enrolledIds.has(text(target?.project_id))) {
        matched.push({
          project_id: text(target.project_id),
          repository: text(target.repository),
          ref: refSpec || (Array.isArray(target.references) && target.references[0]) || "",
        });
      } else {
        excludedUnenrolled.push({
          project_id: text(target?.project_id),
          repository: text(target?.repository),
        });
      }
    }
    return { matched, excludedUnenrolled, error: "" };
  }

  function validRelativePath(value, maxBytes) {
    if (typeof value !== "string" || !value || utf8Length(value) > maxBytes) return false;
    if (!RELATIVE_PATH_RE.test(value)) return false;
    const parts = value.split("/");
    return parts.every((part) => part.length > 0 && part !== ".." && !part.startsWith("-"));
  }

  function coerceParams(declarations, rawValues) {
    const params = {};
    const errors = [];
    const list = Array.isArray(declarations) ? declarations : [];
    const values = isRecord(rawValues) ? rawValues : {};

    for (const decl of list) {
      const name = text(decl?.name);
      if (!name) continue;
      const raw = Object.hasOwn(values, name) ? values[name] : "";
      const hasRaw = raw !== undefined && raw !== null && String(raw) !== "";
      if (!hasRaw) {
        if (decl.required) {
          errors.push(`Parameter ${name} is required.`);
        }
        continue;
      }
      if (decl.kind === "bool") {
        if (raw === true || raw === "true") params[name] = true;
        else if (raw === false || raw === "false") params[name] = false;
        else errors.push(`Parameter ${name} must be true or false.`);
      } else if (decl.kind === "integer") {
        const textVal = typeof raw === "number" ? String(raw) : String(raw).trim();
        if (!INT_RE.test(textVal)) {
          errors.push(`Parameter ${name} must be an integer.`);
          continue;
        }
        const parsed = Number(textVal);
        if (!Number.isSafeInteger(parsed)) {
          errors.push(`Parameter ${name} is outside safe integer bounds.`);
          continue;
        }
        if (Number.isSafeInteger(decl.integer_min) && parsed < decl.integer_min) {
          errors.push(`Parameter ${name} must be at least ${decl.integer_min}.`);
          continue;
        }
        if (Number.isSafeInteger(decl.integer_max) && parsed > decl.integer_max) {
          errors.push(`Parameter ${name} must be at most ${decl.integer_max}.`);
          continue;
        }
        params[name] = parsed;
      } else if (decl.kind === "enum") {
        const str = String(raw);
        const allowed = Array.isArray(decl.enum_values) ? decl.enum_values : [];
        if (!allowed.includes(str)) {
          errors.push(`Parameter ${name} must be one of: ${allowed.join(", ")}.`);
          continue;
        }
        params[name] = str;
      } else if (decl.kind === "relative_path") {
        const str = String(raw);
        const maxBytes = Number.isSafeInteger(decl.relative_path_max_bytes) ? decl.relative_path_max_bytes : 256;
        if (!validRelativePath(str, maxBytes)) {
          errors.push(`Parameter ${name} must be a safe relative path (<= ${maxBytes} bytes).`);
          continue;
        }
        params[name] = str;
      } else {
        errors.push(`Parameter ${name} has unsupported type.`);
      }
    }
    return { params, errors };
  }

  function makeRequestId(prefix) {
    if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
      return `${prefix}-${crypto.randomUUID()}`;
    }
    return `${prefix}-${Date.now()}`;
  }

  function validateInputs(runbook, input) {
    const errors = [];
    const targetResult = parseTarget(input?.target);
    if (targetResult.error) errors.push(targetResult.error);
    const paramResult = coerceParams(runbook?.params, input?.params);
    errors.push(...paramResult.errors);
    const rawRequestId = typeof input?.requestId === "string" ? input.requestId.trim() : "";
    let requestId = rawRequestId;
    if (requestId) {
      if (!REQUEST_ID_RE.test(requestId)) {
        errors.push("Request ID must use 1-128 ASCII letters, digits, '.', '_', or '-'.");
      }
    } else {
      requestId = makeRequestId("ui-rb");
    }
    return {
      ok: errors.length === 0,
      errors,
      payload: {
        request_id: requestId,
        target: targetResult.raw || "",
        params: paramResult.params,
      },
    };
  }

  function validateFanoutInputs(runbook, input) {
    const errors = [];
    const cfg = isRecord(runbook?.fanout) ? runbook.fanout : null;
    const maxTargets = Number.isSafeInteger(cfg?.max_targets) ? cfg.max_targets : MAX_FANOUT_TARGETS;
    const maxConcLimit = Number.isSafeInteger(cfg?.max_concurrency_limit) ? cfg.max_concurrency_limit : 32;
    const maxBudgetLimit = Number.isSafeInteger(cfg?.max_budget_cap) ? cfg.max_budget_cap : 10000;

    const targetMode = input?.targetMode === "glob" ? "glob" : "explicit";
    let targets = [];
    let targetGlobs = [];
    if (targetMode === "explicit") {
      const parsed = parseExplicitTargets(input?.targetsText, maxTargets);
      targets = parsed.targets;
      errors.push(...parsed.errors);
    } else {
      const rawGlob = typeof input?.targetGlob === "string" ? input.targetGlob.trim() : "";
      if (!rawGlob) {
        errors.push("Enter a target glob pattern.");
      } else {
        const [pat, rawRef] = rawGlob.split("@");
        if (!validTargetGlob(pat) || (rawGlob.includes("@") && (!rawRef || rawGlob.indexOf("@", rawGlob.indexOf("@") + 1) !== -1 || rawRef.startsWith("-") || rawRef.includes("..") || /^[0-9a-f]{40}$/.test(rawRef)))) {
          errors.push("Target glob must be GLOB or GLOB@REF (not a raw SHA).");
        } else {
          targetGlobs = [rawGlob];
        }
      }
    }

    const concRaw = String(input?.maxConcurrency ?? "").trim();
    const maxConcurrency = Number(concRaw);
    if (!INT_RE.test(concRaw) || !Number.isSafeInteger(maxConcurrency) || maxConcurrency < 1 || maxConcurrency > maxConcLimit) {
      errors.push(`Max concurrency must be an integer between 1 and ${maxConcLimit}.`);
    }

    const budgetRaw = String(input?.budgetCap ?? "").trim();
    const budgetCap = Number(budgetRaw);
    if (!INT_RE.test(budgetRaw) || !Number.isSafeInteger(budgetCap) || budgetCap < 1 || budgetCap > maxBudgetLimit) {
      errors.push(`Budget cap must be an integer between 1 and ${maxBudgetLimit}.`);
    }

    const claimPolicy = ["refuse", "skip", "serialize"].includes(input?.claimPolicy) ? input.claimPolicy : "refuse";
    const scopePath = typeof input?.scopePath === "string" ? input.scopePath.trim() : "";

    const paramResult = coerceParams(runbook?.params, input?.params);
    errors.push(...paramResult.errors);

    const rawRequestId = typeof input?.requestId === "string" ? input.requestId.trim() : "";
    let requestId = rawRequestId;
    if (requestId) {
      if (!REQUEST_ID_RE.test(requestId)) {
        errors.push("Request ID must use 1-128 ASCII letters, digits, '.', '_', or '-'.");
      }
    } else {
      requestId = makeRequestId("ui-fo");
    }

    const payload = {
      request_id: requestId,
      targets,
      target_globs: targetGlobs,
      params: paramResult.params,
      max_concurrency: maxConcurrency,
      budget_cap: budgetCap,
      claim_policy: claimPolicy,
    };
    if (scopePath) payload.scope_path = scopePath;

    return {
      ok: errors.length === 0,
      errors,
      payload,
    };
  }

  function fanoutOutcomeStatus(status) {
    switch (status) {
      case "pending":
        return { status: "queued", label: "Pending" };
      case "active":
        return { status: "running", label: "Active" };
      case "success":
        return { status: "success", label: "Success" };
      case "failure":
        return { status: "failure", label: "Failure" };
      case "unknown":
        return { status: "unknown", label: "Unknown" };
      case "skipped":
        return { status: "cancelled", label: "Skipped" };
      default:
        return { status: "unknown", label: text(status) || "Unknown" };
    }
  }

  function verificationOutcomeStatus(outcome) {
    const raw = text(outcome);
    if (!raw) {
      return { status: "unknown", label: "Recorded (no outcome)" };
    }
    switch (raw) {
      case "Verified":
        return { status: "success", label: "Verified" };
      case "Success":
        return { status: "success", label: "Passed" };
      case "Failure":
      case "Rejected":
        return { status: "failure", label: raw };
      case "Incomplete":
      case "Unknown":
        return { status: "unknown", label: raw };
      case "DomainFailure":
      case "Denied":
      case "ProviderError":
      case "TimedOut":
      case "Cancelled":
      case "Pending":
        return ui()?.outcomeStatus
          ? ui().outcomeStatus(raw, "")
          : {
              status: raw === "Cancelled" ? "cancelled" : raw === "Pending" ? "queued" : "failure",
              label: raw,
            };
      default:
        return { status: "unknown", label: raw };
    }
  }

  function renderStatusPill(status, label) {
    const pill = element("span", undefined, "status-pill");
    pill.dataset.status = status;
    pill.setAttribute("role", "img");
    pill.setAttribute("aria-label", `Status: ${label}`);
    pill.append(element("span", label, "status-pill-label"));
    return pill;
  }

  function classifyError(error) {
    const status = error?.status;
    const code = text(error?.code);
    const message = text(error?.message) || "Request failed.";
    if (status === 400) {
      return {
        kind: "validation",
        retryable: false,
        title: "Invalid dispatch input",
        message,
      };
    }
    if (status === 403) {
      return {
        kind: "forbidden",
        retryable: false,
        title: code === "runbook_grant_required" ? "Missing runbook grant" : "Not authorized",
        message,
      };
    }
    if (status === 404) {
      return {
        kind: "not_found",
        retryable: false,
        title: "Runbook or fan-out not found",
        message,
      };
    }
    if (status === 409) {
      const title = code === "fleet_claim_conflict"
        ? "Fleet claim conflict"
        : code === "project_not_enrolled"
          ? "Project not enrolled"
          : "Request or state conflict";
      return {
        kind: "conflict",
        retryable: false,
        title,
        message: code === "request_id_conflict"
          ? `${message} Reuse the same request ID only with identical target, parameters, and principal.`
          : message,
      };
    }
    if (status === 503 || error?.name === "TimeoutError" || error?.name === "TypeError") {
      return {
        kind: "unknown",
        retryable: true,
        title: "Outcome unknown",
        message: "Do not blindly retry; inspect Runs or reuse the exact request ID and inputs to reconcile.",
      };
    }
    return {
      kind: "error",
      retryable: false,
      title: "Dispatch failed",
      message,
    };
  }

  function targetChoices(runbook) {
    const out = [];
    const targets = Array.isArray(runbook?.allowed_targets) ? runbook.allowed_targets : [];
    for (const target of targets) {
      const repo = text(target?.repository);
      const refs = Array.isArray(target?.references) ? target.references : [];
      for (const reference of refs) {
        if (repo && reference) out.push(`${repo}@${reference}`);
      }
    }
    return out;
  }

  function emptyState(title, copy) {
    const box = element("section", undefined, "panel empty-state");
    box.append(element("p", "Runbooks", "eyebrow"), element("h2", title, "empty-state-title"), element("p", copy, "empty-state-text"));
    return box;
  }

  function appendFact(list, term, value) {
    const row = element("div", undefined, "fact-row");
    row.append(element("dt", term), element("dd", value));
    list.append(row);
  }

  function buildParamField(decl, idPrefix = "runbook-param") {
    const wrap = element("div", undefined, "form-field");
    const id = `${idPrefix}-${decl.name}`;
    const labelText = decl.required ? `${decl.name} (required)` : decl.name;
    const label = element("label", labelText);
    label.htmlFor = id;
    wrap.append(label);

    let input;
    if (decl.kind === "bool") {
      input = document.createElement("select");
      input.id = id;
      input.name = decl.name;
      if (decl.required) {
        const blank = element("option", "Select…");
        blank.value = "";
        input.append(blank);
      }
      for (const val of ["true", "false"]) {
        const opt = element("option", val);
        opt.value = val;
        input.append(opt);
      }
      if (decl.default === true) input.value = "true";
      else if (decl.default === false) input.value = "false";
    } else if (decl.kind === "enum") {
      input = document.createElement("select");
      input.id = id;
      input.name = decl.name;
      if (decl.required) {
        const blank = element("option", "Select…");
        blank.value = "";
        input.append(blank);
      }
      for (const val of decl.enum_values || []) {
        const opt = element("option", val);
        opt.value = val;
        input.append(opt);
      }
      if (typeof decl.default === "string") input.value = decl.default;
    } else if (decl.kind === "integer") {
      input = document.createElement("input");
      input.type = "number";
      input.step = "1";
      input.id = id;
      input.name = decl.name;
      if (Number.isSafeInteger(decl.integer_min)) input.min = String(decl.integer_min);
      if (Number.isSafeInteger(decl.integer_max)) input.max = String(decl.integer_max);
      if (Number.isSafeInteger(decl.default)) input.value = String(decl.default);
    } else {
      input = document.createElement("input");
      input.type = "text";
      input.id = id;
      input.name = decl.name;
      input.autocomplete = "off";
      if (Number.isSafeInteger(decl.relative_path_max_bytes)) {
        input.maxLength = decl.relative_path_max_bytes;
      }
      if (typeof decl.default === "string") input.value = decl.default;
    }
    if (decl.required) input.required = true;
    wrap.append(input);

    const bindings = Array.isArray(decl.bindings)
      ? decl.bindings.map((b) => `${b.node}:${b.shell_env}`).join(", ")
      : "";
    const meta = [decl.kind];
    if (decl.kind === "integer") meta.push(`${decl.integer_min}..=${decl.integer_max}`);
    if (decl.kind === "relative_path") meta.push(`max ${decl.relative_path_max_bytes} B`);
    if (bindings) meta.push(`env ${bindings}`);
    wrap.append(element("p", meta.join(" · "), "small"));
    return { wrap, input, name: decl.name };
  }

  function renderRunbookList(registerPanel, runbooks, fanouts, dispatches, selectedRunbookId, selectedFanoutId, selectedDispatchId, ctx) {
    const heading = element("div", undefined, "section-heading");
    heading.append(element("h2", "Reviewed runbooks"));
    registerPanel.append(heading);
    if (!runbooks.length) {
      registerPanel.append(emptyState("No runbooks configured", "This service was started without --runbooks or defines no runbooks."));
    } else {
      const list = element("div", undefined, "view-stack");
      for (const rb of runbooks) {
        const link = element("a", undefined, "run-choice");
        link.href = `?page=runbooks&runbook=${encodeURIComponent(rb.id)}`;
        if (rb.id === selectedRunbookId && !selectedFanoutId && !selectedDispatchId) link.setAttribute("aria-current", "true");
        link.addEventListener("click", (event) => {
          if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || event.button !== 0) return;
          event.preventDefault();
          ctx.navigate({ page: "runbooks", runbook: rb.id, fanout: "", dispatch: "", rollup_cursor: "" });
        });

        const titleRow = element("div", undefined, "cluster");
        titleRow.append(element("strong", rb.display_name || rb.id), element("code", rb.id, "mono small"));
        const badges = element("div", undefined, "badges");
        const modeBadge = element("span", rb.mode === "mutating" ? "Mutating" : "Read-only", "state-badge");
        if (rb.mode === "mutating") modeBadge.dataset.tone = "warning";
        badges.append(modeBadge);
        if (typeof rb.can_dispatch === "boolean") {
          const grantBadge = element("span", rb.can_dispatch ? "Dispatch allowed" : "Grant required", "state-badge");
          if (!rb.can_dispatch) grantBadge.dataset.tone = "warning";
          badges.append(grantBadge);
        }
        link.append(titleRow, badges);
        if (rb.description) link.append(element("p", rb.description, "small"));
        list.append(link);
      }
      registerPanel.append(list);
    }

    if (Array.isArray(fanouts) && fanouts.length) {
      const foHeading = element("div", undefined, "section-heading");
      foHeading.append(element("h2", "Fan-out runs"));
      registerPanel.append(foHeading);
      const foList = element("div", undefined, "view-stack");
      for (const fo of fanouts) {
        const link = element("a", undefined, "run-choice");
        link.href = `?page=runbooks&runbook=${encodeURIComponent(fo.runbook_id)}&fanout=${encodeURIComponent(fo.id)}`;
        if (fo.id === selectedFanoutId) link.setAttribute("aria-current", "true");
        link.addEventListener("click", (event) => {
          if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || event.button !== 0) return;
          event.preventDefault();
          ctx.navigate({ page: "runbooks", runbook: fo.runbook_id, fanout: fo.id, dispatch: "", rollup_cursor: "" });
        });
        const row = element("div", undefined, "cluster");
        row.append(element("strong", fo.runbook_id), element("code", fo.id, "mono small"));
        const c = fo.counts || {};
        const tally = `${c.success || 0}S/${c.failure || 0}F/${c.unknown || 0}U/${c.skipped || 0}Sk`;
        const meta = element("p", `${fo.phase || fo.admission} · ${tally} · budget ${fo.budget_drawn ?? 0}/${fo.budget_cap ?? 0}`, "small");
        link.append(row, meta);
        foList.append(link);
      }
      registerPanel.append(foList);
    }

    if (Array.isArray(dispatches) && dispatches.length) {
      const dispHeading = element("div", undefined, "section-heading");
      dispHeading.append(element("h2", "Recent dispatches"));
      registerPanel.append(dispHeading);
      const dispList = element("div", undefined, "view-stack");
      for (const disp of dispatches) {
        const link = element("a", undefined, "run-choice");
        link.href = `?page=runbooks&runbook=${encodeURIComponent(disp.runbook_id)}&dispatch=${encodeURIComponent(disp.run_id)}`;
        if (disp.run_id === selectedDispatchId) link.setAttribute("aria-current", "true");
        link.addEventListener("click", (event) => {
          if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || event.button !== 0) return;
          event.preventDefault();
          ctx.navigate({ page: "runbooks", runbook: disp.runbook_id, fanout: "", dispatch: disp.run_id, rollup_cursor: "" });
        });
        const row = element("div", undefined, "cluster");
        row.append(element("strong", disp.runbook_id), element("code", disp.run_id, "mono small"));
        const { status, label } = fanoutOutcomeStatus(disp.status);
        const badges = element("div", undefined, "badges");
        badges.append(renderStatusPill(status, label));
        const meta = element("p", `${text(disp.target)} · patchgate:${text(disp.patchgate_receipt_state)} · verify:${text(disp.verification_receipt_state)}`, "small");
        link.append(row, badges, meta);
        dispList.append(link);
      }
      registerPanel.append(dispList);
    }
  }

  function renderDispatchPanel(detailPanel, runbook, ctx, currentGen) {
    const header = element("div", undefined, "section-heading");
    header.append(element("h2", runbook.display_name || runbook.id));
    detailPanel.append(header);
    if (runbook.description) detailPanel.append(element("p", runbook.description));

    const facts = element("dl", undefined, "facts");
    appendFact(facts, "Runbook ID", runbook.id);
    appendFact(facts, "Mode", runbook.mode === "mutating" ? "mutating" : "read_only");
    appendFact(facts, "Approval gate", runbook.approval_gate === "kiln_publication" ? "kiln_publication" : "none");
    if (runbook.lattice_bundle_id) appendFact(facts, "Lattice bundle", runbook.lattice_bundle_id);
    appendFact(facts, "Bundle identity", runbook.lattice_bundle_identity || "Not recorded");
    appendFact(facts, "Workflow revision", runbook.lattice_workflow_revision || "Not recorded");
    const choices = targetChoices(runbook);
    appendFact(facts, "Allowed targets", choices.join(", ") || "None");
    detailPanel.append(facts);

    const about = element("details", undefined, "about-view");
    about.append(
      element("summary", "Dispatch boundary"),
      element("p", "Typed parameters and allowed repository targets are re-validated by the server. Mutating runbooks require an explicit, non-expired principal grant. Dispatch creates ordinary Kiln CI runs.", "small"),
    );
    detailPanel.append(about);

    const fanoutEnabled = runbook.fanout?.enabled !== false;
    let modeSwitch = null;
    let singleBtn = null;
    let fanoutBtn = null;
    if (fanoutEnabled) {
      modeSwitch = element("div", undefined, "segmented");
      modeSwitch.setAttribute("role", "group");
      modeSwitch.setAttribute("aria-label", "Dispatch mode");
      singleBtn = element("button", "Single target");
      singleBtn.type = "button";
      singleBtn.id = "runbook-mode-single";
      singleBtn.setAttribute("aria-pressed", "true");
      fanoutBtn = element("button", "Fan-out across targets");
      fanoutBtn.type = "button";
      fanoutBtn.id = "runbook-mode-fanout";
      fanoutBtn.setAttribute("aria-pressed", "false");
      modeSwitch.append(singleBtn, fanoutBtn);
      detailPanel.append(modeSwitch);
    }

    const form = element("form", undefined, "view-stack");
    form.id = "runbook-dispatch-form";

    const targetWrap = element("div", undefined, "form-field");
    const targetLabel = element("label", "Target (REPO@REF)");
    targetLabel.htmlFor = "runbook-target";
    const targetInput = document.createElement("input");
    targetInput.id = "runbook-target";
    targetInput.type = "text";
    targetInput.required = true;
    targetInput.autocomplete = "off";
    targetInput.placeholder = choices[0] || "owner/repo@refs/heads/main";
    if (choices.length === 1) targetInput.value = choices[0];

    const datalist = document.createElement("datalist");
    datalist.id = "runbook-target-options";
    for (const choice of choices) {
      const opt = document.createElement("option");
      opt.value = choice;
      datalist.append(opt);
    }
    targetInput.setAttribute("list", datalist.id);
    targetWrap.append(targetLabel, targetInput, datalist);
    form.append(targetWrap);

    const paramControls = [];
    for (const decl of runbook.params || []) {
      const field = buildParamField(decl, "runbook-param");
      paramControls.push(field);
      form.append(field.wrap);
    }

    const reqWrap = element("div", undefined, "form-field");
    const reqLabel = element("label", "Request ID (optional)");
    reqLabel.htmlFor = "runbook-request-id";
    const reqInput = document.createElement("input");
    reqInput.id = "runbook-request-id";
    reqInput.type = "text";
    reqInput.maxLength = 128;
    reqInput.autocomplete = "off";
    reqInput.placeholder = "Leave blank to generate a unique ID";
    reqWrap.append(reqLabel, reqInput);
    form.append(reqWrap);

    const submitRow = element("div", undefined, "cluster");
    const previewBtn = element("button", "Preview effects & budgets");
    previewBtn.id = "runbook-preview-btn";
    previewBtn.type = "button";
    previewBtn.disabled = !ctx.canOperate || busy;
    const submitBtn = element("button", "Dispatch runbook", "primary");
    submitBtn.id = "runbook-dispatch-submit";
    submitBtn.type = "submit";
    submitBtn.dataset.operate = "";
    const permitted = ctx.canOperate && runbook.can_dispatch !== false;
    submitBtn.disabled = !permitted || busy;
    submitRow.append(previewBtn, submitBtn);
    form.append(submitRow);

    if (!ctx.canOperate) {
      form.append(element("p", "Read-only session: operator access is required to dispatch runbooks.", "warning"));
    } else if (runbook.can_dispatch === false) {
      form.append(element("p", `Dispatch refused (${runbook.dispatch_refusal || "runbook_grant_required"}): an explicit principal grant is required for this mutating runbook.`, "warning"));
    }

    const errorBox = element("div", undefined, "warning");
    errorBox.id = "runbook-dispatch-error";
    errorBox.setAttribute("role", "alert");
    errorBox.hidden = true;

    const previewResultBox = element("section", undefined, "panel");
    previewResultBox.id = "runbook-preview-result";
    previewResultBox.hidden = true;

    const resultBox = element("section", undefined, "panel");
    resultBox.id = "runbook-dispatch-result";
    resultBox.hidden = true;

    previewBtn.addEventListener("click", async () => {
      if (busy || !ctx.canOperate) return;
      errorBox.hidden = true;
      errorBox.replaceChildren();
      previewResultBox.hidden = true;
      previewResultBox.replaceChildren();

      const rawParams = {};
      for (const control of paramControls) {
        rawParams[control.name] = control.input.value;
      }
      const checked = validateInputs(runbook, {
        target: (targetInput.value || "").trim(),
        params: rawParams,
        requestId: "preview",
      });
      if (!checked.ok) {
        errorBox.textContent = checked.errors.join(" ");
        errorBox.hidden = false;
        ctx.notice?.(errorBox.textContent, "warning");
        return;
      }

      busy = true;
      previewBtn.disabled = true;
      submitBtn.disabled = true;
      try {
        const preview = await ctx.api(`/api/runbooks/${encodeURIComponent(runbook.id)}/preview`, {
          method: "POST",
          body: {
            target: checked.payload.target,
            params: checked.payload.params,
          },
        });
        if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;
        renderPreviewResult(previewResultBox, preview);
        previewResultBox.hidden = false;
      } catch (err) {
        if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;
        const classified = classifyError(err);
        errorBox.textContent = `${classified.title}: ${classified.message}`;
        errorBox.hidden = false;
        ctx.notice?.(errorBox.textContent, "warning");
      } finally {
        if (generation === currentGen) {
          busy = false;
          previewBtn.disabled = !ctx.canOperate;
          submitBtn.disabled = !permitted;
        }
      }
    });

    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      if (busy || !permitted) return;
      errorBox.hidden = true;
      errorBox.replaceChildren();
      resultBox.hidden = true;
      resultBox.replaceChildren();

      const rawParams = {};
      for (const control of paramControls) {
        rawParams[control.name] = control.input.value;
      }
      const checked = validateInputs(runbook, {
        target: (targetInput.value || "").trim(),
        params: rawParams,
        requestId: reqInput.value || "",
      });
      if (!checked.ok) {
        errorBox.textContent = checked.errors.join(" ");
        errorBox.hidden = false;
        ctx.notice?.(errorBox.textContent, "warning");
        return;
      }

      busy = true;
      submitBtn.disabled = true;
      try {
        const result = await ctx.api(`/api/runbooks/${encodeURIComponent(runbook.id)}/dispatch`, {
          method: "POST",
          body: checked.payload,
        });
        if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;
        const runId = text(result?.run_id) || text(result?.run?.id);
        const statusTitle = result?.replayed ? "Runbook dispatch replayed" : "Runbook dispatched";
        resultBox.append(element("h3", statusTitle));
        const resFacts = element("dl", undefined, "facts");
        appendFact(resFacts, "Run ID", runId || "Not recorded");
        appendFact(resFacts, "Request ID", text(result?.request_id) || checked.payload.request_id);
        appendFact(resFacts, "Target", text(result?.target) || checked.payload.target);
        appendFact(resFacts, "Source revision", text(result?.source_revision) || "Not recorded");
        appendFact(resFacts, "Replayed", result?.replayed ? "true" : "false");
        resultBox.append(resFacts);
        if (runId && typeof ctx.runLink === "function") {
          resultBox.append(ctx.runLink(runId, `Inspect run ${runId}`));
        } else if (runId) {
          const link = element("a", `Inspect run ${runId}`, "row-link");
          link.href = `?page=runs&run=${encodeURIComponent(runId)}&view=pipeline`;
          resultBox.append(link);
        }
        if (runId) {
          const rollupLink = element("a", "Inspect result roll-up", "row-link");
          rollupLink.href = `?page=runbooks&runbook=${encodeURIComponent(runbook.id)}&dispatch=${encodeURIComponent(runId)}`;
          rollupLink.addEventListener("click", (e) => {
            if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || e.button !== 0) return;
            e.preventDefault();
            ctx.navigate({ page: "runbooks", runbook: runbook.id, fanout: "", dispatch: runId, rollup_cursor: "" });
          });
          resultBox.append(rollupLink);
        }
        resultBox.hidden = false;
        ctx.notice?.(`${statusTitle}: ${runId}`, "success");
      } catch (err) {
        if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;
        const classified = classifyError(err);
        errorBox.textContent = `${classified.title}: ${classified.message}`;
        errorBox.hidden = false;
        ctx.notice?.(errorBox.textContent, "warning");
      } finally {
        if (generation === currentGen) {
          busy = false;
          submitBtn.disabled = !permitted;
        }
      }
    });

    detailPanel.append(form, errorBox, previewResultBox, resultBox);

    if (fanoutEnabled) {
      const fanoutForm = renderFanoutForm(runbook, ctx, currentGen, permitted);
      fanoutForm.hidden = true;
      detailPanel.append(fanoutForm);

      singleBtn.addEventListener("click", () => {
        singleBtn.setAttribute("aria-pressed", "true");
        fanoutBtn.setAttribute("aria-pressed", "false");
        form.hidden = false;
        errorBox.hidden = true;
        resultBox.hidden = true;
        fanoutForm.hidden = true;
      });
      fanoutBtn.addEventListener("click", () => {
        singleBtn.setAttribute("aria-pressed", "false");
        fanoutBtn.setAttribute("aria-pressed", "true");
        form.hidden = true;
        errorBox.hidden = true;
        resultBox.hidden = true;
        fanoutForm.hidden = false;
      });
    }
  }

  function renderFanoutForm(runbook, ctx, currentGen, permitted) {
    const wrap = element("div", undefined, "view-stack");
    wrap.id = "runbook-fanout-panel";
    const fanoutForm = element("form", undefined, "view-stack");
    fanoutForm.id = "runbook-fanout-form";

    let targetMode = "explicit";
    const targetModeGroup = element("div", undefined, "segmented");
    const explicitModeBtn = element("button", "Explicit targets");
    explicitModeBtn.type = "button";
    explicitModeBtn.id = "runbook-fanout-mode-explicit";
    explicitModeBtn.setAttribute("aria-pressed", "true");
    const globModeBtn = element("button", "Enrolled-project glob");
    globModeBtn.type = "button";
    globModeBtn.id = "runbook-fanout-mode-glob";
    globModeBtn.setAttribute("aria-pressed", "false");
    targetModeGroup.append(explicitModeBtn, globModeBtn);
    fanoutForm.append(targetModeGroup);

    const explicitWrap = element("div", undefined, "form-field");
    const explicitLabel = element("label", "Targets (one REPO@REF or project per line)");
    explicitLabel.htmlFor = "runbook-fanout-targets";
    const explicitInput = document.createElement("textarea");
    explicitInput.id = "runbook-fanout-targets";
    explicitInput.placeholder = targetChoices(runbook).join("\n") || "onixresearch/kiln@refs/heads/main";
    explicitWrap.append(explicitLabel, explicitInput);

    const globWrap = element("div", undefined, "form-field");
    globWrap.hidden = true;
    const globLabel = element("label", "Enrolled-project glob (GLOB or GLOB@REF)");
    globLabel.htmlFor = "runbook-fanout-glob";
    const globInput = document.createElement("input");
    globInput.id = "runbook-fanout-glob";
    globInput.type = "text";
    globInput.autocomplete = "off";
    globInput.placeholder = "onixresearch/*@refs/heads/main";
    const previewBox = element("div", undefined, "small");
    previewBox.id = "runbook-fanout-preview";
    const updatePreview = () => {
      const res = matchEnrolledTargets(runbook, currentEnrolledProjects(ctx), globInput.value);
      if (res.error) {
        previewBox.textContent = res.error;
      } else {
        const parts = [`Preview matched ${res.matched.length} enrolled allowed target(s)`];
        if (res.matched.length) {
          parts.push(`(${res.matched.map((m) => m.project_id).join(", ")})`);
        }
        if (res.excludedUnenrolled.length) {
          parts.push(`· excluded ${res.excludedUnenrolled.length} unenrolled target(s)`);
        }
        parts.push("— server re-validates enrollment, exact commit SHAs, and fleet claim scopes.");
        previewBox.textContent = parts.join(" ");
      }
    };
    globInput.addEventListener("input", updatePreview);
    refreshPreview = updatePreview;
    globWrap.append(globLabel, globInput, previewBox);

    explicitModeBtn.addEventListener("click", () => {
      targetMode = "explicit";
      explicitModeBtn.setAttribute("aria-pressed", "true");
      globModeBtn.setAttribute("aria-pressed", "false");
      explicitWrap.hidden = false;
      globWrap.hidden = true;
    });
    globModeBtn.addEventListener("click", () => {
      targetMode = "glob";
      explicitModeBtn.setAttribute("aria-pressed", "false");
      globModeBtn.setAttribute("aria-pressed", "true");
      explicitWrap.hidden = true;
      globWrap.hidden = false;
      updatePreview();
    });
    fanoutForm.append(explicitWrap, globWrap);

    const cfg = isRecord(runbook.fanout) ? runbook.fanout : {};
    const concWrap = element("div", undefined, "form-field");
    const concLabel = element("label", "Max concurrency");
    concLabel.htmlFor = "runbook-fanout-concurrency";
    const concInput = document.createElement("input");
    concInput.id = "runbook-fanout-concurrency";
    concInput.type = "number";
    concInput.min = "1";
    concInput.max = String(cfg.max_concurrency_limit || 32);
    concInput.value = String(cfg.default_max_concurrency || 1);
    concWrap.append(concLabel, concInput);

    const budgetWrap = element("div", undefined, "form-field");
    const budgetLabel = element("label", "Parent budget cap (runs)");
    budgetLabel.htmlFor = "runbook-fanout-budget";
    const budgetInput = document.createElement("input");
    budgetInput.id = "runbook-fanout-budget";
    budgetInput.type = "number";
    budgetInput.min = "1";
    budgetInput.max = String(cfg.max_budget_cap || 10000);
    budgetInput.value = String(cfg.default_budget_cap || 16);
    budgetWrap.append(budgetLabel, budgetInput);

    const policyWrap = element("div", undefined, "form-field");
    const policyLabel = element("label", "Fleet claim overlap policy");
    policyLabel.htmlFor = "runbook-fanout-claim-policy";
    const policySelect = document.createElement("select");
    policySelect.id = "runbook-fanout-claim-policy";
    for (const [val, label] of [
      ["refuse", "Refuse on conflict (409)"],
      ["skip", "Skip conflicting targets"],
      ["serialize", "Serialize until claim clears"],
    ]) {
      const opt = element("option", label);
      opt.value = val;
      policySelect.append(opt);
    }
    policyWrap.append(policyLabel, policySelect);

    const scopeWrap = element("div", undefined, "form-field");
    const scopeLabel = element("label", "Write scope path override (optional)");
    scopeLabel.htmlFor = "runbook-fanout-scope-path";
    const scopeInput = document.createElement("input");
    scopeInput.id = "runbook-fanout-scope-path";
    scopeInput.type = "text";
    scopeInput.autocomplete = "off";
    scopeInput.placeholder = "Narrow subpath within project scope (e.g. flake.lock)";
    scopeWrap.append(scopeLabel, scopeInput);

    fanoutForm.append(concWrap, budgetWrap, policyWrap, scopeWrap);

    const fanoutParams = [];
    for (const decl of runbook.params || []) {
      const field = buildParamField(decl, "runbook-fanout-param");
      fanoutParams.push(field);
      fanoutForm.append(field.wrap);
    }

    const reqWrap = element("div", undefined, "form-field");
    const reqLabel = element("label", "Request ID (optional)");
    reqLabel.htmlFor = "runbook-fanout-request-id";
    const reqInput = document.createElement("input");
    reqInput.id = "runbook-fanout-request-id";
    reqInput.type = "text";
    reqInput.maxLength = 128;
    reqInput.autocomplete = "off";
    reqInput.placeholder = "Leave blank to generate a unique ID";
    reqWrap.append(reqLabel, reqInput);
    fanoutForm.append(reqWrap);

    const submitRow = element("div", undefined, "cluster");
    const fanoutPreviewBtn = element("button", "Preview fan-out effects & budgets");
    fanoutPreviewBtn.id = "runbook-fanout-preview-btn";
    fanoutPreviewBtn.type = "button";
    fanoutPreviewBtn.disabled = !ctx.canOperate || busy;
    const submitBtn = element("button", "Dispatch fan-out", "primary");
    submitBtn.id = "runbook-fanout-submit";
    submitBtn.type = "submit";
    submitBtn.dataset.operate = "";
    submitBtn.disabled = !permitted || busy;
    submitRow.append(fanoutPreviewBtn, submitBtn);
    fanoutForm.append(submitRow);

    const errorBox = element("div", undefined, "warning");
    errorBox.id = "runbook-fanout-error";
    errorBox.setAttribute("role", "alert");
    errorBox.hidden = true;

    const fanoutPreviewResultBox = element("section", undefined, "panel");
    fanoutPreviewResultBox.id = "runbook-fanout-preview-result";
    fanoutPreviewResultBox.hidden = true;

    const resultBox = element("section", undefined, "panel");
    resultBox.id = "runbook-fanout-result";
    resultBox.hidden = true;

    fanoutPreviewBtn.addEventListener("click", async () => {
      if (busy || !ctx.canOperate) return;
      errorBox.hidden = true;
      errorBox.replaceChildren();
      fanoutPreviewResultBox.hidden = true;
      fanoutPreviewResultBox.replaceChildren();

      const rawParams = {};
      for (const control of fanoutParams) {
        rawParams[control.name] = control.input.value;
      }
      const checked = validateFanoutInputs(runbook, {
        targetMode,
        targetsText: explicitInput.value,
        targetGlob: globInput.value,
        maxConcurrency: concInput.value,
        budgetCap: budgetInput.value,
        claimPolicy: policySelect.value,
        scopePath: scopeInput.value,
        params: rawParams,
        requestId: "preview",
      });
      if (!checked.ok) {
        errorBox.textContent = checked.errors.join(" ");
        errorBox.hidden = false;
        ctx.notice?.(errorBox.textContent, "warning");
        return;
      }

      const previewBody = {
        targets: checked.payload.targets,
        target_globs: checked.payload.target_globs,
        params: checked.payload.params,
        max_concurrency: checked.payload.max_concurrency,
        budget_cap: checked.payload.budget_cap,
        claim_policy: checked.payload.claim_policy,
      };
      if (checked.payload.scope_path) previewBody.scope_path = checked.payload.scope_path;

      busy = true;
      fanoutPreviewBtn.disabled = true;
      submitBtn.disabled = true;
      try {
        const preview = await ctx.api(`/api/runbooks/${encodeURIComponent(runbook.id)}/preview`, {
          method: "POST",
          body: previewBody,
        });
        if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;
        renderPreviewResult(fanoutPreviewResultBox, preview);
        fanoutPreviewResultBox.hidden = false;
      } catch (err) {
        if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;
        const classified = classifyError(err);
        errorBox.textContent = `${classified.title}: ${classified.message}`;
        errorBox.hidden = false;
        ctx.notice?.(errorBox.textContent, "warning");
      } finally {
        if (generation === currentGen) {
          busy = false;
          fanoutPreviewBtn.disabled = !ctx.canOperate;
          submitBtn.disabled = !permitted;
        }
      }
    });

    fanoutForm.addEventListener("submit", async (event) => {
      event.preventDefault();
      if (busy || !permitted) return;
      errorBox.hidden = true;
      errorBox.replaceChildren();
      resultBox.hidden = true;
      resultBox.replaceChildren();

      const rawParams = {};
      for (const control of fanoutParams) {
        rawParams[control.name] = control.input.value;
      }
      const checked = validateFanoutInputs(runbook, {
        targetMode,
        targetsText: explicitInput.value,
        targetGlob: globInput.value,
        maxConcurrency: concInput.value,
        budgetCap: budgetInput.value,
        claimPolicy: policySelect.value,
        scopePath: scopeInput.value,
        params: rawParams,
        requestId: reqInput.value,
      });
      if (!checked.ok) {
        errorBox.textContent = checked.errors.join(" ");
        errorBox.hidden = false;
        ctx.notice?.(errorBox.textContent, "warning");
        return;
      }

      busy = true;
      submitBtn.disabled = true;
      try {
        const result = await ctx.api(`/api/runbooks/${encodeURIComponent(runbook.id)}/fanout`, {
          method: "POST",
          body: checked.payload,
        });
        if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;
        const fanoutId = text(result?.fanout_id) || text(result?.fanout?.id);
        const title = result?.replayed ? "Fan-out dispatch replayed" : "Fan-out run started";
        resultBox.append(element("h3", title));
        const resFacts = element("dl", undefined, "facts");
        appendFact(resFacts, "Fan-out ID", fanoutId || "Not recorded");
        appendFact(resFacts, "Request ID", text(result?.request_id) || checked.payload.request_id);
        appendFact(resFacts, "Targets", String(result?.fanout?.counts?.total ?? 0));
        appendFact(resFacts, "Replayed", result?.replayed ? "true" : "false");
        resultBox.append(resFacts);
        if (fanoutId) {
          const inspectLink = element("a", `Inspect fan-out ${fanoutId}`, "row-link");
          inspectLink.href = `?page=runbooks&runbook=${encodeURIComponent(runbook.id)}&fanout=${encodeURIComponent(fanoutId)}`;
          inspectLink.addEventListener("click", (e) => {
            if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || e.button !== 0) return;
            e.preventDefault();
            ctx.navigate({ page: "runbooks", runbook: runbook.id, fanout: fanoutId });
          });
          resultBox.append(inspectLink);
        }
        resultBox.hidden = false;
        ctx.notice?.(`${title}: ${fanoutId}`, "success");
      } catch (err) {
        if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;
        const classified = classifyError(err);
        errorBox.textContent = `${classified.title}: ${classified.message}`;
        errorBox.hidden = false;
        if (classified.retryable) reqInput.value = checked.payload.request_id;
        ctx.notice?.(errorBox.textContent, "warning");
      } finally {
        if (generation === currentGen) {
          busy = false;
          submitBtn.disabled = !permitted;
        }
      }
    });

    wrap.append(fanoutForm, errorBox, fanoutPreviewResultBox, resultBox);
    return wrap;
  }

  function renderPreviewResult(container, preview) {
    container.replaceChildren();
    container.append(element("h3", "Dry-run preview (no dispatch performed)"));
    const facts = element("dl", undefined, "facts");
    appendFact(facts, "Mode", text(preview?.mode) || "read_only");
    appendFact(facts, "Approval gate", text(preview?.approval_gate) || "none");
    appendFact(
      facts,
      "Dispatch eligibility",
      preview?.can_dispatch ? "allowed" : `refused (${text(preview?.dispatch_refusal) || "denied"})`,
    );
    appendFact(facts, "Bundle pin verified", preview?.bundle_pin_verified ? "verified" : "unconfigured");
    appendFact(facts, "Bundle identity", text(preview?.bundle_identity) || "Not recorded");
    appendFact(facts, "Workflow revision", text(preview?.workflow_revision) || "Not recorded");
    const budget = isRecord(preview?.budget) ? preview.budget : {};
    appendFact(
      facts,
      "Budget estimate",
      `${budget.estimated_units ?? 0} / ${budget.budget_cap ?? 0} units (max concurrency ${budget.max_concurrency ?? 1})`,
    );
    const nodes = Array.isArray(preview?.nodes) ? preview.nodes : [];
    appendFact(
      facts,
      "Workflow effects",
      nodes.map((n) => `${text(n.id)} (${text(n.kind)}:${text(n.effect_class)})`).join(", ") || "None",
    );
    const targets = Array.isArray(preview?.targets) ? preview.targets : [];
    for (const target of targets) {
      const scopes = Array.isArray(target?.write_scopes) && target.write_scopes.length
        ? target.write_scopes.join(", ")
        : "read-only";
      const conflict = target?.claim_conflict ? ` · conflict: ${target.claim_conflict}` : "";
      appendFact(
        facts,
        `Target ${text(target?.target)}`,
        `rev ${text(target?.source_revision).slice(0, 12)} · scopes: ${scopes}${conflict}`,
      );
    }
    container.append(facts);
    const claims = Array.isArray(preview?.non_claims) ? preview.non_claims : [];
    if (claims.length) {
      container.append(element("p", claims.join(" "), "boundary"));
    }
  }

  function validHttpsUrl(raw) {
    if (
      typeof raw !== "string" ||
      !raw ||
      utf8Length(raw) > 2048 ||
      /[\s\u0000-\u001F\u007F]/.test(raw) ||
      !raw.startsWith("https://")
    ) {
      return null;
    }
    try {
      const url = new URL(raw);
      if (url.protocol === "https:" && url.hostname && !url.username && !url.password) {
        return url.href;
      }
    } catch {
      return null;
    }
    return null;
  }

  function formatNotificationState(notification) {
    const state = text(notification?.state) || "not_configured";
    switch (state) {
      case "delivered":
        return "Delivered";
      case "pending":
        return "Pending";
      case "ambiguous":
        return `Unknown (${text(notification?.error_code) || "ambiguous"})`;
      case "rejected":
        return `Rejected (${text(notification?.error_code) || "rejected"})`;
      default:
        return "Not configured";
    }
  }

  function renderReceiptBadge(receipt, kindLabel, runId) {
    const wrap = element("div", undefined, "view-stack");
    const state = text(receipt?.state) || "missing";
    if (state === "present") {
      if (kindLabel === "patchgate") {
        const counts = isRecord(receipt?.decision_counts) ? receipt.decision_counts : {};
        const entries = Object.entries(counts).map(([k, v]) => `${k}:${v}`);
        const tally = entries.length ? entries.join(" ") : `${receipt?.reviews_count ?? 0} reviews`;
        const pill = renderStatusPill(counts.rejected || counts.blocked || counts.escalated ? "failure" : counts.unknown ? "unknown" : "success", tally);
        wrap.append(pill);
        if (receipt?.authorizes_acceptance === true || receipt?.isolated === false) {
          const warn = element("span", receipt.authorizes_acceptance ? "Claim warning: acceptance flag ignored" : "Non-isolated receipt", "state-badge");
          warn.dataset.tone = "warning";
          wrap.append(warn);
        }
        if (runId) {
          const link = element("a", "Review receipt", "row-link small");
          link.href = `?page=runs&run=${encodeURIComponent(runId)}&view=reviews`;
          wrap.append(link);
        }
      } else {
        const mapped = verificationOutcomeStatus(receipt?.outcome);
        const pill = renderStatusPill(mapped.status, mapped.label);
        if (!text(receipt?.outcome)) {
          pill.title = "Verification artifact recorded without a pass/fail outcome; not treated as passing.";
        }
        wrap.append(pill);
        const reportCount = Array.isArray(receipt?.run_reports) ? receipt.run_reports.length : 0;
        const indepCount = Array.isArray(receipt?.independent_verifications) ? receipt.independent_verifications.length : 0;
        const metaParts = [];
        if (reportCount) metaParts.push(`${reportCount} report${reportCount === 1 ? "" : "s"}`);
        if (indepCount) metaParts.push(`${indepCount} independent`);
        if (receipt?.implementation_verification) metaParts.push("implementer");
        if (metaParts.length) wrap.append(element("span", metaParts.join(" · "), "small"));
        if (runId) {
          const link = element("a", "Evidence", "row-link small");
          link.href = `?page=runs&run=${encodeURIComponent(runId)}&view=evidence`;
          wrap.append(link);
        }
      }
      return wrap;
    }
    if (state === "unavailable") {
      const badge = element("span", `Unavailable (${text(receipt?.unavailable_reason) || "unreadable"})`, "state-badge");
      badge.dataset.tone = "warning";
      wrap.append(badge);
      return wrap;
    }
    if (state === "unrun") {
      const badge = element("span", "Not run", "state-badge");
      wrap.append(badge);
      return wrap;
    }
    const badge = element("span", "Missing — not recorded", "state-badge");
    badge.dataset.tone = "warning";
    badge.title = "No receipt was retained; missing evidence is never treated as passing.";
    wrap.append(badge);
    return wrap;
  }

  function renderPrLinksCell(targetRow) {
    const cell = document.createElement("td");
    const links = Array.isArray(targetRow?.pull_requests) ? targetRow.pull_requests : [];
    if (!links.length) {
      cell.textContent = targetRow?.pull_request_state === "unrun" ? "Not run" : "None recorded";
      return cell;
    }
    const list = element("div", undefined, "view-stack");
    for (const pr of links) {
      const row = element("div", undefined, "cluster");
      const safeHref = validHttpsUrl(pr?.url);
      if (safeHref) {
        const anchor = element("a", text(pr.reference) || safeHref, "row-link");
        anchor.href = safeHref;
        anchor.target = "_blank";
        anchor.rel = "noopener noreferrer";
        anchor.referrerPolicy = "no-referrer";
        row.append(anchor);
      } else {
        row.append(element("code", text(pr?.url) || "Invalid URL", "mono small"));
      }
      const stateBadge = element("span", `${text(pr?.kind) || "pull_request"}: ${text(pr?.state) || "unknown"}`, "state-badge");
      row.append(stateBadge);
      list.append(row);
    }
    cell.append(list);
    return cell;
  }

  function renderRollupSection(container, rollup, ctx, options = {}) {
    const section = element("section", undefined, "view-stack");
    section.id = options.sectionId || "runbook-rollup-section";

    const heading = element("div", undefined, "section-heading");
    heading.append(element("h3", options.title || "Result roll-up"));
    const badges = element("div", undefined, "badges");
    badges.append(element("span", "Read-only observation", "state-badge"));
    if (rollup?.has_unknown) {
      const unk = element("span", "Contains Unknown", "state-badge");
      unk.dataset.tone = "warning";
      badges.append(unk);
    }
    heading.append(badges);
    section.append(heading);

    const claims = Array.isArray(rollup?.non_claims) && rollup.non_claims.length
      ? rollup.non_claims.join(" ")
      : "Result roll-ups project retained run, receipt, and publication records only; they grant no merge, approval, gate, or release authority. Outcome::Unknown and missing or unreadable receipts remain explicit and are never inferred as passing.";
    section.append(element("p", claims, "boundary"));

    const counts = isRecord(rollup?.counts) ? rollup.counts : {};
    const tiles = element("div", undefined, "fact-grid");
    const addTile = (label, value) => {
      const tile = element("div", undefined, "fact");
      tile.append(element("dt", label), element("dd", value));
      tiles.append(tile);
    };
    addTile("Subject", `${text(rollup?.subject_kind)} ${text(rollup?.subject_id)}`);
    addTile("Runbook", `${text(rollup?.runbook_id)} (${text(rollup?.mode)})`);
    addTile(
      "Outcomes",
      `${counts.success ?? 0} passed · ${counts.failure ?? 0} failed · ${counts.unknown ?? 0} unknown · ${counts.skipped ?? 0} skipped · ${(counts.pending ?? 0) + (counts.active ?? 0)} open`,
    );
    addTile(
      "Receipts",
      `Patchgate ${counts.patchgate_present ?? 0} recorded / ${counts.patchgate_missing ?? 0} missing · Verify ${counts.verification_present ?? 0} recorded / ${counts.verification_missing ?? 0} missing`,
    );
    addTile("PR / stack links", `${counts.pull_requests_observed ?? 0} targets observed`);
    addTile("Summary notification", formatNotificationState(rollup?.notification));
    section.append(tiles);

    const about = element("details", undefined, "about-view");
    about.append(
      element("summary", "Roll-up authority and receipt boundary"),
      element(
        "p",
        "Retained Patchgate review receipts, verification receipts, and pull-request links are read-only observations bound to each target's exact revision. Neither this view nor summary webhook notifications grant review acceptance, gate approval, or merge/release authority.",
        "small",
      ),
    );
    section.append(about);

    let filterMode = "all";
    const filterGroup = element("div", undefined, "segmented");
    filterGroup.id = "rollup-status-filters";
    const filterButtons = [];

    const tableWrap = element("div", undefined, "data-table-wrap");
    const table = element("table", undefined, "data-table data-table-stack");
    table.id = "rollup-targets-table";
    const thead = document.createElement("thead");
    const headRow = document.createElement("tr");
    for (const col of ["Target", "Exact Revision", "Outcome", "Patchgate Receipt", "Verification Receipt", "PR / Stack Links", "Run"]) {
      headRow.append(element("th", col));
    }
    thead.append(headRow);
    const tbody = document.createElement("tbody");
    table.append(thead, tbody);
    tableWrap.append(table);

    const targets = Array.isArray(rollup?.targets) ? rollup.targets : [];
    const renderRows = () => {
      tbody.replaceChildren();
      const filtered = targets.filter((t) => {
        if (filterMode === "all") return true;
        if (filterMode === "missing_receipts") {
          return t?.patchgate_receipt?.state === "missing" || t?.verification_receipt?.state === "missing";
        }
        return t?.status === filterMode;
      });
      if (!filtered.length) {
        const emptyRow = document.createElement("tr");
        const td = element("td", "No targets match the selected filter on this page.", "small");
        td.setAttribute("colspan", "7");
        emptyRow.append(td);
        tbody.append(emptyRow);
        return;
      }
      for (const row of filtered) {
        const tr = document.createElement("tr");
        const targetCell = document.createElement("td");
        targetCell.append(element("div", text(row.target), "mono"), element("div", text(row.project_id), "small"));

        const rev = text(row.source_revision);
        const revCell = document.createElement("td");
        revCell.append(element("code", rev ? rev.slice(0, 12) : "Not recorded", "mono"));
        if (rev) revCell.title = rev;

        const outcomeCell = document.createElement("td");
        const outInfo = row.run_outcome && ui()?.outcomeStatus
          ? ui().outcomeStatus(row.run_outcome, row.run_phase || "")
          : fanoutOutcomeStatus(row.status);
        outcomeCell.append(renderStatusPill(outInfo.status, outInfo.label));
        if (row.reason) outcomeCell.append(element("div", text(row.reason), "small"));

        const pgCell = document.createElement("td");
        pgCell.append(renderReceiptBadge(row.patchgate_receipt, "patchgate", row.run_id));

        const verCell = document.createElement("td");
        verCell.append(renderReceiptBadge(row.verification_receipt, "verification", row.run_id));

        const prCell = renderPrLinksCell(row);

        const runCell = document.createElement("td");
        if (row.run_id) {
          if (typeof ctx.runLink === "function") {
            runCell.append(ctx.runLink(row.run_id, row.run_id));
          } else {
            const link = element("a", row.run_id, "row-link");
            link.href = `?page=runs&run=${encodeURIComponent(row.run_id)}&view=pipeline`;
            runCell.append(link);
          }
        } else {
          runCell.textContent = "Not admitted";
        }

        tr.append(targetCell, revCell, outcomeCell, pgCell, verCell, prCell, runCell);
        tbody.append(tr);
      }
    };

    for (const [key, label] of [
      ["all", "All"],
      ["failure", "Failed"],
      ["unknown", "Unknown"],
      ["missing_receipts", "Missing receipts"],
      ["skipped", "Skipped"],
      ["success", "Passed"],
    ]) {
      const btn = element("button", label);
      btn.type = "button";
      btn.setAttribute("aria-pressed", key === "all" ? "true" : "false");
      btn.addEventListener("click", () => {
        filterMode = key;
        for (const other of filterButtons) other.setAttribute("aria-pressed", String(other === btn));
        renderRows();
      });
      filterButtons.push(btn);
      filterGroup.append(btn);
    }
    renderRows();
    section.append(filterGroup, tableWrap);

    const rollupCursor = text(options.rollupCursor);
    const pager = element("div", undefined, "control-bar pager");
    const pageSummary = rollup?.bounded && rollup?.next_cursor
      ? `Showing ${targets.length} of ${rollup.total ?? targets.length} targets · bounded page (more targets available)`
      : `Showing ${targets.length} of ${rollup?.total ?? targets.length} targets · bounded page`;
    pager.append(element("span", pageSummary, "small"));
    const firstBtn = element("button", "First page");
    firstBtn.type = "button";
    firstBtn.id = "rollup-first-page";
    firstBtn.disabled = !rollupCursor;
    firstBtn.addEventListener("click", () => {
      ctx.navigate({ page: "runbooks", rollup_cursor: "" });
    });
    const nextBtn = element("button", "Next page");
    nextBtn.type = "button";
    nextBtn.id = "rollup-next-page";
    nextBtn.disabled = !rollup?.next_cursor;
    nextBtn.addEventListener("click", () => {
      if (rollup?.next_cursor) {
        ctx.navigate({ page: "runbooks", rollup_cursor: rollup.next_cursor });
      }
    });
    pager.append(firstBtn, nextBtn);
    section.append(pager);

    container.append(section);
  }

  async function renderDispatchRollupInspector(detailPanel, runId, rollupCursor, ctx, currentGen) {
    const query = rollupCursor ? `?cursor=${encodeURIComponent(rollupCursor)}` : "";
    let rollup;
    try {
      rollup = await ctx.api(`/api/runbooks/dispatches/${encodeURIComponent(runId)}/rollup${query}`);
    } catch (err) {
      if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;
      if (err?.status === 404) {
        const missing = element("p", `Runbook dispatch not found: ${runId}. No other dispatch was substituted.`, "warning");
        missing.setAttribute("role", "alert");
        detailPanel.replaceChildren(missing);
        return;
      }
      const classified = classifyError(err);
      const warn = element("p", `${classified.title}: ${classified.message}`, "warning");
      warn.setAttribute("role", "alert");
      detailPanel.replaceChildren(warn);
      return;
    }
    if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;
    if (!isRecord(rollup) || !Array.isArray(rollup.targets)) {
      detailPanel.replaceChildren(element("p", "Dispatch roll-up response was invalid.", "warning"));
      return;
    }

    detailPanel.replaceChildren();
    const header = element("div", undefined, "section-heading");
    header.append(element("h2", `Dispatch ${rollup.subject_id || runId}`));
    const badges = element("div", undefined, "badges");
    const outInfo = rollup.overall_outcome && ui()?.outcomeStatus
      ? ui().outcomeStatus(rollup.overall_outcome, rollup.phase || "")
      : { status: "unknown", label: text(rollup.phase) || "Unknown" };
    badges.append(renderStatusPill(outInfo.status, outInfo.label));
    header.append(badges);
    detailPanel.append(header);

    renderRollupSection(detailPanel, rollup, ctx, {
      sectionId: "dispatch-rollup-section",
      title: "Dispatch result roll-up",
      rollupCursor,
    });
  }

  async function renderFanoutInspector(detailPanel, fanoutId, ctx, currentGen, initialError = "", rollupCursor = "") {
    let fanout;
    try {
      fanout = await ctx.api(`/api/runbooks/fanouts/${encodeURIComponent(fanoutId)}`);
    } catch (err) {
      if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;
      if (err?.status === 404) {
        const missing = element("p", `Fan-out run not found: ${fanoutId}. No other fan-out run was substituted.`, "warning");
        missing.setAttribute("role", "alert");
        detailPanel.replaceChildren(missing);
        return;
      }
      const classified = classifyError(err);
      const warn = element("p", `${classified.title}: ${classified.message}`, "warning");
      warn.setAttribute("role", "alert");
      detailPanel.replaceChildren(warn);
      return;
    }
    if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;
    if (!isRecord(fanout) || !Array.isArray(fanout.children)) {
      detailPanel.replaceChildren(element("p", "Fan-out response was invalid.", "warning"));
      return;
    }

    detailPanel.replaceChildren();
    const header = element("div", undefined, "section-heading");
    header.append(element("h2", `Fan-out ${fanout.id}`));
    const badges = element("div", undefined, "badges");
    const phaseLabel = text(fanout.phase) || "active";
    const phaseStatus = phaseLabel === "completed" ? "success" : phaseLabel === "cancelled" ? "cancelled" : phaseLabel === "paused" ? "queued" : "running";
    badges.append(renderStatusPill(phaseStatus, phaseLabel));
    const admBadge = element("span", `Admission: ${text(fanout.admission)}`, "state-badge");
    if (fanout.admission !== "open") admBadge.dataset.tone = "warning";
    badges.append(admBadge);
    header.append(badges);
    detailPanel.append(header);

    const counts = fanout.counts || {};
    const tiles = element("div", undefined, "fact-grid");
    const addTile = (label, value) => {
      const tile = element("div", undefined, "fact");
      tile.append(element("dt", label), element("dd", value));
      tiles.append(tile);
    };
    addTile("Concurrency", `${fanout.active_children ?? 0} / ${fanout.max_concurrency ?? 1}`);
    addTile("Budget", `${fanout.budget_drawn ?? 0} / ${fanout.budget_cap ?? 0}`);
    addTile(
      "Outcomes",
      `${counts.success ?? 0} passed · ${counts.failure ?? 0} failed · ${counts.unknown ?? 0} unknown · ${counts.skipped ?? 0} skipped`,
    );
    addTile("Runbook", text(fanout.runbook_id));
    detailPanel.append(tiles);

    if (fanout.stop_reason) {
      const stopBanner = element("p", `Stop reason: ${fanout.stop_reason}`, "warning");
      stopBanner.id = "fanout-stop-reason";
      detailPanel.append(stopBanner);
    }

    const about = element("details", undefined, "about-view");
    about.append(
      element("summary", "Isolation and admission boundary"),
      element(
        "p",
        "Partial-failure isolation: one child's failure or Unknown outcome never cancels or blocks siblings and never triggers automatic retry. Pause and cancel stop future child admission only; already-running children continue until observed terminal.",
        "small",
      ),
    );
    detailPanel.append(about);

    const controlErr = element("div", undefined, "warning");
    controlErr.id = "fanout-control-error";
    controlErr.setAttribute("role", "alert");
    if (initialError) {
      controlErr.textContent = initialError;
      controlErr.hidden = false;
    } else {
      controlErr.hidden = true;
    }

    const controls = isRecord(fanout.controls) ? fanout.controls : {};
    if (ctx.canOperate && (controls.pause || controls.continue || controls.cancel)) {
      const controlSection = element("section", undefined, "view-stack");
      controlSection.id = "fanout-controls";
      const reqWrap = element("div", undefined, "form-field");
      const reqLabel = element("label", "Control request ID (optional)");
      reqLabel.htmlFor = "fanout-control-request-id";
      const reqInput = document.createElement("input");
      reqInput.id = "fanout-control-request-id";
      reqInput.type = "text";
      reqInput.maxLength = 128;
      reqInput.autocomplete = "off";
      reqInput.placeholder = "Leave blank to generate a unique ID";
      reqWrap.append(reqLabel, reqInput);
      controlSection.append(reqWrap);

      const btnRow = element("div", undefined, "control-bar");
      let pendingRetry = null;
      let lastRetryableControl = null;
      reqInput.addEventListener("input", () => {
        lastRetryableControl = null;
      });

      const retryBtn = element("button", "Retry the same request");
      retryBtn.type = "button";
      retryBtn.id = "fanout-control-retry";
      retryBtn.hidden = true;

      const sendControl = async (action, explicitRequestId) => {
        if (busy) return;
        controlErr.hidden = true;
        controlErr.textContent = "";
        retryBtn.hidden = true;
        pendingRetry = null;

        const rawInput = (reqInput.value || "").trim();
        const crossActionReuse = Boolean(
          !explicitRequestId &&
            lastRetryableControl &&
            rawInput === lastRetryableControl.requestId &&
            (lastRetryableControl.fanoutId !== fanout.id || lastRetryableControl.action !== action),
        );
        if (crossActionReuse) {
          reqInput.value = "";
          lastRetryableControl = null;
        }
        const effectiveInput = crossActionReuse ? "" : rawInput;
        const requestId = explicitRequestId || effectiveInput || makeRequestId("ui-foc");
        if (!REQUEST_ID_RE.test(requestId)) {
          controlErr.textContent = "Control request ID is invalid.";
          controlErr.hidden = false;
          return;
        }
        busy = true;
        try {
          const resp = await ctx.api(`/api/runbooks/fanouts/${encodeURIComponent(fanout.id)}/${action}`, {
            method: "POST",
            body: { request_id: requestId },
          });
          if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;
          lastRetryableControl = null;
          ctx.notice?.(resp?.replayed ? `Fan-out ${action} replayed.` : `Fan-out ${action} applied.`, "success");
          await load(detailPanel.parentElement?.parentElement || detailPanel, ctx);
        } catch (err) {
          if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;
          const classified = classifyError(err);
          const message = `${classified.title}: ${classified.message}`;
          if (classified.retryable) {
            controlErr.textContent = message;
            controlErr.hidden = false;
            pendingRetry = { fanoutId: fanout.id, action, requestId };
            lastRetryableControl = pendingRetry;
            reqInput.value = requestId;
            retryBtn.title = `Retry ${action} with request ID ${requestId}`;
            retryBtn.hidden = false;
          } else if (err?.status === 409) {
            lastRetryableControl = null;
            ctx.notice?.(message, "warning");
            await renderFanoutInspector(detailPanel, fanoutId, ctx, currentGen, message, rollupCursor);
          } else {
            lastRetryableControl = null;
            controlErr.textContent = message;
            controlErr.hidden = false;
          }
        } finally {
          if (generation === currentGen) busy = false;
        }
      };

      retryBtn.addEventListener("click", () => {
        if (pendingRetry && pendingRetry.fanoutId === fanout.id) {
          void sendControl(pendingRetry.action, pendingRetry.requestId);
        }
      });

      if (controls.pause) {
        const pauseBtn = element("button", "Pause admission");
        pauseBtn.type = "button";
        pauseBtn.id = "fanout-pause-btn";
        pauseBtn.addEventListener("click", () => void sendControl("pause"));
        btnRow.append(pauseBtn);
      }
      if (controls.continue) {
        const contBtn = element("button", "Continue admission", "primary");
        contBtn.type = "button";
        contBtn.id = "fanout-continue-btn";
        contBtn.addEventListener("click", () => void sendControl("continue"));
        btnRow.append(contBtn);
      }
      if (controls.cancel) {
        const confirmLabel = element("label", undefined, "cluster");
        const confirmBox = document.createElement("input");
        confirmBox.type = "checkbox";
        confirmBox.id = "fanout-cancel-confirm";
        confirmLabel.append(confirmBox, element("span", "I understand cancelling stops future child admission permanently and does not stop already-running child runs."));
        controlSection.append(confirmLabel);

        const cancelBtn = element("button", "Cancel fan-out", "danger");
        cancelBtn.type = "button";
        cancelBtn.id = "fanout-cancel-btn";
        cancelBtn.disabled = true;
        confirmBox.addEventListener("change", () => {
          cancelBtn.disabled = !confirmBox.checked;
        });
        cancelBtn.addEventListener("click", () => {
          if (confirmBox.checked) void sendControl("cancel");
        });
        btnRow.append(cancelBtn);
      }
      btnRow.append(retryBtn);
      controlSection.append(btnRow, controlErr);
      detailPanel.append(controlSection);
    } else if (initialError) {
      detailPanel.append(controlErr);
    }

    let filterStatus = "all";
    const filterGroup = element("div", undefined, "segmented");
    filterGroup.id = "fanout-ledger-filters";
    const filterButtons = [];
    const tableWrap = element("div", undefined, "data-table-wrap");
    const table = element("table", undefined, "data-table data-table-stack");
    table.id = "fanout-children-table";
    const thead = document.createElement("thead");
    const headRow = document.createElement("tr");
    for (const col of ["Target", "Revision", "Status", "Reason / Scope", "Child Run"]) {
      headRow.append(element("th", col));
    }
    thead.append(headRow);
    const tbody = document.createElement("tbody");
    table.append(thead, tbody);
    tableWrap.append(table);

    const renderRows = () => {
      tbody.replaceChildren();
      const filtered = fanout.children.filter((c) => filterStatus === "all" || c.status === filterStatus);
      for (const child of filtered) {
        const tr = document.createElement("tr");
        const targetCell = element("td", text(child.target), "mono");
        const rev = text(child.source_revision);
        const revCell = element("td", rev ? rev.slice(0, 12) : "", "mono");
        if (rev) revCell.title = rev;
        const { status, label } = fanoutOutcomeStatus(child.status);
        const statusCell = document.createElement("td");
        statusCell.append(renderStatusPill(status, label));
        const reasonParts = [];
        if (child.reason) reasonParts.push(child.reason);
        if (child.write_scope) reasonParts.push(child.write_scope);
        const reasonCell = element("td", reasonParts.join(" · ") || "—", "small");
        const runCell = document.createElement("td");
        if (child.run_id) {
          if (typeof ctx.runLink === "function") {
            runCell.append(ctx.runLink(child.run_id, child.run_id));
          } else {
            const link = element("a", child.run_id, "row-link");
            link.href = `?page=runs&run=${encodeURIComponent(child.run_id)}&view=pipeline`;
            runCell.append(link);
          }
        } else {
          runCell.textContent = "Not admitted";
        }
        tr.append(targetCell, revCell, statusCell, reasonCell, runCell);
        tbody.append(tr);
      }
    };

    for (const [key, label] of [
      ["all", "All"],
      ["failure", "Failed"],
      ["unknown", "Unknown"],
      ["skipped", "Skipped"],
      ["active", "Active"],
      ["success", "Passed"],
    ]) {
      const btn = element("button", label);
      btn.type = "button";
      btn.setAttribute("aria-pressed", key === "all" ? "true" : "false");
      btn.addEventListener("click", () => {
        filterStatus = key;
        for (const other of filterButtons) other.setAttribute("aria-pressed", String(other === btn));
        renderRows();
      });
      filterButtons.push(btn);
      filterGroup.append(btn);
    }
    renderRows();
    detailPanel.append(filterGroup, tableWrap);

    const rollupQuery = rollupCursor ? `?cursor=${encodeURIComponent(rollupCursor)}` : "";
    try {
      const rollup = await ctx.api(`/api/runbooks/fanouts/${encodeURIComponent(fanoutId)}/rollup${rollupQuery}`);
      if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;
      if (isRecord(rollup) && Array.isArray(rollup.targets)) {
        renderRollupSection(detailPanel, rollup, ctx, {
          sectionId: "fanout-rollup-section",
          title: "Result roll-up (receipts, verification & PR links)",
          rollupCursor,
        });
      }
    } catch {
      if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;
      const fallback = element("section", undefined, "view-stack");
      fallback.id = "fanout-rollup-section";
      fallback.append(element("p", "Result roll-up details are currently unavailable; parent fan-out ledger remains shown above.", "small"));
      detailPanel.append(fallback);
    }
  }

  async function load(root, ctx) {
    const currentGen = ++generation;
    busy = false;
    refreshPreview = null;
    root.replaceChildren(element("p", "Loading reviewed runbooks…", "small"));
    let response;
    try {
      response = await ctx.api("/api/runbooks");
    } catch (err) {
      if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;
      const classified = classifyError(err);
      const warn = element("p", `${classified.title}: ${classified.message}`, "warning");
      warn.setAttribute("role", "alert");
      root.replaceChildren(warn);
      return;
    }
    if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;
    if (!isRecord(response) || !Array.isArray(response.runbooks)) {
      root.replaceChildren(element("p", "Runbooks response was invalid.", "warning"));
      return;
    }

    let fanouts = [];
    try {
      const fanoutList = await ctx.api("/api/runbooks/fanouts");
      if (isRecord(fanoutList) && Array.isArray(fanoutList.fanouts)) {
        fanouts = fanoutList.fanouts;
      }
    } catch {
      fanouts = [];
    }
    if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;

    let dispatches = [];
    try {
      const dispatchList = await ctx.api("/api/runbooks/dispatches?limit=20");
      if (isRecord(dispatchList) && Array.isArray(dispatchList.dispatches)) {
        dispatches = dispatchList.dispatches;
      }
    } catch {
      dispatches = [];
    }
    if (generation !== currentGen || (ctx.isCurrent && !ctx.isCurrent())) return;

    const runbooks = response.runbooks;
    const normalized = normalizeRoute(ctx.route);
    const selectedId = normalized.runbook;
    const selectedFanoutId = normalized.fanout || "";
    const selectedDispatchId = normalized.dispatch || "";
    const rollupCursor = normalized.rollup_cursor || "";
    const grid = element("div", undefined, "workspace-grid");
    const left = element("section", undefined, "panel view-stack");
    const right = element("section", undefined, "panel view-stack");
    renderRunbookList(left, runbooks, fanouts, dispatches, selectedId, selectedFanoutId, selectedDispatchId, ctx);

    if (selectedFanoutId) {
      grid.append(left, right);
      root.replaceChildren(grid);
      await renderFanoutInspector(right, selectedFanoutId, ctx, currentGen, "", rollupCursor);
      return;
    }

    if (selectedDispatchId) {
      grid.append(left, right);
      root.replaceChildren(grid);
      await renderDispatchRollupInspector(right, selectedDispatchId, rollupCursor, ctx, currentGen);
      return;
    }

    if (!runbooks.length) {
      right.append(emptyState("No runbooks available", "Configure --runbooks on the service to expose reviewed runbooks."));
    } else if (!selectedId) {
      right.append(emptyState("Select a runbook", "Choose a reviewed runbook from the register to inspect its parameters and dispatch a run."));
    } else {
      const selected = runbooks.find((rb) => rb.id === selectedId);
      if (!selected) {
        const missing = element("p", `Runbook not found: ${selectedId}. No other runbook was substituted.`, "warning");
        missing.setAttribute("role", "alert");
        right.append(missing);
      } else {
        renderDispatchPanel(right, selected, ctx, currentGen);
      }
    }

    grid.append(left, right);
    root.replaceChildren(grid);
  }

  function clear() {
    generation += 1;
    busy = false;
    refreshPreview = null;
  }

  window.KilnRunbooks = Object.freeze({
    load,
    clear,
    refreshEnrolledProjects: () => { refreshPreview?.(); },
    normalizeRoute,
    parseTarget,
    validTargetGlob,
    globMatches,
    parseExplicitTargets,
    matchEnrolledTargets,
    coerceParams,
    validateInputs,
    validateFanoutInputs,
    fanoutOutcomeStatus,
    verificationOutcomeStatus,
    validHttpsUrl,
    classifyError,
    renderPreviewResult,
  });
})();
