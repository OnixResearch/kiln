/* Review-only projection. Nothing here sends a CI/review/merge command. */
export function safePreviewURL(value) {
  try { const u=new URL(value); return u.protocol==="http:" && u.hostname==="127.0.0.1" && !!u.port && !u.username && !u.password && !u.search && !u.hash && /^\/[0-9a-f]{64}\/index\.html$/.test(u.pathname) ? u.href : null; } catch { return null; }
}
export function eligiblePreview(row, context, now=Date.now()) {
  const b=row?.binding;
  return !!b && b.project===context.project && b.repository===context.repository && b.revision===context.revision
    && context.currentRevision===context.revision && context.runs.includes(b.run_id)
    && /^[0-9a-f]{64}$/.test(row.descriptor_identity) && /^[0-9a-f]{64}$/.test(row.engine_identity)
    && Number.isSafeInteger(row.expires_at) && row.expires_at*1000>now && safePreviewURL(row.launch_url)!==null;
}
export function renderPreviews(parent, rows, context) {
  const make=(tag,text)=> { const el=document.createElement(tag); if(text!==undefined)el.textContent=text; return el; };
  const note=make("p","Preview observations never rewrite CI, review or merge evidence. Only opted-in current exact-source artifacts are launchable."); note.className="cr-muted"; parent.append(note);
  const entries=Array.isArray(rows)?rows.slice(0,16):[];
  if (!entries.length) { parent.append(make("p","No admitted runnable artifact preview is retained for this candidate. Artifact/check digests alone are not launch permission.")); return; }
  for(const row of entries) {
    const article=make("article"), facts=make("dl"); article.className="list-row cr-operation";
    const binding=row.binding||{};
    for(const [key,value]of [["Source",binding.revision],["Run / attempt",`${binding.run_id||"not recorded"} / ${binding.attempt_id||"not recorded"}`],["Artifact",binding.artifact_identity],["Executable",row.executable],["Runner",row.engine_identity],["Descriptor",row.descriptor_identity]]) {
      const dt=make("dt",key),dd=make("dd",value||"Not recorded"); dd.className="cr-principal"; facts.append(dt,dd);
    }
    article.append(facts);
    if(eligiblePreview(row,context)) {
      const a=make("a","Launch exact artifact in disposable Linux session"); a.href=safePreviewURL(row.launch_url); a.target="_blank"; a.rel="noopener noreferrer"; a.className="button quiet";
      // Recheck on click, not just on rendering, so retained expired DOM cannot
      // present a launch as current. The isolated consumer rechecks expiry too.
      a.addEventListener("click",event=> { if(!context.isCurrent()||!eligiblePreview(row,context)) { event.preventDefault(); a.replaceWith(make("p","Preview expired or review candidate moved; refresh exact evidence.")); } });
      article.append(a);
    } else article.append(make("p","Preview unavailable: expired, historical, mismatched, or not bound to canonical successful CI. No replacement artifact will be built."));
    parent.append(article);
  }
}
