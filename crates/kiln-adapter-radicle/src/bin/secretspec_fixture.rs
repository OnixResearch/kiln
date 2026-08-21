use std::process::ExitCode;

const PRIMARY_SEED: &str = "0101010101010101010101010101010101010101010101010101010101010101";
const SECONDARY_SEED: &str = "0202020202020202020202020202020202020202020202020202020202020202";
const SUCCESS: u8 = 0;
const FAILURE: u8 = 1;
const EXPECTED_MINIMUM_ARGUMENTS: usize = 6;
const OPTION_PAIR_LENGTH: usize = 2;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.len() < EXPECTED_MINIMUM_ARGUMENTS
        || arguments.first().map(String::as_str) != Some("get")
    {
        return ExitCode::from(FAILURE);
    }
    let provider = option_value(&arguments, "--provider");
    match provider {
        Some("fixture-missing") => ExitCode::from(FAILURE),
        Some("fixture-secondary") => {
            println!("{SECONDARY_SEED}");
            ExitCode::from(SUCCESS)
        }
        _ => {
            println!("{PRIMARY_SEED}");
            ExitCode::from(SUCCESS)
        }
    }
}

fn option_value<'a>(arguments: &'a [String], option: &str) -> Option<&'a str> {
    arguments
        .windows(OPTION_PAIR_LENGTH)
        .find(|pair| pair.first().map(String::as_str) == Some(option))
        .and_then(|pair| pair.get(1))
        .map(String::as_str)
}
