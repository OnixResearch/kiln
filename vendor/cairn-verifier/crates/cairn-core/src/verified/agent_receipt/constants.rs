pub const AGENT_RUN_RECEIPT_SCHEMA_VERSION: &str = "cairn.agent_run_receipt.v1";
pub const NO_MUTATION_PLAN_HASH: &str = "cairn.no_mutation_plan.v1";

pub(super) const REQUIRED_RECEIPT_FIELDS: &[&str] = &[
    "schema_version",
    "task_id",
    "agent_identity",
    "input_hash",
    "plan_hash",
    "tool_manifest",
    "mutation_manifest",
    "verification_manifest",
    "nondeterminism_manifest",
    "replay_evidence",
    "verdict",
    "receipt_hash",
];

pub(super) const KNOWN_ENTROPY_CLASSES: &[&str] = &[
    "clock",
    "network",
    "model_provider",
    "environment",
    "random",
    "remote_api",
    "external_artifact",
];

pub(super) const SIDE_EFFECT_FIELDS: &[&str] = &[
    "files",
    "lifecycle",
    "network_writes",
    "secret_accesses",
    "deploys",
    "other_side_effects",
];
