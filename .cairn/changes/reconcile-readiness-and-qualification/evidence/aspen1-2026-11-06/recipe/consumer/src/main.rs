use anyhow::{Context, Result, anyhow, bail, ensure};
use iroh::{Endpoint, EndpointAddr};
use kiln_adapter_git::{GitSourceProfile, LocalGitAdapter, resolve_and_acquire};
use kiln_core::{source::GitSnapshotLimits, types::{ExternalRef, ProviderProfileId}};
use kiln_hosted::{Request, Response, SCHEMA, execution::Config, service::{Profile, Service}, transport::{self, NetworkProfile}};
use kiln_provider_api::SourceRepositoryPort;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs::{self, OpenOptions}, io::Write, os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt, symlink}, path::{Component, Path, PathBuf}, sync::{Arc, Mutex}, time::Duration};

const ORG: &str = "aspen-qualification";
const PROJECT: &str = "approved-untrusted";
const ATTEMPT: &str = "approved-untrusted-exact-1";
const BAD_ATTEMPT: &str = "invalid-provenance-1";
// Operator-reviewed bounds: the exact committed Kiln tree is ~188MB. A 256MiB
// native snapshot and 512MiB read-only source disk admit it without filtering
// source, and a 2GiB host unit bounds the guest plus source-image construction.
const SOURCE_BYTES: usize = 256 * 1024 * 1024;
const SOURCE_IMAGE_MIB: u64 = 512;
const GUEST_MEMORY_MIB: u64 = 1024;
const HOST_UNIT_MEMORY_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const RECOVERY_POLLS: usize = 60;

fn read_json(path: &Path) -> Result<Value> { Ok(serde_json::from_slice(&fs::read(path)?)?) }
fn record(path: &Path, value: &Value) -> Result<()> {
    let mut f = OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)?;
    f.write_all(&serde_json::to_vec_pretty(value)?)?;
    f.sync_all()?;
    Ok(())
}
fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    let m = fs::symlink_metadata(path)?;
    ensure!(m.is_dir() && !m.file_type().is_symlink() && m.uid() == fs::metadata("/proc/self")?.uid(), "directory must be owned and not a symlink: {}", path.display());
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}
fn text<'a>(v: &'a Value, p: &str) -> Result<&'a str> { v.pointer(p).and_then(Value::as_str).ok_or_else(|| anyhow!("missing text {p}")) }
fn require(v: &Value, p: &str, expected: Value) -> Result<()> { ensure!(v.pointer(p) == Some(&expected), "receipt invariant {p}: expected {expected}, observed {:?}", v.pointer(p)); Ok(()) }
fn immutable_tool(path: &Path) -> Result<Value> {
    let mut p = path.to_path_buf();
    for _ in 0..32 {
        if p.starts_with("/nix/store") {
            ensure!(p.is_file(), "missing immutable executable {}", p.display());
            return Ok(json!({"path":p,"blake3":blake3::hash(&fs::read(&p)?).to_hex().to_string()}));
        }
        let target = fs::read_link(&p).with_context(|| format!("{} is not an immutable Nix-store tool", p.display()))?;
        p = if target.is_absolute() { target } else { p.parent().context("tool parent")?.join(target) };
    }
    bail!("tool symlink depth exceeded")
}
fn tool(name: &str) -> Result<Value> {
    let path = std::env::var_os("PATH").context("PATH missing")?;
    let found = std::env::split_paths(&path).map(|p| p.join(name)).find(|p|p.is_file()).with_context(||format!("tool not found: {name}"))?;
    immutable_tool(&found)
}
fn network(key_file: PathBuf) -> NetworkProfile {
    NetworkProfile {key_file, bind_address: "127.0.0.1:0".parse().expect("literal socket"), relay_urls: Vec::new()}
}
fn take_frame<'a>(input: &mut &'a [u8]) -> Result<&'a [u8]> {
    ensure!(input.len() >= 8, "truncated native snapshot length");
    let n = usize::try_from(u64::from_be_bytes(input[..8].try_into()?))?;
    *input = &input[8..];
    ensure!(input.len() >= n, "truncated native snapshot data");
    let (value, rest) = input.split_at(n); *input = rest; Ok(value)
}
fn acquire_source(source: &Path, commit: &str, work: &Path, out: &Path) -> Result<()> {
    let id = ProviderProfileId::new("aspen-native-git").map_err(|e|anyhow!("{e:?}"))?;
    let profile = GitSourceProfile::pinned_local(id.clone(), GitSnapshotLimits {
        max_entries: 100_000, max_path_bytes: 4096, max_file_bytes: SOURCE_BYTES, max_total_bytes: SOURCE_BYTES,
    });
    let adapter = LocalGitAdapter::new(source.to_path_buf(), commit.to_owned(), profile.clone()).map_err(|e|anyhow!("native Git profile: {e:?}"))?;
    let resolution = resolve_and_acquire(&adapter, id).map_err(|e|anyhow!("native Git resolution: {e:?}"))?;
    ensure!(resolution.exact_revision.native_id.as_str() == commit, "native source changed exact commit");
    let exact = ExternalRef::new("git", "commit", commit).map_err(|e|anyhow!("{e:?}"))?;
    let manifest = adapter.acquire_canonical_manifest(&exact).map_err(|e|anyhow!("native snapshot: {e:?}"))?;
    record(&out.join("native-source.json"), &json!({"profile":profile,"resolution":resolution,"snapshot_blake3":blake3::hash(&manifest).to_hex().to_string(),"snapshot_bytes":manifest.len()}))?;
    let mut input = manifest.strip_prefix(b"kiln.git.snapshot.v1\0").context("native snapshot schema")?;
    private_dir(work)?;
    // Materialize the adapter's exact canonical bytes, never a mutable checkout,
    // Git hooks, shell archive extraction, credentials or a .git directory.
    while !input.is_empty() {
        let kind = input[0]; input = &input[1..];
        let relative = std::str::from_utf8(take_frame(&mut input)?)?;
        let bytes = take_frame(&mut input)?;
        ensure!(!Path::new(relative).is_absolute() && Path::new(relative).components().all(|c|matches!(c,Component::Normal(_))), "unsafe source path");
        let target = work.join(relative);
        let parent = target.parent().context("source parent")?;
        let mut cursor = work.to_path_buf();
        for component in parent.strip_prefix(work)?.components() {
            cursor.push(component);
            if cursor.exists() { ensure!(fs::symlink_metadata(&cursor)?.is_dir(), "source parent is not a directory"); }
            else { fs::create_dir(&cursor)?; }
        }
        match kind {
            b'f' | b'x' => {
                let mut file = OpenOptions::new().create_new(true).write(true).mode(if kind == b'x' {0o755} else {0o644}).open(target)?;
                file.write_all(bytes)?;
            }
            b'l' => { symlink(std::str::from_utf8(bytes)?, target)?; }
            _ => bail!("unsupported native snapshot kind"),
        }
    }
    Ok(())
}

