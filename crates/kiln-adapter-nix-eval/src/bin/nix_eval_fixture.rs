use std::env;
use std::io::{self, Write};
use std::process::{self, Command};
use std::thread;
use std::time::Duration;

const FAILURE_STATUS: i32 = 19;
const FLOOD_BYTES: usize = 131_072;
const TIMEOUT_MILLISECONDS: u64 = 60_000;
const LATE_OUTPUT_MILLISECONDS: u64 = 200;
const FIXTURE_BYTE: u8 = b'x';
const VALID_RECORD: &str = "{\"attr\":\"fixture\",\"attrPath\":[\"fixture\"],\"name\":\"fixture\",\"storeDir\":\"/nix/store\",\"system\":\"x86_64-linux\",\"drvPath\":\"/nix/store/aaaaaaaa-fixture.drv\",\"outputs\":{\"out\":\"/nix/store/aaaaaaaa-fixture\"},\"inputDrvs\":{},\"requiredSystemFeatures\":[],\"constituents\":[],\"namedConstituents\":[],\"globConstituents\":false,\"isCached\":false,\"cacheStatus\":\"notBuilt\",\"neededBuilds\":[],\"neededSubstitutes\":[]}\n";
const ERROR_RECORD: &str = "{\"attr\":\"broken\",\"attrPath\":[\"broken\"],\"error\":\"fixture evaluation error\",\"fatal\":false}\n";

fn main() {
    let Some(mode) = env::args().nth(1) else {
        process::exit(FAILURE_STATUS);
    };
    let result = match mode.as_str() {
        "valid" => write_stdout(VALID_RECORD.as_bytes()),
        "job-error" => write_stdout(ERROR_RECORD.as_bytes()),
        "malformed" => write_stdout(b"{not-json}\n"),
        "truncated" => write_stdout(b"{\"attr\":\"partial\""),
        "output-flood" => write_stdout(&vec![FIXTURE_BYTE; FLOOD_BYTES]),
        "stderr-flood" => write_stderr(&vec![FIXTURE_BYTE; FLOOD_BYTES]),
        "crash" => process::exit(FAILURE_STATUS),
        "signal" => terminate_by_signal(),
        "timeout" => {
            thread::sleep(Duration::from_millis(TIMEOUT_MILLISECONDS));
            Ok(())
        }
        "late-output" => {
            thread::sleep(Duration::from_millis(LATE_OUTPUT_MILLISECONDS));
            write_stdout(VALID_RECORD.as_bytes())
        }
        "descendant" => spawn_descendant(),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unknown fixture mode",
        )),
    };
    if result.is_err() {
        process::exit(FAILURE_STATUS);
    }
}

fn write_stdout(bytes: &[u8]) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(bytes)?;
    stdout.flush()
}

fn write_stderr(bytes: &[u8]) -> io::Result<()> {
    let mut stderr = io::stderr().lock();
    stderr.write_all(bytes)?;
    stderr.flush()
}

fn terminate_by_signal() -> io::Result<()> {
    // SIGABRT can wait for a host core-dump handler before becoming reapable.
    // SIGKILL exercises signal completion without that external dependency.
    nix::sys::signal::raise(nix::sys::signal::Signal::SIGKILL).map_err(io::Error::from)?;
    Err(io::Error::other(
        "signal fixture unexpectedly survived SIGKILL",
    ))
}

fn spawn_descendant() -> io::Result<()> {
    let executable = env::current_exe()?;
    let _child = Command::new(executable).arg("timeout").spawn()?;
    Ok(())
}
