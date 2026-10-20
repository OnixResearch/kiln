use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::CapabilityError;
use crate::CapabilityView;

pub const MAX_CAPABILITY_COUNT: usize = 64;
pub const MAX_PROOF_REFERENCE_COUNT: usize = 64;
pub const MAX_CAVEAT_COUNT: usize = 16;

const MIN_CAPABILITY_COUNT: usize = 1;
const MIN_PROOF_REFERENCE_BYTES: usize = 1;
const MIN_CAVEAT_PAYLOAD_BYTES: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DidField {
    Issuer,
    Audience,
}

impl DidField {
    #[must_use]
    pub const fn claim_name(self) -> &'static str {
        match self {
            Self::Issuer => "iss",
            Self::Audience => "aud",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeField {
    NotBefore,
    ExpiresAt,
}

impl TimeField {
    #[must_use]
    pub const fn claim_name(self) -> &'static str {
        match self {
            Self::NotBefore => "nbf",
            Self::ExpiresAt => "exp",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    MissingDid {
        field: DidField,
    },
    EmptyDid {
        field: DidField,
    },
    DidMustBeTrimmed {
        field: DidField,
    },
    MissingCapabilities,
    EmptyCapabilities,
    TooManyCapabilities {
        actual: usize,
        max: usize,
    },
    InvalidCapability {
        index: usize,
        source: CapabilityError,
    },
    TooManyCaveats {
        capability_index: usize,
        actual: usize,
        max: usize,
    },
    InvalidCaveat {
        capability_index: usize,
        caveat_index: usize,
        source: CaveatError,
    },
    MissingProofs,
    TooManyProofReferences {
        actual: usize,
        max: usize,
    },
    InvalidProofReference {
        index: usize,
        source: ProofReferenceError,
    },
    MissingTime {
        field: TimeField,
    },
    InvalidTimeBounds {
        not_before_seconds: u64,
        expires_at_seconds: u64,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDid { field } => write!(formatter, "missing '{}' claim", field.claim_name()),
            Self::EmptyDid { field } => write!(formatter, "'{}' claim must not be empty", field.claim_name()),
            Self::DidMustBeTrimmed { field } => {
                write!(formatter, "'{}' claim must not contain leading or trailing whitespace", field.claim_name())
            }
            Self::MissingCapabilities => formatter.write_str("missing 'att' claim"),
            Self::EmptyCapabilities => formatter.write_str("'att' claim must contain at least one capability"),
            Self::TooManyCapabilities { actual, max } => {
                write!(formatter, "'att' claim contains {actual} capabilities; max is {max}")
            }
            Self::InvalidCapability { index, source } => {
                write!(formatter, "capability at index {index} is invalid: {source}")
            }
            Self::TooManyCaveats {
                capability_index,
                actual,
                max,
            } => {
                write!(formatter, "capability at index {capability_index} contains {actual} caveats; max is {max}")
            }
            Self::InvalidCaveat {
                capability_index,
                caveat_index,
                source,
            } => {
                write!(formatter, "caveat at capability {capability_index}, index {caveat_index} is invalid: {source}")
            }
            Self::MissingProofs => formatter.write_str("missing 'prf' claim"),
            Self::TooManyProofReferences { actual, max } => {
                write!(formatter, "'prf' claim contains {actual} proof references; max is {max}")
            }
            Self::InvalidProofReference { index, source } => {
                write!(formatter, "proof reference at index {index} is invalid: {source}")
            }
            Self::MissingTime { field } => write!(formatter, "missing '{}' claim", field.claim_name()),
            Self::InvalidTimeBounds {
                not_before_seconds,
                expires_at_seconds,
            } => write!(
                formatter,
                "'nbf' must be earlier than 'exp' (nbf={not_before_seconds}, exp={expires_at_seconds})"
            ),
        }
    }
}

impl core::error::Error for Error {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaveatField {
    Domain,
    Type,
    Key,
    Payload,
}

impl CaveatField {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Domain => "domain",
            Self::Type => "type",
            Self::Key => "key",
            Self::Payload => "payload",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaveatError {
    EmptyField { field: CaveatField },
    FieldMustBeTrimmed { field: CaveatField },
    EmptyPayload,
}

impl fmt::Display for CaveatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "caveat field '{}' must not be empty", field.as_str()),
            Self::FieldMustBeTrimmed { field } => {
                write!(formatter, "caveat field '{}' must not contain leading or trailing whitespace", field.as_str())
            }
            Self::EmptyPayload => formatter.write_str("caveat payload bytes must not be empty"),
        }
    }
}

