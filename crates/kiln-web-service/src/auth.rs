use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use hmac::{Hmac, Mac};
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::access::{AccessProfile, Principal};
use crate::engine::Role;
use crate::profile::{ServiceProfile, read_token};

type HmacSha256 = Hmac<Sha256>;

// HMAC verification avoids data-dependent byte comparison of bearer secrets.
pub(crate) struct TokenVerifier {
    key: HmacSha256,
    tag: Vec<u8>,
}

impl TokenVerifier {
    pub(crate) fn new(secret: &[u8]) -> Result<Self, String> {
        let key = HmacSha256::new_from_slice(secret)
            .map_err(|_| "credential admission failed".to_owned())?;
        let mut expected = key.clone();
        expected.update(secret);
        Ok(Self {
            key,
            tag: expected.finalize().into_bytes().to_vec(),
        })
    }

    pub(crate) fn matches(&self, candidate: &[u8]) -> bool {
        let mut actual = self.key.clone();
        actual.update(candidate);
        actual.verify_slice(&self.tag).is_ok()
    }
}

struct CachedFeedToken {
    verifier: TokenVerifier,
    fingerprint: Option<(SystemTime, u64)>,
}

/// Feed token that reloads when the operator rotates its file.
pub(crate) struct FeedToken {
    path: PathBuf,
    cache: Mutex<Option<CachedFeedToken>>,
}

impl FeedToken {
    fn new(path: PathBuf, verifier: TokenVerifier) -> Self {
        let fingerprint = file_fingerprint(&path);
        Self {
            path,
            cache: Mutex::new(Some(CachedFeedToken {
                verifier,
                fingerprint,
            })),
        }
    }

    fn matches(&self, candidate: &[u8]) -> bool {
        let fingerprint = file_fingerprint(&self.path);
        let mut cache = self.cache.lock().ok();
        if let Some(cached) = cache.as_ref().and_then(|cache| cache.as_ref()) {
            if cached.fingerprint == fingerprint {
                return cached.verifier.matches(candidate);
            }
        }
        match read_token(&self.path).and_then(|secret| TokenVerifier::new(&secret)) {
            Ok(verifier) => {
                let matched = verifier.matches(candidate);
                if let Some(cache) = cache.as_mut() {
                    **cache = Some(CachedFeedToken {
                        verifier,
                        fingerprint,
                    });
                }
                matched
            }
            Err(_) => cache
                .as_ref()
                .and_then(|cache| cache.as_ref())
                .is_some_and(|cached| cached.verifier.matches(candidate)),
        }
    }
}

fn file_fingerprint(path: &Path) -> Option<(SystemTime, u64)> {
    std::fs::metadata(path).ok().and_then(|metadata| {
        metadata
            .modified()
            .ok()
            .map(|modified| (modified, metadata.len()))
    })
}

// r[impl kiln.web.authentication]
pub(crate) struct Credentials {
    operator: TokenVerifier,
    worker: TokenVerifier,
    viewers: Vec<(Principal, TokenVerifier)>,
    proxy: Option<TokenVerifier>,
    feed: Option<FeedToken>,
    access: AccessProfile,
}

