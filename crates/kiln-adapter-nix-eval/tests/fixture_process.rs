use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use bounded_exec::ExecutionLimits;
use kiln_adapter_nix_eval::{
    BoundedNixEvalRuntime, IncrementalDecoder, NixEvalLimits, NixEvalProcessOutput, NixEvalRuntime,
    ProcessInvocation,
};

const TIMEOUT_MILLISECONDS: u64 = 50;
const SIGNAL_TIMEOUT_MILLISECONDS: u64 = 1_000;
const POLL_INTERVAL_MILLISECONDS: u64 = 5;
const TEARDOWN_TIMEOUT_MILLISECONDS: u64 = 1_000;
const STDIN_MAX_BYTES: usize = 1;
const STDOUT_MAX_BYTES: usize = 4_096;
const STDERR_MAX_BYTES: usize = 4_096;
const FAILURE_STATUS: i32 = 19;
const RECORD_BOUND: usize = 8;
const JOB_BOUND: usize = 4;
const ERROR_BOUND: usize = 4;
const METADATA_BOUND: usize = 4_096;
const DIAGNOSTIC_BOUND: usize = 4_096;
const OUTPUT_BOUND: usize = 4;
const DEPENDENCY_BOUND: usize = 8;
const TASK_BOUND: usize = 4;
const GC_ROOT_BOUND: usize = 4;
const EXECUTABLE_BOUND: u64 = 268_435_456;
const MANIFEST_BOUND: u64 = 65_536;
const IN_FLIGHT_BOUND: usize = 4;
const WORKER_BOUND: usize = 4;
const MEMORY_BOUND_MEBIBYTES: u64 = 4_096;

fn invocation(mode: &str) -> ProcessInvocation {
    invocation_with_timeout(mode, TIMEOUT_MILLISECONDS)
}

fn invocation_with_timeout(mode: &str, timeout_milliseconds: u64) -> ProcessInvocation {
    ProcessInvocation {
        program: PathBuf::from(env!("CARGO_BIN_EXE_kiln-nix-eval-fixture")),
        args: vec![OsString::from(mode)],
        current_dir: PathBuf::from("/tmp"),
        environment: Vec::new(),
        limits: ExecutionLimits {
            timeout_ms: timeout_milliseconds,
            stdin_max_bytes: STDIN_MAX_BYTES,
            stdout_max_bytes: STDOUT_MAX_BYTES,
            stderr_max_bytes: STDERR_MAX_BYTES,
            poll_interval_ms: POLL_INTERVAL_MILLISECONDS,
            teardown_timeout_ms: TEARDOWN_TIMEOUT_MILLISECONDS,
        },
    }
}

fn stream_limits() -> NixEvalLimits {
    NixEvalLimits {
        max_line_bytes: STDOUT_MAX_BYTES,
        max_total_stdout_bytes: STDOUT_MAX_BYTES,
        max_stderr_bytes: STDERR_MAX_BYTES,
        max_records: RECORD_BOUND,
        max_jobs: JOB_BOUND,
        max_job_errors: ERROR_BOUND,
        max_metadata_bytes: METADATA_BOUND,
        max_diagnostic_bytes: DIAGNOSTIC_BOUND,
        max_outputs_per_job: OUTPUT_BOUND,
        max_dependencies_per_job: DEPENDENCY_BOUND,
        max_tasks: TASK_BOUND,
        max_gc_roots: GC_ROOT_BOUND,
        max_executable_bytes: EXECUTABLE_BOUND,
        max_manifest_bytes: MANIFEST_BOUND,
        max_in_flight: IN_FLIGHT_BOUND,
        max_workers: WORKER_BOUND,
        max_memory_mebibytes: MEMORY_BOUND_MEBIBYTES,
        stdin_max_bytes: STDIN_MAX_BYTES,
        timeout_milliseconds: TIMEOUT_MILLISECONDS,
        poll_interval_milliseconds: POLL_INTERVAL_MILLISECONDS,
        teardown_timeout_milliseconds: TEARDOWN_TIMEOUT_MILLISECONDS,
    }
}

fn stdout(output: NixEvalProcessOutput) -> Vec<u8> {
    match output {
        NixEvalProcessOutput::Exited { stdout, .. }
        | NixEvalProcessOutput::TimedOut { stdout, .. }
        | NixEvalProcessOutput::Cancelled { stdout, .. }
        | NixEvalProcessOutput::OutputLimitExceeded { stdout, .. } => stdout,
        NixEvalProcessOutput::Unknown => Vec::new(),
    }
}

