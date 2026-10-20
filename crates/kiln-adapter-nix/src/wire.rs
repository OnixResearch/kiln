use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use kiln_core::types::{Alias, CoreResult, Diagnostic};
use serde::{Deserialize, Serialize};

pub const REQUEST_SCHEMA: &str = "kiln.nix-build-request.v1";
pub const OBSERVATION_SCHEMA: &str = "kiln.nix-build-observation.v1";
pub const RECEIPT_SCHEMA: &str = "kiln.nix-build-receipt.v1";
pub const NATIVE_REPORT_SCHEMA: &str = "nix-store-realisations.v1";
pub const MAXIMUM_REQUIRED_PRODUCTS: usize = 16;
pub const MAXIMUM_PRODUCTS: usize = 64;
pub const MAXIMUM_LOGS: usize = 1;
pub const MAXIMUM_GC_ROOTS: usize = 256;
pub const MAXIMUM_PATH_BYTES: usize = 4096;
pub const MAXIMUM_PRODUCT_NAME_BYTES: usize = 64;

pub const REQUIRED_NON_CLAIMS: [&str; 7] = [
    "build observation does not prove build correctness",
    "build observation does not prove sandbox completeness",
    "cache observation does not prove cache truth",
    "product admission does not prove product semantics",
    "build observation does not prove reproducibility",
    "nix build execution does not prove derivation provenance or source trust",
    "build observation does not establish release readiness",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildRequest {
    pub schema: String,
    pub derivation_path: String,
    pub required_products: Vec<Alias>,
}

impl BuildRequest {
    pub fn validate(&self, max_required_products: usize) -> CoreResult<()> {
        if self.schema != REQUEST_SCHEMA {
            return Err(diagnostic(
                "nix_request_schema",
                "nix.request.schema",
                "Nix build request schema is unsupported",
            ));
        }
        if !is_safe_absolute_path(&self.derivation_path)
            || Path::new(&self.derivation_path)
                .extension()
                .is_none_or(|extension| extension != "drv")
        {
            return Err(diagnostic(
                "nix_request_derivation",
                "nix.request.derivation_path",
                "Nix build request derivation is not a safe absolute derivation path",
            ));
        }
        if max_required_products == 0
            || self.required_products.is_empty()
            || self.required_products.len() > max_required_products
        {
            return Err(diagnostic(
                "nix_request_products",
                "nix.request.required_products",
                "Nix build request product count is empty or exceeds its named bound",
            ));
        }
        let mut previous = None;
        for product in &self.required_products {
            if !is_valid_product_name(product.as_str()) {
                return Err(diagnostic(
                    "nix_request_product_name",
                    "nix.request.required_products",
                    "Nix build request product name is not a bounded output name",
                ));
            }
            if previous.is_some_and(|previous| previous >= product) {
                return Err(diagnostic(
                    "nix_request_product_order",
                    "nix.request.required_products",
                    "Nix build request products must be sorted and unique",
                ));
            }
            previous = Some(product);
        }
        Ok(())
    }
}

pub fn parse_realisation_roots(bytes: &[u8], max_roots: usize) -> CoreResult<Vec<PathBuf>> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        diagnostic(
            "nix_realisation_roots_utf8",
            "nix.realisation.roots",
            "Nix realisation output is not valid UTF-8",
        )
    })?;
    let mut roots = Vec::new();
    let mut unique = BTreeSet::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !is_safe_absolute_path(line) {
            return Err(diagnostic(
                "nix_realisation_root_path",
                "nix.realisation.roots",
                "Nix realisation root is not a safe absolute path",
            ));
        }
        let path = PathBuf::from(line);
        if !unique.insert(path.clone()) {
            return Err(diagnostic(
                "nix_realisation_root_duplicate",
                "nix.realisation.roots",
                "Nix realisation output repeats a root path",
            ));
        }
        roots.push(path);
        if roots.len() > max_roots {
            return Err(diagnostic(
                "nix_realisation_root_bound",
                "nix.realisation.roots",
                "Nix realisation root count exceeds its named bound",
            ));
        }
    }
    if roots.is_empty() {
        return Err(diagnostic(
            "nix_realisation_root_missing",
            "nix.realisation.roots",
            "Nix realisation output contains no roots",
        ));
    }
    Ok(roots)
}

#[must_use]
pub fn is_safe_absolute_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_PATH_BYTES
        && value.starts_with('/')
        && Path::new(value)
            .components()
            .all(|component| match component {
                Component::RootDir => true,
                Component::Normal(part) => part.to_str().is_some_and(|text| {
                    !text.is_empty()
                        && text.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric()
                                || matches!(byte, b'.' | b'_' | b'-' | b'+' | b'?' | b'=')
                        })
                }),
                Component::CurDir | Component::ParentDir | Component::Prefix(_) => false,
            })
}

#[must_use]
pub fn is_valid_product_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_PRODUCT_NAME_BYTES
        && value
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn diagnostic(code: &str, field: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, field, message)
}
