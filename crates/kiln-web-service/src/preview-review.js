/* Review-only projection. Nothing here sends a CI/review/merge command. */
export function safePreviewURL(value, descriptorIdentity) {
  try {
    const u=new URL(value), path=/^\/[0-9a-f]{64}\/([0-9a-f]{64})\/index\.html$/.exec(u.pathname);
    return u.protocol==="http:" && u.hostname==="127.0.0.1" && !!u.port && !u.username && !u.password && !u.search && !u.hash && path!==null && path[1]===descriptorIdentity ? u.href : null;
  } catch { return null; }
}
export function eligiblePreview(row, context, now=Date.now()) {
  const b=row?.binding;
  return !!b && b.project===context.project && b.repository===context.repository && b.revision===context.revision
    && context.currentRevision===context.revision && !!context.selectedRun && row.review_run_id===context.selectedRun && context.reviewCandidateRun===context.selectedRun
    && typeof b.candidate_id==="string" && b.candidate_id.length>0
    && typeof b.attempt_id==="string" && b.attempt_id.length>0
    && [b.build_receipt,b.artifact_identity,b.store_identity,row.descriptor_identity,row.engine_identity].every(value=>/^[0-9a-f]{64}$/.test(value))
    && Number.isSafeInteger(row.expires_at) && row.expires_at*1000>now && safePreviewURL(row.launch_url,row.descriptor_identity)!==null;
}
export function renderPreviews(parent, rows, context) {
  const make=(tag,text)=> { const el=document.createElement(tag); if(text!==undefined)el.textContent=text; return el; };
  const note=make("p","Preview observations never rewrite CI, review or merge evidence. Only opted-in current exact-source artifacts are launchable."); note.className="cr-muted"; parent.append(note);
  const entries=Array.isArray(rows)?rows.slice(0,16):[];
  if (!entries.length) { parent.append(make("p","No admitted runnable artifact preview is retained for this candidate. Artifact/check digests alone are not launch permission.")); return; }
  for(const row of entries) {
    const article=make("article"), facts=make("dl"); article.className="list-row cr-operation";
    const binding=row.binding||{};
    for(const [key,value]of [["Source",binding.revision],["Review CI run",row.review_run_id], ["Build run / attempt",`${binding.run_id||"not recorded"} / ${binding.attempt_id||"not recorded"}`],["Candidate",binding.candidate_id],["Build receipt",binding.build_receipt],["Product",binding.product],["Artifact",binding.artifact_identity],["Store",binding.store_identity],["Executable",row.executable],["Runner",row.engine_identity],["Descriptor",row.descriptor_identity]]) {
      const dt=make("dt",key),dd=make("dd",value||"Not recorded"); dd.className="cr-principal"; facts.append(dt,dd);
    }
    article.append(facts);
    if(eligiblePreview(row,context)) {
      const a=make("a","Launch exact artifact in disposable Linux session"); a.href=safePreviewURL(row.launch_url,row.descriptor_identity); a.target="_blank"; a.rel="noopener noreferrer"; a.className="button quiet";
      // Recheck on click, not just on rendering, so retained expired DOM cannot
      // present a launch as current. The isolated consumer rechecks expiry too.
      a.addEventListener("click",event=> { if(!context.isCurrent()||!eligiblePreview(row,context)) { event.preventDefault(); a.replaceWith(make("p","Preview expired or review candidate moved; refresh exact evidence.")); } });
      article.append(a);
    } else article.append(make("p","Preview unavailable: expired, historical, mismatched, or not bound to canonical successful CI. No replacement artifact will be built."));
    parent.append(article);
  }
}