impl core::error::Error for CaveatError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofReferenceError {
    Empty,
}

impl fmt::Display for ProofReferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("proof reference bytes must not be empty"),
        }
    }
}

impl core::error::Error for ProofReferenceError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DidView<'a> {
    value: &'a str,
}

impl<'a> DidView<'a> {
    /// Build a validated borrowed DID claim view.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] when the DID claim is empty or untrimmed.
    pub fn new(field: DidField, value: &'a str) -> Result<Self, Error> {
        let value = validate_did_value(field, value)?;
        Ok(Self { value })
    }

    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caveat {
    domain: String,
    kind: String,
    key: String,
    payload: Vec<u8>,
}

impl Caveat {
    /// Build an owned caveat claim after core validation.
    ///
    /// # Errors
    ///
    /// Returns [`CaveatError`] when identity fields or payload bytes are malformed.
    pub fn new(domain: &str, caveat_type: &str, key: &str, payload: &[u8]) -> Result<Self, CaveatError> {
        let view = CaveatView::new(domain, caveat_type, key, payload)?;
        Ok(Self {
            domain: view.domain().to_owned(),
            kind: view.caveat_type().to_owned(),
            key: view.key().to_owned(),
            payload: view.payload().to_vec(),
        })
    }

    #[must_use]
    pub const fn domain(&self) -> &str {
        self.domain.as_str()
    }
    #[must_use]
    pub const fn caveat_type(&self) -> &str {
        self.kind.as_str()
    }
    #[must_use]
    pub const fn key(&self) -> &str {
        self.key.as_str()
    }
    #[must_use]
    pub const fn payload(&self) -> &[u8] {
        self.payload.as_slice()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaveatInput<'a> {
    pub domain: &'a str,
    pub caveat_type: &'a str,
    pub key: &'a str,
    pub payload: &'a [u8],
}

impl<'a> CaveatInput<'a> {
    #[must_use]
    pub const fn new(domain: &'a str, caveat_type: &'a str, key: &'a str, payload: &'a [u8]) -> Self {
        Self {
            domain,
            caveat_type,
            key,
            payload,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaveatView<'a> {
    domain: &'a str,
    caveat_type: &'a str,
    key: &'a str,
    payload: &'a [u8],
}

impl<'a> CaveatView<'a> {
    /// Build a validated borrowed caveat view.
    ///
    /// # Errors
    ///
    /// Returns [`CaveatError`] when identity fields or payload bytes are malformed.
    pub fn new(domain: &'a str, caveat_type: &'a str, key: &'a str, payload: &'a [u8]) -> Result<Self, CaveatError> {
        let domain = validate_caveat_text(CaveatField::Domain, domain)?;
        let caveat_type = validate_caveat_text(CaveatField::Type, caveat_type)?;
        let key = validate_caveat_text(CaveatField::Key, key)?;
        if payload.len() < MIN_CAVEAT_PAYLOAD_BYTES {
            return Err(CaveatError::EmptyPayload);
        }
        Ok(Self {
            domain,
            caveat_type,
            key,
            payload,
        })
    }

    #[must_use]
    pub const fn domain(self) -> &'a str {
        self.domain
    }
    #[must_use]
    pub const fn caveat_type(self) -> &'a str {
        self.caveat_type
    }
    #[must_use]
    pub const fn key(self) -> &'a str {
        self.key
    }
    #[must_use]
    pub const fn payload(self) -> &'a [u8] {
        self.payload
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capability {
    resource: String,
    ability: String,
    caveats: Vec<Caveat>,
}

impl Capability {
    /// Build an owned capability claim after core validation.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityError`] when the resource or ability is malformed.
    pub fn new(resource: &str, ability: &str) -> Result<Self, CapabilityError> {
        Self::new_with_caveats(resource, ability, &[]).map_err(|error| match error {
            Error::InvalidCapability { source, .. } => source,
            _ => CapabilityError::UnknownField { line_number: 0 },
        })
    }

