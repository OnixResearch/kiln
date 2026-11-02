//! Bounded NAR hashing, reference scanning and input-closure facts.

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::Path;

use kiln_core::build::BuildInputObservation;
use kiln_core::types::{ContentId, Diagnostic};
use nix_archive::nar::{CaseHack, ReferencePattern, encode_path, hash_path};

use crate::derivation::ReferenceCandidate;
use crate::{diagnostic, identity};

/// Bounds for one output/input observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NarBounds {
    pub max_nar_bytes: u64,
    pub max_references: usize,
}

impl NarBounds {
    pub const DEFAULT: Self = Self {
        max_nar_bytes: 16 * 1024 * 1024 * 1024,
        max_references: 4096,
    };
}

/// Exact NAR and reference facts for one realized path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreObjectFacts {
    pub nar_sha256: String,
    pub nar_size: u64,
    pub references: Vec<String>,
    /// Candidate indexes matched by the scan, in ascending order.
    pub matches: Vec<usize>,
    /// True only when the supplied candidate set covers the complete closure.
    pub complete: bool,
}

/// Compute NAR size/SHA-256 and match the supplied store-path candidates over
/// the exact tree at `path`. The tree is read once.
pub fn observe_path(
    path: &Path,
    candidates: &[ReferenceCandidate],
    candidates_complete: bool,
    bounds: NarBounds,
) -> Result<StoreObjectFacts, Diagnostic> {
    if !path.is_absolute() || bounds.max_nar_bytes == 0 || bounds.max_references == 0 {
        return Err(diagnostic(
            "nix_nar_input",
            "nix.nar.path",
            "NAR observation needs an absolute path and positive bounds",
        ));
    }
    let needles = candidates
        .iter()
        .map(|candidate| candidate.needle.as_str())
        .collect::<Vec<_>>();
    let pattern = ReferencePattern::new(needles).map_err(|_| {
        diagnostic(
            "nix_nar_candidate",
            "nix.nar.candidates",
            "reference candidate is not a 32-byte Nix-base32 hash part",
        )
    })?;
    let scan = pattern.scan_path(path, CaseHack::native()).map_err(|_| {
        diagnostic(
            "nix_nar_scan",
            "nix.nar.path",
            "NAR encoding or reference scan failed",
        )
    })?;
    if scan.nar_size > bounds.max_nar_bytes {
        return Err(diagnostic(
            "nix_nar_bound",
            "nix.nar.size",
            "NAR exceeds the named byte bound",
        ));
    }
    let mut references = BTreeSet::new();
    for index in &scan.matches {
        let candidate = candidates.get(*index).ok_or_else(|| {
            diagnostic(
                "nix_nar_candidate_index",
                "nix.nar.candidates",
                "reference scan returned an out-of-range candidate index",
            )
        })?;
        references.insert(candidate.path.clone());
    }
    if references.len() > bounds.max_references {
        return Err(diagnostic(
            "nix_nar_reference_bound",
            "nix.nar.references",
            "NAR reference count exceeds its named bound",
        ));
    }
    Ok(StoreObjectFacts {
        nar_sha256: hex(&scan.nar_sha256),
        nar_size: scan.nar_size,
        references: references.into_iter().collect(),
        matches: scan.matches,
        complete: candidates_complete,
    })
}

/// Compute exact NAR facts for one input path without reference matching.
pub fn input_facts(path: &Path, bounds: NarBounds) -> Result<BuildInputObservation, Diagnostic> {
    if !path.is_absolute() || bounds.max_nar_bytes == 0 {
        return Err(diagnostic(
            "nix_nar_input",
            "nix.nar.path",
            "input NAR observation needs an absolute path and a positive bound",
        ));
    }
    let hash = hash_path(path, CaseHack::native()).map_err(|_| {
        diagnostic(
            "nix_nar_hash",
            "nix.nar.path",
            "input NAR encoding or hashing failed",
        )
    })?;
    if hash.size > bounds.max_nar_bytes {
        return Err(diagnostic(
            "nix_nar_bound",
            "nix.nar.size",
            "input NAR exceeds the named byte bound",
        ));
    }
    let path = path
        .to_str()
        .ok_or_else(|| diagnostic("nix_nar_path", "nix.nar.path", "input path is not UTF-8"))?;
    Ok(BuildInputObservation {
        path: path.to_owned(),
        nar_sha256: hex(&hash.sha256),
        nar_size: hash.size,
    })
}

