//! Pure, policy-bounded presentation of MCP receipts. No receipt fields or hashes are changed.
use serde::Deserialize;
use serde_json::Value;
use std::fmt::{Display, Write};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseProfile {
    Legacy,
    StructuredSummary,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponsePolicy {
    schema: String,
    default_profile: String,
    admitted_profiles: Vec<String>,
    max_summary_bytes: usize,
    max_issue_count: usize,
    fallback: String,
    readiness_states: Vec<String>,
    implementation_outcomes: Vec<String>,
    gate_verdicts: Vec<String>,
}

impl ResponsePolicy {
    pub fn from_json(value: &str) -> Result<Self, String> {
        let policy: Self = serde_json::from_str(value).map_err(|e| e.to_string())?;
        if policy.schema != "cairn.mcp-response-profile.v1"
            || policy.default_profile != "legacy"
            || !policy
                .admitted_profiles
                .iter()
                .map(String::as_str)
                .eq(["legacy", "structured-summary"])
            || !policy.readiness_states.iter().map(String::as_str).eq([
                "ready",
                "blocked",
                "unavailable",
                "not_applicable",
            ])
            || !policy
                .implementation_outcomes
                .iter()
                .map(String::as_str)
                .eq([
                    "ready",
                    "blocked",
                    "incomplete_limit",
                    "completion_only",
                    "empty",
                ])
            || !policy
                .gate_verdicts
                .iter()
                .map(String::as_str)
                .eq(["PASS", "FAIL"])
            || policy.max_summary_bytes < 128
            || policy.max_summary_bytes > 4096
            || policy.max_issue_count == 0
            || policy.max_issue_count > 999_999
            || !policy.fallback.is_ascii()
            || policy.fallback.len() > policy.max_summary_bytes
            || !policy.fallback.contains("structuredContent")
        {
            return Err("invalid MCP response projection policy".into());
        }
        Ok(policy)
    }

    pub fn admit(&self, id: &str) -> Result<ResponseProfile, String> {
        if !self.admitted_profiles.iter().any(|admitted| admitted == id) {
            return Err(format!("unknown MCP response profile: {id}"));
        }
        match id {
            "legacy" => Ok(ResponseProfile::Legacy),
            "structured-summary" => Ok(ResponseProfile::StructuredSummary),
            _ => Err("unsupported MCP response profile policy".into()),
        }
    }

    pub fn default_profile(&self) -> Result<ResponseProfile, String> {
        self.admit(&self.default_profile)
    }

    /// Only fields with closed, tool-specific meanings are projected. Transport success
    /// cannot establish a lifecycle decision; absent/unknown fields stay unclassified.
    pub fn summarize(&self, tool: &str, receipt: &Value) -> String {
        if tool.len() > self.max_summary_bytes.saturating_sub(48) {
            return self.fallback.clone();
        }
        let mut fields = String::with_capacity(self.max_summary_bytes);
        fields.push_str(tool);
        fields.push_str(": ");
        let prefix_len = fields.len();
        let classified = if receipt.get("allowed") == Some(&Value::Bool(false))
            && receipt.get("mutated") == Some(&Value::Bool(false))
            && matches!(
                tool,
                "cairn_init"
                    | "cairn_change_create"
                    | "cairn_sync_execute"
                    | "cairn_archive_execute"
                    | "cairn_migrate_execute"
            ) {
            Self::field(&mut fields, "mutation", "denied");
            true
        } else {
            match tool {
                "cairn_status" => {
                    let known = receipt["changes"].is_array() && receipt["specs"].is_array();
                    self.count(&mut fields, "changes", &receipt["changes"]);
                    self.count(&mut fields, "specs", &receipt["specs"]);
                    known
                }
                "cairn_list_content" => {
                    let known = receipt["active_changes"].is_array();
                    self.count(&mut fields, "active_changes", &receipt["active_changes"]);
                    self.count(&mut fields, "accepted_specs", &receipt["accepted_specs"]);
                    known
                }
                "cairn_drain_plan" => {
                    let known =
                        self.boolean(&mut fields, "drain_complete", &receipt["drain_complete"]);
                    self.number(
                        &mut fields,
                        "active_changes",
                        &receipt["active_change_count"],
                    );
                    self.number(&mut fields, "ready_changes", &receipt["ready_change_count"]);
                    known
                }
                "cairn_doctor" => {
                    let known = self.boolean(&mut fields, "valid", &receipt["valid"]);
                    self.number(&mut fields, "issues", &receipt["validation_issue_count"]);
                    known
                }
                "cairn_readiness" if receipt["schema"] == "cairn.readiness.v1" => {
                    let known = self.closed(
                        &mut fields,
                        "lifecycle",
                        &receipt["classification"],
                        &self.readiness_states,
                    );
                    self.boolean(&mut fields, "valid", &receipt["valid"]);
                    self.count(&mut fields, "blockers", &receipt["blockers"]);
                    known
                }
                "cairn_implementation_ready"
                    if receipt["schema"] == "cairn.implementation-ready.v1" =>
                {
                    let known = self.closed(
                        &mut fields,
                        "lifecycle",
                        &receipt["outcome"],
                        &self.implementation_outcomes,
                    );
                    self.boolean(&mut fields, "complete", &receipt["complete"]);
                    self.number(
                        &mut fields,
                        "active_changes",
                        &receipt["active_change_count"],
                    );
                    known
                }
                "cairn_gate" => {
                    let known = self.closed(
                        &mut fields,
                        "verdict",
                        &receipt["verdict"],
                        &self.gate_verdicts,
                    );
                    self.boolean(&mut fields, "valid", &receipt["valid"]);
                    self.count(&mut fields, "issues", &receipt["issues"]);
                    known
                }
                "cairn_validate"
                | "cairn_agent_receipt_validate"
                | "cairn_agent_completion_validate" => {
                    let known = self.boolean(&mut fields, "valid", &receipt["valid"]);
                    self.count(&mut fields, "issues", &receipt["issues"]);
                    known
                }
                "cairn_sync_plan"
                | "cairn_archive_plan"
                | "cairn_migrate_plan"
                | "cairn_sync_execute"
                | "cairn_archive_execute"
                | "cairn_migrate_execute" => {
                    let known = self.boolean(&mut fields, "blocked", &receipt["blocked"]);
                    self.boolean(&mut fields, "mutated", &receipt["mutated"]);
                    self.count(&mut fields, "conflicts", &receipt["conflicts"]);
                    known
                }
                _ => false,
            }
        };
        if !classified {
            fields.truncate(prefix_len);
            fields.push_str("unclassified");
        }
        fields.push_str("; complete detail in structuredContent");
        if fields.len() <= self.max_summary_bytes {
            fields
        } else {
            self.fallback.clone()
        }
    }

    fn field(out: &mut String, label: &str, value: impl Display) {
        if !out.ends_with(": ") {
            out.push_str("; ");
        }
        write!(out, "{label}={value}").expect("writing to String cannot fail");
    }

    fn closed(&self, out: &mut String, label: &str, value: &Value, admitted: &[String]) -> bool {
        if let Some(state) = value
            .as_str()
            .filter(|state| admitted.iter().any(|known| known == state))
        {
            Self::field(out, label, state);
            true
        } else {
            false
        }
    }

    fn boolean(&self, out: &mut String, label: &str, value: &Value) -> bool {
        if let Some(value) = value.as_bool() {
            Self::field(out, label, value);
            true
        } else {
            false
        }
    }

    fn count(&self, out: &mut String, label: &str, value: &Value) {
        if let Some(rows) = value.as_array() {
            self.number_value(out, label, u64::try_from(rows.len()).unwrap_or(u64::MAX));
        }
    }

    fn number(&self, out: &mut String, label: &str, value: &Value) {
        if let Some(number) = value.as_u64() {
            self.number_value(out, label, number);
        }
    }

    fn number_value(&self, out: &mut String, label: &str, number: u64) {
        let max = self.max_issue_count as u64;
        if number > max {
            if !out.ends_with(": ") {
                out.push_str("; ");
            }
            write!(out, "{label}={max}+ (omitted)").expect("writing to String cannot fail");
        } else {
            Self::field(out, label, number);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn policy() -> ResponsePolicy {
        ResponsePolicy::from_json(include_str!(
            "../../../../cairn-policy/generated/mcp-response-profile.json"
        ))
        .unwrap()
    }

    #[test]
    fn closed_states_never_copy_diagnostics_or_infer_success() {
        let p = policy();
        let blocked = json!({"schema":"cairn.readiness.v1", "classification":"blocked", "valid":true, "blockers":[{"message":"secret diagnostic"}]});
        let text = p.summarize("cairn_readiness", &blocked);
        assert!(!text.contains("secret diagnostic"));
        let unknown = json!({"isError":false, "diagnostic":"x".repeat(100_000)});
        let unknown_with_flags = json!({"isError":true,"valid":true,"verdict":"PASS","diagnostic":"other private source"});
        assert_eq!(
            p.summarize("unknown_family", &unknown),
            p.summarize("unknown_family", &unknown_with_flags)
        );
        assert!(!p.summarize("unknown_family", &unknown).contains('x'));
    }

    #[test]
    fn bounds_and_denial_are_explicit() {
        let p = policy();
        let denied = json!({"allowed":false,"mutated":false,"error":"PRIVATE_DIAGNOSTIC_CANARY".repeat(10_000)});
        let denied_text = p.summarize("cairn_sync_execute", &denied);
        assert!(!denied_text.contains("PRIVATE_DIAGNOSTIC_CANARY"));
        assert_eq!(
            denied_text,
            p.summarize(
                "cairn_sync_execute",
                &json!({"allowed":false,"mutated":false,"error":"different private source"})
            )
        );
        let many = json!({"valid":false,"issues":vec![0;10_001]});
        let text = p.summarize("cairn_validate", &many);
        let more = json!({"valid":false,"issues":vec![0;10_002]});
        assert_eq!(text, p.summarize("cairn_validate", &more));
        assert!(text.len() <= p.max_summary_bytes);
        let long_tool = "x".repeat(10_000);
        assert_eq!(p.summarize(&long_tool, &Value::Null), p.fallback);
    }

    #[test]
    fn policy_rejects_missing_repeated_and_impossible_profiles() {
        let source: Value = serde_json::from_str(include_str!(
            "../../../../cairn-policy/generated/mcp-response-profile.json"
        ))
        .unwrap();
        assert!(policy().admit("unknown").is_err());
        for value in [
            json!(["legacy", "legacy"]),
            json!(["structured-summary"]),
            json!([]),
        ] {
            let mut invalid = source.clone();
            invalid["admitted_profiles"] = value;
            assert!(ResponsePolicy::from_json(&invalid.to_string()).is_err());
        }
        let mut invalid = source;
        invalid["max_summary_bytes"] = json!(8);
        assert!(ResponsePolicy::from_json(&invalid.to_string()).is_err());
    }
}