    /// Build an owned capability claim and caveats after core validation.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] when the capability or caveats are malformed.
    pub fn new_with_caveats(resource: &str, ability: &str, caveats: &[CaveatInput<'_>]) -> Result<Self, Error> {
        let view =
            CapabilityView::new(resource, ability).map_err(|source| Error::InvalidCapability { index: 0, source })?;
        let caveats = build_caveats(0, caveats)?;
        Ok(Self {
            resource: view.resource().to_owned(),
            ability: view.ability().to_owned(),
            caveats,
        })
    }

    /// Borrow this owned capability as a validated view.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityError`] if the stored fields no longer satisfy core
    /// capability rules.
    pub fn view(&self) -> Result<CapabilityView<'_>, CapabilityError> {
        CapabilityView::new(self.resource.as_str(), self.ability.as_str())
    }

    #[must_use]
    pub const fn resource(&self) -> &str {
        self.resource.as_str()
    }

    #[must_use]
    pub const fn ability(&self) -> &str {
        self.ability.as_str()
    }

    #[must_use]
    pub const fn caveats(&self) -> &[Caveat] {
        self.caveats.as_slice()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityInput<'a> {
    pub resource: &'a str,
    pub ability: &'a str,
    pub caveats: &'a [CaveatInput<'a>],
}

impl<'a> CapabilityInput<'a> {
    #[must_use]
    pub const fn new(resource: &'a str, ability: &'a str) -> Self {
        Self {
            resource,
            ability,
            caveats: &[],
        }
    }

