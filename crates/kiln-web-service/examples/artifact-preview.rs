//! Local exact-artifact consumer. Canonical mode builds only through an existing
//! admitted Engine run; no request can assert successful build observations.
use kiln_web_service::preview::{
    PreviewSelection,
    engine::{self, EngineBundle, EngineManifest},
    nix::NativeNix,
    service::PreviewServer,
};
use kiln_web_service::{engine::Engine, profile::ServiceProfile};
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
fn serve_catalog(server: PreviewServer, catalog: &str) -> Result<(), String> {
    let entry = server.entry()?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(catalog)
        .map_err(|error| error.to_string())?;
    let _lease = CatalogLease(PathBuf::from(catalog));
    file.write_all(&serde_json::to_vec(&vec![&entry]).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    println!(
        "{}",
        serde_json::to_string(&entry).map_err(|error| error.to_string())?
    );
    serve(server)
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
        (Some("cleanup-observations"), [_, directory]) => {
            let removed = kiln_web_service::preview::observation::cleanup_expired(
                Path::new(directory),
                kiln_web_service::preview::service::now()?,
            )?;
            println!("{}", serde_json::json!({"removed_expired_observations":removed}));
            Ok(())
        }
        (Some("pin-engine"), [_, root, module_dir]) => {
            let manifest = engine::pin(Path::new(root), module_dir)?;
            println!("{}", serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?);
            Ok(())
        }
        (Some("run-build"), [_, service_profile, source_run_id, request_id]) => {
            let profile = ServiceProfile::load(Path::new(service_profile))?;
            let mut engine_state = Engine::open(profile)?;
            let run_id = engine_state.run_canonical_nix_build(source_run_id, request_id)?;
            println!("{}", serde_json::json!({"run_id":run_id}));
            Ok(())
        }
        (Some("produce"), [_, service_profile, run_id, node_id, selection_path, root, manifest_path, nix, nix_store, scratch, catalog, observations @ ..]) if observations.len() <= 1 => {
            let profile = ServiceProfile::load(Path::new(service_profile))?;
            let engine_state = Engine::open(profile)?;
            let selection: PreviewSelection = json(Path::new(selection_path))?;
            let manifest: EngineManifest = json(Path::new(manifest_path))?;
            let bundle = EngineBundle::load(Path::new(root), manifest)?;
            let nix = native_nix(nix, nix_store)?;
            let mut server = engine_state.prepare_preview_server(run_id, node_id, selection, bundle, &nix, Path::new(scratch))?;
            drop(engine_state);
            if let [directory] = observations {
                server = server.with_observations(Path::new(directory))?;
            }
            serve_catalog(server, catalog)
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
        _ => Err("usage: artifact-preview pin-engine PREBUILT_TRYNIX_TREE MODULE_DIR\n       artifact-preview run-build SERVICE_PROFILE SOURCE_RUN_ID REQUEST_ID\n       artifact-preview produce SERVICE_PROFILE BUILD_RUN_ID BUILD_NODE SELECTION.json PREBUILT_TRYNIX_TREE ENGINE_MANIFEST.json ABS_NIX ABS_NIX_STORE OWNED_SCRATCH NEW_CATALOG.json [OWNED_OBSERVATIONS_DIR]\n       artifact-preview engine-cohort SELECTION.json PREBUILT_TRYNIX_TREE ENGINE_MANIFEST.json ABS_NIX ABS_NIX_STORE OWNED_SCRATCH\n       artifact-preview cleanup OWNED_SCRATCH\n       artifact-preview cleanup-observations OWNED_OBSERVATIONS_DIR\nrun-build uses only the opted-in service-owned Nix profile and persists a new exact-source canonical build run. produce is read-only: it accepts only an exact canonical Build run that already retains the actual admitted Nix plan, product observation and complete runtime closure. It performs no build and imports no plan or observation JSON. With OWNED_OBSERVATIONS_DIR the preview retains bounded browser reports composed against the admitted descriptor; they are preview evidence only. Engine-cohort remains engine-only and cannot enter a review catalog. No command builds latest HEAD or grants upstream cache trust.".into()),
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
