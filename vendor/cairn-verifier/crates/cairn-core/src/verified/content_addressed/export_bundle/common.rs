pub(super) fn bundle_issue(
    code: impl Into<String>,
    message: impl Into<String>,
) -> super::types::ExportBundleIssue {
    super::types::ExportBundleIssue {
        code: code.into(),
        message: message.into(),
    }
}