    #[must_use]
    pub const fn with_caveats(resource: &'a str, ability: &'a str, caveats: &'a [CaveatInput<'a>]) -> Self {
        Self {
            resource,
            ability,
            caveats,
        }
    }
}

impl<'a> From<CapabilityView<'a>> for CapabilityInput<'a> {
    fn from(view: CapabilityView<'a>) -> Self {
        Self {
            resource: view.resource(),
            ability: view.ability(),
            caveats: &[],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofReferenceView<'a> {
    bytes: &'a [u8],
}

impl<'a> ProofReferenceView<'a> {
    /// Build a validated borrowed proof-reference view.
    ///
    /// # Errors
    ///
    /// Returns [`ProofReferenceError`] when the reference is empty.
    pub fn new(bytes: &'a [u8]) -> Result<Self, ProofReferenceError> {
        validate_proof_reference_bytes(bytes)?;
        Ok(Self { bytes })
    }

    #[must_use]
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofReference {
    bytes: Vec<u8>,
}

impl ProofReference {
    /// Build an owned proof reference after structural validation.
    ///
    /// # Errors
    ///
    /// Returns [`ProofReferenceError`] when the reference is empty.
    pub fn new(bytes: &[u8]) -> Result<Self, ProofReferenceError> {
        let view = ProofReferenceView::new(bytes)?;
        Ok(Self {
            bytes: view.as_bytes().to_vec(),
        })
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }

    #[must_use]
    pub const fn view(&self) -> ProofReferenceView<'_> {
        ProofReferenceView {
            bytes: self.bytes.as_slice(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofReferenceInput<'a> {
    pub bytes: &'a [u8],
}

impl<'a> ProofReferenceInput<'a> {
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
}

impl<'a> From<ProofReferenceView<'a>> for ProofReferenceInput<'a> {
    fn from(view: ProofReferenceView<'a>) -> Self {
        Self { bytes: view.as_bytes() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeBounds {
    not_before_seconds: u64,
    expires_at_seconds: u64,
}

impl TimeBounds {
    /// Build a valid token time window.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidTimeBounds`] when `nbf` is not earlier than
    /// `exp`.
    pub const fn new(not_before_seconds: u64, expires_at_seconds: u64) -> Result<Self, Error> {
        if not_before_seconds >= expires_at_seconds {
            return Err(Error::InvalidTimeBounds {
                not_before_seconds,
                expires_at_seconds,
            });
        }
        Ok(Self {
            not_before_seconds,
            expires_at_seconds,
        })
    }

    #[must_use]
    pub const fn not_before_seconds(self) -> u64 {
        self.not_before_seconds
    }

    #[must_use]
    pub const fn expires_at_seconds(self) -> u64 {
        self.expires_at_seconds
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClaimsInput<'a> {
    pub issuer_did: Option<&'a str>,
    pub audience_did: Option<&'a str>,
    pub capabilities: Option<&'a [CapabilityInput<'a>]>,
    pub proofs: Option<&'a [ProofReferenceInput<'a>]>,
    pub not_before_seconds: Option<u64>,
    pub expires_at_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimsView<'a> {
    issuer: DidView<'a>,
    audience: DidView<'a>,
    capabilities: &'a [CapabilityView<'a>],
    proofs: &'a [ProofReferenceView<'a>],
    time_bounds: TimeBounds,
}

impl<'a> ClaimsView<'a> {
    /// Build a borrowed token-claims view from already separated values.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] when DID claims, capability count, proof count, or
    /// temporal bounds are invalid.
    pub fn new(
        issuer_did: &'a str,
        audience_did: &'a str,
        capabilities: &'a [CapabilityView<'a>],
        proofs: &'a [ProofReferenceView<'a>],
        time_bounds: TimeBounds,
    ) -> Result<Self, Error> {
        validate_capability_count(capabilities.len())?;
        validate_proof_reference_count(proofs.len())?;
        Ok(Self {
            issuer: DidView::new(DidField::Issuer, issuer_did)?,
            audience: DidView::new(DidField::Audience, audience_did)?,
            capabilities,
            proofs,
            time_bounds,
        })
    }

    #[must_use]
    pub const fn issuer(self) -> DidView<'a> {
        self.issuer
    }

    #[must_use]
    pub const fn audience(self) -> DidView<'a> {
        self.audience
    }

    #[must_use]
    pub const fn capabilities(self) -> &'a [CapabilityView<'a>] {
        self.capabilities
    }

    #[must_use]
    pub const fn proofs(self) -> &'a [ProofReferenceView<'a>] {
        self.proofs
    }

    #[must_use]
    pub const fn time_bounds(self) -> TimeBounds {
        self.time_bounds
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claims {
    issuer_did: String,
    audience_did: String,
    capabilities: Vec<Capability>,
    proofs: Vec<ProofReference>,
    time_bounds: TimeBounds,
}

impl Claims {
    /// Validate decoded token-claim fields and build owned claims.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] when a required claim is missing or any claim value is
    /// structurally invalid.
    pub fn from_input(input: ClaimsInput<'_>) -> Result<Self, Error> {
        let issuer_did = required_did(DidField::Issuer, input.issuer_did)?.to_owned();
        let audience_did = required_did(DidField::Audience, input.audience_did)?.to_owned();
        let capabilities = build_capabilities(required_capabilities(input.capabilities)?)?;
        let proofs = build_proof_references(required_proofs(input.proofs)?)?;
        let time_bounds = required_time_bounds(input.not_before_seconds, input.expires_at_seconds)?;

        Ok(Self {
            issuer_did,
            audience_did,
            capabilities,
            proofs,
            time_bounds,
        })
    }

    #[must_use]
    pub const fn issuer_did(&self) -> &str {
        self.issuer_did.as_str()
    }

    #[must_use]
    pub const fn audience_did(&self) -> &str {
        self.audience_did.as_str()
    }

    #[must_use]
    pub const fn capabilities(&self) -> &[Capability] {
        self.capabilities.as_slice()
    }

    #[must_use]
    pub const fn proofs(&self) -> &[ProofReference] {
        self.proofs.as_slice()
    }

    #[must_use]
    pub const fn time_bounds(&self) -> TimeBounds {
        self.time_bounds
    }
}

fn required_did(field: DidField, value: Option<&str>) -> Result<&str, Error> {
    let value = value.ok_or(Error::MissingDid { field })?;
    validate_did_value(field, value)
}

fn validate_did_value(field: DidField, value: &str) -> Result<&str, Error> {
    if value.is_empty() {
        return Err(Error::EmptyDid { field });
    }
    if value.trim() != value {
        return Err(Error::DidMustBeTrimmed { field });
    }
    Ok(value)
}

fn required_capabilities<'a>(
    capabilities: Option<&'a [CapabilityInput<'a>]>,
) -> Result<&'a [CapabilityInput<'a>], Error> {
    capabilities.ok_or(Error::MissingCapabilities)
}

