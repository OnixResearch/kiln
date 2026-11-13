"use strict";
// r[impl kiln.web.exploration.scenario.coverage]
(async () => {
  const $ = id => document.getElementById(id);
  const node = (tag, text, cls) => { const el = document.createElement(tag); if (text) el.textContent = text; if (cls) el.className = cls; return el; };
  try {
    const response = await fetch("/examples.json");
    if (!response.ok) throw new Error(`Example index unavailable (${response.status})`);
    const { examples } = await response.json();
    const scenarios = examples.map(example => {
      const card = node("article", "", "example-card");
      const title = node("h3"), link = node("a", example.title); link.href = example.href; title.append(link);
      card.append(title, node("p", example.detail));
      $("scenarios").append(card);
      return { card, query: `${example.title} ${example.detail}`.toLowerCase() };
    });
    let covered = 0;
    for (const destination of window.KilnUi.destinations) {
      const sample = examples.find(example => new URL(example.href, location.href).searchParams.get("page") === destination.page);
      const card = node("article", "", "example-card");
      const title = node("h3"), link = node("a", `${destination.number} ${destination.title}`);
      link.href = sample ? sample.href : `/?page=${destination.page}`; title.append(link);
      card.append(title, node("p", destination.purpose));
      card.append(node("p", sample ? `Try: ${sample.title}` : "No maintained scenario is linked for this destination.", "example-meta"));
      if (sample) covered++;
      $("destinations").append(card);
    }
    $("coverage-status").textContent = `${covered} of ${window.KilnUi.destinations.length} registered destinations have named examples. ${examples.length} linked feature scenarios.`;
    const filter = () => {
      const q = $("example-filter").value.trim().toLowerCase(); let count = 0;
      for (const item of scenarios) { item.card.hidden = !item.query.includes(q); if (!item.card.hidden) count++; }
      $("scenario-count").textContent = `${count} of ${scenarios.length} scenarios`;
    };
    $("example-filter").addEventListener("input", filter); filter();
  } catch (error) { $("coverage-status").textContent = error.message; }
})();
