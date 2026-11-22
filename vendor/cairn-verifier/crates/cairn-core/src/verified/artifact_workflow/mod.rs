mod admission;
mod model;
mod outcome_machine;
mod profile;
mod status;

pub use admission::{
    admit_workflow_profile_registry, admit_workflow_schemas, workflow_profile_registry_from_value,
    workflow_schemas_from_value, workflow_topological_order,
};
pub use model::*;
pub use outcome_machine::{evaluate_outcome_machine_review, outcome_machine_evidence_paths};
pub use profile::{
    evaluate_workflow_profile, parse_acceptance_ids, profile_issue_applies_to_stage,
    select_workflow_profile, synthetic_acceptance_spec, tasks_with_requirement_links,
    workflow_change_profile_metadata_from_value,
};
pub use status::{
    build_workflow_instructions, derive_workflow_status, workflow_markdown_is_structurally_present,
};

#[cfg(test)]
mod tests;
