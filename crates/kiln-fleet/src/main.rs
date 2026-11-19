//! `kiln-fleet` command line.

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(kiln_fleet::cli::main_with(&arguments));
}
