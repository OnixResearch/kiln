//! Offline inventory report over explicit local inputs. Never contacts a provider.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match kiln_web_service::inventory::run_cli_args(&args) {
        Ok(output) => println!("{output}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
