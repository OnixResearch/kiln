//! Bounded Git push transport for native delivery.
//!
//! The push-negotiation callback validates libgit2's advertised-old/new update
//! tuple immediately before upload. A non-force refspec lets receive-pack apply
//! its expected-old compare and fast-forward rule atomically; afterward this
//! shell independently reads the ref back. Transport errors remain unknown.

use git2::{
    CertificateCheckStatus, Cred, CredentialType, Direction, Error, PushOptions, RemoteCallbacks,
    Repository,
};

const MAX_REMOTE_URL_BYTES: usize = 2_048;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRemoteProfile {
    pub remote_name: String,
    pub remote_url: String,
    pub transport: NativeTransport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_host_key_sha256: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeTransport {
    Https,
    SshStrictHostKey,
    LocalBareFixture,
}

pub const NATIVE_TARGET_OBSERVATION_SCHEMA: &str = "kiln.native-git-target-observation.v1";

/// A fresh, read-only observation of one exact qualified remote target.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeTargetObservation {
    pub schema: String,
    pub remote_identity: kiln_core::types::ContentId,
    pub target_ref: String,
    pub current_commit: kiln_core::types::ExternalRef,
    pub freshness_identity: kiln_core::types::ContentId,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PushObservation {
    Confirmed { commit: String },
    Rejected { reason: String },
    Unknown { reason: String },
}

/// Deterministic identity for one exact remote locator and enrolled transport.
pub fn remote_identity(
    profile: &NativeRemoteProfile,
) -> Result<kiln_core::types::ContentId, Error> {
    validate_profile_input(profile)?;
    let transport = match profile.transport {
        NativeTransport::Https => "https-strict-tls",
        NativeTransport::SshStrictHostKey => "ssh-strict-host-key",
        NativeTransport::LocalBareFixture => "local-bare-fixture",
    };
    let pin = profile.ssh_host_key_sha256.as_deref().unwrap_or("-");
    let bytes = format!(
        "kiln.native-git-remote.v1\0{transport}\0{}\0{pin}",
        profile.remote_url
    );
    kiln_codec::identity_from_bytes(bytes.as_bytes())
        .map_err(|error| Error::from_str(&error.to_string()))
}

/// Publish one ref only if the server's advertised old target exactly matches
/// the plan. The source is an immutable object ID, never a worker-controlled ref.
pub fn push_exact_ref(
    repository: &Repository,
    profile: &NativeRemoteProfile,
    expected_remote_identity: &kiln_core::types::ContentId,
    source_oid: git2::Oid,
    destination_ref: &str,
    expected_old: git2::Oid,
    mut credential: impl FnMut(&str, Option<&str>, CredentialType) -> Result<Cred, Error>,
) -> Result<PushObservation, Error> {
    validate_profile(repository, profile)?;
    if &remote_identity(profile)? != expected_remote_identity {
        return Err(Error::from_str(
            "native Git remote identity differs from enrolled authority",
        ));
    }
    if !valid_ref(destination_ref) {
        return Err(Error::from_str("unsupported native Git destination ref"));
    }
    let mut remote = repository.find_remote(&profile.remote_name)?;
    if remote.url()? != profile.remote_url
        || remote
            .pushurl()?
            .is_some_and(|push_url| push_url != profile.remote_url.as_str())
    {
        return Err(Error::from_str(
            "native Git remote or push URL identity changed",
        ));
    }

    let refspec = format!("{source_oid}:{destination_ref}");
    let mut observed_rejection = None;
    let mut guard_mismatch = false;
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|url, username, allowed| {
        if url != profile.remote_url {
            return Err(Error::from_str(
                "credential request URL differs from enrolled remote",
            ));
        }
        credential(url, username, allowed)
    });
    configure_host_key_check(&mut callbacks, profile);
    callbacks.push_negotiation(|updates| match updates {
        [update]
            if update
                .dst_refname()
                .is_ok_and(|name| name == destination_ref)
                && update.src() == expected_old
                && update.dst() == source_oid =>
        {
            Ok(())
        }
        _ => {
            guard_mismatch = true;
            Err(Error::from_str(
                "native Git advertised old target or candidate differs from guarded operation",
            ))
        }
    });
    callbacks.push_update_reference(|reference, status| {
        if reference == destination_ref {
            if let Some(status) = status {
                observed_rejection = Some(status.to_owned());
            }
        }
        Ok(())
    });
    let mut options = PushOptions::new();
    options.remote_callbacks(callbacks);
    let result = remote.push(&[&refspec], Some(&mut options));
    drop(options);
    if let Err(error) = result {
        return Ok(if guard_mismatch {
            PushObservation::Rejected {
                reason: "advertised target/candidate mismatch".into(),
            }
        } else {
            match observed_rejection {
                Some(reason) => PushObservation::Rejected { reason },
                None => PushObservation::Unknown {
                    reason: error.message().to_owned(),
                },
            }
        });
    }
    if let Some(reason) = observed_rejection {
        return Ok(PushObservation::Rejected { reason });
    }

    // Reconnect with the same scoped credential provider and read the exact
    // enrolled fetch URL; a push acknowledgement alone is not acceptance.
    let mut read_callbacks = RemoteCallbacks::new();
    read_callbacks.credentials(|url, username, allowed| {
        if url != profile.remote_url {
            return Err(Error::from_str(
                "credential request URL differs from enrolled remote",
            ));
        }
        credential(url, username, allowed)
    });
    configure_host_key_check(&mut read_callbacks, profile);
    let connection = remote.connect_auth(Direction::Fetch, Some(read_callbacks), None)?;
    let readback = connection
        .list()?
        .iter()
        .find(|head| head.name() == destination_ref);
    match readback {
        Some(head) if head.oid() == source_oid => Ok(PushObservation::Confirmed {
            commit: source_oid.to_string(),
        }),
        Some(head) => Ok(PushObservation::Unknown {
            reason: format!(
                "remote read-back is {} rather than expected {}",
                head.oid(),
                source_oid
            ),
        }),
        None => Ok(PushObservation::Unknown {
            reason: "remote ref absent during read-back".into(),
        }),
    }
}

