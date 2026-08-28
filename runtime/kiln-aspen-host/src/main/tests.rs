use super::*;

const TEST_MAXIMUM_REQUESTS: &str = "8";
const TEST_TIMEOUT_MILLISECONDS: &str = "30000";
const TEST_POLL_INTERVAL_MILLISECONDS: &str = "10";
const TEST_POLL_INTERVAL_VALUE: u64 = 10;
const ZERO_BOUND: &str = "0";
const MALFORMED_BOUND: &str = "ten";

fn complete_arguments(poll_interval: &str) -> Vec<String> {
    [
        "--aspen-profile",
        "aspen.json",
        "--radicle-profile",
        "radicle.json",
        "--socket",
        "/run/kiln/aspen.sock",
        "--extension",
        "/nix/store/extension",
        "--state-root",
        "/var/lib/kiln-aspen",
        "--max-requests",
        TEST_MAXIMUM_REQUESTS,
        "--timeout-ms",
        TEST_TIMEOUT_MILLISECONDS,
        "--poll-interval-ms",
        poll_interval,
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

#[test]
fn arguments_accept_every_path_and_positive_bound() {
    let result = parse_arguments(complete_arguments(TEST_POLL_INTERVAL_MILLISECONDS).into_iter());
    let Ok(arguments) = result else {
        unreachable!("complete positive host arguments must pass");
    };
    assert_eq!(
        arguments.poll_interval_milliseconds,
        TEST_POLL_INTERVAL_VALUE,
    );
    assert_eq!(
        arguments.maximum_requests.to_string(),
        TEST_MAXIMUM_REQUESTS
    );
}

// r[verify kiln.aspen_host.lattice_poll_pacing.rejected]
#[test]
fn poll_interval_rejects_missing_zero_malformed_and_duplicate_values() {
    let missing = parse_arguments(
        [
            "--aspen-profile",
            "aspen.json",
            "--radicle-profile",
            "radicle.json",
            "--socket",
            "/run/kiln/aspen.sock",
            "--extension",
            "/nix/store/extension",
            "--state-root",
            "/var/lib/kiln-aspen",
            "--max-requests",
            TEST_MAXIMUM_REQUESTS,
            "--timeout-ms",
            TEST_TIMEOUT_MILLISECONDS,
        ]
        .into_iter()
        .map(str::to_string),
    );
    assert!(missing.is_err());
    assert!(parse_arguments(complete_arguments(ZERO_BOUND).into_iter()).is_err());
    assert!(parse_arguments(complete_arguments(MALFORMED_BOUND).into_iter()).is_err());

    let mut duplicated = complete_arguments(TEST_POLL_INTERVAL_MILLISECONDS);
    duplicated.extend(
        ["--poll-interval-ms", TEST_POLL_INTERVAL_MILLISECONDS]
            .into_iter()
            .map(str::to_string),
    );
    assert!(parse_arguments(duplicated.into_iter()).is_err());
}

#[test]
fn request_bound_rejects_zero() {
    let arguments = complete_arguments(TEST_POLL_INTERVAL_MILLISECONDS)
        .into_iter()
        .map(|argument| {
            if argument == TEST_MAXIMUM_REQUESTS {
                ZERO_BOUND.to_string()
            } else {
                argument
            }
        });
    assert!(parse_arguments(arguments).is_err());
}
