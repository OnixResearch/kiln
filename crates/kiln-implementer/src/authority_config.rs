//! `kiln.implementation-authority-config.v1`: the one composition helper that
//! selects the implementation authority profile for every protected operation.
//!
//! The operator authors `config/implementation-authority.ncl` and exports it
//! to JSON. This module re-checks every Nickel bound, then composes the
//! selected profile: absent (denies everything), the explicit local operator,
//! or verified UCAN over the packaged or a reviewed policy file, static keys,
//! a bounded proof directory, the durable replay/revocation/decision store and
//! role-separated holder keys.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use kiln_adapter_basalt::compose::{
    ED25519_PUBLIC_KEY_BYTES, proof_store, select_holder_grant, static_keys,
};
use kiln_adapter_basalt::{
    AuthorityServices, BasaltAuthority, HolderKey, RoleSigner, VerifiedAuthorityConfig,
};
use kiln_core::implementation::authority::{AuthoritySelection, LocalOperatorAuthority};
use kiln_core::implementation::grant::GrantRole;
use serde::{Deserialize, Serialize};
use ucan::token::DenyUnknownCaveats;
use ucan::{HolderInvocationLimits, VerificationLimits};

use crate::authority_state::{
    RecordingAuthority, SqliteAuthorityState, SqliteReplayLedger, SqliteRevocationView,
    authority_state_path,
};
use crate::{Implementer, diagnostic, text};

/// Configuration schema.
pub const IMPLEMENTATION_AUTHORITY_CONFIG_SCHEMA: &str = "kiln.implementation-authority-config.v1";
/// Revocation feed schema read from `revocation.path`.
pub const IMPLEMENTATION_AUTHORITY_REVOCATIONS_SCHEMA: &str =
    "kiln.implementation-authority-revocations.v1";
/// Largest exported configuration read.
pub const MAX_AUTHORITY_CONFIG_BYTES: u64 = 64 * 1_024;
/// Largest revocation feed read.
pub const MAX_REVOCATION_FEED_BYTES: u64 = 4 * 1_024 * 1_024;
/// Largest reviewed policy file read.
pub const MAX_POLICY_FILE_BYTES: u64 = 1_024 * 1_024;
/// Most admitted root issuers.
pub const MAX_TRUSTED_ROOTS: usize = 16;
/// Most static verification keys.
pub const MAX_STATIC_KEYS: usize = 64;
/// Most compact tokens loaded from the proof directory.
pub const MAX_PROOF_FILES: usize = 256;
/// Largest compact token file.
pub const MAX_PROOF_FILE_BYTES: u64 = 16_384;
/// Most revoked references in one feed.
pub const MAX_REVOKED_REFERENCES: usize = 65_536;
/// Longest configured path.
pub const MAX_AUTHORITY_PATH_BYTES: usize = 4_096;
/// Longest audience, operator or DID.
pub const MAX_AUTHORITY_NAME_BYTES: usize = 256;
/// Smallest `revocation.max_age_ms`. Decisions read the shell clock to the
/// whole second and check the view at the last millisecond of that second, so
/// a sub-second bound could never be met reliably; it is refused instead.
pub const MIN_REVOCATION_MAX_AGE_MS: u64 = 1_000;
/// Extension of compact tokens in the proof directory.
pub const PROOF_FILE_EXTENSION: &str = "ucan";
/// Width of hex-encoded 32-byte values.
const HEX32_CHARS: usize = 64;
/// Ed25519 seed width in a holder seed file.
const HOLDER_SEED_BYTES: u64 = 32;

/// Exported `kiln.implementation-authority-config.v1`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationAuthorityConfig {
    pub schema: String,
    pub profile: ProfileConfig,
}

/// The selected profile.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ProfileConfig {
    /// Every protected operation denies.
    Absent,
    /// The explicit local-operator compatibility profile.
    LocalOperator { operator: String },
    /// Verified UCAN.
    VerifiedUcan(Box<VerifiedUcanConfig>),
}

