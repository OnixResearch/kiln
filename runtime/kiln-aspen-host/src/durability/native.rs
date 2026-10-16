use kiln_adapter_aspen::profile::AdmittedAspenRuntimeProfile;
use molten::fabric_durability::*;
use molten::system_extension::NATIVE_INSTANCE_STATE_SCHEMA;

use crate::cohort::role_ref;
use crate::error::{HostBridgeError, Result};

const JOURNAL_RECORDS_PER_OPERATION: u64 = 32;
const JOURNAL_LIFECYCLE_RECORD_ALLOWANCE: u64 = 64;
const NATIVE_JOURNAL_RECORD_MAXIMUM_BYTES: u64 = 1_048_576;

pub(super) fn profile(
    host_profile: &AdmittedAspenRuntimeProfile,
    cohort_identity: &str,
) -> Result<CanonicalDurableProfile> {
    let (maximum_log_records, maximum_namespace_bytes) = bounds(host_profile)?;
    let declaration_ref = role_ref(cohort_identity, "native-journal-profile");
    canonical_durable_profile(&DurableStateProfile {
        schema: DURABLE_STATE_PROFILE_SCHEMA.to_string(),
        profile_id: "kiln-aspen-native-journal-v1".to_string(),
        profile_ref: declaration_ref,
        adapter_kind: DurableAdapterKind::LiveRedb,
        supported_levels: vec![
            DurabilityLevel::Buffered,
            DurabilityLevel::ProcessLoss,
            DurabilityLevel::MachineLoss,
        ],
        max_namespaces: 1,
        max_log_records: maximum_log_records,
        max_ordered_entries: 1,
        max_operation_bytes: NATIVE_JOURNAL_RECORD_MAXIMUM_BYTES,
        max_namespace_bytes: maximum_namespace_bytes,
        max_batch_operations: maximum_log_records,
        max_snapshots: 1,
        max_effect_transactions: 1,
        non_claims: REQUIRED_DURABILITY_NON_CLAIMS.to_vec(),
    })
    .map_err(|error| HostBridgeError::Admission(format!("native journal profile: {error}")))
}

pub(super) fn descriptor(
    host_profile: &AdmittedAspenRuntimeProfile,
    cohort_identity: &str,
    durability_profile_ref: &str,
) -> Result<DurableNamespaceDescriptor> {
    let (maximum_log_records, maximum_namespace_bytes) = bounds(host_profile)?;
    let adapter_id = "kiln-aspen-native-journal".to_string();
    let namespace_id = "kiln-aspen-native-instances".to_string();
    Ok(DurableNamespaceDescriptor {
        schema: DURABLE_STATE_NAMESPACE_SCHEMA.to_string(),
        profile_ref: durability_profile_ref.to_string(),
        adapter_id: adapter_id.clone(),
        namespace_id: namespace_id.clone(),
        generation: host_profile.profile.generation,
        value_schema_ref: role_ref(cohort_identity, NATIVE_INSTANCE_STATE_SCHEMA),
        atomicity_domain: AtomicityDomain {
            domain_id: "kiln-aspen-native-journal-domain".to_string(),
            adapter_id,
            namespace_id,
            generation: host_profile.profile.generation,
            object_classes: vec![DurableObjectClass::LogRecord],
            max_operations: maximum_log_records,
            max_bytes: maximum_namespace_bytes,
            supported_levels: vec![DurabilityLevel::MachineLoss],
        },
        retention_authority_ref: Some(role_ref(cohort_identity, "native-journal-retention")),
        quota_bytes: maximum_namespace_bytes,
    })
}

fn bounds(profile: &AdmittedAspenRuntimeProfile) -> Result<(u64, u64)> {
    let maximum_records = u64::try_from(profile.profile.bounds.max_records).map_err(|_| {
        HostBridgeError::Admission("native journal record bound does not fit u64".to_string())
    })?;
    let maximum_log_records = maximum_records
        .checked_mul(JOURNAL_RECORDS_PER_OPERATION)
        .and_then(|value| value.checked_add(JOURNAL_LIFECYCLE_RECORD_ALLOWANCE))
        .ok_or_else(|| {
            HostBridgeError::Admission("native journal log bound overflow".to_string())
        })?;
    let maximum_namespace_bytes = maximum_log_records
        .checked_mul(NATIVE_JOURNAL_RECORD_MAXIMUM_BYTES)
        .ok_or_else(|| {
            HostBridgeError::Admission("native journal namespace bound overflow".to_string())
        })?;
    Ok((maximum_log_records, maximum_namespace_bytes))
}
