// Adapter for the existing pinned TryNix startVM/share/guestDriver protocol.
// It never imports the package picker, walks a cache, discovers bin siblings,
// realises a flake, tolerates an unsigned closure or contacts an external site.
export function shellQuote(value) { return `'${String(value).replaceAll("'", "'\\''")}'`; }
export function exactCommand(descriptor) {
  if (!descriptor.executable.startsWith(`${descriptor.root_path}/bin/`) || descriptor.executable.includes("/../")) throw new Error("preview_entrypoint_mismatch");
  return ["env", "-i", "PATH=/bin:/sbin", "TERM=xterm-256color", "LANG=C.UTF-8", descriptor.executable, ...descriptor.arguments].map(shellQuote).join(" ");
}
export function requireBrowser(scope = globalThis) {
  if (!scope.crossOriginIsolated || typeof scope.SharedArrayBuffer !== "function" || typeof scope.WebAssembly?.instantiate !== "function" || !scope.crypto?.subtle) throw new Error("preview_browser_engine_unavailable: cross-origin isolation, WebAssembly threads and WebCrypto are required");
}
export function validateManifest(manifest, now = Date.now()) {
  const d = manifest?.descriptor;
  if (!d || d.version !== 1 || d.engine.protocol !== "trynix-startvm-v1" || d.platform !== "x86_64-linux" || d.engine.identity !== manifest.engine_identity && manifest.engine_identity !== undefined) throw new Error("preview_engine_mismatch");
  if (!d.closure_complete || !Array.isArray(d.closure) || !d.closure.length || d.closure.length > 128) throw new Error("preview_partial_closure");
  if (d.limits.expires_at * 1000 <= now || d.issued_at * 1000 > now) throw new Error("preview_expired");
  if (d.engine.network_access || d.limits.retain_after_session || d.engine.guest_memory_bytes !== 512 * 1024 * 1024) throw new Error("preview_engine_capability_mismatch");
  const members = new Map(d.closure.map(m => [m.path, m]));
  if (members.size !== d.closure.length || !members.has(d.root_path) || d.closure.some(m => m.references.some(r => !members.has(r)))) throw new Error("preview_missing_member");
  const total = d.closure.reduce((sum,m) => sum + m.nar_size,0);
  if (!Number.isSafeInteger(total) || total > d.limits.runtime_bytes || total > 128 * 1024 * 1024) throw new Error("preview_resource_exhausted");
  exactCommand(d); return d;
}
const hex = buffer => [...new Uint8Array(buffer)].map(b => b.toString(16).padStart(2,"0")).join("");
async function readExact(url, size, sha, signal) {
  const response = await fetch(url,{credentials:"omit",cache:"no-store",signal});
  if (!response.ok) throw new Error(`preview_member_unavailable: ${url} (${response.status})`);
  const length = response.headers.get("Content-Length");
  if (length !== null && Number(length) !== size) throw new Error("preview_member_size_mismatch");
  const reader = response.body.getReader(), bytes = new Uint8Array(size);
  let offset=0;
  try { for (;;) { const {value,done}=await reader.read(); if (done) break;
    if (offset+value.byteLength>size) throw new Error("preview_resource_exhausted"); bytes.set(value,offset); offset+=value.byteLength;
  }} finally { await reader.cancel(); }
  if (offset !== size || hex(await crypto.subtle.digest("SHA-256",bytes)) !== sha) throw new Error("preview_member_hash_mismatch");
  return bytes;
}
async function launch() {
  const controller = new AbortController();
  let identity="", d=null;
  const post = data => parent.postMessage({...data,preview_identity:identity},location.origin);
  const phase = async (name,fn) => { const at=performance.now();
    try { const value=await fn(); post({type:"phase",phase:name,outcome:"ready",elapsed_ms:performance.now()-at}); return value; }
    catch (error) { post({type:"phase",phase:name,outcome:"failed",elapsed_ms:performance.now()-at,detail:error.message}); throw error; }
  };
  let timer;
  try {
    const manifest = await phase("artifact discovery",async()=> {
      const response=await fetch(new URL("../descriptor.json",location.href),{credentials:"omit",cache:"no-store",signal:controller.signal});
      if (!response.ok) throw new Error("preview_descriptor_unavailable");
      const m=await response.json(); identity=m.identity; d=validateManifest(m); requireBrowser(); return m;
    });
    timer=setTimeout(()=>controller.abort(new Error("preview_resource_exhausted")),d.limits.run_seconds*1000);
    const root = new URL("./",location.href);
    const files = new Map(manifest.engine_manifest.files.map(f=>[f.path,f]));
    const asset = async name => {
      const f=files.get(name); if (!f) throw new Error(`preview_engine_member_missing: ${name}`);
      return readExact(new URL(name,root),f.bytes,f.sha256,controller.signal);
    };
    const {startVM} = await import(new URL(`${manifest.engine_manifest.module_dir}/boot.js`,root));
    const {parseNar} = await import(new URL(`${manifest.engine_manifest.module_dir}/nar.js`,root));
    const vm = await phase("payload hydration",async()=> {
      const guestFiles=new Map();
      for (const [name,path] of Object.entries(manifest.guest_files)) guestFiles.set(name,await asset(path));
      const machine=JSON.parse(new TextDecoder().decode(await asset("guest/machine.json")));
      const snapshot=files.has("qemu/vm.state") ? await asset("qemu/vm.state") : null;
      const handle = await startVM({guestFiles,machine,snapshot,terminalElement:document.getElementById("terminal"),keyBarElement:document.getElementById("keybar"),
        engine:{main:"qemu/out.js",locate:file=>new URL(`qemu/${file}`,root).href}});
      for (let i=0;i<d.closure.length;i++) {
        const member=d.closure[i];
        const bytes=await readExact(new URL(`../nar/${i}`,root),member.nar_size,member.nar_sha256,controller.signal);
        const entries=parseNar(bytes);
        // Runtime NAR names may not escape the exact store member through
        // malformed node names. Symlinks remain inside the disposable guest.
        if (entries.some(e=>e.path.startsWith("/") || e.path.split("/").includes(".."))) throw new Error("preview_nar_path_escape");
        handle.share.write(member.path.slice("/nix/store/".length),entries);
      }
      return handle;
    });
    await phase("guest boot",async()=> {
      await vm.run([d.root_path.slice("/nix/store/".length)]);
      if (!window.trynix?.transcript().includes("trynix: welcome to the multiverse")) throw new Error("preview_guest_readiness_unobserved");
    });
    let outputBytes=0, ended=false;
    window.trynix.master.onWrite(([bytes])=> {
      outputBytes+=typeof bytes === "string" ? new TextEncoder().encode(bytes).byteLength : bytes.byteLength;
      if (outputBytes>d.limits.output_bytes+4096 && !ended) { ended=true; post({type:"result",status:null,error:"preview_output_exhausted",output:""}); }
    });
    const result=await phase("program readiness",async()=> {
      const result=await window.trynix.run(exactCommand(d),{timeoutMs:d.limits.run_seconds*1000});
      if (result.timedOut) throw new Error("preview_program_timeout");
      if (new TextEncoder().encode(result.output).byteLength>d.limits.output_bytes) throw new Error("preview_output_exhausted");
      return result;
    });
    if (!ended) post({type:"result",status:result.status,output:result.output,error:result.status===0?null:"preview_program_failed"});
  } catch (error) {
    post({type:"result",status:null,error:error instanceof RangeError ? "preview_memory_exhausted" : error.message,output:""});
  } finally { clearTimeout(timer); controller.abort(); }
}
if (typeof window !== "undefined" && window.parent !== window) launch();
