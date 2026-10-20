#[path = "../repo_index/mod.rs"]
mod repo_index;

fn main() {
    if let Err(error) = repo_index::run(std::env::args()) {
        eprintln!("repo-index: {error:#}");
        std::process::exit(1);
    }
}
