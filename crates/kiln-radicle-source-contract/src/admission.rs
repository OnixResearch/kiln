use alloc::format;
use core::fmt;

use crate::model::{
    CONTRACT_SCHEMA_VERSION, FeatureLevel, SnapshotLimits, SourceReceipt, SourceRequest,
    Verification,
};

const SHA1_HEX_LENGTH: usize = 40;
const BLAKE3_HEX_LENGTH: usize = 64;
const RADICLE_RID_PREFIX: &str = "rad:z";
const BRANCH_REFERENCE_PREFIX: &str = "refs/heads/";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionError {
    Version { field: &'static str },
    Empty { field: &'static str },
    IdentityBound { field: &'static str },
    Reference { field: &'static str },
    Oid { field: &'static str },
    Digest { field: &'static str },
    ZeroBound { field: &'static str },
    InvalidBound { field: &'static str },
    Linkage { field: &'static str },
    FeatureLevel,
    Verification { field: &'static str },
    MovedSigrefs,
    SnapshotBound { field: &'static str },
}

impl AdmissionError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Version { .. } => "source_contract_version",
            Self::Empty { .. } => "source_contract_empty",
            Self::IdentityBound { .. } => "source_contract_identity_bound",
            Self::Reference { .. } => "source_contract_reference",
            Self::Oid { .. } => "source_contract_oid",
            Self::Digest { .. } => "source_contract_digest",
            Self::ZeroBound { .. } => "source_contract_zero_bound",
            Self::InvalidBound { .. } => "source_contract_invalid_bound",
            Self::Linkage { .. } => "source_contract_linkage",
            Self::FeatureLevel => "source_contract_feature_level",
            Self::Verification { .. } => "source_contract_verification",
            Self::MovedSigrefs => "source_contract_moved_sigrefs",
            Self::SnapshotBound { .. } => "source_contract_snapshot_bound",
        }
    }

    #[must_use]
    pub const fn field(&self) -> &'static str {
        match self {
            Self::Version { field }
            | Self::Empty { field }
            | Self::IdentityBound { field }
            | Self::Reference { field }
            | Self::Oid { field }
            | Self::Digest { field }
            | Self::ZeroBound { field }
            | Self::InvalidBound { field }
            | Self::Linkage { field }
            | Self::Verification { field }
            | Self::SnapshotBound { field } => field,
            Self::FeatureLevel => "observation.feature_level",
            Self::MovedSigrefs => "observation.sigrefs_commit_after",
        }
    }
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at {}", self.code(), self.field())
    }
}

fn admit_version(version: u16, field: &'static str) -> Result<(), AdmissionError> {
    if version == CONTRACT_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(AdmissionError::Version { field })
    }
}

fn admit_text(value: &str, maximum: u64, field: &'static str) -> Result<(), AdmissionError> {
    if value.is_empty() || value.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(AdmissionError::Empty { field });
    }
    let length = u64::try_from(value.len()).map_err(|_| AdmissionError::IdentityBound { field })?;
    if length > maximum {
        return Err(AdmissionError::IdentityBound { field });
    }
    Ok(())
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn admit_oid(value: &str, field: &'static str) -> Result<(), AdmissionError> {
    if is_lower_hex(value, SHA1_HEX_LENGTH) {
        Ok(())
    } else {
        Err(AdmissionError::Oid { field })
    }
}

fn admit_digest(value: &str, field: &'static str) -> Result<(), AdmissionError> {
    if is_lower_hex(value, BLAKE3_HEX_LENGTH) {
        Ok(())
    } else {
        Err(AdmissionError::Digest { field })
    }
}

fn admit_limits(limits: &SnapshotLimits) -> Result<(), AdmissionError> {
    for (value, field) in [
        (
            limits.max_identity_bytes,
            "request.limits.max_identity_bytes",
        ),
        (
            limits.max_reference_bytes,
            "request.limits.max_reference_bytes",
        ),
        (limits.max_entries, "request.limits.max_entries"),
        (limits.max_path_bytes, "request.limits.max_path_bytes"),
        (limits.max_file_bytes, "request.limits.max_file_bytes"),
        (limits.max_total_bytes, "request.limits.max_total_bytes"),
    ] {
        if value == 0 {
            return Err(AdmissionError::ZeroBound { field });
        }
    }
    if limits.max_file_bytes > limits.max_total_bytes {
        return Err(AdmissionError::InvalidBound {
            field: "request.limits.max_file_bytes",
        });
    }
    Ok(())
}

// r[impl kiln.radicle.source_contract.request]
pub fn admit_request(request: &SourceRequest) -> Result<(), AdmissionError> {
    admit_version(request.schema_version, "request.schema_version")?;
    admit_limits(&request.limits)?;
    admit_text(
        &request.profile_id,
        request.limits.max_identity_bytes,
        "request.profile_id",
    )?;
    admit_text(
        &request.repository_id,
        request.limits.max_identity_bytes,
        "request.repository_id",
    )?;
    if !request.repository_id.starts_with(RADICLE_RID_PREFIX) {
        return Err(AdmissionError::Reference {
            field: "request.repository_id",
        });
    }
    admit_text(
        &request.signer_node_id,
        request.limits.max_identity_bytes,
        "request.signer_node_id",
    )?;
    admit_text(
        &request.signed_reference,
        request.limits.max_reference_bytes,
        "request.signed_reference",
    )?;
    if !request
        .signed_reference
        .starts_with(BRANCH_REFERENCE_PREFIX)
        || request.signed_reference.contains("..")
        || request.signed_reference.contains('\\')
    {
        return Err(AdmissionError::Reference {
            field: "request.signed_reference",
        });
    }
    admit_oid(
        &request.expected_sigrefs_commit,
        "request.expected_sigrefs_commit",
    )?;
    admit_oid(&request.expected_revision, "request.expected_revision")?;
    Ok(())
}

fn require_link(equal: bool, field: &'static str) -> Result<(), AdmissionError> {
    if equal {
        Ok(())
    } else {
        Err(AdmissionError::Linkage { field })
    }
}

fn require_verified(value: Verification, field: &'static str) -> Result<(), AdmissionError> {
    if value == Verification::Verified {
        Ok(())
    } else {
        Err(AdmissionError::Verification { field })
    }
}

// r[impl kiln.radicle.source_contract.sigrefs]
// r[impl kiln.radicle.source_contract.snapshot]
// r[impl kiln.radicle.source_contract.linkage]
pub fn admit_receipt(receipt: &SourceReceipt) -> Result<(), AdmissionError> {
    admit_version(receipt.schema_version, "receipt.schema_version")?;
    admit_request(&receipt.request)?;
    let request = &receipt.request;
    let observation = &receipt.observation;
    let snapshot = &receipt.snapshot;

    admit_version(observation.schema_version, "observation.schema_version")?;
    admit_version(snapshot.schema_version, "snapshot.schema_version")?;
    require_link(
        observation.profile_id == request.profile_id,
        "observation.profile_id",
    )?;
    require_link(
        observation.repository_id == request.repository_id,
        "observation.repository_id",
    )?;
    require_link(
        observation.signer_node_id == request.signer_node_id,
        "observation.signer_node_id",
    )?;
    require_link(
        observation.signed_reference == request.signed_reference,
        "observation.signed_reference",
    )?;

    let expected_sigrefs_reference = format!(
        "refs/namespaces/{}/refs/rad/sigrefs",
        request.signer_node_id
    );
    require_link(
        observation.namespaced_sigrefs_reference == expected_sigrefs_reference,
        "observation.namespaced_sigrefs_reference",
    )?;
    admit_text(
        &observation.namespaced_sigrefs_reference,
        request.limits.max_reference_bytes,
        "observation.namespaced_sigrefs_reference",
    )?;

    admit_oid(
        &observation.sigrefs_commit_before,
        "observation.sigrefs_commit_before",
    )?;
    admit_oid(
        &observation.sigrefs_commit_after,
        "observation.sigrefs_commit_after",
    )?;
    if observation.sigrefs_commit_before != observation.sigrefs_commit_after {
        return Err(AdmissionError::MovedSigrefs);
    }
    require_link(
        observation.sigrefs_commit_before == request.expected_sigrefs_commit,
        "observation.sigrefs_commit_before",
    )?;
    if let Some(parent) = &observation.sigrefs_parent {
        admit_oid(parent, "observation.sigrefs_parent")?;
    }
    admit_oid(
        &observation.identity_root_commit,
        "observation.identity_root_commit",
    )?;
    admit_oid(&observation.signed_revision, "observation.signed_revision")?;
    admit_oid(
        &observation.namespaced_revision,
        "observation.namespaced_revision",
    )?;
    require_link(
        observation.signed_revision == request.expected_revision,
        "observation.signed_revision",
    )?;
    require_link(
        observation.namespaced_revision == request.expected_revision,
        "observation.namespaced_revision",
    )?;
    if observation.feature_level != FeatureLevel::Parent {
        return Err(AdmissionError::FeatureLevel);
    }
    require_verified(observation.signature, "observation.signature")?;
    require_verified(
        observation.repository_identity,
        "observation.repository_identity",
    )?;
    require_verified(observation.namespace_target, "observation.namespace_target")?;

    admit_oid(&snapshot.revision, "snapshot.revision")?;
    require_link(
        snapshot.revision == request.expected_revision,
        "snapshot.revision",
    )?;
    admit_oid(&snapshot.native_tree, "snapshot.native_tree")?;
    admit_digest(
        &snapshot.canonical_tree_blake3,
        "snapshot.canonical_tree_blake3",
    )?;
    if snapshot.entry_count > request.limits.max_entries {
        return Err(AdmissionError::SnapshotBound {
            field: "snapshot.entry_count",
        });
    }
    if snapshot.total_bytes > request.limits.max_total_bytes {
        return Err(AdmissionError::SnapshotBound {
            field: "snapshot.total_bytes",
        });
    }
    Ok(())
}
