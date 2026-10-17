use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::PathBuf;

use iroh::{EndpointId, RelayUrl};
use lattice_workflow_contract::{CapabilityProfile, validate_profile};
use lattice_workflow_transport::{
    ExchangeCommand, ExchangeRequest, ExchangeResponse, TransportProfile,
    validate_exchange_request, validate_exchange_response, validate_response_limits,
    validate_transport_profile,
};
use serde::{Deserialize, Serialize};

use crate::Error;

pub const PROFILE_SCHEMA: &str = "kiln.iroh-workflow-profile.v1";
pub const IROH_VERSION: &str = "0.95.1";
pub const ALPN: &[u8] = b"kiln.workflow.v1";
const MAXIMUM_PEERS: usize = 64;
const MAXIMUM_RELAYS: usize = 8;
const MAXIMUM_CONCURRENT_EXCHANGES: u32 = 64;
const MAXIMUM_EXCHANGE_MILLISECONDS: u64 = 300_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub iroh_version: String,
    pub alpn: String,
    pub bind_address: SocketAddr,
    pub relay_urls: Vec<String>,
    pub allowed_peers: BTreeSet<EndpointId>,
    pub socket_path: PathBuf,
    pub transport: TransportProfile,
    pub contract: CapabilityProfile,
    pub exchange_timeout_ms: u64,
}

impl Profile {
    // r[impl kiln.iroh.workflow.profile]
    pub fn admit(&self) -> Result<(), Error> {
        let path = self.socket_path.as_os_str().as_encoded_bytes();
        let unsafe_component = path
            .split(|byte| *byte == b'/')
            .any(|component| component == b"." || component == b"..");
        if self.schema != PROFILE_SCHEMA
            || self.iroh_version != IROH_VERSION
            || self.alpn.as_bytes() != ALPN
            || self.allowed_peers.is_empty()
            || self.allowed_peers.len() > MAXIMUM_PEERS
            || self.relay_urls.len() > MAXIMUM_RELAYS
            || !self.socket_path.is_absolute()
            || self.socket_path.file_name().is_none()
            || unsafe_component
            || path.contains(&0)
            || path.ends_with(b"/")
            || self.exchange_timeout_ms == 0
            || self.exchange_timeout_ms > MAXIMUM_EXCHANGE_MILLISECONDS
            || self.transport.limits.maximum_connections > MAXIMUM_CONCURRENT_EXCHANGES
        {
            return Err(Error::InvalidProfile);
        }
        validate_transport_profile(&self.transport).map_err(|_| Error::InvalidProfile)?;
        validate_profile(&self.contract).map_err(|_| Error::InvalidProfile)?;
        for url in &self.relay_urls {
            let relay: RelayUrl = url.parse().map_err(|_| Error::InvalidProfile)?;
            if !relay.as_str().starts_with("https://") {
                return Err(Error::InvalidProfile);
            }
        }
        Ok(())
    }

    // r[impl kiln.iroh.workflow.peers]
    pub fn admit_peer(&self, peer: EndpointId) -> Result<(), Error> {
        if self.allowed_peers.contains(&peer) {
            Ok(())
        } else {
            Err(Error::PeerDenied)
        }
    }

    // r[impl kiln.iroh.workflow.transport]
    pub fn admit_request(&self, request: &ExchangeRequest) -> Result<(), Error> {
        validate_exchange_request(request).map_err(|_| Error::InvalidRequest)?;
        let profile = match &request.command {
            ExchangeCommand::Dispatch { profile, .. }
            | ExchangeCommand::RecoverDispatch { profile, .. }
            | ExchangeCommand::Observe { profile, .. }
            | ExchangeCommand::Control { profile, .. }
            | ExchangeCommand::ReadLog { profile, .. }
            | ExchangeCommand::Reconcile { profile, .. } => profile,
        };
        if profile != &self.contract {
            return Err(Error::InvalidRequest);
        }
        Ok(())
    }

    pub fn admit_response(
        &self,
        request: &ExchangeRequest,
        response: &ExchangeResponse,
    ) -> Result<(), Error> {
        validate_exchange_response(request, response).map_err(|_| Error::InvalidResponse)?;
        validate_response_limits(&self.transport, response).map_err(|_| Error::InvalidResponse)
    }
}
