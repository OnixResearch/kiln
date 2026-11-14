//! Local exact-artifact consumer. No command builds or signs a product.
use kiln_core::preview::PreviewBinding;
use kiln_web_service::preview::{
    engine::{self, EngineBundle, EngineManifest},
    nix::NativeNix,
    read_request,
    service::PreviewServer,
};
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

fn json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    kiln_web_service::preview::read_json(path, kiln_web_service::preview::MAX_REQUEST_BYTES)
}
fn native_nix(nix: &str, nix_store: &str) -> Result<NativeNix, String> {
    let native = NativeNix::new(PathBuf::from(nix), PathBuf::from(nix_store))?;
    match env::var_os("KILN_PREVIEW_STORE_ROOT") {
        Some(root) => native.with_store_root(Path::new(&root)),
        None => Ok(native),
    }
}
fn serve(server: PreviewServer) -> Result<(), String> {
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let interrupt = signal_hook::flag::register(signal_hook::consts::SIGINT, stop.clone())
        .map_err(|e| e.to_string())?;
    let terminate = signal_hook::flag::register(signal_hook::consts::SIGTERM, stop.clone())
        .map_err(|e| e.to_string())?;
    let result = server.serve_until(&stop);
    signal_hook::low_level::unregister(interrupt);
    signal_hook::low_level::unregister(terminate);
    result
}
struct CatalogLease(PathBuf);
impl Drop for CatalogLease {
    fn drop(&mut self) {
        let _removed = fs::remove_file(&self.0);
    }
}
fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    match (args.first().map(String::as_str), args.as_slice()) {
        (Some("cleanup"), [_, scratch]) => {
            let removed = kiln_web_service::preview::retention::cleanup(
                Path::new(scratch),
                kiln_web_service::preview::service::now()?,
            )?;
            println!("{}", serde_json::json!({"removed_expired_owned_exports":removed}));
            Ok(())
        }
        (Some("pin-engine"), [_, root, module_dir]) => {
            let manifest = engine::pin(Path::new(root), module_dir)?;
            println!("{}", serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?);
            Ok(())
        }
        (Some("produce"), [_, plan, observation, current, selection, root, manifest, nix, nix_store, scratch]) => {
            let input = kiln_web_service::preview::PreviewProduction {
                plan: json(Path::new(plan))?,
                observation: json(Path::new(observation))?,
                current: json(Path::new(current))?,
                selection: json(Path::new(selection))?,
            };
            let manifest = json(Path::new(manifest))?;
            let engine = EngineBundle::load(Path::new(root), manifest)?;
            let nix = native_nix(nix, nix_store)?;
            let request = kiln_web_service::preview::produce(
                input, &engine, &nix, Path::new(scratch),
                kiln_web_service::preview::service::now()?,
            )?;
            println!("{}", serde_json::to_string_pretty(&request).map_err(|e| e.to_string())?);
            Ok(())
        }
        (Some("engine-cohort"), [_, selection, root, manifest, nix, nix_store, scratch]) => {
            let selection = json(Path::new(selection))?;
            let manifest = json(Path::new(manifest))?;
            let engine = EngineBundle::load(Path::new(root), manifest)?;
            let nix = native_nix(nix, nix_store)?;
            let server = PreviewServer::prepare_engine_cohort(selection, engine, &nix, Path::new(scratch))?;
            println!("{}", serde_json::json!({"qualification":"engine-only; no canonical CI binding","launch_url":server.launch_url()?}));
            serve(server)
        }
        (Some("serve"), [_, request, current, policy, root, manifest, nix, nix_store, scratch, catalog]) => {
            let request = read_request(Path::new(request))?;
            let current: PreviewBinding = json(Path::new(current))?;
            let policy = json(Path::new(policy))?;
            let manifest: EngineManifest = json(Path::new(manifest))?;
            let engine = EngineBundle::load(Path::new(root), manifest)?;
            let nix = native_nix(nix, nix_store)?;
            let server = PreviewServer::prepare(request, &current, &policy, engine, &nix, Path::new(scratch))?;
            let entry = server.entry()?;
            // Never overwrite a shared console catalog; the caller explicitly
            // selects this owned disposable file as its optional projection.
            let mut file = fs::OpenOptions::new().write(true).create_new(true).open(catalog).map_err(|e| e.to_string())?;
            let _lease = CatalogLease(PathBuf::from(catalog));
            file.write_all(&serde_json::to_vec(&vec![&entry]).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
            println!("{}", serde_json::to_string(&entry).map_err(|e| e.to_string())?);
            serve(server)
        }
        _ => Err("usage: artifact-preview pin-engine PREBUILT_TRYNIX_TREE MODULE_DIR\n       artifact-preview produce PLAN.json OBSERVATION.json CURRENT_BINDING.json SELECTION.json PREBUILT_TRYNIX_TREE ENGINE_MANIFEST.json ABS_NIX ABS_NIX_STORE OWNED_SCRATCH\n       artifact-preview serve ADMITTED_REQUEST.json CURRENT_BINDING.json SELECTION.json PREBUILT_TRYNIX_TREE ENGINE_MANIFEST.json ABS_NIX ABS_NIX_STORE OWNED_SCRATCH NEW_CATALOG.json\n       artifact-preview engine-cohort SELECTION.json PREBUILT_TRYNIX_TREE ENGINE_MANIFEST.json ABS_NIX ABS_NIX_STORE OWNED_SCRATCH\nNormal previews require canonical admitted CI records and independent operator runtime/trust policy; engine-cohort has no CI binding and cannot enter a review catalog. No command creates CI evidence or grants upstream cache trust.".into()),
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
