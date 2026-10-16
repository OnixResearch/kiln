use kiln_core::types::{ContentId, CoreResult, Diagnostic, EffectId, ExternalRef};
use serde::{Deserialize, Serialize};

pub const IDENTITY_MAPPING_SCHEMA: &str = "kiln.aspen-runtime.identity-mapping.v1";
const BLAKE3_HEX_LENGTH: usize = 64;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KilnArtifactRole {
    SourceManifest,
    InputRoot,
    Output,
    Log,
    Checkpoint,
    Journal,
    Outbox,
    CallbackRequest,
    CallbackOutcome,
}

impl KilnArtifactRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceManifest => "source_manifest",
            Self::InputRoot => "input_root",
            Self::Output => "output",
            Self::Log => "log",
            Self::Checkpoint => "checkpoint",
            Self::Journal => "journal",
            Self::Outbox => "outbox",
            Self::CallbackRequest => "callback_request",
            Self::CallbackOutcome => "callback_outcome",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct KilnIdentity {
    pub namespace: String,
    pub role: KilnArtifactRole,
    pub schema: String,
    pub digest: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct AspenIdentity {
    pub namespace: String,
    pub kind: String,
    pub schema: String,
    pub digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IdentityMappingReceipt {
    pub schema: String,
    pub kiln: KilnIdentity,
    pub aspen: AspenIdentity,
    pub producer_effect: Option<EffectId>,
    pub byte_count: usize,
    pub verification_receipt: ExternalRef,
    pub mapping_identity: ContentId,
}

#[derive(Debug)]
pub struct ContentIdentityInput<'a> {
    pub role: KilnArtifactRole,
    pub kiln_schema: &'a str,
    pub aspen_kind: &'a str,
    pub aspen_schema: &'a str,
    pub bytes: &'a [u8],
    pub producer_effect: Option<EffectId>,
    pub verification_receipt: ExternalRef,
}

// r[impl kiln.aspen_runtime.identity] r[impl kiln.aspen_runtime.content]
pub fn map_content_identity(input: ContentIdentityInput<'_>) -> CoreResult<IdentityMappingReceipt> {
    if input.kiln_schema.is_empty() || input.aspen_kind.is_empty() || input.aspen_schema.is_empty()
    {
        return Err(Diagnostic::new(
            "aspen_identity_missing_role",
            "aspen.identity",
            "identity mapping requires explicit role, kind, and schema bindings",
        ));
    }
    input.verification_receipt.validate()?;
    let digest = blake3::hash(input.bytes).to_hex().to_string();
    let kiln = KilnIdentity {
        namespace: "kiln".to_string(),
        role: input.role,
        schema: input.kiln_schema.to_string(),
        digest: digest.clone(),
    };
    let aspen = AspenIdentity {
        namespace: "molten".to_string(),
        kind: input.aspen_kind.to_string(),
        schema: input.aspen_schema.to_string(),
        digest,
    };
    let material = serde_json::to_vec(&(
        IDENTITY_MAPPING_SCHEMA,
        &kiln,
        &aspen,
        &input.producer_effect,
        input.bytes.len(),
        &input.verification_receipt,
    ))
    .map_err(|_| {
        Diagnostic::new(
            "aspen_mapping_encoding",
            "aspen.identity",
            "identity mapping receipt encoding failed",
        )
    })?;
    let mapping_identity =
        ContentId::from_blake3_hex(blake3::hash(&material).to_hex().to_string())?;
    Ok(IdentityMappingReceipt {
        schema: IDENTITY_MAPPING_SCHEMA.to_string(),
        kiln,
        aspen,
        producer_effect: input.producer_effect,
        byte_count: input.bytes.len(),
        verification_receipt: input.verification_receipt,
        mapping_identity,
    })
}

// r[impl kiln.aspen_runtime.identity]
pub fn admit_mapping(receipt: &IdentityMappingReceipt, bytes: &[u8]) -> CoreResult<()> {
    if receipt.schema != IDENTITY_MAPPING_SCHEMA {
        return Err(Diagnostic::new(
            "aspen_mapping_schema",
            "aspen.identity.schema",
            "identity mapping receipt schema is unsupported",
        ));
    }
    if receipt.kiln.namespace != "kiln" || receipt.aspen.namespace != "molten" {
        return Err(Diagnostic::new(
            "aspen_mapping_namespace",
            "aspen.identity.namespace",
            "Kiln and Aspen identities must retain distinct namespaces",
        ));
    }
    validate_digest(&receipt.kiln.digest)?;
    validate_digest(&receipt.aspen.digest)?;
    let observed = blake3::hash(bytes).to_hex().to_string();
    if receipt.kiln.digest != observed
        || receipt.aspen.digest != observed
        || receipt.byte_count != bytes.len()
    {
        return Err(Diagnostic::new(
            "aspen_mapping_content_drift",
            "aspen.identity.digest",
            "mapped content does not match both recorded identity domains",
        ));
    }
    receipt.verification_receipt.validate()?;
    Ok(())
}

pub fn content_id(identity: &KilnIdentity) -> CoreResult<ContentId> {
    validate_digest(&identity.digest)?;
    ContentId::from_blake3_hex(identity.digest.clone())
}

fn validate_digest(digest: &str) -> CoreResult<()> {
    if digest.len() != BLAKE3_HEX_LENGTH
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(Diagnostic::new(
            "aspen_identity_digest",
            "aspen.identity.digest",
            "identity digest must be canonical lowercase BLAKE3 hex",
        ));
    }
    Ok(())
}