impl Credentials {
    pub(crate) fn load(profile: &ServiceProfile, access: AccessProfile) -> Result<Self, String> {
        access.admit()?;
        let operator = Zeroizing::new(read_token(&profile.operator_token_file)?);
        let worker = Zeroizing::new(read_token(&profile.worker_token_file)?);
        let feed = profile
            .feeds
            .as_ref()
            .map(|feeds| read_token(&feeds.token_file))
            .transpose()?
            .map(Zeroizing::new);
        let mut secrets = vec![operator, worker];
        if let Some(feed) = &feed {
            secrets.push(feed.clone());
        }
        for viewer in &access.viewers {
            secrets.push(Zeroizing::new(read_token(&viewer.token_file)?));
        }
        if let Some(proxy) = &access.proxy {
            secrets.push(Zeroizing::new(read_token(&proxy.credential_file)?));
        }
        let mut verifiers: Vec<TokenVerifier> = Vec::with_capacity(secrets.len());
        for secret in &secrets {
            if verifiers.iter().any(|existing| existing.matches(secret)) {
                return Err("all role and proxy credentials must be distinct".into());
            }
            verifiers.push(TokenVerifier::new(secret)?);
        }
        let mut verifiers = verifiers.into_iter();
        let operator = verifiers.next().ok_or("operator credential unavailable")?;
        let worker = verifiers.next().ok_or("worker credential unavailable")?;
        let feed_path = profile.feeds.as_ref().map(|feeds| feeds.token_file.clone());
        let feed = match feed_path {
            Some(path) => Some(FeedToken::new(
                path,
                verifiers.next().ok_or("feed credential unavailable")?,
            )),
            None => None,
        };
        let viewers = access
            .viewers
            .iter()
            .map(|viewer| {
                let verifier = verifiers.next().ok_or("viewer credential unavailable")?;
                Ok((
                    Principal {
                        id: format!("viewer:{}", viewer.id),
                        label: viewer.label.clone(),
                        role: Role::Viewer,
                        agent: viewer.agent.clone(),
                    },
                    verifier,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let proxy = verifiers.next();
        Ok(Self {
            operator,
            worker,
            viewers,
            proxy,
            feed,
            access,
        })
    }

    pub(crate) fn authenticate(&self, authorization: Option<&str>) -> Option<Principal> {
        let candidate = authorization?.strip_prefix("Bearer ")?;
        if !token(candidate) {
            return None;
        }
        let operator = self.operator.matches(candidate.as_bytes());
        let worker = self.worker.matches(candidate.as_bytes());
        if operator {
            Some(Principal {
                id: "local:operator".into(),
                label: "Local operator".into(),
                role: Role::Operator,
                agent: None,
            })
        } else if worker {
            Some(Principal {
                id: "local:worker".into(),
                label: "Registered worker".into(),
                role: Role::Worker,
                agent: None,
            })
        } else {
            self.viewers
                .iter()
                .find(|(_, verifier)| verifier.matches(candidate.as_bytes()))
                .map(|(principal, _)| principal.clone())
        }
    }

    pub(crate) fn feed_matches(&self, candidate: &[u8]) -> bool {
        self.feed
            .as_ref()
            .is_some_and(|feed| feed.matches(candidate))
    }

    pub(crate) fn authenticate_request(
        &self,
        authorization: Option<&str>,
        proxy_key: Option<&str>,
        principal: Option<&str>,
    ) -> Option<Principal> {
        if proxy_key.is_some() || principal.is_some() {
            // Never combine two identities or let a bearer override forged proxy headers.
            if authorization.is_some() || !self.proxy_matches(proxy_key?) {
                return None;
            }
            self.access.forwarded(principal?)
        } else {
            self.authenticate(authorization)
        }
    }

    pub(crate) fn proxy_matches(&self, candidate: &str) -> bool {
        token(candidate)
            && self
                .proxy
                .as_ref()
                .is_some_and(|verifier| verifier.matches(candidate.as_bytes()))
    }
}

fn token(candidate: &str) -> bool {
    candidate.len() == 64 && candidate.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feed_token_reloads_after_file_rotation() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let path = directory.path().join("feed.token");
        let first = "a".repeat(64);
        std::fs::write(&path, &first).unwrap_or_else(|_| unreachable!());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .unwrap_or_else(|_| unreachable!());
        let token = FeedToken::new(
            path.clone(),
            TokenVerifier::new(first.as_bytes()).unwrap_or_else(|_| unreachable!()),
        );
        assert!(token.matches(first.as_bytes()));
        let second = "b".repeat(64);
        std::fs::write(&path, &second).unwrap_or_else(|_| unreachable!());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .unwrap_or_else(|_| unreachable!());
        assert!(token.matches(second.as_bytes()));
        assert!(!token.matches(first.as_bytes()));
    }

    // r[verify kiln.web.authentication.scenario.operator] r[verify kiln.web.authentication.scenario.denied]
    #[test]
    fn bearer_roles_cannot_be_substituted_or_partially_matched() {
        let operator = "a".repeat(64);
        let worker = "b".repeat(64);
        let credentials = Credentials {
            operator: TokenVerifier::new(operator.as_bytes()).unwrap_or_else(|_| unreachable!()),
            worker: TokenVerifier::new(worker.as_bytes()).unwrap_or_else(|_| unreachable!()),
            viewers: Vec::new(),
            proxy: None,
            feed: None,
            access: AccessProfile::default(),
        };
        assert!(matches!(
            credentials.authenticate(Some(&format!("Bearer {operator}"))),
            Some(Principal {
                role: Role::Operator,
                ..
            })
        ));
        assert!(matches!(
            credentials.authenticate(Some(&format!("Bearer {worker}"))),
            Some(Principal {
                role: Role::Worker,
                ..
            })
        ));
        assert!(
            credentials
                .authenticate(Some(&format!(
                    "Bearer {}c",
                    operator.chars().take(63).collect::<String>()
                )))
                .is_none()
        );
        assert!(
            credentials
                .authenticate(Some(&format!("Basic {operator}")))
                .is_none()
        );
        assert!(credentials.authenticate(None).is_none());
        assert!(credentials.authenticate(Some("Bearer a")).is_none());
    }

    // r[verify kiln.web.network.boundaries]
    #[test]
    fn proxy_identity_requires_distinct_credential_and_explicit_binding() {
        let secret = "c".repeat(64);
        let access: AccessProfile = serde_json::from_value(serde_json::json!({
            "schema":"kiln.web-access.v1", "proxy":{
                "listen_address":"127.0.0.1:8099", "credential_file":"/private/proxy",
                "tailscaled_socket_path":"/run/tailscale/tailscaled.sock",
                "principals":[{"user_id":"47256370424925063", "login_name":"reader@example.com", "label":"Reader", "role":"viewer"}]
            }
        })).unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        let credentials = Credentials {
            operator: TokenVerifier::new("a".repeat(64).as_bytes())
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
            worker: TokenVerifier::new("b".repeat(64).as_bytes())
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
            viewers: vec![(
                Principal {
                    id: "viewer:private".into(),
                    label: "Private viewer".into(),
                    role: Role::Viewer,
                    agent: None,
                },
                TokenVerifier::new("d".repeat(64).as_bytes())
                    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
            )],
            proxy: Some(
                TokenVerifier::new(secret.as_bytes())
                    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
            ),
            feed: None,
            access,
        };
        let id = "tailscale:47256370424925063";
        let admitted = credentials
            .authenticate_request(None, Some(&secret), Some(id))
            .unwrap_or_else(|| unreachable!("registered principal not admitted"));
        assert_eq!(admitted.id, id);
        assert_eq!(admitted.role, Role::Viewer);
        assert_eq!(admitted.label, "Reader");
        let viewer = credentials
            .authenticate(Some(&format!("Bearer {}", "d".repeat(64))))
            .unwrap_or_else(|| unreachable!("registered viewer not admitted"));
        assert_eq!(viewer.id, "viewer:private");
        assert_eq!(viewer.role, Role::Viewer);
        for (authorization, key, principal) in [
            (None, None, Some(id)),
            (None, Some(secret.as_str()), None),
            (None, Some(secret.as_str()), Some("tailscale:123")),
            (None, Some("wrong"), Some(id)),
            (Some("Bearer operator"), Some(secret.as_str()), Some(id)),
        ] {
            assert!(
                credentials
                    .authenticate_request(authorization, key, principal)
                    .is_none()
            );
        }
        assert!(credentials.authenticate_request(None, None, None).is_none());
    }
}
