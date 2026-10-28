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
        Some("heartbeat") => kiln_web_service::worker::heartbeat_cli_main(arguments),
        Some("ci") => match kiln_web_service::developer::cli_main(&arguments.collect::<Vec<_>>()) {
            Ok(code) => code,
            Err(message) => {
                eprintln!("kiln ci: {message}");
                1
            }
        },
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
        Some("worker-target") => worker_target(arguments),
        Some("extend-projects") => extend_projects(arguments),
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
        "usage: kiln-web-service serve --profile <path> [--access <path>] [--automation <path>] [--cobs <path>] [--sources <path>] [--reviews <path>] [--cache <path>] [--workers <path>] [--work <path>] [--code-review <path>]\n       kiln-web-service check --profile <path> [--access <path>] [--automation <path>] [--cobs <path>] [--sources <path>] [--reviews <path>] [--cache <path>] [--workers <path>] [--work <path>] [--code-review <path>]\n       kiln-web-service proxy-auth --access <path>\n       kiln-web-service job --profile <path> --identity <blake3>\n       kiln-web-service heartbeat --profile <path> --identity <blake3>\n       kiln-web-service ci --profile <path> --identity <blake3> <submit|watch|logs|cancel|retry> [arguments]\n       kiln-web-service reviewed-request --profile <path> --project <id> [--nodes <id,id,...>]\n       kiln-web-service extend-projects --previous-profile <path> --profile <path>"
    );
    eprintln!(
        "       kiln-web-service worker-target --provider-profile <id> --socket <path> --platform <id>"
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

fn worker_target(mut arguments: impl Iterator<Item = String>) -> i32 {
    match (
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
    ) {
        (
            Some(provider_flag),
            Some(provider),
            Some(socket_flag),
            Some(socket),
            Some(platform_flag),
            Some(platform),
            None,
        ) if provider_flag == "--provider-profile"
            && socket_flag == "--socket"
            && platform_flag == "--platform" =>
        {
            match kiln_web_service::profile::worker_target_identity(
                &provider,
                std::path::Path::new(&socket),
                &platform,
            ) {
                Ok(identity) => {
                    println!("{}", identity.as_str());
                    0
                }
                Err(message) => {
                    eprintln!("kiln-web-service: {message}");
                    1
                }
            }
        }
        _ => usage(),
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
            "--sources" => &mut options.sources,
            "--reviews" => &mut options.reviews,
            "--cache" => &mut options.cache,
            "--workers" => &mut options.workers,
            "--work" => &mut options.work,
            "--code-review" => &mut options.code_review,
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

fn extend_projects(mut arguments: impl Iterator<Item = String>) -> i32 {
    let mut previous = None;
    let mut next = None;
    while let Some(flag) = arguments.next() {
        let slot = match flag.as_str() {
            "--previous-profile" => &mut previous,
            "--profile" => &mut next,
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
    let (Some(previous), Some(next)) = (previous, next) else {
        return usage();
    };
    status((|| {
        let previous =
            kiln_web_service::profile::ServiceProfile::load(std::path::Path::new(&previous))?;
        let next = kiln_web_service::profile::ServiceProfile::load(std::path::Path::new(&next))?;
        kiln_web_service::engine::extend_projects(&previous, &next)?;
        println!(
            "extended project registry: {} -> {} configured projects; enrollment unchanged",
            previous.projects.len(),
            next.projects.len()
        );
        Ok(())
    })())
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
