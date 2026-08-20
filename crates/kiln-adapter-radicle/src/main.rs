use std::io::{Read, Write};

use kiln_adapter_radicle::run_protocol;
use kiln_core::types::RunId;
use kiln_provider_api::{
    PortFailure, PortResult, TriggerIntakePort, TriggerLifecyclePolicy, TriggerRequest, TriggerRun,
    TriggerTerminal,
};

const INPUT_OUTPUT_EXIT_CODE: i32 = 1;
const PROTOCOL_EXIT_CODE: i32 = 2;

// r[impl kiln.providers.composition_root]
struct LocalFixtureService;

impl TriggerIntakePort for LocalFixtureService {
    fn accept(&mut self, _trigger: TriggerRequest) -> PortResult<TriggerRun> {
        Ok(TriggerRun {
            run_id: RunId::new("kiln-local-reference-run")?,
            terminal: TriggerTerminal::Success,
            log_url: None,
        })
    }

    fn observe(
        &mut self,
        accepted: &TriggerRun,
        _policy: TriggerLifecyclePolicy,
    ) -> PortResult<TriggerRun> {
        Ok(accepted.clone())
    }
}

fn report_error(error: &PortFailure) {
    match error {
        PortFailure::Domain(diagnostic) => {
            eprintln!("{}: {}", diagnostic.code, diagnostic.message);
        }
        PortFailure::Infrastructure(failure)
        | PortFailure::Retryable(failure)
        | PortFailure::TimedOut(failure)
        | PortFailure::Cancelled(failure)
        | PortFailure::Unknown(failure) => {
            eprintln!("{}: {}", failure.code, failure.message);
        }
    }
}

fn main() {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        eprintln!("radicle broker input read failed");
        std::process::exit(INPUT_OUTPUT_EXIT_CODE);
    }
    let mut service = LocalFixtureService;
    match run_protocol(&input, &mut service) {
        Ok(lines) => {
            let output = format!("{}\n", lines.join("\n"));
            if std::io::stdout().write_all(output.as_bytes()).is_err() {
                eprintln!("radicle broker output write failed");
                std::process::exit(INPUT_OUTPUT_EXIT_CODE);
            }
        }
        Err(error) => {
            report_error(&error);
            std::process::exit(PROTOCOL_EXIT_CODE);
        }
    }
}
