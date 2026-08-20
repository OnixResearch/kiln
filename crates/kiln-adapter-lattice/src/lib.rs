mod adapter;
mod mapping;
mod profile;
#[cfg(target_os = "linux")]
mod unix;

pub use adapter::LatticeAdapter;
pub use mapping::{accepted_observation, admitted_observation, lower_request};
pub use profile::{
    LatticeAdapterLimits, LatticeProfile, PINNED_LATTICE_CONTRACT_LICENSE,
    PINNED_LATTICE_CONTRACT_SCHEMA, PINNED_LATTICE_GRAPH_SCHEMA, PINNED_LATTICE_OBSERVATION_SCHEMA,
    PINNED_LATTICE_PLATFORM, PINNED_LATTICE_REVISION, PINNED_LATTICE_SOURCE, admit_profile,
};
#[cfg(target_os = "linux")]
pub use unix::UnixWorkflowExchange;

#[cfg(test)]
mod tests;
