use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use git2::{Cred, CredentialType, Oid, Repository, Signature, Time};
use kiln_adapter_git::native_publication::{
    NativeRemoteProfile, NativeTransport, PushObservation, push_exact_ref, remote_identity,
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    local: Repository,
    remote: Repository,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "kiln-native-git-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(&root)
            .unwrap_or_else(|error| unreachable!("fixture setup failed: {error}"));
        let remote_path = root.join("remote.git");
        let local_path = root.join("local");
        let remote = Repository::init_bare(&remote_path)
            .unwrap_or_else(|error| unreachable!("fixture setup failed: {error}"));
        let local = Repository::init(&local_path)
            .unwrap_or_else(|error| unreachable!("fixture setup failed: {error}"));
        local
            .remote("origin", remote_path.to_str().unwrap_or_default())
            .unwrap_or_else(|error| unreachable!("fixture setup failed: {error}"));
        Self {
            root,
            local,
            remote,
        }
    }

    fn profile(&self) -> NativeRemoteProfile {
        NativeRemoteProfile {
            remote_name: "origin".into(),
            remote_url: self.root.join("remote.git").to_string_lossy().into_owned(),
            transport: NativeTransport::LocalBareFixture,
            ssh_host_key_sha256: None,
        }
    }

    fn commit(&self, parent: Option<Oid>, content: &[u8]) -> Oid {
        let blob = self
            .local
            .blob(content)
            .unwrap_or_else(|error| unreachable!("fixture commit failed: {error}"));
        let mut builder = self
            .local
            .treebuilder(None)
            .unwrap_or_else(|error| unreachable!("fixture commit failed: {error}"));
        builder
            .insert("file", blob, 0o100644)
            .unwrap_or_else(|error| unreachable!("fixture commit failed: {error}"));
        let tree_oid = builder
            .write()
            .unwrap_or_else(|error| unreachable!("fixture commit failed: {error}"));
        let tree = self
            .local
            .find_tree(tree_oid)
            .unwrap_or_else(|error| unreachable!("fixture commit failed: {error}"));
        let signature = Signature::new(
            "Fixture",
            "fixture@example.invalid",
            &Time::new(1_700_000_000, 0),
        )
        .unwrap_or_else(|error| unreachable!("fixture commit failed: {error}"));
        let parents = parent
            .iter()
            .map(|oid| {
                self.local
                    .find_commit(*oid)
                    .unwrap_or_else(|error| unreachable!("fixture commit failed: {error}"))
            })
            .collect::<Vec<_>>();
        let parents = parents.iter().collect::<Vec<_>>();
        self.local
            .commit(None, &signature, &signature, "fixture", &tree, &parents)
            .unwrap_or_else(|error| unreachable!("fixture commit failed: {error}"))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn no_credentials(_: &str, _: Option<&str>, _: CredentialType) -> Result<Cred, git2::Error> {
    Err(git2::Error::from_str(
        "local fixture unexpectedly requested credentials",
    ))
}

#[test]
fn exact_push_checks_advertised_old_and_rejects_non_fast_forward() {
    let fixture = Fixture::new();
    let profile = fixture.profile();
    let identity = remote_identity(&profile)
        .unwrap_or_else(|error| unreachable!("fixture identity failed: {error}"));
    let base = fixture.commit(None, b"base");
    assert!(matches!(
        push_exact_ref(
            &fixture.local,
            &profile,
            &identity,
            base,
            "refs/heads/main",
            Oid::ZERO_SHA1,
            no_credentials
        ),
        Ok(PushObservation::Confirmed { .. }),
    ));

    let candidate = fixture.commit(Some(base), b"candidate");
    assert!(matches!(
        push_exact_ref(
            &fixture.local,
            &profile,
            &identity,
            candidate,
            "refs/heads/candidate",
            Oid::ZERO_SHA1,
            no_credentials
        ),
        Ok(PushObservation::Confirmed { .. }),
    ));

    let advanced = fixture.commit(Some(base), b"advanced");
    assert!(matches!(
        push_exact_ref(
            &fixture.local,
            &profile,
            &identity,
            advanced,
            "refs/heads/main",
            base,
            no_credentials
        ),
        Ok(PushObservation::Confirmed { .. }),
    ));
    assert!(matches!(
        push_exact_ref(
            &fixture.local,
            &profile,
            &identity,
            candidate,
            "refs/heads/main",
            base,
            no_credentials
        ),
        Ok(PushObservation::Rejected { .. }),
    ));
    assert_eq!(
        fixture.remote.refname_to_id("refs/heads/main").ok(),
        Some(advanced),
    );
}

#[test]
fn ssh_transport_requires_a_lowercase_sha256_host_pin() {
    let profile = NativeRemoteProfile {
        remote_name: "origin".into(),
        remote_url: "ssh://git@example.invalid/repo.git".into(),
        transport: NativeTransport::SshStrictHostKey,
        ssh_host_key_sha256: None,
    };
    assert!(remote_identity(&profile).is_err());
    let mut pinned = profile;
    pinned.ssh_host_key_sha256 = Some("a".repeat(64));
    let first = remote_identity(&pinned)
        .unwrap_or_else(|error| unreachable!("valid test pin rejected: {error}"));
    pinned.ssh_host_key_sha256 = Some("b".repeat(64));
    let second = remote_identity(&pinned)
        .unwrap_or_else(|error| unreachable!("valid test pin rejected: {error}"));
    assert_ne!(first, second);
}
