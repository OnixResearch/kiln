//! Authenticated read-only projections over explicitly supplied observations.
use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use kiln_core::implementation::campaign::{
    CampaignOperation, CampaignState, DecisionRequestState, EffectState,
};
use kiln_core::implementation::continuation::ContinuationRecord;
use kiln_core::types::ContentId;
use kiln_implementer::observation::ImplementationObservation;
use rusqlite::{Connection, OpenFlags, params};

use super::{ApiResponse, Context, Principal, Request, error_response};
use crate::cairn_workspace::agents::InventoryObservation;
use crate::engine::Role;

const MAXIMUM_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const CONTENT_ID_HEX_BYTES: usize = blake3::OUT_LEN * 2;
const CAMPAIGN_INDEX_SCHEMA: &str = "kiln.implementation-campaigns.v1";
const MAX_CAMPAIGN_INDEX_SCAN: usize = 1_000;
const MAX_CAMPAIGN_INDEX_ENTRIES: usize = 200;
const MAX_CAMPAIGN_ROW_BYTES: i64 = 32 * 1024 * 1024;
const MAX_CONTINUATION_ROW_BYTES: i64 = 64 * 1024;
const CAMPAIGN_BUSY_TIMEOUT_MS: u64 = 10_000;

type AdmissionFailure = (u16, &'static str);

fn admit_request(
    method: &str,
    role: Role,
    target: &str,
) -> Result<Option<ContentId>, AdmissionFailure> {
    if role == Role::Worker {
        return Err((403, "inventory_forbidden"));
    }
    if method != "GET" {
        return Err((405, "inventory_read_only"));
    }
    let Some((_, query)) = target.split_once('?') else {
        return Ok(None);
    };
    let value = query
        .strip_prefix("work=")
        .filter(|value| {
            value.len() == CONTENT_ID_HEX_BYTES
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .ok_or((400, "inventory_work_query_invalid"))?;
    ContentId::from_blake3_hex(value)
        .map(Some)
        .map_err(|_| (400, "inventory_work_query_invalid"))
}

fn admit_campaign_request(
    method: &str,
    role: Role,
    target: &str,
) -> Result<Option<String>, AdmissionFailure> {
    if role == Role::Worker {
        return Err((403, "inventory_forbidden"));
    }
    if method != "GET" {
        return Err((405, "inventory_read_only"));
    }
    let Some((_, query)) = target.split_once('?') else {
        return Ok(None);
    };
    // Each percent-encoded UTF-8 byte occupies three query bytes (%HH).
    let value = query
        .strip_prefix("campaign=")
        .filter(|value| {
            value.len() <= kiln_core::implementation::campaign::MAX_ID_BYTES * 3
                && !value.contains('&')
        })
        .ok_or((400, "campaign_projection_query_invalid"))?;
    let identity = crate::engine::api::decoded(value)
        .map_err(|_| (400, "campaign_projection_query_invalid"))?;
    if identity.trim().is_empty()
        || identity.len() > kiln_core::implementation::campaign::MAX_ID_BYTES
        || identity.chars().any(char::is_control)
    {
        return Err((400, "campaign_projection_query_invalid"));
    }
    Ok(Some(identity))
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
struct CampaignSummary {
    campaign_id: String,
    status: &'static str,
    store_version: u64,
    revision: u64,
    repository: String,
    repository_identity: String,
    member_count: usize,
    pending_decisions: usize,
    unissued_decisions: usize,
    pending_effects: usize,
    paused: bool,
    cancelled: bool,
    complete: bool,
    updated_at: Option<u64>,
}

#[derive(Debug, serde::Serialize)]
struct CampaignIndex {
    schema: &'static str,
    campaigns: Vec<CampaignSummary>,
    bounded: bool,
    truncated: bool,
}

fn campaign_status(state: &CampaignState, complete: bool) -> &'static str {
    if state.cancelled {
        "cancelled"
    } else if complete {
        "complete"
    } else if state.paused {
        "paused"
    } else if state.pending_effects.values().any(|effect| {
        !effect.progress_applied
            && matches!(effect.state, EffectState::IntentPersisted | EffectState::Unknown)
    }) {
        "reconciling"
    } else {
        match state.evaluate().decision.operation {
            CampaignOperation::Blocked => "blocked",
            CampaignOperation::Wait => "waiting",
            _ => "active",
        }
    }
}

fn summarize_campaign(
    state: &CampaignState,
    store_version: u64,
    last_batch_ms: Option<u64>,
) -> CampaignSummary {
    let complete = state.complete();
    let pending_decisions = state
        .decisions
        .values()
        .filter(|decision| decision.state == DecisionRequestState::Outstanding)
        .count();
    let unissued_decisions = state
        .decisions
        .values()
        .filter(|decision| decision.state == DecisionRequestState::Unissued)
        .count();
    let pending_effects = state
        .pending_effects
        .values()
        .filter(|effect| !effect.progress_applied)
        .count();
    let updated_at = last_batch_ms
        .into_iter()
        .chain(state.decisions.values().map(|decision| decision.created_ms))
        .max();
    CampaignSummary {
        campaign_id: state.contract.campaign_id.clone(),
        status: campaign_status(state, complete),
        store_version,
        revision: state.contract.revision,
        repository: state.contract.repository.clone(),
        repository_identity: crate::inventory::campaign_projection::repository_identity(
            &state.contract.repository,
        ),
        member_count: state.contract.members.len(),
        pending_decisions,
        unissued_decisions,
        pending_effects,
        paused: state.paused,
        cancelled: state.cancelled,
        complete,
        updated_at,
    }
}

fn build_campaign_index(mut campaigns: Vec<CampaignSummary>, overflow: bool) -> CampaignIndex {
    campaigns.sort_by(|left, right| {
        let left_done = u8::from(left.complete || left.cancelled);
        let right_done = u8::from(right.complete || right.cancelled);
        left_done
            .cmp(&right_done)
            .then_with(|| Reverse(left.updated_at).cmp(&Reverse(right.updated_at)))
            .then_with(|| left.campaign_id.cmp(&right.campaign_id))
    });
    let truncated = overflow || campaigns.len() > MAX_CAMPAIGN_INDEX_ENTRIES;
    campaigns.truncate(MAX_CAMPAIGN_INDEX_ENTRIES);
    CampaignIndex {
        schema: CAMPAIGN_INDEX_SCHEMA,
        campaigns,
        bounded: true,
        truncated,
    }
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, &'static str> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            params![table],
            |row| row.get::<_, i64>(0),
        )
        .map(|exists| exists != 0)
        .map_err(|_| "campaign_projection_unavailable")
}

fn read_continuation_times(
    connection: &Connection,
) -> Result<BTreeMap<String, Option<u64>>, &'static str> {
    if !table_exists(connection, "implementation_campaign_continuation")? {
        return Ok(BTreeMap::new());
    }
    let mut statement = connection
        .prepare(
            "SELECT campaign_id, CASE WHEN length(bytes) <= ?1 THEN bytes ELSE NULL END \
             FROM implementation_campaign_continuation ORDER BY campaign_id LIMIT ?2",
        )
        .map_err(|_| "campaign_projection_unavailable")?;
    let rows = statement
        .query_map(
            params![
                MAX_CONTINUATION_ROW_BYTES,
                i64::try_from(MAX_CAMPAIGN_INDEX_SCAN + 1).unwrap_or(i64::MAX)
            ],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<Vec<u8>>>(1)?)),
        )
        .map_err(|_| "campaign_projection_unavailable")?;
    let mut times = BTreeMap::new();
    for row in rows {
        let (campaign_id, bytes) = row.map_err(|_| "campaign_projection_unavailable")?;
        let Some(bytes) = bytes else {
            return Err("campaign_projection_record_invalid");
        };
        let record: ContinuationRecord =
            serde_json::from_slice(&bytes).map_err(|_| "campaign_projection_record_invalid")?;
        if record.campaign_id != campaign_id {
            return Err("campaign_projection_record_invalid");
        }
        times.insert(campaign_id, record.last_batch_ms);
    }
    Ok(times)
}

