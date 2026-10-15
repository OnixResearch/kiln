use kiln_core::types::{ContentId, CoreResult, Diagnostic, ExternalRef};

use crate::WORKER_PROTOCOL_VERSION;

// r[impl kiln.celld.partitioning]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionAssignment {
    pub partition: u64,
    pub object_name: String,
    pub object_identity: ContentId,
}

/// Deterministically map one stream to one named celld Durable Object using a
/// BLAKE3 partition window from the explicit versioned projection.
pub fn map_partition(
    stream: &ExternalRef,
    partition_count: usize,
) -> CoreResult<PartitionAssignment> {
    if partition_count == 0 {
        return Err(Diagnostic::new(
            "celld_partition_bound",
            "celld.partition.policy",
            "celld partition policy requires a positive partition count",
        ));
    }
    stream.validate()?;
    let window = format!(
        "{}|{}|{}",
        stream.provider_namespace.as_str(),
        stream.native_kind.as_str(),
        stream.native_id.as_str()
    );
    let digest = blake3::hash(window.as_bytes());
    let window_bytes: [u8; 8] = digest.as_bytes()[..8].try_into().map_err(|_| {
        Diagnostic::new(
            "celld_partition_window",
            "celld.partition.policy",
            "celld partition window is not exactly eight bytes",
        )
    })?;
    let count = u64::try_from(partition_count).map_err(|_| {
        Diagnostic::new(
            "celld_partition_bound",
            "celld.partition.policy",
            "celld partition count cannot be represented",
        )
    })?;
    let partition = u64::from_be_bytes(window_bytes) % count;
    let object_identity = ContentId::from_blake3_hex(digest.to_hex().as_str().to_string())
        .map_err(|_| {
            Diagnostic::new(
                "celld_object_identity",
                "celld.object_name",
                "celld object name BLAKE3 identity failed",
            )
        })?;
    let suffix: String = digest.to_hex().as_str().chars().take(16).collect();
    let object_name = format!("{WORKER_PROTOCOL_VERSION}:partition-{partition}:{suffix}");
    Ok(PartitionAssignment {
        partition,
        object_name,
        object_identity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream() -> ExternalRef {
        ExternalRef::new("kiln", "stream", "builds")
            .unwrap_or_else(|_| unreachable!("stream builds"))
    }

    #[test]
    // r[verify kiln.celld.partitioning.scenario.stable]
    fn maps_a_partition_deterministically() {
        let first = map_partition(&stream(), 16).unwrap_or_else(|_| unreachable!("maps"));
        let second = map_partition(&stream(), 16).unwrap_or_else(|_| unreachable!("maps"));
        assert_eq!(first, second);
        assert!(first.partition < 16);
        assert!(first.object_name.starts_with("1:partition-"));
    }

    #[test]
    fn maps_different_streams_within_the_partition_policy() {
        let other = ExternalRef::new("kiln", "stream", "deploys")
            .unwrap_or_else(|_| unreachable!("builds"));
        let first = map_partition(&stream(), 16).unwrap_or_else(|_| unreachable!("maps"));
        let second = map_partition(&other, 16).unwrap_or_else(|_| unreachable!("maps"));
        assert!(first.partition < 16 && second.partition < 16);
    }

    #[test]
    fn rejects_zero_partition_count() {
        let error = map_partition(&stream(), 0)
            .err()
            .unwrap_or_else(|| unreachable!("zero partition count must fail"));
        assert_eq!(error.code, "celld_partition_bound");
    }
}
