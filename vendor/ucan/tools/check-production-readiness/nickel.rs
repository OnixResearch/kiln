use std::path::Path;
use std::process::Command;

use crate::error::ShellError;

const MAXIMUM_EVALUATOR_OUTPUT_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub struct Evaluation {
    pub success: bool,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

pub fn export(nickel: &Path, source: &Path) -> Result<Evaluation, ShellError> {
    let output = Command::new(nickel)
        .arg("export")
        .arg("--color")
        .arg("never")
        .arg("--format")
        .arg("json")
        .arg(source)
        .output()
        .map_err(|error| ShellError::new("nickel-execution", error.to_string()))?;
    if output.stdout.len() > MAXIMUM_EVALUATOR_OUTPUT_BYTES || output.stderr.len() > MAXIMUM_EVALUATOR_OUTPUT_BYTES {
        return Err(ShellError::new("nickel-output-bound", "Nickel output exceeded the bounded checker budget"));
    }
    Ok(Evaluation {
        success: output.status.success(),
        stdout: output.stdout,
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}
