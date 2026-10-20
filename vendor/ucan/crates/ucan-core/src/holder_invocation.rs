use core::fmt;

pub const HOLDER_INVOCATION_DOMAIN: &[u8] = b"ucan/holder-invocation/v1";
pub const HOLDER_INVOCATION_DIGEST_BYTES: usize = 32;
pub const HOLDER_INVOCATION_NONCE_BYTES: usize = 32;
const LENGTH_PREFIX_BYTES: usize = core::mem::size_of::<u32>();
const VARIABLE_FIELD_COUNT: usize = 3;
const FIXED_WIDTH_FIELD_COUNT: usize = 4;
const FIXED_FIELDS_BYTES: usize = HOLDER_INVOCATION_DIGEST_BYTES * FIXED_WIDTH_FIELD_COUNT;
const VARIABLE_PREFIX_BYTES: usize = LENGTH_PREFIX_BYTES * VARIABLE_FIELD_COUNT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HolderInvocationField {
    Audience,
    Resource,
    Ability,
}

impl HolderInvocationField {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Audience => "audience",
            Self::Resource => "resource",
            Self::Ability => "ability",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HolderInvocationCanonicalError {
    EmptyField {
        field: HolderInvocationField,
    },
    FieldTooLong {
        field: HolderInvocationField,
        actual: usize,
        limit: usize,
    },
    LimitExceedsCanonicalWidth {
        field: HolderInvocationField,
        limit: usize,
    },
    CanonicalLengthOverflow,
    OutputLengthMismatch {
        actual: usize,
        expected: usize,
    },
}

impl fmt::Display for HolderInvocationCanonicalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "{} must not be empty", field.as_str()),
            Self::FieldTooLong { field, actual, limit } => {
                write!(formatter, "{} byte length {actual} exceeds limit {limit}", field.as_str())
            }
            Self::LimitExceedsCanonicalWidth { field, limit } => {
                write!(formatter, "{} limit {limit} exceeds the canonical u32 width", field.as_str())
            }
            Self::CanonicalLengthOverflow => formatter.write_str("holder invocation canonical length overflow"),
            Self::OutputLengthMismatch { actual, expected } => {
                write!(formatter, "canonical output byte length {actual} does not equal exact length {expected}")
            }
        }
    }
}

impl core::error::Error for HolderInvocationCanonicalError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_field_names)]
pub struct HolderInvocationFieldLimits {
    audience_bytes: usize,
    resource_bytes: usize,
    ability_bytes: usize,
}

impl HolderInvocationFieldLimits {
    /// Construct admitted variable-field limits for the canonical format.
    ///
    /// # Errors
    ///
    /// Returns [`HolderInvocationCanonicalError::LimitExceedsCanonicalWidth`]
    /// when a limit cannot fit in the canonical `u32` length prefix.
    pub fn try_new(
        audience_bytes: usize,
        resource_bytes: usize,
        ability_bytes: usize,
    ) -> Result<Self, HolderInvocationCanonicalError> {
        admit_limit(HolderInvocationField::Audience, audience_bytes)?;
        admit_limit(HolderInvocationField::Resource, resource_bytes)?;
        admit_limit(HolderInvocationField::Ability, ability_bytes)?;
        Ok(Self {
            audience_bytes,
            resource_bytes,
            ability_bytes,
        })
    }

    #[must_use]
    pub const fn audience_bytes(self) -> usize {
        self.audience_bytes
    }

    #[must_use]
    pub const fn resource_bytes(self) -> usize {
        self.resource_bytes
    }