struct Rpc { endpoint: Endpoint, remote: EndpointAddr, out: PathBuf }
impl Rpc {
    async fn response(&self, id: &str, action: &str, input: Value) -> Result<Response> {
        let request = Request {schema:SCHEMA.into(), organization:ORG.into(), request_id:id.into(), action:action.into(), input};
        record(&self.out.join(format!("{id}.request.json")), &serde_json::to_value(&request)?)?;
        match transport::call(&self.endpoint, &self.remote, &request).await {
            Ok(response) => {record(&self.out.join(format!("{id}.response.json")), &serde_json::to_value(&response)?)?; Ok(response)}
            Err(error) => {record(&self.out.join(format!("{id}.transport-error.json")), &json!({"error":error,"new_attempt_permitted":false}))?; bail!("{action}: {error}")}
        }
    }
    async fn call(&self, id: &str, action: &str, input: Value) -> Result<Value> {
        let r = self.response(id,action,input).await?;
        ensure!(r.error.is_none(), "{action} refused: {:?}",r.error);
        r.result.context("missing service result")
    }
    async fn settle(&self, prefix: &str) -> Result<Value> {
        for n in 0..RECOVERY_POLLS {
            let got = self.call(&format!("{prefix}-get-{n}"), "execution.get", json!({"project":PROJECT,"attempt_id":ATTEMPT})).await?;
            if let Some(result) = got.get("result").filter(|r|!r.is_null()) {return Ok(result.clone());}
            if !matches!(got.get("state").and_then(Value::as_str),Some("reserved"|"running"|"unknown")) {bail!("attempt terminated without a success receipt: {got}");}
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        let recovered = self.call(&format!("{prefix}-recover"), "execution.recover", json!({"project":PROJECT,"attempt_id":ATTEMPT})).await?;
        bail!("uncertain original attempt recovered, not rerun; qualification incomplete: {recovered}")
    }
}

fn verify_receipt(receipt: &Value, uid: u32, group: &str) -> Result<()> {
    require(receipt,"/state",json!("succeeded"))?;
    require(receipt,"/attempt_id",json!(ATTEMPT))?;
    require(receipt,"/cleaned",json!(true))?;
    require(receipt,"/hostile_isolation_claim",json!(false))?;
    let jobs = receipt.get("jobs").and_then(Value::as_array).context("jobs missing")?;
    ensure!(jobs.len() == 1,"unexpected jobs");
    let job = &jobs[0];
    require(job,"/state",json!("succeeded"))?;
    require(job,"/cache/outcome",json!("disabled_untrusted"))?;
    let vm = job.get("microvm").context("no actual microvm observation")?;
    for (p,v) in [
        ("/backend",json!("firecracker")),("/host_admission/decision",json!("admitted")),
        ("/host_admission/untrusted_capable",json!(true)),("/host_admission/hostile_isolation_claim",json!(false)),
        ("/host_admission/override_applied",json!(false)),("/cve_2026_98432",json!("fixed")),
        ("/smt_active",json!(false)),("/ksm_running",json!(false)),
        ("/confinement/ok",json!(true)),("/confinement/no_new_privs",json!(1)),
        ("/confinement/cap_eff",json!("0000000000000000")),("/confinement/seccomp",json!(2)),
        ("/cache_assignment/group",json!(group)),
    ] {require(vm,p,v)?;}
    require(vm,"/confinement/uid",json!(uid))?;
    ensure!(text(vm,"/host_kernel_package")?.starts_with("/nix/store/"),"host kernel must be immutable");
    ensure!(text(vm,"/cache_assignment/schemata")?.lines().any(|s|s.trim_start().starts_with("L3:")||s.trim_start().starts_with("L3CODE:")),"no actual CAT L3 schemata");
    ensure!(vm.pointer("/cache_assignment/threads").and_then(Value::as_array).is_some_and(|a|!a.is_empty()),"no assigned VMM threads");
    ensure!(vm.get("physical_host_identity").is_none(),"raw machine identity leaked");
    let steps = job.get("steps").and_then(Value::as_array).context("steps missing")?;
    ensure!(steps.len()==1,"unexpected step count");
    require(&steps[0],"/state",json!("succeeded"))?;
    require(&steps[0],"/exit_code",json!(0))?;
    require(&steps[0],"/cleaned",json!(true))?;
    ensure!(text(&steps[0],"/stdout")?.contains("KILN_APPROVED_UNTRUSTED_OK"),"guest positive result missing");
    Ok(())
}

async fn qualification(rpc: &Rpc, saved: &Value, recover: bool, uid: u32, group: &str, commit: &str, owner: &str) -> Result<()> {
    let info = rpc.call(if recover {"recovery-identity"} else {"identity"},"system.info",json!({})).await?;
    require(&info,"/peer",json!(owner))?; require(&info,"/role",json!("owner"))?;
    let approver = format!("iroh:{owner}");
    // Provenance belongs to the exact durable request, not the receipt schema.
    require(saved,"/untrusted/source_commit",json!(commit))?;
    require(saved,"/untrusted/approver_id",json!(approver))?;
    if !recover {
        let mut bad = saved.clone();
        bad["attempt_id"] = json!(BAD_ATTEMPT);
        bad["untrusted"]["source_commit"] = json!("HEAD");
        let refused = rpc.response("negative-invalid-provenance","execution.run",bad).await?;
        ensure!(refused.error.is_some(),"invalid exact-source provenance was not refused");
        ensure!(serde_json::to_string(&refused.error)?.contains("untrusted run provenance invalid"),"unexpected refusal; not the intended negative");
        let listed = rpc.call("negative-effects","execution.list",json!({"project":PROJECT})).await?;
        let attempts = listed.get("attempts").and_then(Value::as_array).context("attempt list")?;
        ensure!(!attempts.iter().any(|a|a.get("attempt_id")==Some(&json!(BAD_ATTEMPT))),"negative reserved an attempt");
        // A timeout does not grant permission to create a different attempt.
        // All subsequent reads/recovery concern this exact persisted identity.
        let _outcome = rpc.response("positive-exact-attempt","execution.run",saved.clone()).await;
    }
    let receipt = rpc.settle(if recover {"recovery"} else {"positive"}).await?;
    record(&rpc.out.join(if recover {"recovered-receipt.json"} else {"receipt.json"}),&receipt)?;
    verify_receipt(&receipt,uid,group)?;
    record(&rpc.out.join(if recover {"recovery-verdict.json"} else {"verdict.json"}),&json!({
        "actual_hosted_operator_untrusted_execution":"passed", "web_exact_commit_approval_exercised":false,
        "approval_boundary":"operator-authorized authenticated hosted execution.run; provenance supplied by this consumer, not verified against a Web approval by hosted",
        "missing_combined_web_approval_prerequisite":"real production Web run blocked on this exact source commit, an authenticated interactive operator approval, and supported exact-provenance handoff into this hosted deployment",
        "hostile_isolation_claim":false,"attempt_id":ATTEMPT,"source_commit":commit,"authenticated_peer":owner,
        "new_attempt_created_during_recovery":false
    }))?;
    Ok(())
}

#[tokio::main(flavor="multi_thread",worker_threads=4)]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().context("usage: consumer run|recover --guest DIR --state DIR --source DIR --output DIR --cat GROUP --commit EXACT40 --operator-label LABEL")?;
    ensure!(mode=="run"||mode=="recover"||mode=="resume-bootstrap","mode must be run, recover, or resume-bootstrap");
    let resume_bootstrap=mode=="resume-bootstrap";
    let mut options = BTreeMap::new();
    while let Some(k)=args.next(){ensure!(k.starts_with("--"),"expected named argument"); let v=args.next().context("missing option value")?;ensure!(options.insert(k,v).is_none(),"duplicate option");}
    let opt = |k: &str| -> Result<&str> {options.get(k).map(String::as_str).with_context(||format!("missing {k}"))};
    ensure!(options.len()==7,"expected exactly seven named options");
    let guest_dir=fs::canonicalize(opt("--guest")?)?;
    let source=fs::canonicalize(opt("--source")?)?;
    let state=PathBuf::from(opt("--state")?);
    let out=PathBuf::from(opt("--output")?);
    ensure!(state.is_absolute()&&out.is_absolute(),"absolute state/output roots required");
    ensure!(state != out && !state.starts_with(&out) && !out.starts_with(&state),"state/output roots must be disjoint");
    let group=opt("--cat")?;
    ensure!(!group.is_empty()&&group.len()<=128&&group.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_'),"invalid CAT group");
    let commit=opt("--commit")?;
    ensure!(commit.len()==40&&commit.bytes().all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase()),"full lower-case SHA1 required");
    let label=opt("--operator-label")?;
    ensure!(!label.is_empty()&&label.len()<=256,"operator identity label required");
    let uid=fs::metadata("/proc/self")?.uid(); ensure!(uid!=0,"qualification must run as dedicated unprivileged user");
    let runtime=PathBuf::from(format!("/run/user/{uid}"));
    ensure!(runtime.join("systemd").is_dir(),"systemd user manager required");
    let guest=read_json(&guest_dir.join("manifest.json"))?;
    for key in ["firecracker","kernel","initrd","guest_shell"]{ensure!(guest.get(key).is_some(),"missing manifest {key}");}
    private_dir(&out)?; private_dir(&state)?;
    let binding=json!({"guest_manifest":guest,"guest_directory":guest_dir,"source":source,"state":state,"output":out,"cat_group":group,"source_commit":commit,"operator_label":label,"uid":uid});
    let recover=mode=="recover";
    let (profile,saved,owner) = if recover {
        ensure!(read_json(&out.join("binding.json"))?==binding,"recovery inputs differ from original exact execution");
        let p: Profile=serde_json::from_value(read_json(&state.join("profile.json"))?)?;
        let owner=p.bootstrap_peer.clone();
        (p,read_json(&state.join("attempt.json"))?,owner)
    } else {
        ensure!(!state.join("profile.json").exists()&&!state.join("attempt.json").exists(),"attempt initialization already completed; use recover with identical inputs");
        let owner=if resume_bootstrap {
            ensure!(read_json(&out.join("binding.json"))?==binding,"bootstrap recovery inputs differ");
            ensure!(state.join("server.key").is_file(),"original server key missing");
            let endpoint=transport::bind(&network(state.join("operator.key"))).await.map_err(|e|anyhow!(e))?;
            let owner=endpoint.id().to_string(); endpoint.close().await; owner
        } else {
            ensure!(!out.join("binding.json").exists(),"bootstrap already started; retain state");
            record(&out.join("binding.json"),&binding)?;
            let owner=transport::keygen(&state.join("operator.key")).map_err(|e|anyhow!(e))?;
            transport::keygen(&state.join("server.key")).map_err(|e|anyhow!(e))?; owner
        };
        let mut profile: Profile=serde_json::from_value(json!({
            "schema":SCHEMA,"service_id":"aspen1-approved-qualification","state_directory":state.join("service"),
            "network":network(state.join("server.key")),"bootstrap_peer":owner,"bootstrap_organization":ORG,"operator_peers":[owner],
            "source_profiles":[],"operations":null,"secrets":null,"execution":null,"fleet":null
        }))?;
        let mut service=Service::open(profile.clone()).map_err(|e|anyhow!(e))?;
        // Bootstrap is local administrative configuration; execution below uses
        // genuine QUIC peer authentication, never Service::handle impersonation.
        let setup=|service: &mut Service,id:&str,action:&str,input:Value|->Result<Value>{
            let r=service.handle(&owner,Request{schema:SCHEMA.into(),organization:ORG.into(),request_id:id.into(),action:action.into(),input});
            record(&out.join(format!("bootstrap-{id}.json")),&serde_json::to_value(&r)?)?;
            ensure!(r.error.is_none(),"bootstrap {action}: {:?}",r.error);r.result.context("bootstrap result")
        };
        if !resume_bootstrap {setup(&mut service,"account","finance.account.configure",json!({"request_id":"qualification-account","expected_revision":0,"currency":"usd","cap_minor":1000,"request_max_minor":1000,"max_concurrent":1}))?;}
        let price=if resume_bootstrap {read_json(&out.join("bootstrap-price.json"))?.get("result").cloned().context("original price missing")?} else {setup(&mut service,"price","finance.price.admit",json!({"class":"linux","currency":"usd","unit_minor":1,"unit_ms":1000,"max_units":120}))?};
        let pipeline=json!({"schema":"kiln.hosted.pipeline.v1","name":"Approved untrusted qualification","triggers":["manual"],"jobs":{"qualify":{"runner":"linux","timeout_ms":60000,"steps":[{"kind":"run","name":"real-source-no-credentials","run":"set -euo pipefail; [[ $(id -u) == 1000 ]]; [[ -z ${KILN_WORKLOAD_TOKEN+x} ]]; [[ ! -e /work/.git ]]; [[ -f /work/Cargo.toml ]]; [[ -f /work/crates/kiln-hosted/src/execution.rs ]]; [[ ! -e /root/.ssh/id_ed25519 ]]; [[ ! -e /root/.aws/credentials ]]; [[ ! -e /work/operator.key ]]; [[ ! -e /work/server.key ]]; [[ $(ls /sys/class/net) == lo ]]; printf 'KILN_APPROVED_UNTRUSTED_OK source=exact-native-git workload_token=absent network=none guest_uid=%s\\n' \"$(id -u)\""}]}}});
        if resume_bootstrap {ensure!(read_json(&out.join("reviewed-pipeline.json"))?==pipeline,"original reviewed pipeline differs");} else {record(&out.join("reviewed-pipeline.json"),&pipeline)?;}
        let plan=if resume_bootstrap {read_json(&out.join("bootstrap-review.json"))?.get("result").cloned().context("original review missing")?} else {setup(&mut service,"review","pipelines.review",json!({"project":PROJECT,"name":"approved-untrusted","expected_revision":0,"config":pipeline}))?};
        drop(service);
        let work=state.join("work"); private_dir(&work)?;
        let checkout=work.join(ORG).join(PROJECT).join("exact");
        acquire_source(&source,commit,&checkout,&out)?;
        let config: Config=serde_json::from_value(json!({
            "systemd_run":tool("systemd-run")?,"systemctl":tool("systemctl")?,"env":tool("env")?,"manager_runtime_directory":runtime,"work_root":work,
            "trusted_projects":[{"organization":ORG,"project":PROJECT}],
            "pricing":[{"organization":ORG,"runner":"linux","price_id":text(&price,"/price_id")?,"class":"linux","max_units":120}],
            "runners":{"linux":{"kind":"firecracker","firecracker":guest["firecracker"],"kernel":guest["kernel"],"initrd":guest["initrd"],"guest_shell":guest["guest_shell"],"version":guest["vmm"]["version"],"vcpus":1,"memory_mib":GUEST_MEMORY_MIB,"boot_timeout_ms":20000,"source_image_mib":SOURCE_IMAGE_MIB,"warm_pool":null,"qualification_identity":null,"untrusted_capable":true,"trust_domain":"qualification-untrusted-only","host_identity":"aspen1-operator-declared","cache_partition":group,"network":"none"}},
            "maximum_concurrent":1,"memory_bytes":HOST_UNIT_MEMORY_BYTES,"processes":64,"cpu_percent":200,"maximum_timeout_ms":120000,"maximum_output_bytes":262144
        }))?;
        profile.execution=Some(config);
        record(&state.join("profile.json"),&serde_json::to_value(&profile)?)?;
        let approver=format!("iroh:{owner}");
        let saved=json!({"project":PROJECT,"pipeline_id":text(&plan,"/identity")?,"attempt_id":ATTEMPT,"price_id":text(&price,"/price_id")?,"class":"linux","workspace":"exact","trust":"trusted","untrusted":{"reason":"fork","source_commit":commit,"approver_id":approver}});
        record(&state.join("attempt.json"),&saved)?;
        record(&out.join("operator-authorization.json"),&json!({"authority":"explicit user-authorized aspen1 maintenance and qualification in parent session","operator_label":label,"authenticated_operator_peer":owner,"source_commit":commit,"attempt":ATTEMPT,"untrusted_reason":"fork is the hosted untrusted policy lane selected for this qualification, not a claim of a forge webhook","web_approval_exercised":false,"hosted_approval_verified":false,"approval_entrypoint":"none; hosted execution.run trusts the authenticated caller's provenance assertion","credentials_configured":false,"workload_policies_configured":false,"cache_configured":false,"private_key_files_outside_source":true}))?;
        (profile,saved,owner)
    };
    let service=Arc::new(Mutex::new(Service::open(profile.clone()).map_err(|e|anyhow!(e))?));
    let server=transport::bind(&profile.network).await.map_err(|e|anyhow!(e))?;
    let sockets=server.bound_sockets();
    ensure!(sockets.iter().all(|s|s.ip().is_loopback()),"non-loopback server refused");
    let socket=*sockets.iter().find(|s|s.is_ipv4()).context("loopback server socket")?;
    let remote=EndpointAddr::new(server.id()).with_ip_addr(socket);
    let endpoint=transport::bind(&network(state.join("operator.key"))).await.map_err(|e|anyhow!(e))?;
    ensure!(endpoint.id().to_string()==owner,"operator key identity differs");
    let rpc_out = if recover {
        let root=out.join("recoveries"); private_dir(&root)?;
        let mut selected=None;
        for index in 1..=1000 {
            let candidate=root.join(format!("recovery-{index}"));
            match fs::create_dir(&candidate) {
                Ok(()) => {private_dir(&candidate)?;selected=Some(candidate);break;}
                Err(error) if error.kind()==std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        selected.context("recovery evidence directory bound reached")?
    } else {out.clone()};
    let rpc=Rpc{endpoint,remote,out:rpc_out};
    let (shutdown,rx)=tokio::sync::oneshot::channel();
    let serving=tokio::spawn(transport::serve(server,service,async{let _=rx.await;}));
    let result=qualification(&rpc,&saved,recover,uid,group,commit,&owner).await;
    let _=shutdown.send(());
    rpc.endpoint.close().await;
    let served=serving.await.context("service join failed")?.map_err(|e|anyhow!(e));
    served?;
    if let Err(error)=&result {
        let failure=rpc.out.join(if recover {"recovery-failure.json"} else {"failure.json"});
        if !failure.exists(){record(&failure,&json!({"error":format!("{error:#}"),"attempt_id":ATTEMPT,"retry_new_attempt":false,"instruction":"retain state; use recover with identical inputs; parent owns final watchdog and cleanup"}))?;}
    }
    result?;
    println!("actual hosted operator-authorized untrusted execution passed; Web approval NOT exercised; evidence={}",out.display());
    Ok(())
}
