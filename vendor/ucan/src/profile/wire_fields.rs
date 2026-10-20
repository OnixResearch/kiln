//! Test-support payload field builders for final-profile Delegation and
//! Invocation envelopes. These mirror the fixture projection used by the
//! wire slice without depending on fixture files.

use ucan_core::profile::v1 as core;

use crate::profile::v1::Cbor;

/// Build Delegation payload fields.
#[must_use]
pub fn delegation_payload(
    issuer: &str,
    audience: &str,
    subject: Option<&str>,
    command: Option<&str>,
    policy: Option<&serde_json::Value>,
    nbf: Option<i64>,
    exp: Option<i64>,
    _proofs: &[Vec<u8>],
) -> Vec<(String, Cbor)> {
    let mut fields = base_fields(audience, subject, command);
    fields.insert(0, (core::FIELD_TAG_ANNOTATION.to_owned(), Cbor::Text(core::DELEGATION_TAG.to_owned())));
    fields.insert(1, (core::FIELD_VERSION.to_owned(), Cbor::Text(core::WIRE_VERSION.to_owned())));
    fields.insert(2, (core::FIELD_ISSUER.to_owned(), Cbor::Text(issuer.to_owned())));
    if let Some(policy) = policy {
        fields.push((core::FIELD_POLICY.to_owned(), crate::profile::v1::cbor_from_json_value(policy)));
    }
    append_times(&mut fields, nbf, exp);
    fields
}

/// Build Invocation payload fields.
#[must_use]
pub fn invocation_payload(
    invoker: &str,
    audience: &str,
    subject: &str,
    command: &str,
    proofs: &[Vec<u8>],
) -> Vec<(String, Cbor)> {
    let mut fields = base_fields(audience, Some(subject), Some(command));
    fields.insert(0, (core::FIELD_TAG_ANNOTATION.to_owned(), Cbor::Text(core::INVOCATION_TAG.to_owned())));
    fields.insert(1, (core::FIELD_VERSION.to_owned(), Cbor::Text(core::WIRE_VERSION.to_owned())));
    fields.insert(2, (core::FIELD_ISSUER.to_owned(), Cbor::Text(invoker.to_owned())));
    fields.push((
        core::FIELD_PROOFS.to_owned(),
        Cbor::Array(proofs.iter().map(|cid| Cbor::Bytes(cid.clone())).collect()),
    ));
    fields
}

fn base_fields(audience: &str, subject: Option<&str>, command: Option<&str>) -> Vec<(String, Cbor)> {
    let mut fields = vec![(core::FIELD_AUDIENCE.to_owned(), Cbor::Text(audience.to_owned()))];
    if let Some(subject) = subject {
        fields.push((core::FIELD_SUBJECT.to_owned(), Cbor::Text(subject.to_owned())));
    }
    if let Some(command) = command {
        fields.push((core::FIELD_COMMAND.to_owned(), Cbor::Text(command.to_owned())));
    }
    fields
}

fn append_times(fields: &mut Vec<(String, Cbor)>, nbf: Option<i64>, exp: Option<i64>) {
    if let Some(nbf) = nbf {
        fields.push((core::FIELD_NOT_BEFORE.to_owned(), Cbor::UInt(u64::try_from(nbf).unwrap_or(0))));
    }
    if let Some(exp) = exp {
        fields.push((core::FIELD_EXPIRES.to_owned(), Cbor::UInt(u64::try_from(exp).unwrap_or(0))));
    }
}