    #[must_use]
    pub const fn ability_bytes(self) -> usize {
        self.ability_bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HolderInvocationCanonicalFields<'a> {
    token_reference: &'a [u8; HOLDER_INVOCATION_DIGEST_BYTES],
    audience: &'a str,
    resource: &'a str,
    ability: &'a str,
    operation_profile: &'a [u8; HOLDER_INVOCATION_DIGEST_BYTES],
    request_binding: &'a [u8; HOLDER_INVOCATION_DIGEST_BYTES],
    nonce: &'a [u8; HOLDER_INVOCATION_NONCE_BYTES],
}

impl<'a> HolderInvocationCanonicalFields<'a> {
    #[must_use]
    pub const fn new(
        token_reference: &'a [u8; HOLDER_INVOCATION_DIGEST_BYTES],
        audience: &'a str,
        resource: &'a str,
        ability: &'a str,
        operation_profile: &'a [u8; HOLDER_INVOCATION_DIGEST_BYTES],
        request_binding: &'a [u8; HOLDER_INVOCATION_DIGEST_BYTES],
        nonce: &'a [u8; HOLDER_INVOCATION_NONCE_BYTES],
    ) -> Self {
        Self {
            token_reference,
            audience,
            resource,
            ability,
            operation_profile,
            request_binding,
            nonce,
        }
    }
}

/// Calculate the exact canonical holder-invocation byte length after field admission.
///
/// # Errors
///
/// Returns [`HolderInvocationCanonicalError`] for empty, oversized, or
/// overflowing fields.
pub fn holder_invocation_canonical_len(
    fields: &HolderInvocationCanonicalFields<'_>,
    limits: HolderInvocationFieldLimits,
) -> Result<usize, HolderInvocationCanonicalError> {
    admit_fields(fields, limits)?;
    let variable_bytes = fields
        .audience
        .len()
        .checked_add(fields.resource.len())
        .and_then(|length| length.checked_add(fields.ability.len()))
        .ok_or(HolderInvocationCanonicalError::CanonicalLengthOverflow)?;
    HOLDER_INVOCATION_DOMAIN
        .len()
        .checked_add(FIXED_FIELDS_BYTES)
        .and_then(|length| length.checked_add(VARIABLE_PREFIX_BYTES))
        .and_then(|length| length.checked_add(variable_bytes))
        .ok_or(HolderInvocationCanonicalError::CanonicalLengthOverflow)
}

/// Write one exact canonical holder-invocation request into caller-owned output.
///
/// # Errors
///
/// Returns [`HolderInvocationCanonicalError`] when a field is not admitted or
/// the output slice does not have the exact calculated length.
///
/// # Panics
///
/// Panics only if an internal write violates the admitted exact-length invariant.
pub fn write_holder_invocation_canonical(
    fields: &HolderInvocationCanonicalFields<'_>,
    limits: HolderInvocationFieldLimits,
    output: &mut [u8],
) -> Result<(), HolderInvocationCanonicalError> {
    let expected = holder_invocation_canonical_len(fields, limits)?;
    if output.len() != expected {
        return Err(HolderInvocationCanonicalError::OutputLengthMismatch {
            actual: output.len(),
            expected,
        });
    }

    let mut cursor = 0;
    write_bytes(output, &mut cursor, HOLDER_INVOCATION_DOMAIN);
    write_bytes(output, &mut cursor, fields.token_reference);
    write_variable(output, &mut cursor, fields.audience.as_bytes());
    write_variable(output, &mut cursor, fields.resource.as_bytes());
    write_variable(output, &mut cursor, fields.ability.as_bytes());
    write_bytes(output, &mut cursor, fields.operation_profile);
    write_bytes(output, &mut cursor, fields.request_binding);
    write_bytes(output, &mut cursor, fields.nonce);
    assert_eq!(cursor, expected, "admitted canonical writes must fill exact output");
    Ok(())
}

fn admit_fields(
    fields: &HolderInvocationCanonicalFields<'_>,
    limits: HolderInvocationFieldLimits,
) -> Result<(), HolderInvocationCanonicalError> {
    admit_field(HolderInvocationField::Audience, fields.audience, limits.audience_bytes())?;
    admit_field(HolderInvocationField::Resource, fields.resource, limits.resource_bytes())?;
    admit_field(HolderInvocationField::Ability, fields.ability, limits.ability_bytes())?;
    Ok(())
}

fn admit_limit(field: HolderInvocationField, limit: usize) -> Result<(), HolderInvocationCanonicalError> {
    if u32::try_from(limit).is_err() {
        return Err(HolderInvocationCanonicalError::LimitExceedsCanonicalWidth { field, limit });
    }
    Ok(())
}

const fn admit_field(
    field: HolderInvocationField,
    value: &str,
    limit: usize,
) -> Result<(), HolderInvocationCanonicalError> {
    if value.is_empty() {
        return Err(HolderInvocationCanonicalError::EmptyField { field });
    }
    if value.len() > limit {
        return Err(HolderInvocationCanonicalError::FieldTooLong {
            field,
            actual: value.len(),
            limit,
        });
    }
    Ok(())
}

fn write_variable(output: &mut [u8], cursor: &mut usize, value: &[u8]) {
    let length = u32::try_from(value.len()).expect("admitted variable field length must fit u32");
    write_bytes(output, cursor, &length.to_be_bytes());
    write_bytes(output, cursor, value);
}

fn write_bytes(output: &mut [u8], cursor: &mut usize, value: &[u8]) {
    let end = cursor.checked_add(value.len()).expect("admitted canonical write must not overflow");
    assert!(end <= output.len(), "admitted canonical write must stay inside exact output");
    output[*cursor..end].copy_from_slice(value);
    *cursor = end;
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use super::HOLDER_INVOCATION_DIGEST_BYTES;
    use super::HOLDER_INVOCATION_DOMAIN;
    use super::HOLDER_INVOCATION_NONCE_BYTES;
    use super::HolderInvocationCanonicalError;
    use super::HolderInvocationCanonicalFields;
    use super::HolderInvocationField;
    use super::HolderInvocationFieldLimits;
    use super::holder_invocation_canonical_len;
    use super::write_holder_invocation_canonical;

    const AUDIENCE: &str = "did:key:z6Mkaudience";
    const RESOURCE: &str = "did:key:z6Mkresource/photos";
    const ABILITY: &str = "store/add";
    const AUDIENCE_LIMIT: usize = 128;
    const RESOURCE_LIMIT: usize = 256;
    const ABILITY_LIMIT: usize = 64;
    const TOKEN_REFERENCE_BYTE: u8 = 1;
    const PROFILE_BYTE: u8 = 2;
    const REQUEST_BYTE: u8 = 3;
    const NONCE_BYTE: u8 = 4;
    const SUBSTITUTION_BYTE: u8 = 9;

    fn limits() -> HolderInvocationFieldLimits {
        HolderInvocationFieldLimits::try_new(AUDIENCE_LIMIT, RESOURCE_LIMIT, ABILITY_LIMIT)
            .expect("test limits must fit the canonical format")
    }

    fn canonical(
        token_reference: &[u8; HOLDER_INVOCATION_DIGEST_BYTES],
        audience: &str,
        resource: &str,
        ability: &str,
        operation_profile: &[u8; HOLDER_INVOCATION_DIGEST_BYTES],
        request_binding: &[u8; HOLDER_INVOCATION_DIGEST_BYTES],
        nonce: &[u8; HOLDER_INVOCATION_NONCE_BYTES],
    ) -> Vec<u8> {
        let fields = HolderInvocationCanonicalFields::new(
            token_reference,
            audience,
            resource,
            ability,
            operation_profile,
            request_binding,
            nonce,
        );
        let length = holder_invocation_canonical_len(&fields, limits()).expect("test fields must be admitted");
        let mut output = vec![0_u8; length];
        write_holder_invocation_canonical(&fields, limits(), output.as_mut_slice())
            .expect("exact output must accept canonical bytes");
        output
    }

    #[test]
    fn writes_deterministic_exact_canonical_request() {
        let token_reference = [TOKEN_REFERENCE_BYTE; HOLDER_INVOCATION_DIGEST_BYTES];
        let operation_profile = [PROFILE_BYTE; HOLDER_INVOCATION_DIGEST_BYTES];
        let request_binding = [REQUEST_BYTE; HOLDER_INVOCATION_DIGEST_BYTES];
        let nonce = [NONCE_BYTE; HOLDER_INVOCATION_NONCE_BYTES];

        let first =
            canonical(&token_reference, AUDIENCE, RESOURCE, ABILITY, &operation_profile, &request_binding, &nonce);
        let second =
            canonical(&token_reference, AUDIENCE, RESOURCE, ABILITY, &operation_profile, &request_binding, &nonce);

        assert_eq!(first, second);
        assert!(first.starts_with(HOLDER_INVOCATION_DOMAIN));
    }

    #[test]
    fn every_bound_field_changes_canonical_bytes() {
        let token_reference = [TOKEN_REFERENCE_BYTE; HOLDER_INVOCATION_DIGEST_BYTES];
        let operation_profile = [PROFILE_BYTE; HOLDER_INVOCATION_DIGEST_BYTES];
        let request_binding = [REQUEST_BYTE; HOLDER_INVOCATION_DIGEST_BYTES];
        let nonce = [NONCE_BYTE; HOLDER_INVOCATION_NONCE_BYTES];
        let baseline =
            canonical(&token_reference, AUDIENCE, RESOURCE, ABILITY, &operation_profile, &request_binding, &nonce);
        let substituted_token = [SUBSTITUTION_BYTE; HOLDER_INVOCATION_DIGEST_BYTES];
        let substituted_profile = [SUBSTITUTION_BYTE; HOLDER_INVOCATION_DIGEST_BYTES];
        let substituted_request = [SUBSTITUTION_BYTE; HOLDER_INVOCATION_DIGEST_BYTES];
        let substituted_nonce = [SUBSTITUTION_BYTE; HOLDER_INVOCATION_NONCE_BYTES];
        let candidates = [
            canonical(&substituted_token, AUDIENCE, RESOURCE, ABILITY, &operation_profile, &request_binding, &nonce),
            canonical(
                &token_reference,
                "did:key:z6Mkother",
                RESOURCE,
                ABILITY,
                &operation_profile,
                &request_binding,
                &nonce,
            ),
            canonical(
                &token_reference,
                AUDIENCE,
                "did:key:z6Mkresource/videos",
                ABILITY,
                &operation_profile,
                &request_binding,
                &nonce,
            ),
            canonical(
                &token_reference,
                AUDIENCE,
                RESOURCE,
                "store/remove",
                &operation_profile,
                &request_binding,
                &nonce,
            ),
            canonical(&token_reference, AUDIENCE, RESOURCE, ABILITY, &substituted_profile, &request_binding, &nonce),
            canonical(&token_reference, AUDIENCE, RESOURCE, ABILITY, &operation_profile, &substituted_request, &nonce),
            canonical(
                &token_reference,
                AUDIENCE,
                RESOURCE,
                ABILITY,
                &operation_profile,
                &request_binding,
                &substituted_nonce,
            ),
        ];

        for candidate in candidates {
            assert_ne!(baseline, candidate);
        }
    }

    #[test]
    fn rejects_each_empty_or_oversized_variable_field() {
        let token_reference = [TOKEN_REFERENCE_BYTE; HOLDER_INVOCATION_DIGEST_BYTES];
        let operation_profile = [PROFILE_BYTE; HOLDER_INVOCATION_DIGEST_BYTES];
        let request_binding = [REQUEST_BYTE; HOLDER_INVOCATION_DIGEST_BYTES];
        let nonce = [NONCE_BYTE; HOLDER_INVOCATION_NONCE_BYTES];
        let cases = [
            ("", RESOURCE, ABILITY, HolderInvocationField::Audience),
            (AUDIENCE, "", ABILITY, HolderInvocationField::Resource),
            (AUDIENCE, RESOURCE, "", HolderInvocationField::Ability),
        ];

        for (audience, resource, ability, field) in cases {
            let fields = HolderInvocationCanonicalFields::new(
                &token_reference,
                audience,
                resource,
                ability,
                &operation_profile,
                &request_binding,
                &nonce,
            );
            assert_eq!(
                holder_invocation_canonical_len(&fields, limits()),
                Err(HolderInvocationCanonicalError::EmptyField { field })
            );
        }

        let fields = HolderInvocationCanonicalFields::new(
            &token_reference,
            AUDIENCE,
            RESOURCE,
            ABILITY,
            &operation_profile,
            &request_binding,
            &nonce,
        );
        let zero_limits = HolderInvocationFieldLimits::try_new(0, 0, 0).expect("zero limits fit u32");
        assert_eq!(
            holder_invocation_canonical_len(&fields, zero_limits),
            Err(HolderInvocationCanonicalError::FieldTooLong {
                field: HolderInvocationField::Audience,
                actual: AUDIENCE.len(),
                limit: 0,
            })
        );
    }

    #[test]
    fn rejects_non_exact_output_lengths() {
        let token_reference = [TOKEN_REFERENCE_BYTE; HOLDER_INVOCATION_DIGEST_BYTES];
        let operation_profile = [PROFILE_BYTE; HOLDER_INVOCATION_DIGEST_BYTES];
        let request_binding = [REQUEST_BYTE; HOLDER_INVOCATION_DIGEST_BYTES];
        let nonce = [NONCE_BYTE; HOLDER_INVOCATION_NONCE_BYTES];
        let fields = HolderInvocationCanonicalFields::new(
            &token_reference,
            AUDIENCE,
            RESOURCE,
            ABILITY,
            &operation_profile,
            &request_binding,
            &nonce,
        );
        let exact = holder_invocation_canonical_len(&fields, limits()).expect("test fields must be admitted");
        let short_length = exact.checked_sub(1).expect("canonical request must not be empty");
        let long_length = exact.checked_add(1).expect("test canonical length must not overflow");
        let mut short = vec![0_u8; short_length];
        let mut long = vec![0_u8; long_length];

        assert_eq!(
            write_holder_invocation_canonical(&fields, limits(), short.as_mut_slice()),
            Err(HolderInvocationCanonicalError::OutputLengthMismatch {
                actual: short_length,
                expected: exact,
            })
        );
        assert_eq!(
            write_holder_invocation_canonical(&fields, limits(), long.as_mut_slice()),
            Err(HolderInvocationCanonicalError::OutputLengthMismatch {
                actual: long_length,
                expected: exact,
            })
        );
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn rejects_limits_wider_than_canonical_prefix() {
        let too_wide = usize::try_from(u64::from(u32::MAX) + 1).expect("test host must represent u32 max plus one");
        assert_eq!(
            HolderInvocationFieldLimits::try_new(too_wide, RESOURCE_LIMIT, ABILITY_LIMIT),
            Err(HolderInvocationCanonicalError::LimitExceedsCanonicalWidth {
                field: HolderInvocationField::Audience,
                limit: too_wide,
            })
        );
    }
}