const fn validate_capability_count(count: usize) -> Result<(), Error> {
    if count < MIN_CAPABILITY_COUNT {
        return Err(Error::EmptyCapabilities);
    }
    if count > MAX_CAPABILITY_COUNT {
        return Err(Error::TooManyCapabilities {
            actual: count,
            max: MAX_CAPABILITY_COUNT,
        });
    }
    Ok(())
}

fn build_capabilities(inputs: &[CapabilityInput<'_>]) -> Result<Vec<Capability>, Error> {
    validate_capability_count(inputs.len())?;
    let mut capabilities = Vec::with_capacity(inputs.len());

    for (index, input) in inputs.iter().enumerate() {
        let view = CapabilityView::new(input.resource, input.ability)
            .map_err(|source| Error::InvalidCapability { index, source })?;
        let caveats = build_caveats(index, input.caveats)?;
        capabilities.push(Capability {
            resource: view.resource().to_owned(),
            ability: view.ability().to_owned(),
            caveats,
        });
    }

    Ok(capabilities)
}

fn build_caveats(capability_index: usize, inputs: &[CaveatInput<'_>]) -> Result<Vec<Caveat>, Error> {
    if inputs.len() > MAX_CAVEAT_COUNT {
        return Err(Error::TooManyCaveats {
            capability_index,
            actual: inputs.len(),
            max: MAX_CAVEAT_COUNT,
        });
    }
    let mut caveats = Vec::with_capacity(inputs.len());
    for (caveat_index, input) in inputs.iter().enumerate() {
        let caveat = Caveat::new(input.domain, input.caveat_type, input.key, input.payload).map_err(|source| {
            Error::InvalidCaveat {
                capability_index,
                caveat_index,
                source,
            }
        })?;
        caveats.push(caveat);
    }
    Ok(caveats)
}

fn validate_caveat_text(field: CaveatField, value: &str) -> Result<&str, CaveatError> {
    if value.is_empty() {
        return Err(CaveatError::EmptyField { field });
    }
    if value.trim() != value {
        return Err(CaveatError::FieldMustBeTrimmed { field });
    }
    Ok(value)
}

fn required_proofs<'a>(proofs: Option<&'a [ProofReferenceInput<'a>]>) -> Result<&'a [ProofReferenceInput<'a>], Error> {
    proofs.ok_or(Error::MissingProofs)
}

const fn validate_proof_reference_count(count: usize) -> Result<(), Error> {
    if count > MAX_PROOF_REFERENCE_COUNT {
        return Err(Error::TooManyProofReferences {
            actual: count,
            max: MAX_PROOF_REFERENCE_COUNT,
        });
    }
    Ok(())
}

fn build_proof_references(inputs: &[ProofReferenceInput<'_>]) -> Result<Vec<ProofReference>, Error> {
    validate_proof_reference_count(inputs.len())?;
    let mut proof_references = Vec::with_capacity(inputs.len());

    for (index, input) in inputs.iter().enumerate() {
        let proof_reference =
            ProofReference::new(input.bytes).map_err(|source| Error::InvalidProofReference { index, source })?;
        proof_references.push(proof_reference);
    }

    Ok(proof_references)
}

const fn validate_proof_reference_bytes(bytes: &[u8]) -> Result<(), ProofReferenceError> {
    if bytes.len() < MIN_PROOF_REFERENCE_BYTES {
        return Err(ProofReferenceError::Empty);
    }
    Ok(())
}

fn required_time_bounds(not_before_seconds: Option<u64>, expires_at_seconds: Option<u64>) -> Result<TimeBounds, Error> {
    let not_before_seconds = not_before_seconds.ok_or(Error::MissingTime {
        field: TimeField::NotBefore,
    })?;
    let expires_at_seconds = expires_at_seconds.ok_or(Error::MissingTime {
        field: TimeField::ExpiresAt,
    })?;
    TimeBounds::new(not_before_seconds, expires_at_seconds)
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use super::Capability;
    use super::CapabilityInput;
    use super::Claims;
    use super::ClaimsInput;
    use super::ClaimsView;
    use super::DidField;
    use super::Error;
    use super::MAX_CAPABILITY_COUNT;
    use super::MAX_PROOF_REFERENCE_COUNT;
    use super::ProofReferenceError;
    use super::ProofReferenceInput;
    use super::ProofReferenceView;
    use super::TimeBounds;
    use super::TimeField;
    use crate::CapabilityError;
    use crate::CapabilityView;

    const ISSUER_DID: &str = "did:key:z6Mkissuer";
    const AUDIENCE_DID: &str = "did:key:z6Mkaudience";
    const VALID_RESOURCE: &str = "did:key:z6Mkresource";
    const VALID_ABILITY: &str = "store/add";
    const WILDCARD_ABILITY: &str = "store/*";
    const EMBEDDED_WILDCARD_ABILITY: &str = "sto*re/add";
    const PROOF_BYTES: &[u8] = b"bafyproof";
    const EMPTY_PROOF_BYTES: &[u8] = b"";
    const NOT_BEFORE_SECONDS: u64 = 1_000;
    const EXPIRES_AT_SECONDS: u64 = 2_000;
    const SAME_TIME_SECONDS: u64 = 1_000;
    const FIRST_INDEX: usize = 0;

    #[test]
    fn builds_owned_claims_from_valid_input() {
        let capabilities = [CapabilityInput::new(VALID_RESOURCE, VALID_ABILITY)];
        let proofs = [ProofReferenceInput::new(PROOF_BYTES)];
        let claims = Claims::from_input(valid_input(capabilities.as_slice(), proofs.as_slice()))
            .expect("valid claims should build");

        assert_eq!(claims.issuer_did(), ISSUER_DID);
        assert_eq!(claims.audience_did(), AUDIENCE_DID);
        assert_eq!(claims.capabilities().len(), capabilities.len());
        assert_eq!(claims.capabilities()[FIRST_INDEX].resource(), VALID_RESOURCE);
        assert_eq!(claims.capabilities()[FIRST_INDEX].ability(), VALID_ABILITY);
        assert_eq!(claims.proofs().len(), proofs.len());
        assert_eq!(claims.proofs()[FIRST_INDEX].as_bytes(), PROOF_BYTES);
        assert_eq!(claims.time_bounds().not_before_seconds(), NOT_BEFORE_SECONDS);
        assert_eq!(claims.time_bounds().expires_at_seconds(), EXPIRES_AT_SECONDS);
    }

    #[test]
    fn builds_borrowed_claim_view_from_valid_parts() {
        let capabilities = [CapabilityView::new(VALID_RESOURCE, WILDCARD_ABILITY).expect("capability should validate")];
        let proofs = [ProofReferenceView::new(PROOF_BYTES).expect("proof should validate")];
        let bounds = TimeBounds::new(NOT_BEFORE_SECONDS, EXPIRES_AT_SECONDS).expect("bounds should validate");
        let claims = ClaimsView::new(ISSUER_DID, AUDIENCE_DID, capabilities.as_slice(), proofs.as_slice(), bounds)
            .expect("borrowed claims should validate");

        assert_eq!(claims.issuer().as_str(), ISSUER_DID);
        assert_eq!(claims.audience().as_str(), AUDIENCE_DID);
        assert_eq!(claims.capabilities()[FIRST_INDEX].ability(), WILDCARD_ABILITY);
        assert_eq!(claims.proofs()[FIRST_INDEX].as_bytes(), PROOF_BYTES);
        assert_eq!(claims.time_bounds(), bounds);
    }

    #[test]
    fn accepts_empty_proof_list_when_prf_claim_is_present() {
        let capabilities = [CapabilityInput::new(VALID_RESOURCE, VALID_ABILITY)];
        let proofs = [];
        let claims = Claims::from_input(valid_input(capabilities.as_slice(), proofs.as_slice()))
            .expect("root token claims may have no proof references");

        assert!(claims.proofs().is_empty());
        assert_eq!(claims.capabilities().len(), capabilities.len());
    }

    #[test]
    fn rejects_missing_issuer_claim() {
        let capabilities = [CapabilityInput::new(VALID_RESOURCE, VALID_ABILITY)];
        let proofs = [ProofReferenceInput::new(PROOF_BYTES)];
        let input = ClaimsInput {
            issuer_did: None,
            ..valid_input(capabilities.as_slice(), proofs.as_slice())
        };
        let error = Claims::from_input(input).expect_err("missing issuer should fail");

        assert_eq!(error, Error::MissingDid {
            field: DidField::Issuer
        });
    }

    #[test]
    fn rejects_untrimmed_audience_claim() {
        let capabilities = [CapabilityInput::new(VALID_RESOURCE, VALID_ABILITY)];
        let proofs = [ProofReferenceInput::new(PROOF_BYTES)];
        let input = ClaimsInput {
            audience_did: Some(" did:key:z6Mkaudience"),
            ..valid_input(capabilities.as_slice(), proofs.as_slice())
        };
        let error = Claims::from_input(input).expect_err("untrimmed audience should fail");

        assert_eq!(error, Error::DidMustBeTrimmed {
            field: DidField::Audience
        });
    }

    #[test]
    fn rejects_missing_capability_claim() {
        let proofs = [ProofReferenceInput::new(PROOF_BYTES)];
        let input = ClaimsInput {
            capabilities: None,
            ..valid_input(&[], proofs.as_slice())
        };
        let error = Claims::from_input(input).expect_err("missing attenuations should fail");

        assert_eq!(error, Error::MissingCapabilities);
    }

    #[test]
    fn rejects_empty_capability_claim() {
        let proofs = [ProofReferenceInput::new(PROOF_BYTES)];
        let error =
            Claims::from_input(valid_input(&[], proofs.as_slice())).expect_err("empty attenuations should fail");

        assert_eq!(error, Error::EmptyCapabilities);
    }

    #[test]
    fn rejects_too_many_capabilities() {
        let capabilities = repeated_capability_inputs(MAX_CAPABILITY_COUNT + 1);
        let proofs = [ProofReferenceInput::new(PROOF_BYTES)];
        let error = Claims::from_input(valid_input(capabilities.as_slice(), proofs.as_slice()))
            .expect_err("oversized attenuation list should fail");

        assert_eq!(error, Error::TooManyCapabilities {
            actual: capabilities.len(),
            max: MAX_CAPABILITY_COUNT,
        });
    }

    #[test]
    fn rejects_invalid_capability_claim() {
        let capabilities = [CapabilityInput::new(VALID_RESOURCE, EMBEDDED_WILDCARD_ABILITY)];
        let proofs = [ProofReferenceInput::new(PROOF_BYTES)];
        let error = Claims::from_input(valid_input(capabilities.as_slice(), proofs.as_slice()))
            .expect_err("invalid capability should fail");

        assert_eq!(error, Error::InvalidCapability {
            index: FIRST_INDEX,
            source: CapabilityError::InvalidAbilityCharacter { character: '*' },
        });
    }

    #[test]
    fn rejects_missing_proof_claim() {
        let capabilities = [CapabilityInput::new(VALID_RESOURCE, VALID_ABILITY)];
        let input = ClaimsInput {
            proofs: None,
            ..valid_input(capabilities.as_slice(), &[])
        };
        let error = Claims::from_input(input).expect_err("missing proofs claim should fail");

        assert_eq!(error, Error::MissingProofs);
    }

    #[test]
    fn rejects_empty_proof_reference() {
        let capabilities = [CapabilityInput::new(VALID_RESOURCE, VALID_ABILITY)];
        let proofs = [ProofReferenceInput::new(EMPTY_PROOF_BYTES)];
        let error = Claims::from_input(valid_input(capabilities.as_slice(), proofs.as_slice()))
            .expect_err("empty proof reference should fail");

        assert_eq!(error, Error::InvalidProofReference {
            index: FIRST_INDEX,
            source: ProofReferenceError::Empty,
        });
    }

    #[test]
    fn rejects_too_many_proof_references() {
        let capabilities = [CapabilityInput::new(VALID_RESOURCE, VALID_ABILITY)];
        let proofs = repeated_proof_inputs(MAX_PROOF_REFERENCE_COUNT + 1);
        let error = Claims::from_input(valid_input(capabilities.as_slice(), proofs.as_slice()))
            .expect_err("oversized proof list should fail");

        assert_eq!(error, Error::TooManyProofReferences {
            actual: proofs.len(),
            max: MAX_PROOF_REFERENCE_COUNT,
        });
    }

    #[test]
    fn rejects_missing_expiration_claim() {
        let capabilities = [CapabilityInput::new(VALID_RESOURCE, VALID_ABILITY)];
        let proofs = [ProofReferenceInput::new(PROOF_BYTES)];
        let input = ClaimsInput {
            expires_at_seconds: None,
            ..valid_input(capabilities.as_slice(), proofs.as_slice())
        };
        let error = Claims::from_input(input).expect_err("missing expiration should fail");

        assert_eq!(error, Error::MissingTime {
            field: TimeField::ExpiresAt
        });
    }

    #[test]
    fn rejects_time_bounds_with_no_positive_duration() {
        let capabilities = [CapabilityInput::new(VALID_RESOURCE, VALID_ABILITY)];
        let proofs = [ProofReferenceInput::new(PROOF_BYTES)];
        let input = ClaimsInput {
            expires_at_seconds: Some(SAME_TIME_SECONDS),
            ..valid_input(capabilities.as_slice(), proofs.as_slice())
        };
        let error = Claims::from_input(input).expect_err("zero-duration bounds should fail");

        assert_eq!(error, Error::InvalidTimeBounds {
            not_before_seconds: NOT_BEFORE_SECONDS,
            expires_at_seconds: SAME_TIME_SECONDS,
        });
    }

    #[test]
    fn owned_capability_round_trips_to_borrowed_view() {
        let capability = Capability::new(VALID_RESOURCE, VALID_ABILITY).expect("capability should validate");
        let view = capability.view().expect("owned capability should stay valid");

        assert_eq!(view.resource(), VALID_RESOURCE);
        assert_eq!(view.ability(), VALID_ABILITY);
    }

    fn valid_input<'a>(
        capabilities: &'a [CapabilityInput<'a>],
        proofs: &'a [ProofReferenceInput<'a>],
    ) -> ClaimsInput<'a> {
        ClaimsInput {
            issuer_did: Some(ISSUER_DID),
            audience_did: Some(AUDIENCE_DID),
            capabilities: Some(capabilities),
            proofs: Some(proofs),
            not_before_seconds: Some(NOT_BEFORE_SECONDS),
            expires_at_seconds: Some(EXPIRES_AT_SECONDS),
        }
    }

    fn repeated_capability_inputs(count: usize) -> Vec<CapabilityInput<'static>> {
        vec![CapabilityInput::new(VALID_RESOURCE, VALID_ABILITY); count]
    }

    fn repeated_proof_inputs(count: usize) -> Vec<ProofReferenceInput<'static>> {
        vec![ProofReferenceInput::new(PROOF_BYTES); count]
    }
}