// r[impl kiln.preview.comparison] Baseline/candidate comparison over service-projected
// observations. The baseline lane never offers a launch; nothing here writes review state.
const OBSERVATION_IDENTITY = /^[0-9a-f]{64}$/;
const MAX_LANE_OUTPUT = 16384;
export const COMPARISON_REASONS = Object.freeze({
  same_observation: "Baseline and candidate are the same observation.",
  engine_differs: "The preview engine identities differ.",
  platform_differs: "The platforms differ.",
  arguments_differ: "The program arguments differ.",
  executable_differs: "The executable path within the product differs.",
});
export function comparisonRoute(value) {
  const match = /^([0-9a-f]{64})~([0-9a-f]{64})$/.exec(value || "");
  return match ? { baseline: match[1], candidate: match[2] } : null;
}
export function comparisonChoices(observations) {
  const rows = Array.isArray(observations) ? observations.filter(row => OBSERVATION_IDENTITY.test(row?.identity || "") && row?.observation) : [];
  return { baselines: rows, candidates: rows.filter(row => row.current === true) };
}
export function explainComparison(comparison) {
  if (comparison?.compatible === true) return ["Comparable: same engine, platform, arguments and executable path."];
  return (Array.isArray(comparison?.reasons) ? comparison.reasons : []).map(code => COMPARISON_REASONS[code] || `Unrecognised incompatibility: ${code}`);
}
// A response applies only to the pair still selected and still current.
export function acceptComparison(requested, selectedNow, response, isCurrent) {
  const c = response?.comparison;
  return requested === selectedNow && isCurrent() && !!c && `${c.baseline}~${c.candidate}` === requested && response?.candidate?.current === true;
}
export function laneFacts(row) {
  const o = row?.observation || {}, b = o.binding || {}, invocation = o.invocation || {};
  return [
    ["Observation", row?.identity], ["Currency", row?.current ? "Current revision and selected CI run" : "Historical"],
    ["Project / repository", `${b.project || "?"} / ${b.repository || "?"}`], ["Source", b.revision],
    ["Review CI run", row?.review_run_id || "No retained canonical build backs this observation"],
    ["Build run / attempt", `${b.run_id || "?"} / ${b.attempt_id || "?"}`], ["Artifact", b.artifact_identity],
    ["Descriptor", o.descriptor_identity], ["Engine", o.engine_identity], ["Platform", o.platform],
    ["Invocation", [invocation.executable, ...(Array.isArray(invocation.arguments) ? invocation.arguments : [])].join(" ")],
    ["Observed", Number.isSafeInteger(o.observed_at) ? new Date(o.observed_at * 1000).toISOString() : "Not recorded"],
    ["Exit status", o.result?.status ?? "No exit status"], ["Error", o.result?.error || "None"],
  ];
}
// r[impl kiln.preview.structural] Opt-in, display-only structural evidence that the
// service computed for the two exact revisions; no grammar runs here.
const MAX_STRUCTURAL_FILES = 64, MAX_STRUCTURAL_ROWS = 400;
export const STRUCTURAL_REASONS = Object.freeze({
  unsupported_language: "No pinned grammar parses this file.",
  binary: "Binary content.",
  too_large: "A side exceeds the byte bound.",
  too_many_nodes: "A side exceeds the token bound.",
  work_bound: "Matching exceeded the work bound.",
  interrupted: "Matching exceeded the time bound.",
  parse_error: "A side does not parse.",
  truncated: "The provider truncated this comparison.",
  unsupported_provider: "The review provider cannot read these revisions.",
  source_unavailable: "The review provider could not read the two revisions.",
  evidence_exhausted: "The record exceeded its size bound, so it keeps no files.",
});
export function structuralReason(code) {
  return STRUCTURAL_REASONS[code] || COMPARISON_REASONS[code] || `Unrecognised reason: ${code}`;
}
// Evidence applies only to the comparison and the exact revisions it was computed for.
export function acceptStructural(response) {
  const record = response?.structural, e = record?.evidence;
  return !!e && OBSERVATION_IDENTITY.test(record.identity || "") && e.comparison === response?.comparison?.identity
    && e.baseline_revision === response?.baseline?.observation?.binding?.revision
    && e.candidate_revision === response?.candidate?.observation?.binding?.revision;
}
export function structuralSummary(file) {
  if (file?.status !== "structural") return `Fallback: ${structuralReason(file?.reason)} No partial tree is shown.`;
  const hunks = Array.isArray(file.hunks) ? file.hunks.length : 0, moved = Array.isArray(file.moved) ? file.moved.length : 0;
  if (!hunks && !moved) return "No syntactic change: identical or formatting only.";
  return `${hunks} changed region${hunks === 1 ? "" : "s"} and ${moved} moved unchanged block${moved === 1 ? "" : "s"}; ${file.novel_old_tokens ?? 0} removed and ${file.novel_new_tokens ?? 0} added tokens.`;
}
// Rows as "<side> <old line> <new line>  <text>"; exact blob line numbers, bounded.
export function structuralLines(file, limit = MAX_STRUCTURAL_ROWS) {
  const rows = (Array.isArray(file?.hunks) ? file.hunks : []).flatMap(hunk => Array.isArray(hunk?.rows) ? hunk.rows : []);
  const mark = { old: "-", new: "+", both: " " };
  const lines = rows.slice(0, limit).map(row => `${mark[row?.side] || "?"} ${String(row?.old_line ?? "").padStart(5)} ${String(row?.new_line ?? "").padStart(5)}  ${String(row?.text ?? "")}`);
  return { lines, omitted: Math.max(0, rows.length - limit) };
}
export function renderStructural(parent, response) {
  const make = (tag, text, cls) => { const el = document.createElement(tag); if (text !== undefined) el.textContent = String(text); if (cls) el.className = cls; return el; };
  const section = make("section", undefined, "cr-operation cr-preview-structural"); parent.append(section);
  section.append(make("h4", "Structural source evidence"), make("p", "Opt-in, display-only preview evidence: the service compared the two exact revisions with the review's structural engine. It never changes CI, review approval, merge or completion state and does not measure review time.", "cr-muted"));
  if (!acceptStructural(response)) { section.append(make("p", "Structural evidence is missing or bound to another comparison. Nothing was substituted.", "cr-notice")); return section; }
  const { identity, evidence } = response.structural, settings = evidence.settings || {};
  const facts = make("dl", undefined, "cr-facts");
  for (const [key, value] of [["Evidence", identity], ["Comparison", evidence.comparison], ["Baseline source", evidence.baseline_revision], ["Candidate source", evidence.candidate_revision],
    ["Rules", `${settings.rules} · engine ${settings.engine_rules}`], ["Grammar identity", settings.grammar_identity],
    ["Bounds", `${settings.max_bytes} bytes, ${settings.max_nodes} tokens and ${settings.max_steps} steps per file; ${settings.deadline_ms} ms; ${settings.context_lines} context lines`], ["Whitespace", settings.whitespace]]) {
    facts.append(make("dt", key), make("dd", value ?? "Not recorded", "cr-principal"));
  }
  section.append(facts);
  const reasons = Array.isArray(evidence.reasons) ? evidence.reasons : [];
  if (evidence.status !== "structural") {
    section.append(make("p", evidence.status === "incompatible" ? "Structural evidence withheld: the subjects are not comparable, so source is not compared as if only it changed." : "Structural evidence unavailable. No other revision was substituted.", "cr-notice"));
    const list = make("ul"); for (const code of reasons) list.append(make("li", structuralReason(code))); section.append(list);
    return section;
  }
  const files = Array.isArray(evidence.files) ? evidence.files : [];
  if (!files.length) { section.append(make("p", "No source change between the two revisions.")); return section; }
  const list = make("ul");
  for (const file of files.slice(0, MAX_STRUCTURAL_FILES)) {
    const item = make("li"); item.append(make("code", file?.path || "?"), make("p", structuralSummary(file)));
    for (const block of file?.status === "structural" && Array.isArray(file.moved) ? file.moved : []) item.append(make("p", `Moved unchanged: lines ${block.old_start}–${block.old_end} → ${block.new_start}–${block.new_end}.`, "cr-muted"));
    const { lines, omitted } = structuralLines(file);
    if (lines.length) { const rows = make("pre", lines.join("\n")); rows.setAttribute("aria-label", `${file.path} structural rows: removed, added and context lines with baseline and candidate line numbers`); item.append(rows); }
    if (omitted) item.append(make("p", `${omitted} more rows are not shown.`, "cr-muted"));
    list.append(item);
  }
  section.append(list);
  if (files.length > MAX_STRUCTURAL_FILES) section.append(make("p", `${files.length - MAX_STRUCTURAL_FILES} more files are not shown.`, "cr-muted"));
  return section;
}
export function renderComparison(parent, observations, context) {
  const make = (tag, text, cls) => { const el = document.createElement(tag); if (text !== undefined) el.textContent = String(text); if (cls) el.className = cls; return el; };
  const box = make("section", undefined, "card cr-card cr-preview-compare"); box.setAttribute("aria-labelledby", "cr-preview-compare-title");
  const title = make("h3", "Compare preview evidence"); title.id = "cr-preview-compare-title"; title.tabIndex = -1;
  box.append(title, make("p", "Observations are browser-reported preview evidence. They never change CI, review approval or merge state. Only the current candidate can launch.", "cr-muted"));
  parent.append(box);
  const { baselines, candidates } = comparisonChoices(observations);
  const requested = comparisonRoute(context.requested);
  const linked = !!requested && baselines.some(row => row.identity === requested.baseline) && candidates.some(row => row.identity === requested.candidate);
  if (context.requested && !linked) {
    box.append(make("p", "The linked baseline or candidate observation is missing, expired or no longer current. No substitute was selected.", "cr-notice"));
  }
  if (!baselines.length) { box.append(make("p", "No retained preview observation exists for this change.")); return box; }
  if (!candidates.length) { box.append(make("p", "No current-candidate observation is retained. Launch the current exact artifact to retain one.")); return box; }
  const form = make("form", undefined, "toolbar cr-toolbar");
  const select = (label, rows, chosen) => {
    const wrapper = make("label", label), field = make("select");
    for (const row of rows) { const option = make("option", `${row.current ? "Current" : "Historical"} · ${row.identity.slice(0, 12)} · ${String(row.observation?.binding?.revision || "").slice(0, 12)}`); option.value = row.identity; option.selected = row.identity === chosen; field.append(option); }
    wrapper.append(field); form.append(wrapper); return field;
  };
  const baseline = select("Baseline", baselines, requested?.baseline), candidate = select("Current candidate", candidates, requested?.candidate);
  const option = make("label", undefined, "cr-field cr-check"), structural = make("input"); structural.type = "checkbox"; structural.checked = context.structural === true;
  option.append(make("span", "Structural source evidence (opt-in)"), structural); form.append(option);
  const submit = make("button", "Compare"); submit.type = "submit"; form.append(submit);
  form.addEventListener("submit", event => { event.preventDefault(); context.select(`${baseline.value}~${candidate.value}`, structural.checked); });
  box.append(form);
  const result = make("div", undefined, "cr-preview-result"); result.setAttribute("aria-live", "polite"); box.append(result);
  if (!linked) return box;
  const pair = `${requested.baseline}~${requested.candidate}`;
  result.append(make("p", "Comparing exact observations…", "cr-muted"));
  context.compare(requested.baseline, requested.candidate, context.structural === true).then(response => {
    if (!acceptComparison(pair, context.selected(), response, context.isCurrent)) return;
    result.replaceChildren();
    result.append(make("p", `Comparison ${response.comparison.identity} · rules ${response.comparison.rules}`, "cr-principal"));
    const reasons = make("ul"); for (const line of explainComparison(response.comparison)) reasons.append(make("li", line)); result.append(reasons);
    result.append(make("p", `Exit status ${response.comparison.exit_status_equal ? "matches" : "differs"}; output ${response.comparison.output_equal ? "matches" : "differs"}. Equal output is not CI evidence.`));
    const lanes = make("div", undefined, "cr-preview-lanes");
    for (const [label, row, note] of [["Baseline", response.baseline, "Historical or baseline lane: launch unavailable."], ["Current candidate", response.candidate, "Launch the current candidate from Exact artifact previews."]]) {
      const lane = make("section", undefined, "cr-operation"); lane.append(make("h4", label));
      const facts = make("dl", undefined, "cr-facts"); for (const [key, value] of laneFacts(row)) { facts.append(make("dt", key), make("dd", value ?? "Not recorded", "cr-principal")); } lane.append(facts);
      const phases = make("ol"); for (const phase of Array.isArray(row?.observation?.phases) ? row.observation.phases : []) phases.append(make("li", `${phase.phase}: ${phase.outcome} (${phase.elapsed_ms} ms)${phase.detail ? ` · ${phase.detail}` : ""}`)); lane.append(phases);
      const output = make("pre", String(row?.observation?.result?.output || "").slice(0, MAX_LANE_OUTPUT)); output.setAttribute("aria-label", `${label} program output`); lane.append(output);
      lane.append(make("p", note, "cr-muted")); lanes.append(lane);
    }
    result.append(lanes); if (context.structural === true) renderStructural(result, response); title.focus();
  }).catch(error => {
    if (context.selected() !== pair || !context.isCurrent()) return;
    result.replaceChildren(make("p", `Comparison unavailable: ${String(error?.message || error).slice(0, 256)}. No substitute was selected; CI evidence is unchanged.`, "cr-notice"));
  });
  return box;
}
