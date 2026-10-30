//! Tangled and AT Protocol identifier syntax. Pure; no resolution.

use serde::{Deserialize, Serialize};

/// AT Protocol DIDs are at most 2 KiB.
pub const MAX_DID_BYTES: usize = 2048;
/// Kiln accepts at most this many bytes of a Git reference name.
pub const MAX_REFERENCE_BYTES: usize = 1024;
/// AT Protocol record keys are at most 512 bytes.
pub const MAX_RECORD_KEY_BYTES: usize = 512;
/// Knots accept pipeline ids up to 256 bytes (`knot-atproto` `MAX_PIPELINE_ID_LEN`).
pub const MAX_PIPELINE_ID_BYTES: usize = 256;
/// `did:plc` identifiers carry 24 base32 characters.
const PLC_IDENTIFIER_CHARS: usize = 24;
const SHA1_HEX_CHARS: usize = 40;
const SHA256_HEX_CHARS: usize = 64;
/// DNS host names are at most 253 bytes.
const MAX_HOST_BYTES: usize = 253;
const MAX_LABEL_BYTES: usize = 63;

/// Generic AT Protocol DID syntax.
#[must_use]
pub fn valid_did(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("did:") else {
        return false;
    };
    let Some((method, identifier)) = rest.split_once(':') else {
        return false;
    };
    value.len() <= MAX_DID_BYTES
        && !method.is_empty()
        && method.bytes().all(|byte| byte.is_ascii_lowercase())
        && !identifier.is_empty()
        && !identifier.ends_with([':', '%'])
        && identifier.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'%' | b'-')
        })
}

/// `did:plc:` followed by 24 lowercase base32 characters.
#[must_use]
pub fn valid_plc(value: &str) -> bool {
    value.strip_prefix("did:plc:").is_some_and(|identifier| {
        identifier.len() == PLC_IDENTIFIER_CHARS
            && identifier
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || (b'2'..=b'7').contains(&byte))
    })
}

/// The HTTPS authority of a `did:web` service DID (`%3A` encodes a port).
#[must_use]
pub fn web_authority(value: &str) -> Option<String> {
    let encoded = value.strip_prefix("did:web:")?;
    if encoded.contains(':') {
        // Paths in did:web are not used by Tangled services.
        return None;
    }
    let (host, port) = match encoded
        .split_once("%3A")
        .or_else(|| encoded.split_once("%3a"))
    {
        Some((host, port)) => (host, Some(port)),
        None => (encoded, None),
    };
    if !valid_host(host) {
        return None;
    }
    match port {
        Some(port)
            if !port.is_empty() && port.len() <= 5 && port.bytes().all(|b| b.is_ascii_digit()) =>
        {
            port.parse::<u16>().ok().filter(|port| *port != 0)?;
            Some(format!("{host}:{port}"))
        }
        Some(_) => None,
        None => Some(host.to_string()),
    }
}

fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= MAX_HOST_BYTES
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= MAX_LABEL_BYTES
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

/// Git object formats a knot may serve.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectFormat {
    Sha1,
    Sha256,
}

impl ObjectFormat {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
        }
    }
}

/// A lowercase hexadecimal Git object id of either format.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ObjectId(String);

impl ObjectId {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let length_ok = matches!(value.len(), SHA1_HEX_CHARS | SHA256_HEX_CHARS);
        (length_ok
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
        .then(|| Self(value.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn format(&self) -> ObjectFormat {
        if self.0.len() == SHA256_HEX_CHARS {
            ObjectFormat::Sha256
        } else {
            ObjectFormat::Sha1
        }
    }

    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.bytes().all(|byte| byte == b'0')
    }
}

impl TryFrom<String> for ObjectId {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).ok_or("object id must be 40 or 64 lowercase hex characters")
    }
}

impl From<ObjectId> for String {
    fn from(value: ObjectId) -> Self {
        value.0
    }
}

/// A Git reference name accepted by `git check-ref-format`, bounded.
#[must_use]
pub fn valid_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REFERENCE_BYTES
        && !value.ends_with('.')
        && !value.ends_with('/')
        && !value.contains("..")
        && !value.contains("@{")
        && !value.bytes().any(|byte| {
            byte <= b' '
                || byte == 127
                || matches!(byte, b'~' | b'^' | b':' | b'?' | b'*' | b'[' | b'\\')
        })
        && value
            .split('/')
            .all(|part| !part.is_empty() && !part.starts_with('.') && !part.ends_with(".lock"))
}

/// Branch name of a full `refs/heads/` reference.
#[must_use]
pub fn branch_name(reference: &str) -> Option<&str> {
    reference
        .strip_prefix("refs/heads/")
        .filter(|name| !name.is_empty())
}

/// Tag name of a full `refs/tags/` reference.
#[must_use]
pub fn tag_name(reference: &str) -> Option<&str> {
    reference
        .strip_prefix("refs/tags/")
        .filter(|name| !name.is_empty())
}

/// AT Protocol record-key syntax.
#[must_use]
pub fn valid_record_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_RECORD_KEY_BYTES
        && !matches!(value, "." | "..")
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'~' | b'-')
        })
}

/// `at://<did>/<collection>/<rkey>` naming a record in a repository account.
#[must_use]
pub fn valid_at_uri(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("at://") else {
        return false;
    };
    let mut parts = rest.split('/');
    let (Some(authority), Some(collection), Some(key), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    valid_did(authority)
        && !collection.is_empty()
        && collection.len() <= MAX_RECORD_KEY_BYTES
        && collection
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
        && valid_record_key(key)
}
