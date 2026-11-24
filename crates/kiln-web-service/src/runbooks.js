// Runbook dispatch console view: lists reviewed runbooks, validates typed
// parameters and allowed targets, and submits audited dispatches that create
// ordinary source-bound Kiln runs.
(() => {
  "use strict";

  const RUNBOOK_ID_MAX = 64;
  const REQUEST_ID_RE = /^[A-Za-z0-9._-]{1,128}$/;
  const RELATIVE_PATH_RE = /^[A-Za-z0-9._/-]+$/;
  const INT_RE = /^-?(?:0|[1-9][0-9]*)$/;

  let generation = 0;
  let busy = false;

  const ui = () => window.KilnUi;
  const element = (tag, value, className) => {
    const node = document.createElement(tag);
    if (value !== undefined) node.textContent = String(value);
    if (className) node.className = className;
    return node;
  };
  const isRecord = (value) => value !== null && typeof value === "object" && !Array.isArray(value);
  const text = (value) => typeof value === "string" ? value : "";
  const utf8Length = (value) => {
    if (ui()?.utf8Bytes) return ui().utf8Bytes(value);
    return new TextEncoder().encode(String(value)).length;
  };

  function normalizeRoute(route) {
    const raw = typeof route?.runbook === "string" ? route.runbook : "";
    const trimmed = raw.trim();
    return {
      runbook: trimmed.length <= RUNBOOK_ID_MAX && /^[a-z][a-z0-9-]{0,63}$/.test(trimmed)
        ? trimmed
        : "",
    };
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
    } else if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
      requestId = `ui-rb-${crypto.randomUUID()}`;
    } else {
      requestId = `ui-rb-${Date.now()}`;
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
        title: "Runbook or project not found",
        message,
      };
    }
    if (status === 409) {
      return {
        kind: "conflict",
        retryable: false,
        title: "Request ID conflict",
        message: `${message} Reuse the same request ID only with identical target, parameters, and principal.`,
      };
    }
    if (status === 503 || error?.name === "TimeoutError" || error?.name === "TypeError") {
      return {
        kind: "unknown",
        retryable: false,
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

  function buildParamField(decl) {
    const wrap = element("div", undefined, "form-field");
    const id = `runbook-param-${decl.name}`;
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

  function renderRunbookList(registerPanel, runbooks, selectedId, ctx) {
    const heading = element("div", undefined, "section-heading");
    heading.append(element("h2", "Reviewed runbooks"));
    registerPanel.append(heading);
    if (!runbooks.length) {
      registerPanel.append(emptyState("No runbooks configured", "This service was started without --runbooks or defines no runbooks."));
      return;
    }
    const list = element("div", undefined, "view-stack");
    for (const rb of runbooks) {
      const link = element("a", undefined, "run-choice");
      link.href = `?page=runbooks&runbook=${encodeURIComponent(rb.id)}`;
      if (rb.id === selectedId) link.setAttribute("aria-current", "true");
      link.addEventListener("click", (event) => {
        if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || event.button !== 0) return;
        event.preventDefault();
        ctx.navigate({ page: "runbooks", runbook: rb.id });
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

  function renderDispatchPanel(detailPanel, runbook, ctx, currentGen) {
    const header = element("div", undefined, "section-heading");
    header.append(element("h2", runbook.display_name || runbook.id));
    detailPanel.append(header);
    if (runbook.description) detailPanel.append(element("p", runbook.description));

    const facts = element("dl", undefined, "facts");
    appendFact(facts, "Runbook ID", runbook.id);
    appendFact(facts, "Mode", runbook.mode === "mutating" ? "mutating" : "read_only");
    if (runbook.lattice_bundle_id) appendFact(facts, "Lattice bundle", runbook.lattice_bundle_id);
    appendFact(facts, "Bundle identity", runbook.lattice_bundle_identity || "Not recorded");
    appendFact(facts, "Workflow revision", runbook.lattice_workflow_revision || "Not recorded");
    const choices = targetChoices(runbook);
    appendFact(facts, "Allowed targets", choices.join(", ") || "None");
    detailPanel.append(facts);

    const about = element("details", undefined, "about-view");
    about.append(
      element("summary", "Dispatch boundary"),
      element("p", "Typed parameters and allowed repository targets are re-validated by the server. Mutating runbooks require an explicit, non-expired principal grant. Dispatch creates an ordinary Kiln CI run.", "small"),
    );
    detailPanel.append(about);

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
      const field = buildParamField(decl);
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
    const submitBtn = element("button", "Dispatch runbook", "primary");
    submitBtn.id = "runbook-dispatch-submit";
    submitBtn.type = "submit";
    submitBtn.dataset.operate = "";
    const permitted = ctx.canOperate && runbook.can_dispatch !== false;
    submitBtn.disabled = !permitted || busy;
    submitRow.append(submitBtn);
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

    const resultBox = element("section", undefined, "panel");
    resultBox.id = "runbook-dispatch-result";
    resultBox.hidden = true;

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
        target: targetInput.value.trim(),
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

    detailPanel.append(form, errorBox, resultBox);
  }

  async function load(root, ctx) {
    const currentGen = ++generation;
    busy = false;
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
    const runbooks = response.runbooks;
    const { runbook: selectedId } = normalizeRoute(ctx.route);
    const grid = element("div", undefined, "workspace-grid");
    const left = element("section", undefined, "panel view-stack");
    const right = element("section", undefined, "panel view-stack");
    renderRunbookList(left, runbooks, selectedId, ctx);

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
  }

  window.KilnRunbooks = Object.freeze({
    load,
    clear,
    normalizeRoute,
    parseTarget,
    coerceParams,
    validateInputs,
    classifyError,
  });
})();
