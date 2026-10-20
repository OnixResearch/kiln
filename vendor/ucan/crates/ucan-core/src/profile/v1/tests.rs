//! Positive and negative admission tests for the no-std wire-profile core.

use alloc::vec;

use super::*;

fn delegation_ok() -> FieldAdmission {
    let mut admission = FieldAdmission::for_kind(SpecKind::Delegation);
    admission.observe(FIELD_ISSUER, 64).expect("issuer");
    admission.observe(FIELD_AUDIENCE, 64).expect("audience");
    admission
}

#[test]
fn valid_delegation_fields_admit() {
    let mut admission = delegation_ok();
    admission.observe(FIELD_SUBJECT, 64).expect("subject");
    admission.observe(FIELD_COMMAND, 8).expect("command");
    admission.observe(FIELD_POLICY, 4).expect("policy");
    admission.observe(FIELD_EXPIRES, 4).expect("expires");
    admission.observe(FIELD_NONCE, 16).expect("nonce");
    assert_eq!(admission.admit(), Ok(()));
    assert_eq!(SpecKind::from_tag(DELEGATION_TAG), Some(SpecKind::Delegation));
    assert_eq!(SpecKind::from_tag(INVOCATION_TAG), Some(SpecKind::Invocation));
    assert_eq!(SpecKind::from_tag("ucan/future@1.0.0"), None);
}

#[test]
fn valid_invocation_fields_admit() {
    let mut admission = FieldAdmission::for_kind(SpecKind::Invocation);
    admission.observe(FIELD_ISSUER, 64).expect("issuer");
    admission.observe(FIELD_AUDIENCE, 64).expect("audience");
    admission.observe(FIELD_COMMAND, 8).expect("command");
    admission.observe(FIELD_PROOFS, 2).expect("proofs");
    assert_eq!(admission.admit(), Ok(()));
}

#[test]
fn missing_required_fields_fail_closed() {
    let missing_issuer = FieldAdmission::for_kind(SpecKind::Delegation);
    assert_eq!(
        missing_issuer.admit(),
        Err(WireError::MissingField {
            field: FIELD_ISSUER.to_owned()
        })
    );

    let mut no_command = FieldAdmission::for_kind(SpecKind::Invocation);
    no_command.observe(FIELD_ISSUER, 64).expect("issuer");
    no_command.observe(FIELD_AUDIENCE, 64).expect("audience");
    no_command.observe(FIELD_PROOFS, 2).expect("proofs");
    assert_eq!(
        no_command.admit(),
        Err(WireError::MissingField {
            field: FIELD_COMMAND.to_owned()
        })
    );

    let mut no_proofs = FieldAdmission::for_kind(SpecKind::Invocation);
    no_proofs.observe(FIELD_ISSUER, 64).expect("issuer");
    no_proofs.observe(FIELD_AUDIENCE, 64).expect("audience");
    no_proofs.observe(FIELD_COMMAND, 8).expect("command");
    assert_eq!(
        no_proofs.admit(),
        Err(WireError::MissingField {
            field: FIELD_PROOFS.to_owned()
        })
    );
}

#[test]
fn unknown_and_forbidden_fields_fail_closed() {
    let mut unknown = delegation_ok();
    assert_eq!(
        unknown.observe("zzz", 3),
        Err(WireError::UnknownField {
            field: "zzz".to_owned()
        })
    );

    // `args` belongs to Invocation only; a Delegation must reject it.
    let mut forbidden = delegation_ok();
    assert_eq!(
        forbidden.observe(FIELD_ARGS, 4),
        Err(WireError::ForbiddenField {
            field: FIELD_ARGS.to_owned()
        })
    );

    // A tag substitution must select a different nominal kind.
    assert_eq!(SpecKind::from_tag(DELEGATION_TAG), Some(SpecKind::Delegation));
    assert_ne!(SpecKind::Delegation, SpecKind::Invocation);
}

#[test]
fn oversize_fields_fail_before_allocation() {
    let mut admission = FieldAdmission::for_kind(SpecKind::Invocation);
    admission.observe(FIELD_ISSUER, MAX_URI_FIELD_BYTES + 1).expect_err("oversize issuer");
    admission.observe(FIELD_ISSUER, MAX_URI_FIELD_BYTES).expect("boundary issuer");
    admission.observe(FIELD_AUDIENCE, 64).expect("audience");
    admission.observe(FIELD_COMMAND, 8).expect("command");
    admission.observe(FIELD_PROOFS, 2).expect("proofs");
    assert_eq!(admission.admit(), Ok(()));
}

#[test]
fn valid_slash_commands_are_accepted() {
    for command in ["/account", "/msg/send", "/f", "/a/b/c"] {
        let parsed = SlashCommand::new(command).expect("command should validate");
        assert_eq!(parsed.as_str(), command);
    }
}

#[test]
fn invalid_slash_commands_are_rejected() {
    for (command, message) in [
        ("account", "must start with a slash"),
        ("/", "length outside 2..=256"),
        ("/a/", "must not end with a slash"),
        ("/a//b", "empty segment"),
        ("/a b", "unsupported separator character"),
    ] {
        assert!(
            matches!(SlashCommand::new(command), Err(WireError::InvalidCommand { message: found }) if found == message),
            "command {command:?} must fail with {message}"
        );
    }
}

#[test]
fn nonce_admits_only_bounded_bytes() {
    let ok = Nonce::new(vec![7; MAX_NONCE_BYTES]).expect("nonce at the limit");
    assert_eq!(ok.as_slice(), &vec![7; MAX_NONCE_BYTES][..]);
    assert_eq!(
        Nonce::new(vec![7; MAX_NONCE_BYTES + 1]),
        Err(WireError::NonceTooLarge {
            len: MAX_NONCE_BYTES + 1
        })
    );
}

#[test]
fn timestamps_enforce_safe_integer_bounds_and_window() {
    let zero = TimeSeconds::new(0).expect("zero");
    assert_eq!(zero.as_i64(), 0);
    let max_value = TimeSeconds::new(MAX_TIMESTAMP_ABS).expect("max");
    assert_eq!(max_value.as_i64(), MAX_TIMESTAMP_ABS);
    let min_value = TimeSeconds::new(MIN_TIMESTAMP_ABS).expect("min");
    assert_eq!(min_value.as_i64(), MIN_TIMESTAMP_ABS);
    assert_eq!(
        TimeSeconds::new(MAX_TIMESTAMP_ABS + 1),
        Err(WireError::TimestampOutOfBounds {
            field: "timestamp".to_owned()
        })
    );
    assert_eq!(admit_time_window(Some(100), Some(200)), Ok(()));
    assert_eq!(admit_time_window(Some(200), Some(100)), Err(WireError::InvertedTimeWindow));
    assert_eq!(
        admit_time_window(Some(MAX_TIMESTAMP_ABS + 1), None),
        Err(WireError::TimestampOutOfBounds {
            field: "timestamp".to_owned()
        })
    );
    assert_eq!(
        admit_time_window(None, Some(MIN_TIMESTAMP_ABS - 1)),
        Err(WireError::TimestampOutOfBounds {
            field: "timestamp".to_owned()
        })
    );
}

#[test]
fn ipld_value_bounds_are_enforced() {
    let deep = IpldValue::List(vec![IpldValue::List(vec![IpldValue::List(vec![IpldValue::Null])])]);
    assert!(IpldValue::new(deep).is_ok());
    let too_many = IpldValue::List(vec![IpldValue::Null; MAX_IPLD_LIST_ITEMS + 1]);
    assert!(matches!(IpldValue::new(too_many), Err(WireError::ValueTooLarge { .. })));
}
