use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::str::FromStr;

use kiln_core::cob::{CobCapability, CobEffectPlan};
use kiln_core::types::{CobProfileId, CoreResult, Diagnostic};
use radicle_oid::Oid;
use serde::{Deserialize, Serialize};

use crate::status::{
    PINNED_PLATFORM, PINNED_RADICLE_COHORT, PINNED_SECRETSPEC_EXECUTABLE_BLAKE3,
    PINNED_SECRETSPEC_EXECUTABLE_PATH, PINNED_SECRETSPEC_VERSION, PinnedSecretSpecExecutable,
    SecretSpecDeclaration,
};

pub const MUTATION_PROFILE_ID: &str = "radicle-patch-review-v1";
pub const PATCH_TYPE_NAME: &str = "xyz.radicle.patch";
pub const PATCH_SCHEMA_LINE: &str = "patch-actions-0.25.1";
pub const DEFAULT_MAX_PAYLOAD_BYTES: usize = 16_384;
pub const DEFAULT_MAX_HISTORY_ENTRIES: usize = 1_024;
pub const DEFAULT_MAX_HISTORY_BYTES: usize = 16_777_216;
pub const DEFAULT_PREFLIGHT_DEADLINE_MS: u64 = 5_000;
pub const DEFAULT_SIGNER_DEADLINE_MS: u64 = 15_000;
pub const DEFAULT_WRITE_DEADLINE_MS: u64 = 15_000;

const ABSOLUTE_MAX_TARGETS: usize = 256;
const ABSOLUTE_MAX_OPERATIONS: usize = 256;
const ABSOLUTE_MAX_PAYLOAD_BYTES: usize = 1_048_576;
const ABSOLUTE_MAX_HISTORY_ENTRIES: usize = 65_536;
const ABSOLUTE_MAX_HISTORY_BYTES: usize = 536_870_912;
const ABSOLUTE_MAX_DEADLINE_MS: u64 = 300_000;
const MAX_SUMMARY_BYTES: usize = 4_096;
const MAX_DECLARATION_NAME_BYTES: usize = 256;
const MAX_SECRET_NAME_BYTES: usize = 512;
const MAX_PROVIDER_BYTES: usize = 64;
const MAX_IDEMPOTENCY_KEY_BYTES: usize = 128;
const BLAKE3_HEX_LENGTH: usize = 64;

