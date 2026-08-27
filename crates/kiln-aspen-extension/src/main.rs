use std::io::{Read, Write};
use std::process::ExitCode;

use kiln_adapter_aspen::profile::{admit_profile, profile_fixture};
use kiln_adapter_aspen::service::HostedCallback;
use kiln_aspen_extension::KilnAspenCallback;

const READ_OVERFLOW_SENTINEL_BYTES: usize = 1;
const FAILURE_EXIT_CODE: u8 = 1;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            match writeln!(std::io::stderr().lock(), "{message}") {
                Ok(()) | Err(_) => {}
            }
            ExitCode::from(FAILURE_EXIT_CODE)
        }
    }
}

fn run() -> Result<(), String> {
    let admitted = admit_profile(&profile_fixture())
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let read_limit = admitted
        .profile
        .bounds
        .max_callback_input_bytes
        .checked_add(READ_OVERFLOW_SENTINEL_BYTES)
        .ok_or_else(|| "callback input read bound overflowed".to_string())?;
    let read_limit = u64::try_from(read_limit)
        .map_err(|_| "callback input read bound does not fit u64".to_string())?;
    let mut input = Vec::new();
    std::io::stdin()
        .lock()
        .take(read_limit)
        .read_to_end(&mut input)
        .map_err(|error| format!("callback input read failed: {error}"))?;
    if input.len() > admitted.profile.bounds.max_callback_input_bytes {
        return Err("callback input exceeds its named byte bound".to_string());
    }
    let maximum_output_bytes = admitted.profile.bounds.max_callback_output_bytes;
    let mut callback = KilnAspenCallback::new(admitted);
    let output = callback
        .invoke(&input, maximum_output_bytes)
        .map_err(|error| format!("callback denied: {error:?}"))?;
    if output.len() > maximum_output_bytes {
        return Err("callback output exceeds its named byte bound".to_string());
    }
    std::io::stdout()
        .lock()
        .write_all(&output)
        .map_err(|error| format!("callback output write failed: {error}"))?;
    Ok(())
}
