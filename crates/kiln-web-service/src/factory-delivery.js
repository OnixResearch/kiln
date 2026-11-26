// Delivery and workflow observations. Only the existing owner views can act.
(() => {
  "use strict";
  const CAIRN = "/api/implementation/cairn";
  const REVIEW = "/api/code-review";
  const hex = /^[a-f0-9]{64}$/;
  const str = (value) => typeof value === "string" && value.length > 0;
  const list = (value) => Array.isArray(value) ? value : [];
  const one = (rows, matches) => {
    const found = list(rows).filter(matches);
    return found.length === 1 ? found[0] : null;
  };
  const word = (value) => typeof value === "boolean" ? (value ? "Yes" : "No") : (str(value) || Number.isSafeInteger(value)) ? String(value) : "Not recorded";
  const ref = (value) => [value?.provider_namespace, value?.native_kind, value?.native_id].filter(str).join(" / ");
  const identity = (value) => typeof value === "string" && hex.test(value);
  const campaignId = (value) => str(value) && value.trim() && new TextEncoder().encode(value).length <= 256 && !/[\p{Cc}\p{Cs}]/u.test(value);
  const query = (values) => new URLSearchParams(values).toString();
  const generations = new WeakMap();
  function el(tag, text, className) {
    const node = document.createElement(tag);
    if (text !== undefined) node.textContent = text;
    if (className) node.className = className;
    return node;
  }
  const note = (text, warning = false) => el("p", text, warning ? "warning" : "small");
  function panel(root, title) {
    const node = el("section", undefined, "panel view-stack");
    node.append(el("h2", title));
    root.append(node);
    return node;
  }
  function facts(node, entries) {
    const dl = el("dl", undefined, "facts");
    for (const [label, value] of entries) dl.append(el("dt", label), el("dd", word(value)));
    node.append(dl);
  }
  function bullets(node, rows) {
    if (!rows.length) return;
    const ul = el("ul");
    for (const row of rows) ul.append(el("li", row));
    node.append(ul);
  }
  function choices(node, ctx, label, rows, choose) {
    if (!rows.length) return;
    const field = el("label", label), select = el("select");
    const prompt = el("option", "Choose…");
    prompt.value = "";
    select.append(prompt);
    rows.forEach(({ title }, index) => {
      const option = el("option", title);
      option.value = String(index);
      select.append(option);
    });
    select.addEventListener("change", () => {
      if (select.value !== "" && rows[Number(select.value)]) ctx.navigate(choose(rows[Number(select.value)]));
    });
    field.append(select);
    node.append(field);
  }
  function start(root, ctx) {
    const sequence = (generations.get(root) || 0) + 1;
    generations.set(root, sequence);
    const current = () => generations.get(root) === sequence && (!ctx.isCurrent || ctx.isCurrent());
    if (current()) root.replaceChildren();
    // Keep each source's failure local; a failed index must not block an exact detail read.
    async function read(section, label, url, render) {
      const slot = el("div", undefined, "view-stack");
      slot.append(note(`Loading ${label}…`));
      section.append(slot);
      try {
        const value = await ctx.api(url);
        if (!current()) return;
        const result = el("div", undefined, "view-stack");
        const rendering = render(result, value);
        if (current()) slot.replaceChildren(result);
        await rendering;
      } catch (error) {
        if (current()) slot.replaceChildren(note(`${label} unavailable: ${error?.message || "source did not return an observation"}.`, true));
      }
    }
    return { current, read };
  }
  function routeLink(node, ctx, patch, label) {
    node.append(ctx.link(patch, label));
  }
  function reviewPatch(page, change, revision = "") {
    return { page, review_change: change, review_revision: revision, review_base: "", review_interdiff: "",
      review_interdiff_change: "", review_commits: "", review_preview_compare: "", review_preview_structural: "",
      review_file: "", review_side: "", review_line: "" };
  }
  async function campaignPublication(node, ctx, read, projection, member) {
    node.append(el("h3", "Publication and integration"));
    const effects = list(projection.pending_effects).filter(effect => effect?.member === member.member
      && effect.selection_identity === projection.selection_identity && list(member.pending_effect_ids).includes(effect.effect_id)
      && ["publish", "integrate", "lifecycle"].includes(effect.operation));
    if (effects.length) bullets(node, effects.map(effect => `${effect.operation}: ${word(effect.state)} · ${word(effect.owner)} · ${word(effect.effect_id)} · progress applied: ${word(effect.progress_applied)}`));
    else node.append(note("No matching publication, integration or lifecycle effect is listed for this selection. This does not establish an outcome."));
    if (!identity(projection.repository_identity) || !str(member.change) || !identity(member.work_identity)
      || !identity(member.attempt_identity) || !Number.isSafeInteger(member.attempt_fence)) {
      node.append(note("The projection does not supply an exact work, attempt and fence for publication read-back."));
      return;
    }
    await read(node, "Cairn publication read-back", `${CAIRN}/detail?${query({ repository: projection.repository_identity, change: member.change })}`, (target, value) => {
      const detail = value?.schema === "kiln.cairn-workspace-restricted-detail.v1" ? value.detail : value;
      if (detail?.schema !== "kiln.cairn-workspace-detail.v4" || detail.repository_identity !== projection.repository_identity
        || detail.change !== member.change || !Array.isArray(detail.units)) throw new Error("Cairn returned a different or malformed change");
      const membership = one(detail.memberships, row => row?.campaign_id === projection.campaign_id && row.member === member.member
        && row.store_version === projection.store_version && row.selection_revision === projection.selection_revision
        && row.work_identity === member.work_identity && row.attempt_identity === member.attempt_identity && row.attempt_fence === member.attempt_fence);
      const matches = detail.units.flatMap(unit => list(unit?.attempts).filter(attempt => attempt?.identity === member.attempt_identity
        && attempt.work_identity === member.work_identity && attempt.fence === member.attempt_fence).map(attempt => ({ unit, attempt })));
      if (!membership || matches.length !== 1) {
        target.append(note("Cairn did not return the same campaign version, member and attempt binding. No publication record was substituted.", true));
        return;
      }
      const { unit, attempt } = matches[0], record = attempt.publication;
      if (!record) target.append(note("No publication record was returned for this exact attempt."));
      else facts(target, [["Recorded publication phase", record.phase], ["Attempt applicability", attempt.applicability],
        ["Source status", record.source_status], ["Branch", record.branch], ["Base branch", record.base_branch],
        ["Pull request reference", ref(record.pull_request)], ["Remote head", ref(record.remote_head)], ["Merge commit", ref(record.merge_commit)]]);
      routeLink(target, ctx, { page: "cairn", cairn_repository: projection.repository_identity,
        cairn_change: member.change, cairn_unit: unit.native_id }, "Open exact Cairn unit");
    });
  }
  function renderEnvironments(node, value) {
    if (value?.schema !== "kiln.delivery-environments.v1" || typeof value.configured !== "boolean" || !Array.isArray(value.observations)) {
      throw new Error("unexpected deployment observation projection");
    }
    if (!value.configured) {
      node.append(note("Deployment rollout unavailable: no deployment owner is configured. No deployment action is available.", true));
      return;
    }
    if (!value.observations.length) node.append(note("The configured deployment source returned no observations. Activation, health and rollback remain unknown."));
    for (const row of value.observations) {
      if (!str(row?.environment) || !str(row.service) || !str(row.owner)
        || !["fresh", "stale"].includes(row.freshness) || !["healthy", "degraded", "unhealthy", "unknown"].includes(row.health)) {
        throw new Error("deployment observation does not name an exact target and recorded health and freshness");
      }
      const target = el("article");
      target.append(el("h3", `${row.environment} / ${row.service}`));
      if (row.freshness === "stale") target.append(note("Stale observation: these recorded values do not establish current rollout state.", true));
      facts(target, [["Deployment owner", row.owner], ["Intended revision", row.intended_revision],
        ["Observed revision", row.observed_revision], ["Rollback revision", row.rollback_revision], ["Observed at (ms)", row.observed_at_ms],
        ["Freshness", row.freshness], ["Recorded health", row.health], ["Recorded blocker", row.blocker]]);
      node.append(target);
    }
  }
  async function load(root, ctx) {
    const { current, read } = start(root, ctx);
    if (!current()) return;
    const route = ctx.route || {};
    const campaign = panel(root, "Campaign delivery evidence");
    campaign.append(note("Campaign progress and code-review readiness are separate observations. No relationship is inferred between these selections."));
    const lookup = el("form"), field = el("label", "Exact campaign ID"), input = el("input");
    input.value = route.campaign || "";
    input.maxLength = 256;
    input.required = true;
    input.addEventListener("input", () => input.setCustomValidity(""));
    field.append(input);
    const open = el("button", "Inspect campaign");
    open.type = "submit";
    lookup.append(field, open);
    lookup.addEventListener("submit", (event) => {
      event.preventDefault();
      if (!campaignId(input.value)) {
        input.setCustomValidity("Enter a campaign identity of up to 256 UTF-8 bytes without control characters.");
        input.reportValidity();
        return;
      }
      ctx.navigate({ page: "delivery", campaign: input.value, delivery_member: "" });
    });
    campaign.append(lookup);
    const review = panel(root, "Merge readiness · Code Review");
    const rollout = panel(root, "Deployment rollout observations");
    rollout.append(note("Only the configured deployment source can report these observations. Publication, merge, a preview or workflow selection does not establish activation, health or rollback. This view is read-only."));
    const reads = [
      read(rollout, "deployment observations", "/api/delivery/environments", renderEnvironments),
      read(campaign, "Cairn campaign index", CAIRN, (node, value) => {
        const index = value?.schema === "kiln.cairn-workspace-restricted-index.v1" ? value.index : value;
        if (index?.schema !== "kiln.cairn-workspace-index.v2" || !Array.isArray(index.changes)) throw new Error("unexpected Cairn index");
        const ids = [...new Set(index.changes.flatMap(change => list(change?.campaigns)).filter(str))];
        choices(node, ctx, "Campaign from Cairn", ids.map(id => ({ id, title: id })), ({ id }) => ({ page: "delivery", campaign: id, delivery_member: "" }));
        if (!ids.length || index.truncated || index.snapshot?.campaigns_truncated) node.append(note("This bounded Cairn index does not enumerate all campaigns."));
      }),
      read(review, "code-review index", REVIEW, (node, value) => {
        if (value?.schema !== "kiln.code-review.v1" || typeof value.configured !== "boolean" || !Array.isArray(value.changes)) throw new Error("unexpected review index");
        if (!value.configured) { node.append(note("Code Review is not configured.", true)); return; }
        const rows = value.changes.filter(change => identity(change?.id)).map(change => ({ id: change.id, title: `${change.title || "Review change"} · ${change.id}` }));
        choices(node, ctx, "Review change", rows, ({ id }) => reviewPatch("delivery", id));
        if (!rows.length) node.append(note("No changes were returned in this review window."));
        else if (value.next_offset != null) node.append(note("Only the first review window is shown; use Code Review to search other entries."));
        routeLink(node, ctx, reviewPatch("code-review", ""), "Browse Code Review");
      }),
    ];
    if (!route.campaign) campaign.append(note(route.delivery_member ? "An exact campaign is required to select that member." : "Choose a campaign to inspect its selected members."));
    else if (!campaignId(route.campaign)) campaign.append(note("That exact campaign identity is invalid. No campaign was substituted.", true));
    else reads.push(read(campaign, "exact campaign projection", `/api/implementation/campaigns?campaign=${encodeURIComponent(route.campaign)}`, async (node, value) => {
      if (value?.schema !== "kiln.selected-campaign-projection.v1" || value.campaign_id !== route.campaign
        || !identity(value.selection_identity) || !Array.isArray(value.members)) throw new Error("the service did not return the requested campaign");
      facts(node, [["Campaign", value.campaign_id], ["Selection identity", value.selection_identity], ["Selection revision", value.selection_revision], ["Store version", value.store_version]]);
      const members = value.members.filter(member => str(member?.member));
      choices(node, ctx, "Campaign member", members.map(member => ({ id: member.member, title: `${member.member} · ${word(member.stage)}` })),
        ({ id }) => ({ page: "delivery", campaign: value.campaign_id, delivery_member: id }));
      routeLink(node, ctx, { page: "workers", worker_campaign: value.campaign_id }, "Open campaign owner view");
      if (!route.delivery_member) { node.append(note(members.length ? "Choose an exact campaign member. No member is selected automatically." : "No members were listed.")); return; }
      const member = one(members, row => row.member === route.delivery_member);
      if (!member) { node.append(note(`Campaign member unavailable or ambiguous: ${route.delivery_member}. None was substituted.`, true)); return; }
      facts(node, [["Member", member.member], ["Observed", member.observed], ["Campaign stage", member.stage], ["Goal", member.goal],
        ["Work identity", member.work_identity], ["Attempt identity", member.attempt_identity], ["Attempt fence", member.attempt_fence],
        ["Candidate identity", member.candidate_identity], ["Integration target identity", member.integration_target_identity],
        ["Campaign checks satisfied", member.checks_satisfied], ["Campaign reviews satisfied", member.reviews_satisfied]]);
      bullets(node, [...list(member.remaining_obligations), ...list(member.blockers)].filter(str));
      await campaignPublication(node, ctx, read, value, member);
    }));
    if (!route.review_change || !identity(route.review_change)) review.append(note(route.review_change || route.review_revision
      ? "That selection needs an exact Code Review change identity. No change was substituted." : "Choose a Code Review change to read its submit checklist."));
    else reads.push(read(review, "exact review change", `${REVIEW}/changes/${route.review_change}`, (node, value) => {
      const change = value?.change;
      if (change?.id !== route.review_change || !Array.isArray(change.revisions)) throw new Error("the service did not return the requested review change");
      const offered = change.revisions.filter(revision => identity(revision?.id));
      facts(node, [["Review change", change.id], ["Repository", change.repository], ["Target", change.target_ref], ["Change state", change.state]]);
      choices(node, ctx, "Review revision", offered.map(revision => ({ id: revision.id, title: `Revision ${word(revision.number)} · ${revision.id}` })),
        ({ id }) => reviewPatch("delivery", change.id, id));
      if (!route.review_revision) { node.append(note("Choose an exact revision. No revision is selected automatically.")); return; }
      const revision = one(offered, row => row.id === route.review_revision);
      if (!revision) { node.append(note(`Review revision unavailable or ambiguous: ${route.review_revision}. None was substituted.`, true)); return; }
      facts(node, [["Review revision", revision.id], ["Commit", revision.commit], ["Base", revision.base]]);
      const currentRevision = change.revisions.at(-1)?.id;
      if (revision.id !== currentRevision) node.append(note("The owner checklist describes the current revision, not this historical selection. Readiness for this revision is unavailable.", true));
      else if (typeof value.checklist?.ready !== "boolean") node.append(note("Submit checklist unavailable.", true));
      else {
        node.append(note(value.checklist.ready ? "Owner checklist: Ready to submit. Not a submission or rollout." : "Owner checklist: Submit blocked.", !value.checklist.ready));
        bullets(node, list(value.checklist.items).map(item => `${word(item.key)} · ${item.satisfied === true ? "satisfied" : item.satisfied === false ? "not satisfied" : "unknown"} · ${word(item.detail)}`));
      }
      if (change.submission?.candidate?.revision === revision.id) facts(node, [["Observed submission result commit", change.submission.result_commit]]);
      routeLink(node, ctx, reviewPatch("code-review", change.id, revision.id), "Open exact revision and its preview evidence in Code Review");
    }));
    await Promise.all(reads);
  }

  function renderWorkflow(node, ctx, value) {
    const schema = "kiln.observe.inventory.v2";
    if (value?.schema !== schema || value.snapshot?.schema !== schema || typeof value.snapshot.synthetic !== "boolean"
      || !Array.isArray(value.sources) || !Array.isArray(value.facts)
      || (value.snapshot.workflow_revisions !== undefined && !Array.isArray(value.snapshot.workflow_revisions))) throw new Error("unexpected inventory projection");
    const sources = value.sources.filter(source => source?.kind === "workflow_revisions");
    if (value.snapshot.synthetic) node.append(note("Synthetic inventory: fixture observations, not live workflow selection.", true));
    if (!sources.length) node.append(note("No workflow revision source is configured for this inventory. Workflow revisions are unknown here.", true));
    else bullets(node, sources.map(source => `${word(source.source)} · ${word(source.producer)} / ${word(source.scope)} · ${word(source.state)} · generation ${word(source.generation)}${str(source.reason) ? ` · ${source.reason}` : ""}`));
    const defs = list(value.snapshot.workflow_revisions).filter(row => str(row?.meta?.reference?.source) && str(row?.meta?.reference?.id));
    const route = ctx.route || {};
    choices(node, ctx, "Workflow definition", defs.map(({ meta: { reference } }) => ({ ...reference, title: `${reference.source} / ${reference.id}` })),
      ({ source, id }) => ({ page: "runbooks", workflow_source: source, workflow_definition: id, workflow_revision: "" }));
    if (!route.workflow_source && !route.workflow_definition && !route.workflow_revision) {
      node.append(note(defs.length ? "Choose a workflow definition. None is selected automatically." : "No workflow definitions were returned by the admitted inventory."));
      return;
    }
    const def = one(defs, row => row.meta.reference.source === route.workflow_source && row.meta.reference.id === route.workflow_definition);
    if (!def || !Array.isArray(def.revisions)) { node.append(note("The exact workflow source and definition were not returned together. No definition was substituted.", true)); return; }
    const source = one(sources, row => row.source === def.meta.reference.source);
    const observation = one(value.facts, row => row?.reference?.source === def.meta.reference.source && row.reference.id === def.meta.reference.id);
    const selection = def.selected_for_new_sessions;
    const revisions = def.revisions.filter(row => str(row?.revision));
    function slot(field, id) {
      if (list(def.meta.unknown_fields).includes(field)) return "Unknown (producer marked this field unknown)";
      return str(id) && !one(revisions, row => row.revision === id) ? `${id} (revision not listed unambiguously)` : id;
    }
    facts(node, [["Workflow definition", `${def.meta.reference.source} / ${def.meta.reference.id}`], ["Source state", source?.state],
      ["Fact state", observation?.state], ["Observed at (ms)", def.meta.observed_at_ms], ["Valid until (ms)", def.meta.valid_until_ms],
      ["Desired revision", slot("desired", def.desired)], ["Last usable revision", slot("last_usable", def.last_usable)],
      ["Selected for new sessions", slot("selected_for_new_sessions", selection?.revision)], ["Selection kind", selection?.kind],
      ["Workflow rollout generation", selection?.rollout_generation], ["Qualification reference", selection?.qualification]]);
    if (source?.state !== "current" || observation?.state !== "current") node.append(note("The workflow observation is not confirmed current. Recorded slots are not current-state evidence.", true));
    const pins = list(def.pinned);
    node.append(el("h3", "Pinned sessions"));
    if (list(def.meta.unknown_fields).includes("pinned")) node.append(note("Pin observations marked unknown by the producer.", true));
    else if (!pins.length) node.append(note("No pin groups recorded in this observation."));
    else bullets(node, pins.map(pin => `${word(slot("pinned", pin.revision))} · ${word(pin.sessions)} sessions · ${word(pin.status)} · blocked: ${word(pin.blocked)}`));
    choices(node, ctx, "Workflow revision", revisions.map(row => ({ id: row.revision, title: row.revision })),
      ({ id }) => ({ page: "runbooks", workflow_source: def.meta.reference.source, workflow_definition: def.meta.reference.id, workflow_revision: id }));
    if (!route.workflow_revision) return;
    const row = one(revisions, entry => entry.revision === route.workflow_revision);
    if (!row) { node.append(note(`Workflow revision unavailable or ambiguous: ${route.workflow_revision}. None was substituted.`, true)); return; }
    const compiled = row.workflow, worker = row.worker;
    facts(node, [["Workflow revision", row.revision], ["Compiler revision", compiled?.compiler_revision], ["Compiler artifact", compiled?.compiler_artifact],
      ["Plan", compiled?.plan], ["Graph", compiled?.graph], ["Cairn selection", compiled?.selection], ["Cairn obligations", compiled?.obligations],
      ["Policy", compiled?.policy], ["Worker definition", worker?.definition ? `${word(worker.definition.source)} / ${word(worker.definition.id)}` : null],
      ["Worker revision", worker?.revision], ["Worker bundle", worker?.bundle]]);
  }
  async function workflows(root, ctx) {
    const { current, read } = start(root, ctx);
    if (!current()) return;
    const slots = panel(root, "Workflow revision observations");
    slots.append(note("Workflow selection is for new sessions. It does not move existing pins and is not application deployment or rollout authority."));
    const bindings = panel(root, "Reviewed runbook revisions and previews");
    bindings.append(note("Runbook bindings come from the reviewed registry. They are shown separately: the registry does not name an inventory workflow definition. Previewing remains in the runbook owner form."));
    await Promise.all([
      read(slots, "workflow inventory", "/api/inventory", (node, value) => renderWorkflow(node, ctx, value)),
      read(bindings, "runbook registry", "/api/runbooks", (node, value) => {
        if (value?.schema !== "kiln.web-runbooks-view.v1" || typeof value.configured !== "boolean" || !Array.isArray(value.runbooks)) throw new Error("unexpected runbook registry");
        if (!value.configured) { node.append(note("The reviewed runbook registry is not configured.", true)); return; }
        const rows = value.runbooks.filter(row => str(row?.id));
        const runbookPatch = (runbook) => ({ page: "runbooks", runbook, dispatch: "", fanout: "", rollup_cursor: "" });
        choices(node, ctx, "Reviewed runbook", rows.map(row => ({ id: row.id, title: `${row.display_name || row.id} · ${row.id}` })),
          ({ id }) => runbookPatch(id));
        if (!rows.length) node.append(note("No reviewed runbooks were returned for this principal."));
        const requested = ctx.route?.runbook;
        if (!requested) return;
        const runbook = one(rows, row => row.id === requested);
        if (!runbook) { node.append(note(`Runbook unavailable or ambiguous: ${requested}. None was substituted.`, true)); return; }
        facts(node, [["Runbook", runbook.id], ["Registry workflow name", runbook.lattice_workflow_name],
          ["Registry workflow revision", runbook.lattice_workflow_revision], ["Bundle ID", runbook.lattice_bundle_id],
          ["Bundle identity", runbook.lattice_bundle_identity], ["Recorded dispatch eligibility", runbook.can_dispatch],
          ["Recorded dispatch refusal", runbook.dispatch_refusal]]);
        if (ctx.route?.fanout || ctx.route?.dispatch) routeLink(node, ctx, runbookPatch(runbook.id), "Open exact runbook and preview form");
        else {
          const preview = el("a", "Go to preview form for this runbook");
          preview.href = "#runbook-dispatch-form";
          node.append(preview);
        }
      }),
    ]);
  }
  window.KilnFactoryDelivery = { load, workflows };
})();
