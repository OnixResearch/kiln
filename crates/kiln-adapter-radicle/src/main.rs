use std::io::{Read, Write};

use kiln_adapter_radicle::{BrokerRun, BrokerService, BrokerTerminal, MappedTrigger, run_protocol};
use kiln_core::types::{CoreResult, RunId};

struct LocalFixtureService;

impl BrokerService for LocalFixtureService {
    fn accept(&mut self, _trigger: MappedTrigger) -> CoreResult<BrokerRun> {
        Ok(BrokerRun {
            run_id: RunId::new("kiln-local-reference-run")?,
            terminal: BrokerTerminal::Success,
            log_url: None,
        })
    }
}

fn main() {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        eprintln!("radicle broker input read failed");
        std::process::exit(1);
    }
    let mut service = LocalFixtureService;
    match run_protocol(&input, &mut service) {
        Ok(lines) => {
            let output = format!("{}\n", lines.join("\n"));
            if std::io::stdout().write_all(output.as_bytes()).is_err() {
                eprintln!("radicle broker output write failed");
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("{}: {}", error.code, error.message);
            std::process::exit(2);
        }
    }
}
