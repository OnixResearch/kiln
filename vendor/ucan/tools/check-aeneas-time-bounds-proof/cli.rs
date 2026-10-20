use std::path::PathBuf;

use crate::error::CheckerError;

const DEFAULT_ROOT: &str = ".";
const DEFAULT_NICKEL: &str = "nickel";
const OPTION_PAIR_WIDTH: usize = 2;

#[derive(Debug, Eq, PartialEq)]
pub struct Options {
    pub root: PathBuf,
    pub nickel: PathBuf,
}

pub fn parse(arguments: &[String]) -> Result<Options, CheckerError> {
    let mut root = PathBuf::from(DEFAULT_ROOT);
    let mut nickel = PathBuf::from(DEFAULT_NICKEL);
    let mut index = 1;
    if arguments.get(index).is_some_and(|argument| argument == "check") {
        index += 1;
    }
    while index < arguments.len() {
        let flag = &arguments[index];
        let value = arguments
            .get(index + 1)
            .ok_or_else(|| CheckerError::new("arguments", format!("missing value for {flag}")))?;
        match flag.as_str() {
            "--root" => root = PathBuf::from(value),
            "--nickel" => nickel = PathBuf::from(value),
            _ => return Err(CheckerError::new("arguments", format!("unknown option {flag}"))),
        }
        index += OPTION_PAIR_WIDTH;
    }
    Ok(Options { root, nickel })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_defaults_and_explicit_paths() {
        let defaults = parse(&["checker".to_owned()]);
        assert_eq!(defaults.expect("defaults should parse"), Options {
            root: PathBuf::from(DEFAULT_ROOT),
            nickel: PathBuf::from(DEFAULT_NICKEL),
        });
        let explicit = parse(&[
            "checker".to_owned(),
            "check".to_owned(),
            "--root".to_owned(),
            "/repo".to_owned(),
            "--nickel".to_owned(),
            "/nix/store/nickel".to_owned(),
        ]);
        assert_eq!(explicit.expect("explicit options should parse"), Options {
            root: PathBuf::from("/repo"),
            nickel: PathBuf::from("/nix/store/nickel"),
        });
    }

    #[test]
    fn rejects_unknown_and_incomplete_options() {
        assert!(parse(&["checker".to_owned(), "--unknown".to_owned(), "value".to_owned()]).is_err());
        assert!(parse(&["checker".to_owned(), "--root".to_owned()]).is_err());
    }
}
