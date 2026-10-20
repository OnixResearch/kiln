use std::path::PathBuf;

use crate::error::ShellError;

const DEFAULT_ROOT: &str = ".";
const DEFAULT_NICKEL: &str = "nickel";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Check,
    Generate,
}

#[derive(Debug)]
pub struct Options {
    pub mode: Mode,
    pub root: PathBuf,
    pub nickel: PathBuf,
}

pub fn parse(arguments: &[String]) -> Result<Options, ShellError> {
    let mut mode = Mode::Check;
    let mut root = PathBuf::from(DEFAULT_ROOT);
    let mut nickel = PathBuf::from(DEFAULT_NICKEL);
    let mut index = 1;
    if let Some(command) = arguments.get(index) {
        match command.as_str() {
            "check" => {
                mode = Mode::Check;
                index += 1;
            }
            "generate" => {
                mode = Mode::Generate;
                index += 1;
            }
            _ if command.starts_with('-') => {}
            _ => return Err(ShellError::new("arguments", format!("unknown command {command}"))),
        }
    }
    while index < arguments.len() {
        let flag = &arguments[index];
        let value = arguments
            .get(index + 1)
            .ok_or_else(|| ShellError::new("arguments", format!("missing value for {flag}")))?;
        match flag.as_str() {
            "--root" => root = PathBuf::from(value),
            "--nickel" => nickel = PathBuf::from(value),
            _ => return Err(ShellError::new("arguments", format!("unknown option {flag}"))),
        }
        index += 2;
    }
    Ok(Options { mode, root, nickel })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_defaults_and_explicit_paths() {
        let defaults = parse(&["checker".to_owned()]);
        assert!(matches!(defaults, Ok(Options { mode: Mode::Check, .. })));
        let explicit = parse(&[
            "checker".to_owned(),
            "generate".to_owned(),
            "--root".to_owned(),
            "/tmp/repo".to_owned(),
            "--nickel".to_owned(),
            "/nix/store/nickel".to_owned(),
        ]);
        assert!(matches!(
            explicit,
            Ok(Options {
                mode: Mode::Generate,
                ..
            })
        ));
    }

    #[test]
    fn rejects_unknown_commands_and_missing_values() {
        assert!(parse(&["checker".to_owned(), "unknown".to_owned()]).is_err());
        assert!(parse(&["checker".to_owned(), "--root".to_owned()]).is_err());
    }
}
