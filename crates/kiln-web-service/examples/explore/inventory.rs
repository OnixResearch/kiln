//! Explicit synthetic/disconnected inventory input for the existing exploration shell.
//! This module never discovers providers, reserves capacity or looks up canonical work.
// r[impl kiln.inventory.consumers]
use kiln_core::inventory::{InventoryProjection, WorkNeeds, WorkNeedsAttachment};
use kiln_core::types::ContentId;
use kiln_web_service::engine::ApiResponse;
use kiln_web_service::inventory::InventoryInput;

use super::{ACTOR, Result, error, json};

const EVALUATED_AT_MS: u64 = 2000;
pub const SYNTHETIC_WORK: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SYNTHETIC_POLICY: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

pub fn response(method: &str, target: &str) -> Result<ApiResponse> {
    if method != "GET" {
        return Ok(error(405, "Inventory examples are read-only"));
    }
    let query = target.split_once('?').map(|(_, query)| query).unwrap_or("");
    let selected = if query.is_empty() {
        false
    } else if let Some(work) = query.strip_prefix("work=") {
        if work.len() != 64
            || !work
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Ok(error(
                400,
                "Expected one exact lowercase work BLAKE3 identity",
            ));
        }
        if work != SYNTHETIC_WORK {
            return Ok(error(
                404,
                "Requested work is not in this disconnected example; no canonical work lookup is performed",
            ));
        }
        true
    } else {
        return Ok(error(
            400,
            "Only the exact optional work query is supported",
        ));
    };
    Ok(json(200, serde_json::to_value(projection(selected)?)?))
}

pub(super) fn projection(selected: bool) -> Result<InventoryProjection> {
    let mut input: InventoryInput = serde_json::from_str(include_str!(
        "../../../../fixtures/inventory/synthetic-v2.json"
    ))?;
    input.profile.principal = ACTOR.into();
    input.snapshot.principal = ACTOR.into();
    input.snapshot.synthetic = true;
    // Bind this explicitly fictional requirement to fictional work/policy identities.
    // It cannot be consumed by the real implementation-work attachment route.
    let mut needs: WorkNeeds = serde_json::from_str(include_str!(
        "../../../../fixtures/inventory/synthetic-work-needs-v2.json"
    ))?;
    needs.work_version = SYNTHETIC_WORK.into();
    needs.policy_ref = SYNTHETIC_POLICY.into();
    for policy in &mut input.snapshot.policies {
        policy.work_version = SYNTHETIC_WORK.into();
        policy.policy_ref = SYNTHETIC_POLICY.into();
    }
    for model in &mut input.snapshot.models {
        model.routing_policy_ref = Some(SYNTHETIC_POLICY.into());
    }
    let attachment = if selected {
        Some(WorkNeedsAttachment::new(
            ContentId::from_blake3_hex(SYNTHETIC_WORK)
                .map_err(|_| "Invalid synthetic work identity")?,
            1,
            ContentId::from_blake3_hex(SYNTHETIC_POLICY)
                .map_err(|_| "Invalid synthetic policy identity")?,
            needs,
        )?)
    } else {
        None
    };
    Ok(input.project(ACTOR, attachment.as_ref(), EVALUATED_AT_MS)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn normalized_example_has_distinct_inventories_and_no_live_readiness() {
        let inventory: Value =
            serde_json::from_slice(&response("GET", "/api/inventory").unwrap().body).unwrap();
        assert_eq!(inventory["schema"], "kiln.observe.inventory.v2");
        assert_eq!(inventory["snapshot"]["synthetic"], true);
        assert_eq!(inventory["snapshot"]["agents"].as_array().unwrap().len(), 1);
        assert_eq!(
            inventory["snapshot"]["workers"].as_array().unwrap().len(),
            2
        );
        assert_eq!(inventory["snapshot"]["models"].as_array().unwrap().len(), 1);
        assert_eq!(inventory["work_requirements_unknown"], true);
        let target = format!("/api/inventory?work={SYNTHETIC_WORK}");
        let explained: Value =
            serde_json::from_slice(&response("GET", &target).unwrap().body).unwrap();
        assert_eq!(explained["explanation"]["work_version"], SYNTHETIC_WORK);
        assert_eq!(explained["explanation"]["dispatch_authorized"], false);
        assert!(
            explained["explanation"]["blockers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|blocker| blocker["code"] == "synthetic_disconnected")
        );
        assert!(
            explained["explanation"]["candidates"]
                .as_array()
                .unwrap()
                .iter()
                .all(|candidate| candidate["observed_eligible"] == false)
        );
    }

    #[test]
    fn unknown_malformed_queries_and_mutation_fail_closed() {
        assert_eq!(response("POST", "/api/inventory").unwrap().status, 405);
        assert_eq!(
            response("GET", "/api/inventory?work=invalid")
                .unwrap()
                .status,
            400
        );
        assert_eq!(
            response("GET", &format!("/api/inventory?work={}", "c".repeat(64)))
                .unwrap()
                .status,
            404
        );
        assert_eq!(
            response(
                "GET",
                &format!("/api/inventory?work={SYNTHETIC_WORK}&extra=1")
            )
            .unwrap()
            .status,
            400
        );
    }
}
