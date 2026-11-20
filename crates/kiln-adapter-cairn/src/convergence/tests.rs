use super::*;
use kiln_core::convergence::{EvidenceState, select_unsatisfied};
use serde_json::{Value, json};

fn fixture() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/cairn/convergence/pilot/receipt-retained.json"
    ))
    .unwrap_or_else(|error| unreachable!("pilot fixture: {error}"))
}

fn value() -> Value {
    serde_json::from_slice(&fixture())
        .unwrap_or_else(|error| unreachable!("pilot fixture: {error}"))
}

fn identity(value: &Value) -> ContentId {
    ContentId::from_blake3_hex(
        value
            .get("receipt_blake3")
            .and_then(Value::as_str)
            .unwrap_or_else(|| unreachable!("receipt_blake3")),
    )
    .unwrap_or_else(|_| unreachable!("receipt_blake3"))
}

fn binding_from(value: &Value) -> ConvergenceReceiptBinding {
    let inputs = value
        .get("inputs")
        .unwrap_or_else(|| unreachable!("inputs"));
    let revision = value
        .get("revision")
        .unwrap_or_else(|| unreachable!("revision"));
    ConvergenceReceiptBinding {
        receipt_blake3: identity(value),
        revision_kind: revision
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_else(|| unreachable!("kind"))
            .to_string(),
        revision: revision
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or_else(|| unreachable!("value"))
            .to_string(),
        registry_blake3: content_field(inputs, "registry_blake3"),
        case_blake3: content_field(inputs, "case_blake3"),
        policy_blake3: content_field(inputs, "policy_blake3"),
    }
}

fn content_field(value: &Value, field: &str) -> ContentId {
    ContentId::from_blake3_hex(
        value
            .get(field)
            .and_then(Value::as_str)
            .unwrap_or_else(|| unreachable!("{field}")),
    )
    .unwrap_or_else(|_| unreachable!("{field}"))
}

fn reseal(value: &mut Value) {
    let computed = canonical_receipt_identity(value).unwrap_or_else(|_| unreachable!("reseal"));
    value
        .as_object_mut()
        .unwrap_or_else(|| unreachable!("object"))
        .insert(
            "receipt_blake3".into(),
            Value::String(computed.as_str().to_string()),
        );
}

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap_or_else(|error| unreachable!("encode fixture: {error}"))
}

fn field_mut<'a>(value: &'a mut Value, key: &str) -> &'a mut Value {
    value
        .as_object_mut()
        .and_then(|object| object.get_mut(key))
        .unwrap_or_else(|| unreachable!("field {key}"))
}

fn row_mut(value: &mut Value, index: usize) -> &mut Value {
    value
        .as_object_mut()
        .and_then(|object| object.get_mut("rows"))
        .and_then(Value::as_array_mut)
        .and_then(|rows| rows.get_mut(index))
        .unwrap_or_else(|| unreachable!("row {index}"))
}

fn row(value: &Value, index: usize) -> &Value {
    value
        .get("rows")
        .and_then(Value::as_array)
        .and_then(|rows| rows.get(index))
        .unwrap_or_else(|| unreachable!("row {index}"))
}

fn admit(value: &mut Value) -> Result<AdmittedConvergenceReceipt, Diagnostic> {
    reseal(value);
    let binding = binding_from(value);
    admit_convergence_receipt(&bytes(value), &binding)
}

#[test]
fn admits_the_retained_pilot_receipt() {
    let bytes = fixture();
    let value = value();
    let binding = binding_from(&value);
    let admitted = admit_convergence_receipt(&bytes, &binding).unwrap_or_else(|_| unreachable!());
    assert_eq!(admitted.receipt_blake3, binding.receipt_blake3);
    assert_eq!(admitted.rows.len(), 740);
    assert_eq!(
        admitted
            .rows
            .iter()
            .filter(|row| row.state == EvidenceState::Current)
            .count(),
        1
    );
    assert_eq!(
        admitted
            .rows
            .iter()
            .filter(|row| row.state == EvidenceState::Untargeted)
            .count(),
        739
    );
    assert!(admitted.targets_current);
    assert!(!admitted.coverage_complete);
    assert!(!admitted.complete);
    assert!(admitted.unbound_target_issues.is_empty());
    assert_eq!(admitted.revision, binding.revision);
    assert_eq!(admitted.registry_blake3, binding.registry_blake3);
}

