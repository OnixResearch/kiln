// Separate-origin parent. Destroying the iframe disposes the existing engine's
// workers/linear memory; TryNix does not expose an in-place VM replacement API.
const $ = id => document.getElementById(id);
let frame = null, timer = null, manifest = null;
const status = text => { $("status").textContent = text; };
function stop(reason, error = null) {
  clearTimeout(timer);
  timer = null; frame?.remove(); frame = null;
  if (error && window.kilnPreviewObservation && !window.kilnPreviewObservation.result) window.kilnPreviewObservation.result = { type: "result", status: null, error, output: "", preview_identity: manifest.identity };
  $("stop").disabled = true;
  $("start").disabled = !manifest || Date.now() >= manifest.descriptor.limits.expires_at * 1000;
  status(reason);
}
function phase(row) {
  const li = document.createElement("li");
  li.textContent = `${row.phase}: ${row.outcome} (${Math.round(row.elapsed_ms)} ms)${row.detail ? ` · ${row.detail}` : ""}`;
  li.dataset.outcome = row.outcome; $("phases").append(li);
}
window.addEventListener("message", event => {
  if (!frame || event.source !== frame.contentWindow || event.origin !== location.origin || event.data?.preview_identity !== manifest?.identity) return;
  const data = event.data;
  window.kilnPreviewObservation ||= { identity: manifest.identity, phases: [], result: null };
  if (data.type === "phase") window.kilnPreviewObservation.phases.push(data);
  if (data.type === "result") window.kilnPreviewObservation.result = data;
  if (data.type === "phase") phase(data);
  if (data.type === "result") {
    $("output").textContent = String(data.output || "").slice(0, manifest.descriptor.limits.output_bytes);
    status(data.status === 0 ? "Exact artifact finished successfully. Preview only; CI evidence is unchanged." : `Preview failed: ${data.error || `exit ${data.status}`}. CI evidence is unchanged.`);
    const report = window.kilnPreviewObservation;
    // Retain only bounded text evidence, not a live VM after program completion.
    stop($("status").textContent);
    void retain(report);
  }
});
// r[impl kiln.preview.comparison] Report only phases and the bounded result; the
// server copies every identity from its admitted descriptor.
async function retain(report) {
  const body = JSON.stringify({
    phases: report.phases.map(({ phase, outcome, elapsed_ms, detail }) => detail === undefined ? { phase, outcome, elapsed_ms } : { phase, outcome, elapsed_ms, detail }),
    result: { status: report.result.status ?? null, output: String(report.result.output || ""), error: report.result.error ?? null },
  });
  try {
    const response = await fetch("observation", { method: "POST", credentials: "omit", cache: "no-store", headers: { "Content-Type": "application/json" }, body });
    if (!response.ok) throw new Error(`${response.status} ${(await response.text()).slice(0, 256)}`);
    const retained = await response.json();
    report.retained = retained.identity;
    $("retention").textContent = `Observation retained for review comparison: ${retained.identity}`;
  } catch (error) {
    $("retention").textContent = `Observation not retained: ${error.message}. CI evidence is unchanged.`;
  }
}
$("start").addEventListener("click", () => {
  if (!manifest || frame) return;
  const remaining = manifest.descriptor.limits.expires_at * 1000 - Date.now();
  if (remaining <= 0) return stop("Preview expired; no artifact was substituted.");
  $("start").disabled = true; $("stop").disabled = false; $("output").textContent = ""; $("phases").replaceChildren(); $("retention").textContent = "";
  window.kilnPreviewObservation = { identity: manifest.identity, phases: [], result: null };
  frame = document.createElement("iframe"); frame.title = "Exact disposable Linux artifact";
  // This origin is disposable and has no console/repository authority. Same
  // origin is needed by the existing engine's module/worker/SharedArrayBuffer.
  frame.sandbox = "allow-scripts allow-same-origin";
  frame.allow = "cross-origin-isolated"; frame.referrerPolicy = "no-referrer";
  frame.src = "engine/frame.html"; $("session").append(frame);
  status("Launching exact artifact. No builds or moving revision lookup are permitted.");
  timer = setTimeout(() => stop("Preview resource/lifetime budget exhausted; guest removed. CI evidence is unchanged.", "preview_resource_exhausted"), Math.min(remaining, manifest.descriptor.limits.run_seconds * 1000));
});
$("stop").addEventListener("click", () => stop("Preview stopped and guest resources removed. CI evidence is unchanged.", "preview_cancelled"));
window.addEventListener("pagehide", () => stop("Preview closed.", "preview_closed"));
try {
  const response = await fetch("descriptor.json", { credentials: "omit", cache: "no-store" });
  if (!response.ok) throw new Error(`descriptor unavailable (${response.status})`);
  manifest = await response.json(); const d = manifest.descriptor;
  const facts = { "Descriptor": manifest.identity, ...(d.binding ? { "Project / repository": `${d.binding.project} / ${d.binding.repository}`, "Exact source": d.binding.revision, "Candidate": d.binding.candidate_id, "Run / attempt": `${d.binding.run_id} / ${d.binding.attempt_id}`, "Build receipt": d.binding.build_receipt, "Artifact": d.binding.artifact_identity } : { "Qualification": "Engine and exact signed CLI only — no canonical CI binding", "Exact cached path": d.root_path }), "Executable": d.executable, "Engine": d.engine.identity, "Platform": d.platform, "Expires": new Date(d.limits.expires_at * 1000).toISOString() };
  for (const [key,value] of Object.entries(facts)) { const dt=document.createElement("dt"), dd=document.createElement("dd"); dt.textContent=key; dd.textContent=String(value); $("identity").append(dt,dd); }
  $("start").disabled = Date.now() >= d.limits.expires_at * 1000;
  status($("start").disabled ? "Preview expired." : d.binding ? "Exact admitted artifact discovered. Launch is opt-in." : "Engine qualification only; not CI evidence. Launch is opt-in.");
} catch (error) { manifest=null; status(`Preview unavailable: ${error.message}. CI evidence is unchanged.`); }
