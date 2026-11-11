//! Pure admission for one VM-specific Iroh migration bridge.
//!
//! The bridge carries Cloud Hypervisor migration bytes between two protected
//! local Unix sockets. This module binds the operation to authenticated endpoint
//! identities, exact placements, one checkpoint manifest, one protocol
//! generation and bounded stream resources. It performs no I/O.

use serde::{Deserialize, Serialize};

use crate::retained::Placement;
use kiln_core::types::{ContentId, Diagnostic};

pub const BRIDGE_SCHEMA: &str = "kiln.migration-bridge.v1";
pub const MAX_SOCKET_BYTES: usize = 256;
pub const MAX_STREAM_BYTES: u64 = 1 << 40;
pub const MAX_DEADLINE_MS: u64 = 24 * 60 * 60 * 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BridgeEndpoint {
    /// Authenticated Iroh endpoint public key.
    pub id: [u8; 32],
}

impl BridgeEndpoint {
    pub fn new(id: [u8; 32]) -> Result<Self, Diagnostic> {
        if id.iter().all(|byte| *byte == 0) {
            return Err(refusal(
                "bridge_endpoint",
                "bridge.endpoint",
                "bridge endpoint identity must be nonzero",
            ));
        }
        Ok(Self { id })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryMode {
    /// The only admitted initial mode. Postcopy and memfd modes are refused.
    Precopy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationBridgePlan {
    pub schema: String,
    pub operation: ContentId,
    pub session: ContentId,
    pub attempt: ContentId,
    pub fence: u64,
    pub checkpoint: ContentId,
    pub source: Placement,
    pub target: Placement,
    pub source_endpoint: BridgeEndpoint,
    pub target_endpoint: BridgeEndpoint,
    pub source_socket: String,
    pub target_socket: String,
    pub protocol_generation: u16,
    pub memory_mode: MemoryMode,
    pub max_connections: u8,
    pub max_stream_bytes: u64,
    pub deadline_ms: u64,
}

/// Independently resolved facts the bridge must match exactly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeExpectation {
    pub operation: ContentId,
    pub session: ContentId,
    pub attempt: ContentId,
    pub fence: u64,
    pub checkpoint: ContentId,
    pub source: Placement,
    pub target: Placement,
    pub source_endpoint: BridgeEndpoint,
    pub target_endpoint: BridgeEndpoint,
    pub source_socket: String,
    pub target_socket: String,
    pub protocol_generation: u16,
}

/// Admit one bridge plan against exact current facts.
// r[impl kiln.hosted.bridge.plan]
pub fn admit_bridge(
    plan: &MigrationBridgePlan,
    expected: &BridgeExpectation,
) -> Result<ContentId, Diagnostic> {
    if plan.schema != BRIDGE_SCHEMA
        || plan.operation != expected.operation
        || plan.session != expected.session
        || plan.attempt != expected.attempt
        || plan.fence != expected.fence
        || plan.checkpoint != expected.checkpoint
        || plan.source != expected.source
        || plan.target != expected.target
        || plan.source_endpoint != expected.source_endpoint
        || plan.target_endpoint != expected.target_endpoint
        || plan.source_socket != expected.source_socket
        || plan.target_socket != expected.target_socket
        || plan.protocol_generation != expected.protocol_generation
        || plan.source_endpoint == plan.target_endpoint
        || plan.memory_mode != MemoryMode::Precopy
        || plan.max_connections != 1
        || plan.max_stream_bytes == 0
        || plan.max_stream_bytes > MAX_STREAM_BYTES
        || plan.deadline_ms == 0
        || plan.deadline_ms > MAX_DEADLINE_MS
        || plan.protocol_generation == 0
        || plan.source.host == plan.target.host
        || plan.target.generation <= plan.source.generation
    {
        return Err(refusal(
            "bridge_binding",
            "bridge.plan",
            "bridge plan does not bind the exact operation, endpoints, placements or bounds",
        ));
    }
    for identity in [
        &plan.operation,
        &plan.session,
        &plan.attempt,
        &plan.checkpoint,
    ] {
        identity.validate()?;
    }
    validate_socket(&plan.source_socket, "bridge.source_socket")?;
    validate_socket(&plan.target_socket, "bridge.target_socket")?;
    let mut bytes = Vec::new();
    for value in [
        BRIDGE_SCHEMA,
        plan.operation.as_str(),
        plan.session.as_str(),
    ] {
        push(&mut bytes, value)?;
    }
    push(&mut bytes, plan.attempt.as_str())?;
    bytes.extend_from_slice(&plan.fence.to_be_bytes());
    push(&mut bytes, plan.checkpoint.as_str())?;
    for placement in [&plan.source, &plan.target] {
        push(&mut bytes, &placement.host)?;
        bytes.extend_from_slice(&placement.generation.to_be_bytes());
    }
    bytes.extend_from_slice(&plan.source_endpoint.id);
    bytes.extend_from_slice(&plan.target_endpoint.id);
    push(&mut bytes, &plan.source_socket)?;
    push(&mut bytes, &plan.target_socket)?;
    bytes.extend_from_slice(&plan.protocol_generation.to_be_bytes());
    bytes.push(1);
    bytes.push(plan.max_connections);
    bytes.extend_from_slice(&plan.max_stream_bytes.to_be_bytes());
    bytes.extend_from_slice(&plan.deadline_ms.to_be_bytes());
    ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string())
}

fn validate_socket(path: &str, field: &str) -> Result<(), Diagnostic> {
    if path.is_empty()
        || path.len() > MAX_SOCKET_BYTES
        || !path.starts_with('/')
        || path.chars().any(char::is_control)
        || path.split('/').any(|part| part == "..")
    {
        return Err(refusal(
            "bridge_socket",
            field,
            "bridge socket must be an absolute bounded non-escaping path",
        ));
    }
    Ok(())
}

fn push(bytes: &mut Vec<u8>, value: &str) -> Result<(), Diagnostic> {
    let length = u32::try_from(value.len()).map_err(|_| {
        refusal(
            "bridge_encoding",
            "bridge.plan",
            "bridge field exceeds its encoding bound",
        )
    })?;
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn refusal(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    fn endpoint(byte: u8) -> BridgeEndpoint {
        BridgeEndpoint::new([byte; 32]).unwrap_or_else(|_| unreachable!())
    }

    fn source() -> Placement {
        Placement {
            host: "host-a".to_string(),
            generation: 1,
        }
    }

    fn target() -> Placement {
        Placement {
            host: "host-b".to_string(),
            generation: 2,
        }
    }

    fn plan() -> MigrationBridgePlan {
        MigrationBridgePlan {
            schema: BRIDGE_SCHEMA.to_string(),
            operation: content('1'),
            session: content('2'),
            attempt: content('3'),
            fence: 7,
            checkpoint: content('4'),
            source: source(),
            target: target(),
            source_endpoint: endpoint(1),
            target_endpoint: endpoint(2),
            source_socket: "/run/kiln/source.sock".to_string(),
            target_socket: "/run/kiln/target.sock".to_string(),
            protocol_generation: 1,
            memory_mode: MemoryMode::Precopy,
            max_connections: 1,
            max_stream_bytes: 1024,
            deadline_ms: 1000,
        }
    }

    fn expectation() -> BridgeExpectation {
        let plan = plan();
        BridgeExpectation {
            operation: plan.operation,
            session: plan.session,
            attempt: plan.attempt,
            fence: plan.fence,
            checkpoint: plan.checkpoint,
            source: plan.source,
            target: plan.target,
            source_endpoint: plan.source_endpoint,
            target_endpoint: plan.target_endpoint,
            source_socket: plan.source_socket,
            target_socket: plan.target_socket,
            protocol_generation: plan.protocol_generation,
        }
    }

    // r[verify kiln.hosted.bridge.plan] r[verify kiln.hosted.bridge.plan.scenario.admit]
    #[test]
    fn admits_one_exact_precopy_bridge() {
        let plan = plan();
        let identity =
            admit_bridge(&plan, &expectation()).unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(
            identity,
            admit_bridge(&plan, &expectation()).unwrap_or_else(|_| unreachable!())
        );
    }

    // r[verify kiln.hosted.bridge.plan.scenario.refused]
    #[test]
    fn refuses_wrong_peers_sockets_protocol_and_unsupported_modes() {
        let base = plan();
        let cases = [
            {
                let mut plan = base.clone();
                plan.source_endpoint = endpoint(3);
                plan
            },
            {
                let mut plan = base.clone();
                plan.target_socket = "/tmp/target.sock".to_string();
                plan
            },
            {
                let mut plan = base.clone();
                plan.protocol_generation = 2;
                plan
            },
            {
                let mut plan = base.clone();
                plan.max_connections = 2;
                plan
            },
            {
                let mut plan = base.clone();
                plan.source_socket = "/run/kiln/../escape.sock".to_string();
                plan
            },
        ];
        for plan in cases {
            assert!(admit_bridge(&plan, &expectation()).is_err());
        }
    }

    #[test]
    fn rejects_zero_endpoints() {
        assert!(BridgeEndpoint::new([0; 32]).is_err());
    }
}
