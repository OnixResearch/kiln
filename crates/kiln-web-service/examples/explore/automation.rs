//! Local automation admission examples. Notification delivery is never called.
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use hmac::{Hmac, Mac};
use kiln_adapter_github::events::DeliveryHeaders;
use kiln_web_service::automation::AutomationProfile;
use kiln_web_service::engine::Engine;
use serde_json::json;
use sha2::Sha256;

use super::{Result, accepted};
const SYNTHETIC_SECRET: &[u8] = b"synthetic-example-not-an-installed-credential";

pub fn configure(root: &Path, engine: &mut Engine) -> Result {
    let secret = root.join("synthetic-trigger.secret");
    std::fs::write(&secret, SYNTHETIC_SECRET)?;
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o600))?;
    let profile: AutomationProfile = serde_json::from_value(json!({
        "schema":"kiln.web-automation.v1","max_records":1024,"status_limit":50,"max_event_bytes":65536,"max_tick_effects":4,
        "triggers":[{"id":"example-github","project_id":"checks","reference":"refs/heads/main","repository":{"owner":"examples","repository":"source","repository_id":42,"allow_fork_pull_requests":false,"untrusted_pull_requests":"approve"},"secret_file":secret,"enabled":true,"push":true,"pull_request":true,"pull_request_base":"main"}],
        "schedules":[
            {"id":"example-interval","project_id":"docs","reference":"refs/heads/main","enabled":false,"interval_ms":3600000,"min_interval_ms":60000,"max_interval_ms":86400000,"next_due_ms":null},
            {"id":"example-calendar","project_id":"docs","reference":"refs/heads/main","enabled":false,"cron":{"expression":"H 3 * * *","timezone":"UTC"},"min_interval_ms":60000,"max_interval_ms":86400000,"next_due_ms":null}
        ],
        "notifications":[{"id":"example-withheld-notification","project_id":"checks","enabled":true,"on_success":true,"on_failure":true,"host":"localhost","port":9,"path":"/intentionally-never-delivered","addresses":["127.0.0.1"],"bearer_secret_file":null,"ca_certificate_file":null,"allow_loopback":true,"allow_private":false,"timeout_ms":1000,"max_request_bytes":8192,"max_response_bytes":128,"max_resolver_candidates":1}]
    }))?;
    engine.configure_automation(profile)?;
    Ok(())
}
pub fn awaiting_approval(engine: &mut Engine, revision: &str) -> Result<String> {
    let repository =
        json!({"id":42,"name":"source","full_name":"examples/source","owner":{"login":"examples"}});
    let fork = json!({"id":99,"name":"source-fork","full_name":"outside/source-fork","owner":{"login":"outside"}});
    let body = serde_json::to_vec(
        &json!({"action":"synchronize","repository":repository,"number":7,"pull_request":{"number":7,"state":"open","author_association":"NONE","head":{"ref":"main","sha":revision,"repo":fork},"base":{"ref":"main","sha":revision,"repo":repository}}}),
    )?;
    let mut mac = Hmac::<Sha256>::new_from_slice(SYNTHETIC_SECRET)?;
    mac.update(&body);
    let signature = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let headers = DeliveryHeaders {
        delivery: "00000000-0000-4000-8000-0000000000e1".into(),
        event: "pull_request".into(),
        signature: format!("sha256={signature}"),
    };
    let response = accepted(engine.automation_trigger("example-github", &headers, &body)?)?;
    Ok(response
        .get("run_id")
        .ok_or("Run identity absent")?
        .as_str()
        .ok_or("Synthetic untrusted intake omitted its run identity")?
        .into())
}
