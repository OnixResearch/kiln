#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::non_trait_imports,
    reason = "the test-only binary mirrors the published extension process boundary"
)]
#![allow(
    tigerstyle::path_segment_repetition,
    reason = "published Kiln Aspen extension names preserve their protocol ownership"
)]

use std::io::{Read, Write};

use kiln_adapter_aspen::profile::{admit_profile, profile_fixture};
use kiln_aspen_extension::KilnAspenCallback;

const READ_OVERFLOW_SENTINEL_BYTES: usize = 1;
const FAILURE_EXIT_CODE: u8 = 1;

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            match writeln!(std::io::stderr().lock(), "{message}") {
                Ok(()) | Err(_) => {}
            }
            std::process::ExitCode::from(FAILURE_EXIT_CODE)
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
    let callback = KilnAspenCallback::new(admitted);
    let output = callback
        .invoke_aspen_native(&input)
        .map_err(|error| format!("callback denied: {error}"))?;
    if output.len() > maximum_output_bytes {
        return Err("callback output exceeds its named byte bound".to_string());
    }
    std::io::stdout()
        .lock()
        .write_all(&output)
        .map_err(|error| format!("callback output write failed: {error}"))?;
    Ok(())
}
