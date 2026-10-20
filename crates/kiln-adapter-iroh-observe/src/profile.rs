use std::collections::BTreeSet;
use std::net::SocketAddr;

use iroh::{EndpointId, RelayUrl};
use kiln_observe_core::ObservationProfile;
use serde::{Deserialize, Serialize};

use crate::{ObserveError, ObserveResult};

pub const PROFILE_SCHEMA: &str = "kiln.iroh.observe-profile.v1";
pub const IROH_VERSION: &str = "0.95.1";
pub const IROH_OBSERVATION_ALPN: &[u8] = b"kiln.observe.v1";
pub const IROH_BLOBS_COHORT: &str = "deferred";
pub const BAO_TREE_COHORT: &str = "deferred";

const MAXIMUM_PEERS: usize = 64;
const MAXIMUM_RELAYS: usize = 8;
const MAXIMUM_CONNECTIONS: u64 = 64;
const MAXIMUM_FRAME_BYTES: u64 = 67_108_864;
const MAXIMUM_DEADLINE_MILLISECONDS: u64 = 300_000;

// r[impl kiln.observe.iroh.nonclaims]
pub const IROH_OBSERVATION_NON_CLAIMS: &[&str] = &[
    "iroh observation does not prove transport acceptance",
    "iroh observation does not prove live completeness",
    "verified payload delivery does not prove source truth",
    "iroh observation does not prove control execution",
    "iroh observation does not prove provider correctness",
    "iroh observation does not prove CI success",
    "iroh observation does not establish release readiness",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CohortProfile {
    pub iroh_version: String,
    pub iroh_blobs: String,
    pub bao_tree: String,
    pub alpn: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointProfile {
    pub bind_address: SocketAddr,
    pub relay_urls: Vec<String>,
    pub max_connections: u64,
    pub max_frame_bytes: u64,
    pub max_payload_bytes: u64,
    pub request_timeout_milliseconds: u64,
    pub write_timeout_milliseconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeerProfile {
    pub allowed: BTreeSet<EndpointId>,
    pub revoked: BTreeSet<EndpointId>,
    pub max_peers: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IrohObservationProfile {
    pub schema_version: u16,
    pub profile_id: String,
    pub cohort: CohortProfile,
    pub endpoint: EndpointProfile,
    pub peers: PeerProfile,
    pub core: ObservationProfile,
    pub non_claims: Vec<String>,
}

impl IrohObservationProfile {
    // r[impl kiln.observe.iroh.profile.scenario.match]
    // r[impl kiln.observe.iroh.profile.scenario.drift]
    pub fn admit(&self) -> ObserveResult<()> {
        if self.schema_version != 1 {
            return Err(ObserveError::new(
                "unsupported_iroh_profile_version",
                "Iroh observation profile version is unsupported",
            ));
        }
        if self.profile_id.is_empty()
            || self.profile_id.contains('\r')
            || self.profile_id.contains('\n')
        {
            return Err(ObserveError::new(
                "invalid_iroh_profile_id",
                "Iroh observation profile identifier is empty or contains a line break",
            ));
        }
        self.core
            .admit()
            .map_err(|error| ObserveError::new("invalid_core_profile", error.message))?;
        if self.cohort.iroh_version != IROH_VERSION
            || self.cohort.iroh_blobs != IROH_BLOBS_COHORT
            || self.cohort.bao_tree != BAO_TREE_COHORT
            || self.cohort.alpn.as_bytes() != IROH_OBSERVATION_ALPN
        {
            return Err(ObserveError::new(
                "iroh_cohort_drift",
                "Iroh observation cohort differs from the pinned profile",
            ));
        }
        if self.endpoint.max_connections == 0
            || self.endpoint.max_connections > MAXIMUM_CONNECTIONS
            || self.endpoint.max_frame_bytes < self.core.bounds.max_encoded_bytes
            || self.endpoint.max_frame_bytes > MAXIMUM_FRAME_BYTES
            || self.endpoint.max_payload_bytes < self.core.bounds.max_log_bytes
            || self.endpoint.max_payload_bytes > self.endpoint.max_frame_bytes
            || self.endpoint.request_timeout_milliseconds == 0
            || self.endpoint.request_timeout_milliseconds > MAXIMUM_DEADLINE_MILLISECONDS
            || self.endpoint.write_timeout_milliseconds == 0
            || self.endpoint.write_timeout_milliseconds > MAXIMUM_DEADLINE_MILLISECONDS
            || self.endpoint.relay_urls.len() > MAXIMUM_RELAYS
        {
            return Err(ObserveError::new(
                "invalid_iroh_bounds",
                "Iroh observation endpoint bounds are invalid",
            ));
        }
        for relay in &self.endpoint.relay_urls {
            let parsed: RelayUrl = relay
                .parse()
                .map_err(|_| ObserveError::new("invalid_relay", "relay URL is malformed"))?;
            if !parsed.as_str().starts_with("https://") {
                return Err(ObserveError::new(
                    "invalid_relay",
                    "relay URL must use https",
                ));
            }
        }
        if self.peers.allowed.is_empty()
            || self.peers.max_peers == 0
            || self.peers.allowed.len() as u64 > self.peers.max_peers
            || self.peers.revoked.len() as u64 > self.peers.max_peers
            || self.peers.allowed.len() > MAXIMUM_PEERS
            || self.peers.revoked.len() > MAXIMUM_PEERS
        {
            return Err(ObserveError::new(
                "invalid_iroh_peers",
                "Iroh observation peer policy is empty or exceeds its bound",
            ));
        }
        if self.non_claims.is_empty() {
            return Err(ObserveError::new(
                "missing_iroh_non_claims",
                "Iroh observation profile must declare non-claims",
            ));
        }
        for required in IROH_OBSERVATION_NON_CLAIMS {
            if !self
                .non_claims
                .iter()
                .any(|claim| claim.as_str() == *required)
            {
                return Err(ObserveError::new(
                    "missing_required_iroh_non_claim",
                    "Iroh observation profile omits a required non-claim",
                ));
            }
        }
        Ok(())
    }

    // r[impl kiln.observe.iroh.peer]
    // r[impl kiln.observe.iroh.peer.scenario.trusted]
    // r[impl kiln.observe.iroh.peer.scenario.denied]
    pub fn admit_peer(&self, peer: EndpointId) -> ObserveResult<()> {
        if self.peers.revoked.contains(&peer) {
            return Err(ObserveError::new(
                "iroh_peer_revoked",
                "Iroh observation peer is revoked",
            ));
        }
        if self.peers.allowed.contains(&peer) {
            Ok(())
        } else {
            Err(ObserveError::new(
                "iroh_peer_denied",
                "Iroh observation peer is not enrolled",
            ))
        }
    }
}

// r[impl kiln.observe.iroh.peer]
#[must_use]
pub fn did_key(endpoint: EndpointId) -> String {
    let mut payload = vec![0xed, 0x01];
    payload.extend_from_slice(endpoint.as_bytes());
    format!("did:key:z{}", base58_encode(&payload))
}

fn base58_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let mut digits: Vec<u8> = Vec::new();
    for byte in bytes {
        let mut carry = u32::from(*byte);
        for digit in digits.iter_mut().rev() {
            let value = u32::from(*digit) * 256 + carry;
            *digit = (value % 58) as u8;
            carry = value / 58;
        }
        while carry > 0 {
            digits.insert(0, (carry % 58) as u8);
            carry /= 58;
        }
    }
    for byte in bytes {
        if *byte == 0 {
            digits.insert(0, 0);
        } else {
            break;
        }
    }
    digits
        .iter()
        .map(|digit| ALPHABET.get(usize::from(*digit)).copied().unwrap_or(b'1') as char)
        .collect()
}