fn read_campaign_index(database: &Path) -> Result<CampaignIndex, &'static str> {
    let connection = Connection::open_with_flags(
        database,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| "campaign_projection_unavailable")?;
    connection
        .busy_timeout(Duration::from_millis(CAMPAIGN_BUSY_TIMEOUT_MS))
        .map_err(|_| "campaign_projection_unavailable")?;
    connection
        .pragma_update(None, "query_only", true)
        .map_err(|_| "campaign_projection_unavailable")?;
    if !table_exists(&connection, "implementation_campaign")? {
        return Ok(CampaignIndex {
            schema: CAMPAIGN_INDEX_SCHEMA,
            campaigns: Vec::new(),
            bounded: true,
            truncated: false,
        });
    }
    let continuation_times = read_continuation_times(&connection)?;
    let scan_limit = i64::try_from(MAX_CAMPAIGN_INDEX_SCAN + 1).unwrap_or(i64::MAX);
    let mut statement = connection
        .prepare(
            "SELECT campaign_id, version, length(bytes), \
             CASE WHEN length(bytes) <= ?1 THEN bytes ELSE NULL END \
             FROM implementation_campaign ORDER BY campaign_id LIMIT ?2",
        )
        .map_err(|_| "campaign_projection_unavailable")?;
    let rows = statement
        .query_map(params![MAX_CAMPAIGN_ROW_BYTES, scan_limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<Vec<u8>>>(3)?,
            ))
        })
        .map_err(|_| "campaign_projection_unavailable")?;
    let mut summaries = Vec::new();
    let mut overflow = false;
    for row in rows {
        if summaries.len() >= MAX_CAMPAIGN_INDEX_SCAN {
            overflow = true;
            break;
        }
        let (campaign_id, version, length, bytes) =
            row.map_err(|_| "campaign_projection_unavailable")?;
        if !(0..=MAX_CAMPAIGN_ROW_BYTES).contains(&length) {
            return Err("campaign_projection_record_invalid");
        }
        let Some(bytes) = bytes else {
            return Err("campaign_projection_record_invalid");
        };
        let store_version =
            u64::try_from(version).map_err(|_| "campaign_projection_record_invalid")?;
        let state: CampaignState =
            serde_json::from_slice(&bytes).map_err(|_| "campaign_projection_record_invalid")?;
        if state.contract.campaign_id != campaign_id {
            return Err("campaign_projection_record_invalid");
        }
        let last_batch_ms = continuation_times.get(&campaign_id).copied().flatten();
        summaries.push(summarize_campaign(&state, store_version, last_batch_ms));
    }
    Ok(build_campaign_index(summaries, overflow))
}