#[test]
fn current_and_untargeted_rows_are_not_acceptance_or_work() {
    let mut value = value();
    let admitted = admit(&mut value).unwrap_or_else(|_| unreachable!());
    let selection = select_unsatisfied(&admitted.rows).unwrap_or_else(|_| unreachable!());
    assert!(selection.items.is_empty());
    assert_eq!(selection.exclusions.len(), 740);
    assert!(!admitted.complete);
}

#[test]
fn rejects_unknown_schema() {
    let mut value = value();
    *field_mut(&mut value, "schema_version") = json!("cairn.convergence.v2");
    let binding = binding_from(&value);
    assert_eq!(
        admit_convergence_receipt(&bytes(&value), &binding).map(|_| ()),
        Err(Diagnostic::new(
            ISSUE_UNKNOWN_SCHEMA,
            "convergence.receipt.schema_version",
            "convergence receipt schema is not admitted",
        ))
    );
}

#[test]
fn rejects_unknown_status() {
    let mut value = value();
    *field_mut(&mut value, "status") = json!("draft");
    let binding = binding_from(&value);
    assert_eq!(
        admit_convergence_receipt(&bytes(&value), &binding).map(|_| ()),
        Err(Diagnostic::new(
            ISSUE_UNKNOWN_STATUS,
            "convergence.receipt.status",
            "convergence receipt status is not admitted",
        ))
    );
}

#[test]
fn rejects_unknown_top_level_or_row_field() {
    let mut top = value();
    top.as_object_mut()
        .unwrap_or_else(|| unreachable!("object"))
        .insert("extra".into(), json!(true));
    let binding = binding_from(&top);
    assert_eq!(
        admit_convergence_receipt(&bytes(&top), &binding).map(|_| ()),
        Err(malformed())
    );

    let mut row_value = value();
    row_mut(&mut row_value, 0)
        .as_object_mut()
        .unwrap_or_else(|| unreachable!("row object"))
        .insert("extra".into(), json!(true));
    let binding = binding_from(&row_value);
    assert_eq!(
        admit_convergence_receipt(&bytes(&row_value), &binding).map(|_| ()),
        Err(malformed())
    );
}

#[test]
fn rejects_unknown_state_after_identity_holds() {
    let mut value = value();
    *field_mut(row_mut(&mut value, 0), "state") = json!("mystery");
    assert_eq!(
        admit(&mut value).map(|_| ()),
        Err(Diagnostic::new(
            ISSUE_UNKNOWN_STATE,
            "convergence.rows.state",
            "convergence row state is not admitted",
        ))
    );
}

#[test]
fn rejects_malformed_json() {
    let value = value();
    let binding = binding_from(&value);
    assert_eq!(
        admit_convergence_receipt(b"{", &binding).map(|_| ()),
        Err(malformed())
    );
}

#[test]
fn rejects_oversized_input_before_parsing() {
    let value = value();
    let binding = binding_from(&value);
    let oversized = vec![b' '; MAX_CONVERGENCE_RECEIPT_BYTES + 1];
    assert_eq!(
        admit_convergence_receipt(&oversized, &binding).map(|_| ()),
        Err(Diagnostic::new(
            ISSUE_OVERSIZED,
            "convergence.receipt.bytes",
            "convergence receipt exceeds its named byte bound",
        ))
    );
}

#[test]
fn rejects_a_tampered_receipt_identity() {
    let mut value = value();
    let binding = binding_from(&value);
    *field_mut(row_mut(&mut value, 0), "requirement_id") = json!("tampered.row");
    assert_eq!(
        admit_convergence_receipt(&bytes(&value), &binding).map(|_| ()),
        Err(Diagnostic::new(
            ISSUE_RECEIPT_IDENTITY_MISMATCH,
            "convergence.receipt.receipt_blake3",
            "recomputed receipt BLAKE3 differs from the receipt field",
        ))
    );
}