/// Verified UCAN profile.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedUcanConfig {
    pub audience: String,
    pub generation: u64,
    pub policy: PolicyConfig,
    pub trusted_roots: Vec<String>,
    pub keys: Vec<KeyConfig>,
    pub proofs_dir: PathBuf,
    pub revocation: RevocationConfig,
    pub replay: ReplayConfig,
    pub holders: HoldersConfig,
    pub projection_enabled: bool,
}

/// Packaged Basalt policy or one reviewed policy file.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PolicyConfig {
    Packaged,
    File { path: PathBuf, blake3: String },
}

/// One static verification key.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyConfig {
    pub did: String,
    pub ed25519: String,
}

/// Revocation feed and its maximum age.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevocationConfig {
    pub path: PathBuf,
    pub max_age_ms: u64,
}

/// Replay-record retention.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayConfig {
    pub retention_ms: u64,
}

/// Role-separated holder secrets.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HoldersConfig {
    pub coordinator: SecretRef,
    pub verifier: SecretRef,
    pub publisher: SecretRef,
}

/// Reference to an owner-only raw 32-byte Ed25519 seed file. The seed is read
/// once at selection and never enters argv, logs or the request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretRef {
    pub seed_path: PathBuf,
}

/// Revocation feed, `kiln.implementation-authority-revocations.v1`.
///
/// `observed_at_ms` is when the feed's producer observed this revocation
/// state, in Unix milliseconds. The view's age is measured from it, never
/// from when a process imported the feed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevocationFeed {
    pub schema: String,
    pub generation: u64,
    pub observed_at_ms: u64,
    pub revoked: Vec<String>,
}

impl ImplementationAuthorityConfig {
    /// The absent profile.
    #[must_use]
    pub fn absent() -> Self {
        Self {
            schema: IMPLEMENTATION_AUTHORITY_CONFIG_SCHEMA.into(),
            profile: ProfileConfig::Absent,
        }
    }

    /// The explicit local-operator profile.
    #[must_use]
    pub fn local_operator(operator: impl Into<String>) -> Self {
        Self {
            schema: IMPLEMENTATION_AUTHORITY_CONFIG_SCHEMA.into(),
            profile: ProfileConfig::LocalOperator {
                operator: operator.into(),
            },
        }
    }

    /// Read and validate one exported configuration.
    ///
    /// # Errors
    /// Returns a message for an unreadable, oversized, malformed or invalid file.
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = read_bounded(path, MAX_AUTHORITY_CONFIG_BYTES)?;
        let config: Self = serde_json::from_slice(&bytes)
            .map_err(|error| format!("authority config is malformed: {error}"))?;
        config.validate()?;
        Ok(config)
    }

    /// Resolve a shell's selection: exactly one of an explicit local operator
    /// or an exported configuration, or the absent profile.
    ///
    /// # Errors
    /// Returns a message when both are given or the configuration is invalid.
    pub fn resolve(local_operator: Option<&str>, config: Option<&Path>) -> Result<Self, String> {
        match (local_operator, config) {
            (Some(_), Some(_)) => {
                Err("select either a local operator or an authority config, not both".into())
            }
            (Some(operator), None) => {
                let config = Self::local_operator(operator);
                config.validate()?;
                Ok(config)
            }
            (None, Some(path)) => Self::load(path),
            (None, None) => Ok(Self::absent()),
        }
    }

    // r[impl kiln.implementation.authority.config]
    /// Re-check every bound the Nickel contract enforces.
    ///
    /// # Errors
    /// Returns the first violated bound.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != IMPLEMENTATION_AUTHORITY_CONFIG_SCHEMA {
            return Err("unsupported authority config schema".into());
        }
        match &self.profile {
            ProfileConfig::Absent => Ok(()),
            ProfileConfig::LocalOperator { operator } => bounded_name(operator, "local operator"),
            ProfileConfig::VerifiedUcan(verified) => verified.validate(),
        }
    }
}

