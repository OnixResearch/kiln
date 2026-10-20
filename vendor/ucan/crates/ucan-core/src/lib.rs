#![no_std]
#![allow(clippy::cargo_common_metadata)]

extern crate alloc;

pub mod holder_invocation;
#[cfg(feature = "peer-auth")]
pub mod peer_auth;
pub mod profile;
pub mod token;
#[cfg(feature = "verification-cache")]
pub mod verification_cache;

use core::fmt;

const ASSIGNMENT_SEPARATOR: char = '=';
const ABILITY_SEGMENT_SEPARATOR: char = '/';
const ALLOWED_ABILITY_HYPHEN: char = '-';
const ALLOWED_ABILITY_UNDERSCORE: char = '_';
const WILDCARD_SEGMENT: &str = "*";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityView<'a> {
    resource: &'a str,
    ability: &'a str,
}

impl<'a> CapabilityView<'a> {
    /// Build a validated capability view from already-separated fields.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityError`] when either field is empty, untrimmed, or
    /// when the ability contains empty or invalid segments.
    pub fn new(resource: &'a str, ability: &'a str) -> Result<Self, CapabilityError> {
        let resource = validate_resource_value(resource)?;
        let ability = validate_ability_value(ability)?;
        Ok(Self { resource, ability })
    }

    #[must_use]
    pub const fn resource(&self) -> &'a str {
        self.resource
    }

    #[must_use]
    pub const fn ability(&self) -> &'a str {
        self.ability
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityField {
    Resource,
    Ability,
}

impl CapabilityField {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Resource => "resource",
            Self::Ability => "ability",
        }
    }

    fn parse(field_name: &str) -> Option<Self> {
        match field_name.trim() {
            "resource" => Some(Self::Resource),
            "ability" => Some(Self::Ability),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityError {
    MissingSeparator { line_number: usize },
    UnknownField { line_number: usize },
    DuplicateField { field: CapabilityField },
    MissingField { field: CapabilityField },
    EmptyFieldValue { field: CapabilityField },
    FieldMustBeTrimmed { field: CapabilityField },
    EmptyAbilitySegment,
    InvalidAbilityCharacter { character: char },
}

impl fmt::Display for CapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSeparator { line_number } => {
                write!(formatter, "line {line_number} must contain '{ASSIGNMENT_SEPARATOR}'")
            }
            Self::UnknownField { line_number } => {
                write!(formatter, "line {line_number} contains an unknown field")
            }
            Self::DuplicateField { field } => {
                write!(formatter, "field '{}' was provided more than once", field.as_str())
            }
            Self::MissingField { field } => {
                write!(formatter, "missing field '{}'", field.as_str())
            }
            Self::EmptyFieldValue { field } => {
                write!(formatter, "field '{}' must not be empty", field.as_str())
            }
            Self::FieldMustBeTrimmed { field } => {
                write!(formatter, "field '{}' must not contain leading or trailing whitespace", field.as_str())
            }
            Self::EmptyAbilitySegment => formatter.write_str("ability segments must not be empty"),
            Self::InvalidAbilityCharacter { character } => {
                write!(formatter, "ability contains invalid character '{character}'")
            }
        }
    }
}

impl core::error::Error for CapabilityError {}

/// Parse a simple `resource=...` / `ability=...` capability document.
///
/// # Errors
///
/// Returns [`CapabilityError`] when a required field is missing, duplicated,
/// malformed, or fails validation.
pub fn parse_capability_document(input: &str) -> Result<CapabilityView<'_>, CapabilityError> {
    let mut resource = None;
    let mut ability = None;

    for (index, raw_line) in input.lines().enumerate() {
        if raw_line.trim().is_empty() {
            continue;
        }

        let line_number = index + 1;
        let (field, value) = parse_assignment_line(raw_line, line_number)?;
        record_field(field, value, &mut resource, &mut ability)?;
    }

    let resource = resource.ok_or(CapabilityError::MissingField {
        field: CapabilityField::Resource,
    })?;
    let ability = ability.ok_or(CapabilityError::MissingField {
        field: CapabilityField::Ability,
    })?;

    CapabilityView::new(resource, ability)
}

fn parse_assignment_line(line: &str, line_number: usize) -> Result<(CapabilityField, &str), CapabilityError> {
    let (field_name, value) =
        line.split_once(ASSIGNMENT_SEPARATOR).ok_or(CapabilityError::MissingSeparator { line_number })?;
    let field = CapabilityField::parse(field_name).ok_or(CapabilityError::UnknownField { line_number })?;
    Ok((field, value))
}

const fn record_field<'a>(
    field: CapabilityField,
    value: &'a str,
    resource: &mut Option<&'a str>,
    ability: &mut Option<&'a str>,
) -> Result<(), CapabilityError> {
    match field {
        CapabilityField::Resource => record_unique_field(field, value, resource),
        CapabilityField::Ability => record_unique_field(field, value, ability),
    }
}

const fn record_unique_field<'a>(
    field: CapabilityField,
    value: &'a str,
    slot: &mut Option<&'a str>,
) -> Result<(), CapabilityError> {
    if slot.is_some() {
        return Err(CapabilityError::DuplicateField { field });
    }
    *slot = Some(value);
    Ok(())
}

