const FIELD_MACHINE_POLICY: &str = "machine_contract_policy";
const FIELD_SCHEMA_VERSION: &str = "machine_contract_policy.schema_version";
const FIELD_EXIT_CLASSES: &str = "machine_contract_policy.exit_classes";
const FIELD_EXIT_CLASS_ID: &str = "machine_contract_policy.exit_classes.id";
const FIELD_EXIT_CLASS_STATUS: &str = "machine_contract_policy.exit_classes.process_status";
const FIELD_DIAGNOSTIC_FIELDS: &str = "machine_contract_policy.diagnostic_fields";
const FIELD_DIAGNOSTIC_ORDER: &str = "machine_contract_policy.diagnostic_order";
const FIELD_DIAGNOSTICS: &str = "machine_contract_policy.diagnostics";
const FIELD_DIAGNOSTIC_CODE: &str = "machine_contract_policy.diagnostics.code";
const FIELD_DIAGNOSTIC_EXIT_CLASS: &str = "machine_contract_policy.diagnostics.exit_class";
const FIELD_DIAGNOSTIC_SEVERITY: &str = "machine_contract_policy.diagnostics.severity";
const FIELD_DIAGNOSTIC_MESSAGE: &str = "machine_contract_policy.diagnostics.message";
const FIELD_DIAGNOSTIC_TARGET: &str = "machine_contract_policy.diagnostics.target";
const FIELD_DIAGNOSTIC_FIX: &str = "machine_contract_policy.diagnostics.fix";
const FIELD_BOUNDS: &str = "machine_contract_policy.bounds";
const FIELD_MAX_COMMAND_BYTES: &str = "machine_contract_policy.bounds.max_command_bytes";
const FIELD_MAX_DIAGNOSTICS: &str = "machine_contract_policy.bounds.max_diagnostics";
const FIELD_MAX_CODE_BYTES: &str = "machine_contract_policy.bounds.max_code_bytes";
const FIELD_MAX_MESSAGE_BYTES: &str = "machine_contract_policy.bounds.max_message_bytes";
const FIELD_MAX_TARGET_BYTES: &str = "machine_contract_policy.bounds.max_target_bytes";
const FIELD_MAX_FIX_BYTES: &str = "machine_contract_policy.bounds.max_fix_bytes";
const FIELD_MAX_STDOUT_BYTES: &str = "machine_contract_policy.bounds.max_stdout_bytes";
const FIELD_MAX_STDERR_BYTES: &str = "machine_contract_policy.bounds.max_stderr_bytes";
const FIELD_REQUIRED_NON_CLAIMS: &str = "machine_contract_policy.required_non_claims";
const EXPECTED_DIAGNOSTIC_FIELDS: &[&str] = &["code", "severity", "message", "target", "fix"];
const SERIALIZATION_OVERHEAD_PER_ENTRY: usize = 128;

