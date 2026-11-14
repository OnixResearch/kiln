use super::types::ExportBundleIssue;

pub(super) fn bundle_issue(
    code: impl Into<String>,
    message: impl Into<String>,
) -> ExportBundleIssue {
    ExportBundleIssue {
        code: code.into(),
        message: message.into(),
    }
}