/// Independently inspect one exact remote ref using the enrolled locator and
/// supplied credential callback. This does not update local refs.
pub fn read_remote_ref(
    repository: &Repository,
    profile: &NativeRemoteProfile,
    expected_remote_identity: &kiln_core::types::ContentId,
    reference: &str,
    mut credential: impl FnMut(&str, Option<&str>, CredentialType) -> Result<Cred, Error>,
) -> Result<Option<git2::Oid>, Error> {
    validate_profile(repository, profile)?;
    if &remote_identity(profile)? != expected_remote_identity || !valid_ref(reference) {
        return Err(Error::from_str(
            "native Git read-back identity or ref is invalid",
        ));
    }
    let mut remote = repository.find_remote(&profile.remote_name)?;
    if remote.url()? != profile.remote_url
        || remote
            .pushurl()?
            .is_some_and(|push_url| push_url != profile.remote_url.as_str())
    {
        return Err(Error::from_str(
            "native Git remote or push URL identity changed",
        ));
    }
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|url, username, allowed| {
        if url != profile.remote_url {
            return Err(Error::from_str(
                "credential request URL differs from enrolled remote",
            ));
        }
        credential(url, username, allowed)
    });
    configure_host_key_check(&mut callbacks, profile);
    let connection = remote.connect_auth(Direction::Fetch, Some(callbacks), None)?;
    Ok(connection
        .list()?
        .iter()
        .find(|head| head.name() == reference)
        .map(|head| head.oid()))
}

/// Read one exact target through the enrolled remote and return its complete
/// freshness binding. No local ref is updated and an absent/non-commit target
/// is not represented as a successful observation.
pub fn observe_remote_target(
    repository: &Repository,
    profile: &NativeRemoteProfile,
    expected_remote_identity: &kiln_core::types::ContentId,
    target_ref: &str,
    credential: impl FnMut(&str, Option<&str>, CredentialType) -> Result<Cred, Error>,
) -> Result<NativeTargetObservation, Error> {
    let oid = read_remote_ref(
        repository,
        profile,
        expected_remote_identity,
        target_ref,
        credential,
    )?
    .ok_or_else(|| Error::from_str("qualified target ref is absent"))?;
    repository
        .find_commit(oid)
        .map_err(|_| Error::from_str("qualified target object is not an available Git commit"))?;
    let current_commit = kiln_core::types::ExternalRef::new("git", "commit", &oid.to_string())
        .map_err(|error| {
            Error::from_str(&format!(
                "qualified target commit identity is invalid: {error:?}"
            ))
        })?;
    let bytes = format!(
        "{}\0{}\0{}\0{}",
        NATIVE_TARGET_OBSERVATION_SCHEMA,
        expected_remote_identity.as_str(),
        target_ref,
        current_commit.native_id.as_str()
    );
    let freshness_identity = kiln_codec::identity_from_bytes(bytes.as_bytes())
        .map_err(|error| Error::from_str(&format!("target freshness identity failed: {error}")))?;
    Ok(NativeTargetObservation {
        schema: NATIVE_TARGET_OBSERVATION_SCHEMA.into(),
        remote_identity: expected_remote_identity.clone(),
        target_ref: target_ref.to_owned(),
        current_commit,
        freshness_identity,
    })
}