pub(super) fn from_value(
    value: &serde_json::Value,
) -> Result<crate::verified::cli_machine::MachineContractPolicy, super::PolicyError> {
    // r[impl cairn.cli_machine_contract.policy]
    let policy = value
        .get(FIELD_MACHINE_POLICY)
        .ok_or(super::PolicyError::MissingField(FIELD_MACHINE_POLICY))?;
    let schema_version = super::fields::string_field(policy, FIELD_SCHEMA_VERSION)?;
    if schema_version != crate::verified::cli_machine::CLI_MACHINE_SCHEMA_V1 {
        return Err(super::PolicyError::InvalidField(FIELD_SCHEMA_VERSION));
    }
    let bounds_value = policy
        .get("bounds")
        .ok_or(super::PolicyError::MissingField(FIELD_BOUNDS))?;
    let bounds = crate::verified::cli_machine::MachineContractBounds {
        max_command_bytes: bounded_usize_field(
            bounds_value,
            FIELD_MAX_COMMAND_BYTES,
            crate::verified::cli_machine::CLI_MACHINE_MAX_COMMAND_BYTES_LIMIT,
        )?,
        max_diagnostics: bounded_usize_field(
            bounds_value,
            FIELD_MAX_DIAGNOSTICS,
            crate::verified::cli_machine::CLI_MACHINE_MAX_DIAGNOSTICS_LIMIT,
        )?,
        max_code_bytes: bounded_usize_field(
            bounds_value,
            FIELD_MAX_CODE_BYTES,
            crate::verified::cli_machine::CLI_MACHINE_MAX_FIELD_BYTES_LIMIT,
        )?,
        max_message_bytes: bounded_usize_field(
            bounds_value,
            FIELD_MAX_MESSAGE_BYTES,
            crate::verified::cli_machine::CLI_MACHINE_MAX_FIELD_BYTES_LIMIT,
        )?,
        max_target_bytes: bounded_usize_field(
            bounds_value,
            FIELD_MAX_TARGET_BYTES,
            crate::verified::cli_machine::CLI_MACHINE_MAX_FIELD_BYTES_LIMIT,
        )?,
        max_fix_bytes: bounded_usize_field(
            bounds_value,
            FIELD_MAX_FIX_BYTES,
            crate::verified::cli_machine::CLI_MACHINE_MAX_FIELD_BYTES_LIMIT,
        )?,
        max_stdout_bytes: bounded_usize_field(
            bounds_value,
            FIELD_MAX_STDOUT_BYTES,
            crate::verified::cli_machine::CLI_MACHINE_MAX_CAPTURE_BYTES_LIMIT,
        )?,
        max_stderr_bytes: bounded_usize_field(
            bounds_value,
            FIELD_MAX_STDERR_BYTES,
            crate::verified::cli_machine::CLI_MACHINE_MAX_CAPTURE_BYTES_LIMIT,
        )?,
    };
    let exit_classes = parse_exit_classes(policy)?;
    let diagnostic_fields = super::fields::string_array_field(policy, FIELD_DIAGNOSTIC_FIELDS)?;
    if diagnostic_fields
        != EXPECTED_DIAGNOSTIC_FIELDS
            .iter()
            .map(|field| (*field).to_string())
            .collect::<Vec<_>>()
    {
        return Err(super::PolicyError::InvalidField(FIELD_DIAGNOSTIC_FIELDS));
    }
    let diagnostics = parse_diagnostics(policy, &bounds)?;
    let diagnostic_order = super::fields::string_array_field(policy, FIELD_DIAGNOSTIC_ORDER)?;
    validate_diagnostic_order(&diagnostics, &diagnostic_order)?;
    let required_non_claims = super::fields::string_array_field(policy, FIELD_REQUIRED_NON_CLAIMS)?;
    if required_non_claims.is_empty()
        || required_non_claims.len() > bounds.max_diagnostics
        || required_non_claims
            .iter()
            .any(|claim| !is_single_line_bounded(claim, bounds.max_message_bytes))
    {
        return Err(super::PolicyError::InvalidField(FIELD_REQUIRED_NON_CLAIMS));
    }
    validate_combined_bounds(&bounds)?;
    Ok(crate::verified::cli_machine::MachineContractPolicy {
        schema_version,
        exit_classes,
        diagnostic_fields,
        diagnostic_order,
        diagnostics,
        bounds,
        required_non_claims,
    })
}

