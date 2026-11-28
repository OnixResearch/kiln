// Read-only runtime observations for Fleet attempts. Inventory v2 is the
// producer-admitted boundary; a dispatch plan or Fleet session is not a worker fact.
(() => {
  "use strict";
  const SCHEMA = "kiln.observe.inventory.v2";
  const TOKEN = /^[A-Za-z0-9_.:@+-]{1,256}$/;
  const ID = /^[a-f0-9]{64}$/;
  const STATES = new Set(["current", "missing", "stale", "future", "incomplete", "out_of_order", "unavailable", "unknown"]);
  const record = (value) => value !== null && typeof value === "object" && !Array.isArray(value);
  const exact = (value) => Number.isSafeInteger(value) && value >= 0;
  const token = (value) => typeof value === "string" && TOKEN.test(value) && !value.includes("..");
  const ref = (value) => record(value) && token(value.source) && token(value.id);
  const key = (value) => JSON.stringify([value.source, value.id]);
  const name = (value) => ref(value) ? `${value.source} / ${value.id}` : "Unknown";
  const shown = (value) => typeof value === "string" && value ? value : "Unknown / not supplied";
  const element = (tag, value, className) => {
    const node = document.createElement(tag);
    if (value !== undefined) node.textContent = String(value);
    if (className) node.className = className;
    return node;
  };
  function line(node, label, value) {
    const row = element("p", undefined, "small");
    row.append(element("strong", `${label}: `), element("span", String(value)));
    node.append(row);
  }
  function validate(projection) {
    const snapshot = projection?.snapshot;
    if (!record(projection) || projection.schema !== SCHEMA || !ID.test(projection.identity)
      || !exact(projection.evaluated_at_ms) || !record(snapshot) || snapshot.schema !== SCHEMA
      || typeof snapshot.synthetic !== "boolean" || !Array.isArray(projection.sources) || projection.sources.length > 64
      || !Array.isArray(projection.facts) || projection.facts.length > 4096
      || !Array.isArray(projection.attempt_explanations) || projection.attempt_explanations.length > 4096
      || ["workers", "compute", "models", "admissions"].some((type) => !Array.isArray(snapshot[type]) || snapshot[type].length > 4096)
      || projection.sources.some((s) => !token(s?.source) || !STATES.has(s.state))
      || projection.facts.some((f) => !ref(f?.reference) || !STATES.has(f.state))
      || ["workers", "compute", "models", "admissions"].some((type) => snapshot[type].some((row) => !ref(row?.meta?.reference) || !Array.isArray(row.meta.unknown_fields)))) {
      throw new Error("Inventory v2 projection invalid; runtime evidence unavailable.");
    }
    return projection;
  }
  function index(rows, selector) {
    const found = new Map();
    for (const row of rows) {
      const identity = selector(row);
      found.set(identity, found.has(identity) ? null : row);
    }
    return found;
  }
  // The projection's source and fact states are calculated by the admitting
  // service. Expiry since evaluation can only make them less trustworthy.
  function current(projection, meta, now) {
    if (!ref(meta?.reference) || !Array.isArray(meta.unknown_fields) || meta.unknown_fields.length
      || !exact(meta.valid_until_ms) || now >= meta.valid_until_ms) return false;
    const source = projection.sources.find((s) => s.source === meta.reference.source);
    const fact = projection.facts.find((f) => ref(f.reference) && key(f.reference) === key(meta.reference));
    return source?.state === "current" && fact?.state === "current"
      && exact(source.valid_until_ms) && now < source.valid_until_ms;
  }
  function exactBinding(projection, binding, worker, admissions, now) {
    if (!ref(binding.worker) || key(binding.worker) !== key(worker.meta.reference)
      || binding.attempt !== worker.attempt || binding.fence !== worker.fence
      || !ref(binding.execution_grant) || !ref(worker.execution_grant)
      || key(binding.execution_grant) !== key(worker.execution_grant)
      || !Array.isArray(binding.model_services) || !Array.isArray(binding.request_admissions)
      || binding.model_services.length !== binding.request_admissions.length
      || binding.model_services.length !== worker.model_services?.length
      || binding.model_services.some((service, i) => !ref(service) || !ref(worker.model_services[i]) || key(service) !== key(worker.model_services[i]))) return false;
    const pairs = [[binding.execution_grant, worker.execution],
      ...binding.request_admissions.map((grant, i) => [grant, binding.model_services[i]])];
    return pairs.every(([grantRef, resource]) => {
      if (!ref(grantRef) || !ref(resource)) return false;
      const grant = admissions.get(key(grantRef));
      return grant && current(projection, grant.meta, now) && grant.state === "admitted"
        && grant.work_version === binding.work_version && grant.attempt === binding.attempt
        && grant.fence === binding.fence && ref(grant.resource) && key(grant.resource) === key(resource);
    });
  }
  function observation(projection, attempt, now = Date.now()) {
    const snapshot = projection.snapshot;
    const result = { worker: null, machine: null, binding: null, services: [], reasons: [] };
    if (snapshot.synthetic) { result.reasons.push("Synthetic inventory; no live runtime attribution."); return result; }
    if (!ID.test(attempt?.attempt) || !exact(attempt.fence)) {
      result.reasons.push("Fleet attempt or fence not recorded exactly."); return result;
    }
    const workers = snapshot.workers.filter((worker) => worker.attempt === attempt.attempt && worker.fence === attempt.fence);
    if (workers.length !== 1) {
      result.reasons.push(workers.length ? "Multiple worker facts match this attempt/fence; incarnation ambiguous." : "No worker fact matches this exact attempt/fence; absence is not proof of termination.");
      return result;
    }
    const worker = workers[0];
    if (!current(projection, worker.meta, now) || !token(worker.incarnation)
      || !ref(worker.execution) || !token(worker.pinned_revision) || typeof worker.lifecycle !== "string") {
      result.reasons.push("Matching worker fact is stale, incomplete, expired or invalid; lifecycle unknown.");
      return result;
    }
    result.worker = worker;
    const devices = snapshot.compute.filter((device) => ref(device.meta.reference) && key(device.meta.reference) === key(worker.execution));
    if (devices.length === 1 && current(projection, devices[0].meta, now) && token(devices[0].host)) {
      const host = devices[0].host;
      if (typeof attempt.session?.machine === "string" && attempt.session.machine && attempt.session.machine !== host) {
        result.reasons.push("Fleet session machine conflicts with admitted execution host; machine unknown.");
      } else result.machine = host;
    } else result.reasons.push("Exact execution device/host observation not current; compute machine unknown.");
    const bindings = projection.attempt_explanations.filter(({ binding }) => binding?.attempt === attempt.attempt && binding.fence === attempt.fence);
    const work = attempt.work;
    if (!ID.test(work)) {
      result.reasons.push("Fleet work identity absent; request/serving lineage unknown."); return result;
    }
    if (record(attempt.session) && (attempt.session.work !== work
      || attempt.session.attempt !== attempt.attempt || attempt.session.fence !== attempt.fence)) {
      result.reasons.push("Fleet session identity disagrees with this work/attempt/fence; serving lineage unknown.");
      return result;
    }
    const matching = bindings.filter(({ binding }) => binding.work_version === work && ref(binding.worker) && key(binding.worker) === key(worker.meta.reference));
    if (matching.length !== 1 || bindings.length !== 1) {
      result.reasons.push("Unique exact work/attempt/fence/worker binding not supplied; serving lineage unknown."); return result;
    }
    const { binding, blockers } = matching[0];
    const admissions = index(snapshot.admissions, (grant) => key(grant.meta.reference));
    if (!Array.isArray(blockers) || blockers.length || !exactBinding(projection, binding, worker, admissions, now)) {
      result.reasons.push(`Exact owner admission not proven${Array.isArray(blockers) && blockers.length ? ` (${blockers.map((b) => shown(b.code)).join(", ")})` : ""}; serving lineage unknown.`);
      return result;
    }
    result.binding = binding;
    const models = index(snapshot.models, (model) => key(model.meta.reference));
    for (const serviceRef of binding.model_services) {
      const model = models.get(key(serviceRef));
      if (!model || !current(projection, model.meta, now) || !token(model.artifact)
        || !token(model.configuration) || !token(model.deployment) || !token(model.claimed_model)
        || model.artifact_available !== true || !token(model.artifact_verification_evidence)
        || model.artifact_verification_evidence === "redacted") {
        result.reasons.push(`Admitted request to ${name(serviceRef)}, but current verified artifact/deployment observation absent; serving lineage unknown for this service.`);
        continue;
      }
      result.services.push({ reference: serviceRef, model });
    }
    if (!binding.model_services.length) result.reasons.push("No request-admitted model services bound; serving model unknown.");
    return result;
  }
  const PRODUCERS = [
    "Lattice worker producer: admitted source/generation, bounded validity and completeness; stable worker reference, incarnation, pinned definition/revision, exact attempt/fence, lifecycle and execution reference. A session name or placement plan alone is not worker liveness.",
    "Aspen/compute producer: current device reference with observed host, health and accounting-pool provenance. Fleet session machine cannot substitute for an execution-device observation; capacity requires authoritative pool accounting, not device counts.",
    "Kiln/owner producer: exact current work/attempt/fence/worker binding, execution grant and per-service request admissions; each grant must be current, admitted and resource-matched. Rejection, stale fences and missing grants never become permission.",
    "Animus/serving producer: current service reference, artifact availability and verification evidence, configuration/deployment and claimed model. Request admission binds the service, not a proof of actual weight bytes or that a specific request used a replica; native request/replica provenance is still required for that claim.",
  ];
  function render(root, supplied, attempts) {
    const projection = validate(supplied);
    if (!Array.isArray(attempts) || attempts.length > 4096) throw new Error("Fleet attempts list invalid.");
    const now = Date.now();
    const page = element("section", undefined, "panel fleet-section fleet-runtime");
    page.append(element("h2", "Exact runtime worker observations"),
      element("p", `Read-only inventory ${projection.identity} evaluated at ${projection.evaluated_at_ms} ms. Worker and serving facts are distinct from Fleet placement and configured capacity.`, "small"));
    if (!attempts.length) page.append(element("p", "No attempts in this bounded Fleet list; not evidence that no workers exist.", "small"));
    for (const attempt of attempts) {
      const observed = observation(projection, attempt, now);
      const article = element("article", undefined, "inventory-row fleet-runtime-attempt");
      article.append(element("h3", `Attempt ${shown(attempt?.attempt)} · fence ${exact(attempt?.fence) ? attempt.fence : "unknown"}`));
      if (observed.worker) {
        const worker = observed.worker;
        line(article, "Worker / incarnation", `${name(worker.meta.reference)} / ${worker.incarnation}`);
        line(article, "Observed lifecycle", worker.lifecycle);
        line(article, "Pinned revision", worker.pinned_revision);
        line(article, "Execution reference", name(worker.execution));
        line(article, "Compute machine", observed.machine || "Unknown; Fleet placement is not compute evidence");
        line(article, "Worker source", `${worker.meta.reference.source}; observed ${worker.meta.observed_at_ms} ms; valid until ${worker.meta.valid_until_ms} ms`);
      } else line(article, "Runtime worker", "Unknown; no current unique incarnation/lifecycle proof");
      if (observed.binding) {
        line(article, "Admitted owner binding", `work ${observed.binding.work_version}; execution grant ${name(observed.binding.execution_grant)}; request grants ${observed.binding.request_admissions.map(name).join(", ") || "none"}`);
      }
      if (observed.services.length) {
        for (const { reference, model } of observed.services) {
          line(article, `Request-admitted serving ${name(reference)}`, `claimed model ${model.claimed_model}; artifact ${model.artifact}; configuration ${model.configuration}; deployment ${model.deployment}; artifact verification evidence ${model.artifact_verification_evidence}; state ${model.state}. Claim is not proof of actual weight bytes or specific request/replica.`);
        }
      } else line(article, "Serving model / lineage", "Unknown; planned or permitted models do not prove an admitted request or artifact.");
      for (const reason of observed.reasons) article.append(element("p", reason, "small"));
      page.append(article);
    }
    const contracts = element("details", undefined, "inventory-disclosure");
    contracts.open = root.querySelector(".fleet-runtime details.inventory-disclosure")?.open === true;
    contracts.append(element("summary", "Remaining native producer contracts — not supplied by Fleet"));
    const list = element("ul");
    for (const contract of PRODUCERS) list.append(element("li", contract));
    contracts.append(list);
    page.append(contracts);
    root.replaceChildren(page);
    return page;
  }
  let generation = 0;
  let mounted = null;
  let controller = null;
  function clear() {
    generation += 1;
    controller?.abort(); controller = null;
    mounted?.replaceChildren(); mounted?.removeAttribute("aria-busy"); mounted = null;
  }
  async function load(root, ctx, attempts) {
    const current = ++generation;
    controller?.abort();
    mounted = root;
    if (ctx?.capabilities?.inventory === false) {
      root.replaceChildren();
      root.removeAttribute("aria-busy");
      return;
    }
    const request = new AbortController(); controller = request;
    root.setAttribute("aria-busy", "true");
    if (!root.querySelector(".fleet-runtime")) {
      root.replaceChildren(element("p", "Reading admitted runtime inventory…", "small"));
    }
    const live = () => current === generation && root.isConnected && ctx.isCurrent();
    try {
      const projection = await ctx.api("/api/inventory", { quiet: true, controller: request });
      if (live()) render(root, projection, attempts);
    } catch (error) {
      if (live()) root.replaceChildren(element("p", `Runtime inventory unavailable: ${shown(error?.message)}. Worker incarnation, lifecycle, compute host and serving lineage unknown; Fleet placement does not fill these gaps.`, "small"));
    } finally {
      if (current === generation) controller = null;
      if (live()) root.removeAttribute("aria-busy");
    }
  }
  window.KilnFleetRuntime = Object.freeze({ load, clear, render, observation });
})();
