//! Peer identity for the fleet API.
//!
//! Loopback is the coordinator. A tailnet address is named by
//! `tailscale whois --json <ip>` (one bounded child, cached per address for
//! [`WHOIS_CACHE_TTL_MS`]) and its `.Node.Name` is mapped through the
//! configured `peers`. Everything else is refused.

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::Deserialize;

use crate::config::{FleetConfig, tailnet};
use crate::exec::Child;

pub const WHOIS_TIMEOUT_MS: u64 = 5_000;
pub const WHOIS_CACHE_TTL_MS: u64 = 60_000;
pub const MAX_WHOIS_CACHE_ENTRIES: usize = 256;
pub const MAX_WHOIS_OUTPUT_BYTES: usize = 64 * 1_024;
/// Working directory of the `tailscale` child; it reads nothing there.
const WHOIS_DIRECTORY: &str = "/";

#[derive(Deserialize)]
struct Whois {
    #[serde(rename = "Node")]
    node: WhoisNode,
}

#[derive(Deserialize)]
struct WhoisNode {
    #[serde(rename = "Name")]
    name: String,
}

/// Cached outcome of one address lookup.
type Cached = (u64, Result<String, String>);

/// Maps peer addresses to configured machines.
#[derive(Debug)]
pub struct PeerIdentity {
    coordinator: String,
    peers: BTreeMap<String, String>,
    tailscale: PathBuf,
    cache: Mutex<BTreeMap<IpAddr, Cached>>,
}

impl PeerIdentity {
    #[must_use]
    pub fn new(config: &FleetConfig) -> Self {
        Self {
            coordinator: config.machine.clone(),
            peers: config
                .peers
                .iter()
                .map(|(node, machine)| (normalize(node), machine.clone()))
                .collect(),
            tailscale: config.tailscale.clone(),
            cache: Mutex::new(BTreeMap::new()),
        }
    }

    // r[impl kiln.fleet.api]
    /// The machine at `address`.
    ///
    /// # Errors
    /// Refuses addresses outside loopback and the tailnet, nodes that are not
    /// configured peers, and failed lookups.
    pub fn identify(&self, address: IpAddr, now_ms: u64) -> Result<String, String> {
        let address = address.to_canonical();
        if address.is_loopback() {
            return Ok(self.coordinator.clone());
        }
        if !tailnet(address) {
            return Err(format!(
                "{address} is neither loopback nor a tailnet address"
            ));
        }
        if let Ok(cache) = self.cache.lock()
            && let Some((expires, outcome)) = cache.get(&address)
            && *expires > now_ms
        {
            return outcome.clone();
        }
        // A failed lookup is not cached: the next request asks again.
        let node = self.whois(address)?;
        let outcome = self
            .peers
            .get(&normalize(&node))
            .cloned()
            .ok_or_else(|| format!("tailnet node {node} is not a configured peer"));
        if let Ok(mut cache) = self.cache.lock() {
            if cache.len() >= MAX_WHOIS_CACHE_ENTRIES {
                cache.retain(|_, (expires, _)| *expires > now_ms);
            }
            if cache.len() < MAX_WHOIS_CACHE_ENTRIES {
                cache.insert(
                    address,
                    (now_ms.saturating_add(WHOIS_CACHE_TTL_MS), outcome.clone()),
                );
            }
        }
        outcome
    }

    fn whois(&self, address: IpAddr) -> Result<String, String> {
        let finished = Child::new(
            &self.tailscale,
            std::path::Path::new(WHOIS_DIRECTORY),
            WHOIS_TIMEOUT_MS,
            MAX_WHOIS_OUTPUT_BYTES,
        )
        .args(["whois", "--json", &address.to_string()])
        .run()?
        .require_success()?;
        let whois: Whois = serde_json::from_slice(&finished.stdout)
            .map_err(|_| "tailscale whois output is not the expected JSON".to_owned())?;
        Ok(whois.node.name)
    }
}

/// Node names compare case-insensitively, with or without the trailing dot.
fn normalize(node: &str) -> String {
    node.trim_end_matches('.').to_ascii_lowercase()
}