/// Validate a capability resource string.
///
/// # Errors
///
/// Returns [`CapabilityError`] when the resource is empty or contains leading
/// or trailing whitespace.
pub fn validate_resource_value(resource: &str) -> Result<&str, CapabilityError> {
    validate_present_trimmed(CapabilityField::Resource, resource)
}

/// Validate a capability ability string.
///
/// # Errors
///
/// Returns [`CapabilityError`] when the ability is empty, untrimmed, contains
/// empty segments, uses unsupported characters, or uses `*` anywhere other
/// than as a full segment.
pub fn validate_ability_value(ability: &str) -> Result<&str, CapabilityError> {
    let ability = validate_present_trimmed(CapabilityField::Ability, ability)?;

    for segment in ability.split(ABILITY_SEGMENT_SEPARATOR) {
        validate_ability_segment(segment)?;
    }

    Ok(ability)
}

fn validate_present_trimmed(field: CapabilityField, value: &str) -> Result<&str, CapabilityError> {
    if value.is_empty() {
        return Err(CapabilityError::EmptyFieldValue { field });
    }
    if value.trim() != value {
        return Err(CapabilityError::FieldMustBeTrimmed { field });
    }
    Ok(value)
}

fn validate_ability_segment(segment: &str) -> Result<(), CapabilityError> {
    if segment.is_empty() {
        return Err(CapabilityError::EmptyAbilitySegment);
    }
    if segment == WILDCARD_SEGMENT {
        return Ok(());
    }

    for character in segment.chars() {
        if !is_allowed_non_wildcard_ability_character(character) {
            return Err(CapabilityError::InvalidAbilityCharacter { character });
        }
    }

    Ok(())
}

const fn is_allowed_non_wildcard_ability_character(character: char) -> bool {
    character.is_ascii_lowercase()
        || character.is_ascii_digit()
        || character == ALLOWED_ABILITY_HYPHEN
        || character == ALLOWED_ABILITY_UNDERSCORE
}

#[cfg(test)]
mod tests {
    use super::CapabilityError;
    use super::CapabilityField;
    use super::CapabilityView;
    use super::parse_capability_document;

    const VALID_RESOURCE: &str = "did:key:z6Mkexample";
    const VALID_ABILITY: &str = "store/add";

    #[test]
    fn parses_valid_capability_document() {
        let document = "resource=did:key:z6Mkexample\nability=store/add\n";
        let capability = parse_capability_document(document).expect("document should parse");

        assert_eq!(capability.resource(), VALID_RESOURCE);
        assert_eq!(capability.ability(), VALID_ABILITY);
    }

    #[test]
    fn constructs_capability_from_valid_parts() {
        let capability = CapabilityView::new(VALID_RESOURCE, VALID_ABILITY).expect("parts should validate");

        assert_eq!(capability.resource(), VALID_RESOURCE);
        assert_eq!(capability.ability(), VALID_ABILITY);
    }

    #[test]
    fn rejects_duplicate_resource_field() {
        let document = "resource=did:key:first\nresource=did:key:second\nability=store/add\n";
        let error = parse_capability_document(document).expect_err("duplicate field should fail");

        assert_eq!(error, CapabilityError::DuplicateField {
            field: CapabilityField::Resource,
        });
    }

    #[test]
    fn rejects_missing_ability_field() {
        let document = "resource=did:key:z6Mkexample\n";
        let error = parse_capability_document(document).expect_err("missing field should fail");

        assert_eq!(error, CapabilityError::MissingField {
            field: CapabilityField::Ability,
        });
    }

    #[test]
    fn rejects_untrimmed_resource_value() {
        let error =
            CapabilityView::new(" did:key:z6Mkexample", VALID_ABILITY).expect_err("untrimmed resource should fail");

        assert_eq!(error, CapabilityError::FieldMustBeTrimmed {
            field: CapabilityField::Resource,
        });
    }

    #[test]
    fn rejects_empty_ability_segments() {
        let error = CapabilityView::new(VALID_RESOURCE, "store//add").expect_err("empty ability segment should fail");

        assert_eq!(error, CapabilityError::EmptyAbilitySegment);
    }

    #[test]
    fn rejects_uppercase_ability_characters() {
        let error = CapabilityView::new(VALID_RESOURCE, "store/Add").expect_err("uppercase ability should fail");

        assert_eq!(error, CapabilityError::InvalidAbilityCharacter { character: 'A' });
    }

    #[test]
    fn allows_glob_ability_segments() {
        let capability = CapabilityView::new(VALID_RESOURCE, "store/*").expect("glob segment should validate");

        assert_eq!(capability.ability(), "store/*");
    }

    #[test]
    fn rejects_wildcard_inside_ability_segment() {
        let error =
            CapabilityView::new(VALID_RESOURCE, "sto*re/add").expect_err("wildcard inside a segment should fail");

        assert_eq!(error, CapabilityError::InvalidAbilityCharacter { character: '*' });
    }
}