impl VerifiedUcanConfig {
    fn validate(&self) -> Result<(), String> {
        bounded_name(&self.audience, "audience")?;
        if self.generation == 0 {
            return Err("authority generation must be non-zero".into());
        }
        if self.trusted_roots.is_empty() || self.trusted_roots.len() > MAX_TRUSTED_ROOTS {
            return Err("trusted roots must be non-empty and bounded".into());
        }
        for root in &self.trusted_roots {
            did_key(root)?;
        }
        if self.trusted_roots.iter().collect::<BTreeSet<_>>().len() != self.trusted_roots.len() {
            return Err("trusted roots contain a duplicate DID".into());
        }
        if self.keys.is_empty() || self.keys.len() > MAX_STATIC_KEYS {
            return Err("static keys must be non-empty and bounded".into());
        }
        let mut dids = BTreeSet::new();
        for key in &self.keys {
            did_key(&key.did)?;
            hex32(&key.ed25519, "key ed25519")?;
            if !dids.insert(key.did.as_str()) {
                return Err("static keys contain a duplicate DID".into());
            }
        }
        if let PolicyConfig::File { path, blake3 } = &self.policy {
            absolute(path, "policy path")?;
            hex32(blake3, "policy blake3")?;
        }
        absolute(&self.proofs_dir, "proofs_dir")?;
        absolute(&self.revocation.path, "revocation path")?;
        if self.revocation.max_age_ms < MIN_REVOCATION_MAX_AGE_MS {
            return Err(format!(
                "revocation max_age_ms must be at least {MIN_REVOCATION_MAX_AGE_MS}: decisions read time to the whole second"
            ));
        }
        if self.replay.retention_ms == 0 {
            return Err("replay retention must be non-zero".into());
        }
        let seeds = [
            &self.holders.coordinator.seed_path,
            &self.holders.verifier.seed_path,
            &self.holders.publisher.seed_path,
        ];
        for seed in seeds {
            absolute(seed, "holder seed path")?;
        }
        if seeds.iter().collect::<BTreeSet<_>>().len() != seeds.len() {
            return Err("each holder role needs its own seed".into());
        }
        Ok(())
    }
}

fn bounded_name(value: &str, field: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > MAX_AUTHORITY_NAME_BYTES
        || value.trim() != value
        || !value.is_ascii()
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(format!("{field} must be bounded non-empty trimmed ASCII"));
    }
    Ok(())
}

fn did_key(value: &str) -> Result<(), String> {
    bounded_name(value, "DID")?;
    if !value.starts_with("did:key:z") {
        return Err("DID must be an Ed25519 did:key".into());
    }
    Ok(())
}

fn hex32(value: &str, field: &str) -> Result<[u8; 32], String> {
    if value.len() != HEX32_CHARS
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{field} must be 32 lowercase hex bytes"));
    }
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let pair = value
            .get(index * 2..index * 2 + 2)
            .ok_or_else(|| format!("{field} must be 32 lowercase hex bytes"))?;
        *byte = u8::from_str_radix(pair, 16).map_err(text)?;
    }
    Ok(bytes)
}

fn absolute(path: &Path, field: &str) -> Result<(), String> {
    if !path.is_absolute()
        || path.as_os_str().len() > MAX_AUTHORITY_PATH_BYTES
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(format!("{field} must be a bounded absolute path"));
    }
    Ok(())
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(text)?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(format!("{} is not a bounded ordinary file", path.display()));
    }
    std::fs::read(path).map_err(text)
}

fn read_seed(reference: &SecretRef) -> Result<[u8; 32], String> {
    let metadata = std::fs::symlink_metadata(&reference.seed_path).map_err(text)?;
    if !metadata.is_file() || metadata.len() != HOLDER_SEED_BYTES {
        return Err("holder seed must be a raw 32-byte file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("holder seed must be readable by its owner only".into());
        }
    }
    let bytes = std::fs::read(&reference.seed_path).map_err(text)?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| "holder seed must be a raw 32-byte file".to_string())
}

