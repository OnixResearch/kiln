#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRunGateIssue {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRunGateReport {
    pub classification: String,
    pub verdict: String,
    pub valid: bool,
    pub issues: Vec<AgentRunGateIssue>,
    pub receipt_hash: String,
}
