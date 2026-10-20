use std::fs;
use std::io::Read;
use std::io::Write;
use std::path::Path;

use ucan_core::CapabilityView;

use crate::core;
use crate::error::Error;
use crate::error::Result;

const RESOURCE_FIELD_NAME: &str = "resource";
const ABILITY_FIELD_NAME: &str = "ability";
const FIELD_SEPARATOR: &str = "=";
const LINE_SEPARATOR: &str = "\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaveatDocument {
    pub domain: String,
    pub caveat_type: String,
    pub key: String,
    pub payload: Vec<u8>,
}

impl CaveatDocument {
    /// Build an owned shell caveat document after validating it against the core.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Capability`] when caveat identity or payload bytes are malformed.
    pub fn new(domain: String, caveat_type: String, key: String, payload: Vec<u8>) -> Result<Self> {
        let _validated =
            core::token::CaveatView::new(domain.as_str(), caveat_type.as_str(), key.as_str(), payload.as_slice())?;
        Ok(Self {
            domain,
            caveat_type,
            key,
            payload,
        })
    }

    #[must_use]
    pub const fn domain(&self) -> &str {
        self.domain.as_str()
    }
    #[must_use]
    pub const fn caveat_type(&self) -> &str {
        self.caveat_type.as_str()
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityDocument {
    pub resource: String,
    pub ability: String,
    pub caveats: Vec<CaveatDocument>,
}

impl CapabilityDocument {
    /// Build an owned shell document after validating it against the core.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Capability`] when the provided fields violate core
    /// validation rules.
    pub fn new(resource: String, ability: String) -> Result<Self> {
        Self::with_caveats(resource, ability, Vec::new())
    }

    /// Build an owned shell document with caveats after validating it against the core.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Capability`] when the capability or caveats violate core validation rules.
    pub fn with_caveats(resource: String, ability: String, caveats: Vec<CaveatDocument>) -> Result<Self> {
        let _capability = core::CapabilityView::new(resource.as_str(), ability.as_str())?;
        let caveat_inputs: Vec<core::token::CaveatInput<'_>> = caveats
            .iter()
            .map(|caveat| {
                core::token::CaveatInput::new(
                    caveat.domain.as_str(),
                    caveat.caveat_type.as_str(),
                    caveat.key.as_str(),
                    caveat.payload.as_slice(),
                )
            })
            .collect();
        let _validated =
            core::token::Capability::new_with_caveats(resource.as_str(), ability.as_str(), caveat_inputs.as_slice())?;
        Ok(Self {
            resource,
            ability,
            caveats,
        })
    }

    /// Borrow this owned shell document as a validated core view.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Capability`] when the stored fields no longer satisfy
    /// core validation rules.
    pub fn view(&self) -> Result<CapabilityView<'_>> {
        core::CapabilityView::new(self.resource.as_str(), self.ability.as_str()).map_err(Error::from)
    }
}

/// Read and validate a capability document from disk.
///
/// # Errors
///
/// Returns [`Error::Io`] when the file cannot be read and
/// [`Error::Capability`] when the content is malformed.
pub fn load_capability_from_path(path: impl AsRef<Path>) -> Result<CapabilityDocument> {
    let input = fs::read_to_string(path)?;
    parse_owned_capability(input.as_str())
}

/// Read and validate a capability document from any reader.
///
/// # Errors
///
/// Returns [`Error::Io`] when the reader fails and [`Error::Capability`]
/// when the content is malformed.
pub fn load_capability_from_reader<R: Read>(mut reader: R) -> Result<CapabilityDocument> {
    let mut input = String::new();
    reader.read_to_string(&mut input)?;
    parse_owned_capability(input.as_str())
}

/// Serialize a validated capability document to a writer.
///
/// # Errors
///
/// Returns [`Error::Capability`] when the document is invalid and
/// [`Error::Io`] when the writer fails.
pub fn write_capability_to_writer<W: Write>(mut writer: W, capability: &CapabilityDocument) -> Result<()> {
    let capability = capability.view()?;
    writer.write_all(RESOURCE_FIELD_NAME.as_bytes())?;
    writer.write_all(FIELD_SEPARATOR.as_bytes())?;
    writer.write_all(capability.resource().as_bytes())?;
    writer.write_all(LINE_SEPARATOR.as_bytes())?;
    writer.write_all(ABILITY_FIELD_NAME.as_bytes())?;
    writer.write_all(FIELD_SEPARATOR.as_bytes())?;
    writer.write_all(capability.ability().as_bytes())?;
    writer.write_all(LINE_SEPARATOR.as_bytes())?;
    Ok(())
}

fn parse_owned_capability(input: &str) -> Result<CapabilityDocument> {
    let capability = core::parse_capability_document(input)?;
    Ok(CapabilityDocument {
        resource: capability.resource().to_owned(),
        ability: capability.ability().to_owned(),
        caveats: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Cursor;

    use tempfile::tempdir;

    use super::ABILITY_FIELD_NAME;
    use super::CapabilityDocument;
    use super::FIELD_SEPARATOR;
    use super::LINE_SEPARATOR;
    use super::RESOURCE_FIELD_NAME;
    use super::load_capability_from_path;
    use super::load_capability_from_reader;
    use super::write_capability_to_writer;
    use crate::Error;

    const VALID_RESOURCE: &str = "did:key:z6Mkexample";
    const VALID_ABILITY: &str = "store/add";

    #[test]
    fn loads_valid_capability_from_reader() {
        let input = format!(
            "{RESOURCE_FIELD_NAME}{FIELD_SEPARATOR}{VALID_RESOURCE}{LINE_SEPARATOR}{ABILITY_FIELD_NAME}{FIELD_SEPARATOR}{VALID_ABILITY}{LINE_SEPARATOR}"
        );
        let capability = load_capability_from_reader(Cursor::new(input)).expect("reader input should parse");

        assert_eq!(capability, CapabilityDocument {
            resource: VALID_RESOURCE.to_owned(),
            ability: VALID_ABILITY.to_owned(),
            caveats: Vec::new(),
        });
    }

    #[test]
    fn writes_valid_capability_document() {
        let capability = CapabilityDocument::new(VALID_RESOURCE.to_owned(), VALID_ABILITY.to_owned())
            .expect("capability should validate");
        let mut output = Vec::new();

        write_capability_to_writer(&mut output, &capability).expect("writing should succeed");

        let output = String::from_utf8(output).expect("shell output should remain utf-8");
        let expected = format!(
            "{RESOURCE_FIELD_NAME}{FIELD_SEPARATOR}{VALID_RESOURCE}{LINE_SEPARATOR}{ABILITY_FIELD_NAME}{FIELD_SEPARATOR}{VALID_ABILITY}{LINE_SEPARATOR}"
        );
        assert_eq!(output, expected);
    }

    #[test]
    fn rejects_invalid_capability_from_reader() {
        let input = format!(
            "{RESOURCE_FIELD_NAME}{FIELD_SEPARATOR}{VALID_RESOURCE}{LINE_SEPARATOR}{ABILITY_FIELD_NAME}{FIELD_SEPARATOR}store//add{LINE_SEPARATOR}"
        );
        let error = load_capability_from_reader(Cursor::new(input)).expect_err("invalid ability should fail");

        assert!(matches!(error, Error::Capability { .. }));
    }

    #[test]
    fn reports_missing_file_from_path() {
        let directory = tempdir().expect("tempdir should exist");
        let missing_path = directory.path().join("missing.ucan");
        let error = load_capability_from_path(missing_path).expect_err("missing file should fail");

        assert!(matches!(error, Error::Io { .. }));
    }

    #[test]
    fn loads_valid_capability_from_path() {
        let directory = tempdir().expect("tempdir should exist");
        let capability_path = directory.path().join("capability.ucan");
        let input = format!(
            "{RESOURCE_FIELD_NAME}{FIELD_SEPARATOR}{VALID_RESOURCE}{LINE_SEPARATOR}{ABILITY_FIELD_NAME}{FIELD_SEPARATOR}{VALID_ABILITY}{LINE_SEPARATOR}"
        );
        fs::write(&capability_path, input).expect("capability file should be written");

        let capability = load_capability_from_path(&capability_path).expect("path input should parse");

        assert_eq!(capability.resource, VALID_RESOURCE);
        assert_eq!(capability.ability, VALID_ABILITY);
    }
}