fn read_proofs(directory: &Path) -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(directory).map_err(text)? {
        let path = entry.map_err(text)?.path();
        if path.extension().and_then(|extension| extension.to_str()) == Some(PROOF_FILE_EXTENSION) {
            paths.push(path);
        }
        if paths.len() > MAX_PROOF_FILES {
            return Err("proof directory exceeds its file bound".into());
        }
    }
    paths.sort();
    paths
        .iter()
        .map(|path| {
            let bytes = read_bounded(path, MAX_PROOF_FILE_BYTES)?;
            String::from_utf8(bytes)
                .map(|token| token.trim().to_string())
                .map_err(|_| "proof file is not a compact token".to_string())
        })
        .collect()
}

// r[impl kiln.implementation.authority.state.scenario.stale]
/// Import the revocation feed into the durable view, observed at the feed's
/// own `observed_at_ms`.
///
/// Importing never refreshes the view: an unchanged feed keeps its age
/// however often, and however recently, a process imports it, so a feed
/// older than `max_age_ms` refuses every decision until its producer
/// observes revocations again. A feed observed after `now_ms` is refused, so
/// a future-dated feed cannot extend its own freshness. The view only moves
/// to a newer `(generation, observed_at_ms)`: a feed older than the recorded
/// view adds its revocations and leaves the view as it is.
///
/// The whole feed is validated before anything is written, then imported in
/// one transaction: a malformed reference anywhere leaves no revocation and no
/// generation change behind.
///
/// # Errors
/// Returns a message for an unreadable, malformed or future-dated feed, or a
/// failed commit.
pub fn import_revocations(
    state: &SqliteAuthorityState,
    feed: &Path,
    now_ms: u64,
) -> Result<u64, String> {
    let feed: RevocationFeed =
        serde_json::from_slice(&read_bounded(feed, MAX_REVOCATION_FEED_BYTES)?)
            .map_err(|error| format!("revocation feed is malformed: {error}"))?;
    if feed.schema != IMPLEMENTATION_AUTHORITY_REVOCATIONS_SCHEMA
        || feed.revoked.len() > MAX_REVOKED_REFERENCES
    {
        return Err("revocation feed schema or bound is invalid".into());
    }
    if feed.observed_at_ms > now_ms {
        return Err("revocation feed is observed in the future".into());
    }
    let revoked = feed
        .revoked
        .iter()
        .map(|reference| hex32(reference, "revoked reference"))
        .collect::<Result<Vec<_>, _>>()?;
    state.import_revocation_feed(feed.generation, feed.observed_at_ms, &revoked)?;
    Ok(feed.generation)
}

impl Implementer {
    // r[impl kiln.implementation.authority.config]
    // r[impl kiln.implementation.authority.selection.scenario.scoped]
    /// Select the configured authority profile for every protected operation
    /// of one entry point, loading only the holder seeds of `roles`.
    ///
    /// `roles` is fixed by the calling entry point: the holder roles of the
    /// operations it authorizes itself (for a reviewed entry point,
    /// [`entry_point_roles`](kiln_core::implementation::authority::entry_point_roles)).
    /// The verified profile opens no other seed file and its signer holds no
    /// other key, so an operation of another role is refused before it is
    /// signed. The absent and local-operator profiles load no seed.
    ///
    /// The database path is the ledger path the instance was opened with; the
    /// verified profile keeps its durable state beside it.
    ///
    /// # Errors
    /// Returns a message for an invalid configuration, an unreadable policy,
    /// proof, revocation feed or loaded holder seed, or a loaded holder
    /// without exactly one current grant.
    pub fn select_authority(
        &mut self,
        config: &ImplementationAuthorityConfig,
        roles: &BTreeSet<GrantRole>,
    ) -> Result<(), String> {
        config.validate()?;
        match &config.profile {
            ProfileConfig::Absent => {
                self.authority = None;
                self.authority_profile = AuthoritySelection::Absent;
                self.authority_last = None;
                Ok(())
            }
            ProfileConfig::LocalOperator { operator } => {
                let authority =
                    LocalOperatorAuthority::new(operator.clone(), false).map_err(diagnostic)?;
                self.set_authority(
                    Box::new(authority),
                    AuthoritySelection::LocalOperator {
                        operator: operator.clone(),
                    },
                );
                Ok(())
            }
            ProfileConfig::VerifiedUcan(verified) => self.select_verified(verified, roles),
        }
    }

