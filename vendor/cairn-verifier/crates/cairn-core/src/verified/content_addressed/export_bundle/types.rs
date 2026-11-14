#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportBundleIssue {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportBundleReport {
    pub valid: bool,
    pub verdict: String,
    pub issues: Vec<ExportBundleIssue>,
    pub computed_bundle_hash: String,
}
