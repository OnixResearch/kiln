//! Read-only HTTP shell of the Cairn workspace over the `--implementation-work`
//! database: role, method and exact query admission, one bounded snapshot read
//! and the response byte bound. The detail also asks its caller for the
//! resource inventory observation, which fails independently. The service and
//! the exploration workspace share it.
use std::path::Path;

use kiln_core::implementation::campaign::MAX_ID_BYTES;
use kiln_implementer::observation::ImplementationObservation;
use kiln_provider_api::inventory::InventoryReadFailure;

use super::agents::InventoryObservation;
use super::{CairnKey, MALFORMED, detail, index};
use crate::engine::{ApiResponse, Role};

pub const INDEX_ROUTE: &str = "/api/implementation/cairn";
pub const DETAIL_ROUTE: &str = "/api/implementation/cairn/detail";
pub const ROUTES: [&str; 2] = [INDEX_ROUTE, DETAIL_ROUTE];
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const REPOSITORY_HEX_BYTES: usize = blake3::OUT_LEN * 2;
/// Each percent-encoded UTF-8 byte occupies three query bytes (%HH).
const MAX_CHANGE_QUERY_BYTES: usize = MAX_ID_BYTES * 3;

type AdmissionFailure = (u16, &'static str);

/// The index, or the detail of one exact change.
fn admit(method: &str, role: Role, target: &str) -> Result<Option<CairnKey>, AdmissionFailure> {
    if role == Role::Worker {
        return Err((403, "cairn_workspace_forbidden"));
    }
    if method != "GET" {
        return Err((405, "cairn_workspace_read_only"));
    }
    match target.split_once('?') {
        None if target == INDEX_ROUTE => Ok(None),
        Some((DETAIL_ROUTE, query)) => detail_key(query)
            .map(Some)
            .ok_or((400, "cairn_workspace_query_invalid")),
        _ => Err((400, "cairn_workspace_query_invalid")),
    }
}

/// Exactly `repository=HEX&change=NAME`, in that order.
fn detail_key(query: &str) -> Option<CairnKey> {
    let (repository, change) = query.strip_prefix("repository=")?.split_once("&change=")?;
    if repository.len() != REPOSITORY_HEX_BYTES
        || !repository
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || change.len() > MAX_CHANGE_QUERY_BYTES
        || change.contains('&')
    {
        return None;
    }
    let change = crate::engine::api::decoded(change).ok()?;
    let admitted = !change.trim().is_empty()
        && change.len() <= MAX_ID_BYTES
        && !change.contains('/')
        && !change.chars().any(char::is_control);
    admitted.then(|| CairnKey {
        repository_identity: repository.to_owned(),
        change,
    })
}

fn error(status: u16, code: &str) -> ApiResponse {
    ApiResponse {
        status,
        content_type: "application/json",
        body: serde_json::json!({"code": code, "message": "Request could not be admitted"})
            .to_string()
            .into_bytes(),
        download_name: None,
        headers: Vec::new(),
    }
}

fn encode(value: &impl serde::Serialize) -> Result<Vec<u8>, &'static str> {
    serde_json::to_vec(value).map_err(|_| "cairn_workspace_encoding_invalid")
}

// r[impl kiln.cairn.workspace]
// r[impl kiln.cairn.workspace.agents.scenario.source]
/// Serves both routes for an authenticated principal's role over the
/// configured implementation database. `inventory` is called only for an
/// admitted detail request.
pub fn response(
    database: Option<&Path>,
    method: &str,
    target: &str,
    role: Role,
    inventory: &dyn Fn() -> InventoryObservation,
) -> ApiResponse {
    let key = match admit(method, role, target) {
        Ok(key) => key,
        Err((status, code)) => return error(status, code),
    };
    let Some(database) = database else {
        return error(503, "cairn_workspace_input_not_configured");
    };
    let snapshot = match ImplementationObservation::open(database)
        .and_then(|observation| observation.snapshot())
    {
        Ok(snapshot) => snapshot,
        Err(InventoryReadFailure::MalformedObservation) => return error(503, MALFORMED),
        Err(_) => return error(503, "cairn_workspace_unavailable"),
    };
    let body = match key {
        None => index(&snapshot).and_then(|index| encode(&index)),
        Some(key) => match detail(&snapshot, &key, &inventory()) {
            Ok(Some(detail)) => encode(&detail),
            Ok(None) => return error(404, "cairn_change_not_found"),
            Err(code) => Err(code),
        },
    };
    match body {
        Ok(body) if body.len() <= MAX_RESPONSE_BYTES => ApiResponse {
            status: 200,
            content_type: "application/json",
            body,
            download_name: None,
            headers: Vec::new(),
        },
        Ok(_) => error(503, "cairn_workspace_byte_bound"),
        Err(code) => error(503, code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detail_query(query: &str) -> Result<Option<CairnKey>, AdmissionFailure> {
        admit("GET", Role::Viewer, &format!("{DETAIL_ROUTE}?{query}"))
    }

    #[test]
    // r[verify kiln.cairn.workspace]
    fn only_exact_read_requests_are_admitted() {
        let repository = "ab".repeat(32);
        assert_eq!(admit("GET", Role::Operator, INDEX_ROUTE), Ok(None));
        assert_eq!(
            detail_query(&format!("repository={repository}&change=add%20cairn")),
            Ok(Some(CairnKey {
                repository_identity: repository.clone(),
                change: "add cairn".into(),
            }))
        );
        assert_eq!(
            admit("GET", Role::Worker, INDEX_ROUTE),
            Err((403, "cairn_workspace_forbidden"))
        );
        assert_eq!(
            admit("POST", Role::Operator, INDEX_ROUTE),
            Err((405, "cairn_workspace_read_only"))
        );
        let invalid = (400, "cairn_workspace_query_invalid");
        assert_eq!(
            admit("GET", Role::Viewer, &format!("{INDEX_ROUTE}?x=1")),
            Err(invalid)
        );
        assert_eq!(admit("GET", Role::Viewer, DETAIL_ROUTE), Err(invalid));
        for query in [
            format!("change=a&repository={repository}"),
            format!("repository={}&change=a", "AB".repeat(32)),
            format!("repository={}&change=a", "ab".repeat(31)),
            format!("repository={repository}&change="),
            format!("repository={repository}&change=%20"),
            format!("repository={repository}&change=a%2Fb"),
            format!("repository={repository}&change=a%0Ab"),
            format!("repository={repository}&change=a&extra=1"),
            format!("repository={repository}&change=%zz"),
            format!(
                "repository={repository}&change={}",
                "a".repeat(MAX_ID_BYTES + 1)
            ),
        ] {
            assert_eq!(detail_query(&query), Err(invalid), "{query}");
        }
    }

    #[test]
    // r[verify kiln.cairn.workspace]
    fn reads_fail_closed_without_creating_a_store() {
        let root = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let absent = root.path().join("absent.sqlite");
        let status = |database: Option<&Path>| {
            let response = response(database, "GET", INDEX_ROUTE, Role::Viewer, &|| {
                unreachable!("the index reads no inventory")
            });
            (
                response.status,
                serde_json::from_slice::<serde_json::Value>(&response.body)
                    .ok()
                    .and_then(|body| {
                        body.get("code")
                            .and_then(|code| code.as_str())
                            .map(str::to_owned)
                    }),
            )
        };
        assert_eq!(
            status(None),
            (503, Some("cairn_workspace_input_not_configured".into()))
        );
        assert_eq!(
            status(Some(&absent)),
            (503, Some("cairn_workspace_unavailable".into()))
        );
        assert!(!absent.exists());
    }
}
