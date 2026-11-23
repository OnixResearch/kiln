//! Operator consumer for retained capability inspection, immutable local object
//! import, and the selected hosted retained authority. It performs no deployment
//! and never converts unavailable migration prerequisites into skipped success.
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use kiln_core::types::ContentId;
use kiln_hosted::execution::{Backend, Config};
use kiln_hosted::microvm::retained::{self, Operation, SelectedAspenAuthority};
use kiln_hosted::microvm::transfer::{CheckpointStore, DurableObject};
use kiln_microvm::handoff::{CheckpointExpectation, CheckpointManifest};
use serde::de::DeserializeOwned;
use serde_json::json;

const MAX_INPUT_BYTES: u64 = 1_048_576;
const LOCAL_IMPORT_DEADLINE_SECONDS: u64 = 60;
fn load<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err("operator input exceeds bound".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

fn run(args: &[String]) -> Result<(), String> {
    match args {
        [command] if command == "capabilities" => {
            println!("{}", json!({"schema":"kiln.retained-host-capabilities.v1", "molten_revision":retained::SELECTED_MOLTEN_REVISION,
                "host_runtime_iroh":"1.1.0", "workflow_iroh":"1.1.0", "retained_authority":"unavailable",
                "cross_host_fabric":"unqualified-loopback-only", "live_migration":"denied",
                "local_checkpoint_storage":"bounded-verified-fsync", "qualification":"not-run",
                "missing":["Aspen current-placement CAS and resource fencing", "Aspen admitted authenticated cross-host stream",
                    "Lattice retained guest provider/reconnect gateway", "two compatible KVM hosts and exact CH final-ACK/fencing qualification"]}));
            Ok(())
        },
        [command] if command == "require-migration" => kiln_microvm::retained::drain::require_live_capability().map_err(|e| format!("{e:?}")),
        [command, config, backend] if command == "profile" => {
            let config: Config = load(config)?;
            let backend: Backend = load(backend)?;
            let profile = retained::observed_profile(&config, &backend)?;
            println!("{}", json!({"profile":profile,"qualification":"not-qualified"}));
            Ok(())
        },
        [command, directory, operation, object, input] if command == "import-object" => {
            let operation = ContentId::from_blake3_hex(operation.clone()).map_err(|e| format!("{e:?}"))?;
            let object: DurableObject = load(object)?;
            let mut input = OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(input).map_err(|e| e.to_string())?;
            let metadata = input.metadata().map_err(|e| e.to_string())?;
            if !metadata.is_file() || metadata.len() != object.bytes { return Err("input is not the exact regular checkpoint object".into()); }
            let receipt = CheckpointStore::open(Path::new(directory))?.receive(&operation,&object,&mut input,
                Instant::now()+Duration::from_secs(LOCAL_IMPORT_DEADLINE_SECONDS),&AtomicBool::new(false))?;
            println!("{}", serde_json::to_string(&receipt).map_err(|e| e.to_string())?);
            Ok(())
        },
        [command, directory, object] if command == "verify-object" => {
            let object: DurableObject = load(object)?;
            CheckpointStore::open(Path::new(directory))?.verify(&object)?;
            println!("{}", json!({"verified":object,"placement_committed":false}));
            Ok(())
        },
        [command, directory, manifest, expected, objects] if command == "publish-checkpoint" => {
            let manifest: CheckpointManifest = load(manifest)?;
            let expected: CheckpointExpectation = load(expected)?;
            let objects: [DurableObject;4] = load(objects)?;
            let identity = CheckpointStore::open(Path::new(directory))?.publish_checkpoint(&manifest,&expected,&objects)?;
            println!("{}",json!({"durable_checkpoint":identity,"placement_committed":false,"destination_resumed":false}));
            Ok(())
        },
        [command, database, operation] if command == "reopen-checkpoint" => {
            let operation: Operation = load(operation)?;
            let mut db = rusqlite::Connection::open_with_flags(database,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE).map_err(|e| e.to_string())?;
            retained::reopen_retired_checkpoint(&mut db,&mut SelectedAspenAuthority,&operation)?;
            println!("{}",json!({"receiving":operation.session,"work_dispatched":false}));
            Ok(())
        },
        [command, database, config, backend, operation] if command == "apply" => {
            let config: Config = load(config)?;
            let backend: Backend = load(backend)?;
            let operation: Operation = load(operation)?;
            // Open an existing operator database only, never initialize a new
            // ownership ledger as a substitute for the selected Aspen provider.
            let mut db = rusqlite::Connection::open_with_flags(database,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE).map_err(|e| e.to_string())?;
            let outcome = retained::execute(&mut db,&config,&backend,&mut SelectedAspenAuthority,&operation)?;
            println!("{}",serde_json::to_string(&outcome).map_err(|e| e.to_string())?);
            Ok(())
        },
        _ => Err("usage: retained-host capabilities | require-migration | profile CONFIG BACKEND | import-object PRIVATE-STORE OPERATION-HEX OBJECT-JSON INPUT | verify-object PRIVATE-STORE OBJECT-JSON | publish-checkpoint PRIVATE-STORE MANIFEST-JSON INDEPENDENT-FACTS-JSON OBJECTS-JSON | reopen-checkpoint EXISTING-DB OPERATION-JSON | apply EXISTING-DB CONFIG BACKEND OPERATION-JSON".into()),
    }
}
fn main() -> std::process::ExitCode {
    match run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