#[test]
fn rejects_a_stale_declared_binding() {
    let bytes = fixture();
    let value = value();
    let mut binding = binding_from(&value);
    binding.registry_blake3 =
        ContentId::from_blake3_hex("a".repeat(64)).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        admit_convergence_receipt(&bytes, &binding).map(|_| ()),
        Err(binding_mismatch("convergence.inputs.registry_blake3"))
    );

    let mut binding = binding_from(&value);
    binding.receipt_blake3 =
        ContentId::from_blake3_hex("b".repeat(64)).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        admit_convergence_receipt(&bytes, &binding).map(|_| ()),
        Err(binding_mismatch("convergence.binding.receipt_blake3"))
    );

    let mut binding = binding_from(&value);
    binding.revision = "0000000000000000000000000000000000000000".to_string();
    assert_eq!(
        admit_convergence_receipt(&bytes, &binding).map(|_| ()),
        Err(binding_mismatch("convergence.receipt.revision.value"))
    );

    let mut binding = binding_from(&value);
    binding.revision_kind = "jujutsu".to_string();
    assert_eq!(
        admit_convergence_receipt(&bytes, &binding).map(|_| ()),
        Err(binding_mismatch("convergence.binding.revision_kind"))
    );

    let mut binding = binding_from(&value);
    binding.case_blake3 =
        ContentId::from_blake3_hex("c".repeat(64)).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        admit_convergence_receipt(&bytes, &binding).map(|_| ()),
        Err(binding_mismatch("convergence.inputs.case_blake3"))
    );

    let mut binding = binding_from(&value);
    binding.policy_blake3 =
        ContentId::from_blake3_hex("d".repeat(64)).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        admit_convergence_receipt(&bytes, &binding).map(|_| ()),
        Err(binding_mismatch("convergence.inputs.policy_blake3"))
    );
}

#[test]
fn rejects_a_non_git_receipt_revision() {
    let mut value = value();
    let mut binding = binding_from(&value);
    binding.revision_kind = "git".to_string();
    *field_mut(field_mut(&mut value, "revision"), "kind") = json!("jujutsu");
    reseal(&mut value);
    assert_eq!(
        admit_convergence_receipt(&bytes(&value), &binding).map(|_| ()),
        Err(Diagnostic::new(
            ISSUE_INVALID_REVISION_KIND,
            "convergence.receipt.revision.kind",
            "only a Git revision binds a Kiln candidate",
        ))
    );
}

#[test]
fn rejects_duplicate_requirement_rows_after_identity_holds() {
    let mut value = value();
    let duplicate = row(&value, 0).clone();
    field_mut(&mut value, "rows")
        .as_array_mut()
        .unwrap_or_else(|| unreachable!("rows"))
        .push(duplicate);
    assert_eq!(
        admit(&mut value).map(|_| ()),
        Err(Diagnostic::new(
            ISSUE_DUPLICATE_REQUIREMENT,
            "convergence.rows.requirement_id",
            "a requirement identity appears more than once",
        ))
    );
}

#[test]
fn rejects_a_row_count_over_the_named_bound() {
    let mut value = value();
    let template = row(&value, 0).clone();
    let rows = value
        .get_mut("rows")
        .and_then(Value::as_array_mut)
        .unwrap_or_else(|| unreachable!("rows"));
    rows.clear();
    for index in 0..=MAX_CONVERGENCE_ROWS {
        let mut row = template.clone();
        *field_mut(&mut row, "requirement_id") = json!(format!("synthetic.{index}"));
        rows.push(row);
    }
    assert_eq!(
        admit(&mut value).map(|_| ()),
        Err(Diagnostic::new(
            ISSUE_ROW_BOUND,
            "convergence.rows",
            "convergence row count exceeds its named bound",
        ))
    );
}

#[test]
fn rejects_a_malformed_row_identity_after_identity_holds() {
    let mut value = value();
    *field_mut(row_mut(&mut value, 0), "row_blake3") = json!("zz");
    assert_eq!(
        admit(&mut value).map(|_| ()),
        Err(Diagnostic::new(
            ISSUE_INVALID_IDENTITY,
            "convergence.rows.row_blake3",
            "identity must be 64 hexadecimal characters",
        ))
    );
}

#[test]
fn rejects_an_unknown_bound_field() {
    let mut value = value();
    *field_mut(row_mut(&mut value, 0), "bound") = json!({ "unknown": "fact" });
    let binding = binding_from(&value);
    assert_eq!(
        admit_convergence_receipt(&bytes(&value), &binding).map(|_| ()),
        Err(malformed())
    );
}

#[test]
fn rejects_a_non_prefixed_producer_identity() {
    let mut value = value();
    *field_mut(row_mut(&mut value, 0), "bound") =
        json!({ "producer_job_identity": "a".repeat(64) });
    assert_eq!(
        admit(&mut value).map(|_| ()),
        Err(Diagnostic::new(
            ISSUE_INVALID_IDENTITY,
            "convergence.rows.bound.producer_job_identity",
            "identity must be 64 hexadecimal characters",
        ))
    );
}
