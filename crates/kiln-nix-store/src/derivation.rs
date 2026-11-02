//! Bounded derivation admission.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kiln_core::build::BuildConstraints;
use kiln_core::types::{Alias, ContentId, Diagnostic};
use nix_derivation::{Derivation, StorePath};

use crate::{diagnostic, identity};

/// Hard bounds for one derivation admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DerivationBounds {
    pub max_bytes: usize,
    pub max_outputs: usize,
    pub max_inputs: usize,
    pub max_text_bytes: usize,
}

impl DerivationBounds {
    pub const DEFAULT: Self = Self {
        max_bytes: 4 * 1024 * 1024,
        max_outputs: 64,
        max_inputs: 4096,
        max_text_bytes: 4096,
    };
}

/// An output the caller expects the exact derivation to produce.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedOutput {
    pub name: String,
    pub path: PathBuf,
}

/// One exact reference candidate: a store path and its 32-byte hash part.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceCandidate {
    pub path: String,
    pub needle: String,
}

/// One admitted derivation output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedOutput {
    pub name: String,
    pub path: String,
    /// Declared by the derivation rather than only by the caller.
    pub declared_path: Option<String>,
    pub content_address: Option<String>,
}

/// Bounded facts from one parsed derivation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedDerivation {
    pub identity: ContentId,
    pub name: String,
    pub system: String,
    pub outputs: Vec<AdmittedOutput>,
    pub input_sources: Vec<String>,
    pub input_derivations: Vec<String>,
    pub constraints: BuildConstraints,
    pub reference_candidates: Vec<ReferenceCandidate>,
    /// False when an input derivation's outputs are not resolved here.
    pub reference_complete: bool,
}