fn validate_profile(repository: &Repository, profile: &NativeRemoteProfile) -> Result<(), Error> {
    validate_profile_input(profile)?;
    let remote = repository.find_remote(&profile.remote_name)?;
    if remote.url()? != profile.remote_url {
        return Err(Error::from_str("native Git remote identity changed"));
    }
    Ok(())
}

fn validate_profile_input(profile: &NativeRemoteProfile) -> Result<(), Error> {
    if profile.remote_name.is_empty()
        || profile.remote_name.len() > 128
        || profile.remote_name.contains(['\0', '\n', '\r'])
        || profile.remote_url.is_empty()
        || profile.remote_url.len() > MAX_REMOTE_URL_BYTES
        || profile.remote_url.contains(['\0', '\n', '\r'])
    {
        return Err(Error::from_str("invalid native Git remote profile"));
    }
    let supported = match profile.transport {
        NativeTransport::Https => {
            profile.remote_url.starts_with("https://")
                && !https_authority_has_userinfo(&profile.remote_url)
                && !profile.remote_url.contains(['?', '#'])
                && profile.ssh_host_key_sha256.is_none()
        }
        NativeTransport::SshStrictHostKey => {
            let pin = profile.ssh_host_key_sha256.as_deref().unwrap_or("");
            (profile.remote_url.starts_with("ssh://") || profile.remote_url.starts_with("git@"))
                && !ssh_url_has_password(&profile.remote_url)
                && pin.len() == 64
                && pin
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        }
        NativeTransport::LocalBareFixture => {
            (profile.remote_url.starts_with("file://") || profile.remote_url.starts_with('/'))
                && profile.ssh_host_key_sha256.is_none()
        }
    };
    if !supported {
        return Err(Error::from_str(
            "unsupported native Git transport or missing strict host-key pin",
        ));
    }
    Ok(())
}

fn https_authority_has_userinfo(url: &str) -> bool {
    url.strip_prefix("https://")
        .and_then(|remainder| remainder.split('/').next())
        .is_none_or(|authority| authority.contains('@'))
}

fn ssh_url_has_password(url: &str) -> bool {
    let Some(remainder) = url.strip_prefix("ssh://") else {
        return false;
    };
    let authority = remainder.split('/').next().unwrap_or_default();
    authority
        .rsplit_once('@')
        .is_some_and(|(userinfo, _)| userinfo.contains(':'))
}

