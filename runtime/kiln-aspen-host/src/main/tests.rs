use super::*;

const TEST_MAXIMUM_REQUESTS: &str = "8";
const TEST_TIMEOUT_MILLISECONDS: &str = "30000";
const ZERO_REQUESTS: &str = "0";

#[test]
fn arguments_require_every_path_and_positive_bound() {
    let valid = parse_arguments(
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
    assert!(valid.is_ok());
    assert!(
        parse_arguments(
            ["--aspen-profile", "aspen.json"]
                .into_iter()
                .map(str::to_string),
        )
        .is_err(),
    );
    assert!(
        parse_arguments(
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
                ZERO_REQUESTS,
                "--timeout-ms",
                TEST_TIMEOUT_MILLISECONDS,
            ]
            .into_iter()
            .map(str::to_string),
        )
        .is_err(),
    );
}