// r[impl kiln.dashboard.campaign.projection]
// r[impl kiln.implementation.campaign.index_route]
pub(super) fn campaign_response(
    request: &Request,
    context: &Context<'_>,
    principal: &Principal,
) -> ApiResponse {
    let identity = match admit_campaign_request(&request.method, principal.role, &request.target) {
        Ok(identity) => identity,
        Err((status, code)) => return error_response(status, code),
    };
    let Some(database) = context.implementation_work else {
        return error_response(503, "campaign_projection_input_not_configured");
    };
    let Some(identity) = identity else {
        let index = match read_campaign_index(database) {
            Ok(index) => index,
            Err(code) => return error_response(503, code),
        };
        return match serde_json::to_vec(&index) {
            Ok(body) if body.len() <= MAXIMUM_RESPONSE_BYTES => ApiResponse {
                status: 200,
                content_type: "application/json",
                body,
                download_name: None,
                headers: Vec::new(),
            },
            _ => error_response(503, "campaign_projection_byte_bound"),
        };
    };
    let projection = match crate::inventory::read_campaign_projection(database, &identity) {
        Ok(Some(projection)) => projection,
        Ok(None) => return error_response(404, "campaign_projection_not_found"),
        Err(code) => return error_response(503, &code),
    };
    // Resource observations have an independent authority/failure boundary.
    let inventory = context.inventory.and_then(|path| {
        crate::engine::host_time_ms()
            .ok()
            .and_then(|now| crate::inventory::read_inventory(path, &principal.id, now).ok())
    });
    let projection = match crate::inventory::campaign_projection::compose_resource_inventory(
        projection,
        inventory.as_ref(),
    ) {
        Ok(projection) => projection,
        Err(code) => return error_response(503, &code),
    };
    match serde_json::to_vec(&projection) {
        Ok(body) if body.len() <= MAXIMUM_RESPONSE_BYTES => ApiResponse {
            status: 200,
            content_type: "application/json",
            body,
            download_name: None,
            headers: Vec::new(),
        },
        _ => error_response(503, "campaign_projection_byte_bound"),
    }
}

