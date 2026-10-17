use std::error::Error;
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use iroh::{EndpointAddr, SecretKey};
use kiln_adapter_iroh::{Profile, bind_endpoint, serve_proxy, serve_worker};
use serde::de::DeserializeOwned;
use tokio::signal::unix::{Signal, SignalKind, signal};

const SECRET_KEY_BYTES: usize = 32;
const MAXIMUM_PROFILE_BYTES: u64 = 1_048_576;
const MAXIMUM_ENDPOINT_BYTES: u64 = 65_536;
const HELP: &str = "kiln-adapter-iroh — authenticated native Lattice workflow transport

USAGE:
  kiln-adapter-iroh keygen --key PATH
  kiln-adapter-iroh validate --profile JSON
  kiln-adapter-iroh worker --profile JSON --key PATH
  kiln-adapter-iroh proxy --profile JSON --key PATH --remote ENDPOINT-JSON-PATH
  kiln-adapter-iroh --help

Key files contain exactly 32 raw secret bytes and must have no group/world permissions.
keygen creates a new mode-0600 file, refuses overwrite, and prints only the public key.
Profiles are bounded JSON exported from the typed config/iroh-workflow.ncl builder.
Worker/proxy print one public EndpointAddr JSON line at startup. Save the worker line
as the proxy --remote file; its endpoint ID must be enrolled in allowed_peers.
Worker socket_path selects Lattice; proxy socket_path is its private local listener.
SIGINT and SIGTERM stop the service and its owned exchanges. No automatic retries.
";

type CliResult<T> = Result<T, Box<dyn Error>>;

enum Command {
    Help,
    Keygen {
        key: PathBuf,
    },
    Validate {
        profile: PathBuf,
    },
    Worker {
        profile: PathBuf,
        key: PathBuf,
    },
    Proxy {
        profile: PathBuf,
        key: PathBuf,
        remote: PathBuf,
    },
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn parse_command() -> CliResult<Command> {
    let mut args = std::env::args_os().skip(1);
    let Some(command) = args.next() else {
        return Ok(Command::Help);
    };
    if command == "--help" || command == "-h" || command == "help" {
        if args.next().is_some() {
            return Err(invalid("help takes no arguments").into());
        }
        return Ok(Command::Help);
    }
    if !["keygen", "validate", "worker", "proxy"]
        .iter()
        .any(|name| command == *name)
    {
        return Err(invalid("unknown command; use --help").into());
    }
    let mut key = None;
    let mut profile = None;
    let mut remote = None;
    while let Some(flag) = args.next() {
        let slot = match flag.to_str() {
            Some("--key") if command != "validate" => &mut key,
            Some("--profile") if command != "keygen" => &mut profile,
            Some("--remote") if command == "proxy" => &mut remote,
            _ => return Err(invalid("unknown or inapplicable option; use --help").into()),
        };
        if slot.is_some() {
            return Err(invalid("duplicate option").into());
        }
        let value: OsString = args
            .next()
            .ok_or_else(|| invalid("option requires a path"))?;
        if value.is_empty() || value.as_encoded_bytes().starts_with(b"--") {
            return Err(invalid("option requires a path").into());
        }
        *slot = Some(PathBuf::from(value));
    }
    let required = |value: Option<PathBuf>| {
        value.ok_or_else(|| invalid("missing required option; use --help"))
    };
    match command.to_str() {
        Some("keygen") => Ok(Command::Keygen {
            key: required(key)?,
        }),
        Some("validate") => Ok(Command::Validate {
            profile: required(profile)?,
        }),
        Some("worker") => Ok(Command::Worker {
            profile: required(profile)?,
            key: required(key)?,
        }),
        Some("proxy") => Ok(Command::Proxy {
            profile: required(profile)?,
            key: required(key)?,
            remote: required(remote)?,
        }),
        _ => Err(invalid("unknown command; use --help").into()),
    }
}

fn read_json<T: DeserializeOwned>(path: &Path, maximum_bytes: u64) -> CliResult<T> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("JSON input must be a regular file").into());
    }
    let mut bytes = Vec::new();
    file.take(maximum_bytes + 1).read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len())? > maximum_bytes {
        return Err(invalid("JSON input exceeds its byte limit").into());
    }
    Ok(serde_json::from_slice(&bytes)?)
}

fn load_profile(path: &Path) -> CliResult<Profile> {
    let profile: Profile = read_json(path, MAXIMUM_PROFILE_BYTES)?;
    profile.admit()?;
    Ok(profile)
}

