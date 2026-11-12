//! The `kiln` operator CLI's help, version and refusal surface, observed at the
//! process boundary: which stream receives the text and which status the process
//! returns.

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

/// Each case must write its report to standard error only and exit with its status.
fn assert_refused(cases: &[(&[&str], i32, &str)]) {
    for (arguments, status, report) in cases {
        let output = kiln(arguments);
        assert_eq!(output.status.code(), Some(*status), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        let stderr = text(&output.stderr);
        assert!(stderr.starts_with(report), "{arguments:?}: {stderr}");
    }
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
    assert_refused(&[
        (&[], EXIT_USAGE, USAGE_PREFIX),
        (&["--helpx"], EXIT_FAILURE, "unsupported command: --helpx"),
        (&["-help"], EXIT_FAILURE, "unsupported command: -help"),
        (&["--help", "demo"], EXIT_USAGE, USAGE_PREFIX),
        (&["help", "dashboard"], EXIT_USAGE, USAGE_PREFIX),
    ]);
}

// r[verify kiln.cli.version]
// r[verify kiln.cli.version.scenario.version]
#[test]
fn both_version_spellings_print_the_name_and_package_version() {
    let expected = format!("kiln {}\n", env!("CARGO_PKG_VERSION"));
    for spelling in ["--version", "-V"] {
        let output = kiln(&[spelling]);
        assert_eq!(output.status.code(), Some(EXIT_SUCCESS), "{spelling}");
        assert_eq!(text(&output.stdout), expected, "{spelling}");
        assert!(output.stderr.is_empty(), "{spelling}");
    }
    let usage = kiln(&["--help"]);
    assert!(
        text(&usage.stdout).contains("\n  --version, -V"),
        "usage does not name the version request"
    );
}

// r[verify kiln.cli.version.scenario.refused]
#[test]
fn a_version_request_takes_no_argument_and_has_no_other_spelling() {
    assert_refused(&[
        (&["--version", "demo"], EXIT_USAGE, USAGE_PREFIX),
        (&["-V", "--help"], EXIT_USAGE, USAGE_PREFIX),
        (&["-v"], EXIT_FAILURE, "unsupported command: -v"),
        (&["version"], EXIT_FAILURE, "unsupported command: version"),
        (&["--ver"], EXIT_FAILURE, "unsupported command: --ver"),
    ]);
}
