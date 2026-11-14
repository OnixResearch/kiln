//! Authenticated read-only projections over explicitly supplied observations.
use kiln_core::types::ContentId;
use kiln_implementer::observation::ImplementationObservation;

use super::{ApiResponse, Context, Principal, Request, error_response};
use crate::engine::Role;

const MAXIMUM_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const CONTENT_ID_HEX_BYTES: usize = blake3::OUT_LEN * 2;

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
}
