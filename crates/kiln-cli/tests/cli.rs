//! The `kiln` operator CLI's help and refusal surface, observed at the process
//! boundary: which stream receives the text and which status the process returns.

use std::process::{Command, Output};

const COMMANDS: [&str; 4] = ["demo", "evidence", "host-qualify", "dashboard"];
const USAGE_PREFIX: &str = "usage: kiln ";
const EXIT_SUCCESS: i32 = 0;
const EXIT_FAILURE: i32 = 1;
const EXIT_USAGE: i32 = 2;

fn kiln(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kiln"))
        .args(arguments)
        .output()
        .unwrap_or_else(|error| unreachable!("kiln did not start: {error}"))
}

fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap_or_else(|error| unreachable!("kiln wrote non-UTF-8: {error}"))
}

// r[verify kiln.cli.help]
// r[verify kiln.cli.help.scenario.help]
#[test]
fn every_help_spelling_prints_the_same_usage_on_standard_output() {
    let long = kiln(&["--help"]);
    assert_eq!(long.status.code(), Some(EXIT_SUCCESS));
    assert!(long.stderr.is_empty());
    let usage = text(&long.stdout);
    assert!(usage.starts_with(USAGE_PREFIX), "{usage}");
    for command in COMMANDS {
        assert!(
            usage.contains(&format!("\n  {command}")),
            "usage does not list {command}: {usage}"
        );
    }
    for spelling in ["-h", "help"] {
        let output = kiln(&[spelling]);
        assert_eq!(output.status.code(), Some(EXIT_SUCCESS), "{spelling}");
        assert_eq!(output.stdout, long.stdout, "{spelling}");
        assert!(output.stderr.is_empty(), "{spelling}");
    }
}

// r[verify kiln.cli.help.scenario.refused]
#[test]
fn unusable_input_is_refused_on_standard_error_only() {
    let cases: [(&[&str], i32, &str); 5] = [
        (&[], EXIT_USAGE, USAGE_PREFIX),
        (&["--helpx"], EXIT_FAILURE, "unsupported command: --helpx"),
        (&["-help"], EXIT_FAILURE, "unsupported command: -help"),
        (&["--help", "demo"], EXIT_USAGE, USAGE_PREFIX),
        (&["help", "dashboard"], EXIT_USAGE, USAGE_PREFIX),
    ];
    for (arguments, status, report) in cases {
        let output = kiln(arguments);
        assert_eq!(output.status.code(), Some(status), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        let stderr = text(&output.stderr);
        assert!(stderr.starts_with(report), "{arguments:?}: {stderr}");
    }
}
