use crate::engine::{ApiResponse, Role};
use bounded_exec::{
    CommandSpec, Completion, EnvironmentMode, ExecutionLimits, Input, OutcomePolicy, RunRequest,
    TerminationScope,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

pub const ROUTE: &str = "/api/lattice/drafts";
const SCHEMA: &str = "lattice.source-edit.v1";
const PROFILE_SCHEMA: &str = "kiln.lattice-draft-owner.v1";
const MAX_PROFILE: usize = 16 * 1024;
const MAX_MEMBER_BYTES: usize = 48 * 1024;
const MAX_NATIVE_IO: usize = 1024 * 1024;
const MAX_BROWSER_REQUEST: usize = 6 * MAX_MEMBER_BYTES + 4096;
const NATIVE_TIMEOUT_MS: u64 = 10_000;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftOwner {
    schema: String,
    executable: PathBuf,
    config: PathBuf,
    workflows: BTreeSet<String>,
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Inspect {
        workflow: String,
    },
    View {
        workflow: String,
        expected_head: String,
        draft_id: String,
        member: String,
    },
    Save {
        workflow: String,
        expected_head: String,
        draft_id: String,
        member: String,
        #[serde(deserialize_with = "required_draft")]
        expected_draft: ExpectedDraft,
        content: String,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(transparent)]
struct ExpectedDraft(Option<String>);

fn required_draft<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<ExpectedDraft, D::Error> {
    ExpectedDraft::deserialize(decoder)
}

impl Request {
    fn workflow(&self) -> &str {
        match self {
            Self::Inspect { workflow }
            | Self::View { workflow, .. }
            | Self::Save { workflow, .. } => workflow,
        }
    }

    fn encode(&self) -> Option<Vec<u8>> {
        let mut value = json!({"schema": SCHEMA, "workflow": self.workflow()});
        let (action, common) = match self {
            Self::Inspect { .. } => ("inspect", None),
            Self::View {
                expected_head,
                draft_id,
                member,
                ..
            } => ("view", Some((expected_head, draft_id, member))),
            Self::Save {
                expected_head,
                draft_id,
                member,
                expected_draft,
                content,
                ..
            } => {
                if content.len() > MAX_MEMBER_BYTES
                    || expected_draft.0.as_ref().is_some_and(|s| !digest(s))
                {
                    return None;
                }
                value["expected_draft"] = json!(expected_draft);
                value["bytes"] = json!(content.as_bytes());
                ("save", Some((expected_head, draft_id, member)))
            }
        };
        value["action"] = json!(action);
        if let Some((head, draft, member)) = common {
            if !digest(head) || !identifier(draft) || !logical_member(member) {
                return None;
            }
            value["expected_head"] = json!(head);
            value["draft_id"] = json!(draft);
            value["member"] = json!(member);
        }
        serde_json::to_vec(&value).ok()
    }

    fn admit_response(&self, mut value: Value) -> Option<Value> {
        if value["schema"] != SCHEMA
            || value["workflow"] != self.workflow()
            || !valid_digest(&value["head"])
        {
            return None;
        }
        match self {
            Self::Inspect { .. } => {
                let source = &value["source"];
                if value["status"] != "inspected" || !value.as_object()?.contains_key("source") {
                    return None;
                }
                if !source.is_null() {
                    let entry = source["entry"].as_str()?;
                    let members = source["members"].as_array()?;
                    if !valid_digest(&source["source_revision"])
                        || members.len() > 1024
                        || !logical_member(entry)
                        || !members.iter().any(|m| m == entry)
                        || !members
                            .iter()
                            .all(|m| m.as_str().is_some_and(logical_member))
                    {
                        return None;
                    }
                }
            }
            Self::View {
                expected_head,
                draft_id,
                member,
                ..
            } => {
                if value["status"] != "viewed"
                    || !matches_member(&value, expected_head, draft_id, member)
                    || !value.as_object()?.contains_key("draft_revision")
                    || !(value["draft_revision"].is_null()
                        || valid_digest(&value["draft_revision"]))
                {
                    return None;
                }
                let bytes: Vec<u8> =
                    serde_json::from_value(value.as_object_mut()?.remove("bytes")?).ok()?;
                if bytes.len() > MAX_MEMBER_BYTES {
                    return None;
                }
                value["content"] = json!(String::from_utf8(bytes).ok()?);
            }
            Self::Save {
                expected_head,
                draft_id,
                member,
                ..
            } => {
                if value["status"] != "saved"
                    || !matches_member(&value, expected_head, draft_id, member)
                    || !valid_digest(&value["draft_revision"])
                    || !value["receipt"].is_object()
                {
                    return None;
                }
            }
        }
        Some(value)
    }
}

fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}
fn logical_member(s: &str) -> bool {
    s.len() <= 512
        && !s.contains('\\')
        && !s.chars().any(char::is_control)
        && s.split('/').all(|part| !matches!(part, "" | "." | ".."))
}
fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_digest(v: &Value) -> bool {
    v.as_str().is_some_and(digest)
}
fn matches_member(v: &Value, head: &str, draft: &str, member: &str) -> bool {
    v["head"] == head
        && v["draft_id"] == draft
        && v["member"] == member
        && valid_digest(&v["source_revision"])
}

fn json_response(status: u16, value: Value) -> ApiResponse {
    ApiResponse {
        status,
        content_type: "application/json",
        body: value.to_string().into_bytes(),
        download_name: None,
        headers: vec![("Cache-Control", "no-store".into())],
    }
}
fn error(status: u16, code: &str) -> ApiResponse {
    json_response(status, json!({"code": code, "message": code}))
}

impl DraftOwner {
    pub fn load(path: &Path) -> Result<Self, String> {
        if !crate::profile::absolute(path) {
            return Err("Lattice draft profile must be absolute".into());
        }
        let mut bytes = Vec::new();
        File::open(path)
            .and_then(|f| f.take(MAX_PROFILE as u64 + 1).read_to_end(&mut bytes))
            .map_err(|e| e.to_string())?;
        if bytes.len() > MAX_PROFILE {
            return Err("Lattice draft profile exceeds its bound".into());
        }
        let owner: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if owner.schema != PROFILE_SCHEMA
            || !crate::profile::absolute(&owner.executable)
            || !owner.executable.is_file()
            || !crate::profile::absolute(&owner.config)
            || !owner.config.is_file()
            || owner.workflows.is_empty()
            || owner.workflows.len() > 64
            || !owner.workflows.iter().all(|id| identifier(id))
        {
            return Err("Invalid explicit Lattice draft owner enrollment".into());
        }
        Ok(owner)
    }

    fn call(&self, request: &Request, input: Vec<u8>) -> ApiResponse {
        let output = bounded_exec::run(RunRequest {
            command: CommandSpec {
                program: self.executable.clone(),
                args: vec![
                    "--config".into(),
                    self.config.as_os_str().into(),
                    "authoring".into(),
                    "source-edit".into(),
                ],
                current_dir: PathBuf::from("/"),
                environment_mode: EnvironmentMode::Clear,
                environment: Vec::new(),
                input: Input::Bytes(input),
            },
            limits: ExecutionLimits {
                timeout_ms: NATIVE_TIMEOUT_MS,
                stdin_max_bytes: MAX_NATIVE_IO,
                stdout_max_bytes: MAX_NATIVE_IO,
                stderr_max_bytes: 16 * 1024,
                poll_interval_ms: 10,
                teardown_timeout_ms: 1000,
            },
            termination_scope: TerminationScope::ProcessGroup,
            outcome_policy: match OutcomePolicy::new(vec![0, 1], true, true) {
                Ok(policy) => policy,
                Err(_) => return error(503, "lattice_draft_unavailable"),
            },
        });
        let unknown = || error(502, "lattice_draft_outcome_unknown");
        let Ok(output) = output else {
            return unknown();
        };
        if output.completion != Completion::Exited
            || output.stdout.truncated
            || output.stderr.truncated
        {
            return unknown();
        }
        let Ok(value) = serde_json::from_slice::<Value>(&output.stdout.bytes) else {
            return unknown();
        };
        match output.exit_code {
            Some(0) => request
                .admit_response(value)
                .map_or_else(unknown, |reply| json_response(200, reply)),
            Some(1) if value["schema"] == SCHEMA && value["status"] == "rejected" => {
                match value["code"].as_str().filter(|code| identifier(code)) {
                    Some(code) => error(409, code),
                    None => unknown(),
                }
            }
            _ => unknown(),
        }
    }

    pub fn response(&self, method: &str, body: &[u8], role: Role) -> ApiResponse {
        if role != Role::Operator {
            return error(403, "lattice_draft_forbidden");
        }
        match method {
            "GET" if body.is_empty() => json_response(200, json!({"workflows": self.workflows})),
            "POST" => {
                if body.len() > MAX_BROWSER_REQUEST {
                    return error(413, "lattice_draft_too_large");
                }
                let Ok(request) = serde_json::from_slice::<Request>(body) else {
                    return error(400, "lattice_draft_invalid");
                };
                if !self.workflows.contains(request.workflow()) {
                    return error(404, "lattice_draft_workflow_not_enrolled");
                }
                let Some(input) = request.encode() else {
                    return error(400, "lattice_draft_invalid");
                };
                self.call(&request, input)
            }
            _ => error(405, "lattice_draft_method_denied"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edit_requires_an_explicit_draft_precondition_and_confined_member() {
        let mut input = json!({"action":"save","workflow":"fixture","expected_head":"a".repeat(64),
            "draft_id":"new-draft","member":"workflow.ncl","content":"{ enabled = true }"});
        assert!(serde_json::from_value::<Request>(input.clone()).is_err());
        input["expected_draft"] = Value::Null;
        let request =
            serde_json::from_value::<Request>(input.clone()).expect("explicit create precondition");
        let bytes = request.encode().expect("native edit request");
        let wire: Value = serde_json::from_slice(&bytes).expect("native JSON");
        assert_eq!(wire["bytes"], json!(b"{ enabled = true }"));
        assert!(wire.get("expected_draft").is_some_and(Value::is_null));
        input["member"] = json!("modules/../workflow.ncl");
        assert!(
            serde_json::from_value::<Request>(input)
                .expect("request shape")
                .encode()
                .is_none()
        );
    }

    #[test]
    fn a_draft_receipt_for_another_catalog_head_is_not_a_commit() {
        let request = Request::Save {
            workflow: "fixture".into(),
            expected_head: "a".repeat(64),
            draft_id: "new-draft".into(),
            member: "workflow.ncl".into(),
            expected_draft: ExpectedDraft(None),
            content: "replacement".into(),
        };
        let mut receipt = json!({"schema":SCHEMA,"status":"saved","workflow":"fixture","head":"b".repeat(64),
            "draft_id":"new-draft","member":"workflow.ncl","source_revision":"c".repeat(64),"draft_revision":"d".repeat(64),"receipt":{"compiler":"native"}});
        assert!(request.admit_response(receipt.clone()).is_none());
        receipt["head"] = json!("a".repeat(64));
        assert!(request.admit_response(receipt).is_some());
    }
}
