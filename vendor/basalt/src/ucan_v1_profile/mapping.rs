use crate::ucan_enforcement::EnforcementAction;
use crate::ucan_semantics_identity::{UCAN_RUNTIME_REVISION, UCAN_SUPPORT_CATALOG_BLAKE3};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fmt;
use ucan::profile::v1_authorization::FinalCommand;

const MAPPING_SCHEMA: &str = "basalt.ucan-v1-command-mapping.v1";
const FINAL_PROFILE: &str = "ucan-wg-1.0.0";
const MAXIMUM_MAPPING_ENTRIES: usize = 64;
const REQUIRED_NON_CLAIM_COUNT: usize = 5;
const REQUIRED_NON_CLAIMS: [&str; REQUIRED_NON_CLAIM_COUNT] = [
    "mapping does not prove key trust",
    "external resource ownership remains caller evidence",
    "mapping does not prove revocation freshness or replay durability",
    "mapping does not prove transport or effect correctness",
    "mapping does not prove release eligibility or whole-system safety",
];

/// Checked-in output of the reviewed Nickel mapping.
pub const FINAL_PROFILE_MAPPING_JSON: &str =
    include_str!("../../contracts/ucan-v1-profile/generated/mapping.json");

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum SubjectClass {
    Exact,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum ResourceBinding {
    ExternalOwner,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct MappingEntryDto {
    command: String,
    action: EnforcementAction,
    subject_class: SubjectClass,
    resource_binding: ResourceBinding,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct MappingDto {
    schema: String,
    protocol_profile: String,
    runtime_revision: String,
    support_catalog_blake3: String,
    entries: Vec<MappingEntryDto>,
    non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MappingEntry {
    command: FinalCommand,
    action: EnforcementAction,
}

/// Validated closed final-profile command mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalProfileMapping {
    entries: Vec<MappingEntry>,
    identity: [u8; blake3::OUT_LEN],
}

impl FinalProfileMapping {
    /// Return the deterministic BLAKE3 identity of the exact mapping bytes.
    #[must_use]
    pub const fn identity(&self) -> &[u8; blake3::OUT_LEN] {
        &self.identity
    }

    pub(super) fn action_for(&self, command: &str) -> Option<EnforcementAction> {
        self.entries
            .iter()
            .find(|entry| entry.command.as_str() == command)
            .map(|entry| entry.action)
    }
}

/// Stable final-profile mapping rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalProfileMappingError {
    Malformed,
    SchemaMismatch,
    ProfileMismatch,
    SourceMismatch,
    Empty,
    LimitExceeded,
    InvalidCommand,
    DuplicateCommand,
    UnsupportedBinding,
    NonClaimsWeakened,
}

impl fmt::Display for FinalProfileMappingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Malformed => "final-profile mapping JSON is malformed",
            Self::SchemaMismatch => "final-profile mapping schema does not match",
            Self::ProfileMismatch => "final-profile mapping selects the wrong protocol profile",
            Self::SourceMismatch => "final-profile mapping selects the wrong UCAN source cohort",
            Self::Empty => "final-profile mapping has no entries",
            Self::LimitExceeded => "final-profile mapping exceeds its entry limit",
            Self::InvalidCommand => "final-profile mapping contains an invalid command",
            Self::DuplicateCommand => "final-profile mapping contains a duplicate command",
            Self::UnsupportedBinding => {
                "final-profile mapping weakens its subject or resource binding"
            }
            Self::NonClaimsWeakened => "final-profile mapping weakens its non-claims",
        })
    }
}

impl std::error::Error for FinalProfileMappingError {}

/// Parse and validate one exact Nickel-exported final-profile mapping.
///
/// # Errors
///
/// Returns a stable error for malformed, stale, ambiguous, or weakened input.
///
/// # Panics
///
/// Panics only if the validated unique-command invariant is broken internally.
pub fn parse_final_profile_mapping(
    json: &str,
) -> Result<FinalProfileMapping, FinalProfileMappingError> {
    let dto: MappingDto =
        serde_json::from_str(json).map_err(|_| FinalProfileMappingError::Malformed)?;
    if dto.schema != MAPPING_SCHEMA {
        return Err(FinalProfileMappingError::SchemaMismatch);
    }
    if dto.protocol_profile != FINAL_PROFILE {
        return Err(FinalProfileMappingError::ProfileMismatch);
    }
    if dto.runtime_revision != UCAN_RUNTIME_REVISION
        || dto.support_catalog_blake3 != UCAN_SUPPORT_CATALOG_BLAKE3
    {
        return Err(FinalProfileMappingError::SourceMismatch);
    }
    if dto.entries.is_empty() {
        return Err(FinalProfileMappingError::Empty);
    }
    if dto.entries.len() > MAXIMUM_MAPPING_ENTRIES {
        return Err(FinalProfileMappingError::LimitExceeded);
    }
    if dto.non_claims != REQUIRED_NON_CLAIMS.map(str::to_owned) {
        return Err(FinalProfileMappingError::NonClaimsWeakened);
    }

    let mut seen = BTreeSet::new();
    let mut entries = Vec::with_capacity(dto.entries.len());
    for entry in dto.entries {
        if entry.subject_class != SubjectClass::Exact
            || entry.resource_binding != ResourceBinding::ExternalOwner
        {
            return Err(FinalProfileMappingError::UnsupportedBinding);
        }
        let command = FinalCommand::new(entry.command.as_str())
            .map_err(|_| FinalProfileMappingError::InvalidCommand)?;
        if !seen.insert(entry.command) {
            return Err(FinalProfileMappingError::DuplicateCommand);
        }
        entries.push(MappingEntry {
            command,
            action: entry.action,
        });
    }
    assert_eq!(
        entries.len(),
        seen.len(),
        "validated commands must remain unique"
    );
    Ok(FinalProfileMapping {
        entries,
        identity: *blake3::hash(json.as_bytes()).as_bytes(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIT_OBJECT_HEX_LENGTH: usize = 40;

    #[test]
    fn checked_mapping_admits_exact_commands() {
        let mapping = parse_final_profile_mapping(FINAL_PROFILE_MAPPING_JSON)
            .expect("checked mapping must parse");
        assert_eq!(
            mapping.action_for("/document/write"),
            Some(EnforcementAction::DocumentWrite)
        );
        assert_eq!(mapping.action_for("/document"), None);
    }

    #[test]
    fn malformed_stale_and_duplicate_mappings_fail_closed() {
        assert_eq!(
            parse_final_profile_mapping("{}"),
            Err(FinalProfileMappingError::Malformed)
        );

        let mut stale: serde_json::Value = serde_json::from_str(FINAL_PROFILE_MAPPING_JSON)
            .expect("checked mapping JSON must parse");
        stale["runtime_revision"] = serde_json::Value::String("0".repeat(GIT_OBJECT_HEX_LENGTH));
        assert_eq!(
            parse_final_profile_mapping(&serde_json::to_string(&stale).expect("JSON")),
            Err(FinalProfileMappingError::SourceMismatch)
        );

        let mut duplicate: serde_json::Value = serde_json::from_str(FINAL_PROFILE_MAPPING_JSON)
            .expect("checked mapping JSON must parse");
        let first = duplicate["entries"][0].clone();
        duplicate["entries"]
            .as_array_mut()
            .expect("entries array")
            .push(first);
        assert_eq!(
            parse_final_profile_mapping(&serde_json::to_string(&duplicate).expect("JSON")),
            Err(FinalProfileMappingError::DuplicateCommand)
        );
    }
}
