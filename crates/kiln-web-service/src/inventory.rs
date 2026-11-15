//! Explicit local snapshot ingestion and read-only API/CLI/offline rendering.
//! No provider discovery, network client, mutation, clock lookup or environment lookup.
// r[impl kiln.inventory.consumers]
#[path = "campaign_projection.rs"]
pub mod campaign_projection;

/// Separate v1 campaign projection; intentionally does not alter inventory-v2 semantics.
pub fn read_campaign_projection(
    path: &Path,
    campaign_id: &str,
) -> Result<Option<campaign_projection::CampaignProjection>, String> {
    campaign_projection::read_campaign(path, campaign_id)
}
pub fn read_campaign_projection_with_inventory(
    path: &Path,
    campaign_id: &str,
    resource: Option<&InventoryProjection>,
) -> Result<Option<campaign_projection::CampaignProjection>, String> {
    campaign_projection::read_campaign_with_inventory(path, campaign_id, resource)
}

pub fn render_campaign_json(
    projection: &campaign_projection::CampaignProjection,
) -> Result<String, String> {
    let json = serde_json::to_string_pretty(projection)
        .map_err(|_| "campaign_projection_encoding_invalid")?;
    if json.len() > campaign_projection::MAX_PROJECTION_BYTES {
        return Err("campaign_projection_byte_bound".into());
    }
    Ok(json)
}

/// Bounded offline rendering of the exact JSON semantic projection; no script, network request, or active control is emitted.
pub fn render_campaign_html(
    projection: &campaign_projection::CampaignProjection,
) -> Result<String, String> {
    let json = render_campaign_json(projection)?;
    let mut html = String::from(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'\"><title>Selected campaigns</title><style>body{font:1rem/1.5 system-ui;max-width:80rem;margin:2rem auto;padding:1rem}pre{white-space:pre-wrap;overflow-wrap:anywhere}section{border:1px solid;padding:1rem;margin-block:1rem}</style></head><body><main><h1>Selected campaign progress and blockers</h1><p>Read-only projection of admitted campaign snapshots; no reservation, dispatch or lifecycle authority.</p>",
    );
    html.push_str(&format!("<section><h2>{}</h2><p>Selection revision {} · identity <code>{}</code></p><pre>{}</pre></section>", escape(&projection.campaign_id), projection.selection_revision, escape(&projection.identity), escape(&json)));
    html.push_str("</main></body></html>\n");
    Ok(html)
}

/// Offline campaign command with an optional explicitly supplied resource snapshot.
/// Database access remains read-only; inventory freshness uses the explicit clock.
pub fn run_campaign_cli_args(args: &[String]) -> Result<String, String> {
    let command_len = if args
        .get(2)
        .is_some_and(|value| matches!(value.as_str(), "json" | "html"))
    {
        3
    } else {
        2
    };
    let (command, resource_args) = args.split_at_checked(command_len)
        .ok_or("usage: campaign-projection DATABASE CAMPAIGN_ID [json|html] [--inventory INPUT PRINCIPAL NOW_MS]")?;
    let resource = match resource_args {
        [] => None,
        [flag, path, principal, now] if flag == "--inventory" => {
            let now = now.parse::<u64>().map_err(|_| "inventory_clock_invalid")?;
            Some(read_inventory(Path::new(path), principal, now)?)
        }
        _ => return Err("usage: campaign-projection DATABASE CAMPAIGN_ID [json|html] [--inventory INPUT PRINCIPAL NOW_MS]".into()),
    };
    run_campaign_cli_args_with_inventory(command, resource.as_ref())
}

/// Same optional, already-admitted resource snapshot composition used by live API handlers.
pub fn run_campaign_cli_args_with_inventory(
    args: &[String],
    resource: Option<&InventoryProjection>,
) -> Result<String, String> {
    let [database, campaign_id, format @ ..] = args else {
        return Err("usage: campaign-projection DATABASE CAMPAIGN_ID [json|html]".into());
    };
    let format = match format {
        [] => "json",
        [value] if ["json", "html"].contains(&value.as_str()) => value.as_str(),
        _ => return Err("campaign_projection_format_unsupported".into()),
    };
    let projection =
        read_campaign_projection_with_inventory(Path::new(database), campaign_id, resource)?
            .ok_or("campaign_projection_not_found")?;
    if format == "html" {
        render_campaign_html(&projection)
    } else {
        render_campaign_json(&projection)
    }
}

use std::fs::File;
use std::io::Read;
use std::path::Path;

use kiln_core::inventory::{
    InventoryProfile, InventoryProjection, InventorySnapshot, WorkNeedsAttachment,
    project_inventory,
};
use kiln_core::types::ContentId;
use kiln_provider_api::inventory::{InventoryReadFailure, InventoryWorkPort};
use serde::{Deserialize, Serialize};

