"use strict";
const $ = id => document.getElementById(id);
const uuid = () => crypto.randomUUID();
let session;
const pages = {
  overview: {title:"Overview",description:"Your authenticated connection, organization and current authority.",ops:[["system.info",{}]]},
  projects:{title:"Projects & connections",description:"Connect an existing remote repository. No source migration or repository hosting.",ops:[["connections.list",{project:"demo"}],["connections.profiles",{project:"demo"}],["connections.connect",{project:"demo",connection:"github",repository_id:1,owner:"your-owner",name:"your-repository",profile_blake3:"",expected_revision:0}],["connections.status",{project:"demo",connection:"github",generation:1}],["connections.tick",{project:"demo",connection:"github",generation:1}],["connections.disconnect",{project:"demo",connection:"github",expected_revision:1}]]},
  pipelines:{title:"Pipelines",description:"Compile, review and migrate repository-owned pipelines. Unsupported migration semantics fail explicitly.",ops:[["pipelines.examples",{}],["pipelines.list",{project:"demo"}],["pipelines.compile",{schema:"kiln.hosted.pipeline.v1",name:"Checks",triggers:["manual"],jobs:{check:{runner:"linux-reviewed",steps:[{kind:"run",run:"printf 'hello\\n'",shell:"bash"}]}}}],["pipelines.review",{project:"demo",name:"checks",expected_revision:0,config:{schema:"kiln.hosted.pipeline.v1",name:"Checks",triggers:["manual"],jobs:{check:{runner:"linux-reviewed",steps:[{kind:"run",run:"printf 'hello\\n'",shell:"bash"}]}}}}],["pipelines.migrate",{yaml:"name: Checks\non: [push]\njobs:\n  check:\n    runs-on: linux-reviewed\n    steps:\n      - run: printf 'hello\\n'\n        shell: bash\n"}]]},
  runs:{title:"Runs & capacity",description:"Reviewed execution, exact source bindings and bounded fleet operations. Unknown effects are not safe retries.",ops:[
    ["execution.list",{project:"demo"}],
    ["execution.get",{project:"demo",attempt_id:""}],
    ["execution.run",{project:"demo",pipeline_id:"",attempt_id:uuid(),price_id:"",class:"linux-reviewed",workspace:"snapshot",trust:"trusted"}],
    ["execution.cancel",{project:"demo",attempt_id:""}],
    ["execution.recover",{project:"demo",attempt_id:""}],
    ["fleet.status",{}],
    ["fleet.configure",{expected_revision:0,minimum:0,maximum:1,slots_per_instance:1}],
    ["fleet.provision",{instance_id:"worker-1",request_id:uuid(),expected_revision:0}],
    ["fleet.deprovision",{instance_id:"worker-1",request_id:uuid(),expected_revision:1}],
    ["fleet.reconcile",{instance_id:"worker-1",request_id:uuid(),expected_revision:1}],
    ["connections.status",{project:"demo",connection:"github",generation:1}]
  ]},
  usage:{title:"Usage & invoices",description:"Integer prices, reserved exposure and auditable usage. Execution, invoice and payment outcomes remain separate.",ops:[["finance.account.get",{}],["finance.usage.list",{}],["finance.account.configure",{request_id:uuid(),expected_revision:0,cap_minor:10000,request_max_minor:1000,max_concurrent:2,currency:"usd"}],["finance.price.admit",{class:"linux-reviewed",currency:"usd",unit_minor:1,unit_ms:1000,max_units:300}],["finance.invoice.draft",{request_id:uuid()}],["finance.invoice.get",{invoice_id:""}],["finance.invoice.export",{invoice_id:""}],["finance.reconcile",{}]]},
  team:{title:"Team & access",description:"Invite an exact Iroh peer. Roles and project scopes are checked against current durable membership.",ops:[["accounts.list",{}],["accounts.invite",{peer:"",role:"member",projects:["demo"],expires_at_ms:Date.now()+3600000,nonce:uuid()}],["accounts.set_membership",{peer:"",expected_revision:1,role:"viewer",projects:["demo"],active:false}],["accounts.revoke_invitation",{invitation:""}],["accounts.accept",{invitation:""}]]},
  secrets:{title:"Secrets & identity",description:"Manage scoped secret references. Secret values and job capabilities are never returned through this administration surface.",ops:[["secrets.list",{project:"demo"}],["secrets.discovery",{}],["secrets.jwks",{}],["secrets.register",{project:"demo",name:"deploy",resolver:"reviewed-resolver",expected_revision:0,expires_ms:Date.now()+86400000}],["secrets.revoke",{project:"demo",name:"deploy",expected_revision:1}]]},
  operations:{title:"Operations",description:"Incident admission holds, retained support observations and content retention separate from financial/replay evidence.",ops:[["operations.health",{}],["operations.hold.get",{}],["operations.audit.list",{after:0,limit:100}],["operations.hold.set",{held:true,reason:"Operator incident hold",expected_revision:0}],["operations.support.record",{subject:"incident",observation:"Operator observation"}],["operations.backup",{backup_id:"operator-snapshot"}],["operations.retention.sweep",{limit:100}]]}
};
function renderRecords(value){
  $("records").replaceChildren();
  const rows=Array.isArray(value)?value:Object.values(value||{}).find(Array.isArray);
  if(!rows?.length||typeof rows[0]!=="object"||rows[0]===null)return;
  const keys=[...new Set(rows.slice(0,50).flatMap(Object.keys))].slice(0,7);
  const table=document.createElement("table"),head=document.createElement("thead"),tr=document.createElement("tr");
  keys.forEach(key=>{const th=document.createElement("th");th.textContent=key;tr.append(th);});head.append(tr);table.append(head);
  const body=document.createElement("tbody");rows.slice(0,50).forEach(row=>{const tr=document.createElement("tr");keys.forEach(key=>{const td=document.createElement("td");td.textContent=typeof row[key]==="object"?JSON.stringify(row[key]):String(row[key]??"—");tr.append(td);});body.append(tr);});table.append(body);$("records").append(table);
}
function newIdentity(){$("request-id").value=uuid();}
function chooseOperation(){const page=pages[location.hash.slice(1)]||pages.overview;const operation=page.ops.find(([action])=>action===$("operation").value);$("input").value=JSON.stringify(operation?.[1]||{},null,2);$("operation-title").textContent=$("operation").value;newIdentity();}
function navigate(){
  const key=Object.hasOwn(pages,location.hash.slice(1))?location.hash.slice(1):"overview",page=pages[key];
  document.querySelectorAll("nav a").forEach(link=>{if(link.hash==="#"+key)link.setAttribute("aria-current","page");else link.removeAttribute("aria-current");});
  $("page-title").textContent=page.title;$("page-description").textContent=page.description;
  $("operation").replaceChildren(...page.ops.map(([action])=>{const option=document.createElement("option");option.value=action;option.textContent=action;return option;}));chooseOperation();
}
async function request(event){
  event?.preventDefault();if(!session)return;
  let input;try{input=JSON.parse($("input").value);}catch{$("status").textContent="Invalid JSON input";$("status").className="error";return;}
  const action=$("operation").value;
  const mutation=!/(\.(info|list|status|get|examples|compile|migrate|discovery|jwks|health|export|reconcile|profiles)$)/.test(action)||action==="fleet.reconcile";
  if(mutation&&!confirm(`Send ${action} to organization ${session.organization}?\n\n${JSON.stringify(input,null,2)}`))return;
  const envelope={schema:"kiln.hosted.v1",organization:session.organization,request_id:$("request-id").value,action,input};
  $("send").disabled=true;$("status").className="";$("status").textContent="Waiting for authenticated Iroh response…";
  try{
    const response=await fetch("/rpc",{method:"POST",headers:{"Content-Type":"application/json","X-Kiln-Local-Client":"1"},body:JSON.stringify(envelope)});
    const result=await response.json();if(!response.ok||result.error)throw new Error(result.error||`HTTP ${response.status}`);
    $("result").textContent=JSON.stringify(result.result,null,2);renderRecords(result.result);$("status").textContent="Recorded response · "+envelope.request_id;$("status").className="success";
    $("connection-state").textContent="Iroh connected";
    if(action==="system.info"){$("role").textContent=result.result.role;}
  }catch(error){$("result").textContent=error.message;$("records").replaceChildren();$("status").textContent="Operation rejected or outcome unavailable · retain request identity";$("status").className="error";}
  finally{$("send").disabled=false;}
}
$("request-form").addEventListener("submit",request);$("operation").addEventListener("change",chooseOperation);$("new-request").addEventListener("click",newIdentity);window.addEventListener("hashchange",navigate);
(async()=>{
  navigate();
  try{
    const response=await fetch("/session");if(!response.ok)throw new Error("Local session unavailable");session=await response.json();
    $("organization").textContent=session.organization.toUpperCase();
    for(const [label,key]of[["Client peer","peer"],["Server peer","server"]]){const p=document.createElement("span"),strong=document.createElement("strong");strong.textContent=label;p.append(strong,document.createTextNode(session[key]));$("identity").append(p);}
    const identityResponse=await fetch("/rpc",{method:"POST",headers:{"Content-Type":"application/json","X-Kiln-Local-Client":"1"},body:JSON.stringify({schema:"kiln.hosted.v1",organization:session.organization,request_id:uuid(),action:"system.info",input:{}})});
    const identity=await identityResponse.json();if(!identityResponse.ok||identity.error)throw new Error(identity.error||"Current identity unavailable");
    $("role").textContent=identity.result.role;$("connection-state").textContent="Iroh connected";
    await request();
  }catch(error){$("connection-state").textContent="Connection unavailable";$("result").textContent=error.message;}
})();
