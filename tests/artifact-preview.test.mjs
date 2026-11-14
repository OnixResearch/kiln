import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
import {test} from "node:test";
import {runInNewContext} from "node:vm";
const load=async name=>import(`data:text/javascript;base64,${Buffer.from(await readFile(new URL(`../crates/kiln-web-service/src/${name}`,import.meta.url))).toString("base64")}`);
const runtime=await load("preview-runtime.js"), review=await load("preview-review.js");
const root="/nix/store/00000000000000000000000000000000-cli";
function manifest() { return {identity:"a".repeat(64),descriptor:{version:1,platform:"x86_64-linux",root_path:root,executable:`${root}/bin/cli`,arguments:["literal arg"],issued_at:100,closure_complete:true,closure:[{path:root,nar_size:32,references:[]}],engine:{protocol:"trynix-startvm-v1",identity:"b".repeat(64),network_access:false,guest_memory_bytes:512*1024*1024},limits:{expires_at:200,runtime_bytes:1024,run_seconds:60,output_bytes:4096,retain_after_session:false}}}; }
function row() { return {descriptor_identity:"a".repeat(64),binding:{project:"p",repository:"r",revision:"c".repeat(40),run_id:"run"},engine_identity:"b".repeat(64),expires_at:200,launch_url:`http://127.0.0.1:12345/${"a".repeat(64)}/index.html`}; }
const context={project:"p",repository:"r",revision:"c".repeat(40),currentRevision:"c".repeat(40),runs:["run"]};

test("retained exact current preview is eligible, not a digest-inferred URL",()=> {
  assert.equal(review.eligiblePreview(row(),context,101000),true);
  const stale={...context,currentRevision:"d".repeat(40)};
  assert.equal(review.eligiblePreview(row(),stale,101000),false);
  assert.equal(review.eligiblePreview(row(),{...context,runs:[]},101000),false);
  assert.equal(review.eligiblePreview(row(),context,200000),false);
  const changed=row(); changed.binding.revision="d".repeat(40);
  assert.equal(review.eligiblePreview(changed,context,101000),false);
});
test("only exact disposable loopback capabilities can launch",()=> {
  assert.ok(review.safePreviewURL(row().launch_url));
  for(const url of ["https://trynix.dev/?pkg=hello",row().launch_url+"?candidate=new",row().launch_url.replace("127.0.0.1","example.com"),row().launch_url.replace("http://","http://password@"),"javascript:alert(1)"]) assert.equal(review.safePreviewURL(url),null);
});
test("exact runtime manifest remains distinct from engine-only qualification",()=> {
  const m=manifest(); assert.equal(runtime.validateManifest(m,101000),m.descriptor);
  m.descriptor.qualification="engine-only; no canonical CI binding";
  assert.equal(m.descriptor.binding,undefined);
  assert.equal(runtime.validateManifest(m,101000),m.descriptor);
});
test("missing member, partial closure, expiry and payload exhaustion reject",()=> {
  const m=manifest(); m.descriptor.closure[0].references=["/nix/store/missing"];
  assert.throws(()=>runtime.validateManifest(m,101000),/missing_member/);
  m.descriptor.closure[0].references=[]; m.descriptor.closure_complete=false;
  assert.throws(()=>runtime.validateManifest(m,101000),/partial_closure/);
  m.descriptor.closure_complete=true; m.descriptor.limits.runtime_bytes=31;
  assert.throws(()=>runtime.validateManifest(m,101000),/resource_exhausted/);
  m.descriptor.limits.runtime_bytes=1024;
  assert.throws(()=>runtime.validateManifest(m,200000),/expired/);
});
test("unavailable browser, wrong engine, network guest and Redox fail closed",()=> {
  assert.throws(()=>runtime.requireBrowser({}),/browser_engine_unavailable/);
  runtime.requireBrowser({crossOriginIsolated:true,SharedArrayBuffer,WebAssembly,crypto:{subtle:{}}});
  const m=manifest(); m.descriptor.engine.protocol="unavailable";
  assert.throws(()=>runtime.validateManifest(m,101000),/engine_mismatch/);
  m.descriptor.engine.protocol="trynix-startvm-v1"; m.descriptor.engine.network_access=true;
  assert.throws(()=>runtime.validateManifest(m,101000),/engine_capability_mismatch/);
  m.descriptor.engine.network_access=false; m.descriptor.platform="x86_64-redox";
  assert.throws(()=>runtime.validateManifest(m,101000),/engine_mismatch/);
});
test("entrypoint and arguments remain exact literals with an empty guest environment",()=> {
  const d=manifest().descriptor; d.arguments=["a'b","$(not-executed)"];
  assert.equal(runtime.exactCommand(d),`'env' '-i' 'PATH=/bin:/sbin' 'TERM=xterm-256color' 'LANG=C.UTF-8' '${root}/bin/cli' 'a'\\''b' '$(not-executed)'`);
  d.executable="/bin/cli"; assert.throws(()=>runtime.exactCommand(d),/entrypoint_mismatch/);
});

test("a new guest clears old evidence and ignores the previous frame's result", async () => {
  const nodes = new Map();
  const element = () => ({
    children: [], handlers: new Map(), contentWindow: {}, textContent: "", disabled: false,
    addEventListener(type, handler) { this.handlers.set(type, handler); },
    append(...children) { for (const child of children) { child.parent = this; this.children.push(child); } },
    replaceChildren() { this.children = []; },
    remove() { if (this.parent) this.parent.children = this.parent.children.filter(child => child !== this); },
  });
  for (const id of ["status", "start", "stop", "output", "phases", "session", "identity"]) nodes.set(id, element());
  const events = new Map();
  const window = { addEventListener(type, handler) { events.set(type, handler); } };
  const m = manifest(); m.descriptor.limits.expires_at = Math.floor(Date.now() / 1000) + 60;
  const origin = "http://127.0.0.1:12345";
  const source = await readFile(new URL("../crates/kiln-web-service/src/preview-shell.js", import.meta.url), "utf8");
  await runInNewContext(`(async () => { ${source}\n })()`, {
    window, location: { origin }, document: { getElementById: id => nodes.get(id), createElement: element },
    fetch: async () => ({ ok: true, json: async () => m }), setTimeout, clearTimeout,
  });
  const click = id => nodes.get(id).handlers.get("click")();
  const result = frame => events.get("message")({ source: frame.contentWindow, origin, data: { type: "result", preview_identity: m.identity, status: 0, output: "first result" } });
  try {
    click("start"); const first = nodes.get("session").children[0];
    result(first);
    assert.equal(window.kilnPreviewObservation.result.status, 0);
    assert.equal(nodes.get("session").children.length, 0);
    click("start");
    assert.equal(window.kilnPreviewObservation.result, null);
    assert.deepEqual(Array.from(window.kilnPreviewObservation.phases), []);
    result(first);
    assert.equal(window.kilnPreviewObservation.result, null);
    assert.equal(nodes.get("session").children.length, 1);
    click("stop");
    assert.equal(window.kilnPreviewObservation.result.error, "preview_cancelled");
    assert.equal(nodes.get("session").children.length, 0);
  } finally { click("stop"); }
});
