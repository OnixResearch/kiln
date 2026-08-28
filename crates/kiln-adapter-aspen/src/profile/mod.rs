use std::collections::BTreeSet;

use kiln_core::types::{ContentId, CoreResult, Diagnostic};

mod fixture;
mod model;

pub use fixture::*;
pub use model::*;

// r[impl kiln.aspen_runtime.profile]
pub fn admit_profile(profile: &AspenRuntimeProfile) -> CoreResult<AdmittedAspenRuntimeProfile> {
    require_source_cohort(profile)?;
    require_host_cohort(profile)?;
    require_non_empty_refs(profile)?;
    require_bounds(&profile.bounds)?;
    require_ports(&profile.ports)?;
    require_non_claims(&profile.non_claims)?;

    let bytes = serde_json::to_vec(profile).map_err(|_| {
        Diagnostic::new(
            "aspen_profile_encoding",
            "aspen.profile",
            "Aspen profile identity encoding failed",
        )
    })?;
    let profile_identity = ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string())?;
    Ok(AdmittedAspenRuntimeProfile {
        profile: profile.clone(),
        profile_identity,
    })
}

fn require_source_cohort(profile: &AspenRuntimeProfile) -> CoreResult<()> {
    require_equal(
        "profile.schema",
        &profile.schema,
        ASPEN_RUNTIME_PROFILE_SCHEMA,
    )?;
    require_equal(
        "profile.profile_id",
        &profile.profile_id,
        ASPEN_RUNTIME_PROFILE_ID,
    )?;
    require_equal("profile.source", &profile.source, ASPEN_SOURCE)?;
    require_equal(
        "profile.source_revision",
        &profile.source_revision,
        ASPEN_REVISION,
    )?;
    require_equal(
        "profile.bounded_exec_source",
        &profile.bounded_exec_source,
        BOUNDED_EXEC_SOURCE,
    )?;
    require_equal(
        "profile.bounded_exec_revision",
        &profile.bounded_exec_revision,
        BOUNDED_EXEC_REVISION,
    )
}

fn require_host_cohort(profile: &AspenRuntimeProfile) -> CoreResult<()> {
    for (field, actual, expected) in [
        (
            "profile.native_host_schema",
            profile.native_host_schema.as_str(),
            ASPEN_NATIVE_HOST_PROFILE_SCHEMA,
        ),
        (
            "profile.native_callback_envelope_schema",
            profile.native_callback_envelope_schema.as_str(),
            ASPEN_NATIVE_CALLBACK_ENVELOPE_SCHEMA,
        ),
        (
            "profile.native_callback_outcome_schema",
            profile.native_callback_outcome_schema.as_str(),
            ASPEN_NATIVE_CALLBACK_OUTCOME_SCHEMA,
        ),
        (
            "profile.native_instance_schema",
            profile.native_instance_schema.as_str(),
            ASPEN_NATIVE_INSTANCE_SCHEMA,
        ),
        (
            "profile.native_ingress_schema",
            profile.native_ingress_schema.as_str(),
            ASPEN_NATIVE_INGRESS_SCHEMA,
        ),
        (
            "profile.extension_protocol_schema",
            profile.extension_protocol_schema.as_str(),
            KILN_EXTENSION_PROTOCOL_SCHEMA,
        ),
        (
            "profile.extension_id",
            profile.extension_id.as_str(),
            KILN_EXTENSION_ID,
        ),
        (
            "profile.service_id",
            profile.service_id.as_str(),
            KILN_EXTENSION_SERVICE_ID,
        ),
        (
            "profile.execution_profile",
            profile.execution_profile.as_str(),
            KILN_NATIVE_EXECUTION_PROFILE,
        ),
        (
            "profile.workflow_provider_profile",
            profile.workflow_provider_profile.as_str(),
            KILN_WORKFLOW_PROVIDER_PROFILE,
        ),
        ("profile.alpn", profile.alpn.as_str(), ASPEN_NATIVE_ALPN),
        (
            "profile.framing",
            profile.framing.as_str(),
            ASPEN_NATIVE_FRAMING,
        ),
        (
            "profile.claim_level",
            profile.claim_level.as_str(),
            LOCAL_CLAIM_LEVEL,
        ),
    ] {
        require_equal(field, actual, expected)?;
    }
    Ok(())
}