fn load_key(path: &Path) -> CliResult<SecretKey> {
    // O_NOFOLLOW rejects a symlink atomically. O_NONBLOCK avoids hanging on a FIFO
    // before the descriptor metadata check rejects all non-regular files.
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.len() != SECRET_KEY_BYTES as u64
    {
        return Err(invalid("key must be a private regular file of exactly 32 raw bytes").into());
    }
    let mut bytes = [0_u8; SECRET_KEY_BYTES];
    file.read_exact(&mut bytes)?;
    let mut extra = [0_u8; 1];
    if file.read(&mut extra)? != 0 {
        return Err(invalid("key must contain exactly 32 raw bytes").into());
    }
    Ok(SecretKey::from_bytes(&bytes))
}

fn generate_key(path: &Path) -> CliResult<()> {
    let mut bytes = [0_u8; SECRET_KEY_BYTES];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    let key = SecretKey::from_bytes(&bytes);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    println!("{}", key.public());
    Ok(())
}

struct Shutdown {
    interrupt: Signal,
    terminate: Signal,
}

impl Shutdown {
    fn install() -> io::Result<Self> {
        // Register synchronously before any networking; registration failure is a
        // startup error, never an immediately-completed successful shutdown future.
        Ok(Self {
            interrupt: signal(SignalKind::interrupt())?,
            terminate: signal(SignalKind::terminate())?,
        })
    }

    async fn wait(mut self) {
        tokio::select! {
            _ = self.interrupt.recv() => {},
            _ = self.terminate.recv() => {},
        }
    }
}

fn print_endpoint(endpoint: &iroh::Endpoint) -> CliResult<()> {
    let mut stdout = io::stdout().lock();
    serde_json::to_writer(&mut stdout, &endpoint.addr())?;
    writeln!(stdout)?;
    stdout.flush()?;
    Ok(())
}

async fn run(command: Command) -> CliResult<()> {
    match command {
        Command::Help => print!("{HELP}"),
        Command::Keygen { key } => generate_key(&key)?,
        Command::Validate { profile } => {
            load_profile(&profile)?;
            println!("profile admitted");
        }
        Command::Worker { profile, key } => {
            let profile = load_profile(&profile)?;
            let key = load_key(&key)?;
            let shutdown = Shutdown::install()?;
            let endpoint = bind_endpoint(&profile, key).await?;
            let result: CliResult<()> = async {
                print_endpoint(&endpoint)?;
                serve_worker(endpoint.clone(), profile, shutdown.wait()).await?;
                Ok(())
            }
            .await;
            endpoint.close().await;
            result?;
        }
        Command::Proxy {
            profile,
            key,
            remote,
        } => {
            let profile = load_profile(&profile)?;
            let remote: EndpointAddr = read_json(&remote, MAXIMUM_ENDPOINT_BYTES)?;
            profile.admit_peer(remote.id)?;
            let key = load_key(&key)?;
            let shutdown = Shutdown::install()?;
            let endpoint = bind_endpoint(&profile, key).await?;
            let result: CliResult<()> = async {
                print_endpoint(&endpoint)?;
                serve_proxy(endpoint.clone(), remote, profile, shutdown.wait()).await?;
                Ok(())
            }
            .await;
            endpoint.close().await;
            result?;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let result = match parse_command() {
        Ok(command) => run(command).await,
        Err(error) => Err(error),
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("kiln-adapter-iroh: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    // r[verify kiln.iroh.workflow.profile.scenario.valid]
    // r[verify kiln.iroh.workflow.profile.scenario.invalid]
    #[test]
    fn identity_is_private_create_once_and_rejects_unsafe_inputs() -> CliResult<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("identity.key");
        generate_key(&path)?;
        let original = std::fs::read(&path)?;
        let public = load_key(&path)?.public();
        assert_eq!(
            std::fs::metadata(&path)?.permissions().mode() & 0o777,
            0o600
        );
        assert!(generate_key(&path).is_err());
        assert_eq!(std::fs::read(&path)?, original);
        assert_eq!(load_key(&path)?.public(), public);

        let link = root.path().join("linked.key");
        symlink(&path, &link)?;
        assert!(load_key(&link).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))?;
        assert!(load_key(&path).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        std::fs::write(&path, [0; SECRET_KEY_BYTES - 1])?;
        assert!(load_key(&path).is_err());
        std::fs::write(&path, [0; SECRET_KEY_BYTES + 1])?;
        assert!(load_key(&path).is_err());
        assert!(load_key(root.path()).is_err());
        Ok(())
    }
}