/// Canonical BLAKE3 identity over sorted input facts.
#[must_use]
pub fn input_closure_identity(inputs: &[BuildInputObservation]) -> Option<ContentId> {
    if inputs.is_empty() {
        return None;
    }
    let mut fields = Vec::with_capacity(inputs.len() * 3);
    for input in inputs {
        fields.push(input.path.as_bytes().to_vec());
        fields.push(input.nar_sha256.as_bytes().to_vec());
        fields.push(input.nar_size.to_le_bytes().to_vec());
    }
    let borrowed = fields.iter().map(Vec::as_slice).collect::<Vec<_>>();
    Some(identity("kiln.nix-input-closure.v1", &borrowed))
}

/// Stream a NAR for `path` into `writer`, enforcing the NAR byte bound.
pub fn write_nar(writer: impl Write, path: &Path, max_bytes: u64) -> Result<u64, Diagnostic> {
    if !path.is_absolute() || max_bytes == 0 {
        return Err(diagnostic(
            "nix_nar_write",
            "nix.nar.path",
            "NAR writing needs an absolute path and a positive bound",
        ));
    }
    let mut limited = LimitedWriter {
        inner: writer,
        remaining: max_bytes,
        written: 0,
    };
    encode_path(&mut limited, path, CaseHack::native()).map_err(|error| {
        if matches!(error, nix_archive::nar::Error::Io(_)) && limited.remaining == 0 {
            diagnostic(
                "nix_nar_bound",
                "nix.nar.size",
                "NAR exceeds the named byte bound",
            )
        } else {
            diagnostic("nix_nar_write", "nix.nar.path", "NAR encoding failed")
        }
    })?;
    Ok(limited.written)
}

struct LimitedWriter<W> {
    inner: W,
    remaining: u64,
    written: u64,
}

impl<W: Write> Write for LimitedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let length =
            u64::try_from(bytes.len()).map_err(|_| io::Error::other("NAR write length"))?;
        if length > self.remaining {
            return Err(io::Error::other("NAR byte bound exceeded"));
        }
        let written = self.inner.write(bytes)?;
        let written_u64 =
            u64::try_from(written).map_err(|_| io::Error::other("NAR write length"))?;
        self.remaining = self.remaining.saturating_sub(written_u64);
        self.written = self.written.saturating_add(written_u64);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn hex(bytes: &[u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        let high = DIGITS.get(usize::from(byte >> 4)).copied().unwrap_or(b'0');
        let low = DIGITS
            .get(usize::from(byte & 0x0f))
            .copied()
            .unwrap_or(b'0');
        output.push(high as char);
        output.push(low as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::derivation::needle;
    use std::fs;
    use std::path::PathBuf;

    fn store_path(name: &str) -> PathBuf {
        PathBuf::from(format!("/nix/store/{}-{name}", "0".repeat(32)))
    }

    // r[verify kiln.nix_store.nar.scenario.product]
    #[test]
    fn observes_nar_size_hash_and_matching_references() {
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let file = directory.path().join("content");
        let candidate = store_path("candidate");
        let needle = needle(candidate.to_str().unwrap_or_default())
            .unwrap_or_else(|| unreachable!("fixture needle"));
        fs::write(&file, needle.as_bytes()).unwrap_or_else(|_| unreachable!());
        let facts = observe_path(
            &file,
            &[ReferenceCandidate {
                path: candidate.to_string_lossy().into_owned(),
                needle,
            }],
            true,
            NarBounds::DEFAULT,
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(facts.nar_sha256.len(), 64);
        assert!(facts.nar_size > 0);
        assert_eq!(facts.references, [candidate.to_string_lossy().into_owned()]);
        assert!(facts.complete);
        assert!(input_facts(&file, NarBounds::DEFAULT).is_ok());
        assert!(
            input_closure_identity(&[
                input_facts(&file, NarBounds::DEFAULT).unwrap_or_else(|_| unreachable!())
            ])
            .is_some()
        );
    }

    // r[verify kiln.nix_store.refscan.scenario.incomplete]
    #[test]
    fn reports_incomplete_candidates_and_enforces_nar_bounds() {
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let file = directory.path().join("content");
        fs::write(&file, b"content").unwrap_or_else(|_| unreachable!());
        let facts = observe_path(&file, &[], false, NarBounds::DEFAULT)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(!facts.complete);
        let tight = NarBounds {
            max_nar_bytes: 8,
            ..NarBounds::DEFAULT
        };
        assert!(observe_path(&file, &[], true, tight).is_err());
        assert!(input_facts(&file, tight).is_err());
        assert!(write_nar(Vec::new(), &file, 8).is_err());
    }
}
