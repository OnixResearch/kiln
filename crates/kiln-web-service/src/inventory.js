// Read-only inventory v2 operator views. All data is text; no provider controls/discovery.
// r[impl kiln.inventory.consumers]
(() => {
  "use strict";
  const SCHEMA = "kiln.observe.inventory.v2";
  const MAX_ITEMS = 4096;
  const TOKEN = /^[A-Za-z0-9_.:@+-]{1,256}$/;
  const collections = ["agents", "workers", "compute", "pools", "models", "admissions", "policies", "attempts"];
  const record = (v) => v !== null && typeof v === "object" && !Array.isArray(v);
  const text = (v) => v === null || v === undefined ? "Unknown / not supplied" : typeof v === "string" ? v : String(v);
  const token = (v) => typeof v === "string" && TOKEN.test(v) && !v.includes("..");
  const reference = (v) => record(v) && token(v.source) && token(v.id);
  // Public tokens cannot contain slash/space; this scoped rendering is injective even with colons in either component.
  const refText = (v) => reference(v) ? `${v.source} / ${v.id}` : "Unknown binding";
  const list = (v) => Array.isArray(v) && v.length ? v.map((x) => reference(x) ? refText(x) : text(x)).join(", ") : "None supplied (not absence evidence)";
  const number = (v) => Number.isSafeInteger(v) && v >= 0 ? String(v) : "Unknown";
  function validate(value) {
    if (!record(value) || value.schema !== SCHEMA || !/^[a-f0-9]{64}$/.test(value.identity) || !/^[a-f0-9]{64}$/.test(value.profile_identity)
      || !Number.isSafeInteger(value.evaluated_at_ms) || value.evaluated_at_ms < 0 || !Array.isArray(value.sources) || value.sources.length > 64
      || !record(value.snapshot) || value.snapshot.schema !== SCHEMA || typeof value.snapshot.synthetic !== "boolean"
      || typeof value.work_requirements_unknown !== "boolean" || !Array.isArray(value.attempt_explanations)
      || !Array.isArray(value.facts) || value.facts.length > MAX_ITEMS
      || collections.some((key) => !Array.isArray(value.snapshot[key]) || value.snapshot[key].length > MAX_ITEMS)
      || value.sources.some((s) => !token(s?.source) || !["current", "missing", "stale", "future", "incomplete", "out_of_order", "unavailable"].includes(s.state))) {
      throw new Error("Inventory v2 projection is invalid; no physical or inference capacity is inferred.");
    }
    for (const key of collections.filter((k) => k !== "attempts")) {
      if (value.snapshot[key].some((row) => !reference(row?.meta?.reference) || !Array.isArray(row.meta.unknown_fields))) throw new Error("Inventory row provenance is invalid.");
    }
    return value;
  }
  function sourceBasis(projection, meta) {
    const source = projection.sources.find((s) => s.source === meta?.reference?.source);
    if (!source) return "Source missing; freshness unknown";
    const unknown = meta.unknown_fields.length ? `; unknown fields: ${list(meta.unknown_fields)}` : "";
    const fact = projection.facts.find((f) => refText(f.reference) === refText(meta.reference));
    return `source ${source.state}; fact ${fact?.state || "unknown"}; producer ${source.producer}/${source.scope}; generation ${number(source.generation)}; observed ${number(meta.observed_at_ms)} ms; valid until ${number(meta.valid_until_ms)} ms${unknown}`;
  }
  function element(tag, content, className) {
    const node = document.createElement(tag);
    if (content !== undefined) node.textContent = String(content);
    if (className) node.className = className;
    return node;
  }
  function facts(values) {
    const node = element("dl", undefined, "facts inventory-facts");
    for (const [label, value] of values) node.append(element("dt", label), element("dd", text(value)));
    return node;
  }
  function details(title, value) {
    const node = element("details", undefined, "inventory-disclosure");
    node.append(element("summary", title), element("pre", JSON.stringify(value, null, 2), "inventory-evidence"));
    return node;
  }
  function structuredTable(headers, rows, emptyMessage) {
    if (!rows.length) return element("p", emptyMessage, "small");
    const wrap = element("div", undefined, "data-table-wrap");
    const tbl = element("table", undefined, "data-table data-table-stack inventory-table");
    const thead = element("thead");
    const headRow = element("tr");
    for (const label of headers) {
      const th = element("th", label);
      th.scope = "col";
      headRow.append(th);
    }
    thead.append(headRow);
    const tbody = element("tbody");
    for (const cells of rows) {
      const tr = element("tr");
      cells.forEach((value, idx) => {
        const td = element("td", text(value));
        td.dataset.label = headers[idx] || "";
        tr.append(td);
      });
      tbody.append(tr);
    }
    tbl.append(thead, tbody);
    wrap.append(tbl);
    return wrap;
  }
  function formatMetricValue(value) {
    if (value === null || value === undefined || value === "") return "not recorded";
    if (Number.isSafeInteger(value)) return String(value);
    if (typeof value === "object") return JSON.stringify(value);
    return String(value);
  }
  function row(projection, item, title, values, extra) {
    const node = element("article", undefined, "inventory-row");
    node.append(element("h4", title), element("p", refText(item.meta.reference), "inventory-reference"),
      element("p", sourceBasis(projection, item.meta), "small inventory-source"), facts(values));
    if (extra) node.append(extra);
    return node;
  }
  function section(title, id, nodes) {
    const node = element("section", undefined, "panel inventory-section"); node.id = id;
    const heading = element("h3", title); heading.tabIndex = -1;
    node.append(heading);
    if (!nodes.length) node.append(element("p", "No rows supplied in this bounded snapshot. Check producer completeness/freshness; this does not establish an empty fleet.", "small"));
    else node.append(...nodes);
    return node;
  }
  function blockers(values) {
    const node = element("ul", undefined, "inventory-blockers");
    if (!values.length) node.append(element("li", "No blockers in these supplied observations. This is not a reservation or dispatch authorization."));
    for (const value of values) node.append(element("li", `${text(value.code)} · ${text(value.constraint)} · ${value.subject ? refText(value.subject) : "snapshot / policy"}${value.owner ? ` · owner ${text(value.owner)}` : ""}`));
    return node;
  }
  // Producer wait reasons are exact codes with an owner; never free text, a reservation or dispatch.
  function waits(values) {
    if (!Array.isArray(values) || !values.length) return "None supplied (not readiness evidence)";
    return values.map((v) => `${text(v?.code)} · ${v?.subject?.kind === "route" ? `route:${text(v.subject.name)}` : `fact:${text(v?.subject?.field)}`} · owner ${text(v?.owner)}`).join("; ");
  }
  // Unmanaged is not unknown: no producer revision lifecycle governs the definition.
  function management(value) {
    if (value === "unmanaged") return "Unmanaged (no producer revision lifecycle; not unknown, not unqualified)";
    if (value === "managed") return "Managed by the producer's revision lifecycle";
    return "Not published by the producer";
  }
  // A qualification reference is evidence, never authority; fixture provenance is never live evidence.
  function qualification(v) {
    const provenance = v?.provenance === "live" ? "live provenance" : v?.provenance === "fixture" ? "fixture provenance (not live evidence)" : `provenance ${text(v?.provenance)}`;
    return `${text(v?.task_family)} · ${text(v?.reference)} · ${text(v?.evidence_class)} · ${provenance}`;
  }
  function qualifications(values) {
    if (!Array.isArray(values) || !values.length) return "None current (not absence evidence)";
    return values.map(qualification).join("; ");
  }
  function pin(value) {
    if (value === null || value === undefined) return "Not published (an unknown pin is listed in unknown fields)";
    if (value.state === "unmanaged") return "Unmanaged (admitted outside a revision lifecycle)";
    return `${text(value.status)} · qualification ${text(value.qualification)} · rollout generation ${number(value.rollout_generation)} · pin ${text(value.identity)}${value.blocked ? " · blocked: further effects or export refused" : ""}`;
  }
  const CAMPAIGN_SCHEMA = "kiln.selected-campaign-projection.v1";
  function validateCampaign(value) {
    if (!record(value) || value.schema !== CAMPAIGN_SCHEMA || !/^[a-f0-9]{64}$/.test(value.identity)
      || !/^[a-f0-9]{64}$/.test(value.campaign_identity) || (value.resource_inventory_identity !== null && !/^[a-f0-9]{64}$/.test(value.resource_inventory_identity))
      || !/^[a-f0-9]{64}$/.test(value.selection_identity) || !/^[a-f0-9]{64}$/.test(value.repository_identity) || !Number.isSafeInteger(value.selection_revision) || value.selection_revision < 0
      || !Number.isSafeInteger(value.store_version) || value.store_version < 0 || typeof value.campaign_id !== "string"
      || value.campaign_id.length > 256 || !Array.isArray(value.members) || value.members.length > MAX_ITEMS
      || !Array.isArray(value.pending_effects) || value.pending_effects.length > MAX_ITEMS
      || !Array.isArray(value.decisions) || value.decisions.length > MAX_ITEMS) throw new Error("Selected campaign projection is invalid; it grants no owner authority.");
    for (const member of value.members) {
      if (!record(member) || typeof member.member !== "string" || member.member.length > 512
        || typeof member.stage !== "string" || typeof member.canonical_work_state !== "string"
        || (member.canonical_work_revision !== null && (!Number.isSafeInteger(member.canonical_work_revision) || member.canonical_work_revision < 0))
        || typeof member.required_context_binding !== "boolean" || !Array.isArray(member.tasks) || member.tasks.length > 4096
        || !Array.isArray(member.dependencies) || member.dependencies.length > 64
        || !Array.isArray(member.waits) || !Array.isArray(member.blockers)
        || typeof member.resource_binding_state !== "string"
        || !Array.isArray(member.wait_categories) || !Array.isArray(member.evidence)
        || member.blockers.length > MAX_ITEMS || member.waits.length > 64 || member.evidence.length > 128
        || !validHold(member.selection_hold)) throw new Error("Selected campaign member is invalid.");
    }
    return value;
  }
  // A member's retained selection explanation. Absent in projections produced before holds were exposed.
  function validHold(hold) {
    return hold === undefined || hold === null || (record(hold) && typeof hold.operation === "string"
      && typeof hold.reason === "string" && hold.reason.length <= 4096 && (hold.owner === null || typeof hold.owner === "string"));
  }
  function selectionHold(hold) {
    if (!hold) return element("p", "Selection hold: none recorded for this member in this snapshot.", "small");
    return element("p", `Selection hold: ${hold.operation} · ${hold.reason} · owner ${text(hold.owner)}`);
  }
  function renderCampaign(value) {
    const projection = validateCampaign(value);
    const page = element("div", undefined, "inventory-page campaign-projection");
    const header = element("header", undefined, "panel inventory-header");
    const stages = new Map();
    for (const member of projection.members) stages.set(member.stage, (stages.get(member.stage) || 0) + 1);
    header.append(element("h2", `Selected campaign · ${projection.campaign_id}`),
      element("p", `Selection ${projection.selection_identity} · revision ${number(projection.selection_revision)} · store version ${number(projection.store_version)} · projection ${projection.identity}`, "inventory-reference"),
      facts([["Selected members", projection.members.length], ["Observed members", projection.members.filter((member) => member.observed === true).length],
        ["Observed stages of selected members", [...stages].map(([stage, count]) => `${stage}: ${count}`).join("; ") || "None supplied"]]),
      element("p", "Read-only admitted snapshot. A proposed next action is not a grant, reservation, dispatch, landing, or lifecycle authorization.", "small"));
    page.append(header);
    const nodes = projection.members.map((member) => {
      const article = element("article", undefined, "inventory-row campaign-member");
      const freshnessRows = Array.isArray(member.input_freshness)
        ? member.input_freshness.map((entry) => Array.isArray(entry)
          ? [text(entry[0]), text(entry[1])]
          : [text(entry?.subject || entry?.kind), text(entry?.state || entry?.freshness)])
        : [];
      const waitCategoryRows = Array.isArray(member.wait_categories)
        ? member.wait_categories.map((entry) => [text(entry?.category), text(entry?.state)])
        : [];
      article.append(element("h4", `${member.change}${member.tasks.length ? ` · ${member.tasks.join(", ")}` : ""}`),
        facts([["Selected member", member.member], ["Goal", member.goal], ["Current observed stage", member.stage], ["Enumeration", member.enumeration], ["Owner snapshot", member.owner_snapshot], ["Work identity / canonical status / revision", `${text(member.work_identity)} / ${text(member.canonical_work_state)} / ${number(member.canonical_work_revision)}`], ["Attempt / fence", `${text(member.attempt_identity)} / ${number(member.attempt_fence)}`], ["Candidate", member.candidate_identity], ["Integration target", member.integration_target_identity], ["Checks satisfied", member.checks_satisfied], ["Reviews satisfied", member.reviews_satisfied], ["Resource binding state", member.resource_binding_state], ["Required checks", list(member.required_checks)], ["Required reviews", list(member.required_reviews)]]),
        element("p", `Wait categories: ${member.waits.length ? member.waits.join(", ") : "none explicitly represented; not readiness evidence"}`),
        element("p", `Remaining obligations: ${member.remaining_obligations.length ? member.remaining_obligations.join(", ") : "none supplied"}`),
        selectionHold(member.selection_hold),
        structuredTable(["Input freshness subject", "State"], freshnessRows, "No input freshness entries recorded."),
        structuredTable(["Wait category", "Observed state"], waitCategoryRows, "No structured wait categories recorded."),
        details("Raw member freshness, evidence and exact resource binding JSON", { freshness: member.input_freshness, dependencies: member.dependencies, evidence: member.evidence, pending_effect_ids: member.pending_effect_ids, resource_inventory_identity: member.resource_inventory_identity, resource_binding_state: member.resource_binding_state, resource_binding: member.resource_binding, required_checks: member.required_checks, required_reviews: member.required_reviews, wait_categories: member.wait_categories }));
      const listNode = element("ul", undefined, "inventory-blockers");
      if (!member.blockers.length) listNode.append(element("li", "No blocker strings supplied; do not infer eligibility."));
      else for (const blocker of member.blockers) listNode.append(element("li", blocker));
      article.append(listNode); return article;
    });
    page.append(section("Every selected member, including blocked and incomplete members", "campaign-members", nodes));
    const budget = record(projection.budget) ? projection.budget : {};
    const budgetEntries = Object.entries(budget).map(([k, v]) => [k.replaceAll("_", " "), formatMetricValue(v)]);
    const nextAction = record(projection.next_action) ? projection.next_action : {};
    const pendingEffectRows = projection.pending_effects.map((eff) => [
      text(eff?.effect_id || eff?.id),
      text(eff?.operation),
      text(eff?.owner),
      text(eff?.state),
      eff?.progress_applied === undefined ? "not recorded" : String(eff.progress_applied),
    ]);
    const decisionRows = projection.decisions.map((dec) => [
      text(dec?.identity),
      text(dec?.category),
      text(dec?.state),
      text(dec?.member),
      text(dec?.question),
      dec?.resolved_answer ?? "not recorded",
    ]);
    const nonClaimsList = element("ul", undefined, "inventory-blockers");
    if (!projection.non_claims.length) nonClaimsList.append(element("li", "No non-claims supplied."));
    else for (const item of projection.non_claims) nonClaimsList.append(element("li", text(item)));
    const obligationBody = element("div", undefined, "inventory-obligations-structured");
    obligationBody.append(
      element("h4", "Selection basis and next-action proposal"),
      facts([
        ["Campaign identity", projection.campaign_identity],
        ["Resource inventory identity", projection.resource_inventory_identity ?? "not recorded"],
        ["Selection basis", projection.selection_basis],
        ["Snapshot source", projection.source],
        ["Proposed operation", nextAction.operation ?? "not recorded"],
        ["Target member", nextAction.member ?? "none"],
        ["Owner", nextAction.owner ?? "not recorded"],
        ["Reason", nextAction.reason ?? "not recorded"],
      ]),
      element("h4", "Aggregate budget"),
      budgetEntries.length ? facts(budgetEntries) : element("p", "No aggregate budget fields recorded.", "small"),
      element("h4", "Pending effects"),
      structuredTable(["Effect ID", "Operation", "Owner", "State", "Progress applied"], pendingEffectRows, "No pending effects recorded."),
      element("h4", "Human decisions"),
      structuredTable(["Identity", "Category", "State", "Member", "Question", "Resolved"], decisionRows, "No human decisions recorded."),
      element("h4", "Non-claims"),
      nonClaimsList,
      details("Raw snapshot-level obligations and non-claims JSON", { campaign_identity: projection.campaign_identity, resource_inventory_identity: projection.resource_inventory_identity, pending_effects: projection.pending_effects, decisions: projection.decisions, budget: projection.budget, next_action: projection.next_action, selection_basis: projection.selection_basis, source: projection.source, non_claims: projection.non_claims }),
    );
    page.append(section("Pending effects, human decisions, aggregate budget, next-action proposal", "campaign-obligations", [obligationBody]));
    return page;
  }
  // r[impl kiln.implementation.campaign.controls.scenario.console]
  // Controls render only what the service lists for this principal; the
  // canonical owner still decides every command.
  const INBOX_SCHEMA = "kiln.campaign-inbox.v1";
  const DEFINITIONS_SCHEMA = "kiln.campaign-definitions.v1";
  const COMMANDS_ROUTE = "/api/implementation/campaigns/commands";
  const HEX = /^[a-f0-9]{64}$/;
  const MAX_DEFINITIONS = 64;
  const ADMISSION = Object.freeze({
    open: "Open: the canonical owner admits successor work and decision issuance.",
    paused: "Paused: no successor admission or decision issuance. Running attempts and workers continue, and retained effects still reconcile.",
    cancelled: "Cancelled: permanently closed. This does not observe worker termination, provider cleanup or refunded usage.",
  });
  const CONTROL_LABELS = Object.freeze({ continue: "Continue admission", pause: "Pause successor admission", cancel: "Cancel campaign" });
  const SCOPE = Object.freeze({
    current: "Current: the member's work, policy and candidate still match this decision.",
    changed: "Changed: the member moved on since this decision was asked; it cannot be answered here.",
    member_unobserved: "Member unobserved: the scope cannot be checked, so no answer is offered.",
  });
  const REFUSALS = Object.freeze({
    start_not_granted: "no current start grant for this principal",
    campaign_exists: "the campaign already exists; a changed selection needs an explicit owner revision",
    endpoint_unsupported: "a member's completion goal is not offered by the delivery route",
  });
  const nullableHex = (v) => v === null || HEX.test(v);
  function validateInbox(value) {
    if (!record(value) || value.schema !== INBOX_SCHEMA || typeof value.campaign_id !== "string" || value.campaign_id.length > 256
      || !Number.isSafeInteger(value.store_version) || value.store_version < 0 || !HEX.test(value.campaign_identity)
      || !Object.hasOwn(ADMISSION, value.admission) || !Array.isArray(value.operations) || value.operations.length > 6
      || value.operations.some((op) => typeof op !== "string") || !Array.isArray(value.decisions) || value.decisions.length > MAX_ITEMS
      || !Array.isArray(value.non_claims)) throw new Error("Campaign inbox is invalid; no control is offered.");
    for (const decision of value.decisions) {
      if (!record(decision) || !HEX.test(decision.identity) || typeof decision.member !== "string" || typeof decision.question !== "string"
        || !Array.isArray(decision.options) || decision.options.some((option) => typeof option !== "string")
        || !Array.isArray(decision.actions) || decision.actions.some((action) => action !== "resolve" && action !== "issue")
        || !HEX.test(decision.policy_identity) || !nullableHex(decision.work_identity) || !nullableHex(decision.candidate_identity)
        || !Object.hasOwn(SCOPE, decision.scope) || !Array.isArray(decision.review_roles)) throw new Error("Campaign decision is invalid; no answer is offered.");
    }
    return value;
  }
  function validateDefinitions(value) {
    if (!record(value) || value.schema !== DEFINITIONS_SCHEMA || !Array.isArray(value.definitions) || value.definitions.length > MAX_DEFINITIONS
      || value.definitions.some((d) => !record(d) || !HEX.test(d.definition_identity) || typeof d.campaign_id !== "string"
        || !Array.isArray(d.members) || !Array.isArray(d.offered_goals) || typeof d.can_start !== "boolean" || !Array.isArray(d.refusals))) {
      throw new Error("Campaign definitions are invalid; no start is offered.");
    }
    return value;
  }
  // Retained buttons follow their own enablement rule after a command settles.
  function controlButton(label, allowed = () => true) {
    const button = element("button", label); button.type = "button"; button.allowed = allowed;
    return button;
  }
  // r[impl kiln.cairn.workspace.scenario.decisions] The answer and issue actions the inbox lists for one decision, shared
  // by the campaign view and the Cairn changes page. Nothing is offered for an action the inbox does not list.
  function decisionAnswer(inbox, decision, send, buttons) {
    const nodes = [];
    const binding = { campaign_id: inbox.campaign_id, expected_version: inbox.store_version, decision_identity: decision.identity };
    if (decision.actions.includes("resolve")) {
      const form = element("form", undefined, "campaign-decision-answer");
      const fieldset = element("fieldset"); fieldset.append(element("legend", "Answer this decision"));
      const radios = decision.options.map((option) => {
        const input = element("input"); Object.assign(input, { type: "radio", name: `answer-${decision.identity}`, value: option });
        const label = element("label"); label.append(input, element("span", option));
        fieldset.append(label);
        return input;
      });
      const submit = controlButton("Submit answer"); submit.type = "submit"; buttons.push(submit);
      const note = element("p", undefined, "small"); note.setAttribute("role", "status");
      form.append(fieldset, submit, note);
      form.addEventListener("submit", (event) => {
        event.preventDefault();
        const chosen = radios.find((input) => input.checked);
        if (!chosen) { note.textContent = "Choose one of the declared options."; return; }
        send({ command: "resolve_decision", ...binding, work_identity: decision.work_identity, policy_identity: decision.policy_identity, candidate_identity: decision.candidate_identity, answer: chosen.value });
      });
      nodes.push(form);
    }
    if (decision.actions.includes("issue")) {
      const issue = controlButton("Issue to reviewers"); buttons.push(issue);
      issue.addEventListener("click", () => send({ command: "issue_decision", ...binding }));
      nodes.push(issue);
    }
    return nodes;
  }
  function decisionArticle(inbox, decision, send, buttons) {
    const article = element("article", undefined, "inventory-row campaign-decision");
    article.append(element("h4", `${text(decision.category).replaceAll("_", " ")} · ${text(decision.state)}`), element("p", decision.question),
      facts([["Member", decision.member], ["Options", decision.options.join(", ")], ["Recommendation", decision.recommendation ?? "None supplied"],
        ["Scope", SCOPE[decision.scope]], ["Bound work / policy / candidate", `${decision.work_identity ?? "no work binding"} / ${decision.policy_identity} / ${decision.candidate_identity ?? "no candidate binding"}`],
        ["Priority / created", `${number(decision.priority)} / ${number(decision.created_ms)} ms`], ["Queue note", decision.pending_reason ?? "None"],
        ["Resolved answer", decision.resolved_answer ?? "Not resolved"], ["Required review roles", decision.review_roles.length ? decision.review_roles.join(", ") : "None declared"]]),
      ...decisionAnswer(inbox, decision, send, buttons));
    return article;
  }
  function renderControls(value, send) {
    const inbox = validateInbox(value);
    const buttons = [];
    const panel = element("section", undefined, "panel inventory-section campaign-controls");
    const heading = element("h3", "Campaign controls and decision inbox"); heading.tabIndex = -1;
    panel.append(heading, facts([["Admission", ADMISSION[inbox.admission]], ["Store version each command expects", number(inbox.store_version)],
      ["Principal", inbox.principal], ["Grant expiry", Number.isSafeInteger(inbox.grant_expires_at_ms) ? `${number(inbox.grant_expires_at_ms)} ms` : "No expiry recorded or no grant"]]));
    const operations = inbox.operations.filter((operation) => Object.hasOwn(CONTROL_LABELS, operation));
    if (operations.length) {
      const controls = element("div", undefined, "campaign-control-actions");
      for (const operation of operations) {
        let allowed = () => true;
        if (operation === "cancel") {
          const confirm = element("input"); confirm.type = "checkbox";
          const label = element("label"); label.append(confirm, element("span", "I understand cancellation is permanent and does not stop running workers."));
          allowed = () => confirm.checked;
          controls.append(label);
          confirm.addEventListener("change", () => { button.disabled = Boolean(button._inFlight) || !allowed(); });
        }
        const button = controlButton(CONTROL_LABELS[operation], allowed); button.dataset.campaignCommand = operation;
        button.disabled = !allowed(); buttons.push(button);
        button.addEventListener("click", () => send({ command: operation, campaign_id: inbox.campaign_id, expected_version: inbox.store_version }));
        controls.append(button);
      }
      panel.append(controls);
    } else panel.append(element("p", "No admission control is open to this principal in the current state.", "small"));
    const decisions = element("div", undefined, "campaign-decisions");
    for (const decision of inbox.decisions) decisions.append(decisionArticle(inbox, decision, send, buttons));
    if (!inbox.decisions.length) decisions.append(element("p", "No decision requests are retained for this campaign.", "small"));
    panel.append(element("h4", "Decision inbox"), decisions, details("What these controls do not do", inbox.non_claims));
    panel.controlButtons = buttons;
    return panel;
  }
  function renderDefinitions(value, send, onSelect) {
    const listing = validateDefinitions(value);
    const buttons = [];
    const panel = element("section", undefined, "panel inventory-section campaign-definitions");
    const heading = element("h3", "Configured campaign definitions"); heading.tabIndex = -1;
    panel.append(heading, element("p", "Each preview is a frozen selection. Starting it creates exactly that selection through the canonical owner; a preview grants no claim, dispatch or completion.", "small"));
    if (!listing.definitions.length) panel.append(element("p", "No campaign definitions are configured for this service.", "small"));
    for (const definition of listing.definitions) {
      const article = element("article", undefined, "inventory-row campaign-definition");
      const existing = definition.existing ? `Started at store version ${number(definition.existing.store_version)}; ${definition.existing.same_selection ? "same selection" : "a different selection"}; selected identity ${text(definition.existing.selection_identity)}` : "Not started";
      article.append(element("h4", `${definition.campaign_id} · revision ${number(definition.revision)}`),
        facts([["Definition identity", definition.definition_identity], ["Repository", definition.repository], ["Delivery route", definition.delivery_route],
          ["Completion goals this route offers", definition.offered_goals.join(", ")], ["Concurrency / outstanding decisions", `${number(definition.max_concurrency)} / ${number(definition.max_outstanding_decisions)}`],
          ["One writer per repository", definition.require_one_writer_per_repository], ["Parent budget", definition.parent_budget_id], ["Agent definition", definition.agent_definition], ["Campaign", existing]]));
      const members = element("ul", undefined, "inventory-blockers");
      for (const member of definition.members) {
        const required = (values) => values.length ? values.join(", ") : "none required";
        members.append(element("li", `${text(member.member)} · goal ${text(member.goal)}${member.goal_supported ? "" : " (not offered by this route)"} · profile ${text(member.profile)} · prerequisites ${member.prerequisite_inclusion ? "included" : "excluded"} · checks ${required(member.required_checks)} · reviews ${required(member.required_reviews)}`));
      }
      article.append(members, details("Exact member bindings: workflow profile, policy, repository instructions, integration policy and context", definition.members));
      if (definition.refusals.length) article.append(element("p", `Cannot start: ${definition.refusals.map((code) => REFUSALS[code] || text(code)).join("; ")}.`, "small"));
      if (definition.can_start) {
        const start = controlButton("Start this exact selection"); buttons.push(start);
        start.addEventListener("click", () => send({ command: "start", definition_identity: definition.definition_identity }));
        article.append(start);
      }
      if (definition.existing && onSelect) {
        const open = controlButton("Open existing campaign");
        open.addEventListener("click", () => onSelect(definition.campaign_id));
        article.append(open);
      }
      panel.append(article);
    }
    panel.controlButtons = buttons;
    return panel;
  }
  function newRequestId() {
    const bytes = new Uint8Array(16);
    globalThis.crypto.getRandomValues(bytes);
    return `console-${[...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("")}`;
  }
  // One submission at a time. A lost response keeps its request ID so a retry
  // reconciles the same owner operation instead of applying a new one.
  function commandRunner(ctx, ui, newId = newRequestId) {
    let pending = null;
    const lock = (busy) => {
      for (const button of ui.buttons()) {
        button._inFlight = busy;
        button.disabled = busy || !button.allowed();
      }
    };
    async function send(intent, requestId) {
      if (pending?.inFlight) return;
      pending = { intent, requestId, inFlight: true };
      ui.retry.hidden = true; lock(true);
      ui.status.textContent = `Sending ${text(intent.command).replaceAll("_", " ")} (request ${requestId})…`;
      try {
        const result = await ctx.api(COMMANDS_ROUTE, { method: "POST", body: { ...intent, request_id: requestId } });
        pending = null;
        ui.status.textContent = `${result.replayed ? "Already applied" : "Applied"}: ${text(result.command).replaceAll("_", " ")} at store version ${number(result.store_version)}; admission ${text(result.admission)}. Request ${text(result.request_id)}.`;
        ui.applied(result, ui.status.textContent);
      } catch (error) {
        if (error?.name === "AbortError") { pending = null; return; }
        if (error?.status === 409) {
          pending = null;
          ui.status.textContent = `${text(error.message)} Nothing was applied; the current state was reloaded.`;
          ui.reload(ui.status.textContent);
        } else if ([400, 403, 404].includes(error?.status)) {
          pending = null;
          ui.status.textContent = `${text(error.message)} Nothing was applied.`;
        } else {
          pending = { intent, requestId, inFlight: false };
          ui.status.textContent = `No confirmed response for request ${requestId}; the command may have applied. Retry sends the same request ID, so the owner returns the recorded outcome instead of applying it twice.`;
          ui.retry.hidden = false;
        }
      } finally { if (!pending?.inFlight) lock(false); }
    }
    ui.retry.addEventListener("click", () => { if (pending && !pending.inFlight) void send(pending.intent, pending.requestId); });
    return { start: (intent) => send(intent, newId()) };
  }
  // `notice` carries the last outcome across the reload that replaces this UI.
  function runnerUi(panel, reload, applied, notice = "") {
    const status = element("p", notice || undefined, "small campaign-command-status"); status.setAttribute("role", "status");
    const retry = element("button", "Retry the same request", "campaign-command-retry"); retry.type = "button"; retry.hidden = true;
    return { status, retry, reload, applied, buttons: () => panel.current?.controlButtons || [] };
  }
  let definitionsGeneration = 0;
  let definitionsMounted = null;
  function controlsUnavailable(error, subject) {
    const configured = !(error?.status === 503 && text(error?.message).startsWith("campaign_controls_not_configured"));
    return element("p", configured ? `${subject} unavailable: ${text(error?.message)} No control is offered.` : "Campaign controls are not configured for this service; the campaign projection is read-only.", "small");
  }
  // r[impl kiln.cairn.workspace.scenario.decisions] One campaign's inbox, admitted before any control is offered.
  async function readInbox(ctx, campaignId) {
    const inbox = validateInbox(await ctx.api(`/api/implementation/campaigns/inbox?campaign=${encodeURIComponent(campaignId)}`));
    if (inbox.campaign_id !== campaignId) throw new Error("The campaign inbox names a different campaign; no control is offered.");
    return inbox;
  }
  async function loadControls(holder, ctx, campaignId, live, reload, newId, notice) {
    holder.replaceChildren(element("p", "Reading campaign controls…", "small"));
    try {
      const inbox = await readInbox(ctx, campaignId);
      if (!live()) return;
      const panel = {};
      const ui = runnerUi(panel, reload, (result, message) => reload(message), notice);
      const runner = commandRunner(ctx, ui, newId);
      panel.current = renderControls(inbox, runner.start);
      holder.replaceChildren(panel.current, ui.status, ui.retry);
    } catch (error) {
      if (!live() || error?.name === "AbortError") return;
      holder.replaceChildren(controlsUnavailable(error, "Campaign controls"));
    }
  }
  async function loadDefinitions(root, ctx, onStarted, newId = newRequestId, notice = "") {
    const current = ++definitionsGeneration; definitionsMounted = root;
    const live = () => definitionsGeneration === current && root.isConnected && ctx.isCurrent();
    root.setAttribute("aria-busy", "true");
    try {
      const listing = await ctx.api("/api/implementation/campaign-definitions");
      if (!live()) return;
      const panel = {};
      const ui = runnerUi(panel, (message) => { if (live()) void loadDefinitions(root, ctx, onStarted, newId, message); }, (result) => onStarted(result.campaign_id), notice);
      const runner = commandRunner(ctx, ui, newId);
      panel.current = renderDefinitions(listing, runner.start);
      root.replaceChildren(panel.current, ui.status, ui.retry);
    } catch (error) {
      if (!live() || error?.name === "AbortError") return;
      root.replaceChildren(controlsUnavailable(error, "Campaign definitions"));
    } finally { if (live()) root.removeAttribute("aria-busy"); }
  }
  function render(projection) {
    validate(projection);
    const snapshot = projection.snapshot;
    const page = element("div", undefined, "inventory-page");
    const heading = element("header", undefined, "panel inventory-header");
    heading.append(element("h2", "Observed resource inventories"), element("p", snapshot.synthetic ? "Synthetic, disconnected example — not live fleet or capacity evidence." : "Explicitly supplied observations. Configured project slots below are a different v1 inventory.", "small"),
      element("p", "Configured, healthy, idle, reserved, executing, model cached, loading, ready and request admitted are separate facts. Explanations never reserve or dispatch resources.", "small"));
    const navigation = element("nav", undefined, "inventory-navigation"); navigation.setAttribute("aria-label", "Resource inventory sections");
    for (const [id, label] of [["agents", "Agents"], ["workers", "Workers"], ["compute", "Compute"], ["models", "Models / Serving"], ["work", "Work / attempts"]]) {
      const link = element("a", label); link.href = `#inventory-${id}`;
      link.addEventListener("click", (event) => { event.preventDefault(); const target = page.querySelector(`#inventory-${id} h3`); target?.focus(); target?.scrollIntoView({ block: "start" }); });
      navigation.append(link);
    }
    heading.append(navigation, details("Snapshot identity, sources, freshness and non-claims", { identity: projection.identity, profile_identity: projection.profile_identity, evaluated_at_ms: projection.evaluated_at_ms, sources: projection.sources, non_claims: projection.non_claims }));
    page.append(heading);
    page.append(section("Agents — immutable definitions, not running instances", "inventory-agents", snapshot.agents.map((agent) => row(projection, agent, agent.profile_revision, [
      ["Task families (declared)", list(agent.task_families)], ["Output contracts", list(agent.output_contracts)], ["Harnesses", list(agent.harnesses)], ["Tools", list(agent.tools)], ["Permitted models", list(agent.permitted_models)], ["Authority", agent.authority],
      ["Revision lifecycle", management(agent.revisions.management)], ["Desired revision", agent.revisions.desired], ["Prepared revision", agent.revisions.prepared], ["Qualified revision", agent.revisions.qualified], ["Selected for new sessions", agent.revisions.selected_for_new_sessions], ["Preparation failure", agent.revisions.preparation_failure], ["Revision generation", number(agent.revisions.generation)],
      ["Task-family qualification references (family · reference · class · provenance)", qualifications(agent.task_family_qualifications)],
      ["Observed instances of this definition", String(snapshot.workers.filter((w) => refText(w.definition) === refText(agent.meta.reference)).length) + " supplied; not a fleet total"], ["Producer wait reasons (code · subject · owner)", waits(agent.wait_reasons)],
    ], details("Qualification: exact subject / corpus / protocol, accepted denominators, cost, unknown use and limitations (not availability)", agent.qualifications)))));
    page.append(section("Workers — actual instances and pinned bindings", "inventory-workers", snapshot.workers.map((worker) => row(projection, worker, `${worker.meta.reference.id} · ${worker.lifecycle}`, [
      ["Incarnation", worker.incarnation], ["Stable session", worker.session], ["Definition", refText(worker.definition)], ["Actually pinned revision", worker.pinned_revision], ["Revision pin", pin(worker.pin)], ["Observed lifecycle", worker.lifecycle], ["Attempt", worker.attempt], ["Fence", number(worker.fence)], ["Execution binding", refText(worker.execution)], ["Execution grant", refText(worker.execution_grant)], ["Workspace alias", worker.workspace], ["Independent model-service bindings", list(worker.model_services)], ["Producer wait reasons (code · subject · owner)", waits(worker.wait_reasons)],
    ]))));
    const computeRows = snapshot.compute.map((device) => row(projection, device, `${device.kind} · ${device.host}`, [
      ["Host", device.host], ["Device", device.device], ["Provider pool", device.pool], ["Health / draining", device.health], ["Sandbox capabilities", list(device.sandbox)], ["Backend capabilities", list(device.backends)], ["Shared accounting pool identities", list(device.accounting_pools)], ["Shared serving allocations (charged once to the deployment)", list(device.shared_serving)], ["Compatibility / qualification", list(device.qualification)],
    ]));
    computeRows.push(...snapshot.pools.map((pool) => row(projection, pool, `${pool.meta.reference.id} · shared ${pool.unit}`, [
      ["Physical capacity", number(pool.capacity)], ["Authoritative allocatable", number(pool.allocatable)], ["Reserved", number(pool.reserved)], ["Unit", pool.unit], ["Accounting complete", String(pool.accounting_complete)], ["Accounting rule", "Count this exact pool once; device/service references are not additional capacity."],
    ], details("Allocations by exact consumer; unknown remainder is not zero", pool.allocations))));
    page.append(section("Compute — devices and authoritative shared resource pools", "inventory-compute", computeRows));
    page.append(section("Models / Serving — artifacts, deployments and request capacity", "inventory-models", snapshot.models.map((model) => row(projection, model, `${model.endpoint_alias} · ${model.state}`, [
      ["Artifact reference", model.artifact], ["Artifact available / cached", model.artifact_available], ["Artifact verification evidence", model.artifact_verification_evidence], ["Claimed model (not exact weight verification)", model.claimed_model], ["Configuration", model.configuration], ["Deployment", model.deployment], ["Endpoint alias (not private URL)", model.endpoint_alias], ["Replicas", list(model.replicas)], ["Deployment readiness", model.state], ["Backend", model.backend], ["Context tokens", number(model.context_tokens)], ["Tools", list(model.tools)], ["Modalities", list(model.modalities)], ["Inference admission capacity", number(model.available_requests)], ["Measurement provenance", model.measurement], ["Owning queue reason", model.queue_reason], ["Compute-device bindings", list(model.compute_devices)], ["Shared pool references (not additive capacity)", list(model.accounting_pools)],
      ["Routing permission (not readiness)", model.routing_permission], ["Routing policy identity", model.routing_policy_ref],
    ]))));
    const work = section("Exact work / attempt drilldown", "inventory-work", []);
    // Replace the generic no-row message; an unknown work binding is not an empty attempt history.
    work.lastChild.remove();
    if (projection.work_requirements_unknown) work.append(element("p", "Exact current admitted work requirements are unknown. Select a work version with an admitted requirements attachment; no readiness inferred.", "small"));
    const explanation = projection.explanation;
    if (explanation) {
      work.append(facts([["Exact work version", explanation.work_version], ["Definition", refText(explanation.definition)], ["Policy", explanation.policy_ref], ["Needed task-family qualification", explanation.task_family_qualification ? qualification(explanation.task_family_qualification) : "None current for this exact revision"], ["Candidate combinations", number(explanation.combinations_considered)], ["Truncated", String(explanation.truncated)], ["Dispatch authorization", "Never granted by this view"]]), blockers(explanation.blockers));
      for (const candidate of explanation.candidates) {
        const item = element("details", undefined, "inventory-disclosure");
        item.append(element("summary", `${refText(candidate.worker)} → ${refText(candidate.execution)} + ${refText(candidate.model)} · ${candidate.observed_eligible ? "observed eligible, revalidate" : "blocked / unknown"}`), blockers(candidate.blockers));
        work.append(item);
      }
    }
    for (const attempt of projection.attempt_explanations) {
      const worker = snapshot.workers.find((w) => refText(w.meta.reference) === refText(attempt.binding.worker));
      const agent = worker && snapshot.agents.find((a) => refText(a.meta.reference) === refText(worker.definition));
      const node = element("details", undefined, "inventory-disclosure");
      node.append(element("summary", `Attempt ${attempt.binding.attempt} · fence ${attempt.binding.fence}`), facts([["Work", attempt.binding.work_version], ["Worker", refText(attempt.binding.worker)], ["Definition", agent ? refText(agent.meta.reference) : "Unknown"], ["Pinned revision", worker?.pinned_revision], ["Execution grant", refText(attempt.binding.execution_grant)], ["Model services", list(attempt.binding.model_services)], ["Request admissions", list(attempt.binding.request_admissions)]]), blockers(attempt.blockers));
      work.append(node);
    }
    const amounts = (value) => {
      if (!record(value)) return "None recorded";
      const entries = Object.entries(value);
      return entries.length ? entries.map(([dimension, amount]) => `${text(dimension)}: ${number(amount)}`).join(", ") : "None recorded";
    };
    const lineageText = (value) => {
      if (!record(value)) return "None supplied";
      const blocker = value.blocker ? ` · blocker ${text(value.blocker)}` : "";
      return `root ${text(value.root)} · account ${text(value.account)} · depth ${number(value.depth)}${blocker}`;
    };
    const servingText = (value) => {
      if (!record(value)) return "None supplied";
      const obs = value.observation;
      const window = obs ? ` · ${text(obs.readiness)} · remaining ${number(obs.remaining_slots)} · queued ${number(obs.queued)}` : "";
      return `${text(value.provider)} · gate ${text(value.gate)}${window}`;
    };
    for (const policy of snapshot.policies) {
      const adm = policy.admission_explanation;
      const rows = [
        ["Work version", policy.work_version],
        ["Policy", policy.policy_ref],
        ["Eligible", String(Boolean(policy.eligible))],
        ["Owner reason", policy.owner_reason],
      ];
      if (record(adm)) {
        rows.push(
          ["Fair-admission phase", adm.phase],
          ["Fair-admission reason", adm.reason],
          ["Queue position", adm.position === null || adm.position === undefined ? "Not queued / active" : number(adm.position)],
          ["Held reservations", amounts(adm.held)],
          ["Measured usage", amounts(adm.measured)],
          ["Settled (host-measured)", amounts(adm.settled_measured)],
          ["Settled (unknown provenance)", amounts(adm.settled_unknown)],
          ["Lineage", lineageText(adm.lineage)],
          ["Serving gate", servingText(adm.serving)],
        );
      }
      work.append(row(projection, policy, `Policy · ${text(policy.work_version)}`, rows));
    }
    work.append(details("Owning producer admission/rejection reasons and policy/budget/waiting facts", { admissions: snapshot.admissions, policies: snapshot.policies }));
    page.append(work);
    return page;
  }
  let generation = 0;
  let mounted = null;
  let selectedWork = "";
  let campaignGeneration = 0;
  let campaignMounted = null;
  let campaignIndexResetGen = 0;
  const campaignIndexGenerations = new WeakMap();
  function clear() {
    generation += 1; campaignGeneration += 1; definitionsGeneration += 1; campaignIndexResetGen += 1;
    mounted?.replaceChildren(); mounted?.removeAttribute("aria-busy"); mounted = null; selectedWork = "";
    campaignMounted?.replaceChildren(); campaignMounted?.removeAttribute("aria-busy"); campaignMounted = null;
    definitionsMounted?.replaceChildren(); definitionsMounted?.removeAttribute("aria-busy"); definitionsMounted = null;
    clearFleetCampaign();
  }
  async function load(root, ctx, work = selectedWork) {
    const current = ++generation; mounted = root; selectedWork = work;
    root.setAttribute("aria-busy", "true");
    const live = () => generation === current && root.isConnected && ctx.isCurrent();
    try {
      const projection = validate(await ctx.api(`/api/inventory${work ? `?work=${encodeURIComponent(work)}` : ""}`));
      if (!live()) return;
      const body = render(projection);
      const form = element("form", undefined, "inventory-work-form");
      const label = element("label", "Exact admitted work identity"); label.htmlFor = "inventory-work-identity";
      const input = element("input"); Object.assign(input, { id: "inventory-work-identity", value: work, type: "text", maxLength: 64, pattern: "[a-f0-9]{64}", placeholder: "64-character work BLAKE3 identity" });
      const submit = element("button", "Explain current work"); submit.type = "submit";
      const status = element("p", work && projection.work_requirements_unknown ? `Requested work ${work}: current admitted requirements not supplied; readiness unknown.` : undefined, "small"); status.setAttribute("role", "status");
      const reset = element("button", "Clear work selection"); reset.type = "button";
      reset.addEventListener("click", () => void load(root, ctx, ""));
      form.append(label, input, submit, reset, status);
      form.addEventListener("submit", (event) => { event.preventDefault(); const id = input.value.trim(); if (!/^[a-f0-9]{64}$/.test(id)) { status.textContent = "Enter one exact 64-character admitted work identity."; input.focus(); return; } void load(root, ctx, id); });
      body.firstChild.append(form); root.replaceChildren(body);
    } catch (error) {
      if (!live() || error?.name === "AbortError") return;
      const panel = element("section", undefined, "panel inventory-unavailable");
      panel.append(element("h2", "Observed inventories unavailable"), element("p", `${text(error?.message)} No absence, liveness, free compute or model capacity inferred. Configured project admission slots remain a different inventory.`, "small"));
      if (work) panel.append(element("p", `Requested exact work: ${work}`, "inventory-reference"));
      const retry = element("button", "Retry inventory read"); retry.type = "button"; retry.addEventListener("click", () => void load(root, ctx, work));
      panel.append(retry); root.replaceChildren(panel);
    } finally { if (live()) root.removeAttribute("aria-busy"); }
  }
  // r[impl kiln.cairn.workspace] Each selected change opens its exact Cairn change (repository identity and change
  // name). The links sit beside the inert projection, which keeps no control of its own.
  function cairnLinks(projection) {
    const nav = element("nav", undefined, "panel campaign-cairn-links");
    nav.setAttribute("aria-label", "Cairn changes in this campaign");
    nav.append(element("h3", "Cairn changes in this campaign"));
    const list = element("ul");
    for (const change of [...new Set(projection.members.map((member) => member.change))]) {
      const item = element("li");
      const link = element("a", change);
      link.href = `?page=cairn&cairn_repository=${projection.repository_identity}&cairn_change=${encodeURIComponent(change)}`;
      item.append(link);
      list.append(item);
    }
    nav.append(list);
    return nav;
  }
  async function loadCampaign(root, ctx, campaignId, newId = newRequestId, notice = "") {
    const current = ++campaignGeneration; campaignMounted = root;
    root.setAttribute("aria-busy", "true");
    const live = () => campaignGeneration === current && root.isConnected && ctx.isCurrent();
    try {
      const projection = validateCampaign(await ctx.api(`/api/implementation/campaigns?campaign=${encodeURIComponent(campaignId)}`));
      if (!live()) return;
      const holder = element("div", undefined, "campaign-controls-root");
      root.replaceChildren(renderCampaign(projection), cairnLinks(projection), holder);
      // The inbox has its own failure boundary; the projection stays read-only without it.
      void loadControls(holder, ctx, campaignId, live, (message) => { if (live()) void loadCampaign(root, ctx, campaignId, newId, message); }, newId, notice);
    } catch (error) {
      if (!live() || error?.name === "AbortError") return;
      const panel = element("section", undefined, "panel inventory-unavailable");
      panel.append(element("h2", "Selected campaign projection unavailable"), element("p", `${text(error?.message)} This does not imply no campaign or free capacity.`, "small"));
      root.replaceChildren(panel);
    } finally { if (live()) root.removeAttribute("aria-busy"); }
  }
  const CAMPAIGN_INDEX_SCHEMA = "kiln.implementation-campaigns.v1";
  function validateCampaignIndex(value) {
    if (!record(value) || value.schema !== CAMPAIGN_INDEX_SCHEMA || !Array.isArray(value.campaigns)
      || value.campaigns.length > MAX_ITEMS || typeof value.bounded !== "boolean" || typeof value.truncated !== "boolean") {
      throw new Error("Campaign index response is invalid.");
    }
    for (const item of value.campaigns) {
      if (!record(item) || typeof item.campaign_id !== "string" || !item.campaign_id.trim() || item.campaign_id.length > 256
        || typeof item.status !== "string" || !Number.isSafeInteger(item.pending_decisions) || item.pending_decisions < 0) {
        throw new Error("Campaign index entry is invalid.");
      }
    }
    return value;
  }
  function formatCampaignLabel(item) {
    const count = item.pending_decisions;
    return `${item.campaign_id} · ${item.status} · ${count} pending decision${count === 1 ? "" : "s"}`;
  }
  function renderCampaignIndex(value, selectedId, onSelect, options = {}) {
    const listing = validateCampaignIndex(value);
    const wrap = element("div", undefined, "campaign-index");
    if (!listing.campaigns.length) {
      const empty = element("p", "No active or recent implementation campaigns in the store.", "small");
      empty.setAttribute("role", "status");
      wrap.append(empty);
      if (selectedId) {
        const deep = element("p", `Deep-linked campaign: ${selectedId} (not in current index)`, "small");
        wrap.append(deep);
      }
      return wrap;
    }
    const label = element("label", "Implementation campaign");
    const selectId = options.selectId || "";
    if (selectId) label.htmlFor = selectId;
    const select = element("select");
    if (selectId) select.id = selectId;
    select.setAttribute("aria-label", "Implementation campaign");
    const placeholder = element("option", "Select a campaign…");
    placeholder.value = "";
    select.append(placeholder);
    let foundSelected = !selectedId;
    for (const item of listing.campaigns) {
      const option = element("option", formatCampaignLabel(item));
      option.value = item.campaign_id;
      if (item.campaign_id === selectedId) foundSelected = true;
      select.append(option);
    }
    if (selectedId && !foundSelected) {
      const deep = element("option", `${selectedId} · deep-linked (not in current index)`);
      deep.value = selectedId;
      select.append(deep);
    }
    select.value = selectedId || "";
    select.addEventListener("change", () => {
      if (onSelect) onSelect(select.value);
    });
    wrap.append(label, select);
    if (listing.truncated) {
      wrap.append(element("p", "Campaign index is bounded; older campaigns may be omitted.", "small"));
    }
    return wrap;
  }
  async function loadCampaignIndex(holder, ctx, selectedId = "", onSelect = () => {}, options = {}) {
    const resetGen = campaignIndexResetGen;
    const current = (campaignIndexGenerations.get(holder) || 0) + 1;
    campaignIndexGenerations.set(holder, current);
    const live = () => resetGen === campaignIndexResetGen && campaignIndexGenerations.get(holder) === current && holder.isConnected && ctx.isCurrent?.() !== false;
    holder.replaceChildren(element("p", "Reading campaign index…", "small"));
    try {
      const listing = validateCampaignIndex(await ctx.api("/api/implementation/campaigns", { quiet: true }));
      if (!live()) return;
      holder.replaceChildren(renderCampaignIndex(listing, selectedId, onSelect, options));
    } catch (error) {
      if (!live() || error?.name === "AbortError") return;
      holder.replaceChildren(controlsUnavailable(error, "Campaign index"));
    }
  }
  let fleetCampaignGeneration = 0;
  let fleetCampaignMounted = null;
  function clearFleetCampaign() {
    fleetCampaignGeneration += 1;
    fleetCampaignMounted?.replaceChildren();
    fleetCampaignMounted?.removeAttribute("aria-busy");
    fleetCampaignMounted = null;
  }
  // Fleet composes the existing frozen previews, admitted projection and owner-guarded inbox.
  // Each read fails independently; none of these surfaces implies provider authority.
  async function loadFleetCampaign(root, ctx, campaignId = "", onSelect = () => {}) {
    clearFleetCampaign();
    const current = fleetCampaignGeneration;
    fleetCampaignMounted = root;
    const live = () => fleetCampaignGeneration === current && root.isConnected && ctx.isCurrent();
    const indexRoot = element("section", undefined, "panel fleet-campaign-index");
    const definitionsRoot = element("div", undefined, "fleet-campaign-definitions");
    const selectedRoot = element("div", undefined, "fleet-campaign-selected");
    root.replaceChildren(indexRoot, definitionsRoot, selectedRoot);
    let selectionGeneration = 0;
    let chosenId = campaignId;
    let indexGeneration = 0;
    async function campaignIndex() {
      const readGeneration = ++indexGeneration;
      const indexLive = () => live() && indexGeneration === readGeneration;
      indexRoot.replaceChildren(element("h3", "Implementation campaigns"), element("p", "Reading campaign index…", "small"));
      try {
        const listing = validateCampaignIndex(await ctx.api("/api/implementation/campaigns", { quiet: true }));
        if (!indexLive()) return;
        indexRoot.replaceChildren(element("h3", "Implementation campaigns"), renderCampaignIndex(listing, chosenId, select, { selectId: "fleet-campaign-select" }));
      } catch (error) {
        if (indexLive() && error?.name !== "AbortError") {
          indexRoot.replaceChildren(element("h3", "Implementation campaigns"), controlsUnavailable(error, "Campaign index"));
        }
      }
    }
    async function selected(id, notice = "") {
      const selectedGeneration = ++selectionGeneration;
      chosenId = id;
      const selectedLive = () => live() && selectionGeneration === selectedGeneration;
      if (!id) {
        selectedRoot.replaceChildren(element("p", "Choose an existing campaign above to inspect its exact selection, progress and decision inbox.", "small"));
        return;
      }
      if (typeof id !== "string" || !id.trim() || id.length > 256) {
        selectedRoot.replaceChildren(element("p", "Invalid exact campaign selection; no controls are offered.", "small"));
        return;
      }
      selectedRoot.replaceChildren(element("p", `Reading exact campaign ${id}…`, "small"));
      selectedRoot.setAttribute("aria-busy", "true");
      try {
        const projection = validateCampaign(await ctx.api(`/api/implementation/campaigns?campaign=${encodeURIComponent(id)}`));
        if (!selectedLive()) return;
        if (projection.campaign_id !== id) throw new Error("The projection names a different campaign.");
        const controlsRoot = element("div", undefined, "campaign-controls-root");
        const refreshButton = controlButton("Refresh exact campaign");
        refreshButton.addEventListener("click", () => { if (selectedLive()) { void selected(id); void definitions(); void campaignIndex(); } });
        selectedRoot.replaceChildren(renderCampaign(projection), cairnLinks(projection), refreshButton, controlsRoot);
        try {
          const inbox = await readInbox(ctx, id);
          if (!selectedLive()) return;
          if (inbox.campaign_identity !== projection.campaign_identity || inbox.store_version !== projection.store_version) {
            controlsRoot.replaceChildren(element("p", "Campaign changed between the progress and inbox reads. Refresh the exact campaign before using controls.", "small"));
            return;
          }
          const panel = {};
          const refresh = (message) => { if (selectedLive()) { void selected(id, message); void definitions(); void campaignIndex(); } };
          const ui = runnerUi(panel, refresh, (_result, message) => refresh(message), notice);
          const runner = commandRunner(ctx, ui);
          panel.current = renderControls(inbox, (intent) => { if (selectedLive()) void runner.start(intent); });
          controlsRoot.replaceChildren(panel.current, ui.status, ui.retry);
        } catch (error) {
          if (selectedLive() && error?.name !== "AbortError") controlsRoot.replaceChildren(controlsUnavailable(error, "Campaign controls"));
        }
      } catch (error) {
        if (selectedLive() && error?.name !== "AbortError") selectedRoot.replaceChildren(element("p", `Exact campaign ${id} unavailable: ${text(error?.message)} No alternate campaign or control was substituted.`, "small"));
      } finally { if (selectedLive()) selectedRoot.removeAttribute("aria-busy"); }
    }
    function select(id) {
      if (!live()) return;
      const selectEl = indexRoot.querySelector?.("#fleet-campaign-select");
      if (selectEl && selectEl.value !== id) selectEl.value = id;
      void selected(id);
      onSelect(id);
    }
    let definitionsGeneration = 0;
    async function definitions(notice = "") {
      if (ctx?.capabilities?.campaign_controls === false) {
        definitionsRoot.replaceChildren();
        return;
      }
      const readGeneration = ++definitionsGeneration;
      const definitionsLive = () => live() && definitionsGeneration === readGeneration;
      definitionsRoot.setAttribute("aria-busy", "true");
      try {
        const listing = validateDefinitions(await ctx.api("/api/implementation/campaign-definitions"));
        if (!definitionsLive()) return;
        const panel = {};
        const ui = runnerUi(panel, (message) => { if (definitionsLive()) void definitions(message); }, (result) => {
          if (definitionsLive()) { select(result.campaign_id); void definitions(); void campaignIndex(); }
        }, notice);
        const runner = commandRunner(ctx, ui);
        panel.current = renderDefinitions(listing, (intent) => { if (definitionsLive()) void runner.start(intent); }, select);
        definitionsRoot.replaceChildren(panel.current, ui.status, ui.retry);
      } catch (error) {
        if (definitionsLive() && error?.name !== "AbortError") definitionsRoot.replaceChildren(controlsUnavailable(error, "Campaign definitions"));
      } finally { if (definitionsLive()) definitionsRoot.removeAttribute("aria-busy"); }
    }
    await Promise.all([campaignIndex(), definitions(), selected(chosenId)]);
  }
  window.KilnInventory = Object.freeze({ validate, sourceBasis, refText, render, validateCampaign, validateCampaignIndex, renderCampaignIndex, loadCampaignIndex, renderCampaign, cairnLinks, validateInbox, validateDefinitions, renderControls, renderDefinitions, commandRunner, runnerUi, decisionAnswer, readInbox, controlsUnavailable, newRequestId, load, loadCampaign, loadDefinitions, loadFleetCampaign, clearFleetCampaign, clear, scopes: SCOPE });
})();
