use thiserror::Error;

// r[impl cairn.cli_machine_contract.envelope]
// r[impl cairn.cli_machine_contract.functional_boundary]
pub const CLI_MACHINE_SCHEMA_V1: &str = "cairn.cli-machine.v1";
pub const CLI_MACHINE_UNKNOWN_COMMAND: &str = "unknown";
pub const CLI_MACHINE_SEVERITY_ERROR: &str = "error";
pub const CLI_MACHINE_SEVERITY_WARNING: &str = "warning";
pub const CLI_MACHINE_SEVERITY_INFO: &str = "info";
pub const CLI_MACHINE_MAX_COMMAND_BYTES_LIMIT: usize = 256;
pub const CLI_MACHINE_MAX_DIAGNOSTICS_LIMIT: usize = 256;
pub const CLI_MACHINE_MAX_FIELD_BYTES_LIMIT: usize = 16 * 1_024;
pub const CLI_MACHINE_MAX_CAPTURE_BYTES_LIMIT: usize = 16 * 1_024 * 1_024;
pub const CLI_MACHINE_MAX_PROCESS_STATUS: u8 = 125;

pub const CLI_MACHINE_DIAGNOSTIC_MISSING_COMMAND: &str = "cairn.cli.missing_command";
pub const CLI_MACHINE_DIAGNOSTIC_UNKNOWN_COMMAND: &str = "cairn.cli.unknown_command";
pub const CLI_MACHINE_DIAGNOSTIC_INCOMPATIBLE_OPTION: &str = "cairn.cli.incompatible_option";
pub const CLI_MACHINE_DIAGNOSTIC_INVOCATION_FAILED: &str =
    "cairn.cli.invocation_preparation_failed";
pub const CLI_MACHINE_DIAGNOSTIC_NOT_FOUND: &str = "cairn.cli.not_found";
pub const CLI_MACHINE_DIAGNOSTIC_POLICY_ERROR: &str = "cairn.cli.policy_error";
pub const CLI_MACHINE_DIAGNOSTIC_BLOCKED: &str = "cairn.cli.blocked";
pub const CLI_MACHINE_DIAGNOSTIC_OUTPUT_TOO_LARGE: &str = "cairn.cli.output_too_large";
pub const CLI_MACHINE_DIAGNOSTIC_OUTPUT_INVALID: &str = "cairn.cli.output_invalid";
pub const CLI_MACHINE_DIAGNOSTIC_INTERNAL_ERROR: &str = "cairn.cli.internal_error";

pub const CLI_MACHINE_REQUIRED_DIAGNOSTIC_CODES: &[&str] = &[
    CLI_MACHINE_DIAGNOSTIC_MISSING_COMMAND,
    CLI_MACHINE_DIAGNOSTIC_UNKNOWN_COMMAND,
    CLI_MACHINE_DIAGNOSTIC_INCOMPATIBLE_OPTION,
    CLI_MACHINE_DIAGNOSTIC_INVOCATION_FAILED,
    CLI_MACHINE_DIAGNOSTIC_NOT_FOUND,
    CLI_MACHINE_DIAGNOSTIC_POLICY_ERROR,
    CLI_MACHINE_DIAGNOSTIC_BLOCKED,
    CLI_MACHINE_DIAGNOSTIC_OUTPUT_TOO_LARGE,
    CLI_MACHINE_DIAGNOSTIC_OUTPUT_INVALID,
    CLI_MACHINE_DIAGNOSTIC_INTERNAL_ERROR,
];

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum MachineExitClass {
    Success,
    InvalidArguments,
    NotFound,
    PolicyError,
    Blocked,
    InternalError,
}

impl MachineExitClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::InvalidArguments => "invalid_arguments",
            Self::NotFound => "not_found",
            Self::PolicyError => "policy_error",
            Self::Blocked => "blocked",
            Self::InternalError => "internal_error",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "success" => Some(Self::Success),
            "invalid_arguments" => Some(Self::InvalidArguments),
            "not_found" => Some(Self::NotFound),
            "policy_error" => Some(Self::PolicyError),
            "blocked" => Some(Self::Blocked),
            "internal_error" => Some(Self::InternalError),
            _ => None,
        }
    }
}