fn configure_host_key_check(callbacks: &mut RemoteCallbacks<'_>, profile: &NativeRemoteProfile) {
    if profile.transport == NativeTransport::SshStrictHostKey {
        let expected = profile.ssh_host_key_sha256.clone().unwrap_or_default();
        callbacks.certificate_check(move |certificate, _hostname| {
            let matches = certificate
                .as_hostkey()
                .and_then(|key| key.hash_sha256())
                .is_some_and(|actual| sha256_hex_matches(actual, &expected));
            if matches {
                Ok(CertificateCheckStatus::CertificateOk)
            } else {
                Err(Error::from_str(
                    "SSH host key does not match the enrolled SHA-256 pin",
                ))
            }
        });
    }
}
fn sha256_hex_matches(actual: &[u8], expected: &str) -> bool {
    actual.len() == 32
        && expected.len() == 64
        && actual
            .iter()
            .zip(expected.as_bytes().chunks_exact(2))
            .all(|(byte, pair)| match pair {
                [high, low] => {
                    hex_nibble(*high) == Some(byte >> 4) && hex_nibble(*low) == Some(byte & 0x0f)
                }
                _ => false,
            })
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn valid_ref(reference: &str) -> bool {
    reference.len() <= 1_024
        && reference.starts_with("refs/heads/")
        && git2::Reference::is_valid_name(reference)
        && !reference.contains("..")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_host_key_pin_comparison_is_exact_and_lowercase() {
        let actual = [0xab; 32];
        assert!(sha256_hex_matches(&actual, &"ab".repeat(32)));
        assert!(!sha256_hex_matches(&actual, &"AB".repeat(32)));
        assert!(!sha256_hex_matches(&actual, &"ac".repeat(32)));
        assert!(!sha256_hex_matches(&actual, "ab"));
    }

    #[test]
    fn read_only_target_observation_binds_remote_ref_commit_and_freshness() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "kiln-native-target-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap_or_else(|_| unreachable!());
        let remote_path = root.join("remote.git");
        let local_path = root.join("local.git");
        let remote = Repository::init_bare(&remote_path).unwrap_or_else(|_| unreachable!());
        let local = Repository::init(&local_path).unwrap_or_else(|_| unreachable!());
        let blob = local
            .blob(b"exact source\n")
            .unwrap_or_else(|_| unreachable!());
        let mut tree_builder = local.treebuilder(None).unwrap_or_else(|_| unreachable!());
        tree_builder
            .insert("source.txt", blob, 0o100644)
            .unwrap_or_else(|_| unreachable!());
        let tree = local
            .find_tree(tree_builder.write().unwrap_or_else(|_| unreachable!()))
            .unwrap_or_else(|_| unreachable!());
        let signature = git2::Signature::now("fixture", "fixture@example.invalid")
            .unwrap_or_else(|_| unreachable!());
        let commit = local
            .commit(
                Some("refs/heads/main"),
                &signature,
                &signature,
                "fixture",
                &tree,
                &[],
            )
            .unwrap_or_else(|_| unreachable!());
        drop(remote);
        let remote_name = local
            .remote(
                "origin",
                remote_path.to_str().unwrap_or_else(|| unreachable!()),
            )
            .unwrap_or_else(|_| unreachable!());
        drop(remote_name);
        let mut remote = local
            .find_remote("origin")
            .unwrap_or_else(|_| unreachable!());
        remote
            .push(&["refs/heads/main:refs/heads/main"], None)
            .unwrap_or_else(|_| unreachable!());
        drop(remote);
        let profile = NativeRemoteProfile {
            remote_name: "origin".into(),
            remote_url: remote_path.to_string_lossy().into_owned(),
            transport: NativeTransport::LocalBareFixture,
            ssh_host_key_sha256: None,
        };
        let identity = remote_identity(&profile).unwrap_or_else(|_| unreachable!());
        let first =
            observe_remote_target(&local, &profile, &identity, "refs/heads/main", |_, _, _| {
                Cred::default()
            })
            .unwrap_or_else(|_| unreachable!());
        let second =
            observe_remote_target(&local, &profile, &identity, "refs/heads/main", |_, _, _| {
                Cred::default()
            })
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(first.schema, NATIVE_TARGET_OBSERVATION_SCHEMA);
        assert_eq!(first.remote_identity, identity);
        assert_eq!(first.target_ref, "refs/heads/main");
        assert_eq!(first.current_commit.native_id.as_str(), commit.to_string());
        assert_eq!(first.freshness_identity, second.freshness_identity);
        let parent = local.find_commit(commit).unwrap_or_else(|_| unreachable!());
        let moved = local
            .commit(
                Some("refs/heads/main"),
                &signature,
                &signature,
                "moved target",
                &tree,
                &[&parent],
            )
            .unwrap_or_else(|_| unreachable!());
        drop(parent);
        let mut remote = local
            .find_remote("origin")
            .unwrap_or_else(|_| unreachable!());
        remote
            .push(&["refs/heads/main:refs/heads/main"], None)
            .unwrap_or_else(|_| unreachable!());
        drop(remote);
        let after_move =
            observe_remote_target(&local, &profile, &identity, "refs/heads/main", |_, _, _| {
                Cred::default()
            })
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            after_move.current_commit.native_id.as_str(),
            moved.to_string()
        );
        assert_ne!(after_move.freshness_identity, first.freshness_identity);
        std::fs::remove_dir_all(root).unwrap_or_else(|_| unreachable!());
    }
}
