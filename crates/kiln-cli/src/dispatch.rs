//! Sibling binary resolution and subprocess delegation for unified `kiln` subcommands.
//!
//! Candidate ordering is pure in [`resolve_sibling_candidates`]; process
//! spawning and exit-code forwarding stay in [`run_delegated`].

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Resolve candidate executable paths for `binary_name`: first beside
/// `current_exe`, then bare `binary_name` for `PATH` lookup.
pub fn resolve_sibling_candidates(current_exe: Option<&Path>, binary_name: &str) -> Vec<PathBuf> {
    if binary_name.is_empty() {
        return Vec::new();
    }
    let mut candidates = Vec::new();
    if let Some(exe) = current_exe
        && let Some(dir) = exe.parent()
        && !dir.as_os_str().is_empty()
    {
        candidates.push(dir.join(binary_name));
    }
    let bare = PathBuf::from(binary_name);
    if !candidates.contains(&bare) {
        candidates.push(bare);
    }
    candidates
}

/// Execute `binary_name` with inherited stdio, preferring a sibling binary beside
/// `current_exe` and falling back to `PATH`.
pub fn run_delegated(binary_name: &str, args: Vec<String>) -> Result<i32, String> {
    let current_exe = std::env::current_exe().ok();
    run_delegated_with_exe(current_exe.as_deref(), binary_name, &args)
}

fn run_delegated_with_exe(
    current_exe: Option<&Path>,
    binary_name: &str,
    args: &[String],
) -> Result<i32, String> {
    let candidates = resolve_sibling_candidates(current_exe, binary_name);
    for candidate in candidates {
        match Command::new(&candidate)
            .args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
        {
            Ok(status) => return Ok(exit_code_from_status(status)),
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "failed to execute '{}': {error}",
                    candidate.display()
                ));
            }
        }
    }
    Err(format!(
        "could not find '{binary_name}' beside kiln or on PATH"
    ))
}

fn exit_code_from_status(status: std::process::ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status.signal().map_or(1, |signal| 128 + signal)
    }
    #[cfg(not(unix))]
    {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_sibling_candidates_prefers_current_exe_directory_then_path() {
        let candidates =
            resolve_sibling_candidates(Some(Path::new("/usr/local/bin/kiln")), "kiln-fleet");
        assert_eq!(
            candidates,
            vec![
                PathBuf::from("/usr/local/bin/kiln-fleet"),
                PathBuf::from("kiln-fleet"),
            ]
        );
    }

    #[test]
    fn resolve_sibling_candidates_falls_back_to_bare_name_without_current_exe() {
        assert_eq!(
            resolve_sibling_candidates(None, "kiln-fleet"),
            vec![PathBuf::from("kiln-fleet")]
        );
        assert_eq!(
            resolve_sibling_candidates(Some(Path::new("kiln")), "kiln-fleet"),
            vec![PathBuf::from("kiln-fleet")]
        );
        assert!(resolve_sibling_candidates(Some(Path::new("/bin/kiln")), "").is_empty());
    }

    #[test]
    fn run_delegated_reports_missing_binary_clearly() {
        let missing = "kiln-nonexistent-binary-for-test";
        let result = run_delegated_with_exe(
            Some(Path::new("/nonexistent/dir/kiln")),
            missing,
            &["--help".to_owned()],
        );
        let Err(message) = result else {
            unreachable!("expected missing binary error, got {result:?}");
        };
        assert!(message.contains(missing), "{message}");
        assert!(message.contains("beside kiln or on PATH"), "{message}");
    }
}