const REQUIRED_NON_CLAIMS: [&str; 7] = [
    "patch review mutation does not complete Kiln attempts or mutate canonical CI state",
    "patch review mutation does not create close archive or merge patches",
    "patch review mutation does not rewrite Git history",
    "local mutation does not prove peer convergence or remote visibility",
    "single-writer admission does not prove actor trust",
    "SecretSpec resolution does not grant ambient collaboration authority",
    "passing conformance does not establish release readiness",
];

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredMutationTarget {
    pub repository_rid: String,
    pub object_type: String,
    pub object_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchVerdict {
    Accept,
    Reject,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchReviewAction {
    pub revision: String,
    pub verdict: PatchVerdict,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationDeadlines {
    pub preflight_ms: u64,
    pub signer_ms: u64,
    pub write_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadicleMutationProfile {
    pub id: CobProfileId,
    pub platform: String,
    pub radicle_cohort: String,
    pub secretspec_version: String,
    pub targets: BTreeSet<DeclaredMutationTarget>,
    /// BLAKE3 action identity to exact typed action.
    pub operations: BTreeMap<String, PatchReviewAction>,
    pub signer: SecretSpecDeclaration,
    pub executable: PinnedSecretSpecExecutable,
    pub max_payload_bytes: usize,
    pub max_history_entries: usize,
    pub max_history_bytes: usize,
    pub deadlines: MutationDeadlines,
    pub single_writer: bool,
    pub allow_patch_create: bool,
    pub allow_patch_close: bool,
    pub allow_patch_merge: bool,
    pub allow_history_rewrite: bool,
    pub non_claims: Vec<String>,
}

impl RadicleMutationProfile {
    pub fn pinned(
        targets: BTreeSet<DeclaredMutationTarget>,
        operations: BTreeMap<String, PatchReviewAction>,
        signer: SecretSpecDeclaration,
        executable: PinnedSecretSpecExecutable,
    ) -> Self {
        Self {
            id: CobProfileId::new(MUTATION_PROFILE_ID).unwrap_or_else(|_| unreachable!()),
            platform: PINNED_PLATFORM.to_string(),
            radicle_cohort: PINNED_RADICLE_COHORT.to_string(),
            secretspec_version: PINNED_SECRETSPEC_VERSION.to_string(),
            targets,
            operations,
            signer,
            executable,
            max_payload_bytes: DEFAULT_MAX_PAYLOAD_BYTES,
            max_history_entries: DEFAULT_MAX_HISTORY_ENTRIES,
            max_history_bytes: DEFAULT_MAX_HISTORY_BYTES,
            deadlines: MutationDeadlines {
                preflight_ms: DEFAULT_PREFLIGHT_DEADLINE_MS,
                signer_ms: DEFAULT_SIGNER_DEADLINE_MS,
                write_ms: DEFAULT_WRITE_DEADLINE_MS,
            },
            single_writer: true,
            allow_patch_create: false,
            allow_patch_close: false,
            allow_patch_merge: false,
            allow_history_rewrite: false,
            non_claims: REQUIRED_NON_CLAIMS.map(str::to_string).to_vec(),
        }
    }
}

#[derive(Serialize)]
struct PatchReviewWire<'a> {
    #[serde(rename = "type")]
    action_type: &'static str,
    revision: &'a str,
    summary: &'a str,
    verdict: PatchVerdict,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    labels: Vec<String>,
}

pub fn render_patch_review(action: &PatchReviewAction) -> CoreResult<Vec<u8>> {
    admit_patch_review(action)?;
    serde_json::to_vec(&PatchReviewWire {
        action_type: "review",
        revision: &action.revision,
        summary: &action.summary,
        verdict: action.verdict,
        labels: Vec::new(),
    })
    .map_err(|_| {
        Diagnostic::new(
            "radicle_mutation_encoding",
            "radicle_mutation.action",
            "Patch review action could not be encoded",
        )
    })
}

pub fn operation_identity(action: &PatchReviewAction) -> CoreResult<String> {
    Ok(blake3::hash(&render_patch_review(action)?).to_string())
}

// r[impl kiln.radicle.mutation_scope]
// r[impl kiln.radicle.mutation_bounds]
pub fn admit_mutation_profile(profile: &RadicleMutationProfile) -> CoreResult<()> {
    admit_mutation_profile_with_executable_policy(profile, true)
}

#[cfg(feature = "status-native-fixtures")]
pub(crate) fn admit_mutation_fixture_profile(profile: &RadicleMutationProfile) -> CoreResult<()> {
    admit_mutation_profile_with_executable_policy(profile, false)
}

fn admit_mutation_profile_with_executable_policy(
    profile: &RadicleMutationProfile,
    require_production_executable: bool,
) -> CoreResult<()> {
    let production_executable = profile.executable.path
        == PathBuf::from(PINNED_SECRETSPEC_EXECUTABLE_PATH)
        && profile.executable.blake3 == PINNED_SECRETSPEC_EXECUTABLE_BLAKE3;
    let deadlines = profile.deadlines;
    if profile.id.as_str() != MUTATION_PROFILE_ID
        || profile.platform != PINNED_PLATFORM
        || profile.radicle_cohort != PINNED_RADICLE_COHORT
        || profile.secretspec_version != PINNED_SECRETSPEC_VERSION
        || profile.executable.version != PINNED_SECRETSPEC_VERSION
        || (require_production_executable && !production_executable)
        || !signer_declaration_admitted(&profile.signer)
        || profile.targets.is_empty()
        || profile.targets.len() > ABSOLUTE_MAX_TARGETS
        || profile.operations.is_empty()
        || profile.operations.len() > ABSOLUTE_MAX_OPERATIONS
        || profile.max_payload_bytes == 0
        || profile.max_payload_bytes > ABSOLUTE_MAX_PAYLOAD_BYTES
        || profile.max_history_entries == 0
        || profile.max_history_entries > ABSOLUTE_MAX_HISTORY_ENTRIES
        || profile.max_history_bytes == 0
        || profile.max_history_bytes > ABSOLUTE_MAX_HISTORY_BYTES
        || !deadline_admitted(deadlines.preflight_ms)
        || !deadline_admitted(deadlines.signer_ms)
        || !deadline_admitted(deadlines.write_ms)
        || !profile.single_writer
        || profile.allow_patch_create
        || profile.allow_patch_close
        || profile.allow_patch_merge
        || profile.allow_history_rewrite
        || profile.non_claims != REQUIRED_NON_CLAIMS.map(str::to_string)
    {
        return Err(Diagnostic::new(
            "radicle_mutation_profile",
            "radicle_mutation.profile",
            "Radicle mutation profile exceeds the pinned authority or bounds",
        ));
    }
    for target in &profile.targets {
        admit_target(target)?;
    }
    for (identity, action) in &profile.operations {
        if identity.len() != BLAKE3_HEX_LENGTH
            || !identity
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || operation_identity(action)? != *identity
            || render_patch_review(action)?.len() > profile.max_payload_bytes
        {
            return Err(Diagnostic::new(
                "radicle_mutation_operation",
                "radicle_mutation.operations",
                "Declared patch review operation has invalid identity or bounds",
            ));
        }
    }
    Ok(())
}

pub fn admit_mutation_plan<'a>(
    profile: &'a RadicleMutationProfile,
    plan: &CobEffectPlan,
) -> CoreResult<&'a PatchReviewAction> {
    if plan.profile_id != profile.id || plan.action != CobCapability::Review {
        return Err(Diagnostic::new(
            "radicle_mutation_action",
            "radicle_mutation.plan",
            "Mutation plan does not select the pinned patch review capability",
        ));
    }
    if plan.repository.provider_namespace.as_str() != "radicle"
        || plan.repository.native_kind.as_str() != "repository"
        || plan.actor.provider_namespace.as_str() != "radicle"
        || plan.actor.native_kind.as_str() != "node"
        || plan.idempotency_key.provider_namespace.as_str() != "kiln"
        || plan.idempotency_key.native_id.as_str().is_empty()
        || plan.idempotency_key.native_id.as_str().len() > MAX_IDEMPOTENCY_KEY_BYTES
        || !plan
            .idempotency_key
            .native_id
            .as_str()
            .chars()
            .all(|character| {
                character.is_ascii_alphanumeric() || character == '-' || character == '_'
            })
        || plan.expected_heads.len() != 1
    {
        return Err(Diagnostic::new(
            "radicle_mutation_plan_identity",
            "radicle_mutation.plan",
            "Mutation plan has invalid provider identities or head count",
        ));
    }
    let target = DeclaredMutationTarget {
        repository_rid: plan.repository.native_id.as_str().to_string(),
        object_type: PATCH_TYPE_NAME.to_string(),
        object_id: plan.object_id.as_str().to_string(),
    };
    if !profile.targets.contains(&target) {
        return Err(Diagnostic::new(
            "radicle_mutation_target",
            "radicle_mutation.target",
            "Mutation plan targets an undeclared repository object",
        ));
    }
    profile
        .operations
        .get(plan.payload_identity.as_str())
        .ok_or_else(|| {
            Diagnostic::new(
                "radicle_mutation_payload",
                "radicle_mutation.payload_identity",
                "Mutation payload identity is not declared by the profile",
            )
        })
}

fn signer_declaration_admitted(declaration: &SecretSpecDeclaration) -> bool {
    let provider_admitted = declaration.provider.as_ref().is_none_or(|provider| {
        !provider.is_empty()
            && provider.len() <= MAX_PROVIDER_BYTES
            && provider
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
    });
    declaration.project_root.is_absolute()
        && !declaration.project.is_empty()
        && declaration.project.len() <= MAX_DECLARATION_NAME_BYTES
        && !declaration.profile.is_empty()
        && declaration.profile.len() <= MAX_DECLARATION_NAME_BYTES
        && !declaration.secret_key.is_empty()
        && declaration.secret_key.len() <= MAX_SECRET_NAME_BYTES
        && declaration.secret_key.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
        && provider_admitted
}

fn admit_patch_review(action: &PatchReviewAction) -> CoreResult<()> {
    if Oid::from_str(&action.revision).is_err()
        || action.summary.is_empty()
        || action.summary.len() > MAX_SUMMARY_BYTES
    {
        return Err(Diagnostic::new(
            "radicle_mutation_review",
            "radicle_mutation.review",
            "Patch review action has an invalid revision or summary",
        ));
    }
    Ok(())
}

fn admit_target(target: &DeclaredMutationTarget) -> CoreResult<()> {
    if !target.repository_rid.starts_with("rad:z")
        || target
            .repository_rid
            .strip_prefix("rad:z")
            .is_none_or(|suffix| {
                suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_alphanumeric())
            })
        || target.object_type != PATCH_TYPE_NAME
        || Oid::from_str(&target.object_id).is_err()
    {
        return Err(Diagnostic::new(
            "radicle_mutation_target",
            "radicle_mutation.targets",
            "Declared mutation target has an invalid RID, type, or object identity",
        ));
    }
    Ok(())
}

const fn deadline_admitted(deadline_ms: u64) -> bool {
    deadline_ms > 0 && deadline_ms <= ABSOLUTE_MAX_DEADLINE_MS
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteFact {
    Applied,
    RejectedBeforeAcceptance,
    AmbiguousAfterAcceptance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteClassification {
    Applied,
    ProviderError,
    TimedOut,
    Unknown,
}

// r[impl kiln.radicle.mutation_bounds]
#[must_use]
pub const fn classify_write(expired: bool, fact: WriteFact) -> WriteClassification {
    match (expired, fact) {
        (_, WriteFact::AmbiguousAfterAcceptance) | (true, WriteFact::Applied) => {
            WriteClassification::Unknown
        }
        (true, WriteFact::RejectedBeforeAcceptance) => WriteClassification::TimedOut,
        (false, WriteFact::Applied) => WriteClassification::Applied,
        (false, WriteFact::RejectedBeforeAcceptance) => WriteClassification::ProviderError,
    }
}
