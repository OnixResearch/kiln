#![no_std]

// r[impl kiln.radicle.source_contract.boundary]
extern crate alloc;

mod admission;
mod model;

pub use admission::{AdmissionError, admit_receipt, admit_request};
pub use model::{
    CONTRACT_SCHEMA_VERSION, FeatureLevel, SOURCE_NON_CLAIMS, SignedRefsObservation,
    SnapshotEvidence, SnapshotLimits, SourceReceipt, SourceRequest, Verification, fixture_receipt,
    fixture_request,
};

#[cfg(test)]
mod tests;
