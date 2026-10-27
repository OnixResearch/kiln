//! Filesystem/network shell for declared report ingress; no report outcome affects child exit truth.
use std::path::Path;

use base64::Engine as _;
use kiln_core::source::SourceResolution;

use super::client::{RunBinding, ServiceClient, exact_binding};
use super::files;
use super::profile::{Step, WorkerProfile};
use crate::reports::{MAX_REPORT_BYTES, MAX_TOTAL_REPORT_BYTES, ReportBinding, ReportUpload};

impl ServiceClient {
    /// Called after every completed command, including failures and cancellation.
    /// Attempt all declarations even if one is missing or rejected. Unavailability
    /// is recorded explicitly and returns ingress failure, never a passing suite.
    pub(super) fn upload_reports(
        &self,
        profile: &WorkerProfile,
        source: &SourceResolution,
        binding: &RunBinding,
        step: &Step,
        checkout: &Path,
        total_report_bytes: &mut usize,
    ) -> Result<(), String> {
        let mut first_error = None;
        for declaration in &step.reports {
            let result = (|| {
                let current = self
                    .lookup(profile, source)?
                    .ok_or("worker active report run missing")?;
                exact_binding(binding, &current)?;
                if !current.nodes.iter().any(|node| node.id == step.node_id) {
                    return Err("worker report node binding invalid".into());
                }
                let bytes = read_declared(checkout, &declaration.path);
                let (body_base64, blake3, unavailable) = match &bytes {
                    Ok(bytes) => {
                        *total_report_bytes = total_report_bytes
                            .checked_add(bytes.len())
                            .ok_or("worker report total overflow")?;
                        if *total_report_bytes > MAX_TOTAL_REPORT_BYTES {
                            return Err("worker report total exceeds bound".into());
                        }
                        (
                            Some(base64::engine::general_purpose::STANDARD.encode(bytes)),
                            Some(blake3::hash(bytes).to_hex().to_string()),
                            None,
                        )
                    }
                    // This also covers absent files. The safe descriptor walker deliberately
                    // does not reveal filesystem details or follow links to classify failures.
                    Err(_) => (None, None, Some("unreadable".into())),
                };
                let request = ReportUpload {
                    binding: ReportBinding {
                        run_id: binding.id.clone(),
                        source_revision: binding.source_revision.clone(),
                        attempt_id: binding.attempt_id.clone(),
                        execution: binding.execution.clone(),
                        node_id: step.node_id.clone(),
                    },
                    kind: declaration.kind,
                    name: declaration
                        .path
                        .to_str()
                        .ok_or("worker report name invalid")?
                        .to_owned(),
                    body_base64,
                    blake3,
                    unavailable,
                };
                let response = self
                    .client
                    .post(format!("{}/api/runs/{}/reports", self.origin, binding.id))
                    .header(reqwest::header::AUTHORIZATION, self.authorization.clone())
                    .header(reqwest::header::ORIGIN, &self.origin)
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .body(
                        serde_json::to_vec(&request)
                            .map_err(|_| "worker report encoding failed")?,
                    )
                    .send()
                    .map_err(|_| "worker report upload failed")?;
                self.response(response)?;
                bytes
                    .map(|_| ())
                    .map_err(|_| "declared worker report missing or unreadable".into())
            })();
            if let Err(error) = result {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

fn read_declared(checkout: &Path, relative: &Path) -> Result<Vec<u8>, String> {
    if !crate::reports::safe_relative_path(relative) {
        return Err("worker report path invalid".into());
    }
    files::artifact(checkout, relative, MAX_REPORT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_reports_read_exact_regular_bytes_and_deny_missing_links_escapes_and_oversize() {
        let root =
            tempfile::tempdir().unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        let source = b"<testsuite tests='1'><testcase name='real'/></testsuite>";
        std::fs::write(root.path().join("real.xml"), source)
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        assert_eq!(
            read_declared(root.path(), Path::new("real.xml"))
                .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}")),
            source
        );
        assert!(read_declared(root.path(), Path::new("missing.xml")).is_err());
        assert!(read_declared(root.path(), Path::new("../real.xml")).is_err());
        std::os::unix::fs::symlink("real.xml", root.path().join("link.xml"))
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        assert!(read_declared(root.path(), Path::new("link.xml")).is_err());
        std::os::unix::fs::symlink(root.path(), root.path().join("directory-link"))
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        assert!(read_declared(root.path(), Path::new("directory-link/real.xml")).is_err());
        std::fs::write(
            root.path().join("large.xml"),
            vec![b'x'; MAX_REPORT_BYTES + 1],
        )
        .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        assert!(read_declared(root.path(), Path::new("large.xml")).is_err());
        std::fs::hard_link(root.path().join("real.xml"), root.path().join("hard.xml"))
            .unwrap_or_else(|error| unreachable!("fixture failed: {error:?}"));
        assert!(read_declared(root.path(), Path::new("hard.xml")).is_err());
    }
}