// r[impl kiln.cairn.workspace.agents.scenario.source]
/// The resource inventory beside a Cairn change detail, for the requesting
/// principal and the host clock. Its failure never fails the detail.
pub(super) fn cairn_observation(
    context: &Context<'_>,
    principal: &Principal,
) -> InventoryObservation {
    let Some(path) = context.inventory else {
        return InventoryObservation::NotConfigured;
    };
    let Ok(now) = crate::engine::host_time_ms() else {
        return InventoryObservation::Unavailable("clock_unavailable".into());
    };
    match crate::inventory::read_inventory(path, &principal.id, now) {
        Ok(projection) => InventoryObservation::Admitted(Box::new(projection)),
        Err(code) => InventoryObservation::Unavailable(code),
    }
}

// r[impl kiln.inventory.consumers]
pub(super) fn response(
    request: &Request,
    context: &Context<'_>,
    principal: &Principal,
) -> ApiResponse {
    let work = match admit_request(&request.method, principal.role, &request.target) {
        Ok(work) => work,
        Err((status, code)) => return error_response(status, code),
    };
    let Some(path) = context.inventory else {
        return error_response(503, "inventory_input_not_configured");
    };
    let now = match crate::engine::host_time_ms() {
        Ok(now) => now,
        Err(_) => return error_response(503, "clock_unavailable"),
    };
    let projection = if let Some(work) = &work {
        let Some(database) = context.implementation_work else {
            return error_response(503, "inventory_work_source_not_configured");
        };
        let observation = match ImplementationObservation::open(database) {
            Ok(observation) => observation,
            Err(_) => return error_response(503, "inventory_current_work_unavailable"),
        };
        crate::inventory::read_projection(path, &principal.id, Some(work), &observation, now)
    } else {
        crate::inventory::read_inventory(path, &principal.id, now)
    };
    let projection = match projection {
        Ok(projection) => projection,
        Err(code) => {
            let status = match code.as_str() {
                "inventory_principal_denied" => 403,
                "inventory_work_stale" => 409,
                _ => 503,
            };
            return error_response(status, &code);
        }
    };
    match serde_json::to_vec(&projection) {
        Ok(body) if body.len() <= MAXIMUM_RESPONSE_BYTES => ApiResponse {
            status: 200,
            content_type: "application/json",
            body,
            download_name: None,
            headers: Vec::new(),
        },
        _ => error_response(503, "inventory_projection_byte_bound"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_and_mutating_requests_cannot_reach_observation_io() {
        assert!(matches!(
            admit_request("GET", Role::Worker, "/api/inventory"),
            Err((403, "inventory_forbidden"))
        ));
        assert!(matches!(
            admit_request("POST", Role::Operator, "/api/inventory"),
            Err((405, "inventory_read_only"))
        ));
        assert!(matches!(
            admit_request("GET", Role::Viewer, "/api/inventory"),
            Ok(None)
        ));
    }

    #[test]
    fn exact_work_queries_reject_duplicate_unknown_and_encoded_fields() {
        let identity = "a".repeat(64);
        assert!(matches!(
            admit_request("GET", Role::Viewer, &format!("/api/inventory?work={identity}")),
            Ok(Some(work)) if work.as_str() == identity
        ));
        for query in [
            "work=short".to_owned(),
            format!("work={identity}&work={identity}"),
            format!("work={identity}&unknown=1"),
            format!("unknown={identity}"),
            format!("work=%61{}", "a".repeat(63)),
            "".to_owned(),
        ] {
            assert!(matches!(
                admit_request("GET", Role::Viewer, &format!("/api/inventory?{query}")),
                Err((400, "inventory_work_query_invalid"))
            ));
        }
    }

    #[test]
    fn campaign_lookup_preserves_opaque_ids_and_rejects_crossed_queries() {
        assert_eq!(
            admit_campaign_request("GET", Role::Viewer, "/api/implementation/campaigns"),
            Ok(None)
        );
        let target = "/api/implementation/campaigns?campaign=group%20%CE%B1%2Bbeta%2F..%3Fx";
        assert_eq!(
            admit_campaign_request("GET", Role::Viewer, target),
            Ok(Some("group α+beta/..?x".into()))
        );
        for (value, expected) in [
            (".", "."),
            ("..", ".."),
            ("a%26campaign%3Db", "a&campaign=b"),
        ] {
            assert_eq!(
                admit_campaign_request(
                    "GET",
                    Role::Viewer,
                    &format!("/api/implementation/campaigns?campaign={value}")
                ),
                Ok(Some(expected.to_owned()))
            );
        }
        assert_eq!(
            admit_campaign_request("GET", Role::Worker, target),
            Err((403, "inventory_forbidden"))
        );
        assert_eq!(
            admit_campaign_request("GET", Role::Worker, "/api/implementation/campaigns"),
            Err((403, "inventory_forbidden"))
        );
        assert_eq!(
            admit_campaign_request("POST", Role::Operator, "/api/implementation/campaigns"),
            Err((405, "inventory_read_only"))
        );
        assert_eq!(
            admit_campaign_request("POST", Role::Operator, target),
            Err((405, "inventory_read_only"))
        );
        for value in [
            "".to_owned(),
            "%00".to_owned(),
            "%FF".to_owned(),
            "%".to_owned(),
            "x&campaign=y".to_owned(),
            "x&unknown=y".to_owned(),
            "x".repeat(kiln_core::implementation::campaign::MAX_ID_BYTES + 1),
        ] {
            assert_eq!(
                admit_campaign_request(
                    "GET",
                    Role::Viewer,
                    &format!("/api/implementation/campaigns?campaign={value}")
                ),
                Err((400, "campaign_projection_query_invalid"))
            );
        }
        assert_eq!(
            admit_campaign_request("GET", Role::Viewer, "/api/implementation/campaigns?unknown=1"),
            Err((400, "campaign_projection_query_invalid"))
        );
    }

    #[test]
    fn campaign_index_orders_active_campaigns_and_handles_empty_or_corrupt_stores()
    -> Result<(), Box<dyn std::error::Error>> {
        use kiln_core::implementation::campaign::{
            CAMPAIGN_SCHEMA, CampaignContract, CompletionGoal, DecisionKind, HumanDecisionRequest,
            MemberBinding, MemberSelector, WorkflowProfile,
        };
        use kiln_core::types::ContentId;
        use kiln_implementer::campaign::CampaignStore;

        let dir = tempfile::tempdir()?;
        let empty_db = dir.path().join("empty.sqlite");
        let conn = Connection::open(&empty_db)?;
        drop(conn);
        let empty = read_campaign_index(&empty_db).map_err(std::io::Error::other)?;
        assert_eq!(empty.schema, CAMPAIGN_INDEX_SCHEMA);
        assert!(empty.campaigns.is_empty());
        assert!(!empty.truncated);

        let db_path = dir.path().join("campaigns.sqlite");
        let mut store = CampaignStore::open(&db_path).map_err(std::io::Error::other)?;
        let empty_table = read_campaign_index(&db_path).map_err(std::io::Error::other)?;
        assert!(empty_table.campaigns.is_empty());

        let hash = ContentId::from_blake3_hex("a".repeat(64)).map_err(|_| "invalid hash")?;
        let make_contract = |id: &str| CampaignContract {
            schema: CAMPAIGN_SCHEMA.to_owned(),
            campaign_id: id.to_owned(),
            revision: 1,
            repository: "/repo".to_owned(),
            members: vec![MemberBinding {
                selector: MemberSelector::WholeChange {
                    change: "chg-1".to_owned(),
                },
                prerequisite_inclusion: false,
                profile: WorkflowProfile::SpecDriven,
                profile_version: hash.clone(),
                policy: hash.clone(),
                repository_instructions: hash.clone(),
                integration_policy: hash.clone(),
                goal: CompletionGoal::VerifiedCandidate,
                required_checks: Vec::new(),
                required_reviews: Vec::new(),
                context: Vec::new(),
            }],
            max_concurrency: 1,
            max_outstanding_decisions: 4,
            parent_budget_id: hash.clone(),
            agent_definition: hash.clone(),
            require_one_writer_per_repository: true,
        };
        store.start(make_contract("camp-done")).map_err(std::io::Error::other)?;
        store.cancel("camp-done", 1).map_err(std::io::Error::other)?;
        store.start(make_contract("camp-older")).map_err(std::io::Error::other)?;
        store.start(make_contract("camp-newer")).map_err(std::io::Error::other)?;
        let mut newer_state = store.inspect("camp-newer").map_err(std::io::Error::other)?.ok_or("missing")?;
        newer_state.decisions.insert(
            hash.clone(),
            HumanDecisionRequest {
                identity: hash.clone(),
                kind: DecisionKind::Preference,
                member: "change:chg-1".to_owned(),
                work_identity: None,
                decision_revision: None,
                policy_identity: hash.clone(),
                candidate_identity: None,
                state: DecisionRequestState::Outstanding,
                created_ms: 9_000,
                question: "Proceed?".to_owned(),
                options: vec!["yes".to_owned()],
                recommendation: None,
                priority: 1,
                pending_reason: None,
                resolved_answer: None,
                active_effort_ms: None,
            },
        );
        let newer_bytes = serde_json::to_vec(&newer_state)?;
        let write_conn = Connection::open(&db_path)?;
        write_conn.execute(
            "UPDATE implementation_campaign SET version = 2, bytes = ?1 WHERE campaign_id = 'camp-newer'",
            params![newer_bytes],
        )?;
        let index = read_campaign_index(&db_path).map_err(std::io::Error::other)?;
        assert_eq!(index.campaigns.len(), 3);
        assert_eq!(index.campaigns[0].campaign_id, "camp-newer");
        assert_eq!(index.campaigns[0].pending_decisions, 1);
        assert_eq!(index.campaigns[0].updated_at, Some(9_000));
        assert_eq!(index.campaigns[0].status, "waiting");
        assert_eq!(index.campaigns[1].campaign_id, "camp-older");
        assert_eq!(index.campaigns[2].campaign_id, "camp-done");
        assert_eq!(index.campaigns[2].status, "cancelled");

        write_conn.execute(
            "UPDATE implementation_campaign SET bytes = ?1 WHERE campaign_id = 'camp-older'",
            params![b"{bad json".to_vec()],
        )?;
        assert_eq!(read_campaign_index(&db_path).err(), Some("campaign_projection_record_invalid"));
        Ok(())
    }
}
