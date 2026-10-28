use kiln_hosted::{bridge, service, transport};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};

const USAGE: &str = "usage: kiln-hosted keygen --key PATH\n       kiln-hosted check --profile SERVER.json\n       kiln-hosted serve --profile SERVER.json\n       kiln-hosted connect --profile CLIENT.json\n       kiln-hosted rpc --profile CLIENT.json --request REQUEST.json\n       kiln-hosted compile --input PIPELINE.json\n       kiln-hosted migrate --input WORKFLOW.yml\n       kiln-hosted backup --profile SERVER.json --id BACKUP_ID\n       kiln-hosted restore --profile SERVER.json --id BACKUP_ID --target NEW_ID --identity MANIFEST_BLAKE3";

fn options(args: impl Iterator<Item = String>) -> Result<BTreeMap<String, String>, String> {
    let mut args = args;
    let mut fields = BTreeMap::new();
    while let Some(key) = args.next() {
        if !key.starts_with("--") {
            return Err("expected named option".into());
        }
        let value = args
            .next()
            .filter(|v| !v.starts_with("--"))
            .ok_or("option value missing")?;
        if fields.insert(key, value).is_some() {
            return Err("duplicate option".into());
        }
    }
    Ok(fields)
}
fn take(fields: &mut BTreeMap<String, String>, key: &str) -> Result<String, String> {
    fields
        .remove(key)
        .ok_or_else(|| format!("required option {key}"))
}
fn complete(fields: &BTreeMap<String, String>) -> Result<(), String> {
    if fields.is_empty() {
        Ok(())
    } else {
        Err("unsupported option".into())
    }
}
fn read(path: &Path) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|_| "input unavailable")?;
    if !file
        .metadata()
        .map_err(|_| "input metadata unavailable")?
        .is_file()
    {
        return Err("regular input file required".into());
    }
    let mut bytes = Vec::new();
    file.take(kiln_hosted::MAX_FRAME_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "input read failed")?;
    if bytes.len() > kiln_hosted::MAX_FRAME_BYTES {
        return Err("input exceeds bound".into());
    }
    Ok(bytes)
}
async fn shutdown() {
    if let Ok(mut terminate) =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
    {
        tokio::select! {_=tokio::signal::ctrl_c()=>{},_=terminate.recv()=>{}}
    } else {
        let _signal = tokio::signal::ctrl_c().await;
    }
}
async fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        println!("{USAGE}");
        return Ok(());
    };
    if matches!(command.as_str(), "--help" | "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let mut fields = options(args)?;
    match command.as_str() {
        "keygen" => {
            let path = take(&mut fields, "--key")?;
            complete(&fields)?;
            println!("{}", transport::keygen(Path::new(&path))?);
        }
        "check" | "serve" => {
            let path = take(&mut fields, "--profile")?;
            complete(&fields)?;
            let profile = service::Profile::load(Path::new(&path))?;
            if command == "check" {
                println!(
                    "hosted profile structurally admitted; no listener, source effects or production qualification"
                );
                return Ok(());
            }
            let network = profile.network.clone();
            let service = Arc::new(Mutex::new(service::Service::open(profile)?));
            let endpoint = transport::bind(&network).await?;
            println!(
                "{}",
                serde_json::to_string(&endpoint.addr()).map_err(|_| "endpoint encoding failed")?
            );
            transport::serve(endpoint, service, shutdown()).await?;
        }
        "connect" | "rpc" => {
            let path = take(&mut fields, "--profile")?;
            let request_path = if command == "rpc" {
                Some(take(&mut fields, "--request")?)
            } else {
                None
            };
            complete(&fields)?;
            let profile = bridge::Profile::load(Path::new(&path))?;
            let endpoint = transport::bind(&profile.network).await?;
            if let Some(path) = request_path {
                let request: kiln_hosted::Request =
                    serde_json::from_slice(&read(Path::new(&path))?)
                        .map_err(|_| "request JSON rejected")?;
                if request.organization != profile.organization {
                    return Err("client organization mismatch".into());
                }
                let response = transport::call(&endpoint, &profile.remote, &request).await;
                endpoint.close().await;
                let response = response?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&response)
                        .map_err(|_| "response encoding failed")?
                );
                if response.error.is_some() {
                    return Err("customer operation rejected; see response".into());
                }
            } else {
                bridge::serve(profile, endpoint, shutdown()).await?;
            }
        }
        "compile" | "migrate" => {
            let path = take(&mut fields, "--input")?;
            complete(&fields)?;
            let bytes = read(Path::new(&path))?;
            let output = if command == "compile" {
                serde_json::to_value(kiln_hosted::pipelines::compile_value(
                    serde_json::from_slice(&bytes).map_err(|_| "pipeline JSON rejected")?,
                )?)
                .map_err(|_| "compiled output encoding failed")?
            } else {
                serde_json::to_value(kiln_hosted::pipelines::migrate(
                    std::str::from_utf8(&bytes).map_err(|_| "workflow must be UTF-8")?,
                )?)
                .map_err(|_| "migration encoding failed")?
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&output).map_err(|_| "output encoding failed")?
            );
        }
        "backup" | "restore" => {
            let path = take(&mut fields, "--profile")?;
            let id = take(&mut fields, "--id")?;
            let target = if command == "restore" {
                Some((
                    take(&mut fields, "--target")?,
                    take(&mut fields, "--identity")?,
                ))
            } else {
                None
            };
            complete(&fields)?;
            let profile = service::Profile::load(Path::new(&path))?;
            let operations = profile
                .operations
                .as_ref()
                .ok_or("operations profile required")?
                .clone();
            if let Some((target, identity)) = target {
                let restored =
                    kiln_hosted::operations::restore(&operations, &id, &target, &identity)?;
                println!("{}", restored.display());
            } else {
                let service = service::Service::open(profile)?;
                let manifest = kiln_hosted::operations::backup(
                    &service.db,
                    &operations,
                    &id,
                    kiln_hosted::now_ms()?,
                )?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &serde_json::json!({"identity":manifest.identity()?,"manifest":manifest})
                    )
                    .map_err(|_| "manifest encoding failed")?
                );
            }
        }
        _ => return Err(USAGE.into()),
    }
    Ok(())
}
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("kiln-hosted: {error}");
        std::process::exit(1);
    }
}
