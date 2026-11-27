//! Optional off-by-default Slurm batch-cluster adapter for Kiln's
//! `RemoteExecutionPort` and `RemoteLifecyclePort`.
//!
//! Kiln retains CI domain meaning, tenant admission, deterministic BLAKE3
//! correlation, and outcome classification. Slurm retains node placement,
//! queuing, and cluster execution. Slurm job IDs are preserved strictly as
//! explicit provider `ExternalRef` facts and never replace Kiln identities.

pub mod adapter;
pub mod protocol;

pub use adapter::SlurmAdapter;
pub use protocol::{
    ParsedSbatchOutput, SLURM_ACTION_NATIVE_KIND, SLURM_CORRELATION_DOMAIN, SLURM_JOB_NATIVE_KIND,
    SLURM_PROFILE_SCHEMA, SLURM_PROTOCOL_VERSION, SLURM_PROVIDER_NAMESPACE, SlurmCorrelation,
    SlurmExecutablePaths, SlurmJobPhase, SlurmJobRecord, SlurmPolicyBounds, SlurmProfile,
    SlurmRequestedFeature, SlurmResources, SlurmSubmissionSpec, admit_profile, admit_submission,
    build_sbatch_args, build_scancel_args, build_squeue_args, classify_slurm_state,
    parse_sacct_output, parse_sbatch_output, parse_squeue_output, validate_profile_structure,
    validate_slurm_operation_ref,
};