pub const CLI_MACHINE_EXIT_CLASSES: &[MachineExitClass] = &[
    MachineExitClass::Success,
    MachineExitClass::InvalidArguments,
    MachineExitClass::NotFound,
    MachineExitClass::PolicyError,
    MachineExitClass::Blocked,
    MachineExitClass::InternalError,
];

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum MachineDiagnosticSeverity {
    Error,
    Warning,
    Info,
}

impl MachineDiagnosticSeverity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => CLI_MACHINE_SEVERITY_ERROR,
            Self::Warning => CLI_MACHINE_SEVERITY_WARNING,
            Self::Info => CLI_MACHINE_SEVERITY_INFO,
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            CLI_MACHINE_SEVERITY_ERROR => Some(Self::Error),
            CLI_MACHINE_SEVERITY_WARNING => Some(Self::Warning),
            CLI_MACHINE_SEVERITY_INFO => Some(Self::Info),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineExitClassPolicy {
    pub id: MachineExitClass,
    pub process_status: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineDiagnosticDefinition {
    pub code: String,
    pub exit_class: MachineExitClass,
    pub severity: MachineDiagnosticSeverity,
    pub message: String,
    pub target: String,
    pub fix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineContractBounds {
    pub max_command_bytes: usize,
    pub max_diagnostics: usize,
    pub max_code_bytes: usize,
    pub max_message_bytes: usize,
    pub max_target_bytes: usize,
    pub max_fix_bytes: usize,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineContractPolicy {
    pub schema_version: String,
    pub exit_classes: Vec<MachineExitClassPolicy>,
    pub diagnostic_fields: Vec<String>,
    pub diagnostic_order: Vec<String>,
    pub diagnostics: Vec<MachineDiagnosticDefinition>,
    pub bounds: MachineContractBounds,
    pub required_non_claims: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineFailureKind {
    MissingCommand,
    UnknownCommand,
    IncompatibleOption,
    InvocationPreparation,
    NotFound,
    Policy,
    Blocked,
    OutputTooLarge,
    OutputInvalid,
    Internal,
}

impl MachineFailureKind {
    #[must_use]
    pub const fn diagnostic_code(self) -> &'static str {
        match self {
            Self::MissingCommand => CLI_MACHINE_DIAGNOSTIC_MISSING_COMMAND,
            Self::UnknownCommand => CLI_MACHINE_DIAGNOSTIC_UNKNOWN_COMMAND,
            Self::IncompatibleOption => CLI_MACHINE_DIAGNOSTIC_INCOMPATIBLE_OPTION,
            Self::InvocationPreparation => CLI_MACHINE_DIAGNOSTIC_INVOCATION_FAILED,
            Self::NotFound => CLI_MACHINE_DIAGNOSTIC_NOT_FOUND,
            Self::Policy => CLI_MACHINE_DIAGNOSTIC_POLICY_ERROR,
            Self::Blocked => CLI_MACHINE_DIAGNOSTIC_BLOCKED,
            Self::OutputTooLarge => CLI_MACHINE_DIAGNOSTIC_OUTPUT_TOO_LARGE,
            Self::OutputInvalid => CLI_MACHINE_DIAGNOSTIC_OUTPUT_INVALID,
            Self::Internal => CLI_MACHINE_DIAGNOSTIC_INTERNAL_ERROR,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineOutcome {
    CommandResult {
        result: serde_json::Value,
        process_status: u8,
    },
    ControlledFailure {
        failures: Vec<MachineFailureKind>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MachineDiagnostic {
    pub code: String,
    pub severity: MachineDiagnosticSeverity,
    pub message: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MachineEnvelope {
    pub schema_version: String,
    pub ok: bool,
    pub command: String,
    pub exit_class: MachineExitClass,
    pub result: Option<serde_json::Value>,
    pub diagnostics: Vec<MachineDiagnostic>,
    pub non_claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineProcessOutcome {
    pub envelope: MachineEnvelope,
    pub process_status: u8,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MachineContractError {
    #[error("machine policy has no definition for diagnostic code {0}")]
    MissingDiagnosticDefinition(String),
    #[error("machine policy has no process status for exit class {0}")]
    MissingExitClassStatus(String),
    #[error("machine failure set is empty")]
    EmptyFailureSet,
    #[error("machine command is not admitted")]
    InvalidCommand,
}

#[must_use]
pub fn is_machine_command_name_safe(command: &str, max_bytes: usize) -> bool {
    !command.is_empty()
        && command.len() <= max_bytes
        && command
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

pub fn build_machine_outcome(
    policy: &MachineContractPolicy,
    command: &str,
    outcome: MachineOutcome,
) -> Result<MachineProcessOutcome, MachineContractError> {
    // r[impl cairn.cli_machine_contract.envelope]
    // r[impl cairn.cli_machine_contract.diagnostics]
    // r[impl cairn.cli_machine_contract.privacy]
    // r[impl cairn.cli_machine_contract.functional_boundary]
    // r[impl cairn.cli_machine_contract.functional_boundary.pure]
    if !is_machine_command_name_safe(command, policy.bounds.max_command_bytes) {
        return Err(MachineContractError::InvalidCommand);
    }
    match outcome {
        MachineOutcome::CommandResult {
            result,
            process_status,
        } => Ok(MachineProcessOutcome {
            envelope: MachineEnvelope {
                schema_version: policy.schema_version.clone(),
                ok: true,
                command: command.to_string(),
                exit_class: MachineExitClass::Success,
                result: Some(result),
                diagnostics: Vec::new(),
                non_claims: policy.required_non_claims.clone(),
            },
            process_status,
        }),
        MachineOutcome::ControlledFailure { failures } => {
            build_failure_outcome(policy, command, &failures)
        }
    }
}

fn build_failure_outcome(
    policy: &MachineContractPolicy,
    command: &str,
    failures: &[MachineFailureKind],
) -> Result<MachineProcessOutcome, MachineContractError> {
    if failures.is_empty() {
        return Err(MachineContractError::EmptyFailureSet);
    }
    let mut diagnostics = failures
        .iter()
        .map(|failure| diagnostic_from_kind(policy, *failure))
        .collect::<Result<Vec<_>, _>>()?;
    sort_machine_diagnostics(policy, &mut diagnostics);
    diagnostics.dedup_by(|left, right| left.code == right.code && left.target == right.target);
    diagnostics.truncate(policy.bounds.max_diagnostics);
    let primary_code = diagnostics
        .first()
        .ok_or(MachineContractError::EmptyFailureSet)?
        .code
        .as_str();
    let exit_class = diagnostic_definition(policy, primary_code)?.exit_class;
    let process_status = process_status(policy, exit_class)?;
    Ok(MachineProcessOutcome {
        envelope: MachineEnvelope {
            schema_version: policy.schema_version.clone(),
            ok: false,
            command: command.to_string(),
            exit_class,
            result: None,
            diagnostics,
            non_claims: policy.required_non_claims.clone(),
        },
        process_status,
    })
}

fn diagnostic_from_kind(
    policy: &MachineContractPolicy,
    failure: MachineFailureKind,
) -> Result<MachineDiagnostic, MachineContractError> {
    let definition = diagnostic_definition(policy, failure.diagnostic_code())?;
    Ok(MachineDiagnostic {
        code: definition.code.clone(),
        severity: definition.severity,
        message: definition.message.clone(),
        target: definition.target.clone(),
        fix: definition.fix.clone(),
    })
}

fn diagnostic_definition<'a>(
    policy: &'a MachineContractPolicy,
    code: &str,
) -> Result<&'a MachineDiagnosticDefinition, MachineContractError> {
    policy
        .diagnostics
        .iter()
        .find(|definition| definition.code == code)
        .ok_or_else(|| MachineContractError::MissingDiagnosticDefinition(code.to_string()))
}

fn process_status(
    policy: &MachineContractPolicy,
    exit_class: MachineExitClass,
) -> Result<u8, MachineContractError> {
    policy
        .exit_classes
        .iter()
        .find(|class| class.id == exit_class)
        .map(|class| class.process_status)
        .ok_or_else(|| {
            MachineContractError::MissingExitClassStatus(exit_class.as_str().to_string())
        })
}

pub fn sort_machine_diagnostics(
    policy: &MachineContractPolicy,
    diagnostics: &mut [MachineDiagnostic],
) {
    diagnostics.sort_by(|left, right| {
        diagnostic_order(policy, &left.code)
            .cmp(&diagnostic_order(policy, &right.code))
            .then(left.code.cmp(&right.code))
            .then(left.target.cmp(&right.target))
    });
}

fn diagnostic_order(policy: &MachineContractPolicy, code: &str) -> usize {
    policy
        .diagnostic_order
        .iter()
        .position(|admitted| admitted == code)
        .unwrap_or(policy.diagnostic_order.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_STATUS_SUCCESS: u8 = 0;
    const TEST_STATUS_INVALID_ARGUMENTS: u8 = 2;
    const TEST_STATUS_NOT_FOUND: u8 = 3;
    const TEST_STATUS_POLICY_ERROR: u8 = 4;
    const TEST_STATUS_BLOCKED: u8 = 5;
    const TEST_STATUS_INTERNAL_ERROR: u8 = 6;
    const TEST_BOUND: usize = 4_096;
    const TEST_MAX_DIAGNOSTICS: usize = 16;

    fn policy() -> MachineContractPolicy {
        let classes = [
            (MachineExitClass::Success, TEST_STATUS_SUCCESS),
            (
                MachineExitClass::InvalidArguments,
                TEST_STATUS_INVALID_ARGUMENTS,
            ),
            (MachineExitClass::NotFound, TEST_STATUS_NOT_FOUND),
            (MachineExitClass::PolicyError, TEST_STATUS_POLICY_ERROR),
            (MachineExitClass::Blocked, TEST_STATUS_BLOCKED),
            (MachineExitClass::InternalError, TEST_STATUS_INTERNAL_ERROR),
        ];
        let diagnostics = CLI_MACHINE_REQUIRED_DIAGNOSTIC_CODES
            .iter()
            .map(|code| MachineDiagnosticDefinition {
                code: (*code).to_string(),
                exit_class: if *code == CLI_MACHINE_DIAGNOSTIC_UNKNOWN_COMMAND
                    || *code == CLI_MACHINE_DIAGNOSTIC_MISSING_COMMAND
                    || *code == CLI_MACHINE_DIAGNOSTIC_INCOMPATIBLE_OPTION
                    || *code == CLI_MACHINE_DIAGNOSTIC_INVOCATION_FAILED
                {
                    MachineExitClass::InvalidArguments
                } else if *code == CLI_MACHINE_DIAGNOSTIC_NOT_FOUND {
                    MachineExitClass::NotFound
                } else if *code == CLI_MACHINE_DIAGNOSTIC_POLICY_ERROR {
                    MachineExitClass::PolicyError
                } else if *code == CLI_MACHINE_DIAGNOSTIC_BLOCKED {
                    MachineExitClass::Blocked
                } else {
                    MachineExitClass::InternalError
                },
                severity: MachineDiagnosticSeverity::Error,
                message: format!("stable message for {code}"),
                target: "command".to_string(),
                fix: None,
            })
            .collect::<Vec<_>>();
        MachineContractPolicy {
            schema_version: CLI_MACHINE_SCHEMA_V1.to_string(),
            exit_classes: classes
                .into_iter()
                .map(|(id, process_status)| MachineExitClassPolicy { id, process_status })
                .collect(),
            diagnostic_fields: vec![
                "code".to_string(),
                "severity".to_string(),
                "message".to_string(),
                "target".to_string(),
                "fix".to_string(),
            ],
            diagnostic_order: CLI_MACHINE_REQUIRED_DIAGNOSTIC_CODES
                .iter()
                .map(|code| (*code).to_string())
                .collect(),
            diagnostics,
            bounds: MachineContractBounds {
                max_command_bytes: TEST_BOUND,
                max_diagnostics: TEST_MAX_DIAGNOSTICS,
                max_code_bytes: TEST_BOUND,
                max_message_bytes: TEST_BOUND,
                max_target_bytes: TEST_BOUND,
                max_fix_bytes: TEST_BOUND,
                max_stdout_bytes: TEST_BOUND,
                max_stderr_bytes: TEST_BOUND,
            },
            required_non_claims: vec!["bounded transport claim".to_string()],
        }
    }

    #[test]
    fn success_preserves_nested_result_and_status() {
        let policy = policy();
        let result = serde_json::json!({
            "valid": false,
            "receipt_hash": "unchanged",
        });
        let outcome = build_machine_outcome(
            &policy,
            "validate",
            MachineOutcome::CommandResult {
                result: result.clone(),
                process_status: TEST_STATUS_INTERNAL_ERROR,
            },
        )
        .expect("valid policy builds success envelope");
        assert!(outcome.envelope.ok);
        assert_eq!(outcome.envelope.result, Some(result));
        assert_eq!(outcome.process_status, TEST_STATUS_INTERNAL_ERROR);
        assert_eq!(outcome.envelope.exit_class, MachineExitClass::Success);
        assert!(outcome.envelope.diagnostics.is_empty());
        assert_eq!(outcome.envelope.non_claims, policy.required_non_claims);
    }

    #[test]
    fn failure_uses_stable_policy_definition_and_nonzero_status() {
        let policy = policy();
        let outcome = build_machine_outcome(
            &policy,
            "validate",
            MachineOutcome::ControlledFailure {
                failures: vec![MachineFailureKind::UnknownCommand],
            },
        )
        .expect("known failure builds envelope");
        assert!(!outcome.envelope.ok);
        assert_eq!(outcome.envelope.result, None);
        assert_eq!(
            outcome.envelope.exit_class,
            MachineExitClass::InvalidArguments
        );
        assert_eq!(outcome.process_status, TEST_STATUS_INVALID_ARGUMENTS);
        assert_eq!(outcome.envelope.diagnostics.len(), 1);
        assert_eq!(
            outcome.envelope.diagnostics[0].code,
            CLI_MACHINE_DIAGNOSTIC_UNKNOWN_COMMAND
        );
    }

    #[test]
    fn diagnostic_order_ignores_message_text() {
        let policy = policy();
        let outcome = build_machine_outcome(
            &policy,
            "validate",
            MachineOutcome::ControlledFailure {
                failures: vec![
                    MachineFailureKind::Internal,
                    MachineFailureKind::UnknownCommand,
                ],
            },
        )
        .expect("known failures build envelope");
        let codes = outcome
            .envelope
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            codes,
            vec![
                CLI_MACHINE_DIAGNOSTIC_UNKNOWN_COMMAND,
                CLI_MACHINE_DIAGNOSTIC_INTERNAL_ERROR,
            ]
        );
    }

    #[test]
    fn failure_class_is_independent_of_input_order() {
        let policy = policy();
        let forward = build_machine_outcome(
            &policy,
            "validate",
            MachineOutcome::ControlledFailure {
                failures: vec![
                    MachineFailureKind::Internal,
                    MachineFailureKind::UnknownCommand,
                ],
            },
        )
        .expect("known failures build envelope");
        let reverse = build_machine_outcome(
            &policy,
            "validate",
            MachineOutcome::ControlledFailure {
                failures: vec![
                    MachineFailureKind::UnknownCommand,
                    MachineFailureKind::Internal,
                ],
            },
        )
        .expect("known failures build envelope");
        assert_eq!(forward, reverse);
        assert_eq!(
            forward.envelope.exit_class,
            MachineExitClass::InvalidArguments
        );
    }

    #[test]
    // r[verify cairn.cli_machine_contract.diagnostics.unknown]
    fn unknown_failure_definition_fails_closed() {
        let mut policy = policy();
        policy
            .diagnostics
            .retain(|definition| definition.code != CLI_MACHINE_DIAGNOSTIC_INTERNAL_ERROR);
        let error = build_machine_outcome(
            &policy,
            "validate",
            MachineOutcome::ControlledFailure {
                failures: vec![MachineFailureKind::Internal],
            },
        )
        .expect_err("missing fallback definition must fail");
        assert_eq!(
            error,
            MachineContractError::MissingDiagnosticDefinition(
                CLI_MACHINE_DIAGNOSTIC_INTERNAL_ERROR.to_string()
            )
        );
    }

    #[test]
    fn unsafe_command_name_is_not_admitted() {
        let error = build_machine_outcome(
            &policy(),
            "../../secret",
            MachineOutcome::ControlledFailure {
                failures: vec![MachineFailureKind::Internal],
            },
        )
        .expect_err("unsafe command name must fail");
        assert_eq!(error, MachineContractError::InvalidCommand);
    }
}
