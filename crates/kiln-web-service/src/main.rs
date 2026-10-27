fn main() {
    // The streaming helper must retain non-UTF-8 reviewed command arguments.
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("stream-child")) {
        std::process::exit(kiln_web_service::worker::stream_child_main(
            std::env::args_os().skip(2),
        ));
    }
    let mut arguments = std::env::args().skip(1);
    let status = match arguments.next().as_deref() {
        Some("job") => kiln_web_service::worker::cli_main(arguments),
        Some(command @ ("serve" | "check")) => service(command, arguments),
        Some("proxy-auth") => {
            match (
                arguments.next().as_deref(),
                arguments.next(),
                arguments.next(),
            ) {
                (Some("--access"), Some(path), None) => status(
                    kiln_web_service::server::proxy_auth(std::path::Path::new(&path)),
                ),
                _ => usage(),
            }
        }
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
        "usage: kiln-web-service serve --profile <path> [--access <path>] [--automation <path>] [--cobs <path>]\n       kiln-web-service check --profile <path> [--access <path>] [--automation <path>] [--cobs <path>]\n       kiln-web-service proxy-auth --access <path>\n       kiln-web-service job --profile <path> --identity <blake3>\n       kiln-web-service reviewed-request --profile <path> --project <id> [--nodes <id,id,...>]"
    );
    2
}

fn status(result: Result<(), String>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("kiln-web-service: {message}");
            1
        }
    }
}

fn service(command: &str, mut arguments: impl Iterator<Item = String>) -> i32 {
    let mut profile = None;
    let mut options = kiln_web_service::server::Options::default();
    while let Some(flag) = arguments.next() {
        let slot = match flag.as_str() {
            "--profile" => &mut profile,
            "--access" => &mut options.access,
            "--automation" => &mut options.automation,
            "--cobs" => &mut options.cobs,
            _ => return usage(),
        };
        if slot.is_some() {
            return usage();
        }
        let Some(value) = arguments.next().filter(|value| !value.starts_with("--")) else {
            return usage();
        };
        *slot = Some(std::path::PathBuf::from(value));
    }
    let Some(profile) = profile else {
        return usage();
    };
    status(if command == "check" {
        kiln_web_service::server::check_with_options(&profile, &options)
    } else {
        kiln_web_service::server::run_with_options(&profile, &options)
    })
}

fn reviewed_request(mut arguments: impl Iterator<Item = String>) -> i32 {
    let mut path = None;
    let mut project_id = None;
    let mut selected = None;
    while let Some(flag) = arguments.next() {
        let slot = match flag.as_str() {
            "--profile" => &mut path,
            "--project" => &mut project_id,
            "--nodes" => &mut selected,
            _ => return usage(),
        };
        if slot.is_some() {
            return usage();
        }
        let Some(value) = arguments.next().filter(|value| !value.starts_with("--")) else {
            return usage();
        };
        *slot = Some(value);
    }
    let (Some(path), Some(project_id)) = (path, project_id) else {
        return usage();
    };
    let nodes = selected.map(|value| value.split(',').map(str::to_owned).collect::<Vec<_>>());
    if nodes
        .as_ref()
        .is_some_and(|nodes| nodes.iter().any(String::is_empty))
    {
        return usage();
    }
    let result = kiln_web_service::profile::ServiceProfile::load(std::path::Path::new(&path))
        .and_then(|profile| {
            let project = profile
                .projects
                .iter()
                .find(|item| item.id == project_id)
                .ok_or_else(|| "unknown installed project".to_owned())?;
            match nodes {
                Some(nodes) => project.reviewed_request_for_nodes(&nodes),
                None => project.reviewed_request(),
            }
        });
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
