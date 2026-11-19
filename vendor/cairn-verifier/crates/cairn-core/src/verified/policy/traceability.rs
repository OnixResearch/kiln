const TRACEABILITY_POLICY_FIELD: &str = "traceability_policy";
const TRACEABILITY_DEFAULT_PROFILE_FIELD: &str = "traceability_policy.default_profile";
const TRACEABILITY_PROFILES_FIELD: &str = "traceability_policy.profiles";
const TRACEABILITY_PROFILE_ID_FIELD: &str = "traceability_policy.profiles.id";
const TRACEABILITY_REQUIREMENT_SOURCES_FIELD: &str =
    "traceability_policy.profiles.requirement_sources";
const TRACEABILITY_EVIDENCE_SOURCES_FIELD: &str = "traceability_policy.profiles.evidence_sources";
const TRACEABILITY_MARKER_VERBS_FIELD: &str = "traceability_policy.profiles.marker_verbs";
const TRACEABILITY_ASSURANCE_LEVEL_FIELD: &str = "traceability_policy.profiles.assurance_level";
const TRACEABILITY_ANCHOR_KIND_FIELD: &str = "traceability_policy.profiles.anchor_kind";
const TRACEABILITY_RUST_CONFIGURATION_FIELD: &str =
    "traceability_policy.profiles.rust_configuration_path";
const TRACEABILITY_SOURCE_ROOT_FIELD: &str = "traceability_policy.profiles.sources.root";
const TRACEABILITY_SOURCE_KIND_FIELD: &str = "traceability_policy.profiles.sources.source_kind";
const TRACEABILITY_SOURCE_EXTENSIONS_FIELD: &str =
    "traceability_policy.profiles.sources.extensions";

const TRACEABILITY_REQUIREMENT_SOURCE_KINDS: &[&str] = &["accepted_specs", "requirements_doc"];
const TRACEABILITY_EVIDENCE_SOURCE_KINDS: &[&str] =
    &["rust_source", "nickel_source", "tool_source"];
const TRACEABILITY_MARKER_VERBS: &[&str] = &[
    crate::verified::traceability::TRACEABILITY_VERB_IMPL,
    crate::verified::traceability::TRACEABILITY_VERB_VERIFY,
    crate::verified::traceability::TRACEABILITY_VERB_DEPENDS,
    crate::verified::traceability::TRACEABILITY_VERB_RELATED,
];

pub(super) fn traceability_policy(
    value: &serde_json::Value,
) -> Result<super::TraceabilityPolicy, super::PolicyError> {
    // r[impl cairn.supersede.traceability_ownership.configured_profiles]
    let policy = value
        .get(TRACEABILITY_POLICY_FIELD)
        .ok_or(super::PolicyError::MissingField(TRACEABILITY_POLICY_FIELD))?;
    let default_profile = super::fields::string_field(policy, TRACEABILITY_DEFAULT_PROFILE_FIELD)?;
    let profiles_value = policy
        .get("profiles")
        .and_then(serde_json::Value::as_array)
        .ok_or(super::PolicyError::MissingField(
            TRACEABILITY_PROFILES_FIELD,
        ))?;
    if profiles_value.is_empty() {
        return Err(super::PolicyError::InvalidField(
            TRACEABILITY_PROFILES_FIELD,
        ));
    }

    let mut seen_profile_ids = std::collections::BTreeSet::new();
    let mut profiles = Vec::with_capacity(profiles_value.len());
    for profile_value in profiles_value {
        let profile = parse_profile(profile_value)?;
        if !seen_profile_ids.insert(profile.id.clone()) {
            return Err(super::PolicyError::InvalidField(
                TRACEABILITY_PROFILE_ID_FIELD,
            ));
        }
        profiles.push(profile);
    }

    if default_profile.is_empty() || !profiles.iter().any(|profile| profile.id == default_profile) {
        return Err(super::PolicyError::InvalidField(
            TRACEABILITY_DEFAULT_PROFILE_FIELD,
        ));
    }

    if !profiles.iter().any(|profile| {
        profile.id == crate::verified::traceability::TRACEABILITY_PROFILE_CAIRN_DEFAULT
    }) {
        return Err(super::PolicyError::InvalidField(
            TRACEABILITY_PROFILES_FIELD,
        ));
    }

    Ok(super::TraceabilityPolicy {
        default_profile,
        profiles,
    })
}

