use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use kiln_adapter_nix_eval::{
    BoundedNixEvalRuntime, NixEvalProcessOutput, NixEvalRuntime, ProcessInvocation,
};
use kiln_core::types::{ContentId, ExternalRef};
use kiln_provider_api::{InfrastructureFailure, PortFailure, PortResult};

/// Retains the actual bounded process bytes before the adapter normalizes them.
#[derive(Debug)]
pub(crate) struct RetainingEvalRuntime {
    pub root: PathBuf,
}

impl RetainingEvalRuntime {
    fn retain(&self, output: &NixEvalProcessOutput) -> PortResult<()> {
        let (stdout, stderr) = match output {
            NixEvalProcessOutput::Exited { stdout, stderr, .. }
            | NixEvalProcessOutput::TimedOut { stdout, stderr }
            | NixEvalProcessOutput::Cancelled { stdout, stderr }
            | NixEvalProcessOutput::OutputLimitExceeded { stdout, stderr } => {
                (stdout.as_slice(), stderr.as_slice())
            }
            NixEvalProcessOutput::Unknown => (&[][..], &[][..]),
        };
        crate::store_bytes(&self.root.join("evaluator.stdout.log"), stdout)
            .and_then(|()| crate::store_bytes(&self.root.join("evaluator.stderr.log"), stderr))
            .map_err(|message| {
                PortFailure::Infrastructure(InfrastructureFailure {
                    code: "pilot_evaluator_evidence".to_string(),
                    message,
                })
            })
    }
}

impl NixEvalRuntime for RetainingEvalRuntime {
    fn file_identity(&self, path: &Path, max_bytes: u64) -> PortResult<ContentId> {
        BoundedNixEvalRuntime.file_identity(path, max_bytes)
    }
    fn read_file(&self, path: &Path, max_bytes: u64) -> PortResult<Vec<u8>> {
        BoundedNixEvalRuntime.read_file(path, max_bytes)
    }
    fn file_identity_no_follow(&self, path: &Path, max_bytes: u64) -> PortResult<ContentId> {
        BoundedNixEvalRuntime.file_identity_no_follow(path, max_bytes)
    }
    fn read_file_no_follow(&self, path: &Path, max_bytes: u64) -> PortResult<Vec<u8>> {
        BoundedNixEvalRuntime.read_file_no_follow(path, max_bytes)
    }
    fn prepare_gc_root_directory(&mut self, path: &Path) -> PortResult<()> {
        BoundedNixEvalRuntime.prepare_gc_root_directory(path)
    }
    fn list_gc_roots(&self, path: &Path, max_roots: usize) -> PortResult<Vec<ExternalRef>> {
        BoundedNixEvalRuntime.list_gc_roots(path, max_roots)
    }
    fn cleanup_gc_root_directory(&mut self, path: &Path, max_roots: usize) -> PortResult<()> {
        BoundedNixEvalRuntime.cleanup_gc_root_directory(path, max_roots)
    }
    fn run(&mut self, invocation: &ProcessInvocation) -> PortResult<NixEvalProcessOutput> {
        let output = BoundedNixEvalRuntime.run(invocation)?;
        self.retain(&output)?;
        Ok(output)
    }
    fn run_with_cancellation(
        &mut self,
        invocation: &ProcessInvocation,
        cancellation: &AtomicBool,
    ) -> PortResult<NixEvalProcessOutput> {
        let output = BoundedNixEvalRuntime.run_with_cancellation(invocation, cancellation)?;
        self.retain(&output)?;
        Ok(output)
    }
}
