"use strict";
// r[impl kiln.web.exploration.scenario.coverage]
// r[impl kiln.web.exploration.cases]
(async () => {
  const $ = id => document.getElementById(id);
  const node = (tag, text, cls) => { const el = document.createElement(tag); if (text) el.textContent = text; if (cls) el.className = cls; return el; };
  try {
    const response = await fetch("/examples.json");
    if (!response.ok) throw new Error(`Example index unavailable (${response.status})`);
    const payload = await response.json();
    const workspace = payload.workspace;
    const examples = Array.isArray(payload.examples) ? payload.examples : [];
    const scenarios = examples.map(example => {
      const card = node("article", "", "example-card");
      const title = node("h3"), link = node("a", example.title); link.href = example.href;
      if (example.case_id) link.dataset.caseId = example.case_id;
      if (example.recipe && example.recipe.revision) link.dataset.recipeRevision = example.recipe.revision;
      title.append(link);
      const recipeText = example.recipe ? `${example.recipe.id} ${example.recipe.version} ${example.recipe.revision}` : "no recipe";
      const caseText = example.case_id ? `Case ${example.case_id}` : "Untitled example";
      const meta = node("p", `${caseText} · ${recipeText}`, "example-meta");
      card.append(title, node("p", example.detail), meta);
      $("scenarios").append(card);
      return { card, example, query: `${example.title} ${example.detail} ${example.case_id || ""}`.toLowerCase() };
    });
    let covered = 0;
    for (const destination of window.KilnUi.destinations) {
      const sample = examples.find(example => new URL(example.href, location.href).searchParams.get("page") === destination.page);
      const card = node("article", "", "example-card");
      const title = node("h3"), link = node("a", `${destination.number} ${destination.title}`);
      link.href = sample ? sample.href : `/?page=${destination.page}`; title.append(link);
      card.append(title, node("p", destination.purpose));
      card.append(node("p", sample ? `Try: ${sample.title} (${sample.case_id || "no case"})` : "No maintained scenario is linked for this destination.", "example-meta"));
      if (sample) covered++;
      $("destinations").append(card);
    }
    $("coverage-status").textContent = `${covered} of ${window.KilnUi.destinations.length} registered destinations have named examples. ${examples.length} linked feature scenarios.`;

    const caseSelect = $("case-select");
    const themeSelect = $("case-theme");
    const densitySelect = $("case-density");
    const viewportSelect = $("case-viewport");
    const openLink = $("case-open-link");
    const caseMeta = $("case-meta");
    const requestedRecipe = new URLSearchParams(location.search).get("recipe");
    const resolveCase = async (example, recipeRevision) => {
      if (!example || !example.case_id || !example.recipe) return null;
      const revision = recipeRevision || example.recipe.revision;
      const url = `/examples/cases/${encodeURIComponent(example.case_id)}?recipe=${encodeURIComponent(revision)}&workspace=${encodeURIComponent(workspace)}`;
      const res = await fetch(url);
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.message || `Case resolution failed (${res.status})`);
      }
      return res.json();
    };
    const updateOpenLink = async () => {
      const selected = examples.find(item => item.case_id === caseSelect.value);
      if (!selected) return;
      const params = new URLSearchParams();
      params.set("case", selected.case_id);
      params.set("recipe", selected.recipe.revision);
      params.set("source", selected.source_revision || "");
      params.set("theme", themeSelect.value);
      params.set("density", densitySelect.value);
      params.set("viewport", viewportSelect.value);
      const base = selected.href || "/";
      const separator = base.includes("?") ? "&" : "?";
      openLink.href = `${base}${separator}${params.toString()}`;
      openLink.textContent = `Open ${selected.case_id}`;
      try {
        const resolved = await resolveCase(selected, requestedRecipe);
        caseMeta.textContent = `${resolved.case_id} · ${resolved.recipe.id} ${resolved.recipe.version} ${resolved.recipe.revision} · workspace ${resolved.workspace} · route ${resolved.route}`;
      } catch (error) {
        caseMeta.textContent = error.message;
      }
    };
    if (caseSelect) {
      for (const example of examples) {
        if (!example.case_id) continue;
        const option = document.createElement("option");
        option.value = example.case_id;
        option.textContent = `${example.case_id} - ${example.title}`;
        caseSelect.append(option);
      }
      [themeSelect, densitySelect, viewportSelect, caseSelect].forEach((select) => {
        if (select) select.addEventListener("change", updateOpenLink);
      });
      caseSelect.addEventListener("keydown", event => {
        if (event.key === "Enter" && openLink.href) openLink.click();
      });
      const requestedCase = new URLSearchParams(location.search).get("case");
      if (requestedCase && examples.some(item => item.case_id === requestedCase)) {
        caseSelect.value = requestedCase;
      }
      if (requestedCase && !examples.some(item => item.case_id === requestedCase)) {
        caseMeta.textContent = `Unknown or moved case: ${requestedCase}`;
      } else {
        await updateOpenLink();
      }
    }

    const filter = () => {
      const q = $("example-filter").value.trim().toLowerCase(); let count = 0;
      for (const item of scenarios) { item.card.hidden = !item.query.includes(q); if (!item.card.hidden) count++; }
      $("scenario-count").textContent = `${count} of ${scenarios.length} scenarios`;
    };
    $("example-filter").addEventListener("input", filter); filter();
  } catch (error) { $("coverage-status").textContent = error.message; }
})();