fn parse_profile(
    value: &serde_json::Value,
) -> Result<crate::verified::traceability::TraceabilityProfile, super::PolicyError> {
    let id = super::fields::string_field(value, TRACEABILITY_PROFILE_ID_FIELD)?;
    if id.is_empty() {
        return Err(super::PolicyError::InvalidField(
            TRACEABILITY_PROFILE_ID_FIELD,
        ));
    }
    let requirement_sources = parse_sources(
        value,
        "requirement_sources",
        TRACEABILITY_REQUIREMENT_SOURCES_FIELD,
        TRACEABILITY_REQUIREMENT_SOURCE_KINDS,
    )?;
    let evidence_sources = parse_sources(
        value,
        "evidence_sources",
        TRACEABILITY_EVIDENCE_SOURCES_FIELD,
        TRACEABILITY_EVIDENCE_SOURCE_KINDS,
    )?;
    let marker_verbs = parse_marker_verbs(value)?;
    let assurance_level = super::fields::string_field(value, TRACEABILITY_ASSURANCE_LEVEL_FIELD)?;
    let anchor_kind = super::fields::string_field(value, TRACEABILITY_ANCHOR_KIND_FIELD)?;
    if assurance_level
        != crate::verified::content_bound_trace_index::TRACEABILITY_ASSURANCE_DECLARED
    {
        return Err(super::PolicyError::InvalidField(
            TRACEABILITY_ASSURANCE_LEVEL_FIELD,
        ));
    }
    if anchor_kind
        != crate::verified::content_bound_trace_index::TRACEABILITY_ANCHOR_EXACT_MARKER_BYTES
        && anchor_kind
            != crate::verified::content_bound_trace_index::TRACEABILITY_ANCHOR_RUST_ITEM_V1
    {
        return Err(super::PolicyError::InvalidField(
            TRACEABILITY_ANCHOR_KIND_FIELD,
        ));
    }
    let rust_configuration_path = match value.get("rust_configuration_path") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(path))
            if is_safe_traceability_path(path)
                && path.len() <= 4096
                && !path.contains(':')
                && !path.contains('\0') =>
        {
            Some(path.clone())
        }
        _ => {
            return Err(super::PolicyError::InvalidField(
                TRACEABILITY_RUST_CONFIGURATION_FIELD,
            ))
        }
    };
    if (anchor_kind == crate::verified::content_bound_trace_index::TRACEABILITY_ANCHOR_RUST_ITEM_V1)
        != rust_configuration_path.is_some()
        || (rust_configuration_path.is_some()
            && evidence_sources.iter().any(|source| {
                source.source_kind != "rust_source"
                    || source.extensions.iter().any(|extension| extension != "rs")
            }))
    {
        return Err(super::PolicyError::InvalidField(
            TRACEABILITY_RUST_CONFIGURATION_FIELD,
        ));
    }
    Ok(crate::verified::traceability::TraceabilityProfile {
        id,
        requirement_sources,
        evidence_sources,
        marker_verbs,
        assurance_level,
        anchor_kind,
        rust_configuration_path,
    })
}

fn parse_sources(
    value: &serde_json::Value,
    lookup: &str,
    field: &'static str,
    allowed_source_kinds: &[&str],
) -> Result<Vec<crate::verified::traceability::TraceabilitySource>, super::PolicyError> {
    let sources = value
        .get(lookup)
        .and_then(serde_json::Value::as_array)
        .ok_or(super::PolicyError::MissingField(field))?;
    if sources.is_empty() {
        return Err(super::PolicyError::InvalidField(field));
    }
    sources
        .iter()
        .map(|source| parse_source(source, allowed_source_kinds))
        .collect()
}

fn parse_source(
    value: &serde_json::Value,
    allowed_source_kinds: &[&str],
) -> Result<crate::verified::traceability::TraceabilitySource, super::PolicyError> {
    let root = super::fields::string_field(value, TRACEABILITY_SOURCE_ROOT_FIELD)?;
    let source_kind = super::fields::string_field(value, TRACEABILITY_SOURCE_KIND_FIELD)?;
    let extensions = traceability_string_array(value, TRACEABILITY_SOURCE_EXTENSIONS_FIELD)?;
    if !is_safe_traceability_path(&root) {
        return Err(super::PolicyError::InvalidField(
            TRACEABILITY_SOURCE_ROOT_FIELD,
        ));
    }
    if !allowed_source_kinds.contains(&source_kind.as_str()) {
        return Err(super::PolicyError::InvalidField(
            TRACEABILITY_SOURCE_KIND_FIELD,
        ));
    }
    if extensions.is_empty()
        || extensions
            .iter()
            .any(|extension| !is_safe_extension(extension))
    {
        return Err(super::PolicyError::InvalidField(
            TRACEABILITY_SOURCE_EXTENSIONS_FIELD,
        ));
    }
    Ok(crate::verified::traceability::TraceabilitySource {
        root,
        source_kind,
        extensions,
    })
}

fn parse_marker_verbs(value: &serde_json::Value) -> Result<Vec<String>, super::PolicyError> {
    let marker_verbs = traceability_string_array(value, TRACEABILITY_MARKER_VERBS_FIELD)?;
    if marker_verbs.is_empty()
        || marker_verbs
            .iter()
            .any(|verb| !TRACEABILITY_MARKER_VERBS.contains(&verb.as_str()))
    {
        return Err(super::PolicyError::InvalidField(
            TRACEABILITY_MARKER_VERBS_FIELD,
        ));
    }
    let unique = marker_verbs
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    if unique.len() != marker_verbs.len() {
        return Err(super::PolicyError::InvalidField(
            TRACEABILITY_MARKER_VERBS_FIELD,
        ));
    }
    Ok(marker_verbs)
}

fn traceability_string_array(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<Vec<String>, super::PolicyError> {
    let lookup = field.rsplit('.').next().unwrap_or(field);
    value
        .get(lookup)
        .and_then(serde_json::Value::as_array)
        .ok_or(super::PolicyError::MissingField(field))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or(super::PolicyError::InvalidField(field))
        })
        .collect()
}

fn is_safe_traceability_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains('\\')
        && path
            .split('/')
            .all(|component| !matches!(component, "" | "." | ".."))
}

fn is_safe_extension(extension: &str) -> bool {
    !extension.is_empty()
        && !extension.starts_with('.')
        && extension
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}
