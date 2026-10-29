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

// r[impl kiln.web.authentication]
pub(crate) struct Credentials {
    operator: TokenVerifier,
    worker: TokenVerifier,
    viewers: Vec<(Principal, TokenVerifier)>,
    proxy: Option<TokenVerifier>,
    access: AccessProfile,
}

impl Credentials {
    pub(crate) fn load(profile: &ServiceProfile, access: AccessProfile) -> Result<Self, String> {
        access.admit()?;
        let operator = Zeroizing::new(read_token(&profile.operator_token_file)?);
        let worker = Zeroizing::new(read_token(&profile.worker_token_file)?);
        let mut secrets = vec![operator, worker];
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
            })
        } else if worker {
            Some(Principal {
                id: "local:worker".into(),
                label: "Registered worker".into(),
                role: Role::Worker,
            })
        } else {
            self.viewers
                .iter()
                .find(|(_, verifier)| verifier.matches(candidate.as_bytes()))
                .map(|(principal, _)| principal.clone())
        }
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
                },
                TokenVerifier::new("d".repeat(64).as_bytes())
                    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
            )],
            proxy: Some(
                TokenVerifier::new(secret.as_bytes())
                    .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
            ),
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
