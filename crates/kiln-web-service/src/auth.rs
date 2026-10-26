use hmac::{Hmac, Mac};
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::engine::Role;
use crate::profile::{ServiceProfile, read_token};

type HmacSha256 = Hmac<Sha256>;

// HMAC verification avoids data-dependent byte comparison of bearer secrets.
struct TokenVerifier {
    key: HmacSha256,
    tag: Vec<u8>,
}

impl TokenVerifier {
    fn new(secret: &[u8]) -> Result<Self, String> {
        let key = HmacSha256::new_from_slice(secret)
            .map_err(|_| "credential admission failed".to_owned())?;
        let mut expected = key.clone();
        expected.update(secret);
        Ok(Self {
            key,
            tag: expected.finalize().into_bytes().to_vec(),
        })
    }

    fn matches(&self, candidate: &[u8]) -> bool {
        let mut actual = self.key.clone();
        actual.update(candidate);
        actual.verify_slice(&self.tag).is_ok()
    }
}

pub(crate) struct Credentials {
    operator: TokenVerifier,
    worker: TokenVerifier,
}

impl Credentials {
    pub(crate) fn load(profile: &ServiceProfile) -> Result<Self, String> {
        let operator = Zeroizing::new(read_token(&profile.operator_token_file)?);
        let worker = Zeroizing::new(read_token(&profile.worker_token_file)?);
        let operator = TokenVerifier::new(&operator)?;
        if operator.matches(&worker) {
            return Err("operator and worker credentials must be distinct".to_owned());
        }
        Ok(Self {
            operator,
            worker: TokenVerifier::new(&worker)?,
        })
    }

    pub(crate) fn authenticate(&self, authorization: Option<&str>) -> Option<Role> {
        let candidate = authorization?.strip_prefix("Bearer ")?;
        if candidate.len() != 64 || !candidate.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let operator = self.operator.matches(candidate.as_bytes());
        let worker = self.worker.matches(candidate.as_bytes());
        if operator {
            Some(Role::Operator)
        } else if worker {
            Some(Role::Worker)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_roles_cannot_be_substituted_or_partially_matched() {
        let operator = "a".repeat(64);
        let worker = "b".repeat(64);
        let credentials = Credentials {
            operator: TokenVerifier::new(operator.as_bytes()).unwrap_or_else(|_| unreachable!()),
            worker: TokenVerifier::new(worker.as_bytes()).unwrap_or_else(|_| unreachable!()),
        };
        assert!(matches!(
            credentials.authenticate(Some(&format!("Bearer {operator}"))),
            Some(Role::Operator)
        ));
        assert!(matches!(
            credentials.authenticate(Some(&format!("Bearer {worker}"))),
            Some(Role::Worker)
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
}