fn require_equal(field: &'static str, actual: &str, expected: &str) -> CoreResult<()> {
    if actual != expected {
        return Err(Diagnostic::new(
            "aspen_profile_drift",
            field,
            "Aspen runtime profile does not match the reviewed cohort",
        ));
    }
    Ok(())
}

fn require_non_empty_refs(profile: &AspenRuntimeProfile) -> CoreResult<()> {
    if profile.generation == 0 {
        return Err(Diagnostic::new(
            "aspen_generation_zero",
            "profile.generation",
            "Aspen service generation must be nonzero",
        ));
    }
    for (field, value) in [
        ("profile.endpoint_ref", profile.endpoint_ref.as_str()),
        (
            "profile.transport_profile_ref",
            profile.transport_profile_ref.as_str(),
        ),
        ("profile.policy_ref", profile.policy_ref.as_str()),
        ("profile.authority_ref", profile.authority_ref.as_str()),
        ("profile.resource_ref", profile.resource_ref.as_str()),
    ] {
        if value.is_empty() {
            return Err(Diagnostic::new(
                "aspen_profile_missing_ref",
                field,
                "Aspen runtime profile requires every authority and runtime reference",
            ));
        }
    }
    Ok(())
}

fn require_bounds(bounds: &AspenRuntimeBounds) -> CoreResult<()> {
    let values = [
        bounds.max_callback_input_bytes,
        bounds.max_callback_output_bytes,
        bounds.max_materialized_value_bytes,
        bounds.max_materialized_values,
        bounds.max_content_bytes,
        bounds.max_log_bytes,
        bounds.max_effects,
        bounds.max_records,
    ];
    if values.contains(&0) || bounds.callback_timeout_millis == 0 {
        return Err(Diagnostic::new(
            "aspen_profile_zero_bound",
            "profile.bounds",
            "Aspen runtime profile bounds must be nonzero",
        ));
    }
    if bounds.max_materialized_value_bytes > bounds.max_callback_input_bytes
        || bounds.max_materialized_value_bytes > bounds.max_callback_output_bytes
    {
        return Err(Diagnostic::new(
            "aspen_profile_value_bound",
            "profile.bounds.max_materialized_value_bytes",
            "Aspen value bound must fit both callback frame bounds",
        ));
    }
    if bounds.max_log_bytes > bounds.max_content_bytes {
        return Err(Diagnostic::new(
            "aspen_profile_log_bound",
            "profile.bounds.max_log_bytes",
            "Aspen log bound must not exceed the content bound",
        ));
    }
    Ok(())
}

fn require_ports(ports: &[AspenPortBinding]) -> CoreResult<()> {
    let expected = expected_ports();
    let actual = ports.iter().cloned().collect::<BTreeSet<_>>();
    if ports.len() != REQUIRED_PORT_COUNT
        || actual.len() != REQUIRED_PORT_COUNT
        || actual != expected
    {
        return Err(Diagnostic::new(
            "aspen_port_cohort_drift",
            "profile.ports",
            "Aspen runtime port cohort must match every reviewed exact binding",
        ));
    }
    Ok(())
}

// r[impl kiln.aspen_runtime.nonclaims]
fn require_non_claims(non_claims: &[String]) -> CoreResult<()> {
    let expected = REQUIRED_NON_CLAIMS
        .iter()
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>();
    let actual = non_claims.iter().cloned().collect::<BTreeSet<_>>();
    if non_claims.len() != REQUIRED_NON_CLAIM_COUNT || actual != expected {
        return Err(Diagnostic::new(
            "aspen_non_claim_drift",
            "profile.non_claims",
            "Aspen runtime profile must retain every required non-claim exactly once",
        ));
    }
    Ok(())
}
