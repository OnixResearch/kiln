fn main() {
    let mut arguments = std::env::args().skip(1);
    let status = match arguments.next().as_deref() {
        Some("job") => kiln_web_service::worker::cli_main(arguments),
        Some(command @ ("serve" | "check")) => match (
            arguments.next().as_deref(),
            arguments.next(),
            arguments.next(),
        ) {
            (Some("--profile"), Some(path), None) => {
                let result = if command == "check" {
                    kiln_web_service::server::check(std::path::Path::new(&path))
                } else {
                    kiln_web_service::server::run(std::path::Path::new(&path))
                };
                match result {
                    Ok(()) => 0,
                    Err(message) => {
                        eprintln!("kiln-web-service: {message}");
                        1
                    }
                }
            }
            _ => usage(),
        },
        Some("reviewed-request") => reviewed_request(arguments),
        Some("--help" | "-h") | None => {
            usage();
            0
        }
        _ => usage(),
    };
    std::process::exit(status);
}

fn usage() -> i32 {
    eprintln!(
        "usage: kiln-web-service serve --profile <path>\n       kiln-web-service check --profile <path>\n       kiln-web-service job --profile <path> --identity <blake3>\n       kiln-web-service reviewed-request --profile <path> --project <id>"
    );
    2
}

fn reviewed_request(mut arguments: impl Iterator<Item = String>) -> i32 {
    let result = match (
        arguments.next().as_deref(),
        arguments.next(),
        arguments.next().as_deref(),
        arguments.next(),
        arguments.next(),
    ) {
        (Some("--profile"), Some(path), Some("--project"), Some(project), None) => {
            kiln_web_service::profile::ServiceProfile::load(std::path::Path::new(&path)).and_then(
                |profile| {
                    profile
                        .projects
                        .iter()
                        .find(|item| item.id == project)
                        .ok_or_else(|| "unknown installed project".to_owned())
                        .and_then(|project| project.reviewed_request())
                },
            )
        }
        _ => return usage(),
    };
    match result.and_then(|request| {
        serde_json::to_string_pretty(&request).map_err(|_| "request encoding failed".to_owned())
    }) {
        Ok(json) => {
            println!("{json}");
            0
        }
        Err(message) => {
            eprintln!("kiln-web-service: {message}");
            1
        }
    }
}
