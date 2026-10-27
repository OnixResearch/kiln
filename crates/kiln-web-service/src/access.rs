//! Separately admitted access policy and attested tailnet identities.
use std::collections::BTreeSet;
use std::io::Read;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::engine::Role;

#[derive(Clone, Debug, Serialize)]
pub struct Principal {
    pub id: String,
    pub label: String,
    pub role: Role,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserRole {
    Viewer,
    Operator,
}
impl From<BrowserRole> for Role {
    fn from(value: BrowserRole) -> Self {
        match value {
            BrowserRole::Viewer => Self::Viewer,
            BrowserRole::Operator => Self::Operator,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewerBinding {
    pub id: String,
    pub label: String,
    pub token_file: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TailnetBinding {
    /// Tailscale UserProfile.ID, represented as its exact decimal string.
    pub user_id: String,
    pub login_name: String,
    pub label: String,
    pub role: BrowserRole,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProxyProfile {
    pub listen_address: SocketAddr,
    pub credential_file: PathBuf,
    pub tailscaled_socket_path: PathBuf,
    pub principals: Vec<TailnetBinding>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessProfile {
    pub schema: String,
    #[serde(default)]
    pub viewers: Vec<ViewerBinding>,
    #[serde(default)]
    pub proxy: Option<ProxyProfile>,
}
impl Default for AccessProfile {
    fn default() -> Self {
        Self {
            schema: "kiln.web-access.v1".into(),
            viewers: Vec::new(),
            proxy: None,
        }
    }
}
impl AccessProfile {
    pub fn identity(&self) -> Result<String, String> {
        self.admit()?;
        crate::engine::identity(self).map(|id| id.as_str().to_owned())
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = crate::worker::files::regular(path, 262_144, false)
            .map_err(|_| "access policy unavailable or unsafe")?;
        let profile: Self =
            serde_json::from_slice(&bytes).map_err(|_| "access policy malformed")?;
        profile.admit()?;
        Ok(profile)
    }
    pub fn admit(&self) -> Result<(), String> {
        if self.schema != "kiln.web-access.v1" || self.viewers.len() > 128 {
            return Err("access policy schema or bound rejected".into());
        }
        let mut ids = BTreeSet::new();
        for viewer in &self.viewers {
            if !crate::profile::identifier(&viewer.id)
                || !label(&viewer.label)
                || !crate::profile::absolute(&viewer.token_file)
                || !ids.insert(&viewer.id)
            {
                return Err("viewer binding rejected".into());
            }
        }
        if let Some(proxy) = &self.proxy {
            if !proxy.listen_address.ip().is_loopback()
                || proxy.listen_address.port() == 0
                || !crate::profile::absolute(&proxy.credential_file)
                || !crate::profile::absolute(&proxy.tailscaled_socket_path)
                || proxy.principals.is_empty()
                || proxy.principals.len() > 128
            {
                return Err("private proxy policy rejected".into());
            }
            let mut users = BTreeSet::new();
            let mut logins = BTreeSet::new();
            for binding in &proxy.principals {
                if binding.user_id.is_empty()
                    || binding.user_id.len() > 20
                    || binding.user_id.parse::<u64>().is_err()
                    || !binding.user_id.bytes().all(|b| b.is_ascii_digit())
                    || binding.user_id.starts_with('0')
                    || !label(&binding.label)
                    || !label(&binding.login_name)
                    || binding.login_name.contains('*')
                    || !users.insert(&binding.user_id)
                    || !logins.insert(&binding.login_name)
                {
                    return Err("tailnet principal binding rejected".into());
                }
            }
        }
        Ok(())
    }
    pub(crate) fn forwarded(&self, id: &str) -> Option<Principal> {
        let user_id = id.strip_prefix("tailscale:")?;
        let binding = self
            .proxy
            .as_ref()?
            .principals
            .iter()
            .find(|binding| user_id == binding.user_id)?;
        Some(Principal {
            id: id.to_owned(),
            label: binding.label.clone(),
            role: binding.role.into(),
        })
    }
}
fn label(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && value.bytes().all(|b| (b' '..=b'~').contains(&b))
}

/// Only facts returned by the private tailscaled LocalAPI may select a binding.
#[derive(Deserialize)]
struct WhoIs {
    #[serde(rename = "UserProfile")]
    user: WhoIsUser,
    #[serde(rename = "Node")]
    node: WhoIsNode,
}
#[derive(Deserialize)]
struct WhoIsUser {
    #[serde(rename = "ID")]
    id: u64,
    #[serde(rename = "LoginName")]
    login: String,
}
#[derive(Deserialize)]
struct WhoIsNode {
    #[serde(rename = "Tags", default)]
    tags: Vec<String>,
    #[serde(rename = "User")]
    user: u64,
}
fn admit_whois(profile: &ProxyProfile, bytes: &[u8]) -> Result<Principal, String> {
    let who: WhoIs =
        serde_json::from_slice(bytes).map_err(|_| "peer identity response malformed")?;
    if who.user.id == 0 || who.node.user != who.user.id || !who.node.tags.is_empty() {
        return Err("tagged or mismatched peer identity denied".into());
    }
    let user_id = who.user.id.to_string();
    let binding = profile
        .principals
        .iter()
        .find(|binding| binding.user_id == user_id && binding.login_name == who.user.login)
        .ok_or("peer identity not registered")?;
    Ok(Principal {
        id: format!("tailscale:{}", binding.user_id),
        label: binding.label.clone(),
        role: binding.role.into(),
    })
}

pub(crate) struct IdentityResolver {
    client: reqwest::blocking::Client,
}
impl IdentityResolver {
    pub(crate) fn new(profile: &ProxyProfile) -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .unix_socket(profile.tailscaled_socket_path.clone())
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(3))
            .build()
            .map_err(|_| "peer resolver unavailable")?;
        Ok(Self { client })
    }
    pub(crate) fn resolve(&self, profile: &ProxyProfile, peer: &str) -> Result<Principal, String> {
        let address: SocketAddr = peer.parse().map_err(|_| "peer address malformed")?;
        let tailnet = match address.ip() {
            IpAddr::V4(ip) => (u32::from(ip) & 0xffc0_0000) == 0x6440_0000,
            IpAddr::V6(ip) => (u128::from(ip) >> 80) == 0xfd7a115ca1e0,
        };
        if !tailnet || address.port() == 0 {
            return Err("non-tailnet peer denied".into());
        }
        let response = self
            .client
            .get("http://local-tailscaled.sock/localapi/v0/whois")
            .query(&[("addr", address.to_string())])
            .send()
            .map_err(|_| "peer identity lookup failed")?;
        if !response.status().is_success() {
            return Err("peer identity unavailable".into());
        }
        let mut bytes = Vec::new();
        response
            .take(65_537)
            .read_to_end(&mut bytes)
            .map_err(|_| "peer identity read failed")?;
        if bytes.len() > 65_536 {
            return Err("peer identity response exceeds bound".into());
        }
        admit_whois(profile, &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn proxy() -> ProxyProfile {
        ProxyProfile {
            listen_address: "127.0.0.1:8089"
                .parse()
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
            credential_file: "/private/proxy".into(),
            tailscaled_socket_path: "/run/tailscale/tailscaled.sock".into(),
            principals: vec![TailnetBinding {
                user_id: "123".into(),
                login_name: "operator@example.com".into(),
                label: "Operator".into(),
                role: BrowserRole::Viewer,
            }],
        }
    }
    #[test]
    fn whois_requires_exact_registered_user_and_untagged_node() {
        let valid = br#"{"Node":{"User":123,"Tags":[]},"UserProfile":{"ID":123,"LoginName":"operator@example.com"}}"#;
        let admitted = admit_whois(&proxy(), valid)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        assert_eq!(admitted.id, "tailscale:123");
        assert_eq!(admitted.role, Role::Viewer);
        for denied in [
            br#"{"Node":{"User":123,"Tags":[]},"UserProfile":{"ID":123,"LoginName":"forged@example.com"}}"#.as_slice(),
            br#"{"Node":{"User":124,"Tags":[]},"UserProfile":{"ID":123,"LoginName":"operator@example.com"}}"#,
            br#"{"Node":{"User":123,"Tags":["tag:server"]},"UserProfile":{"ID":123,"LoginName":"operator@example.com"}}"#,
            br#"{"UserProfile":{"ID":123,"LoginName":"operator@example.com"}}"#,
        ] { assert!(admit_whois(&proxy(), denied).is_err()); }
    }
    #[test]
    fn map_never_admits_wildcard_or_public_resolver() {
        let mut profile = AccessProfile {
            proxy: Some(proxy()),
            ..AccessProfile::default()
        };
        assert!(profile.admit().is_ok());
        profile
            .proxy
            .as_mut()
            .unwrap_or_else(|| unreachable!("proxy fixture missing"))
            .listen_address = "0.0.0.0:8089"
            .parse()
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        assert!(profile.admit().is_err());
        profile
            .proxy
            .as_mut()
            .unwrap_or_else(|| unreachable!("proxy fixture missing"))
            .listen_address = "127.0.0.1:8089"
            .parse()
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        let mut invalid = serde_json::to_value(&profile)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        *invalid
            .pointer_mut("/proxy/principals/0/role")
            .unwrap_or_else(|| unreachable!("principal fixture missing")) =
            serde_json::json!("worker");
        assert!(serde_json::from_value::<AccessProfile>(invalid).is_err());
        profile
            .proxy
            .as_mut()
            .unwrap_or_else(|| unreachable!("proxy fixture missing"))
            .principals
            .first_mut()
            .unwrap_or_else(|| unreachable!("principal fixture missing"))
            .login_name = "*".into();
        assert!(profile.admit().is_err());
    }

    #[test]
    fn localapi_resolver_preserves_large_user_ids_and_rejects_non_tailnet_peers() {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixListener;
        let directory =
            tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        let mut profile = proxy();
        profile.tailscaled_socket_path = directory.path().join("tailscaled.sock");
        profile
            .principals
            .first_mut()
            .unwrap_or_else(|| unreachable!("principal fixture missing"))
            .user_id = "47256370424925063".into();
        let listener = UnixListener::bind(&profile.tailscaled_socket_path)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        let responder = std::thread::spawn(move || {
            let (mut stream, _) = listener
                .accept()
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
            let mut request = BufReader::new(&mut stream);
            let mut first = String::new();
            request
                .read_line(&mut first)
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
            assert!(first.starts_with("GET /localapi/v0/whois?addr=100.110.43.11%3A8443 HTTP/1.1"));
            loop {
                let mut line = String::new();
                request
                    .read_line(&mut line)
                    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
                if line == "\r\n" {
                    break;
                }
                assert!(!line.is_empty());
            }
            let body = br#"{"Node":{"User":47256370424925063,"Tags":[]},"UserProfile":{"ID":47256370424925063,"LoginName":"operator@example.com"}}"#;
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
            stream
                .write_all(body)
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        });
        let resolver = IdentityResolver::new(&profile)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        assert!(resolver.resolve(&profile, "127.0.0.1:8443").is_err());
        let principal = resolver
            .resolve(&profile, "100.110.43.11:8443")
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        assert_eq!(principal.id, "tailscale:47256370424925063");
        assert_eq!(principal.role, Role::Viewer);
        responder
            .join()
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
    }
}