/// Parse and validate one exact `.drv` against the admitted platform and
/// expected outputs. This is structural admission, not input-modulo path
/// recomputation.
pub fn admit(
    bytes: &[u8],
    path: &Path,
    platform: &str,
    expected: &[ExpectedOutput],
    bounds: DerivationBounds,
) -> Result<AdmittedDerivation, Diagnostic> {
    if bytes.is_empty() || bytes.len() > bounds.max_bytes {
        return Err(diagnostic(
            "nix_derivation_bytes",
            "nix.derivation.bytes",
            "derivation bytes are empty or exceed their named bound",
        ));
    }
    if bounds.max_outputs == 0 || bounds.max_inputs == 0 || bounds.max_text_bytes == 0 {
        return Err(diagnostic(
            "nix_derivation_bounds",
            "nix.derivation.bounds",
            "derivation bounds must be positive",
        ));
    }
    let path_text = path.to_str().ok_or_else(|| {
        diagnostic(
            "nix_derivation_path",
            "nix.derivation.path",
            "derivation path is not UTF-8",
        )
    })?;
    let store_path = StorePath::from_absolute_path(path_text.as_bytes()).map_err(|_| {
        diagnostic(
            "nix_derivation_path",
            "nix.derivation.path",
            "derivation path is not an absolute Nix store path",
        )
    })?;
    let file_name = store_path.name();
    let name = file_name.strip_suffix(".drv").ok_or_else(|| {
        diagnostic(
            "nix_derivation_path",
            "nix.derivation.path",
            "derivation path does not end in .drv",
        )
    })?;
    let derivation = Derivation::from_aterm_bytes(bytes, name).map_err(|_| {
        diagnostic(
            "nix_derivation_parse",
            "nix.derivation.aterm",
            "derivation ATerm is malformed",
        )
    })?;
    if derivation.system() != platform {
        return Err(diagnostic(
            "nix_derivation_system",
            "nix.derivation.system",
            "derivation system differs from the admitted platform",
        ));
    }
    if derivation.builder().is_empty() || derivation.builder().len() > bounds.max_text_bytes {
        return Err(diagnostic(
            "nix_derivation_builder",
            "nix.derivation.builder",
            "derivation builder is empty or exceeds its named bound",
        ));
    }
    let declared = derivation.outputs();
    if declared.is_empty() || declared.len() > bounds.max_outputs {
        return Err(diagnostic(
            "nix_derivation_outputs",
            "nix.derivation.outputs",
            "derivation output count is empty or exceeds its named bound",
        ));
    }
    if expected.is_empty() || expected.len() > bounds.max_outputs {
        return Err(diagnostic(
            "nix_derivation_expected",
            "nix.derivation.expected_outputs",
            "expected output count is empty or exceeds its named bound",
        ));
    }
    let expected_names: BTreeSet<&str> =
        expected.iter().map(|output| output.name.as_str()).collect();
    if expected_names.len() != expected.len() {
        return Err(diagnostic(
            "nix_derivation_expected_order",
            "nix.derivation.expected_outputs",
            "expected outputs must be sorted and unique",
        ));
    }
    let declared_names: BTreeSet<&str> = declared.keys().map(String::as_str).collect();
    if !expected_names.is_subset(&declared_names) {
        return Err(diagnostic(
            "nix_derivation_output_mismatch",
            "nix.derivation.outputs",
            "expected output names are not all declared by the derivation",
        ));
    }
    let mut outputs = Vec::with_capacity(expected.len());
    let mut reference_candidates = Vec::new();
    for output in expected {
        let declared_output = declared.get(&output.name).ok_or_else(|| {
            diagnostic(
                "nix_derivation_output_missing",
                "nix.derivation.outputs",
                "expected output is not declared by the derivation",
            )
        })?;
        let expected_path = output.path.to_str().ok_or_else(|| {
            diagnostic(
                "nix_derivation_output_path",
                "nix.derivation.expected_outputs",
                "expected output path is not UTF-8",
            )
        })?;
        let declared_path = declared_output
            .path(name, &output.name)
            .map_err(|_| {
                diagnostic(
                    "nix_derivation_output_path",
                    "nix.derivation.outputs",
                    "derivation output path cannot be resolved",
                )
            })?
            .map(|path| path.to_absolute_path());
        if let Some(declared_path) = &declared_path {
            if declared_path != expected_path {
                return Err(diagnostic(
                    "nix_derivation_output_path_mismatch",
                    "nix.derivation.outputs",
                    "expected output path differs from the derivation declaration",
                ));
            }
        }
        let needle = needle(expected_path).ok_or_else(|| {
            diagnostic(
                "nix_derivation_output_path",
                "nix.derivation.expected_outputs",
                "expected output path is not a Nix store path",
            )
        })?;
        reference_candidates.push(ReferenceCandidate {
            path: expected_path.to_owned(),
            needle,
        });
        outputs.push(AdmittedOutput {
            name: output.name.clone(),
            path: expected_path.to_owned(),
            declared_path,
            content_address: declared_output
                .fixed_content_address()
                .map(|hash| hash.to_nix_string()),
        });
    }
    let input_sources = derivation
        .input_sources()
        .iter()
        .map(StorePath::to_absolute_path)
        .collect::<Vec<_>>();
    let input_derivations = derivation
        .input_derivations()
        .keys()
        .map(StorePath::to_absolute_path)
        .collect::<Vec<_>>();
    let total_inputs = input_sources.len().saturating_add(input_derivations.len());
    if total_inputs > bounds.max_inputs {
        return Err(diagnostic(
            "nix_derivation_inputs",
            "nix.derivation.inputs",
            "derivation input count exceeds its named bound",
        ));
    }
    for input in input_sources.iter().chain(input_derivations.iter()) {
        if input.len() > bounds.max_text_bytes {
            return Err(diagnostic(
                "nix_derivation_input_text",
                "nix.derivation.inputs",
                "derivation input path exceeds its named text bound",
            ));
        }
        let needle = needle(input).ok_or_else(|| {
            diagnostic(
                "nix_derivation_input_path",
                "nix.derivation.inputs",
                "derivation input is not a Nix store path",
            )
        })?;
        reference_candidates.push(ReferenceCandidate {
            path: input.clone(),
            needle,
        });
    }
    reference_candidates.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.needle.cmp(&right.needle))
    });
    reference_candidates.dedup();
    let reference_complete = input_derivations.is_empty();
    let constraints = BuildConstraints {
        system: Alias::new(platform.to_owned()).map_err(|_| {
            diagnostic(
                "nix_derivation_system",
                "nix.derivation.system",
                "derivation system is not a canonical alias",
            )
        })?,
        min_memory_bytes: None,
        available_read_only_paths: Vec::new(),
        network_access: false,
        provide_bin_sh: true,
    };
    let mut identity_fields: Vec<Vec<u8>> = vec![
        path_text.as_bytes().to_vec(),
        platform.as_bytes().to_vec(),
        name.as_bytes().to_vec(),
    ];
    for output in &outputs {
        identity_fields.push(output.name.as_bytes().to_vec());
        identity_fields.push(output.path.as_bytes().to_vec());
    }
    for input in &input_sources {
        identity_fields.push(input.as_bytes().to_vec());
    }
    for input in &input_derivations {
        identity_fields.push(input.as_bytes().to_vec());
    }
    let fields = identity_fields
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    let identity = identity("kiln.nix-derivation.v1", &fields);
    Ok(AdmittedDerivation {
        identity,
        name: name.to_owned(),
        system: derivation.system().to_owned(),
        outputs,
        input_sources,
        input_derivations,
        constraints,
        reference_candidates,
        reference_complete,
    })
}

