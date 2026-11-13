//! Shared synthetic Patchgate document for retained receipts and run reports.
use super::{Result, now_ms};
use serde_json::{Value, json};

pub fn document(decision: &str, index: usize) -> Result<Value> {
    let hash = blake3::hash(decision.as_bytes()).to_hex().to_string();
    let native = json!({
        "synthetic":true,"dry_run":true,"authorizes_acceptance":false,
        "main":{"channel":"example-main","state":"retained-example"},
        "inbox":{"channel":"example-inbox","state":"retained-example"},
        "items":[{"hash":hash,"message":format!("Synthetic {decision}: explain release identity"),"paths":["src/lib.rs","config/release.ncl"],"state":decision,"decided":!matches!(decision,"pending"|"waiting"),"reason":"Illustrative decision; no Patchgate execution occurred","needs":if decision=="blocked" {json!([["example-prerequisite","rejected"]])}else{json!([])}}],
        "records":[{"hash":hash,"decision":decision,"by":"gate","at":kiln_web_service::feeds::rfc3339(now_ms()?.saturating_sub(index as u64*60_000)),"reason":"Synthetic example retained for UI exploration","summary":"No model or verifier ran","coverage":{"planned_pages":2,"complete":index==0,"pages":[{"decision":"accept"}]},"verification":{"results":[{"check":"synthetic-example-only"}]},"findings":[]}],
        "review_usage":{"requests":0,"max_requests":4,"elapsed_seconds":0,"timeout_seconds":30,"stop":"synthetic_example"}
    });
    Ok(json!({
        "schema":"kiln.patchgate.review.v1","synthetic":true,
        "request_identity":blake3::hash(format!("request-{decision}").as_bytes()).to_hex().to_string(),
        "observation":{"contract_version":1,"effect_id":format!("synthetic-{decision}"),"idempotency_key":hash,"candidate_id":format!("synthetic-candidate-{decision}"),"candidate_root":hash,"pipeline_revision":"synthetic-pipeline","policy_identity":hash,"provider_profile":"synthetic-patchgate-observation","native_binding_identity":hash,"report_identity":blake3::hash(&serde_json::to_vec(&native)?).to_hex().to_string(),"isolated":true,"authorizes_acceptance":false,"reviews":[{"change":{"contract_version":1,"provider_namespace":"pijul","native_kind":"change","native_id":hash},"decision":decision}]},
        "native_report":native.to_string(),"non_claims":["No Patchgate or Pijul operation ran","Synthetic UI rehearsal; no merge or release authority"]
    }))
}