fn parse_exit_classes(
    policy: &serde_json::Value,
) -> Result<Vec<crate::verified::cli_machine::MachineExitClassPolicy>, super::PolicyError> {
    let values = policy
        .get("exit_classes")
        .and_then(serde_json::Value::as_array)
        .ok_or(super::PolicyError::MissingField(FIELD_EXIT_CLASSES))?;
    if values.len() != crate::verified::cli_machine::CLI_MACHINE_EXIT_CLASSES.len() {
        return Err(super::PolicyError::InvalidField(FIELD_EXIT_CLASSES));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut classes = Vec::with_capacity(values.len());
    for value in values {
        let id_text = super::fields::string_field(value, FIELD_EXIT_CLASS_ID)?;
        let Some(id) = crate::verified::cli_machine::MachineExitClass::parse(&id_text) else {
            return Err(super::PolicyError::InvalidField(FIELD_EXIT_CLASS_ID));
        };
        if !seen.insert(id) {
            return Err(super::PolicyError::InvalidField(FIELD_EXIT_CLASSES));
        }
        let status = value
            .get("process_status")
            .and_then(serde_json::Value::as_u64)
            .ok_or(super::PolicyError::MissingField(FIELD_EXIT_CLASS_STATUS))?;
        let process_status = u8::try_from(status)
            .map_err(|_| super::PolicyError::InvalidField(FIELD_EXIT_CLASS_STATUS))?;
        if process_status > crate::verified::cli_machine::CLI_MACHINE_MAX_PROCESS_STATUS
            || (id == crate::verified::cli_machine::MachineExitClass::Success
                && process_status != 0)
            || (id != crate::verified::cli_machine::MachineExitClass::Success
                && process_status == 0)
        {
            return Err(super::PolicyError::InvalidField(FIELD_EXIT_CLASS_STATUS));
        }
        classes.push(crate::verified::cli_machine::MachineExitClassPolicy { id, process_status });
    }
    if crate::verified::cli_machine::CLI_MACHINE_EXIT_CLASSES
        .iter()
        .any(|class| !seen.contains(class))
    {
        return Err(super::PolicyError::InvalidField(FIELD_EXIT_CLASSES));
    }
    Ok(classes)
}

fn parse_diagnostics(
    policy: &serde_json::Value,
    bounds: &crate::verified::cli_machine::MachineContractBounds,
) -> Result<Vec<crate::verified::cli_machine::MachineDiagnosticDefinition>, super::PolicyError> {
    let values = policy
        .get("diagnostics")
        .and_then(serde_json::Value::as_array)
        .ok_or(super::PolicyError::MissingField(FIELD_DIAGNOSTICS))?;
    if values.is_empty() || values.len() > bounds.max_diagnostics {
        return Err(super::PolicyError::InvalidField(FIELD_DIAGNOSTICS));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut definitions = Vec::with_capacity(values.len());
    for value in values {
        let code = super::fields::string_field(value, FIELD_DIAGNOSTIC_CODE)?;
        if !is_diagnostic_code_safe(&code, bounds.max_code_bytes) || !seen.insert(code.clone()) {
            return Err(super::PolicyError::InvalidField(FIELD_DIAGNOSTIC_CODE));
        }
        let exit_class_text = super::fields::string_field(value, FIELD_DIAGNOSTIC_EXIT_CLASS)?;
        let Some(exit_class) =
            crate::verified::cli_machine::MachineExitClass::parse(&exit_class_text)
        else {
            return Err(super::PolicyError::InvalidField(
                FIELD_DIAGNOSTIC_EXIT_CLASS,
            ));
        };
        if exit_class == crate::verified::cli_machine::MachineExitClass::Success {
            return Err(super::PolicyError::InvalidField(
                FIELD_DIAGNOSTIC_EXIT_CLASS,
            ));
        }
        let severity_text = super::fields::string_field(value, FIELD_DIAGNOSTIC_SEVERITY)?;
        let Some(severity) =
            crate::verified::cli_machine::MachineDiagnosticSeverity::parse(&severity_text)
        else {
            return Err(super::PolicyError::InvalidField(FIELD_DIAGNOSTIC_SEVERITY));
        };
        let message = super::fields::string_field(value, FIELD_DIAGNOSTIC_MESSAGE)?;
        if !is_single_line_bounded(&message, bounds.max_message_bytes) {
            return Err(super::PolicyError::InvalidField(FIELD_DIAGNOSTIC_MESSAGE));
        }
        let target = super::fields::string_field(value, FIELD_DIAGNOSTIC_TARGET)?;
        if !is_diagnostic_target_safe(&target, bounds.max_target_bytes) {
            return Err(super::PolicyError::InvalidField(FIELD_DIAGNOSTIC_TARGET));
        }
        let fix_text = super::fields::string_field(value, FIELD_DIAGNOSTIC_FIX)?;
        let fix = if fix_text.is_empty() {
            None
        } else if is_single_line_bounded(&fix_text, bounds.max_fix_bytes) {
            Some(fix_text)
        } else {
            return Err(super::PolicyError::InvalidField(FIELD_DIAGNOSTIC_FIX));
        };
        definitions.push(crate::verified::cli_machine::MachineDiagnosticDefinition {
            code,
            exit_class,
            severity,
            message,
            target,
            fix,
        });
    }
    if crate::verified::cli_machine::CLI_MACHINE_REQUIRED_DIAGNOSTIC_CODES
        .iter()
        .any(|required| !seen.contains(*required))
    {
        return Err(super::PolicyError::InvalidField(FIELD_DIAGNOSTICS));
    }
    Ok(definitions)
}

fn validate_diagnostic_order(
    diagnostics: &[crate::verified::cli_machine::MachineDiagnosticDefinition],
    diagnostic_order: &[String],
) -> Result<(), super::PolicyError> {
    if diagnostics.len() != diagnostic_order.len() {
        return Err(super::PolicyError::InvalidField(FIELD_DIAGNOSTIC_ORDER));
    }
    let definitions = diagnostics
        .iter()
        .map(|definition| definition.code.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let order = diagnostic_order
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    if order.len() != diagnostic_order.len() || definitions != order {
        return Err(super::PolicyError::InvalidField(FIELD_DIAGNOSTIC_ORDER));
    }
    Ok(())
}

fn validate_combined_bounds(
    bounds: &crate::verified::cli_machine::MachineContractBounds,
) -> Result<(), super::PolicyError> {
    let diagnostic_bytes = bounds
        .max_code_bytes
        .checked_add(bounds.max_message_bytes)
        .and_then(|total| total.checked_add(bounds.max_target_bytes))
        .and_then(|total| total.checked_add(bounds.max_fix_bytes))
        .and_then(|total| total.checked_add(SERIALIZATION_OVERHEAD_PER_ENTRY))
        .and_then(|total| total.checked_mul(bounds.max_diagnostics))
        .ok_or(super::PolicyError::InvalidField(FIELD_MAX_STDOUT_BYTES))?;
    let non_claim_bytes = bounds
        .max_message_bytes
        .checked_add(SERIALIZATION_OVERHEAD_PER_ENTRY)
        .and_then(|total| total.checked_mul(bounds.max_diagnostics))
        .ok_or(super::PolicyError::InvalidField(FIELD_MAX_STDOUT_BYTES))?;
    let minimum_failure_envelope_bytes = diagnostic_bytes
        .checked_add(non_claim_bytes)
        .ok_or(super::PolicyError::InvalidField(FIELD_MAX_STDOUT_BYTES))?;
    if minimum_failure_envelope_bytes > bounds.max_stdout_bytes {
        return Err(super::PolicyError::InvalidField(FIELD_MAX_STDOUT_BYTES));
    }
    Ok(())
}

fn bounded_usize_field(
    value: &serde_json::Value,
    field: &'static str,
    maximum: usize,
) -> Result<usize, super::PolicyError> {
    let lookup = field.rsplit('.').next().unwrap_or(field);
    let raw = value
        .get(lookup)
        .and_then(serde_json::Value::as_u64)
        .ok_or(super::PolicyError::MissingField(field))?;
    let converted = usize::try_from(raw).map_err(|_| super::PolicyError::InvalidField(field))?;
    if converted == 0 || converted > maximum {
        return Err(super::PolicyError::InvalidField(field));
    }
    Ok(converted)
}

fn is_diagnostic_code_safe(value: &str, max_bytes: usize) -> bool {
    let Some(first) = value.bytes().next() else {
        return false;
    };
    (first.is_ascii_lowercase() || first.is_ascii_digit())
        && value.len() <= max_bytes
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn is_diagnostic_target_safe(value: &str, max_bytes: usize) -> bool {
    if value.is_empty()
        || value.len() > max_bytes
        || value.starts_with('/')
        || value.starts_with('\\')
    {
        return false;
    }
    if value
        .split(['/', '\\'])
        .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return false;
    }
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/'))
}

fn is_single_line_bounded(value: &str, max_bytes: usize) -> bool {
    !value.is_empty() && value.len() <= max_bytes && !value.contains('\n') && !value.contains('\r')
}