// r[verify kiln.nix_eval_jobs.conformance]
// r[verify kiln.nix_eval_jobs.terminal_completeness]
#[test]
fn deterministic_fixture_processes_preserve_process_classes() {
    let mut runtime = BoundedNixEvalRuntime;
    let valid = runtime
        .run(&invocation("valid"))
        .unwrap_or_else(|_| unreachable!());
    assert!(matches!(
        valid,
        NixEvalProcessOutput::Exited {
            process_status: Some(0),
            ..
        }
    ));

    let crash = runtime
        .run(&invocation("crash"))
        .unwrap_or_else(|_| unreachable!());
    assert!(matches!(
        crash,
        NixEvalProcessOutput::Exited {
            process_status: Some(FAILURE_STATUS),
            ..
        }
    ));

    let signal = runtime
        .run(&invocation_with_timeout(
            "signal",
            SIGNAL_TIMEOUT_MILLISECONDS,
        ))
        .unwrap_or_else(|_| unreachable!());
    assert!(
        matches!(
            signal,
            NixEvalProcessOutput::Exited {
                process_status: None,
                signal: Some(_),
                ..
            } | NixEvalProcessOutput::Unknown
        ),
        "unexpected signal fixture outcome: {signal:?}"
    );

    let timed_out = runtime
        .run(&invocation("timeout"))
        .unwrap_or_else(|_| unreachable!());
    assert!(matches!(timed_out, NixEvalProcessOutput::TimedOut { .. }));

    let output_flood = runtime
        .run(&invocation("output-flood"))
        .unwrap_or_else(|_| unreachable!());
    assert!(matches!(
        output_flood,
        NixEvalProcessOutput::OutputLimitExceeded { .. }
    ));

    let stderr_flood = runtime
        .run(&invocation("stderr-flood"))
        .unwrap_or_else(|_| unreachable!());
    assert!(matches!(
        stderr_flood,
        NixEvalProcessOutput::OutputLimitExceeded { .. }
    ));
}

// r[verify kiln.nix_eval_jobs.stream_admission]
#[test]
fn fixture_stream_modes_reach_the_intended_schema_boundary() {
    let mut runtime = BoundedNixEvalRuntime;

    let job_error = stdout(
        runtime
            .run(&invocation("job-error"))
            .unwrap_or_else(|_| unreachable!()),
    );
    let mut decoder = IncrementalDecoder::new(stream_limits());
    decoder.push(&job_error).unwrap_or_else(|_| unreachable!());
    let admitted = decoder.finish().unwrap_or_else(|_| unreachable!());
    assert_eq!(admitted.errors.len(), 1);

    let malformed = stdout(
        runtime
            .run(&invocation("malformed"))
            .unwrap_or_else(|_| unreachable!()),
    );
    let mut decoder = IncrementalDecoder::new(stream_limits());
    assert!(decoder.push(&malformed).is_err());

    let truncated = stdout(
        runtime
            .run(&invocation("truncated"))
            .unwrap_or_else(|_| unreachable!()),
    );
    let mut decoder = IncrementalDecoder::new(stream_limits());
    decoder.push(&truncated).unwrap_or_else(|_| unreachable!());
    assert!(decoder.finish().is_err());
}

// r[verify kiln.nix_eval_jobs.failure_scope]
#[test]
fn cancellation_late_output_and_descendants_finish_under_owned_teardown() {
    let mut runtime = BoundedNixEvalRuntime;
    let cancellation = AtomicBool::new(true);
    let cancelled = runtime
        .run_with_cancellation(&invocation("timeout"), &cancellation)
        .unwrap_or_else(|_| unreachable!());
    assert!(matches!(cancelled, NixEvalProcessOutput::Cancelled { .. }));

    let late_output = runtime
        .run(&invocation("late-output"))
        .unwrap_or_else(|_| unreachable!());
    assert!(matches!(late_output, NixEvalProcessOutput::TimedOut { .. }));

    let descendant = runtime
        .run(&invocation("descendant"))
        .unwrap_or_else(|_| unreachable!());
    assert!(matches!(
        descendant,
        NixEvalProcessOutput::Exited {
            process_status: Some(0),
            ..
        }
    ));
}