/// Parse the 32-byte hash part used by Nix reference scanning from an absolute
/// store path.
#[must_use]
pub fn needle(path: &str) -> Option<String> {
    let store_path = StorePath::from_absolute_path(path.as_bytes()).ok()?;
    let encoded = nix_derivation::nixbase32::encode(store_path.digest());
    Some(encoded.chars().take(32).collect())
}

/// Sorted map of output names to absolute paths.
#[must_use]
pub fn output_bindings(admitted: &AdmittedDerivation) -> BTreeMap<String, String> {
    admitted
        .outputs
        .iter()
        .map(|output| (output.name.clone(), output.path.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix_derivation::DerivationBuilder;

    fn valid() -> (Vec<u8>, PathBuf, String, String) {
        let validated = DerivationBuilder::new("example", "x86_64-linux", "/bin/sh")
            .input_addressed_output("out")
            .argument("-c")
            .build()
            .unwrap_or_else(|_| unreachable!("fixture derivation"));
        let output = validated
            .resolved_outputs()
            .get("out")
            .and_then(|output| output.path.clone())
            .unwrap_or_else(|| unreachable!("fixture output"))
            .to_absolute_path();
        let name = "example";
        let _ = name;
        let path = validated
            .as_derivation()
            .drv_path()
            .unwrap_or_else(|_| unreachable!("fixture drv path"))
            .to_absolute_path();
        (
            validated.as_derivation().to_aterm_bytes(),
            PathBuf::from(path),
            name.into(),
            output,
        )
    }

    // r[verify kiln.nix_store.derivation.scenario.valid]
    #[test]
    fn admits_matching_derivation_and_binds_outputs_inputs_and_identity() {
        let (bytes, path, _, output) = valid();
        let admitted = admit(
            &bytes,
            &path,
            "x86_64-linux",
            &[ExpectedOutput {
                name: "out".into(),
                path: PathBuf::from(&output),
            }],
            DerivationBounds::DEFAULT,
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(admitted.system, "x86_64-linux");
        assert_eq!(admitted.outputs.len(), 1);
        assert_eq!(
            admitted.outputs.first().map(|output| output.path.as_str()),
            Some(output.as_str())
        );
        assert!(admitted.input_sources.is_empty());
        assert!(admitted.reference_complete);
        assert_eq!(admitted.constraints.system.as_str(), "x86_64-linux");
        assert!(admitted.constraints.provide_bin_sh);
        assert!(!admitted.constraints.network_access);
        assert!(
            admitted
                .reference_candidates
                .iter()
                .any(|candidate| candidate.path == output && candidate.needle.len() == 32)
        );
    }

    // r[verify kiln.nix_store.derivation.scenario.mismatch]
    #[test]
    fn rejects_platform_output_name_path_and_malformed_aters() {
        let (bytes, path, _, output) = valid();
        let expected = [ExpectedOutput {
            name: "out".into(),
            path: PathBuf::from(&output),
        }];
        for (bytes, path, platform, expected) in [
            (
                bytes.clone(),
                path.clone(),
                "aarch64-linux".to_owned(),
                expected.to_vec(),
            ),
            (
                bytes.clone(),
                path.clone(),
                "x86_64-linux".to_owned(),
                Vec::new(),
            ),
            (
                bytes.clone(),
                path.clone(),
                "x86_64-linux".to_owned(),
                vec![ExpectedOutput {
                    name: "missing".into(),
                    path: PathBuf::from(&output),
                }],
            ),
            (
                b"not a derivation".to_vec(),
                path.clone(),
                "x86_64-linux".to_owned(),
                expected.to_vec(),
            ),
            (
                bytes,
                path,
                "x86_64-linux".to_owned(),
                vec![ExpectedOutput {
                    name: "out".into(),
                    path: PathBuf::from("/nix/store/00000000000000000000000000000000-other"),
                }],
            ),
        ] {
            assert!(
                admit(
                    &bytes,
                    &path,
                    &platform,
                    &expected,
                    DerivationBounds::DEFAULT
                )
                .is_err()
            );
        }
    }

    #[test]
    fn bounds_and_needles_are_enforced() {
        let (bytes, path, _, output) = valid();
        let expected = [ExpectedOutput {
            name: "out".into(),
            path: PathBuf::from(&output),
        }];
        let tight = DerivationBounds {
            max_bytes: 1,
            ..DerivationBounds::DEFAULT
        };
        assert!(admit(&bytes, &path, "x86_64-linux", &expected, tight).is_err());
        assert_eq!(
            needle("/nix/store/00000000000000000000000000000000-x"),
            Some("0".repeat(32))
        );
        assert_eq!(needle("/not/a/store/path"), None);
    }
}