pub const MAX_INVENTORY_INPUT_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_REQUIREMENTS_INPUT_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryInput {
    pub profile: InventoryProfile,
    pub snapshot: InventorySnapshot,
}

fn read_bounded<T: serde::de::DeserializeOwned>(path: &Path, limit: u64) -> Result<T, String> {
    let file = File::open(path).map_err(|_| "inventory_input_unavailable")?;
    if !file
        .metadata()
        .map_err(|_| "inventory_input_unavailable")?
        .is_file()
    {
        return Err("inventory_input_not_regular".into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "inventory_input_unavailable")?;
    if bytes.len() as u64 > limit {
        return Err("inventory_input_byte_bound".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "inventory_input_invalid".into())
}

impl InventoryInput {
    pub fn load(path: &Path) -> Result<Self, String> {
        read_bounded(path, MAX_INVENTORY_INPUT_BYTES)
    }

    pub fn project(
        &self,
        principal: &str,
        needs: Option<&WorkNeedsAttachment>,
        now_ms: u64,
    ) -> Result<InventoryProjection, String> {
        if self.profile.principal != principal {
            return Err("inventory_principal_denied".into());
        }
        if let Some(needs) = needs {
            needs.validate_for(
                &needs.work_identity,
                needs.work_revision,
                &needs.policy_identity,
            )?;
        }
        project_inventory(
            &self.profile,
            &self.snapshot,
            needs.map(|n| &n.needs),
            now_ms,
        )
        .map_err(str::to_owned)
    }
}

/// Authenticated GET handler integration. The path comes from operator configuration, never query data.
/// Exact work requirements are retrieved from current canonical state, not from the observation file.
pub fn read_projection(
    path: &Path,
    principal: &str,
    work: Option<&ContentId>,
    works: &dyn InventoryWorkPort,
    now_ms: u64,
) -> Result<InventoryProjection, String> {
    let attachment = work
        .map(|id| works.current_requirements(id))
        .transpose()
        .map_err(|failure| match failure {
            InventoryReadFailure::Unavailable => "inventory_current_work_unavailable",
            InventoryReadFailure::UnsupportedProfile => "inventory_work_profile_unsupported",
            InventoryReadFailure::PrincipalDenied => "inventory_principal_denied",
            InventoryReadFailure::MalformedObservation => "inventory_work_observation_invalid",
            InventoryReadFailure::StaleWork => "inventory_work_stale",
        })?
        .flatten();
    InventoryInput::load(path)?.project(principal, attachment.as_ref(), now_ms)
}

/// No-work route; absent requirements are explicitly unknown, never inferred from observations.
pub fn read_inventory(
    path: &Path,
    principal: &str,
    now_ms: u64,
) -> Result<InventoryProjection, String> {
    InventoryInput::load(path)?.project(principal, None, now_ms)
}

pub fn load_requirements(
    path: &Path,
    work: &ContentId,
    revision: u64,
    policy: &ContentId,
) -> Result<WorkNeedsAttachment, String> {
    let attachment: WorkNeedsAttachment = read_bounded(path, MAX_REQUIREMENTS_INPUT_BYTES)?;
    attachment.validate_for(work, revision, policy)?;
    Ok(attachment)
}

/// CLI and example share this bounded offline path; the caller owns stdout only.
pub fn run_cli_args(args: &[String]) -> Result<String, String> {
    const USAGE: &str = "usage: inventory INPUT PRINCIPAL NOW_MS [json|html] [ATTACHMENT WORK_ID REVISION POLICY_ID]";
    let [input_path, principal, clock, rest @ ..] = args else {
        return Err(USAGE.into());
    };
    let (format, attachment_args) = match rest {
        [] => ("json", None),
        [format] => (format.as_str(), None),
        [format, path, work, revision, policy] => {
            (format.as_str(), Some((path, work, revision, policy)))
        }
        _ => return Err(USAGE.into()),
    };
    let now = clock
        .parse::<u64>()
        .map_err(|_| "inventory_clock_invalid")?;
    if !["json", "html"].contains(&format) {
        return Err("inventory_format_unsupported".into());
    }
    let attachment = if let Some((path, work, revision, policy)) = attachment_args {
        let work =
            ContentId::from_blake3_hex(work).map_err(|_| "inventory_work_identity_invalid")?;
        let revision = revision
            .parse::<u64>()
            .map_err(|_| "inventory_work_revision_invalid")?;
        let policy =
            ContentId::from_blake3_hex(policy).map_err(|_| "inventory_policy_identity_invalid")?;
        Some(load_requirements(
            Path::new(path),
            &work,
            revision,
            &policy,
        )?)
    } else {
        None
    };
    let input = InventoryInput::load(Path::new(input_path))?;
    let projection = input.project(principal, attachment.as_ref(), now)?;
    if format == "html" {
        render_html(&projection)
    } else {
        render_json(&projection)
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Identical admitted semantic payload for CLI JSON and API; HTML never embeds executable data.
pub fn render_json(projection: &InventoryProjection) -> Result<String, String> {
    serde_json::to_string_pretty(projection)
        .map_err(|_| "inventory_projection_encoding_invalid".into())
}

pub fn render_html(projection: &InventoryProjection) -> Result<String, String> {
    let json = render_json(projection)?;
    // This deliberately contains no scripts, links, fetches, forms or mutable controls.
    let mut html = String::from(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'\"><title>Kiln resource inventories</title><style>body{font:1rem/1.5 system-ui;max-width:80rem;margin:2rem auto;padding:1rem}pre{white-space:pre-wrap;overflow-wrap:anywhere}section{border:1px solid;padding:1rem;margin-block:1rem}summary{min-height:44px}a{min-height:44px}</style></head><body><main><h1>Agents, Workers, Compute, Models / Serving</h1>",
    );
    html.push_str(if projection.snapshot.synthetic { "<p><strong>Synthetic, disconnected example. No live capacity claim.</strong></p>" } else { "<p>Explicitly supplied admitted observations. Read only; provider revalidation required.</p>" });
    html.push_str(&format!(
        "<p>Projection <code>{}</code>; profile <code>{}</code>; evaluated at {} ms.</p>",
        escape(&projection.identity),
        escape(&projection.profile_identity),
        projection.evaluated_at_ms
    ));
    for (title, rows) in [
        (
            "Sources and freshness",
            serde_json::to_value(&projection.sources),
        ),
        ("Agents", serde_json::to_value(&projection.snapshot.agents)),
        (
            "Workers",
            serde_json::to_value(&projection.snapshot.workers),
        ),
        (
            "Compute devices",
            serde_json::to_value(&projection.snapshot.compute),
        ),
        (
            "Shared resource pools (counted once)",
            serde_json::to_value(&projection.snapshot.pools),
        ),
        (
            "Per-fact freshness and unknown state",
            serde_json::to_value(&projection.facts),
        ),
        (
            "Models / Serving",
            serde_json::to_value(&projection.snapshot.models),
        ),
        (
            "Work / attempt bindings",
            serde_json::to_value(&projection.snapshot.attempts),
        ),
        (
            "Attempt binding/revalidation blockers",
            serde_json::to_value(&projection.attempt_explanations),
        ),
        (
            "Authoritative reservations and request admission",
            serde_json::to_value(&projection.snapshot.admissions),
        ),
        (
            "Policy / waiting / budgets",
            serde_json::to_value(&projection.snapshot.policies),
        ),
        (
            "Task readiness (never dispatch authority)",
            serde_json::to_value(&projection.explanation),
        ),
    ] {
        let body = serde_json::to_string_pretty(
            &rows.map_err(|_| "inventory_projection_encoding_invalid")?,
        )
        .map_err(|_| "inventory_projection_encoding_invalid")?;
        html.push_str(&format!(
            "<section><h2>{}</h2><pre>{}</pre></section>",
            title,
            escape(&body)
        ));
    }
    if projection.work_requirements_unknown {
        html.push_str(
            "<p>Exact admitted work requirements are unknown. No readiness inferred.</p>",
        );
    }
    html.push_str("<details><summary>Complete admitted semantic projection</summary><pre>");
    html.push_str(&escape(&json));
    html.push_str("</pre></details></main></body></html>\n");
    Ok(html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_data_is_inert_and_offline() {
        assert_eq!(
            escape("</script><img src=\"https://private\">&"),
            "&lt;/script&gt;&lt;img src=&quot;https://private&quot;&gt;&amp;"
        );
    }

    #[test]
    fn api_json_and_offline_html_retain_identical_projection_identity() {
        let input: InventoryInput = serde_json::from_str(include_str!(
            "../../../fixtures/inventory/synthetic-v2.json"
        ))
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        let projection = input
            .project("operator", None, 2000)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let json = render_json(&projection).unwrap_or_else(|error| unreachable!("{error:?}"));
        let roundtrip: InventoryProjection =
            serde_json::from_str(&json).unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(roundtrip, projection);
        let html = render_html(&projection).unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(html.contains(&projection.identity));
        assert!(html.contains(&projection.profile_identity));
        assert!(html.contains(&escape(&json)));
        for title in [
            "Agents",
            "Workers",
            "Compute devices",
            "Models / Serving",
            "Per-fact freshness",
            "Attempt binding/revalidation",
        ] {
            assert!(html.contains(title));
        }
        assert!(!html.contains("<script"));
        assert!(!html.contains("src=\"http"));
        assert_eq!(
            input
                .project("other-principal", None, 2000)
                .err()
                .unwrap_or_else(|| unreachable!()),
            "inventory_principal_denied"
        );
    }

    #[test]
    fn explicit_file_consumer_enforces_byte_bound_and_principal() {
        let mut file =
            tempfile::NamedTempFile::new().unwrap_or_else(|error| unreachable!("{error:?}"));
        std::io::Write::write_all(
            &mut file,
            include_bytes!("../../../fixtures/inventory/synthetic-v2.json"),
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        let projection = read_inventory(file.path(), "operator", 2000)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(projection.snapshot.synthetic);
        assert_eq!(
            read_inventory(file.path(), "other-principal", 2000)
                .err()
                .unwrap_or_else(|| unreachable!()),
            "inventory_principal_denied"
        );
        assert!(read_bounded::<InventoryInput>(file.path(), 10).is_err());
        let mut args = vec![
            file.path()
                .to_str()
                .unwrap_or_else(|| unreachable!())
                .to_owned(),
            "operator".to_owned(),
            "2000".to_owned(),
            "json".to_owned(),
        ];
        let cli: InventoryProjection = serde_json::from_str(
            &run_cli_args(&args).unwrap_or_else(|error| unreachable!("{error:?}")),
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(cli, projection);
        *args.get_mut(3).unwrap_or_else(|| unreachable!()) = "html".into();
        assert!(
            run_cli_args(&args)
                .unwrap_or_else(|error| unreachable!("{error:?}"))
                .contains(&projection.identity)
        );
        *args.get_mut(3).unwrap_or_else(|| unreachable!()) = "unsupported".into();
        assert_eq!(
            run_cli_args(&args).err().unwrap_or_else(|| unreachable!()),
            "inventory_format_unsupported"
        );
        *args.get_mut(2).unwrap_or_else(|| unreachable!()) = "not-a-clock".into();
        assert_eq!(
            run_cli_args(&args).err().unwrap_or_else(|| unreachable!()),
            "inventory_clock_invalid"
        );
        assert!(run_cli_args(&[]).is_err());
    }

    #[test]
    fn explicit_input_rejects_unknown_fields_and_versions() {
        assert!(serde_json::from_str::<InventoryInput>(r#"{"unexpected":true}"#).is_err());
        let path = Path::new("/a-nonexistent-explicit-kiln-inventory-input");
        assert_eq!(
            InventoryInput::load(path)
                .err()
                .unwrap_or_else(|| unreachable!()),
            "inventory_input_unavailable"
        );
    }

    struct WorkPort(Option<InventoryReadFailure>);
    impl InventoryWorkPort for WorkPort {
        fn current_requirements(
            &self,
            _: &ContentId,
        ) -> Result<Option<WorkNeedsAttachment>, InventoryReadFailure> {
            self.0.clone().map_or(Ok(None), Err)
        }
    }

    #[test]
    fn current_work_failures_remain_distinct_and_missing_attachment_is_unknown() {
        let work = ContentId::from_blake3_hex("a".repeat(64))
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        let mut file =
            tempfile::NamedTempFile::new().unwrap_or_else(|error| unreachable!("{error:?}"));
        std::io::Write::write_all(
            &mut file,
            include_bytes!("../../../fixtures/inventory/synthetic-v2.json"),
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        for (failure, code) in [
            (
                InventoryReadFailure::Unavailable,
                "inventory_current_work_unavailable",
            ),
            (
                InventoryReadFailure::UnsupportedProfile,
                "inventory_work_profile_unsupported",
            ),
            (
                InventoryReadFailure::PrincipalDenied,
                "inventory_principal_denied",
            ),
            (
                InventoryReadFailure::MalformedObservation,
                "inventory_work_observation_invalid",
            ),
            (InventoryReadFailure::StaleWork, "inventory_work_stale"),
        ] {
            assert_eq!(
                read_projection(
                    file.path(),
                    "operator",
                    Some(&work),
                    &WorkPort(Some(failure)),
                    2000
                )
                .err()
                .unwrap_or_else(|| unreachable!()),
                code
            );
        }
        let unknown = read_projection(file.path(), "operator", Some(&work), &WorkPort(None), 2000)
            .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(unknown.work_requirements_unknown);
        assert!(unknown.explanation.is_none());
    }
}