    fn select_verified(
        &mut self,
        config: &VerifiedUcanConfig,
        roles: &BTreeSet<GrantRole>,
    ) -> Result<(), String> {
        let now_ms = self.now_ms();
        let verifier = match &config.policy {
            PolicyConfig::Packaged => VerifiedAuthorityConfig::from_packaged_policy(
                config.generation,
                config.audience.clone(),
                config.trusted_roots.clone(),
            )?,
            PolicyConfig::File { path, blake3 } => {
                let bytes = read_bounded(path, MAX_POLICY_FILE_BYTES)?;
                let json = String::from_utf8(bytes).map_err(text)?;
                VerifiedAuthorityConfig::from_policy_json(
                    &json,
                    blake3,
                    config.generation,
                    config.audience.clone(),
                    config.trusted_roots.clone(),
                )?
            }
        };
        let mut keys = Vec::with_capacity(config.keys.len());
        for key in &config.keys {
            let public: [u8; ED25519_PUBLIC_KEY_BYTES] = hex32(&key.ed25519, "key ed25519")?;
            keys.push((key.did.clone(), public));
        }
        let tokens = read_proofs(&config.proofs_dir)?;
        let resolver = static_keys(&keys)?;
        let proofs = proof_store(&tokens)?;
        let mut holders = Vec::new();
        for (role, reference) in [
            (GrantRole::Coding, &config.holders.coordinator),
            (GrantRole::Verification, &config.holders.verifier),
            (GrantRole::Publication, &config.holders.publisher),
        ] {
            // Another role's seed file is never opened.
            if !roles.contains(&role) {
                continue;
            }
            let seed = read_seed(reference)?;
            let probe = HolderKey::new(role, seed, "probe".into())?;
            let grant = select_holder_grant(
                &tokens,
                probe.did(),
                &resolver,
                &proofs,
                now_ms / 1_000,
                VerificationLimits::default(),
            )?;
            holders.push(HolderKey::new(role, seed, grant)?);
        }
        let state_path = authority_state_path(&self.database)?;
        let state = SqliteAuthorityState::open(&state_path)?;
        import_revocations(&state, &config.revocation.path, now_ms)?;
        let signer = RoleSigner::new(
            &verifier,
            holders,
            Box::new(kiln_adapter_basalt::signer::os_nonce),
        )?;
        let services = AuthorityServices {
            key_resolver: Box::new(resolver),
            proof_store: Box::new(proofs),
            revocations: Box::new(SqliteRevocationView::open(
                &state_path,
                config.revocation.max_age_ms,
            )?),
            caveat_policies: Box::new(DenyUnknownCaveats),
            replay: Box::new(SqliteReplayLedger::open(&state_path)?),
            replay_retention_ms: config.replay.retention_ms,
            verification_limits: VerificationLimits::default(),
            holder_limits: HolderInvocationLimits::default(),
        };
        let authority = BasaltAuthority::verified(verifier, services)?;
        self.set_authority(
            Box::new(RecordingAuthority::new(Box::new(authority), state)),
            AuthoritySelection::Verified(Box::new(signer)),
        );
        Ok(())
    }
}
