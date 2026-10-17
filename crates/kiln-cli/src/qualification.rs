use std::fs::File;
use std::io::Read;
use std::path::Path;

use kiln_codec::{DEFAULT_MAX_CANONICAL_BYTES, identity_from_bytes};
use kiln_core::hosting::qualification::{
    HostQualificationCohort, HostQualificationReceipt, validate_host_qualification,
};

const MAX_QUALIFICATION_INPUT_BYTES: usize = DEFAULT_MAX_CANONICAL_BYTES;

fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let file =
        File::open(path).map_err(|error| format!("qualification input open failed: {error}"))?;
    let limit = u64::try_from(MAX_QUALIFICATION_INPUT_BYTES)
        .map_err(|_| "qualification bound does not fit the host")?
        + 1;
    let mut bytes = Vec::new();
    file.take(limit)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("qualification input read failed: {error}"))?;
    if bytes.len() > MAX_QUALIFICATION_INPUT_BYTES {
        return Err("qualification input exceeds the canonical byte bound".to_string());
    }
    Ok(bytes)
}

// r[impl kiln.aspen_host.production_evidence]
// r[impl kiln.aspen_host.production_evidence.accepted]
/// Verifies the operator's expected cohort, receipt consistency and retained artifact bytes.
/// Artifact filenames are their validated BLAKE3 digests; no receipt-selected path is opened.
/// Byte identity and receipt admission do not establish observation truth or deployment authority.
pub(super) fn qualify(
    cohort_path: &Path,
    receipt_path: &Path,
    artifact_root: &Path,
) -> Result<String, String> {
    let cohort_bytes = read_bounded(cohort_path)?;
    let cohort: HostQualificationCohort = serde_json::from_slice(&cohort_bytes)
        .map_err(|error| format!("qualification cohort JSON failed: {error}"))?;
    let cohort_identity = identity_from_bytes(&cohort_bytes)
        .map_err(|error| format!("qualification cohort identity failed: {error}"))?;
    let receipt_bytes = read_bounded(receipt_path)?;
    let receipt: HostQualificationReceipt = serde_json::from_slice(&receipt_bytes)
        .map_err(|error| format!("qualification receipt JSON failed: {error}"))?;
    validate_host_qualification(&cohort, &cohort_identity, &receipt)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let mut verified_artifacts = 0;
    for (index, observation) in receipt.observations.iter().enumerate() {
        if receipt
            .observations
            .iter()
            .take(index)
            .any(|prior| prior.artifact == observation.artifact)
        {
            continue;
        }
        let path = artifact_root.join(observation.artifact.as_str());
        let bytes = read_bounded(&path)?;
        let observed = identity_from_bytes(&bytes)
            .map_err(|error| format!("qualification artifact identity failed: {error}"))?;
        if observed != observation.artifact {
            return Err(
                "qualification artifact bytes differ from their exact identity".to_string(),
            );
        }
        verified_artifacts += 1;
    }
    let receipt_identity = identity_from_bytes(&receipt_bytes)
        .map_err(|error| format!("qualification receipt identity failed: {error}"))?;
    serde_json::to_string(&serde_json::json!({
        "schema": "kiln.host-qualification-admission.v1",
        "result": "admitted",
        "cohort": cohort_identity.as_str(),
        "receipt": receipt_identity.as_str(),
        "verified_artifacts": verified_artifacts,
        "claim": "bounded_local_host",
        "non_claims": [
            "admission checks supplied observation consistency and retained artifact identity, not observation truth",
            "machine-local evidence does not prove global durability or exactly-once delivery",
            "admission does not prove CI correctness, provider truth, host sandboxing or availability",
            "admission supplies no publication, release, deployment or credential authority"
        ]
    }))
    .map_err(|error| format!("qualification result encoding failed: {error}"))
}
