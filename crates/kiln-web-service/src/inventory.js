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
    for (const value of values) node.append(element("li", `${text(value.code)} · ${text(value.constraint)} · ${value.subject ? refText(value.subject) : "snapshot / policy"}`));
    return node;
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
    header.append(element("h2", `Selected campaign · ${projection.campaign_id}`),
      element("p", `Selection revision ${number(projection.selection_revision)} · store version ${number(projection.store_version)} · projection ${projection.identity}`, "inventory-reference"),
      element("p", "Read-only admitted snapshot. A proposed next action is not a grant, reservation, dispatch, landing, or lifecycle authorization.", "small"));
    page.append(header);
    const nodes = projection.members.map((member) => {
      const article = element("article", undefined, "inventory-row campaign-member");
      article.append(element("h4", `${member.change}${member.tasks.length ? ` · ${member.tasks.join(", ")}` : ""}`),
        facts([["Selected member", member.member], ["Goal", member.goal], ["Current observed stage", member.stage], ["Enumeration", member.enumeration], ["Owner snapshot", member.owner_snapshot], ["Work identity / canonical status / revision", `${text(member.work_identity)} / ${text(member.canonical_work_state)} / ${number(member.canonical_work_revision)}`], ["Attempt / fence", `${text(member.attempt_identity)} / ${number(member.attempt_fence)}`], ["Candidate", member.candidate_identity], ["Integration target", member.integration_target_identity], ["Checks satisfied", member.checks_satisfied], ["Reviews satisfied", member.reviews_satisfied]]),
        details("Freshness, dependencies, evidence, pending effects and exact resource binding", { freshness: member.input_freshness, dependencies: member.dependencies, evidence: member.evidence, pending_effect_ids: member.pending_effect_ids, resource_inventory_identity: member.resource_inventory_identity, resource_binding_state: member.resource_binding_state, resource_binding: member.resource_binding, required_checks: member.required_checks, required_reviews: member.required_reviews, wait_categories: member.wait_categories }),
        element("p", `Wait categories: ${member.waits.length ? member.waits.join(", ") : "none explicitly represented; not readiness evidence"}`),
        element("p", `Remaining obligations: ${member.remaining_obligations.length ? member.remaining_obligations.join(", ") : "none supplied"}`),
        selectionHold(member.selection_hold));
      const listNode = element("ul", undefined, "inventory-blockers");
      if (!member.blockers.length) listNode.append(element("li", "No blocker strings supplied; do not infer eligibility."));
      else for (const blocker of member.blockers) listNode.append(element("li", blocker));
      article.append(listNode); return article;
    });
    page.append(section("Every selected member, including blocked and incomplete members", "campaign-members", nodes));
    page.append(section("Pending effects, human decisions, aggregate budget, next-action proposal", "campaign-obligations", [
      details("Snapshot-level obligations and non-claims", { campaign_identity: projection.campaign_identity, resource_inventory_identity: projection.resource_inventory_identity, pending_effects: projection.pending_effects, decisions: projection.decisions, budget: projection.budget, next_action: projection.next_action, selection_basis: projection.selection_basis, source: projection.source, non_claims: projection.non_claims }),
    ]));
    return page;
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
      ["Desired revision", agent.revisions.desired], ["Prepared revision", agent.revisions.prepared], ["Qualified revision", agent.revisions.qualified], ["Selected for new sessions", agent.revisions.selected_for_new_sessions], ["Preparation failure", agent.revisions.preparation_failure], ["Revision generation", number(agent.revisions.generation)],
      ["Observed instances of this definition", String(snapshot.workers.filter((w) => refText(w.definition) === refText(agent.meta.reference)).length) + " supplied; not a fleet total"],
    ], details("Qualification: exact subject / corpus / protocol, accepted denominators, cost, unknown use and limitations (not availability)", agent.qualifications)))));
    page.append(section("Workers — actual instances and pinned bindings", "inventory-workers", snapshot.workers.map((worker) => row(projection, worker, `${worker.meta.reference.id} · ${worker.lifecycle}`, [
      ["Incarnation", worker.incarnation], ["Stable session", worker.session], ["Definition", refText(worker.definition)], ["Actually pinned revision", worker.pinned_revision], ["Observed lifecycle", worker.lifecycle], ["Attempt", worker.attempt], ["Fence", number(worker.fence)], ["Execution binding", refText(worker.execution)], ["Execution grant", refText(worker.execution_grant)], ["Workspace alias", worker.workspace], ["Independent model-service bindings", list(worker.model_services)],
    ]))));
    const computeRows = snapshot.compute.map((device) => row(projection, device, `${device.kind} · ${device.host}`, [
      ["Host", device.host], ["Device", device.device], ["Provider pool", device.pool], ["Health / draining", device.health], ["Sandbox capabilities", list(device.sandbox)], ["Backend capabilities", list(device.backends)], ["Shared accounting pool identities", list(device.accounting_pools)], ["Compatibility / qualification", list(device.qualification)],
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
      work.append(facts([["Exact work version", explanation.work_version], ["Definition", refText(explanation.definition)], ["Policy", explanation.policy_ref], ["Candidate combinations", number(explanation.combinations_considered)], ["Truncated", String(explanation.truncated)], ["Dispatch authorization", "Never granted by this view"]]), blockers(explanation.blockers));
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
    work.append(details("Owning producer admission/rejection reasons and policy/budget/waiting facts", { admissions: snapshot.admissions, policies: snapshot.policies }));
    page.append(work);
    return page;
  }
  let generation = 0;
  let mounted = null;
  let selectedWork = "";
  let campaignGeneration = 0;
  let campaignMounted = null;
  function clear() {
    generation += 1; campaignGeneration += 1;
    mounted?.replaceChildren(); mounted?.removeAttribute("aria-busy"); mounted = null; selectedWork = "";
    campaignMounted?.replaceChildren(); campaignMounted?.removeAttribute("aria-busy"); campaignMounted = null;
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
  async function loadCampaign(root, ctx, campaignId) {
    const current = ++campaignGeneration; campaignMounted = root;
    root.setAttribute("aria-busy", "true");
    const live = () => campaignGeneration === current && root.isConnected && ctx.isCurrent();
    try {
      const projection = validateCampaign(await ctx.api(`/api/implementation/campaigns?campaign=${encodeURIComponent(campaignId)}`));
      if (!live()) return;
      root.replaceChildren(renderCampaign(projection));
    } catch (error) {
      if (!live() || error?.name === "AbortError") return;
      const panel = element("section", undefined, "panel inventory-unavailable");
      panel.append(element("h2", "Selected campaign projection unavailable"), element("p", `${text(error?.message)} This does not imply no campaign or free capacity.`, "small"));
      root.replaceChildren(panel);
    } finally { if (live()) root.removeAttribute("aria-busy"); }
  }
  window.KilnInventory = Object.freeze({ validate, sourceBasis, refText, render, validateCampaign, renderCampaign, load, loadCampaign, clear });
})();
